//! Authorized, aggregate-only EDHREC data boundary.
//!
//! This module deliberately contains no network client, website parser, HTML
//! parser, or undocumented endpoint knowledge. It can only import a bounded
//! JSON file that carries explicit authorization metadata for aggregate data.
//! The raw JSON is inspected for obvious player-level and decklist fields
//! before it is deserialized into a closed, versioned schema.
//!
//! EDHREC-style inclusion and synergy values are derived locally from counts:
//!
//! `synergy = commander_or_theme_inclusion - color_identity_inclusion`
//!
//! Imported percentages are not part of the schema and are rejected as unknown
//! fields. The installed snapshot retains one previous generation, uses a
//! content SHA-256, and is atomically activated after staged validation.

use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

pub(crate) const EDHREC_AGGREGATE_SCHEMA_VERSION: &str = "1";
pub(crate) const EDHREC_DERIVATION_VERSION: &str = "edhrec-count-derivation-1";

const MAX_IMPORT_BYTES: usize = 128 * 1024 * 1024;
const MAX_STORED_BYTES: u64 = 144 * 1024 * 1024;
const MAX_SCOPES: usize = 50_000;
const MAX_CARDS_PER_SCOPE: usize = 10_000;
const MAX_TOTAL_CARD_FACTS: usize = 2_000_000;
const MAX_SOURCE_MIX_ENTRIES: usize = 64;
const MAX_DECK_COUNT: u64 = 1_000_000_000;
const MAX_RANK: u64 = 10_000_000;
const MAX_IDENTIFIER_BYTES: usize = 256;
const MAX_SHORT_TEXT_BYTES: usize = 2 * 1024;
const MAX_NOTES_BYTES: usize = 16 * 1024;
const MAX_ATTRIBUTION_BYTES: usize = 4 * 1024;

