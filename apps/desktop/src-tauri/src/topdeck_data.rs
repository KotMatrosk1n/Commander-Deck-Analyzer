//! Explicit ingestion of aggregate TopDeck EDH tournament observations.
//!
//! This module intentionally does not retain raw API responses, credentials,
//! tournament names, locations, player names, player IDs, emails, Discord
//! handles, or raw decklists. The documented bulk endpoint is queried with
//! standings columns that omit player identity and with round data disabled.
//! The response is reduced in memory to pseudonymized tournament keys and
//! commander-level aggregates before an atomic local snapshot is written.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use reqwest::{Client, Response, StatusCode};
use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Number;
use sha2::{Digest, Sha256};
use url::Url;

use crate::card_data::CardRepository;
use crate::parser::normalize_card_name;

pub(crate) const TOPDECK_EDH_SCHEMA_VERSION: &str = "topdeck-edh-tournament-observations/v1";
pub(crate) const TOPDECK_EDH_INGESTOR_VERSION: &str = "topdeck-edh-ingestor-1";
pub(crate) const TOPDECK_EDH_DATASET_NAME: &str = "TopDeck EDH tournament observations";
pub(crate) const TOPDECK_TOURNAMENTS_ENDPOINT: &str = "https://topdeck.gg/api/v2/tournaments";
pub(crate) const TOPDECK_ATTRIBUTION_TEXT: &str = "Data provided by TopDeck.gg";
pub(crate) const TOPDECK_ATTRIBUTION_URL: &str = "https://topdeck.gg";

const TOPDECK_GAME: &str = "Magic: The Gathering";
const TOPDECK_FORMAT: &str = "EDH";
const USER_AGENT: &str = concat!(
    "CommanderDeckAnalyzer/",
    env!("CARGO_PKG_VERSION"),
    " TopDeckEdhUpdater"
);
const MAX_API_KEY_BYTES: usize = 1_024;
const MAX_REQUEST_BYTES: usize = 4 * 1_024;
const MAX_RESPONSE_BYTES: usize = 64 * 1_024 * 1_024;
const MAX_SNAPSHOT_BYTES: u64 = 32 * 1_024 * 1_024;
const MAX_TOURNAMENTS: usize = 10_000;
const MAX_STANDINGS: u64 = 250_000;
const MAX_UNIQUE_COMMANDER_CANDIDATES: usize = 25_000;
const MAX_TOURNAMENT_ID_BYTES: usize = 512;
const MAX_DECKLIST_BYTES: usize = 2 * 1_024 * 1_024;
const MAX_COMMANDER_NAME_BYTES: usize = 512;
const MAX_ROUNDS_PER_RESULT: u64 = 10_000;
const MAX_PARTICIPANTS: u64 = 100_000;
const MAX_LAST_DAYS: u32 = 3_660;
const MAX_DATE_RANGE_SECONDS: i64 = 3_660 * 24 * 60 * 60;
const MIN_SUPPORTED_TIMESTAMP: i64 = 946_684_800; // 2000-01-01 UTC.
const MAX_SUPPORTED_TIMESTAMP: i64 = 4_102_444_800; // 2100-01-01 UTC.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const UPDATE_TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) type TopDeckEdhUpdateReporter = Arc<dyn Fn(TopDeckEdhUpdateProgress) + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub(crate) enum TopDeckEdhError {
    #[error("TopDeck.gg request failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("TopDeck EDH snapshot file error: {0}")]
    Io(#[from] io::Error),
    #[error("TopDeck EDH snapshot JSON was invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

/// The user-selected query boundary persisted with the sanitized snapshot.
///
/// Exactly one time mode is accepted: either `last`, or a complete
/// `start`/`end` Unix-second range. This prevents an accidental unbounded bulk
/// query while retaining the exact scope that was requested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TopDeckEdhQueryScope {
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub last: Option<u32>,
    pub participant_min: Option<u32>,
    pub participant_max: Option<u32>,
    #[serde(default)]
    pub include_leagues: bool,
}

