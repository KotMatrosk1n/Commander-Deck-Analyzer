//! Local, updateable Commander Spellbook bulk snapshot index.
//!
//! Commander Spellbook publishes `variants.json.gz`, a large catalog that is
//! intentionally downloaded at runtime rather than redistributed with the app.
//! This module keeps the network and catalog facts separate from analyzer
//! conclusions:
//!
//! * the HTTPS endpoint is exact-allowlisted and redirects are disabled;
//! * compressed and decompressed byte limits are enforced while streaming;
//! * each variant is decoded and inserted independently, so the full JSON
//!   document is never resident in memory;
//! * a complete next SQLite database is checked before it replaces the live
//!   database, with rollback if activation fails;
//! * matching respects card quantities and command-zone requirements; and
//! * relevance and table lethality remain explicit, conservative
//!   classifications rather than treating every infinite result as a win.
//!
//! The upstream project currently publishes an ETag and Last-Modified value but
//! no signed manifest or checksum. We therefore retain those values and a local
//! SHA-256 digest for reproducibility, while reporting the authenticity basis
//! honestly as HTTPS transport rather than a cryptographic publisher signature.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::Utc;
use flate2::read::GzDecoder;
use reqwest::header::{
    ACCEPT, CONTENT_ENCODING, CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED,
    USER_AGENT,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use url::Url;

pub const SPELLBOOK_BULK_SNAPSHOT_URL: &str =
    "https://json.commanderspellbook.com/variants.json.gz";
pub const COMBO_STORE_SCHEMA_VERSION: &str = "1";
pub const COMBO_STORE_MATCH_VERSION: &str = "spellbook-local-match-v1";

const SNAPSHOT_HOST: &str = "json.commanderspellbook.com";
const SNAPSHOT_PATH: &str = "/variants.json.gz";
const USER_AGENT_VALUE: &str = concat!("CommanderDeckAnalyzer/", env!("CARGO_PKG_VERSION"));
const ACCEPT_VALUE: &str = "application/json,application/gzip;q=0.9,*/*;q=0.1";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const DATABASE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_MAX_COMPRESSED_BYTES: u64 = 128 * 1024 * 1024;
const DEFAULT_MAX_DECOMPRESSED_BYTES: u64 = 1024 * 1024 * 1024;
const DEFAULT_MIN_VARIANTS: u64 = 1_000;
const DEFAULT_MAX_VARIANTS: u64 = 1_000_000;
const DEFAULT_MAX_ALIASES: u64 = 1_000_000;
const MAX_COMPONENTS_PER_VARIANT: usize = 256;
const MAX_VARIANT_ID_BYTES: usize = 256;
const MAX_NAME_BYTES: usize = 1_024;
const MAX_SHORT_TEXT_BYTES: usize = 8 * 1024;
const MAX_LONG_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_DECK_ENTRIES: usize = 2_000;
const MAX_HEADER_VALUE_BYTES: usize = 4 * 1024;
const CATALOG_FORMAT: &str = "commander-spellbook-variants-json-v1";

static DOWNLOAD_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, thiserror::Error)]
pub enum ComboStoreError {
    #[error("Combo database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Commander Spellbook snapshot request failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Combo snapshot file error: {0}")]
    Io(#[from] io::Error),
    #[error("Commander Spellbook snapshot JSON was invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("The Commander Spellbook snapshot endpoint is invalid: {0}")]
    Url(#[from] url::ParseError),
    #[error("The Commander Spellbook snapshot endpoint did not pass the exact HTTPS allowlist.")]
    InvalidEndpoint,
    #[error("Commander Spellbook returned HTTP {0}.")]
    ProviderStatus(reqwest::StatusCode),
    #[error("Commander Spellbook returned unsupported content type {0:?}.")]
    UnexpectedContentType(Option<String>),
    #[error("Commander Spellbook returned unsupported content encoding {0:?}.")]
    UnexpectedContentEncoding(Option<String>),
    #[error(
        "The compressed Commander Spellbook snapshot exceeded the {limit_bytes} byte safety limit."
    )]
    CompressedTooLarge { limit_bytes: u64 },
    #[error(
        "The decompressed Commander Spellbook snapshot exceeded the {limit_bytes} byte safety limit."
    )]
    DecompressedTooLarge { limit_bytes: u64 },
    #[error("The downloaded file is not a gzip snapshot.")]
    InvalidGzip,
    #[error("The Commander Spellbook snapshot failed validation: {0}")]
    InvalidSnapshot(String),
    #[error("The snapshot SHA-256 did not match the expected digest.")]
    HashMismatch,
    #[error("Combo database schema {found:?} is incompatible with supported schema {expected}.")]
    IncompatibleSchema {
        found: Option<String>,
        expected: &'static str,
    },
    #[error("The new combo database failed its SQLite integrity check: {0}")]
    Integrity(String),
    #[error("Combo database update coordination failed because a worker lock was poisoned.")]
    Coordination,
    #[error("The background combo-index worker failed: {0}")]
    Worker(String),
}

/// Resource limits for one bulk snapshot.
///
/// Production callers should use [`SnapshotLimits::default`]. Exposing the
/// values makes offline import policy auditable and lets tests exercise failure
/// paths with tiny fixtures.
#[derive(Debug, Clone, Copy)]
pub struct SnapshotLimits {
    pub max_compressed_bytes: u64,
    pub max_decompressed_bytes: u64,
    pub min_variants: u64,
    pub max_variants: u64,
    pub max_aliases: u64,
}

impl Default for SnapshotLimits {
    fn default() -> Self {
        Self {
            max_compressed_bytes: DEFAULT_MAX_COMPRESSED_BYTES,
            max_decompressed_bytes: DEFAULT_MAX_DECOMPRESSED_BYTES,
            min_variants: DEFAULT_MIN_VARIANTS,
            max_variants: DEFAULT_MAX_VARIANTS,
            max_aliases: DEFAULT_MAX_ALIASES,
        }
    }
}

