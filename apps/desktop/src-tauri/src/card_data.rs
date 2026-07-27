use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use chrono::Utc;
use reqwest::header::{ACCEPT, USER_AGENT};
use rusqlite::types::Type;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::de::{self, DeserializeOwned, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::domain::{
    CardDefinition, CardFaceDefinition, DataState, DataStatus, DataUpdateProgress,
    RelatedCardComponentDefinition,
};
use crate::parser::normalize_card_name;

const SCRYFALL_COLLECTION_URL: &str = "https://api.scryfall.com/cards/collection";
const SCRYFALL_BULK_URL: &str = "https://api.scryfall.com/bulk-data/oracle-cards";
const USER_AGENT_VALUE: &str = concat!("CommanderDeckAnalyzer/", env!("CARGO_PKG_VERSION"));
const ACCEPT_VALUE: &str = "application/json;q=0.9,*/*;q=0.8";
const CARD_DATA_SCHEMA_VERSION: &str = "5";
const SCRYFALL_CARD_INGESTOR_VERSION: &str = "scryfall-oracle-cards-2";
/// Reviewed against Scryfall's public `api-types` CardFields/CardFace contract
/// at this upstream revision. Fields outside this versioned classification are
/// retained and blocked by execution coverage instead of being discarded.
pub(crate) const SCRYFALL_FIELD_CLASSIFICATION_VERSION: &str = "scryfall-card-fields/2026-07-23/api-types-c16cdfba9e09a0d3aef9ef0db6c36153a7529615+live-union/v1";
const MINIMUM_FULL_SNAPSHOT_CARDS: u64 = 25_000;
const MAXIMUM_BULK_METADATA_BYTES: usize = 1024 * 1024;
const MAXIMUM_BULK_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum CardDataError {
    #[error("Card database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Card data request failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Card data file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Card data response was invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CardDataUpdateCheck {
    pub update_available: bool,
    pub installed_version: Option<String>,
    pub available_version: Option<String>,
    pub detail: String,
}

#[derive(Debug)]
struct CardDataUpdateLocalState {
    state: DataState,
    last_updated: Option<String>,
    snapshot_sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CardRepository {
    database_path: PathBuf,
}

impl CardRepository {
    pub fn new(database_path: impl Into<PathBuf>) -> Result<Self, CardDataError> {
        let repository = Self {
            database_path: database_path.into(),
        };
        if let Some(parent) = repository.database_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = repository.open()?;
        initialize_schema(&connection)?;
        seed_basic_lands(&connection)?;
        Ok(repository)
    }

    /// Creates a repository handle for read-only update inspection. Unlike
    /// [`CardRepository::new`], this does not create directories, initialize a
    /// database, seed cards, or change SQLite journal settings.
    pub(crate) fn for_update_check(database_path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: database_path.into(),
        }
    }

    pub fn status(&self) -> Result<DataStatus, CardDataError> {
        let connection = self.open()?;
        let card_count: u64 =
            connection.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
        let last_updated = metadata(&connection, "card_data_updated_at")?;
        let source = metadata(&connection, "card_data_source")?
            .unwrap_or_else(|| "Scryfall card data".to_string());
        let snapshot_sha256 = metadata(&connection, "card_data_snapshot_sha256")?;
        let ingestor_version = metadata(&connection, "card_data_ingestor_version")?;
        let full_snapshot_is_current = ingestor_version.as_deref()
            == Some(SCRYFALL_CARD_INGESTOR_VERSION)
            && snapshot_sha256.is_some();
        let state = card_data_state(card_count, full_snapshot_is_current);
        let message = match state {
            DataState::Ready => format!("{card_count} Oracle cards are available locally."),
            DataState::Partial if card_count >= MINIMUM_FULL_SNAPSHOT_CARDS => format!(
                "{card_count} legacy card records are installed, but their deck-card filter predates the current ingestor. Refresh the full snapshot before relying on same-named card legality."
            ),
            DataState::Partial => {
                format!("{card_count} cards are cached locally; missing cards resolve on demand.")
            }
            _ => "Only built-in basic lands are available. Analyze while online or download the full card database.".into(),
        };

        Ok(DataStatus {
            state,
            card_count,
            last_updated,
            source,
            message,
            snapshot_sha256,
        })
    }

    pub fn get_many(
        &self,
        names: &[String],
    ) -> Result<HashMap<String, CardDefinition>, CardDataError> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT name, normalized_name, oracle_id, mana_value, mana_cost, type_line,
                    oracle_text, layout, colors, color_indicator, color_identity, keywords,
                    produced_mana, power, toughness, loyalty, defense, faces_json,
                    related_components_json, image_uri, legal_commander, edhrec_rank, updated_at,
                    root_mana_value, hand_modifier, life_modifier, attraction_lights,
                    commander_legality, unreviewed_fields_json, source_schema_version,
                    game_changer
             FROM cards WHERE normalized_name = ?1",
        )?;
        let mut alias_statement = connection.prepare(
            "SELECT cards.name, cards.normalized_name, cards.oracle_id, cards.mana_value,
                    cards.mana_cost, cards.type_line, cards.oracle_text, cards.layout,
                    cards.colors, cards.color_indicator, cards.color_identity, cards.keywords,
                    cards.produced_mana, cards.power, cards.toughness, cards.loyalty,
                    cards.defense, cards.faces_json, cards.related_components_json,
                    cards.image_uri, cards.legal_commander, cards.edhrec_rank, cards.updated_at,
                    cards.root_mana_value, cards.hand_modifier, cards.life_modifier,
                    cards.attraction_lights, cards.commander_legality,
                    cards.unreviewed_fields_json, cards.source_schema_version,
                    cards.game_changer
             FROM card_aliases
             JOIN cards ON cards.normalized_name = card_aliases.normalized_name
             WHERE card_aliases.alias = ?1",
        )?;
        let mut cards = HashMap::new();

        for name in names {
            let normalized = normalize_card_name(name);
            let card = statement
                .query_row([&normalized], row_to_card)
                .optional()?
                .or(alias_statement
                    .query_row([&normalized], row_to_card)
                    .optional()?);
            if let Some(card) = card {
                cards.insert(normalized, card);
            }
        }

        Ok(cards)
    }

    fn store_with_aliases(
        &self,
        records: &[(CardDefinition, Vec<String>)],
    ) -> Result<(), CardDataError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        {
            let mut statement = transaction.prepare(UPSERT_CARD_SQL)?;
            let mut delete_aliases =
                transaction.prepare("DELETE FROM card_aliases WHERE normalized_name = ?1")?;
            let mut insert_alias = transaction.prepare(
                "INSERT INTO card_aliases(alias, normalized_name) VALUES (?1, ?2)
                 ON CONFLICT(alias) DO UPDATE SET normalized_name = excluded.normalized_name",
            )?;
            for (card, aliases) in records {
                insert_card(&mut statement, card)?;
                delete_aliases.execute([&card.normalized_name])?;
                for alias in aliases {
                    let normalized_alias = normalize_card_name(alias);
                    if !normalized_alias.is_empty() && normalized_alias != card.normalized_name {
                        insert_alias.execute([&normalized_alias, &card.normalized_name])?;
                    }
                }
            }
        }
        set_metadata(
            &transaction,
            "card_data_updated_at",
            &Utc::now().to_rfc3339(),
        )?;
        set_metadata(&transaction, "card_data_source", "Scryfall API cache")?;
        transaction.commit()?;
        Ok(())
    }

    pub async fn resolve_missing(
        &self,
        names: &[String],
    ) -> Result<(Vec<CardDefinition>, Vec<String>), CardDataError> {
        let unique_names = names
            .iter()
            .filter(|name| !name.trim().is_empty())
            .map(|name| name.trim().to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        if unique_names.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }

        let client = http_client()?;
        let mut resolved = Vec::new();
        let mut unresolved = Vec::new();

        for (chunk_index, chunk) in unique_names.chunks(75).enumerate() {
            if chunk_index > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            }
            let identifiers = chunk
                .iter()
                .map(|name| ScryfallIdentifier {
                    name: scryfall_collection_lookup_name(name),
                })
                .collect::<Vec<_>>();
            let response = client
                .post(SCRYFALL_COLLECTION_URL)
                .json(&ScryfallCollectionRequest { identifiers })
                .send()
                .await?
                .error_for_status()?
                .json::<ScryfallCollectionResponse>()
                .await?;

            let records = response
                .data
                .into_iter()
                .filter_map(deck_card_with_face_aliases)
                .collect::<Vec<_>>();
            resolved.extend(records.iter().map(|(card, _)| card.clone()));
            self.store_with_aliases(&records)?;
            unresolved.extend(
                response
                    .not_found
                    .into_iter()
                    .filter_map(|identifier| identifier.name),
            );
        }

        Ok((resolved, unresolved))
    }

    pub async fn install_full_snapshot(
        &self,
        mut report: impl FnMut(DataUpdateProgress),
    ) -> Result<DataStatus, CardDataError> {
        let client = http_client()?;
        report(DataUpdateProgress {
            phase: "manifest".into(),
            completed_units: 0,
            total_units: None,
            progress: 0.01,
            detail: "Checking the latest Scryfall Oracle card snapshot…".into(),
        });

        let metadata = fetch_bulk_metadata(&client).await?;
        let download_url = reqwest::Url::parse(&metadata.download_uri).map_err(|_| {
            CardDataError::Message("Scryfall returned an invalid download URL.".into())
        })?;
        validate_bulk_download_url(&download_url)?;

        let parent = self
            .database_path
            .parent()
            .ok_or_else(|| CardDataError::Message("Card data path has no parent.".into()))?;
        std::fs::create_dir_all(parent)?;
        let download_path = parent.join("oracle-cards.download.json");
        let next_database_path = parent.join("cards.next.sqlite");
        let backup_database_path = parent.join("cards.previous.sqlite");

        let mut response = client
            .get(download_url)
            .timeout(std::time::Duration::from_secs(30 * 60))
            .send()
            .await?
            .error_for_status()?;
        let total_bytes = response.content_length();
        if total_bytes.is_some_and(|bytes| bytes > MAXIMUM_BULK_DOWNLOAD_BYTES) {
            return Err(CardDataError::Message(
                "The advertised card snapshot is unexpectedly large; the current database was left unchanged."
                    .into(),
            ));
        }
        let mut downloaded = 0u64;
        let mut download_hasher = Sha256::new();
        let mut output = tokio::fs::File::create(&download_path).await?;

        while let Some(chunk) = response.chunk().await? {
            downloaded += chunk.len() as u64;
            if downloaded > MAXIMUM_BULK_DOWNLOAD_BYTES {
                drop(output);
                let _ = tokio::fs::remove_file(&download_path).await;
                return Err(CardDataError::Message(
                    "The card snapshot exceeded the 1 GB safety limit; the current database was left unchanged."
                        .into(),
                ));
            }
            output.write_all(&chunk).await?;
            download_hasher.update(&chunk);
            let ratio = total_bytes
                .filter(|total| *total > 0)
                .map(|total| downloaded as f32 / total as f32)
                .unwrap_or(0.0);
            report(DataUpdateProgress {
                phase: "download".into(),
                completed_units: downloaded,
                total_units: total_bytes,
                progress: 0.03 + ratio * 0.52,
                detail: format!("Downloading Oracle cards: {}", format_bytes(downloaded)),
            });
        }
        output.flush().await?;
        drop(output);
        let download_sha256 = format!("{:x}", download_hasher.finalize());

        if next_database_path.exists() {
            std::fs::remove_file(&next_database_path)?;
        }
        let mut next_connection = Connection::open(&next_database_path)?;
        initialize_schema(&next_connection)?;
        let transaction = next_connection.transaction()?;
        let input = BufReader::new(File::open(&download_path)?);
        let mut processed = 0u64;
        let mut accepted = 0u64;
        {
            let mut statement = transaction.prepare(UPSERT_CARD_SQL)?;
            let mut insert_alias = transaction.prepare(
                "INSERT INTO card_aliases(alias, normalized_name) VALUES (?1, ?2)
                 ON CONFLICT(alias) DO UPDATE SET normalized_name = excluded.normalized_name",
            )?;
            deserialize_card_array(input, |raw| {
                processed += 1;
                let Some((card, aliases)) = deck_card_with_face_aliases(raw) else {
                    return Ok(());
                };
                insert_card(&mut statement, &card).map_err(|error| error.to_string())?;
                for alias in aliases {
                    let normalized_alias = normalize_card_name(&alias);
                    if !normalized_alias.is_empty() && normalized_alias != card.normalized_name {
                        insert_alias
                            .execute([&normalized_alias, &card.normalized_name])
                            .map_err(|error| error.to_string())?;
                    }
                }
                accepted += 1;
                if processed.is_multiple_of(2_000) {
                    report(DataUpdateProgress {
                        phase: "index".into(),
                        completed_units: processed,
                        total_units: None,
                        progress: (0.55 + processed as f32 / 120_000.0 * 0.40).min(0.95),
                        detail: format!(
                            "Indexing deck cards locally: {accepted} accepted from {processed} Oracle objects"
                        ),
                    });
                }
                Ok(())
            })?;
        }
        let stored: u64 =
            transaction.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
        let excluded_extras = processed.saturating_sub(accepted);
        let snapshot_updated_at = metadata
            .updated_at
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        set_metadata(&transaction, "card_data_updated_at", &snapshot_updated_at)?;
        set_metadata(
            &transaction,
            "card_data_source",
            "Scryfall Oracle Cards bulk data",
        )?;
        set_metadata(&transaction, "card_data_snapshot_sha256", &download_sha256)?;
        set_metadata(
            &transaction,
            "card_data_snapshot_card_count",
            &stored.to_string(),
        )?;
        set_metadata(
            &transaction,
            "card_data_snapshot_raw_object_count",
            &processed.to_string(),
        )?;
        set_metadata(
            &transaction,
            "card_data_snapshot_excluded_extra_count",
            &excluded_extras.to_string(),
        )?;
        set_metadata(
            &transaction,
            "card_data_ingestor_version",
            SCRYFALL_CARD_INGESTOR_VERSION,
        )?;
        transaction.commit()?;
        if stored < MINIMUM_FULL_SNAPSHOT_CARDS {
            drop(next_connection);
            let _ = std::fs::remove_file(&next_database_path);
            let _ = std::fs::remove_file(&download_path);
            return Err(CardDataError::Message(format!(
                "The downloaded snapshot contained only {stored} deck-card identities; the current database was left unchanged."
            )));
        }
        let integrity: String =
            next_connection.query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;
        if integrity != "ok" {
            drop(next_connection);
            let _ = std::fs::remove_file(&next_database_path);
            let _ = std::fs::remove_file(&download_path);
            return Err(CardDataError::Message(
                "The new card database failed its SQLite integrity check; the current database was left unchanged."
                    .into(),
            ));
        }
        next_connection.execute_batch("PRAGMA optimize;")?;
        drop(next_connection);

        report(DataUpdateProgress {
            phase: "install".into(),
            completed_units: stored,
            total_units: Some(stored),
            progress: 0.97,
            detail: "Validating and activating the new snapshot…".into(),
        });

        if backup_database_path.exists() {
            std::fs::remove_file(&backup_database_path)?;
        }
        if self.database_path.exists() {
            std::fs::rename(&self.database_path, &backup_database_path)?;
        }
        if let Err(error) = std::fs::rename(&next_database_path, &self.database_path) {
            if backup_database_path.exists() && !self.database_path.exists() {
                let _ = std::fs::rename(&backup_database_path, &self.database_path);
            }
            return Err(CardDataError::Io(error));
        }
        let _ = std::fs::remove_file(&download_path);

        report(DataUpdateProgress {
            phase: "complete".into(),
            completed_units: stored,
            total_units: Some(stored),
            progress: 1.0,
            detail: format!(
                "{stored} deck-card identities are ready for offline analysis ({excluded_extras} non-deck game pieces excluded)."
            ),
        });
        self.status()
    }

    /// Checks Scryfall's small bulk-data manifest without downloading or
    /// activating the Oracle-card snapshot.
    pub(crate) async fn check_for_update(&self) -> Result<CardDataUpdateCheck, CardDataError> {
        let current = self.update_check_local_state()?;
        let client = http_client()?;
        let metadata = fetch_bulk_metadata(&client).await?;
        let download_url = reqwest::Url::parse(&metadata.download_uri).map_err(|_| {
            CardDataError::Message("Scryfall returned an invalid download URL.".into())
        })?;
        validate_bulk_download_url(&download_url)?;
        let installed_version = matches!(current.state, DataState::Ready)
            .then(|| current.last_updated.clone())
            .flatten();
        let available_version = metadata.updated_at.clone();
        let update_available = card_update_is_available(
            current.state,
            current.snapshot_sha256.as_deref(),
            installed_version.as_deref(),
            available_version.as_deref(),
        );
        let detail = if update_available {
            if !matches!(current.state, DataState::Ready) {
                "A complete current Scryfall Oracle-card snapshot is not installed.".into()
            } else {
                match available_version.as_deref() {
                    Some(version) => {
                        format!("Scryfall reports an Oracle-card snapshot from {version}.")
                    }
                    None => "Scryfall reports an Oracle-card snapshot whose version could not be compared locally."
                        .into(),
                }
            }
        } else {
            "The installed Scryfall Oracle-card snapshot matches the current manifest.".into()
        };
        Ok(CardDataUpdateCheck {
            update_available,
            installed_version,
            available_version,
            detail,
        })
    }

    fn update_check_local_state(&self) -> Result<CardDataUpdateLocalState, CardDataError> {
        if !self.database_path.try_exists()? {
            return Ok(CardDataUpdateLocalState {
                state: DataState::Empty,
                last_updated: None,
                snapshot_sha256: None,
            });
        }
        let connection = Connection::open_with_flags(
            &self.database_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let card_count: u64 =
            connection.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
        let last_updated = metadata(&connection, "card_data_updated_at")?;
        let snapshot_sha256 = metadata(&connection, "card_data_snapshot_sha256")?;
        let ingestor_version = metadata(&connection, "card_data_ingestor_version")?;
        let full_snapshot_is_current = ingestor_version.as_deref()
            == Some(SCRYFALL_CARD_INGESTOR_VERSION)
            && snapshot_sha256.is_some();
        Ok(CardDataUpdateLocalState {
            state: card_data_state(card_count, full_snapshot_is_current),
            last_updated,
            snapshot_sha256,
        })
    }

    fn open(&self) -> Result<Connection, rusqlite::Error> {
        let connection = Connection::open(&self.database_path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        Ok(connection)
    }
}

fn card_update_is_available(
    state: DataState,
    snapshot_sha256: Option<&str>,
    installed_version: Option<&str>,
    available_version: Option<&str>,
) -> bool {
    !matches!(state, DataState::Ready)
        || snapshot_sha256.is_none()
        || available_version.is_none()
        || installed_version != available_version
}

fn card_data_state(card_count: u64, full_snapshot_is_current: bool) -> DataState {
    if card_count <= 6 {
        DataState::Empty
    } else if card_count < MINIMUM_FULL_SNAPSHOT_CARDS || !full_snapshot_is_current {
        DataState::Partial
    } else {
        DataState::Ready
    }
}

fn initialize_schema(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS cards (
            normalized_name TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            oracle_id TEXT,
            mana_value REAL NOT NULL,
            mana_cost TEXT,
            type_line TEXT NOT NULL,
            oracle_text TEXT NOT NULL,
            layout TEXT NOT NULL,
            colors TEXT NOT NULL,
            color_indicator TEXT NOT NULL,
            color_identity TEXT NOT NULL,
            keywords TEXT NOT NULL,
            produced_mana TEXT NOT NULL,
            power TEXT,
            toughness TEXT,
            loyalty TEXT,
            defense TEXT,
            faces_json TEXT NOT NULL,
            related_components_json TEXT NOT NULL,
            image_uri TEXT,
            legal_commander INTEGER NOT NULL,
            edhrec_rank INTEGER,
            updated_at TEXT NOT NULL,
            root_mana_value REAL,
            hand_modifier TEXT,
            life_modifier TEXT,
            attraction_lights TEXT NOT NULL,
            commander_legality TEXT,
            unreviewed_fields_json TEXT NOT NULL,
            source_schema_version TEXT NOT NULL,
            game_changer INTEGER
         );
         CREATE INDEX IF NOT EXISTS cards_oracle_id ON cards(oracle_id);",
    )?;
    if !column_exists(connection, "cards", "edhrec_rank")? {
        connection.execute("ALTER TABLE cards ADD COLUMN edhrec_rank INTEGER", [])?;
    }
    for (column, migration) in [
        (
            "layout",
            "ALTER TABLE cards ADD COLUMN layout TEXT NOT NULL DEFAULT ''",
        ),
        (
            "colors",
            "ALTER TABLE cards ADD COLUMN colors TEXT NOT NULL DEFAULT '[]'",
        ),
        (
            "color_indicator",
            "ALTER TABLE cards ADD COLUMN color_indicator TEXT NOT NULL DEFAULT '[]'",
        ),
        (
            "produced_mana",
            "ALTER TABLE cards ADD COLUMN produced_mana TEXT NOT NULL DEFAULT '[]'",
        ),
        ("power", "ALTER TABLE cards ADD COLUMN power TEXT"),
        ("toughness", "ALTER TABLE cards ADD COLUMN toughness TEXT"),
        ("loyalty", "ALTER TABLE cards ADD COLUMN loyalty TEXT"),
        ("defense", "ALTER TABLE cards ADD COLUMN defense TEXT"),
        (
            "faces_json",
            "ALTER TABLE cards ADD COLUMN faces_json TEXT NOT NULL DEFAULT '[]'",
        ),
        (
            "related_components_json",
            "ALTER TABLE cards ADD COLUMN related_components_json TEXT NOT NULL DEFAULT '[]'",
        ),
        (
            "root_mana_value",
            "ALTER TABLE cards ADD COLUMN root_mana_value REAL",
        ),
        (
            "hand_modifier",
            "ALTER TABLE cards ADD COLUMN hand_modifier TEXT",
        ),
        (
            "life_modifier",
            "ALTER TABLE cards ADD COLUMN life_modifier TEXT",
        ),
        (
            "attraction_lights",
            "ALTER TABLE cards ADD COLUMN attraction_lights TEXT NOT NULL DEFAULT '[]'",
        ),
        (
            "commander_legality",
            "ALTER TABLE cards ADD COLUMN commander_legality TEXT",
        ),
        (
            "unreviewed_fields_json",
            "ALTER TABLE cards ADD COLUMN unreviewed_fields_json TEXT NOT NULL DEFAULT '{}'",
        ),
        (
            "source_schema_version",
            "ALTER TABLE cards ADD COLUMN source_schema_version TEXT NOT NULL DEFAULT ''",
        ),
        (
            "game_changer",
            "ALTER TABLE cards ADD COLUMN game_changer INTEGER",
        ),
    ] {
        if !column_exists(connection, "cards", column)? {
            connection.execute(migration, [])?;
        }
    }
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS card_aliases (
            alias TEXT PRIMARY KEY NOT NULL,
            normalized_name TEXT NOT NULL,
            FOREIGN KEY(normalized_name) REFERENCES cards(normalized_name) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS card_aliases_card ON card_aliases(normalized_name);",
    )?;
    // Older snapshots keyed every Scryfall Oracle object by normalized name.
    // A same-named token could therefore overwrite the physical card (Storm
    // Crow is the known regression). Tokens, emblems, and art cards are
    // related game pieces rather than legal deck entries; remove migrated
    // top-level rows so online resolution or the next full refresh can restore
    // the real card without preserving a false legality result.
    connection.execute(
        "DELETE FROM cards
         WHERE lower(trim(type_line)) LIKE 'token %'
            OR lower(trim(layout)) IN ('token', 'double_faced_token', 'emblem', 'art_series')",
        [],
    )?;
    set_metadata(connection, "schema_version", CARD_DATA_SCHEMA_VERSION)?;
    set_metadata(
        connection,
        "scryfall_field_classification_version",
        SCRYFALL_FIELD_CLASSIFICATION_VERSION,
    )?;
    Ok(())
}