/// An API key is accepted only on this update input. This type deliberately
/// does not implement `Serialize`, `Clone`, or `Debug`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TopDeckEdhUpdateRequest {
    pub api_key: String,
    pub scope: TopDeckEdhQueryScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopDeckCommanderAggregate {
    /// Lowercase, whitespace-normalized Oracle card names. One or two names.
    pub commanders: Vec<String>,
    pub entries: u64,
    pub wins: u64,
    pub draws: u64,
    pub losses: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopDeckTournamentObservation {
    /// Stable SHA-256 pseudonym derived from the public TopDeck tournament ID.
    pub tournament_key: String,
    pub start_date: i64,
    pub swiss_rounds: Option<u32>,
    pub top_cut_size: Option<u32>,
    pub is_league: bool,
    pub participant_count: u64,
    pub identified_commander_entries: u64,
    pub unidentified_entries: u64,
    pub total_wins: u64,
    pub total_draws: u64,
    pub total_losses: u64,
    pub commanders: Vec<TopDeckCommanderAggregate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredCommanderAggregate {
    commanders: Vec<String>,
    entries: u64,
    wins: u64,
    draws: u64,
    losses: u64,
}

impl From<TopDeckCommanderAggregate> for StoredCommanderAggregate {
    fn from(value: TopDeckCommanderAggregate) -> Self {
        Self {
            commanders: value.commanders,
            entries: value.entries,
            wins: value.wins,
            draws: value.draws,
            losses: value.losses,
        }
    }
}

impl From<&StoredCommanderAggregate> for TopDeckCommanderAggregate {
    fn from(value: &StoredCommanderAggregate) -> Self {
        Self {
            commanders: value.commanders.clone(),
            entries: value.entries,
            wins: value.wins,
            draws: value.draws,
            losses: value.losses,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredTournamentObservation {
    tournament_key: String,
    start_date: i64,
    swiss_rounds: Option<u32>,
    top_cut_size: Option<u32>,
    is_league: bool,
    participant_count: u64,
    identified_commander_entries: u64,
    unidentified_entries: u64,
    total_wins: u64,
    total_draws: u64,
    total_losses: u64,
    commanders: Vec<StoredCommanderAggregate>,
}

impl From<TopDeckTournamentObservation> for StoredTournamentObservation {
    fn from(value: TopDeckTournamentObservation) -> Self {
        Self {
            tournament_key: value.tournament_key,
            start_date: value.start_date,
            swiss_rounds: value.swiss_rounds,
            top_cut_size: value.top_cut_size,
            is_league: value.is_league,
            participant_count: value.participant_count,
            identified_commander_entries: value.identified_commander_entries,
            unidentified_entries: value.unidentified_entries,
            total_wins: value.total_wins,
            total_draws: value.total_draws,
            total_losses: value.total_losses,
            commanders: value
                .commanders
                .into_iter()
                .map(StoredCommanderAggregate::from)
                .collect(),
        }
    }
}

impl From<&StoredTournamentObservation> for TopDeckTournamentObservation {
    fn from(value: &StoredTournamentObservation) -> Self {
        Self {
            tournament_key: value.tournament_key.clone(),
            start_date: value.start_date,
            swiss_rounds: value.swiss_rounds,
            top_cut_size: value.top_cut_size,
            is_league: value.is_league,
            participant_count: value.participant_count,
            identified_commander_entries: value.identified_commander_entries,
            unidentified_entries: value.unidentified_entries,
            total_wins: value.total_wins,
            total_draws: value.total_draws,
            total_losses: value.total_losses,
            commanders: value
                .commanders
                .iter()
                .map(TopDeckCommanderAggregate::from)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TopDeckEdhSnapshot {
    pub schema_version: String,
    pub ingestor_version: String,
    pub dataset_name: String,
    pub installed_at: String,
    pub endpoint: String,
    pub query_scope: TopDeckEdhQueryScope,
    pub request_fingerprint: String,
    pub source_response_sha256: String,
    pub source_response_bytes: u64,
    /// Identity for future analysis-cache invalidation. This includes only the
    /// normalized scope and sanitized observations, not wall-clock install time
    /// or discarded identity-bearing response fields.
    pub cache_fingerprint: String,
    /// Integrity identity for the complete persisted record, including source
    /// provenance and install time. This remains separate from cache identity
    /// so a provenance-only change does not invalidate analysis behavior.
    pub snapshot_integrity_sha256: String,
    pub attribution_text: String,
    pub attribution_url: String,
    tournaments: Vec<StoredTournamentObservation>,
}

impl TopDeckEdhSnapshot {
    #[allow(dead_code)] // Foundation for later aggregate-data consumers.
    pub(crate) fn observations(&self) -> Vec<TopDeckTournamentObservation> {
        self.tournaments
            .iter()
            .map(TopDeckTournamentObservation::from)
            .collect()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopDeckEdhStatus {
    pub ready: bool,
    pub schema_version: String,
    pub ingestor_version: String,
    pub dataset_name: String,
    pub installed_at: Option<String>,
    pub endpoint: String,
    pub query_scope: Option<TopDeckEdhQueryScope>,
    pub request_fingerprint: Option<String>,
    pub source_response_sha256: Option<String>,
    pub source_response_bytes: Option<u64>,
    pub cache_fingerprint: Option<String>,
    pub snapshot_integrity_sha256: Option<String>,
    pub tournament_count: u64,
    pub standing_count: u64,
    pub identified_commander_entries: u64,
    pub unidentified_entries: u64,
    pub commander_archetype_count: u64,
    pub attribution_text: String,
    pub attribution_url: String,
    pub authenticity_basis: String,
    pub privacy_summary: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopDeckEdhUpdateProgress {
    pub phase: String,
    pub completed_units: u64,
    pub total_units: Option<u64>,
    pub progress: f32,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub(crate) enum TopDeckEdhUpdateOutcome {
    Installed { status: TopDeckEdhStatus },
    Unchanged { status: TopDeckEdhStatus },
}

#[derive(Debug, Clone)]
pub(crate) struct TopDeckEdhStore {
    root: PathBuf,
}

impl TopDeckEdhStore {
    pub(crate) fn new(root: impl Into<PathBuf>) -> Result<Self, TopDeckEdhError> {
        let store = Self { root: root.into() };
        fs::create_dir_all(&store.root)?;
        if store.next_path().exists() {
            fs::remove_file(store.next_path())?;
        }
        store.recover_previous_if_needed()?;
        Ok(store)
    }

    pub(crate) fn status(&self) -> Result<TopDeckEdhStatus, TopDeckEdhError> {
        match self.load_active()? {
            Some(snapshot) => {
                let mut status = status_from_snapshot(&snapshot);
                if self.corrupt_path().exists() {
                    status.message = format!(
                        "A damaged newer {} snapshot was quarantined and the previous aggregate snapshot was restored.",
                        TOPDECK_EDH_DATASET_NAME
                    );
                }
                Ok(status)
            }
            None => {
                let mut status = empty_status();
                if self.corrupt_path().exists() {
                    status.message = format!(
                        "A damaged local {} snapshot was quarantined. Run an explicit update with a TopDeck.gg API key to restore it.",
                        TOPDECK_EDH_DATASET_NAME
                    );
                }
                Ok(status)
            }
        }
    }

    #[allow(dead_code)] // Foundation for later aggregate-data consumers.
    pub(crate) fn load_active(&self) -> Result<Option<TopDeckEdhSnapshot>, TopDeckEdhError> {
        // Re-check on each read so corruption discovered after process startup
        // cannot soft-lock status or a replacement update until restart.
        self.recover_previous_if_needed()?;
        let path = self.live_path();
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(load_snapshot_file(&path)?))
    }

    pub(crate) async fn update_from_network(
        &self,
        request: TopDeckEdhUpdateRequest,
        card_repository: &CardRepository,
        reporter: Option<TopDeckEdhUpdateReporter>,
    ) -> Result<TopDeckEdhUpdateOutcome, TopDeckEdhError> {
        let TopDeckEdhUpdateRequest { api_key, scope } = request;
        validate_scope(&scope)?;
        let request_body = api_request_body(&scope)?;
        if request_body.len() > MAX_REQUEST_BYTES {
            return Err(TopDeckEdhError::Invalid(
                "The TopDeck.gg request exceeded the 4 KiB safety limit.".into(),
            ));
        }
        let authorization = validate_api_key(&api_key)?;
        let endpoint = Url::parse(TOPDECK_TOURNAMENTS_ENDPOINT)
            .map_err(|error| TopDeckEdhError::Invalid(error.to_string()))?;
        validate_endpoint(&endpoint)?;

        emit(
            &reporter,
            "request",
            0,
            None,
            0.02,
            "Requesting the selected TopDeck EDH tournament scope",
        );
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(UPDATE_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()?;
        let response = client
            .post(endpoint)
            .header(AUTHORIZATION, authorization)
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json")
            .body(request_body)
            .send()
            .await?;
        // The raw credential is no longer needed after the authenticated
        // request has been sent. It never enters status, progress, errors, or
        // the persisted snapshot.
        drop(api_key);
        require_success_without_redirect(&response)?;
        validate_response_content_type(&response)?;
        let total_bytes = response.content_length();
        emit(
            &reporter,
            "download",
            0,
            total_bytes,
            0.08,
            "Downloading a bounded TopDeck EDH response",
        );
        let response_bytes = read_bounded_response(response, MAX_RESPONSE_BYTES, &reporter).await?;

        emit(
            &reporter,
            "aggregate",
            response_bytes.len() as u64,
            Some(response_bytes.len() as u64),
            0.72,
            "Discarding identity fields and aggregating commander observations",
        );
        let snapshot = build_snapshot(
            &response_bytes,
            scope,
            Utc::now().to_rfc3339(),
            |candidate_names| {
                let resolved = card_repository.get_many(candidate_names).map_err(|error| {
                    TopDeckEdhError::Invalid(format!(
                        "The local Oracle card database could not validate TopDeck commander names: {error}"
                    ))
                })?;
                Ok(resolved
                    .into_iter()
                    .filter_map(|(lookup_name, card)| {
                        normalize_commander_name(&card.name)
                            .map(|canonical_name| (lookup_name, canonical_name))
                    })
                    .collect())
            },
        )?;
        if self.load_active()?.as_ref().is_some_and(|active| {
            active.cache_fingerprint == snapshot.cache_fingerprint
                && active.source_response_sha256 == snapshot.source_response_sha256
        }) {
            emit(
                &reporter,
                "complete",
                snapshot.tournaments.len() as u64,
                Some(snapshot.tournaments.len() as u64),
                1.0,
                "The sanitized local observations are unchanged",
            );
            return Ok(TopDeckEdhUpdateOutcome::Unchanged {
                status: self.status()?,
            });
        }

        emit(
            &reporter,
            "activate",
            snapshot.tournaments.len() as u64,
            Some(snapshot.tournaments.len() as u64),
            0.92,
            "Atomically activating the sanitized aggregate snapshot",
        );
        self.activate(&snapshot)?;
        emit(
            &reporter,
            "complete",
            snapshot.tournaments.len() as u64,
            Some(snapshot.tournaments.len() as u64),
            1.0,
            "TopDeck EDH tournament observations are ready locally",
        );
        Ok(TopDeckEdhUpdateOutcome::Installed {
            status: self.status()?,
        })
    }

    fn activate(&self, snapshot: &TopDeckEdhSnapshot) -> Result<(), TopDeckEdhError> {
        validate_snapshot(snapshot)?;
        let encoded = serde_json::to_vec(snapshot)?;
        if encoded.len() as u64 > MAX_SNAPSHOT_BYTES {
            return Err(TopDeckEdhError::Invalid(
                "The sanitized TopDeck EDH snapshot exceeded the 32 MiB safety limit.".into(),
            ));
        }

        let next = self.next_path();
        let live = self.live_path();
        let previous = self.previous_path();
        if next.exists() {
            fs::remove_file(&next)?;
        }
        let mut staged = fs::File::create(&next)?;
        staged.write_all(&encoded)?;
        staged.sync_all()?;
        drop(staged);
        load_snapshot_file(&next)?;

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

    fn recover_previous_if_needed(&self) -> Result<(), TopDeckEdhError> {
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
        self.root.join("topdeck-edh.json")
    }

    fn next_path(&self) -> PathBuf {
        self.root.join("topdeck-edh.next.json")
    }

    fn previous_path(&self) -> PathBuf {
        self.root.join("topdeck-edh.previous.json")
    }

    fn corrupt_path(&self) -> PathBuf {
        self.root.join("topdeck-edh.corrupt.json")
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TopDeckApiQuery<'a> {
    game: &'static str,
    format: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    participant_min: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    participant_max: Option<u32>,
    columns: &'a [&'static str],
    rounds: bool,
    leagues: bool,
}

fn api_request_body(scope: &TopDeckEdhQueryScope) -> Result<Vec<u8>, TopDeckEdhError> {
    validate_scope(scope)?;
    let query = TopDeckApiQuery {
        game: TOPDECK_GAME,
        format: TOPDECK_FORMAT,
        start: scope.start,
        end: scope.end,
        last: scope.last,
        participant_min: scope.participant_min,
        participant_max: scope.participant_max,
        // Explicitly omit identity columns and round/player data.
        columns: &["decklist", "wins", "draws", "losses"],
        rounds: false,
        leagues: scope.include_leagues,
    };
    Ok(serde_json::to_vec(&query)?)
}

fn validate_scope(scope: &TopDeckEdhQueryScope) -> Result<(), TopDeckEdhError> {
    match (scope.start, scope.end, scope.last) {
        (None, None, Some(last)) if (1..=MAX_LAST_DAYS).contains(&last) => {}
        (Some(start), Some(end), None)
            if (MIN_SUPPORTED_TIMESTAMP..=MAX_SUPPORTED_TIMESTAMP).contains(&start)
                && (MIN_SUPPORTED_TIMESTAMP..=MAX_SUPPORTED_TIMESTAMP).contains(&end)
                && end >= start
                && end - start <= MAX_DATE_RANGE_SECONDS => {}
        (None, None, Some(_)) => {
            return Err(TopDeckEdhError::Invalid(format!(
                "TopDeck EDH `last` must be between 1 and {MAX_LAST_DAYS} days."
            )));
        }
        _ => {
            return Err(TopDeckEdhError::Invalid(
                "Choose either a bounded `last` window or a complete bounded `start`/`end` range."
                    .into(),
            ));
        }
    }

    if scope
        .participant_min
        .is_some_and(|value| value == 0 || value as u64 > MAX_PARTICIPANTS)
        || scope
            .participant_max
            .is_some_and(|value| value == 0 || value as u64 > MAX_PARTICIPANTS)
    {
        return Err(TopDeckEdhError::Invalid(format!(
            "TopDeck participant bounds must be between 1 and {MAX_PARTICIPANTS}."
        )));
    }
    if let (Some(minimum), Some(maximum)) = (scope.participant_min, scope.participant_max)
        && minimum > maximum
    {
        return Err(TopDeckEdhError::Invalid(
            "TopDeck participantMin cannot exceed participantMax.".into(),
        ));
    }
    Ok(())
}

fn validate_api_key(api_key: &str) -> Result<HeaderValue, TopDeckEdhError> {
    if api_key.is_empty()
        || api_key.len() > MAX_API_KEY_BYTES
        || api_key.trim() != api_key
        || api_key.chars().any(char::is_control)
    {
        return Err(TopDeckEdhError::Invalid(
            "Enter a valid TopDeck.gg API key for this update.".into(),
        ));
    }
    HeaderValue::from_str(api_key).map_err(|_| {
        TopDeckEdhError::Invalid("Enter a valid TopDeck.gg API key for this update.".into())
    })
}

fn validate_endpoint(url: &Url) -> Result<(), TopDeckEdhError> {
    let valid = url.scheme() == "https"
        && url.host_str() == Some("topdeck.gg")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/api/v2/tournaments"
        && url.query().is_none()
        && url.fragment().is_none();
    if !valid {
        return Err(TopDeckEdhError::Invalid(
            "The TopDeck tournaments endpoint did not pass the exact HTTPS allowlist.".into(),
        ));
    }
    Ok(())
}

fn require_success_without_redirect(response: &Response) -> Result<(), TopDeckEdhError> {
    if response.status().is_redirection() {
        return Err(TopDeckEdhError::Invalid(
            "The TopDeck.gg endpoint attempted an unexpected redirect.".into(),
        ));
    }
    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(TopDeckEdhError::Invalid(
            "TopDeck.gg rejected the API key. The local snapshot was left unchanged.".into(),
        ));
    }
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        return Err(TopDeckEdhError::Invalid(
            "TopDeck.gg rate-limited the update. Try again later; the local snapshot was left unchanged."
                .into(),
        ));
    }
    if !response.status().is_success() {
        return Err(TopDeckEdhError::Invalid(format!(
            "TopDeck.gg returned HTTP {}. The local snapshot was left unchanged.",
            response.status()
        )));
    }
    Ok(())
}

fn validate_response_content_type(response: &Response) -> Result<(), TopDeckEdhError> {
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if content_type != Some("application/json") {
        return Err(TopDeckEdhError::Invalid(
            "TopDeck.gg returned an unsupported content type; expected application/json.".into(),
        ));
    }
    Ok(())
}

async fn read_bounded_response(
    mut response: Response,
    maximum_bytes: usize,
    reporter: &Option<TopDeckEdhUpdateReporter>,
) -> Result<Vec<u8>, TopDeckEdhError> {
    if response
        .content_length()
        .is_some_and(|size| size > maximum_bytes as u64)
    {
        return Err(TopDeckEdhError::Invalid(
            "The TopDeck.gg response exceeded the 64 MiB safety limit.".into(),
        ));
    }
    let total = response.content_length();
    let capacity = total
        .and_then(|size| usize::try_from(size).ok())
        .unwrap_or(0)
        .min(maximum_bytes);
    let mut body = Vec::with_capacity(capacity);
    while let Some(chunk) = response.chunk().await? {
        append_bounded(&mut body, &chunk, maximum_bytes)?;
        let ratio = total
            .filter(|value| *value > 0)
            .map(|value| body.len() as f32 / value as f32)
            .unwrap_or(0.0);
        emit(
            reporter,
            "download",
            body.len() as u64,
            total,
            0.08 + ratio.clamp(0.0, 1.0) * 0.54,
            "Downloading a bounded TopDeck EDH response",
        );
    }
    Ok(body)
}

fn append_bounded(
    destination: &mut Vec<u8>,
    chunk: &[u8],
    maximum_bytes: usize,
) -> Result<(), TopDeckEdhError> {
    if destination.len().saturating_add(chunk.len()) > maximum_bytes {
        return Err(TopDeckEdhError::Invalid(
            "The TopDeck.gg response exceeded the configured safety limit.".into(),
        ));
    }
    destination.extend_from_slice(chunk);
    Ok(())
}

struct RawTournament {
    tid: String,
    start_date: Number,
    game: String,
    format: String,
    swiss_num: Option<Number>,
    top_cut: Option<Number>,
    is_league: bool,
    is_team_event: bool,
    standings: Vec<RawStanding>,
}

struct RawStanding {
    commander_candidates: Option<Vec<String>>,
    wins: Option<Number>,
    draws: Option<Number>,
    losses: Option<Number>,
}

struct TournamentResponseSeed {
    max_tournaments: usize,
    max_standings: u64,
}

impl<'de> DeserializeSeed<'de> for TournamentResponseSeed {
    type Value = Vec<RawTournament>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_seq(TournamentResponseVisitor {
            max_tournaments: self.max_tournaments,
            max_standings: self.max_standings,
        })
    }
}

struct TournamentResponseVisitor {
    max_tournaments: usize,
    max_standings: u64,
}

impl<'de> Visitor<'de> for TournamentResponseVisitor {
    type Value = Vec<RawTournament>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an array of TopDeck tournaments")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut tournaments = Vec::new();
        let mut standing_count = 0u64;
        loop {
            if tournaments.len() >= self.max_tournaments {
                if sequence.next_element::<IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom(format!(
                        "TopDeck.gg returned more than {} tournaments",
                        self.max_tournaments
                    )));
                }
                break;
            }
            let Some(tournament) = sequence.next_element_seed(RawTournamentSeed {
                standing_count: &mut standing_count,
                max_standings: self.max_standings,
            })?
            else {
                break;
            };
            tournaments.push(tournament);
        }
        Ok(tournaments)
    }
}

struct RawTournamentSeed<'a> {
    standing_count: &'a mut u64,
    max_standings: u64,
}

impl<'de> DeserializeSeed<'de> for RawTournamentSeed<'_> {
    type Value = RawTournament;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_map(RawTournamentVisitor {
            standing_count: self.standing_count,
            max_standings: self.max_standings,
        })
    }
}

struct RawTournamentVisitor<'a> {
    standing_count: &'a mut u64,
    max_standings: u64,
}

impl<'de> Visitor<'de> for RawTournamentVisitor<'_> {
    type Value = RawTournament;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a TopDeck tournament object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tid = None;
        let mut start_date = None;
        let mut game = None;
        let mut format = None;
        let mut swiss_num = None;
        let mut saw_swiss_num = false;
        let mut top_cut = None;
        let mut saw_top_cut = false;
        let mut is_league = None;
        let mut is_team_event = None;
        let mut standings = None;

        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                "TID" => set_once(&mut tid, map.next_value()?, "TID")?,
                "startDate" => {
                    set_once(&mut start_date, map.next_value()?, "startDate")?;
                }
                "game" => set_once(&mut game, map.next_value()?, "game")?,
                "format" => set_once(&mut format, map.next_value()?, "format")?,
                "swissNum" => {
                    if saw_swiss_num {
                        return Err(de::Error::duplicate_field("swissNum"));
                    }
                    saw_swiss_num = true;
                    swiss_num = map.next_value::<Option<Number>>()?;
                }
                "topCut" => {
                    if saw_top_cut {
                        return Err(de::Error::duplicate_field("topCut"));
                    }
                    saw_top_cut = true;
                    top_cut = map.next_value::<Option<Number>>()?;
                }
                "isLeague" => {
                    set_once(&mut is_league, map.next_value()?, "isLeague")?;
                }
                "isTeamEvent" => {
                    set_once(&mut is_team_event, map.next_value()?, "isTeamEvent")?;
                }
                "standings" => {
                    if standings.is_some() {
                        return Err(de::Error::duplicate_field("standings"));
                    }
                    standings = Some(map.next_value_seed(StandingListSeed {
                        standing_count: self.standing_count,
                        max_standings: self.max_standings,
                    })?);
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        Ok(RawTournament {
            tid: tid.ok_or_else(|| de::Error::missing_field("TID"))?,
            start_date: start_date.ok_or_else(|| de::Error::missing_field("startDate"))?,
            game: game.ok_or_else(|| de::Error::missing_field("game"))?,
            format: format.ok_or_else(|| de::Error::missing_field("format"))?,
            swiss_num,
            top_cut,
            is_league: is_league.unwrap_or(false),
            is_team_event: is_team_event.unwrap_or(false),
            standings: standings.unwrap_or_default(),
        })
    }
}

fn set_once<T, E>(slot: &mut Option<T>, value: T, field: &'static str) -> Result<(), E>
where
    E: de::Error,
{
    if slot.is_some() {
        return Err(E::duplicate_field(field));
    }
    *slot = Some(value);
    Ok(())
}

struct StandingListSeed<'a> {
    standing_count: &'a mut u64,
    max_standings: u64,
}

impl<'de> DeserializeSeed<'de> for StandingListSeed<'_> {
    type Value = Vec<RawStanding>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_seq(StandingListVisitor {
            standing_count: self.standing_count,
            max_standings: self.max_standings,
        })
    }
}