#[derive(Debug, thiserror::Error)]
pub(crate) enum EdhrecDataError {
    #[error("EDHREC aggregate snapshot file error: {0}")]
    Io(#[from] io::Error),
    #[error("EDHREC aggregate snapshot JSON was invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("EDHREC aggregate snapshot failed validation: {0}")]
    Invalid(String),
    #[error("EDHREC aggregate snapshot was rejected by the privacy guard: {0}")]
    Privacy(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EdhrecAuthorizationBasis {
    /// A provider-signed or provider-issued agreement specifically authorizes
    /// this application's use of the aggregate dataset.
    WrittenProviderAgreement,
    /// The provider published an explicit data/API license that covers this
    /// aggregate file and the declared local use.
    ProviderPublishedDataLicense,
    /// The provider directly supplied this export with written authorization
    /// for the declared local use.
    ProviderSuppliedAuthorizedExport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EdhrecSourceKind {
    DeckBuilder,
    CardDatabase,
    ProviderAggregate,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecTimeWindow {
    /// Inclusive calendar date in `YYYY-MM-DD` form.
    pub start_date: String,
    /// Inclusive calendar date in `YYYY-MM-DD` form.
    pub end_date: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecAccessAuthorization {
    /// Stable provider identifier. Version 1 accepts only `edhrec`.
    pub provider_id: String,
    pub provider_name: String,
    pub basis: EdhrecAuthorizationBasis,
    /// Contract number, license revision, authorization email/message ID, or
    /// another provider-verifiable reference. A claim such as "public web
    /// page" is not sufficient.
    pub authorization_reference: String,
    pub license_or_agreement: String,
    pub authorized_at: String,
    pub expires_at: Option<String>,
    pub terms_url: Option<String>,
    /// Must be true. Raw decklists and player-level rows are not accepted.
    pub aggregate_only: bool,
    /// Must be explicitly authorized for this local snapshot store.
    pub local_cache_allowed: bool,
    /// Must be explicitly authorized for locally derived analysis.
    pub derived_analysis_allowed: bool,
    /// Retained for downstream export policy. This importer never
    /// redistributes the data itself.
    pub redistribution_allowed: bool,
    pub attribution: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecSourceMixEntry {
    pub source_id: String,
    pub source_name: String,
    pub source_kind: EdhrecSourceKind,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecPopularity {
    pub deck_count: u64,
    pub rank: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecThemeScope {
    pub theme_id: String,
    pub theme_version: String,
    pub display_name: Option<String>,
    pub popularity: EdhrecPopularity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecCardAggregate {
    pub card_oracle_id: String,
    pub inclusion_deck_count: u64,
    pub eligible_deck_count: u64,
    pub color_identity_inclusion_deck_count: u64,
    pub color_identity_eligible_deck_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EdhrecDerivedCardMetrics {
    /// Fraction in the inclusive range `0.0..=1.0`.
    pub inclusion_rate: f64,
    /// Fraction in the inclusive range `0.0..=1.0`.
    pub color_identity_baseline_rate: f64,
    /// Inclusion minus color-identity baseline, in `-1.0..=1.0`.
    pub synergy_score: f64,
    /// The same difference expressed as percentage points.
    pub synergy_percentage_points: f64,
}

impl EdhrecCardAggregate {
    /// Calculates all percentage-like values from counts in this record.
    ///
    /// Imported percentages are intentionally absent from the schema. `None`
    /// is returned for a manually constructed, unvalidated value with a zero
    /// denominator; validated snapshots always return `Some`.
    pub(crate) fn derived_metrics(&self) -> Option<EdhrecDerivedCardMetrics> {
        if self.eligible_deck_count == 0 || self.color_identity_eligible_deck_count == 0 {
            return None;
        }
        let inclusion_rate = self.inclusion_deck_count as f64 / self.eligible_deck_count as f64;
        let color_identity_baseline_rate = self.color_identity_inclusion_deck_count as f64
            / self.color_identity_eligible_deck_count as f64;
        let synergy_score = inclusion_rate - color_identity_baseline_rate;
        Some(EdhrecDerivedCardMetrics {
            inclusion_rate,
            color_identity_baseline_rate,
            synergy_score,
            synergy_percentage_points: synergy_score * 100.0,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecCommanderScope {
    pub commander_oracle_id: String,
    pub partner_oracle_id: Option<String>,
    pub commander_popularity: EdhrecPopularity,
    pub theme: Option<EdhrecThemeScope>,
    pub cards: Vec<EdhrecCardAggregate>,
}

impl EdhrecCommanderScope {
    pub(crate) fn scope_deck_count(&self) -> u64 {
        self.theme
            .as_ref()
            .map_or(self.commander_popularity.deck_count, |theme| {
                theme.popularity.deck_count
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecAggregateImport {
    pub schema_version: String,
    pub generated_at: String,
    pub time_window: EdhrecTimeWindow,
    pub access: EdhrecAccessAuthorization,
    pub source_mix: Vec<EdhrecSourceMixEntry>,
    pub deduplication_notes: String,
    pub scopes: Vec<EdhrecCommanderScope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecAggregateSnapshot {
    pub schema_version: String,
    pub derivation_version: String,
    pub installed_at: String,
    /// SHA-256 of the canonical serialization of `data`, after closed-schema
    /// deserialization and validation.
    pub snapshot_sha256: String,
    /// Size of the exact user/provider-supplied JSON file.
    pub source_bytes: u64,
    pub data: EdhrecAggregateImport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EdhrecDataStatus {
    pub ready: bool,
    pub schema_version: String,
    pub derivation_version: String,
    pub generated_at: Option<String>,
    pub installed_at: Option<String>,
    pub snapshot_sha256: Option<String>,
    pub source_bytes: Option<u64>,
    pub time_window: Option<EdhrecTimeWindow>,
    pub scope_count: u64,
    pub card_fact_count: u64,
    pub provider_name: Option<String>,
    pub authorization_basis: Option<EdhrecAuthorizationBasis>,
    pub license_or_agreement: Option<String>,
    pub authorization_expires_at: Option<String>,
    pub redistribution_allowed: bool,
    pub attribution: Option<String>,
    pub authenticity_basis: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub(crate) enum EdhrecImportOutcome {
    Installed { status: EdhrecDataStatus },
    Unchanged { status: EdhrecDataStatus },
}

#[derive(Debug, Clone)]
pub(crate) struct EdhrecDataStore {
    root: PathBuf,
}

impl EdhrecDataStore {
    pub(crate) fn new(root: impl Into<PathBuf>) -> Result<Self, EdhrecDataError> {
        let store = Self { root: root.into() };
        fs::create_dir_all(&store.root)?;
        if store.next_path().exists() {
            fs::remove_file(store.next_path())?;
        }
        store.recover_previous_if_needed()?;
        Ok(store)
    }

    /// Imports a provider-supplied or otherwise expressly authorized local
    /// aggregate file. This method performs no network activity.
    pub(crate) fn import_authorized_file(
        &self,
        path: &Path,
    ) -> Result<EdhrecImportOutcome, EdhrecDataError> {
        let size = fs::metadata(path)?.len();
        if size > MAX_IMPORT_BYTES as u64 {
            return Err(EdhrecDataError::Invalid(format!(
                "The aggregate import exceeded the {} MiB safety limit.",
                MAX_IMPORT_BYTES / (1024 * 1024)
            )));
        }
        let bytes = fs::read(path)?;
        self.import_authorized_json(&bytes)
    }

    /// Imports bounded aggregate JSON bytes after the privacy guard and closed
    /// schema validation. This method performs no network activity.
    pub(crate) fn import_authorized_json(
        &self,
        bytes: &[u8],
    ) -> Result<EdhrecImportOutcome, EdhrecDataError> {
        let data = parse_authorized_import(bytes, MAX_IMPORT_BYTES)?;
        let digest = aggregate_digest(&data)?;
        if self
            .load_active()?
            .as_ref()
            .is_some_and(|snapshot| snapshot.snapshot_sha256 == digest)
        {
            return Ok(EdhrecImportOutcome::Unchanged {
                status: self.status()?,
            });
        }

        let snapshot = EdhrecAggregateSnapshot {
            schema_version: EDHREC_AGGREGATE_SCHEMA_VERSION.into(),
            derivation_version: EDHREC_DERIVATION_VERSION.into(),
            installed_at: Utc::now().to_rfc3339(),
            snapshot_sha256: digest,
            source_bytes: bytes.len() as u64,
            data,
        };
        validate_snapshot(&snapshot, Utc::now())?;
        self.activate(&snapshot)?;
        Ok(EdhrecImportOutcome::Installed {
            status: self.status()?,
        })
    }

    pub(crate) fn load_active(&self) -> Result<Option<EdhrecAggregateSnapshot>, EdhrecDataError> {
        // Re-check on every read so corruption discovered after process
        // startup cannot trap status or replacement imports behind a restart.
        self.recover_previous_if_needed()?;
        let path = self.live_path();
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(load_snapshot_file(&path)?))
    }

    pub(crate) fn status(&self) -> Result<EdhrecDataStatus, EdhrecDataError> {
        match self.load_active()? {
            Some(snapshot) => {
                let (scope_count, card_fact_count) = snapshot_counts(&snapshot.data);
                let mut message = "Authorized aggregate counts are installed as optional research context. Current ratings do not consume them."
                    .to_string();
                if self.corrupt_path().exists() {
                    message = "A damaged newer aggregate snapshot was quarantined and the previous authorized snapshot was restored."
                        .into();
                }
                Ok(EdhrecDataStatus {
                    ready: true,
                    schema_version: snapshot.schema_version,
                    derivation_version: snapshot.derivation_version,
                    generated_at: Some(snapshot.data.generated_at),
                    installed_at: Some(snapshot.installed_at),
                    snapshot_sha256: Some(snapshot.snapshot_sha256),
                    source_bytes: Some(snapshot.source_bytes),
                    time_window: Some(snapshot.data.time_window),
                    scope_count,
                    card_fact_count,
                    provider_name: Some(snapshot.data.access.provider_name),
                    authorization_basis: Some(snapshot.data.access.basis),
                    license_or_agreement: Some(snapshot.data.access.license_or_agreement),
                    authorization_expires_at: snapshot.data.access.expires_at,
                    redistribution_allowed: snapshot.data.access.redistribution_allowed,
                    attribution: Some(snapshot.data.access.attribution),
                    authenticity_basis:
                        "Caller-supplied authorization metadata plus a local SHA-256 integrity digest; no publisher signature is verified."
                            .into(),
                    message,
                })
            }
            None => Ok(empty_status(self.corrupt_path().exists())),
        }
    }

    fn activate(&self, snapshot: &EdhrecAggregateSnapshot) -> Result<(), EdhrecDataError> {
        let next = self.next_path();
        let live = self.live_path();
        let previous = self.previous_path();
        if next.exists() {
            fs::remove_file(&next)?;
        }
        let encoded = serde_json::to_vec(snapshot)?;
        if encoded.len() as u64 > MAX_STORED_BYTES {
            return Err(EdhrecDataError::Invalid(
                "The encoded aggregate snapshot exceeded the stored-file safety limit.".into(),
            ));
        }
        let mut staged_file = fs::File::create(&next)?;
        staged_file.write_all(&encoded)?;
        staged_file.sync_all()?;
        drop(staged_file);
        let staged = load_snapshot_file(&next)?;
        if staged.snapshot_sha256 != snapshot.snapshot_sha256 {
            return Err(EdhrecDataError::Invalid(
                "The staged aggregate snapshot digest changed before activation.".into(),
            ));
        }

        if previous.exists() {
            fs::remove_file(&previous)?;
        }
        let had_live = live.exists();
        if had_live {
            fs::rename(&live, &previous)?;
        }
        if let Err(error) = fs::rename(&next, &live) {
            if had_live && !live.exists() {
                let _ = fs::rename(&previous, &live);
            }
            return Err(error.into());
        }
        if self.corrupt_path().exists() {
            let _ = fs::remove_file(self.corrupt_path());
        }
        Ok(())
    }

    fn recover_previous_if_needed(&self) -> Result<(), EdhrecDataError> {
        let live = self.live_path();
        let previous = self.previous_path();
        if !live.exists() {
            if previous.exists() && load_snapshot_file(&previous).is_ok() {
                fs::rename(previous, live)?;
            }
            return Ok(());
        }
        if load_snapshot_file(&live).is_ok() {
            return Ok(());
        }

        let previous_is_valid = previous.exists() && load_snapshot_file(&previous).is_ok();
        let corrupt = self.corrupt_path();
        if corrupt.exists() {
            fs::remove_file(&corrupt)?;
        }
        fs::rename(&live, &corrupt)?;
        if previous_is_valid && let Err(error) = fs::rename(&previous, &live) {
            let _ = fs::rename(&corrupt, &live);
            return Err(error.into());
        }
        Ok(())
    }

    fn live_path(&self) -> PathBuf {
        self.root.join("edhrec-aggregates.json")
    }

    fn next_path(&self) -> PathBuf {
        self.root.join("edhrec-aggregates.next.json")
    }

    fn previous_path(&self) -> PathBuf {
        self.root.join("edhrec-aggregates.previous.json")
    }

    fn corrupt_path(&self) -> PathBuf {
        self.root.join("edhrec-aggregates.corrupt.json")
    }
}

fn parse_authorized_import(
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<EdhrecAggregateImport, EdhrecDataError> {
    if bytes.is_empty() {
        return Err(EdhrecDataError::Invalid(
            "The aggregate import was empty.".into(),
        ));
    }
    if bytes.len() > maximum_bytes {
        return Err(EdhrecDataError::Invalid(format!(
            "The aggregate import exceeded the {} byte safety limit.",
            maximum_bytes
        )));
    }
    let raw: Value = serde_json::from_slice(bytes)?;
    reject_private_or_raw_fields(&raw, "$")?;
    let data: EdhrecAggregateImport = serde_json::from_value(raw)?;
    validate_import(&data, Utc::now())?;
    Ok(data)
}

fn reject_private_or_raw_fields(value: &Value, path: &str) -> Result<(), EdhrecDataError> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let normalized = normalize_privacy_key(key);
                if is_forbidden_privacy_key(&normalized) {
                    return Err(EdhrecDataError::Privacy(format!(
                        "field `{path}.{key}` looks like player-level, account, match, or raw decklist data"
                    )));
                }
                let child_path = format!("{path}.{key}");
                reject_private_or_raw_fields(child, &child_path)?;
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                let child_path = format!("{path}[{index}]");
                reject_private_or_raw_fields(child, &child_path)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn normalize_privacy_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_forbidden_privacy_key(key: &str) -> bool {
    matches!(
        key,
        "player"
            | "players"
            | "playerid"
            | "playerids"
            | "playername"
            | "playernames"
            | "user"
            | "users"
            | "userid"
            | "userids"
            | "username"
            | "usernames"
            | "email"
            | "emails"
            | "emailaddress"
            | "emailaddresses"
            | "account"
            | "accounts"
            | "accountid"
            | "accountids"
            | "owner"
            | "owners"
            | "ownerid"
            | "ownerids"
            | "profile"
            | "profiles"
            | "profileid"
            | "profileids"
            | "discordid"
            | "deck"
            | "decks"
            | "deckid"
            | "deckids"
            | "deckname"
            | "decknames"
            | "deckurl"
            | "deckurls"
            | "decklist"
            | "decklists"
            | "rawdecklist"
            | "rawdecklists"
            | "maindeck"
            | "mainboard"
            | "sideboard"
            | "maybeboard"
            | "considering"
            | "matchid"
            | "matchids"
            | "gameid"
            | "gameids"
            | "podid"
            | "podids"
            | "replay"
            | "replays"
            | "gamelog"
            | "gamelogs"
    )
}

fn validate_import(
    data: &EdhrecAggregateImport,
    now: DateTime<Utc>,
) -> Result<(), EdhrecDataError> {
    if data.schema_version != EDHREC_AGGREGATE_SCHEMA_VERSION {
        return Err(EdhrecDataError::Invalid(format!(
            "Schema version {:?} is unsupported; expected {}.",
            data.schema_version, EDHREC_AGGREGATE_SCHEMA_VERSION
        )));
    }
    let generated_at = parse_timestamp("generatedAt", &data.generated_at)?;
    if generated_at > now + chrono::Duration::days(1) {
        return Err(EdhrecDataError::Invalid(
            "generatedAt is implausibly far in the future.".into(),
        ));
    }
    validate_time_window(&data.time_window, generated_at.date_naive())?;
    validate_access(&data.access, generated_at, now)?;

    if data.source_mix.is_empty() || data.source_mix.len() > MAX_SOURCE_MIX_ENTRIES {
        return Err(EdhrecDataError::Invalid(format!(
            "sourceMix must contain 1..={MAX_SOURCE_MIX_ENTRIES} entries."
        )));
    }
    let mut source_ids = HashSet::new();
    for source in &data.source_mix {
        validate_identifier("sourceMix.sourceId", &source.source_id)?;
        validate_text(
            "sourceMix.sourceName",
            &source.source_name,
            MAX_SHORT_TEXT_BYTES,
            false,
        )?;
        validate_text("sourceMix.notes", &source.notes, MAX_NOTES_BYTES, false)?;
        if !source_ids.insert(source.source_id.as_str()) {
            return Err(EdhrecDataError::Invalid(format!(
                "sourceMix contains duplicate sourceId {:?}.",
                source.source_id
            )));
        }
    }
    validate_text(
        "deduplicationNotes",
        &data.deduplication_notes,
        MAX_NOTES_BYTES,
        false,
    )?;

    if data.scopes.is_empty() || data.scopes.len() > MAX_SCOPES {
        return Err(EdhrecDataError::Invalid(format!(
            "scopes must contain 1..={MAX_SCOPES} entries."
        )));
    }
    let mut scope_keys = HashSet::new();
    let mut total_card_facts = 0usize;
    for scope in &data.scopes {
        validate_scope(scope)?;
        let scope_key = normalized_scope_key(scope);
        if !scope_keys.insert(scope_key) {
            return Err(EdhrecDataError::Invalid(
                "Duplicate commander/partner/theme scope detected, including reversed commander pairs."
                    .into(),
            ));
        }
        total_card_facts = total_card_facts
            .checked_add(scope.cards.len())
            .ok_or_else(|| EdhrecDataError::Invalid("Card-fact count overflowed.".into()))?;
        if total_card_facts > MAX_TOTAL_CARD_FACTS {
            return Err(EdhrecDataError::Invalid(format!(
                "The snapshot exceeded the {MAX_TOTAL_CARD_FACTS} card-fact limit."
            )));
        }
    }
    Ok(())
}

fn validate_time_window(
    window: &EdhrecTimeWindow,
    generated_date: NaiveDate,
) -> Result<(), EdhrecDataError> {
    let start = parse_date("timeWindow.startDate", &window.start_date)?;
    let end = parse_date("timeWindow.endDate", &window.end_date)?;
    if start > end {
        return Err(EdhrecDataError::Invalid(
            "timeWindow.startDate must not be after endDate.".into(),
        ));
    }
    if end > generated_date {
        return Err(EdhrecDataError::Invalid(
            "timeWindow.endDate must not be after the generatedAt date.".into(),
        ));
    }
    if let Some(label) = window.label.as_deref() {
        validate_text("timeWindow.label", label, MAX_SHORT_TEXT_BYTES, false)?;
    }
    Ok(())
}

fn validate_access(
    access: &EdhrecAccessAuthorization,
    generated_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(), EdhrecDataError> {
    if access.provider_id != "edhrec" {
        return Err(EdhrecDataError::Invalid(
            "access.providerId must be exactly `edhrec` for schema version 1.".into(),
        ));
    }
    validate_text(
        "access.providerName",
        &access.provider_name,
        MAX_SHORT_TEXT_BYTES,
        false,
    )?;
    validate_text(
        "access.authorizationReference",
        &access.authorization_reference,
        MAX_SHORT_TEXT_BYTES,
        false,
    )?;
    validate_text(
        "access.licenseOrAgreement",
        &access.license_or_agreement,
        MAX_SHORT_TEXT_BYTES,
        false,
    )?;
    validate_text(
        "access.attribution",
        &access.attribution,
        MAX_ATTRIBUTION_BYTES,
        false,
    )?;
    if let Some(url) = access.terms_url.as_deref() {
        validate_https_url("access.termsUrl", url)?;
    }
    if !access.aggregate_only {
        return Err(EdhrecDataError::Invalid(
            "access.aggregateOnly must be true; raw decklists are not accepted.".into(),
        ));
    }
    if !access.local_cache_allowed {
        return Err(EdhrecDataError::Invalid(
            "The declared authorization does not permit local caching.".into(),
        ));
    }
    if !access.derived_analysis_allowed {
        return Err(EdhrecDataError::Invalid(
            "The declared authorization does not permit derived local analysis.".into(),
        ));
    }

    let authorized_at = parse_timestamp("access.authorizedAt", &access.authorized_at)?;
    if authorized_at > generated_at {
        return Err(EdhrecDataError::Invalid(
            "access.authorizedAt must not be after generatedAt.".into(),
        ));
    }
    if let Some(expires_at) = access.expires_at.as_deref() {
        let expiry = parse_timestamp("access.expiresAt", expires_at)?;
        if expiry <= authorized_at {
            return Err(EdhrecDataError::Invalid(
                "access.expiresAt must be after authorizedAt.".into(),
            ));
        }
        if expiry <= now {
            return Err(EdhrecDataError::Invalid(
                "The declared aggregate-data authorization has expired.".into(),
            ));
        }
    }
    Ok(())
}

fn validate_scope(scope: &EdhrecCommanderScope) -> Result<(), EdhrecDataError> {
    validate_oracle_id("commanderOracleId", &scope.commander_oracle_id)?;
    if let Some(partner) = scope.partner_oracle_id.as_deref() {
        validate_oracle_id("partnerOracleId", partner)?;
        if partner == scope.commander_oracle_id {
            return Err(EdhrecDataError::Invalid(
                "partnerOracleId must differ from commanderOracleId.".into(),
            ));
        }
    }
    validate_popularity("commanderPopularity", &scope.commander_popularity)?;
    if let Some(theme) = &scope.theme {
        validate_identifier("theme.themeId", &theme.theme_id)?;
        validate_identifier("theme.themeVersion", &theme.theme_version)?;
        if let Some(name) = theme.display_name.as_deref() {
            validate_text("theme.displayName", name, MAX_SHORT_TEXT_BYTES, false)?;
        }
        validate_popularity("theme.popularity", &theme.popularity)?;
        if theme.popularity.deck_count > scope.commander_popularity.deck_count {
            return Err(EdhrecDataError::Invalid(
                "Theme deckCount cannot exceed commander deckCount.".into(),
            ));
        }
    }

    if scope.cards.is_empty() || scope.cards.len() > MAX_CARDS_PER_SCOPE {
        return Err(EdhrecDataError::Invalid(format!(
            "Each scope must contain 1..={MAX_CARDS_PER_SCOPE} card aggregates."
        )));
    }
    let scope_deck_count = scope.scope_deck_count();
    let mut card_ids = HashSet::new();
    for card in &scope.cards {
        validate_oracle_id("cardOracleId", &card.card_oracle_id)?;
        if !card_ids.insert(card.card_oracle_id.as_str()) {
            return Err(EdhrecDataError::Invalid(format!(
                "Scope contains duplicate cardOracleId {:?}.",
                card.card_oracle_id
            )));
        }
        validate_count("inclusionDeckCount", card.inclusion_deck_count)?;
        validate_count("eligibleDeckCount", card.eligible_deck_count)?;
        validate_count(
            "colorIdentityInclusionDeckCount",
            card.color_identity_inclusion_deck_count,
        )?;
        validate_count(
            "colorIdentityEligibleDeckCount",
            card.color_identity_eligible_deck_count,
        )?;
        if card.eligible_deck_count == 0 || card.color_identity_eligible_deck_count == 0 {
            return Err(EdhrecDataError::Invalid(
                "Card aggregate denominators must be greater than zero.".into(),
            ));
        }
        if card.inclusion_deck_count > card.eligible_deck_count {
            return Err(EdhrecDataError::Invalid(
                "inclusionDeckCount cannot exceed eligibleDeckCount.".into(),
            ));
        }
        if card.color_identity_inclusion_deck_count > card.color_identity_eligible_deck_count {
            return Err(EdhrecDataError::Invalid(
                "colorIdentityInclusionDeckCount cannot exceed its eligible denominator.".into(),
            ));
        }
        if card.eligible_deck_count > scope_deck_count {
            return Err(EdhrecDataError::Invalid(
                "eligibleDeckCount cannot exceed the commander/theme scope deckCount.".into(),
            ));
        }
        if card.derived_metrics().is_none() {
            return Err(EdhrecDataError::Invalid(
                "Card aggregate metrics could not be derived from their counts.".into(),
            ));
        }
    }
    Ok(())
}

fn validate_popularity(label: &str, value: &EdhrecPopularity) -> Result<(), EdhrecDataError> {
    validate_count(&format!("{label}.deckCount"), value.deck_count)?;
    if value.deck_count == 0 {
        return Err(EdhrecDataError::Invalid(format!(
            "{label}.deckCount must be greater than zero."
        )));
    }
    if let Some(rank) = value.rank
        && !(1..=MAX_RANK).contains(&rank)
    {
        return Err(EdhrecDataError::Invalid(format!(
            "{label}.rank must be in 1..={MAX_RANK}."
        )));
    }
    Ok(())
}

fn validate_count(label: &str, value: u64) -> Result<(), EdhrecDataError> {
    if value > MAX_DECK_COUNT {
        return Err(EdhrecDataError::Invalid(format!(
            "{label} exceeded the {MAX_DECK_COUNT} safety cap."
        )));
    }
    Ok(())
}

fn normalized_scope_key(scope: &EdhrecCommanderScope) -> String {
    let mut commander_ids = vec![scope.commander_oracle_id.as_str()];
    if let Some(partner) = scope.partner_oracle_id.as_deref() {
        commander_ids.push(partner);
    }
    commander_ids.sort_unstable();
    let (theme_id, theme_version) = scope.theme.as_ref().map_or(("", ""), |theme| {
        (theme.theme_id.as_str(), theme.theme_version.as_str())
    });
    format!("{}|{theme_id}|{theme_version}", commander_ids.join("+"))
}

fn validate_oracle_id(label: &str, value: &str) -> Result<(), EdhrecDataError> {
    let bytes = value.as_bytes();
    let valid = bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase(),
        })
        && bytes.iter().any(|byte| *byte != b'0' && *byte != b'-');
    if !valid {
        return Err(EdhrecDataError::Invalid(format!(
            "{label} must be a canonical lowercase Oracle UUID."
        )));
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), EdhrecDataError> {
    validate_text(label, value, MAX_IDENTIFIER_BYTES, false)?;
    if !value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
    }) {
        return Err(EdhrecDataError::Invalid(format!(
            "{label} may contain only ASCII letters, digits, hyphen, underscore, period, or colon."
        )));
    }
    Ok(())
}

fn validate_text(
    label: &str,
    value: &str,
    maximum_bytes: usize,
    allow_empty: bool,
) -> Result<(), EdhrecDataError> {
    if (!allow_empty && value.trim().is_empty()) || value.len() > maximum_bytes {
        return Err(EdhrecDataError::Invalid(format!(
            "{label} must be non-empty and no longer than {maximum_bytes} UTF-8 bytes."
        )));
    }
    if value
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(EdhrecDataError::Invalid(format!(
            "{label} contains unsupported control characters."
        )));
    }
    Ok(())
}

fn validate_https_url(label: &str, value: &str) -> Result<(), EdhrecDataError> {
    let url = Url::parse(value)
        .map_err(|error| EdhrecDataError::Invalid(format!("{label} is invalid: {error}")))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(EdhrecDataError::Invalid(format!(
            "{label} must be an HTTPS URL without embedded credentials."
        )));
    }
    Ok(())
}

fn parse_timestamp(label: &str, value: &str) -> Result<DateTime<Utc>, EdhrecDataError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| EdhrecDataError::Invalid(format!("{label} must be an RFC 3339 timestamp.")))
}

fn parse_date(label: &str, value: &str) -> Result<NaiveDate, EdhrecDataError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| EdhrecDataError::Invalid(format!("{label} must use YYYY-MM-DD.")))
}

fn aggregate_digest(data: &EdhrecAggregateImport) -> Result<String, EdhrecDataError> {
    let canonical = serde_json::to_vec(data)?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
}

fn validate_snapshot(
    snapshot: &EdhrecAggregateSnapshot,
    now: DateTime<Utc>,
) -> Result<(), EdhrecDataError> {
    if snapshot.schema_version != EDHREC_AGGREGATE_SCHEMA_VERSION
        || snapshot.derivation_version != EDHREC_DERIVATION_VERSION
        || snapshot.data.schema_version != EDHREC_AGGREGATE_SCHEMA_VERSION
    {
        return Err(EdhrecDataError::Invalid(
            "The installed EDHREC aggregate snapshot uses an unsupported schema or derivation version."
                .into(),
        ));
    }
    parse_timestamp("installedAt", &snapshot.installed_at)?;
    if snapshot.source_bytes == 0 || snapshot.source_bytes > MAX_IMPORT_BYTES as u64 {
        return Err(EdhrecDataError::Invalid(
            "sourceBytes was outside the supported import range.".into(),
        ));
    }
    if snapshot.snapshot_sha256.len() != 64
        || !snapshot
            .snapshot_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(EdhrecDataError::Invalid(
            "snapshotSha256 must be a lowercase SHA-256 digest.".into(),
        ));
    }
    validate_import(&snapshot.data, now)?;
    if aggregate_digest(&snapshot.data)? != snapshot.snapshot_sha256 {
        return Err(EdhrecDataError::Invalid(
            "The aggregate snapshot SHA-256 did not match its canonical data.".into(),
        ));
    }
    Ok(())
}

fn load_snapshot_file(path: &Path) -> Result<EdhrecAggregateSnapshot, EdhrecDataError> {
    if fs::metadata(path)?.len() > MAX_STORED_BYTES {
        return Err(EdhrecDataError::Invalid(
            "The installed aggregate snapshot exceeded the stored-file safety limit.".into(),
        ));
    }
    let bytes = fs::read(path)?;
    let snapshot: EdhrecAggregateSnapshot = serde_json::from_slice(&bytes)?;
    validate_snapshot(&snapshot, Utc::now())?;
    Ok(snapshot)
}

fn snapshot_counts(data: &EdhrecAggregateImport) -> (u64, u64) {
    (
        data.scopes.len() as u64,
        data.scopes
            .iter()
            .map(|scope| scope.cards.len() as u64)
            .sum(),
    )
}

fn empty_status(had_corrupt_snapshot: bool) -> EdhrecDataStatus {
    EdhrecDataStatus {
        ready: false,
        schema_version: EDHREC_AGGREGATE_SCHEMA_VERSION.into(),
        derivation_version: EDHREC_DERIVATION_VERSION.into(),
        generated_at: None,
        installed_at: None,
        snapshot_sha256: None,
        source_bytes: None,
        time_window: None,
        scope_count: 0,
        card_fact_count: 0,
        provider_name: None,
        authorization_basis: None,
        license_or_agreement: None,
        authorization_expires_at: None,
        redistribution_allowed: false,
        attribution: None,
        authenticity_basis:
            "No authorized aggregate snapshot is installed; website scraping is intentionally unsupported."
                .into(),
        message: if had_corrupt_snapshot {
            "A damaged local aggregate snapshot was quarantined. Import a valid provider-authorized aggregate file to restore this optional data source."
                .into()
        } else {
            "No authorized EDHREC aggregate file is installed. Direct website access and undocumented endpoints are intentionally unsupported."
                .into()
        },
    }
}