fn column_exists(
    connection: &Connection,
    table: &str,
    expected_column: &str,
) -> Result<bool, rusqlite::Error> {
    // Callers use compile-time table names. Reject anything else before
    // constructing the PRAGMA because SQLite does not bind identifiers.
    if table != "cards" {
        return Ok(false);
    }
    let mut statement = connection.prepare("PRAGMA table_info(cards)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == expected_column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn seed_basic_lands(connection: &Connection) -> Result<(), rusqlite::Error> {
    let existing: u64 = connection.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
    if existing > 0 {
        return Ok(());
    }

    let now = Utc::now().to_rfc3339();
    for (name, subtype, color, symbol) in [
        ("Plains", "Plains", "W", "{W}"),
        ("Island", "Island", "U", "{U}"),
        ("Swamp", "Swamp", "B", "{B}"),
        ("Mountain", "Mountain", "R", "{R}"),
        ("Forest", "Forest", "G", "{G}"),
        ("Wastes", "Wastes", "", "{C}"),
    ] {
        let card = CardDefinition {
            name: name.into(),
            normalized_name: normalize_card_name(name),
            oracle_id: None,
            layout: "normal".into(),
            root_mana_value: None,
            mana_value: 0.0,
            mana_cost: None,
            type_line: format!("Basic Land - {subtype}"),
            oracle_text: format!("{{T}}: Add {symbol}."),
            colors: Vec::new(),
            color_indicator: Vec::new(),
            color_identity: if color.is_empty() {
                Vec::new()
            } else {
                vec![color.into()]
            },
            keywords: Vec::new(),
            produced_mana: vec![symbol.trim_matches(['{', '}']).to_string()],
            power: None,
            toughness: None,
            loyalty: None,
            defense: None,
            hand_modifier: None,
            life_modifier: None,
            attraction_lights: Vec::new(),
            faces: Vec::new(),
            related_components: Vec::new(),
            image_uri: None,
            edhrec_rank: None,
            game_changer: None,
            commander_legality: None,
            legal_commander: true,
            unreviewed_fields: BTreeMap::new(),
            // Legacy compatibility records predate a complete upstream schema-5
            // capture and therefore remain strict-gate blockers.
            source_schema_version: String::new(),
            updated_at: now.clone(),
        };
        let mut statement = connection.prepare(UPSERT_CARD_SQL)?;
        insert_card(&mut statement, &card)?;
    }
    Ok(())
}

const UPSERT_CARD_SQL: &str = "INSERT INTO cards (
        normalized_name, name, oracle_id, mana_value, mana_cost, type_line, oracle_text,
        layout, colors, color_indicator, color_identity, keywords, produced_mana, power,
        toughness, loyalty, defense, faces_json, related_components_json, image_uri,
        legal_commander, edhrec_rank, updated_at, root_mana_value, hand_modifier,
        life_modifier, attraction_lights, commander_legality, unreviewed_fields_json,
        source_schema_version, game_changer
     ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
        ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
        ?29, ?30, ?31
     )
     ON CONFLICT(normalized_name) DO UPDATE SET
        name = excluded.name,
        oracle_id = excluded.oracle_id,
        mana_value = excluded.mana_value,
        mana_cost = excluded.mana_cost,
        type_line = excluded.type_line,
        oracle_text = excluded.oracle_text,
        layout = excluded.layout,
        colors = excluded.colors,
        color_indicator = excluded.color_indicator,
        color_identity = excluded.color_identity,
        keywords = excluded.keywords,
        produced_mana = excluded.produced_mana,
        power = excluded.power,
        toughness = excluded.toughness,
        loyalty = excluded.loyalty,
        defense = excluded.defense,
        faces_json = excluded.faces_json,
        related_components_json = excluded.related_components_json,
        image_uri = excluded.image_uri,
        legal_commander = excluded.legal_commander,
        edhrec_rank = excluded.edhrec_rank,
        updated_at = excluded.updated_at,
        root_mana_value = excluded.root_mana_value,
        hand_modifier = excluded.hand_modifier,
        life_modifier = excluded.life_modifier,
        attraction_lights = excluded.attraction_lights,
        commander_legality = excluded.commander_legality,
        unreviewed_fields_json = excluded.unreviewed_fields_json,
        source_schema_version = excluded.source_schema_version,
        game_changer = excluded.game_changer";