struct StandingListVisitor<'a> {
    standing_count: &'a mut u64,
    max_standings: u64,
}

impl<'de> Visitor<'de> for StandingListVisitor<'_> {
    type Value = Vec<RawStanding>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an array of TopDeck standings")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut standings = Vec::new();
        loop {
            if *self.standing_count >= self.max_standings {
                if sequence.next_element::<IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom(format!(
                        "TopDeck.gg returned more than {} standings",
                        self.max_standings
                    )));
                }
                break;
            }
            let Some(standing) = sequence.next_element::<RawStanding>()? else {
                break;
            };
            *self.standing_count += 1;
            standings.push(standing);
        }
        Ok(standings)
    }
}

impl<'de> Deserialize<'de> for RawStanding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_map(RawStandingVisitor)
    }
}

struct RawStandingVisitor;

impl<'de> Visitor<'de> for RawStandingVisitor {
    type Value = RawStanding;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a TopDeck standing object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut commander_candidates = None;
        let mut saw_decklist = false;
        let mut wins = None;
        let mut saw_wins = false;
        let mut draws = None;
        let mut saw_draws = false;
        let mut losses = None;
        let mut saw_losses = false;

        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                "decklist" => {
                    if saw_decklist {
                        return Err(de::Error::duplicate_field("decklist"));
                    }
                    saw_decklist = true;
                    let decklist = map.next_value::<Option<String>>()?;
                    if decklist
                        .as_ref()
                        .is_some_and(|value| value.len() > MAX_DECKLIST_BYTES)
                    {
                        return Err(de::Error::custom(
                            "TopDeck.gg returned a decklist larger than the 2 MiB per-entry safety limit",
                        ));
                    }
                    commander_candidates = decklist.as_deref().and_then(commanders_from_decklist);
                }
                "deckObj" => {
                    // Structured deck data can be arbitrarily nested. The
                    // bounded plaintext decklist is sufficient for extracting
                    // candidates, and the local Oracle database verifies them.
                    map.next_value::<IgnoredAny>()?;
                }
                "wins" => {
                    if saw_wins {
                        return Err(de::Error::duplicate_field("wins"));
                    }
                    saw_wins = true;
                    wins = map.next_value::<Option<Number>>()?;
                }
                "draws" => {
                    if saw_draws {
                        return Err(de::Error::duplicate_field("draws"));
                    }
                    saw_draws = true;
                    draws = map.next_value::<Option<Number>>()?;
                }
                "losses" => {
                    if saw_losses {
                        return Err(de::Error::duplicate_field("losses"));
                    }
                    saw_losses = true;
                    losses = map.next_value::<Option<Number>>()?;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(RawStanding {
            commander_candidates,
            wins,
            draws,
            losses,
        })
    }
}