/// HTTP/file provenance retained with an installed catalog.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotSource {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    /// Optional publisher-provided digest for a future signed/checksummed
    /// manifest. Commander Spellbook does not currently publish one.
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComboStoreStatus {
    pub ready: bool,
    pub schema_version: String,
    pub upstream_version: Option<String>,
    pub upstream_timestamp: Option<String>,
    pub installed_at: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub snapshot_sha256: Option<String>,
    pub compressed_bytes: Option<u64>,
    pub decompressed_bytes: Option<u64>,
    pub variant_count: u64,
    pub alias_count: u64,
    pub authenticity_basis: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ComboDataUpdateCheck {
    pub update_available: bool,
    pub installed_version: Option<String>,
    pub available_version: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComboUpdateProgress {
    pub phase: String,
    pub completed_units: u64,
    pub total_units: Option<u64>,
    pub progress: f32,
    pub detail: String,
}

pub type ComboUpdateReporter = Arc<dyn Fn(ComboUpdateProgress) + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum ComboUpdateOutcome {
    Installed { status: ComboStoreStatus },
    NotModified { status: ComboStoreStatus },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComboDeckCard {
    pub name: String,
    pub oracle_id: Option<String>,
    pub quantity: u32,
    pub is_commander: bool,
}

impl ComboDeckCard {
    pub fn new(name: impl Into<String>, quantity: u32, is_commander: bool) -> Self {
        Self {
            name: name.into(),
            oracle_id: None,
            quantity,
            is_commander,
        }
    }
}

/// A template match resolved by a future Scryfall-query/template evaluator.
///
/// The store never guesses that a free-form template is satisfied. If callers
/// do not provide a resolution, the result explicitly reports it as unresolved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTemplate {
    pub template_id: u64,
    pub quantity: u32,
    pub commander_quantity: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MatchRelevance {
    Relevant,
    Borderline,
    NotRelevant,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TableLethality {
    /// A produced feature explicitly says the game is won or every opponent
    /// loses. This is documentary evidence, not a rules-engine proof.
    DocumentedTableWin,
    /// The output names unbounded damage/life loss to every opponent, but game
    /// state, prevention, replacement effects, and target legality remain.
    LikelyTableLethal,
    /// The line is unbounded but only produces resources/engine output and
    /// therefore still needs a payoff or conversion condition.
    RequiresPayoffOrConversion,
    /// The catalog output is insufficient to classify table lethality.
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TemplateMatchState {
    NotRequired,
    Satisfied,
    Unresolved,
    Unsatisfied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalCardRequirement {
    pub name: String,
    pub normalized_name: String,
    pub oracle_id: Option<String>,
    pub quantity: u32,
    pub must_be_commander: bool,
    pub zone_locations: Vec<String>,
    pub battlefield_state: Option<String>,
    pub exile_state: Option<String>,
    pub library_state: Option<String>,
    pub graveyard_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalTemplateRequirement {
    pub id: u64,
    pub name: String,
    pub scryfall_query: Option<String>,
    pub quantity: u32,
    pub must_be_commander: bool,
    pub zone_locations: Vec<String>,
    pub battlefield_state: Option<String>,
    pub exile_state: Option<String>,
    pub library_state: Option<String>,
    pub graveyard_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalProducedFeature {
    pub id: u64,
    pub name: String,
    pub quantity: u32,
    pub uncountable: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalComboMatch {
    pub variant_id: String,
    pub status: String,
    pub bracket_tag: Option<String>,
    pub identity: String,
    pub commander_legal: Option<bool>,
    pub cards: Vec<LocalCardRequirement>,
    pub templates: Vec<LocalTemplateRequirement>,
    pub template_match: TemplateMatchState,
    pub produces: Vec<LocalProducedFeature>,
    pub mana_needed: Option<String>,
    pub mana_value_needed: Option<u32>,
    /// Bulk snapshots do not expose whether the stated mana is a proven
    /// minimum, so consumers must treat it as a reported requirement.
    pub mana_minimum_confirmed: bool,
    pub easy_prerequisites: Option<String>,
    pub notable_prerequisites: Option<String>,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub popularity: Option<u64>,
    pub relevance: MatchRelevance,
    pub table_lethality: TableLethality,
    pub has_unbounded_result: bool,
}

#[derive(Debug, Clone)]
pub struct ComboStore {
    database_path: PathBuf,
    coordination: Arc<RwLock<()>>,
}

impl ComboStore {
    pub fn new(database_path: impl Into<PathBuf>) -> Result<Self, ComboStoreError> {
        let store = Self {
            database_path: database_path.into(),
            coordination: Arc::new(RwLock::new(())),
        };
        if let Some(parent) = store.database_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = store.open()?;
        initialize_schema(&connection)?;
        Ok(store)
    }

    #[allow(dead_code)] // Retained for diagnostics and offline-import tooling.
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn status(&self) -> Result<ComboStoreStatus, ComboStoreError> {
        let _guard = self
            .coordination
            .read()
            .map_err(|_| ComboStoreError::Coordination)?;
        let connection = self.open()?;
        status_from_connection(&connection)
    }

    /// Performs a conditional provider request and stops after response
    /// metadata. A modified response body is never downloaded, indexed, or
    /// activated until the user separately confirms installation.
    pub(crate) async fn check_for_update(&self) -> Result<ComboDataUpdateCheck, ComboStoreError> {
        let endpoint = Url::parse(SPELLBOOK_BULK_SNAPSHOT_URL)?;
        validate_snapshot_endpoint(&endpoint)?;
        let current = self.status_for_update_check()?;
        let client = snapshot_http_client()?;
        let mut request = client.get(endpoint).timeout(CHECK_TIMEOUT);
        if current.ready {
            if let Some(etag) = current.etag.as_deref() {
                request = request.header(IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = current.last_modified.as_deref() {
                request = request.header(IF_MODIFIED_SINCE, last_modified);
            }
        }
        let response = request.send().await?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(combo_update_check_result(&current, true, None));
        }
        if !response.status().is_success() {
            return Err(ComboStoreError::ProviderStatus(response.status()));
        }
        validate_response_headers(&response)?;
        let response_etag = bounded_header(&response, ETAG)?;
        let response_last_modified = bounded_header(&response, LAST_MODIFIED)?;
        let validators_match = combo_response_validators_match(
            &current,
            response_etag.as_deref(),
            response_last_modified.as_deref(),
        );
        let available_version = response_etag.or(response_last_modified);
        // The response body is intentionally not consumed by an update check.
        drop(response);
        Ok(combo_update_check_result(
            &current,
            validators_match,
            available_version,
        ))
    }

    /// Downloads and installs the published snapshot without blocking Tokio's
    /// async worker while gzip/JSON/SQLite indexing runs.
    pub async fn update_from_network(
        &self,
        reporter: Option<ComboUpdateReporter>,
    ) -> Result<ComboUpdateOutcome, ComboStoreError> {
        let endpoint = Url::parse(SPELLBOOK_BULK_SNAPSHOT_URL)?;
        validate_snapshot_endpoint(&endpoint)?;
        let current = self.status()?;
        emit_progress(
            &reporter,
            ComboUpdateProgress {
                phase: "manifest".into(),
                completed_units: 0,
                total_units: None,
                progress: 0.01,
                detail: "Checking the Commander Spellbook combo catalog…".into(),
            },
        );

        let client = snapshot_http_client()?;
        let mut request = client.get(endpoint).timeout(DOWNLOAD_TIMEOUT);
        if current.ready {
            if let Some(etag) = current.etag.as_deref() {
                request = request.header(IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = current.last_modified.as_deref() {
                request = request.header(IF_MODIFIED_SINCE, last_modified);
            }
        }
        let mut response = request.send().await?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            emit_progress(
                &reporter,
                ComboUpdateProgress {
                    phase: "complete".into(),
                    completed_units: current.variant_count,
                    total_units: Some(current.variant_count),
                    progress: 1.0,
                    detail: "The local combo catalog is already current.".into(),
                },
            );
            return Ok(ComboUpdateOutcome::NotModified { status: current });
        }
        if !response.status().is_success() {
            return Err(ComboStoreError::ProviderStatus(response.status()));
        }
        validate_response_headers(&response)?;

        let limits = SnapshotLimits::default();
        let total_bytes = response.content_length();
        if total_bytes.is_some_and(|bytes| bytes > limits.max_compressed_bytes) {
            return Err(ComboStoreError::CompressedTooLarge {
                limit_bytes: limits.max_compressed_bytes,
            });
        }
        let source = SnapshotSource {
            etag: bounded_header(&response, ETAG)?,
            last_modified: bounded_header(&response, LAST_MODIFIED)?,
            expected_sha256: None,
        };
        let download_path = self.unique_download_path()?;
        let mut output = tokio::fs::File::create(&download_path).await?;
        let mut downloaded = 0u64;
        let download_result: Result<(), ComboStoreError> = loop {
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break Ok(()),
                Err(error) => break Err(error.into()),
            };
            downloaded = match downloaded.checked_add(chunk.len() as u64) {
                Some(total) if total <= limits.max_compressed_bytes => total,
                _ => {
                    break Err(ComboStoreError::CompressedTooLarge {
                        limit_bytes: limits.max_compressed_bytes,
                    });
                }
            };
            if let Err(error) = output.write_all(&chunk).await {
                break Err(error.into());
            }
            let ratio = total_bytes
                .filter(|total| *total > 0)
                .map(|total| downloaded as f32 / total as f32)
                .unwrap_or(0.0);
            emit_progress(
                &reporter,
                ComboUpdateProgress {
                    phase: "download".into(),
                    completed_units: downloaded,
                    total_units: total_bytes,
                    progress: 0.02 + ratio.clamp(0.0, 1.0) * 0.48,
                    detail: format!(
                        "Downloading the combo catalog \u{2014} {}",
                        format_bytes(downloaded)
                    ),
                },
            );
        };
        if download_result.is_ok()
            && let Err(error) = output.flush().await
        {
            drop(output);
            let _ = tokio::fs::remove_file(&download_path).await;
            return Err(error.into());
        }
        drop(output);
        if let Err(error) = download_result {
            let _ = tokio::fs::remove_file(&download_path).await;
            return Err(error);
        }

        let store = self.clone();
        let worker_path = download_path.clone();
        let worker_reporter = reporter.clone();
        let worker_result = tokio::task::spawn_blocking(move || {
            store.install_from_gzip_path_with_limits(&worker_path, source, limits, worker_reporter)
        })
        .await
        .map_err(|error| ComboStoreError::Worker(error.to_string()));
        let _ = tokio::fs::remove_file(&download_path).await;
        let status = worker_result??;
        emit_progress(
            &reporter,
            ComboUpdateProgress {
                phase: "complete".into(),
                completed_units: status.variant_count,
                total_units: Some(status.variant_count),
                progress: 1.0,
                detail: format!(
                    "{} Commander Spellbook variants are ready offline.",
                    status.variant_count
                ),
            },
        );
        Ok(ComboUpdateOutcome::Installed { status })
    }

    /// Installs a previously downloaded runtime snapshot using production
    /// safety limits. This performs synchronous CPU and disk work; callers on
    /// an async runtime should invoke it through `spawn_blocking`.
    #[allow(dead_code)] // Public offline snapshot-install API; network updates use the limited variant.
    pub fn install_from_gzip_path(
        &self,
        snapshot_path: &Path,
        source: SnapshotSource,
    ) -> Result<ComboStoreStatus, ComboStoreError> {
        self.install_from_gzip_path_with_limits(
            snapshot_path,
            source,
            SnapshotLimits::default(),
            None,
        )
    }

    fn install_from_gzip_path_with_limits(
        &self,
        snapshot_path: &Path,
        source: SnapshotSource,
        limits: SnapshotLimits,
        reporter: Option<ComboUpdateReporter>,
    ) -> Result<ComboStoreStatus, ComboStoreError> {
        validate_limits(limits)?;
        validate_snapshot_source(&source)?;
        let _guard = self
            .coordination
            .write()
            .map_err(|_| ComboStoreError::Coordination)?;
        let inspected = inspect_compressed_file(snapshot_path, limits.max_compressed_bytes)?;
        if let Some(expected) = source.expected_sha256.as_deref()
            && !constant_time_digest_eq(expected, &inspected.sha256)
        {
            return Err(ComboStoreError::HashMismatch);
        }

        let paths = ActivationPaths::for_database(&self.database_path)?;
        remove_file_if_exists(&paths.next)?;
        let build_result = self.build_next_database(
            snapshot_path,
            &paths.next,
            &source,
            &inspected,
            limits,
            reporter,
        );
        if let Err(error) = build_result {
            let _ = remove_file_if_exists(&paths.next);
            return Err(error);
        }
        activate_database(&paths)?;
        let connection = self.open()?;
        status_from_connection(&connection)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_next_database(
        &self,
        snapshot_path: &Path,
        next_path: &Path,
        source: &SnapshotSource,
        inspected: &InspectedCompressedFile,
        limits: SnapshotLimits,
        reporter: Option<ComboUpdateReporter>,
    ) -> Result<(), ComboStoreError> {
        let mut connection = Connection::open(next_path)?;
        configure_build_connection(&connection)?;
        initialize_schema(&connection)?;
        let transaction = connection.transaction()?;
        let file = File::open(snapshot_path)?;
        let compressed = BufReader::with_capacity(128 * 1024, file);
        let gzip = GzDecoder::new(compressed);
        let exceeded = Rc::new(Cell::new(false));
        let decompressed_bytes = Rc::new(Cell::new(0u64));
        let limited = DecompressedLimitReader::new(
            gzip,
            limits.max_decompressed_bytes,
            Rc::clone(&exceeded),
            Rc::clone(&decompressed_bytes),
        );
        let buffered = BufReader::with_capacity(256 * 1024, limited);

        let mut writer = SqliteSnapshotWriter::new(&transaction, limits, reporter)?;
        let parsed = deserialize_snapshot(buffered, &mut writer);
        if exceeded.get() {
            return Err(ComboStoreError::DecompressedTooLarge {
                limit_bytes: limits.max_decompressed_bytes,
            });
        }
        let header = parsed?;
        writer.validate_counts()?;
        let variant_count = writer.variant_count;
        let duplicate_variant_rows = writer.duplicate_variant_rows;
        let alias_count = writer.alias_count;
        drop(writer);

        validate_snapshot_header(&header)?;
        set_metadata(&transaction, "schema_version", COMBO_STORE_SCHEMA_VERSION)?;
        set_metadata(&transaction, "catalog_format", CATALOG_FORMAT)?;
        set_metadata(&transaction, "upstream_version", &header.version)?;
        set_metadata(&transaction, "upstream_timestamp", &header.timestamp)?;
        set_metadata(
            &transaction,
            "installed_at",
            &Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        )?;
        set_optional_metadata(&transaction, "etag", source.etag.as_deref())?;
        set_optional_metadata(
            &transaction,
            "last_modified",
            source.last_modified.as_deref(),
        )?;
        set_metadata(&transaction, "snapshot_sha256", &inspected.sha256)?;
        set_metadata(
            &transaction,
            "compressed_bytes",
            &inspected.bytes.to_string(),
        )?;
        set_metadata(
            &transaction,
            "decompressed_bytes",
            &decompressed_bytes.get().to_string(),
        )?;
        set_metadata(&transaction, "variant_count", &variant_count.to_string())?;
        set_metadata(
            &transaction,
            "duplicate_variant_rows",
            &duplicate_variant_rows.to_string(),
        )?;
        set_metadata(&transaction, "alias_count", &alias_count.to_string())?;
        let authenticity_basis = if duplicate_variant_rows == 0 {
            "Exact-allowlisted HTTPS transport plus locally computed SHA-256; no publisher-signed manifest".into()
        } else {
            format!(
                "Exact-allowlisted HTTPS transport plus locally computed SHA-256; no publisher-signed manifest; {duplicate_variant_rows} semantically identical duplicate variant row{} deterministically stored once",
                if duplicate_variant_rows == 1 {
                    " was"
                } else {
                    "s were"
                }
            )
        };
        set_metadata(&transaction, "authenticity_basis", &authenticity_basis)?;
        transaction.commit()?;

        let integrity: String =
            connection.query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(ComboStoreError::Integrity(integrity));
        }
        let foreign_key_errors: u64 =
            connection.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })?;
        if foreign_key_errors != 0 {
            return Err(ComboStoreError::Integrity(format!(
                "{foreign_key_errors} foreign-key violations"
            )));
        }
        connection.execute_batch("ANALYZE; PRAGMA optimize;")?;
        Ok(())
    }

    /// Returns variants whose explicit card ingredients are all present.
    ///
    /// Template ingredients are never silently assumed. They are classified as
    /// satisfied, unsatisfied, or unresolved in each result. Use
    /// [`ComboStore::find_fully_satisfied_matches`] when only fully resolved
    /// variants are useful to the caller.
    pub fn find_matches(
        &self,
        cards: &[ComboDeckCard],
        resolved_templates: &[ResolvedTemplate],
    ) -> Result<Vec<LocalComboMatch>, ComboStoreError> {
        validate_deck_input(cards, resolved_templates)?;
        let _guard = self
            .coordination
            .read()
            .map_err(|_| ComboStoreError::Coordination)?;
        let connection = self.open()?;
        let inventory = DeckInventory::from_entries(&connection, cards)?;
        let template_inventory = TemplateInventory::from_entries(resolved_templates);
        let candidate_ids = candidate_variant_ids(&connection, &inventory, &template_inventory)?;
        let mut matches = Vec::new();
        for variant_id in candidate_ids {
            if let Some(candidate) =
                load_match(&connection, &variant_id, &inventory, &template_inventory)?
            {
                matches.push(candidate);
            }
        }
        matches.sort_by(|left, right| {
            let left_size = left.cards.iter().map(|card| card.quantity).sum::<u32>();
            let right_size = right.cards.iter().map(|card| card.quantity).sum::<u32>();
            left_size
                .cmp(&right_size)
                .then_with(|| right.popularity.cmp(&left.popularity))
                .then_with(|| left.variant_id.cmp(&right.variant_id))
        });
        Ok(matches)
    }

    pub fn find_fully_satisfied_matches(
        &self,
        cards: &[ComboDeckCard],
        resolved_templates: &[ResolvedTemplate],
    ) -> Result<Vec<LocalComboMatch>, ComboStoreError> {
        Ok(self
            .find_matches(cards, resolved_templates)?
            .into_iter()
            .filter(|candidate| {
                matches!(
                    candidate.template_match,
                    TemplateMatchState::NotRequired | TemplateMatchState::Satisfied
                )
            })
            .collect())
    }

    #[allow(dead_code)] // Retained for future deep-link and diagnostic lookup.
    pub fn resolve_variant_id(&self, id_or_alias: &str) -> Result<Option<String>, ComboStoreError> {
        let id = id_or_alias.trim();
        if id.is_empty() || id.len() > MAX_VARIANT_ID_BYTES {
            return Ok(None);
        }
        let _guard = self
            .coordination
            .read()
            .map_err(|_| ComboStoreError::Coordination)?;
        let connection = self.open()?;
        if connection
            .query_row("SELECT 1 FROM variants WHERE id = ?1", [id], |_| Ok(()))
            .optional()?
            .is_some()
        {
            return Ok(Some(id.to_string()));
        }
        Ok(connection
            .query_row(
                "SELECT variant_id FROM variant_aliases WHERE alias_id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    fn open(&self) -> Result<Connection, ComboStoreError> {
        let connection = Connection::open(&self.database_path)?;
        connection.busy_timeout(DATABASE_BUSY_TIMEOUT)?;
        connection.execute_batch(
            "PRAGMA journal_mode = DELETE;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        Ok(connection)
    }

    fn status_for_update_check(&self) -> Result<ComboStoreStatus, ComboStoreError> {
        let _guard = self
            .coordination
            .read()
            .map_err(|_| ComboStoreError::Coordination)?;
        if !self.database_path.try_exists()? {
            return Ok(empty_combo_store_status());
        }
        let connection = Connection::open_with_flags(
            &self.database_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.busy_timeout(DATABASE_BUSY_TIMEOUT)?;
        status_from_connection(&connection)
    }

    fn unique_download_path(&self) -> Result<PathBuf, ComboStoreError> {
        let parent = self.database_path.parent().ok_or_else(|| {
            ComboStoreError::InvalidSnapshot("the combo database path has no parent".into())
        })?;
        let sequence = DOWNLOAD_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Ok(parent.join(format!(
            "combos.download.{}.{}.json.gz",
            std::process::id(),
            sequence
        )))
    }
}

fn combo_update_check_result(
    current: &ComboStoreStatus,
    not_modified: bool,
    available_version: Option<String>,
) -> ComboDataUpdateCheck {
    let installed_version = current.ready.then(|| {
        current
            .upstream_version
            .clone()
            .or_else(|| current.upstream_timestamp.clone())
            .or_else(|| current.etag.clone())
            .or_else(|| current.last_modified.clone())
    });
    let installed_version = installed_version.flatten();
    let update_available = !current.ready || !not_modified;
    ComboDataUpdateCheck {
        update_available,
        installed_version: installed_version.clone(),
        available_version: available_version.or_else(|| installed_version.clone()),
        detail: if !current.ready {
            "The Commander Spellbook catalog is not installed.".into()
        } else if not_modified {
            "The installed Commander Spellbook catalog matches the current provider response."
                .into()
        } else {
            "Commander Spellbook reports a possibly changed catalog. Content will be downloaded and validated only after confirmation."
                .into()
        },
    }
}

fn combo_response_validators_match(
    current: &ComboStoreStatus,
    response_etag: Option<&str>,
    response_last_modified: Option<&str>,
) -> bool {
    if !current.ready {
        return false;
    }
    let mut compared = false;
    for (installed, available) in [
        (current.etag.as_deref(), response_etag),
        (current.last_modified.as_deref(), response_last_modified),
    ] {
        if let (Some(installed), Some(available)) = (installed, available) {
            compared = true;
            if installed != available {
                return false;
            }
        }
    }
    compared
}

fn empty_combo_store_status() -> ComboStoreStatus {
    ComboStoreStatus {
        ready: false,
        schema_version: COMBO_STORE_SCHEMA_VERSION.into(),
        upstream_version: None,
        upstream_timestamp: None,
        installed_at: None,
        etag: None,
        last_modified: None,
        snapshot_sha256: None,
        compressed_bytes: None,
        decompressed_bytes: None,
        variant_count: 0,
        alias_count: 0,
        authenticity_basis: "No downloaded Commander Spellbook snapshot is installed.".into(),
    }
}

pub fn validate_snapshot_endpoint(endpoint: &Url) -> Result<(), ComboStoreError> {
    let valid = endpoint.scheme() == "https"
        && endpoint
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case(SNAPSHOT_HOST))
        && endpoint.port_or_known_default() == Some(443)
        && endpoint.port().is_none()
        && endpoint.username().is_empty()
        && endpoint.password().is_none()
        && endpoint.path() == SNAPSHOT_PATH
        && endpoint.query().is_none()
        && endpoint.fragment().is_none();
    if valid {
        Ok(())
    } else {
        Err(ComboStoreError::InvalidEndpoint)
    }
}

fn snapshot_http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                USER_AGENT,
                USER_AGENT_VALUE.parse().expect("valid user agent"),
            );
            headers.insert(ACCEPT, ACCEPT_VALUE.parse().expect("valid accept header"));
            headers
        })
        .build()
}

fn validate_response_headers(response: &reqwest::Response) -> Result<(), ComboStoreError> {
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let media_type = content_type
        .as_deref()
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !matches!(
        media_type.as_str(),
        "application/json" | "application/gzip" | "application/x-gzip" | "application/octet-stream"
    ) {
        return Err(ComboStoreError::UnexpectedContentType(content_type));
    }

    let encoding = response
        .headers()
        .get(CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_ascii_lowercase());
    if encoding
        .as_deref()
        .is_some_and(|value| value != "gzip" && value != "identity")
    {
        return Err(ComboStoreError::UnexpectedContentEncoding(encoding));
    }
    Ok(())
}

fn bounded_header(
    response: &reqwest::Response,
    name: reqwest::header::HeaderName,
) -> Result<Option<String>, ComboStoreError> {
    let Some(value) = response.headers().get(name) else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| {
        ComboStoreError::InvalidSnapshot("an HTTP metadata header was not valid text".into())
    })?;
    validate_bounded_text("HTTP metadata header", value, MAX_HEADER_VALUE_BYTES)?;
    if value.chars().any(char::is_control) {
        return Err(ComboStoreError::InvalidSnapshot(
            "an HTTP metadata header contained control characters".into(),
        ));
    }
    Ok(Some(value.to_string()))
}

fn emit_progress(reporter: &Option<ComboUpdateReporter>, progress: ComboUpdateProgress) {
    if let Some(reporter) = reporter {
        reporter(progress);
    }
}

#[derive(Debug)]
struct ActivationPaths {
    live: PathBuf,
    next: PathBuf,
    previous: PathBuf,
}

impl ActivationPaths {
    fn for_database(live: &Path) -> Result<Self, ComboStoreError> {
        let parent = live.parent().ok_or_else(|| {
            ComboStoreError::InvalidSnapshot("the combo database path has no parent".into())
        })?;
        let file_name = live
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                ComboStoreError::InvalidSnapshot("the combo database filename is invalid".into())
            })?;
        Ok(Self {
            live: live.to_path_buf(),
            next: parent.join(format!("{file_name}.next")),
            previous: parent.join(format!("{file_name}.previous")),
        })
    }
}

fn activate_database(paths: &ActivationPaths) -> Result<(), ComboStoreError> {
    remove_file_if_exists(&paths.previous)?;
    let had_live = paths.live.exists();
    if had_live {
        fs::rename(&paths.live, &paths.previous)?;
    }
    if let Err(error) = fs::rename(&paths.next, &paths.live) {
        if had_live && !paths.live.exists() {
            let _ = fs::rename(&paths.previous, &paths.live);
        }
        return Err(error.into());
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<(), io::Error> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn configure_build_connection(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.busy_timeout(DATABASE_BUSY_TIMEOUT)?;
    connection.execute_batch(
        "PRAGMA journal_mode = DELETE;
         PRAGMA synchronous = FULL;
         PRAGMA foreign_keys = ON;
         PRAGMA temp_store = MEMORY;
         PRAGMA cache_size = -65536;",
    )?;
    Ok(())
}

fn initialize_schema(connection: &Connection) -> Result<(), ComboStoreError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS variants (
            id TEXT PRIMARY KEY NOT NULL,
            status TEXT NOT NULL,
            identity TEXT NOT NULL,
            mana_needed TEXT,
            mana_value_needed INTEGER,
            easy_prerequisites TEXT,
            notable_prerequisites TEXT,
            description TEXT,
            notes TEXT,
            popularity INTEGER,
            spoiler INTEGER NOT NULL,
            bracket_tag TEXT,
            commander_legal INTEGER,
            variant_count INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS variant_cards (
            variant_id TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            spellbook_card_id INTEGER NOT NULL,
            oracle_id TEXT,
            normalized_name TEXT NOT NULL,
            display_name TEXT NOT NULL,
            quantity INTEGER NOT NULL CHECK(quantity > 0),
            must_be_commander INTEGER NOT NULL,
            zone_locations_json TEXT NOT NULL,
            battlefield_state TEXT,
            exile_state TEXT,
            library_state TEXT,
            graveyard_state TEXT,
            PRIMARY KEY(variant_id, ordinal),
            FOREIGN KEY(variant_id) REFERENCES variants(id) ON DELETE CASCADE
                DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TABLE IF NOT EXISTS variant_templates (
            variant_id TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            template_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            scryfall_query TEXT,
            quantity INTEGER NOT NULL CHECK(quantity > 0),
            must_be_commander INTEGER NOT NULL,
            zone_locations_json TEXT NOT NULL,
            battlefield_state TEXT,
            exile_state TEXT,
            library_state TEXT,
            graveyard_state TEXT,
            PRIMARY KEY(variant_id, ordinal),
            FOREIGN KEY(variant_id) REFERENCES variants(id) ON DELETE CASCADE
                DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TABLE IF NOT EXISTS variant_features (
            variant_id TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            feature_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            quantity INTEGER NOT NULL CHECK(quantity > 0),
            uncountable INTEGER NOT NULL,
            status TEXT NOT NULL,
            PRIMARY KEY(variant_id, ordinal),
            FOREIGN KEY(variant_id) REFERENCES variants(id) ON DELETE CASCADE
                DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TABLE IF NOT EXISTS variant_aliases (
            alias_id TEXT PRIMARY KEY NOT NULL,
            variant_id TEXT,
            FOREIGN KEY(variant_id) REFERENCES variants(id) ON DELETE CASCADE
                DEFERRABLE INITIALLY DEFERRED
         );
         CREATE INDEX IF NOT EXISTS variant_cards_name
             ON variant_cards(normalized_name, variant_id);
         CREATE INDEX IF NOT EXISTS variant_cards_oracle
             ON variant_cards(oracle_id, variant_id) WHERE oracle_id IS NOT NULL;
         CREATE INDEX IF NOT EXISTS variant_cards_variant
             ON variant_cards(variant_id);
         CREATE INDEX IF NOT EXISTS variant_templates_template
             ON variant_templates(template_id, variant_id);
         CREATE INDEX IF NOT EXISTS variant_features_variant
             ON variant_features(variant_id);
         CREATE INDEX IF NOT EXISTS variant_aliases_variant
             ON variant_aliases(variant_id);",
    )?;

    let found = metadata(connection, "schema_version")?;
    if let Some(found) = found.as_deref()
        && found != COMBO_STORE_SCHEMA_VERSION
    {
        return Err(ComboStoreError::IncompatibleSchema {
            found: Some(found.to_string()),
            expected: COMBO_STORE_SCHEMA_VERSION,
        });
    }
    if found.is_none() {
        set_metadata(connection, "schema_version", COMBO_STORE_SCHEMA_VERSION)?;
        set_metadata(connection, "catalog_format", CATALOG_FORMAT)?;
    }
    Ok(())
}

fn status_from_connection(connection: &Connection) -> Result<ComboStoreStatus, ComboStoreError> {
    let variant_count: u64 =
        connection.query_row("SELECT COUNT(*) FROM variants", [], |row| row.get(0))?;
    let alias_count: u64 =
        connection.query_row("SELECT COUNT(*) FROM variant_aliases", [], |row| row.get(0))?;
    Ok(ComboStoreStatus {
        ready: variant_count > 0,
        schema_version: metadata(connection, "schema_version")?
            .unwrap_or_else(|| COMBO_STORE_SCHEMA_VERSION.into()),
        upstream_version: metadata(connection, "upstream_version")?,
        upstream_timestamp: metadata(connection, "upstream_timestamp")?,
        installed_at: metadata(connection, "installed_at")?,
        etag: metadata(connection, "etag")?,
        last_modified: metadata(connection, "last_modified")?,
        snapshot_sha256: metadata(connection, "snapshot_sha256")?,
        compressed_bytes: parse_optional_metadata(connection, "compressed_bytes")?,
        decompressed_bytes: parse_optional_metadata(connection, "decompressed_bytes")?,
        variant_count,
        alias_count,
        authenticity_basis: metadata(connection, "authenticity_basis")?
            .unwrap_or_else(|| "No downloaded Commander Spellbook snapshot is installed.".into()),
    })
}

fn metadata(connection: &Connection, key: &str) -> Result<Option<String>, rusqlite::Error> {
    connection
        .query_row("SELECT value FROM metadata WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
}

fn parse_optional_metadata<T>(
    connection: &Connection,
    key: &str,
) -> Result<Option<T>, ComboStoreError>
where
    T: std::str::FromStr,
{
    metadata(connection, key)?
        .map(|value| {
            value.parse::<T>().map_err(|_| {
                ComboStoreError::InvalidSnapshot(format!(
                    "stored combo metadata {key:?} was invalid"
                ))
            })
        })
        .transpose()
}

fn set_metadata(connection: &Connection, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    connection.execute(
        "INSERT INTO metadata(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

fn set_optional_metadata(
    connection: &Connection,
    key: &str,
    value: Option<&str>,
) -> Result<(), rusqlite::Error> {
    if let Some(value) = value {
        set_metadata(connection, key, value)
    } else {
        connection.execute("DELETE FROM metadata WHERE key = ?1", [key])?;
        Ok(())
    }
}

struct SqliteSnapshotWriter<'transaction> {
    insert_variant: rusqlite::Statement<'transaction>,
    insert_card: rusqlite::Statement<'transaction>,
    insert_template: rusqlite::Statement<'transaction>,
    insert_feature: rusqlite::Statement<'transaction>,
    insert_alias: rusqlite::Statement<'transaction>,
    limits: SnapshotLimits,
    reporter: Option<ComboUpdateReporter>,
    variant_fingerprints: HashMap<String, [u8; 32]>,
    variant_rows_seen: u64,
    variant_count: u64,
    duplicate_variant_rows: u64,
    alias_count: u64,
}

impl<'transaction> SqliteSnapshotWriter<'transaction> {
    fn new(
        transaction: &'transaction Transaction<'transaction>,
        limits: SnapshotLimits,
        reporter: Option<ComboUpdateReporter>,
    ) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            insert_variant: transaction.prepare(
                "INSERT INTO variants(
                    id, status, identity, mana_needed, mana_value_needed,
                    easy_prerequisites, notable_prerequisites, description, notes,
                    popularity, spoiler, bracket_tag, commander_legal, variant_count
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14
                 )",
            )?,
            insert_card: transaction.prepare(
                "INSERT INTO variant_cards(
                    variant_id, ordinal, spellbook_card_id, oracle_id,
                    normalized_name, display_name, quantity, must_be_commander,
                    zone_locations_json, battlefield_state, exile_state,
                    library_state, graveyard_state
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13
                 )",
            )?,
            insert_template: transaction.prepare(
                "INSERT INTO variant_templates(
                    variant_id, ordinal, template_id, name, scryfall_query,
                    quantity, must_be_commander, zone_locations_json,
                    battlefield_state, exile_state, library_state, graveyard_state
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12
                 )",
            )?,
            insert_feature: transaction.prepare(
                "INSERT INTO variant_features(
                    variant_id, ordinal, feature_id, name, quantity,
                    uncountable, status
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?,
            insert_alias: transaction
                .prepare("INSERT INTO variant_aliases(alias_id, variant_id) VALUES (?1, ?2)")?,
            limits,
            reporter,
            variant_fingerprints: HashMap::new(),
            variant_rows_seen: 0,
            variant_count: 0,
            duplicate_variant_rows: 0,
            alias_count: 0,
        })
    }

    fn validate_counts(&self) -> Result<(), ComboStoreError> {
        if self.variant_count < self.limits.min_variants {
            return Err(ComboStoreError::InvalidSnapshot(format!(
                "only {} variants were present; at least {} were required",
                self.variant_count, self.limits.min_variants
            )));
        }
        Ok(())
    }
}

trait SnapshotConsumer {
    fn consume_variant(&mut self, variant: RawVariant) -> Result<(), String>;
    fn consume_alias(&mut self, alias: RawAlias) -> Result<(), String>;
}

impl SnapshotConsumer for SqliteSnapshotWriter<'_> {
    fn consume_variant(&mut self, variant: RawVariant) -> Result<(), String> {
        if self.variant_rows_seen >= self.limits.max_variants {
            return Err(format!(
                "the snapshot exceeded the {} variant safety limit",
                self.limits.max_variants
            ));
        }
        self.variant_rows_seen += 1;
        variant.validate().map_err(|error| error.to_string())?;
        let id = variant.id.clone();
        let fingerprint = variant
            .imported_semantics_fingerprint()
            .map_err(|error| error.to_string())?;
        if let Some(previous) = self.variant_fingerprints.get(&id) {
            if previous != &fingerprint {
                return Err(format!(
                    "duplicate variant id {id:?} had conflicting importer-visible content"
                ));
            }
            self.duplicate_variant_rows += 1;
            return Ok(());
        }
        self.variant_fingerprints.insert(id.clone(), fingerprint);
        self.insert_variant
            .execute(params![
                id,
                variant.status,
                variant.identity,
                variant.mana_needed,
                variant.mana_value_needed,
                variant.easy_prerequisites,
                variant.notable_prerequisites,
                variant.description,
                variant.notes,
                variant.popularity,
                variant.spoiler as i64,
                variant.bracket_tag,
                variant
                    .legalities
                    .and_then(|value| value.commander)
                    .map(i64::from),
                variant.variant_count,
            ])
            .map_err(|error| error.to_string())?;

        for (ordinal, piece) in variant.uses.into_iter().enumerate() {
            let zones =
                serde_json::to_string(&piece.zone_locations).map_err(|error| error.to_string())?;
            self.insert_card
                .execute(params![
                    id,
                    ordinal as u64,
                    piece.card.id,
                    piece.card.oracle_id,
                    normalize_card_name(&piece.card.name),
                    piece.card.name,
                    piece.quantity,
                    piece.must_be_commander as i64,
                    zones,
                    piece.battlefield_card_state,
                    piece.exile_card_state,
                    piece.library_card_state,
                    piece.graveyard_card_state,
                ])
                .map_err(|error| error.to_string())?;
        }
        for (ordinal, piece) in variant.requires.into_iter().enumerate() {
            let zones =
                serde_json::to_string(&piece.zone_locations).map_err(|error| error.to_string())?;
            self.insert_template
                .execute(params![
                    id,
                    ordinal as u64,
                    piece.template.id,
                    piece.template.name,
                    piece.template.scryfall_query,
                    piece.quantity,
                    piece.must_be_commander as i64,
                    zones,
                    piece.battlefield_card_state,
                    piece.exile_card_state,
                    piece.library_card_state,
                    piece.graveyard_card_state,
                ])
                .map_err(|error| error.to_string())?;
        }
        for (ordinal, produced) in variant.produces.into_iter().enumerate() {
            self.insert_feature
                .execute(params![
                    id,
                    ordinal as u64,
                    produced.feature.id,
                    produced.feature.name,
                    produced.quantity,
                    produced.feature.uncountable as i64,
                    produced.feature.status,
                ])
                .map_err(|error| error.to_string())?;
        }

        self.variant_count += 1;
        if self.variant_count.is_multiple_of(5_000) {
            emit_progress(
                &self.reporter,
                ComboUpdateProgress {
                    phase: "index".into(),
                    completed_units: self.variant_count,
                    total_units: None,
                    progress: (0.50 + self.variant_count as f32 / 400_000.0).min(0.94),
                    detail: format!(
                        "Building the local combo index \u{2014} {} variants",
                        self.variant_count
                    ),
                },
            );
        }
        Ok(())
    }

    fn consume_alias(&mut self, alias: RawAlias) -> Result<(), String> {
        if self.alias_count >= self.limits.max_aliases {
            return Err(format!(
                "the snapshot exceeded the {} alias safety limit",
                self.limits.max_aliases
            ));
        }
        validate_bounded_text("variant alias", &alias.id, MAX_VARIANT_ID_BYTES)
            .map_err(|error| error.to_string())?;
        if let Some(variant_id) = alias.variant.as_deref() {
            validate_bounded_text("aliased variant id", variant_id, MAX_VARIANT_ID_BYTES)
                .map_err(|error| error.to_string())?;
        }
        self.insert_alias
            .execute(params![alias.id, alias.variant])
            .map_err(|error| error.to_string())?;
        self.alias_count += 1;
        Ok(())
    }
}

#[derive(Debug)]
struct SnapshotHeader {
    timestamp: String,
    version: String,
}

fn deserialize_snapshot(
    reader: impl Read,
    consumer: &mut impl SnapshotConsumer,
) -> Result<SnapshotHeader, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    let header = SnapshotSeed { consumer }.deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(header)
}

struct SnapshotSeed<'consumer, C> {
    consumer: &'consumer mut C,
}

impl<'de, C> DeserializeSeed<'de> for SnapshotSeed<'_, C>
where
    C: SnapshotConsumer,
{
    type Value = SnapshotHeader;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(SnapshotRootVisitor {
            consumer: self.consumer,
        })
    }
}

struct SnapshotRootVisitor<'consumer, C> {
    consumer: &'consumer mut C,
}