fn insert_card(
    statement: &mut rusqlite::Statement<'_>,
    card: &CardDefinition,
) -> Result<(), rusqlite::Error> {
    let colors = serialize_json(&card.colors)?;
    let color_indicator = serialize_json(&card.color_indicator)?;
    let color_identity = serialize_json(&card.color_identity)?;
    let keywords = serialize_json(&card.keywords)?;
    let produced_mana = serialize_json(&card.produced_mana)?;
    let faces = serialize_json(&card.faces)?;
    let related_components = serialize_json(&card.related_components)?;
    let attraction_lights = serialize_json(&card.attraction_lights)?;
    let unreviewed_fields = serialize_json(&card.unreviewed_fields)?;
    statement.execute(params![
        card.normalized_name,
        card.name,
        card.oracle_id,
        card.mana_value,
        card.mana_cost,
        card.type_line,
        card.oracle_text,
        card.layout,
        colors,
        color_indicator,
        color_identity,
        keywords,
        produced_mana,
        card.power,
        card.toughness,
        card.loyalty,
        card.defense,
        faces,
        related_components,
        card.image_uri,
        card.legal_commander as i64,
        card.edhrec_rank,
        card.updated_at,
        card.root_mana_value,
        card.hand_modifier,
        card.life_modifier,
        attraction_lights,
        card.commander_legality,
        unreviewed_fields,
        card.source_schema_version,
        card.game_changer.map(i64::from),
    ])?;
    Ok(())
}