fn parse_bounded_tournaments(response_bytes: &[u8]) -> Result<Vec<RawTournament>, TopDeckEdhError> {
    parse_bounded_tournaments_with_limits(response_bytes, MAX_TOURNAMENTS, MAX_STANDINGS)
}

fn parse_bounded_tournaments_with_limits(
    response_bytes: &[u8],
    max_tournaments: usize,
    max_standings: u64,
) -> Result<Vec<RawTournament>, TopDeckEdhError> {
    let mut deserializer = serde_json::Deserializer::from_slice(response_bytes);
    let tournaments = TournamentResponseSeed {
        max_tournaments,
        max_standings,
    }
    .deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(tournaments)
}

#[derive(Default)]
struct MutableCommanderAggregate {
    entries: u64,
    wins: u64,
    draws: u64,
    losses: u64,
}

fn build_snapshot<F>(
    response_bytes: &[u8],
    scope: TopDeckEdhQueryScope,
    installed_at: String,
    resolve_commander_names: F,
) -> Result<TopDeckEdhSnapshot, TopDeckEdhError>
where
    F: FnOnce(&[String]) -> Result<HashMap<String, String>, TopDeckEdhError>,
{
    validate_scope(&scope)?;
    DateTime::parse_from_rfc3339(&installed_at).map_err(|_| {
        TopDeckEdhError::Invalid("The TopDeck snapshot install timestamp was invalid.".into())
    })?;
    let raw = parse_bounded_tournaments(response_bytes)?;
    let candidate_names = raw
        .iter()
        .flat_map(|tournament| tournament.standings.iter())
        .filter_map(|standing| standing.commander_candidates.as_ref())
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if candidate_names.len() > MAX_UNIQUE_COMMANDER_CANDIDATES {
        return Err(TopDeckEdhError::Invalid(format!(
            "TopDeck.gg returned more than {MAX_UNIQUE_COMMANDER_CANDIDATES} distinct commander candidates."
        )));
    }
    let candidate_lookups = candidate_names
        .iter()
        .map(|candidate| normalize_card_name(candidate))
        .collect::<HashSet<_>>();
    let verified_commanders = resolve_commander_names(&candidate_names)?;
    for (lookup, canonical) in &verified_commanders {
        if !candidate_lookups.contains(lookup)
            || normalize_commander_name(canonical).as_deref() != Some(canonical.as_str())
        {
            return Err(TopDeckEdhError::Invalid(
                "The local Oracle commander-name resolver returned a non-canonical result.".into(),
            ));
        }
    }

    let mut tournament_keys = HashSet::new();
    let mut standing_count = 0u64;
    let mut tournaments = Vec::with_capacity(raw.len());
    for tournament in raw {
        if tournament.game != TOPDECK_GAME || tournament.format != TOPDECK_FORMAT {
            return Err(TopDeckEdhError::Invalid(
                "TopDeck.gg returned a tournament outside the requested Magic: The Gathering / EDH scope."
                    .into(),
            ));
        }
        if tournament.is_team_event {
            return Err(TopDeckEdhError::Invalid(
                "TopDeck.gg returned a team event, which this EDH aggregate schema does not model."
                    .into(),
            ));
        }
        if tournament.is_league && !scope.include_leagues {
            return Err(TopDeckEdhError::Invalid(
                "TopDeck.gg returned a league outside the selected query scope.".into(),
            ));
        }
        if tournament.tid.is_empty()
            || tournament.tid.len() > MAX_TOURNAMENT_ID_BYTES
            || tournament.tid.chars().any(char::is_control)
        {
            return Err(TopDeckEdhError::Invalid(
                "TopDeck.gg returned an invalid tournament identifier.".into(),
            ));
        }
        let tournament_key = pseudonymize_tournament_id(&tournament.tid);
        if !tournament_keys.insert(tournament_key.clone()) {
            return Err(TopDeckEdhError::Invalid(
                "TopDeck.gg returned the same tournament more than once.".into(),
            ));
        }

        let start_date = number_to_i64(&tournament.start_date, "startDate")?;
        validate_returned_start_date(start_date, &scope)?;
        let swiss_rounds = optional_bounded_u32(
            tournament.swiss_num.as_ref(),
            "swissNum",
            MAX_ROUNDS_PER_RESULT,
        )?;
        let top_cut_size =
            optional_bounded_u32(tournament.top_cut.as_ref(), "topCut", MAX_PARTICIPANTS)?;
        let participant_count = tournament.standings.len() as u64;
        validate_returned_participants(participant_count, &scope)?;
        standing_count = checked_add(standing_count, participant_count, "standing count")?;
        if standing_count > MAX_STANDINGS {
            return Err(TopDeckEdhError::Invalid(format!(
                "TopDeck.gg returned more than {MAX_STANDINGS} standings."
            )));
        }

        let mut commander_groups = BTreeMap::<Vec<String>, MutableCommanderAggregate>::new();
        let mut identified_commander_entries = 0u64;
        let mut total_wins = 0u64;
        let mut total_draws = 0u64;
        let mut total_losses = 0u64;
        for standing in tournament.standings {
            let wins = required_result_count(standing.wins.as_ref(), "wins")?;
            let draws = required_result_count(standing.draws.as_ref(), "draws")?;
            let losses = required_result_count(standing.losses.as_ref(), "losses")?;
            total_wins = checked_add(total_wins, wins, "win total")?;
            total_draws = checked_add(total_draws, draws, "draw total")?;
            total_losses = checked_add(total_losses, losses, "loss total")?;

            let commanders = verified_commander_names(&standing, &verified_commanders);
            if let Some(commanders) = commanders {
                identified_commander_entries =
                    checked_add(identified_commander_entries, 1, "identified deck count")?;
                let aggregate = commander_groups.entry(commanders).or_default();
                aggregate.entries = checked_add(aggregate.entries, 1, "commander entries")?;
                aggregate.wins = checked_add(aggregate.wins, wins, "commander wins")?;
                aggregate.draws = checked_add(aggregate.draws, draws, "commander draws")?;
                aggregate.losses = checked_add(aggregate.losses, losses, "commander losses")?;
            }
        }
        let unidentified_entries = participant_count
            .checked_sub(identified_commander_entries)
            .ok_or_else(|| {
                TopDeckEdhError::Invalid("TopDeck commander counts were inconsistent.".into())
            })?;
        let commanders = commander_groups
            .into_iter()
            .map(|(commanders, aggregate)| {
                StoredCommanderAggregate::from(TopDeckCommanderAggregate {
                    commanders,
                    entries: aggregate.entries,
                    wins: aggregate.wins,
                    draws: aggregate.draws,
                    losses: aggregate.losses,
                })
            })
            .collect();
        tournaments.push(StoredTournamentObservation {
            tournament_key,
            start_date,
            swiss_rounds,
            top_cut_size,
            is_league: tournament.is_league,
            participant_count,
            identified_commander_entries,
            unidentified_entries,
            total_wins,
            total_draws,
            total_losses,
            commanders,
        });
    }
    tournaments.sort_by(|left, right| left.tournament_key.cmp(&right.tournament_key));

    let request_body = api_request_body(&scope)?;
    let mut snapshot = TopDeckEdhSnapshot {
        schema_version: TOPDECK_EDH_SCHEMA_VERSION.into(),
        ingestor_version: TOPDECK_EDH_INGESTOR_VERSION.into(),
        dataset_name: TOPDECK_EDH_DATASET_NAME.into(),
        installed_at,
        endpoint: TOPDECK_TOURNAMENTS_ENDPOINT.into(),
        query_scope: scope,
        request_fingerprint: sha256_hex(&request_body),
        source_response_sha256: sha256_hex(response_bytes),
        source_response_bytes: response_bytes.len() as u64,
        cache_fingerprint: String::new(),
        snapshot_integrity_sha256: String::new(),
        attribution_text: TOPDECK_ATTRIBUTION_TEXT.into(),
        attribution_url: TOPDECK_ATTRIBUTION_URL.into(),
        tournaments,
    };
    snapshot.cache_fingerprint = calculate_cache_fingerprint(&snapshot)?;
    snapshot.snapshot_integrity_sha256 = calculate_snapshot_integrity(&snapshot)?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn verified_commander_names(
    standing: &RawStanding,
    verified_commanders: &HashMap<String, String>,
) -> Option<Vec<String>> {
    let candidates = standing.commander_candidates.as_ref()?;
    let mut canonical = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        canonical.push(
            verified_commanders
                .get(&normalize_card_name(candidate))?
                .clone(),
        );
    }
    normalize_commander_set(canonical)
}

