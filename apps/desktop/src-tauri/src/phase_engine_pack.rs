//! Local, integrity-checked store for a separately built phase-rs worker pack.
//!
//! There is intentionally no downloader and no worker build target here. A
//! future, separately authorized process may import a local pack. The store
//! validates every byte into a private staging directory, atomically switches
//! a small activation pointer, retains exactly one rollback pointer, and
//! re-verifies the active pack before returning it.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::execution_coverage::{
    EXECUTION_COVERAGE_COMPILER_VERSION, EXECUTION_COVERAGE_SCHEMA_VERSION,
};
use crate::phase_worker_protocol::{
    PHASE_ENGINE_NAME, PHASE_SOURCE_REPOSITORY, PHASE_WORKER_PROTOCOL_VERSION,
    WorkerDataSourceProvenance, WorkerProvenance,
};

pub const PHASE_ENGINE_PACK_SCHEMA_VERSION: &str = "commander-phase-engine-pack/v1";
pub const PHASE_ENGINE_PACK_STORE_VERSION: &str = "phase-engine-pack-store-0.1";

const MANIFEST_FILE_NAME: &str = "engine-pack.json";
const ACTIVATION_SCHEMA_VERSION: &str = "commander-phase-engine-pack-activation/v1";
const MANIFEST_SIZE_LIMIT: u64 = 256 * 1024;
const ACTIVATION_SIZE_LIMIT: u64 = 16 * 1024;
const MAXIMUM_FILE_COUNT: usize = 256;
const MAXIMUM_DIRECTORY_DEPTH: usize = 8;
const MAXIMUM_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAXIMUM_WORKER_BYTES: u64 = 128 * 1024 * 1024;
const MAXIMUM_PACK_BYTES: u64 = 1024 * 1024 * 1024;
const MAXIMUM_PATH_BYTES: usize = 240;
const MAXIMUM_VERSION_BYTES: usize = 128;
const MAXIMUM_DATA_NOTICE_PATHS: usize = 8;
const MTGJSON_SOURCE_NAME: &str = "MTGJSON";
const MTGJSON_SOURCE_URL: &str = "https://mtgjson.com/";
const MTGJSON_LICENSE_EXPRESSION: &str = "MIT";
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, thiserror::Error)]
pub enum PhaseEnginePackError {
    #[error("Phase engine-pack file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Phase engine-pack JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Phase engine pack is invalid: {0}")]
    Invalid(String),
    #[error("Phase engine-pack selection is invalid: {0}")]
    Selection(String),
    #[error("Phase engine-pack activation failed: {0}")]
    Activation(String),
    #[error("Phase engine-pack integrity verification failed: {0}")]
    Integrity(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhaseEnginePackManifest {
    pub schema_version: String,
    pub protocol_version: String,
    pub pack_version: String,
    pub created_at: String,
    pub host_execution_coverage_schema: String,
    pub host_execution_coverage_compiler: String,
    pub engine: PhaseEngineDescriptor,
    pub card_data_source: PhaseEnginePackDataSource,
    pub rules_data_source: PhaseEnginePackDataSource,
    pub license_expression: String,
    pub files: Vec<PhaseEnginePackFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhaseEngineDescriptor {
    pub name: String,
    pub version: String,
    pub source_repository: String,
    pub source_revision: String,
    pub source_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhaseEnginePackDataSource {
    pub name: String,
    pub source_url: String,
    pub version: String,
    pub revision: String,
    pub source_artifact_path: String,
    pub source_artifact_sha256: String,
    pub content_sha256: String,
    pub license_expression: String,
    pub attribution: String,
    pub notice_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PhaseEnginePackFileRole {
    WorkerExecutable,
    EngineSource,
    SourceData,
    CardData,
    RulesData,
    License,
    Notice,
    RuntimeData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhaseEnginePackFile {
    pub path: String,
    pub role: PhaseEnginePackFileRole,
    pub bytes: u64,
    pub sha256: String,
}

impl PhaseEnginePackManifest {
    pub fn validate(&self) -> Result<(), PhaseEnginePackError> {
        if self.schema_version != PHASE_ENGINE_PACK_SCHEMA_VERSION {
            return Err(PhaseEnginePackError::Invalid(format!(
                "schemaVersion must be {PHASE_ENGINE_PACK_SCHEMA_VERSION}"
            )));
        }
        if self.protocol_version != PHASE_WORKER_PROTOCOL_VERSION {
            return Err(PhaseEnginePackError::Invalid(format!(
                "protocolVersion must be {PHASE_WORKER_PROTOCOL_VERSION}"
            )));
        }
        validate_version("packVersion", &self.pack_version)?;
        if self.host_execution_coverage_schema != EXECUTION_COVERAGE_SCHEMA_VERSION
            || self.host_execution_coverage_compiler != EXECUTION_COVERAGE_COMPILER_VERSION
        {
            return Err(PhaseEnginePackError::Invalid(format!(
                "pack must pin host execution coverage {EXECUTION_COVERAGE_SCHEMA_VERSION} / {EXECUTION_COVERAGE_COMPILER_VERSION}"
            )));
        }
        let created_at = DateTime::parse_from_rfc3339(&self.created_at).map_err(|_| {
            PhaseEnginePackError::Invalid("createdAt must be an RFC 3339 timestamp".into())
        })?;
        if created_at.with_timezone(&Utc) > Utc::now() + chrono::Duration::minutes(5) {
            return Err(PhaseEnginePackError::Invalid(
                "createdAt cannot be materially in the future".into(),
            ));
        }
        self.engine.validate()?;
        if self.license_expression != "MIT OR Apache-2.0" {
            return Err(PhaseEnginePackError::Invalid(
                "licenseExpression must preserve phase-rs dual licensing as MIT OR Apache-2.0"
                    .into(),
            ));
        }
        if self.files.len() < 9 || self.files.len() > MAXIMUM_FILE_COUNT {
            return Err(PhaseEnginePackError::Invalid(format!(
                "files must contain 9 through {MAXIMUM_FILE_COUNT} entries"
            )));
        }

        let mut exact_paths = HashSet::new();
        let mut folded_paths = HashSet::new();
        let mut role_counts = HashMap::<PhaseEnginePackFileRole, usize>::new();
        let mut total_bytes = 0_u64;
        for file in &self.files {
            validate_relative_pack_path(&file.path)?;
            if file.path.eq_ignore_ascii_case(MANIFEST_FILE_NAME) {
                return Err(PhaseEnginePackError::Invalid(
                    "engine-pack.json is metadata and cannot appear in files".into(),
                ));
            }
            if !exact_paths.insert(file.path.as_str())
                || !folded_paths.insert(file.path.to_ascii_lowercase())
            {
                return Err(PhaseEnginePackError::Invalid(format!(
                    "duplicate or case-colliding pack path {}",
                    file.path
                )));
            }
            if file.bytes == 0 || file.bytes > MAXIMUM_FILE_BYTES {
                return Err(PhaseEnginePackError::Invalid(format!(
                    "{} must contain 1 through {MAXIMUM_FILE_BYTES} bytes",
                    file.path
                )));
            }
            if file.role == PhaseEnginePackFileRole::WorkerExecutable
                && file.bytes > MAXIMUM_WORKER_BYTES
            {
                return Err(PhaseEnginePackError::Invalid(format!(
                    "worker executable exceeds the {MAXIMUM_WORKER_BYTES}-byte limit"
                )));
            }
            validate_sha256(&format!("{} sha256", file.path), &file.sha256)?;
            total_bytes = total_bytes.checked_add(file.bytes).ok_or_else(|| {
                PhaseEnginePackError::Invalid("declared pack size overflow".into())
            })?;
            *role_counts.entry(file.role).or_default() += 1;
        }
        if total_bytes > MAXIMUM_PACK_BYTES {
            return Err(PhaseEnginePackError::Invalid(format!(
                "declared pack exceeds the {MAXIMUM_PACK_BYTES}-byte limit"
            )));
        }

        for required in [
            PhaseEnginePackFileRole::WorkerExecutable,
            PhaseEnginePackFileRole::EngineSource,
            PhaseEnginePackFileRole::CardData,
            PhaseEnginePackFileRole::RulesData,
        ] {
            if role_counts.get(&required).copied() != Some(1) {
                return Err(PhaseEnginePackError::Invalid(format!(
                    "pack requires exactly one {required:?} file"
                )));
            }
        }
        if role_counts
            .get(&PhaseEnginePackFileRole::License)
            .copied()
            .unwrap_or_default()
            < 2
        {
            return Err(PhaseEnginePackError::Invalid(
                "pack requires both MIT and Apache-2.0 license texts".into(),
            ));
        }
        let worker = self
            .file_for_role(PhaseEnginePackFileRole::WorkerExecutable)
            .expect("required worker role was checked");
        if Path::new(&worker.path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("exe"))
        {
            return Err(PhaseEnginePackError::Invalid(
                "the Windows worker executable path must end in .exe".into(),
            ));
        }
        let source = self
            .file_for_role(PhaseEnginePackFileRole::EngineSource)
            .expect("required source role was checked");
        if source.sha256 != self.engine.source_sha256 {
            return Err(PhaseEnginePackError::Invalid(
                "engine.sourceSha256 must equal the verified engine-source archive file".into(),
            ));
        }
        self.card_data_source.validate(
            "cardDataSource",
            &self.files,
            self.file_for_role(PhaseEnginePackFileRole::CardData)
                .expect("required card-data role was checked"),
        )?;
        if self.card_data_source.name != MTGJSON_SOURCE_NAME
            || self.card_data_source.source_url != MTGJSON_SOURCE_URL
            || self.card_data_source.license_expression != MTGJSON_LICENSE_EXPRESSION
            || !self
                .card_data_source
                .attribution
                .contains(MTGJSON_SOURCE_NAME)
            || self
                .files
                .iter()
                .find(|file| file.path == self.card_data_source.source_artifact_path)
                .is_none_or(|file| file.role != PhaseEnginePackFileRole::SourceData)
        {
            return Err(PhaseEnginePackError::Invalid(format!(
                "phase-rs v1 card data must identify and attribute {MTGJSON_SOURCE_NAME} at {MTGJSON_SOURCE_URL} under {MTGJSON_LICENSE_EXPRESSION}"
            )));
        }
        self.rules_data_source.validate(
            "rulesDataSource",
            &self.files,
            self.file_for_role(PhaseEnginePackFileRole::RulesData)
                .expect("required rules-data role was checked"),
        )?;
        if self.rules_data_source.name != PHASE_ENGINE_NAME
            || self.rules_data_source.source_url != PHASE_SOURCE_REPOSITORY
            || self.rules_data_source.version != self.engine.version
            || self.rules_data_source.revision != self.engine.source_revision
            || self.rules_data_source.source_artifact_sha256 != self.engine.source_sha256
            || self.rules_data_source.source_artifact_path != source.path
            || self.rules_data_source.license_expression != self.license_expression
            || !self
                .rules_data_source
                .attribution
                .contains(PHASE_ENGINE_NAME)
        {
            return Err(PhaseEnginePackError::Invalid(
                "rulesDataSource must identify the exact pinned phase-rs source and dual license"
                    .into(),
            ));
        }
        Ok(())
    }

    pub fn file_for_role(&self, role: PhaseEnginePackFileRole) -> Option<&PhaseEnginePackFile> {
        self.files.iter().find(|file| file.role == role)
    }
}

impl PhaseEnginePackDataSource {
    fn validate(
        &self,
        field: &str,
        files: &[PhaseEnginePackFile],
        content_file: &PhaseEnginePackFile,
    ) -> Result<(), PhaseEnginePackError> {
        validate_bounded_text(field, "name", &self.name, 256)?;
        validate_https_source_url(field, &self.source_url)?;
        validate_version(&format!("{field}.version"), &self.version)?;
        validate_version(&format!("{field}.revision"), &self.revision)?;
        validate_relative_pack_path(&self.source_artifact_path)?;
        validate_sha256(
            &format!("{field}.sourceArtifactSha256"),
            &self.source_artifact_sha256,
        )?;
        let source_artifact = files
            .iter()
            .find(|file| file.path == self.source_artifact_path)
            .ok_or_else(|| {
                PhaseEnginePackError::Invalid(format!(
                    "{field}.sourceArtifactPath references undeclared file {}",
                    self.source_artifact_path
                ))
            })?;
        if !matches!(
            source_artifact.role,
            PhaseEnginePackFileRole::EngineSource | PhaseEnginePackFileRole::SourceData
        ) || source_artifact.sha256 != self.source_artifact_sha256
        {
            return Err(PhaseEnginePackError::Invalid(format!(
                "{field} source artifact path/hash must match a declared engine-source or source-data file"
            )));
        }
        validate_sha256(&format!("{field}.contentSha256"), &self.content_sha256)?;
        if self.content_sha256 != content_file.sha256 {
            return Err(PhaseEnginePackError::Invalid(format!(
                "{field}.contentSha256 must equal the declared {} payload hash",
                content_file.path
            )));
        }
        validate_bounded_text(field, "licenseExpression", &self.license_expression, 256)?;
        validate_bounded_text(field, "attribution", &self.attribution, 1_024)?;
        if self.notice_paths.is_empty() || self.notice_paths.len() > MAXIMUM_DATA_NOTICE_PATHS {
            return Err(PhaseEnginePackError::Invalid(format!(
                "{field}.noticePaths must contain 1 through {MAXIMUM_DATA_NOTICE_PATHS} declared notice/license paths"
            )));
        }
        let mut notice_paths = HashSet::new();
        for path in &self.notice_paths {
            validate_relative_pack_path(path)?;
            if !notice_paths.insert(path.as_str()) {
                return Err(PhaseEnginePackError::Invalid(format!(
                    "{field}.noticePaths contains duplicate path {path}"
                )));
            }
            let notice = files
                .iter()
                .find(|file| file.path == *path)
                .ok_or_else(|| {
                    PhaseEnginePackError::Invalid(format!(
                        "{field}.noticePaths references undeclared file {path}"
                    ))
                })?;
            if !matches!(
                notice.role,
                PhaseEnginePackFileRole::License | PhaseEnginePackFileRole::Notice
            ) {
                return Err(PhaseEnginePackError::Invalid(format!(
                    "{field}.noticePaths entry {path} is not a license or notice file"
                )));
            }
        }
        Ok(())
    }

    fn to_worker_provenance(&self, files: &[PhaseEnginePackFile]) -> WorkerDataSourceProvenance {
        let notice_sha256s = self
            .notice_paths
            .iter()
            .map(|path| {
                files
                    .iter()
                    .find(|file| file.path == *path)
                    .expect("verified notice path")
                    .sha256
                    .clone()
            })
            .collect();
        WorkerDataSourceProvenance {
            name: self.name.clone(),
            source_url: self.source_url.clone(),
            version: self.version.clone(),
            revision: self.revision.clone(),
            source_artifact_sha256: self.source_artifact_sha256.clone(),
            content_sha256: self.content_sha256.clone(),
            license_expression: self.license_expression.clone(),
            attribution: self.attribution.clone(),
            notice_sha256s,
        }
    }
}

impl PhaseEngineDescriptor {
    fn validate(&self) -> Result<(), PhaseEnginePackError> {
        if self.name != PHASE_ENGINE_NAME {
            return Err(PhaseEnginePackError::Invalid(format!(
                "engine.name must be {PHASE_ENGINE_NAME}"
            )));
        }
        validate_version("engine.version", &self.version)?;
        if self.source_repository != PHASE_SOURCE_REPOSITORY {
            return Err(PhaseEnginePackError::Invalid(format!(
                "engine.sourceRepository must be {PHASE_SOURCE_REPOSITORY}"
            )));
        }
        if !is_lowercase_git_revision(&self.source_revision) {
            return Err(PhaseEnginePackError::Invalid(
                "engine.sourceRevision must be a full 40-character lowercase Git revision".into(),
            ));
        }
        validate_sha256("engine.sourceSha256", &self.source_sha256)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActivationPointer {
    schema_version: String,
    protocol_version: String,
    pack_version: String,
    pack_content_sha256: String,
    manifest_sha256: String,
    activated_at: String,
}

#[derive(Debug, Clone)]
pub struct VerifiedPhaseEnginePack {
    root: PathBuf,
    pub manifest: PhaseEnginePackManifest,
    pub manifest_sha256: String,
    pub pack_content_sha256: String,
}

impl VerifiedPhaseEnginePack {
    pub fn worker_executable_path(&self) -> PathBuf {
        let worker = self
            .manifest
            .file_for_role(PhaseEnginePackFileRole::WorkerExecutable)
            .expect("verified packs always have one worker executable");
        self.root.join(relative_path(&worker.path))
    }

    pub fn provenance(&self) -> WorkerProvenance {
        let worker = self
            .manifest
            .file_for_role(PhaseEnginePackFileRole::WorkerExecutable)
            .expect("verified worker");
        let card_data = self
            .manifest
            .file_for_role(PhaseEnginePackFileRole::CardData)
            .expect("verified card data");
        let rules_data = self
            .manifest
            .file_for_role(PhaseEnginePackFileRole::RulesData)
            .expect("verified rules data");
        WorkerProvenance {
            protocol_version: self.manifest.protocol_version.clone(),
            pack_version: self.manifest.pack_version.clone(),
            pack_content_sha256: self.pack_content_sha256.clone(),
            manifest_sha256: self.manifest_sha256.clone(),
            engine_name: self.manifest.engine.name.clone(),
            engine_version: self.manifest.engine.version.clone(),
            engine_source_repository: self.manifest.engine.source_repository.clone(),
            engine_source_revision: self.manifest.engine.source_revision.clone(),
            engine_source_sha256: self.manifest.engine.source_sha256.clone(),
            worker_executable_sha256: worker.sha256.clone(),
            card_data_sha256: card_data.sha256.clone(),
            rules_data_sha256: rules_data.sha256.clone(),
            card_data_source: self
                .manifest
                .card_data_source
                .to_worker_provenance(&self.manifest.files),
            rules_data_source: self
                .manifest
                .rules_data_source
                .to_worker_provenance(&self.manifest.files),
            host_execution_coverage_schema: self.manifest.host_execution_coverage_schema.clone(),
            host_execution_coverage_compiler: self
                .manifest
                .host_execution_coverage_compiler
                .clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PhaseEnginePackSummary {
    pub pack_version: String,
    pub protocol_version: String,
    pub pack_content_sha256: String,
    pub manifest_sha256: String,
    pub engine_version: String,
    pub engine_source_revision: String,
    pub engine_source_sha256: String,
    pub worker_executable_sha256: String,
    pub card_data_sha256: String,
    pub rules_data_sha256: String,
    pub card_data_source: WorkerDataSourceProvenance,
    pub rules_data_source: WorkerDataSourceProvenance,
    pub host_execution_coverage_schema: String,
    pub host_execution_coverage_compiler: String,
}

impl From<&VerifiedPhaseEnginePack> for PhaseEnginePackSummary {
    fn from(pack: &VerifiedPhaseEnginePack) -> Self {
        let provenance = pack.provenance();
        Self {
            pack_version: provenance.pack_version,
            protocol_version: provenance.protocol_version,
            pack_content_sha256: provenance.pack_content_sha256,
            manifest_sha256: provenance.manifest_sha256,
            engine_version: provenance.engine_version,
            engine_source_revision: provenance.engine_source_revision,
            engine_source_sha256: provenance.engine_source_sha256,
            worker_executable_sha256: provenance.worker_executable_sha256,
            card_data_sha256: provenance.card_data_sha256,
            rules_data_sha256: provenance.rules_data_sha256,
            card_data_source: provenance.card_data_source,
            rules_data_source: provenance.rules_data_source,
            host_execution_coverage_schema: provenance.host_execution_coverage_schema,
            host_execution_coverage_compiler: provenance.host_execution_coverage_compiler,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseEnginePackStatus {
    pub installed: bool,
    pub active: Option<PhaseEnginePackSummary>,
    pub rollback_available: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct PhaseEnginePackStore {
    root: PathBuf,
}

impl PhaseEnginePackStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, PhaseEnginePackError> {
        let store = Self { root: root.into() };
        fs::create_dir_all(store.packs_directory())?;
        store.recover_interrupted_activation()?;
        store.recover_invalid_active()?;
        store.cleanup_staging_directories();
        store.prune_inactive_generations();
        Ok(store)
    }

    pub fn status(&self) -> Result<PhaseEnginePackStatus, PhaseEnginePackError> {
        let active = self.load_active()?;
        let rollback_available = self.load_pointer(&self.previous_pointer_path()).is_ok()
            && self
                .load_verified_from_pointer(&self.previous_pointer_path())
                .is_ok();
        Ok(match active {
            Some(pack) => PhaseEnginePackStatus {
                installed: true,
                active: Some((&pack).into()),
                rollback_available,
                message: if rollback_available {
                    "A verified phase-rs engine pack is installed; one verified rollback generation is retained."
                        .into()
                } else {
                    "A verified phase-rs engine pack is installed; no rollback generation is available."
                        .into()
                },
            },
            None => PhaseEnginePackStatus {
                installed: false,
                active: None,
                rollback_available: false,
                message: "No phase-rs engine pack is installed. The built-in analyzer remains fail-closed for strict card execution."
                    .into(),
            },
        })
    }

    pub fn load_active(&self) -> Result<Option<VerifiedPhaseEnginePack>, PhaseEnginePackError> {
        if !self.active_pointer_path().exists() {
            return Ok(None);
        }
        self.load_verified_from_pointer(&self.active_pointer_path())
            .map(Some)
    }

    /// Verifies and imports an explicit local directory. This operation never
    /// performs a network request and never builds or launches the worker.
    pub fn install_local_pack(
        &self,
        selected_directory: &Path,
    ) -> Result<PhaseEnginePackStatus, PhaseEnginePackError> {
        if !selected_directory.is_absolute() {
            return Err(PhaseEnginePackError::Selection(
                "choose an absolute engine-pack directory".into(),
            ));
        }
        let source = selected_directory.canonicalize().map_err(|error| {
            PhaseEnginePackError::Selection(format!(
                "selected engine-pack directory could not be resolved: {error}"
            ))
        })?;
        if !source.is_dir() {
            return Err(PhaseEnginePackError::Selection(
                "selected engine-pack path is not a directory".into(),
            ));
        }
        if source.starts_with(canonical_or_original(&self.root)) {
            return Err(PhaseEnginePackError::Selection(
                "select a source directory outside the private engine-pack store".into(),
            ));
        }

        let verified_source = verify_pack_directory(&source)?;
        if self
            .load_active()?
            .as_ref()
            .is_some_and(|active| active.pack_content_sha256 == verified_source.pack_content_sha256)
        {
            return self.status();
        }

        let staging = self.unique_staging_path();
        fs::create_dir(&staging)?;
        let copy_result = copy_pack_to_staging(&source, &staging, &verified_source.manifest);
        if let Err(error) = copy_result {
            let _ = remove_internal_directory(&self.root, &staging);
            return Err(error);
        }
        let verified_staging = match verify_pack_directory(&staging) {
            Ok(pack) => pack,
            Err(error) => {
                let _ = remove_internal_directory(&self.root, &staging);
                return Err(error);
            }
        };
        if verified_staging.pack_content_sha256 != verified_source.pack_content_sha256
            || verified_staging.manifest_sha256 != verified_source.manifest_sha256
        {
            let _ = remove_internal_directory(&self.root, &staging);
            return Err(PhaseEnginePackError::Integrity(
                "staged pack identity changed while it was copied".into(),
            ));
        }

        let generation = self
            .packs_directory()
            .join(&verified_staging.pack_content_sha256);
        if generation.exists() {
            let existing = verify_pack_directory(&generation)?;
            if existing.pack_content_sha256 != verified_staging.pack_content_sha256
                || existing.manifest_sha256 != verified_staging.manifest_sha256
            {
                let _ = remove_internal_directory(&self.root, &staging);
                return Err(PhaseEnginePackError::Integrity(
                    "an existing generation directory conflicts with the staged identity".into(),
                ));
            }
            remove_internal_directory(&self.root, &staging)?;
        } else {
            fs::rename(&staging, &generation)?;
        }

        let installed = verify_pack_directory(&generation)?;
        let pointer = ActivationPointer {
            schema_version: ACTIVATION_SCHEMA_VERSION.into(),
            protocol_version: PHASE_WORKER_PROTOCOL_VERSION.into(),
            pack_version: installed.manifest.pack_version.clone(),
            pack_content_sha256: installed.pack_content_sha256.clone(),
            manifest_sha256: installed.manifest_sha256.clone(),
            activated_at: Utc::now().to_rfc3339(),
        };
        self.activate_pointer(&pointer)?;
        let active = self.load_active()?.ok_or_else(|| {
            PhaseEnginePackError::Activation("active pointer disappeared after activation".into())
        })?;
        if active.pack_content_sha256 != installed.pack_content_sha256 {
            return Err(PhaseEnginePackError::Activation(
                "post-activation pack identity does not match the verified staging generation"
                    .into(),
            ));
        }
        self.prune_inactive_generations();
        self.status()
    }

    /// Atomically swaps the active and immediately previous verified
    /// generations. No older generation is retained.
    pub fn rollback(&self) -> Result<PhaseEnginePackStatus, PhaseEnginePackError> {
        let active_path = self.active_pointer_path();
        let previous_path = self.previous_pointer_path();
        self.load_verified_from_pointer(&active_path)?;
        self.load_verified_from_pointer(&previous_path)?;

        let temporary = self.root.join("active.rollback.json");
        remove_internal_file_if_exists(&self.root, &temporary)?;
        fs::rename(&active_path, &temporary)?;
        if let Err(error) = fs::rename(&previous_path, &active_path) {
            let _ = fs::rename(&temporary, &active_path);
            return Err(PhaseEnginePackError::Activation(format!(
                "could not activate rollback pointer: {error}"
            )));
        }
        if let Err(error) = fs::rename(&temporary, &previous_path) {
            return Err(PhaseEnginePackError::Activation(format!(
                "rollback activated, but the former generation could not be retained: {error}"
            )));
        }
        self.load_verified_from_pointer(&active_path)?;
        self.prune_inactive_generations();
        self.status()
    }

    fn activate_pointer(&self, pointer: &ActivationPointer) -> Result<(), PhaseEnginePackError> {
        validate_pointer(pointer)?;
        let next = self.next_pointer_path();
        remove_internal_file_if_exists(&self.root, &next)?;
        write_json_create_new(&next, pointer)?;
        self.load_verified_from_pointer(&next)?;

        let active = self.active_pointer_path();
        let previous = self.previous_pointer_path();
        remove_internal_file_if_exists(&self.root, &previous)?;
        if active.exists() {
            fs::rename(&active, &previous)?;
        }
        if let Err(error) = fs::rename(&next, &active) {
            if previous.exists() {
                let _ = fs::rename(&previous, &active);
            }
            return Err(PhaseEnginePackError::Activation(format!(
                "could not atomically replace the active pointer: {error}"
            )));
        }
        Ok(())
    }

    fn load_verified_from_pointer(
        &self,
        pointer_path: &Path,
    ) -> Result<VerifiedPhaseEnginePack, PhaseEnginePackError> {
        let pointer = self.load_pointer(pointer_path)?;
        let generation = self.packs_directory().join(&pointer.pack_content_sha256);
        let pack = verify_pack_directory(&generation)?;
        if pack.pack_content_sha256 != pointer.pack_content_sha256
            || pack.manifest_sha256 != pointer.manifest_sha256
            || pack.manifest.pack_version != pointer.pack_version
            || pack.manifest.protocol_version != pointer.protocol_version
        {
            return Err(PhaseEnginePackError::Integrity(
                "activation pointer does not match the verified generation".into(),
            ));
        }
        pack.provenance().validate().map_err(|error| {
            PhaseEnginePackError::Integrity(format!("worker provenance is invalid: {error}"))
        })?;
        Ok(pack)
    }

    fn load_pointer(&self, path: &Path) -> Result<ActivationPointer, PhaseEnginePackError> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(PhaseEnginePackError::Integrity(
                "activation pointer must be a regular file".into(),
            ));
        }
        if metadata.len() == 0 || metadata.len() > ACTIVATION_SIZE_LIMIT {
            return Err(PhaseEnginePackError::Integrity(
                "activation pointer size is outside its bound".into(),
            ));
        }
        let bytes = fs::read(path)?;
        let pointer: ActivationPointer = serde_json::from_slice(&bytes)?;
        validate_pointer(&pointer)?;
        Ok(pointer)
    }

    fn recover_interrupted_activation(&self) -> Result<(), PhaseEnginePackError> {
        let active = self.active_pointer_path();
        let previous = self.previous_pointer_path();
        let next = self.next_pointer_path();
        let rollback = self.root.join("active.rollback.json");

        if !active.exists() {
            if previous.exists() && self.load_verified_from_pointer(&previous).is_ok() {
                fs::rename(&previous, &active)?;
            } else if next.exists() && self.load_verified_from_pointer(&next).is_ok() {
                fs::rename(&next, &active)?;
            } else if rollback.exists() && self.load_verified_from_pointer(&rollback).is_ok() {
                fs::rename(&rollback, &active)?;
            }
        }
        if active.exists()
            && !previous.exists()
            && rollback.exists()
            && self.load_verified_from_pointer(&rollback).is_ok()
        {
            fs::rename(&rollback, &previous)?;
        }
        remove_internal_file_if_exists(&self.root, &next)?;
        remove_internal_file_if_exists(&self.root, &rollback)?;
        Ok(())
    }

    fn recover_invalid_active(&self) -> Result<(), PhaseEnginePackError> {
        let active = self.active_pointer_path();
        if !active.exists() || self.load_verified_from_pointer(&active).is_ok() {
            return Ok(());
        }
        let previous = self.previous_pointer_path();
        let quarantine = self.root.join(format!(
            "active.corrupt.{}.json",
            TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::rename(&active, &quarantine)?;
        if previous.exists()
            && self.load_verified_from_pointer(&previous).is_ok()
            && let Err(error) = fs::rename(&previous, &active)
        {
            let _ = fs::rename(&quarantine, &active);
            return Err(PhaseEnginePackError::Activation(format!(
                "could not restore the verified rollback generation: {error}"
            )));
        }
        Ok(())
    }

    fn cleanup_staging_directories(&self) {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(".staging-") {
                let _ = remove_internal_directory(&self.root, &entry.path());
            }
        }
    }

    fn prune_inactive_generations(&self) {
        let mut retained = HashSet::new();
        for pointer_path in [self.active_pointer_path(), self.previous_pointer_path()] {
            if let Ok(pointer) = self.load_pointer(&pointer_path) {
                retained.insert(pointer.pack_content_sha256);
            }
        }
        let Ok(entries) = fs::read_dir(self.packs_directory()) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_lowercase_sha256(&name) && !retained.contains(&name) {
                let _ = remove_internal_directory(&self.root, &entry.path());
            }
        }
    }

    fn unique_staging_path(&self) -> PathBuf {
        self.root.join(format!(
            ".staging-{}-{}",
            std::process::id(),
            TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn packs_directory(&self) -> PathBuf {
        self.root.join("packs")
    }

    fn active_pointer_path(&self) -> PathBuf {
        self.root.join("active.json")
    }

    fn previous_pointer_path(&self) -> PathBuf {
        self.root.join("active.previous.json")
    }

    fn next_pointer_path(&self) -> PathBuf {
        self.root.join("active.next.json")
    }
}

fn verify_pack_directory(root: &Path) -> Result<VerifiedPhaseEnginePack, PhaseEnginePackError> {
    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PhaseEnginePackError::Integrity(
            "engine-pack root must be a real directory, not a symlink".into(),
        ));
    }
    let manifest_path = root.join(MANIFEST_FILE_NAME);
    let manifest_metadata = fs::symlink_metadata(&manifest_path)?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err(PhaseEnginePackError::Integrity(
            "engine-pack manifest must be a regular file".into(),
        ));
    }
    if manifest_metadata.len() == 0 || manifest_metadata.len() > MANIFEST_SIZE_LIMIT {
        return Err(PhaseEnginePackError::Integrity(format!(
            "engine-pack manifest must contain 1 through {MANIFEST_SIZE_LIMIT} bytes"
        )));
    }
    let manifest_bytes = fs::read(&manifest_path)?;
    let manifest: PhaseEnginePackManifest = serde_json::from_slice(&manifest_bytes)?;
    manifest.validate()?;

    let discovered = enumerate_regular_files(root)?;
    let expected = std::iter::once(MANIFEST_FILE_NAME.to_string())
        .chain(manifest.files.iter().map(|file| file.path.clone()))
        .collect::<HashSet<_>>();
    let discovered_paths = discovered.keys().cloned().collect::<HashSet<_>>();
    if expected != discovered_paths {
        let missing = expected
            .difference(&discovered_paths)
            .take(5)
            .cloned()
            .collect::<Vec<_>>();
        let extra = discovered_paths
            .difference(&expected)
            .take(5)
            .cloned()
            .collect::<Vec<_>>();
        return Err(PhaseEnginePackError::Integrity(format!(
            "pack file inventory differs from the manifest (missing: {missing:?}; extra: {extra:?})"
        )));
    }

    for declared in &manifest.files {
        let actual_path = discovered.get(&declared.path).ok_or_else(|| {
            PhaseEnginePackError::Integrity(format!("missing file {}", declared.path))
        })?;
        let metadata = fs::symlink_metadata(actual_path)?;
        if metadata.len() != declared.bytes {
            return Err(PhaseEnginePackError::Integrity(format!(
                "{} byte count differs from the manifest",
                declared.path
            )));
        }
        let actual_sha256 = sha256_file(actual_path, declared.bytes)?;
        if actual_sha256 != declared.sha256 {
            return Err(PhaseEnginePackError::Integrity(format!(
                "{} SHA-256 differs from the manifest",
                declared.path
            )));
        }
    }

    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let pack_content_sha256 = pack_content_digest(&manifest, &manifest_sha256)?;
    Ok(VerifiedPhaseEnginePack {
        root: root.to_path_buf(),
        manifest,
        manifest_sha256,
        pack_content_sha256,
    })
}

fn enumerate_regular_files(root: &Path) -> Result<HashMap<String, PathBuf>, PhaseEnginePackError> {
    fn walk(
        root: &Path,
        directory: &Path,
        depth: usize,
        files: &mut HashMap<String, PathBuf>,
        folded: &mut HashSet<String>,
    ) -> Result<(), PhaseEnginePackError> {
        if depth > MAXIMUM_DIRECTORY_DEPTH {
            return Err(PhaseEnginePackError::Integrity(format!(
                "pack directory nesting exceeds {MAXIMUM_DIRECTORY_DEPTH}"
            )));
        }
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(PhaseEnginePackError::Integrity(format!(
                    "pack cannot contain symlink {}",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                walk(root, &path, depth + 1, files, folded)?;
                continue;
            }
            if !metadata.is_file() {
                return Err(PhaseEnginePackError::Integrity(format!(
                    "pack contains unsupported filesystem object {}",
                    path.display()
                )));
            }
            if files.len() > MAXIMUM_FILE_COUNT {
                return Err(PhaseEnginePackError::Integrity(
                    "pack contains too many files".into(),
                ));
            }
            let relative = path.strip_prefix(root).map_err(|_| {
                PhaseEnginePackError::Integrity("pack file escaped its root".into())
            })?;
            let key = path_to_manifest_string(relative)?;
            validate_relative_pack_path(&key)?;
            if !folded.insert(key.to_ascii_lowercase()) {
                return Err(PhaseEnginePackError::Integrity(format!(
                    "pack contains case-colliding path {key}"
                )));
            }
            files.insert(key, path);
        }
        Ok(())
    }

    let mut files = HashMap::new();
    let mut folded = HashSet::new();
    walk(root, root, 0, &mut files, &mut folded)?;
    Ok(files)
}

fn copy_pack_to_staging(
    source: &Path,
    staging: &Path,
    manifest: &PhaseEnginePackManifest,
) -> Result<(), PhaseEnginePackError> {
    let paths = std::iter::once(MANIFEST_FILE_NAME.to_string())
        .chain(manifest.files.iter().map(|file| file.path.clone()));
    for relative in paths {
        let source_path = source.join(relative_path(&relative));
        let destination = staging.join(relative_path(&relative));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let source_metadata = fs::symlink_metadata(&source_path)?;
        if source_metadata.file_type().is_symlink() || !source_metadata.is_file() {
            return Err(PhaseEnginePackError::Integrity(format!(
                "source file {relative} changed before staging"
            )));
        }
        let mut source_file = File::open(&source_path)?;
        let mut destination_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&destination)?;
        std::io::copy(&mut source_file, &mut destination_file)?;
        destination_file.sync_all()?;
    }
    Ok(())
}

fn pack_content_digest(
    manifest: &PhaseEnginePackManifest,
    manifest_sha256: &str,
) -> Result<String, PhaseEnginePackError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct DigestInput<'a> {
        domain: &'static str,
        manifest_sha256: &'a str,
        schema_version: &'a str,
        protocol_version: &'a str,
        pack_version: &'a str,
        engine: &'a PhaseEngineDescriptor,
        files: Vec<&'a PhaseEnginePackFile>,
    }
    let mut files = manifest.files.iter().collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let bytes = serde_json::to_vec(&DigestInput {
        domain: PHASE_ENGINE_PACK_SCHEMA_VERSION,
        manifest_sha256,
        schema_version: &manifest.schema_version,
        protocol_version: &manifest.protocol_version,
        pack_version: &manifest.pack_version,
        engine: &manifest.engine,
        files,
    })?;
    Ok(sha256_hex(&bytes))
}

fn validate_pointer(pointer: &ActivationPointer) -> Result<(), PhaseEnginePackError> {
    if pointer.schema_version != ACTIVATION_SCHEMA_VERSION {
        return Err(PhaseEnginePackError::Activation(format!(
            "activation schemaVersion must be {ACTIVATION_SCHEMA_VERSION}"
        )));
    }
    if pointer.protocol_version != PHASE_WORKER_PROTOCOL_VERSION {
        return Err(PhaseEnginePackError::Activation(format!(
            "activation protocolVersion must be {PHASE_WORKER_PROTOCOL_VERSION}"
        )));
    }
    validate_version("activation packVersion", &pointer.pack_version)?;
    validate_sha256("activation packContentSha256", &pointer.pack_content_sha256)?;
    validate_sha256("activation manifestSha256", &pointer.manifest_sha256)?;
    DateTime::parse_from_rfc3339(&pointer.activated_at).map_err(|_| {
        PhaseEnginePackError::Activation("activatedAt must be an RFC 3339 timestamp".into())
    })?;
    Ok(())
}

fn validate_relative_pack_path(value: &str) -> Result<(), PhaseEnginePackError> {
    if value.is_empty()
        || value.len() > MAXIMUM_PATH_BYTES
        || value.contains('\\')
        || value.starts_with('/')
        || value.ends_with('/')
        || value.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.ends_with('.')
                || part.ends_with(' ')
                || part.chars().any(char::is_control)
        })
    {
        return Err(PhaseEnginePackError::Invalid(format!(
            "pack path {value:?} must be a normalized bounded relative slash path"
        )));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(PhaseEnginePackError::Invalid(format!(
            "pack path {value:?} is not a safe relative path"
        )));
    }
    Ok(())
}

fn relative_path(value: &str) -> PathBuf {
    value.split('/').collect()
}

fn path_to_manifest_string(path: &Path) -> Result<String, PhaseEnginePackError> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(PhaseEnginePackError::Integrity(
                "discovered pack path is not normalized".into(),
            ));
        };
        let part = part.to_str().ok_or_else(|| {
            PhaseEnginePackError::Integrity("pack paths must be valid UTF-8".into())
        })?;
        parts.push(part);
    }
    Ok(parts.join("/"))
}

fn sha256_file(path: &Path, expected_bytes: u64) -> Result<String, PhaseEnginePackError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut read_total = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        read_total = read_total.checked_add(read as u64).ok_or_else(|| {
            PhaseEnginePackError::Integrity("file size overflow while hashing".into())
        })?;
        if read_total > expected_bytes || read_total > MAXIMUM_FILE_BYTES {
            return Err(PhaseEnginePackError::Integrity(format!(
                "{} grew beyond its declared bound while hashing",
                path.display()
            )));
        }
        hasher.update(&buffer[..read]);
    }
    if read_total != expected_bytes {
        return Err(PhaseEnginePackError::Integrity(format!(
            "{} changed size while hashing",
            path.display()
        )));
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn write_json_create_new<T: Serialize>(path: &Path, value: &T) -> Result<(), PhaseEnginePackError> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() as u64 > ACTIVATION_SIZE_LIMIT {
        return Err(PhaseEnginePackError::Activation(
            "activation pointer exceeds its size limit".into(),
        ));
    }
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn remove_internal_file_if_exists(root: &Path, path: &Path) -> Result<(), PhaseEnginePackError> {
    ensure_internal_path(root, path)?;
    if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(PhaseEnginePackError::Integrity(format!(
                "refusing to remove non-file internal path {}",
                path.display()
            )));
        }
        fs::remove_file(path)?;
    }
    Ok(())
}

fn remove_internal_directory(root: &Path, path: &Path) -> Result<(), PhaseEnginePackError> {
    ensure_internal_path(root, path)?;
    if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(PhaseEnginePackError::Integrity(format!(
                "refusing to remove non-directory internal path {}",
                path.display()
            )));
        }
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn ensure_internal_path(root: &Path, path: &Path) -> Result<(), PhaseEnginePackError> {
    if path == root || !path.starts_with(root) {
        return Err(PhaseEnginePackError::Integrity(format!(
            "internal path {} escapes the engine-pack store",
            path.display()
        )));
    }
    Ok(())
}

fn canonical_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn validate_bounded_text(
    parent: &str,
    field: &str,
    value: &str,
    maximum_bytes: usize,
) -> Result<(), PhaseEnginePackError> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > maximum_bytes
        || value.chars().any(char::is_control)
    {
        return Err(PhaseEnginePackError::Invalid(format!(
            "{parent}.{field} must be non-empty, trimmed, control-free text no longer than {maximum_bytes} bytes"
        )));
    }
    Ok(())
}

fn validate_https_source_url(field: &str, value: &str) -> Result<(), PhaseEnginePackError> {
    if value.len() > 2_048 {
        return Err(PhaseEnginePackError::Invalid(format!(
            "{field}.sourceUrl exceeds the URL length bound"
        )));
    }
    let parsed = Url::parse(value).map_err(|_| {
        PhaseEnginePackError::Invalid(format!("{field}.sourceUrl must be an absolute HTTPS URL"))
    })?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(PhaseEnginePackError::Invalid(format!(
            "{field}.sourceUrl must be credential-free HTTPS without query or fragment"
        )));
    }
    Ok(())
}

fn validate_version(field: &str, value: &str) -> Result<(), PhaseEnginePackError> {
    if value.is_empty()
        || value.len() > MAXIMUM_VERSION_BYTES
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+' | b'/')
        })
    {
        return Err(PhaseEnginePackError::Invalid(format!(
            "{field} must be a bounded ASCII version identifier"
        )));
    }
    Ok(())
}

fn validate_sha256(field: &str, value: &str) -> Result<(), PhaseEnginePackError> {
    if !is_lowercase_sha256(value) {
        return Err(PhaseEnginePackError::Invalid(format!(
            "{field} must contain exactly 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_lowercase_git_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