fn row_to_card(row: &rusqlite::Row<'_>) -> Result<CardDefinition, rusqlite::Error> {
    let colors: String = row.get(8)?;
    let color_indicator: String = row.get(9)?;
    let color_identity: String = row.get(10)?;
    let keywords: String = row.get(11)?;
    let produced_mana: String = row.get(12)?;
    let faces: String = row.get(17)?;
    let related_components: String = row.get(18)?;
    let attraction_lights: String = row.get(26)?;
    let unreviewed_fields: String = row.get(28)?;
    Ok(CardDefinition {
        name: row.get(0)?,
        normalized_name: row.get(1)?,
        oracle_id: row.get(2)?,
        layout: row.get(7)?,
        root_mana_value: row.get(23)?,
        mana_value: row.get(3)?,
        mana_cost: row.get(4)?,
        type_line: row.get(5)?,
        oracle_text: row.get(6)?,
        colors: deserialize_json_column(8, &colors)?,
        color_indicator: deserialize_json_column(9, &color_indicator)?,
        color_identity: deserialize_json_column(10, &color_identity)?,
        keywords: deserialize_json_column(11, &keywords)?,
        produced_mana: deserialize_json_column(12, &produced_mana)?,
        power: row.get(13)?,
        toughness: row.get(14)?,
        loyalty: row.get(15)?,
        defense: row.get(16)?,
        hand_modifier: row.get(24)?,
        life_modifier: row.get(25)?,
        attraction_lights: deserialize_json_column(26, &attraction_lights)?,
        faces: deserialize_json_column(17, &faces)?,
        related_components: deserialize_json_column(18, &related_components)?,
        image_uri: row.get(19)?,
        legal_commander: row.get::<_, i64>(20)? != 0,
        edhrec_rank: row.get(21)?,
        game_changer: row.get::<_, Option<i64>>(30)?.map(|value| value != 0),
        commander_legality: row.get(27)?,
        unreviewed_fields: deserialize_json_column(28, &unreviewed_fields)?,
        source_schema_version: row.get(29)?,
        updated_at: row.get(22)?,
    })
}