fn commanders_from_decklist(decklist: &str) -> Option<Vec<String>> {
    let mut in_commanders = false;
    let mut names = Vec::new();
    for raw_line in decklist.lines() {
        let line = raw_line.trim();
        if let Some(header) = decklist_header(line) {
            in_commanders = header == "commander" || header == "commanders";
            continue;
        }
        if !in_commanders || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = decklist_card_name(line).and_then(normalize_commander_name) {
            names.push(name);
            if names.len() > 2 {
                return None;
            }
        }
    }
    normalize_commander_set(names)
}

fn decklist_header(line: &str) -> Option<String> {
    let trimmed = line.trim_matches('~').trim();
    let is_marked = line.starts_with("~~") && line.ends_with("~~");
    let is_plain_known_header = matches!(
        trimmed.to_ascii_lowercase().as_str(),
        "commander"
            | "commanders"
            | "mainboard"
            | "deck"
            | "sideboard"
            | "maybeboard"
            | "companion"
    );
    (is_marked || is_plain_known_header).then(|| trimmed.to_ascii_lowercase())
}

fn decklist_card_name(line: &str) -> Option<&str> {
    let mut parts = line.splitn(2, char::is_whitespace);
    let first = parts.next()?;
    let quantity_text = first.strip_suffix('x').unwrap_or(first);
    if quantity_text
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        let quantity = quantity_text.parse::<u32>().ok()?;
        if quantity == 0 || quantity > 2 {
            return None;
        }
        parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    } else {
        Some(line)
    }
}

