use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::domain::CardDefinition;
use crate::parser::normalize_card_name;

pub(crate) const BUNDLED_SEMANTIC_OVERRIDE_JSON: &str =
    include_str!("../data/semantic-overrides.json");
pub(crate) const BUNDLED_SEMANTIC_OVERRIDE_VERSION: &str = "semantic-overrides-2026.07.23-r0";
pub(crate) const SEMANTIC_OVERRIDE_SCHEMA_VERSION: u16 = 1;

const PACKAGE_SIZE_LIMIT: u64 = 4 * 1024 * 1024;
const ACTIVATION_POINTER_SIZE_LIMIT: u64 = 16 * 1024;
const ACTIVATION_SCHEMA_VERSION: u16 = 1;
const MAXIMUM_SOURCES: usize = 128;
const MAXIMUM_OVERRIDES: usize = 4_096;
const MAXIMUM_CITATIONS_PER_OVERRIDE: usize = 8;
const MAXIMUM_UNSUPPORTED_CLAUSES: usize = 16;
const MAXIMUM_SHORT_TEXT_BYTES: usize = 240;
const MAXIMUM_URL_BYTES: usize = 2_048;
static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, thiserror::Error)]
pub enum SemanticStoreError {
    #[error("Semantic-annotation package file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("The semantic-annotation package is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("The semantic-annotation package is invalid: {0}")]
    Invalid(String),
    #[error("Semantic-annotation activation metadata is invalid: {0}")]
    Activation(String),
    #[error("The selected semantic-annotation package is invalid: {0}")]
    Selection(String),
    #[error("The selected package would downgrade the active semantic annotations: {0}")]
    Downgrade(String),
    #[error("The selected package conflicts with the active semantic annotations: {0}")]
    Conflict(String),
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SemanticPackageOrigin {
    Bundled,
    LocalImport,
    BundledFallback,
}

impl SemanticPackageOrigin {
    pub fn as_cache_value(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::LocalImport => "local-import",
            Self::BundledFallback => "bundled-fallback",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SemanticPackageProvenance {
    pub origin: SemanticPackageOrigin,
    pub snapshot_sha256: String,
    pub imported_at: Option<String>,
    pub authenticity_basis: String,
    pub warning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SemanticPackageSnapshot {
    pub package: SemanticOverridePackage,
    pub provenance: SemanticPackageProvenance,
}

impl SemanticPackageSnapshot {
    pub fn status(&self) -> SemanticPackageStatus {
        SemanticPackageStatus {
            ready: true,
            origin: self.provenance.origin,
            schema_version: self.package.schema_version,
            package_version: self.package.package_version.clone(),
            effective_date: self.package.effective_date.clone(),
            verified_at: self.package.verified_at.clone(),
            snapshot_sha256: self.provenance.snapshot_sha256.clone(),
            imported_at: self.provenance.imported_at.clone(),
            source_count: self.package.sources.len() as u32,
            override_count: self.package.overrides.len() as u32,
            authenticity_basis: self.provenance.authenticity_basis.clone(),
            message: self.provenance.warning.clone().unwrap_or_else(|| {
                match self.provenance.origin {
                    SemanticPackageOrigin::Bundled => {
                        "Using the empty semantic-annotation fallback bundled with this app build."
                            .into()
                    }
                    SemanticPackageOrigin::LocalImport => {
                        "Using a structurally validated local semantic-annotation package.".into()
                    }
                    SemanticPackageOrigin::BundledFallback => {
                        "Using the bundled empty semantic-annotation fallback.".into()
                    }
                }
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticPackageStatus {
    pub ready: bool,
    pub origin: SemanticPackageOrigin,
    pub schema_version: u16,
    pub package_version: String,
    pub effective_date: String,
    pub verified_at: String,
    pub snapshot_sha256: String,
    pub imported_at: Option<String>,
    pub source_count: u32,
    pub override_count: u32,
    pub authenticity_basis: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticImportOutcome {
    pub activated: bool,
    pub status: SemanticPackageStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticOverridePackage {
    pub schema_version: u16,
    pub package_version: String,
    pub effective_date: String,
    pub verified_at: String,
    pub sources: Vec<SemanticSource>,
    pub overrides: Vec<CardSemanticOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticSource {
    pub title: String,
    pub url: String,
    pub accessed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardSemanticOverride {
    #[serde(default)]
    pub oracle_id: Option<String>,
    #[serde(default)]
    pub normalized_name: Option<String>,
    #[serde(default)]
    pub oracle_text_sha256: Option<String>,
    #[serde(default)]
    pub add_roles: Vec<SemanticRole>,
    #[serde(default)]
    pub remove_roles: Vec<SemanticRole>,
    #[serde(default)]
    pub semantic_confidence: Option<f32>,
    #[serde(default)]
    pub effect_support: Option<EffectSupportMetadata>,
    pub source_citations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectSupportMetadata {
    #[serde(default)]
    pub supported_effects: Vec<SupportedEffect>,
    #[serde(default)]
    pub unsupported_clauses: Vec<String>,
}

/// Closed role vocabulary. Package data can only toggle these established
/// semantic signals; it cannot inject executable code or direct numeric
/// bracket-score/win-turn fields.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum SemanticRole {
    ManaSource,
    Ramp,
    FastMana,
    Draw,
    Tutor,
    Removal,
    Counterspell,
    BoardWipe,
    Protection,
    Engine,
    Enabler,
    Payoff,
    WinCondition,
    ComboPiece,
    Graveyard,
    Token,
    Sacrifice,
    Stax,
    Recursion,
    SpellMatters,
    ArtifactMatters,
    EnchantmentMatters,
    TokenMatters,
    DeathMatters,
    CreatureMatters,
}

/// Describes which already-supported descriptor families were manually
/// reviewed. It deliberately carries no magnitude, turn, target, or score.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum SupportedEffect {
    DrawCards,
    ImpulseAccess,
    Tutor,
    LandsToBattlefield,
    ManaProduction,
    CreatureTokens,
    TreasureTokens,
    TargetedRemoval,
    BoardWipe,
    MassLandDenial,
    Protection,
    Recursion,
    ExtraTurns,
}

#[derive(Debug, Clone, Copy)]
pub enum OverrideMatch<'a> {
    None,
    Applied(&'a CardSemanticOverride),
    OracleTextGuardMismatch,
}

impl SemanticOverridePackage {
    pub fn validate(&self) -> Result<(), SemanticStoreError> {
        if self.schema_version != SEMANTIC_OVERRIDE_SCHEMA_VERSION {
            return Err(SemanticStoreError::Invalid(format!(
                "schemaVersion must be {SEMANTIC_OVERRIDE_SCHEMA_VERSION}"
            )));
        }
        validate_version(&self.package_version)?;
        let effective = package_date("effectiveDate", &self.effective_date)?;
        let verified = package_date("verifiedAt", &self.verified_at)?;
        if verified < effective {
            return Err(SemanticStoreError::Invalid(
                "verifiedAt cannot be before effectiveDate".into(),
            ));
        }
        if self.sources.len() > MAXIMUM_SOURCES {
            return Err(SemanticStoreError::Invalid(format!(
                "sources cannot contain more than {MAXIMUM_SOURCES} entries"
            )));
        }
        if self.overrides.len() > MAXIMUM_OVERRIDES {
            return Err(SemanticStoreError::Invalid(format!(
                "overrides cannot contain more than {MAXIMUM_OVERRIDES} entries"
            )));
        }
        if !self.overrides.is_empty() && self.sources.is_empty() {
            return Err(SemanticStoreError::Invalid(
                "non-empty packages require at least one HTTPS source".into(),
            ));
        }

        let mut source_urls = HashSet::new();
        for source in &self.sources {
            validate_short_text("source title", &source.title)?;
            validate_https_url("source URL", &source.url)?;
            let accessed = package_date("source accessedAt", &source.accessed_at)?;
            if accessed > verified {
                return Err(SemanticStoreError::Invalid(format!(
                    "source {} was accessed after verifiedAt",
                    source.url
                )));
            }
            if !source_urls.insert(source.url.as_str()) {
                return Err(SemanticStoreError::Invalid(format!(
                    "duplicate source URL {}",
                    source.url
                )));
            }
        }

        let mut oracle_ids = HashSet::new();
        let mut normalized_names = HashSet::new();
        for annotation in &self.overrides {
            annotation.validate(&source_urls)?;
            if let Some(oracle_id) = &annotation.oracle_id
                && !oracle_ids.insert(oracle_id.as_str())
            {
                return Err(SemanticStoreError::Invalid(format!(
                    "duplicate oracleId {oracle_id}"
                )));
            }
            if let Some(normalized_name) = &annotation.normalized_name
                && !normalized_names.insert(normalized_name.as_str())
            {
                return Err(SemanticStoreError::Invalid(format!(
                    "duplicate normalizedName {normalized_name}"
                )));
            }
        }
        Ok(())
    }

    pub fn match_card<'a>(&'a self, card: &CardDefinition) -> OverrideMatch<'a> {
        let exact_oracle_match = card.oracle_id.as_deref().and_then(|oracle_id| {
            self.overrides
                .iter()
                .find(|annotation| annotation.oracle_id.as_deref() == Some(oracle_id))
        });
        let name_match = self.overrides.iter().find(|annotation| {
            annotation.normalized_name.as_deref() == Some(card.normalized_name.as_str())
                && (card.oracle_id.is_none() || annotation.oracle_id.is_none())
        });
        let Some(annotation) = exact_oracle_match.or(name_match) else {
            return OverrideMatch::None;
        };
        if annotation
            .oracle_text_sha256
            .as_deref()
            .is_some_and(|expected| expected != oracle_text_sha256(&card.oracle_text))
        {
            OverrideMatch::OracleTextGuardMismatch
        } else {
            OverrideMatch::Applied(annotation)
        }
    }
}

impl CardSemanticOverride {
    fn validate(&self, source_urls: &HashSet<&str>) -> Result<(), SemanticStoreError> {
        if self.oracle_id.is_none() && self.normalized_name.is_none() {
            return Err(SemanticStoreError::Invalid(
                "each override requires oracleId or normalizedName".into(),
            ));
        }
        if let Some(oracle_id) = &self.oracle_id
            && !is_canonical_uuid(oracle_id)
        {
            return Err(SemanticStoreError::Invalid(format!(
                "oracleId {oracle_id} must be a lowercase canonical UUID"
            )));
        }
        if let Some(normalized_name) = &self.normalized_name {
            validate_short_text("normalizedName", normalized_name)?;
            if normalize_card_name(normalized_name) != *normalized_name {
                return Err(SemanticStoreError::Invalid(format!(
                    "normalizedName {normalized_name} is not in canonical normalized form"
                )));
            }
        }
        if let Some(hash) = &self.oracle_text_sha256
            && !is_lowercase_sha256(hash)
        {
            return Err(SemanticStoreError::Invalid(
                "oracleTextSha256 must contain exactly 64 lowercase hexadecimal characters".into(),
            ));
        }
        ensure_unique("addRoles", &self.add_roles)?;
        ensure_unique("removeRoles", &self.remove_roles)?;
        if self
            .add_roles
            .iter()
            .any(|role| self.remove_roles.contains(role))
        {
            return Err(SemanticStoreError::Invalid(
                "the same role cannot appear in addRoles and removeRoles".into(),
            ));
        }
        if let Some(confidence) = self.semantic_confidence
            && (!confidence.is_finite() || !(0.0..=0.99).contains(&confidence))
        {
            return Err(SemanticStoreError::Invalid(
                "semanticConfidence must be a finite number from 0 through 0.99".into(),
            ));
        }
        if let Some(metadata) = &self.effect_support {
            metadata.validate()?;
        }
        let changes_semantics = !self.add_roles.is_empty()
            || !self.remove_roles.is_empty()
            || self.semantic_confidence.is_some()
            || self.effect_support.as_ref().is_some_and(|metadata| {
                !metadata.supported_effects.is_empty() || !metadata.unsupported_clauses.is_empty()
            });
        if !changes_semantics {
            return Err(SemanticStoreError::Invalid(
                "each override must declare at least one semantic change".into(),
            ));
        }
        if self.source_citations.is_empty()
            || self.source_citations.len() > MAXIMUM_CITATIONS_PER_OVERRIDE
        {
            return Err(SemanticStoreError::Invalid(format!(
                "sourceCitations must contain 1 to {MAXIMUM_CITATIONS_PER_OVERRIDE} URLs"
            )));
        }
        let mut citations = HashSet::new();
        for citation in &self.source_citations {
            validate_https_url("source citation", citation)?;
            if !source_urls.contains(citation.as_str()) {
                return Err(SemanticStoreError::Invalid(format!(
                    "source citation {citation} is not declared in package sources"
                )));
            }
            if !citations.insert(citation.as_str()) {
                return Err(SemanticStoreError::Invalid(format!(
                    "duplicate source citation {citation}"
                )));
            }
        }
        Ok(())
    }
}

impl EffectSupportMetadata {
    fn validate(&self) -> Result<(), SemanticStoreError> {
        ensure_unique("supportedEffects", &self.supported_effects)?;
        if self.unsupported_clauses.len() > MAXIMUM_UNSUPPORTED_CLAUSES {
            return Err(SemanticStoreError::Invalid(format!(
                "unsupportedClauses cannot contain more than {MAXIMUM_UNSUPPORTED_CLAUSES} entries"
            )));
        }
        let mut normalized = HashSet::new();
        for clause in &self.unsupported_clauses {
            validate_short_text("unsupported clause", clause)?;
            if !normalized.insert(clause.to_lowercase()) {
                return Err(SemanticStoreError::Invalid(format!(
                    "duplicate unsupported clause {clause}"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum ActiveSelection {
    #[default]
    Runtime,
    Bundled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActivationPointer {
    schema_version: u16,
    #[serde(default)]
    selection: ActiveSelection,
    snapshot_sha256: String,
    package_version: String,
    effective_date: String,
    imported_at: String,
}

#[derive(Debug, Clone)]
pub struct SemanticStore {
    root: PathBuf,
}

impl SemanticStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, SemanticStoreError> {
        let store = Self { root: root.into() };
        fs::create_dir_all(store.packages_directory())?;
        store.recover_interrupted_activation()?;
        store.cleanup_staged_files();
        match store.load_runtime_package() {
            Ok(Some(snapshot)) => {
                store.prune_inactive_generations(&snapshot.provenance.snapshot_sha256)
            }
            Ok(None) => store.prune_inactive_generations(""),
            Err(_) => {}
        }
        let _ = bundled_semantic_package()?;
        Ok(store)
    }

    pub fn status(&self) -> Result<SemanticPackageStatus, SemanticStoreError> {
        Ok(self.load_active()?.status())
    }

    pub fn load_active(&self) -> Result<SemanticPackageSnapshot, SemanticStoreError> {
        let bundled = bundled_semantic_snapshot(SemanticPackageOrigin::Bundled, None)?;
        match self.load_runtime_package() {
            Ok(Some(snapshot)) => {
                if let Some(reason) = runtime_is_superseded_by_bundled(&snapshot, &bundled)? {
                    bundled_semantic_snapshot(SemanticPackageOrigin::BundledFallback, Some(reason))
                } else {
                    Ok(snapshot)
                }
            }
            Ok(None) => Ok(bundled),
            Err(_) => bundled_semantic_snapshot(
                SemanticPackageOrigin::BundledFallback,
                Some(
                    "The locally imported semantic annotations could not be verified; using the bundled empty fallback."
                        .into(),
                ),
            ),
        }
    }

    /// Imports only the explicit user-selected JSON file. Source citations are
    /// validated as HTTPS references but are never fetched by this operation.
    pub fn import_local_file(
        &self,
        selected_path: &Path,
    ) -> Result<SemanticImportOutcome, SemanticStoreError> {
        let bytes = read_selected_package_file(selected_path)?;
        let candidate = parse_and_validate_package(&bytes)?;
        validate_not_future(&candidate)?;
        let candidate_sha256 = sha256_hex(&bytes);
        let current = self.load_active()?;
        if candidate_sha256 == current.provenance.snapshot_sha256 {
            return Ok(SemanticImportOutcome {
                activated: false,
                status: current.status(),
                message: "That exact semantic-annotation snapshot is already active.".into(),
            });
        }
        validate_forward_activation(&current, &candidate, &candidate_sha256)?;

        self.install_generation(&candidate_sha256, &bytes)?;
        let pointer = ActivationPointer {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            selection: ActiveSelection::Runtime,
            snapshot_sha256: candidate_sha256.clone(),
            package_version: candidate.package_version,
            effective_date: candidate.effective_date,
            imported_at: Utc::now().to_rfc3339(),
        };
        self.activate_pointer(&pointer)?;
        let active = self.load_runtime_package()?.ok_or_else(|| {
            SemanticStoreError::Activation("activation pointer disappeared".into())
        })?;
        if active.provenance.snapshot_sha256 != candidate_sha256 {
            return Err(SemanticStoreError::Activation(
                "the activated snapshot hash did not match the imported package".into(),
            ));
        }
        self.prune_inactive_generations(&candidate_sha256);
        Ok(SemanticImportOutcome {
            activated: true,
            status: active.status(),
            message: "The local semantic-annotation package was validated and activated.".into(),
        })
    }

    pub fn reset_to_bundled(&self) -> Result<SemanticImportOutcome, SemanticStoreError> {
        let current = self.load_active()?;
        if current.provenance.origin == SemanticPackageOrigin::Bundled {
            self.prune_inactive_generations("");
            return Ok(SemanticImportOutcome {
                activated: false,
                status: current.status(),
                message: "The bundled semantic-annotation package is already active.".into(),
            });
        }

        let bundled = bundled_semantic_snapshot(SemanticPackageOrigin::Bundled, None)?;
        let pointer = ActivationPointer {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            selection: ActiveSelection::Bundled,
            snapshot_sha256: bundled.provenance.snapshot_sha256.clone(),
            package_version: bundled.package.package_version.clone(),
            effective_date: bundled.package.effective_date.clone(),
            imported_at: Utc::now().to_rfc3339(),
        };
        self.activate_pointer(&pointer)?;
        if self.load_runtime_package()?.is_some() {
            return Err(SemanticStoreError::Activation(
                "reset verification unexpectedly selected a runtime semantic package".into(),
            ));
        }
        self.prune_inactive_generations("");
        let status = self.load_active()?.status();
        if status.origin != SemanticPackageOrigin::Bundled {
            return Err(SemanticStoreError::Activation(
                "reset verification did not report bundled semantic provenance".into(),
            ));
        }
        Ok(SemanticImportOutcome {
            activated: true,
            status,
            message: "Reset semantic annotations to the empty package bundled with this app build."
                .into(),
        })
    }

    fn load_runtime_package(&self) -> Result<Option<SemanticPackageSnapshot>, SemanticStoreError> {
        let pointer_path = self.active_pointer_path();
        if !pointer_path.exists() {
            return Ok(None);
        }
        let pointer = read_activation_pointer(&pointer_path)?;
        if matches!(pointer.selection, ActiveSelection::Bundled) {
            return Ok(None);
        }
        let package_path = self.package_path(&pointer.snapshot_sha256)?;
        let bytes = read_bounded_file(&package_path, PACKAGE_SIZE_LIMIT)?;
        let actual_sha256 = sha256_hex(&bytes);
        if actual_sha256 != pointer.snapshot_sha256 {
            return Err(SemanticStoreError::Activation(format!(
                "active package hash mismatch: expected {}, found {actual_sha256}",
                pointer.snapshot_sha256
            )));
        }
        let package = parse_and_validate_package(&bytes)?;
        validate_not_future(&package)?;
        if package.package_version != pointer.package_version
            || package.effective_date != pointer.effective_date
        {
            return Err(SemanticStoreError::Activation(
                "active package metadata does not match its activation pointer".into(),
            ));
        }
        Ok(Some(SemanticPackageSnapshot {
            package,
            provenance: SemanticPackageProvenance {
                origin: SemanticPackageOrigin::LocalImport,
                snapshot_sha256: actual_sha256,
                imported_at: Some(pointer.imported_at),
                authenticity_basis:
                    "User-selected local JSON; schema, bounds, citations, and SHA-256 were verified, but no digital signature was available."
                        .into(),
                warning: None,
            },
        }))
    }

    fn install_generation(&self, sha256: &str, bytes: &[u8]) -> Result<(), SemanticStoreError> {
        let destination = self.package_path(sha256)?;
        if destination.exists() {
            let installed = read_bounded_file(&destination, PACKAGE_SIZE_LIMIT)?;
            if sha256_hex(&installed) != sha256 {
                return Err(SemanticStoreError::Conflict(format!(
                    "stored generation {sha256} does not match its filename"
                )));
            }
            return Ok(());
        }
        let temporary = self.temporary_path("package");
        write_new_synced_file(&temporary, bytes)?;
        let readback = read_bounded_file(&temporary, PACKAGE_SIZE_LIMIT)?;
        if sha256_hex(&readback) != sha256 {
            let _ = fs::remove_file(&temporary);
            return Err(SemanticStoreError::Activation(
                "staged package failed its SHA-256 readback check".into(),
            ));
        }
        parse_and_validate_package(&readback)?;
        if let Err(error) = fs::rename(&temporary, &destination) {
            let _ = fs::remove_file(&temporary);
            if destination.exists() {
                let installed = read_bounded_file(&destination, PACKAGE_SIZE_LIMIT)?;
                if sha256_hex(&installed) == sha256 {
                    return Ok(());
                }
            }
            return Err(error.into());
        }
        Ok(())
    }

    fn activate_pointer(&self, pointer: &ActivationPointer) -> Result<(), SemanticStoreError> {
        let active = self.active_pointer_path();
        let backup = self.backup_pointer_path();
        let next = self.temporary_path("activation");
        let encoded = serde_json::to_vec_pretty(pointer)
            .map_err(|error| SemanticStoreError::Activation(error.to_string()))?;
        write_new_synced_file(&next, &encoded)?;

        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        let had_active = active.exists();
        if had_active && let Err(error) = fs::rename(&active, &backup) {
            let _ = fs::remove_file(&next);
            return Err(error.into());
        }
        if let Err(error) = fs::rename(&next, &active) {
            let _ = fs::remove_file(&next);
            if had_active && let Err(rollback_error) = fs::rename(&backup, &active) {
                return Err(SemanticStoreError::Activation(format!(
                    "activation pointer swap failed ({error}) and the previous pointer could not be restored ({rollback_error}); the backup was retained for startup recovery"
                )));
            }
            return Err(error.into());
        }
        let verification_error = match (pointer.selection, self.load_runtime_package()) {
            (ActiveSelection::Runtime, Ok(Some(snapshot)))
                if snapshot.provenance.snapshot_sha256 == pointer.snapshot_sha256 =>
            {
                None
            }
            (ActiveSelection::Bundled, Ok(None)) => None,
            (ActiveSelection::Runtime, Ok(Some(_))) => {
                Some("activated a different semantic generation".into())
            }
            (ActiveSelection::Runtime, Ok(None)) => {
                Some("runtime activation selected the bundled package".into())
            }
            (ActiveSelection::Bundled, Ok(Some(_))) => {
                Some("bundled reset retained a runtime semantic generation".into())
            }
            (_, Err(error)) => Some(error.to_string()),
        };
        if let Some(error) = verification_error {
            if let Err(removal_error) = fs::remove_file(&active) {
                return Err(SemanticStoreError::Activation(format!(
                    "activation verification failed ({error}) and the failed pointer could not be removed ({removal_error}); the previous backup was retained for startup recovery"
                )));
            }
            if had_active {
                if let Err(rollback_error) = fs::rename(&backup, &active) {
                    return Err(SemanticStoreError::Activation(format!(
                        "activation verification failed ({error}) and the previous pointer could not be restored ({rollback_error}); the backup was retained for startup recovery"
                    )));
                }
                return Err(SemanticStoreError::Activation(format!(
                    "activation verification failed and the previous semantic package was restored: {error}"
                )));
            }
            return Err(SemanticStoreError::Activation(format!(
                "activation verification failed and the failed pointer was removed; the bundled fallback remains available: {error}"
            )));
        }
        if backup.exists() {
            let _ = fs::remove_file(backup);
        }
        Ok(())
    }

    fn recover_interrupted_activation(&self) -> Result<(), SemanticStoreError> {
        let active = self.active_pointer_path();
        let backup = self.backup_pointer_path();
        if !backup.exists() {
            return Ok(());
        }
        let active_is_valid = active.exists() && self.pointer_generation_is_valid(&active);
        if active_is_valid {
            let _ = fs::remove_file(backup);
        } else {
            if active.exists() {
                fs::remove_file(&active)?;
            }
            if self.pointer_generation_is_valid(&backup) {
                fs::rename(backup, active)?;
            } else {
                fs::remove_file(backup)?;
            }
        }
        Ok(())
    }

    fn pointer_generation_is_valid(&self, pointer_path: &Path) -> bool {
        read_activation_pointer(pointer_path)
            .and_then(|pointer| {
                if pointer.selection == ActiveSelection::Bundled {
                    return Ok(());
                }
                let bytes = read_bounded_file(
                    &self.package_path(&pointer.snapshot_sha256)?,
                    PACKAGE_SIZE_LIMIT,
                )?;
                if sha256_hex(&bytes) != pointer.snapshot_sha256 {
                    return Err(SemanticStoreError::Activation(
                        "generation hash mismatch".into(),
                    ));
                }
                let package = parse_and_validate_package(&bytes)?;
                validate_not_future(&package)?;
                if package.package_version != pointer.package_version
                    || package.effective_date != pointer.effective_date
                {
                    return Err(SemanticStoreError::Activation(
                        "generation metadata mismatch".into(),
                    ));
                }
                Ok(())
            })
            .is_ok()
    }

    fn prune_inactive_generations(&self, active_sha256: &str) {
        let Ok(entries) = fs::read_dir(self.packages_directory()) else {
            return;
        };
        let active_file_name = format!("{active_sha256}.json");
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let is_generation = file_name.strip_suffix(".json").is_some_and(is_sha256);
            if file_name != active_file_name && is_generation {
                let _ = fs::remove_file(path);
            }
        }
    }

    fn cleanup_staged_files(&self) {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if file_name.starts_with(".package-") || file_name.starts_with(".activation-") {
                let _ = fs::remove_file(path);
            }
        }
    }

    fn package_path(&self, sha256: &str) -> Result<PathBuf, SemanticStoreError> {
        if !is_sha256(sha256) {
            return Err(SemanticStoreError::Activation(
                "snapshot SHA-256 must contain exactly 64 hexadecimal characters".into(),
            ));
        }
        Ok(self.packages_directory().join(format!("{sha256}.json")))
    }

    fn packages_directory(&self) -> PathBuf {
        self.root.join("packages")
    }

    fn active_pointer_path(&self) -> PathBuf {
        self.root.join("active.json")
    }

    fn backup_pointer_path(&self) -> PathBuf {
        self.root.join("active.previous.json")
    }

    fn temporary_path(&self, purpose: &str) -> PathBuf {
        let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        self.root
            .join(format!(".{purpose}-{}-{sequence}.tmp", std::process::id()))
    }
}

pub(crate) fn bundled_semantic_package() -> Result<SemanticOverridePackage, SemanticStoreError> {
    let package = serde_json::from_str::<SemanticOverridePackage>(BUNDLED_SEMANTIC_OVERRIDE_JSON)?;
    package.validate()?;
    if package.package_version != BUNDLED_SEMANTIC_OVERRIDE_VERSION {
        return Err(SemanticStoreError::Invalid(format!(
            "bundled version constant {} does not match package {}",
            BUNDLED_SEMANTIC_OVERRIDE_VERSION, package.package_version
        )));
    }
    Ok(package)
}

pub(crate) fn bundled_semantic_snapshot(
    origin: SemanticPackageOrigin,
    warning: Option<String>,
) -> Result<SemanticPackageSnapshot, SemanticStoreError> {
    Ok(SemanticPackageSnapshot {
        package: bundled_semantic_package()?,
        provenance: SemanticPackageProvenance {
            origin,
            snapshot_sha256: sha256_hex(BUNDLED_SEMANTIC_OVERRIDE_JSON.as_bytes()),
            imported_at: None,
            authenticity_basis:
                "Bundled with the app build; the app does not independently verify a digital signature."
                    .into(),
            warning,
        },
    })
}

pub(crate) fn oracle_text_sha256(oracle_text: &str) -> String {
    sha256_hex(oracle_text.as_bytes())
}

fn runtime_is_superseded_by_bundled(
    runtime: &SemanticPackageSnapshot,
    bundled: &SemanticPackageSnapshot,
) -> Result<Option<String>, SemanticStoreError> {
    if runtime.package.package_version == bundled.package.package_version {
        return Ok(
            (runtime.provenance.snapshot_sha256 != bundled.provenance.snapshot_sha256).then(|| {
                format!(
                    "The locally imported semantic package uses bundled version {} with conflicting bytes; using the bundled fallback.",
                    bundled.package.package_version
                )
            }),
        );
    }
    let runtime_date = package_date("effectiveDate", &runtime.package.effective_date)?;
    let bundled_date = package_date("effectiveDate", &bundled.package.effective_date)?;
    let superseded = runtime_date < bundled_date
        || runtime_date == bundled_date
            && !matches!(
                (
                    revision_number(&runtime.package.package_version),
                    revision_number(&bundled.package.package_version),
                ),
                (Some(runtime_revision), Some(bundled_revision))
                    if runtime_revision > bundled_revision
            );
    Ok(superseded.then(|| {
        format!(
            "The locally imported semantic package {} (effective {}) does not supersede bundled {} (effective {}); using the bundled fallback.",
            runtime.package.package_version,
            runtime.package.effective_date,
            bundled.package.package_version,
            bundled.package.effective_date
        )
    }))
}

fn validate_forward_activation(
    current: &SemanticPackageSnapshot,
    candidate: &SemanticOverridePackage,
    candidate_sha256: &str,
) -> Result<(), SemanticStoreError> {
    if candidate.package_version == current.package.package_version {
        return Err(SemanticStoreError::Conflict(format!(
            "version {} is already active with SHA-256 {}; the conflicting candidate has SHA-256 {candidate_sha256}",
            candidate.package_version, current.provenance.snapshot_sha256
        )));
    }
    let current_date = package_date("active effectiveDate", &current.package.effective_date)?;
    let candidate_date = package_date("candidate effectiveDate", &candidate.effective_date)?;
    if candidate_date < current_date {
        return Err(SemanticStoreError::Downgrade(format!(
            "candidate {} is effective {}, before active {} ({})",
            candidate.package_version,
            candidate.effective_date,
            current.package.package_version,
            current.package.effective_date
        )));
    }
    if candidate_date == current_date {
        let current_revision = revision_number(&current.package.package_version);
        let candidate_revision = revision_number(&candidate.package_version);
        if !matches!((current_revision, candidate_revision), (Some(old), Some(new)) if new > old) {
            return Err(SemanticStoreError::Downgrade(format!(
                "packages with the same effective date must increase a numeric -r revision (active {}, candidate {})",
                current.package.package_version, candidate.package_version
            )));
        }
    }
    Ok(())
}

fn validate_not_future(package: &SemanticOverridePackage) -> Result<(), SemanticStoreError> {
    let effective = package_date("effectiveDate", &package.effective_date)?;
    let verified = package_date("verifiedAt", &package.verified_at)?;
    let today = Utc::now().date_naive();
    if effective > today || verified > today {
        return Err(SemanticStoreError::Invalid(
            "effectiveDate and verifiedAt cannot be later than the current UTC date".into(),
        ));
    }
    Ok(())
}

fn read_selected_package_file(path: &Path) -> Result<Vec<u8>, SemanticStoreError> {
    if !path.is_absolute() {
        return Err(SemanticStoreError::Selection(
            "choose an absolute local JSON file path".into(),
        ));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("json"))
    {
        return Err(SemanticStoreError::Selection(
            "semantic-annotation packages must use the .json extension".into(),
        ));
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(SemanticStoreError::Selection(
            "the selected path must be a regular local file, not a directory or symbolic link"
                .into(),
        ));
    }
    if metadata.len() == 0 || metadata.len() > PACKAGE_SIZE_LIMIT {
        return Err(SemanticStoreError::Selection(format!(
            "semantic-annotation packages must contain 1 to {PACKAGE_SIZE_LIMIT} bytes"
        )));
    }
    let canonical = fs::canonicalize(path)?;
    if !is_local_disk_path(&canonical) {
        return Err(SemanticStoreError::Selection(
            "semantic-annotation packages must be selected from a local disk".into(),
        ));
    }
    // Open the canonical target once and validate/read that same handle. This
    // avoids reopening a user-controlled path after the metadata checks.
    let file = File::open(&canonical)?;
    let opened_metadata = file.metadata()?;
    if !opened_metadata.file_type().is_file()
        || opened_metadata.len() == 0
        || opened_metadata.len() > PACKAGE_SIZE_LIMIT
    {
        return Err(SemanticStoreError::Selection(format!(
            "semantic-annotation packages must be regular files containing 1 to {PACKAGE_SIZE_LIMIT} bytes"
        )));
    }
    let mut bytes = Vec::with_capacity(opened_metadata.len() as usize);
    file.take(PACKAGE_SIZE_LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > PACKAGE_SIZE_LIMIT {
        return Err(SemanticStoreError::Selection(format!(
            "semantic-annotation packages cannot exceed {PACKAGE_SIZE_LIMIT} bytes"
        )));
    }
    Ok(bytes)
}

#[cfg(windows)]
fn is_local_disk_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
    )
}

#[cfg(not(windows))]
fn is_local_disk_path(path: &Path) -> bool {
    path.is_absolute()
}

fn read_activation_pointer(path: &Path) -> Result<ActivationPointer, SemanticStoreError> {
    let bytes = read_bounded_file(path, ACTIVATION_POINTER_SIZE_LIMIT)?;
    let pointer: ActivationPointer = serde_json::from_slice(&bytes)
        .map_err(|error| SemanticStoreError::Activation(error.to_string()))?;
    if pointer.schema_version != ACTIVATION_SCHEMA_VERSION || pointer.imported_at.trim().is_empty()
    {
        return Err(SemanticStoreError::Activation(
            "unsupported pointer schema or missing import timestamp".into(),
        ));
    }
    if !is_sha256(&pointer.snapshot_sha256) {
        return Err(SemanticStoreError::Activation(
            "pointer contains an invalid SHA-256".into(),
        ));
    }
    let imported_at = DateTime::parse_from_rfc3339(&pointer.imported_at).map_err(|_| {
        SemanticStoreError::Activation("importedAt must be an RFC 3339 timestamp".into())
    })?;
    if imported_at.with_timezone(&Utc) > Utc::now() + Duration::minutes(5) {
        return Err(SemanticStoreError::Activation(
            "importedAt cannot be more than five minutes in the future".into(),
        ));
    }
    Ok(pointer)
}

fn read_bounded_file(path: &Path, limit: u64) -> Result<Vec<u8>, SemanticStoreError> {
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    if length == 0 || length > limit {
        return Err(SemanticStoreError::Selection(format!(
            "{} must contain 1 to {limit} bytes",
            path.display()
        )));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(SemanticStoreError::Selection(format!(
            "{} exceeds the {limit}-byte limit",
            path.display()
        )));
    }
    Ok(bytes)
}

fn write_new_synced_file(path: &Path, bytes: &[u8]) -> Result<(), SemanticStoreError> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

fn parse_and_validate_package(bytes: &[u8]) -> Result<SemanticOverridePackage, SemanticStoreError> {
    let package = serde_json::from_slice::<SemanticOverridePackage>(bytes)?;
    package.validate()?;
    Ok(package)
}

fn validate_version(value: &str) -> Result<(), SemanticStoreError> {
    if !(3..=80).contains(&value.len())
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-_".contains(character))
        || revision_number(value).is_none()
    {
        return Err(SemanticStoreError::Invalid(
            "packageVersion must be 3 to 80 ASCII letters, digits, '.', '-', or '_' and end in -rN"
                .into(),
        ));
    }
    Ok(())
}

fn package_date(field: &str, value: &str) -> Result<NaiveDate, SemanticStoreError> {
    let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        SemanticStoreError::Invalid(format!("{field} must use the exact YYYY-MM-DD format"))
    })?;
    if parsed.format("%Y-%m-%d").to_string() != value {
        return Err(SemanticStoreError::Invalid(format!(
            "{field} must use the exact YYYY-MM-DD format"
        )));
    }
    Ok(parsed)
}

fn validate_short_text(field: &str, value: &str) -> Result<(), SemanticStoreError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAXIMUM_SHORT_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(SemanticStoreError::Invalid(format!(
            "{field} must contain 1 to {MAXIMUM_SHORT_TEXT_BYTES} trimmed bytes without control characters"
        )));
    }
    Ok(())
}

fn validate_https_url(field: &str, value: &str) -> Result<(), SemanticStoreError> {
    if value.len() > MAXIMUM_URL_BYTES {
        return Err(SemanticStoreError::Invalid(format!(
            "{field} exceeds {MAXIMUM_URL_BYTES} bytes"
        )));
    }
    let url = Url::parse(value)
        .map_err(|_| SemanticStoreError::Invalid(format!("{field} is not a valid URL")))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(SemanticStoreError::Invalid(format!(
            "{field} must be an HTTPS URL with a host and no embedded credentials"
        )));
    }
    Ok(())
}

fn ensure_unique<T>(field: &str, values: &[T]) -> Result<(), SemanticStoreError>
where
    T: Copy + Eq + std::hash::Hash,
{
    let mut unique = HashSet::new();
    if values.iter().any(|value| !unique.insert(*value)) {
        return Err(SemanticStoreError::Invalid(format!(
            "{field} cannot contain duplicates"
        )));
    }
    Ok(())
}

fn is_canonical_uuid(value: &str) -> bool {
    value.len() == 36
        && value == value.to_ascii_lowercase()
        && value.chars().enumerate().all(|(index, character)| {
            if [8, 13, 18, 23].contains(&index) {
                character == '-'
            } else {
                character.is_ascii_hexdigit()
            }
        })
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn revision_number(version: &str) -> Option<u64> {
    version.rsplit_once("-r")?.1.parse().ok()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
