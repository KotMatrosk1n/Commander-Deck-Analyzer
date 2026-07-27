//! Versioned, deterministic interaction-scenario contracts.
//!
//! This module is deliberately isolated from the production simulator. It
//! describes paired baseline/stressed episodes and aggregates their outcomes;
//! it does not invent Magic actions or claim that a response was legal. Reports
//! are therefore labeled `response-pressure` unless the caller identifies a
//! strict legal-action engine as the execution source.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

pub const INTERACTION_SCENARIO_INPUT_VERSION: &str = "commander-interaction-scenarios/input/v2";
pub const INTERACTION_SCENARIO_REPORT_VERSION: &str = "commander-interaction-scenarios/report/v2";
pub const INTERACTION_DIRECTIVE_VERSION: &str = "commander-interaction-directives/v1";
pub const INTERACTION_CHECKPOINT_VERSION: &str = "commander-interaction-checkpoints/v1";
pub const RESPONSE_PRESSURE_LABEL: &str = "response-pressure";
pub const STRICT_LEGAL_ACTION_LABEL: &str = "strict-legal-action";

const LEGACY_INTERACTION_SCENARIO_INPUT_VERSION: &str = "commander-interaction-scenarios/input/v1";
const LEGACY_INTERACTION_SCENARIO_REPORT_VERSION: &str =
    "commander-interaction-scenarios/report/v1";
const MAX_EPISODES: usize = 100_000;
const MAX_EPISODE_ID_BYTES: usize = 160;
const MAX_ENGINE_FIELD_BYTES: usize = 160;
const MAX_UNSUPPORTED_REASON_BYTES: usize = 1_024;
const MAX_TURN_CAP: u16 = 100;

/// The fixed scenario set. Variant and report order are part of the v1
/// reproducibility contract.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase")]
pub enum InteractionScenario {
    TargetedPermanentRemoval,
    CommanderRemovalRecast,
    FirstRelevantSpellCountered,
    CreatureWipe,
    GraveyardShutdown,
    GenericTaxStax,
    RuleOfLawCap,
    FirstWinAttemptStopped,
}