fn normalize_commander_name(value: &str) -> Option<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty()
        || normalized.len() > MAX_COMMANDER_NAME_BYTES
        || normalized.chars().any(char::is_control)
        || normalized.contains('@')
        || normalized.to_ascii_lowercase().contains("://")
    {
        return None;
    }
    Some(normalized.to_lowercase())
}

fn normalize_commander_set(mut names: Vec<String>) -> Option<Vec<String>> {
    names.sort();
    names.dedup();
    (matches!(names.len(), 1 | 2)).then_some(names)
}

fn required_result_count(value: Option<&Number>, label: &str) -> Result<u64, TopDeckEdhError> {
    let number = value.ok_or_else(|| {
        TopDeckEdhError::Invalid(format!(
            "TopDeck.gg omitted the requested {label} standings field."
        ))
    })?;
    let parsed = number_to_u64(number, label)?;
    if parsed > MAX_ROUNDS_PER_RESULT {
        return Err(TopDeckEdhError::Invalid(format!(
            "TopDeck.gg returned an implausible {label} count."
        )));
    }
    Ok(parsed)
}

fn number_to_i64(number: &Number, label: &str) -> Result<i64, TopDeckEdhError> {
    number
        .as_i64()
        .or_else(|| {
            number.as_f64().and_then(|value| {
                (value.is_finite()
                    && value.fract() == 0.0
                    && value >= i64::MIN as f64
                    && value <= i64::MAX as f64)
                    .then_some(value as i64)
            })
        })
        .ok_or_else(|| {
            TopDeckEdhError::Invalid(format!("TopDeck.gg returned an invalid integer {label}."))
        })
}

fn number_to_u64(number: &Number, label: &str) -> Result<u64, TopDeckEdhError> {
    number
        .as_u64()
        .or_else(|| {
            number.as_f64().and_then(|value| {
                (value.is_finite()
                    && value.fract() == 0.0
                    && value >= 0.0
                    && value <= u64::MAX as f64)
                    .then_some(value as u64)
            })
        })
        .ok_or_else(|| {
            TopDeckEdhError::Invalid(format!(
                "TopDeck.gg returned an invalid non-negative integer {label}."
            ))
        })
}

fn optional_bounded_u32(
    value: Option<&Number>,
    label: &str,
    maximum: u64,
) -> Result<Option<u32>, TopDeckEdhError> {
    value
        .map(|number| {
            let value = number_to_u64(number, label)?;
            if value > maximum || value > u32::MAX as u64 {
                return Err(TopDeckEdhError::Invalid(format!(
                    "TopDeck.gg returned an implausible {label}."
                )));
            }
            Ok(value as u32)
        })
        .transpose()
}

fn validate_returned_start_date(
    start_date: i64,
    scope: &TopDeckEdhQueryScope,
) -> Result<(), TopDeckEdhError> {
    if !(MIN_SUPPORTED_TIMESTAMP..=MAX_SUPPORTED_TIMESTAMP).contains(&start_date) {
        return Err(TopDeckEdhError::Invalid(
            "TopDeck.gg returned an out-of-range tournament startDate.".into(),
        ));
    }
    if let (Some(start), Some(end)) = (scope.start, scope.end)
        && !(start..=end).contains(&start_date)
    {
        return Err(TopDeckEdhError::Invalid(
            "TopDeck.gg returned a tournament outside the selected date range.".into(),
        ));
    }
    Ok(())
}