impl<'de, C> Visitor<'de> for SnapshotRootVisitor<'_, C>
where
    C: SnapshotConsumer,
{
    type Value = SnapshotHeader;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Commander Spellbook bulk snapshot object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut timestamp = None;
        let mut version = None;
        let mut saw_variants = false;
        let mut saw_aliases = false;
        let consumer = self.consumer;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "timestamp" => {
                    if timestamp.is_some() {
                        return Err(de::Error::duplicate_field("timestamp"));
                    }
                    timestamp = Some(map.next_value::<String>()?);
                }
                "version" => {
                    if version.is_some() {
                        return Err(de::Error::duplicate_field("version"));
                    }
                    version = Some(map.next_value::<String>()?);
                }
                "variants" => {
                    if saw_variants {
                        return Err(de::Error::duplicate_field("variants"));
                    }
                    saw_variants = true;
                    map.next_value_seed(CallbackSequenceSeed::<RawVariant, _, _> {
                        callback: |variant| consumer.consume_variant(variant),
                        marker: PhantomData,
                    })?;
                }
                "aliases" => {
                    if saw_aliases {
                        return Err(de::Error::duplicate_field("aliases"));
                    }
                    saw_aliases = true;
                    map.next_value_seed(CallbackSequenceSeed::<RawAlias, _, _> {
                        callback: |alias| consumer.consume_alias(alias),
                        marker: PhantomData,
                    })?;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        if !saw_variants {
            return Err(de::Error::missing_field("variants"));
        }
        Ok(SnapshotHeader {
            timestamp: timestamp.ok_or_else(|| de::Error::missing_field("timestamp"))?,
            version: version.ok_or_else(|| de::Error::missing_field("version"))?,
        })
    }
}

