use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use chrono::Utc;
use flate2::read::GzDecoder;
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
const SCRYFALL_NAMED_URL: &str = "https://api.scryfall.com/cards/named";
const SCRYFALL_DISPLAY_ALIAS_URL: &str = "https://api.scryfall.com/cards/search?q=has%3Aflavor_name&unique=prints&order=name&include_variations=true&include_extras=true";
const SCRYFALL_BULK_URL: &str = "https://api.scryfall.com/bulk-data/oracle-cards";
const USER_AGENT_VALUE: &str = concat!("CommanderDeckAnalyzer/", env!("CARGO_PKG_VERSION"));
const ACCEPT_VALUE: &str = "application/json;q=0.9,*/*;q=0.8";
pub(crate) const CARD_DATA_SCHEMA_VERSION: &str = "8";
pub(crate) const SCRYFALL_CARD_INGESTOR_VERSION: &str = "scryfall-oracle-cards-6";
/// Reviewed against Scryfall's public `api-types` CardFields/CardFace contract
/// at this upstream revision. Fields outside this versioned classification are
/// retained and blocked by execution coverage instead of being discarded.
pub(crate) const SCRYFALL_FIELD_CLASSIFICATION_VERSION: &str = "scryfall-card-fields/2026-07-28/api-types-c16cdfba9e09a0d3aef9ef0db6c36153a7529615+live-union/v3";
pub(crate) const CARD_ALIAS_RESOLUTION_VERSION: &str = "scryfall-display-aliases-3";
const MINIMUM_FULL_SNAPSHOT_CARDS: u64 = 25_000;
const MAXIMUM_BULK_METADATA_BYTES: usize = 1024 * 1024;
const MAXIMUM_BULK_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;
const MAXIMUM_DISPLAY_ALIAS_PAGE_BYTES: usize = 16 * 1024 * 1024;
const MAXIMUM_DISPLAY_ALIAS_PAGES: usize = 32;
const MAXIMUM_DISPLAY_ALIAS_RECORDS: usize = 10_000;

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
        let schema_version = metadata(&connection, "schema_version")?
            .unwrap_or_else(|| CARD_DATA_SCHEMA_VERSION.into());
        let ingestor_version = metadata(&connection, "card_data_ingestor_version")?;
        let alias_catalog_version = metadata(&connection, "card_data_alias_catalog_version")?;
        let alias_catalog_sha256 = metadata(&connection, "card_data_alias_catalog_sha256")?;
        let alias_catalog_record_count =
            metadata(&connection, "card_data_alias_catalog_record_count")?
                .and_then(|value| value.parse::<u64>().ok());
        let full_snapshot_is_current = full_snapshot_metadata_is_current(
            &schema_version,
            ingestor_version.as_deref(),
            snapshot_sha256.as_deref(),
            alias_catalog_version.as_deref(),
            alias_catalog_sha256.as_deref(),
            alias_catalog_record_count,
        );
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
            schema_version,
            ingestor_version,
            alias_catalog_version,
            alias_catalog_sha256,
            alias_catalog_record_count,
        })
    }

    pub fn get_many(
        &self,
        names: &[String],
    ) -> Result<HashMap<String, CardDefinition>, CardDataError> {
        let connection = self.open()?;
        let mut exact_name_statement = connection.prepare(
            "SELECT name, normalized_name, oracle_id, mana_value, mana_cost, type_line,
                    oracle_text, layout, colors, color_indicator, color_identity, keywords,
                    produced_mana, power, toughness, loyalty, defense, faces_json,
                    related_components_json, image_uri, legal_commander, updated_at,
                    root_mana_value, hand_modifier, life_modifier, attraction_lights,
                    commander_legality, unreviewed_fields_json, source_schema_version,
                    game_changer
             FROM cards
             WHERE exact_name = ?1",
        )?;
        let mut exact_alias_statement = connection.prepare(
            "SELECT DISTINCT
                    cards.name, cards.normalized_name, cards.oracle_id, cards.mana_value,
                    cards.mana_cost, cards.type_line, cards.oracle_text, cards.layout,
                    cards.colors, cards.color_indicator, cards.color_identity, cards.keywords,
                    cards.produced_mana, cards.power, cards.toughness, cards.loyalty,
                    cards.defense, cards.faces_json, cards.related_components_json,
                    cards.image_uri, cards.legal_commander, cards.updated_at,
                    cards.root_mana_value, cards.hand_modifier, cards.life_modifier,
                    cards.attraction_lights, cards.commander_legality,
                    cards.unreviewed_fields_json, cards.source_schema_version,
                    cards.game_changer
             FROM card_aliases
             JOIN cards ON cards.card_id = card_aliases.card_id
             WHERE card_aliases.exact_alias = ?1",
        )?;
        let mut normalized_name_statement = connection.prepare(
            "SELECT name, normalized_name, oracle_id, mana_value, mana_cost, type_line,
                    oracle_text, layout, colors, color_indicator, color_identity, keywords,
                    produced_mana, power, toughness, loyalty, defense, faces_json,
                    related_components_json, image_uri, legal_commander, updated_at,
                    root_mana_value, hand_modifier, life_modifier, attraction_lights,
                    commander_legality, unreviewed_fields_json, source_schema_version,
                    game_changer
             FROM cards
             WHERE normalized_name = ?1
               AND identity_disambiguated = 1",
        )?;
        let mut normalized_alias_statement = connection.prepare(
            "SELECT DISTINCT
                    cards.name, cards.normalized_name, cards.oracle_id, cards.mana_value,
                    cards.mana_cost, cards.type_line, cards.oracle_text, cards.layout,
                    cards.colors, cards.color_indicator, cards.color_identity, cards.keywords,
                    cards.produced_mana, cards.power, cards.toughness, cards.loyalty,
                    cards.defense, cards.faces_json, cards.related_components_json,
                    cards.image_uri, cards.legal_commander, cards.updated_at,
                    cards.root_mana_value, cards.hand_modifier, cards.life_modifier,
                    cards.attraction_lights, cards.commander_legality,
                    cards.unreviewed_fields_json, cards.source_schema_version,
                    cards.game_changer
             FROM card_aliases
             JOIN cards ON cards.card_id = card_aliases.card_id
             WHERE card_aliases.alias = ?1",
        )?;
        let mut cards = HashMap::new();

        for name in names {
            let normalized = normalize_card_name(name);
            let exact = normalize_exact_card_name(name);
            let card = match query_unique_card(&mut exact_name_statement, &exact)? {
                UniqueCardQuery::Unique(card) => Some(*card),
                UniqueCardQuery::Ambiguous => None,
                UniqueCardQuery::Missing => {
                    match query_unique_card(&mut exact_alias_statement, &exact)? {
                        UniqueCardQuery::Unique(card) => Some(*card),
                        UniqueCardQuery::Ambiguous => None,
                        UniqueCardQuery::Missing => {
                            match query_unique_card(&mut normalized_name_statement, &normalized)? {
                                UniqueCardQuery::Unique(card) => Some(*card),
                                UniqueCardQuery::Ambiguous => None,
                                UniqueCardQuery::Missing => {
                                    match query_unique_card(
                                        &mut normalized_alias_statement,
                                        &normalized,
                                    )? {
                                        UniqueCardQuery::Unique(card) => Some(*card),
                                        UniqueCardQuery::Missing | UniqueCardQuery::Ambiguous => {
                                            None
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };
            if let Some(card) = card {
                cards.insert(normalized, card);
            }
        }

        Ok(cards)
    }

    fn enrich_with_resolved_aliases(
        &self,
        records: &[(CardDefinition, Vec<String>)],
    ) -> Result<(), CardDataError> {
        self.store_records_with_aliases(records, false)
    }

    fn store_records_with_aliases(
        &self,
        records: &[(CardDefinition, Vec<String>)],
        replace_existing_cards: bool,
    ) -> Result<(), CardDataError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        {
            let mut statement = transaction.prepare(UPSERT_CARD_SQL)?;
            let mut existing_card =
                transaction.prepare("SELECT 1 FROM cards WHERE card_id = ?1")?;
            let mut insert_alias = transaction.prepare(
                "INSERT OR IGNORE INTO card_aliases(
                    exact_alias, alias, card_id, printing_id
                 ) VALUES (?1, ?2, ?3, '')",
            )?;
            for (card, aliases) in records {
                let card_id = card_storage_id(card);
                let exists = existing_card
                    .query_row([&card_id], |_| Ok(()))
                    .optional()?
                    .is_some();
                if replace_existing_cards || !exists {
                    insert_card(&mut statement, card)?;
                }
                for alias in aliases {
                    let exact_alias = normalize_exact_card_name(alias);
                    let normalized_alias = normalize_card_name(alias);
                    if !exact_alias.is_empty()
                        && exact_alias != normalize_exact_card_name(&card.name)
                        && !normalized_alias.is_empty()
                    {
                        insert_alias.execute(params![exact_alias, normalized_alias, card_id,])?;
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

            let mut records = response
                .data
                .into_iter()
                .filter_map(deck_card_with_face_aliases)
                .collect::<Vec<_>>();
            let mut chunk_unresolved = Vec::new();
            for identifier in response.not_found {
                let Some(name) = identifier.name else {
                    continue;
                };
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                let Some(raw) = fetch_scryfall_named_exact(&client, &name).await? else {
                    let Some(front_name) = alternate_multiface_front_name(&name) else {
                        chunk_unresolved.push(name);
                        continue;
                    };
                    let Some(raw) = fetch_scryfall_named_exact(&client, front_name).await? else {
                        chunk_unresolved.push(name);
                        continue;
                    };
                    let Some(record) = revalidate_scryfall_named_exact_response(&name, raw) else {
                        chunk_unresolved.push(name);
                        continue;
                    };
                    records.push(record);
                    continue;
                };
                let Some(record) = revalidate_scryfall_named_exact_response(&name, raw) else {
                    chunk_unresolved.push(name);
                    continue;
                };
                records.push(record);
            }
            resolved.extend(records.iter().map(|(card, _)| card.clone()));
            self.enrich_with_resolved_aliases(&records)?;
            unresolved.extend(chunk_unresolved);
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
        let (download_uri, payload_format) = metadata.preferred_download()?;
        let download_url = reqwest::Url::parse(download_uri).map_err(|_| {
            CardDataError::Message("Scryfall returned an invalid download URL.".into())
        })?;
        validate_bulk_download_url(&download_url)?;

        let parent = self
            .database_path
            .parent()
            .ok_or_else(|| CardDataError::Message("Card data path has no parent.".into()))?;
        std::fs::create_dir_all(parent)?;
        let download_path = parent.join(payload_format.temporary_filename());
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
        report(DataUpdateProgress {
            phase: "aliases".into(),
            completed_units: 0,
            total_units: None,
            progress: 0.54,
            detail: "Indexing current alternate printing names from Scryfall".into(),
        });
        let display_alias_catalog = fetch_display_alias_catalog(&client).await?;

        if next_database_path.exists() {
            std::fs::remove_file(&next_database_path)?;
        }
        let mut next_connection = Connection::open(&next_database_path)?;
        initialize_schema(&next_connection)?;
        let transaction = next_connection.transaction()?;
        let mut processed = 0u64;
        let mut accepted = 0u64;
        {
            let mut statement = transaction.prepare(UPSERT_CARD_SQL)?;
            let mut insert_alias = transaction.prepare(
                "INSERT OR IGNORE INTO card_aliases(
                    exact_alias, alias, card_id, printing_id
                 ) VALUES (?1, ?2, ?3, '')",
            )?;
            deserialize_card_snapshot(File::open(&download_path)?, payload_format, |raw| {
                processed += 1;
                let Some((card, aliases)) = deck_card_with_face_aliases(raw) else {
                    return Ok(());
                };
                insert_card(&mut statement, &card).map_err(|error| error.to_string())?;
                let card_id = card_storage_id(&card);
                let exact_name = normalize_exact_card_name(&card.name);
                for alias in aliases {
                    let exact_alias = normalize_exact_card_name(&alias);
                    let normalized_alias = normalize_card_name(&alias);
                    if !exact_alias.is_empty()
                        && exact_alias != exact_name
                        && !normalized_alias.is_empty()
                    {
                        insert_alias
                            .execute(params![exact_alias, normalized_alias, card_id,])
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
        let (alias_count, excluded_alias_extras, unresolved_alias_identities) =
            install_display_alias_catalog(&transaction, &display_alias_catalog)?;
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
        set_metadata(
            &transaction,
            "card_data_alias_catalog_version",
            CARD_ALIAS_RESOLUTION_VERSION,
        )?;
        set_metadata(
            &transaction,
            "card_data_alias_catalog_sha256",
            &display_alias_catalog.sha256,
        )?;
        set_metadata(
            &transaction,
            "card_data_alias_catalog_record_count",
            &display_alias_catalog.total_records.to_string(),
        )?;
        set_metadata(
            &transaction,
            "card_data_alias_count",
            &alias_count.to_string(),
        )?;
        set_metadata(
            &transaction,
            "card_data_alias_catalog_excluded_extra_count",
            &excluded_alias_extras.to_string(),
        )?;
        set_metadata(
            &transaction,
            "card_data_alias_catalog_unresolved_identity_count",
            &unresolved_alias_identities.to_string(),
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
                "{stored} deck-card identities and {alias_count} alternate names are ready for offline analysis ({excluded_extras} snapshot extras and {excluded_alias_extras} alias-catalog extras excluded)."
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
        let (download_uri, _) = metadata.preferred_download()?;
        let download_url = reqwest::Url::parse(download_uri).map_err(|_| {
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
        let schema_version = metadata(&connection, "schema_version")?
            .unwrap_or_else(|| CARD_DATA_SCHEMA_VERSION.into());
        let ingestor_version = metadata(&connection, "card_data_ingestor_version")?;
        let alias_catalog_version = metadata(&connection, "card_data_alias_catalog_version")?;
        let alias_catalog_sha256 = metadata(&connection, "card_data_alias_catalog_sha256")?;
        let alias_catalog_record_count =
            metadata(&connection, "card_data_alias_catalog_record_count")?
                .and_then(|value| value.parse::<u64>().ok());
        let full_snapshot_is_current = full_snapshot_metadata_is_current(
            &schema_version,
            ingestor_version.as_deref(),
            snapshot_sha256.as_deref(),
            alias_catalog_version.as_deref(),
            alias_catalog_sha256.as_deref(),
            alias_catalog_record_count,
        );
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

fn full_snapshot_metadata_is_current(
    schema_version: &str,
    ingestor_version: Option<&str>,
    snapshot_sha256: Option<&str>,
    alias_catalog_version: Option<&str>,
    alias_catalog_sha256: Option<&str>,
    alias_catalog_record_count: Option<u64>,
) -> bool {
    let valid_sha256 = |value: Option<&str>| {
        value.is_some_and(|digest| {
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    };
    schema_version == CARD_DATA_SCHEMA_VERSION
        && ingestor_version == Some(SCRYFALL_CARD_INGESTOR_VERSION)
        && valid_sha256(snapshot_sha256)
        && alias_catalog_version == Some(CARD_ALIAS_RESOLUTION_VERSION)
        && valid_sha256(alias_catalog_sha256)
        && alias_catalog_record_count.is_some_and(|count| count > 0)
}

fn initialize_schema(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS cards (
            card_id TEXT PRIMARY KEY NOT NULL,
            exact_name TEXT NOT NULL,
            normalized_name TEXT NOT NULL,
            identity_disambiguated INTEGER NOT NULL,
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
            updated_at TEXT NOT NULL,
            root_mana_value REAL,
            hand_modifier TEXT,
            life_modifier TEXT,
            attraction_lights TEXT NOT NULL,
            commander_legality TEXT,
            unreviewed_fields_json TEXT NOT NULL,
            source_schema_version TEXT NOT NULL,
            game_changer INTEGER
         );",
    )?;
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
    if !card_schema_is_current(connection)? {
        migrate_card_identity_schema(connection)?;
    }
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS card_aliases (
            exact_alias TEXT NOT NULL,
            alias TEXT NOT NULL,
            card_id TEXT NOT NULL,
            printing_id TEXT NOT NULL DEFAULT '',
            PRIMARY KEY(exact_alias, card_id, printing_id),
            FOREIGN KEY(card_id) REFERENCES cards(card_id) ON DELETE CASCADE
         );",
    )?;
    if !card_alias_schema_is_current(connection)? {
        migrate_card_alias_schema(connection)?;
    }
    connection.execute_batch(
        "CREATE INDEX IF NOT EXISTS cards_oracle_id ON cards(oracle_id);
         CREATE INDEX IF NOT EXISTS cards_exact_name ON cards(exact_name);
         CREATE INDEX IF NOT EXISTS cards_normalized_name ON cards(normalized_name);
         CREATE INDEX IF NOT EXISTS card_aliases_exact ON card_aliases(exact_alias);
         CREATE INDEX IF NOT EXISTS card_aliases_normalized ON card_aliases(alias);
         CREATE INDEX IF NOT EXISTS card_aliases_card ON card_aliases(card_id);",
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

fn migrate_card_identity_schema(connection: &Connection) -> Result<(), rusqlite::Error> {
    let aliases_exist = table_exists(connection, "card_aliases")?;
    connection.execute_batch("PRAGMA foreign_keys = OFF; BEGIN IMMEDIATE;")?;
    let migration = (|| {
        if aliases_exist {
            connection.execute("ALTER TABLE card_aliases RENAME TO card_aliases_legacy", [])?;
        }
        connection.execute_batch(
            "ALTER TABLE cards RENAME TO cards_legacy;
             CREATE TABLE cards (
                card_id TEXT PRIMARY KEY NOT NULL,
                exact_name TEXT NOT NULL,
                normalized_name TEXT NOT NULL,
                identity_disambiguated INTEGER NOT NULL,
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
             INSERT OR REPLACE INTO cards (
                card_id, exact_name, normalized_name, identity_disambiguated,
                name, oracle_id, mana_value, mana_cost, type_line, oracle_text,
                layout, colors, color_indicator, color_identity, keywords,
                produced_mana, power, toughness, loyalty, defense, faces_json,
                related_components_json, image_uri, legal_commander, updated_at,
                root_mana_value, hand_modifier, life_modifier, attraction_lights,
                commander_legality, unreviewed_fields_json, source_schema_version,
                game_changer
             )
             SELECT
                CASE
                    WHEN oracle_id IS NOT NULL AND trim(oracle_id) <> ''
                        THEN 'oracle:' || lower(trim(oracle_id))
                    ELSE 'legacy:' || normalized_name
                END,
                lower(trim(name)),
                normalized_name,
                0,
                name,
                oracle_id,
                mana_value,
                mana_cost,
                type_line,
                oracle_text,
                layout,
                colors,
                color_indicator,
                color_identity,
                keywords,
                produced_mana,
                power,
                toughness,
                loyalty,
                defense,
                faces_json,
                related_components_json,
                image_uri,
                legal_commander,
                updated_at,
                root_mana_value,
                hand_modifier,
                life_modifier,
                attraction_lights,
                commander_legality,
                unreviewed_fields_json,
                source_schema_version,
                game_changer
             FROM cards_legacy;
             CREATE TABLE card_aliases (
                exact_alias TEXT NOT NULL,
                alias TEXT NOT NULL,
                card_id TEXT NOT NULL,
                printing_id TEXT NOT NULL DEFAULT '',
                PRIMARY KEY(exact_alias, card_id, printing_id),
                FOREIGN KEY(card_id) REFERENCES cards(card_id) ON DELETE CASCADE
             );",
        )?;
        if aliases_exist {
            connection.execute_batch(
                "INSERT OR IGNORE INTO card_aliases(
                    exact_alias, alias, card_id, printing_id
                 )
                 SELECT
                    legacy_alias.alias,
                    legacy_alias.alias,
                    CASE
                        WHEN legacy_card.oracle_id IS NOT NULL
                             AND trim(legacy_card.oracle_id) <> ''
                            THEN 'oracle:' || lower(trim(legacy_card.oracle_id))
                        ELSE 'legacy:' || legacy_card.normalized_name
                    END,
                    ''
                 FROM card_aliases_legacy AS legacy_alias
                 JOIN cards_legacy AS legacy_card
                   ON legacy_card.normalized_name = legacy_alias.normalized_name;
                 DROP TABLE card_aliases_legacy;",
            )?;
        }
        connection.execute_batch("DROP TABLE cards_legacy; COMMIT;")?;
        Ok(())
    })();
    if migration.is_err() {
        let _ = connection.execute_batch("ROLLBACK;");
    }
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    migration
}

fn migrate_card_alias_schema(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "PRAGMA foreign_keys = OFF;
         BEGIN IMMEDIATE;
         ALTER TABLE card_aliases RENAME TO card_aliases_legacy;
         CREATE TABLE card_aliases (
            exact_alias TEXT NOT NULL,
            alias TEXT NOT NULL,
            card_id TEXT NOT NULL,
            printing_id TEXT NOT NULL DEFAULT '',
            PRIMARY KEY(exact_alias, card_id, printing_id),
            FOREIGN KEY(card_id) REFERENCES cards(card_id) ON DELETE CASCADE
         );
         INSERT OR IGNORE INTO card_aliases(exact_alias, alias, card_id, printing_id)
         SELECT legacy.alias, legacy.alias, cards.card_id, ''
         FROM card_aliases_legacy AS legacy
         JOIN cards ON cards.normalized_name = legacy.normalized_name
         WHERE (
            SELECT COUNT(*)
            FROM cards AS candidate
            WHERE candidate.normalized_name = legacy.normalized_name
         ) = 1;
         DROP TABLE card_aliases_legacy;
         COMMIT;
         PRAGMA foreign_keys = ON;",
    )
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

fn table_exists(connection: &Connection, table: &str) -> Result<bool, rusqlite::Error> {
    if !matches!(table, "cards" | "card_aliases") {
        return Ok(false);
    }
    connection.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE type = 'table' AND name = ?1
         )",
        [table],
        |row| row.get(0),
    )
}

fn card_schema_is_current(connection: &Connection) -> Result<bool, rusqlite::Error> {
    let mut statement = connection.prepare("PRAGMA table_info(cards)")?;
    let columns = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
    })?;
    let columns = columns.collect::<Result<Vec<_>, _>>()?;
    let primary_key = columns
        .iter()
        .filter(|(_, position)| *position > 0)
        .cloned()
        .collect::<Vec<_>>();
    Ok(primary_key == [("card_id".to_string(), 1)]
        && columns.iter().any(|(name, _)| name == "exact_name")
        && columns.iter().any(|(name, _)| name == "normalized_name")
        && columns
            .iter()
            .any(|(name, _)| name == "identity_disambiguated"))
}

fn card_alias_schema_is_current(connection: &Connection) -> Result<bool, rusqlite::Error> {
    let mut statement = connection.prepare("PRAGMA table_info(card_aliases)")?;
    let columns = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
    })?;
    let primary_key = columns
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, position)| *position > 0)
        .collect::<Vec<_>>();
    Ok(primary_key
        == [
            ("exact_alias".to_string(), 1),
            ("card_id".to_string(), 2),
            ("printing_id".to_string(), 3),
        ])
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
            type_line: format!("Basic Land \u{2014} {subtype}"),
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
            game_changer: None,
            commander_legality: None,
            legal_commander: true,
            unreviewed_fields: BTreeMap::new(),
            // Bundled compatibility fixtures predate a complete upstream
            // field capture and therefore remain strict-gate blockers.
            source_schema_version: String::new(),
            updated_at: now.clone(),
        };
        let mut statement = connection.prepare(UPSERT_CARD_SQL)?;
        insert_card(&mut statement, &card)?;
    }
    Ok(())
}

const UPSERT_CARD_SQL: &str = "INSERT INTO cards (
        card_id, exact_name, normalized_name, identity_disambiguated, name,
        oracle_id, mana_value, mana_cost, type_line, oracle_text, layout, colors,
        color_indicator, color_identity, keywords, produced_mana, power,
        toughness, loyalty, defense, faces_json, related_components_json,
        image_uri, legal_commander, updated_at, root_mana_value, hand_modifier,
        life_modifier, attraction_lights, commander_legality,
        unreviewed_fields_json, source_schema_version, game_changer
     ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
        ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
        ?29, ?30, ?31, ?32, ?33
     )
     ON CONFLICT(card_id) DO UPDATE SET
        exact_name = excluded.exact_name,
        normalized_name = excluded.normalized_name,
        identity_disambiguated = excluded.identity_disambiguated,
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
    let card_id = card_storage_id(card);
    let exact_name = normalize_exact_card_name(&card.name);
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
        card_id,
        exact_name,
        card.normalized_name,
        1,
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

fn card_storage_id(card: &CardDefinition) -> String {
    if let Some(oracle_id) = card
        .oracle_id
        .as_deref()
        .map(str::trim)
        .filter(|oracle_id| !oracle_id.is_empty())
    {
        return format!("oracle:{}", oracle_id.to_ascii_lowercase());
    }

    let face_oracle_ids = card
        .faces
        .iter()
        .filter_map(|face| face.oracle_id.as_deref())
        .map(str::trim)
        .filter(|oracle_id| !oracle_id.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    if !face_oracle_ids.is_empty() {
        return format!(
            "face-oracles:{}:{}",
            card.layout.trim().to_ascii_lowercase(),
            face_oracle_ids.join(":")
        );
    }

    let fallback = serde_json::to_vec(&(
        normalize_exact_card_name(&card.name),
        card.layout.trim().to_ascii_lowercase(),
        card.type_line.trim(),
        card.oracle_text.as_str(),
    ))
    .expect("card storage identity material is serializable");
    format!("legacy:{}", sha256_hex(&fallback))
}

fn normalize_exact_card_name(name: &str) -> String {
    let mut normalized = String::new();
    let mut pending_space = false;
    for character in name.trim().chars().flat_map(char::to_lowercase) {
        if character.is_whitespace() {
            pending_space = !normalized.is_empty();
        } else {
            if pending_space {
                normalized.push(' ');
                pending_space = false;
            }
            normalized.push(character);
        }
    }
    normalized
}

enum UniqueCardQuery {
    Missing,
    Unique(Box<CardDefinition>),
    Ambiguous,
}

fn query_unique_card(
    statement: &mut rusqlite::Statement<'_>,
    lookup_key: &str,
) -> Result<UniqueCardQuery, rusqlite::Error> {
    let mut rows = statement.query([lookup_key])?;
    let Some(first) = rows.next()? else {
        return Ok(UniqueCardQuery::Missing);
    };
    let card = row_to_card(first)?;
    if rows.next()?.is_some() {
        return Ok(UniqueCardQuery::Ambiguous);
    }
    Ok(UniqueCardQuery::Unique(Box::new(card)))
}

fn row_to_card(row: &rusqlite::Row<'_>) -> Result<CardDefinition, rusqlite::Error> {
    let colors: String = row.get(8)?;
    let color_indicator: String = row.get(9)?;
    let color_identity: String = row.get(10)?;
    let keywords: String = row.get(11)?;
    let produced_mana: String = row.get(12)?;
    let faces: String = row.get(17)?;
    let related_components: String = row.get(18)?;
    let attraction_lights: String = row.get(25)?;
    let unreviewed_fields: String = row.get(27)?;
    Ok(CardDefinition {
        name: row.get(0)?,
        normalized_name: row.get(1)?,
        oracle_id: row.get(2)?,
        layout: row.get(7)?,
        root_mana_value: row.get(22)?,
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
        hand_modifier: row.get(23)?,
        life_modifier: row.get(24)?,
        attraction_lights: deserialize_json_column(25, &attraction_lights)?,
        faces: deserialize_json_column(17, &faces)?,
        related_components: deserialize_json_column(18, &related_components)?,
        image_uri: row.get(19)?,
        legal_commander: row.get::<_, i64>(20)? != 0,
        game_changer: row.get::<_, Option<i64>>(29)?.map(|value| value != 0),
        commander_legality: row.get(26)?,
        unreviewed_fields: deserialize_json_column(27, &unreviewed_fields)?,
        source_schema_version: row.get(28)?,
        updated_at: row.get(21)?,
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
struct ScryfallDisplayAliasPage {
    total_cards: usize,
    has_more: bool,
    next_page: Option<String>,
    #[serde(default)]
    data: Vec<ScryfallDisplayAliasCard>,
}

#[derive(Debug, Deserialize)]
struct ScryfallDisplayAliasCard {
    id: String,
    name: String,
    oracle_id: Option<String>,
    #[serde(default)]
    layout: String,
    #[serde(default)]
    type_line: String,
    flavor_name: Option<String>,
    printed_name: Option<String>,
    #[serde(default)]
    card_faces: Vec<ScryfallDisplayAliasFace>,
}

#[derive(Debug, Deserialize)]
struct ScryfallDisplayAliasFace {
    oracle_id: Option<String>,
    name: Option<String>,
    flavor_name: Option<String>,
    printed_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct DisplayAliasCatalogRecord {
    printing_id: String,
    canonical_name: String,
    oracle_ids: Vec<String>,
    layout: String,
    type_line: String,
    aliases: Vec<String>,
}

#[derive(Debug)]
struct DisplayAliasCatalog {
    records: Vec<DisplayAliasCatalogRecord>,
    sha256: String,
    total_records: usize,
}

async fn fetch_scryfall_named_exact(
    client: &reqwest::Client,
    name: &str,
) -> Result<Option<ScryfallCard>, CardDataError> {
    let mut endpoint = reqwest::Url::parse(SCRYFALL_NAMED_URL)
        .expect("the built-in Scryfall named endpoint is valid");
    endpoint.query_pairs_mut().append_pair("exact", name);
    let response = client.get(endpoint).send().await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(Some(response.error_for_status()?.json().await?))
}

async fn fetch_display_alias_catalog(
    client: &reqwest::Client,
) -> Result<DisplayAliasCatalog, CardDataError> {
    let mut next_page = Some(
        reqwest::Url::parse(SCRYFALL_DISPLAY_ALIAS_URL)
            .expect("the built-in Scryfall display-alias endpoint is valid"),
    );
    let mut expected_total = None;
    let mut printing_ids = HashSet::new();
    let mut records = Vec::new();
    let mut pages = 0usize;

    while let Some(endpoint) = next_page.take() {
        pages += 1;
        if pages > MAXIMUM_DISPLAY_ALIAS_PAGES {
            return Err(CardDataError::Message(format!(
                "Scryfall's display-name catalog exceeded the {MAXIMUM_DISPLAY_ALIAS_PAGES}-page safety limit."
            )));
        }
        validate_display_alias_page_url(&endpoint)?;
        if pages > 1 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let response = client.get(endpoint).send().await?.error_for_status()?;
        let page: ScryfallDisplayAliasPage = read_bounded_json_response(
            response,
            MAXIMUM_DISPLAY_ALIAS_PAGE_BYTES,
            "display-name catalog page",
        )
        .await?;

        match expected_total {
            Some(total) if total != page.total_cards => {
                return Err(CardDataError::Message(
                    "Scryfall's display-name catalog changed total size during pagination; the current database was left unchanged."
                        .into(),
                ));
            }
            None => expected_total = Some(page.total_cards),
            _ => {}
        }
        if page.total_cards > MAXIMUM_DISPLAY_ALIAS_RECORDS {
            return Err(CardDataError::Message(format!(
                "Scryfall's display-name catalog exceeded the {MAXIMUM_DISPLAY_ALIAS_RECORDS}-record safety limit."
            )));
        }

        for raw in page.data {
            if raw.id.trim().is_empty() || !printing_ids.insert(raw.id.clone()) {
                return Err(CardDataError::Message(
                    "Scryfall's display-name catalog contained a missing or duplicate printing id; the current database was left unchanged."
                        .into(),
                ));
            }
            let mut oracle_ids = raw
                .oracle_id
                .iter()
                .filter(|value| !value.trim().is_empty())
                .cloned()
                .collect::<Vec<_>>();
            if oracle_ids.is_empty() {
                oracle_ids.extend(
                    raw.card_faces
                        .iter()
                        .filter_map(|face| face.oracle_id.as_ref())
                        .filter(|value| !value.trim().is_empty())
                        .cloned(),
                );
            }
            oracle_ids.sort();
            oracle_ids.dedup();

            let mut aliases = Vec::new();
            aliases.extend(raw.flavor_name.iter().cloned());
            aliases.extend(raw.printed_name.iter().cloned());
            for face in &raw.card_faces {
                aliases.extend(face.name.iter().cloned());
                aliases.extend(face.flavor_name.iter().cloned());
                aliases.extend(face.printed_name.iter().cloned());
            }
            let mut aliases = deduplicate_aliases(aliases);
            aliases.sort();
            records.push(DisplayAliasCatalogRecord {
                printing_id: raw.id,
                canonical_name: raw.name,
                oracle_ids,
                layout: raw.layout,
                type_line: raw.type_line,
                aliases,
            });
        }

        next_page = match (page.has_more, page.next_page) {
            (true, Some(next)) => Some(reqwest::Url::parse(&next).map_err(|_| {
                CardDataError::Message(
                    "Scryfall returned an invalid display-name catalog page URL.".into(),
                )
            })?),
            (true, None) => {
                return Err(CardDataError::Message(
                    "Scryfall ended display-name catalog pagination without a next page; the current database was left unchanged."
                        .into(),
                ));
            }
            (false, None) => None,
            (false, Some(_)) => {
                return Err(CardDataError::Message(
                    "Scryfall returned a next display-name page after marking the catalog complete; the current database was left unchanged."
                        .into(),
                ));
            }
        };
    }

    let total_records = expected_total.unwrap_or_default();
    if records.len() != total_records || printing_ids.len() != total_records {
        return Err(CardDataError::Message(format!(
            "Scryfall's display-name catalog declared {total_records} records but delivered {} unique records; the current database was left unchanged.",
            printing_ids.len()
        )));
    }
    records.sort_by(|left, right| left.printing_id.cmp(&right.printing_id));
    let sha256 = sha256_hex(&serde_json::to_vec(&(
        CARD_ALIAS_RESOLUTION_VERSION,
        SCRYFALL_DISPLAY_ALIAS_URL,
        &records,
    ))?);
    Ok(DisplayAliasCatalog {
        records,
        sha256,
        total_records,
    })
}

fn install_display_alias_catalog(
    transaction: &rusqlite::Transaction<'_>,
    catalog: &DisplayAliasCatalog,
) -> Result<(u64, u64, u64), CardDataError> {
    let targets_by_oracle = deck_card_targets_by_oracle_identity(transaction)?;
    let exact_names_by_card_id = deck_card_exact_names_by_id(transaction)?;
    let mut insert_alias = transaction.prepare(
        "INSERT OR IGNORE INTO card_aliases(
            exact_alias, alias, card_id, printing_id
         ) VALUES (?1, ?2, ?3, ?4)",
    )?;
    let mut excluded_non_deck = 0u64;
    let mut unresolved_identity_count = 0u64;

    for record in &catalog.records {
        if !is_deck_card_identity(&record.layout, &record.type_line) {
            excluded_non_deck += 1;
            continue;
        }
        let canonical = normalize_exact_card_name(&record.canonical_name);
        let mut targets = BTreeSet::new();
        let mut missing_identity = record.oracle_ids.is_empty();
        for oracle_id in &record.oracle_ids {
            let Some(identity_targets) = targets_by_oracle.get(oracle_id.trim()) else {
                missing_identity = true;
                continue;
            };
            for target in identity_targets {
                targets.insert(target.clone());
            }
        }
        let target = targets.iter().next();
        if missing_identity
            || targets.len() != 1
            || target.and_then(|target| exact_names_by_card_id.get(target)) != Some(&canonical)
        {
            unresolved_identity_count = unresolved_identity_count.saturating_add(1);
            continue;
        }
        let target = targets
            .into_iter()
            .next()
            .expect("one alias catalog target was established");
        for alias in &record.aliases {
            let exact_alias = normalize_exact_card_name(alias);
            let normalized_alias = normalize_card_name(alias);
            if !exact_alias.is_empty() && exact_alias != canonical && !normalized_alias.is_empty() {
                insert_alias.execute(params![
                    exact_alias,
                    normalized_alias,
                    target,
                    record.printing_id,
                ])?;
            }
        }
    }

    let alias_count = transaction.query_row("SELECT COUNT(*) FROM card_aliases", [], |row| {
        row.get::<_, u64>(0)
    })?;
    Ok((alias_count, excluded_non_deck, unresolved_identity_count))
}

fn deck_card_targets_by_oracle_identity(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<BTreeMap<String, BTreeSet<String>>, CardDataError> {
    let mut statement = transaction.prepare(
        "SELECT card_id, oracle_id, faces_json
         FROM cards
         ORDER BY card_id",
    )?;
    let mut rows = statement.query([])?;
    let mut targets = BTreeMap::<String, BTreeSet<String>>::new();
    while let Some(row) = rows.next()? {
        let card_id: String = row.get(0)?;
        let root_oracle_id: Option<String> = row.get(1)?;
        let faces_json: String = row.get(2)?;
        let faces = serde_json::from_str::<Vec<CardFaceDefinition>>(&faces_json)?;
        for oracle_id in root_oracle_id
            .iter()
            .chain(faces.iter().filter_map(|face| face.oracle_id.as_ref()))
            .map(|oracle_id| oracle_id.trim())
            .filter(|oracle_id| !oracle_id.is_empty())
        {
            targets
                .entry(oracle_id.to_string())
                .or_default()
                .insert(card_id.clone());
        }
    }
    Ok(targets)
}

fn deck_card_exact_names_by_id(
    transaction: &rusqlite::Transaction<'_>,
) -> Result<BTreeMap<String, String>, CardDataError> {
    let mut statement =
        transaction.prepare("SELECT card_id, exact_name FROM cards ORDER BY card_id")?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

async fn read_bounded_json_response<T: DeserializeOwned>(
    mut response: reqwest::Response,
    maximum_bytes: usize,
    label: &str,
) -> Result<T, CardDataError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes as u64)
    {
        return Err(CardDataError::Message(format!(
            "Scryfall's {label} exceeded the {maximum_bytes}-byte safety limit."
        )));
    }
    let mut body = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or(0)
            .min(maximum_bytes),
    );
    while let Some(chunk) = response.chunk().await? {
        if body.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err(CardDataError::Message(format!(
                "Scryfall's {label} exceeded the {maximum_bytes}-byte safety limit."
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&body)?)
}

fn validate_display_alias_page_url(url: &reqwest::Url) -> Result<(), CardDataError> {
    if url.scheme() != "https"
        || url.host_str() != Some("api.scryfall.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
        || url.path() != "/cards/search"
    {
        return Err(CardDataError::Message(
            "Scryfall returned a display-name catalog URL outside its trusted HTTPS search endpoint."
                .into(),
        ));
    }
    let mut query = BTreeMap::<String, String>::new();
    for (key, value) in url.query_pairs() {
        if query.insert(key.into_owned(), value.into_owned()).is_some() {
            return Err(CardDataError::Message(
                "Scryfall returned duplicate display-name catalog query fields.".into(),
            ));
        }
    }
    for (field, expected) in [
        ("q", "has:flavor_name"),
        ("unique", "prints"),
        ("order", "name"),
        ("include_variations", "true"),
        ("include_extras", "true"),
    ] {
        if query.get(field).map(String::as_str) != Some(expected) {
            return Err(CardDataError::Message(format!(
                "Scryfall changed the required `{field}` display-name catalog query field."
            )));
        }
    }
    for (field, value) in &query {
        let valid = match field.as_str() {
            "q" | "unique" | "order" | "include_variations" | "include_extras" => true,
            "format" => value == "json",
            "include_multilingual" => value == "false",
            "page" => value
                .parse::<usize>()
                .is_ok_and(|page| (2..=MAXIMUM_DISPLAY_ALIAS_PAGES).contains(&page)),
            _ => false,
        };
        if !valid {
            return Err(CardDataError::Message(format!(
                "Scryfall returned an unexpected `{field}` display-name catalog query field."
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BulkPayloadFormat {
    JsonArray,
    JsonLinesGzip,
}

impl BulkPayloadFormat {
    fn temporary_filename(self) -> &'static str {
        match self {
            Self::JsonArray => "oracle-cards.download.json",
            Self::JsonLinesGzip => "oracle-cards.download.jsonl.gz",
        }
    }
}

#[derive(Debug, Deserialize)]
struct BulkDataMetadata {
    #[serde(default)]
    download_uri: Option<String>,
    #[serde(default)]
    jsonl_download_uri: Option<String>,
    updated_at: Option<String>,
}

impl BulkDataMetadata {
    fn preferred_download(&self) -> Result<(&str, BulkPayloadFormat), CardDataError> {
        if let Some(uri) = self
            .jsonl_download_uri
            .as_deref()
            .filter(|uri| !uri.trim().is_empty())
        {
            return Ok((uri, BulkPayloadFormat::JsonLinesGzip));
        }
        if let Some(uri) = self
            .download_uri
            .as_deref()
            .filter(|uri| !uri.trim().is_empty())
        {
            return Ok((uri, BulkPayloadFormat::JsonArray));
        }
        Err(CardDataError::Message(
            "Scryfall's bulk-data manifest did not include a supported download URL.".into(),
        ))
    }
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
    flavor_name: Option<String>,
    printed_name: Option<String>,
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
    flavor_name: Option<String>,
    printed_name: Option<String>,
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
        .filter(|(field, value)| {
            !is_reviewed_non_gameplay_scryfall_field(field)
                && !is_bounded_numeric_rank_metadata(field, value)
        })
        .collect()
}

fn is_bounded_numeric_rank_metadata(field: &str, value: &Value) -> bool {
    field.ends_with("_rank")
        && (value.is_null()
            || value
                .as_u64()
                .is_some_and(|rank| u32::try_from(rank).is_ok()))
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
    let aliases = scryfall_card_aliases(&raw);
    (CardDefinition::from(raw), aliases)
}

fn scryfall_card_aliases(raw: &ScryfallCard) -> Vec<String> {
    let mut aliases = Vec::new();
    aliases.extend(raw.flavor_name.iter().cloned());
    aliases.extend(raw.printed_name.iter().cloned());
    for face in &raw.card_faces {
        aliases.extend(face.name.iter().cloned());
        aliases.extend(face.flavor_name.iter().cloned());
        aliases.extend(face.printed_name.iter().cloned());
    }
    deduplicate_aliases(aliases)
}

fn deduplicate_aliases(aliases: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    aliases
        .into_iter()
        .filter(|alias| {
            let exact = normalize_exact_card_name(alias);
            !exact.is_empty() && seen.insert(exact)
        })
        .collect()
}

fn deck_card_with_face_aliases(raw: ScryfallCard) -> Option<(CardDefinition, Vec<String>)> {
    is_deck_card_identity(&raw.layout, &raw.type_line).then(|| card_with_face_aliases(raw))
}

fn revalidate_scryfall_named_exact_response(
    requested_name: &str,
    raw: ScryfallCard,
) -> Option<(CardDefinition, Vec<String>)> {
    let (card, mut aliases) = deck_card_with_face_aliases(raw)?;
    let requested = normalize_exact_card_name(requested_name);
    let returned_name_matches = !requested.is_empty()
        && (normalize_exact_card_name(&card.name) == requested
            || aliases
                .iter()
                .any(|alias| normalize_exact_card_name(alias) == requested)
            || multiface_names_are_equivalent(requested_name, &card.name));
    if !returned_name_matches {
        return None;
    }
    aliases.push(requested_name.to_string());
    Some((card, deduplicate_aliases(aliases)))
}

fn alternate_multiface_front_name(name: &str) -> Option<&str> {
    let components = multiface_name_components(name)?;
    let separator = name.find('/')?;
    let front = name[..separator].trim();
    (!front.is_empty() && components.len() == 2).then_some(front)
}

fn multiface_names_are_equivalent(left: &str, right: &str) -> bool {
    let Some(left) = multiface_name_components(left) else {
        return false;
    };
    let Some(right) = multiface_name_components(right) else {
        return false;
    };
    left == right
}

fn multiface_name_components(name: &str) -> Option<Vec<String>> {
    let components = name
        .split('/')
        .map(str::trim)
        .filter(|component| !component.is_empty())
        .map(normalize_exact_card_name)
        .collect::<Vec<_>>();
    (components.len() == 2 && components.iter().all(|component| !component.is_empty()))
        .then_some(components)
}

fn is_deck_card_identity(layout: &str, type_line: &str) -> bool {
    let non_deck_layout = matches!(
        layout.trim().to_ascii_lowercase().as_str(),
        "token" | "double_faced_token" | "emblem" | "art_series"
    );
    let token_type = type_line
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("token ");
    !non_deck_layout && !token_type
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

fn deserialize_card_json_lines(
    reader: impl std::io::Read,
    mut callback: impl FnMut(ScryfallCard) -> Result<(), String>,
) -> Result<u64, CardDataError> {
    let mut count = 0u64;
    for card in serde_json::Deserializer::from_reader(reader).into_iter::<ScryfallCard>() {
        callback(card?).map_err(CardDataError::Message)?;
        count = count.saturating_add(1);
    }
    Ok(count)
}

fn deserialize_card_snapshot(
    input: File,
    format: BulkPayloadFormat,
    callback: impl FnMut(ScryfallCard) -> Result<(), String>,
) -> Result<u64, CardDataError> {
    match format {
        BulkPayloadFormat::JsonArray => deserialize_card_array(BufReader::new(input), callback),
        BulkPayloadFormat::JsonLinesGzip => {
            let decoder = GzDecoder::new(BufReader::new(input));
            deserialize_card_json_lines(BufReader::new(decoder), callback)
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes < 1024 * 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / MIB)
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
