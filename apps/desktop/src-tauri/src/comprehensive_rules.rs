//! Explicit, local-first ingestion of Magic's official Comprehensive Rules.
//!
//! The downloaded document is reference data. It cannot inject executable
//! behavior. `rules_capabilities` is the separate, code-reviewed bridge that
//! may turn a verified rule heading into strategic (report-only) metadata.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use regex::Regex;
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const COMPREHENSIVE_RULES_SCHEMA_VERSION: &str = "1";
pub(crate) const COMPREHENSIVE_RULES_PARSER_VERSION: &str = "comprehensive-rules-parser-1";
pub(crate) const COMPREHENSIVE_RULES_SOURCE_PAGE: &str = "https://magic.wizards.com/en/rules";

const MAX_LANDING_BYTES: usize = 2 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_SNAPSHOT_BYTES: u64 = 32 * 1024 * 1024;
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const MIN_RULE_COUNT: usize = 2_000;
const MIN_SECTION_COUNT: usize = 100;
const MIN_COMMANDER_RULE_COUNT: usize = 20;

pub(crate) type ComprehensiveRulesUpdateReporter =
    Arc<dyn Fn(ComprehensiveRulesUpdateProgress) + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ComprehensiveRulesError {
    #[error("Comprehensive Rules network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Comprehensive Rules file error: {0}")]
    Io(#[from] io::Error),
    #[error("Comprehensive Rules serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ComprehensiveRule {
    pub rule_id: String,
    pub heading: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ComprehensiveRulesSnapshot {
    pub schema_version: String,
    pub parser_version: String,
    pub effective_date: String,
    pub installed_at: String,
    pub source_page_url: String,
    pub document_url: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub snapshot_sha256: String,
    pub document_bytes: u64,
    pub section_count: u64,
    pub example_count: u64,
    pub glossary_count: u64,
    pub commander_rule_count: u64,
    pub rules: Vec<ComprehensiveRule>,
    /// Stored only in the user's local application-data directory. The
    /// official rules document is intentionally not bundled with releases.
    pub source_text: String,
}

impl ComprehensiveRulesSnapshot {
    pub(crate) fn has_rule_heading(&self, rule_id: &str, heading: &str) -> bool {
        self.rules.iter().any(|rule| {
            rule.rule_id == rule_id
                && rule
                    .heading
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(heading))
        })
    }

    #[allow(dead_code)]
    pub(crate) fn rule_heading(&self, rule_id: &str) -> Option<&str> {
        self.rules
            .iter()
            .find(|rule| rule.rule_id == rule_id)
            .and_then(|rule| rule.heading.as_deref())
    }

    pub(crate) fn keyword_rule(&self, heading: &str) -> Option<(&str, &str)> {
        self.rules.iter().find_map(|rule| {
            let is_keyword_rule =
                rule.rule_id.starts_with("701.") || rule.rule_id.starts_with("702.");
            let rule_heading = rule.heading.as_deref()?;
            (is_keyword_rule && rule_heading.eq_ignore_ascii_case(heading))
                .then_some((rule.rule_id.as_str(), rule_heading))
        })
    }

    pub(crate) fn rule_count(&self) -> u64 {
        self.rules
            .iter()
            .filter(|rule| rule.rule_id.contains('.'))
            .count() as u64
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RulesCompatibility {
    Compatible,
    Changed,
    ReferenceOnly,
    NotInstalled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ComprehensiveRulesStatus {
    pub ready: bool,
    pub schema_version: String,
    pub parser_version: String,
    pub effective_date: Option<String>,
    pub installed_at: Option<String>,
    pub source_page_url: String,
    pub document_url: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub snapshot_sha256: Option<String>,
    pub document_bytes: Option<u64>,
    pub rule_count: u64,
    pub section_count: u64,
    pub example_count: u64,
    pub glossary_count: u64,
    pub commander_rule_count: u64,
    pub compatibility: RulesCompatibility,
    pub changed_capability_rule_ids: Vec<String>,
    pub authenticity_basis: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ComprehensiveRulesUpdateCheck {
    pub update_available: bool,
    pub installed_version: Option<String>,
    pub available_version: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ComprehensiveRulesUpdateProgress {
    pub phase: String,
    pub completed_units: u64,
    pub total_units: Option<u64>,
    pub progress: f32,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub(crate) enum ComprehensiveRulesUpdateOutcome {
    Installed { status: ComprehensiveRulesStatus },
    NotModified { status: ComprehensiveRulesStatus },
}

#[derive(Debug, Clone)]
pub(crate) struct ComprehensiveRulesStore {
    root: PathBuf,
}

impl ComprehensiveRulesStore {
    pub(crate) fn new(root: impl Into<PathBuf>) -> Result<Self, ComprehensiveRulesError> {
        let store = Self { root: root.into() };
        fs::create_dir_all(&store.root)?;
        let next = store.next_path();
        if next.exists() {
            let _ = fs::remove_file(next);
        }
        store.recover_previous_if_needed()?;
        Ok(store)
    }

    pub(crate) fn status(&self) -> Result<ComprehensiveRulesStatus, ComprehensiveRulesError> {
        match self.load_active()? {
            Some(snapshot) => {
                let mut status = status_from_snapshot(&snapshot);
                if self.corrupt_path().exists() {
                    status.message = format!(
                        "A damaged newer rules snapshot was quarantined and the valid {} rules were restored. Check for updates when convenient.",
                        snapshot.effective_date
                    );
                }
                Ok(status)
            }
            None => {
                let mut status = empty_status();
                if self.corrupt_path().exists() {
                    status.message =
                        "A damaged local rules snapshot was quarantined. Install the official Comprehensive Rules again to restore rules-backed annotations."
                            .into();
                }
                Ok(status)
            }
        }
    }

    pub(crate) fn load_active(
        &self,
    ) -> Result<Option<ComprehensiveRulesSnapshot>, ComprehensiveRulesError> {
        let path = self.live_path();
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(load_snapshot_file(&path)?))
    }

    /// Discovers the official document and performs a conditional request,
    /// but does not read, parse, write, or activate a changed rules body.
    pub(crate) async fn check_for_update(
        &self,
    ) -> Result<ComprehensiveRulesUpdateCheck, ComprehensiveRulesError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(UPDATE_CHECK_TIMEOUT)
            .user_agent("CommanderDeckAnalyzer/0.3 ComprehensiveRulesUpdater")
            .build()?;
        let landing_url = Url::parse(COMPREHENSIVE_RULES_SOURCE_PAGE)
            .map_err(|error| ComprehensiveRulesError::Invalid(error.to_string()))?;
        let landing_response = client.get(landing_url.clone()).send().await?;
        require_success_without_redirect(&landing_response, "official rules page")?;
        let landing_bytes =
            read_bounded_response(landing_response, MAX_LANDING_BYTES, "official rules page")
                .await?;
        let landing = String::from_utf8(landing_bytes).map_err(|_| {
            ComprehensiveRulesError::Invalid("The official rules page was not UTF-8.".into())
        })?;
        let document_url = discover_document_url(&landing_url, &landing)?;
        validate_document_url(&document_url)?;

        let active = self.load_active()?;
        let mut request = client.get(document_url.clone());
        if let Some(current) = active
            .as_ref()
            .filter(|snapshot| snapshot.document_url == document_url.as_str())
        {
            if let Some(etag) = current.etag.as_deref() {
                request = request.header(IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = current.last_modified.as_deref() {
                request = request.header(IF_MODIFIED_SINCE, last_modified);
            }
        }
        let response = request.send().await?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(comprehensive_rules_update_check_result(
                active.as_ref(),
                true,
                document_version(&document_url),
            ));
        }
        require_success_without_redirect(&response, "Comprehensive Rules TXT")?;
        let response_etag = bounded_header(response.headers().get(ETAG));
        let response_last_modified = bounded_header(response.headers().get(LAST_MODIFIED));
        let validators_match = comprehensive_rules_response_validators_match(
            active.as_ref(),
            &document_url,
            response_etag.as_deref(),
            response_last_modified.as_deref(),
        );
        let available_version = document_version(&document_url)
            .or(response_etag)
            .or(response_last_modified)
            .or_else(|| Some(document_url.to_string()));
        // The changed document body is intentionally not consumed by a check.
        drop(response);
        Ok(comprehensive_rules_update_check_result(
            active.as_ref(),
            validators_match,
            available_version,
        ))
    }

    pub(crate) async fn update_from_network(
        &self,
        reporter: Option<ComprehensiveRulesUpdateReporter>,
    ) -> Result<ComprehensiveRulesUpdateOutcome, ComprehensiveRulesError> {
        emit(
            &reporter,
            "discover",
            0,
            None,
            0.02,
            "Discovering the current official Wizards TXT document",
        );
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .user_agent("CommanderDeckAnalyzer/0.3 ComprehensiveRulesUpdater")
            .build()?;
        let landing_url = Url::parse(COMPREHENSIVE_RULES_SOURCE_PAGE)
            .map_err(|error| ComprehensiveRulesError::Invalid(error.to_string()))?;
        let landing_response = client.get(landing_url.clone()).send().await?;
        require_success_without_redirect(&landing_response, "official rules page")?;
        let landing_bytes =
            read_bounded_response(landing_response, MAX_LANDING_BYTES, "official rules page")
                .await?;
        let landing = String::from_utf8(landing_bytes).map_err(|_| {
            ComprehensiveRulesError::Invalid("The official rules page was not UTF-8.".into())
        })?;
        let document_url = discover_document_url(&landing_url, &landing)?;
        validate_document_url(&document_url)?;

        emit(
            &reporter,
            "download",
            0,
            None,
            0.10,
            "Downloading the official Comprehensive Rules TXT",
        );
        let active = self.load_active()?;
        let mut request = client.get(document_url.clone());
        if let Some(current) = active
            .as_ref()
            .filter(|snapshot| snapshot.document_url == document_url.as_str())
        {
            if let Some(etag) = current.etag.as_deref() {
                request = request.header(IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = current.last_modified.as_deref() {
                request = request.header(IF_MODIFIED_SINCE, last_modified);
            }
        }
        let response = request.send().await?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(ComprehensiveRulesUpdateOutcome::NotModified {
                status: self.status()?,
            });
        }
        require_success_without_redirect(&response, "Comprehensive Rules TXT")?;
        let etag = bounded_header(response.headers().get(ETAG));
        let last_modified = bounded_header(response.headers().get(LAST_MODIFIED));
        let bytes =
            read_bounded_response(response, MAX_DOCUMENT_BYTES, "Comprehensive Rules document")
                .await?;
        let sha = format!("{:x}", Sha256::digest(&bytes));
        if active
            .as_ref()
            .is_some_and(|snapshot| snapshot.snapshot_sha256 == sha)
        {
            return Ok(ComprehensiveRulesUpdateOutcome::NotModified {
                status: self.status()?,
            });
        }

        emit(
            &reporter,
            "index",
            bytes.len() as u64,
            Some(bytes.len() as u64),
            0.62,
            "Parsing numbered rules, keyword headings, glossary, and Commander section",
        );
        let text = decode_rules_text(&bytes)?;
        let mut snapshot = parse_rules_document(&text, document_url.as_str())?;
        ensure_not_future_effective(&snapshot.effective_date, Utc::now().date_naive())?;
        snapshot.etag = etag;
        snapshot.last_modified = last_modified;
        snapshot.snapshot_sha256 = sha;
        snapshot.document_bytes = bytes.len() as u64;
        snapshot.installed_at = Utc::now().to_rfc3339();
        if let Some(current) = active.as_ref()
            && snapshot.effective_date < current.effective_date
        {
            return Err(ComprehensiveRulesError::Invalid(format!(
                "The discovered rules are effective {}, older than the installed {}. The installed snapshot was left unchanged.",
                snapshot.effective_date, current.effective_date
            )));
        }
        validate_snapshot(&snapshot)?;

        emit(
            &reporter,
            "activate",
            snapshot.rule_count(),
            Some(snapshot.rule_count()),
            0.92,
            "Validating and atomically activating the new rules index",
        );
        self.activate(&snapshot)?;
        emit(
            &reporter,
            "complete",
            snapshot.rule_count(),
            Some(snapshot.rule_count()),
            1.0,
            "The official Comprehensive Rules are ready for local analysis",
        );
        Ok(ComprehensiveRulesUpdateOutcome::Installed {
            status: self.status()?,
        })
    }

    fn activate(
        &self,
        snapshot: &ComprehensiveRulesSnapshot,
    ) -> Result<(), ComprehensiveRulesError> {
        let next = self.next_path();
        let live = self.live_path();
        let previous = self.previous_path();
        if next.exists() {
            fs::remove_file(&next)?;
        }
        let encoded = serde_json::to_vec(snapshot)?;
        let mut staged_file = fs::File::create(&next)?;
        staged_file.write_all(&encoded)?;
        staged_file.sync_all()?;
        drop(staged_file);
        let staged: ComprehensiveRulesSnapshot = serde_json::from_slice(&fs::read(&next)?)?;
        validate_snapshot(&staged)?;
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

    fn recover_previous_if_needed(&self) -> Result<(), ComprehensiveRulesError> {
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
        self.root.join("rules.json")
    }

    fn next_path(&self) -> PathBuf {
        self.root.join("rules.next.json")
    }

    fn previous_path(&self) -> PathBuf {
        self.root.join("rules.previous.json")
    }

    fn corrupt_path(&self) -> PathBuf {
        self.root.join("rules.corrupt.json")
    }
}

fn load_snapshot_file(
    path: &std::path::Path,
) -> Result<ComprehensiveRulesSnapshot, ComprehensiveRulesError> {
    if fs::metadata(path)?.len() > MAX_SNAPSHOT_BYTES {
        return Err(ComprehensiveRulesError::Invalid(
            "The local Comprehensive Rules snapshot exceeded the 32 MiB safety limit.".into(),
        ));
    }
    let bytes = fs::read(path)?;
    let snapshot: ComprehensiveRulesSnapshot = serde_json::from_slice(&bytes)?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

async fn read_bounded_response(
    mut response: reqwest::Response,
    maximum_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, ComprehensiveRulesError> {
    if response
        .content_length()
        .is_some_and(|size| size > maximum_bytes as u64)
    {
        return Err(ComprehensiveRulesError::Invalid(format!(
            "The {label} exceeded the {} MiB safety limit.",
            maximum_bytes / (1024 * 1024)
        )));
    }
    let capacity = response
        .content_length()
        .and_then(|size| usize::try_from(size).ok())
        .unwrap_or(0)
        .min(maximum_bytes);
    let mut body = Vec::with_capacity(capacity);
    while let Some(chunk) = response.chunk().await? {
        append_bounded(&mut body, &chunk, maximum_bytes, label)?;
    }
    Ok(body)
}

fn append_bounded(
    destination: &mut Vec<u8>,
    chunk: &[u8],
    maximum_bytes: usize,
    label: &str,
) -> Result<(), ComprehensiveRulesError> {
    if destination.len().saturating_add(chunk.len()) > maximum_bytes {
        return Err(ComprehensiveRulesError::Invalid(format!(
            "The {label} exceeded the {} MiB safety limit.",
            maximum_bytes / (1024 * 1024)
        )));
    }
    destination.extend_from_slice(chunk);
    Ok(())
}

fn parse_rules_document(
    source_text: &str,
    document_url: &str,
) -> Result<ComprehensiveRulesSnapshot, ComprehensiveRulesError> {
    let effective_date = parse_effective_date(source_text)?;
    validate_url_date(document_url, &effective_date)?;
    let normalized = source_text.replace("\r\n", "\n").replace('\r', "\n");
    let game_starts = normalized
        .match_indices("\n1. Game Concepts\n")
        .map(|(index, _)| index + 1)
        .collect::<Vec<_>>();
    let game_start = game_starts.get(1).copied().ok_or_else(|| {
        ComprehensiveRulesError::Invalid(
            "The document did not contain the expected rules body after its contents.".into(),
        )
    })?;
    let glossary_start = normalized[game_start..]
        .find("\nGlossary\n")
        .map(|index| game_start + index + 1)
        .ok_or_else(|| ComprehensiveRulesError::Invalid("Glossary marker missing.".into()))?;
    let credits_start = normalized[glossary_start..]
        .find("\nCredits\n")
        .map(|index| glossary_start + index + 1)
        .ok_or_else(|| ComprehensiveRulesError::Invalid("Credits marker missing.".into()))?;
    let body = &normalized[game_start..glossary_start];
    let glossary = &normalized[glossary_start + "Glossary\n".len()..credits_start];

    let section_re = Regex::new(r"^(\d{3})\. (.+)$").expect("valid section regex");
    let rule_re = Regex::new(r"^(\d{3}\.\d+[a-z]?)\.? (.+)$").expect("valid numbered rule regex");
    let mut rules = Vec::<ComprehensiveRule>::new();
    let mut example_count = 0u64;
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with("Example:") {
            example_count += 1;
            if let Some(rule) = rules.last_mut() {
                rule.body.push('\n');
                rule.body.push_str(line);
            }
            continue;
        }
        if let Some(captures) = section_re.captures(line) {
            rules.push(ComprehensiveRule {
                rule_id: captures[1].to_string(),
                heading: Some(captures[2].trim().to_string()),
                body: captures[2].trim().to_string(),
            });
            continue;
        }
        if let Some(captures) = rule_re.captures(line) {
            let rule_id = captures[1].to_string();
            let text = captures[2].trim().to_string();
            let is_keyword_heading = (rule_id.starts_with("701.") || rule_id.starts_with("702."))
                && !rule_id
                    .chars()
                    .last()
                    .is_some_and(|character| character.is_ascii_alphabetic())
                && !text.ends_with('.')
                && text.split_whitespace().count() <= 8;
            rules.push(ComprehensiveRule {
                rule_id,
                heading: is_keyword_heading.then_some(text.clone()),
                body: text,
            });
        } else if let Some(rule) = rules.last_mut() {
            rule.body.push(' ');
            rule.body.push_str(line);
        }
    }
    let section_count = rules
        .iter()
        .filter(|rule| !rule.rule_id.contains('.'))
        .count() as u64;
    let commander_rule_count = rules
        .iter()
        .filter(|rule| rule.rule_id == "903" || rule.rule_id.starts_with("903."))
        .count() as u64;
    let glossary_count = glossary
        .split("\n\n")
        .filter(|block| block.lines().filter(|line| !line.trim().is_empty()).count() >= 2)
        .count() as u64;

    let snapshot = ComprehensiveRulesSnapshot {
        schema_version: COMPREHENSIVE_RULES_SCHEMA_VERSION.into(),
        parser_version: COMPREHENSIVE_RULES_PARSER_VERSION.into(),
        effective_date,
        installed_at: String::new(),
        source_page_url: COMPREHENSIVE_RULES_SOURCE_PAGE.into(),
        document_url: document_url.into(),
        etag: None,
        last_modified: None,
        snapshot_sha256: String::new(),
        document_bytes: 0,
        section_count,
        example_count,
        glossary_count,
        commander_rule_count,
        rules,
        source_text: source_text.into(),
    };
    validate_rule_structure(&snapshot)?;
    Ok(snapshot)
}

fn validate_snapshot(snapshot: &ComprehensiveRulesSnapshot) -> Result<(), ComprehensiveRulesError> {
    if snapshot.schema_version != COMPREHENSIVE_RULES_SCHEMA_VERSION
        || snapshot.parser_version != COMPREHENSIVE_RULES_PARSER_VERSION
    {
        return Err(ComprehensiveRulesError::Invalid(
            "The local Comprehensive Rules index uses an unsupported schema or parser.".into(),
        ));
    }
    if snapshot.source_page_url != COMPREHENSIVE_RULES_SOURCE_PAGE {
        return Err(ComprehensiveRulesError::Invalid(
            "The local Comprehensive Rules snapshot did not name the exact official source page."
                .into(),
        ));
    }
    chrono::DateTime::parse_from_rfc3339(&snapshot.installed_at).map_err(|_| {
        ComprehensiveRulesError::Invalid(
            "The local Comprehensive Rules installation timestamp was invalid.".into(),
        )
    })?;
    for header in [snapshot.etag.as_deref(), snapshot.last_modified.as_deref()]
        .into_iter()
        .flatten()
    {
        if header.len() > 512 || header.chars().any(char::is_control) {
            return Err(ComprehensiveRulesError::Invalid(
                "The local Comprehensive Rules snapshot contained an invalid HTTP validator."
                    .into(),
            ));
        }
    }
    validate_document_url(
        &Url::parse(&snapshot.document_url)
            .map_err(|error| ComprehensiveRulesError::Invalid(error.to_string()))?,
    )?;
    validate_url_date(&snapshot.document_url, &snapshot.effective_date)?;
    validate_document_provenance(snapshot)?;
    validate_rule_structure(snapshot)?;

    let reparsed = parse_rules_document(&snapshot.source_text, &snapshot.document_url)?;
    if reparsed.effective_date != snapshot.effective_date
        || reparsed.section_count != snapshot.section_count
        || reparsed.example_count != snapshot.example_count
        || reparsed.glossary_count != snapshot.glossary_count
        || reparsed.commander_rule_count != snapshot.commander_rule_count
        || reparsed.rules != snapshot.rules
    {
        return Err(ComprehensiveRulesError::Invalid(
            "The local Comprehensive Rules index did not match its stored official source text."
                .into(),
        ));
    }
    Ok(())
}

fn validate_document_provenance(
    snapshot: &ComprehensiveRulesSnapshot,
) -> Result<(), ComprehensiveRulesError> {
    if snapshot.source_text.len() > MAX_DOCUMENT_BYTES
        || snapshot.snapshot_sha256.len() != 64
        || !snapshot
            .snapshot_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ComprehensiveRulesError::Invalid(
            "The local Comprehensive Rules document provenance was invalid.".into(),
        ));
    }
    let source_bytes = snapshot.source_text.as_bytes();
    let source_length = source_bytes.len() as u64;
    let plain_hash = format!("{:x}", Sha256::digest(source_bytes));
    let plain_matches = snapshot.document_bytes == source_length
        && snapshot.snapshot_sha256.eq_ignore_ascii_case(&plain_hash);
    let bom_matches = if snapshot.document_bytes == source_length.saturating_add(3) {
        let mut hasher = Sha256::new();
        hasher.update([0xEF, 0xBB, 0xBF]);
        hasher.update(source_bytes);
        snapshot
            .snapshot_sha256
            .eq_ignore_ascii_case(&format!("{:x}", hasher.finalize()))
    } else {
        false
    };
    if !plain_matches && !bom_matches {
        return Err(ComprehensiveRulesError::Invalid(
            "The local Comprehensive Rules source text did not match its recorded byte length and SHA-256."
                .into(),
        ));
    }
    Ok(())
}

fn validate_rule_structure(
    snapshot: &ComprehensiveRulesSnapshot,
) -> Result<(), ComprehensiveRulesError> {
    let rule_count = snapshot.rule_count() as usize;
    if rule_count < MIN_RULE_COUNT
        || snapshot.section_count < MIN_SECTION_COUNT as u64
        || snapshot.commander_rule_count < MIN_COMMANDER_RULE_COUNT as u64
    {
        return Err(ComprehensiveRulesError::Invalid(format!(
            "The rules document was incomplete ({} numbered rules, {} sections, {} Commander rules).",
            rule_count, snapshot.section_count, snapshot.commander_rule_count
        )));
    }
    for required in [
        "101.1", "103.5", "106.1", "117.1", "118.1", "400.1", "601.1", "602.1", "603.1", "608.1",
        "613.1", "614.1", "704.5", "903.1", "903.3", "903.4", "903.5a", "903.5b", "903.5c",
        "903.6", "903.8", "903.9a", "903.9b", "903.10a", "903.11",
    ] {
        if !snapshot.rules.iter().any(|rule| rule.rule_id == required) {
            return Err(ComprehensiveRulesError::Invalid(format!(
                "Required Comprehensive Rule {required} was missing."
            )));
        }
    }
    Ok(())
}

fn parse_effective_date(source: &str) -> Result<String, ComprehensiveRulesError> {
    let regex =
        Regex::new(r"(?i)These rules are effective as of ([A-Z][a-z]+ [0-9]{1,2}, [0-9]{4})\.")
            .expect("valid effective-date regex");
    let value = regex
        .captures(source)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str())
        .ok_or_else(|| {
            ComprehensiveRulesError::Invalid(
                "The official effective-date declaration was missing.".into(),
            )
        })?;
    let parsed = NaiveDate::parse_from_str(value, "%B %d, %Y").map_err(|_| {
        ComprehensiveRulesError::Invalid("The rules effective date was invalid.".into())
    })?;
    Ok(parsed.format("%Y-%m-%d").to_string())
}

fn ensure_not_future_effective(
    effective_date: &str,
    today_utc: NaiveDate,
) -> Result<(), ComprehensiveRulesError> {
    let effective = NaiveDate::parse_from_str(effective_date, "%Y-%m-%d").map_err(|_| {
        ComprehensiveRulesError::Invalid("The rules effective date was invalid.".into())
    })?;
    if effective > today_utc {
        return Err(ComprehensiveRulesError::Invalid(format!(
            "The discovered Comprehensive Rules are not effective until {effective_date}; the installed rules were left unchanged."
        )));
    }
    Ok(())
}

fn discover_document_url(landing_url: &Url, html: &str) -> Result<Url, ComprehensiveRulesError> {
    let regex =
        Regex::new(r#"(?i)href\s*=\s*["']([^"']*MagicCompRules(?:%20|\s)*20[0-9]{6}\.txt)["']"#)
            .expect("valid rules-link regex");
    let mut candidates = regex
        .captures_iter(html)
        .filter_map(|captures| captures.get(1))
        .filter_map(|capture| landing_url.join(capture.as_str()).ok())
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    candidates.dedup_by(|left, right| left.as_str() == right.as_str());
    if candidates.len() != 1 {
        return Err(ComprehensiveRulesError::Invalid(format!(
            "Expected one official Comprehensive Rules TXT link, found {}.",
            candidates.len()
        )));
    }
    Ok(candidates.remove(0))
}

fn validate_document_url(url: &Url) -> Result<(), ComprehensiveRulesError> {
    let path_re =
        Regex::new(r"^/(20[0-9]{2})/downloads/MagicCompRules(?:%20| )?(20[0-9]{6})\.txt$")
            .expect("valid document path regex");
    let captures = path_re.captures(url.path()).ok_or_else(|| {
        ComprehensiveRulesError::Invalid(
            "The Comprehensive Rules TXT URL did not match the exact Wizards path allowlist."
                .into(),
        )
    })?;
    let valid = url.scheme() == "https"
        && url.host_str() == Some("media.wizards.com")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && captures[1] == captures[2][..4];
    if !valid {
        return Err(ComprehensiveRulesError::Invalid(
            "The Comprehensive Rules TXT URL did not pass the exact HTTPS allowlist.".into(),
        ));
    }
    Ok(())
}

fn document_version(url: &Url) -> Option<String> {
    let stem = url.path().strip_suffix(".txt")?;
    let digits = stem.get(stem.len().checked_sub(8)?..)?;
    NaiveDate::parse_from_str(digits, "%Y%m%d")
        .ok()
        .map(|date| date.format("%Y-%m-%d").to_string())
}

fn comprehensive_rules_update_check_result(
    active: Option<&ComprehensiveRulesSnapshot>,
    not_modified: bool,
    available_version: Option<String>,
) -> ComprehensiveRulesUpdateCheck {
    let installed_version = active.map(|snapshot| snapshot.effective_date.clone());
    let update_available = active.is_none() || !not_modified;
    ComprehensiveRulesUpdateCheck {
        update_available,
        installed_version: installed_version.clone(),
        available_version: available_version.or_else(|| installed_version.clone()),
        detail: if active.is_none() {
            "The official Comprehensive Rules are not installed.".into()
        } else if not_modified {
            "The installed Comprehensive Rules match the current official document.".into()
        } else {
            "Wizards reports a possibly changed Comprehensive Rules document. It will be downloaded and validated only after confirmation."
                .into()
        },
    }
}

fn comprehensive_rules_response_validators_match(
    active: Option<&ComprehensiveRulesSnapshot>,
    document_url: &Url,
    response_etag: Option<&str>,
    response_last_modified: Option<&str>,
) -> bool {
    let Some(current) = active.filter(|snapshot| snapshot.document_url == document_url.as_str())
    else {
        return false;
    };
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

fn validate_url_date(
    document_url: &str,
    effective_date: &str,
) -> Result<(), ComprehensiveRulesError> {
    let url = Url::parse(document_url)
        .map_err(|error| ComprehensiveRulesError::Invalid(error.to_string()))?;
    validate_document_url(&url)?;
    let digits = effective_date.replace('-', "");
    if !url.path().contains(&digits) {
        return Err(ComprehensiveRulesError::Invalid(format!(
            "The URL date did not match the declared effective date {effective_date}."
        )));
    }
    Ok(())
}

fn decode_rules_text(bytes: &[u8]) -> Result<String, ComprehensiveRulesError> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = String::from_utf8(bytes.to_vec()).map_err(|_| {
        ComprehensiveRulesError::Invalid("The Comprehensive Rules TXT was not UTF-8.".into())
    })?;
    if text.contains('\0') {
        return Err(ComprehensiveRulesError::Invalid(
            "The Comprehensive Rules TXT contained NUL bytes.".into(),
        ));
    }
    Ok(text)
}

fn require_success_without_redirect(
    response: &reqwest::Response,
    label: &str,
) -> Result<(), ComprehensiveRulesError> {
    if response.status().is_redirection() {
        return Err(ComprehensiveRulesError::Invalid(format!(
            "The {label} attempted an unexpected redirect."
        )));
    }
    if !response.status().is_success() {
        return Err(ComprehensiveRulesError::Invalid(format!(
            "The {label} returned HTTP {}.",
            response.status()
        )));
    }
    Ok(())
}

fn bounded_header(value: Option<&reqwest::header::HeaderValue>) -> Option<String> {
    value
        .and_then(|header| header.to_str().ok())
        .filter(|header| header.len() <= 512 && !header.chars().any(char::is_control))
        .map(str::to_string)
}

fn status_from_snapshot(snapshot: &ComprehensiveRulesSnapshot) -> ComprehensiveRulesStatus {
    let (capability_count, changed_capability_rule_ids) =
        crate::rules_capabilities::capability_compatibility(snapshot);
    let compatibility = if changed_capability_rule_ids.is_empty() {
        RulesCompatibility::Compatible
    } else if changed_capability_rule_ids.len() == capability_count {
        RulesCompatibility::ReferenceOnly
    } else {
        RulesCompatibility::Changed
    };
    ComprehensiveRulesStatus {
        ready: true,
        schema_version: snapshot.schema_version.clone(),
        parser_version: snapshot.parser_version.clone(),
        effective_date: Some(snapshot.effective_date.clone()),
        installed_at: Some(snapshot.installed_at.clone()),
        source_page_url: snapshot.source_page_url.clone(),
        document_url: Some(snapshot.document_url.clone()),
        etag: snapshot.etag.clone(),
        last_modified: snapshot.last_modified.clone(),
        snapshot_sha256: Some(snapshot.snapshot_sha256.clone()),
        document_bytes: Some(snapshot.document_bytes),
        rule_count: snapshot.rule_count(),
        section_count: snapshot.section_count,
        example_count: snapshot.example_count,
        glossary_count: snapshot.glossary_count,
        commander_rule_count: snapshot.commander_rule_count,
        compatibility,
        changed_capability_rule_ids,
        authenticity_basis:
            "Downloaded from the exact allowlisted official Wizards HTTPS endpoint; locally hashed with SHA-256. Wizards does not publish a signed manifest for this document."
                .into(),
        message: format!(
            "Official Comprehensive Rules effective {} are installed locally.",
            snapshot.effective_date
        ),
    }
}

fn empty_status() -> ComprehensiveRulesStatus {
    ComprehensiveRulesStatus {
        ready: false,
        schema_version: COMPREHENSIVE_RULES_SCHEMA_VERSION.into(),
        parser_version: COMPREHENSIVE_RULES_PARSER_VERSION.into(),
        effective_date: None,
        installed_at: None,
        source_page_url: COMPREHENSIVE_RULES_SOURCE_PAGE.into(),
        document_url: None,
        etag: None,
        last_modified: None,
        snapshot_sha256: None,
        document_bytes: None,
        rule_count: 0,
        section_count: 0,
        example_count: 0,
        glossary_count: 0,
        commander_rule_count: 0,
        compatibility: RulesCompatibility::NotInstalled,
        changed_capability_rule_ids: Vec::new(),
        authenticity_basis: "Not installed. No Comprehensive Rules document has been downloaded."
            .into(),
        message:
            "Install the official Comprehensive Rules to enable rules-backed semantic annotations."
                .into(),
    }
}

fn emit(
    reporter: &Option<ComprehensiveRulesUpdateReporter>,
    phase: &str,
    completed_units: u64,
    total_units: Option<u64>,
    progress: f32,
    detail: &str,
) {
    if let Some(reporter) = reporter {
        reporter(ComprehensiveRulesUpdateProgress {
            phase: phase.into(),
            completed_units,
            total_units,
            progress,
            detail: detail.into(),
        });
    }
}