fn serialize_json(value: &impl serde::Serialize) -> Result<String, rusqlite::Error> {
    serde_json::to_string(value)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn deserialize_json_column<T: DeserializeOwned>(
    column_index: usize,
    value: &str,
) -> Result<T, rusqlite::Error> {
    serde_json::from_str(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column_index, Type::Text, Box::new(error))
    })
}

fn metadata(connection: &Connection, key: &str) -> Result<Option<String>, rusqlite::Error> {
    connection
        .query_row("SELECT value FROM metadata WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
}

fn set_metadata(connection: &Connection, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    connection.execute(
        "INSERT INTO metadata(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

fn http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                USER_AGENT,
                USER_AGENT_VALUE.parse().expect("valid user agent"),
            );
            headers.insert(ACCEPT, ACCEPT_VALUE.parse().expect("valid accept header"));
            headers
        })
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(120))
        .build()
}

fn validate_bulk_download_url(url: &reqwest::Url) -> Result<(), CardDataError> {
    if url.scheme() != "https"
        || url.host_str() != Some("data.scryfall.io")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err(CardDataError::Message(
            "Scryfall returned a card snapshot URL outside its trusted HTTPS data host.".into(),
        ));
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct ScryfallCollectionRequest<'a> {
    identifiers: Vec<ScryfallIdentifier<'a>>,
}