struct CallbackSequenceSeed<T, F, E> {
    callback: F,
    marker: PhantomData<fn(T) -> E>,
}

impl<'de, T, F, E> DeserializeSeed<'de> for CallbackSequenceSeed<T, F, E>
where
    T: Deserialize<'de>,
    F: FnMut(T) -> Result<(), E>,
    E: fmt::Display,
{
    type Value = u64;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(CallbackSequenceVisitor {
            callback: self.callback,
            marker: PhantomData,
        })
    }
}

struct CallbackSequenceVisitor<T, F, E> {
    callback: F,
    marker: PhantomData<fn(T) -> E>,
}

impl<'de, T, F, E> Visitor<'de> for CallbackSequenceVisitor<T, F, E>
where
    T: Deserialize<'de>,
    F: FnMut(T) -> Result<(), E>,
    E: fmt::Display,
{
    type Value = u64;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an array of snapshot records")
    }

    fn visit_seq<A>(mut self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut count = 0u64;
        while let Some(value) = sequence.next_element::<T>()? {
            (self.callback)(value).map_err(de::Error::custom)?;
            count += 1;
        }
        Ok(count)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RawVariant {
    id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    uses: Vec<RawCardPiece>,
    #[serde(default)]
    requires: Vec<RawTemplatePiece>,
    #[serde(default)]
    produces: Vec<RawProducedFeature>,
    #[serde(default)]
    identity: String,
    #[serde(default)]
    mana_needed: Option<String>,
    #[serde(default)]
    mana_value_needed: Option<u32>,
    #[serde(default)]
    easy_prerequisites: Option<String>,
    #[serde(default)]
    notable_prerequisites: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    popularity: Option<u64>,
    #[serde(default)]
    spoiler: bool,
    #[serde(default)]
    bracket_tag: Option<String>,
    #[serde(default)]
    legalities: Option<RawLegalities>,
    #[serde(default)]
    variant_count: u32,
}

impl RawVariant {
    fn validate(&self) -> Result<(), ComboStoreError> {
        validate_bounded_text("variant id", &self.id, MAX_VARIANT_ID_BYTES)?;
        if self.uses.is_empty() && self.requires.is_empty() {
            return Err(ComboStoreError::InvalidSnapshot(format!(
                "variant {:?} had no card or template ingredients",
                self.id
            )));
        }
        if self.uses.len() > MAX_COMPONENTS_PER_VARIANT
            || self.requires.len() > MAX_COMPONENTS_PER_VARIANT
            || self.produces.len() > MAX_COMPONENTS_PER_VARIANT
        {
            return Err(ComboStoreError::InvalidSnapshot(format!(
                "variant {:?} exceeded the per-variant component limit",
                self.id
            )));
        }
        validate_bounded_text("variant status", &self.status, 16)?;
        validate_bounded_text("variant identity", &self.identity, 16)?;
        validate_optional_text(
            "mana requirement",
            self.mana_needed.as_deref(),
            MAX_SHORT_TEXT_BYTES,
        )?;
        validate_optional_text(
            "easy prerequisites",
            self.easy_prerequisites.as_deref(),
            MAX_LONG_TEXT_BYTES,
        )?;
        validate_optional_text(
            "notable prerequisites",
            self.notable_prerequisites.as_deref(),
            MAX_LONG_TEXT_BYTES,
        )?;
        validate_optional_text(
            "description",
            self.description.as_deref(),
            MAX_LONG_TEXT_BYTES,
        )?;
        validate_optional_text("notes", self.notes.as_deref(), MAX_LONG_TEXT_BYTES)?;
        for piece in &self.uses {
            piece.validate()?;
        }
        for piece in &self.requires {
            piece.validate()?;
        }
        for produced in &self.produces {
            produced.validate()?;
        }
        Ok(())
    }

    fn imported_semantics_fingerprint(&self) -> Result<[u8; 32], serde_json::Error> {
        // Hash the typed projection that is actually persisted and queried.
        // JSON object order, absent/defaulted fields, and upstream presentation
        // metadata cannot create false conflicts, while any importer-visible
        // semantic difference rejects the staged snapshot.
        let encoded = serde_json::to_vec(self)?;
        let digest = Sha256::digest(encoded);
        let mut fingerprint = [0u8; 32];
        fingerprint.copy_from_slice(&digest);
        Ok(fingerprint)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct RawLegalities {
    #[serde(default)]
    commander: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RawCardPiece {
    card: RawCard,
    #[serde(default)]
    zone_locations: Vec<String>,
    #[serde(default)]
    battlefield_card_state: Option<String>,
    #[serde(default)]
    exile_card_state: Option<String>,
    #[serde(default)]
    library_card_state: Option<String>,
    #[serde(default)]
    graveyard_card_state: Option<String>,
    #[serde(default)]
    must_be_commander: bool,
    quantity: u32,
}

impl RawCardPiece {
    fn validate(&self) -> Result<(), ComboStoreError> {
        if self.quantity == 0 {
            return Err(ComboStoreError::InvalidSnapshot(format!(
                "card {:?} had zero quantity",
                self.card.name
            )));
        }
        validate_bounded_text("card name", &self.card.name, MAX_NAME_BYTES)?;
        validate_optional_text("Oracle id", self.card.oracle_id.as_deref(), 128)?;
        validate_state_fields(
            &self.zone_locations,
            [
                self.battlefield_card_state.as_deref(),
                self.exile_card_state.as_deref(),
                self.library_card_state.as_deref(),
                self.graveyard_card_state.as_deref(),
            ],
        )
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RawCard {
    id: u64,
    name: String,
    #[serde(default)]
    oracle_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RawTemplatePiece {
    template: RawTemplate,
    #[serde(default)]
    zone_locations: Vec<String>,
    #[serde(default)]
    battlefield_card_state: Option<String>,
    #[serde(default)]
    exile_card_state: Option<String>,
    #[serde(default)]
    library_card_state: Option<String>,
    #[serde(default)]
    graveyard_card_state: Option<String>,
    #[serde(default)]
    must_be_commander: bool,
    quantity: u32,
}

impl RawTemplatePiece {
    fn validate(&self) -> Result<(), ComboStoreError> {
        if self.quantity == 0 {
            return Err(ComboStoreError::InvalidSnapshot(format!(
                "template {:?} had zero quantity",
                self.template.name
            )));
        }
        validate_bounded_text("template name", &self.template.name, MAX_NAME_BYTES)?;
        validate_optional_text(
            "template Scryfall query",
            self.template.scryfall_query.as_deref(),
            MAX_SHORT_TEXT_BYTES,
        )?;
        validate_state_fields(
            &self.zone_locations,
            [
                self.battlefield_card_state.as_deref(),
                self.exile_card_state.as_deref(),
                self.library_card_state.as_deref(),
                self.graveyard_card_state.as_deref(),
            ],
        )
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RawTemplate {
    id: u64,
    name: String,
    #[serde(default)]
    scryfall_query: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RawProducedFeature {
    feature: RawFeature,
    quantity: u32,
}

impl RawProducedFeature {
    fn validate(&self) -> Result<(), ComboStoreError> {
        if self.quantity == 0 {
            return Err(ComboStoreError::InvalidSnapshot(format!(
                "feature {:?} had zero quantity",
                self.feature.name
            )));
        }
        validate_bounded_text("feature name", &self.feature.name, MAX_NAME_BYTES)?;
        validate_bounded_text("feature status", &self.feature.status, 16)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct RawFeature {
    id: u64,
    name: String,
    #[serde(default)]
    uncountable: bool,
    #[serde(default)]
    status: String,
}

#[derive(Debug, Deserialize)]
struct RawAlias {
    id: String,
    #[serde(default)]
    variant: Option<String>,
}

fn validate_snapshot_header(header: &SnapshotHeader) -> Result<(), ComboStoreError> {
    validate_bounded_text("snapshot version", &header.version, 128)?;
    validate_bounded_text("snapshot timestamp", &header.timestamp, 128)
}

fn validate_snapshot_source(source: &SnapshotSource) -> Result<(), ComboStoreError> {
    for (field, value) in [
        ("snapshot ETag", source.etag.as_deref()),
        ("snapshot Last-Modified", source.last_modified.as_deref()),
    ] {
        if let Some(value) = value {
            validate_bounded_text(field, value, MAX_HEADER_VALUE_BYTES)?;
            if value.chars().any(char::is_control) {
                return Err(ComboStoreError::InvalidSnapshot(format!(
                    "{field} contained control characters"
                )));
            }
        }
    }
    if let Some(digest) = source.expected_sha256.as_deref() {
        let digest = digest.trim();
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ComboStoreError::InvalidSnapshot(
                "the expected snapshot SHA-256 was not a 64-character hexadecimal digest".into(),
            ));
        }
    }
    Ok(())
}

fn validate_limits(limits: SnapshotLimits) -> Result<(), ComboStoreError> {
    if limits.max_compressed_bytes == 0
        || limits.max_decompressed_bytes == 0
        || limits.min_variants > limits.max_variants
        || limits.max_variants == 0
        || limits.max_aliases == 0
    {
        return Err(ComboStoreError::InvalidSnapshot(
            "snapshot safety limits were internally inconsistent".into(),
        ));
    }
    Ok(())
}

fn validate_bounded_text(
    field: &str,
    value: &str,
    max_bytes: usize,
) -> Result<(), ComboStoreError> {
    if value.trim().is_empty() {
        return Err(ComboStoreError::InvalidSnapshot(format!(
            "{field} was empty"
        )));
    }
    if value.len() > max_bytes {
        return Err(ComboStoreError::InvalidSnapshot(format!(
            "{field} exceeded {max_bytes} bytes"
        )));
    }
    Ok(())
}

fn validate_optional_text(
    field: &str,
    value: Option<&str>,
    max_bytes: usize,
) -> Result<(), ComboStoreError> {
    if value.is_some_and(|value| value.len() > max_bytes) {
        return Err(ComboStoreError::InvalidSnapshot(format!(
            "{field} exceeded {max_bytes} bytes"
        )));
    }
    Ok(())
}

fn validate_state_fields(
    zones: &[String],
    states: [Option<&str>; 4],
) -> Result<(), ComboStoreError> {
    if zones.len() > 32 {
        return Err(ComboStoreError::InvalidSnapshot(
            "an ingredient had too many zone locations".into(),
        ));
    }
    for zone in zones {
        validate_bounded_text("zone location", zone, 64)?;
    }
    for state in states.into_iter().flatten() {
        if state.len() > MAX_SHORT_TEXT_BYTES {
            return Err(ComboStoreError::InvalidSnapshot(
                "an ingredient state description was too long".into(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
struct InspectedCompressedFile {
    bytes: u64,
    sha256: String,
}

fn inspect_compressed_file(
    path: &Path,
    max_compressed_bytes: u64,
) -> Result<InspectedCompressedFile, ComboStoreError> {
    let file = File::open(path)?;
    let advertised = file.metadata()?.len();
    if advertised > max_compressed_bytes {
        return Err(ComboStoreError::CompressedTooLarge {
            limit_bytes: max_compressed_bytes,
        });
    }
    let mut reader = BufReader::with_capacity(128 * 1024, file);
    let mut prefix = [0u8; 2];
    reader.read_exact(&mut prefix).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            ComboStoreError::InvalidGzip
        } else {
            error.into()
        }
    })?;
    if prefix != [0x1f, 0x8b] {
        return Err(ComboStoreError::InvalidGzip);
    }
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    let mut total = 2u64;
    let mut buffer = [0u8; 128 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or(ComboStoreError::CompressedTooLarge {
                limit_bytes: max_compressed_bytes,
            })?;
        if total > max_compressed_bytes {
            return Err(ComboStoreError::CompressedTooLarge {
                limit_bytes: max_compressed_bytes,
            });
        }
        hasher.update(&buffer[..read]);
    }
    Ok(InspectedCompressedFile {
        bytes: total,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

fn constant_time_digest_eq(expected: &str, actual: &str) -> bool {
    let expected = expected.trim().as_bytes();
    let actual = actual.as_bytes();
    if expected.len() != actual.len() {
        return false;
    }
    expected
        .iter()
        .zip(actual)
        .fold(0u8, |difference, (left, right)| {
            difference | (left.to_ascii_lowercase() ^ *right)
        })
        == 0
}

struct DecompressedLimitReader<R> {
    inner: R,
    remaining: u64,
    exceeded: Rc<Cell<bool>>,
    consumed: Rc<Cell<u64>>,
}

impl<R> DecompressedLimitReader<R> {
    fn new(inner: R, limit: u64, exceeded: Rc<Cell<bool>>, consumed: Rc<Cell<u64>>) -> Self {
        Self {
            inner,
            remaining: limit,
            exceeded,
            consumed,
        }
    }
}

impl<R: Read> Read for DecompressedLimitReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut probe = [0u8; 1];
            if self.inner.read(&mut probe)? == 0 {
                return Ok(0);
            }
            self.exceeded.set(true);
            return Err(io::Error::other("decompressed snapshot limit exceeded"));
        }
        let allowed = usize::try_from(self.remaining.min(buffer.len() as u64))
            .expect("bounded by buffer length");
        let read = self.inner.read(&mut buffer[..allowed])?;
        self.remaining -= read as u64;
        self.consumed.set(self.consumed.get() + read as u64);
        Ok(read)
    }
}

#[derive(Default)]
struct DeckInventory {
    total: HashMap<String, u32>,
    commanders: HashMap<String, u32>,
}

impl DeckInventory {
    fn from_entries(
        connection: &Connection,
        entries: &[ComboDeckCard],
    ) -> Result<Self, ComboStoreError> {
        let mut inventory = Self::default();
        let mut oracle_name = connection.prepare(
            "SELECT normalized_name
             FROM variant_cards
             WHERE oracle_id = ?1
             ORDER BY normalized_name
             LIMIT 1",
        )?;
        for entry in entries {
            let normalized_input = normalize_card_name(&entry.name);
            let normalized = if let Some(oracle_id) = entry.oracle_id.as_deref() {
                oracle_name
                    .query_row([oracle_id], |row| row.get::<_, String>(0))
                    .optional()?
                    .unwrap_or(normalized_input)
            } else {
                normalized_input
            };
            add_quantity(&mut inventory.total, &normalized, entry.quantity)?;
            if entry.is_commander {
                add_quantity(&mut inventory.commanders, &normalized, entry.quantity)?;
            }
        }
        Ok(inventory)
    }
}

#[derive(Default)]
struct TemplateInventory {
    total: HashMap<u64, u32>,
    commanders: HashMap<u64, u32>,
}

impl TemplateInventory {
    fn from_entries(entries: &[ResolvedTemplate]) -> Self {
        let mut inventory = Self::default();
        for entry in entries {
            inventory
                .total
                .entry(entry.template_id)
                .and_modify(|quantity| *quantity = quantity.saturating_add(entry.quantity))
                .or_insert(entry.quantity);
            inventory
                .commanders
                .entry(entry.template_id)
                .and_modify(|quantity| {
                    *quantity = quantity.saturating_add(entry.commander_quantity)
                })
                .or_insert(entry.commander_quantity);
        }
        inventory
    }
}

fn add_quantity(
    inventory: &mut HashMap<String, u32>,
    key: &str,
    quantity: u32,
) -> Result<(), ComboStoreError> {
    let current = inventory.entry(key.to_string()).or_default();
    *current = current.checked_add(quantity).ok_or_else(|| {
        ComboStoreError::InvalidSnapshot("deck card quantities overflowed".into())
    })?;
    Ok(())
}

fn validate_deck_input(
    cards: &[ComboDeckCard],
    templates: &[ResolvedTemplate],
) -> Result<(), ComboStoreError> {
    if cards.is_empty() {
        return Err(ComboStoreError::InvalidSnapshot(
            "at least one deck card is required for combo matching".into(),
        ));
    }
    if cards.len() > MAX_DECK_ENTRIES {
        return Err(ComboStoreError::InvalidSnapshot(format!(
            "combo matching accepts at most {MAX_DECK_ENTRIES} deck entries"
        )));
    }
    for card in cards {
        validate_bounded_text("deck card name", &card.name, MAX_NAME_BYTES)?;
        validate_optional_text("deck card Oracle id", card.oracle_id.as_deref(), 128)?;
        if card.quantity == 0 {
            return Err(ComboStoreError::InvalidSnapshot(format!(
                "deck card {:?} had zero quantity",
                card.name
            )));
        }
    }
    for template in templates {
        if template.quantity == 0 || template.commander_quantity > template.quantity {
            return Err(ComboStoreError::InvalidSnapshot(format!(
                "resolved template {} had invalid quantities",
                template.template_id
            )));
        }
    }
    Ok(())
}

fn candidate_variant_ids(
    connection: &Connection,
    inventory: &DeckInventory,
    templates: &TemplateInventory,
) -> Result<HashSet<String>, rusqlite::Error> {
    let mut ids = HashSet::new();
    let mut by_card =
        connection.prepare("SELECT variant_id FROM variant_cards WHERE normalized_name = ?1")?;
    for normalized in inventory.total.keys() {
        let rows = by_card.query_map([normalized], |row| row.get::<_, String>(0))?;
        for row in rows {
            ids.insert(row?);
        }
    }
    let mut by_template =
        connection.prepare("SELECT variant_id FROM variant_templates WHERE template_id = ?1")?;
    for template_id in templates.total.keys() {
        let rows = by_template.query_map([template_id], |row| row.get::<_, String>(0))?;
        for row in rows {
            ids.insert(row?);
        }
    }
    Ok(ids)
}

#[derive(Debug)]
struct StoredVariantHeader {
    id: String,
    status: String,
    identity: String,
    mana_needed: Option<String>,
    mana_value_needed: Option<u32>,
    easy_prerequisites: Option<String>,
    notable_prerequisites: Option<String>,
    description: Option<String>,
    notes: Option<String>,
    popularity: Option<u64>,
    bracket_tag: Option<String>,
    commander_legal: Option<bool>,
}

fn load_match(
    connection: &Connection,
    variant_id: &str,
    inventory: &DeckInventory,
    template_inventory: &TemplateInventory,
) -> Result<Option<LocalComboMatch>, ComboStoreError> {
    let header = connection.query_row(
        "SELECT id, status, identity, mana_needed, mana_value_needed,
                easy_prerequisites, notable_prerequisites, description, notes,
                popularity, bracket_tag, commander_legal
         FROM variants WHERE id = ?1",
        [variant_id],
        |row| {
            Ok(StoredVariantHeader {
                id: row.get(0)?,
                status: row.get(1)?,
                identity: row.get(2)?,
                mana_needed: row.get(3)?,
                mana_value_needed: row.get(4)?,
                easy_prerequisites: row.get(5)?,
                notable_prerequisites: row.get(6)?,
                description: row.get(7)?,
                notes: row.get(8)?,
                popularity: row.get(9)?,
                bracket_tag: row.get(10)?,
                commander_legal: row.get::<_, Option<i64>>(11)?.map(|value| value != 0),
            })
        },
    )?;
    let cards = load_cards(connection, variant_id)?;
    if !card_requirements_satisfied(&cards, inventory) {
        return Ok(None);
    }
    let templates = load_templates(connection, variant_id)?;
    let template_match = classify_template_match(&templates, template_inventory);
    let produces = load_features(connection, variant_id)?;
    let relevance = classify_relevance(&produces);
    let has_unbounded_result = produces.iter().any(|feature| {
        feature.uncountable || feature.name.to_ascii_lowercase().contains("infinite")
    });
    let table_lethality = classify_table_lethality(&produces, has_unbounded_result);
    Ok(Some(LocalComboMatch {
        variant_id: header.id,
        status: header.status,
        bracket_tag: header.bracket_tag,
        identity: header.identity,
        commander_legal: header.commander_legal,
        cards,
        templates,
        template_match,
        produces,
        mana_needed: header.mana_needed,
        mana_value_needed: header.mana_value_needed,
        mana_minimum_confirmed: false,
        easy_prerequisites: header.easy_prerequisites,
        notable_prerequisites: header.notable_prerequisites,
        description: header.description,
        notes: header.notes,
        popularity: header.popularity,
        relevance,
        table_lethality,
        has_unbounded_result,
    }))
}

fn load_cards(
    connection: &Connection,
    variant_id: &str,
) -> Result<Vec<LocalCardRequirement>, ComboStoreError> {
    let mut statement = connection.prepare(
        "SELECT display_name, normalized_name, oracle_id, quantity,
                must_be_commander, zone_locations_json, battlefield_state,
                exile_state, library_state, graveyard_state
         FROM variant_cards WHERE variant_id = ?1 ORDER BY ordinal",
    )?;
    let rows = statement.query_map([variant_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, u32>(3)?,
            row.get::<_, i64>(4)? != 0,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
        ))
    })?;
    let mut cards = Vec::new();
    for row in rows {
        let (
            name,
            normalized_name,
            oracle_id,
            quantity,
            must_be_commander,
            zones,
            battlefield_state,
            exile_state,
            library_state,
            graveyard_state,
        ) = row?;
        cards.push(LocalCardRequirement {
            name,
            normalized_name,
            oracle_id,
            quantity,
            must_be_commander,
            zone_locations: serde_json::from_str(&zones)?,
            battlefield_state,
            exile_state,
            library_state,
            graveyard_state,
        });
    }
    Ok(cards)
}

fn load_templates(
    connection: &Connection,
    variant_id: &str,
) -> Result<Vec<LocalTemplateRequirement>, ComboStoreError> {
    let mut statement = connection.prepare(
        "SELECT template_id, name, scryfall_query, quantity, must_be_commander,
                zone_locations_json, battlefield_state, exile_state,
                library_state, graveyard_state
         FROM variant_templates WHERE variant_id = ?1 ORDER BY ordinal",
    )?;
    let rows = statement.query_map([variant_id], |row| {
        Ok((
            row.get::<_, u64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, u32>(3)?,
            row.get::<_, i64>(4)? != 0,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
        ))
    })?;
    let mut templates = Vec::new();
    for row in rows {
        let (
            id,
            name,
            scryfall_query,
            quantity,
            must_be_commander,
            zones,
            battlefield_state,
            exile_state,
            library_state,
            graveyard_state,
        ) = row?;
        templates.push(LocalTemplateRequirement {
            id,
            name,
            scryfall_query,
            quantity,
            must_be_commander,
            zone_locations: serde_json::from_str(&zones)?,
            battlefield_state,
            exile_state,
            library_state,
            graveyard_state,
        });
    }
    Ok(templates)
}

fn load_features(
    connection: &Connection,
    variant_id: &str,
) -> Result<Vec<LocalProducedFeature>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT feature_id, name, quantity, uncountable, status
         FROM variant_features WHERE variant_id = ?1 ORDER BY ordinal",
    )?;
    let rows = statement.query_map([variant_id], |row| {
        Ok(LocalProducedFeature {
            id: row.get(0)?,
            name: row.get(1)?,
            quantity: row.get(2)?,
            uncountable: row.get::<_, i64>(3)? != 0,
            status: row.get(4)?,
        })
    })?;
    rows.collect()
}

fn card_requirements_satisfied(cards: &[LocalCardRequirement], inventory: &DeckInventory) -> bool {
    let mut required = HashMap::<&str, u32>::new();
    let mut commander_required = HashMap::<&str, u32>::new();
    for card in cards {
        let Some(total) = required
            .entry(&card.normalized_name)
            .or_default()
            .checked_add(card.quantity)
        else {
            return false;
        };
        required.insert(&card.normalized_name, total);
        if card.must_be_commander {
            let Some(total) = commander_required
                .entry(&card.normalized_name)
                .or_default()
                .checked_add(card.quantity)
            else {
                return false;
            };
            commander_required.insert(&card.normalized_name, total);
        }
    }
    required.iter().all(|(name, quantity)| {
        inventory.total.get(*name).copied().unwrap_or_default() >= *quantity
    }) && commander_required.iter().all(|(name, quantity)| {
        inventory.commanders.get(*name).copied().unwrap_or_default() >= *quantity
    })
}

fn classify_template_match(
    templates: &[LocalTemplateRequirement],
    inventory: &TemplateInventory,
) -> TemplateMatchState {
    if templates.is_empty() {
        return TemplateMatchState::NotRequired;
    }
    let mut unresolved = false;
    for template in templates {
        let Some(available) = inventory.total.get(&template.id).copied() else {
            unresolved = true;
            continue;
        };
        if available < template.quantity {
            return TemplateMatchState::Unsatisfied;
        }
        if template.must_be_commander
            && inventory
                .commanders
                .get(&template.id)
                .copied()
                .unwrap_or_default()
                < template.quantity
        {
            return TemplateMatchState::Unsatisfied;
        }
    }
    if unresolved {
        TemplateMatchState::Unresolved
    } else {
        TemplateMatchState::Satisfied
    }
}

fn classify_relevance(features: &[LocalProducedFeature]) -> MatchRelevance {
    if features.iter().any(|feature| feature.status == "S") {
        MatchRelevance::Relevant
    } else if features.iter().any(|feature| feature.status == "C") {
        MatchRelevance::Borderline
    } else if features.is_empty() {
        MatchRelevance::Unknown
    } else if features.iter().all(|feature| !feature.status.is_empty()) {
        MatchRelevance::NotRelevant
    } else {
        MatchRelevance::Unknown
    }
}

fn classify_table_lethality(
    features: &[LocalProducedFeature],
    has_unbounded_result: bool,
) -> TableLethality {
    let normalized = features
        .iter()
        .map(|feature| feature.name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if normalized.iter().any(|name| {
        name.contains("win the game")
            || name.contains("each opponent loses the game")
            || name.contains("all opponents lose the game")
    }) {
        TableLethality::DocumentedTableWin
    } else if normalized.iter().any(|name| {
        (name.contains("infinite damage") || name.contains("infinite life loss"))
            && (name.contains("each opponent")
                || name.contains("all opponent")
                || name.contains("any target"))
    }) {
        TableLethality::LikelyTableLethal
    } else if has_unbounded_result {
        TableLethality::RequiresPayoffOrConversion
    } else {
        TableLethality::Unknown
    }
}

fn normalize_card_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn format_bytes(bytes: u64) -> String {
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes < 1024 * 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / MIB)
    }
}