impl InteractionScenario {
    pub const ALL: [Self; 8] = [
        Self::TargetedPermanentRemoval,
        Self::CommanderRemovalRecast,
        Self::FirstRelevantSpellCountered,
        Self::CreatureWipe,
        Self::GraveyardShutdown,
        Self::GenericTaxStax,
        Self::RuleOfLawCap,
        Self::FirstWinAttemptStopped,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::TargetedPermanentRemoval => "targeted-permanent-removal",
            Self::CommanderRemovalRecast => "commander-removal-recast",
            Self::FirstRelevantSpellCountered => "first-relevant-spell-countered",
            Self::CreatureWipe => "creature-wipe",
            Self::GraveyardShutdown => "graveyard-shutdown",
            Self::GenericTaxStax => "generic-tax-stax",
            Self::RuleOfLawCap => "rule-of-law-cap",
            Self::FirstWinAttemptStopped => "first-win-attempt-stopped",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InteractionDirective {
    pub directive_version: String,
    pub checkpoint_version: String,
    pub scenario: InteractionScenario,
    pub scenario_id: String,
    pub checkpoint: ScenarioCheckpoint,
    pub intervention: ScenarioIntervention,
    pub recovery_checkpoint: RecoveryCheckpoint,
    pub selection: DeterministicSelection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ScenarioCheckpoint {
    FirstEligibleNoncommanderPermanentEstablished,
    FirstCommanderEstablished,
    FirstRelevantSpellOnStack,
    FirstRelevantCreatureBoardEstablished { minimum_creatures: u16 },
    FirstGraveyardDependentAction,
    FirstTaxableAction,
    FirstSecondSpellAttemptInTurn,
    FirstWinAttempt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ScenarioIntervention {
    ExileSelectedPermanent,
    RemoveCommanderToCommandZone,
    CounterSelectedSpell,
    DestroyAllCreatures,
    ShutDownGraveyardActions,
    AddGenericTax { generic_mana: u16 },
    CapSpellsPerTurn { cap: u16 },
    StopWinAttempt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryCheckpoint {
    EstablishReplacementPermanentOrWinAttempt,
    RecastCommander,
    ResolveNextRelevantSpellOrWinAttempt,
    ReestablishRelevantCreatureBoard,
    EstablishNongraveyardPlanOrRemoveShutdown,
    PayThroughOrRemoveTax,
    ProgressUnderCapOrRemoveRule,
    ProduceNextWinAttempt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeterministicSelection {
    pub occurrence: SelectionOccurrence,
    pub tie_breakers: Vec<SelectionTieBreaker>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SelectionOccurrence {
    First,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SelectionTieBreaker {
    EventSequence,
    StableObjectId,
    StablePlayerId,
}

/// Returns the complete deterministic v1 directive for a scenario.
pub fn directive_for(scenario: InteractionScenario) -> InteractionDirective {
    let (checkpoint, intervention, recovery_checkpoint) = match scenario {
        InteractionScenario::TargetedPermanentRemoval => (
            ScenarioCheckpoint::FirstEligibleNoncommanderPermanentEstablished,
            ScenarioIntervention::ExileSelectedPermanent,
            RecoveryCheckpoint::EstablishReplacementPermanentOrWinAttempt,
        ),
        InteractionScenario::CommanderRemovalRecast => (
            ScenarioCheckpoint::FirstCommanderEstablished,
            ScenarioIntervention::RemoveCommanderToCommandZone,
            RecoveryCheckpoint::RecastCommander,
        ),
        InteractionScenario::FirstRelevantSpellCountered => (
            ScenarioCheckpoint::FirstRelevantSpellOnStack,
            ScenarioIntervention::CounterSelectedSpell,
            RecoveryCheckpoint::ResolveNextRelevantSpellOrWinAttempt,
        ),
        InteractionScenario::CreatureWipe => (
            ScenarioCheckpoint::FirstRelevantCreatureBoardEstablished {
                minimum_creatures: 2,
            },
            ScenarioIntervention::DestroyAllCreatures,
            RecoveryCheckpoint::ReestablishRelevantCreatureBoard,
        ),
        InteractionScenario::GraveyardShutdown => (
            ScenarioCheckpoint::FirstGraveyardDependentAction,
            ScenarioIntervention::ShutDownGraveyardActions,
            RecoveryCheckpoint::EstablishNongraveyardPlanOrRemoveShutdown,
        ),
        InteractionScenario::GenericTaxStax => (
            ScenarioCheckpoint::FirstTaxableAction,
            ScenarioIntervention::AddGenericTax { generic_mana: 1 },
            RecoveryCheckpoint::PayThroughOrRemoveTax,
        ),
        InteractionScenario::RuleOfLawCap => (
            ScenarioCheckpoint::FirstSecondSpellAttemptInTurn,
            ScenarioIntervention::CapSpellsPerTurn { cap: 1 },
            RecoveryCheckpoint::ProgressUnderCapOrRemoveRule,
        ),
        InteractionScenario::FirstWinAttemptStopped => (
            ScenarioCheckpoint::FirstWinAttempt,
            ScenarioIntervention::StopWinAttempt,
            RecoveryCheckpoint::ProduceNextWinAttempt,
        ),
    };

    InteractionDirective {
        directive_version: INTERACTION_DIRECTIVE_VERSION.into(),
        checkpoint_version: INTERACTION_CHECKPOINT_VERSION.into(),
        scenario,
        scenario_id: scenario.id().into(),
        checkpoint,
        intervention,
        recovery_checkpoint,
        selection: DeterministicSelection {
            occurrence: SelectionOccurrence::First,
            tie_breakers: vec![
                SelectionTieBreaker::EventSequence,
                SelectionTieBreaker::StableObjectId,
                SelectionTieBreaker::StablePlayerId,
            ],
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ScenarioExecutionSource {
    ResponsePressure,
    StrictLegalActionEngine {
        engine_id: String,
        engine_version: String,
        legal_action_schema_version: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioReportInput {
    pub schema_version: String,
    pub scenario: InteractionScenario,
    pub execution_source: ScenarioExecutionSource,
    pub episodes: Vec<ScenarioEpisodeInput>,
}

impl ScenarioReportInput {
    pub fn new(
        scenario: InteractionScenario,
        execution_source: ScenarioExecutionSource,
        episodes: Vec<ScenarioEpisodeInput>,
    ) -> Self {
        Self {
            schema_version: INTERACTION_SCENARIO_INPUT_VERSION.into(),
            scenario,
            execution_source,
            episodes,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioEpisodeInput {
    pub episode_id: String,
    /// The shared deterministic seed used for both members of this pair.
    pub episode_seed: u64,
    pub turn_cap: u16,
    pub applicability: ScenarioApplicability,
    pub baseline: EpisodeOutcomeInput,
    pub stressed: EpisodeOutcomeInput,
    pub events: ScenarioEventCounters,
    /// `None` means no effectful intervention created a recovery opportunity.
    pub recovery: Option<RecoveryObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ScenarioApplicability {
    Applicable,
    NotApplicable {
        reason: InapplicabilityReason,
    },
    /// Semantic or execution coverage was insufficient to determine
    /// applicability. This is intentionally not counted as not-applicable.
    Undetermined {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::enum_variant_names)]
#[serde(rename_all = "camelCase")]
pub enum InapplicabilityReason {
    NoEligibleNoncommanderPermanent,
    NoCommanderSubject,
    NoRelevantSpellClass,
    NoRelevantCreatureBoardPlan,
    NoGraveyardDependency,
    NoTaxableActionClass,
    NoMultispellPlan,
    NoRepresentableWinAttempt,
}

impl InapplicabilityReason {
    pub const fn scenario(self) -> InteractionScenario {
        match self {
            Self::NoEligibleNoncommanderPermanent => InteractionScenario::TargetedPermanentRemoval,
            Self::NoCommanderSubject => InteractionScenario::CommanderRemovalRecast,
            Self::NoRelevantSpellClass => InteractionScenario::FirstRelevantSpellCountered,
            Self::NoRelevantCreatureBoardPlan => InteractionScenario::CreatureWipe,
            Self::NoGraveyardDependency => InteractionScenario::GraveyardShutdown,
            Self::NoTaxableActionClass => InteractionScenario::GenericTaxStax,
            Self::NoMultispellPlan => InteractionScenario::RuleOfLawCap,
            Self::NoRepresentableWinAttempt => InteractionScenario::FirstWinAttemptStopped,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EpisodeOutcomeInput {
    pub credible_threat: CensoredTurn,
    pub first_win_attempt: CensoredTurn,
    /// `None` is retained only when deserializing a legacy v1 episode. A v2
    /// input must provide an independently observed or censored resolution;
    /// it is never inferred from `first_win_attempt`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_table_win: Option<CensoredTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CensoredTurn {
    Observed {
        turn: u16,
    },
    /// The event did not occur on or before `at_turn`.
    RightCensored {
        at_turn: u16,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RecoveryObservation {
    Recovered { turn: u16 },
    RightCensored { at_turn: u16 },
}

/// Per-episode event counts. The v1 directives select only the first
/// opportunity, so all fields except `affected_game_events` are binary counts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioEventCounters {
    pub checkpoint_matches: u32,
    pub opportunities: u32,
    pub directive_attempts: u32,
    pub directive_applied: u32,
    pub directive_rejected: u32,
    pub directive_no_ops: u32,
    pub affected_game_events: u32,
}

impl ScenarioEventCounters {
    fn effectful_interventions(&self) -> u32 {
        self.directive_applied.saturating_sub(self.directive_no_ops)
    }

    fn is_zero(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioReport {
    pub schema_version: String,
    pub directive: InteractionDirective,
    pub measurement: MeasurementDescriptor,
    pub counters: AggregateScenarioCounters,
    pub episodes: Vec<ScenarioEpisodeReport>,
    pub credible_threat_delay: PairedDelayDistribution,
    pub first_win_attempt_delay: PairedDelayDistribution,
    /// `None` identifies a legacy v1 report whose episodes did not observe the
    /// resolved-table-win endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_table_win_delay: Option<PairedDelayDistribution>,
    pub recovery: RecoverySummary,
}

/// O(1)-size production projection of a full scenario report. Per-episode
/// traces remain available to validation callers but are intentionally not
/// embedded in the analysis report or cache.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactScenarioReport {
    pub schema_version: String,
    pub directive: InteractionDirective,
    pub measurement: MeasurementDescriptor,
    pub sampling: ScenarioSamplingSummary,
    pub applicability: CompactApplicabilitySummary,
    pub counters: AggregateScenarioCounters,
    pub credible_threat_delay: CompactPairedDelayDistribution,
    pub first_win_attempt_delay: CompactPairedDelayDistribution,
    /// `None` identifies a legacy v1 projection with unknown resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_table_win_delay: Option<CompactPairedDelayDistribution>,
    pub recovery: CompactRecoverySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioSamplingSummary {
    pub master_seed: u64,
    /// Exact decimal representation for JavaScript/JSON consumers, where a
    /// `u64` numeric value may exceed the safe-integer range.
    #[serde(default)]
    pub master_seed_exact: String,
    pub seed_derivation_version: String,
    pub episode_count: u32,
    pub maximum_turn: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactApplicabilitySummary {
    pub applicable_episodes: u64,
    pub not_applicable_episodes: u64,
    pub undetermined_episodes: u64,
    pub primary_not_applicable_reason: Option<InapplicabilityReason>,
    pub distinct_not_applicable_reasons: u32,
    pub primary_undetermined_reason: Option<String>,
    pub distinct_undetermined_reasons: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactPairedDelayDistribution {
    pub metric: DelayMetric,
    pub total_episode_pairs: u32,
    pub applicable_pairs: u32,
    pub effectful_pairs: u32,
    pub observed_pairs: u32,
    pub right_censored_pairs: u32,
    pub no_op_invariant_pairs: u32,
    pub non_estimable_pairs: u32,
    pub excluded_pairs: u32,
    pub observed_delay_p10_turns: Option<f64>,
    pub observed_delay_median_turns: Option<f64>,
    pub observed_delay_p90_turns: Option<f64>,
    /// The true delay for each right-censored pair is strictly greater than
    /// its bound. These aggregates preserve that distinction without
    /// serializing one record per episode.
    pub censored_bound_min_turns: Option<i32>,
    pub censored_bound_median_turns: Option<f64>,
    pub censored_bound_max_turns: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactRecoverySummary {
    pub opportunities: u32,
    pub recovered: u32,
    pub right_censored: u32,
    pub recovered_by_turn_cap_rate: Option<f64>,
    pub observed_recovery_p10_turn: Option<f64>,
    pub observed_recovery_median_turn: Option<f64>,
    pub observed_recovery_p90_turn: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeasurementDescriptor {
    /// Exactly `response-pressure` unless a strict legal-action engine identity
    /// was supplied in the input.
    pub label: String,
    pub execution_source: ScenarioExecutionSource,
    pub claim_boundary: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AggregateScenarioCounters {
    pub total_episodes: u64,
    pub applicable_episodes: u64,
    pub not_applicable_episodes: u64,
    pub undetermined_episodes: u64,
    pub applicable_without_opportunity_episodes: u64,
    pub opportunity_episodes: u64,
    pub checkpoint_events: u64,
    pub opportunity_events: u64,
    pub directive_attempt_events: u64,
    pub directive_applied_events: u64,
    pub directive_rejected_events: u64,
    pub directive_no_op_events: u64,
    pub affected_game_events: u64,
    pub effectful_intervention_episodes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioEpisodeReport {
    pub episode_id: String,
    pub episode_seed: u64,
    pub turn_cap: u16,
    pub applicability: ScenarioApplicability,
    pub disposition: EpisodeDisposition,
    pub baseline: EpisodeOutcomeInput,
    pub stressed: EpisodeOutcomeInput,
    pub events: ScenarioEventCounters,
    /// Serializes as JSON `null` when no recovery opportunity existed.
    pub recovery: Option<RecoveryObservation>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum EpisodeDisposition {
    NotApplicable,
    Undetermined,
    ApplicableNoOpportunity,
    ApplicableOpportunityUnexercised,
    ApplicableDirectiveRejected,
    ApplicableDirectiveNoOp,
    EffectfulInterventionApplied,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DelayMetric {
    CredibleThreat,
    FirstWinAttempt,
    ResolvedTableWin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairedDelayDistribution {
    pub metric: DelayMetric,
    pub total_episode_pairs: u32,
    pub applicable_pairs: u32,
    pub effectful_pairs: u32,
    pub observed_pairs: u32,
    pub right_censored_pairs: u32,
    pub no_op_invariant_pairs: u32,
    pub non_estimable_pairs: u32,
    pub excluded_pairs: u32,
    pub median_observed_delay_turns: Option<f64>,
    pub observations: Vec<PairedDelayObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairedDelayObservation {
    pub episode_id: String,
    pub baseline: CensoredTurn,
    pub stressed: CensoredTurn,
    pub value: PairedDelayValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PairedDelayValue {
    Observed {
        delay_turns: i32,
    },
    /// If the baseline was observed at B and the stressed outcome was not
    /// observed through C, the true paired delay is strictly greater than
    /// `C - B`.
    RightCensored {
        greater_than_turns: i32,
    },
    /// No effectful directive was applied. Validation requires the stressed
    /// outcome to be byte-for-byte equal to the baseline outcome.
    NoOpInvariant {
        delay_turns: i32,
    },
    NonEstimable {
        reason: NonEstimableDelayReason,
    },
    Excluded {
        reason: DelayExclusionReason,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NonEstimableDelayReason {
    BaselineRightCensored,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DelayExclusionReason {
    NotApplicable,
    ApplicabilityUndetermined,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoverySummary {
    pub opportunities: u32,
    pub recovered: u32,
    pub right_censored: u32,
    /// `None` serializes as `null`; it must not be read as perfect recovery.
    pub recovered_by_turn_cap_rate: Option<f64>,
    pub observations: Vec<EpisodeRecoveryReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EpisodeRecoveryReport {
    pub episode_id: String,
    pub observation: RecoveryObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioReportError {
    UnsupportedSchema {
        found: String,
    },
    TooManyEpisodes {
        found: usize,
        maximum: usize,
    },
    InvalidExecutionSource(String),
    InvalidEpisodeId {
        episode_id: String,
        reason: String,
    },
    DuplicateEpisodeId(String),
    InvalidTurnCap {
        episode_id: String,
        turn_cap: u16,
    },
    InvalidTurnObservation {
        episode_id: String,
        field: &'static str,
        reason: String,
    },
    InapplicabilityMismatch {
        episode_id: String,
        scenario: InteractionScenario,
        reason: InapplicabilityReason,
    },
    InvalidApplicability {
        episode_id: String,
        reason: String,
    },
    InvalidEventCounters {
        episode_id: String,
        reason: String,
    },
    NoOpChangedOutcome {
        episode_id: String,
    },
    InvalidRecovery {
        episode_id: String,
        reason: String,
    },
}

impl Display for ScenarioReportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchema { found } => write!(
                formatter,
                "unsupported interaction scenario input schema `{found}`; expected \
                 `{LEGACY_INTERACTION_SCENARIO_INPUT_VERSION}` or \
                 `{INTERACTION_SCENARIO_INPUT_VERSION}`"
            ),
            Self::TooManyEpisodes { found, maximum } => {
                write!(formatter, "episode count {found} exceeds maximum {maximum}")
            }
            Self::InvalidExecutionSource(reason) => {
                write!(formatter, "invalid execution source: {reason}")
            }
            Self::InvalidEpisodeId { episode_id, reason } => {
                write!(formatter, "invalid episode id `{episode_id}`: {reason}")
            }
            Self::DuplicateEpisodeId(episode_id) => {
                write!(formatter, "duplicate episode id `{episode_id}`")
            }
            Self::InvalidTurnCap {
                episode_id,
                turn_cap,
            } => write!(
                formatter,
                "episode `{episode_id}` has invalid turn cap {turn_cap}"
            ),
            Self::InvalidTurnObservation {
                episode_id,
                field,
                reason,
            } => write!(
                formatter,
                "episode `{episode_id}` has invalid {field} observation: {reason}"
            ),
            Self::InapplicabilityMismatch {
                episode_id,
                scenario,
                reason,
            } => write!(
                formatter,
                "episode `{episode_id}` uses {reason:?} for {scenario:?}"
            ),
            Self::InvalidApplicability { episode_id, reason } => write!(
                formatter,
                "episode `{episode_id}` has invalid applicability: {reason}"
            ),
            Self::InvalidEventCounters { episode_id, reason } => write!(
                formatter,
                "episode `{episode_id}` has invalid event counters: {reason}"
            ),
            Self::NoOpChangedOutcome { episode_id } => write!(
                formatter,
                "episode `{episode_id}` changed its stressed outcome without an effectful intervention"
            ),
            Self::InvalidRecovery { episode_id, reason } => write!(
                formatter,
                "episode `{episode_id}` has invalid recovery observation: {reason}"
            ),
        }
    }
}

impl Error for ScenarioReportError {}

/// Validates and deterministically aggregates paired scenario episodes.
pub fn build_scenario_report(
    mut input: ScenarioReportInput,
) -> Result<ScenarioReport, ScenarioReportError> {
    validate_report_input(&input)?;
    let is_v2 = input.schema_version == INTERACTION_SCENARIO_INPUT_VERSION;
    input
        .episodes
        .sort_by(|first, second| first.episode_id.cmp(&second.episode_id));

    let measurement = measurement_descriptor(input.execution_source.clone());
    let counters = aggregate_counters(&input.episodes);
    let credible_threat_delay =
        build_delay_distribution(&input.episodes, DelayMetric::CredibleThreat)
            .expect("credible-threat observations are required in every schema");
    let first_win_attempt_delay =
        build_delay_distribution(&input.episodes, DelayMetric::FirstWinAttempt)
            .expect("first-win-attempt observations are required in every schema");
    let resolved_table_win_delay = if is_v2 {
        Some(
            build_delay_distribution(&input.episodes, DelayMetric::ResolvedTableWin)
                .expect("validated v2 inputs contain resolved-table-win observations"),
        )
    } else {
        None
    };
    let recovery = build_recovery_summary(&input.episodes);
    let episodes = input
        .episodes
        .into_iter()
        .map(|episode| {
            let disposition = episode_disposition(&episode);
            ScenarioEpisodeReport {
                episode_id: episode.episode_id,
                episode_seed: episode.episode_seed,
                turn_cap: episode.turn_cap,
                applicability: episode.applicability,
                disposition,
                baseline: episode.baseline,
                stressed: episode.stressed,
                events: episode.events,
                recovery: episode.recovery,
            }
        })
        .collect();

    Ok(ScenarioReport {
        schema_version: if is_v2 {
            INTERACTION_SCENARIO_REPORT_VERSION
        } else {
            LEGACY_INTERACTION_SCENARIO_REPORT_VERSION
        }
        .into(),
        directive: directive_for(input.scenario),
        measurement,
        counters,
        episodes,
        credible_threat_delay,
        first_win_attempt_delay,
        resolved_table_win_delay,
        recovery,
    })
}

pub fn compact_scenario_report(
    report: &ScenarioReport,
    master_seed: u64,
    seed_derivation_version: impl Into<String>,
) -> CompactScenarioReport {
    let not_applicable_reasons = report
        .episodes
        .iter()
        .filter_map(|episode| match &episode.applicability {
            ScenarioApplicability::NotApplicable { reason } => Some(*reason),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let undetermined_reasons = report
        .episodes
        .iter()
        .filter_map(|episode| match &episode.applicability {
            ScenarioApplicability::Undetermined { reason } => Some(reason.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let maximum_turn = report
        .episodes
        .iter()
        .map(|episode| episode.turn_cap)
        .max()
        .unwrap_or(0);
    let mut observed_recovery_turns = report
        .recovery
        .observations
        .iter()
        .filter_map(|episode| match &episode.observation {
            RecoveryObservation::Recovered { turn } => Some(i32::from(*turn)),
            RecoveryObservation::RightCensored { .. } => None,
        })
        .collect::<Vec<_>>();
    observed_recovery_turns.sort_unstable();

    CompactScenarioReport {
        schema_version: report.schema_version.clone(),
        directive: report.directive.clone(),
        measurement: report.measurement.clone(),
        sampling: ScenarioSamplingSummary {
            master_seed,
            master_seed_exact: master_seed.to_string(),
            seed_derivation_version: seed_derivation_version.into(),
            episode_count: report.counters.total_episodes.min(u64::from(u32::MAX)) as u32,
            maximum_turn,
        },
        applicability: CompactApplicabilitySummary {
            applicable_episodes: report.counters.applicable_episodes,
            not_applicable_episodes: report.counters.not_applicable_episodes,
            undetermined_episodes: report.counters.undetermined_episodes,
            primary_not_applicable_reason: not_applicable_reasons.first().copied(),
            distinct_not_applicable_reasons: not_applicable_reasons.len().min(u32::MAX as usize)
                as u32,
            primary_undetermined_reason: undetermined_reasons.first().cloned(),
            distinct_undetermined_reasons: undetermined_reasons.len().min(u32::MAX as usize) as u32,
        },
        counters: report.counters.clone(),
        credible_threat_delay: compact_delay_distribution(&report.credible_threat_delay),
        first_win_attempt_delay: compact_delay_distribution(&report.first_win_attempt_delay),
        resolved_table_win_delay: report
            .resolved_table_win_delay
            .as_ref()
            .map(compact_delay_distribution),
        recovery: CompactRecoverySummary {
            opportunities: report.recovery.opportunities,
            recovered: report.recovery.recovered,
            right_censored: report.recovery.right_censored,
            recovered_by_turn_cap_rate: report.recovery.recovered_by_turn_cap_rate,
            observed_recovery_p10_turn: percentile_i32(&observed_recovery_turns, 0.10),
            observed_recovery_median_turn: percentile_i32(&observed_recovery_turns, 0.50),
            observed_recovery_p90_turn: percentile_i32(&observed_recovery_turns, 0.90),
        },
    }
}

fn compact_delay_distribution(
    distribution: &PairedDelayDistribution,
) -> CompactPairedDelayDistribution {
    let mut observed_delays = distribution
        .observations
        .iter()
        .filter_map(|observation| match &observation.value {
            PairedDelayValue::Observed { delay_turns } => Some(*delay_turns),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut censored_bounds = distribution
        .observations
        .iter()
        .filter_map(|observation| match &observation.value {
            PairedDelayValue::RightCensored { greater_than_turns } => Some(*greater_than_turns),
            _ => None,
        })
        .collect::<Vec<_>>();
    observed_delays.sort_unstable();
    censored_bounds.sort_unstable();

    CompactPairedDelayDistribution {
        metric: distribution.metric,
        total_episode_pairs: distribution.total_episode_pairs,
        applicable_pairs: distribution.applicable_pairs,
        effectful_pairs: distribution.effectful_pairs,
        observed_pairs: distribution.observed_pairs,
        right_censored_pairs: distribution.right_censored_pairs,
        no_op_invariant_pairs: distribution.no_op_invariant_pairs,
        non_estimable_pairs: distribution.non_estimable_pairs,
        excluded_pairs: distribution.excluded_pairs,
        observed_delay_p10_turns: percentile_i32(&observed_delays, 0.10),
        observed_delay_median_turns: percentile_i32(&observed_delays, 0.50),
        observed_delay_p90_turns: percentile_i32(&observed_delays, 0.90),
        censored_bound_min_turns: censored_bounds.first().copied(),
        censored_bound_median_turns: percentile_i32(&censored_bounds, 0.50),
        censored_bound_max_turns: censored_bounds.last().copied(),
    }
}

fn validate_report_input(input: &ScenarioReportInput) -> Result<(), ScenarioReportError> {
    if input.schema_version != INTERACTION_SCENARIO_INPUT_VERSION
        && input.schema_version != LEGACY_INTERACTION_SCENARIO_INPUT_VERSION
    {
        return Err(ScenarioReportError::UnsupportedSchema {
            found: input.schema_version.clone(),
        });
    }
    if input.episodes.len() > MAX_EPISODES {
        return Err(ScenarioReportError::TooManyEpisodes {
            found: input.episodes.len(),
            maximum: MAX_EPISODES,
        });
    }
    validate_execution_source(&input.execution_source)?;

    let mut episode_ids = BTreeSet::new();
    for episode in &input.episodes {
        validate_episode(
            input.scenario,
            episode,
            input.schema_version == INTERACTION_SCENARIO_INPUT_VERSION,
        )?;
        if !episode_ids.insert(episode.episode_id.as_str()) {
            return Err(ScenarioReportError::DuplicateEpisodeId(
                episode.episode_id.clone(),
            ));
        }
    }
    Ok(())
}

fn validate_execution_source(source: &ScenarioExecutionSource) -> Result<(), ScenarioReportError> {
    let ScenarioExecutionSource::StrictLegalActionEngine {
        engine_id,
        engine_version,
        legal_action_schema_version,
    } = source
    else {
        return Ok(());
    };
    for (field, value) in [
        ("engineId", engine_id),
        ("engineVersion", engine_version),
        ("legalActionSchemaVersion", legal_action_schema_version),
    ] {
        if value.trim().is_empty() {
            return Err(ScenarioReportError::InvalidExecutionSource(format!(
                "{field} cannot be empty"
            )));
        }
        if value.len() > MAX_ENGINE_FIELD_BYTES {
            return Err(ScenarioReportError::InvalidExecutionSource(format!(
                "{field} exceeds {MAX_ENGINE_FIELD_BYTES} bytes"
            )));
        }
    }
    Ok(())
}

fn validate_episode(
    scenario: InteractionScenario,
    episode: &ScenarioEpisodeInput,
    require_resolved_table_win: bool,
) -> Result<(), ScenarioReportError> {
    if episode.episode_id.trim().is_empty() {
        return Err(ScenarioReportError::InvalidEpisodeId {
            episode_id: episode.episode_id.clone(),
            reason: "the id cannot be empty".into(),
        });
    }
    if episode.episode_id.len() > MAX_EPISODE_ID_BYTES {
        return Err(ScenarioReportError::InvalidEpisodeId {
            episode_id: episode.episode_id.clone(),
            reason: format!("the id exceeds {MAX_EPISODE_ID_BYTES} bytes"),
        });
    }
    if episode.turn_cap == 0 || episode.turn_cap > MAX_TURN_CAP {
        return Err(ScenarioReportError::InvalidTurnCap {
            episode_id: episode.episode_id.clone(),
            turn_cap: episode.turn_cap,
        });
    }
    validate_outcome(
        &episode.episode_id,
        "baseline",
        &episode.baseline,
        episode.turn_cap,
        require_resolved_table_win,
    )?;
    validate_outcome(
        &episode.episode_id,
        "stressed",
        &episode.stressed,
        episode.turn_cap,
        require_resolved_table_win,
    )?;
    validate_event_counters(&episode.episode_id, &episode.events)?;

    match &episode.applicability {
        ScenarioApplicability::Applicable => {}
        ScenarioApplicability::NotApplicable { reason } => {
            if reason.scenario() != scenario {
                return Err(ScenarioReportError::InapplicabilityMismatch {
                    episode_id: episode.episode_id.clone(),
                    scenario,
                    reason: *reason,
                });
            }
            validate_inactive_episode(episode, "not-applicable")?;
        }
        ScenarioApplicability::Undetermined { reason } => {
            if reason.trim().is_empty() || reason.len() > MAX_UNSUPPORTED_REASON_BYTES {
                return Err(ScenarioReportError::InvalidApplicability {
                    episode_id: episode.episode_id.clone(),
                    reason: format!(
                        "undetermined reason must contain 1..={MAX_UNSUPPORTED_REASON_BYTES} bytes"
                    ),
                });
            }
            validate_inactive_episode(episode, "undetermined")?;
        }
    }

    let effectful = episode.events.effectful_interventions();
    if effectful == 0 {
        if episode.baseline != episode.stressed {
            return Err(ScenarioReportError::NoOpChangedOutcome {
                episode_id: episode.episode_id.clone(),
            });
        }
        if episode.recovery.is_some() {
            return Err(ScenarioReportError::InvalidRecovery {
                episode_id: episode.episode_id.clone(),
                reason: "recovery must be null when no effectful intervention occurred".into(),
            });
        }
    } else {
        let recovery =
            episode
                .recovery
                .as_ref()
                .ok_or_else(|| ScenarioReportError::InvalidRecovery {
                    episode_id: episode.episode_id.clone(),
                    reason: "an effectful intervention requires an observed or censored recovery"
                        .into(),
                })?;
        validate_recovery(&episode.episode_id, recovery, episode.turn_cap)?;
    }
    Ok(())
}

fn validate_inactive_episode(
    episode: &ScenarioEpisodeInput,
    status: &'static str,
) -> Result<(), ScenarioReportError> {
    if !episode.events.is_zero() {
        return Err(ScenarioReportError::InvalidApplicability {
            episode_id: episode.episode_id.clone(),
            reason: format!("{status} episodes must have zero event counters"),
        });
    }
    Ok(())
}

fn validate_outcome(
    episode_id: &str,
    prefix: &'static str,
    outcome: &EpisodeOutcomeInput,
    turn_cap: u16,
    require_resolved_table_win: bool,
) -> Result<(), ScenarioReportError> {
    let (threat_field, attempt_field, resolved_field) = match prefix {
        "baseline" => (
            "baseline.credibleThreat",
            "baseline.firstWinAttempt",
            "baseline.resolvedTableWin",
        ),
        _ => (
            "stressed.credibleThreat",
            "stressed.firstWinAttempt",
            "stressed.resolvedTableWin",
        ),
    };
    validate_censored_turn(episode_id, threat_field, &outcome.credible_threat, turn_cap)?;
    validate_censored_turn(
        episode_id,
        attempt_field,
        &outcome.first_win_attempt,
        turn_cap,
    )?;
    if !require_resolved_table_win && outcome.resolved_table_win.is_some() {
        return Err(ScenarioReportError::InvalidTurnObservation {
            episode_id: episode_id.into(),
            field: resolved_field,
            reason: format!(
                "the endpoint must be omitted by legacy schema {LEGACY_INTERACTION_SCENARIO_INPUT_VERSION}"
            ),
        });
    }
    if let Some(resolved_table_win) = &outcome.resolved_table_win {
        validate_censored_turn(episode_id, resolved_field, resolved_table_win, turn_cap)?;
    } else if require_resolved_table_win {
        return Err(ScenarioReportError::InvalidTurnObservation {
            episode_id: episode_id.into(),
            field: resolved_field,
            reason: format!(
                "the endpoint is required by schema {INTERACTION_SCENARIO_INPUT_VERSION}"
            ),
        });
    }

    validate_endpoint_order(
        episode_id,
        threat_field,
        &outcome.credible_threat,
        attempt_field,
        &outcome.first_win_attempt,
    )?;
    if let Some(resolved_table_win) = &outcome.resolved_table_win {
        validate_endpoint_order(
            episode_id,
            attempt_field,
            &outcome.first_win_attempt,
            resolved_field,
            resolved_table_win,
        )?;
    }
    Ok(())
}

fn validate_endpoint_order(
    episode_id: &str,
    upstream_field: &'static str,
    upstream: &CensoredTurn,
    downstream_field: &'static str,
    downstream: &CensoredTurn,
) -> Result<(), ScenarioReportError> {
    match (upstream, downstream) {
        (
            CensoredTurn::Observed {
                turn: upstream_turn,
            },
            CensoredTurn::Observed {
                turn: downstream_turn,
            },
        ) if downstream_turn < upstream_turn => Err(ScenarioReportError::InvalidTurnObservation {
            episode_id: episode_id.into(),
            field: downstream_field,
            reason: format!(
                "observed turn {downstream_turn} precedes {upstream_field} at turn \
                     {upstream_turn}"
            ),
        }),
        (CensoredTurn::RightCensored { at_turn }, CensoredTurn::Observed { turn }) => {
            Err(ScenarioReportError::InvalidTurnObservation {
                episode_id: episode_id.into(),
                field: downstream_field,
                reason: format!(
                    "cannot be observed at turn {turn} when {upstream_field} was not observed through \
                 turn {at_turn}"
                ),
            })
        }
        _ => Ok(()),
    }
}

fn validate_censored_turn(
    episode_id: &str,
    field: &'static str,
    observation: &CensoredTurn,
    turn_cap: u16,
) -> Result<(), ScenarioReportError> {
    match observation {
        CensoredTurn::Observed { turn } if *turn == 0 || *turn > turn_cap => {
            Err(ScenarioReportError::InvalidTurnObservation {
                episode_id: episode_id.into(),
                field,
                reason: format!("observed turn must be in 1..={turn_cap}"),
            })
        }
        CensoredTurn::RightCensored { at_turn } if *at_turn != turn_cap => {
            Err(ScenarioReportError::InvalidTurnObservation {
                episode_id: episode_id.into(),
                field,
                reason: format!(
                    "right-censor turn {at_turn} must equal the episode turn cap {turn_cap}"
                ),
            })
        }
        _ => Ok(()),
    }
}

fn validate_recovery(
    episode_id: &str,
    observation: &RecoveryObservation,
    turn_cap: u16,
) -> Result<(), ScenarioReportError> {
    match observation {
        RecoveryObservation::Recovered { turn } if *turn == 0 || *turn > turn_cap => {
            Err(ScenarioReportError::InvalidRecovery {
                episode_id: episode_id.into(),
                reason: format!("recovery turn must be in 1..={turn_cap}"),
            })
        }
        RecoveryObservation::RightCensored { at_turn } if *at_turn != turn_cap => {
            Err(ScenarioReportError::InvalidRecovery {
                episode_id: episode_id.into(),
                reason: format!(
                    "right-censor turn {at_turn} must equal the episode turn cap {turn_cap}"
                ),
            })
        }
        _ => Ok(()),
    }
}

fn validate_event_counters(
    episode_id: &str,
    events: &ScenarioEventCounters,
) -> Result<(), ScenarioReportError> {
    for (field, value) in [
        ("checkpointMatches", events.checkpoint_matches),
        ("opportunities", events.opportunities),
        ("directiveAttempts", events.directive_attempts),
        ("directiveApplied", events.directive_applied),
        ("directiveRejected", events.directive_rejected),
        ("directiveNoOps", events.directive_no_ops),
    ] {
        if value > 1 {
            return Err(ScenarioReportError::InvalidEventCounters {
                episode_id: episode_id.into(),
                reason: format!("{field} must be a binary count for a first-occurrence directive"),
            });
        }
    }
    if events.checkpoint_matches != events.opportunities {
        return Err(ScenarioReportError::InvalidEventCounters {
            episode_id: episode_id.into(),
            reason: "checkpointMatches must equal opportunities in directive v1".into(),
        });
    }
    if events.directive_attempts > events.opportunities {
        return Err(ScenarioReportError::InvalidEventCounters {
            episode_id: episode_id.into(),
            reason: "directiveAttempts cannot exceed opportunities".into(),
        });
    }
    if events.directive_attempts != events.directive_applied + events.directive_rejected {
        return Err(ScenarioReportError::InvalidEventCounters {
            episode_id: episode_id.into(),
            reason: "directiveAttempts must equal directiveApplied + directiveRejected".into(),
        });
    }
    if events.directive_no_ops > events.directive_applied {
        return Err(ScenarioReportError::InvalidEventCounters {
            episode_id: episode_id.into(),
            reason: "directiveNoOps cannot exceed directiveApplied".into(),
        });
    }
    let effectful = events.effectful_interventions();
    if effectful == 0 && events.affected_game_events != 0 {
        return Err(ScenarioReportError::InvalidEventCounters {
            episode_id: episode_id.into(),
            reason: "affectedGameEvents must be zero without an effectful intervention".into(),
        });
    }
    if effectful > 0 && events.affected_game_events == 0 {
        return Err(ScenarioReportError::InvalidEventCounters {
            episode_id: episode_id.into(),
            reason: "an effectful intervention must record at least one affected game event".into(),
        });
    }
    Ok(())
}

fn measurement_descriptor(source: ScenarioExecutionSource) -> MeasurementDescriptor {
    match source {
        source @ ScenarioExecutionSource::ResponsePressure => MeasurementDescriptor {
            label: RESPONSE_PRESSURE_LABEL.into(),
            execution_source: source,
            claim_boundary: "Modeled pressure response only; this report does not establish that \
                             the intervention was a legal Magic action."
                .into(),
        },
        source @ ScenarioExecutionSource::StrictLegalActionEngine { .. } => MeasurementDescriptor {
            label: STRICT_LEGAL_ACTION_LABEL.into(),
            execution_source: source,
            claim_boundary: "Scenario actions and targets were supplied by the identified \
                                 strict legal-action engine; outcome accuracy remains an empirical \
                                 validation question."
                .into(),
        },
    }
}

fn episode_disposition(episode: &ScenarioEpisodeInput) -> EpisodeDisposition {
    match episode.applicability {
        ScenarioApplicability::NotApplicable { .. } => EpisodeDisposition::NotApplicable,
        ScenarioApplicability::Undetermined { .. } => EpisodeDisposition::Undetermined,
        ScenarioApplicability::Applicable if episode.events.opportunities == 0 => {
            EpisodeDisposition::ApplicableNoOpportunity
        }
        ScenarioApplicability::Applicable if episode.events.directive_attempts == 0 => {
            EpisodeDisposition::ApplicableOpportunityUnexercised
        }
        ScenarioApplicability::Applicable if episode.events.directive_rejected > 0 => {
            EpisodeDisposition::ApplicableDirectiveRejected
        }
        ScenarioApplicability::Applicable if episode.events.effectful_interventions() == 0 => {
            EpisodeDisposition::ApplicableDirectiveNoOp
        }
        ScenarioApplicability::Applicable => EpisodeDisposition::EffectfulInterventionApplied,
    }
}

fn aggregate_counters(episodes: &[ScenarioEpisodeInput]) -> AggregateScenarioCounters {
    let mut result = AggregateScenarioCounters {
        total_episodes: episodes.len() as u64,
        ..Default::default()
    };
    for episode in episodes {
        match episode.applicability {
            ScenarioApplicability::Applicable => result.applicable_episodes += 1,
            ScenarioApplicability::NotApplicable { .. } => {
                result.not_applicable_episodes += 1;
            }
            ScenarioApplicability::Undetermined { .. } => result.undetermined_episodes += 1,
        }
        if matches!(episode.applicability, ScenarioApplicability::Applicable) {
            if episode.events.opportunities == 0 {
                result.applicable_without_opportunity_episodes += 1;
            } else {
                result.opportunity_episodes += 1;
            }
        }
        result.checkpoint_events += u64::from(episode.events.checkpoint_matches);
        result.opportunity_events += u64::from(episode.events.opportunities);
        result.directive_attempt_events += u64::from(episode.events.directive_attempts);
        result.directive_applied_events += u64::from(episode.events.directive_applied);
        result.directive_rejected_events += u64::from(episode.events.directive_rejected);
        result.directive_no_op_events += u64::from(episode.events.directive_no_ops);
        result.affected_game_events += u64::from(episode.events.affected_game_events);
        if episode.events.effectful_interventions() > 0 {
            result.effectful_intervention_episodes += 1;
        }
    }
    result
}

fn build_delay_distribution(
    episodes: &[ScenarioEpisodeInput],
    metric: DelayMetric,
) -> Option<PairedDelayDistribution> {
    let mut observations = Vec::with_capacity(episodes.len());
    let mut applicable_pairs = 0u32;
    let mut effectful_pairs = 0u32;
    let mut observed_pairs = 0u32;
    let mut right_censored_pairs = 0u32;
    let mut no_op_invariant_pairs = 0u32;
    let mut non_estimable_pairs = 0u32;
    let mut excluded_pairs = 0u32;
    let mut observed_delays = Vec::new();

    for episode in episodes {
        let (baseline, stressed) = match metric {
            DelayMetric::CredibleThreat => (
                Some(episode.baseline.credible_threat.clone()),
                Some(episode.stressed.credible_threat.clone()),
            ),
            DelayMetric::FirstWinAttempt => (
                Some(episode.baseline.first_win_attempt.clone()),
                Some(episode.stressed.first_win_attempt.clone()),
            ),
            DelayMetric::ResolvedTableWin => (
                episode.baseline.resolved_table_win.clone(),
                episode.stressed.resolved_table_win.clone(),
            ),
        };
        let (Some(baseline), Some(stressed)) = (baseline, stressed) else {
            return None;
        };
        let effectful = episode.events.effectful_interventions() > 0;
        let value = match episode.applicability {
            ScenarioApplicability::NotApplicable { .. } => {
                excluded_pairs += 1;
                PairedDelayValue::Excluded {
                    reason: DelayExclusionReason::NotApplicable,
                }
            }
            ScenarioApplicability::Undetermined { .. } => {
                excluded_pairs += 1;
                PairedDelayValue::Excluded {
                    reason: DelayExclusionReason::ApplicabilityUndetermined,
                }
            }
            ScenarioApplicability::Applicable if !effectful => {
                applicable_pairs += 1;
                no_op_invariant_pairs += 1;
                PairedDelayValue::NoOpInvariant { delay_turns: 0 }
            }
            ScenarioApplicability::Applicable => {
                applicable_pairs += 1;
                effectful_pairs += 1;
                match (&baseline, &stressed) {
                    (
                        CensoredTurn::Observed {
                            turn: baseline_turn,
                        },
                        CensoredTurn::Observed {
                            turn: stressed_turn,
                        },
                    ) => {
                        observed_pairs += 1;
                        let delay = i32::from(*stressed_turn) - i32::from(*baseline_turn);
                        observed_delays.push(delay);
                        PairedDelayValue::Observed { delay_turns: delay }
                    }
                    (
                        CensoredTurn::Observed {
                            turn: baseline_turn,
                        },
                        CensoredTurn::RightCensored { at_turn },
                    ) => {
                        right_censored_pairs += 1;
                        PairedDelayValue::RightCensored {
                            greater_than_turns: i32::from(*at_turn) - i32::from(*baseline_turn),
                        }
                    }
                    (CensoredTurn::RightCensored { .. }, _) => {
                        non_estimable_pairs += 1;
                        PairedDelayValue::NonEstimable {
                            reason: NonEstimableDelayReason::BaselineRightCensored,
                        }
                    }
                }
            }
        };
        observations.push(PairedDelayObservation {
            episode_id: episode.episode_id.clone(),
            baseline,
            stressed,
            value,
        });
    }

    observed_delays.sort_unstable();
    Some(PairedDelayDistribution {
        metric,
        total_episode_pairs: episodes.len() as u32,
        applicable_pairs,
        effectful_pairs,
        observed_pairs,
        right_censored_pairs,
        no_op_invariant_pairs,
        non_estimable_pairs,
        excluded_pairs,
        median_observed_delay_turns: median_i32(&observed_delays),
        observations,
    })
}

fn median_i32(values: &[i32]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        Some(f64::from(values[middle]))
    } else {
        Some((f64::from(values[middle - 1]) + f64::from(values[middle])) / 2.0)
    }
}

fn percentile_i32(values: &[i32], probability: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let position = probability * (values.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position - lower as f64;
    Some(f64::from(values[lower]) * (1.0 - fraction) + f64::from(values[upper]) * fraction)
}

fn build_recovery_summary(episodes: &[ScenarioEpisodeInput]) -> RecoverySummary {
    let mut observations = Vec::new();
    let mut recovered = 0u32;
    let mut right_censored = 0u32;
    for episode in episodes {
        let Some(observation) = episode.recovery.clone() else {
            continue;
        };
        match observation {
            RecoveryObservation::Recovered { .. } => recovered += 1,
            RecoveryObservation::RightCensored { .. } => right_censored += 1,
        }
        observations.push(EpisodeRecoveryReport {
            episode_id: episode.episode_id.clone(),
            observation,
        });
    }
    let opportunities = observations.len() as u32;
    RecoverySummary {
        opportunities,
        recovered,
        right_censored,
        recovered_by_turn_cap_rate: (opportunities > 0)
            .then(|| f64::from(recovered) / f64::from(opportunities)),
        observations,
    }
}