#[derive(serde::Serialize)]
struct ScryfallIdentifier<'a> {
    name: &'a str,
}

#[derive(Debug, Deserialize)]
struct ScryfallCollectionResponse {
    #[serde(default)]
    data: Vec<ScryfallCard>,
    #[serde(default)]
    not_found: Vec<MissingIdentifier>,
}

#[derive(Debug, Deserialize)]
struct MissingIdentifier {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BulkDataMetadata {
    download_uri: String,
    updated_at: Option<String>,
}

async fn fetch_bulk_metadata(client: &reqwest::Client) -> Result<BulkDataMetadata, CardDataError> {
    let mut response = client
        .get(SCRYFALL_BULK_URL)
        .send()
        .await?
        .error_for_status()?;
    if response
        .content_length()
        .is_some_and(|length| length > MAXIMUM_BULK_METADATA_BYTES as u64)
    {
        return Err(CardDataError::Message(
            "Scryfall's bulk-data manifest exceeded the 1 MiB safety limit.".into(),
        ));
    }
    let mut body = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or(0)
            .min(MAXIMUM_BULK_METADATA_BYTES),
    );
    while let Some(chunk) = response.chunk().await? {
        if body.len().saturating_add(chunk.len()) > MAXIMUM_BULK_METADATA_BYTES {
            return Err(CardDataError::Message(
                "Scryfall's bulk-data manifest exceeded the 1 MiB safety limit.".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&body)?)
}

#[derive(Debug, Deserialize)]
struct ScryfallCard {
    name: String,
    oracle_id: Option<String>,
    #[serde(default)]
    layout: String,
    #[serde(default, rename = "cmc")]
    mana_value: Option<f32>,
    mana_cost: Option<String>,
    #[serde(default)]
    type_line: String,
    oracle_text: Option<String>,
    #[serde(default)]
    colors: Vec<String>,
    #[serde(default)]
    color_indicator: Vec<String>,
    #[serde(default)]
    color_identity: Vec<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    produced_mana: Vec<String>,
    hand_modifier: Option<String>,
    life_modifier: Option<String>,
    #[serde(default)]
    attraction_lights: Vec<u8>,
    power: Option<String>,
    toughness: Option<String>,
    loyalty: Option<String>,
    defense: Option<String>,
    image_uris: Option<ImageUris>,
    #[serde(default)]
    card_faces: Vec<ScryfallCardFace>,
    #[serde(default)]
    all_parts: Vec<ScryfallRelatedCard>,
    #[serde(default)]
    legalities: HashMap<String, String>,
    edhrec_rank: Option<u32>,
    game_changer: Option<bool>,
    #[serde(flatten)]
    extra_fields: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct ScryfallCardFace {
    oracle_id: Option<String>,
    #[serde(default)]
    layout: String,
    name: Option<String>,
    #[serde(default, rename = "cmc")]
    mana_value: Option<f32>,
    mana_cost: Option<String>,
    type_line: Option<String>,
    oracle_text: Option<String>,
    #[serde(default)]
    colors: Vec<String>,
    #[serde(default)]
    color_indicator: Vec<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    produced_mana: Vec<String>,
    power: Option<String>,
    toughness: Option<String>,
    loyalty: Option<String>,
    defense: Option<String>,
    hand_modifier: Option<String>,
    life_modifier: Option<String>,
    #[serde(default)]
    attraction_lights: Vec<u8>,
    edhrec_rank: Option<u32>,
    image_uris: Option<ImageUris>,
    #[serde(flatten)]
    extra_fields: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct ScryfallRelatedCard {
    id: String,
    component: String,
    name: String,
    type_line: String,
    uri: Option<String>,
    #[serde(flatten)]
    extra_fields: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct ImageUris {
    normal: Option<String>,
    small: Option<String>,
}

/// These reviewed fields affect printing, artwork, marketplace/provider
/// provenance, localized display, or non-Commander popularity only. They are
/// intentionally classified here rather than falling through as unknown.
///
/// Every rules- or Commander-analysis-bearing CardFields/CardFace property is
/// modeled explicitly above. A newly introduced top-level field therefore
/// survives in `unreviewed_fields` and blocks strict analysis until reviewed.
fn is_reviewed_non_gameplay_scryfall_field(field: &str) -> bool {
    matches!(
        field,
        "object"
            | "id"
            | "resource_id"
            | "lang"
            | "prints_search_uri"
            | "rulings_uri"
            | "scryfall_uri"
            | "uri"
            | "arena_id"
            | "mtgo_id"
            | "mtgo_foil_id"
            | "multiverse_ids"
            | "tcgplayer_id"
            | "tcgplayer_etched_id"
            | "cardmarket_id"
            | "reserved"
            | "penny_rank"
            | "foil"
            | "nonfoil"
            | "artist"
            | "artist_id"
            | "artist_ids"
            | "booster"
            | "border_color"
            | "card_back_id"
            | "collector_number"
            | "content_warning"
            | "digital"
            | "finishes"
            | "flavor_name"
            | "flavor_text"
            | "frame"
            | "frame_effects"
            | "full_art"
            | "games"
            | "highres_image"
            | "image_updated_at"
            | "illustration_id"
            | "image_status"
            | "oversized"
            | "preview"
            | "prices"
            | "printed_name"
            | "printed_text"
            | "printed_type_line"
            | "promo"
            | "promo_types"
            | "purchase_uris"
            | "rarity"
            | "related_uris"
            | "released_at"
            | "reprint"
            | "scryfall_set_uri"
            | "security_stamp"
            | "set"
            | "set_id"
            | "set_name"
            | "set_search_uri"
            | "set_type"
            | "set_uri"
            | "source"
            | "source_uri"
            | "story_spotlight"
            | "textless"
            | "variation"
            | "variation_of"
            | "watermark"
    )
}

fn unreviewed_scryfall_fields(fields: BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    fields
        .into_iter()
        .filter(|(field, _)| !is_reviewed_non_gameplay_scryfall_field(field))
        .collect()
}

impl From<ScryfallCard> for CardDefinition {
    fn from(raw: ScryfallCard) -> Self {
        let face_text = raw
            .card_faces
            .iter()
            .filter_map(|face| face.oracle_text.as_deref())
            .collect::<Vec<_>>()
            .join("\n//\n");
        let oracle_text = raw.oracle_text.unwrap_or(face_text);
        let mana_cost = raw.mana_cost.or_else(|| {
            let costs = raw
                .card_faces
                .iter()
                .filter_map(|face| face.mana_cost.as_deref())
                .collect::<Vec<_>>()
                .join(" // ");
            (!costs.is_empty()).then_some(costs)
        });
        let image_uri = raw
            .image_uris
            .as_ref()
            .and_then(|images| images.normal.clone().or(images.small.clone()))
            .or_else(|| {
                raw.card_faces.iter().find_map(|face| {
                    face.image_uris
                        .as_ref()
                        .and_then(|images| images.normal.clone().or(images.small.clone()))
                })
            });
        let commander_legality = raw.legalities.get("commander").cloned();
        let legal_commander = matches!(commander_legality.as_deref(), Some("legal" | "restricted"));
        let normalized_name = normalize_card_name(&raw.name);
        let faces = raw
            .card_faces
            .iter()
            .map(|face| CardFaceDefinition {
                oracle_id: face.oracle_id.clone(),
                layout: face.layout.clone(),
                name: face.name.clone().unwrap_or_default(),
                mana_value: face.mana_value,
                mana_cost: face.mana_cost.clone(),
                type_line: face.type_line.clone().unwrap_or_default(),
                oracle_text: face.oracle_text.clone().unwrap_or_default(),
                colors: face.colors.clone(),
                color_indicator: face.color_indicator.clone(),
                keywords: face.keywords.clone(),
                produced_mana: face.produced_mana.clone(),
                power: face.power.clone(),
                toughness: face.toughness.clone(),
                loyalty: face.loyalty.clone(),
                defense: face.defense.clone(),
                hand_modifier: face.hand_modifier.clone(),
                life_modifier: face.life_modifier.clone(),
                attraction_lights: face.attraction_lights.clone(),
                edhrec_rank: face.edhrec_rank,
                image_uri: face
                    .image_uris
                    .as_ref()
                    .and_then(|images| images.normal.clone().or_else(|| images.small.clone())),
                unreviewed_fields: unreviewed_scryfall_fields(face.extra_fields.clone()),
            })
            .collect::<Vec<_>>();
        let related_components = raw
            .all_parts
            .into_iter()
            .map(|component| RelatedCardComponentDefinition {
                id: component.id,
                component: component.component,
                name: component.name,
                type_line: component.type_line,
                uri: component.uri,
                unreviewed_fields: unreviewed_scryfall_fields(component.extra_fields),
            })
            .collect();
        let root_mana_value = raw.mana_value;
        // Keep the compatibility value useful for legacy descriptive models,
        // while strict coverage consumes the exact optional root/face values.
        let mana_value = root_mana_value
            .or_else(|| faces.first().and_then(|face| face.mana_value))
            .unwrap_or_default();

        CardDefinition {
            name: raw.name,
            normalized_name,
            oracle_id: raw.oracle_id,
            layout: raw.layout,
            root_mana_value,
            mana_value,
            mana_cost,
            type_line: raw.type_line,
            oracle_text,
            colors: raw.colors,
            color_indicator: raw.color_indicator,
            color_identity: raw.color_identity,
            keywords: raw.keywords,
            produced_mana: raw.produced_mana,
            power: raw.power,
            toughness: raw.toughness,
            loyalty: raw.loyalty,
            defense: raw.defense,
            hand_modifier: raw.hand_modifier,
            life_modifier: raw.life_modifier,
            attraction_lights: raw.attraction_lights,
            faces,
            related_components,
            image_uri,
            edhrec_rank: raw.edhrec_rank,
            game_changer: raw.game_changer,
            commander_legality,
            legal_commander,
            unreviewed_fields: unreviewed_scryfall_fields(raw.extra_fields),
            source_schema_version: SCRYFALL_FIELD_CLASSIFICATION_VERSION.into(),
            updated_at: Utc::now().to_rfc3339(),
        }
    }
}

fn card_with_face_aliases(raw: ScryfallCard) -> (CardDefinition, Vec<String>) {
    let aliases = raw
        .card_faces
        .iter()
        .filter_map(|face| face.name.clone())
        .collect::<Vec<_>>();
    (CardDefinition::from(raw), aliases)
}

fn deck_card_with_face_aliases(raw: ScryfallCard) -> Option<(CardDefinition, Vec<String>)> {
    let non_deck_layout = matches!(
        raw.layout.trim().to_ascii_lowercase().as_str(),
        "token" | "double_faced_token" | "emblem" | "art_series"
    );
    let token_type = raw
        .type_line
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("token ");
    (!non_deck_layout && !token_type).then(|| card_with_face_aliases(raw))
}

fn scryfall_collection_lookup_name(name: &str) -> &str {
    // Scryfall's single-card named endpoint accepts a complete split-card
    // name, but its collection endpoint currently returns that same identifier
    // as not-found. Either face name resolves the canonical object, whose
    // aliases are then stored for both faces and the complete name.
    name.split_once("//")
        .map(|(front, _)| front.trim())
        .filter(|front| !front.is_empty())
        .unwrap_or(name)
}

fn deserialize_card_array(
    reader: impl std::io::Read,
    callback: impl FnMut(ScryfallCard) -> Result<(), String>,
) -> Result<u64, CardDataError> {
    struct CardArrayVisitor<F> {
        callback: F,
    }

    impl<'de, F> Visitor<'de> for CardArrayVisitor<F>
    where
        F: FnMut(ScryfallCard) -> Result<(), String>,
    {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an array of Scryfall card objects")
        }

        fn visit_seq<A>(mut self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut count = 0u64;
            while let Some(card) = sequence.next_element::<ScryfallCard>()? {
                (self.callback)(card).map_err(de::Error::custom)?;
                count += 1;
            }
            Ok(count)
        }
    }

    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    let count = deserializer.deserialize_seq(CardArrayVisitor { callback })?;
    Ok(count)
}

fn format_bytes(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes < 1024 * 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / MIB)
    }
}
