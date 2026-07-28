use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeckEntry {
    pub quantity: u16,
    pub name: String,
    pub line_number: usize,
    pub is_commander: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeckIssue {
    pub severity: IssueSeverity,
    pub code: String,
    pub message: String,
    pub line_number: Option<usize>,
    pub card_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckParseResult {
    pub entries: Vec<DeckEntry>,
    pub card_count: u32,
    pub unique_card_count: usize,
    pub ignored_line_count: usize,
    pub commanders: Vec<String>,
    pub issues: Vec<DeckIssue>,
    pub canonical_text: String,
    pub is_commander_sized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisOptions {
    pub opening_hand_simulations: u32,
    pub game_simulations: u32,
    pub maximum_turn: u8,
    pub mulligan_policy: MulliganPolicy,
    pub pilot_policy: PilotPolicy,
    pub interaction_profile: InteractionProfile,
    #[serde(default)]
    pub declared_intent: DeckIntent,
    /// When false, unresolved card names remain local and are reported as
    /// coverage gaps instead of being sent to Scryfall's collection API.
    #[serde(default)]
    pub allow_online_card_resolution: bool,
    pub seed: Option<u64>,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            opening_hand_simulations: 1_000,
            game_simulations: 1_000,
            maximum_turn: 6,
            mulligan_policy: MulliganPolicy::Aggressive,
            pilot_policy: PilotPolicy::Race,
            interaction_profile: InteractionProfile::HighPower,
            declared_intent: DeckIntent::Unspecified,
            allow_online_card_resolution: false,
            seed: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MulliganPolicy {
    Conservative,
    Balanced,
    Aggressive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PilotPolicy {
    Balanced,
    Race,
    Protect,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InteractionProfile {
    None,
    Light,
    Typical,
    HighPower,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeckIntent {
    #[default]
    Unspecified,
    Exhibition,
    Social,
    Optimized,
    Cedh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeRequest {
    pub run_id: String,
    pub deck_text: String,
    #[serde(default)]
    pub commander_names: Vec<String>,
    #[serde(default)]
    pub options: AnalysisOptions,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgress {
    pub run_id: String,
    pub stage: AnalysisStage,
    pub stage_label: String,
    pub completed_units: u32,
    pub total_units: u32,
    pub overall_progress: f32,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisStage {
    Validating,
    ResolvingCards,
    Compiling,
    OpeningHands,
    Goldfish,
    Interference,
    Scoring,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisReport {
    pub run_id: String,
    pub deck: AnalyzedDeckSummary,
    pub recommendation: BracketRecommendation,
    pub overview: OverviewMetrics,
    pub opening_hands: OpeningHandReport,
    pub win_speed: WinSpeedReport,
    pub synergy: SynergyReport,
    pub coverage: CoverageReport,
    pub evidence: Vec<EvidenceItem>,
    #[serde(default)]
    pub policy: crate::rules::PolicyEvaluation,
    pub assumptions: AnalysisAssumptions,
    pub versions: DataVersions,
    pub cache: AnalysisCacheInfo,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzedDeckSummary {
    pub card_count: u32,
    pub unique_card_count: usize,
    pub commanders: Vec<String>,
    pub resolved_cards: u32,
    pub unresolved_cards: Vec<String>,
    #[serde(default)]
    pub canonical_deck: String,
    #[serde(default)]
    pub canonical_deck_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BracketRecommendation {
    pub likely_bracket: u8,
    pub range_low: u8,
    pub range_high: u8,
    pub confidence: ConfidenceLevel,
    pub rules_floor: Option<u8>,
    pub probabilities: Vec<BracketProbability>,
    pub summary: String,
    #[serde(default)]
    pub calibration_status: CalibrationStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfidenceLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CalibrationStatus {
    #[default]
    Uncalibrated,
    EmpiricallyCalibrated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BracketProbability {
    pub bracket: u8,
    pub probability: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewMetrics {
    pub mana_score: u8,
    pub consistency_score: u8,
    pub speed_score: u8,
    /// Identifies the evidence source selected for the overview speed score.
    /// This remains separate from the explicit win-attempt endpoint so broad
    /// development or setup evidence cannot be mislabeled as a win.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_score_basis: Option<SpeedScoreBasis>,
    pub interaction_score: u8,
    pub synergy_score: u8,
    pub resilience_score: u8,
    pub commander_on_curve_rate: f32,
    /// Compatibility field: a conservative kept-hand probability proxy based
    /// on cards specifically tied to the highest-confidence detected plan.
    /// This is not proof that a complete line is available or executable.
    pub primary_plan_access_rate: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpeedScoreBasis {
    RecognizedWinAttempt,
    GenericConversionMilestone,
    ProactiveDevelopment,
    CredibleThreat,
    StructuralPace,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpeningHandReport {
    pub simulations: u32,
    /// Versioned digest contract for the complete nine-candidate London
    /// cohort generated for every opening-hand episode.
    #[serde(default)]
    pub candidate_cohort_version: String,
    /// SHA-256 over every full shuffled library in the candidate cohort,
    /// encoded by normalized card identity rather than compiled indices.
    #[serde(default)]
    pub candidate_cohort_sha256: String,
    pub keepable_seven_rate: f32,
    pub keepable_after_mulligans_rate: f32,
    pub average_mulligans: f32,
    pub average_cards_kept: f32,
    pub two_land_rate: f32,
    pub three_land_by_turn_three_rate: f32,
    pub ramp_access_rate: f32,
    pub engine_access_rate: f32,
    pub confidence_margin: f32,
    /// Sampling follows the London procedure, while keep/bottom decisions
    /// currently use strategic-role heuristics rather than a complete
    /// executable look-ahead policy.
    #[serde(default)]
    pub policy_fidelity: SimulationFidelity,
    #[serde(default)]
    pub mana: ManaAnalysisReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManaAnalysisReport {
    pub reliability_band: ManaReliabilityBand,
    pub reliability_score: f32,
    pub model_confidence: f32,
    pub average_opening_color_coverage: f32,
    pub average_turn_three_color_coverage: f32,
    pub land_source_count: u32,
    pub nonland_source_count: u32,
    pub conditional_source_count: u32,
    pub unknown_source_count: u32,
    pub enters_tapped_land_count: u32,
    pub colors: Vec<ManaColorSourceReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ManaReliabilityBand {
    #[default]
    Unknown,
    Fragile,
    Mixed,
    Supported,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManaColorSourceReport {
    pub color: String,
    pub exact_sources: u32,
    pub conditional_sources: u32,
    pub tapped_sources: u32,
    pub weighted_source_equivalents: f32,
    pub demand_pip_appearances: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WinSpeedReport {
    pub simulations: u32,
    #[serde(default)]
    pub fidelity: SimulationFidelity,
    #[serde(default)]
    pub fidelity_message: String,
    #[serde(default)]
    pub coverage_manifest_sha256: Option<String>,
    /// Versioned meaning of the serialized timing endpoints. `None` identifies
    /// a legacy report whose absent resolved-win fields must remain unknown;
    /// consumers must never synthesize them from modeled win attempts.
    #[serde(default)]
    pub timing_endpoint_version: Option<String>,
    /// First turn the bounded model considers the deck able to present a
    /// credible threat. These existing fields are retained for scoring and
    /// cache compatibility; they do not represent wins.
    pub baseline: TurnDistribution,
    pub interfered: TurnDistribution,
    /// First turn the bounded model presents either a recognized reviewed
    /// table-lethal line or a rules-backed damage assignment that would
    /// eliminate every remaining opponent if its damage connects. This is an
    /// explicit attempt endpoint, not a generic engine heuristic or
    /// multiplayer win percentage.
    #[serde(default)]
    pub baseline_win_attempt: TurnDistribution,
    #[serde(default)]
    pub interfered_win_attempt: TurnDistribution,
    /// Overview-only pace signal. Per episode this is the earlier of a
    /// recognized explicit attempt and the separately named broad
    /// engine/combat development milestone. It does not change either
    /// endpoint's meaning and is not a resolved-win claim.
    #[serde(default)]
    pub baseline_model_pace: TurnDistribution,
    #[serde(default)]
    pub interfered_model_pace: TurnDistribution,
    /// First turn a fully typed table-lethal conversion is proven to have
    /// resolved. The legacy structural simulator does not receive authority to
    /// populate this endpoint. `None` means the report predates this contract,
    /// while `Some(default distribution)` means no resolution was demonstrated
    /// by the turn cap.
    #[serde(default)]
    pub baseline_resolved_table_win: Option<TurnDistribution>,
    #[serde(default)]
    pub interfered_resolved_table_win: Option<TurnDistribution>,
    #[serde(default)]
    pub median_delay: Option<f32>,
    #[serde(default)]
    pub win_attempt_median_delay: Option<f32>,
    #[serde(default)]
    pub resolved_table_win_median_delay: Option<f32>,
    /// Pairwise delay observations retain the baseline and stressed result for
    /// the same sampled deck order. Runs where the baseline demonstrated an
    /// endpoint but the stressed run did not are right-censored instead of
    /// being discarded or converted into an invented turn.
    #[serde(default)]
    pub paired_threat_delay: PairedTurnDelayReport,
    #[serde(default)]
    pub paired_win_attempt_delay: PairedTurnDelayReport,
    #[serde(default)]
    pub paired_resolved_table_win_delay: Option<PairedTurnDelayReport>,
    #[serde(default)]
    pub first_attempt_opportunities: u32,
    pub first_attempt_stopped_rate: f32,
    #[serde(default)]
    pub recovery_opportunities: u32,
    #[serde(default)]
    pub recovered_attempts: u32,
    /// None means no stopped attempt created a recovery opportunity. It must
    /// never be interpreted as a perfect recovery rate.
    #[serde(default)]
    pub recovery_by_max_turn_rate: Option<f32>,
    pub cumulative_threat_rate: Vec<TurnRate>,
    #[serde(default)]
    pub cumulative_interfered_threat_rate: Vec<TurnRate>,
    #[serde(default)]
    pub cumulative_win_attempt_rate: Vec<TurnRate>,
    #[serde(default)]
    pub cumulative_interfered_win_attempt_rate: Vec<TurnRate>,
    /// First turn a broad engine/combat density heuristic reaches its
    /// conversion-shaped milestone. This is deliberately separate from
    /// `baseline_win_attempt`: no value in this endpoint is a recognized
    /// explicit route, an attempted win, or a resolved win.
    #[serde(default)]
    pub baseline_generic_conversion_milestone: TurnDistribution,
    #[serde(default)]
    pub interfered_generic_conversion_milestone: TurnDistribution,
    #[serde(default)]
    pub cumulative_generic_conversion_milestone_rate: Vec<TurnRate>,
    #[serde(default)]
    pub cumulative_interfered_generic_conversion_milestone_rate: Vec<TurnRate>,
    /// Identifies the reviewed known-line or rules-backed combat routes behind
    /// explicit attempts and aggregates bounded early-turn reasons why no such
    /// route was presented.
    /// This is diagnostic provenance, not a claim that every possible deck
    /// route has been recognized.
    #[serde(default)]
    pub attempt_provenance: AttemptProvenanceReport,
    /// Exact T1/T2 combination-weighted access diagnostics for recognized
    /// table-lethal routes under the one fixed aggressive policy. These
    /// route-skeleton probabilities are deliberately not attempt or win
    /// probabilities until an ordered executor supplies a legal witness.
    #[serde(default)]
    pub early_turn_evaluation: Option<crate::early_turn_evaluator::EarlyTurnEvaluationReport>,
    #[serde(default)]
    pub cumulative_resolved_table_win_rate: Option<Vec<TurnRate>>,
    #[serde(default)]
    pub cumulative_interfered_resolved_table_win_rate: Option<Vec<TurnRate>>,
    /// Canonical paired interaction scenarios. These remain response-pressure
    /// measurements until their actions are supplied by a strict legal-action
    /// engine.
    #[serde(default)]
    pub interaction_scenarios: Vec<crate::interaction_scenarios::CompactScenarioReport>,
    pub stress_tests: Vec<StressTestResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AttemptProvenanceReport {
    /// One entry per recognized table-lethal known line plus any executable
    /// rules-backed combat route, in deterministic order. Generic
    /// engine/combat milestones never appear in this list.
    pub explicit_routes: Vec<ExplicitAttemptRouteReport>,
    /// Counts of the first broad heuristic milestone by kind. These remain
    /// explicitly non-attempt diagnostics.
    pub generic_milestone_kinds: Vec<GenericMilestoneKindReport>,
    /// The bounded turn through which route blockers were sampled.
    pub early_failure_horizon: u8,
    /// One deterministic best-progress blocker per still-at-risk episode and
    /// turn. Rates use all simulated episodes as the denominator.
    pub early_turn_blockers: Vec<EarlyTurnAttemptBlockerReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplicitAttemptRouteReport {
    pub route_id: String,
    pub name: String,
    pub cards: Vec<String>,
    pub prerequisites: Vec<String>,
    pub model_confidence: f32,
    pub baseline_attempts: u32,
    pub interfered_attempts: u32,
    pub baseline_rate: f32,
    pub interfered_rate: f32,
    pub baseline_first_attempt: TurnDistribution,
    pub interfered_first_attempt: TurnDistribution,
    /// Per-turn cumulative incidence for this exact route. The aggregate
    /// distribution alone cannot identify whether a small early cohort came
    /// from Oracle, Pact, or another reviewed route.
    #[serde(default)]
    pub cumulative_baseline_attempt_rate: Vec<TurnRate>,
    #[serde(default)]
    pub cumulative_interfered_attempt_rate: Vec<TurnRate>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum GenericMilestoneKind {
    #[default]
    Engine,
    Combat,
    EngineAndCombat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericMilestoneKindReport {
    pub kind: GenericMilestoneKind,
    pub baseline_episodes: u32,
    pub interfered_episodes: u32,
    pub baseline_rate: f32,
    pub interfered_rate: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum TimingSampleKind {
    #[default]
    Baseline,
    Interfered,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ExplicitAttemptBlockerReason {
    #[default]
    NoRecognizedExplicitRoute,
    MissingNamedPieces,
    NamedPiecesNotUsableTogether,
    InsufficientNamedCardMana,
    UnsupportedRequirement,
    UnmetPrerequisite,
    UnsupportedActivationCost,
    InsufficientActivationMana,
    DeferredAfterStoppedAttempt,
    ReadyButNotSelected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EarlyTurnAttemptBlockerReport {
    pub sample: TimingSampleKind,
    pub turn: u8,
    /// Absent only when the deck contains no recognized explicit route.
    pub route_id: Option<String>,
    pub route_name: Option<String>,
    /// First named route card that was unavailable in the modeled usable
    /// zones. Absent for sequencing, prerequisite, and mana blockers.
    pub blocked_card: Option<String>,
    pub reason: ExplicitAttemptBlockerReason,
    pub episodes: u32,
    pub rate: f32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SimulationFidelity {
    StrictExecutable,
    #[default]
    LegacyHeuristic,
    BlockedUnsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PairedTurnDelayReport {
    pub observed_pairs: u32,
    pub prevented_by_turn_cap: u32,
    pub baseline_not_demonstrated: u32,
    pub stressed_only: u32,
    pub median: Option<f32>,
    pub p10: Option<f32>,
    pub p90: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TurnDistribution {
    /// Population quantiles across every simulated episode. A quantile is
    /// absent when too many episodes are right-censored at the turn cap to
    /// identify it.
    pub median: Option<f32>,
    pub p10: Option<f32>,
    pub p90: Option<f32>,
    /// Successful-episode-only diagnostics. These are deliberately separate
    /// from the population quantiles so a rare fast result cannot masquerade
    /// as the deck's expected timing.
    #[serde(default)]
    pub conditional_median: Option<f32>,
    #[serde(default)]
    pub conditional_p10: Option<f32>,
    #[serde(default)]
    pub conditional_p90: Option<f32>,
    pub demonstrated_rate: f32,
    #[serde(default)]
    pub right_censored_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnRate {
    pub turn: u8,
    pub rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StressTestResult {
    pub name: String,
    pub outcome: String,
    pub severity: IssueSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SynergyReport {
    pub detected_plans: Vec<StrategyPlan>,
    pub known_lines: Vec<KnownLine>,
    pub role_counts: Vec<RoleCount>,
    /// A report-only structural reading of posture, archetype, and combo
    /// families. It cannot affect simulation, scoring, or claims about the
    /// pilot's declared intent.
    #[serde(default)]
    pub strategic_profile: Option<crate::strategic_profile::StrategicProfileReport>,
    /// A report-only graph of explicit producer/payoff and trigger
    /// relationships compiled from typed Oracle-text profiles. The bounded
    /// simulator does not execute these links.
    #[serde(default)]
    pub graph: SynergyGraph,
    pub commander_dependence: f32,
    pub cohesion_score: u8,
    pub orphaned_cards: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SynergyGraph {
    pub model_version: String,
    pub ability_model_version: String,
    pub node_count: u16,
    pub connected_card_count: u16,
    pub edge_count: u32,
    pub displayed_edge_count: u16,
    pub graph_coverage: f32,
    pub unsupported_clause_count: u32,
    pub resources: Vec<SynergyResourceCoverage>,
    pub links: Vec<SynergyLink>,
    pub commander_links: Vec<SynergyLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynergyResourceCoverage {
    pub resource: String,
    pub producer_count: u16,
    pub consumer_count: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynergyLink {
    pub source_card: String,
    pub target_card: String,
    pub relation: SynergyRelation,
    pub resource: String,
    pub confidence: f32,
    pub evidence: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SynergyRelation {
    Provides,
    Triggers,
    KnownCombination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyPlan {
    pub name: String,
    pub confidence: f32,
    pub supporting_cards: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownLine {
    pub name: String,
    pub cards: Vec<String>,
    pub compactness: u8,
    pub is_infinite: bool,
    /// True only when the documented line itself is expected to convert the
    /// table if it resolves. Infinite mana or an unbounded engine without a
    /// payoff deliberately remains false.
    #[serde(default)]
    pub table_lethal_if_resolved: bool,
    #[serde(default)]
    pub outcome: KnownLineOutcome,
    #[serde(default)]
    pub mana_needed: Option<String>,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    #[serde(default)]
    pub model_confidence: f32,
    /// Machine-checkable conditions consumed by the bounded simulator.
    /// Human-readable `prerequisites` remain in the report; this internal
    /// representation is intentionally omitted from serialized reports.
    #[serde(skip)]
    pub simulation_requirements: Vec<LineRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineRequirement {
    AdditionalCreature {
        count: u8,
    },
    NonlandManaCapacity {
        minimum: u8,
    },
    /// Mana paid after all named line pieces have been cast/placed. This is
    /// deliberately distinct from catalog `mana_needed`, which may instead
    /// describe the total execution cost from particular starting zones.
    AdditionalActivationMana {
        cost: &'static str,
    },
    /// The source reports a total execution cost. The bounded simulator cannot
    /// safely subtract already-paid spell costs, so this remains report-only.
    TotalExecutionMana,
    /// Every named card is cast through the ordinary spell-cost path. The
    /// displayed mana value is descriptive total casting cost and must not be
    /// charged again as an activation.
    NamedCardsPayPrintedCosts,
    /// Reviewed bounded adapter: a same-turn permanent entry must precede the
    /// named library-exile spell. This records only the compact sequencing
    /// abstraction and is not a general stack, priority, trigger, or library
    /// executor.
    ReviewedEmptyLibrarySequence,
    /// The named permanents must expose a typed, repeatable state cycle that
    /// returns the source to its original tap state while strictly increasing
    /// mana. It is a threat, not a table win without a typed outlet.
    ReviewedInfiniteManaLoop,
    /// A typed escape permission, transactional self-sacrifice mana ability,
    /// mill spell, and Storm trigger must jointly prove the reviewed
    /// graveyard loop. Printed spell costs are paid separately.
    ExecutableGraveyardStormLoop,
    /// A typed "tap two untapped artifacts" activation, a typed Dwarf-tap
    /// Treasure trigger, and an artifact Dwarf jointly prove a repeatable
    /// positive Treasure cycle. The cycle is a resource threat until a typed
    /// table conversion is also present.
    ExecutableArtifactTapTreasureLoop,
    /// A typed artifact-entry trigger with optional self-untap, a typed
    /// tap-an-untapped-Dwarf activation cost, the typed Dwarf-tap Treasure
    /// trigger, and the exact battlefield-wide all-creature-types static
    /// scope jointly prove the reviewed Maskwood artifact-Dwarf cycle.
    /// Attachment-scoped or otherwise dynamic type grants do not satisfy it.
    ExecutableMaskwoodArtifactDwarfTreasureLoop,
    /// A reviewed infinite-colorless permanent cycle plus an exact variable-X
    /// creature tutor/haste-overrun spell can present a precombat attempt.
    /// The infinite loop supplies X while the ordinary mana model must still
    /// pay every fixed colored pip. Three attack-capable creatures must exist
    /// after the tutor resolves. This is never table-resolution proof because
    /// blockers, damage, and opponent life totals remain outside the bounded
    /// executor.
    ExecutableInfiniteManaCreatureOverrunAttempt,
    ExternalEnabler,
    SingletonLibrary,
    GraveyardSetup {
        minimum_cast_cards: u8,
    },
    CombatAccess,
    Unmodeled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum KnownLineOutcome {
    TableWin,
    InfiniteMana,
    InfiniteEngine,
    #[default]
    Engine,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleCount {
    pub role: String,
    pub count: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageReport {
    pub identity_resolution: f32,
    pub semantic_coverage: f32,
    pub simulation_coverage: f32,
    pub approximated_cards: Vec<String>,
    pub unresolved_cards: Vec<String>,
    pub notes: Vec<String>,
    #[serde(default)]
    pub execution_manifest: Option<crate::execution_coverage::CompactExecutionCoverageManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceItem {
    pub direction: EvidenceDirection,
    pub title: String,
    pub detail: String,
    pub weight: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceDirection {
    Raises,
    Lowers,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisAssumptions {
    pub opening_hand_simulations: u32,
    pub game_simulations: u32,
    pub maximum_turn: u8,
    pub mulligan_policy: MulliganPolicy,
    pub pilot_policy: PilotPolicy,
    pub interaction_profile: InteractionProfile,
    pub declared_intent: DeckIntent,
    #[serde(default)]
    pub allow_online_card_resolution: bool,
    /// Exact decimal representation for JavaScript/JSON consumers, where a
    /// `u64` numeric value may exceed the safe-integer range.
    #[serde(default)]
    pub seed_exact: String,
    pub seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataVersions {
    pub card_data: String,
    #[serde(default)]
    pub card_snapshot_sha256: Option<String>,
    #[serde(default)]
    pub rules_package: String,
    #[serde(default)]
    pub rules_snapshot_sha256: Option<String>,
    #[serde(default)]
    pub rules_package_origin: Option<String>,
    pub semantic_model: String,
    #[serde(default)]
    pub semantic_package: Option<String>,
    #[serde(default)]
    pub semantic_snapshot_sha256: Option<String>,
    #[serde(default)]
    pub semantic_package_origin: Option<String>,
    #[serde(default)]
    pub semantic_imported_at: Option<String>,
    #[serde(default)]
    pub semantic_authenticity_basis: Option<String>,
    #[serde(default)]
    pub comprehensive_rules_effective_date: Option<String>,
    #[serde(default)]
    pub comprehensive_rules_snapshot_sha256: Option<String>,
    #[serde(default)]
    pub comprehensive_rules_parser_version: Option<String>,
    #[serde(default)]
    pub rule_capability_model: Option<String>,
    #[serde(default)]
    pub strategic_profile_model: Option<String>,
    pub simulation_engine: String,
    #[serde(default)]
    pub effective_hand_strength_model: Option<String>,
    #[serde(default)]
    pub ability_program: Option<String>,
    #[serde(default)]
    pub turn_planner: Option<String>,
    #[serde(default)]
    pub strict_engine: Option<String>,
    #[serde(default)]
    pub execution_coverage_compiler: Option<String>,
    pub bracket_model: String,
    #[serde(default)]
    pub combo_catalog: Option<String>,
    #[serde(default)]
    pub combo_snapshot_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisCacheInfo {
    pub hit: bool,
    pub created_at: String,
    pub key_version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataStatus {
    pub state: DataState,
    pub card_count: u64,
    pub last_updated: Option<String>,
    pub source: String,
    pub message: String,
    pub snapshot_sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DataState {
    Ready,
    Partial,
    Empty,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataUpdateProgress {
    pub phase: String,
    pub completed_units: u64,
    pub total_units: Option<u64>,
    pub progress: f32,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub provider: String,
    pub deck_name: Option<String>,
    pub commanders: Vec<String>,
    pub deck_text: String,
    pub source_url: String,
    pub imported_at: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CardFaceDefinition {
    /// Face-specific Oracle identity. Reversible cards may omit the root
    /// `oracle_id` and expose distinct identities only on their faces.
    #[serde(default)]
    pub oracle_id: Option<String>,
    /// Face-specific Scryfall layout, retained because reversible faces can
    /// carry a layout even when the root object is structurally sparse.
    #[serde(default)]
    pub layout: String,
    pub name: String,
    /// Exact face mana value (`cmc` in Scryfall). `None` means the upstream
    /// object omitted it; it must not silently become zero.
    #[serde(default)]
    pub mana_value: Option<f32>,
    pub mana_cost: Option<String>,
    pub type_line: String,
    pub oracle_text: String,
    pub colors: Vec<String>,
    pub color_indicator: Vec<String>,
    /// Scryfall currently exposes keywords at card level. This field retains
    /// face-level keyword data if the upstream record supplies it, without
    /// guessing attribution from the combined list.
    pub keywords: Vec<String>,
    #[serde(default)]
    pub produced_mana: Vec<String>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
    #[serde(default)]
    pub hand_modifier: Option<String>,
    #[serde(default)]
    pub life_modifier: Option<String>,
    #[serde(default)]
    pub attraction_lights: Vec<u8>,
    pub image_uri: Option<String>,
    /// Fields not present in the reviewed upstream schema are retained
    /// losslessly. Coverage compilation blocks functional metrics until each
    /// field is explicitly classified in a later schema version.
    #[serde(default)]
    pub unreviewed_fields: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RelatedCardComponentDefinition {
    /// Scryfall card identifier for the related token, meld piece/result, or
    /// other component.
    pub id: String,
    /// Open string retained from Scryfall (`token`, `meld_part`,
    /// `meld_result`, `combo_piece`, or a future value).
    pub component: String,
    pub name: String,
    pub type_line: String,
    pub uri: Option<String>,
    /// Forward-compatible capture for future Scryfall component fields.
    #[serde(default)]
    pub unreviewed_fields: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CardDefinition {
    pub name: String,
    pub normalized_name: String,
    pub oracle_id: Option<String>,
    /// Scryfall layout such as `normal`, `split`, `transform`, `modal_dfc`,
    /// `adventure`, `meld`, or a future retained value.
    #[serde(default)]
    pub layout: String,
    /// Exact root-level Scryfall `cmc`. Reversible roots currently omit it, so
    /// absence is represented explicitly instead of being converted to zero.
    #[serde(default)]
    pub root_mana_value: Option<f32>,
    /// Compatibility/effective mana value used by legacy descriptive models.
    /// Strict execution coverage consumes `root_mana_value` and face values.
    pub mana_value: f32,
    pub mana_cost: Option<String>,
    pub type_line: String,
    pub oracle_text: String,
    #[serde(default)]
    pub colors: Vec<String>,
    #[serde(default)]
    pub color_indicator: Vec<String>,
    pub color_identity: Vec<String>,
    pub keywords: Vec<String>,
    #[serde(default)]
    pub produced_mana: Vec<String>,
    #[serde(default)]
    pub power: Option<String>,
    #[serde(default)]
    pub toughness: Option<String>,
    #[serde(default)]
    pub loyalty: Option<String>,
    #[serde(default)]
    pub defense: Option<String>,
    #[serde(default)]
    pub hand_modifier: Option<String>,
    #[serde(default)]
    pub life_modifier: Option<String>,
    #[serde(default)]
    pub attraction_lights: Vec<u8>,
    /// Exact per-face fields retained independently from legacy combined
    /// top-level strings.
    #[serde(default)]
    pub faces: Vec<CardFaceDefinition>,
    /// Related external game pieces retained by stable Scryfall identifier.
    #[serde(default)]
    pub related_components: Vec<RelatedCardComponentDefinition>,
    pub image_uri: Option<String>,
    /// Current Scryfall/Wizards Game Changer designation. This is policy and
    /// bracket context, not executable Oracle rules text.
    #[serde(default)]
    pub game_changer: Option<bool>,
    /// Exact value of `legalities.commander`, retained separately from the
    /// compatibility boolean so future legality values cannot be collapsed.
    #[serde(default)]
    pub commander_legality: Option<String>,
    pub legal_commander: bool,
    /// Unknown future root fields are losslessly retained and become explicit
    /// execution-coverage blockers until reviewed.
    #[serde(default)]
    pub unreviewed_fields: BTreeMap<String, serde_json::Value>,
    /// Version of the upstream-field classification used while this record
    /// was parsed. Empty/older values mean newly retained fields may have been
    /// lost and strict functional gates must remain blocked until refresh.
    #[serde(default)]
    pub source_schema_version: String,
    pub updated_at: String,
}