fn validate_returned_participants(
    participants: u64,
    scope: &TopDeckEdhQueryScope,
) -> Result<(), TopDeckEdhError> {
    if participants > MAX_PARTICIPANTS {
        return Err(TopDeckEdhError::Invalid(
            "TopDeck.gg returned an implausible participant count.".into(),
        ));
    }
    if scope
        .participant_min
        .is_some_and(|minimum| participants < minimum as u64)
        || scope
            .participant_max
            .is_some_and(|maximum| participants > maximum as u64)
    {
        return Err(TopDeckEdhError::Invalid(
            "TopDeck.gg returned a tournament outside the selected participant scope.".into(),
        ));
    }
    Ok(())
}

fn pseudonymize_tournament_id(tournament_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"topdeck-edh-tournament-v1\0");
    hasher.update(tournament_id.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheFingerprintInput<'a> {
    schema_version: &'a str,
    ingestor_version: &'a str,
    dataset_name: &'a str,
    endpoint: &'a str,
    query_scope: &'a TopDeckEdhQueryScope,
    tournaments: &'a [StoredTournamentObservation],
}

fn calculate_cache_fingerprint(snapshot: &TopDeckEdhSnapshot) -> Result<String, TopDeckEdhError> {
    let identity = CacheFingerprintInput {
        schema_version: &snapshot.schema_version,
        ingestor_version: &snapshot.ingestor_version,
        dataset_name: &snapshot.dataset_name,
        endpoint: &snapshot.endpoint,
        query_scope: &snapshot.query_scope,
        tournaments: &snapshot.tournaments,
    };
    Ok(sha256_hex(&serde_json::to_vec(&identity)?))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotIntegrityInput<'a> {
    schema_version: &'a str,
    ingestor_version: &'a str,
    dataset_name: &'a str,
    installed_at: &'a str,
    endpoint: &'a str,
    query_scope: &'a TopDeckEdhQueryScope,
    request_fingerprint: &'a str,
    source_response_sha256: &'a str,
    source_response_bytes: u64,
    cache_fingerprint: &'a str,
    attribution_text: &'a str,
    attribution_url: &'a str,
    tournaments: &'a [StoredTournamentObservation],
}

fn calculate_snapshot_integrity(snapshot: &TopDeckEdhSnapshot) -> Result<String, TopDeckEdhError> {
    let identity = SnapshotIntegrityInput {
        schema_version: &snapshot.schema_version,
        ingestor_version: &snapshot.ingestor_version,
        dataset_name: &snapshot.dataset_name,
        installed_at: &snapshot.installed_at,
        endpoint: &snapshot.endpoint,
        query_scope: &snapshot.query_scope,
        request_fingerprint: &snapshot.request_fingerprint,
        source_response_sha256: &snapshot.source_response_sha256,
        source_response_bytes: snapshot.source_response_bytes,
        cache_fingerprint: &snapshot.cache_fingerprint,
        attribution_text: &snapshot.attribution_text,
        attribution_url: &snapshot.attribution_url,
        tournaments: &snapshot.tournaments,
    };
    Ok(sha256_hex(&serde_json::to_vec(&identity)?))
}

fn load_snapshot_file(path: &Path) -> Result<TopDeckEdhSnapshot, TopDeckEdhError> {
    if fs::metadata(path)?.len() > MAX_SNAPSHOT_BYTES {
        return Err(TopDeckEdhError::Invalid(
            "The local TopDeck EDH snapshot exceeded the 32 MiB safety limit.".into(),
        ));
    }
    let bytes = fs::read(path)?;
    let snapshot: TopDeckEdhSnapshot = serde_json::from_slice(&bytes)?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn validate_snapshot(snapshot: &TopDeckEdhSnapshot) -> Result<(), TopDeckEdhError> {
    if snapshot.schema_version != TOPDECK_EDH_SCHEMA_VERSION
        || snapshot.ingestor_version != TOPDECK_EDH_INGESTOR_VERSION
        || snapshot.dataset_name != TOPDECK_EDH_DATASET_NAME
        || snapshot.endpoint != TOPDECK_TOURNAMENTS_ENDPOINT
        || snapshot.attribution_text != TOPDECK_ATTRIBUTION_TEXT
        || snapshot.attribution_url != TOPDECK_ATTRIBUTION_URL
    {
        return Err(TopDeckEdhError::Invalid(
            "The local TopDeck EDH snapshot provenance or schema was incompatible.".into(),
        ));
    }
    validate_endpoint(
        &Url::parse(&snapshot.endpoint)
            .map_err(|error| TopDeckEdhError::Invalid(error.to_string()))?,
    )?;
    validate_scope(&snapshot.query_scope)?;
    DateTime::parse_from_rfc3339(&snapshot.installed_at).map_err(|_| {
        TopDeckEdhError::Invalid("The local TopDeck EDH install timestamp was invalid.".into())
    })?;
    validate_sha256(&snapshot.request_fingerprint, "TopDeck request fingerprint")?;
    validate_sha256(
        &snapshot.source_response_sha256,
        "TopDeck source-response SHA-256",
    )?;
    validate_sha256(&snapshot.cache_fingerprint, "TopDeck cache fingerprint")?;
    validate_sha256(
        &snapshot.snapshot_integrity_sha256,
        "TopDeck snapshot-integrity SHA-256",
    )?;
    if snapshot.source_response_bytes > MAX_RESPONSE_BYTES as u64 {
        return Err(TopDeckEdhError::Invalid(
            "The local TopDeck EDH response byte count exceeded the configured limit.".into(),
        ));
    }
    let expected_request = sha256_hex(&api_request_body(&snapshot.query_scope)?);
    if snapshot.request_fingerprint != expected_request {
        return Err(TopDeckEdhError::Invalid(
            "The local TopDeck EDH query scope did not match its request fingerprint.".into(),
        ));
    }
    if snapshot.tournaments.len() > MAX_TOURNAMENTS {
        return Err(TopDeckEdhError::Invalid(
            "The local TopDeck EDH snapshot contained too many tournaments.".into(),
        ));
    }

    let mut prior_key: Option<&str> = None;
    let mut standing_count = 0u64;
    for tournament in &snapshot.tournaments {
        validate_sha256(&tournament.tournament_key, "pseudonymized tournament key")?;
        if prior_key.is_some_and(|prior| prior >= tournament.tournament_key.as_str()) {
            return Err(TopDeckEdhError::Invalid(
                "The local TopDeck EDH tournaments were duplicated or not canonicalized.".into(),
            ));
        }
        prior_key = Some(&tournament.tournament_key);
        validate_returned_start_date(tournament.start_date, &snapshot.query_scope)?;
        validate_returned_participants(tournament.participant_count, &snapshot.query_scope)?;
        if tournament.is_league && !snapshot.query_scope.include_leagues {
            return Err(TopDeckEdhError::Invalid(
                "The local TopDeck EDH snapshot contained an out-of-scope league.".into(),
            ));
        }
        if tournament
            .swiss_rounds
            .is_some_and(|value| value as u64 > MAX_ROUNDS_PER_RESULT)
            || tournament
                .top_cut_size
                .is_some_and(|value| value as u64 > MAX_PARTICIPANTS)
        {
            return Err(TopDeckEdhError::Invalid(
                "The local TopDeck EDH tournament metadata exceeded safety bounds.".into(),
            ));
        }
        standing_count = checked_add(
            standing_count,
            tournament.participant_count,
            "standing count",
        )?;
        if standing_count > MAX_STANDINGS {
            return Err(TopDeckEdhError::Invalid(
                "The local TopDeck EDH snapshot contained too many standings.".into(),
            ));
        }
        let partition_count = checked_add(
            tournament.identified_commander_entries,
            tournament.unidentified_entries,
            "participant partition",
        )?;
        if partition_count != tournament.participant_count {
            return Err(TopDeckEdhError::Invalid(
                "The local TopDeck EDH identified/unidentified counts were inconsistent.".into(),
            ));
        }

        let mut prior_commanders: Option<&[String]> = None;
        let mut identified = 0u64;
        let mut identified_wins = 0u64;
        let mut identified_draws = 0u64;
        let mut identified_losses = 0u64;
        for aggregate in &tournament.commanders {
            if prior_commanders.is_some_and(|prior| prior >= aggregate.commanders.as_slice()) {
                return Err(TopDeckEdhError::Invalid(
                    "The local TopDeck EDH commander aggregates were duplicated or not canonicalized."
                        .into(),
                ));
            }
            prior_commanders = Some(&aggregate.commanders);
            if aggregate.entries == 0
                || !matches!(aggregate.commanders.len(), 1 | 2)
                || aggregate
                    .commanders
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || aggregate
                    .commanders
                    .iter()
                    .any(|name| normalize_commander_name(name).as_deref() != Some(name.as_str()))
            {
                return Err(TopDeckEdhError::Invalid(
                    "The local TopDeck EDH commander aggregate was invalid.".into(),
                ));
            }
            identified = checked_add(identified, aggregate.entries, "commander entries")?;
            identified_wins = checked_add(identified_wins, aggregate.wins, "commander wins")?;
            identified_draws = checked_add(identified_draws, aggregate.draws, "commander draws")?;
            identified_losses =
                checked_add(identified_losses, aggregate.losses, "commander losses")?;
        }
        if identified != tournament.identified_commander_entries
            || identified_wins > tournament.total_wins
            || identified_draws > tournament.total_draws
            || identified_losses > tournament.total_losses
        {
            return Err(TopDeckEdhError::Invalid(
                "The local TopDeck EDH commander totals were inconsistent.".into(),
            ));
        }
    }
    if calculate_cache_fingerprint(snapshot)? != snapshot.cache_fingerprint {
        return Err(TopDeckEdhError::Invalid(
            "The local TopDeck EDH cache fingerprint did not match its sanitized observations."
                .into(),
        ));
    }
    if calculate_snapshot_integrity(snapshot)? != snapshot.snapshot_integrity_sha256 {
        return Err(TopDeckEdhError::Invalid(
            "The local TopDeck EDH snapshot integrity hash did not match its complete persisted record."
                .into(),
        ));
    }
    Ok(())
}

fn validate_sha256(value: &str, label: &str) -> Result<(), TopDeckEdhError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(TopDeckEdhError::Invalid(format!(
            "The local {label} was invalid."
        )));
    }
    Ok(())
}

