//! Version-pinned, process-isolated protocol boundary for a future phase-rs
//! execution worker.
//!
//! This module deliberately contains no direct phase-rs dependency and does
//! not launch a process. A separately built and installed engine pack will
//! communicate over private NDJSON stdin/stdout. Every message is strict,
//! bounded, request-correlated, and bound to the verified pack identity.
//! Unknown fields and partial outcomes fail closed.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

use crate::execution_coverage::{
    EXECUTION_COVERAGE_COMPILER_VERSION, EXECUTION_COVERAGE_SCHEMA_VERSION,
};

pub const PHASE_WORKER_PROTOCOL_VERSION: &str = "commander-phase-worker-protocol/v1";
pub const PHASE_WORKER_CLIENT_VERSION: &str = "phase-worker-host-0.1";
pub const PHASE_ENGINE_NAME: &str = "phase-rs";
pub const PHASE_SOURCE_REPOSITORY: &str = "https://github.com/phase-rs/phase";

pub const MAXIMUM_NDJSON_LINE_BYTES: usize = 1024 * 1024;
pub const MAXIMUM_DECK_ENTRIES: usize = 500;
pub const MAXIMUM_DECK_CARDS: u32 = 500;
pub const MAXIMUM_BATCH_EPISODES: u32 = 1_000;
pub const MAXIMUM_COVERAGE_GAPS: usize = 4_096;
pub const MAXIMUM_EFFECT_LEAVES: u32 = 250_000;
pub const MAXIMUM_REQUEST_ID_BYTES: usize = 128;
const MAXIMUM_NAME_BYTES: usize = 256;
const MAXIMUM_DETAIL_BYTES: usize = 1_024;
const MAXIMUM_VERSION_BYTES: usize = 128;
const MAXIMUM_CAPABILITIES: usize = 16;
const MAXIMUM_DATA_NOTICES: usize = 8;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PhaseWorkerProtocolError {
    #[error("Worker protocol message exceeds the {MAXIMUM_NDJSON_LINE_BYTES}-byte limit.")]
    LineTooLong,
    #[error("Worker protocol message is not valid JSON: {0}")]
    Json(String),
    #[error("Worker protocol message is invalid: {0}")]
    Invalid(String),
    #[error("Worker protocol version is unsupported: {0}")]
    UnsupportedProtocol(String),
    #[error("Worker identity does not match the verified engine pack: {0}")]
    IdentityMismatch(String),
    #[error("Worker request correlation failed: {0}")]
    Correlation(String),
    #[error("Worker returned incomplete or unsupported execution: {0}")]
    NotStrict(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireEnvelope {
    #[serde(rename = "type")]
    message_type: String,
    protocol_version: String,
    request_id: String,
    payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerProvenance {
    pub protocol_version: String,
    pub pack_version: String,
    pub pack_content_sha256: String,
    pub manifest_sha256: String,
    pub engine_name: String,
    pub engine_version: String,
    pub engine_source_repository: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerDataSourceProvenance {
    pub name: String,
    pub source_url: String,
    pub version: String,
    pub revision: String,
    pub source_artifact_sha256: String,
    pub content_sha256: String,
    pub license_expression: String,
    pub attribution: String,
    pub notice_sha256s: Vec<String>,
}

impl WorkerDataSourceProvenance {
    pub fn validate(&self, field: &str) -> Result<(), PhaseWorkerProtocolError> {
        validate_bounded_text(&format!("{field}.name"), &self.name, MAXIMUM_NAME_BYTES)?;
        validate_https_source_url(&format!("{field}.sourceUrl"), &self.source_url)?;
        validate_version(&format!("{field}.version"), &self.version)?;
        validate_version(&format!("{field}.revision"), &self.revision)?;
        validate_sha256(
            &format!("{field}.sourceArtifactSha256"),
            &self.source_artifact_sha256,
        )?;
        validate_sha256(&format!("{field}.contentSha256"), &self.content_sha256)?;
        validate_bounded_text(
            &format!("{field}.licenseExpression"),
            &self.license_expression,
            MAXIMUM_NAME_BYTES,
        )?;
        validate_bounded_text(
            &format!("{field}.attribution"),
            &self.attribution,
            MAXIMUM_DETAIL_BYTES,
        )?;
        if self.notice_sha256s.is_empty() || self.notice_sha256s.len() > MAXIMUM_DATA_NOTICES {
            return Err(PhaseWorkerProtocolError::Invalid(format!(
                "{field}.noticeSha256s must contain 1 through {MAXIMUM_DATA_NOTICES} notice or license hashes"
            )));
        }
        let mut notices = HashSet::new();
        for hash in &self.notice_sha256s {
            validate_sha256(&format!("{field}.noticeSha256s"), hash)?;
            if !notices.insert(hash.as_str()) {
                return Err(PhaseWorkerProtocolError::Invalid(format!(
                    "{field}.noticeSha256s contains a duplicate"
                )));
            }
        }
        Ok(())
    }
}

impl WorkerProvenance {
    pub fn validate(&self) -> Result<(), PhaseWorkerProtocolError> {
        if self.protocol_version != PHASE_WORKER_PROTOCOL_VERSION {
            return Err(PhaseWorkerProtocolError::UnsupportedProtocol(
                self.protocol_version.clone(),
            ));
        }
        if self.engine_name != PHASE_ENGINE_NAME {
            return Err(PhaseWorkerProtocolError::Invalid(format!(
                "engineName must be {PHASE_ENGINE_NAME}"
            )));
        }
        if self.engine_source_repository != PHASE_SOURCE_REPOSITORY {
            return Err(PhaseWorkerProtocolError::Invalid(format!(
                "engineSourceRepository must be {PHASE_SOURCE_REPOSITORY}"
            )));
        }
        validate_version("packVersion", &self.pack_version)?;
        validate_version("engineVersion", &self.engine_version)?;
        if !is_lowercase_git_revision(&self.engine_source_revision) {
            return Err(PhaseWorkerProtocolError::Invalid(
                "engineSourceRevision must be a full 40-character lowercase Git revision".into(),
            ));
        }
        for (field, hash) in [
            ("packContentSha256", &self.pack_content_sha256),
            ("manifestSha256", &self.manifest_sha256),
            ("engineSourceSha256", &self.engine_source_sha256),
            ("workerExecutableSha256", &self.worker_executable_sha256),
            ("cardDataSha256", &self.card_data_sha256),
            ("rulesDataSha256", &self.rules_data_sha256),
        ] {
            validate_sha256(field, hash)?;
        }
        self.card_data_source.validate("cardDataSource")?;
        self.rules_data_source.validate("rulesDataSource")?;
        if self.card_data_source.content_sha256 != self.card_data_sha256
            || self.rules_data_source.content_sha256 != self.rules_data_sha256
        {
            return Err(PhaseWorkerProtocolError::Invalid(
                "worker data-source content hashes must match the top-level card/rules data hashes"
                    .into(),
            ));
        }
        if self.host_execution_coverage_schema != EXECUTION_COVERAGE_SCHEMA_VERSION
            || self.host_execution_coverage_compiler != EXECUTION_COVERAGE_COMPILER_VERSION
        {
            return Err(PhaseWorkerProtocolError::Invalid(format!(
                "worker pack must pin host execution coverage {EXECUTION_COVERAGE_SCHEMA_VERSION} / {EXECUTION_COVERAGE_COMPILER_VERSION}"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum WorkerCapability {
    StrictPreflightDeck,
    DeterministicRunBatch,
    CooperativeCancellation,
    MonotonicProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerRuntimeLimits {
    pub maximum_ndjson_line_bytes: u32,
    pub maximum_deck_entries: u32,
    pub maximum_deck_cards: u32,
    pub maximum_batch_episodes: u32,
    pub maximum_turns: u16,
    pub maximum_coverage_gaps: u32,
}

impl WorkerRuntimeLimits {
    pub fn host_limits() -> Self {
        Self {
            maximum_ndjson_line_bytes: MAXIMUM_NDJSON_LINE_BYTES as u32,
            maximum_deck_entries: MAXIMUM_DECK_ENTRIES as u32,
            maximum_deck_cards: MAXIMUM_DECK_CARDS,
            maximum_batch_episodes: MAXIMUM_BATCH_EPISODES,
            maximum_turns: 100,
            maximum_coverage_gaps: MAXIMUM_COVERAGE_GAPS as u32,
        }
    }

    fn validate_worker_limits(&self) -> Result<(), PhaseWorkerProtocolError> {
        if self.maximum_ndjson_line_bytes == 0
            || self.maximum_ndjson_line_bytes > MAXIMUM_NDJSON_LINE_BYTES as u32
            || self.maximum_deck_entries == 0
            || self.maximum_deck_entries > MAXIMUM_DECK_ENTRIES as u32
            || self.maximum_deck_cards == 0
            || self.maximum_deck_cards > MAXIMUM_DECK_CARDS
            || self.maximum_batch_episodes == 0
            || self.maximum_batch_episodes > MAXIMUM_BATCH_EPISODES
            || self.maximum_turns == 0
            || self.maximum_turns > 100
            || self.maximum_coverage_gaps == 0
            || self.maximum_coverage_gaps > MAXIMUM_COVERAGE_GAPS as u32
        {
            return Err(PhaseWorkerProtocolError::Invalid(
                "worker limits must be positive and no larger than the host's hard limits".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HelloRequest {
    pub client_version: String,
    pub nonce: String,
    pub expected_provenance: WorkerProvenance,
    pub hard_limits: WorkerRuntimeLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HelloOutcome {
    pub provenance: WorkerProvenance,
    pub nonce: String,
    pub capabilities: Vec<WorkerCapability>,
    pub limits: WorkerRuntimeLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerDeck {
    pub canonical_deck_sha256: String,
    pub card_snapshot_sha256: String,
    pub cards: Vec<WorkerDeckCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerDeckCard {
    pub oracle_id: String,
    pub canonical_name: String,
    pub card_record_sha256: String,
    pub quantity: u16,
    pub command_zone: bool,
}

impl WorkerDeck {
    pub fn validate(&self) -> Result<(), PhaseWorkerProtocolError> {
        validate_sha256("canonicalDeckSha256", &self.canonical_deck_sha256)?;
        validate_sha256("cardSnapshotSha256", &self.card_snapshot_sha256)?;
        if self.cards.is_empty() || self.cards.len() > MAXIMUM_DECK_ENTRIES {
            return Err(PhaseWorkerProtocolError::Invalid(format!(
                "cards must contain 1 to {MAXIMUM_DECK_ENTRIES} entries"
            )));
        }
        let mut identities = HashSet::new();
        let mut total = 0_u32;
        let mut commanders = 0_u32;
        for card in &self.cards {
            if !is_canonical_uuid(&card.oracle_id) {
                return Err(PhaseWorkerProtocolError::Invalid(format!(
                    "oracleId {} must be a lowercase canonical UUID",
                    card.oracle_id
                )));
            }
            validate_bounded_text("canonicalName", &card.canonical_name, MAXIMUM_NAME_BYTES)?;
            validate_sha256("cardRecordSha256", &card.card_record_sha256)?;
            if card.quantity == 0 {
                return Err(PhaseWorkerProtocolError::Invalid(
                    "card quantities must be positive".into(),
                ));
            }
            if !identities.insert(card.oracle_id.as_str()) {
                return Err(PhaseWorkerProtocolError::Invalid(format!(
                    "duplicate oracleId {}",
                    card.oracle_id
                )));
            }
            total = total.checked_add(u32::from(card.quantity)).ok_or_else(|| {
                PhaseWorkerProtocolError::Invalid("deck quantity overflow".into())
            })?;
            if card.command_zone {
                if card.quantity != 1 {
                    return Err(PhaseWorkerProtocolError::Invalid(
                        "each command-zone entry must have quantity one".into(),
                    ));
                }
                commanders = commanders.checked_add(1).ok_or_else(|| {
                    PhaseWorkerProtocolError::Invalid("command-zone quantity overflow".into())
                })?;
            }
        }
        if total == 0 || total > MAXIMUM_DECK_CARDS {
            return Err(PhaseWorkerProtocolError::Invalid(format!(
                "deck must contain at most {MAXIMUM_DECK_CARDS} cards"
            )));
        }
        if commanders == 0 || commanders > 2 {
            return Err(PhaseWorkerProtocolError::Invalid(
                "strict Commander preflight requires one or two command-zone cards".into(),
            ));
        }
        Ok(())
    }

    pub fn total_cards(&self) -> u32 {
        self.cards.iter().map(|card| u32::from(card.quantity)).sum()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PreflightScope {
    FullCommanderDeckExecution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreflightDeckRequest {
    pub expected_provenance: WorkerProvenance,
    pub scope: PreflightScope,
    pub strict: bool,
    pub host_execution_coverage_manifest_sha256: String,
    pub deck: WorkerDeck,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CoverageTotals {
    pub cards_total: u32,
    pub unique_cards_total: u32,
    pub faces_total: u32,
    pub abilities_total: u32,
    pub effect_leaves_total: u32,
    pub executable_effect_leaves: u32,
    pub unsupported_effect_leaves: u32,
    pub unresolved_effect_leaves: u32,
    pub ambiguous_effect_leaves: u32,
    pub total_gap_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardCoverageTotals {
    pub oracle_id: String,
    pub card_record_sha256: String,
    pub coverage_tree_sha256: String,
    pub faces_total: u32,
    pub abilities_total: u32,
    pub effect_leaves_total: u32,
    pub executable_effect_leaves: u32,
    pub unsupported_effect_leaves: u32,
    pub unresolved_effect_leaves: u32,
    pub ambiguous_effect_leaves: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CoverageGapKind {
    MissingCard,
    CardDataMismatch,
    UnsupportedLayout,
    UnsupportedAbility,
    UnsupportedCost,
    UnsupportedTiming,
    UnsupportedChoice,
    UnsupportedTarget,
    UnsupportedZone,
    UnsupportedCondition,
    UnsupportedEffect,
    UnsupportedRuleDependency,
    AmbiguousOracleBinding,
    WorkerDataMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CoverageGap {
    pub gap_id: String,
    pub oracle_id: String,
    pub face_index: u16,
    pub ability_index: u16,
    pub effect_leaf_index: u16,
    pub kind: CoverageGapKind,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreflightDeckOutcome {
    pub provenance: WorkerProvenance,
    pub canonical_deck_sha256: String,
    pub card_snapshot_sha256: String,
    pub host_execution_coverage_manifest_sha256: String,
    pub scope: PreflightScope,
    pub totals: CoverageTotals,
    pub card_coverage: Vec<CardCoverageTotals>,
    pub gaps: Vec<CoverageGap>,
    pub gaps_truncated: bool,
    pub complete: bool,
    pub preflight_sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum WorkerScenario {
    Goldfish,
    TargetedPermanentRemoval,
    CommanderRemoveAndRecast,
    CounterFirstRelevantSpell,
    CreatureBoardWipe,
    GraveyardShutdown,
    GenericTax,
    RuleOfLaw,
    FirstWinAttemptStopped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunBatchRequest {
    pub expected_provenance: WorkerProvenance,
    pub canonical_deck_sha256: String,
    pub preflight_sha256: String,
    pub scenario: WorkerScenario,
    pub scenario_input_sha256: String,
    pub seed_exact: String,
    pub episode_start: u32,
    pub episode_count: u32,
    pub maximum_turns: u16,
    pub request_sha256: String,
}

impl RunBatchRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        expected_provenance: WorkerProvenance,
        canonical_deck_sha256: String,
        preflight_sha256: String,
        scenario: WorkerScenario,
        scenario_input_sha256: String,
        seed_exact: String,
        episode_start: u32,
        episode_count: u32,
        maximum_turns: u16,
    ) -> Result<Self, PhaseWorkerProtocolError> {
        let mut request = Self {
            expected_provenance,
            canonical_deck_sha256,
            preflight_sha256,
            scenario,
            scenario_input_sha256,
            seed_exact,
            episode_start,
            episode_count,
            maximum_turns,
            request_sha256: String::new(),
        };
        request.validate_without_fingerprint()?;
        request.request_sha256 = request_fingerprint(&request)?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), PhaseWorkerProtocolError> {
        self.validate_without_fingerprint()?;
        validate_sha256("requestSha256", &self.request_sha256)?;
        let expected = request_fingerprint(self)?;
        if self.request_sha256 != expected {
            return Err(PhaseWorkerProtocolError::Invalid(
                "requestSha256 does not match the exact deterministic batch request".into(),
            ));
        }
        Ok(())
    }

    fn validate_without_fingerprint(&self) -> Result<(), PhaseWorkerProtocolError> {
        self.expected_provenance.validate()?;
        validate_sha256("canonicalDeckSha256", &self.canonical_deck_sha256)?;
        validate_sha256("preflightSha256", &self.preflight_sha256)?;
        validate_sha256("scenarioInputSha256", &self.scenario_input_sha256)?;
        validate_exact_u64("seedExact", &self.seed_exact)?;
        if self.episode_count == 0 || self.episode_count > MAXIMUM_BATCH_EPISODES {
            return Err(PhaseWorkerProtocolError::Invalid(format!(
                "episodeCount must be 1 through {MAXIMUM_BATCH_EPISODES}"
            )));
        }
        self.episode_start
            .checked_add(self.episode_count)
            .ok_or_else(|| {
                PhaseWorkerProtocolError::Invalid("episode index range overflows u32".into())
            })?;
        if self.maximum_turns == 0 || self.maximum_turns > 100 {
            return Err(PhaseWorkerProtocolError::Invalid(
                "maximumTurns must be 1 through 100".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EpisodeTerminal {
    Win,
    TurnLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerEpisodeOutcome {
    pub episode_index: u32,
    pub terminal: EpisodeTerminal,
    pub terminal_turn: Option<u16>,
    pub opening_hand_size: u8,
    pub mulligans: u8,
    pub lands_played: u16,
    pub spells_cast: u16,
    pub commander_casts: u16,
    pub deterministic_trace_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunBatchOutcome {
    pub provenance: WorkerProvenance,
    pub canonical_deck_sha256: String,
    pub preflight_sha256: String,
    pub request_sha256: String,
    pub seed_exact: String,
    pub scenario: WorkerScenario,
    pub completed_episodes: u32,
    pub episodes: Vec<WorkerEpisodeOutcome>,
    pub outcome_sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProgressStage {
    Starting,
    Preflight,
    Simulating,
    Finalizing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerProgress {
    pub provenance: WorkerProvenance,
    pub stage: ProgressStage,
    pub completed_units: u32,
    pub total_units: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelRequest {
    pub expected_provenance: WorkerProvenance,
    pub target_request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelOutcome {
    pub provenance: WorkerProvenance,
    pub target_request_id: String,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkerErrorCode {
    InvalidRequest,
    UnsupportedCardFunction,
    UnsupportedScenario,
    Cancelled,
    EnginePanic,
    InternalFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerErrorOutcome {
    pub provenance: WorkerProvenance,
    pub code: WorkerErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientMessage {
    Hello(HelloRequest),
    PreflightDeck(PreflightDeckRequest),
    RunBatch(RunBatchRequest),
    Cancel(CancelRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerMessage {
    Hello(HelloOutcome),
    PreflightDeck(PreflightDeckOutcome),
    Progress(WorkerProgress),
    RunBatch(RunBatchOutcome),
    Cancel(CancelOutcome),
    Error(WorkerErrorOutcome),
}

pub fn encode_client_message(
    request_id: &str,
    message: &ClientMessage,
) -> Result<Vec<u8>, PhaseWorkerProtocolError> {
    validate_request_id(request_id)?;
    validate_client_message(message)?;
    let (message_type, payload) = match message {
        ClientMessage::Hello(payload) => ("hello", to_value(payload)?),
        ClientMessage::PreflightDeck(payload) => ("preflight_deck", to_value(payload)?),
        ClientMessage::RunBatch(payload) => ("run_batch", to_value(payload)?),
        ClientMessage::Cancel(payload) => ("cancel", to_value(payload)?),
    };
    encode_envelope(request_id, message_type, payload)
}

pub fn decode_client_line(
    line: &[u8],
) -> Result<(String, ClientMessage), PhaseWorkerProtocolError> {
    let envelope = decode_envelope(line)?;
    let message = match envelope.message_type.as_str() {
        "hello" => ClientMessage::Hello(from_value(envelope.payload)?),
        "preflight_deck" => ClientMessage::PreflightDeck(from_value(envelope.payload)?),
        "run_batch" => ClientMessage::RunBatch(from_value(envelope.payload)?),
        "cancel" => ClientMessage::Cancel(from_value(envelope.payload)?),
        other => {
            return Err(PhaseWorkerProtocolError::Invalid(format!(
                "unknown client message type {other}"
            )));
        }
    };
    validate_client_message(&message)?;
    Ok((envelope.request_id, message))
}

pub fn encode_server_message(
    request_id: &str,
    message: &ServerMessage,
) -> Result<Vec<u8>, PhaseWorkerProtocolError> {
    validate_request_id(request_id)?;
    validate_server_message(message)?;
    let (message_type, payload) = match message {
        ServerMessage::Hello(payload) => ("hello_outcome", to_value(payload)?),
        ServerMessage::PreflightDeck(payload) => ("preflight_deck_outcome", to_value(payload)?),
        ServerMessage::Progress(payload) => ("progress", to_value(payload)?),
        ServerMessage::RunBatch(payload) => ("run_batch_outcome", to_value(payload)?),
        ServerMessage::Cancel(payload) => ("cancel_outcome", to_value(payload)?),
        ServerMessage::Error(payload) => ("error", to_value(payload)?),
    };
    encode_envelope(request_id, message_type, payload)
}

pub fn decode_server_line(
    line: &[u8],
) -> Result<(String, ServerMessage), PhaseWorkerProtocolError> {
    let envelope = decode_envelope(line)?;
    let message = match envelope.message_type.as_str() {
        "hello_outcome" => ServerMessage::Hello(from_value(envelope.payload)?),
        "preflight_deck_outcome" => ServerMessage::PreflightDeck(from_value(envelope.payload)?),
        "progress" => ServerMessage::Progress(from_value(envelope.payload)?),
        "run_batch_outcome" => ServerMessage::RunBatch(from_value(envelope.payload)?),
        "cancel_outcome" => ServerMessage::Cancel(from_value(envelope.payload)?),
        "error" => ServerMessage::Error(from_value(envelope.payload)?),
        other => {
            return Err(PhaseWorkerProtocolError::Invalid(format!(
                "unknown server message type {other}"
            )));
        }
    };
    validate_server_message(&message)?;
    Ok((envelope.request_id, message))
}

fn encode_envelope(
    request_id: &str,
    message_type: &str,
    payload: Value,
) -> Result<Vec<u8>, PhaseWorkerProtocolError> {
    let mut bytes = serde_json::to_vec(&WireEnvelope {
        message_type: message_type.into(),
        protocol_version: PHASE_WORKER_PROTOCOL_VERSION.into(),
        request_id: request_id.into(),
        payload,
    })
    .map_err(|error| PhaseWorkerProtocolError::Json(error.to_string()))?;
    if bytes.len() + 1 > MAXIMUM_NDJSON_LINE_BYTES {
        return Err(PhaseWorkerProtocolError::LineTooLong);
    }
    bytes.push(b'\n');
    Ok(bytes)
}

fn decode_envelope(line: &[u8]) -> Result<WireEnvelope, PhaseWorkerProtocolError> {
    if line.len() > MAXIMUM_NDJSON_LINE_BYTES {
        return Err(PhaseWorkerProtocolError::LineTooLong);
    }
    if line.iter().any(|byte| *byte == b'\n' || *byte == b'\r') {
        let trimmed = line.strip_suffix(b"\n").unwrap_or(line);
        if trimmed.iter().any(|byte| *byte == b'\n' || *byte == b'\r') {
            return Err(PhaseWorkerProtocolError::Invalid(
                "one decode call must contain exactly one NDJSON record".into(),
            ));
        }
    }
    let trimmed = line.strip_suffix(b"\n").unwrap_or(line);
    if trimmed.is_empty() {
        return Err(PhaseWorkerProtocolError::Invalid(
            "empty worker protocol record".into(),
        ));
    }
    let envelope: WireEnvelope = serde_json::from_slice(trimmed)
        .map_err(|error| PhaseWorkerProtocolError::Json(error.to_string()))?;
    if envelope.protocol_version != PHASE_WORKER_PROTOCOL_VERSION {
        return Err(PhaseWorkerProtocolError::UnsupportedProtocol(
            envelope.protocol_version,
        ));
    }
    validate_request_id(&envelope.request_id)?;
    Ok(envelope)
}

fn validate_client_message(message: &ClientMessage) -> Result<(), PhaseWorkerProtocolError> {
    match message {
        ClientMessage::Hello(request) => {
            validate_version("clientVersion", &request.client_version)?;
            validate_nonce(&request.nonce)?;
            request.expected_provenance.validate()?;
            if request.hard_limits != WorkerRuntimeLimits::host_limits() {
                return Err(PhaseWorkerProtocolError::Invalid(
                    "hello hardLimits must exactly match this host protocol release".into(),
                ));
            }
        }
        ClientMessage::PreflightDeck(request) => {
            request.expected_provenance.validate()?;
            if !request.strict {
                return Err(PhaseWorkerProtocolError::Invalid(
                    "preflight_deck must request strict execution".into(),
                ));
            }
            validate_sha256(
                "hostExecutionCoverageManifestSha256",
                &request.host_execution_coverage_manifest_sha256,
            )?;
            request.deck.validate()?;
        }
        ClientMessage::RunBatch(request) => request.validate()?,
        ClientMessage::Cancel(request) => {
            request.expected_provenance.validate()?;
            validate_request_id(&request.target_request_id)?;
        }
    }
    Ok(())
}

fn validate_server_message(message: &ServerMessage) -> Result<(), PhaseWorkerProtocolError> {
    let provenance = match message {
        ServerMessage::Hello(outcome) => {
            validate_nonce(&outcome.nonce)?;
            if outcome.capabilities.is_empty() || outcome.capabilities.len() > MAXIMUM_CAPABILITIES
            {
                return Err(PhaseWorkerProtocolError::Invalid(
                    "hello capabilities are empty or exceed the protocol bound".into(),
                ));
            }
            let unique = outcome.capabilities.iter().copied().collect::<HashSet<_>>();
            if unique.len() != outcome.capabilities.len() {
                return Err(PhaseWorkerProtocolError::Invalid(
                    "hello capabilities contain duplicates".into(),
                ));
            }
            outcome.limits.validate_worker_limits()?;
            &outcome.provenance
        }
        ServerMessage::PreflightDeck(outcome) => {
            validate_preflight_shape(outcome)?;
            &outcome.provenance
        }
        ServerMessage::Progress(progress) => {
            if progress.total_units == 0 || progress.completed_units > progress.total_units {
                return Err(PhaseWorkerProtocolError::Invalid(
                    "progress units must be bounded and completed cannot exceed total".into(),
                ));
            }
            &progress.provenance
        }
        ServerMessage::RunBatch(outcome) => {
            validate_batch_outcome_shape(outcome)?;
            &outcome.provenance
        }
        ServerMessage::Cancel(outcome) => {
            validate_request_id(&outcome.target_request_id)?;
            &outcome.provenance
        }
        ServerMessage::Error(outcome) => {
            validate_bounded_text(
                "worker error message",
                &outcome.message,
                MAXIMUM_DETAIL_BYTES,
            )?;
            &outcome.provenance
        }
    };
    provenance.validate()
}

#[derive(Debug, Clone)]
pub struct VerifiedWorkerSession {
    provenance: WorkerProvenance,
    limits: WorkerRuntimeLimits,
}

impl VerifiedWorkerSession {
    pub fn establish(
        request_id: &str,
        response_request_id: &str,
        request: &HelloRequest,
        outcome: &HelloOutcome,
    ) -> Result<Self, PhaseWorkerProtocolError> {
        validate_request_id(request_id)?;
        require_same_request(request_id, response_request_id)?;
        validate_client_message(&ClientMessage::Hello(request.clone()))?;
        validate_server_message(&ServerMessage::Hello(outcome.clone()))?;
        require_identity(&request.expected_provenance, &outcome.provenance)?;
        if request.nonce != outcome.nonce {
            return Err(PhaseWorkerProtocolError::Correlation(
                "hello nonce was not echoed exactly".into(),
            ));
        }
        let required = [
            WorkerCapability::StrictPreflightDeck,
            WorkerCapability::DeterministicRunBatch,
            WorkerCapability::CooperativeCancellation,
            WorkerCapability::MonotonicProgress,
        ];
        for capability in required {
            if !outcome.capabilities.contains(&capability) {
                return Err(PhaseWorkerProtocolError::NotStrict(format!(
                    "worker did not advertise required capability {capability:?}"
                )));
            }
        }
        Ok(Self {
            provenance: outcome.provenance.clone(),
            limits: outcome.limits.clone(),
        })
    }

    pub fn provenance(&self) -> &WorkerProvenance {
        &self.provenance
    }

    pub fn verify_preflight(
        &self,
        request_id: &str,
        response_request_id: &str,
        request: &PreflightDeckRequest,
        outcome: &PreflightDeckOutcome,
    ) -> Result<VerifiedStrictPreflight, PhaseWorkerProtocolError> {
        require_same_request(request_id, response_request_id)?;
        validate_client_message(&ClientMessage::PreflightDeck(request.clone()))?;
        validate_server_message(&ServerMessage::PreflightDeck(outcome.clone()))?;
        require_identity(&self.provenance, &request.expected_provenance)?;
        require_identity(&self.provenance, &outcome.provenance)?;
        if request.deck.cards.len() > self.limits.maximum_deck_entries as usize
            || request.deck.total_cards() > self.limits.maximum_deck_cards
        {
            return Err(PhaseWorkerProtocolError::NotStrict(
                "deck exceeds verified worker limits".into(),
            ));
        }
        if outcome.canonical_deck_sha256 != request.deck.canonical_deck_sha256
            || outcome.card_snapshot_sha256 != request.deck.card_snapshot_sha256
            || outcome.host_execution_coverage_manifest_sha256
                != request.host_execution_coverage_manifest_sha256
            || outcome.scope != request.scope
        {
            return Err(PhaseWorkerProtocolError::Correlation(
                "preflight outcome does not match the exact deck, card snapshot, host coverage manifest, and requested scope".into(),
            ));
        }
        if outcome.totals.cards_total != request.deck.total_cards()
            || outcome.totals.unique_cards_total != request.deck.cards.len() as u32
        {
            return Err(PhaseWorkerProtocolError::Invalid(
                "preflight card totals do not conserve the submitted deck".into(),
            ));
        }
        if outcome.card_coverage.len() != request.deck.cards.len() {
            return Err(PhaseWorkerProtocolError::Invalid(
                "preflight must return one exact coverage row for every submitted card identity"
                    .into(),
            ));
        }
        let requested_cards = request
            .deck
            .cards
            .iter()
            .map(|card| (card.oracle_id.as_str(), card))
            .collect::<HashMap<_, _>>();
        for card_coverage in &outcome.card_coverage {
            let requested = requested_cards
                .get(card_coverage.oracle_id.as_str())
                .ok_or_else(|| {
                    PhaseWorkerProtocolError::Invalid(format!(
                        "preflight returned coverage for unrequested oracleId {}",
                        card_coverage.oracle_id
                    ))
                })?;
            if card_coverage.card_record_sha256 != requested.card_record_sha256 {
                return Err(PhaseWorkerProtocolError::Correlation(format!(
                    "preflight card-record hash differs for oracleId {}",
                    card_coverage.oracle_id
                )));
            }
        }
        if outcome.gaps_truncated
            || !outcome.complete
            || outcome.totals.total_gap_count != 0
            || !outcome.gaps.is_empty()
            || outcome.totals.effect_leaves_total == 0
            || outcome.totals.executable_effect_leaves != outcome.totals.effect_leaves_total
        {
            return Err(PhaseWorkerProtocolError::NotStrict(format!(
                "strict preflight has {} gap(s), truncated={}, complete={}",
                outcome.totals.total_gap_count, outcome.gaps_truncated, outcome.complete
            )));
        }
        let expected_fingerprint = preflight_fingerprint(outcome)?;
        if outcome.preflight_sha256 != expected_fingerprint {
            return Err(PhaseWorkerProtocolError::Invalid(
                "preflightSha256 does not bind the exact verified preflight outcome".into(),
            ));
        }
        Ok(VerifiedStrictPreflight {
            provenance: self.provenance.clone(),
            canonical_deck_sha256: outcome.canonical_deck_sha256.clone(),
            preflight_sha256: outcome.preflight_sha256.clone(),
        })
    }

    pub fn verify_batch(
        &self,
        request_id: &str,
        response_request_id: &str,
        preflight: &VerifiedStrictPreflight,
        request: &RunBatchRequest,
        outcome: &RunBatchOutcome,
    ) -> Result<VerifiedBatchOutcome, PhaseWorkerProtocolError> {
        require_same_request(request_id, response_request_id)?;
        request.validate()?;
        validate_server_message(&ServerMessage::RunBatch(outcome.clone()))?;
        require_identity(&self.provenance, &preflight.provenance)?;
        require_identity(&self.provenance, &request.expected_provenance)?;
        require_identity(&self.provenance, &outcome.provenance)?;
        if request.episode_count > self.limits.maximum_batch_episodes
            || request.maximum_turns > self.limits.maximum_turns
        {
            return Err(PhaseWorkerProtocolError::NotStrict(
                "run_batch exceeds verified worker limits".into(),
            ));
        }
        if request.canonical_deck_sha256 != preflight.canonical_deck_sha256
            || request.preflight_sha256 != preflight.preflight_sha256
            || outcome.canonical_deck_sha256 != request.canonical_deck_sha256
            || outcome.preflight_sha256 != request.preflight_sha256
            || outcome.request_sha256 != request.request_sha256
            || outcome.seed_exact != request.seed_exact
            || outcome.scenario != request.scenario
        {
            return Err(PhaseWorkerProtocolError::Correlation(
                "run_batch outcome is not bound to its verified preflight and exact request".into(),
            ));
        }
        if outcome.completed_episodes != request.episode_count
            || outcome.episodes.len() != request.episode_count as usize
        {
            return Err(PhaseWorkerProtocolError::NotStrict(
                "partial batch outcomes cannot be used as strict execution".into(),
            ));
        }
        for (offset, episode) in outcome.episodes.iter().enumerate() {
            let expected_index = request.episode_start + offset as u32;
            if episode.episode_index != expected_index {
                return Err(PhaseWorkerProtocolError::Invalid(
                    "episode outcomes must be complete, unique, and in deterministic index order"
                        .into(),
                ));
            }
            if episode
                .terminal_turn
                .is_some_and(|turn| turn == 0 || turn > request.maximum_turns)
            {
                return Err(PhaseWorkerProtocolError::Invalid(
                    "episode terminalTurn falls outside the requested turn bound".into(),
                ));
            }
            match (episode.terminal, episode.terminal_turn) {
                (EpisodeTerminal::Win, None) | (EpisodeTerminal::TurnLimit, Some(_)) => {
                    return Err(PhaseWorkerProtocolError::Invalid(
                        "episode terminal and terminalTurn are inconsistent".into(),
                    ));
                }
                _ => {}
            }
            if !(1..=7).contains(&episode.opening_hand_size) {
                return Err(PhaseWorkerProtocolError::Invalid(
                    "openingHandSize must be 1 through 7".into(),
                ));
            }
            validate_sha256(
                "deterministicTraceSha256",
                &episode.deterministic_trace_sha256,
            )?;
        }
        let expected_outcome_sha = batch_outcome_fingerprint(outcome)?;
        if outcome.outcome_sha256 != expected_outcome_sha {
            return Err(PhaseWorkerProtocolError::Invalid(
                "outcomeSha256 does not bind the complete deterministic episode outcomes".into(),
            ));
        }
        Ok(VerifiedBatchOutcome {
            provenance: self.provenance.clone(),
            canonical_deck_sha256: outcome.canonical_deck_sha256.clone(),
            preflight_sha256: outcome.preflight_sha256.clone(),
            request_sha256: outcome.request_sha256.clone(),
            outcome_sha256: outcome.outcome_sha256.clone(),
            episodes: outcome.episodes.clone(),
        })
    }

    pub fn verify_cancel(
        &self,
        request_id: &str,
        response_request_id: &str,
        request: &CancelRequest,
        outcome: &CancelOutcome,
    ) -> Result<(), PhaseWorkerProtocolError> {
        require_same_request(request_id, response_request_id)?;
        validate_client_message(&ClientMessage::Cancel(request.clone()))?;
        validate_server_message(&ServerMessage::Cancel(outcome.clone()))?;
        require_identity(&self.provenance, &request.expected_provenance)?;
        require_identity(&self.provenance, &outcome.provenance)?;
        if request.target_request_id != outcome.target_request_id || !outcome.acknowledged {
            return Err(PhaseWorkerProtocolError::Correlation(
                "worker did not acknowledge cancellation of the exact target request".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedStrictPreflight {
    pub provenance: WorkerProvenance,
    pub canonical_deck_sha256: String,
    pub preflight_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedBatchOutcome {
    pub provenance: WorkerProvenance,
    pub canonical_deck_sha256: String,
    pub preflight_sha256: String,
    pub request_sha256: String,
    pub outcome_sha256: String,
    pub episodes: Vec<WorkerEpisodeOutcome>,
}

#[derive(Debug, Default)]
pub struct ProgressValidator {
    requests: HashMap<String, (ProgressStage, u32, u32)>,
}

impl ProgressValidator {
    pub fn observe(
        &mut self,
        session: &VerifiedWorkerSession,
        request_id: &str,
        progress: &WorkerProgress,
    ) -> Result<(), PhaseWorkerProtocolError> {
        validate_request_id(request_id)?;
        validate_server_message(&ServerMessage::Progress(progress.clone()))?;
        require_identity(session.provenance(), &progress.provenance)?;
        if let Some((previous_stage, previous_completed, previous_total)) =
            self.requests.get(request_id)
            && (stage_ordinal(progress.stage) < stage_ordinal(*previous_stage)
                || progress.total_units != *previous_total
                || progress.completed_units < *previous_completed)
        {
            return Err(PhaseWorkerProtocolError::Invalid(
                "worker progress must be monotonic with a stable total".into(),
            ));
        }
        self.requests.insert(
            request_id.into(),
            (
                progress.stage,
                progress.completed_units,
                progress.total_units,
            ),
        );
        Ok(())
    }

    pub fn finish(&mut self, request_id: &str) {
        self.requests.remove(request_id);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerFailureKind {
    Unsupported,
    Cancelled,
    Stall,
    Timeout,
    UnexpectedEof,
    Panic,
    ProtocolViolation,
    IntegrityMismatch,
}

impl From<WorkerErrorCode> for WorkerFailureKind {
    fn from(code: WorkerErrorCode) -> Self {
        match code {
            WorkerErrorCode::UnsupportedCardFunction | WorkerErrorCode::UnsupportedScenario => {
                Self::Unsupported
            }
            WorkerErrorCode::Cancelled => Self::Cancelled,
            WorkerErrorCode::EnginePanic | WorkerErrorCode::InternalFailure => Self::Panic,
            WorkerErrorCode::InvalidRequest => Self::ProtocolViolation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerHealthState {
    Healthy,
    RestartRequired { reason: WorkerFailureKind },
    Quarantined { reason: WorkerFailureKind },
}

#[derive(Debug, Clone)]
pub struct WorkerHealthTracker {
    state: WorkerHealthState,
    consecutive_runtime_failures: u8,
}

impl Default for WorkerHealthTracker {
    fn default() -> Self {
        Self {
            state: WorkerHealthState::Healthy,
            consecutive_runtime_failures: 0,
        }
    }
}

impl WorkerHealthTracker {
    pub fn state(&self) -> &WorkerHealthState {
        &self.state
    }

    /// Unsupported execution and cooperative cancellation fail the current
    /// strict request but do not imply a corrupt process. Runtime, protocol,
    /// and integrity failures require a clean process restart; two consecutive
    /// failures quarantine the pack until an explicit verified-pack reset.
    pub fn record_failure(&mut self, failure: WorkerFailureKind) {
        match failure {
            WorkerFailureKind::Unsupported | WorkerFailureKind::Cancelled => {}
            WorkerFailureKind::Stall
            | WorkerFailureKind::Timeout
            | WorkerFailureKind::UnexpectedEof
            | WorkerFailureKind::Panic
            | WorkerFailureKind::ProtocolViolation
            | WorkerFailureKind::IntegrityMismatch => {
                self.consecutive_runtime_failures =
                    self.consecutive_runtime_failures.saturating_add(1);
                self.state = if self.consecutive_runtime_failures >= 2 {
                    WorkerHealthState::Quarantined { reason: failure }
                } else {
                    WorkerHealthState::RestartRequired { reason: failure }
                };
            }
        }
    }

    /// Called only after a new process repeats a fully verified hello against
    /// the same installed pack. It permits new work but deliberately does not
    /// clear the failure counter until a complete strict batch succeeds.
    pub fn record_verified_restart(&mut self) -> Result<(), PhaseWorkerProtocolError> {
        match self.state {
            WorkerHealthState::RestartRequired { .. } => {
                self.state = WorkerHealthState::Healthy;
                Ok(())
            }
            WorkerHealthState::Quarantined { .. } => Err(PhaseWorkerProtocolError::NotStrict(
                "quarantined worker packs require explicit re-verification or replacement".into(),
            )),
            WorkerHealthState::Healthy => Ok(()),
        }
    }

    pub fn record_complete_batch(&mut self) {
        self.consecutive_runtime_failures = 0;
        self.state = WorkerHealthState::Healthy;
    }

    pub fn reset_for_verified_pack_change(&mut self) {
        self.consecutive_runtime_failures = 0;
        self.state = WorkerHealthState::Healthy;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerRuntimePolicy {
    pub hello_timeout_ms: u64,
    pub preflight_timeout_ms: u64,
    pub batch_wall_timeout_ms: u64,
    pub output_stall_timeout_ms: u64,
    pub cancellation_grace_ms: u64,
}

impl Default for WorkerRuntimePolicy {
    fn default() -> Self {
        Self {
            hello_timeout_ms: 2_000,
            preflight_timeout_ms: 30_000,
            batch_wall_timeout_ms: 120_000,
            output_stall_timeout_ms: 10_000,
            cancellation_grace_ms: 1_000,
        }
    }
}

impl WorkerRuntimePolicy {
    pub fn validate(self) -> Result<Self, PhaseWorkerProtocolError> {
        if !(250..=10_000).contains(&self.hello_timeout_ms)
            || !(1_000..=120_000).contains(&self.preflight_timeout_ms)
            || !(5_000..=600_000).contains(&self.batch_wall_timeout_ms)
            || !(500..=60_000).contains(&self.output_stall_timeout_ms)
            || !(100..=5_000).contains(&self.cancellation_grace_ms)
            || self.output_stall_timeout_ms >= self.batch_wall_timeout_ms
        {
            return Err(PhaseWorkerProtocolError::Invalid(
                "worker timeout policy falls outside the protocol's bounded safety envelope".into(),
            ));
        }
        Ok(self)
    }
}

pub fn preflight_fingerprint(
    outcome: &PreflightDeckOutcome,
) -> Result<String, PhaseWorkerProtocolError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Fingerprint<'a> {
        protocol_version: &'static str,
        provenance: &'a WorkerProvenance,
        canonical_deck_sha256: &'a str,
        card_snapshot_sha256: &'a str,
        host_execution_coverage_manifest_sha256: &'a str,
        scope: PreflightScope,
        totals: &'a CoverageTotals,
        card_coverage: &'a [CardCoverageTotals],
        gaps: &'a [CoverageGap],
        gaps_truncated: bool,
        complete: bool,
    }
    hash_json(&Fingerprint {
        protocol_version: PHASE_WORKER_PROTOCOL_VERSION,
        provenance: &outcome.provenance,
        canonical_deck_sha256: &outcome.canonical_deck_sha256,
        card_snapshot_sha256: &outcome.card_snapshot_sha256,
        host_execution_coverage_manifest_sha256: &outcome.host_execution_coverage_manifest_sha256,
        scope: outcome.scope,
        totals: &outcome.totals,
        card_coverage: &outcome.card_coverage,
        gaps: &outcome.gaps,
        gaps_truncated: outcome.gaps_truncated,
        complete: outcome.complete,
    })
}

pub fn batch_outcome_fingerprint(
    outcome: &RunBatchOutcome,
) -> Result<String, PhaseWorkerProtocolError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Fingerprint<'a> {
        protocol_version: &'static str,
        provenance: &'a WorkerProvenance,
        canonical_deck_sha256: &'a str,
        preflight_sha256: &'a str,
        request_sha256: &'a str,
        seed_exact: &'a str,
        scenario: WorkerScenario,
        completed_episodes: u32,
        episodes: &'a [WorkerEpisodeOutcome],
    }
    hash_json(&Fingerprint {
        protocol_version: PHASE_WORKER_PROTOCOL_VERSION,
        provenance: &outcome.provenance,
        canonical_deck_sha256: &outcome.canonical_deck_sha256,
        preflight_sha256: &outcome.preflight_sha256,
        request_sha256: &outcome.request_sha256,
        seed_exact: &outcome.seed_exact,
        scenario: outcome.scenario,
        completed_episodes: outcome.completed_episodes,
        episodes: &outcome.episodes,
    })
}

fn request_fingerprint(request: &RunBatchRequest) -> Result<String, PhaseWorkerProtocolError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Fingerprint<'a> {
        protocol_version: &'static str,
        expected_provenance: &'a WorkerProvenance,
        canonical_deck_sha256: &'a str,
        preflight_sha256: &'a str,
        scenario: WorkerScenario,
        scenario_input_sha256: &'a str,
        seed_exact: &'a str,
        episode_start: u32,
        episode_count: u32,
        maximum_turns: u16,
    }
    hash_json(&Fingerprint {
        protocol_version: PHASE_WORKER_PROTOCOL_VERSION,
        expected_provenance: &request.expected_provenance,
        canonical_deck_sha256: &request.canonical_deck_sha256,
        preflight_sha256: &request.preflight_sha256,
        scenario: request.scenario,
        scenario_input_sha256: &request.scenario_input_sha256,
        seed_exact: &request.seed_exact,
        episode_start: request.episode_start,
        episode_count: request.episode_count,
        maximum_turns: request.maximum_turns,
    })
}

fn validate_preflight_shape(
    outcome: &PreflightDeckOutcome,
) -> Result<(), PhaseWorkerProtocolError> {
    validate_sha256("canonicalDeckSha256", &outcome.canonical_deck_sha256)?;
    validate_sha256("cardSnapshotSha256", &outcome.card_snapshot_sha256)?;
    validate_sha256(
        "hostExecutionCoverageManifestSha256",
        &outcome.host_execution_coverage_manifest_sha256,
    )?;
    validate_sha256("preflightSha256", &outcome.preflight_sha256)?;
    let totals = &outcome.totals;
    if totals.cards_total == 0
        || totals.unique_cards_total == 0
        || totals.unique_cards_total > totals.cards_total
        || totals.faces_total < totals.unique_cards_total
        || totals.abilities_total > MAXIMUM_EFFECT_LEAVES
        || totals.effect_leaves_total > MAXIMUM_EFFECT_LEAVES
    {
        return Err(PhaseWorkerProtocolError::Invalid(
            "preflight structural totals are outside protocol bounds".into(),
        ));
    }
    if outcome.card_coverage.len() != totals.unique_cards_total as usize {
        return Err(PhaseWorkerProtocolError::Invalid(
            "cardCoverage must contain one row for every unique card".into(),
        ));
    }
    let mut card_ids = HashSet::new();
    let mut summed_faces = 0_u32;
    let mut summed_abilities = 0_u32;
    let mut summed_leaves = 0_u32;
    let mut summed_executable = 0_u32;
    let mut summed_unsupported = 0_u32;
    let mut summed_unresolved = 0_u32;
    let mut summed_ambiguous = 0_u32;
    for card in &outcome.card_coverage {
        if !is_canonical_uuid(&card.oracle_id) || !card_ids.insert(card.oracle_id.as_str()) {
            return Err(PhaseWorkerProtocolError::Invalid(
                "cardCoverage oracleId values must be unique canonical UUIDs".into(),
            ));
        }
        validate_sha256("cardCoverage.cardRecordSha256", &card.card_record_sha256)?;
        validate_sha256(
            "cardCoverage.coverageTreeSha256",
            &card.coverage_tree_sha256,
        )?;
        if card.faces_total == 0
            || card.abilities_total == 0
            || card.effect_leaves_total == 0
            || card
                .unsupported_effect_leaves
                .checked_add(card.unresolved_effect_leaves)
                .and_then(|value| value.checked_add(card.ambiguous_effect_leaves))
                .and_then(|blockers| card.executable_effect_leaves.checked_add(blockers))
                .is_none_or(|covered| covered != card.effect_leaves_total)
        {
            return Err(PhaseWorkerProtocolError::Invalid(format!(
                "cardCoverage for {} must account for every face, ability, and effect leaf",
                card.oracle_id
            )));
        }
        summed_faces = checked_sum(summed_faces, card.faces_total)?;
        summed_abilities = checked_sum(summed_abilities, card.abilities_total)?;
        summed_leaves = checked_sum(summed_leaves, card.effect_leaves_total)?;
        summed_executable = checked_sum(summed_executable, card.executable_effect_leaves)?;
        summed_unsupported = checked_sum(summed_unsupported, card.unsupported_effect_leaves)?;
        summed_unresolved = checked_sum(summed_unresolved, card.unresolved_effect_leaves)?;
        summed_ambiguous = checked_sum(summed_ambiguous, card.ambiguous_effect_leaves)?;
    }
    if summed_faces != totals.faces_total
        || summed_abilities != totals.abilities_total
        || summed_leaves != totals.effect_leaves_total
        || summed_executable != totals.executable_effect_leaves
        || summed_unsupported != totals.unsupported_effect_leaves
        || summed_unresolved != totals.unresolved_effect_leaves
        || summed_ambiguous != totals.ambiguous_effect_leaves
    {
        return Err(PhaseWorkerProtocolError::Invalid(
            "cardCoverage rows must exactly sum to the deck coverage totals".into(),
        ));
    }
    let blockers = totals
        .unsupported_effect_leaves
        .checked_add(totals.unresolved_effect_leaves)
        .and_then(|value| value.checked_add(totals.ambiguous_effect_leaves))
        .ok_or_else(|| {
            PhaseWorkerProtocolError::Invalid("preflight coverage totals overflow".into())
        })?;
    if blockers != totals.total_gap_count
        || totals
            .executable_effect_leaves
            .checked_add(blockers)
            .is_none_or(|total| total != totals.effect_leaves_total)
    {
        return Err(PhaseWorkerProtocolError::Invalid(
            "executable, unsupported, unresolved, and ambiguous leaves must exactly conserve effectLeavesTotal"
                .into(),
        ));
    }
    if outcome.gaps.len() > MAXIMUM_COVERAGE_GAPS
        || outcome.gaps.len() as u32 > totals.total_gap_count
        || (!outcome.gaps_truncated && outcome.gaps.len() as u32 != totals.total_gap_count)
    {
        return Err(PhaseWorkerProtocolError::Invalid(
            "coverage gap details do not match totalGapCount and truncation state".into(),
        ));
    }
    if outcome.complete
        != (!outcome.gaps_truncated
            && totals.total_gap_count == 0
            && totals.effect_leaves_total > 0
            && totals.executable_effect_leaves == totals.effect_leaves_total)
    {
        return Err(PhaseWorkerProtocolError::Invalid(
            "complete must be true exactly when every effect leaf is executable and no gap detail is truncated"
                .into(),
        ));
    }
    let mut gap_ids = HashSet::new();
    for gap in &outcome.gaps {
        validate_bounded_text("gapId", &gap.gap_id, MAXIMUM_NAME_BYTES)?;
        if !gap_ids.insert(gap.gap_id.as_str()) {
            return Err(PhaseWorkerProtocolError::Invalid(
                "coverage gap identifiers must be unique".into(),
            ));
        }
        if !is_canonical_uuid(&gap.oracle_id) {
            return Err(PhaseWorkerProtocolError::Invalid(
                "coverage gap oracleId must be a lowercase canonical UUID".into(),
            ));
        }
        if !card_ids.contains(gap.oracle_id.as_str()) {
            return Err(PhaseWorkerProtocolError::Invalid(
                "coverage gap oracleId is absent from cardCoverage".into(),
            ));
        }
        validate_bounded_text("coverage gap detail", &gap.detail, MAXIMUM_DETAIL_BYTES)?;
    }
    Ok(())
}

fn validate_batch_outcome_shape(outcome: &RunBatchOutcome) -> Result<(), PhaseWorkerProtocolError> {
    validate_sha256("canonicalDeckSha256", &outcome.canonical_deck_sha256)?;
    validate_sha256("preflightSha256", &outcome.preflight_sha256)?;
    validate_sha256("requestSha256", &outcome.request_sha256)?;
    validate_sha256("outcomeSha256", &outcome.outcome_sha256)?;
    validate_exact_u64("seedExact", &outcome.seed_exact)?;
    if outcome.completed_episodes == 0
        || outcome.completed_episodes > MAXIMUM_BATCH_EPISODES
        || outcome.episodes.len() != outcome.completed_episodes as usize
    {
        return Err(PhaseWorkerProtocolError::Invalid(
            "completed batch outcomes must contain exactly 1 through 1000 episodes".into(),
        ));
    }
    Ok(())
}

fn checked_sum(left: u32, right: u32) -> Result<u32, PhaseWorkerProtocolError> {
    left.checked_add(right).ok_or_else(|| {
        PhaseWorkerProtocolError::Invalid("worker coverage totals overflow u32".into())
    })
}

fn require_identity(
    expected: &WorkerProvenance,
    actual: &WorkerProvenance,
) -> Result<(), PhaseWorkerProtocolError> {
    expected.validate()?;
    actual.validate()?;
    if actual != expected {
        return Err(PhaseWorkerProtocolError::IdentityMismatch(
            "protocol, pack, engine, worker, card-data, or rules-data identity differs".into(),
        ));
    }
    Ok(())
}

fn require_same_request(expected: &str, actual: &str) -> Result<(), PhaseWorkerProtocolError> {
    validate_request_id(expected)?;
    validate_request_id(actual)?;
    if expected != actual {
        return Err(PhaseWorkerProtocolError::Correlation(format!(
            "expected requestId {expected}, received {actual}"
        )));
    }
    Ok(())
}

fn validate_request_id(value: &str) -> Result<(), PhaseWorkerProtocolError> {
    if value.is_empty()
        || value.len() > MAXIMUM_REQUEST_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
    {
        return Err(PhaseWorkerProtocolError::Invalid(
            "requestId must be 1 to 128 ASCII identifier characters".into(),
        ));
    }
    Ok(())
}

fn validate_nonce(value: &str) -> Result<(), PhaseWorkerProtocolError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PhaseWorkerProtocolError::Invalid(
            "hello nonce must contain exactly 64 hexadecimal characters".into(),
        ));
    }
    Ok(())
}

fn validate_version(field: &str, value: &str) -> Result<(), PhaseWorkerProtocolError> {
    if value.is_empty()
        || value.len() > MAXIMUM_VERSION_BYTES
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+' | b'/')
        })
    {
        return Err(PhaseWorkerProtocolError::Invalid(format!(
            "{field} must be a bounded ASCII version identifier"
        )));
    }
    Ok(())
}

fn validate_https_source_url(field: &str, value: &str) -> Result<(), PhaseWorkerProtocolError> {
    if value.len() > 2_048 {
        return Err(PhaseWorkerProtocolError::Invalid(format!(
            "{field} exceeds the URL length bound"
        )));
    }
    let parsed = Url::parse(value).map_err(|_| {
        PhaseWorkerProtocolError::Invalid(format!("{field} must be an absolute HTTPS URL"))
    })?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(PhaseWorkerProtocolError::Invalid(format!(
            "{field} must be a credential-free HTTPS source URL without query or fragment"
        )));
    }
    Ok(())
}

fn validate_sha256(field: &str, value: &str) -> Result<(), PhaseWorkerProtocolError> {
    if !is_lowercase_sha256(value) {
        return Err(PhaseWorkerProtocolError::Invalid(format!(
            "{field} must contain exactly 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn validate_exact_u64(field: &str, value: &str) -> Result<(), PhaseWorkerProtocolError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || value.parse::<u64>().is_err()
    {
        return Err(PhaseWorkerProtocolError::Invalid(format!(
            "{field} must be the canonical decimal representation of an unsigned 64-bit integer"
        )));
    }
    Ok(())
}

fn validate_bounded_text(
    field: &str,
    value: &str,
    maximum_bytes: usize,
) -> Result<(), PhaseWorkerProtocolError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > maximum_bytes
        || value.chars().any(char::is_control)
    {
        return Err(PhaseWorkerProtocolError::Invalid(format!(
            "{field} must be non-empty, trimmed, control-free text no longer than {maximum_bytes} bytes"
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

fn is_canonical_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte),
        })
}

fn stage_ordinal(stage: ProgressStage) -> u8 {
    match stage {
        ProgressStage::Starting => 0,
        ProgressStage::Preflight => 1,
        ProgressStage::Simulating => 2,
        ProgressStage::Finalizing => 3,
    }
}

fn to_value<T: Serialize>(value: &T) -> Result<Value, PhaseWorkerProtocolError> {
    serde_json::to_value(value).map_err(|error| PhaseWorkerProtocolError::Json(error.to_string()))
}

fn from_value<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, PhaseWorkerProtocolError> {
    serde_json::from_value(value).map_err(|error| PhaseWorkerProtocolError::Json(error.to_string()))
}

fn hash_json<T: Serialize>(value: &T) -> Result<String, PhaseWorkerProtocolError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| PhaseWorkerProtocolError::Json(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
