use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::rules::{
    BUNDLED_POLICY_JSON, CommanderPolicyPackage, PolicyPackageError, bundled_policy,
};

const POLICY_PACKAGE_SIZE_LIMIT: u64 = 2 * 1024 * 1024;
const ACTIVATION_POINTER_SIZE_LIMIT: u64 = 16 * 1024;
const ACTIVATION_SCHEMA_VERSION: u16 = 1;
static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, thiserror::Error)]
pub enum PolicyStoreError {
    #[error("Policy package file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Package(#[from] PolicyPackageError),
    #[error("Policy activation metadata is invalid: {0}")]
    Activation(String),
    #[error("The selected policy package is invalid: {0}")]
    Selection(String),
    #[error("The selected package would downgrade the active policy: {0}")]
    Downgrade(String),
    #[error("The selected package conflicts with the active policy: {0}")]
    Conflict(String),
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PolicyPackageOrigin {
    Bundled,
    LocalImport,
    BundledFallback,
}

impl PolicyPackageOrigin {
    pub fn as_cache_value(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::LocalImport => "local-import",
            Self::BundledFallback => "bundled-fallback",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PolicyPackageProvenance {
    pub origin: PolicyPackageOrigin,
    pub snapshot_sha256: String,
    pub imported_at: Option<String>,
    pub authenticity_basis: String,
    pub warning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PolicyPackageSnapshot {
    pub package: CommanderPolicyPackage,
    pub provenance: PolicyPackageProvenance,
}

impl PolicyPackageSnapshot {
    pub fn status(&self) -> PolicyPackageStatus {
        PolicyPackageStatus {
            ready: true,
            origin: self.provenance.origin,
            schema_version: self.package.schema_version,
            package_version: self.package.package_version.clone(),
            effective_date: self.package.effective_date.clone(),
            verified_at: self.package.verified_at.clone(),
            policy_status: self.package.status.clone(),
            snapshot_sha256: self.provenance.snapshot_sha256.clone(),
            imported_at: self.provenance.imported_at.clone(),
            source_count: self.package.sources.len() as u32,
            bracket_note_count: self.package.bracket_policy.notes.len() as u32,
            authenticity_basis: self.provenance.authenticity_basis.clone(),
            message: self.provenance.warning.clone().unwrap_or_else(|| {
                match self.provenance.origin {
                    PolicyPackageOrigin::Bundled => {
                        "Using the Commander policy package bundled with this app build.".into()
                    }
                    PolicyPackageOrigin::LocalImport => {
                        "Using a structurally validated local policy package.".into()
                    }
                    PolicyPackageOrigin::BundledFallback => {
                        "Using the bundled Commander policy fallback.".into()
                    }
                }
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyPackageStatus {
    pub ready: bool,
    pub origin: PolicyPackageOrigin,
    pub schema_version: u16,
    pub package_version: String,
    pub effective_date: String,
    pub verified_at: String,
    pub policy_status: String,
    pub snapshot_sha256: String,
    pub imported_at: Option<String>,
    pub source_count: u32,
    pub bracket_note_count: u32,
    pub authenticity_basis: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyImportOutcome {
    pub activated: bool,
    pub status: PolicyPackageStatus,
    pub message: String,
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
pub struct PolicyStore {
    root: PathBuf,
}

impl PolicyStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, PolicyStoreError> {
        let store = Self { root: root.into() };
        fs::create_dir_all(store.packages_directory())?;
        store.recover_interrupted_activation()?;
        store.cleanup_staged_files();
        match store.load_runtime_package() {
            Ok(Some(snapshot)) => {
                store.prune_inactive_generations(&snapshot.provenance.snapshot_sha256)
            }
            Ok(None) => store.prune_inactive_generations(""),
            // Preserve the content-addressed file if the pointer is damaged;
            // analysis will fall back safely and a later import can repair it.
            Err(_) => {}
        }
        // Fail during app setup rather than at first analysis if a build ever
        // ships an invalid fallback.
        let _ = bundled_policy()?;
        Ok(store)
    }

    pub fn status(&self) -> Result<PolicyPackageStatus, PolicyStoreError> {
        Ok(self.load_active()?.status())
    }

    /// Loads one immutable policy package and its exact byte provenance.
    /// Callers that coordinate imports must retain their shared read lease for
    /// as long as this snapshot is used.
    pub fn load_active(&self) -> Result<PolicyPackageSnapshot, PolicyStoreError> {
        let bundled = bundled_policy_snapshot(PolicyPackageOrigin::Bundled, None)?;
        match self.load_runtime_package() {
            Ok(Some(snapshot)) => {
                if let Some(reason) = runtime_is_superseded_by_bundled(&snapshot, &bundled)? {
                    bundled_policy_snapshot(PolicyPackageOrigin::BundledFallback, Some(reason))
                } else {
                    Ok(snapshot)
                }
            }
            Ok(None) => Ok(bundled),
            Err(error) => bundled_policy_snapshot(
                PolicyPackageOrigin::BundledFallback,
                Some(format!(
                    "The locally imported policy could not be loaded ({error}); using the bundled fallback."
                )),
            ),
        }
    }

    /// Imports only an explicit local JSON file. No source URL in the package
    /// is fetched, and the selected path is not retained in app data.
    pub fn import_local_file(
        &self,
        selected_path: &Path,
    ) -> Result<PolicyImportOutcome, PolicyStoreError> {
        let bytes = read_selected_policy_file(selected_path)?;
        let candidate = parse_and_validate_package(&bytes)?;
        let candidate_sha256 = sha256_hex(&bytes);
        let current = self.load_active()?;

        if candidate_sha256 == current.provenance.snapshot_sha256 {
            return Ok(PolicyImportOutcome {
                activated: false,
                status: current.status(),
                message: "That exact policy snapshot is already active.".into(),
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

        let active = self
            .load_runtime_package()?
            .ok_or_else(|| PolicyStoreError::Activation("activation pointer disappeared".into()))?;
        if active.provenance.snapshot_sha256 != candidate_sha256 {
            return Err(PolicyStoreError::Activation(
                "the activated snapshot hash did not match the imported package".into(),
            ));
        }
        self.prune_inactive_generations(&candidate_sha256);
        Ok(PolicyImportOutcome {
            activated: true,
            status: active.status(),
            message: "The local Commander policy package was validated and activated.".into(),
        })
    }

    pub fn reset_to_bundled(&self) -> Result<PolicyImportOutcome, PolicyStoreError> {
        let current = self.load_active()?;
        if current.provenance.origin == PolicyPackageOrigin::Bundled {
            self.prune_inactive_generations("");
            return Ok(PolicyImportOutcome {
                activated: false,
                status: current.status(),
                message: "The Commander policy package bundled with this app is already active."
                    .into(),
            });
        }

        let bundled = bundled_policy_snapshot(PolicyPackageOrigin::Bundled, None)?;
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
            return Err(PolicyStoreError::Activation(
                "reset verification unexpectedly selected a runtime policy package".into(),
            ));
        }
        self.prune_inactive_generations("");
        let status = self.load_active()?.status();
        if status.origin != PolicyPackageOrigin::Bundled {
            return Err(PolicyStoreError::Activation(
                "reset verification did not report bundled policy provenance".into(),
            ));
        }
        Ok(PolicyImportOutcome {
            activated: true,
            status,
            message: "Reset Commander policy to the package bundled with this app build.".into(),
        })
    }

    fn load_runtime_package(&self) -> Result<Option<PolicyPackageSnapshot>, PolicyStoreError> {
        let pointer_path = self.active_pointer_path();
        if !pointer_path.exists() {
            return Ok(None);
        }
        let pointer = read_activation_pointer(&pointer_path)?;
        if pointer.selection == ActiveSelection::Bundled {
            return Ok(None);
        }
        let package_path = self.package_path(&pointer.snapshot_sha256)?;
        let bytes = read_bounded_file(&package_path, POLICY_PACKAGE_SIZE_LIMIT)?;
        let actual_sha256 = sha256_hex(&bytes);
        if actual_sha256 != pointer.snapshot_sha256 {
            return Err(PolicyStoreError::Activation(format!(
                "active package hash mismatch: expected {}, found {actual_sha256}",
                pointer.snapshot_sha256
            )));
        }
        let package = parse_and_validate_package(&bytes)?;
        if package.package_version != pointer.package_version
            || package.effective_date != pointer.effective_date
        {
            return Err(PolicyStoreError::Activation(
                "active package metadata does not match its activation pointer".into(),
            ));
        }
        Ok(Some(PolicyPackageSnapshot {
            package,
            provenance: PolicyPackageProvenance {
                origin: PolicyPackageOrigin::LocalImport,
                snapshot_sha256: actual_sha256,
                imported_at: Some(pointer.imported_at),
                authenticity_basis:
                    "User-selected local JSON; schema, content, and SHA-256 were verified, but no digital signature was available."
                        .into(),
                warning: None,
            },
        }))
    }

    fn install_generation(&self, sha256: &str, bytes: &[u8]) -> Result<(), PolicyStoreError> {
        let destination = self.package_path(sha256)?;
        if destination.exists() {
            let installed = read_bounded_file(&destination, POLICY_PACKAGE_SIZE_LIMIT)?;
            if sha256_hex(&installed) != sha256 {
                return Err(PolicyStoreError::Conflict(format!(
                    "stored generation {sha256} does not match its filename"
                )));
            }
            return Ok(());
        }

        let temporary = self.temporary_path("package");
        write_new_synced_file(&temporary, bytes)?;
        let readback = read_bounded_file(&temporary, POLICY_PACKAGE_SIZE_LIMIT)?;
        if sha256_hex(&readback) != sha256 {
            let _ = fs::remove_file(&temporary);
            return Err(PolicyStoreError::Activation(
                "staged policy failed its SHA-256 readback check".into(),
            ));
        }
        parse_and_validate_package(&readback)?;
        if let Err(error) = fs::rename(&temporary, &destination) {
            let _ = fs::remove_file(&temporary);
            if destination.exists() {
                let installed = read_bounded_file(&destination, POLICY_PACKAGE_SIZE_LIMIT)?;
                if sha256_hex(&installed) == sha256 {
                    return Ok(());
                }
            }
            return Err(error.into());
        }
        Ok(())
    }

    fn activate_pointer(&self, pointer: &ActivationPointer) -> Result<(), PolicyStoreError> {
        let active = self.active_pointer_path();
        let backup = self.backup_pointer_path();
        let next = self.temporary_path("activation");
        let encoded = serde_json::to_vec_pretty(pointer)
            .map_err(|error| PolicyStoreError::Activation(error.to_string()))?;
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
                return Err(PolicyStoreError::Activation(format!(
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
                Some("activated a different policy generation".into())
            }
            (ActiveSelection::Runtime, Ok(None)) => {
                Some("runtime activation selected the bundled policy".into())
            }
            (ActiveSelection::Bundled, Ok(Some(_))) => {
                Some("bundled reset retained a runtime policy generation".into())
            }
            (_, Err(error)) => Some(error.to_string()),
        };
        if let Some(error) = verification_error {
            if let Err(removal_error) = fs::remove_file(&active) {
                return Err(PolicyStoreError::Activation(format!(
                    "activation verification failed ({error}) and the failed pointer could not be removed ({removal_error}); the previous backup was retained for startup recovery"
                )));
            }
            if had_active {
                if let Err(rollback_error) = fs::rename(&backup, &active) {
                    return Err(PolicyStoreError::Activation(format!(
                        "activation verification failed ({error}) and the previous pointer could not be restored ({rollback_error}); the backup was retained for startup recovery"
                    )));
                }
                return Err(PolicyStoreError::Activation(format!(
                    "activation verification failed and the previous policy was restored: {error}"
                )));
            }
            return Err(PolicyStoreError::Activation(format!(
                "activation verification failed and the failed pointer was removed; the bundled fallback remains available: {error}"
            )));
        }
        if backup.exists() {
            // Activation is already committed and verified. A stale backup is
            // harmless and is cleaned on the next store initialization.
            let _ = fs::remove_file(backup);
        }
        Ok(())
    }

    fn recover_interrupted_activation(&self) -> Result<(), PolicyStoreError> {
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
                let package_path = self.package_path(&pointer.snapshot_sha256)?;
                let bytes = read_bounded_file(&package_path, POLICY_PACKAGE_SIZE_LIMIT)?;
                if sha256_hex(&bytes) != pointer.snapshot_sha256 {
                    return Err(PolicyStoreError::Activation(
                        "generation hash mismatch".into(),
                    ));
                }
                let package = parse_and_validate_package(&bytes)?;
                if package.package_version != pointer.package_version
                    || package.effective_date != pointer.effective_date
                {
                    return Err(PolicyStoreError::Activation(
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
            let is_generation = file_name.strip_suffix(".json").is_some_and(|stem| {
                stem.len() == 64 && stem.chars().all(|character| character.is_ascii_hexdigit())
            });
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

    fn package_path(&self, sha256: &str) -> Result<PathBuf, PolicyStoreError> {
        if sha256.len() != 64
            || !sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(PolicyStoreError::Activation(
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

fn runtime_is_superseded_by_bundled(
    runtime: &PolicyPackageSnapshot,
    bundled: &PolicyPackageSnapshot,
) -> Result<Option<String>, PolicyStoreError> {
    if runtime.package.package_version == bundled.package.package_version {
        return Ok(
            (runtime.provenance.snapshot_sha256 != bundled.provenance.snapshot_sha256).then(|| {
                format!(
                    "The locally imported policy uses the bundled version {} with conflicting bytes; using the bundled fallback.",
                    bundled.package.package_version
                )
            }),
        );
    }

    let runtime_date = policy_date(&runtime.package.effective_date)?;
    let bundled_date = policy_date(&bundled.package.effective_date)?;
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
            "The locally imported policy {} (effective {}) does not supersede bundled {} (effective {}); using the bundled fallback.",
            runtime.package.package_version,
            runtime.package.effective_date,
            bundled.package.package_version,
            bundled.package.effective_date
        )
    }))
}

fn read_selected_policy_file(path: &Path) -> Result<Vec<u8>, PolicyStoreError> {
    if !path.is_absolute() {
        return Err(PolicyStoreError::Selection(
            "choose an absolute local JSON file path".into(),
        ));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("json"))
    {
        return Err(PolicyStoreError::Selection(
            "policy packages must use the .json extension".into(),
        ));
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(PolicyStoreError::Selection(
            "the selected path must be a regular local file, not a directory or symbolic link"
                .into(),
        ));
    }
    if metadata.len() == 0 || metadata.len() > POLICY_PACKAGE_SIZE_LIMIT {
        return Err(PolicyStoreError::Selection(format!(
            "policy packages must contain 1 to {} bytes",
            POLICY_PACKAGE_SIZE_LIMIT
        )));
    }
    let canonical = fs::canonicalize(path)?;
    if !is_local_disk_path(&canonical) {
        return Err(PolicyStoreError::Selection(
            "policy packages must be selected from a local disk".into(),
        ));
    }
    read_bounded_file(&canonical, POLICY_PACKAGE_SIZE_LIMIT)
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

fn read_activation_pointer(path: &Path) -> Result<ActivationPointer, PolicyStoreError> {
    let bytes = read_bounded_file(path, ACTIVATION_POINTER_SIZE_LIMIT)?;
    let pointer: ActivationPointer = serde_json::from_slice(&bytes)
        .map_err(|error| PolicyStoreError::Activation(error.to_string()))?;
    if pointer.schema_version != ACTIVATION_SCHEMA_VERSION || pointer.imported_at.trim().is_empty()
    {
        return Err(PolicyStoreError::Activation(
            "unsupported pointer schema or missing import timestamp".into(),
        ));
    }
    // Validates both shape and path-traversal safety before the value is used.
    if pointer.snapshot_sha256.len() != 64
        || !pointer
            .snapshot_sha256
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(PolicyStoreError::Activation(
            "pointer contains an invalid SHA-256".into(),
        ));
    }
    Ok(pointer)
}

fn read_bounded_file(path: &Path, limit: u64) -> Result<Vec<u8>, PolicyStoreError> {
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    if length == 0 || length > limit {
        return Err(PolicyStoreError::Selection(format!(
            "{} must contain 1 to {limit} bytes",
            path.display()
        )));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(PolicyStoreError::Selection(format!(
            "{} exceeds the {limit}-byte limit",
            path.display()
        )));
    }
    Ok(bytes)
}

fn write_new_synced_file(path: &Path, bytes: &[u8]) -> Result<(), PolicyStoreError> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

fn parse_and_validate_package(bytes: &[u8]) -> Result<CommanderPolicyPackage, PolicyStoreError> {
    let package = serde_json::from_slice::<CommanderPolicyPackage>(bytes)
        .map_err(PolicyPackageError::from)?;
    package.validate()?;
    Ok(package)
}

pub(crate) fn bundled_policy_snapshot(
    origin: PolicyPackageOrigin,
    warning: Option<String>,
) -> Result<PolicyPackageSnapshot, PolicyStoreError> {
    Ok(PolicyPackageSnapshot {
        package: bundled_policy()?,
        provenance: PolicyPackageProvenance {
            origin,
            snapshot_sha256: sha256_hex(BUNDLED_POLICY_JSON.as_bytes()),
            imported_at: None,
            authenticity_basis:
                "Bundled with the app build; the app does not independently verify a digital signature."
                    .into(),
            warning,
        },
    })
}

fn validate_forward_activation(
    current: &PolicyPackageSnapshot,
    candidate: &CommanderPolicyPackage,
    candidate_sha256: &str,
) -> Result<(), PolicyStoreError> {
    if candidate.package_version == current.package.package_version {
        return Err(PolicyStoreError::Conflict(format!(
            "version {} is already active with SHA-256 {}; the conflicting candidate has SHA-256 {candidate_sha256}",
            candidate.package_version, current.provenance.snapshot_sha256
        )));
    }
    let current_date = policy_date(&current.package.effective_date)?;
    let candidate_date = policy_date(&candidate.effective_date)?;
    if candidate_date < current_date {
        return Err(PolicyStoreError::Downgrade(format!(
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
            return Err(PolicyStoreError::Downgrade(format!(
                "packages with the same effective date must increase a numeric -r revision (active {}, candidate {})",
                current.package.package_version, candidate.package_version
            )));
        }
    }
    Ok(())
}

fn policy_date(value: &str) -> Result<NaiveDate, PolicyStoreError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| PolicyStoreError::Activation("invalid policy effective date".into()))
}

fn revision_number(version: &str) -> Option<u64> {
    version.rsplit_once("-r")?.1.parse().ok()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