fn checked_add(left: u64, right: u64, label: &str) -> Result<u64, TopDeckEdhError> {
    left.checked_add(right)
        .ok_or_else(|| TopDeckEdhError::Invalid(format!("The TopDeck EDH {label} overflowed.")))
}

fn status_from_snapshot(snapshot: &TopDeckEdhSnapshot) -> TopDeckEdhStatus {
    let tournament_count = snapshot.tournaments.len() as u64;
    let standing_count = snapshot
        .tournaments
        .iter()
        .map(|tournament| tournament.participant_count)
        .sum();
    let identified_commander_entries = snapshot
        .tournaments
        .iter()
        .map(|tournament| tournament.identified_commander_entries)
        .sum();
    let unidentified_entries = snapshot
        .tournaments
        .iter()
        .map(|tournament| tournament.unidentified_entries)
        .sum();
    let commander_archetype_count = snapshot
        .tournaments
        .iter()
        .flat_map(|tournament| {
            tournament
                .commanders
                .iter()
                .map(|aggregate| aggregate.commanders.clone())
        })
        .collect::<BTreeSet<_>>()
        .len() as u64;
    TopDeckEdhStatus {
        ready: true,
        schema_version: snapshot.schema_version.clone(),
        ingestor_version: snapshot.ingestor_version.clone(),
        dataset_name: snapshot.dataset_name.clone(),
        installed_at: Some(snapshot.installed_at.clone()),
        endpoint: snapshot.endpoint.clone(),
        query_scope: Some(snapshot.query_scope.clone()),
        request_fingerprint: Some(snapshot.request_fingerprint.clone()),
        source_response_sha256: Some(snapshot.source_response_sha256.clone()),
        source_response_bytes: Some(snapshot.source_response_bytes),
        cache_fingerprint: Some(snapshot.cache_fingerprint.clone()),
        snapshot_integrity_sha256: Some(snapshot.snapshot_integrity_sha256.clone()),
        tournament_count,
        standing_count,
        identified_commander_entries,
        unidentified_entries,
        commander_archetype_count,
        attribution_text: snapshot.attribution_text.clone(),
        attribution_url: snapshot.attribution_url.clone(),
        authenticity_basis:
            "Fetched from the exact allowlisted TopDeck.gg HTTPS endpoint with a user-supplied per-update API key; the response is locally SHA-256 hashed. No publisher signature is verified."
                .into(),
        privacy_summary:
            "Only pseudonymized tournament keys and locally Oracle-validated commander-level aggregates are stored. API keys, raw responses, tournament names and locations, player identity fields, unknown deck text, and raw decklists are not retained."
                .into(),
        message: format!(
            "{} are installed as optional research context for the selected query scope. Current ratings do not consume them, and EDH observations are not assumed to be cEDH events.",
            TOPDECK_EDH_DATASET_NAME
        ),
    }
}

fn empty_status() -> TopDeckEdhStatus {
    TopDeckEdhStatus {
        ready: false,
        schema_version: TOPDECK_EDH_SCHEMA_VERSION.into(),
        ingestor_version: TOPDECK_EDH_INGESTOR_VERSION.into(),
        dataset_name: TOPDECK_EDH_DATASET_NAME.into(),
        installed_at: None,
        endpoint: TOPDECK_TOURNAMENTS_ENDPOINT.into(),
        query_scope: None,
        request_fingerprint: None,
        source_response_sha256: None,
        source_response_bytes: None,
        cache_fingerprint: None,
        snapshot_integrity_sha256: None,
        tournament_count: 0,
        standing_count: 0,
        identified_commander_entries: 0,
        unidentified_entries: 0,
        commander_archetype_count: 0,
        attribution_text: TOPDECK_ATTRIBUTION_TEXT.into(),
        attribution_url: TOPDECK_ATTRIBUTION_URL.into(),
        authenticity_basis:
            "Not installed. No TopDeck.gg response has been downloaded or retained.".into(),
        privacy_summary:
            "Updates store only pseudonymized tournament keys and commander-level aggregates. API keys are required per update and are not persisted."
                .into(),
        message: format!(
            "No {} snapshot is installed. An explicit update requires a TopDeck.gg API key.",
            TOPDECK_EDH_DATASET_NAME
        ),
    }
}

fn emit(
    reporter: &Option<TopDeckEdhUpdateReporter>,
    phase: &str,
    completed_units: u64,
    total_units: Option<u64>,
    progress: f32,
    detail: &str,
) {
    if let Some(reporter) = reporter {
        reporter(TopDeckEdhUpdateProgress {
            phase: phase.into(),
            completed_units,
            total_units,
            progress,
            detail: detail.into(),
        });
    }
}
