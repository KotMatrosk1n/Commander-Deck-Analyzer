//! Bounded, deterministic execution over compiled card semantics.
//!
//! Production behavior is universal: card and deck names may identify
//! physical objects or catalogued line ingredients, but they must never
//! select a card's rules behavior. Executable behavior comes from normalized
//! card characteristics and typed ability/effect programs; unsupported
//! semantics fail closed. Named decks and cards belong only in regressions.

use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;

use rand::prelude::{Rng, SeedableRng, SliceRandom};
use rand_chacha::ChaCha8Rng;
use sha2::{Digest, Sha256};

use crate::ability_program::{
    AbilityCompilation, AbilityCost, AbilityEffect, AbilityPrecondition, AbilityTiming,
    ActivationWindow, AffectedPlayers, AlternativeSpellCostComponent, AtomicAdditionalCostTiming,
    AtomicBargainCost, AtomicCardNameReference, AtomicCastCostWaiver,
    AtomicCastPermissionCondition, AtomicCost, AtomicEffect, AtomicEffectDuration,
    AtomicGraveyardScope, AtomicInitiation, AtomicLibrarySearch, AtomicManaValueSubject,
    AtomicMovementCondition, AtomicSearchChooser, AtomicShuffleTiming, AtomicStateCondition,
    AtomicTrackedObject, AttachmentKind as ProgramAttachmentKind, BargainSacrificeKind,
    BargainSearchCastOrHandEffect, CardType as ProgramCardType, CastPermissionKind,
    CommanderEligibility, ControllerRelation, ControllerStateCondition, CopyTargetChoice,
    CounterKind, CounterTarget, CoupledLifeLoss, CreatureTokenKeyword, DelayedEvent,
    DelayedObjectReference, DiscardCost, DiscardedObjectReference, EntryLinkedCardFilter,
    EntryLinkedManaOutput, FixedManaProfile, GrantedAbilityKind, GrantedCreatureKeyword,
    GrantedSelfCost, LibraryPosition, LibraryRemainderPlacement,
    LibrarySelectionEffect as ProgramLibrarySelectionEffect, LinkedEntryObject,
    ManaCost as ProgramManaCost, ManaKind as ProgramManaKind, ManaPaymentAmount, ManaRetention,
    NecropotenceDiscardEvent, ObjectFilter as ProgramObjectFilter, OpponentChoiceSearchSplitEffect,
    OptionalManaPayment, PermanentEntryProcedure, PlayerSelector, RandomSelection,
    RepetitionPolicy, ReplacedSpellCost, ResourceKind, SelfTransferTutorActivationWindow,
    SelfTransferTutorCost, SelfTransferTutorResolutionStep, SpecificCardType, SpellCopyCount,
    SpellCostReductionCondition, SpellCostReductionScope, StaticCreatureModifierTarget,
    StaticModifierValue, StepProcedure, TargetSelector, TokenKind, TriggerAbilitySource,
    TriggerEventKind, TriggerMultiplierEvent, TurnStep, TutorEffect as ProgramTutorEffect,
    TutorFilter, UnsupportedReasonCode, Zone as ProgramZone,
};
use crate::combat_effects::{
    AttachmentConstraint as CombatAttachmentConstraint, AttachmentKind as CombatAttachmentKind,
    CombatEffectSet, CombatEffectState, CombatKeyword, ContinuousModifier, ControllerConstraint,
    CountedValue, CreatureType, CreatureTypeConstraint, DynamicValue, EffectId as CombatEffectId,
    EffectTarget, KeywordGrant, ObjectId, PermanentFilter, PermanentSnapshot, PermanentType,
    PlayerId, TriggerContext as CombatTriggerContext, TriggerEventKind as CombatTriggerEventKind,
    TriggerMultiplier as CombatTriggerMultiplier,
};
use crate::combat_terminal::{
    CombatAttacker, CommanderCombatState, ConnectedCombatDamage, OpponentId, PresentedAttack,
    allocate_attack,
};
use crate::domain::{
    AnalysisOptions, AttemptProvenanceReport, DeckIntent, EarlyTurnAttemptBlockerReport,
    ExplicitAttemptBlockerReason, ExplicitAttemptRouteReport, GenericMilestoneKind,
    GenericMilestoneKindReport, InteractionProfile, IssueSeverity, LineRequirement,
    ManaAnalysisReport, ManaReliabilityBand, MulliganPolicy, OpeningHandReport,
    PairedTurnDelayReport, PilotPolicy, StressTestResult, TimingSampleKind, TurnDistribution,
    TurnRate, WinSpeedReport,
};
use crate::effects::{
    EffectMagnitude, ManaProductionKind, TutorDestination, TutorInstruction, TutorSourceZone,
};
use crate::empty_library_win::{
    ReviewedLibraryExileReceipt, compile_reviewed_empty_library_win_program,
    execute_reviewed_library_exile_transaction,
};
use crate::interaction_scenarios::{
    CensoredTurn, CompactScenarioReport, EpisodeOutcomeInput, InapplicabilityReason,
    InteractionScenario, RecoveryObservation, ScenarioApplicability, ScenarioEpisodeInput,
    ScenarioEventCounters, ScenarioExecutionSource, ScenarioReportError, ScenarioReportInput,
    build_scenario_report, compact_scenario_report,
};
use crate::interference::{
    OpponentEventTimeline, OpponentSpellActivity, OpponentTurnActivity, TableActivityTimeline,
    TablePaymentDecision, TableTurnActivity, interaction_parameters,
};
use crate::mana::{
    EntersTapped, ManaColorMask, ManaCostFace, ManaCostProfile, ManaModel, ManaSourceProfile,
    analysis_report, parse_mana_cost,
};
use crate::semantics::{CompiledCard, CompiledDeck, role};
use crate::turn_event_state::{ControllerCondition as RuntimeControllerCondition, TurnEventState};
use crate::turn_planner::{
    PlannerConfig, PlannerValue, PlanningHorizon, TurnPlanningDomain, plan_turn,
};

pub(crate) const SIMULATION_ENGINE_VERSION: &str = "abstract-play-0.42";
pub(crate) const TIMING_ENDPOINT_VERSION: &str = "commander-timing-endpoints/v3";
pub(crate) const EFFECTIVE_HAND_STRENGTH_VERSION: &str = "mtg-effective-hand-strength/v4";
pub(crate) const MAX_INTERACTION_SCENARIO_EPISODES: u32 = 1_000;
const MIN_SCENARIO_SEMANTIC_COVERAGE: f32 = 0.70;
const WIN_SPEED_EPISODE_SEED_DOMAIN: u64 = 0x474f_4c44;
pub(crate) const INTERACTION_SCENARIO_SEED_DERIVATION_VERSION: &str =
    "splitmix64/master-gold-index/v1";
const CAST_PLANNER_BEAM_WIDTH: usize = 8;
const CAST_PLANNER_MAX_EXPANSIONS: usize = 96;
const CAST_PLANNER_MAX_ACTIONS: usize = 8;
const CAST_PLANNER_MAX_COMMITTED_ACTIONS: usize = 64;
const MAX_EPISODE_WORKERS: usize = 20;
const OPENING_EPISODE_BATCH_SIZE: u32 = 1_024;
const COMMANDER_STARTING_LIFE: f32 = 40.0;
const EARLY_ATTEMPT_DIAGNOSTIC_HORIZON: u8 = 6;
const COMBAT_DAMAGE_ROUTE_INDEX: usize = usize::MAX;
const AGGRESSIVE_DELAYED_ACCESS_MINIMUM_LIFE: f32 = 5.0;
const AGGRESSIVE_DELAYED_ACCESS_TARGET_CONFIDENCE: f64 = 0.85;
const MAXIMUM_CLEANUP_HAND_SIZE: usize = 7;
pub(crate) const OPENING_CANDIDATE_COHORT_VERSION: &str = "opening-candidate-cohort/v1";

#[derive(Debug, thiserror::Error)]
pub enum SimulationError {
    #[error("Analysis cancelled.")]
    Cancelled,
    #[error("The compiled deck has too few library cards to simulate.")]
    LibraryTooSmall,
    #[error("Could not build the paired interaction scenario suite: {0}")]
    InteractionScenario(#[from] ScenarioReportError),
}

#[derive(Debug, Default, Clone, Copy)]
struct HandEvaluation {
    lands: u8,
    ramp: u8,
    fast_mana: u8,
    executable_one_land_acceleration: u8,
    executable_zero_land_acceleration: u8,
    engines: u8,
    draw: u8,
    tutors: u8,
    cedh_hand_plans: u8,
    independent_hand_plans: u8,
    route_relevant_tutors: u8,
    early_actions: u8,
    meaningful_early_actions: u8,
    explicit_route_access: bool,
    reviewed_route_catalog_present: bool,
    command_zone_plan_access: bool,
    directly_payable_one_land_plan: bool,
    accelerated_one_land_plan: bool,
    accelerated_zero_land_plan: bool,
    color_requirements_known: bool,
    color_coverage: f32,
    color_floor: f32,
    effective_hand_strength_assessed: bool,
    effective_hand_strength: f32,
    effective_route_strength: f32,
    effective_mana_readiness: f32,
    effective_color_viability: f32,
}

#[derive(Debug, Default)]
struct ReviewedOpeningRouteAssessment {
    has_reviewed_routes: bool,
    direct_complete: bool,
    tutor_complete: bool,
    best_direct_fraction: f32,
    relevant_hand_tutors: HashSet<usize>,
    relevant_command_zone_tutors: HashSet<usize>,
}

#[derive(Debug, Clone, Copy)]
struct OpeningPersistentManaSource {
    origin_position: usize,
    behavior: OpeningManaBehavior,
    capacity: u8,
    available_from_turn: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpeningManaBehavior {
    Fixed(ManaColorMask),
    ControlledLegendaryColors,
    MetalcraftAnyColor,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct OpeningBoardState {
    artifacts: u8,
    legendary_creature_or_planeswalker_colors: ManaColorMask,
}

#[derive(Debug, Clone)]
struct LondonHandSample {
    hand: Vec<usize>,
    draw_order: Vec<usize>,
    initial_keepable: bool,
    accepted_by_policy: bool,
    paid_mulligans: u8,
}

#[derive(Debug)]
struct OpeningEpisodeResult {
    simulation_index: u32,
    candidate_orders: Vec<Vec<usize>>,
    initial_keepable: bool,
    accepted_by_policy: bool,
    paid_mulligans: u8,
    cards_kept: usize,
    kept_evaluation: HandEvaluation,
    turn_three_evaluation: HandEvaluation,
}

struct OpeningCandidateCohort {
    hasher: Sha256,
}

impl OpeningCandidateCohort {
    fn new(seed: u64, simulations: u32, library_size: usize) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(OPENING_CANDIDATE_COHORT_VERSION.as_bytes());
        hasher.update(seed.to_le_bytes());
        hasher.update(simulations.to_le_bytes());
        hasher.update((library_size as u64).to_le_bytes());
        Self { hasher }
    }

    fn record(&mut self, deck: &CompiledDeck, simulation_index: u32, attempt: u8, order: &[usize]) {
        self.hasher.update(simulation_index.to_le_bytes());
        self.hasher.update([attempt]);
        self.hasher.update((order.len() as u64).to_le_bytes());
        for card_index in order {
            let normalized_name = deck
                .cards
                .get(*card_index)
                .map(|card| card.normalized_name.as_str())
                .unwrap_or("<missing-card>");
            self.hasher
                .update((normalized_name.len() as u64).to_le_bytes());
            self.hasher.update(normalized_name.as_bytes());
        }
    }

    fn finish(self) -> String {
        format!("{:x}", self.hasher.finalize())
    }
}

#[derive(Debug, Default)]
struct ManaAccessProfile {
    required_colors: Vec<ManaColorMask>,
    source_weights_by_card: Vec<Vec<f32>>,
    sources_by_card: Vec<Option<ManaSourceProfile>>,
    costs_by_card: Vec<Option<ManaCostProfile>>,
}

impl ManaAccessProfile {
    fn compile(deck: &CompiledDeck, mana: &ManaModel) -> Self {
        let required_colors = mana
            .reliability
            .colors
            .iter()
            .filter(|summary| mana.reliability.required_colors.intersects(summary.color))
            .map(|summary| summary.color)
            .collect::<Vec<_>>();
        let sources = mana
            .sources
            .iter()
            .map(|source| (normalize_mana_name(&source.name), source))
            .collect::<std::collections::HashMap<_, _>>();
        let costs = mana
            .cards
            .iter()
            .map(|card| (normalize_mana_name(&card.name), &card.cost))
            .collect::<std::collections::HashMap<_, _>>();
        let source_weights_by_card = deck
            .cards
            .iter()
            .map(|card| {
                sources
                    .get(&normalize_mana_name(&card.name))
                    .map(|source| source_weights(source, &required_colors))
                    .unwrap_or_else(|| vec![0.0; required_colors.len()])
            })
            .collect();
        let sources_by_card = deck
            .cards
            .iter()
            .map(|card| {
                sources
                    .get(&normalize_mana_name(&card.name))
                    .cloned()
                    .cloned()
            })
            .collect();
        let costs_by_card = deck
            .cards
            .iter()
            .map(|card| {
                costs
                    .get(&normalize_mana_name(&card.name))
                    .cloned()
                    .cloned()
            })
            .collect();
        Self {
            required_colors,
            source_weights_by_card,
            sources_by_card,
            costs_by_card,
        }
    }

    fn coverage(&self, card_indices: &[usize]) -> Option<(f32, f32)> {
        if self.required_colors.is_empty() {
            return None;
        }
        let mut totals = vec![0.0f32; self.required_colors.len()];
        for index in card_indices {
            let Some(weights) = self.source_weights_by_card.get(*index) else {
                continue;
            };
            for (total, weight) in totals.iter_mut().zip(weights) {
                *total += weight;
            }
        }
        for total in &mut totals {
            *total = total.clamp(0.0, 1.0);
        }
        let average = totals.iter().sum::<f32>() / totals.len() as f32;
        let floor = totals.iter().copied().fold(1.0f32, f32::min);
        Some((average, floor))
    }

    fn source(&self, card_index: usize) -> Option<&ManaSourceProfile> {
        self.sources_by_card
            .get(card_index)
            .and_then(Option::as_ref)
    }

    fn cost(&self, card_index: usize) -> Option<&ManaCostProfile> {
        self.costs_by_card.get(card_index).and_then(Option::as_ref)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct BattlefieldManaSource {
    colors: ManaColorMask,
    capacity: u8,
    reliability: f32,
    available_from_turn: u8,
    is_land: bool,
    card_index: Option<usize>,
    behavior: BattlefieldManaBehavior,
    source_damage_on_first_spend: u8,
    damage_free_colors_on_first_spend: ManaColorMask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SacrificeOnFirstSpend {
    card_index: usize,
    sequence: u16,
}

/// `ManaSourceProfile::reliability` is a reporting weight, not an event
/// probability. Trajectories admit only sources whose Oracle-text conditions
/// and entry timing are exact here; typed conditional adapters construct their
/// own sources with an exact `1.0` trajectory marker.
fn mana_source_profile_is_exact_for_trajectory(source: &ManaSourceProfile) -> bool {
    !source.conditional
        && !source.unknown
        && !matches!(
            source.enters_tapped,
            EntersTapped::Conditional | EntersTapped::Unknown
        )
}

fn reusable_nonland_source_is_available_on_entry(
    card: &CompiledCard,
    source: &ManaSourceProfile,
) -> bool {
    !card.has(role::CREATURE) && source.enters_tapped != EntersTapped::Always
}

fn battlefield_source_is_trajectory_available(source: &BattlefieldManaSource, turn: u8) -> bool {
    turn >= source.available_from_turn && source.reliability >= 0.999
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum BattlefieldManaBehavior {
    #[default]
    Fixed,
    AnyColorAmongControlledLegendaryCreaturesAndPlaneswalkers,
    AnyColorWithMetalcraft,
    LinkedCardColors(ManaColorMask),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PoolManaSource {
    colors: ManaColorMask,
    remaining: u8,
    is_treasure: bool,
    treasure_on_first_spend: u8,
    first_spend_recorded: bool,
    origin_card_index: Option<usize>,
    /// Exact battlefield object when this source is backed by a permanent.
    /// `activation_used` is deliberately separate: a summoning-sick mana
    /// creature is unavailable as a mana ability but is not physically tapped
    /// and can still pay a non-{T} "tap an untapped creature" cost.
    origin_sequence: Option<u16>,
    physically_tapped: bool,
    behavior: BattlefieldManaBehavior,
    base_capacity: u8,
    is_land: bool,
    activation_used: bool,
    source_damage_on_first_spend: u8,
    damage_free_colors_on_first_spend: ManaColorMask,
    same_type_coupled: bool,
    sacrifice_on_first_spend: Option<SacrificeOnFirstSpend>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TurnManaPool {
    sources: Vec<PoolManaSource>,
    pending_triggered_treasures: u8,
    pending_source_damage: u8,
}

#[derive(Debug, Clone, Default)]
struct TapCandidatePaymentConstraint {
    candidate_sequences: BTreeSet<u16>,
    maximum_consumed_candidates: usize,
}

impl TapCandidatePaymentConstraint {
    fn admits(&self, pool: &TurnManaPool) -> bool {
        let consumed = pool
            .sources
            .iter()
            .filter(|source| source.physically_tapped)
            .filter_map(|source| source.origin_sequence)
            .filter(|sequence| self.candidate_sequences.contains(sequence))
            .collect::<BTreeSet<_>>()
            .len();
        consumed <= self.maximum_consumed_candidates
    }

    fn source_is_candidate(&self, source: &PoolManaSource) -> bool {
        source
            .origin_sequence
            .is_some_and(|sequence| self.candidate_sequences.contains(&sequence))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ActiveAbilityContext {
    nonland_mana_bonus: u8,
    dwarf_treasure_per_tap: u8,
}

impl TurnManaPool {
    fn from_battlefield_with_ability_context(
        battlefield: &[BattlefieldManaSource],
        turn: u8,
        deck: &CompiledDeck,
        context: ActiveAbilityContext,
        zones: &KnownLineZoneState,
    ) -> Self {
        let mut battlefield_sequences = HashMap::<usize, VecDeque<u16>>::new();
        for presence in &zones.battlefield {
            battlefield_sequences
                .entry(presence.card_index)
                .or_default()
                .push_back(presence.sequence);
        }
        let sources = battlefield
            .iter()
            .filter(|source| battlefield_source_is_trajectory_available(source, turn))
            .map(|source| {
                let origin_sequence = source.card_index.and_then(|card_index| {
                    battlefield_sequences
                        .get_mut(&card_index)
                        .and_then(VecDeque::pop_front)
                });
                let is_dwarf = origin_sequence.is_some_and(|sequence| {
                    creature_object_has_effective_subtype(
                        deck,
                        zones,
                        sequence,
                        source.card_index,
                        "Dwarf",
                    )
                });
                PoolManaSource {
                    colors: source.colors,
                    remaining: 0,
                    is_treasure: false,
                    treasure_on_first_spend: if is_dwarf {
                        context.dwarf_treasure_per_tap
                    } else {
                        0
                    },
                    first_spend_recorded: false,
                    origin_card_index: source.card_index,
                    origin_sequence,
                    physically_tapped: false,
                    behavior: source.behavior,
                    base_capacity: source.capacity,
                    is_land: source.is_land,
                    activation_used: false,
                    source_damage_on_first_spend: source.source_damage_on_first_spend,
                    damage_free_colors_on_first_spend: source.damage_free_colors_on_first_spend,
                    same_type_coupled: false,
                    sacrifice_on_first_spend: None,
                }
            })
            .collect();
        let mut pool = Self {
            sources,
            pending_triggered_treasures: 0,
            pending_source_damage: 0,
        };
        pool.refresh_battlefield_sources(deck, zones, context);
        pool
    }

    fn refresh_battlefield_sources(
        &mut self,
        deck: &CompiledDeck,
        zones: &KnownLineZoneState,
        context: ActiveAbilityContext,
    ) {
        let artifact_count = battlefield_artifact_count(deck, zones)
            .saturating_add(self.remaining_treasures() as usize);
        let legendary_colors = controlled_legendary_creature_or_planeswalker_colors(deck, zones);
        for source in &mut self.sources {
            if source.origin_card_index.is_none()
                || source.is_treasure
                || source.activation_used
                || source.physically_tapped
            {
                continue;
            }
            let colors = match source.behavior {
                BattlefieldManaBehavior::Fixed => source.colors,
                BattlefieldManaBehavior::AnyColorAmongControlledLegendaryCreaturesAndPlaneswalkers => {
                    legendary_colors
                }
                BattlefieldManaBehavior::AnyColorWithMetalcraft => {
                    if artifact_count >= 3 {
                        ManaColorMask::ANY_COLOR
                    } else {
                        ManaColorMask::NONE
                    }
                }
                BattlefieldManaBehavior::LinkedCardColors(colors) => colors,
            };
            source.colors = colors;
            source.remaining = if colors.is_empty() {
                0
            } else {
                source.base_capacity.saturating_add(if source.is_land {
                    0
                } else {
                    context.nonland_mana_bonus
                })
            };
            source.same_type_coupled = !source.is_land
                && context.nonland_mana_bonus > 0
                && source.base_capacity == 1
                && source_option_count(colors) > 1;
        }
    }

    fn total(&self) -> u8 {
        self.sources
            .iter()
            .fold(0u8, |total, source| total.saturating_add(source.remaining))
    }

    fn add_floating(&mut self, colors: ManaColorMask, amount: u8) {
        if amount > 0 {
            self.sources.push(PoolManaSource {
                colors,
                remaining: amount,
                is_treasure: false,
                ..PoolManaSource::default()
            });
        }
    }

    fn add_floating_from_nonland_permanent(
        &mut self,
        colors: ManaColorMask,
        amount: u8,
        is_dwarf: bool,
        context: ActiveAbilityContext,
    ) {
        let base_amount = amount;
        let amount = base_amount.saturating_add(context.nonland_mana_bonus);
        if amount > 0 {
            self.sources.push(PoolManaSource {
                colors,
                remaining: amount,
                is_treasure: false,
                treasure_on_first_spend: if is_dwarf {
                    context.dwarf_treasure_per_tap
                } else {
                    0
                },
                first_spend_recorded: false,
                same_type_coupled: context.nonland_mana_bonus > 0
                    && base_amount == 1
                    && source_option_count(colors) > 1,
                ..PoolManaSource::default()
            });
        }
    }

    fn add_treasures(&mut self, amount: u8) {
        if amount > 0 {
            self.sources.push(PoolManaSource {
                colors: ManaColorMask::ANY_COLOR,
                remaining: amount,
                is_treasure: true,
                ..PoolManaSource::default()
            });
        }
    }

    fn remaining_treasures(&self) -> u8 {
        self.sources
            .iter()
            .filter(|source| source.is_treasure)
            .fold(0u8, |total, source| total.saturating_add(source.remaining))
    }

    fn spend_treasures(&mut self, mut amount: u8) -> bool {
        if self.remaining_treasures() < amount {
            return false;
        }
        let mut candidate = self.clone();
        for source in candidate
            .sources
            .iter_mut()
            .filter(|source| source.is_treasure)
        {
            let spent = source.remaining.min(amount);
            source.remaining -= spent;
            amount -= spent;
            if amount == 0 {
                *self = candidate;
                return true;
            }
        }
        false
    }

    fn apply_pressure(&mut self, mut amount: u8) {
        // Consume the least flexible sources first so a one-mana pressure
        // abstraction does not disproportionately erase color access.
        self.sources.sort_by_key(|source| {
            (
                source_damage_for_spend(*source, None),
                source_option_count(source.colors),
                source.is_treasure,
            )
        });
        for source_index in 0..self.sources.len() {
            let removed = self.sources[source_index].remaining.min(amount);
            self.spend_from_source(source_index, removed, None);
            amount -= removed;
            if amount == 0 {
                break;
            }
        }
    }

    fn pay(
        &mut self,
        cost: Option<&ManaCostProfile>,
        fallback_mana_value: u8,
        reserve: u8,
    ) -> bool {
        let Some(cost) = cost.filter(|cost| !cost.faces.is_empty()) else {
            let mut candidate = self.clone();
            let paid = candidate.pay_generic(fallback_mana_value);
            if paid {
                candidate.resolve_pending_tap_triggers();
            }
            if paid && candidate.total() >= reserve {
                *self = candidate;
                return true;
            }
            return false;
        };

        // The legacy trajectory model stores one merged effect descriptor per
        // physical card. Paying whichever face is cheapest and then applying
        // that merged descriptor can execute the wrong face. Until a
        // face-bound action program is available, multiface costs fail closed.
        if cost.faces.len() != 1 {
            return false;
        }

        let mut best: Option<Self> = None;
        for face in &cost.faces {
            let mut candidate = self.clone();
            if !candidate.pay_face(face) {
                continue;
            }
            candidate.resolve_pending_tap_triggers();
            if candidate.total() >= reserve
                && best
                    .as_ref()
                    .is_none_or(|current| candidate.total() > current.total())
            {
                best = Some(candidate);
            }
        }
        if let Some(candidate) = best {
            *self = candidate;
            true
        } else {
            false
        }
    }

    fn pay_with_additional_generic(
        &mut self,
        cost: Option<&ManaCostProfile>,
        fallback_mana_value: u8,
        additional_generic: u8,
        reserve: u8,
    ) -> bool {
        self.pay_with_generic_adjustment(cost, fallback_mana_value, additional_generic, 0, reserve)
    }

    fn pay_with_generic_adjustment(
        &mut self,
        cost: Option<&ManaCostProfile>,
        fallback_mana_value: u8,
        additional_generic: u8,
        generic_reduction: u16,
        reserve: u8,
    ) -> bool {
        self.pay_with_generic_adjustment_and_constraint(
            cost,
            fallback_mana_value,
            additional_generic,
            generic_reduction,
            reserve,
            None,
        )
    }

    fn pay_with_generic_adjustment_and_constraint(
        &mut self,
        cost: Option<&ManaCostProfile>,
        fallback_mana_value: u8,
        additional_generic: u8,
        generic_reduction: u16,
        reserve: u8,
        tap_constraint: Option<&TapCandidatePaymentConstraint>,
    ) -> bool {
        if additional_generic == 0 && generic_reduction == 0 {
            return self.pay_with_constraint(cost, fallback_mana_value, reserve, tap_constraint);
        }
        if let Some(cost) = cost.filter(|cost| !cost.faces.is_empty()) {
            if cost.faces.len() != 1 {
                return false;
            }
            let faces = adjusted_announced_cost_faces(
                &cost.faces[0],
                additional_generic,
                generic_reduction,
            );
            self.pay_face_candidates(&faces, reserve, tap_constraint)
        } else {
            self.pay_with_constraint(
                None,
                fallback_mana_value
                    .saturating_add(additional_generic)
                    .saturating_sub(generic_reduction.min(u16::from(u8::MAX)) as u8),
                reserve,
                tap_constraint,
            )
        }
    }

    fn pay_face(&mut self, face: &ManaCostFace) -> bool {
        self.pay_face_with_constraint(face, None)
    }

    fn pay_face_with_constraint(
        &mut self,
        face: &ManaCostFace,
        tap_constraint: Option<&TapCandidatePaymentConstraint>,
    ) -> bool {
        let mut requirements = Vec::new();
        let generic = face.generic_value.min(u8::MAX as u16) as u8;
        for pip in &face.pips {
            // Variable choices, Phyrexian life payments, snow-source
            // provenance, and unknown symbols need typed cost execution.
            // Skipping or genericizing them underpays the spell, so the
            // bounded model rejects the payment instead.
            if pip.is_variable || pip.is_phyrexian || pip.is_snow || pip.is_unknown {
                return false;
            }
            if pip.is_hybrid && pip.generic_value.is_some() && !pip.colors.is_empty() {
                requirements.push((pip.colors, pip.generic_value.unwrap_or(2).min(8) as u8));
            } else if pip.is_colorless {
                requirements.push((ManaColorMask::COLORLESS, 0));
            } else if !pip.colors.is_empty() {
                requirements.push((pip.colors, 0));
            }
        }
        requirements.sort_by_key(|(colors, generic_alternative)| {
            (
                usize::from(*generic_alternative > 0),
                source_option_count(*colors),
            )
        });
        pay_requirements(self, &requirements, 0, generic, tap_constraint)
    }

    fn pay_generic(&mut self, mut amount: u8) -> bool {
        if self.total() < amount {
            return false;
        }
        self.sources.sort_by_key(|source| {
            (
                source_damage_for_spend(*source, None),
                source_option_count(source.colors),
                source.is_treasure,
            )
        });
        for source_index in 0..self.sources.len() {
            let spent = self.sources[source_index].remaining.min(amount);
            self.spend_from_source(source_index, spent, None);
            amount -= spent;
            if amount == 0 {
                return true;
            }
        }
        amount == 0
    }

    fn spend_from_source(
        &mut self,
        source_index: usize,
        amount: u8,
        accepted_colors: Option<ManaColorMask>,
    ) {
        let Some(source) = self.sources.get_mut(source_index) else {
            return;
        };
        if amount == 0 || source.remaining < amount {
            return;
        }
        let source_damage = source_damage_for_spend(*source, accepted_colors);
        source.remaining -= amount;
        if !source.first_spend_recorded {
            source.first_spend_recorded = true;
            source.activation_used = true;
            source.physically_tapped |= source.origin_card_index.is_some() && !source.is_treasure;
            self.pending_triggered_treasures = self
                .pending_triggered_treasures
                .saturating_add(source.treasure_on_first_spend);
            self.pending_source_damage = self.pending_source_damage.saturating_add(source_damage);
        }
    }

    fn resolve_pending_tap_triggers(&mut self) {
        let treasures = std::mem::take(&mut self.pending_triggered_treasures);
        self.add_treasures(treasures);
    }

    fn take_pending_source_damage(&mut self) -> u8 {
        std::mem::take(&mut self.pending_source_damage)
    }

    fn settle_pending_source_damage(&mut self, life_total: &mut f32) -> bool {
        *life_total -= f32::from(self.take_pending_source_damage());
        *life_total > 0.0
    }

    fn pay_with_constraint(
        &mut self,
        cost: Option<&ManaCostProfile>,
        fallback_mana_value: u8,
        reserve: u8,
        tap_constraint: Option<&TapCandidatePaymentConstraint>,
    ) -> bool {
        let Some(cost) = cost.filter(|cost| !cost.faces.is_empty()) else {
            let mut candidate = self.clone();
            let paid =
                pay_generic_with_constraint(&mut candidate, fallback_mana_value, tap_constraint);
            if paid {
                candidate.resolve_pending_tap_triggers();
            }
            if paid
                && candidate.total() >= reserve
                && tap_constraint.is_none_or(|constraint| constraint.admits(&candidate))
            {
                *self = candidate;
                return true;
            }
            return false;
        };
        if cost.faces.len() != 1 {
            return false;
        }
        self.pay_face_candidates(&cost.faces, reserve, tap_constraint)
    }

    fn pay_face_candidates(
        &mut self,
        faces: &[ManaCostFace],
        reserve: u8,
        tap_constraint: Option<&TapCandidatePaymentConstraint>,
    ) -> bool {
        let mut best: Option<Self> = None;
        for face in faces {
            let mut candidate = self.clone();
            if !candidate.pay_face_with_constraint(face, tap_constraint) {
                continue;
            }
            candidate.resolve_pending_tap_triggers();
            if candidate.total() >= reserve
                && tap_constraint.is_none_or(|constraint| constraint.admits(&candidate))
                && best
                    .as_ref()
                    .is_none_or(|current| candidate.total() > current.total())
            {
                best = Some(candidate);
            }
        }
        if let Some(candidate) = best {
            *self = candidate;
            true
        } else {
            false
        }
    }
}

fn active_ability_context(deck: &CompiledDeck, zones: &KnownLineZoneState) -> ActiveAbilityContext {
    let mut context = ActiveAbilityContext::default();
    for presence in &zones.battlefield {
        let Some(card) = deck.cards.get(presence.card_index) else {
            continue;
        };
        for ability in card.ability_program.executable_abilities() {
            if matches!(
                ability.timing,
                AbilityTiming::Triggered {
                    event: crate::ability_program::TriggerEvent {
                        kind: TriggerEventKind::PermanentTappedForMana,
                        actor: ControllerRelation::You,
                        ..
                    }
                }
            ) {
                for effect in &ability.effects {
                    if let AbilityEffect::ModifyNonlandMana(modifier) = effect
                        && modifier.kind == ProgramManaKind::AnyTypeProducedByTriggeringPermanent
                    {
                        context.nonland_mana_bonus = context.nonland_mana_bonus.saturating_add(
                            modifier.additional_amount.min(u16::from(u8::MAX)) as u8,
                        );
                    }
                }
            }
            if matches!(
                &ability.timing,
                AbilityTiming::Triggered {
                    event: crate::ability_program::TriggerEvent {
                        kind: TriggerEventKind::PermanentBecomesTapped,
                        actor: ControllerRelation::You,
                        object_filter,
                    }
                } if object_filter.subtype.as_deref() == Some("Dwarf")
            ) {
                for effect in &ability.effects {
                    if let AbilityEffect::CreateToken(token) = effect
                        && token.kind == TokenKind::Treasure
                    {
                        context.dwarf_treasure_per_tap = context
                            .dwarf_treasure_per_tap
                            .saturating_add(token.count.min(u16::from(u8::MAX)) as u8);
                    }
                }
            }
        }
    }
    context
}

fn type_line_has_subtype(type_line: &str, subtype: &str) -> bool {
    type_line
        .split(|character: char| !character.is_alphabetic())
        .any(|word| word.eq_ignore_ascii_case(subtype))
}

fn card_has_keyword(card: &CompiledCard, keyword: &str) -> bool {
    card.ability_program.executable_abilities().any(|ability| {
        ability.normalized_oracle.eq_ignore_ascii_case(keyword)
            || oracle_declares_source_keyword(&ability.normalized_oracle, keyword)
    }) || card.ability_program.unsupported_abilities().any(|ability| {
        ability.normalized_oracle.eq_ignore_ascii_case(keyword)
            || oracle_declares_source_keyword(&ability.normalized_oracle, keyword)
    })
}

fn oracle_declares_source_keyword(oracle: &str, keyword: &str) -> bool {
    let keyword = keyword.to_ascii_lowercase();
    oracle.split(',').map(str::trim).any(|clause| {
        let clause = clause.to_ascii_lowercase();
        clause == keyword
            || clause
                .strip_prefix(&keyword)
                .is_some_and(|suffix| suffix.trim_start().starts_with('('))
    })
}

fn combat_object_id(sequence: u16) -> ObjectId {
    ObjectId(u64::from(sequence).saturating_add(1))
}

fn permanent_types_for_card(card: &CompiledCard) -> BTreeSet<PermanentType> {
    [
        ("Artifact", PermanentType::Artifact),
        ("Battle", PermanentType::Battle),
        ("Creature", PermanentType::Creature),
        ("Enchantment", PermanentType::Enchantment),
        ("Kindred", PermanentType::Kindred),
        ("Land", PermanentType::Land),
        ("Planeswalker", PermanentType::Planeswalker),
    ]
    .into_iter()
    .filter_map(|(name, kind)| type_line_has_subtype(&card.type_line, name).then_some(kind))
    .collect()
}

fn creature_types_from_type_line(type_line: &str) -> BTreeSet<String> {
    let subtype_text = type_line
        .split_once('\u{2014}')
        .map(|(_, subtypes)| subtypes)
        .or_else(|| type_line.split_once(" - ").map(|(_, subtypes)| subtypes));
    subtype_text
        .into_iter()
        .flat_map(|subtypes| {
            subtypes
                .split(|character: char| {
                    !(character.is_alphabetic() || character == '-' || character == '\'')
                })
                .filter(|subtype| !subtype.is_empty())
                .map(|subtype| subtype.to_ascii_lowercase())
        })
        .collect()
}

fn token_creature_types(description: &str) -> BTreeSet<String> {
    const NON_TYPES: [&str; 13] = [
        "white",
        "blue",
        "black",
        "red",
        "green",
        "colorless",
        "legendary",
        "snow",
        "artifact",
        "enchantment",
        "creature",
        "token",
        "and",
    ];
    description
        .split(|character: char| {
            !(character.is_alphabetic() || character == '-' || character == '\'')
        })
        .map(str::to_ascii_lowercase)
        .filter(|word| !word.is_empty() && !NON_TYPES.contains(&word.as_str()))
        .collect()
}

fn preferred_creature_type(deck: &CompiledDeck, zones: &KnownLineZoneState) -> Option<String> {
    let mut scores = HashMap::<String, u32>::new();
    for card in &deck.cards {
        if card.effects.card_types.is_creature {
            let weight =
                u32::from(card.quantity).saturating_mul(if card.is_commander { 4 } else { 1 });
            for creature_type in creature_types_from_type_line(&card.type_line) {
                let current = scores.get(&creature_type).copied().unwrap_or_default();
                scores.insert(creature_type, current.saturating_add(weight));
            }
        }
        for ability in card.ability_program.executable_abilities() {
            for effect in &ability.effects {
                let AbilityEffect::CreateToken(token) = effect else {
                    continue;
                };
                let TokenKind::Creature { description, .. } = &token.kind else {
                    continue;
                };
                let weight = u32::from(card.quantity)
                    .saturating_mul(u32::from(token.count))
                    .saturating_mul(3);
                for creature_type in token_creature_types(description) {
                    let current = scores.get(&creature_type).copied().unwrap_or_default();
                    scores.insert(creature_type, current.saturating_add(weight));
                }
            }
        }
    }
    for presence in &zones.battlefield {
        if let Some(card) = deck.cards.get(presence.card_index) {
            for creature_type in creature_types_from_type_line(&card.type_line) {
                let current = scores.get(&creature_type).copied().unwrap_or_default();
                scores.insert(creature_type, current.saturating_add(4));
            }
        }
    }
    for token in &zones.creature_tokens {
        for creature_type in &token.creature_types {
            let current = scores.get(creature_type).copied().unwrap_or_default();
            scores.insert(creature_type.clone(), current.saturating_add(5));
        }
    }
    scores
        .into_iter()
        .max_by(|(left_type, left_score), (right_type, right_score)| {
            left_score
                .cmp(right_score)
                .then_with(|| right_type.cmp(left_type))
        })
        .map(|(creature_type, _)| creature_type)
}

fn printed_combat_keywords(card: &CompiledCard) -> BTreeSet<CombatKeyword> {
    [
        ("Deathtouch", CombatKeyword::Deathtouch),
        ("Double strike", CombatKeyword::DoubleStrike),
        ("First strike", CombatKeyword::FirstStrike),
        ("Flying", CombatKeyword::Flying),
        ("Haste", CombatKeyword::Haste),
        ("Hexproof", CombatKeyword::Hexproof),
        ("Indestructible", CombatKeyword::Indestructible),
        ("Lifelink", CombatKeyword::Lifelink),
        ("Menace", CombatKeyword::Menace),
        ("Reach", CombatKeyword::Reach),
        ("Shroud", CombatKeyword::Shroud),
        ("Trample", CombatKeyword::Trample),
        ("Vigilance", CombatKeyword::Vigilance),
    ]
    .into_iter()
    .filter_map(|(oracle, keyword)| card_has_keyword(card, oracle).then_some(keyword))
    .collect()
}

fn program_keyword_to_combat(keyword: GrantedCreatureKeyword) -> CombatKeyword {
    match keyword {
        GrantedCreatureKeyword::CantBeBlocked => CombatKeyword::CantBeBlocked,
        GrantedCreatureKeyword::Deathtouch => CombatKeyword::Deathtouch,
        GrantedCreatureKeyword::DoubleStrike => CombatKeyword::DoubleStrike,
        GrantedCreatureKeyword::FirstStrike => CombatKeyword::FirstStrike,
        GrantedCreatureKeyword::Flying => CombatKeyword::Flying,
        GrantedCreatureKeyword::Haste => CombatKeyword::Haste,
        GrantedCreatureKeyword::Hexproof => CombatKeyword::Hexproof,
        GrantedCreatureKeyword::Indestructible => CombatKeyword::Indestructible,
        GrantedCreatureKeyword::Lifelink => CombatKeyword::Lifelink,
        GrantedCreatureKeyword::Menace => CombatKeyword::Menace,
        GrantedCreatureKeyword::Reach => CombatKeyword::Reach,
        GrantedCreatureKeyword::Shroud => CombatKeyword::Shroud,
        GrantedCreatureKeyword::Trample => CombatKeyword::Trample,
        GrantedCreatureKeyword::Vigilance => CombatKeyword::Vigilance,
    }
}

fn card_has_program_creature_keyword(card: &CompiledCard, keyword: GrantedCreatureKeyword) -> bool {
    card.effects.card_types.is_creature
        && printed_combat_keywords(card).contains(&program_keyword_to_combat(keyword))
}

fn controller_has_creature_with_program_keyword(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    keyword: GrantedCreatureKeyword,
) -> bool {
    let combat_keyword = program_keyword_to_combat(keyword);
    let combat_runtime = combat_effect_runtime(deck, zones);
    zones.battlefield.iter().any(|presence| {
        let Some(card) = deck.cards.get(presence.card_index) else {
            return false;
        };
        if !card.effects.card_types.is_creature {
            return false;
        }
        combat_runtime
            .as_ref()
            .and_then(|(state, effects)| {
                state
                    .evaluate_creature(combat_object_id(presence.sequence), effects)
                    .ok()
            })
            .is_some_and(|profile| profile.keywords.contains(&combat_keyword))
            || printed_combat_keywords(card).contains(&combat_keyword)
    }) || zones.creature_tokens.iter().any(|token| {
        combat_runtime
            .as_ref()
            .and_then(|(state, effects)| {
                state
                    .evaluate_creature(combat_object_id(token.sequence), effects)
                    .ok()
            })
            .is_some_and(|profile| profile.keywords.contains(&combat_keyword))
            || token.printed_keywords.contains(&combat_keyword)
    })
}

fn generic_spell_cost_reduction(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    card_index: usize,
) -> u16 {
    let Some(spell) = deck.cards.get(card_index) else {
        return 0;
    };
    let mut reduction = 0u16;

    for source_presence in &zones.battlefield {
        let Some(source) = deck.cards.get(source_presence.card_index) else {
            continue;
        };
        for ability in source.ability_program.executable_abilities() {
            if ability.timing != AbilityTiming::StaticModifier
                || !ability.costs.is_empty()
                || ability.preconditions
                    != [AbilityPrecondition::SourceZone(ProgramZone::Battlefield)]
            {
                continue;
            }
            let [AbilityEffect::ReduceSpellCost(effect)] = ability.effects.as_slice() else {
                continue;
            };
            let matches = match effect.affected_spell {
                SpellCostReductionScope::CreatureSpellYouCastWithKeyword(keyword) => {
                    effect.condition.is_none() && card_has_program_creature_keyword(spell, keyword)
                }
                SpellCostReductionScope::SourceSpell => false,
            };
            if matches {
                reduction = reduction.saturating_add(effect.generic_mana_reduction);
            }
        }
    }

    for ability in spell.ability_program.executable_abilities() {
        if ability.timing != AbilityTiming::StaticModifier
            || !ability.costs.is_empty()
            || !ability.preconditions.is_empty()
        {
            continue;
        }
        let [AbilityEffect::ReduceSpellCost(effect)] = ability.effects.as_slice() else {
            continue;
        };
        if effect.affected_spell != SpellCostReductionScope::SourceSpell {
            continue;
        }
        let condition_matches = match effect.condition {
            Some(SpellCostReductionCondition::YouControlCreatureWithKeyword(keyword)) => {
                controller_has_creature_with_program_keyword(deck, zones, keyword)
            }
            None => true,
        };
        if condition_matches {
            reduction = reduction.saturating_add(effect.generic_mana_reduction);
        }
    }

    reduction
}

fn exact_self_alternative_spell_cost(
    card: &CompiledCard,
) -> Option<(&ProgramManaCost, u16, GrantedCreatureKeyword)> {
    let mut matched = None;
    for ability in card.ability_program.executable_abilities() {
        if ability.timing != AbilityTiming::StaticModifier
            || !ability.costs.is_empty()
            || !ability.preconditions.is_empty()
        {
            continue;
        }
        let [AbilityEffect::AlternativeSpellCost(effect)] = ability.effects.as_slice() else {
            continue;
        };
        if effect.replaces != ReplacedSpellCost::PrintedManaCost {
            return None;
        }
        let [
            AlternativeSpellCostComponent::Mana(mana),
            AlternativeSpellCostComponent::TapUntappedPermanents { count, filter },
        ] = effect.payment.as_slice()
        else {
            return None;
        };
        if *count == 0
            || filter.controller != ControllerRelation::You
            || filter.card_type != ProgramCardType::Creature
            || matched
                .replace((mana, *count, filter.required_keyword))
                .is_some()
        {
            return None;
        }
    }
    matched
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SpellPaymentChoice {
    Printed,
    Alternative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AlternativeCostTapCandidate {
    sequence: u16,
    card_index: Option<usize>,
    is_dwarf: bool,
}

fn creature_object_has_effective_subtype(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    sequence: u16,
    card_index: Option<usize>,
    subtype: &str,
) -> bool {
    if battlefield_grants_all_creature_types(deck, zones) {
        return true;
    }
    if let Some(card_index) = card_index
        && let Some(card) = deck.cards.get(card_index)
    {
        if card_has_keyword(card, "Changeling") || card_has_subtype(card, subtype) {
            return true;
        }
        let source_has_chosen_type = card.ability_program.executable_abilities().any(|ability| {
            ability
                .effects
                .iter()
                .any(|effect| matches!(effect, AbilityEffect::SourceHasChosenCreatureType(_)))
        });
        return source_has_chosen_type
            && zones
                .chosen_creature_types
                .get(&sequence)
                .is_some_and(|chosen| chosen.eq_ignore_ascii_case(subtype));
    }
    zones
        .creature_tokens
        .iter()
        .find(|token| token.sequence == sequence)
        .is_some_and(|token| {
            token
                .creature_types
                .iter()
                .any(|creature_type| creature_type.eq_ignore_ascii_case(subtype))
        })
}

fn alternative_cost_tap_candidates(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    mana_pool: &TurnManaPool,
    required_keyword: GrantedCreatureKeyword,
) -> Vec<AlternativeCostTapCandidate> {
    let combat_keyword = program_keyword_to_combat(required_keyword);
    let combat_runtime = combat_effect_runtime(deck, zones);
    let physically_tapped_sequences = mana_pool
        .sources
        .iter()
        .filter(|source| source.physically_tapped)
        .filter_map(|source| source.origin_sequence)
        .collect::<BTreeSet<_>>();
    // Synthetic pools used by focused tests may not carry object identities.
    // Treat only an actual recorded activation as tapped; `activation_used`
    // alone also represents summoning sickness and is not enough.
    let mut unidentified_tapped_mana_creatures = mana_pool
        .sources
        .iter()
        .filter(|source| {
            !source.is_land
                && source.origin_sequence.is_none()
                && source.physically_tapped
                && source.activation_used
        })
        .filter_map(|source| source.origin_card_index)
        .fold(HashMap::<usize, usize>::new(), |mut counts, card_index| {
            *counts.entry(card_index).or_default() += 1;
            counts
        });
    let mut candidates = Vec::new();

    for presence in &zones.battlefield {
        let Some(card) = deck.cards.get(presence.card_index) else {
            continue;
        };
        if !card.effects.card_types.is_creature {
            continue;
        }
        if zones
            .tapped_creatures_this_turn
            .contains(&presence.sequence)
            || physically_tapped_sequences.contains(&presence.sequence)
        {
            continue;
        }
        if unidentified_tapped_mana_creatures
            .get_mut(&presence.card_index)
            .is_some_and(|remaining| {
                if *remaining == 0 {
                    false
                } else {
                    *remaining -= 1;
                    true
                }
            })
        {
            continue;
        }
        let profile = combat_runtime.as_ref().and_then(|(state, effects)| {
            state
                .evaluate_creature(combat_object_id(presence.sequence), effects)
                .ok()
        });
        let has_keyword = profile
            .as_ref()
            .is_some_and(|profile| profile.keywords.contains(&combat_keyword))
            || printed_combat_keywords(card).contains(&combat_keyword);
        if has_keyword {
            candidates.push(AlternativeCostTapCandidate {
                sequence: presence.sequence,
                card_index: Some(presence.card_index),
                is_dwarf: creature_object_has_effective_subtype(
                    deck,
                    zones,
                    presence.sequence,
                    Some(presence.card_index),
                    "Dwarf",
                ),
            });
        }
    }

    for token in &zones.creature_tokens {
        if zones.tapped_creatures_this_turn.contains(&token.sequence)
            || physically_tapped_sequences.contains(&token.sequence)
        {
            continue;
        }
        let profile = combat_runtime.as_ref().and_then(|(state, effects)| {
            state
                .evaluate_creature(combat_object_id(token.sequence), effects)
                .ok()
        });
        let has_keyword = profile
            .as_ref()
            .is_some_and(|profile| profile.keywords.contains(&combat_keyword))
            || token.printed_keywords.contains(&combat_keyword);
        if has_keyword {
            candidates.push(AlternativeCostTapCandidate {
                sequence: token.sequence,
                card_index: None,
                is_dwarf: creature_object_has_effective_subtype(
                    deck,
                    zones,
                    token.sequence,
                    None,
                    "Dwarf",
                ),
            });
        }
    }

    candidates.sort_by_key(|candidate| (candidate.sequence, candidate.card_index));
    candidates
}

fn combat_progress_after_current_attack(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    mana_pool: &TurnManaPool,
    turn: u8,
    combat_state: Option<&CommanderCombatState>,
) -> i64 {
    let mut projected = combat_state.cloned().unwrap_or_default();
    if let Some(attack) = plan_combat_attack(deck, zones, turn, &projected, mana_pool) {
        let _ = resolve_all_connected_combat_damage(&mut projected, &attack);
    }
    combat_state_progress_basis_points(&projected)
}

/// Reusable current-combat snapshot for comparing non-mana tap-cost choices.
///
/// The modeled tap changes only attack eligibility: it does not remove the
/// permanent or its continuous effects. Build those effects and every
/// attacker's exact damage profile once, then filter physical attackers for
/// each candidate subset before running the unchanged allocation/terminal
/// evaluator.
struct PrecomputedCombatAttackProjection {
    attackers: Vec<CombatAttacker>,
    combat_state: CommanderCombatState,
}

impl PrecomputedCombatAttackProjection {
    fn new(
        deck: &CompiledDeck,
        zones: &KnownLineZoneState,
        mana_pool: &TurnManaPool,
        turn: u8,
        combat_state: Option<&CommanderCombatState>,
    ) -> Self {
        Self {
            attackers: combat_attackers(deck, zones, turn, Some(mana_pool)),
            combat_state: combat_state.cloned().unwrap_or_default(),
        }
    }

    fn progress_after_tapping(&self, tapped_sequences: &BTreeSet<u16>) -> i64 {
        let available_attackers = self
            .attackers
            .iter()
            .copied()
            .filter(|attacker| {
                attacker
                    .attacker_id
                    .checked_sub(1)
                    .and_then(|sequence| u16::try_from(sequence).ok())
                    .is_none_or(|sequence| !tapped_sequences.contains(&sequence))
            })
            .collect::<Vec<_>>();
        let mut projected = self.combat_state.clone();
        if !available_attackers.is_empty()
            && let Ok(attack) = allocate_attack(&projected, &available_attackers)
        {
            let _ = resolve_all_connected_combat_damage(&mut projected, &attack);
        }
        combat_state_progress_basis_points(&projected)
    }
}

fn choose_alternative_cost_tap_candidates(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    mana_pool: &TurnManaPool,
    turn: u8,
    combat_state: Option<&CommanderCombatState>,
    candidates: &[AlternativeCostTapCandidate],
    required: usize,
) -> Option<Vec<AlternativeCostTapCandidate>> {
    if candidates.len() < required {
        return None;
    }
    let projection =
        PrecomputedCombatAttackProjection::new(deck, zones, mana_pool, turn, combat_state);
    let mut staged_taps = zones.tapped_creatures_this_turn.clone();
    let mut baseline = projection.progress_after_tapping(&staged_taps);
    let mut remaining = candidates.to_vec();
    let mut chosen = Vec::with_capacity(required);
    for _ in 0..required {
        let (best_position, _, selected_progress) = remaining
            .iter()
            .enumerate()
            .map(|(position, candidate)| {
                let mut after_tap = staged_taps.clone();
                after_tap.insert(candidate.sequence);
                let after = projection.progress_after_tapping(&after_tap);
                (
                    position,
                    (
                        baseline.saturating_sub(after),
                        candidate.sequence,
                        candidate.card_index,
                    ),
                    after,
                )
            })
            .min_by_key(|(_, rank, _)| *rank)?;
        let selected = remaining.swap_remove(best_position);
        staged_taps.insert(selected.sequence);
        baseline = selected_progress;
        chosen.push(selected);
    }
    Some(chosen)
}

#[allow(clippy::too_many_arguments)]
fn pay_spell_cost_choice(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    mana_pool: &mut TurnManaPool,
    card_index: usize,
    printed_cost: Option<&ManaCostProfile>,
    fallback_mana_value: u8,
    additional_generic: u8,
    reserve: u8,
    turn: u8,
    payment_choice: SpellPaymentChoice,
    combat_state: Option<&CommanderCombatState>,
) -> bool {
    let Some(card) = deck.cards.get(card_index) else {
        return false;
    };
    let generic_reduction = generic_spell_cost_reduction(deck, zones, card_index);
    match payment_choice {
        SpellPaymentChoice::Printed => {
            let exact_alternative_exists = exact_self_alternative_spell_cost(card).is_some();
            if exact_alternative_exists
                && printed_cost.is_none_or(|cost| {
                    cost.confidence < 0.999
                        || cost.faces.len() != 1
                        || cost.faces[0].confidence < 0.999
                })
            {
                return false;
            }
            mana_pool.pay_with_generic_adjustment(
                printed_cost,
                fallback_mana_value,
                additional_generic,
                generic_reduction,
                reserve,
            )
        }
        SpellPaymentChoice::Alternative => {
            let Some((alternative_mana, tap_count, required_keyword)) =
                exact_self_alternative_spell_cost(card)
            else {
                return false;
            };
            let ProgramManaCost::PrintedSymbols { oracle, .. } = alternative_mana else {
                return false;
            };
            let alternative_cost = parse_mana_cost(Some(oracle));
            if alternative_cost.confidence < 0.999
                || alternative_cost.faces.len() != 1
                || alternative_cost.faces[0].confidence < 0.999
            {
                return false;
            }

            let prepayment_candidates =
                alternative_cost_tap_candidates(deck, zones, mana_pool, required_keyword);
            let required = usize::from(tap_count);
            if prepayment_candidates.len() < required {
                return false;
            }
            let constraint = TapCandidatePaymentConstraint {
                candidate_sequences: prepayment_candidates
                    .iter()
                    .map(|candidate| candidate.sequence)
                    .collect(),
                maximum_consumed_candidates: prepayment_candidates.len() - required,
            };

            let mut staged_pool = mana_pool.clone();
            if !staged_pool.pay_with_generic_adjustment_and_constraint(
                Some(&alternative_cost),
                0,
                additional_generic,
                generic_reduction,
                0,
                Some(&constraint),
            ) {
                return false;
            }
            let mut staged_zones = zones.clone();
            let postpayment_candidates = alternative_cost_tap_candidates(
                deck,
                &staged_zones,
                &staged_pool,
                required_keyword,
            );
            let Some(chosen) = choose_alternative_cost_tap_candidates(
                deck,
                &staged_zones,
                &staged_pool,
                turn,
                combat_state,
                &postpayment_candidates,
                required,
            ) else {
                return false;
            };

            let ability_context = active_ability_context(deck, &staged_zones);
            let mut tapped_dwarves = 0u8;
            for candidate in chosen {
                staged_zones
                    .tapped_creatures_this_turn
                    .insert(candidate.sequence);
                tapped_dwarves = tapped_dwarves.saturating_add(u8::from(candidate.is_dwarf));
                if let Some(source) = staged_pool.sources.iter_mut().find(|source| {
                    !source.is_land
                        && !source.physically_tapped
                        && (source.origin_sequence == Some(candidate.sequence)
                            || source.origin_sequence.is_none()
                                && candidate.card_index.is_some()
                                && source.origin_card_index == candidate.card_index)
                }) {
                    source.remaining = 0;
                    source.activation_used = true;
                    source.physically_tapped = true;
                }
            }
            staged_pool.add_treasures(
                tapped_dwarves.saturating_mul(ability_context.dwarf_treasure_per_tap),
            );
            if staged_pool.total() < reserve {
                return false;
            }
            *zones = staged_zones;
            *mana_pool = staged_pool;
            true
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pay_spell_printed_or_alternative_cost(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    mana_pool: &mut TurnManaPool,
    card_index: usize,
    printed_cost: Option<&ManaCostProfile>,
    fallback_mana_value: u8,
    additional_generic: u8,
    reserve: u8,
    turn: u8,
) -> bool {
    let mut printed_zones = zones.clone();
    let mut printed_pool = mana_pool.clone();
    if pay_spell_cost_choice(
        deck,
        &mut printed_zones,
        &mut printed_pool,
        card_index,
        printed_cost,
        fallback_mana_value,
        additional_generic,
        reserve,
        turn,
        SpellPaymentChoice::Printed,
        None,
    ) {
        *zones = printed_zones;
        *mana_pool = printed_pool;
        return true;
    }
    pay_spell_cost_choice(
        deck,
        zones,
        mana_pool,
        card_index,
        printed_cost,
        fallback_mana_value,
        additional_generic,
        reserve,
        turn,
        SpellPaymentChoice::Alternative,
        None,
    )
}

fn program_specific_type_to_permanent(card_type: SpecificCardType) -> PermanentType {
    match card_type {
        SpecificCardType::Artifact => PermanentType::Artifact,
        SpecificCardType::Enchantment => PermanentType::Enchantment,
    }
}

fn static_modifier_value(value: &StaticModifierValue) -> DynamicValue {
    match value {
        StaticModifierValue::Fixed(value) => DynamicValue::fixed(i32::from(*value)),
        StaticModifierValue::PermanentsYouControl {
            multiplier,
            any_of_card_types,
        } => DynamicValue {
            constant: 0,
            terms: vec![CountedValue::MatchingPermanents {
                multiplier: i32::from(*multiplier),
                filter: PermanentFilter {
                    controller: ControllerConstraint::SameAsSource,
                    any_card_types: any_of_card_types
                        .iter()
                        .copied()
                        .map(program_specific_type_to_permanent)
                        .collect(),
                    ..PermanentFilter::default()
                },
            }],
        },
    }
}

fn static_modifier_value_is_nonnegative(value: &StaticModifierValue) -> bool {
    match value {
        StaticModifierValue::Fixed(value) => *value >= 0,
        StaticModifierValue::PermanentsYouControl { multiplier, .. } => *multiplier >= 0,
    }
}

fn static_modifier_value_can_help(value: &StaticModifierValue) -> bool {
    match value {
        StaticModifierValue::Fixed(value) => *value > 0,
        StaticModifierValue::PermanentsYouControl { multiplier, .. } => *multiplier > 0,
    }
}

fn card_has_beneficial_attached_static_effect(card: &CompiledCard) -> bool {
    card.ability_program.executable_abilities().any(|ability| {
        ability.effects.iter().any(|effect| {
            let AbilityEffect::ApplyStaticCreatureModifier(modifier) = effect else {
                return false;
            };
            modifier.target == StaticCreatureModifierTarget::CreatureEnchantedBySource
                && static_modifier_value_is_nonnegative(&modifier.power_delta)
                && static_modifier_value_is_nonnegative(&modifier.toughness_delta)
                && (static_modifier_value_can_help(&modifier.power_delta)
                    || static_modifier_value_can_help(&modifier.toughness_delta)
                    || !modifier.granted_keywords.is_empty())
        })
    })
}

fn card_has_beneficial_attached_trigger(card: &CompiledCard) -> bool {
    card.ability_program.executable_abilities().any(|ability| {
        matches!(
            ability.timing,
            AbilityTiming::Triggered {
                event: crate::ability_program::TriggerEvent {
                    kind: TriggerEventKind::EnchantedCreatureDealsDamageToOpponent,
                    ..
                }
            }
        ) && ability
            .effects
            .iter()
            .any(|effect| matches!(effect, AbilityEffect::Draw(draw) if draw.count > 0))
    })
}

fn static_modifier_target(target: StaticCreatureModifierTarget) -> EffectTarget {
    match target {
        StaticCreatureModifierTarget::SourceCreature => EffectTarget::Source,
        StaticCreatureModifierTarget::CreaturesYouControl => {
            EffectTarget::Filter(PermanentFilter {
                controller: ControllerConstraint::SameAsSource,
                all_card_types: BTreeSet::from([PermanentType::Creature]),
                ..PermanentFilter::default()
            })
        }
        StaticCreatureModifierTarget::OtherCreaturesYouControl => {
            EffectTarget::Filter(PermanentFilter {
                controller: ControllerConstraint::SameAsSource,
                all_card_types: BTreeSet::from([PermanentType::Creature]),
                exclude_source: true,
                ..PermanentFilter::default()
            })
        }
        StaticCreatureModifierTarget::OtherCreaturesYouControlWithKeyword(keyword) => {
            EffectTarget::Filter(PermanentFilter {
                controller: ControllerConstraint::SameAsSource,
                all_card_types: BTreeSet::from([PermanentType::Creature]),
                required_keywords: BTreeSet::from([program_keyword_to_combat(keyword)]),
                exclude_source: true,
                ..PermanentFilter::default()
            })
        }
        StaticCreatureModifierTarget::CreatureTokensYouControl => {
            EffectTarget::Filter(PermanentFilter {
                controller: ControllerConstraint::SameAsSource,
                all_card_types: BTreeSet::from([PermanentType::Creature]),
                token: Some(true),
                ..PermanentFilter::default()
            })
        }
        StaticCreatureModifierTarget::CreaturesYouControlOfChosenType => {
            EffectTarget::Filter(PermanentFilter {
                controller: ControllerConstraint::SameAsSource,
                all_card_types: BTreeSet::from([PermanentType::Creature]),
                creature_type: Some(CreatureTypeConstraint::ChosenBySource),
                ..PermanentFilter::default()
            })
        }
        StaticCreatureModifierTarget::CreatureEnchantedBySource
        | StaticCreatureModifierTarget::CreatureEquippedBySource => EffectTarget::AttachedToSource,
        StaticCreatureModifierTarget::CreaturesYouControlThatAreEnchantedOrEquipped => {
            EffectTarget::Filter(PermanentFilter {
                controller: ControllerConstraint::SameAsSource,
                all_card_types: BTreeSet::from([PermanentType::Creature]),
                attachment: CombatAttachmentConstraint::AttachedByAnyOf(BTreeSet::from([
                    CombatAttachmentKind::Aura,
                    CombatAttachmentKind::Equipment,
                ])),
                ..PermanentFilter::default()
            })
        }
    }
}

fn battlefield_grants_all_creature_types(deck: &CompiledDeck, zones: &KnownLineZoneState) -> bool {
    zones.battlefield.iter().any(|presence| {
        deck.cards.get(presence.card_index).is_some_and(|card| {
            card.ability_program.executable_abilities().any(|ability| {
                ability.effects.iter().any(|effect| {
                    matches!(
                        effect,
                        AbilityEffect::GrantAllCreatureTypes(grant)
                            if grant.creatures_you_control
                    )
                })
            })
        })
    })
}

fn combat_effect_runtime(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
) -> Option<(CombatEffectState, CombatEffectSet)> {
    const YOU: PlayerId = PlayerId(0);
    let all_creature_types = battlefield_grants_all_creature_types(deck, zones);
    let mut permanents = Vec::new();
    for presence in &zones.battlefield {
        let card = deck.cards.get(presence.card_index)?;
        let mut snapshot = PermanentSnapshot::new(
            combat_object_id(presence.sequence),
            YOU,
            permanent_types_for_card(card),
        );
        snapshot.base_power = card.printed_power.map(i32::from);
        snapshot.base_toughness = card.printed_toughness.map(i32::from);
        let counters = i32::from(
            zones
                .creature_power_counters
                .get(&presence.sequence)
                .copied()
                .unwrap_or_default(),
        );
        let temporary = zones
            .temporary_power_toughness_adjustments
            .get(&presence.sequence)
            .copied()
            .unwrap_or_default();
        snapshot.power_adjustment = counters.saturating_add(i32::from(temporary.0));
        snapshot.toughness_adjustment = counters.saturating_add(i32::from(temporary.1));
        snapshot.printed_keywords = printed_combat_keywords(card);
        snapshot.has_all_creature_types =
            all_creature_types || card_has_keyword(card, "Changeling");
        snapshot.creature_types = creature_types_from_type_line(&card.type_line)
            .into_iter()
            .filter_map(|subtype| CreatureType::new(subtype).ok())
            .collect();
        let source_has_chosen_type = card.ability_program.executable_abilities().any(|ability| {
            ability
                .effects
                .iter()
                .any(|effect| matches!(effect, AbilityEffect::SourceHasChosenCreatureType(_)))
        });
        if source_has_chosen_type
            && let Some(chosen) = zones.chosen_creature_types.get(&presence.sequence)
            && let Ok(chosen) = CreatureType::new(chosen)
        {
            snapshot.creature_types.insert(chosen);
        }
        permanents.push(snapshot);
    }
    for token in &zones.creature_tokens {
        let mut snapshot = PermanentSnapshot::new(
            combat_object_id(token.sequence),
            YOU,
            [PermanentType::Creature],
        );
        snapshot.is_token = true;
        snapshot.base_power = Some(i32::from(token.base_power));
        snapshot.base_toughness = Some(i32::from(token.base_toughness));
        let temporary = zones
            .temporary_power_toughness_adjustments
            .get(&token.sequence)
            .copied()
            .unwrap_or_default();
        snapshot.power_adjustment =
            i32::from(token.combat_power_counters).saturating_add(i32::from(temporary.0));
        snapshot.toughness_adjustment =
            i32::from(token.combat_power_counters).saturating_add(i32::from(temporary.1));
        snapshot.printed_keywords = token.printed_keywords.clone();
        snapshot.has_all_creature_types = all_creature_types;
        snapshot.creature_types = token
            .creature_types
            .iter()
            .filter_map(|subtype| CreatureType::new(subtype).ok())
            .collect();
        permanents.push(snapshot);
    }

    let mut state = CombatEffectState::new(permanents).ok()?;
    for (source_sequence, attachment) in &zones.attachments {
        state
            .attach(
                combat_object_id(*source_sequence),
                combat_object_id(attachment.target_sequence),
                attachment.kind,
            )
            .ok()?;
    }
    for (source_sequence, creature_type) in &zones.chosen_creature_types {
        state
            .set_chosen_creature_type(
                combat_object_id(*source_sequence),
                CreatureType::new(creature_type).ok()?,
            )
            .ok()?;
    }

    let mut effects = CombatEffectSet::default();
    for presence in &zones.battlefield {
        let card = deck.cards.get(presence.card_index)?;
        let source = combat_object_id(presence.sequence);
        for ability in card.ability_program.executable_abilities() {
            for effect in &ability.effects {
                let AbilityEffect::ApplyStaticCreatureModifier(modifier) = effect else {
                    continue;
                };
                let target = static_modifier_target(modifier.target);
                effects.modifiers.push(ContinuousModifier {
                    source,
                    target: target.clone(),
                    power: static_modifier_value(&modifier.power_delta),
                    toughness: static_modifier_value(&modifier.toughness_delta),
                });
                if !modifier.granted_keywords.is_empty() {
                    effects.keyword_grants.push(KeywordGrant {
                        source,
                        target,
                        keywords: modifier
                            .granted_keywords
                            .iter()
                            .copied()
                            .map(program_keyword_to_combat)
                            .collect(),
                    });
                }
            }
        }
    }
    Some((state, effects))
}

fn trigger_multiplier_count(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    event: CombatTriggerEventKind,
    triggering_sequence: Option<u16>,
    ability_source_sequence: u16,
) -> u32 {
    let Some((state, _)) = combat_effect_runtime(deck, zones) else {
        return 1;
    };
    let mut multipliers = Vec::new();
    for presence in &zones.battlefield {
        let Some(card) = deck.cards.get(presence.card_index) else {
            continue;
        };
        for ability in card.ability_program.executable_abilities() {
            let Ok(clause_index) = u16::try_from(ability.clause_index) else {
                continue;
            };
            for effect in &ability.effects {
                let AbilityEffect::MultiplyTriggeredAbility(multiplier) = effect else {
                    continue;
                };
                let (multiplier_event, triggering_object) = match &multiplier.event {
                    TriggerMultiplierEvent::PermanentEntering { any_of_card_types } => {
                        let any_card_types = any_of_card_types
                            .iter()
                            .filter_map(|card_type| match card_type {
                                ProgramCardType::Artifact => Some(PermanentType::Artifact),
                                ProgramCardType::Creature => Some(PermanentType::Creature),
                                _ => None,
                            })
                            .collect::<BTreeSet<_>>();
                        (
                            CombatTriggerEventKind::PermanentEnteredBattlefield,
                            Some(PermanentFilter {
                                any_card_types,
                                ..PermanentFilter::default()
                            }),
                        )
                    }
                    TriggerMultiplierEvent::AnyTriggeredAbility => (event, None),
                };
                let ability_source = match multiplier.ability_source {
                    TriggerAbilitySource::PermanentYouControl => PermanentFilter {
                        controller: ControllerConstraint::SameAsSource,
                        ..PermanentFilter::default()
                    },
                    TriggerAbilitySource::OtherCreatureYouControlOfChosenType => PermanentFilter {
                        controller: ControllerConstraint::SameAsSource,
                        all_card_types: BTreeSet::from([PermanentType::Creature]),
                        creature_type: Some(CreatureTypeConstraint::ChosenBySource),
                        exclude_source: true,
                        ..PermanentFilter::default()
                    },
                };
                multipliers.push(CombatTriggerMultiplier {
                    id: CombatEffectId {
                        source: combat_object_id(presence.sequence),
                        clause_index,
                    },
                    event: multiplier_event,
                    triggering_object,
                    ability_source,
                    additional_triggers: multiplier.additional_times,
                });
            }
        }
    }
    state
        .expanded_trigger_count(
            1,
            CombatTriggerContext {
                event,
                triggering_object: triggering_sequence.map(combat_object_id),
                ability_source: combat_object_id(ability_source_sequence),
            },
            &multipliers,
        )
        .unwrap_or(1)
}

fn equipment_activation_cost(card: &CompiledCard) -> Option<&ProgramManaCost> {
    card.ability_program
        .executable_abilities()
        .find_map(|ability| {
            if ability.timing
                != (AbilityTiming::Activated {
                    window: ActivationWindow::SorcerySpeedOnly,
                })
                || ability.costs.len() != 1
                || ability.effects.len() != 1
            {
                return None;
            }
            let AbilityCost::Mana(cost) = &ability.costs[0] else {
                return None;
            };
            let AbilityEffect::AttachSourceToTarget(attachment) = &ability.effects[0] else {
                return None;
            };
            (attachment.attachment_kind == ProgramAttachmentKind::Equipment
                && attachment.target.card_type == Some(ProgramCardType::Creature)
                && attachment.target.controller == Some(ControllerRelation::You))
            .then_some(cost)
        })
}

fn equipment_grants_shroud(card: &CompiledCard) -> bool {
    card.ability_program.executable_abilities().any(|ability| {
        ability.effects.iter().any(|effect| {
            matches!(
                effect,
                AbilityEffect::ApplyStaticCreatureModifier(modifier)
                    if modifier.target == StaticCreatureModifierTarget::CreatureEquippedBySource
                        && modifier
                            .granted_keywords
                            .contains(&GrantedCreatureKeyword::Shroud)
            )
        })
    })
}

fn equipped_creature_dies_draw_count(card: &CompiledCard) -> u32 {
    let expected_filter = ProgramObjectFilter {
        card_type: Some(ProgramCardType::Creature),
        controller: Some(ControllerRelation::You),
        ..ProgramObjectFilter::default()
    };
    card.ability_program
        .executable_abilities()
        .filter_map(|ability| {
            let AbilityTiming::Triggered { event } = &ability.timing else {
                return None;
            };
            if event.kind != TriggerEventKind::EquippedCreatureDies
                || event.actor != ControllerRelation::You
                || event.object_filter != expected_filter
                || !ability.costs.is_empty()
                || ability.preconditions
                    != [
                        AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                        AbilityPrecondition::EventObjectMatches(expected_filter.clone()),
                    ]
            {
                return None;
            }
            let [AbilityEffect::Draw(draw)] = ability.effects.as_slice() else {
                return None;
            };
            (draw.count > 0 && draw.unless_event_player_pays.is_none())
                .then_some(u32::from(draw.count))
        })
        .fold(0u32, u32::saturating_add)
}

#[allow(clippy::too_many_arguments)]
fn choose_equipment_target_sequence(
    deck: &CompiledDeck,
    card: &CompiledCard,
    source_sequence: u16,
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: &TurnManaPool,
    state: &CombatEffectState,
    effects: &CombatEffectSet,
) -> Option<u16> {
    let legal = PermanentFilter {
        controller: ControllerConstraint::SameAsSource,
        all_card_types: BTreeSet::from([PermanentType::Creature]),
        ..PermanentFilter::default()
    };

    // A typed equipped-death draw trigger makes a zero-toughness attachment a
    // real card-access action rather than a failed combat buff. Prefer a token
    // when several exact lethal targets exist, then the lowest stable object
    // identity. No hidden information or card identity participates.
    if equipped_creature_dies_draw_count(card) > 0
        && let Some(target) = state
            .permanents()
            .filter(|permanent| {
                permanent.id != combat_object_id(source_sequence) && permanent.is_creature()
            })
            .filter_map(|permanent| {
                let mut candidate = state.clone();
                candidate
                    .attach(
                        combat_object_id(source_sequence),
                        permanent.id,
                        CombatAttachmentKind::Equipment,
                    )
                    .ok()?;
                let profile = candidate.evaluate_creature(permanent.id, effects).ok()?;
                profile
                    .toughness
                    .is_some_and(|toughness| toughness <= 0)
                    .then_some((permanent.is_token, permanent.id))
            })
            .min_by_key(|(is_token, object)| (!*is_token, *object))
            .map(|(_, object)| object)
    {
        return u16::try_from(target.0.checked_sub(1)?).ok();
    }

    // This helper exists only to improve the imminent combat. Do not spend an
    // equip activation on a body that cannot attack this turn when a legal
    // attack-capable target exists. Candidate legality is evaluated after the
    // attachment is moved so a typed haste grant can make a newly entered
    // creature eligible. The exact runtime uses the same stable combat score as
    // ordinary attachment selection, with no name- or deck-specific branch.
    let target = state
        .permanents()
        .filter(|permanent| {
            permanent.id != combat_object_id(source_sequence) && permanent.is_creature()
        })
        .filter_map(|permanent| {
            if !state
                .permanent_matches(
                    permanent.id,
                    combat_object_id(source_sequence),
                    &legal,
                    effects,
                )
                .ok()?
            {
                return None;
            }
            let target_sequence = u16::try_from(permanent.id.0.checked_sub(1)?).ok()?;
            let mut candidate_zones = zones.clone();
            candidate_zones.attachments.insert(
                source_sequence,
                BattlefieldAttachment {
                    target_sequence,
                    kind: CombatAttachmentKind::Equipment,
                },
            );
            if !combat_attack_eligible_sequences(deck, &candidate_zones, turn, Some(mana_pool))
                .contains(&target_sequence)
            {
                return None;
            }
            let mut candidate = state.clone();
            candidate
                .attach(
                    combat_object_id(source_sequence),
                    permanent.id,
                    CombatAttachmentKind::Equipment,
                )
                .ok()?;
            let profile = candidate.evaluate_creature(permanent.id, effects).ok()?;
            let access_rank = [
                CombatKeyword::CantBeBlocked,
                CombatKeyword::Flying,
                CombatKeyword::Menace,
                CombatKeyword::Trample,
            ]
            .into_iter()
            .filter(|keyword| profile.keywords.contains(keyword))
            .count() as u8;
            Some((
                (
                    profile.projected_unblocked_damage,
                    profile.power,
                    access_rank,
                    Reverse(permanent.id),
                ),
                permanent.id,
            ))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, object)| object)?;
    u16::try_from(target.0.checked_sub(1)?).ok()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct EquipmentStateBasedResolution {
    removed_card_indices: Vec<usize>,
    removed_creature_tokens: u16,
}

impl EquipmentStateBasedResolution {
    fn extend(&mut self, other: Self) {
        self.removed_card_indices.extend(other.removed_card_indices);
        self.removed_creature_tokens = self
            .removed_creature_tokens
            .saturating_add(other.removed_creature_tokens);
    }
}

fn equipped_creature_death_trigger_draws(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    creature_sequence: u16,
) -> u32 {
    let mut equipment_sources = zones
        .attachments
        .iter()
        .filter_map(|(source_sequence, attachment)| {
            (attachment.kind == CombatAttachmentKind::Equipment
                && attachment.target_sequence == creature_sequence)
                .then_some(*source_sequence)
        })
        .collect::<Vec<_>>();
    equipment_sources.sort_unstable();
    equipment_sources
        .into_iter()
        .filter_map(|source_sequence| {
            let source = zones
                .battlefield
                .iter()
                .find(|presence| presence.sequence == source_sequence)
                .and_then(|presence| deck.cards.get(presence.card_index))?;
            let base_draws = equipped_creature_dies_draw_count(source);
            if base_draws == 0 {
                return None;
            }
            let trigger_count = trigger_multiplier_count(
                deck,
                zones,
                CombatTriggerEventKind::CreatureDied,
                Some(creature_sequence),
                source_sequence,
            );
            Some(base_draws.saturating_mul(trigger_count))
        })
        .fold(0u32, u32::saturating_add)
}

#[allow(clippy::while_let_loop)]
fn resolve_nonpositive_toughness_state_based_actions(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
) -> EquipmentStateBasedResolution {
    let mut resolution = EquipmentStateBasedResolution::default();
    loop {
        let Some((state, effects)) = combat_effect_runtime(deck, zones) else {
            break;
        };
        let mut lethal_sequences = zones
            .battlefield
            .iter()
            .map(|presence| presence.sequence)
            .chain(zones.creature_tokens.iter().map(|token| token.sequence))
            .filter(|sequence| {
                state
                    .evaluate_creature(combat_object_id(*sequence), &effects)
                    .ok()
                    .and_then(|profile| profile.toughness)
                    .is_some_and(|toughness| toughness <= 0)
            })
            .collect::<Vec<_>>();
        lethal_sequences.sort_unstable();
        lethal_sequences.dedup();
        if lethal_sequences.is_empty() {
            break;
        }

        let pending_draws = lethal_sequences
            .iter()
            .copied()
            .map(|sequence| equipped_creature_death_trigger_draws(deck, zones, sequence))
            .fold(0u32, u32::saturating_add);
        for sequence in lethal_sequences {
            if let Some(removed) =
                zones.remove_permanent_sequence_with_attached_auras(deck, sequence, true)
            {
                resolution.removed_card_indices.extend(removed);
                continue;
            }
            let Some(position) = zones
                .creature_tokens
                .iter()
                .position(|token| token.sequence == sequence)
            else {
                continue;
            };
            resolution
                .removed_card_indices
                .extend(zones.remove_attached_aura_permanents(deck, sequence));
            zones.creature_tokens.remove(position);
            zones.remove_object_state(sequence);
            zones.advance_sequence();
            resolution.removed_creature_tokens =
                resolution.removed_creature_tokens.saturating_add(1);
        }
        zones.pending_card_draws = zones.pending_card_draws.saturating_add(pending_draws);
    }
    resolution
}

fn pay_program_mana_cost(mana_pool: &mut TurnManaPool, cost: &ProgramManaCost) -> bool {
    let ProgramManaCost::PrintedSymbols { oracle, .. } = cost else {
        return false;
    };
    let parsed = parse_mana_cost(Some(oracle));
    parsed.confidence >= 0.999 && mana_pool.pay(Some(&parsed), 0, 0)
}

fn activate_equipment_for_combat(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    mana_pool: &mut TurnManaPool,
    turn: u8,
) -> EquipmentStateBasedResolution {
    let mut resolution = EquipmentStateBasedResolution::default();
    let mut equipment = zones
        .battlefield
        .iter()
        .filter_map(|presence| {
            let card = deck.cards.get(presence.card_index)?;
            equipment_activation_cost(card)?;
            Some((
                equipment_grants_shroud(card),
                presence.sequence,
                presence.card_index,
            ))
        })
        .collect::<Vec<_>>();
    // Targeted equip activations happen before a shroud-granting Equipment is
    // moved onto the final target. This is a legal deterministic sequence,
    // not an exception to shroud.
    equipment.sort_by_key(|(grants_shroud, sequence, card_index)| {
        (*grants_shroud, *sequence, *card_index)
    });

    for (_, source_sequence, card_index) in equipment {
        let Some(card) = deck.cards.get(card_index) else {
            continue;
        };
        let Some(cost) = equipment_activation_cost(card) else {
            continue;
        };
        let Some((state, effects)) = combat_effect_runtime(deck, zones) else {
            continue;
        };
        let Some(target_sequence) = choose_equipment_target_sequence(
            deck,
            card,
            source_sequence,
            zones,
            turn,
            mana_pool,
            &state,
            &effects,
        ) else {
            continue;
        };
        if zones
            .attachments
            .get(&source_sequence)
            .is_some_and(|attachment| attachment.target_sequence == target_sequence)
        {
            continue;
        }
        let mut paid = mana_pool.clone();
        if !pay_program_mana_cost(&mut paid, cost) {
            continue;
        }
        zones.attachments.insert(
            source_sequence,
            BattlefieldAttachment {
                target_sequence,
                kind: CombatAttachmentKind::Equipment,
            },
        );
        *mana_pool = paid;
        resolution.extend(resolve_nonpositive_toughness_state_based_actions(
            deck, zones,
        ));
    }
    resolution
}

fn card_has_oracle_paragraph(card: &CompiledCard, expected: &str) -> bool {
    let expected = expected.trim().trim_end_matches('.').to_ascii_lowercase();
    card.ability_program.abilities.iter().any(|ability| {
        let oracle = match ability {
            AbilityCompilation::Executable(ability) => &ability.normalized_oracle,
            AbilityCompilation::Unsupported(ability) => &ability.normalized_oracle,
        };
        oracle.trim().trim_end_matches('.').to_ascii_lowercase() == expected
    })
}

fn four_four_flying_creature_token_replacement_is_active(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
) -> bool {
    zones.battlefield.iter().any(|presence| {
        deck.cards.get(presence.card_index).is_some_and(|card| {
            card_has_oracle_paragraph(
                card,
                "if one or more creature tokens would be created under your control, that many 4/4 white Angel creature tokens with flying and vigilance are created instead",
            )
        })
    })
}

fn combat_attack_eligible_sequences(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: Option<&TurnManaPool>,
) -> BTreeSet<u16> {
    let combat_runtime = combat_effect_runtime(deck, zones);
    combat_attack_eligible_sequences_with_runtime(
        deck,
        zones,
        turn,
        mana_pool,
        combat_runtime.as_ref(),
    )
}

fn combat_attack_eligible_sequences_with_runtime(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: Option<&TurnManaPool>,
    combat_runtime: Option<&(CombatEffectState, CombatEffectSet)>,
) -> BTreeSet<u16> {
    let physically_tapped_sequences = mana_pool
        .into_iter()
        .flat_map(|pool| &pool.sources)
        .filter(|source| source.physically_tapped)
        .filter_map(|source| source.origin_sequence)
        .collect::<BTreeSet<_>>();
    let mut tapped_mana_creatures = mana_pool
        .into_iter()
        .flat_map(|pool| &pool.sources)
        .filter(|source| {
            !source.is_land
                && source.origin_sequence.is_none()
                && source.physically_tapped
                && source.activation_used
        })
        .filter_map(|source| source.origin_card_index)
        .fold(HashMap::<usize, usize>::new(), |mut counts, card_index| {
            *counts.entry(card_index).or_default() += 1;
            counts
        });
    let mut eligible = BTreeSet::new();
    for presence in &zones.battlefield {
        let Some(card) = deck.cards.get(presence.card_index) else {
            continue;
        };
        if !card_is_bounded_attack_capable(card) {
            continue;
        }
        if zones
            .tapped_creatures_this_turn
            .contains(&presence.sequence)
            || physically_tapped_sequences.contains(&presence.sequence)
        {
            if let Some(remaining) = tapped_mana_creatures.get_mut(&presence.card_index)
                && *remaining > 0
            {
                *remaining -= 1;
            }
            continue;
        }
        if tapped_mana_creatures
            .get_mut(&presence.card_index)
            .is_some_and(|remaining| {
                if *remaining == 0 {
                    false
                } else {
                    *remaining -= 1;
                    true
                }
            })
        {
            continue;
        }
        let profile = combat_runtime.and_then(|(state, effects)| {
            state
                .evaluate_creature(combat_object_id(presence.sequence), effects)
                .ok()
        });
        let has_haste = profile
            .as_ref()
            .is_some_and(|profile| profile.keywords.contains(&CombatKeyword::Haste))
            || card_has_keyword(card, "Haste");
        if presence.entered_turn < turn || has_haste {
            eligible.insert(presence.sequence);
        }
    }
    for token in &zones.creature_tokens {
        if zones.tapped_creatures_this_turn.contains(&token.sequence)
            || physically_tapped_sequences.contains(&token.sequence)
        {
            continue;
        }
        let profile = combat_runtime.and_then(|(state, effects)| {
            state
                .evaluate_creature(combat_object_id(token.sequence), effects)
                .ok()
        });
        let has_haste = profile
            .as_ref()
            .is_some_and(|profile| profile.keywords.contains(&CombatKeyword::Haste));
        if token.entered_turn < turn || has_haste {
            eligible.insert(token.sequence);
        }
    }
    eligible
}

fn combat_attackers(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: Option<&TurnManaPool>,
) -> Vec<CombatAttacker> {
    let combat_runtime = combat_effect_runtime(deck, zones);
    let track_commander_damage = deck.commanders.len() == 1;
    let eligible = combat_attack_eligible_sequences_with_runtime(
        deck,
        zones,
        turn,
        mana_pool,
        combat_runtime.as_ref(),
    );
    let mut attackers = zones
        .battlefield
        .iter()
        .filter_map(|presence| {
            if !eligible.contains(&presence.sequence) {
                return None;
            }
            let card = deck.cards.get(presence.card_index)?;
            let profile = combat_runtime.as_ref().and_then(|(state, effects)| {
                state
                    .evaluate_creature(combat_object_id(presence.sequence), effects)
                    .ok()
            });
            let base_power = profile.as_ref().map(|profile| profile.power).or_else(|| {
                card.printed_power.map(|power| {
                    i32::from(power)
                        + i32::from(
                            zones
                                .creature_power_counters
                                .get(&presence.sequence)
                                .copied()
                                .unwrap_or_default(),
                        )
                })
            })?;
            let combat_power = base_power.max(0) as u32;
            let damage_steps = profile
                .as_ref()
                .map_or(1, |profile| profile.unblocked_damage_steps);
            let projected_combat_damage = combat_power.saturating_mul(u32::from(damage_steps));
            (projected_combat_damage > 0).then_some(CombatAttacker {
                attacker_id: u32::from(presence.sequence).saturating_add(1),
                projected_combat_damage,
                is_tracked_commander: track_commander_damage
                    && deck.commanders.first() == Some(&presence.card_index),
            })
        })
        .collect::<Vec<_>>();
    attackers.extend(zones.creature_tokens.iter().filter_map(|token| {
        if !eligible.contains(&token.sequence) {
            return None;
        }
        let profile = combat_runtime.as_ref().and_then(|(state, effects)| {
            state
                .evaluate_creature(combat_object_id(token.sequence), effects)
                .ok()
        });
        let combat_power = profile
            .as_ref()
            .map(|profile| profile.power.max(0) as u32)
            .unwrap_or_else(|| {
                u32::from(token.base_power).saturating_add(u32::from(token.combat_power_counters))
            });
        let damage_steps = profile
            .as_ref()
            .map_or(1, |profile| profile.unblocked_damage_steps);
        let projected_combat_damage = combat_power.saturating_mul(u32::from(damage_steps));
        (projected_combat_damage > 0).then_some(CombatAttacker {
            attacker_id: u32::from(token.sequence).saturating_add(1),
            projected_combat_damage,
            is_tracked_commander: false,
        })
    }));
    attackers
}

fn plan_combat_attack(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    state: &CommanderCombatState,
    mana_pool: &TurnManaPool,
) -> Option<PresentedAttack> {
    let attackers = combat_attackers(deck, zones, turn, Some(mana_pool));
    (!attackers.is_empty())
        .then(|| allocate_attack(state, &attackers).ok())
        .flatten()
}

fn record_nonvigilance_attack_taps(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    mana_pool: &mut TurnManaPool,
    attack: &PresentedAttack,
) {
    for assignment in attack.assignments() {
        let Some(sequence) = assignment
            .attacker_id
            .checked_sub(1)
            .and_then(|value| u16::try_from(value).ok())
        else {
            continue;
        };
        let has_vigilance = combat_profile_for_attacker(deck, zones, assignment.attacker_id)
            .is_some_and(|profile| profile.keywords.contains(&CombatKeyword::Vigilance))
            || zones
                .battlefield
                .iter()
                .find(|presence| presence.sequence == sequence)
                .and_then(|presence| deck.cards.get(presence.card_index))
                .is_some_and(|card| {
                    printed_combat_keywords(card).contains(&CombatKeyword::Vigilance)
                })
            || zones
                .creature_tokens
                .iter()
                .find(|token| token.sequence == sequence)
                .is_some_and(|token| token.printed_keywords.contains(&CombatKeyword::Vigilance));
        if !has_vigilance {
            zones.tapped_creatures_this_turn.insert(sequence);
            for source in &mut mana_pool.sources {
                if source.origin_sequence == Some(sequence) {
                    source.remaining = 0;
                    source.physically_tapped = true;
                    source.activation_used = true;
                }
            }
        }
    }
}

fn deck_has_executable_combat_route(deck: &CompiledDeck) -> bool {
    deck.cards.iter().any(|card| {
        card.quantity > 0
            && (card_is_bounded_attack_capable(card)
                && card.printed_power.is_some_and(|power| power > 0)
                || card
                    .ability_program
                    .executable_abilities()
                    .flat_map(|ability| &ability.effects)
                    .any(|effect| {
                        matches!(
                            effect,
                            AbilityEffect::CreateToken(token)
                                if matches!(token.kind, TokenKind::Creature { power, .. } if power > 0)
                        )
                    }))
    })
}

fn resolve_all_connected_combat_damage(
    state: &mut CommanderCombatState,
    attack: &PresentedAttack,
) -> bool {
    let connected = attack
        .assignments()
        .iter()
        .map(|assignment| ConnectedCombatDamage {
            attacker_id: assignment.attacker_id,
            combat_damage: assignment.assigned_combat_damage,
        })
        .collect::<Vec<_>>();
    state
        .resolve_presented_attack(attack, &connected)
        .ok()
        .is_some_and(|resolution| resolution.resolved_table_win)
}

fn combat_profile_for_attacker(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    attacker_id: u32,
) -> Option<crate::combat_effects::CreatureCombatProfile> {
    let sequence = u16::try_from(attacker_id.checked_sub(1)?).ok()?;
    let (state, effects) = combat_effect_runtime(deck, zones)?;
    state
        .evaluate_creature(combat_object_id(sequence), &effects)
        .ok()
}

fn draw_count_from_effects(effects: &[AbilityEffect]) -> u32 {
    effects
        .iter()
        .filter_map(|effect| {
            let AbilityEffect::Draw(draw) = effect else {
                return None;
            };
            (draw.unless_event_player_pays.is_none()).then_some(u32::from(draw.count))
        })
        .sum()
}

fn attacker_matches_source_chosen_type(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    attacker_id: u32,
    source_sequence: u16,
) -> bool {
    let Some(attacker_sequence) = attacker_id
        .checked_sub(1)
        .and_then(|sequence| u16::try_from(sequence).ok())
    else {
        return false;
    };
    let Some((state, effects)) = combat_effect_runtime(deck, zones) else {
        return false;
    };
    state
        .permanent_matches(
            combat_object_id(attacker_sequence),
            combat_object_id(source_sequence),
            &PermanentFilter {
                controller: ControllerConstraint::SameAsSource,
                all_card_types: BTreeSet::from([PermanentType::Creature]),
                creature_type: Some(CreatureTypeConstraint::ChosenBySource),
                ..PermanentFilter::default()
            },
            &effects,
        )
        .unwrap_or(false)
}

fn combat_attack_draw_count(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    attack: &PresentedAttack,
) -> u32 {
    let sources = zones.battlefield.clone();
    attack.assignments().iter().fold(0u32, |total, assignment| {
        let draws = sources
            .iter()
            .filter_map(|presence| {
                let source = deck.cards.get(presence.card_index)?;
                let count = source
                    .ability_program
                    .executable_abilities()
                    .filter_map(|ability| {
                        let AbilityTiming::Triggered { event } = &ability.timing else {
                            return None;
                        };
                        (event.kind == TriggerEventKind::ChosenTypeCreatureEntersOrAttacks
                            && attacker_matches_source_chosen_type(
                                deck,
                                zones,
                                assignment.attacker_id,
                                presence.sequence,
                            ))
                        .then(|| {
                            let triggering_sequence = assignment
                                .attacker_id
                                .checked_sub(1)
                                .and_then(|sequence| u16::try_from(sequence).ok());
                            draw_count_from_effects(&ability.effects).saturating_mul(
                                trigger_multiplier_count(
                                    deck,
                                    zones,
                                    CombatTriggerEventKind::CreatureAttacked,
                                    triggering_sequence,
                                    presence.sequence,
                                ),
                            )
                        })
                    })
                    .sum::<u32>();
                Some(count)
            })
            .sum::<u32>();
        total.saturating_add(draws)
    })
}

fn combat_damage_draw_count(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    attack: &PresentedAttack,
) -> u32 {
    let sources = zones.battlefield.clone();
    let per_creature_and_attached = attack
        .assignments()
        .iter()
        .filter(|assignment| assignment.assigned_combat_damage > 0)
        .fold(0u32, |total, assignment| {
            let steps = combat_profile_for_attacker(deck, zones, assignment.attacker_id)
                .map_or(1, |profile| u32::from(profile.unblocked_damage_steps));
            let triggering_sequence = assignment
                .attacker_id
                .checked_sub(1)
                .and_then(|sequence| u16::try_from(sequence).ok());
            let per_creature_draws = sources
                .iter()
                .filter_map(|presence| {
                    let source = deck.cards.get(presence.card_index)?;
                    Some(
                        source
                            .ability_program
                            .executable_abilities()
                            .filter_map(|ability| {
                                let AbilityTiming::Triggered { event } = &ability.timing else {
                                    return None;
                                };
                                (event.kind == TriggerEventKind::CreatureDealsCombatDamageToPlayer)
                                    .then(|| {
                                        draw_count_from_effects(&ability.effects).saturating_mul(
                                            trigger_multiplier_count(
                                                deck,
                                                zones,
                                                CombatTriggerEventKind::CreatureDealtCombatDamage,
                                                triggering_sequence,
                                                presence.sequence,
                                            ),
                                        )
                                    })
                            })
                            .sum::<u32>(),
                    )
                })
                .sum::<u32>();
            let attached_draws = zones
                .attachments
                .iter()
                .filter(|(_, attachment)| {
                    u32::from(attachment.target_sequence).saturating_add(1)
                        == assignment.attacker_id
                        && attachment.kind == CombatAttachmentKind::Aura
                })
                .filter_map(|(source_sequence, _)| {
                    let source_index = zones
                        .battlefield
                        .iter()
                        .find(|presence| presence.sequence == *source_sequence)?
                        .card_index;
                    let source = deck.cards.get(source_index)?;
                    Some(
                        source
                            .ability_program
                            .executable_abilities()
                            .filter_map(|ability| {
                                let AbilityTiming::Triggered { event } = &ability.timing else {
                                    return None;
                                };
                                (event.kind
                                    == TriggerEventKind::EnchantedCreatureDealsDamageToOpponent)
                                    .then(|| {
                                        draw_count_from_effects(&ability.effects).saturating_mul(
                                            trigger_multiplier_count(
                                                deck,
                                                zones,
                                                CombatTriggerEventKind::CreatureDealtCombatDamage,
                                                triggering_sequence,
                                                *source_sequence,
                                            ),
                                        )
                                    })
                            })
                            .sum::<u32>(),
                    )
                })
                .sum::<u32>();
            total.saturating_add(
                per_creature_draws
                    .saturating_add(attached_draws)
                    .saturating_mul(steps),
            )
        });

    // “One or more” combat-damage triggers occur once for each damaged
    // player in each combat-damage step, regardless of how many creatures
    // connected with that player in the same step.
    let mut damage_steps_by_opponent = HashMap::<OpponentId, u32>::new();
    for assignment in attack
        .assignments()
        .iter()
        .filter(|assignment| assignment.assigned_combat_damage > 0)
    {
        let steps = combat_profile_for_attacker(deck, zones, assignment.attacker_id)
            .map_or(1, |profile| u32::from(profile.unblocked_damage_steps));
        damage_steps_by_opponent
            .entry(assignment.opponent)
            .and_modify(|current| *current = (*current).max(steps))
            .or_insert(steps);
    }
    let grouped_occurrences = damage_steps_by_opponent.values().copied().sum::<u32>();
    let grouped_draws = sources
        .iter()
        .filter_map(|presence| {
            let source = deck.cards.get(presence.card_index)?;
            let draws_per_occurrence = source
                .ability_program
                .executable_abilities()
                .filter_map(|ability| {
                    let AbilityTiming::Triggered { event } = &ability.timing else {
                        return None;
                    };
                    (event.kind == TriggerEventKind::OneOrMoreCreaturesDealCombatDamageToPlayer)
                        .then_some(draw_count_from_effects(&ability.effects))
                })
                .sum::<u32>();
            (draws_per_occurrence > 0).then(|| {
                draws_per_occurrence
                    .saturating_mul(grouped_occurrences)
                    .saturating_mul(trigger_multiplier_count(
                        deck,
                        zones,
                        CombatTriggerEventKind::CreatureDealtCombatDamage,
                        None,
                        presence.sequence,
                    ))
            })
        })
        .sum::<u32>();

    per_creature_and_attached.saturating_add(grouped_draws)
}

fn draw_bounded_cards(
    hand: &mut Vec<usize>,
    library_order: &[usize],
    next_draw_position: &mut usize,
    count: u32,
) {
    for _ in 0..count {
        let Some(card_index) = library_order.get(*next_draw_position) else {
            break;
        };
        hand.push(*card_index);
        *next_draw_position = (*next_draw_position).saturating_add(1);
    }
}

fn combat_lifelink_gain(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    attack: &PresentedAttack,
) -> f32 {
    attack
        .assignments()
        .iter()
        .filter(|assignment| {
            combat_profile_for_attacker(deck, zones, assignment.attacker_id)
                .is_some_and(|profile| profile.keywords.contains(&CombatKeyword::Lifelink))
        })
        .map(|assignment| assignment.assigned_combat_damage as f32)
        .sum()
}

fn card_has_subtype(card: &CompiledCard, subtype: &str) -> bool {
    type_line_has_subtype(&card.type_line, subtype) || card_has_keyword(card, "Changeling")
}

fn printed_card_colors(card: &CompiledCard) -> ManaColorMask {
    mana_color_mask_from_symbols(&card.colors)
}

fn card_has_visible_multiface_identity(card: &CompiledCard) -> bool {
    card.name.contains(" // ")
        || card.type_line.contains(" // ")
        || !card.ability_program.face_programs.is_empty()
}

fn legacy_single_face_hand_characteristics_available(card: &CompiledCard) -> bool {
    !card_has_visible_multiface_identity(card) && !card.type_line.trim().is_empty()
}

fn printed_hand_card_colors(card: &CompiledCard) -> Option<ManaColorMask> {
    let hand = card.hand_zone_characteristics();
    if hand.exact {
        return Some(mana_color_mask_from_symbols(&hand.colors));
    }
    // Legacy local snapshots can lack the retained layout/face envelope while
    // still retaining an unambiguous single-face type line and Scryfall color
    // characteristic. The strict execution manifest continues to block a
    // certified result for those records; this bounded trajectory fallback
    // only prevents ordinary single-face cards from becoming impossible
    // Chrome Mox imprints. Never merge a visible multiface root here.
    legacy_single_face_hand_characteristics_available(card).then(|| printed_card_colors(card))
}

fn mana_color_mask_from_symbols(symbols: &[String]) -> ManaColorMask {
    symbols
        .iter()
        .fold(ManaColorMask::NONE, |mut colors, symbol| {
            colors |= match symbol.trim().to_ascii_uppercase().as_str() {
                "W" => ManaColorMask::WHITE,
                "U" => ManaColorMask::BLUE,
                "B" => ManaColorMask::BLACK,
                "R" => ManaColorMask::RED,
                "G" => ManaColorMask::GREEN,
                _ => ManaColorMask::NONE,
            };
            colors
        })
}

fn battlefield_artifact_count(deck: &CompiledDeck, zones: &KnownLineZoneState) -> usize {
    zones
        .battlefield
        .iter()
        .filter(|presence| {
            deck.cards
                .get(presence.card_index)
                .is_some_and(|card| card.effects.card_types.is_artifact)
        })
        .count()
}

fn synchronize_mana_sources_with_battlefield(
    mana_sources: &mut Vec<BattlefieldManaSource>,
    zones: &KnownLineZoneState,
) {
    let mut remaining_by_card = HashMap::<usize, usize>::new();
    for presence in &zones.battlefield {
        *remaining_by_card.entry(presence.card_index).or_default() += 1;
    }
    mana_sources.retain(|source| {
        let Some(card_index) = source.card_index else {
            return true;
        };
        let Some(remaining) = remaining_by_card.get_mut(&card_index) else {
            return false;
        };
        if *remaining == 0 {
            return false;
        }
        *remaining -= 1;
        true
    });
}

fn synchronize_turn_pool_with_battlefield(
    mana_pool: &mut TurnManaPool,
    zones: &KnownLineZoneState,
) {
    let battlefield_sequences = zones
        .battlefield
        .iter()
        .map(|presence| presence.sequence)
        .collect::<BTreeSet<_>>();
    let mut remaining_by_card = HashMap::<usize, usize>::new();
    for presence in &zones.battlefield {
        *remaining_by_card.entry(presence.card_index).or_default() += 1;
    }
    mana_pool.sources.retain_mut(|source| {
        let Some(card_index) = source.origin_card_index else {
            return true;
        };
        if let Some(sequence) = source.origin_sequence {
            if battlefield_sequences.contains(&sequence) {
                return true;
            }
            if source.remaining == 0 {
                return false;
            }
            source.origin_card_index = None;
            source.origin_sequence = None;
            source.behavior = BattlefieldManaBehavior::Fixed;
            source.base_capacity = 0;
            source.activation_used = true;
            return true;
        }
        if let Some(remaining) = remaining_by_card.get_mut(&card_index)
            && *remaining > 0
        {
            *remaining -= 1;
            return true;
        }
        if source.remaining == 0 {
            return false;
        }
        // If the source was available, the controller may activate it in
        // response to removal and keep the resulting mana in the current
        // phase. Detach that floating mana from the removed permanent so a
        // later dynamic refresh cannot manufacture a new activation.
        source.origin_card_index = None;
        source.origin_sequence = None;
        source.behavior = BattlefieldManaBehavior::Fixed;
        source.base_capacity = 0;
        source.activation_used = true;
        true
    });
}

fn controlled_legendary_creature_or_planeswalker_colors(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
) -> ManaColorMask {
    zones
        .battlefield
        .iter()
        .filter_map(|presence| deck.cards.get(presence.card_index))
        .filter(|card| {
            type_line_has_subtype(&card.type_line, "Legendary")
                && (card.effects.card_types.is_creature
                    || type_line_has_subtype(&card.type_line, "Planeswalker"))
        })
        .fold(ManaColorMask::NONE, |colors, card| {
            colors | printed_card_colors(card)
        })
}

/// Reserve legal, non-mana Dwarf attackers that entered before this turn.
/// Their tap triggers resolve before the modeled post-combat main actions.
/// This phase-equivalent abstraction never lets the same creature also enter
/// the turn's mana-source pool.
fn reserve_dwarf_attack_taps(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    mana_sources: &[BattlefieldManaSource],
    turn: u8,
) -> (HashSet<usize>, u8) {
    let context = active_ability_context(deck, zones);
    if context.dwarf_treasure_per_tap == 0 {
        return (HashSet::new(), 0);
    }
    let mana_source_cards = mana_sources
        .iter()
        .filter_map(|source| source.card_index)
        .collect::<HashSet<_>>();
    let mut reserved = HashSet::new();
    let mut treasures = 0u8;
    for presence in &zones.battlefield {
        if presence.entered_turn >= turn || mana_source_cards.contains(&presence.card_index) {
            continue;
        }
        let Some(card) = deck.cards.get(presence.card_index) else {
            continue;
        };
        if !card.effects.card_types.is_creature
            || !card_has_subtype(card, "Dwarf")
            || card_has_keyword(card, "Defender")
        {
            continue;
        }
        if reserved.insert(presence.card_index) {
            treasures = treasures.saturating_add(context.dwarf_treasure_per_tap);
        }
    }
    (reserved, treasures)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModeledLineCardKind {
    Permanent,
    Spell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BasicLandSubtype {
    Plains,
    Island,
    Swamp,
    Mountain,
    Forest,
}

impl BasicLandSubtype {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "plains" => Some(Self::Plains),
            "island" => Some(Self::Island),
            "swamp" => Some(Self::Swamp),
            "mountain" => Some(Self::Mountain),
            "forest" => Some(Self::Forest),
            _ => None,
        }
    }

    fn type_name(self) -> &'static str {
        match self {
            Self::Plains => "Plains",
            Self::Island => "Island",
            Self::Swamp => "Swamp",
            Self::Mountain => "Mountain",
            Self::Forest => "Forest",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReviewedFetchland {
    first_subtype: BasicLandSubtype,
    second_subtype: BasicLandSubtype,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ReviewedFetchlandResolution {
    searched_target: Option<usize>,
    player_died: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BattlefieldLineCard {
    card_index: usize,
    entered_turn: u8,
    sequence: u16,
    age_counters: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BattlefieldCreatureToken {
    entered_turn: u8,
    sequence: u16,
    base_power: u16,
    base_toughness: u16,
    creature_types: BTreeSet<String>,
    printed_keywords: BTreeSet<CombatKeyword>,
    combat_power_counters: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct BattlefieldAttachment {
    target_sequence: u16,
    kind: CombatAttachmentKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CreatureDestructionResolution {
    removed_card_indices: Vec<usize>,
    removed_creature_tokens: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TurnLineSpell {
    card_index: usize,
    turn: u8,
    sequence: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingDelayedCardAccess {
    card_index: usize,
    due_turn: u8,
}

/// Minimal zone history used only to decide whether a documented compact line
/// is executable. This is intentionally stricter than the broader strategic
/// abstraction: ambiguous card types are not assigned a zone, tutors do not
/// stand in for cards that were never drawn/cast, and prior-turn spells cannot
/// be reused as though they were still on the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RuntimeCounterKind {
    Quest,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RuntimeTurnEvents {
    state: TurnEventState<u8, u16, RuntimeCounterKind>,
}

impl Default for RuntimeTurnEvents {
    fn default() -> Self {
        Self {
            state: TurnEventState::new([0, 1, 2, 3], [])
                .expect("the fixed Commander table has unique player identities"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct KnownLineZoneState {
    battlefield: Vec<BattlefieldLineCard>,
    creature_tokens: Vec<BattlefieldCreatureToken>,
    attachments: HashMap<u16, BattlefieldAttachment>,
    chosen_creature_types: HashMap<u16, String>,
    creature_power_counters: HashMap<u16, u16>,
    temporary_power_toughness_adjustments: HashMap<u16, (i16, i16)>,
    tapped_creatures_this_turn: BTreeSet<u16>,
    pending_card_draws: u32,
    turn_events: RuntimeTurnEvents,
    spells_cast_this_turn: Vec<TurnLineSpell>,
    typed_overrun_casts_this_turn: Vec<usize>,
    graveyard: Vec<usize>,
    exile: Vec<usize>,
    graveyard_cards_at_turn_start: u16,
    next_sequence: u16,
    /// Set only when the complete reviewed land-sacrifice-mana spell resolves.
    /// The grant is public current-turn state, never inferred from a cast that
    /// was countered, and is cleared before the next turn begins.
    land_sacrifice_mana_grant_turn: Option<u8>,
}

#[derive(Debug, Clone)]
enum CompiledLineActivationCost {
    None,
    Additional(ManaCostProfile),
    Unmodeled,
}

impl KnownLineZoneState {
    fn begin_turn(&mut self) {
        let _ = self.turn_events.state.start_next_turn();
        self.spells_cast_this_turn.clear();
        self.typed_overrun_casts_this_turn.clear();
        self.temporary_power_toughness_adjustments.clear();
        self.tapped_creatures_this_turn.clear();
        self.graveyard_cards_at_turn_start = self.graveyard.len().min(usize::from(u16::MAX)) as u16;
        self.land_sacrifice_mana_grant_turn = None;
    }

    fn record_land_play(&mut self, deck: &CompiledDeck, card_index: usize, turn: u8) {
        self.record_zone_change(deck, card_index, turn, true);
    }

    fn record_cast(&mut self, deck: &CompiledDeck, card_index: usize, turn: u8) {
        self.record_zone_change(deck, card_index, turn, false);
    }

    /// Record a real discard event, then immediately resolve the exact
    /// discarded-object trigger when the complete lifecycle is active. The
    /// object reaches the graveyard first; casts, sacrifices, mills, and
    /// London bottoms must not call this helper.
    fn record_discard(&mut self, deck: &CompiledDeck, card_index: usize) {
        let graveyard_position = self.graveyard.len();
        self.graveyard.push(card_index);
        self.advance_sequence();
        if !active_necropotence_lifecycle(deck, self) {
            return;
        }
        debug_assert_eq!(
            self.graveyard.get(graveyard_position),
            Some(&card_index),
            "the just-discarded object remains in its exact graveyard slot"
        );
        let discarded = self.graveyard.remove(graveyard_position);
        self.advance_sequence();
        self.exile.push(discarded);
    }

    fn record_discards(
        &mut self,
        deck: &CompiledDeck,
        discarded_cards: impl IntoIterator<Item = usize>,
    ) {
        for card_index in discarded_cards {
            self.record_discard(deck, card_index);
        }
    }

    fn record_typed_overrun_cast(&mut self, deck: &CompiledDeck, card_index: usize, turn: u8) {
        self.record_cast(deck, card_index, turn);
        self.typed_overrun_casts_this_turn.push(card_index);
    }

    fn record_put_onto_battlefield(&mut self, deck: &CompiledDeck, card_index: usize, turn: u8) {
        let sequence = self.advance_sequence();
        if deck.cards.get(card_index).is_some_and(|card| {
            matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Permanent)
            )
        }) {
            self.battlefield.push(BattlefieldLineCard {
                card_index,
                entered_turn: turn,
                sequence,
                age_counters: 0,
            });
            self.turn_events.state.register_object(sequence);
            self.resolve_self_entry_turn_state_effects(deck, sequence);
            self.choose_entering_creature_type(deck, sequence);
            self.attach_entering_positive_aura(deck, sequence);
            self.resolve_creature_entry_counters(deck, sequence);
            self.resolve_other_flying_creature_entry_triggers(deck, sequence);
            self.resolve_chosen_type_creature_entry_draw_triggers(deck, sequence);
            self.resolve_permanent_entry_creature_triggers(deck, card_index, sequence, turn);
        }
    }

    fn record_zone_change(
        &mut self,
        deck: &CompiledDeck,
        card_index: usize,
        turn: u8,
        is_land_play: bool,
    ) {
        let sequence = self.advance_sequence();
        let Some(card) = deck.cards.get(card_index) else {
            return;
        };
        match modeled_line_card_kind(card) {
            Some(ModeledLineCardKind::Permanent) if !is_land_play || card.has(role::LAND) => {
                self.battlefield.push(BattlefieldLineCard {
                    card_index,
                    entered_turn: turn,
                    sequence,
                    age_counters: 0,
                });
                self.turn_events.state.register_object(sequence);
                self.resolve_self_entry_turn_state_effects(deck, sequence);
                self.choose_entering_creature_type(deck, sequence);
                self.attach_entering_positive_aura(deck, sequence);
                self.resolve_creature_entry_counters(deck, sequence);
                self.resolve_other_flying_creature_entry_triggers(deck, sequence);
                self.resolve_chosen_type_creature_entry_draw_triggers(deck, sequence);
                self.resolve_permanent_entry_creature_triggers(deck, card_index, sequence, turn);
            }
            Some(ModeledLineCardKind::Spell) if !is_land_play => {
                self.spells_cast_this_turn.push(TurnLineSpell {
                    card_index,
                    turn,
                    sequence,
                });
                // The bounded simulator resolves spells immediately. Keeping a
                // separate current-turn cast event lets a line consume that
                // execution without pretending the card remains usable later.
                self.graveyard.push(card_index);
            }
            _ => {}
        }
    }

    fn remove_named_permanent(
        &mut self,
        deck: &CompiledDeck,
        normalized_name: &str,
        move_to_graveyard: bool,
    ) -> bool {
        let Some(sequence) = self.battlefield.iter().find_map(|presence| {
            deck.cards
                .get(presence.card_index)
                .is_some_and(|card| card.normalized_name == normalized_name)
                .then_some(presence.sequence)
        }) else {
            return false;
        };
        self.remove_permanent_sequence_with_attached_auras(deck, sequence, move_to_graveyard)
            .is_some()
    }

    fn remove_permanent_sequence(
        &mut self,
        deck: &CompiledDeck,
        sequence: u16,
        move_to_graveyard: bool,
    ) -> Option<usize> {
        self.remove_permanent_sequence_with_attached_auras(deck, sequence, move_to_graveyard)
            .and_then(|removed| removed.first().copied())
    }

    fn remove_permanent_sequence_with_attached_auras(
        &mut self,
        deck: &CompiledDeck,
        sequence: u16,
        move_to_graveyard: bool,
    ) -> Option<Vec<usize>> {
        self.battlefield
            .iter()
            .any(|presence| presence.sequence == sequence)
            .then_some(())?;

        let mut pending = VecDeque::from([(sequence, move_to_graveyard)]);
        let mut queued = BTreeSet::from([sequence]);
        let mut removed_card_indices = Vec::new();
        while let Some((current_sequence, current_to_graveyard)) = pending.pop_front() {
            let mut attached_auras = self
                .attachments
                .iter()
                .filter_map(|(source_sequence, attachment)| {
                    (attachment.target_sequence == current_sequence
                        && attachment.kind == CombatAttachmentKind::Aura)
                        .then_some(*source_sequence)
                })
                .collect::<Vec<_>>();
            attached_auras.sort_unstable();

            let Some(position) = self
                .battlefield
                .iter()
                .position(|presence| presence.sequence == current_sequence)
            else {
                continue;
            };
            let removed = self.battlefield.remove(position);
            self.remove_object_state(removed.sequence);
            self.advance_sequence();
            if current_to_graveyard
                && deck
                    .cards
                    .get(removed.card_index)
                    .is_some_and(|card| !card.is_commander)
            {
                self.graveyard.push(removed.card_index);
            }
            removed_card_indices.push(removed.card_index);

            for aura_sequence in attached_auras {
                if queued.insert(aura_sequence) {
                    // An Aura whose enchanted object left is put into its
                    // owner's graveyard as a state-based action regardless of
                    // the destination chosen for that object.
                    pending.push_back((aura_sequence, true));
                }
            }
        }
        Some(removed_card_indices)
    }

    fn remove_attached_aura_permanents(
        &mut self,
        deck: &CompiledDeck,
        target_sequence: u16,
    ) -> Vec<usize> {
        let mut aura_sequences = self
            .attachments
            .iter()
            .filter_map(|(source_sequence, attachment)| {
                (attachment.target_sequence == target_sequence
                    && attachment.kind == CombatAttachmentKind::Aura)
                    .then_some(*source_sequence)
            })
            .collect::<Vec<_>>();
        aura_sequences.sort_unstable();
        aura_sequences
            .into_iter()
            .filter_map(|sequence| {
                self.remove_permanent_sequence_with_attached_auras(deck, sequence, true)
            })
            .flatten()
            .collect()
    }

    fn remove_one_permanent_with_role(&mut self, deck: &CompiledDeck, roles: u32) -> bool {
        let Some(sequence) = self.battlefield.iter().find_map(|presence| {
            deck.cards
                .get(presence.card_index)
                .is_some_and(|card| !card.is_commander && card.has(roles))
                .then_some(presence.sequence)
        }) else {
            return false;
        };
        self.remove_permanent_sequence_with_attached_auras(deck, sequence, true)
            .is_some()
    }

    fn destroy_all_creatures(&mut self, deck: &CompiledDeck) -> CreatureDestructionResolution {
        let indestructible_sequences = combat_effect_runtime(deck, self)
            .map(|(state, effects)| {
                self.battlefield
                    .iter()
                    .map(|presence| presence.sequence)
                    .chain(self.creature_tokens.iter().map(|token| token.sequence))
                    .filter(|sequence| {
                        state
                            .evaluate_creature(combat_object_id(*sequence), &effects)
                            .ok()
                            .is_some_and(|profile| {
                                profile.keywords.contains(&CombatKeyword::Indestructible)
                            })
                    })
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let destroyed_card_sequences = self
            .battlefield
            .iter()
            .filter(|presence| {
                deck.cards.get(presence.card_index).is_some_and(|card| {
                    card.effects.card_types.is_creature || card.has(role::CREATURE)
                }) && !indestructible_sequences.contains(&presence.sequence)
            })
            .map(|presence| presence.sequence)
            .collect::<Vec<_>>();
        let destroyed_token_sequences = self
            .creature_tokens
            .iter()
            .filter(|token| !indestructible_sequences.contains(&token.sequence))
            .map(|token| token.sequence)
            .collect::<Vec<_>>();

        // Destroyed creatures die simultaneously. Capture all equipped-death
        // triggers against the pre-destruction battlefield before removing an
        // Equipment source, its target, or a trigger multiplier.
        let pending_draws = destroyed_card_sequences
            .iter()
            .chain(&destroyed_token_sequences)
            .copied()
            .map(|sequence| equipped_creature_death_trigger_draws(deck, self, sequence))
            .fold(0u32, u32::saturating_add);

        let mut resolution = CreatureDestructionResolution::default();
        for sequence in destroyed_card_sequences {
            if let Some(removed) =
                self.remove_permanent_sequence_with_attached_auras(deck, sequence, true)
            {
                resolution.removed_card_indices.extend(removed);
            }
        }
        for sequence in destroyed_token_sequences {
            let Some(position) = self
                .creature_tokens
                .iter()
                .position(|token| token.sequence == sequence)
            else {
                continue;
            };
            resolution
                .removed_card_indices
                .extend(self.remove_attached_aura_permanents(deck, sequence));
            self.creature_tokens.remove(position);
            self.remove_object_state(sequence);
            self.advance_sequence();
            resolution.removed_creature_tokens =
                resolution.removed_creature_tokens.saturating_add(1);
        }
        self.pending_card_draws = self.pending_card_draws.saturating_add(pending_draws);
        resolution
    }

    fn attach_entering_positive_aura(&mut self, deck: &CompiledDeck, source_sequence: u16) {
        let Some(source_presence) = self
            .battlefield
            .iter()
            .find(|presence| presence.sequence == source_sequence)
        else {
            return;
        };
        let Some(source) = deck.cards.get(source_presence.card_index) else {
            return;
        };
        if !type_line_has_subtype(&source.type_line, "Aura")
            || !(card_has_beneficial_attached_static_effect(source)
                || card_has_beneficial_attached_trigger(source))
        {
            return;
        }
        let Some((state, effects)) = combat_effect_runtime(deck, self) else {
            return;
        };
        let legal = PermanentFilter {
            controller: ControllerConstraint::SameAsSource,
            all_card_types: BTreeSet::from([PermanentType::Creature]),
            ..PermanentFilter::default()
        };
        let Ok(Some(choice)) = state.choose_attachment_target(
            combat_object_id(source_sequence),
            CombatAttachmentKind::Aura,
            &legal,
            &effects,
        ) else {
            return;
        };
        let Ok(target_sequence) = u16::try_from(choice.target.0.saturating_sub(1)) else {
            return;
        };
        self.attachments.insert(
            source_sequence,
            BattlefieldAttachment {
                target_sequence,
                kind: CombatAttachmentKind::Aura,
            },
        );
    }

    fn choose_entering_creature_type(&mut self, deck: &CompiledDeck, source_sequence: u16) {
        let Some(source) = self
            .battlefield
            .iter()
            .find(|presence| presence.sequence == source_sequence)
            .and_then(|presence| deck.cards.get(presence.card_index))
        else {
            return;
        };
        let chooses_type = source
            .ability_program
            .executable_abilities()
            .any(|ability| {
                ability
                    .effects
                    .iter()
                    .any(|effect| matches!(effect, AbilityEffect::ChooseCreatureType(_)))
            });
        if !chooses_type {
            return;
        }
        if let Some(creature_type) = preferred_creature_type(deck, self) {
            self.chosen_creature_types
                .insert(source_sequence, creature_type);
        }
    }

    fn resolve_self_entry_turn_state_effects(&mut self, deck: &CompiledDeck, source_sequence: u16) {
        let Some(source) = self
            .battlefield
            .iter()
            .find(|presence| presence.sequence == source_sequence)
            .and_then(|presence| deck.cards.get(presence.card_index))
        else {
            return;
        };
        let becomes_monarch = source
            .ability_program
            .executable_abilities()
            .any(|ability| {
                matches!(
                    &ability.timing,
                    AbilityTiming::Triggered { event }
                        if event.kind == TriggerEventKind::PermanentEntersBattlefield
                            && event.actor == ControllerRelation::You
                ) && ability
                    .preconditions
                    .contains(&AbilityPrecondition::EventObjectIsSource)
                    && ability.effects.iter().any(|effect| {
                        matches!(
                            effect,
                            AbilityEffect::BecomeMonarch(monarch)
                                if monarch.player == ControllerRelation::You
                        )
                    })
            });
        if becomes_monarch {
            let _ = self.turn_events.state.set_monarch(0);
        }
    }

    fn controller_is_monarch(&self) -> bool {
        self.turn_events
            .state
            .controller_condition(0, RuntimeControllerCondition::IsMonarch)
            .unwrap_or(false)
    }

    fn controller_lost_no_life_this_turn(&self) -> bool {
        self.turn_events
            .state
            .controller_condition(0, RuntimeControllerCondition::LostNoLifeThisTurn)
            .unwrap_or(false)
    }

    fn record_controller_life_loss(&mut self, amount: u32) {
        if amount > 0 {
            let _ = self.turn_events.state.record_life_loss(0, amount);
        }
    }

    fn record_controller_life_delta(&mut self, before: f32, after: f32) {
        let lost = (before - after).max(0.0).floor().min(u32::MAX as f32) as u32;
        self.record_controller_life_loss(lost);
    }

    fn remove_object_state(&mut self, sequence: u16) {
        self.turn_events.state.unregister_object(sequence);
        self.attachments.remove(&sequence);
        self.attachments
            .retain(|_, attachment| attachment.target_sequence != sequence);
        self.chosen_creature_types.remove(&sequence);
        self.creature_power_counters.remove(&sequence);
        self.temporary_power_toughness_adjustments.remove(&sequence);
        self.tapped_creatures_this_turn.remove(&sequence);
    }

    #[allow(clippy::too_many_arguments)]
    fn record_creature_tokens(
        &mut self,
        deck: &CompiledDeck,
        turn: u8,
        count: u16,
        power: i16,
        toughness: i16,
        description: &str,
        keywords: &[CreatureTokenKeyword],
    ) {
        let replaced = four_four_flying_creature_token_replacement_is_active(deck, self);
        let base_power = if replaced {
            4
        } else {
            u16::try_from(power.max(0)).unwrap_or_default()
        };
        if base_power == 0 {
            return;
        }
        let base_toughness = if replaced {
            4
        } else {
            u16::try_from(toughness.max(0)).unwrap_or_default()
        };
        let mut printed_keywords = BTreeSet::new();
        if replaced || keywords.contains(&CreatureTokenKeyword::Flying) {
            printed_keywords.insert(CombatKeyword::Flying);
        }
        if replaced {
            printed_keywords.insert(CombatKeyword::Vigilance);
        }
        let creature_types =
            token_creature_types(if replaced { "white angel" } else { description });
        for _ in 0..count {
            let sequence = self.advance_sequence();
            self.creature_tokens.push(BattlefieldCreatureToken {
                entered_turn: turn,
                sequence,
                base_power,
                base_toughness,
                creature_types: creature_types.clone(),
                printed_keywords: printed_keywords.clone(),
                combat_power_counters: 0,
            });
            self.turn_events.state.register_object(sequence);
            self.resolve_creature_entry_counters(deck, sequence);
            self.resolve_other_flying_creature_entry_triggers(deck, sequence);
            self.resolve_chosen_type_creature_entry_draw_triggers(deck, sequence);
        }
    }

    fn resolve_other_flying_creature_entry_triggers(
        &mut self,
        deck: &CompiledDeck,
        entered_sequence: u16,
    ) {
        let entered_has_flying = self
            .battlefield
            .iter()
            .find(|presence| presence.sequence == entered_sequence)
            .and_then(|presence| deck.cards.get(presence.card_index))
            .is_some_and(|card| {
                card.effects.card_types.is_creature && card_has_keyword(card, "Flying")
            })
            || self
                .creature_tokens
                .iter()
                .find(|token| token.sequence == entered_sequence)
                .is_some_and(|token| token.printed_keywords.contains(&CombatKeyword::Flying));
        if !entered_has_flying {
            return;
        }

        let sources = self.battlefield.clone();
        for presence in sources {
            if presence.sequence == entered_sequence {
                continue;
            }
            let Some(source) = deck.cards.get(presence.card_index) else {
                continue;
            };
            for ability in source.ability_program.executable_abilities() {
                let AbilityTiming::Triggered { event } = &ability.timing else {
                    continue;
                };
                if event.kind != TriggerEventKind::OtherFlyingCreatureEntersBattlefield
                    || !ability.costs.is_empty()
                    || !ability
                        .preconditions
                        .contains(&AbilityPrecondition::SourceZone(ProgramZone::Battlefield))
                {
                    continue;
                }
                for effect in &ability.effects {
                    let AbilityEffect::ModifyPowerToughnessUntilEndOfTurn(modifier) = effect else {
                        continue;
                    };
                    if modifier.target != TargetSelector::SelfPermanent {
                        continue;
                    }
                    let trigger_count = trigger_multiplier_count(
                        deck,
                        self,
                        CombatTriggerEventKind::PermanentEnteredBattlefield,
                        Some(entered_sequence),
                        presence.sequence,
                    )
                    .min(i16::MAX as u32) as i16;
                    let adjustment = self
                        .temporary_power_toughness_adjustments
                        .entry(presence.sequence)
                        .or_default();
                    adjustment.0 = adjustment
                        .0
                        .saturating_add(modifier.power_delta.saturating_mul(trigger_count));
                    adjustment.1 = adjustment
                        .1
                        .saturating_add(modifier.toughness_delta.saturating_mul(trigger_count));
                }
            }
        }
    }

    fn resolve_chosen_type_creature_entry_draw_triggers(
        &mut self,
        deck: &CompiledDeck,
        entered_sequence: u16,
    ) {
        let sources = self.battlefield.clone();
        for presence in sources {
            let Some(source) = deck.cards.get(presence.card_index) else {
                continue;
            };
            for ability in source.ability_program.executable_abilities() {
                let AbilityTiming::Triggered { event } = &ability.timing else {
                    continue;
                };
                if event.kind != TriggerEventKind::ChosenTypeCreatureEntersOrAttacks
                    || !ability.costs.is_empty()
                    || !attacker_matches_source_chosen_type(
                        deck,
                        self,
                        u32::from(entered_sequence).saturating_add(1),
                        presence.sequence,
                    )
                {
                    continue;
                }
                let trigger_count = trigger_multiplier_count(
                    deck,
                    self,
                    CombatTriggerEventKind::PermanentEnteredBattlefield,
                    Some(entered_sequence),
                    presence.sequence,
                );
                self.pending_card_draws = self.pending_card_draws.saturating_add(
                    draw_count_from_effects(&ability.effects).saturating_mul(trigger_count),
                );
            }
        }
    }

    fn take_pending_card_draws(&mut self) -> u32 {
        std::mem::take(&mut self.pending_card_draws)
    }

    fn resolve_creature_entry_counters(&mut self, deck: &CompiledDeck, entered_sequence: u16) {
        let entered_is_creature = self
            .battlefield
            .iter()
            .find(|presence| presence.sequence == entered_sequence)
            .and_then(|presence| deck.cards.get(presence.card_index))
            .is_some_and(|card| card.effects.card_types.is_creature)
            || self
                .creature_tokens
                .iter()
                .any(|token| token.sequence == entered_sequence);
        if !entered_is_creature {
            return;
        }
        let team_counter_sources = self
            .battlefield
            .iter()
            .filter_map(|presence| {
                deck.cards
                    .get(presence.card_index)
                    .is_some_and(|card| {
                        card_has_oracle_paragraph(
                            card,
                            "whenever a creature you control enters, put a +1/+1 counter on each creature you control",
                        )
                    })
                    .then_some(presence.sequence)
            })
            .collect::<Vec<_>>();
        if team_counter_sources.is_empty() {
            return;
        }
        let counters = team_counter_sources
            .into_iter()
            .map(|source_sequence| {
                trigger_multiplier_count(
                    deck,
                    self,
                    CombatTriggerEventKind::PermanentEnteredBattlefield,
                    Some(entered_sequence),
                    source_sequence,
                )
            })
            .sum::<u32>()
            .min(u32::from(u16::MAX)) as u16;
        for presence in &self.battlefield {
            if deck
                .cards
                .get(presence.card_index)
                .is_some_and(|card| card.effects.card_types.is_creature)
            {
                let current = self
                    .creature_power_counters
                    .entry(presence.sequence)
                    .or_default();
                *current = current.saturating_add(counters);
            }
        }
        for token in &mut self.creature_tokens {
            token.combat_power_counters = token.combat_power_counters.saturating_add(counters);
        }
    }

    fn resolve_permanent_entry_creature_triggers(
        &mut self,
        deck: &CompiledDeck,
        entered_card_index: usize,
        entered_sequence: u16,
        turn: u8,
    ) {
        let Some(entered_card) = deck.cards.get(entered_card_index) else {
            return;
        };
        let sources = self.battlefield.clone();
        let mut tokens = Vec::new();
        for presence in sources {
            let Some(source) = deck.cards.get(presence.card_index) else {
                continue;
            };
            for ability in source.ability_program.executable_abilities() {
                let AbilityTiming::Triggered { event } = &ability.timing else {
                    continue;
                };
                if event.kind != TriggerEventKind::PermanentEntersBattlefield
                    || !matches!(
                        event.actor,
                        ControllerRelation::You | ControllerRelation::Any
                    )
                    || !ability.costs.is_empty()
                    || !ability
                        .preconditions
                        .contains(&AbilityPrecondition::SourceZone(ProgramZone::Battlefield))
                    || !program_object_filter_matches(&event.object_filter, entered_card)
                {
                    continue;
                }
                for effect in &ability.effects {
                    if let AbilityEffect::CreateToken(token) = effect
                        && let TokenKind::Creature {
                            power,
                            toughness,
                            description,
                            keywords,
                        } = &token.kind
                    {
                        let trigger_count = trigger_multiplier_count(
                            deck,
                            self,
                            CombatTriggerEventKind::PermanentEnteredBattlefield,
                            Some(entered_sequence),
                            presence.sequence,
                        )
                        .min(u32::from(u16::MAX));
                        tokens.push((
                            u32::from(token.count)
                                .saturating_mul(trigger_count)
                                .min(u32::from(u16::MAX)) as u16,
                            *power,
                            *toughness,
                            description.clone(),
                            keywords.clone(),
                        ));
                    }
                }
            }
        }
        for (count, power, toughness, description, keywords) in tokens {
            self.record_creature_tokens(
                deck,
                turn,
                count,
                power,
                toughness,
                &description,
                &keywords,
            );
        }
    }

    fn usable_count(&self, deck: &CompiledDeck, normalized_name: &str, turn: u8) -> usize {
        let Some(card) = unique_card_by_normalized_name(deck, normalized_name) else {
            return 0;
        };
        match modeled_line_card_kind(card) {
            Some(ModeledLineCardKind::Permanent) => self
                .battlefield
                .iter()
                .filter(|presence| {
                    deck.cards
                        .get(presence.card_index)
                        .is_some_and(|candidate| {
                            candidate.normalized_name == normalized_name
                                && presence.entered_turn <= turn
                        })
                })
                .count(),
            Some(ModeledLineCardKind::Spell) => {
                self.spells_cast_this_turn
                    .iter()
                    .filter(|cast| {
                        cast.turn == turn
                            && deck.cards.get(cast.card_index).is_some_and(|candidate| {
                                candidate.normalized_name == normalized_name
                            })
                    })
                    .count()
            }
            None => 0,
        }
    }

    fn advance_sequence(&mut self) -> u16 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        sequence
    }
}

fn fixed_generic_program_payment(cost: &ProgramManaCost) -> Option<u16> {
    let ProgramManaCost::PrintedSymbols { profile, .. } = cost else {
        return None;
    };
    (profile.white == 0
        && profile.blue == 0
        && profile.black == 0
        && profile.red == 0
        && profile.green == 0
        && profile.colorless == 0
        && profile.variable_x == 0)
        .then_some(profile.generic)
}

fn optional_payment_amount(payment: &OptionalManaPayment, source: &CompiledCard) -> Option<u16> {
    match &payment.amount {
        ManaPaymentAmount::Fixed(cost) => fixed_generic_program_payment(cost),
        ManaPaymentAmount::SourcePower => source
            .printed_power
            .filter(|power| *power >= 0)
            .map(|power| power as u16),
    }
}

fn cumulative_upkeep_program(card: &CompiledCard) -> Option<(u16, u16)> {
    let mut program = None;
    for ability in card.ability_program.executable_abilities() {
        let AbilityTiming::Triggered { event } = &ability.timing else {
            continue;
        };
        if event.kind != TriggerEventKind::BeginningOfUpkeep
            || event.actor != ControllerRelation::You
            || !ability.costs.is_empty()
            || !ability
                .preconditions
                .contains(&AbilityPrecondition::SourceZone(ProgramZone::Battlefield))
        {
            continue;
        }
        let [AbilityEffect::CumulativeUpkeep(upkeep)] = ability.effects.as_slice() else {
            continue;
        };
        if upkeep.counter != CounterKind::Age
            || upkeep.counters_added == 0
            || upkeep.if_not_paid != [AbilityEffect::SacrificeSelf]
        {
            return None;
        }
        let payment = fixed_generic_program_payment(&upkeep.payment_per_counter)?;
        if program.replace((upkeep.counters_added, payment)).is_some() {
            return None;
        }
    }
    program
}

fn upkeep_creature_effects_supported(effects: &[AbilityEffect]) -> bool {
    effects.iter().all(|effect| match effect {
        AbilityEffect::LoseLife(loss) => loss.player == ControllerRelation::You,
        AbilityEffect::CreateToken(token) => {
            matches!(token.kind, TokenKind::Creature { power, .. } if power > 0)
        }
        AbilityEffect::Conditional(conditional) => {
            conditional.controller == ControllerRelation::You
                && conditional.condition == ControllerStateCondition::IsMonarch
                && !conditional.if_true.is_empty()
                && !conditional.if_false.is_empty()
                && upkeep_creature_effects_supported(&conditional.if_true)
                && upkeep_creature_effects_supported(&conditional.if_false)
        }
        _ => false,
    })
}

fn resolve_upkeep_creature_effects(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    effects: &[AbilityEffect],
    turn: u8,
    player_life: &mut f32,
) {
    for effect in effects {
        match effect {
            AbilityEffect::LoseLife(loss) => {
                *player_life -= f32::from(loss.amount);
                zones.record_controller_life_loss(u32::from(loss.amount));
            }
            AbilityEffect::CreateToken(token) => {
                let TokenKind::Creature {
                    power,
                    toughness,
                    description,
                    keywords,
                } = &token.kind
                else {
                    unreachable!("upkeep creature trigger was validated")
                };
                zones.record_creature_tokens(
                    deck,
                    turn,
                    token.count,
                    *power,
                    *toughness,
                    description,
                    keywords,
                );
            }
            AbilityEffect::Conditional(conditional) => {
                let branch = if zones.controller_is_monarch() {
                    &conditional.if_true
                } else {
                    &conditional.if_false
                };
                resolve_upkeep_creature_effects(deck, zones, branch, turn, player_life);
            }
            _ => unreachable!("upkeep creature trigger was validated"),
        }
    }
}

fn resolve_controller_upkeep_creature_triggers(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    turn: u8,
    player_life: &mut f32,
) -> bool {
    let sources = zones.battlefield.clone();
    for presence in sources {
        let Some(source) = deck.cards.get(presence.card_index) else {
            continue;
        };
        for ability in source.ability_program.executable_abilities() {
            let AbilityTiming::Triggered { event } = &ability.timing else {
                continue;
            };
            if event.kind != TriggerEventKind::BeginningOfUpkeep
                || event.actor != ControllerRelation::You
                || !ability.costs.is_empty()
                || !ability
                    .preconditions
                    .contains(&AbilityPrecondition::SourceZone(ProgramZone::Battlefield))
                || !upkeep_creature_effects_supported(&ability.effects)
            {
                continue;
            }
            resolve_upkeep_creature_effects(deck, zones, &ability.effects, turn, player_life);
        }
    }
    *player_life > 0.0
}

fn active_runtime_players(combat_state: &CommanderCombatState) -> Vec<u8> {
    let mut players = Vec::with_capacity(1 + combat_state.active_opponent_count());
    players.push(0);
    players.extend(
        OpponentId::ALL
            .into_iter()
            .filter(|opponent| combat_state.is_opponent_active(*opponent))
            .filter_map(|opponent| u8::try_from(opponent.index() + 1).ok()),
    );
    players
}

fn resolve_opponent_end_step_counter_triggers(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    combat_state: &CommanderCombatState,
    opponent: OpponentId,
) {
    if !combat_state.is_opponent_active(opponent) {
        return;
    }
    let active_players = active_runtime_players(combat_state);
    let Some(turn_player) = u8::try_from(opponent.index() + 1).ok() else {
        return;
    };
    let Ok(_) = zones
        .turn_events
        .state
        .record_active_end_step(turn_player, active_players)
    else {
        return;
    };
    if !zones.controller_lost_no_life_this_turn() {
        return;
    }

    let sources = zones.battlefield.clone();
    for presence in sources {
        let Some(source) = deck.cards.get(presence.card_index) else {
            continue;
        };
        for ability in source.ability_program.executable_abilities() {
            let AbilityTiming::Triggered { event } = &ability.timing else {
                continue;
            };
            let has_exact_condition = ability.preconditions.iter().any(|precondition| {
                matches!(
                    precondition,
                    AbilityPrecondition::ControllerCondition(condition)
                        if condition.controller == ControllerRelation::You
                            && condition.condition
                                == ControllerStateCondition::LostNoLifeThisTurn
                            && condition.check_when_triggering_and_resolving
                )
            });
            if event.kind != TriggerEventKind::BeginningOfEndStep
                || event.actor != ControllerRelation::Opponent
                || !ability.costs.is_empty()
                || !has_exact_condition
            {
                continue;
            }
            let trigger_count = trigger_multiplier_count(
                deck,
                zones,
                CombatTriggerEventKind::BeginningOfEndStep,
                None,
                presence.sequence,
            );
            for effect in &ability.effects {
                let AbilityEffect::AddCounters(counters) = effect else {
                    continue;
                };
                if counters.target != CounterTarget::SourcePermanent
                    || counters.counter != CounterKind::Quest
                    || counters.count == 0
                {
                    continue;
                }
                let amount = u32::from(counters.count).saturating_mul(trigger_count);
                let _ = zones.turn_events.state.add_counters(
                    presence.sequence,
                    RuntimeCounterKind::Quest,
                    amount,
                );
            }
        }
    }
}

fn activate_counter_threshold_token_abilities(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    mana_pool: &mut TurnManaPool,
    turn: u8,
) -> u32 {
    const MAX_ACTIVATIONS_PER_MAIN_PHASE: u32 = 64;
    let sources = zones.battlefield.clone();
    let mut activations = 0u32;
    let mut created_tokens = 0u32;
    for presence in sources {
        let Some(source) = deck.cards.get(presence.card_index) else {
            continue;
        };
        for ability in source.ability_program.executable_abilities() {
            if !matches!(
                ability.timing,
                AbilityTiming::Activated {
                    window: ActivationWindow::NormalPriority
                }
            ) || ability.costs.len() != 1
                || ability.effects.len() != 1
            {
                continue;
            }
            let Some((counter, threshold)) =
                ability
                    .preconditions
                    .iter()
                    .find_map(|precondition| match precondition {
                        AbilityPrecondition::SourceCounterAtLeast { counter, count } => {
                            Some((*counter, *count))
                        }
                        _ => None,
                    })
            else {
                continue;
            };
            if counter != CounterKind::Quest
                || !zones
                    .turn_events
                    .state
                    .counter_threshold_activation_eligible(
                        presence.sequence,
                        &RuntimeCounterKind::Quest,
                        u32::from(threshold),
                    )
                    .unwrap_or(false)
            {
                continue;
            }
            let AbilityCost::Mana(cost) = &ability.costs[0] else {
                continue;
            };
            let AbilityEffect::CreateToken(token) = &ability.effects[0] else {
                continue;
            };
            let TokenKind::Creature {
                power,
                toughness,
                description,
                keywords,
            } = &token.kind
            else {
                continue;
            };
            while activations < MAX_ACTIVATIONS_PER_MAIN_PHASE {
                let mut candidate = mana_pool.clone();
                if !pay_program_mana_cost(&mut candidate, cost) {
                    break;
                }
                *mana_pool = candidate;
                zones.record_creature_tokens(
                    deck,
                    turn,
                    token.count,
                    *power,
                    *toughness,
                    description,
                    keywords,
                );
                activations = activations.saturating_add(1);
                created_tokens = created_tokens.saturating_add(u32::from(token.count));
            }
        }
    }
    created_tokens
}

fn resolve_cumulative_upkeep_triggers(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    mana_pool: &mut TurnManaPool,
) -> Vec<usize> {
    let sources = zones
        .battlefield
        .iter()
        .filter_map(|presence| {
            cumulative_upkeep_program(deck.cards.get(presence.card_index)?)
                .map(|program| (presence.sequence, program))
        })
        .collect::<Vec<_>>();
    let mut removed = Vec::new();

    for (sequence, (counters_added, payment_per_counter)) in sources {
        let Some(position) = zones
            .battlefield
            .iter()
            .position(|presence| presence.sequence == sequence)
        else {
            continue;
        };
        let age_counters = zones.battlefield[position]
            .age_counters
            .saturating_add(counters_added);
        let payment = payment_per_counter.saturating_mul(age_counters);
        let paid = u8::try_from(payment).ok().is_some_and(|payment| {
            let mut candidate = mana_pool.clone();
            if !candidate.pay_generic(payment) {
                return false;
            }
            candidate.resolve_pending_tap_triggers();
            *mana_pool = candidate;
            true
        });
        if paid {
            zones.battlefield[position].age_counters = age_counters;
            continue;
        }
        if let Some(card_index) = zones.remove_permanent_sequence(deck, sequence, true) {
            removed.push(card_index);
        }
    }

    removed
}

fn program_filter_matches_opponent_spell(
    filter: &ProgramObjectFilter,
    spell: &OpponentSpellActivity,
) -> bool {
    if !matches!(
        filter.controller,
        None | Some(ControllerRelation::Opponent | ControllerRelation::Any)
    ) || filter.subtype.is_some()
        || filter.excluded_subtype.is_some()
        || !filter.any_of_card_types.is_empty()
    {
        return false;
    }
    if filter.nonland {
        // The activity feed represents cast spells; lands are never events in
        // this stream.
    }
    let included = match filter.card_type {
        None | Some(ProgramCardType::Spell | ProgramCardType::Card) => true,
        Some(ProgramCardType::Creature) => !spell.is_noncreature(),
        _ => false,
    };
    let excluded = match filter.excluded_card_type {
        None => false,
        Some(ProgramCardType::Creature) => !spell.is_noncreature(),
        _ => return false,
    };
    included && !excluded
}

fn program_filter_matches_opponent_draw(filter: &ProgramObjectFilter) -> bool {
    matches!(filter.card_type, None | Some(ProgramCardType::Card))
        && filter.any_of_card_types.is_empty()
        && filter.excluded_card_type.is_none()
        && filter.subtype.is_none()
        && filter.excluded_subtype.is_none()
        && !filter.nonland
        && matches!(
            filter.controller,
            None | Some(ControllerRelation::Opponent | ControllerRelation::Any)
        )
}

fn program_filter_matches_controller_spell(
    filter: &ProgramObjectFilter,
    card: &CompiledCard,
) -> bool {
    if !matches!(
        filter.controller,
        None | Some(ControllerRelation::You | ControllerRelation::Any)
    ) || filter.subtype.is_some()
        || filter.excluded_subtype.is_some()
    {
        return false;
    }
    let types = card.effects.card_types;
    let included = match filter.card_type {
        None | Some(ProgramCardType::Spell | ProgramCardType::Card) => true,
        Some(ProgramCardType::Creature) => types.is_creature,
        Some(ProgramCardType::Artifact) => types.is_artifact,
        Some(ProgramCardType::Land) => types.is_land,
        Some(ProgramCardType::Permanent) => {
            types.is_land || types.is_creature || types.is_artifact || types.is_enchantment
        }
        Some(ProgramCardType::Dragon) => card_has_subtype(card, "Dragon"),
    };
    let excluded = match filter.excluded_card_type {
        None => false,
        Some(ProgramCardType::Creature) => types.is_creature,
        Some(ProgramCardType::Artifact) => types.is_artifact,
        Some(ProgramCardType::Land) => types.is_land,
        Some(ProgramCardType::Permanent) => {
            types.is_land || types.is_creature || types.is_artifact || types.is_enchantment
        }
        Some(ProgramCardType::Spell | ProgramCardType::Card) => true,
        Some(ProgramCardType::Dragon) => card_has_subtype(card, "Dragon"),
    };
    let matches_specific_type = filter.any_of_card_types.is_empty()
        || filter
            .any_of_card_types
            .iter()
            .any(|card_type| match card_type {
                SpecificCardType::Artifact => types.is_artifact,
                SpecificCardType::Enchantment => types.is_enchantment,
            });
    included && matches_specific_type && !excluded && (!filter.nonland || !types.is_land)
}

fn table_trigger_effects_supported(effects: &[AbilityEffect], source: &CompiledCard) -> bool {
    effects.iter().all(|effect| match effect {
        AbilityEffect::Draw(draw) => draw
            .unless_event_player_pays
            .as_ref()
            .is_none_or(|payment| optional_payment_amount(payment, source).is_some()),
        AbilityEffect::UnlessEventPlayerPays(unless) => {
            optional_payment_amount(&unless.payment, source).is_some()
                && table_trigger_effects_supported(&unless.if_not_paid, source)
        }
        AbilityEffect::LoseLife(loss) => loss.player == ControllerRelation::You,
        AbilityEffect::CreateToken(token) => token.kind == TokenKind::Treasure,
        _ => false,
    })
}

#[allow(clippy::too_many_arguments)]
fn execute_table_trigger_effects(
    effects: &[AbilityEffect],
    source: &CompiledCard,
    payment: TablePaymentDecision,
    hand: &mut Vec<usize>,
    library_order: &[usize],
    next_draw_position: &mut usize,
    treasure_reserve: &mut u8,
    player_life: &mut f32,
) {
    for effect in effects {
        match effect {
            AbilityEffect::Draw(draw) => {
                let paid = draw
                    .unless_event_player_pays
                    .as_ref()
                    .and_then(|payment_rule| optional_payment_amount(payment_rule, source))
                    .is_some_and(|amount| payment.pays_generic(amount));
                if !paid {
                    for _ in 0..draw.count {
                        if let Some(card_index) = library_order.get(*next_draw_position) {
                            hand.push(*card_index);
                            *next_draw_position += 1;
                        }
                    }
                }
            }
            AbilityEffect::UnlessEventPlayerPays(unless) => {
                let paid = optional_payment_amount(&unless.payment, source)
                    .is_some_and(|amount| payment.pays_generic(amount));
                if !paid {
                    execute_table_trigger_effects(
                        &unless.if_not_paid,
                        source,
                        payment,
                        hand,
                        library_order,
                        next_draw_position,
                        treasure_reserve,
                        player_life,
                    );
                }
            }
            AbilityEffect::LoseLife(loss) => {
                *player_life -= f32::from(loss.amount);
            }
            AbilityEffect::CreateToken(token) => {
                *treasure_reserve =
                    treasure_reserve.saturating_add(token.count.min(u16::from(u8::MAX)) as u8);
            }
            _ => unreachable!("table trigger effects are validated before execution"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_opponent_turn_activity(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    opponent: &OpponentTurnActivity,
    hand: &mut Vec<usize>,
    library_order: &[usize],
    next_draw_position: &mut usize,
    treasure_reserve: &mut u8,
    player_life: &mut f32,
) -> bool {
    let sources = zones.battlefield.clone();
    for draw_event in opponent.draws() {
        for presence in &sources {
            let Some(source) = deck.cards.get(presence.card_index) else {
                continue;
            };
            for ability in source.ability_program.executable_abilities() {
                let AbilityTiming::Triggered { event } = &ability.timing else {
                    continue;
                };
                if event.kind != TriggerEventKind::CardDraw
                    || !matches!(
                        event.actor,
                        ControllerRelation::Opponent | ControllerRelation::Any
                    )
                    || !ability.costs.is_empty()
                    || !ability
                        .preconditions
                        .contains(&AbilityPrecondition::SourceZone(ProgramZone::Battlefield))
                    || !program_filter_matches_opponent_draw(&event.object_filter)
                    || !table_trigger_effects_supported(&ability.effects, source)
                {
                    continue;
                }
                execute_table_trigger_effects(
                    &ability.effects,
                    source,
                    draw_event.payment(),
                    hand,
                    library_order,
                    next_draw_position,
                    treasure_reserve,
                    player_life,
                );
            }
        }
    }
    for spell_event in opponent.spells() {
        for presence in &sources {
            let Some(source) = deck.cards.get(presence.card_index) else {
                continue;
            };
            for ability in source.ability_program.executable_abilities() {
                let AbilityTiming::Triggered { event } = &ability.timing else {
                    continue;
                };
                if !matches!(
                    event.actor,
                    ControllerRelation::Opponent | ControllerRelation::Any
                ) || !ability.costs.is_empty()
                    || !ability
                        .preconditions
                        .contains(&AbilityPrecondition::SourceZone(ProgramZone::Battlefield))
                    || !program_filter_matches_opponent_spell(&event.object_filter, spell_event)
                    || !table_trigger_effects_supported(&ability.effects, source)
                {
                    continue;
                }
                let trigger_matches = match event.kind {
                    TriggerEventKind::SpellCast => true,
                    TriggerEventKind::FirstFilteredSpellCastEachTurn => {
                        spell_event.noncreature_ordinal() == Some(1)
                    }
                    TriggerEventKind::SecondSpellCastEachTurn => spell_event.ordinal() == 2,
                    _ => false,
                };
                if trigger_matches {
                    execute_table_trigger_effects(
                        &ability.effects,
                        source,
                        spell_event.payment(),
                        hand,
                        library_order,
                        next_draw_position,
                        treasure_reserve,
                        player_life,
                    );
                }
            }
        }
    }
    *player_life > 0.0
}

#[allow(clippy::too_many_arguments)]
fn apply_table_turn_activity_with_end_steps(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    activity: &TableTurnActivity,
    combat_state: &CommanderCombatState,
    hand: &mut Vec<usize>,
    library_order: &[usize],
    next_draw_position: &mut usize,
    treasure_reserve: &mut u8,
    player_life: &mut f32,
) -> bool {
    for opponent in activity.opponents() {
        let Ok(opponent_id) = OpponentId::new(usize::from(opponent.opponent_index())) else {
            continue;
        };
        if !combat_state.is_opponent_active(opponent_id) {
            continue;
        }

        // “This turn” is the current opponent's turn, not the analyzer's
        // complete three-opponent table round. Persistent monarch/counters
        // survive this epoch advance; life-loss receipts do not.
        let _ = zones.turn_events.state.start_next_turn();
        let life_before_opponent_turn = *player_life;
        if !apply_opponent_turn_activity(
            deck,
            zones,
            opponent,
            hand,
            library_order,
            next_draw_position,
            treasure_reserve,
            player_life,
        ) {
            return false;
        }
        zones.record_controller_life_delta(life_before_opponent_turn, *player_life);
        resolve_opponent_end_step_counter_triggers(deck, zones, combat_state, opponent_id);
    }
    *player_life > 0.0
}

fn controller_spell_trigger_effects_supported(effects: &[AbilityEffect]) -> bool {
    effects.iter().all(|effect| match effect {
        AbilityEffect::AddMana(mana) => matches!(mana.kind, ProgramManaKind::Fixed(_)),
        AbilityEffect::AddManaWithRetention(linked) => {
            linked.retention == ManaRetention::ThroughStepsAndPhasesUntilEndOfTurn
                && matches!(linked.mana.kind, ProgramManaKind::Fixed(_))
        }
        AbilityEffect::LoseLife(loss) => loss.player == ControllerRelation::You,
        AbilityEffect::CreateToken(token) => match &token.kind {
            TokenKind::Treasure => true,
            TokenKind::Creature { power, .. } => *power > 0,
        },
        _ => false,
    })
}

fn apply_controller_spell_cast_triggers(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    cast_card_index: usize,
    turn: u8,
    spell_ordinal: u8,
    mana_pool: &mut TurnManaPool,
    player_life: &mut f32,
) -> bool {
    let Some(cast_card) = deck.cards.get(cast_card_index) else {
        return false;
    };
    for presence in zones.battlefield.clone() {
        let Some(source) = deck.cards.get(presence.card_index) else {
            continue;
        };
        for ability in source.ability_program.executable_abilities() {
            let AbilityTiming::Triggered { event } = &ability.timing else {
                continue;
            };
            if !matches!(
                event.actor,
                ControllerRelation::You | ControllerRelation::Any
            ) || !ability.costs.is_empty()
                || !ability
                    .preconditions
                    .contains(&AbilityPrecondition::SourceZone(ProgramZone::Battlefield))
                || !program_filter_matches_controller_spell(&event.object_filter, cast_card)
                || !controller_spell_trigger_effects_supported(&ability.effects)
            {
                continue;
            }
            let trigger_matches = match event.kind {
                TriggerEventKind::SpellCast => true,
                TriggerEventKind::SecondSpellCastEachTurn => spell_ordinal == 2,
                _ => false,
            };
            if !trigger_matches {
                continue;
            }
            let trigger_count = trigger_multiplier_count(
                deck,
                zones,
                CombatTriggerEventKind::SpellCast,
                None,
                presence.sequence,
            );
            for _ in 0..trigger_count {
                for effect in &ability.effects {
                    match effect {
                        AbilityEffect::AddMana(mana) => {
                            let ProgramManaKind::Fixed(output) = mana.kind else {
                                unreachable!("controller spell trigger was validated")
                            };
                            add_fixed_mana(mana_pool, output);
                        }
                        AbilityEffect::AddManaWithRetention(linked) => {
                            let ProgramManaKind::Fixed(output) = linked.mana.kind else {
                                unreachable!("controller spell trigger was validated")
                            };
                            add_fixed_mana(mana_pool, output);
                        }
                        AbilityEffect::LoseLife(loss) => {
                            *player_life -= f32::from(loss.amount);
                            zones.record_controller_life_loss(u32::from(loss.amount));
                        }
                        AbilityEffect::CreateToken(token) => match &token.kind {
                            TokenKind::Treasure => {
                                mana_pool.add_treasures(token.count.min(u16::from(u8::MAX)) as u8);
                            }
                            TokenKind::Creature {
                                power,
                                toughness,
                                description,
                                keywords,
                            } => zones.record_creature_tokens(
                                deck,
                                turn,
                                token.count,
                                *power,
                                *toughness,
                                description,
                                keywords,
                            ),
                        },
                        _ => unreachable!("controller spell trigger effects were validated"),
                    }
                }
            }
        }
    }
    *player_life > 0.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypedBurstCardAccessProgram {
    WholeHandDiscardThenDraw,
    RepeatableTopCardReveal,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct TypedBurstCardAccessResolution {
    cards_accessed: usize,
    player_died: bool,
}

/// Revalidate the complete executable shape at the runtime boundary. A nearby
/// supported effect, unsupported sibling paragraph, activated/triggered
/// lookalike, or manually assembled partial IR must not receive execution.
fn compile_typed_burst_card_access_program(
    card: &CompiledCard,
) -> Option<TypedBurstCardAccessProgram> {
    let [AbilityCompilation::Executable(ability)] = card.ability_program.abilities.as_slice()
    else {
        return None;
    };
    if ability.timing != AbilityTiming::SpellResolution
        || !ability.costs.is_empty()
        || ability.preconditions != [AbilityPrecondition::SourceZone(ProgramZone::Stack)]
    {
        return None;
    }

    match ability.effects.as_slice() {
        [AbilityEffect::WholeHandDiscardThenDraw(effect)]
            if effect.players == AffectedPlayers::EachPlayer
                && effect.discard.from == ProgramZone::Hand
                && effect.discard.to == ProgramZone::Graveyard
                && effect.draw.count == 7
                && effect.draw.from == ProgramZone::Library
                && effect.draw.to == ProgramZone::Hand =>
        {
            Some(TypedBurstCardAccessProgram::WholeHandDiscardThenDraw)
        }
        [AbilityEffect::RepeatableTopCardReveal(effect)]
            if effect.player == ControllerRelation::You
                && effect.iteration.reveal.count == 1
                && effect.iteration.reveal.from == ProgramZone::Library
                && effect.iteration.movement.from == ProgramZone::Library
                && effect.iteration.movement.to == ProgramZone::Hand
                && effect.iteration.life_loss
                    == CoupledLifeLoss::ManaValueOfCardMovedByThisIteration
                && effect.repetition
                    == RepetitionPolicy::OneMandatoryThenMayRepeatEntireIterationAnyNumberOfTimes =>
        {
            Some(TypedBurstCardAccessProgram::RepeatableTopCardReveal)
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_typed_burst_card_access(
    program: TypedBurstCardAccessProgram,
    deck: &CompiledDeck,
    hand: &mut Vec<usize>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    zones: &mut KnownLineZoneState,
    life_total: &mut f32,
) -> TypedBurstCardAccessResolution {
    match program {
        TypedBurstCardAccessProgram::WholeHandDiscardThenDraw => {
            // Opponent hands and libraries do not exist in this one-player
            // trajectory. Project the EachPlayer instruction onto the modeled
            // controller only; never manufacture opposing objects or cards.
            zones.record_discards(deck, std::mem::take(hand));
            let mut cards_accessed = 0usize;
            for _ in 0..7 {
                if next_draw_position >= library_order.len() {
                    break;
                }
                hand.push(library_order.remove(next_draw_position));
                cards_accessed += 1;
            }
            TypedBurstCardAccessResolution {
                cards_accessed,
                player_died: false,
            }
        }
        TypedBurstCardAccessProgram::RepeatableTopCardReveal => {
            let mut cards_accessed = 0usize;
            while next_draw_position < library_order.len() {
                // The first iteration is mandatory. Every later decision uses
                // only life plus the public remaining multiset, never the
                // hidden next position.
                if cards_accessed > 0
                    && !should_repeat_top_card_reveal(
                        deck,
                        library_order,
                        next_draw_position,
                        *life_total,
                    )
                {
                    break;
                }
                let card_index = library_order[next_draw_position];
                let Some(mana_value) = deck.cards.get(card_index).and_then(actual_card_mana_value)
                else {
                    break;
                };

                // One iteration is atomic and ordered: reveal the real top
                // object, move that same object to hand, then lose its value.
                library_order.remove(next_draw_position);
                hand.push(card_index);
                *life_total -= mana_value;
                cards_accessed += 1;
                if *life_total <= 0.0 {
                    return TypedBurstCardAccessResolution {
                        cards_accessed,
                        player_died: true,
                    };
                }
            }
            TypedBurstCardAccessResolution {
                cards_accessed,
                player_died: false,
            }
        }
    }
}

fn actual_card_mana_value(card: &CompiledCard) -> Option<f32> {
    card.mana_value
        .is_finite()
        .then_some(card.mana_value.max(0.0))
}

fn should_repeat_top_card_reveal(
    deck: &CompiledDeck,
    library_order: &[usize],
    next_draw_position: usize,
    life_total: f32,
) -> bool {
    let Some(unseen_library) = library_order.get(next_draw_position..) else {
        return false;
    };
    let mut remaining = unseen_library.iter();
    let Some(first_index) = remaining.next() else {
        return false;
    };
    let Some(first) = deck.cards.get(*first_index) else {
        return false;
    };
    let Some(mut maximum_mana_value) = actual_card_mana_value(first) else {
        return false;
    };
    for card_index in remaining {
        let Some(mana_value) = deck.cards.get(*card_index).and_then(actual_card_mana_value) else {
            return false;
        };
        maximum_mana_value = maximum_mana_value.max(mana_value);
    }
    // The fixed production policy is the aggressive cEDH policy. Continue
    // exactly while even the highest-mana-value card in the public remaining
    // multiset cannot make the next reveal lethal. The actual hidden top card
    // is still revealed and paid for one physical object at a time.
    life_total > maximum_mana_value
}

fn modeled_line_card_kind(card: &CompiledCard) -> Option<ModeledLineCardKind> {
    let is_permanent = card.has(role::LAND | role::CREATURE | role::ARTIFACT | role::ENCHANTMENT);
    let is_spell = card.has(role::INSTANT_SORCERY);
    match (is_permanent, is_spell) {
        (true, false) => Some(ModeledLineCardKind::Permanent),
        (false, true) => Some(ModeledLineCardKind::Spell),
        // Modal/ambiguous type combinations and unsupported permanent types
        // require a richer face/zone model. Excluding them is safer than
        // assigning whichever face would make a combo work.
        _ => None,
    }
}

fn unique_card_by_normalized_name<'a>(
    deck: &'a CompiledDeck,
    normalized_name: &str,
) -> Option<&'a CompiledCard> {
    let mut matches = deck
        .cards
        .iter()
        .filter(|card| card.normalized_name == normalized_name);
    let card = matches.next()?;
    matches.next().is_none().then_some(card)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypedConditionalManaSource {
    ImprintLinkedCardColors,
    DiscardLandOrFailEntry,
    ControlledLegendaryColors,
    MetalcraftAnyColor,
    FixedWithSourceDamage {
        output: FixedManaProfile,
        damage: u16,
    },
    ColorlessOrAnyColorWithSourceDamage {
        damage: u16,
    },
}

impl TypedConditionalManaSource {
    fn is_entry_linked(self) -> bool {
        matches!(
            self,
            Self::ImprintLinkedCardColors | Self::DiscardLandOrFailEntry
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TypedPermanentEntryResolution {
    entered_battlefield: bool,
    linked_colors: Option<ManaColorMask>,
    moved_hand_card: Option<usize>,
}

fn entry_filter_matches(
    deck: &CompiledDeck,
    card_index: usize,
    filter: EntryLinkedCardFilter,
) -> bool {
    let Some(card) = deck.cards.get(card_index) else {
        return false;
    };
    let hand = card.hand_zone_characteristics();
    let card_types = if hand.exact {
        hand.card_types
    } else if legacy_single_face_hand_characteristics_available(card) {
        // See `printed_hand_card_colors`: this is an explicitly uncertified,
        // single-face-only trajectory fallback. Strict coverage remains
        // blocked until the source record is refreshed.
        card.effects.card_types
    } else {
        return false;
    };
    match filter {
        EntryLinkedCardFilter::NonartifactNonlandCard => {
            !card_types.is_artifact && !card_types.is_land
        }
        EntryLinkedCardFilter::LandCard => card_types.is_land,
    }
}

fn imprint_preservation_score(card: &CompiledCard) -> i32 {
    let mut score = (card.mana_value.max(0.0) * 10.0).round() as i32;
    for (role_mask, value) in [
        (role::WIN_CONDITION, 12_000),
        (role::COMBO_PIECE, 11_000),
        (role::TUTOR, 9_000),
        (role::PAYOFF, 7_000),
        (role::ENGINE, 6_000),
        (role::DRAW, 5_000),
        (role::PROTECTION | role::COUNTERSPELL, 4_000),
        (role::ENABLER, 3_000),
    ] {
        if card.has(role_mask) {
            score += value;
        }
    }
    score
}

#[allow(clippy::too_many_arguments)]
fn select_imprint_hand_position(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
) -> Option<usize> {
    hand.iter()
        .copied()
        .enumerate()
        .filter(|(_, card_index)| {
            entry_filter_matches(
                deck,
                *card_index,
                EntryLinkedCardFilter::NonartifactNonlandCard,
            )
        })
        .filter_map(|(position, card_index)| {
            let card = deck.cards.get(card_index)?;
            let colors = printed_hand_card_colors(card)?;
            let mut remaining_hand = hand.to_vec();
            remaining_hand.remove(position);
            let mut post_entry_pool = mana_pool.clone();
            post_entry_pool.add_floating(colors, 1);
            let reviewed_route_potential = planning_reviewed_sequence_potential(
                deck,
                mana_access,
                &remaining_hand,
                zones,
                turn,
                &post_entry_pool,
                future_additional_generic_per_cast,
            );
            (!colors.is_empty()).then_some((
                position,
                reviewed_route_potential,
                imprint_preservation_score(card),
                source_option_count(colors),
                card.normalized_name.as_str(),
                card_index,
            ))
        })
        .max_by(|left, right| {
            left.1
                .cmp(&right.1)
                // Once the exact route value is tied, pitch the least
                // strategically valuable card and prefer broader colors.
                .then_with(|| right.2.cmp(&left.2))
                .then_with(|| left.3.cmp(&right.3))
                .then_with(|| right.4.cmp(left.4))
                .then_with(|| right.5.cmp(&left.5))
        })
        .map(|(position, _, _, _, _, _)| position)
}

#[allow(clippy::too_many_arguments)]
fn select_land_discard_hand_position(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
) -> Option<usize> {
    hand.iter()
        .copied()
        .enumerate()
        .filter(|(_, card_index)| {
            entry_filter_matches(deck, *card_index, EntryLinkedCardFilter::LandCard)
        })
        .map(|(position, card_index)| {
            let mut remaining_hand = hand.to_vec();
            remaining_hand.remove(position);
            let mut post_entry_pool = mana_pool.clone();
            post_entry_pool.add_floating(ManaColorMask::ANY_COLOR, 1);
            let current_route_potential = planning_reviewed_sequence_potential(
                deck,
                mana_access,
                &remaining_hand,
                zones,
                turn,
                &post_entry_pool,
                future_additional_generic_per_cast,
            );
            let retained_land_potential = retained_land_route_potential(
                deck,
                mana_access,
                &remaining_hand,
                zones,
                turn,
                &post_entry_pool,
                future_additional_generic_per_cast,
            );
            (
                position,
                card_index,
                current_route_potential,
                retained_land_potential,
                land_play_score_for_card(deck, mana_access, card_index),
            )
        })
        .max_by(|left, right| {
            left.2
                .cmp(&right.2)
                .then_with(|| left.3.cmp(&right.3))
                // If route preservation is tied, discard the weaker land.
                .then_with(|| right.4.total_cmp(&left.4))
                .then_with(|| {
                    deck.cards[right.1]
                        .normalized_name
                        .cmp(&deck.cards[left.1].normalized_name)
                })
                .then_with(|| right.1.cmp(&left.1))
        })
        .map(|(position, _, _, _, _)| position)
}

#[allow(clippy::too_many_arguments)]
fn retained_land_route_potential(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
) -> i64 {
    hand.iter()
        .copied()
        .filter(|card_index| {
            entry_filter_matches(deck, *card_index, EntryLinkedCardFilter::LandCard)
        })
        .filter_map(|card_index| {
            let source = mana_access?.source(card_index)?;
            (!source.unknown && !source.colors.is_empty()).then_some(source.colors)
        })
        .map(|colors| {
            let mut future_pool = mana_pool.clone();
            // This is a one-turn look-ahead ranking probe only. It does not
            // play the retained land or mutate the real trajectory.
            future_pool.add_floating(colors, 1);
            planning_reviewed_sequence_potential(
                deck,
                mana_access,
                hand,
                zones,
                turn,
                &future_pool,
                future_additional_generic_per_cast,
            )
        })
        .max()
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn resolve_typed_permanent_entry(
    kind: TypedConditionalManaSource,
    deck: &CompiledDeck,
    source_card_index: usize,
    hand: &mut Vec<usize>,
    mana_access: Option<&ManaAccessProfile>,
    zones: &mut KnownLineZoneState,
    turn: u8,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
) -> TypedPermanentEntryResolution {
    match kind {
        TypedConditionalManaSource::ImprintLinkedCardColors => {
            let moved_hand_card = select_imprint_hand_position(
                deck,
                mana_access,
                hand,
                zones,
                turn,
                mana_pool,
                future_additional_generic_per_cast,
            )
            .map(|position| {
                let card_index = hand.swap_remove(position);
                zones.exile.push(card_index);
                zones.advance_sequence();
                card_index
            });
            let linked_colors = moved_hand_card
                .and_then(|card_index| deck.cards.get(card_index))
                .and_then(printed_hand_card_colors);
            zones.record_cast(deck, source_card_index, turn);
            TypedPermanentEntryResolution {
                entered_battlefield: true,
                linked_colors,
                moved_hand_card,
            }
        }
        TypedConditionalManaSource::DiscardLandOrFailEntry => {
            let Some(position) = select_land_discard_hand_position(
                deck,
                mana_access,
                hand,
                zones,
                turn,
                mana_pool,
                future_additional_generic_per_cast,
            ) else {
                zones.graveyard.push(source_card_index);
                zones.advance_sequence();
                return TypedPermanentEntryResolution::default();
            };
            let discarded = hand.swap_remove(position);
            zones.record_discard(deck, discarded);
            zones.record_cast(deck, source_card_index, turn);
            TypedPermanentEntryResolution {
                entered_battlefield: true,
                linked_colors: None,
                moved_hand_card: Some(discarded),
            }
        }
        _ => {
            zones.record_cast(deck, source_card_index, turn);
            TypedPermanentEntryResolution {
                entered_battlefield: true,
                linked_colors: None,
                moved_hand_card: None,
            }
        }
    }
}

fn compile_typed_conditional_mana_source(
    card: &CompiledCard,
) -> Option<TypedConditionalManaSource> {
    let program = &card.ability_program;
    if program.atomic_transaction.is_some() {
        return None;
    }

    if let Some(permanent) = program.executable_entry_linked_permanent() {
        if program.unsupported_entry_linked_permanent().is_some()
            || !program.abilities.is_empty()
            || !card.effects.card_types.is_artifact
            || permanent.mana_ability.costs != [AbilityCost::TapSelf]
            || permanent.mana_ability.preconditions
                != [
                    AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                    AbilityPrecondition::SourceUntapped,
                ]
        {
            return None;
        }
        return match (&permanent.entry, &permanent.mana_ability.output) {
            (
                PermanentEntryProcedure::OptionalImprint {
                    filter: EntryLinkedCardFilter::NonartifactNonlandCard,
                    from: ProgramZone::Hand,
                    to: ProgramZone::Exile,
                    link: LinkedEntryObject::CardExiledByThisEntry,
                },
                EntryLinkedManaOutput::AnyColorOfLinkedCard {
                    linked: LinkedEntryObject::CardExiledByThisEntry,
                },
            ) => Some(TypedConditionalManaSource::ImprintLinkedCardColors),
            (
                PermanentEntryProcedure::DiscardOrFailToEnter {
                    filter: EntryLinkedCardFilter::LandCard,
                    discard_from: ProgramZone::Hand,
                    discard_to: ProgramZone::Graveyard,
                    success_destination: ProgramZone::Battlefield,
                    failure_destination: ProgramZone::Graveyard,
                },
                EntryLinkedManaOutput::AnyOneColor,
            ) => Some(TypedConditionalManaSource::DiscardLandOrFailEntry),
            _ => None,
        };
    }
    if program.entry_linked_permanent.is_some() || program.unsupported_abilities().next().is_some()
    {
        return None;
    }
    let base_preconditions = [
        AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
        AbilityPrecondition::SourceUntapped,
    ];
    if program.abilities.len() == 2 && card.effects.card_types.is_land {
        let mut has_colorless_mode = false;
        let mut damaging_any_color_mode = None;
        for compilation in &program.abilities {
            let AbilityCompilation::Executable(ability) = compilation else {
                return None;
            };
            if ability.timing
                != (AbilityTiming::Activated {
                    window: crate::ability_program::ActivationWindow::NormalPriority,
                })
                || ability.costs != [AbilityCost::TapSelf]
                || ability.preconditions != base_preconditions
            {
                return None;
            }
            match ability.effects.as_slice() {
                [AbilityEffect::AddMana(mana)]
                    if mana.amount == 1
                        && mana.kind
                            == ProgramManaKind::Fixed(FixedManaProfile {
                                colorless: 1,
                                ..FixedManaProfile::default()
                            })
                        && !has_colorless_mode =>
                {
                    has_colorless_mode = true;
                }
                [AbilityEffect::AddManaAndSourceDamage(linked)]
                    if linked.mana.amount == 1
                        && linked.mana.kind == ProgramManaKind::AnyOneColor
                        && linked.damage.amount == 3
                        && linked.damage.recipient == ControllerRelation::You
                        && damaging_any_color_mode.is_none() =>
                {
                    damaging_any_color_mode = Some(linked.damage.amount);
                }
                _ => return None,
            }
        }
        if has_colorless_mode {
            return damaging_any_color_mode.map(|damage| {
                TypedConditionalManaSource::ColorlessOrAnyColorWithSourceDamage { damage }
            });
        }
        return None;
    }
    if program.abilities.len() != 1 {
        return None;
    }
    let ability = program.executable_abilities().next()?;
    if ability.timing
        != (AbilityTiming::Activated {
            window: crate::ability_program::ActivationWindow::NormalPriority,
        })
        || ability.costs != [AbilityCost::TapSelf]
    {
        return None;
    }
    match ability.effects.as_slice() {
        [AbilityEffect::AddMana(mana)]
            if ability.preconditions == base_preconditions
                && mana.amount == 1
                && mana.kind
                    == ProgramManaKind::AnyColorAmongLegendaryCreaturesAndPlaneswalkersYouControl
                && card.effects.card_types.is_artifact =>
        {
            Some(TypedConditionalManaSource::ControlledLegendaryColors)
        }
        [AbilityEffect::AddMana(mana)]
            if ability.preconditions
                == [
                    AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                    AbilityPrecondition::SourceUntapped,
                    AbilityPrecondition::ResourceAtLeast {
                        resource: ResourceKind::Artifact,
                        count: 3,
                    },
                ]
                && mana.amount == 1
                && mana.kind == ProgramManaKind::AnyOneColor
                && card.effects.card_types.is_artifact =>
        {
            Some(TypedConditionalManaSource::MetalcraftAnyColor)
        }
        [AbilityEffect::AddManaAndSourceDamage(linked)]
            if ability.preconditions == base_preconditions
                && linked.mana.amount == 2
                && linked.mana.kind
                    == ProgramManaKind::Fixed(FixedManaProfile {
                        colorless: 2,
                        ..FixedManaProfile::default()
                    })
                && linked.damage.amount == 2
                && linked.damage.recipient == ControllerRelation::You
                && card.effects.card_types.is_land =>
        {
            Some(TypedConditionalManaSource::FixedWithSourceDamage {
                output: FixedManaProfile {
                    colorless: 2,
                    ..FixedManaProfile::default()
                },
                damage: 2,
            })
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TypedAtomicTransaction {
    HandMana {
        output: FixedManaProfile,
    },
    SacrificeRitual {
        output: FixedManaProfile,
    },
    NameLinkedGraveyardRitual {
        base: FixedManaProfile,
        per_match: FixedManaProfile,
        /// Opponent graveyard objects are not present in the bounded local
        /// state. Zero is therefore an explicit conservative floor, not an
        /// assertion that opponents have no matching cards.
        opponent_matching_card_floor: u16,
    },
    SacrificeTutor {
        tutor: ProgramTutorEffect,
    },
    ThresholdRitual {
        default: FixedManaProfile,
        replacement: FixedManaProfile,
        threshold: u16,
    },
    SearchRandomDiscardShuffle {
        tutor: ProgramTutorEffect,
    },
    TemporaryLandSacrificeManaGrant {
        output: FixedManaProfile,
    },
    BargainSearchCastOrHand {
        maximum_mana_value: u16,
    },
    OpponentChoiceSearchSplit,
}

impl TypedAtomicTransaction {
    fn initiation(&self) -> AtomicInitiation {
        match self {
            Self::HandMana { .. } => AtomicInitiation::HandManaAbility,
            Self::SacrificeRitual { .. }
            | Self::NameLinkedGraveyardRitual { .. }
            | Self::SacrificeTutor { .. }
            | Self::ThresholdRitual { .. }
            | Self::SearchRandomDiscardShuffle { .. }
            | Self::TemporaryLandSacrificeManaGrant { .. }
            | Self::BargainSearchCastOrHand { .. }
            | Self::OpponentChoiceSearchSplit => AtomicInitiation::CastSpell,
        }
    }

    fn is_mana_development(&self) -> bool {
        matches!(
            self,
            Self::HandMana { .. }
                | Self::SacrificeRitual { .. }
                | Self::NameLinkedGraveyardRitual { .. }
                | Self::ThresholdRitual { .. }
                | Self::TemporaryLandSacrificeManaGrant { .. }
        )
    }

    fn is_tutor(&self) -> bool {
        matches!(
            self,
            Self::SacrificeTutor { .. }
                | Self::SearchRandomDiscardShuffle { .. }
                | Self::BargainSearchCastOrHand { .. }
                | Self::OpponentChoiceSearchSplit
        )
    }
}

/// Runtime revalidation is deliberately as narrow as the compiler. This
/// prevents a hand-authored or future extended IR shape from inheriting
/// execution merely because one nearby cost/effect is familiar.
fn compile_typed_atomic_transaction(card: &CompiledCard) -> Option<TypedAtomicTransaction> {
    let transaction = card.ability_program.executable_atomic_transaction()?;
    if card
        .ability_program
        .unsupported_atomic_transaction()
        .is_some()
        || !card.ability_program.abilities.is_empty()
        || card.ability_program.necropotence_lifecycle.is_some()
        || card.ability_program.self_transfer_tutor_permanent.is_some()
        || card.ability_program.entry_linked_permanent.is_some()
        || !card.ability_program.face_programs.is_empty()
        || card
            .ability_program
            .unsupported_abilities()
            .next()
            .is_some()
    {
        return None;
    }

    match (
        transaction.initiation,
        transaction.source_zone,
        transaction.initiation_costs.as_slice(),
        transaction.resolution.as_slice(),
    ) {
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost],
            [AtomicEffect::TemporaryLandSacrificeManaGrant(effect)],
        ) if effect.player == ControllerRelation::You
            && effect.affected_zone == ProgramZone::Battlefield
            && effect.affected_filter
                == (ProgramObjectFilter {
                    card_type: Some(ProgramCardType::Land),
                    controller: Some(ControllerRelation::You),
                    ..ProgramObjectFilter::default()
                })
            && effect.applies_to_future_matching_objects
            && effect.duration == AtomicEffectDuration::UntilEndOfTurn
            && effect.granted_ability.kind == GrantedAbilityKind::ManaAbility
            && effect.granted_ability.source_zone == ProgramZone::Battlefield
            && effect.granted_ability.controller == ControllerRelation::You
            && effect.granted_ability.cost == GrantedSelfCost::SacrificeSelf
            && effect.granted_ability.output
                == (FixedManaProfile {
                    black: 1,
                    ..FixedManaProfile::default()
                })
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            Some(TypedAtomicTransaction::TemporaryLandSacrificeManaGrant {
                output: effect.granted_ability.output,
            })
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost, AtomicCost::Bargain(bargain)],
            [AtomicEffect::BargainSearchCastOrHand(effect)],
        ) if exact_bargain_cost(bargain)
            && exact_bargain_search_cast_or_hand(effect)
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            Some(TypedAtomicTransaction::BargainSearchCastOrHand {
                maximum_mana_value: effect.conditional_cast.mana_value.maximum,
            })
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost],
            [AtomicEffect::OpponentChoiceSearchSplit(effect)],
        ) if exact_opponent_choice_search_split(effect)
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            Some(TypedAtomicTransaction::OpponentChoiceSearchSplit)
        }
        (
            AtomicInitiation::HandManaAbility,
            ProgramZone::Hand,
            [
                AtomicCost::ExileSelf {
                    from: ProgramZone::Hand,
                },
            ],
            [AtomicEffect::AddFixedMana(output)],
        ) if fixed_mana_is_exact_hand_output(*output) => {
            Some(TypedAtomicTransaction::HandMana { output: *output })
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [
                AtomicCost::PrintedManaCost,
                AtomicCost::SacrificePermanents {
                    filter,
                    count: 1,
                    commander_eligibility: CommanderEligibility::Exclude,
                },
            ],
            [AtomicEffect::AddFixedMana(output)],
        ) if exact_noncommander_creature_filter(filter)
            && *output
                == (FixedManaProfile {
                    black: 4,
                    ..FixedManaProfile::default()
                })
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            Some(TypedAtomicTransaction::SacrificeRitual { output: *output })
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [
                AtomicCost::PrintedManaCost,
                AtomicCost::SacrificePermanents {
                    filter,
                    count: 1,
                    commander_eligibility: CommanderEligibility::Exclude,
                },
            ],
            [AtomicEffect::SearchToHand(tutor)],
        ) if exact_noncommander_creature_filter(filter)
            && exact_any_card_tutor(tutor, true)
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            Some(TypedAtomicTransaction::SacrificeTutor {
                tutor: tutor.clone(),
            })
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost],
            [
                AtomicEffect::AddFixedMana(base),
                AtomicEffect::AddManaPerNamedCardInGraveyards(dynamic),
            ],
        ) if *base
            == (FixedManaProfile {
                red: 2,
                ..FixedManaProfile::default()
            })
            && dynamic.mana_per_card
                == (FixedManaProfile {
                    red: 1,
                    ..FixedManaProfile::default()
                })
            && dynamic.card_name == AtomicCardNameReference::SourceCardName
            && dynamic.graveyards == AtomicGraveyardScope::EachPlayerGraveyard
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            Some(TypedAtomicTransaction::NameLinkedGraveyardRitual {
                base: *base,
                per_match: dynamic.mana_per_card,
                opponent_matching_card_floor: 0,
            })
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost],
            [AtomicEffect::ConditionalManaReplacement(effect)],
        ) if effect.default
            == (FixedManaProfile {
                black: 3,
                ..FixedManaProfile::default()
            })
            && effect.replacement
                == (FixedManaProfile {
                    black: 5,
                    ..FixedManaProfile::default()
                })
            && effect.condition
                == (AtomicStateCondition::CardsInZoneAtLeast {
                    player: ControllerRelation::You,
                    zone: ProgramZone::Graveyard,
                    count: 7,
                })
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            Some(TypedAtomicTransaction::ThresholdRitual {
                default: effect.default,
                replacement: effect.replacement,
                threshold: 7,
            })
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost],
            [
                AtomicEffect::SearchToHand(tutor),
                AtomicEffect::RandomDiscard(discard),
                AtomicEffect::ShuffleLibrary(shuffle),
            ],
        ) if exact_any_card_tutor(tutor, false)
            && discard.player == ControllerRelation::You
            && discard.count == 1
            && discard.from == ProgramZone::Hand
            && discard.to == ProgramZone::Graveyard
            && discard.selection == RandomSelection::UniformAmongObjectsInZone
            && shuffle.player == ControllerRelation::You
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            Some(TypedAtomicTransaction::SearchRandomDiscardShuffle {
                tutor: tutor.clone(),
            })
        }
        _ => None,
    }
}

fn fixed_mana_is_exact_hand_output(output: FixedManaProfile) -> bool {
    output
        == (FixedManaProfile {
            red: 1,
            ..FixedManaProfile::default()
        })
        || output
            == (FixedManaProfile {
                green: 1,
                ..FixedManaProfile::default()
            })
}

fn exact_noncommander_creature_filter(filter: &ProgramObjectFilter) -> bool {
    filter
        == &(ProgramObjectFilter {
            card_type: Some(ProgramCardType::Creature),
            controller: Some(ControllerRelation::You),
            ..ProgramObjectFilter::default()
        })
}

fn exact_any_card_tutor(tutor: &ProgramTutorEffect, shuffle_after: bool) -> bool {
    tutor.from == ProgramZone::Library
        && tutor.destination == ProgramZone::Hand
        && tutor.shuffle_after == shuffle_after
        && tutor.filter
            == TutorFilter::AnyOf(vec![ProgramObjectFilter {
                card_type: Some(ProgramCardType::Card),
                ..ProgramObjectFilter::default()
            }])
}

fn exact_atomic_any_card_search(search: &AtomicLibrarySearch, quantity: u16, reveal: bool) -> bool {
    search.player == ControllerRelation::You
        && search.from == ProgramZone::Library
        && search.minimum == quantity
        && search.maximum == quantity
        && search.reveal == reveal
        && search.filter
            == TutorFilter::AnyOf(vec![ProgramObjectFilter {
                card_type: Some(ProgramCardType::Card),
                ..ProgramObjectFilter::default()
            }])
}

fn exact_bargain_cost(cost: &AtomicBargainCost) -> bool {
    cost.player == ControllerRelation::You
        && cost.timing == AtomicAdditionalCostTiming::AsThisSpellIsCast
        && cost.optional
        && cost.from == ProgramZone::Battlefield
        && cost.count == 1
        && cost.eligible_kinds
            == [
                BargainSacrificeKind::Artifact,
                BargainSacrificeKind::Enchantment,
                BargainSacrificeKind::Token,
            ]
        && cost.commander_eligibility == CommanderEligibility::Include
}

fn exact_bargain_search_cast_or_hand(effect: &BargainSearchCastOrHandEffect) -> bool {
    exact_atomic_any_card_search(&effect.search, 1, false)
        && effect.searched_card == AtomicTrackedObject::OnlyCardFoundByThisSearch
        && effect.initial_destination == ProgramZone::Exile
        && effect.face_down
        && effect.shuffle.player == ControllerRelation::You
        && effect.shuffle.timing
            == AtomicShuffleTiming::AfterInitialSearchMovementBeforeConditionalCast
        && effect.conditional_cast.condition == AtomicCastPermissionCondition::ThisSpellWasBargained
        && effect.conditional_cast.card == AtomicTrackedObject::OnlyCardFoundByThisSearch
        && effect.conditional_cast.from == ProgramZone::Exile
        && effect.conditional_cast.optional
        && effect.conditional_cast.mana_value.subject == AtomicManaValueSubject::SpellAsCast
        && effect.conditional_cast.mana_value.maximum == 4
        && effect.conditional_cast.cost_waiver == AtomicCastCostWaiver::ManaCostOnly
        && effect.if_not_cast.condition == AtomicMovementCondition::NotCastByThisEffect
        && effect.if_not_cast.object == AtomicTrackedObject::OnlyCardFoundByThisSearch
        && effect.if_not_cast.from == ProgramZone::Exile
        && effect.if_not_cast.to == ProgramZone::Hand
        && effect.if_not_cast.recipient == ControllerRelation::You
}

fn exact_opponent_choice_search_split(effect: &OpponentChoiceSearchSplitEffect) -> bool {
    exact_atomic_any_card_search(&effect.search, 3, true)
        && effect.chooser == AtomicSearchChooser::TargetOpponent
        && effect.chosen_count == 1
        && effect.chosen_destination == ProgramZone::Hand
        && effect.chosen_recipient == ControllerRelation::You
        && effect.remainder_count == 2
        && effect.remainder_destination == ProgramZone::Graveyard
        && effect.remainder_recipient == ControllerRelation::You
        && effect.shuffle.player == ControllerRelation::You
        && effect.shuffle.timing == AtomicShuffleTiming::AfterOpponentChoiceMovements
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypedFirstUseSelfTransferTutor {
    activation_cost_oracle: String,
}

/// Revalidate the complete first-controller lifecycle before granting runtime
/// authority. A nearby activated search, a hand-authored partial program, or
/// any changed counter/transfer clause must not inherit Wishclaw behavior.
fn compile_typed_first_use_self_transfer_tutor(
    card: &CompiledCard,
) -> Option<TypedFirstUseSelfTransferTutor> {
    let program = &card.ability_program;
    let permanent = program.executable_self_transfer_tutor_permanent()?;
    if program
        .unsupported_self_transfer_tutor_permanent()
        .is_some()
        || program.unsupported_abilities().next().is_some()
        || !program.abilities.is_empty()
        || program.necropotence_lifecycle.is_some()
        || program.entry_linked_permanent.is_some()
        || program.atomic_transaction.is_some()
        || !program.face_programs.is_empty()
        || !card.effects.card_types.is_artifact
        || permanent.entry.counter != CounterKind::Wish
        || permanent.entry.count != 3
        || permanent.activation.source_zone != ProgramZone::Battlefield
        || permanent.activation.window != SelfTransferTutorActivationWindow::DuringYourTurnOnly
    {
        return None;
    }

    let [
        SelfTransferTutorCost::Mana(ProgramManaCost::PrintedSymbols { oracle, profile }),
        SelfTransferTutorCost::TapSelf,
        SelfTransferTutorCost::RemoveCounterFromSelf {
            counter: CounterKind::Wish,
            count: 1,
        },
    ] = permanent.activation.costs.as_slice()
    else {
        return None;
    };
    if profile
        != &(crate::ability_program::ManaCostProfile {
            generic: 1,
            ..crate::ability_program::ManaCostProfile::default()
        })
    {
        return None;
    }
    let [
        SelfTransferTutorResolutionStep::SearchToHand(tutor),
        SelfTransferTutorResolutionStep::OpponentGainsControlOfSource,
    ] = permanent.activation.resolution.as_slice()
    else {
        return None;
    };
    exact_any_card_tutor(tutor, true).then(|| TypedFirstUseSelfTransferTutor {
        activation_cost_oracle: oracle.clone(),
    })
}

fn pay_first_use_self_transfer_tutor_activation(
    pool: &mut TurnManaPool,
    tutor: &TypedFirstUseSelfTransferTutor,
) -> bool {
    let cost = parse_mana_cost(Some(&tutor.activation_cost_oracle));
    activation_cost_is_exactly_modeled(&cost) && pool.pay(Some(&cost), 1, 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TypedNecropotenceLifecycle;

/// Require every mandatory drawback and both movements of the linked hidden
/// object before the simulator may execute any part of this lifecycle.
fn compile_typed_necropotence_lifecycle(card: &CompiledCard) -> Option<TypedNecropotenceLifecycle> {
    let program = &card.ability_program;
    let lifecycle = program.executable_necropotence_lifecycle()?;
    if program.unsupported_necropotence_lifecycle().is_some()
        || program.unsupported_abilities().next().is_some()
        || !program.abilities.is_empty()
        || program.self_transfer_tutor_permanent.is_some()
        || program.entry_linked_permanent.is_some()
        || program.atomic_transaction.is_some()
        || !card.effects.card_types.is_enchantment
        || lifecycle.draw_step.player != ControllerRelation::You
        || lifecycle.draw_step.step != TurnStep::Draw
        || lifecycle.draw_step.procedure != StepProcedure::Skip
        || lifecycle.discarded_card.player != ControllerRelation::You
        || lifecycle.discarded_card.event != NecropotenceDiscardEvent::WheneverYouDiscardOneCard
        || lifecycle.discarded_card.tracked_object
            != DiscardedObjectReference::CardDiscardedByThisTrigger
        || lifecycle.discarded_card.from != ProgramZone::Graveyard
        || lifecycle.discarded_card.destination != ProgramZone::Exile
        || lifecycle.activation.source_zone != ProgramZone::Battlefield
        || lifecycle.activation.window != ActivationWindow::NormalPriority
        || lifecycle.activation.costs != [AbilityCost::PayLife(1)]
    {
        return None;
    }
    let access = &lifecycle.activation.access;
    (access.player == ControllerRelation::You
        && access.count == 1
        && access.from == ProgramZone::Library
        && access.source_position == LibraryPosition::Top
        && access.intermediate == ProgramZone::Exile
        && access.face_down
        && access.tracked_object == DelayedObjectReference::CardMovedByThisEffect
        && access.delayed_event == DelayedEvent::BeginningOfYourNextEndStep
        && access.destination == ProgramZone::Hand)
        .then_some(TypedNecropotenceLifecycle)
}

fn active_necropotence_lifecycle(deck: &CompiledDeck, zones: &KnownLineZoneState) -> bool {
    zones.battlefield.iter().any(|presence| {
        deck.cards
            .get(presence.card_index)
            .and_then(compile_typed_necropotence_lifecycle)
            .is_some()
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct DelayedAccessPublicHandQuality {
    payable_reviewed_routes: u8,
    reviewed_route_potential: i64,
    effective_hand_strength_basis_points: u16,
    payable_protection: u8,
    executable_mana_development: u8,
}

fn exact_card_multiset(card_indices: &[usize]) -> HashMap<usize, u16> {
    let mut counts = HashMap::<usize, u16>::new();
    for card_index in card_indices {
        let count = counts.entry(*card_index).or_default();
        *count = count.saturating_add(1);
    }
    counts
}

fn projected_next_turn_pool(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    mana_sources: &[BattlefieldManaSource],
    turn: u8,
    retained_treasures: u8,
) -> (KnownLineZoneState, TurnManaPool) {
    let mut projected_zones = zones.clone();
    projected_zones.begin_turn();
    let projected_turn = turn.saturating_add(1);
    let mut pool = TurnManaPool::from_battlefield_with_ability_context(
        mana_sources,
        projected_turn,
        deck,
        active_ability_context(deck, &projected_zones),
        &projected_zones,
    );
    pool.add_treasures(retained_treasures);
    (projected_zones, pool)
}

fn independently_payable_protection_count(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    pool: &TurnManaPool,
) -> u8 {
    hand.iter()
        .filter(|card_index| {
            let Some(card) = deck.cards.get(**card_index) else {
                return false;
            };
            if !card.has(role::PROTECTION | role::COUNTERSPELL) {
                return false;
            }
            let mut candidate = pool.clone();
            candidate.pay(
                mana_access.and_then(|access| access.cost(**card_index)),
                card.mana_value.ceil().max(0.0) as u8,
                0,
            )
        })
        .count()
        .min(usize::from(u8::MAX)) as u8
}

fn independently_payable_mana_development_count(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    pool: &TurnManaPool,
) -> u8 {
    hand.iter()
        .filter(|card_index| {
            let Some(card) = deck.cards.get(**card_index) else {
                return false;
            };
            if !card_has_executable_planner_mana_role(card) {
                return false;
            }
            if matches!(
                compile_typed_atomic_transaction(card),
                Some(TypedAtomicTransaction::HandMana { .. })
            ) {
                return true;
            }
            let mut candidate = pool.clone();
            candidate.pay(
                mana_access.and_then(|access| access.cost(**card_index)),
                card.mana_value.ceil().max(0.0) as u8,
                0,
            )
        })
        .count()
        .min(usize::from(u8::MAX)) as u8
}

#[allow(clippy::too_many_arguments)]
fn delayed_access_public_hand_quality(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    actual_remaining_library: &HashMap<usize, u16>,
    zones: &KnownLineZoneState,
    turn: u8,
    projected_pool: &TurnManaPool,
) -> DelayedAccessPublicHandQuality {
    let payable_reviewed_routes = deck
        .known_lines
        .iter()
        .filter(|line| reviewed_empty_library_sequence(line))
        .filter(|line| {
            reviewed_sequence_package_is_jointly_payable(
                line,
                deck,
                hand,
                zones,
                turn,
                projected_pool,
                mana_access,
                0,
            )
        })
        .count()
        .min(usize::from(u8::MAX)) as u8;
    let reviewed_route_potential = planning_reviewed_sequence_potential(
        deck,
        mana_access,
        hand,
        zones,
        turn,
        projected_pool,
        0,
    );
    let best_route_fraction_basis_points = deck
        .known_lines
        .iter()
        .filter(|line| reviewed_line_is_opening_route(line))
        .map(|line| {
            named_line_piece_access_count(line, deck, hand, zones, turn)
                .min(line.cards.len())
                .saturating_mul(10_000)
                / line.cards.len().max(1)
        })
        .max()
        .unwrap_or_default()
        .min(10_000) as u16;
    let executable_access_plans = hand
        .iter()
        .filter(|card_index| {
            deck.cards.get(**card_index).is_some_and(|card| {
                card_has_executable_draw_access(card)
                    || card_has_executable_tutor_access(deck, card, actual_remaining_library)
            })
        })
        .count()
        .min(2);
    let payable_protection =
        independently_payable_protection_count(deck, mana_access, hand, projected_pool);
    let executable_mana_development =
        independently_payable_mana_development_count(deck, mana_access, hand, projected_pool);
    let effective_hand_strength_basis_points = (u32::from(best_route_fraction_basis_points) * 60
        / 100)
        .saturating_add((executable_access_plans as u32).saturating_mul(1_000))
        .saturating_add(u32::from(payable_protection.min(2)).saturating_mul(500))
        .saturating_add(u32::from(executable_mana_development.min(2)).saturating_mul(500))
        .min(10_000) as u16;

    DelayedAccessPublicHandQuality {
        payable_reviewed_routes,
        reviewed_route_potential,
        effective_hand_strength_basis_points,
        payable_protection,
        executable_mana_development,
    }
}

fn hypergeometric_at_least_probability(
    population: usize,
    quality_hits: usize,
    draws: usize,
    required_hits: usize,
) -> f64 {
    if required_hits == 0 {
        return 1.0;
    }
    if population == 0 || quality_hits == 0 || draws == 0 || required_hits > quality_hits.min(draws)
    {
        return 0.0;
    }
    let draws = draws.min(population);
    // Keep exact probabilities for hit counts below the threshold. Probability
    // that crosses the threshold is intentionally omitted, so one minus the
    // remaining mass is P[X >= required_hits].
    let mut below = vec![0.0f64; required_hits];
    below[0] = 1.0;
    for draw in 0..draws {
        let remaining_population = population - draw;
        let mut next = vec![0.0f64; required_hits];
        for (hits_so_far, probability) in below.iter().copied().enumerate() {
            if probability == 0.0 {
                continue;
            }
            let successes_left = quality_hits.saturating_sub(hits_so_far);
            let failures_drawn = draw.saturating_sub(hits_so_far);
            let failures_left = population
                .saturating_sub(quality_hits)
                .saturating_sub(failures_drawn);
            if failures_left > 0 {
                next[hits_so_far] +=
                    probability * failures_left as f64 / remaining_population as f64;
            }
            if successes_left > 0 && hits_so_far + 1 < required_hits {
                next[hits_so_far + 1] +=
                    probability * successes_left as f64 / remaining_population as f64;
            }
        }
        below = next;
    }
    (1.0 - below.into_iter().sum::<f64>()).clamp(0.0, 1.0)
}

#[allow(clippy::too_many_arguments)]
fn aggressive_delayed_access_batch_size(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    unseen_library: &[usize],
    zones: &KnownLineZoneState,
    mana_sources: &[BattlefieldManaSource],
    turn: u8,
    retained_treasures: u8,
    player_life: f32,
) -> usize {
    let safe_life_budget =
        (player_life.floor() - AGGRESSIVE_DELAYED_ACCESS_MINIMUM_LIFE).max(0.0) as usize;
    let life_and_library_cap = safe_life_budget.min(unseen_library.len());
    if life_and_library_cap == 0 {
        return 0;
    }

    // The decision may know the decklist and public zone history, but it must
    // not learn the shuffled top-card order. Canonicalizing the actual unseen
    // multiset before scoring makes that invariant mechanically explicit.
    let mut canonical_unseen = unseen_library.to_vec();
    canonical_unseen.sort_unstable();
    let actual_remaining_library = exact_card_multiset(&canonical_unseen);
    let (projected_zones, projected_pool) =
        projected_next_turn_pool(deck, zones, mana_sources, turn, retained_treasures);
    let projected_turn = turn.saturating_add(1);
    let baseline = delayed_access_public_hand_quality(
        deck,
        mana_access,
        hand,
        &actual_remaining_library,
        &projected_zones,
        projected_turn,
        &projected_pool,
    );
    let protected_complete_route =
        baseline.payable_reviewed_routes > 0 && baseline.payable_protection > 0;
    let mut quality_hits = 0usize;
    for candidate_index in canonical_unseen {
        let mut candidate_hand = hand.to_vec();
        candidate_hand.push(candidate_index);
        let mut candidate_remaining = actual_remaining_library.clone();
        if let Some(count) = candidate_remaining.get_mut(&candidate_index) {
            *count = count.saturating_sub(1);
        }
        let candidate = delayed_access_public_hand_quality(
            deck,
            mana_access,
            &candidate_hand,
            &candidate_remaining,
            &projected_zones,
            projected_turn,
            &projected_pool,
        );
        let improves = if protected_complete_route {
            candidate.payable_reviewed_routes > baseline.payable_reviewed_routes
                || candidate.payable_protection > baseline.payable_protection
        } else {
            (
                candidate.payable_reviewed_routes,
                candidate.reviewed_route_potential,
                candidate.effective_hand_strength_basis_points,
                candidate.payable_protection,
            ) > (
                baseline.payable_reviewed_routes,
                baseline.reviewed_route_potential,
                baseline.effective_hand_strength_basis_points,
                baseline.payable_protection,
            )
        };
        quality_hits = quality_hits.saturating_add(usize::from(improves));
    }

    let refill = MAXIMUM_CLEANUP_HAND_SIZE.saturating_sub(hand.len());
    if quality_hits == 0 {
        return refill.min(life_and_library_cap);
    }
    let required_hits = if baseline.reviewed_route_potential >= 10_000
        || baseline.effective_hand_strength_basis_points >= 5_500
    {
        1
    } else {
        2
    };
    let confidence_draws = (0..=life_and_library_cap).find(|draws| {
        hypergeometric_at_least_probability(
            unseen_library.len(),
            quality_hits,
            *draws,
            required_hits,
        ) >= AGGRESSIVE_DELAYED_ACCESS_TARGET_CONFIDENCE
    });
    confidence_draws
        .unwrap_or(life_and_library_cap)
        .max(refill)
        .min(life_and_library_cap)
}

#[allow(clippy::too_many_arguments)]
fn execute_necropotence_delayed_access_batch(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    zones: &KnownLineZoneState,
    mana_sources: &[BattlefieldManaSource],
    turn: u8,
    retained_treasures: u8,
    player_life: &mut f32,
    pending: &mut Vec<PendingDelayedCardAccess>,
) -> usize {
    if !active_necropotence_lifecycle(deck, zones) {
        return 0;
    }
    let activation_count = aggressive_delayed_access_batch_size(
        deck,
        mana_access,
        hand,
        library_order.get(next_draw_position..).unwrap_or_default(),
        zones,
        mana_sources,
        turn,
        retained_treasures,
        *player_life,
    );
    for _ in 0..activation_count {
        let card_index = library_order.remove(next_draw_position);
        *player_life -= 1.0;
        // This is a distinct, exact physical object in face-down exile. It is
        // deliberately kept outside the observable zone ledger so no planner
        // decision can inspect its identity before the delayed trigger.
        pending.push(PendingDelayedCardAccess {
            card_index,
            due_turn: turn,
        });
    }
    activation_count
}

fn add_fixed_mana(pool: &mut TurnManaPool, output: FixedManaProfile) -> bool {
    let components = [
        (ManaColorMask::WHITE, output.white),
        (ManaColorMask::BLUE, output.blue),
        (ManaColorMask::BLACK, output.black),
        (ManaColorMask::RED, output.red),
        (ManaColorMask::GREEN, output.green),
        (ManaColorMask::COLORLESS, output.colorless),
    ];
    if components
        .iter()
        .any(|(_, amount)| *amount > u16::from(u8::MAX))
    {
        return false;
    }
    for (color, amount) in components {
        pool.add_floating(color, amount as u8);
    }
    true
}

fn atomic_mana_output_for_graveyard_snapshot(
    transaction: &TypedAtomicTransaction,
    graveyard_count: usize,
    known_source_name_matches: usize,
) -> Option<FixedManaProfile> {
    match transaction {
        TypedAtomicTransaction::HandMana { output }
        | TypedAtomicTransaction::SacrificeRitual { output } => Some(*output),
        TypedAtomicTransaction::NameLinkedGraveyardRitual {
            base,
            per_match,
            opponent_matching_card_floor,
        } => {
            let total_matches = known_source_name_matches
                .checked_add(usize::from(*opponent_matching_card_floor))?;
            checked_add_scaled_mana(*base, *per_match, total_matches)
        }
        TypedAtomicTransaction::ThresholdRitual {
            default,
            replacement,
            threshold,
        } => Some(if graveyard_count >= usize::from(*threshold) {
            *replacement
        } else {
            *default
        }),
        TypedAtomicTransaction::SacrificeTutor { .. }
        | TypedAtomicTransaction::SearchRandomDiscardShuffle { .. }
        | TypedAtomicTransaction::TemporaryLandSacrificeManaGrant { .. }
        | TypedAtomicTransaction::BargainSearchCastOrHand { .. }
        | TypedAtomicTransaction::OpponentChoiceSearchSplit => None,
    }
}

fn checked_add_scaled_mana(
    base: FixedManaProfile,
    per_match: FixedManaProfile,
    match_count: usize,
) -> Option<FixedManaProfile> {
    let match_count = u16::try_from(match_count).ok()?;
    let add_component = |base: u16, per_match: u16| {
        per_match
            .checked_mul(match_count)
            .and_then(|scaled| base.checked_add(scaled))
    };
    Some(FixedManaProfile {
        white: add_component(base.white, per_match.white)?,
        blue: add_component(base.blue, per_match.blue)?,
        black: add_component(base.black, per_match.black)?,
        red: add_component(base.red, per_match.red)?,
        green: add_component(base.green, per_match.green)?,
        colorless: add_component(base.colorless, per_match.colorless)?,
    })
}

fn exact_known_graveyard_name_match_count(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    source_card_index: usize,
) -> Option<usize> {
    let source_name = &deck.cards.get(source_card_index)?.normalized_name;
    zones
        .graveyard
        .iter()
        .try_fold(0usize, |matches, card_index| {
            let card = deck.cards.get(*card_index)?;
            matches.checked_add(usize::from(card.normalized_name == *source_name))
        })
}

fn noncommander_creature_sacrifice_position(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
) -> Option<usize> {
    zones.battlefield.iter().position(|presence| {
        deck.cards.get(presence.card_index).is_some_and(|card| {
            !card.is_commander
                && card.effects.card_types.is_creature
                && matches!(
                    modeled_line_card_kind(card),
                    Some(ModeledLineCardKind::Permanent)
                )
        })
    })
}

fn sacrifice_noncommander_creature(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
) -> Option<usize> {
    let position = noncommander_creature_sacrifice_position(deck, zones)?;
    let sequence = zones.battlefield.get(position)?.sequence;
    zones
        .remove_permanent_sequence_with_attached_auras(deck, sequence, true)?
        .first()
        .copied()
}

fn execute_atomic_hand_mana_action(
    deck: &CompiledDeck,
    card_index: usize,
    hand: &mut Vec<usize>,
    zones: &mut KnownLineZoneState,
    mana_pool: &mut TurnManaPool,
) -> bool {
    let Some(card) = deck.cards.get(card_index) else {
        return false;
    };
    let Some(transaction @ TypedAtomicTransaction::HandMana { .. }) =
        compile_typed_atomic_transaction(card)
    else {
        return false;
    };
    let Some(hand_position) = hand.iter().position(|candidate| *candidate == card_index) else {
        return false;
    };
    let Some(output) =
        atomic_mana_output_for_graveyard_snapshot(&transaction, zones.graveyard.len(), 0)
    else {
        return false;
    };

    let mut staged_hand = hand.clone();
    let mut staged_zones = zones.clone();
    let mut staged_pool = mana_pool.clone();
    staged_hand.swap_remove(hand_position);
    staged_zones.exile.push(card_index);
    staged_zones.advance_sequence();
    if !add_fixed_mana(&mut staged_pool, output) {
        return false;
    }
    *hand = staged_hand;
    *zones = staged_zones;
    *mana_pool = staged_pool;
    true
}

fn execute_granted_land_mana_action(
    deck: &CompiledDeck,
    source_card_index: usize,
    source_sequence: u16,
    turn: u8,
    zones: &mut KnownLineZoneState,
    mana_pool: &mut TurnManaPool,
) -> Option<usize> {
    if zones.land_sacrifice_mana_grant_turn != Some(turn) {
        return None;
    }
    let presence = zones
        .battlefield
        .iter()
        .find(|presence| {
            presence.card_index == source_card_index && presence.sequence == source_sequence
        })
        .copied()?;
    let card = deck.cards.get(presence.card_index)?;
    if !card.effects.card_types.is_land {
        return None;
    }

    let mut staged_zones = zones.clone();
    let mut staged_pool = mana_pool.clone();
    let removed = staged_zones.remove_permanent_sequence(deck, source_sequence, true)?;
    if removed != source_card_index {
        return None;
    }
    synchronize_turn_pool_with_battlefield(&mut staged_pool, &staged_zones);
    if !add_fixed_mana(
        &mut staged_pool,
        FixedManaProfile {
            black: 1,
            ..FixedManaProfile::default()
        },
    ) {
        return None;
    }
    *zones = staged_zones;
    *mana_pool = staged_pool;
    Some(removed)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum AtomicTransactionChoice {
    #[default]
    Default,
    Bargain {
        payment: BargainPayment,
        target_card_index: usize,
        cast_without_paying_mana_cost: bool,
    },
    OpponentChoice {
        pile: [usize; 3],
    },
}

fn atomic_transaction_choice_for_action(action: TurnAction) -> AtomicTransactionChoice {
    match action {
        TurnAction::CastBargainTutor {
            bargain,
            target_card_index,
            cast_without_paying_mana_cost,
            ..
        } => AtomicTransactionChoice::Bargain {
            payment: bargain,
            target_card_index,
            cast_without_paying_mana_cost,
        },
        TurnAction::CastOpponentChoiceTutor { pile, .. } => {
            AtomicTransactionChoice::OpponentChoice { pile }
        }
        _ => AtomicTransactionChoice::Default,
    }
}

fn bargain_permanent_is_eligible(card: &CompiledCard) -> bool {
    card.effects.card_types.is_artifact || card.effects.card_types.is_enchantment
}

fn sacrifice_bargain_permanent(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    mana_pool: &mut TurnManaPool,
    card_index: usize,
    sequence: u16,
) -> Option<usize> {
    let presence = zones
        .battlefield
        .iter()
        .find(|presence| presence.card_index == card_index && presence.sequence == sequence)?;
    let card = deck.cards.get(presence.card_index)?;
    if !bargain_permanent_is_eligible(card) {
        return None;
    }
    let removed = zones.remove_permanent_sequence(deck, sequence, true)?;
    if removed != card_index {
        return None;
    }
    let binding = SacrificeOnFirstSpend {
        card_index,
        sequence,
    };
    mana_pool
        .sources
        .retain(|source| source.sacrifice_on_first_spend != Some(binding));
    Some(removed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AtomicSpellCommit {
    sacrificed_card: Option<usize>,
    pre_resolution_graveyard_count: usize,
    pre_resolution_source_name_matches: usize,
    choice: AtomicTransactionChoice,
}

#[allow(clippy::too_many_arguments)]
fn commit_atomic_spell_initiation_with_choice(
    transaction: &TypedAtomicTransaction,
    deck: &CompiledDeck,
    card_index: usize,
    hand: &mut Vec<usize>,
    zones: &mut KnownLineZoneState,
    mana_pool: &mut TurnManaPool,
    mana_access: Option<&ManaAccessProfile>,
    additional_generic: u8,
    choice: AtomicTransactionChoice,
) -> Option<AtomicSpellCommit> {
    if transaction.initiation() != AtomicInitiation::CastSpell {
        return None;
    }
    let card = deck.cards.get(card_index)?;
    let hand_position = hand.iter().position(|candidate| *candidate == card_index)?;
    let mut staged_hand = hand.clone();
    let mut staged_zones = zones.clone();
    let mut staged_pool = mana_pool.clone();
    let transaction_choice_is_valid = matches!(
        (transaction, choice),
        (
            TypedAtomicTransaction::BargainSearchCastOrHand { .. },
            AtomicTransactionChoice::Bargain { .. }
        ) | (
            TypedAtomicTransaction::OpponentChoiceSearchSplit,
            AtomicTransactionChoice::OpponentChoice { .. }
        ) | (
            TypedAtomicTransaction::HandMana { .. }
                | TypedAtomicTransaction::SacrificeRitual { .. }
                | TypedAtomicTransaction::NameLinkedGraveyardRitual { .. }
                | TypedAtomicTransaction::SacrificeTutor { .. }
                | TypedAtomicTransaction::ThresholdRitual { .. }
                | TypedAtomicTransaction::SearchRandomDiscardShuffle { .. }
                | TypedAtomicTransaction::TemporaryLandSacrificeManaGrant { .. },
            AtomicTransactionChoice::Default
        )
    );
    if !transaction_choice_is_valid {
        return None;
    }
    if matches!(
        choice,
        AtomicTransactionChoice::Bargain {
            payment: BargainPayment::Treasure,
            ..
        }
    ) && !staged_pool.spend_treasures(1)
    {
        return None;
    }
    if !staged_pool.pay_with_generic_adjustment(
        mana_access.and_then(|access| access.cost(card_index)),
        card.mana_value.ceil().max(0.0) as u8,
        additional_generic,
        generic_spell_cost_reduction(deck, &staged_zones, card_index),
        0,
    ) {
        return None;
    }
    let sacrificed_card = match transaction {
        TypedAtomicTransaction::SacrificeRitual { .. }
        | TypedAtomicTransaction::SacrificeTutor { .. } => {
            Some(sacrifice_noncommander_creature(deck, &mut staged_zones)?)
        }
        TypedAtomicTransaction::BargainSearchCastOrHand { .. } => match choice {
            AtomicTransactionChoice::Bargain {
                payment:
                    BargainPayment::Permanent {
                        card_index,
                        sequence,
                    },
                ..
            } => Some(sacrifice_bargain_permanent(
                deck,
                &mut staged_zones,
                &mut staged_pool,
                card_index,
                sequence,
            )?),
            AtomicTransactionChoice::Bargain {
                payment: BargainPayment::None | BargainPayment::Treasure,
                ..
            } => None,
            _ => return None,
        },
        TypedAtomicTransaction::NameLinkedGraveyardRitual { .. }
        | TypedAtomicTransaction::ThresholdRitual { .. }
        | TypedAtomicTransaction::SearchRandomDiscardShuffle { .. }
        | TypedAtomicTransaction::TemporaryLandSacrificeManaGrant { .. }
        | TypedAtomicTransaction::OpponentChoiceSearchSplit => None,
        TypedAtomicTransaction::HandMana { .. } => return None,
    };
    let pre_resolution_graveyard_count = staged_zones.graveyard.len();
    let pre_resolution_source_name_matches =
        exact_known_graveyard_name_match_count(deck, &staged_zones, card_index)?;
    staged_hand.swap_remove(hand_position);
    *hand = staged_hand;
    *zones = staged_zones;
    *mana_pool = staged_pool;
    Some(AtomicSpellCommit {
        sacrificed_card,
        pre_resolution_graveyard_count,
        pre_resolution_source_name_matches,
        choice,
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AtomicSpellResolution {
    searched_card: Option<usize>,
    discarded_card: Option<usize>,
    free_cast_offer: Option<usize>,
    opponent_chosen_card: Option<usize>,
    opponent_graveyard_cards: [Option<usize>; 2],
}

/// Rank observable library identities without consulting their hidden order,
/// then expand only the physical copies needed by the caller. This is the
/// authoritative fallback when a previously planned search identity has gone
/// stale between planning and resolution.
#[allow(clippy::too_many_arguments)]
fn ranked_actual_any_card_search_identities(
    deck: &CompiledDeck,
    library_order: &[usize],
    next_draw_position: usize,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
    limit: usize,
) -> Vec<usize> {
    let unseen_start = next_draw_position.min(library_order.len());
    let available_library_copies = exact_card_multiset(&library_order[unseen_start..]);
    let mut candidates = available_library_copies
        .iter()
        .filter_map(|(card_index, copies)| {
            let card = deck.cards.get(*card_index)?;
            Some((
                *card_index,
                exact_tutor_reviewed_route_rank_with_library(
                    deck,
                    *card_index,
                    hand,
                    zones,
                    turn,
                    mana_access,
                    mana_pool,
                    future_additional_generic_per_cast,
                    |index| {
                        available_library_copies
                            .get(&index)
                            .copied()
                            .map(usize::from)
                            .unwrap_or_default()
                    },
                ),
                tutor_target_score(deck, card, hand, zones, turn),
                card.normalized_name.clone(),
                *copies,
            ))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.3.cmp(&right.3))
            .then_with(|| left.0.cmp(&right.0))
    });

    let mut ranked = Vec::with_capacity(limit);
    for (card_index, _, _, _, copies) in candidates {
        for _ in 0..usize::from(copies) {
            ranked.push(card_index);
            if ranked.len() == limit {
                return ranked;
            }
        }
    }
    ranked
}

#[allow(clippy::too_many_arguments)]
fn execute_atomic_spell_resolution_with_choice(
    transaction: &TypedAtomicTransaction,
    pre_resolution_graveyard_count: usize,
    pre_resolution_source_name_matches: usize,
    choice: AtomicTransactionChoice,
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    hand: &mut Vec<usize>,
    turn: u8,
    mana_pool: &mut TurnManaPool,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
    future_additional_generic_per_cast: u8,
) -> AtomicSpellResolution {
    let mut resolution = AtomicSpellResolution::default();
    match transaction {
        TypedAtomicTransaction::HandMana { .. } => {}
        TypedAtomicTransaction::SacrificeRitual { output } => {
            let _ = add_fixed_mana(mana_pool, *output);
        }
        TypedAtomicTransaction::NameLinkedGraveyardRitual { .. } => {
            if let Some(output) = atomic_mana_output_for_graveyard_snapshot(
                transaction,
                pre_resolution_graveyard_count,
                pre_resolution_source_name_matches,
            ) {
                let _ = add_fixed_mana(mana_pool, output);
            }
        }
        TypedAtomicTransaction::ThresholdRitual {
            default,
            replacement,
            threshold,
        } => {
            let output = if pre_resolution_graveyard_count >= usize::from(*threshold) {
                *replacement
            } else {
                *default
            };
            let _ = add_fixed_mana(mana_pool, output);
        }
        TypedAtomicTransaction::SacrificeTutor { tutor } => {
            resolution.searched_card = execute_atomic_search_to_hand(
                tutor,
                deck,
                library_order,
                next_draw_position,
                hand,
                turn,
                zones,
                mana_access,
                mana_pool,
                future_additional_generic_per_cast,
            );
            if tutor.shuffle_after {
                shuffle_unseen_library(library_order, next_draw_position, rng);
            }
        }
        TypedAtomicTransaction::SearchRandomDiscardShuffle { tutor } => {
            resolution.searched_card = execute_uniform_random_discard_search_to_hand(
                tutor,
                deck,
                library_order,
                next_draw_position,
                hand,
                turn,
                zones,
                mana_access,
                mana_pool,
                future_additional_generic_per_cast,
            );
            if !hand.is_empty() {
                let discarded_position = rng.random_range(0..hand.len());
                let discarded = hand.swap_remove(discarded_position);
                zones.record_discard(deck, discarded);
                resolution.discarded_card = Some(discarded);
            }
            shuffle_unseen_library(library_order, next_draw_position, rng);
        }
        TypedAtomicTransaction::TemporaryLandSacrificeManaGrant { .. } => {
            if matches!(choice, AtomicTransactionChoice::Default) {
                zones.land_sacrifice_mana_grant_turn = Some(turn);
            }
        }
        TypedAtomicTransaction::BargainSearchCastOrHand { maximum_mana_value } => {
            if let AtomicTransactionChoice::Bargain {
                payment,
                target_card_index,
                cast_without_paying_mana_cost,
            } = choice
            {
                let requested_position = library_order
                    .iter()
                    .copied()
                    .enumerate()
                    .skip(next_draw_position)
                    .find_map(|(position, candidate)| {
                        (candidate == target_card_index).then_some(position)
                    });
                let target_position = requested_position.or_else(|| {
                    let fallback = ranked_actual_any_card_search_identities(
                        deck,
                        library_order,
                        next_draw_position,
                        hand,
                        zones,
                        turn,
                        mana_access,
                        mana_pool,
                        future_additional_generic_per_cast,
                        1,
                    )
                    .into_iter()
                    .next()?;
                    library_order
                        .iter()
                        .copied()
                        .enumerate()
                        .skip(next_draw_position)
                        .find_map(|(position, candidate)| {
                            (candidate == fallback).then_some(position)
                        })
                });
                if let Some(target_position) = target_position {
                    let target = library_order.remove(target_position);
                    resolution.searched_card = Some(target);
                    shuffle_unseen_library(library_order, next_draw_position, rng);
                    let bargained = payment != BargainPayment::None;
                    let may_offer_cast = bargained
                        && cast_without_paying_mana_cost
                        && deck.cards.get(target).is_some_and(|card| {
                            card.mana_value.ceil().max(0.0) as u16 <= *maximum_mana_value
                        });
                    if may_offer_cast {
                        resolution.free_cast_offer = Some(target);
                    } else {
                        hand.push(target);
                    }
                } else {
                    shuffle_unseen_library(library_order, next_draw_position, rng);
                }
            }
        }
        TypedAtomicTransaction::OpponentChoiceSearchSplit => {
            if let AtomicTransactionChoice::OpponentChoice { pile } = choice {
                let mut staged_library = library_order.clone();
                let mut found = Vec::with_capacity(3);
                for target in pile {
                    let Some(position) = staged_library
                        .iter()
                        .copied()
                        .enumerate()
                        .skip(next_draw_position)
                        .find_map(|(position, candidate)| {
                            (candidate == target).then_some(position)
                        })
                    else {
                        found.clear();
                        break;
                    };
                    found.push(staged_library.remove(position));
                }
                if found.len() != 3 {
                    staged_library = library_order.clone();
                    found.clear();
                    for target in ranked_actual_any_card_search_identities(
                        deck,
                        library_order,
                        next_draw_position,
                        hand,
                        zones,
                        turn,
                        mana_access,
                        mana_pool,
                        future_additional_generic_per_cast,
                        3,
                    ) {
                        let Some(position) = staged_library
                            .iter()
                            .copied()
                            .enumerate()
                            .skip(next_draw_position)
                            .find_map(|(position, candidate)| {
                                (candidate == target).then_some(position)
                            })
                        else {
                            found.clear();
                            break;
                        };
                        found.push(staged_library.remove(position));
                    }
                }
                if !found.is_empty() {
                    let mut worst: Option<(
                        PlannerValue,
                        String,
                        usize,
                        usize,
                        Vec<usize>,
                        KnownLineZoneState,
                    )> = None;
                    for chosen_position in 0..found.len() {
                        let mut outcome_hand = hand.clone();
                        let chosen = found[chosen_position];
                        outcome_hand.push(chosen);
                        let mut outcome_zones = zones.clone();
                        for (position, card_index) in found.iter().copied().enumerate() {
                            if position != chosen_position {
                                outcome_zones.graveyard.push(card_index);
                                outcome_zones.advance_sequence();
                            }
                        }
                        let value = planner_value_with_development(
                            deck,
                            mana_access,
                            &outcome_hand,
                            &outcome_zones,
                            turn,
                            mana_pool,
                            future_additional_generic_per_cast,
                            &[],
                            0,
                        )
                        .0;
                        let name = deck
                            .cards
                            .get(chosen)
                            .map_or_else(String::new, |card| card.normalized_name.clone());
                        let candidate = (
                            value,
                            name,
                            chosen,
                            chosen_position,
                            outcome_hand,
                            outcome_zones,
                        );
                        if worst.as_ref().is_none_or(|current| {
                            (candidate.0, &candidate.1, candidate.2)
                                < (current.0, &current.1, current.2)
                        }) {
                            worst = Some(candidate);
                        }
                    }
                    if let Some((_, _, chosen, chosen_position, outcome_hand, outcome_zones)) =
                        worst
                    {
                        *library_order = staged_library;
                        *hand = outcome_hand;
                        *zones = outcome_zones;
                        resolution.searched_card = Some(chosen);
                        resolution.opponent_chosen_card = Some(chosen);
                        let mut graveyard_cards = found
                            .into_iter()
                            .enumerate()
                            .filter(|(position, _)| *position != chosen_position)
                            .map(|(_, card_index)| card_index)
                            .map(Some)
                            .collect::<Vec<_>>();
                        while graveyard_cards.len() < 2 {
                            graveyard_cards.push(None);
                        }
                        resolution.opponent_graveyard_cards =
                            [graveyard_cards[0], graveyard_cards[1]];
                    }
                }
                shuffle_unseen_library(library_order, next_draw_position, rng);
            }
        }
    }
    resolution
}

#[allow(clippy::too_many_arguments)]
fn execute_atomic_search_to_hand(
    tutor: &ProgramTutorEffect,
    deck: &CompiledDeck,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    hand: &mut Vec<usize>,
    turn: u8,
    zones: &KnownLineZoneState,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
) -> Option<usize> {
    if tutor.from != ProgramZone::Library || tutor.destination != ProgramZone::Hand {
        return None;
    }
    let unseen_start = next_draw_position.min(library_order.len());
    let available_library_copies = exact_card_multiset(&library_order[unseen_start..]);
    let target_position = library_order
        .iter()
        .copied()
        .enumerate()
        .skip(next_draw_position)
        .filter_map(|(position, card_index)| {
            let card = deck.cards.get(card_index)?;
            if !program_tutor_matches(&tutor.filter, card) {
                return None;
            }
            Some((
                position,
                card_index,
                exact_tutor_reviewed_route_rank_with_library(
                    deck,
                    card_index,
                    hand,
                    zones,
                    turn,
                    mana_access,
                    mana_pool,
                    future_additional_generic_per_cast,
                    |index| {
                        available_library_copies
                            .get(&index)
                            .copied()
                            .map(usize::from)
                            .unwrap_or_default()
                    },
                ),
                tutor_target_score(deck, card, hand, zones, turn),
                card.normalized_name.as_str(),
            ))
        })
        .max_by(|left, right| {
            left.2
                .cmp(&right.2)
                .then_with(|| left.3.cmp(&right.3))
                .then_with(|| right.4.cmp(left.4))
                .then_with(|| right.1.cmp(&left.1))
        })
        .map(|(position, _, _, _, _)| position)?;
    let target_index = library_order.remove(target_position);
    hand.push(target_index);
    Some(target_index)
}

#[allow(clippy::too_many_arguments)]
fn execute_uniform_random_discard_search_to_hand(
    tutor: &ProgramTutorEffect,
    deck: &CompiledDeck,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    hand: &mut Vec<usize>,
    turn: u8,
    zones: &KnownLineZoneState,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
) -> Option<usize> {
    let unseen_start = next_draw_position.min(library_order.len());
    let available_library_copies = exact_card_multiset(&library_order[unseen_start..]);
    let choice = best_uniform_random_discard_tutor_target(
        tutor,
        deck,
        hand,
        zones,
        turn,
        mana_access,
        mana_pool,
        future_additional_generic_per_cast,
        &[],
        0,
        |card_index| {
            available_library_copies
                .get(&card_index)
                .copied()
                .map(usize::from)
                .unwrap_or_default()
        },
    )?;
    let target_position = library_order
        .iter()
        .copied()
        .enumerate()
        .skip(next_draw_position)
        .find_map(|(position, card_index)| {
            (card_index == choice.target_index).then_some(position)
        })?;
    let target_index = library_order.remove(target_position);
    hand.push(target_index);
    Some(target_index)
}

fn shuffle_unseen_library(
    library_order: &mut [usize],
    next_draw_position: usize,
    rng: &mut ChaCha8Rng,
) {
    let unseen_start = next_draw_position.min(library_order.len());
    library_order[unseen_start..].shuffle(rng);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FiveColorManaChoice {
    White,
    Blue,
    Black,
    Red,
    Green,
}

impl FiveColorManaChoice {
    const ALL: [Self; 5] = [Self::White, Self::Blue, Self::Black, Self::Red, Self::Green];

    fn mask(self) -> ManaColorMask {
        match self {
            Self::White => ManaColorMask::WHITE,
            Self::Blue => ManaColorMask::BLUE,
            Self::Black => ManaColorMask::BLACK,
            Self::Red => ManaColorMask::RED,
            Self::Green => ManaColorMask::GREEN,
        }
    }

    fn fingerprint_value(self) -> u64 {
        match self {
            Self::White => 0,
            Self::Blue => 1,
            Self::Black => 2,
            Self::Red => 3,
            Self::Green => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TurnAction {
    Cast(usize),
    CastAlternativeCost(usize),
    CastBargainTutor {
        source_card_index: usize,
        bargain: BargainPayment,
        target_card_index: usize,
        cast_without_paying_mana_cost: bool,
    },
    CastOpponentChoiceTutor {
        source_card_index: usize,
        pile: [usize; 3],
    },
    CastReviewedRandomDiscardWithManaResponse {
        source_card_index: usize,
        mana_source_card_index: usize,
        mana_source_sequence: u16,
        tutor_source_card_index: usize,
        tutor_source_sequence: u16,
        permission_source_card_index: usize,
        mill_target_card_index: usize,
        color: FiveColorManaChoice,
    },
    ActivateHandMana(usize),
    ActivateGrantedLandMana {
        source_card_index: usize,
        source_sequence: u16,
    },
    ActivateDiscardHandSacrificeMana {
        source_card_index: usize,
        source_sequence: u16,
        color: FiveColorManaChoice,
    },
    ActivateFirstUseSelfTransferTutor {
        source_card_index: usize,
        source_sequence: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BargainPayment {
    None,
    Permanent { card_index: usize, sequence: u16 },
    Treasure,
}

impl TurnAction {
    fn card_index(self) -> usize {
        match self {
            Self::Cast(card_index)
            | Self::CastAlternativeCost(card_index)
            | Self::ActivateHandMana(card_index) => card_index,
            Self::CastBargainTutor {
                source_card_index, ..
            }
            | Self::CastOpponentChoiceTutor {
                source_card_index, ..
            }
            | Self::CastReviewedRandomDiscardWithManaResponse {
                source_card_index, ..
            }
            | Self::ActivateGrantedLandMana {
                source_card_index, ..
            }
            | Self::ActivateDiscardHandSacrificeMana {
                source_card_index, ..
            }
            | Self::ActivateFirstUseSelfTransferTutor {
                source_card_index, ..
            } => source_card_index,
        }
    }

    fn spell_payment_choice(self) -> Option<SpellPaymentChoice> {
        match self {
            Self::Cast(_) => Some(SpellPaymentChoice::Printed),
            Self::CastAlternativeCost(_) => Some(SpellPaymentChoice::Alternative),
            _ => None,
        }
    }
}

fn action_is_spell_cast(action: TurnAction) -> bool {
    matches!(
        action,
        TurnAction::Cast(_)
            | TurnAction::CastAlternativeCost(_)
            | TurnAction::CastBargainTutor { .. }
            | TurnAction::CastOpponentChoiceTutor { .. }
            | TurnAction::CastReviewedRandomDiscardWithManaResponse { .. }
    )
}

fn nested_free_cast_is_supported(card: &CompiledCard) -> bool {
    let inert_oracle_root = match card.ability_program.abilities.as_slice() {
        [] => true,
        [AbilityCompilation::Unsupported(ability)] => {
            ability.normalized_oracle.is_empty()
                && ability
                    .reasons
                    .iter()
                    .all(|reason| reason.code == UnsupportedReasonCode::EmptyClause)
        }
        _ => false,
    };
    !card.has(role::LAND)
        && modeled_line_card_kind(card).is_some()
        // The bounded nested executor below can safely cast only cards whose
        // complete rules text is inert at this boundary. Any typed or
        // unsupported sibling could carry mandatory costs, targets, or an
        // ordered resolution procedure that this free-cast path cannot honor.
        && inert_oracle_root
        && card.ability_program.atomic_transaction.is_none()
        && card.ability_program.entry_linked_permanent.is_none()
        && card.ability_program.self_transfer_tutor_permanent.is_none()
        && card.ability_program.necropotence_lifecycle.is_none()
        && card.ability_program.face_programs.is_empty()
        && card.effects.unsupported_clauses.is_empty()
        && compile_typed_burst_card_access_program(card).is_none()
        && !card.effects.tutor.is_executable_on_spell_resolution()
        && immediate_cards_drawn(card) == 0
}

fn planning_spell_printed_cost_is_payable(
    domain: &CastPlanningDomain<'_>,
    state: &CastPlanningState,
    card_index: usize,
    card: &CompiledCard,
) -> bool {
    let mut candidate_pool = state.mana_pool.clone();
    let mut candidate_zones = state.zones.clone();
    pay_spell_printed_or_alternative_cost(
        domain.deck,
        &mut candidate_zones,
        &mut candidate_pool,
        card_index,
        domain
            .mana_access
            .and_then(|access| access.cost(card_index)),
        card.mana_value.ceil().max(0.0) as u8,
        domain.additional_generic_per_cast,
        0,
        domain.turn,
    )
}

fn ordinary_planning_cast_actions(
    domain: &CastPlanningDomain<'_>,
    card_index: usize,
    card: &CompiledCard,
) -> Vec<TurnAction> {
    if exact_self_alternative_spell_cost(card).is_none() {
        return vec![TurnAction::Cast(card_index)];
    }
    let mut actions = Vec::with_capacity(2);
    if domain
        .mana_access
        .and_then(|access| access.cost(card_index))
        .is_some_and(|cost| {
            cost.confidence >= 0.999 && cost.faces.len() == 1 && cost.faces[0].confidence >= 0.999
        })
    {
        actions.push(TurnAction::Cast(card_index));
    }
    actions.push(TurnAction::CastAlternativeCost(card_index));
    actions
}

fn ranked_planning_any_card_targets(
    domain: &CastPlanningDomain<'_>,
    state: &CastPlanningState,
    limit: usize,
) -> Vec<usize> {
    if limit == 0 {
        return Vec::new();
    }
    let mut candidates = domain
        .deck
        .cards
        .iter()
        .enumerate()
        .filter(|(card_index, _)| planner_library_has_copy(domain.deck, state, *card_index))
        .map(|(card_index, card)| {
            (
                card_index,
                exact_tutor_reviewed_route_rank_with_library(
                    domain.deck,
                    card_index,
                    &state.hand,
                    &state.zones,
                    domain.turn,
                    domain.mana_access,
                    &state.mana_pool,
                    domain.additional_generic_per_cast,
                    |index| planner_library_copy_count(domain.deck, state, index),
                ),
                tutor_target_score(domain.deck, card, &state.hand, &state.zones, domain.turn),
                card.normalized_name.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let compare = |left: &(usize, ExactTutorReviewedRouteRank, i32, &str),
                   right: &(usize, ExactTutorReviewedRouteRank, i32, &str)| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.3.cmp(right.3))
            .then_with(|| left.0.cmp(&right.0))
    };
    if candidates.len() > limit {
        candidates.select_nth_unstable_by(limit, compare);
        candidates.truncate(limit);
    }
    candidates.sort_by(compare);
    candidates
        .into_iter()
        .map(|(card_index, _, _, _)| card_index)
        .collect()
}

fn planning_bargain_tutor_actions(
    domain: &CastPlanningDomain<'_>,
    state: &CastPlanningState,
    source_card_index: usize,
    maximum_mana_value: u16,
) -> Vec<TurnAction> {
    let targets = ranked_planning_any_card_targets(domain, state, 3);
    let mut payments = vec![BargainPayment::None];
    payments.extend(state.zones.battlefield.iter().filter_map(|presence| {
        domain
            .deck
            .cards
            .get(presence.card_index)
            .filter(|card| bargain_permanent_is_eligible(card))
            .map(|_| BargainPayment::Permanent {
                card_index: presence.card_index,
                sequence: presence.sequence,
            })
    }));
    if state.mana_pool.remaining_treasures() > 0 {
        payments.push(BargainPayment::Treasure);
    }
    payments.sort_unstable();
    payments.dedup();

    let mut actions = Vec::new();
    for target_card_index in targets {
        for payment in &payments {
            let bargained = *payment != BargainPayment::None;
            let can_request_nested_cast = bargained
                && domain
                    .deck
                    .cards
                    .get(target_card_index)
                    .is_some_and(|card| {
                        card.mana_value.ceil().max(0.0) as u16 <= maximum_mana_value
                            && nested_free_cast_is_supported(card)
                    });
            if !domain.rule_of_law_cap_active() || !can_request_nested_cast {
                actions.push(TurnAction::CastBargainTutor {
                    source_card_index,
                    bargain: *payment,
                    target_card_index,
                    cast_without_paying_mana_cost: false,
                });
            }
            if can_request_nested_cast {
                actions.push(TurnAction::CastBargainTutor {
                    source_card_index,
                    bargain: *payment,
                    target_card_index,
                    cast_without_paying_mana_cost: true,
                });
            }
        }
    }
    actions
}

fn planning_opponent_choice_tutor_actions(
    domain: &CastPlanningDomain<'_>,
    state: &CastPlanningState,
    source_card_index: usize,
) -> Vec<TurnAction> {
    let mut physical_candidates = Vec::new();
    for card_index in ranked_planning_any_card_targets(domain, state, 7) {
        for _ in 0..planner_library_copy_count(domain.deck, state, card_index).min(3) {
            physical_candidates.push(card_index);
            if physical_candidates.len() == 7 {
                break;
            }
        }
        if physical_candidates.len() == 7 {
            break;
        }
    }
    let mut actions = Vec::new();
    for first in 0..physical_candidates.len() {
        for second in first + 1..physical_candidates.len() {
            for third in second + 1..physical_candidates.len() {
                let mut pile = [
                    physical_candidates[first],
                    physical_candidates[second],
                    physical_candidates[third],
                ];
                pile.sort_unstable();
                actions.push(TurnAction::CastOpponentChoiceTutor {
                    source_card_index,
                    pile,
                });
            }
        }
    }
    actions.sort_unstable();
    actions.dedup();
    actions
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct StochasticPlannerExpectation {
    value_sums: PlannerValue,
    outcome_count: u16,
}

impl StochasticPlannerExpectation {
    fn conservative_mean(self) -> PlannerValue {
        if self.outcome_count == 0 {
            return PlannerValue::default();
        }
        let divisor = i64::from(self.outcome_count);
        PlannerValue {
            executable_table_conversion: self
                .value_sums
                .executable_table_conversion
                .div_euclid(divisor),
            credible_executable_threat: self
                .value_sums
                .credible_executable_threat
                .div_euclid(divisor),
            route_deficit_reduction: self.value_sums.route_deficit_reduction.div_euclid(divisor),
            card_mana_development: self.value_sums.card_mana_development.div_euclid(divisor),
            protection_preservation: self.value_sums.protection_preservation.div_euclid(divisor),
            scarce_resource_preservation: self
                .value_sums
                .scarce_resource_preservation
                .div_euclid(divisor),
        }
    }

    fn checked_add_outcome(&mut self, value: PlannerValue) -> Option<()> {
        self.value_sums = PlannerValue {
            executable_table_conversion: self
                .value_sums
                .executable_table_conversion
                .checked_add(value.executable_table_conversion)?,
            credible_executable_threat: self
                .value_sums
                .credible_executable_threat
                .checked_add(value.credible_executable_threat)?,
            route_deficit_reduction: self
                .value_sums
                .route_deficit_reduction
                .checked_add(value.route_deficit_reduction)?,
            card_mana_development: self
                .value_sums
                .card_mana_development
                .checked_add(value.card_mana_development)?,
            protection_preservation: self
                .value_sums
                .protection_preservation
                .checked_add(value.protection_preservation)?,
            scarce_resource_preservation: self
                .value_sums
                .scarce_resource_preservation
                .checked_add(value.scarce_resource_preservation)?,
        };
        self.outcome_count = self.outcome_count.checked_add(1)?;
        Some(())
    }
}

fn compare_stochastic_planner_expectations(
    left: StochasticPlannerExpectation,
    right: StochasticPlannerExpectation,
) -> std::cmp::Ordering {
    fn compare_field(
        left_value: i64,
        left_count: u16,
        right_value: i64,
        right_count: u16,
    ) -> std::cmp::Ordering {
        let left_scaled = i128::from(left_value) * i128::from(right_count);
        let right_scaled = i128::from(right_value) * i128::from(left_count);
        left_scaled.cmp(&right_scaled)
    }

    compare_field(
        left.value_sums.executable_table_conversion,
        left.outcome_count,
        right.value_sums.executable_table_conversion,
        right.outcome_count,
    )
    .then_with(|| {
        compare_field(
            left.value_sums.credible_executable_threat,
            left.outcome_count,
            right.value_sums.credible_executable_threat,
            right.outcome_count,
        )
    })
    .then_with(|| {
        compare_field(
            left.value_sums.route_deficit_reduction,
            left.outcome_count,
            right.value_sums.route_deficit_reduction,
            right.outcome_count,
        )
    })
    .then_with(|| {
        compare_field(
            left.value_sums.card_mana_development,
            left.outcome_count,
            right.value_sums.card_mana_development,
            right.outcome_count,
        )
    })
    .then_with(|| {
        compare_field(
            left.value_sums.protection_preservation,
            left.outcome_count,
            right.value_sums.protection_preservation,
            right.outcome_count,
        )
    })
    .then_with(|| {
        compare_field(
            left.value_sums.scarce_resource_preservation,
            left.outcome_count,
            right.value_sums.scarce_resource_preservation,
            right.outcome_count,
        )
    })
}

fn stochastic_expectation_from_outcomes(
    outcomes: impl IntoIterator<Item = PlannerValue>,
) -> Option<StochasticPlannerExpectation> {
    let mut expectation = StochasticPlannerExpectation {
        value_sums: PlannerValue::default(),
        outcome_count: 0,
    };
    for value in outcomes {
        expectation.checked_add_outcome(value)?;
    }
    if expectation.outcome_count == 0 {
        None
    } else {
        Some(expectation)
    }
}

#[allow(clippy::too_many_arguments)]
fn planner_value_with_development(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: &TurnManaPool,
    additional_generic_per_cast: u8,
    planned_casts: &[usize],
    development: i64,
) -> (PlannerValue, bool) {
    let (best_progress, completed_threat, completed_conversion) =
        planning_route_value(deck, zones, turn, planned_casts);
    let reviewed_route_potential = planning_reviewed_sequence_potential(
        deck,
        mana_access,
        hand,
        zones,
        turn,
        mana_pool,
        additional_generic_per_cast,
    );
    let protected_cards = hand
        .iter()
        .filter(|card_index| {
            deck.cards
                .get(**card_index)
                .is_some_and(|card| card.has(role::PROTECTION | role::COUNTERSPELL))
        })
        .count() as i64;
    let preserved_consumable_mana = hand
        .iter()
        .filter(|card_index| {
            deck.cards
                .get(**card_index)
                .is_some_and(card_is_current_turn_consumable_mana)
        })
        .count() as i64;
    (
        PlannerValue {
            executable_table_conversion: i64::from(completed_conversion),
            credible_executable_threat: i64::from(completed_threat),
            route_deficit_reduction: best_progress.max(reviewed_route_potential),
            card_mana_development: development,
            protection_preservation: protected_cards,
            scarce_resource_preservation: i64::from(mana_pool.remaining_treasures())
                + preserved_consumable_mana,
        },
        completed_conversion,
    )
}

fn planning_discard_source_color_alignment(
    domain: &CastPlanningDomain<'_>,
    state: &CastPlanningState,
) -> i64 {
    let Some(mana_access) = domain.mana_access else {
        return 0;
    };
    for line in &domain.deck.known_lines {
        let Some(access) = graveyard_storm_planning_access(
            line,
            domain.deck,
            &state.hand,
            &state.zones,
            |card_index| planner_library_copy_count(domain.deck, state, card_index),
        ) else {
            continue;
        };
        if !access.supported[1] || !access.supported[2] {
            continue;
        }
        if battlefield_contains(&state.zones, access.program.permission_source) {
            return 1;
        }

        let mut candidate_pool = state.mana_pool.clone();
        if !state.hand.contains(&access.program.permission_source) {
            let Some((_, _, tutor)) =
                active_first_use_self_transfer_tutor(domain.deck, &state.zones)
            else {
                continue;
            };
            if planner_library_copy_count(domain.deck, state, access.program.permission_source) == 0
                || !pay_first_use_self_transfer_tutor_activation(&mut candidate_pool, &tutor)
            {
                continue;
            }
        }
        if pay_exact_printed_cost_with_context(
            &mut candidate_pool,
            domain.deck,
            &state.zones,
            mana_access,
            access.program.permission_source,
            domain.additional_generic_per_cast,
        ) {
            return 1;
        }
    }
    0
}

fn planning_action_development(domain: &CastPlanningDomain<'_>, state: &CastPlanningState) -> i64 {
    let ability_context = active_ability_context(domain.deck, &state.zones);
    state
        .planned_actions
        .iter()
        .filter_map(|action| {
            if matches!(action, TurnAction::ActivateDiscardHandSacrificeMana { .. }) {
                // The permanent already received its development credit when
                // cast. Keep only a minimal exact-color tie break when this
                // output can pay the remaining Wishclaw + Breach sequence.
                return Some(planning_discard_source_color_alignment(domain, state));
            }
            if matches!(action, TurnAction::ActivateFirstUseSelfTransferTutor { .. }) {
                // The permanent already received its development credit when
                // it was cast. Its activation is valued through the searched
                // hand's reviewed-route potential.
                return None;
            }
            let card_index = action.card_index();
            if domain
                .deck
                .cards
                .get(card_index)
                .and_then(compile_typed_atomic_transaction)
                .is_some_and(|transaction| {
                    matches!(
                        transaction,
                        TypedAtomicTransaction::SearchRandomDiscardShuffle { .. }
                    )
                })
            {
                // A random-discard search has no persistent development value
                // of its own. Its exact chance-weighted vector is carried by
                // `stochastic_planner_expectation`.
                return None;
            }
            domain.deck.cards.get(card_index).map(|card| {
                planning_card_development_value(
                    card_index,
                    card,
                    domain.policy,
                    domain.mana_access,
                    ability_context,
                )
            })
        })
        .sum()
}

fn deterministic_planner_state_value(
    domain: &CastPlanningDomain<'_>,
    state: &CastPlanningState,
) -> (PlannerValue, bool) {
    let (mut value, completed_conversion) = planner_value_with_development(
        domain.deck,
        domain.mana_access,
        &state.hand,
        &state.zones,
        domain.turn,
        &state.mana_pool,
        domain.additional_generic_per_cast,
        &state.planned_casts,
        planning_action_development(domain, state),
    );
    if state.stochastic_planner_expectation.is_none()
        && let Some(projected_route_value) =
            projected_opponent_end_step_top_tutor_route_value(domain, state)
    {
        // A certain next-turn draw is route access only. It is not a spell in
        // the current hand and therefore cannot create a current endpoint,
        // threat, conversion, or development credit.
        value.route_deficit_reduction = value.route_deficit_reduction.max(projected_route_value);
    }
    (value, completed_conversion)
}

#[derive(Debug, Clone)]
struct CastPlanningState {
    hand: Vec<usize>,
    mana_pool: TurnManaPool,
    zones: KnownLineZoneState,
    player_life: f32,
    spells_cast_this_turn: u8,
    planned_casts: Vec<usize>,
    planned_actions: Vec<TurnAction>,
    /// Expected complete planner value at a hidden-randomness observation
    /// boundary. The speculative hand never contains an assumed surviving
    /// search result; runtime samples the real outcome and replans instead.
    stochastic_planner_expectation: Option<StochasticPlannerExpectation>,
}

/// Compact, collision-safe identity for a speculative cast-planner state.
///
/// The two-lane structural fingerprint makes the normal comparison path much
/// cheaper than formatting a large debug string. The complete canonical state
/// remains part of the key, so even an adversarial 128-bit collision can only
/// make comparison slower; it can never merge distinct game states.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CastPlanningStateKey {
    fingerprint: [u64; 2],
    exact: CanonicalCastPlanningState,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalCastPlanningState {
    hand: Vec<usize>,
    mana_sources: Vec<CanonicalPoolManaSource>,
    pending_triggered_treasures: u8,
    pending_source_damage: u8,
    battlefield: Vec<CanonicalBattlefieldLineCard>,
    creature_tokens: Vec<BattlefieldCreatureToken>,
    attachments: Vec<(u16, BattlefieldAttachment)>,
    chosen_creature_types: Vec<(u16, String)>,
    creature_power_counters: Vec<(u16, u16)>,
    temporary_power_toughness_adjustments: Vec<(u16, (i16, i16))>,
    tapped_creatures_this_turn: Vec<u16>,
    pending_card_draws: u32,
    turn_events: RuntimeTurnEvents,
    spells_cast_this_turn: Vec<CanonicalTurnLineSpell>,
    typed_overrun_casts_this_turn: Vec<usize>,
    graveyard: Vec<usize>,
    exile: Vec<usize>,
    graveyard_cards_at_turn_start: u16,
    next_sequence: u16,
    land_sacrifice_mana_grant_turn: Option<u8>,
    player_life_bits: u32,
    spells_cast_this_turn_count: u8,
    planned_casts: Vec<usize>,
    planned_actions: Vec<TurnAction>,
    stochastic_planner_expectation: Option<StochasticPlannerExpectation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalPoolManaSource {
    origin_card_index: Option<usize>,
    origin_sequence: Option<u16>,
    physically_tapped: bool,
    colors: u8,
    behavior: u16,
    remaining: u8,
    is_treasure: bool,
    treasure_on_first_spend: u8,
    first_spend_recorded: bool,
    base_capacity: u8,
    is_land: bool,
    activation_used: bool,
    source_damage_on_first_spend: u8,
    damage_free_colors_on_first_spend: u8,
    same_type_coupled: bool,
    sacrifice_on_first_spend: Option<SacrificeOnFirstSpend>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalBattlefieldLineCard {
    card_index: usize,
    entered_turn: u8,
    sequence: u16,
    age_counters: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalTurnLineSpell {
    card_index: usize,
    turn: u8,
    sequence: u16,
}

#[derive(Debug, Clone, Copy)]
struct StructuralFingerprint {
    lanes: [u64; 2],
    ordinal: u64,
}

fn splitmix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

impl StructuralFingerprint {
    fn new() -> Self {
        Self {
            lanes: [0x243f_6a88_85a3_08d3, 0x1319_8a2e_0370_7344],
            ordinal: 0,
        }
    }

    fn push(&mut self, tag: u64, value: u64) {
        let position = self.ordinal;
        self.ordinal = self.ordinal.wrapping_add(1);
        let token = splitmix64(
            tag.rotate_left(17)
                ^ value.rotate_left(31)
                ^ position.wrapping_mul(0x9e37_79b9_7f4a_7c15),
        );
        // Zobrist-style token mixing gives cheap incremental structure
        // sensitivity; the second, non-commutative lane protects ordered
        // sequences from simple XOR cancellation.
        self.lanes[0] ^= token;
        self.lanes[1] = self.lanes[1]
            .rotate_left(13)
            .wrapping_add(token ^ 0xa409_3822_299f_31d0);
    }

    fn finish(mut self) -> [u64; 2] {
        self.push(0xffff_ffff_ffff_fffe, self.ordinal);
        [
            splitmix64(self.lanes[0]),
            splitmix64(self.lanes[1] ^ self.lanes[0].rotate_left(29)),
        ]
    }
}

fn canonical_cast_planning_state(state: &CastPlanningState) -> CanonicalCastPlanningState {
    let mut hand = state.hand.clone();
    hand.sort_unstable();
    // Preserve source order. Generic/colored payment and pressure use stable
    // partial ordering, so two otherwise tied sources can still transition
    // differently according to their original position.
    let mana_sources = state
        .mana_pool
        .sources
        .iter()
        .map(|source| CanonicalPoolManaSource {
            origin_card_index: source.origin_card_index,
            origin_sequence: source.origin_sequence,
            physically_tapped: source.physically_tapped,
            colors: mana_color_code(source.colors),
            behavior: mana_behavior_code(source.behavior),
            remaining: source.remaining,
            is_treasure: source.is_treasure,
            treasure_on_first_spend: source.treasure_on_first_spend,
            first_spend_recorded: source.first_spend_recorded,
            base_capacity: source.base_capacity,
            is_land: source.is_land,
            activation_used: source.activation_used,
            source_damage_on_first_spend: source.source_damage_on_first_spend,
            damage_free_colors_on_first_spend: mana_color_code(
                source.damage_free_colors_on_first_spend,
            ),
            same_type_coupled: source.same_type_coupled,
            sacrifice_on_first_spend: source.sacrifice_on_first_spend,
        })
        .collect::<Vec<_>>();
    // Preserve battlefield order for first-match sacrifice/removal policies.
    // Sequence numbers normally agree with insertion order, but the exact
    // collision fallback must remain correct even after sequence saturation.
    let battlefield = state
        .zones
        .battlefield
        .iter()
        .map(|presence| CanonicalBattlefieldLineCard {
            card_index: presence.card_index,
            entered_turn: presence.entered_turn,
            sequence: presence.sequence,
            age_counters: presence.age_counters,
        })
        .collect::<Vec<_>>();
    let spells_cast_this_turn = state
        .zones
        .spells_cast_this_turn
        .iter()
        .map(|cast| CanonicalTurnLineSpell {
            card_index: cast.card_index,
            turn: cast.turn,
            sequence: cast.sequence,
        })
        .collect::<Vec<_>>();
    let typed_overrun_casts_this_turn = state.zones.typed_overrun_casts_this_turn.clone();
    let mut attachments = state
        .zones
        .attachments
        .iter()
        .map(|(source_sequence, attachment)| (*source_sequence, *attachment))
        .collect::<Vec<_>>();
    attachments.sort_unstable();
    let mut chosen_creature_types = state
        .zones
        .chosen_creature_types
        .iter()
        .map(|(source_sequence, creature_type)| (*source_sequence, creature_type.clone()))
        .collect::<Vec<_>>();
    chosen_creature_types.sort_unstable();
    let mut creature_power_counters = state
        .zones
        .creature_power_counters
        .iter()
        .map(|(sequence, counters)| (*sequence, *counters))
        .collect::<Vec<_>>();
    creature_power_counters.sort_unstable();
    let mut temporary_power_toughness_adjustments = state
        .zones
        .temporary_power_toughness_adjustments
        .iter()
        .map(|(sequence, adjustment)| (*sequence, *adjustment))
        .collect::<Vec<_>>();
    temporary_power_toughness_adjustments.sort_unstable();
    let mut graveyard = state.zones.graveyard.clone();
    graveyard.sort_unstable();
    let mut exile = state.zones.exile.clone();
    exile.sort_unstable();
    CanonicalCastPlanningState {
        hand,
        mana_sources,
        pending_triggered_treasures: state.mana_pool.pending_triggered_treasures,
        pending_source_damage: state.mana_pool.pending_source_damage,
        battlefield,
        creature_tokens: state.zones.creature_tokens.clone(),
        attachments,
        chosen_creature_types,
        creature_power_counters,
        temporary_power_toughness_adjustments,
        tapped_creatures_this_turn: state
            .zones
            .tapped_creatures_this_turn
            .iter()
            .copied()
            .collect(),
        pending_card_draws: state.zones.pending_card_draws,
        turn_events: state.zones.turn_events.clone(),
        spells_cast_this_turn,
        typed_overrun_casts_this_turn,
        graveyard,
        exile,
        graveyard_cards_at_turn_start: state.zones.graveyard_cards_at_turn_start,
        next_sequence: state.zones.next_sequence,
        land_sacrifice_mana_grant_turn: state.zones.land_sacrifice_mana_grant_turn,
        player_life_bits: state.player_life.to_bits(),
        spells_cast_this_turn_count: state.spells_cast_this_turn,
        planned_casts: state.planned_casts.clone(),
        planned_actions: state.planned_actions.clone(),
        stochastic_planner_expectation: state.stochastic_planner_expectation,
    }
}

fn cast_planning_state_fingerprint(exact: &CanonicalCastPlanningState) -> [u64; 2] {
    let mut fingerprint = StructuralFingerprint::new();
    fingerprint.push(1, exact.hand.len() as u64);
    for card_index in &exact.hand {
        fingerprint.push(2, *card_index as u64);
    }
    fingerprint.push(3, exact.mana_sources.len() as u64);
    for source in &exact.mana_sources {
        fingerprint.push(4, source.origin_card_index.is_some() as u64);
        fingerprint.push(5, source.origin_card_index.unwrap_or_default() as u64);
        fingerprint.push(129, source.origin_sequence.is_some() as u64);
        fingerprint.push(
            130,
            source
                .origin_sequence
                .map_or(0, |sequence| u64::from(sequence).saturating_add(1)),
        );
        fingerprint.push(131, source.physically_tapped as u64);
        fingerprint.push(6, u64::from(source.colors));
        fingerprint.push(7, u64::from(source.behavior));
        fingerprint.push(8, u64::from(source.remaining));
        fingerprint.push(9, source.is_treasure as u64);
        fingerprint.push(10, u64::from(source.treasure_on_first_spend));
        fingerprint.push(11, source.first_spend_recorded as u64);
        fingerprint.push(12, u64::from(source.base_capacity));
        fingerprint.push(13, source.is_land as u64);
        fingerprint.push(14, source.activation_used as u64);
        fingerprint.push(15, u64::from(source.source_damage_on_first_spend));
        fingerprint.push(49, u64::from(source.damage_free_colors_on_first_spend));
        fingerprint.push(16, source.same_type_coupled as u64);
        fingerprint.push(43, source.sacrifice_on_first_spend.is_some() as u64);
        fingerprint.push(
            44,
            source
                .sacrifice_on_first_spend
                .map_or(0, |binding| binding.card_index as u64),
        );
        fingerprint.push(
            45,
            source
                .sacrifice_on_first_spend
                .map_or(0, |binding| u64::from(binding.sequence)),
        );
    }
    fingerprint.push(17, u64::from(exact.pending_triggered_treasures));
    fingerprint.push(18, u64::from(exact.pending_source_damage));
    fingerprint.push(19, exact.battlefield.len() as u64);
    for presence in &exact.battlefield {
        fingerprint.push(20, presence.card_index as u64);
        fingerprint.push(21, u64::from(presence.entered_turn));
        fingerprint.push(22, u64::from(presence.sequence));
        fingerprint.push(23, u64::from(presence.age_counters));
    }
    fingerprint.push(100, exact.creature_tokens.len() as u64);
    for token in &exact.creature_tokens {
        fingerprint.push(101, u64::from(token.entered_turn));
        fingerprint.push(102, u64::from(token.sequence));
        fingerprint.push(103, u64::from(token.base_power));
        fingerprint.push(104, u64::from(token.base_toughness));
        fingerprint.push(105, u64::from(token.combat_power_counters));
        fingerprint.push(106, token.creature_types.len() as u64);
        for creature_type in &token.creature_types {
            fingerprint.push(107, creature_type.len() as u64);
            for byte in creature_type.bytes() {
                fingerprint.push(108, u64::from(byte));
            }
        }
        fingerprint.push(109, token.printed_keywords.len() as u64);
        for keyword in &token.printed_keywords {
            fingerprint.push(110, *keyword as u64);
        }
    }
    fingerprint.push(111, exact.attachments.len() as u64);
    for (source_sequence, attachment) in &exact.attachments {
        fingerprint.push(112, u64::from(*source_sequence));
        fingerprint.push(113, u64::from(attachment.target_sequence));
        fingerprint.push(114, attachment.kind as u64);
    }
    fingerprint.push(115, exact.chosen_creature_types.len() as u64);
    for (source_sequence, creature_type) in &exact.chosen_creature_types {
        fingerprint.push(116, u64::from(*source_sequence));
        fingerprint.push(117, creature_type.len() as u64);
        for byte in creature_type.bytes() {
            fingerprint.push(118, u64::from(byte));
        }
    }
    fingerprint.push(119, exact.creature_power_counters.len() as u64);
    for (sequence, counters) in &exact.creature_power_counters {
        fingerprint.push(120, u64::from(*sequence));
        fingerprint.push(121, u64::from(*counters));
    }
    fingerprint.push(
        122,
        exact.temporary_power_toughness_adjustments.len() as u64,
    );
    for (sequence, (power, toughness)) in &exact.temporary_power_toughness_adjustments {
        fingerprint.push(123, u64::from(*sequence));
        fingerprint.push(124, i64::from(*power) as u64);
        fingerprint.push(125, i64::from(*toughness) as u64);
    }
    fingerprint.push(58, exact.tapped_creatures_this_turn.len() as u64);
    for sequence in &exact.tapped_creatures_this_turn {
        fingerprint.push(59, u64::from(*sequence));
    }
    fingerprint.push(126, u64::from(exact.pending_card_draws));
    fingerprint.push(127, exact.turn_events.state.turn_sequence());
    fingerprint.push(
        128,
        exact
            .turn_events
            .state
            .monarch()
            .map_or(0, |player| u64::from(player).saturating_add(1)),
    );
    fingerprint.push(24, exact.spells_cast_this_turn.len() as u64);
    for cast in &exact.spells_cast_this_turn {
        fingerprint.push(25, cast.card_index as u64);
        fingerprint.push(26, u64::from(cast.turn));
        fingerprint.push(27, u64::from(cast.sequence));
    }
    fingerprint.push(28, exact.typed_overrun_casts_this_turn.len() as u64);
    for card_index in &exact.typed_overrun_casts_this_turn {
        fingerprint.push(29, *card_index as u64);
    }
    fingerprint.push(30, exact.graveyard.len() as u64);
    for card_index in &exact.graveyard {
        fingerprint.push(31, *card_index as u64);
    }
    fingerprint.push(32, exact.exile.len() as u64);
    for card_index in &exact.exile {
        fingerprint.push(33, *card_index as u64);
    }
    fingerprint.push(34, u64::from(exact.graveyard_cards_at_turn_start));
    fingerprint.push(35, u64::from(exact.next_sequence));
    fingerprint.push(
        57,
        exact
            .land_sacrifice_mana_grant_turn
            .map_or(0, |turn| u64::from(turn).saturating_add(1)),
    );
    fingerprint.push(50, u64::from(exact.player_life_bits));
    fingerprint.push(51, u64::from(exact.spells_cast_this_turn_count));
    fingerprint.push(36, exact.planned_casts.len() as u64);
    for card_index in &exact.planned_casts {
        fingerprint.push(37, *card_index as u64);
    }
    fingerprint.push(38, exact.planned_actions.len() as u64);
    for action in &exact.planned_actions {
        match action {
            TurnAction::Cast(card_index) => fingerprint.push(39, *card_index as u64),
            TurnAction::CastAlternativeCost(card_index) => fingerprint.push(79, *card_index as u64),
            TurnAction::CastBargainTutor {
                source_card_index,
                bargain,
                target_card_index,
                cast_without_paying_mana_cost,
            } => {
                fingerprint.push(58, *source_card_index as u64);
                match bargain {
                    BargainPayment::None => fingerprint.push(59, 0),
                    BargainPayment::Permanent {
                        card_index,
                        sequence,
                    } => {
                        fingerprint.push(59, 1);
                        fingerprint.push(60, *card_index as u64);
                        fingerprint.push(61, u64::from(*sequence));
                    }
                    BargainPayment::Treasure => fingerprint.push(59, 2),
                }
                fingerprint.push(62, *target_card_index as u64);
                fingerprint.push(63, *cast_without_paying_mana_cost as u64);
            }
            TurnAction::CastOpponentChoiceTutor {
                source_card_index,
                pile,
            } => {
                fingerprint.push(64, *source_card_index as u64);
                for card_index in pile {
                    fingerprint.push(65, *card_index as u64);
                }
            }
            TurnAction::CastReviewedRandomDiscardWithManaResponse {
                source_card_index,
                mana_source_card_index,
                mana_source_sequence,
                tutor_source_card_index,
                tutor_source_sequence,
                permission_source_card_index,
                mill_target_card_index,
                color,
            } => {
                fingerprint.push(71, *source_card_index as u64);
                fingerprint.push(72, *mana_source_card_index as u64);
                fingerprint.push(73, u64::from(*mana_source_sequence));
                fingerprint.push(74, *tutor_source_card_index as u64);
                fingerprint.push(75, u64::from(*tutor_source_sequence));
                fingerprint.push(76, *permission_source_card_index as u64);
                fingerprint.push(77, *mill_target_card_index as u64);
                fingerprint.push(78, color.fingerprint_value());
            }
            TurnAction::ActivateHandMana(card_index) => fingerprint.push(40, *card_index as u64),
            TurnAction::ActivateGrantedLandMana {
                source_card_index,
                source_sequence,
            } => {
                fingerprint.push(66, *source_card_index as u64);
                fingerprint.push(67, u64::from(*source_sequence));
            }
            TurnAction::ActivateDiscardHandSacrificeMana {
                source_card_index,
                source_sequence,
                color,
            } => {
                fingerprint.push(68, *source_card_index as u64);
                fingerprint.push(69, u64::from(*source_sequence));
                fingerprint.push(70, color.fingerprint_value());
            }
            TurnAction::ActivateFirstUseSelfTransferTutor {
                source_card_index,
                source_sequence,
            } => {
                fingerprint.push(41, *source_card_index as u64);
                fingerprint.push(42, u64::from(*source_sequence));
            }
        }
    }
    fingerprint.push(46, exact.stochastic_planner_expectation.is_some() as u64);
    fingerprint.push(
        47,
        exact
            .stochastic_planner_expectation
            .map_or(0, |expectation| {
                expectation.value_sums.executable_table_conversion as u64
            }),
    );
    fingerprint.push(
        48,
        exact
            .stochastic_planner_expectation
            .map_or(0, |expectation| u64::from(expectation.outcome_count)),
    );
    fingerprint.push(
        52,
        exact
            .stochastic_planner_expectation
            .map_or(0, |expectation| {
                expectation.value_sums.credible_executable_threat as u64
            }),
    );
    fingerprint.push(
        53,
        exact
            .stochastic_planner_expectation
            .map_or(0, |expectation| {
                expectation.value_sums.route_deficit_reduction as u64
            }),
    );
    fingerprint.push(
        54,
        exact
            .stochastic_planner_expectation
            .map_or(0, |expectation| {
                expectation.value_sums.card_mana_development as u64
            }),
    );
    fingerprint.push(
        55,
        exact
            .stochastic_planner_expectation
            .map_or(0, |expectation| {
                expectation.value_sums.protection_preservation as u64
            }),
    );
    fingerprint.push(
        56,
        exact
            .stochastic_planner_expectation
            .map_or(0, |expectation| {
                expectation.value_sums.scarce_resource_preservation as u64
            }),
    );
    fingerprint.finish()
}

fn cast_planning_state_key(state: &CastPlanningState) -> CastPlanningStateKey {
    let exact = canonical_cast_planning_state(state);
    CastPlanningStateKey {
        fingerprint: cast_planning_state_fingerprint(&exact),
        exact,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CastPlanningError {
    CardDisappeared,
    PaymentChanged,
}

fn reconcile_spent_planning_permanent_mana(
    deck: &CompiledDeck,
    state: &mut CastPlanningState,
) -> Result<(), CastPlanningError> {
    let mut spent_bindings = state
        .mana_pool
        .sources
        .iter()
        .filter(|source| source.first_spend_recorded)
        .filter_map(|source| source.sacrifice_on_first_spend)
        .collect::<Vec<_>>();
    spent_bindings.sort_unstable();
    spent_bindings.dedup();

    for binding in spent_bindings {
        if !state.zones.battlefield.iter().any(|presence| {
            presence.card_index == binding.card_index && presence.sequence == binding.sequence
        }) {
            return Err(CastPlanningError::CardDisappeared);
        }
        let removed = state
            .zones
            .remove_permanent_sequence(deck, binding.sequence, true)
            .ok_or(CastPlanningError::CardDisappeared)?;
        if removed != binding.card_index {
            return Err(CastPlanningError::CardDisappeared);
        }
        // A current plan owns only the casts made since its observable root.
        // The bound one-shot permanent can already be on the battlefield when
        // a replan begins, so its absence from `planned_casts` is not evidence
        // that the physical object disappeared. Remove a same-plan cast when
        // present; the exact battlefield sequence above remains authoritative.
        if let Some(cast_position) = state
            .planned_casts
            .iter()
            .position(|card_index| *card_index == binding.card_index)
        {
            state.planned_casts.remove(cast_position);
        }
        for source in &mut state.mana_pool.sources {
            if source.sacrifice_on_first_spend == Some(binding) {
                // Any remainder is ordinary floating mana after activation;
                // it must never refresh from the sacrificed permanent.
                source.sacrifice_on_first_spend = None;
                source.origin_card_index = None;
                source.origin_sequence = None;
                source.behavior = BattlefieldManaBehavior::Fixed;
                source.base_capacity = 0;
                source.activation_used = true;
            }
        }
    }
    synchronize_turn_pool_with_battlefield(&mut state.mana_pool, &state.zones);
    state.mana_pool.refresh_battlefield_sources(
        deck,
        &state.zones,
        active_ability_context(deck, &state.zones),
    );
    Ok(())
}

fn pay_planning_cost_and_reconcile(
    domain: &CastPlanningDomain<'_>,
    state: &mut CastPlanningState,
    card_index: usize,
    cost: Option<&ManaCostProfile>,
    fallback_mana_value: u8,
    additional_generic: u8,
    reserve: u8,
) -> Result<(), CastPlanningError> {
    if !pay_spell_printed_or_alternative_cost(
        domain.deck,
        &mut state.zones,
        &mut state.mana_pool,
        card_index,
        cost,
        fallback_mana_value,
        additional_generic,
        reserve,
        domain.turn,
    ) {
        return Err(CastPlanningError::PaymentChanged);
    }
    reconcile_spent_planning_permanent_mana(domain.deck, state)
}

#[allow(clippy::too_many_arguments)]
fn pay_planning_cost_choice_and_reconcile(
    domain: &CastPlanningDomain<'_>,
    state: &mut CastPlanningState,
    card_index: usize,
    cost: Option<&ManaCostProfile>,
    fallback_mana_value: u8,
    additional_generic: u8,
    reserve: u8,
    payment_choice: SpellPaymentChoice,
    combat_state: Option<&CommanderCombatState>,
) -> Result<(), CastPlanningError> {
    if !pay_spell_cost_choice(
        domain.deck,
        &mut state.zones,
        &mut state.mana_pool,
        card_index,
        cost,
        fallback_mana_value,
        additional_generic,
        reserve,
        domain.turn,
        payment_choice,
        combat_state,
    ) {
        return Err(CastPlanningError::PaymentChanged);
    }
    reconcile_spent_planning_permanent_mana(domain.deck, state)
}

fn settle_planning_payment(state: &mut CastPlanningState) -> Result<(), CastPlanningError> {
    state
        .mana_pool
        .settle_pending_source_damage(&mut state.player_life)
        .then_some(())
        .ok_or(CastPlanningError::PaymentChanged)
}

fn apply_planning_controller_spell_cast_triggers(
    domain: &CastPlanningDomain<'_>,
    state: &mut CastPlanningState,
    card_index: usize,
) -> Result<(), CastPlanningError> {
    settle_planning_payment(state)?;
    let spell_ordinal = state.spells_cast_this_turn.saturating_add(1);
    if !apply_controller_spell_cast_triggers(
        domain.deck,
        &mut state.zones,
        card_index,
        domain.turn,
        spell_ordinal,
        &mut state.mana_pool,
        &mut state.player_life,
    ) {
        return Err(CastPlanningError::PaymentChanged);
    }
    state.spells_cast_this_turn = spell_ordinal;
    Ok(())
}

#[derive(Debug, Clone)]
struct OpponentEndStepPlanningContext {
    maximum_turn: u8,
    additional_generic_per_cast: u8,
    first_relevant_spell_will_be_countered: bool,
    rule_of_law_cap_active: bool,
    mana_sources: Vec<BattlefieldManaSource>,
}

struct CastPlanningDomain<'a> {
    deck: &'a CompiledDeck,
    mana_access: Option<&'a ManaAccessProfile>,
    zones: &'a KnownLineZoneState,
    turn: u8,
    policy: PilotPolicy,
    additional_generic_per_cast: u8,
    player_life: f32,
    spells_cast_this_turn: u8,
    opponent_end_step: Option<OpponentEndStepPlanningContext>,
}

impl CastPlanningDomain<'_> {
    fn rule_of_law_cap_active(&self) -> bool {
        self.opponent_end_step
            .as_ref()
            .is_some_and(|context| context.rule_of_law_cap_active)
    }

    fn planner_state_evaluation(
        &self,
        state: &CastPlanningState,
    ) -> crate::turn_planner::PlannerStateEvaluation<bool> {
        let (value, completed_conversion) =
            if let Some(expectation) = state.stochastic_planner_expectation {
                // At a hidden-randomness boundary, the physically retained
                // hand is unknown. Use only the complete chance-weighted
                // post-discard vectors; the impossible pre-discard hand must
                // not retain route, protection, or consumable-mana credit.
                (expectation.conservative_mean(), false)
            } else {
                deterministic_planner_state_value(self, state)
            };
        crate::turn_planner::PlannerStateEvaluation {
            endpoint: completed_conversion.then_some(true),
            value,
            dominance: value,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn reviewed_graveyard_storm_primer_window_actions(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    zones: &KnownLineZoneState,
    mana_pool: &TurnManaPool,
    player_life: f32,
    additional_generic_per_cast: u8,
    rule_of_law_cap_active: bool,
    mut available_library_copies: impl FnMut(usize) -> usize,
) -> Vec<TurnAction> {
    if rule_of_law_cap_active || active_necropotence_lifecycle(deck, zones) {
        return Vec::new();
    }
    let Some(mana_access) = mana_access else {
        return Vec::new();
    };
    let Some((tutor_source_sequence, tutor_source_card_index, tutor)) =
        active_first_use_self_transfer_tutor(deck, zones)
    else {
        return Vec::new();
    };
    let mut search_sources = hand
        .iter()
        .copied()
        .filter(|card_index| {
            deck.cards.get(*card_index).is_some_and(|card| {
                matches!(
                    compile_typed_atomic_transaction(card),
                    Some(TypedAtomicTransaction::SearchRandomDiscardShuffle { .. })
                )
            })
        })
        .collect::<Vec<_>>();
    search_sources.sort_unstable();
    search_sources.dedup();
    if search_sources.is_empty() {
        return Vec::new();
    }

    let mut actions = Vec::new();
    for line in &deck.known_lines {
        let Some(program) = compile_graveyard_storm_program(line, deck) else {
            continue;
        };
        if available_library_copies(program.permission_source) == 0
            || available_library_copies(program.mill_spell) == 0
        {
            continue;
        }
        let mana_sources = zones
            .battlefield
            .iter()
            .filter(|presence| presence.card_index == program.mana_source)
            .copied()
            .collect::<Vec<_>>();
        for source_card_index in &search_sources {
            let Some(source_card) = deck.cards.get(*source_card_index) else {
                continue;
            };
            let Some(TypedAtomicTransaction::SearchRandomDiscardShuffle { tutor: search }) =
                compile_typed_atomic_transaction(source_card)
            else {
                continue;
            };
            let Some(search_cost) = mana_access.cost(*source_card_index) else {
                continue;
            };
            if !activation_cost_is_exactly_modeled(search_cost)
                || !deck
                    .cards
                    .get(program.mill_spell)
                    .is_some_and(|target| program_tutor_matches(&search.filter, target))
            {
                continue;
            }
            for mana_source in &mana_sources {
                for color in FiveColorManaChoice::ALL {
                    let mut candidate_hand = hand.to_vec();
                    let Some(search_position) = candidate_hand
                        .iter()
                        .position(|candidate| candidate == source_card_index)
                    else {
                        continue;
                    };
                    let mut candidate_zones = zones.clone();
                    let mut candidate_pool = mana_pool.clone();
                    let mut candidate_life = player_life;
                    if !candidate_pool.pay_with_generic_adjustment(
                        Some(search_cost),
                        0,
                        additional_generic_per_cast,
                        generic_spell_cost_reduction(deck, &candidate_zones, *source_card_index),
                        0,
                    ) || !candidate_pool.settle_pending_source_damage(&mut candidate_life)
                    {
                        continue;
                    }
                    candidate_hand.swap_remove(search_position);
                    if execute_discard_hand_sacrifice_mana_action(
                        deck,
                        program.mana_source,
                        mana_source.sequence,
                        color,
                        &mut candidate_hand,
                        &mut candidate_zones,
                        &mut candidate_pool,
                    )
                    .is_none()
                        || !candidate_hand.is_empty()
                        || !pay_first_use_self_transfer_tutor_activation(
                            &mut candidate_pool,
                            &tutor,
                        )
                    {
                        continue;
                    }
                    let removed_tutor = candidate_zones.remove_permanent_sequence(
                        deck,
                        tutor_source_sequence,
                        false,
                    );
                    if removed_tutor != Some(tutor_source_card_index) {
                        continue;
                    }
                    synchronize_turn_pool_with_battlefield(&mut candidate_pool, &candidate_zones);
                    if !pay_exact_printed_cost_with_context(
                        &mut candidate_pool,
                        deck,
                        &candidate_zones,
                        mana_access,
                        program.permission_source,
                        additional_generic_per_cast,
                    ) || !candidate_pool.settle_pending_source_damage(&mut candidate_life)
                    {
                        continue;
                    }
                    actions.push(TurnAction::CastReviewedRandomDiscardWithManaResponse {
                        source_card_index: *source_card_index,
                        mana_source_card_index: program.mana_source,
                        mana_source_sequence: mana_source.sequence,
                        tutor_source_card_index,
                        tutor_source_sequence,
                        permission_source_card_index: program.permission_source,
                        mill_target_card_index: program.mill_spell,
                        color,
                    });
                }
            }
        }
    }
    actions.sort_unstable();
    actions.dedup();
    actions
}

fn reviewed_primer_window_actions_for_planning(
    domain: &CastPlanningDomain<'_>,
    state: &CastPlanningState,
) -> Vec<TurnAction> {
    reviewed_graveyard_storm_primer_window_actions(
        domain.deck,
        domain.mana_access,
        &state.hand,
        &state.zones,
        &state.mana_pool,
        state.player_life,
        domain.additional_generic_per_cast,
        domain.rule_of_law_cap_active(),
        |card_index| planner_library_copy_count(domain.deck, state, card_index),
    )
}

fn reviewed_primer_window_actions_after_mana_source_cast(
    domain: &CastPlanningDomain<'_>,
    state: &CastPlanningState,
    card_index: usize,
) -> Vec<TurnAction> {
    if domain
        .deck
        .cards
        .get(card_index)
        .and_then(exact_discard_sacrifice_mana_ability)
        != Some(3)
    {
        return Vec::new();
    }
    let mut staged = state.clone();
    if apply_planning_action(domain, &mut staged, TurnAction::Cast(card_index)).is_err() {
        return Vec::new();
    }
    reviewed_primer_window_actions_for_planning(domain, &staged)
}

impl TurnPlanningDomain for CastPlanningDomain<'_> {
    type ObservableState = CastPlanningState;
    type Action = TurnAction;
    type StateKey = CastPlanningStateKey;
    type ActionKey = TurnAction;
    type Endpoint = bool;
    type Error = CastPlanningError;

    fn legal_actions(&self, state: &Self::ObservableState) -> Vec<Self::Action> {
        let mut card_indices = state.hand.clone();
        card_indices.sort_unstable();
        card_indices.dedup();
        let mut actions = Vec::new();
        for card_index in card_indices {
            let Some(card) = self.deck.cards.get(card_index) else {
                continue;
            };
            let transaction = compile_typed_atomic_transaction(card);
            let first_use_self_transfer_tutor = compile_typed_first_use_self_transfer_tutor(card);
            let necropotence_lifecycle = compile_typed_necropotence_lifecycle(card);
            if card.ability_program.self_transfer_tutor_permanent.is_some()
                && first_use_self_transfer_tutor.is_none()
            {
                continue;
            }
            if card.ability_program.necropotence_lifecycle.is_some()
                && necropotence_lifecycle.is_none()
            {
                continue;
            }
            if matches!(
                transaction,
                Some(
                    TypedAtomicTransaction::BargainSearchCastOrHand { .. }
                        | TypedAtomicTransaction::OpponentChoiceSearchSplit
                )
            ) && !planning_spell_printed_cost_is_payable(self, state, card_index, card)
            {
                // Target and pile ranking is deliberately expensive. Printed
                // payment is a cheap necessary condition, so reject an
                // unaffordable tutor before enumerating its choice tree.
                continue;
            }
            let candidate_actions = match transaction.as_ref() {
                Some(TypedAtomicTransaction::HandMana { .. }) => {
                    vec![TurnAction::ActivateHandMana(card_index)]
                }
                Some(TypedAtomicTransaction::BargainSearchCastOrHand { maximum_mana_value }) => {
                    planning_bargain_tutor_actions(self, state, card_index, *maximum_mana_value)
                }
                Some(TypedAtomicTransaction::OpponentChoiceSearchSplit) => {
                    planning_opponent_choice_tutor_actions(self, state, card_index)
                }
                Some(transaction) if transaction.initiation() == AtomicInitiation::CastSpell => {
                    vec![TurnAction::Cast(card_index)]
                }
                Some(_) => Vec::new(),
                None if card.ability_program.atomic_transaction.is_some() => Vec::new(),
                None => ordinary_planning_cast_actions(self, card_index, card),
            };
            for action in candidate_actions {
                // The real executor rechecks reviewed sequence holds after
                // every committed action. The speculative planner carries its
                // own cast history and sacrificed battlefield objects.
                if (self.rule_of_law_cap_active()
                    && state.spells_cast_this_turn >= 1
                    && action_is_spell_cast(action))
                    || card.has(role::LAND)
                    || should_hold_reactive_card(card)
                    || functional_role_card_is_only_noop(
                        self.deck,
                        &state.zones,
                        self.turn,
                        &state.planned_casts,
                        card_index,
                        card,
                    )
                    || action_is_spell_cast(action)
                        && state.planned_casts.is_empty()
                        && should_hold_reviewed_sequence_piece(
                            self.deck,
                            card_index,
                            &state.hand,
                            &state.zones,
                            self.turn,
                            &state.mana_pool,
                            self.mana_access,
                            self.additional_generic_per_cast,
                        )
                {
                    continue;
                }
                actions.push(action);
            }
        }
        if state.zones.land_sacrifice_mana_grant_turn == Some(self.turn) {
            actions.extend(state.zones.battlefield.iter().filter_map(|presence| {
                let card = self.deck.cards.get(presence.card_index)?;
                if !card.effects.card_types.is_land {
                    return None;
                }
                let action = TurnAction::ActivateGrantedLandMana {
                    source_card_index: presence.card_index,
                    source_sequence: presence.sequence,
                };
                Some(action)
            }));
        }
        let ready_primer_actions = reviewed_primer_window_actions_for_planning(self, state);
        let ready_search_sources = ready_primer_actions
            .iter()
            .map(|action| action.card_index())
            .collect::<HashSet<_>>();
        let ready_mana_sources = ready_primer_actions
            .iter()
            .filter_map(|action| match action {
                TurnAction::CastReviewedRandomDiscardWithManaResponse {
                    mana_source_card_index,
                    mana_source_sequence,
                    ..
                } => Some((*mana_source_card_index, *mana_source_sequence)),
                _ => None,
            })
            .collect::<HashSet<_>>();
        let mut preempted_tutors = ready_primer_actions
            .iter()
            .filter_map(|action| match action {
                TurnAction::CastReviewedRandomDiscardWithManaResponse {
                    tutor_source_card_index,
                    tutor_source_sequence,
                    ..
                } => Some((*tutor_source_card_index, *tutor_source_sequence)),
                _ => None,
            })
            .collect::<HashSet<_>>();
        let mut deferred_search_sources = HashSet::new();
        let candidate_mana_source_casts = actions
            .iter()
            .copied()
            .filter(|action| matches!(action, TurnAction::Cast(_)))
            .filter(|action| {
                self.deck
                    .cards
                    .get(action.card_index())
                    .and_then(exact_discard_sacrifice_mana_ability)
                    == Some(3)
            })
            .collect::<Vec<_>>();
        for mana_source_cast in candidate_mana_source_casts {
            for action in reviewed_primer_window_actions_after_mana_source_cast(
                self,
                state,
                mana_source_cast.card_index(),
            ) {
                deferred_search_sources.insert(action.card_index());
                if let TurnAction::CastReviewedRandomDiscardWithManaResponse {
                    tutor_source_card_index,
                    tutor_source_sequence,
                    ..
                } = action
                {
                    preempted_tutors.insert((tutor_source_card_index, tutor_source_sequence));
                }
            }
        }
        actions.retain(|action| {
            !matches!(action, TurnAction::Cast(card_index)
                if ready_search_sources.contains(card_index)
                    || deferred_search_sources.contains(card_index))
        });
        actions.extend(ready_primer_actions);
        for presence in &state.zones.battlefield {
            let Some(card) = self.deck.cards.get(presence.card_index) else {
                continue;
            };
            if exact_discard_sacrifice_mana_ability(card) == Some(3)
                && !ready_mana_sources.contains(&(presence.card_index, presence.sequence))
            {
                actions.extend(FiveColorManaChoice::ALL.map(|color| {
                    TurnAction::ActivateDiscardHandSacrificeMana {
                        source_card_index: presence.card_index,
                        source_sequence: presence.sequence,
                        color,
                    }
                }));
            }
        }
        actions.extend(state.zones.battlefield.iter().filter_map(|presence| {
            if preempted_tutors.contains(&(presence.card_index, presence.sequence)) {
                return None;
            }
            let card = self.deck.cards.get(presence.card_index)?;
            compile_typed_first_use_self_transfer_tutor(card)?;
            Some(TurnAction::ActivateFirstUseSelfTransferTutor {
                source_card_index: presence.card_index,
                source_sequence: presence.sequence,
            })
        }));
        actions.sort_unstable();
        actions.dedup();
        actions
    }

    fn apply_action(
        &self,
        state: &mut Self::ObservableState,
        action: &Self::Action,
    ) -> Result<(), Self::Error> {
        apply_planning_action(self, state, *action)
    }

    fn action_error_is_recoverable(&self, error: &Self::Error) -> bool {
        // Candidate enumeration deliberately stays observation-safe and cheap.
        // Exact payment remains authoritative in the transactional executor,
        // while a disappeared physical object is an invariant failure that
        // must still abort the planner.
        matches!(error, CastPlanningError::PaymentChanged)
    }

    fn canonical_state_key(&self, state: &Self::ObservableState) -> Self::StateKey {
        cast_planning_state_key(state)
    }

    fn action_tie_break_key(&self, action: &Self::Action) -> Self::ActionKey {
        *action
    }

    fn value_vector(&self, state: &Self::ObservableState) -> PlannerValue {
        self.planner_state_evaluation(state).value
    }

    fn terminal_endpoint(&self, state: &Self::ObservableState) -> Option<Self::Endpoint> {
        self.planner_state_evaluation(state).endpoint
    }

    fn evaluate_state(
        &self,
        state: &Self::ObservableState,
    ) -> crate::turn_planner::PlannerStateEvaluation<Self::Endpoint> {
        self.planner_state_evaluation(state)
    }

    fn current_turn_complete(&self, state: &Self::ObservableState) -> bool {
        state.stochastic_planner_expectation.is_some()
            || (!matches!(
                state.planned_actions.last(),
                Some(TurnAction::CastReviewedRandomDiscardWithManaResponse { .. })
            ) && state.planned_casts.last().is_some_and(|card_index| {
                self.deck.cards.get(*card_index).is_some_and(|card| {
                    compile_typed_burst_card_access_program(card).is_some()
                        || compile_typed_atomic_transaction(card).is_some_and(|transaction| {
                            matches!(
                                transaction,
                                TypedAtomicTransaction::SearchRandomDiscardShuffle { .. }
                            )
                        })
                })
            }))
    }
}

fn apply_planning_action(
    domain: &CastPlanningDomain<'_>,
    state: &mut CastPlanningState,
    action: TurnAction,
) -> Result<(), CastPlanningError> {
    apply_planning_action_with_combat_state(domain, state, action, None)
}

fn apply_planning_action_with_combat_state(
    domain: &CastPlanningDomain<'_>,
    state: &mut CastPlanningState,
    action: TurnAction,
    combat_state: Option<&CommanderCombatState>,
) -> Result<(), CastPlanningError> {
    if let TurnAction::ActivateGrantedLandMana {
        source_card_index,
        source_sequence,
    } = action
    {
        let mut staged = state.clone();
        if staged.zones.land_sacrifice_mana_grant_turn != Some(domain.turn) {
            return Err(CastPlanningError::PaymentChanged);
        }
        let presence = staged
            .zones
            .battlefield
            .iter()
            .find(|presence| {
                presence.card_index == source_card_index && presence.sequence == source_sequence
            })
            .copied()
            .ok_or(CastPlanningError::CardDisappeared)?;
        let card = domain
            .deck
            .cards
            .get(presence.card_index)
            .ok_or(CastPlanningError::CardDisappeared)?;
        if !card.effects.card_types.is_land {
            return Err(CastPlanningError::PaymentChanged);
        }
        let removed = staged
            .zones
            .remove_permanent_sequence(domain.deck, source_sequence, true)
            .ok_or(CastPlanningError::CardDisappeared)?;
        if removed != source_card_index {
            return Err(CastPlanningError::CardDisappeared);
        }
        synchronize_turn_pool_with_battlefield(&mut staged.mana_pool, &staged.zones);
        if !add_fixed_mana(
            &mut staged.mana_pool,
            FixedManaProfile {
                black: 1,
                ..FixedManaProfile::default()
            },
        ) {
            return Err(CastPlanningError::PaymentChanged);
        }
        staged.planned_actions.push(action);
        *state = staged;
        return Ok(());
    }

    if let TurnAction::ActivateDiscardHandSacrificeMana {
        source_card_index,
        source_sequence,
        color,
    } = action
    {
        let mut staged = state.clone();
        execute_discard_hand_sacrifice_mana_action(
            domain.deck,
            source_card_index,
            source_sequence,
            color,
            &mut staged.hand,
            &mut staged.zones,
            &mut staged.mana_pool,
        )
        .ok_or(CastPlanningError::PaymentChanged)?;
        staged.planned_actions.push(action);
        *state = staged;
        return Ok(());
    }

    if let TurnAction::ActivateFirstUseSelfTransferTutor {
        source_card_index,
        source_sequence,
    } = action
    {
        let mut staged = state.clone();
        apply_planning_first_use_self_transfer_tutor(
            domain,
            &mut staged,
            source_card_index,
            source_sequence,
        )?;
        staged.planned_actions.push(action);
        *state = staged;
        return Ok(());
    }

    if let TurnAction::CastReviewedRandomDiscardWithManaResponse {
        source_card_index,
        mana_source_card_index,
        mana_source_sequence,
        mill_target_card_index,
        color,
        ..
    } = action
    {
        if !reviewed_primer_window_actions_for_planning(domain, state).contains(&action) {
            return Err(CastPlanningError::PaymentChanged);
        }
        let mut staged = state.clone();
        let source = domain
            .deck
            .cards
            .get(source_card_index)
            .ok_or(CastPlanningError::CardDisappeared)?;
        let source_position = staged
            .hand
            .iter()
            .position(|candidate| *candidate == source_card_index)
            .ok_or(CastPlanningError::CardDisappeared)?;
        let Some(TypedAtomicTransaction::SearchRandomDiscardShuffle { .. }) =
            compile_typed_atomic_transaction(source)
        else {
            return Err(CastPlanningError::PaymentChanged);
        };
        pay_planning_cost_and_reconcile(
            domain,
            &mut staged,
            source_card_index,
            domain
                .mana_access
                .and_then(|access| access.cost(source_card_index)),
            source.mana_value.ceil().max(0.0) as u8,
            domain.additional_generic_per_cast,
            0,
        )?;
        if source_position >= staged.hand.len() || staged.hand[source_position] != source_card_index
        {
            return Err(CastPlanningError::CardDisappeared);
        }
        staged.hand.swap_remove(source_position);
        apply_planning_controller_spell_cast_triggers(domain, &mut staged, source_card_index)?;
        execute_discard_hand_sacrifice_mana_action(
            domain.deck,
            mana_source_card_index,
            mana_source_sequence,
            color,
            &mut staged.hand,
            &mut staged.zones,
            &mut staged.mana_pool,
        )
        .ok_or(CastPlanningError::PaymentChanged)?;
        if !staged.hand.is_empty() {
            return Err(CastPlanningError::PaymentChanged);
        }
        staged
            .zones
            .record_discard(domain.deck, mill_target_card_index);
        if !staged.zones.graveyard.contains(&mill_target_card_index) {
            return Err(CastPlanningError::PaymentChanged);
        }
        staged
            .zones
            .record_cast(domain.deck, source_card_index, domain.turn);
        staged.planned_casts.push(source_card_index);
        staged.planned_actions.push(action);
        staged.stochastic_planner_expectation = None;
        *state = staged;
        return Ok(());
    }

    let card_index = action.card_index();
    let Some(position) = state
        .hand
        .iter()
        .position(|candidate| *candidate == card_index)
    else {
        return Err(CastPlanningError::CardDisappeared);
    };
    let Some(card) = domain.deck.cards.get(card_index) else {
        return Err(CastPlanningError::CardDisappeared);
    };
    if card.ability_program.necropotence_lifecycle.is_some()
        && compile_typed_necropotence_lifecycle(card).is_none()
    {
        return Err(CastPlanningError::PaymentChanged);
    }
    if matches!(action, TurnAction::Cast(_))
        && let Some(kind) = compile_typed_conditional_mana_source(card)
    {
        return apply_planning_conditional_mana_cast(
            domain, state, card_index, position, kind, action,
        );
    }

    match (action, compile_typed_atomic_transaction(card)) {
        (
            TurnAction::ActivateHandMana(_),
            Some(transaction @ TypedAtomicTransaction::HandMana { .. }),
        ) => {
            let Some(output) = atomic_mana_output_for_graveyard_snapshot(
                &transaction,
                state.zones.graveyard.len(),
                0,
            ) else {
                return Err(CastPlanningError::PaymentChanged);
            };
            let mut staged = state.clone();
            staged.hand.swap_remove(position);
            staged.zones.exile.push(card_index);
            staged.zones.advance_sequence();
            if !add_fixed_mana(&mut staged.mana_pool, output) {
                return Err(CastPlanningError::PaymentChanged);
            }
            staged.planned_actions.push(action);
            *state = staged;
            Ok(())
        }
        (TurnAction::ActivateHandMana(_), _) => Err(CastPlanningError::PaymentChanged),
        (TurnAction::ActivateGrantedLandMana { .. }, _) => {
            unreachable!("granted land-mana actions return before hand-card validation")
        }
        (TurnAction::ActivateDiscardHandSacrificeMana { .. }, _) => {
            unreachable!("battlefield mana actions return before hand-card validation")
        }
        (TurnAction::ActivateFirstUseSelfTransferTutor { .. }, _) => {
            unreachable!("battlefield activation actions return before hand-card validation")
        }
        (TurnAction::CastReviewedRandomDiscardWithManaResponse { .. }, _) => {
            unreachable!("reviewed response-window casts return before hand-card validation")
        }
        (
            TurnAction::CastBargainTutor {
                bargain,
                target_card_index,
                cast_without_paying_mana_cost,
                ..
            },
            Some(TypedAtomicTransaction::BargainSearchCastOrHand { maximum_mana_value }),
        ) => {
            if !planner_library_has_copy(domain.deck, state, target_card_index) {
                return Err(CastPlanningError::CardDisappeared);
            }
            let mut staged = state.clone();
            if bargain == BargainPayment::Treasure && !staged.mana_pool.spend_treasures(1) {
                return Err(CastPlanningError::PaymentChanged);
            }
            pay_planning_cost_and_reconcile(
                domain,
                &mut staged,
                card_index,
                domain
                    .mana_access
                    .and_then(|access| access.cost(card_index)),
                card.mana_value.ceil().max(0.0) as u8,
                domain.additional_generic_per_cast,
                0,
            )?;
            let sacrificed_card = match bargain {
                BargainPayment::Permanent {
                    card_index,
                    sequence,
                } => Some(
                    sacrifice_bargain_permanent(
                        domain.deck,
                        &mut staged.zones,
                        &mut staged.mana_pool,
                        card_index,
                        sequence,
                    )
                    .ok_or(CastPlanningError::PaymentChanged)?,
                ),
                BargainPayment::None | BargainPayment::Treasure => None,
            };
            if let Some(sacrificed_card) = sacrificed_card
                && sacrificed_card != card_index
            {
                synchronize_turn_pool_with_battlefield(&mut staged.mana_pool, &staged.zones);
            }
            staged.hand.swap_remove(position);
            apply_planning_controller_spell_cast_triggers(domain, &mut staged, card_index)?;
            staged.planned_casts.push(card_index);
            synchronize_turn_pool_with_battlefield(&mut staged.mana_pool, &staged.zones);
            staged.mana_pool.refresh_battlefield_sources(
                domain.deck,
                &staged.zones,
                active_ability_context(domain.deck, &staged.zones),
            );

            let bargained = bargain != BargainPayment::None;
            let target = domain
                .deck
                .cards
                .get(target_card_index)
                .ok_or(CastPlanningError::CardDisappeared)?;
            let nested_cast_is_rule_blocked =
                domain.rule_of_law_cap_active() && staged.spells_cast_this_turn >= 1;
            let cast_target = bargained
                && cast_without_paying_mana_cost
                && !nested_cast_is_rule_blocked
                && target.mana_value.ceil().max(0.0) as u16 <= maximum_mana_value
                && nested_free_cast_is_supported(target);
            if cast_target {
                pay_planning_cost_and_reconcile(
                    domain,
                    &mut staged,
                    target_card_index,
                    None,
                    0,
                    domain.additional_generic_per_cast,
                    0,
                )?;
                apply_planning_controller_spell_cast_triggers(
                    domain,
                    &mut staged,
                    target_card_index,
                )?;
                staged.planned_casts.push(target_card_index);
            } else {
                staged.hand.push(target_card_index);
            }
            // The searched card is cast while Beseech is still resolving.
            // Beseech then finishes and leaves the stack before that nested
            // spell can resolve.
            staged
                .zones
                .record_cast(domain.deck, card_index, domain.turn);
            if cast_target {
                staged
                    .zones
                    .record_cast(domain.deck, target_card_index, domain.turn);
                apply_conservative_planning_mana_effects(
                    target_card_index,
                    target,
                    domain.mana_access,
                    &staged.zones,
                    &mut staged.mana_pool,
                )?;
                staged.mana_pool.refresh_battlefield_sources(
                    domain.deck,
                    &staged.zones,
                    active_ability_context(domain.deck, &staged.zones),
                );
            }
            staged.planned_actions.push(action);
            *state = staged;
            Ok(())
        }
        (
            TurnAction::CastOpponentChoiceTutor { pile, .. },
            Some(TypedAtomicTransaction::OpponentChoiceSearchSplit),
        ) => {
            let mut remaining = HashMap::<usize, usize>::new();
            for target in pile {
                *remaining.entry(target).or_default() += 1;
            }
            if remaining.into_iter().any(|(target, required)| {
                planner_library_copy_count(domain.deck, state, target) < required
            }) {
                return Err(CastPlanningError::CardDisappeared);
            }
            let mut base = state.clone();
            pay_planning_cost_and_reconcile(
                domain,
                &mut base,
                card_index,
                domain
                    .mana_access
                    .and_then(|access| access.cost(card_index)),
                card.mana_value.ceil().max(0.0) as u8,
                domain.additional_generic_per_cast,
                0,
            )?;
            base.hand.swap_remove(position);
            apply_planning_controller_spell_cast_triggers(domain, &mut base, card_index)?;
            base.planned_casts.push(card_index);
            base.planned_actions.push(action);

            let mut worst: Option<(PlannerValue, usize, CastPlanningState)> = None;
            for chosen_position in 0..3 {
                let mut outcome = base.clone();
                let chosen = pile[chosen_position];
                outcome.hand.push(chosen);
                for (position, target) in pile.iter().copied().enumerate() {
                    if position != chosen_position {
                        outcome.zones.graveyard.push(target);
                        outcome.zones.advance_sequence();
                    }
                }
                let value = deterministic_planner_state_value(domain, &outcome).0;
                if worst
                    .as_ref()
                    .is_none_or(|current| (value, chosen) < (current.0, current.1))
                {
                    worst = Some((value, chosen, outcome));
                }
            }
            let (_, _, mut outcome) = worst.ok_or(CastPlanningError::PaymentChanged)?;
            // The opponent chooses while Intuition is still resolving. Its
            // source reaches the graveyard only after the chosen outcome is
            // fixed, matching the runtime adversarial evaluation boundary.
            outcome
                .zones
                .record_cast(domain.deck, card_index, domain.turn);
            *state = outcome;
            Ok(())
        }
        (TurnAction::CastBargainTutor { .. }, _)
        | (TurnAction::CastOpponentChoiceTutor { .. }, _) => Err(CastPlanningError::PaymentChanged),
        (TurnAction::Cast(_), Some(transaction))
            if transaction.initiation() == AtomicInitiation::CastSpell
                && !matches!(
                    transaction,
                    TypedAtomicTransaction::BargainSearchCastOrHand { .. }
                        | TypedAtomicTransaction::OpponentChoiceSearchSplit
                ) =>
        {
            let mut staged = state.clone();
            pay_planning_cost_and_reconcile(
                domain,
                &mut staged,
                card_index,
                domain
                    .mana_access
                    .and_then(|access| access.cost(card_index)),
                card.mana_value.ceil().max(0.0) as u8,
                domain.additional_generic_per_cast,
                0,
            )?;
            if matches!(
                transaction,
                TypedAtomicTransaction::SacrificeRitual { .. }
                    | TypedAtomicTransaction::SacrificeTutor { .. }
            ) && sacrifice_noncommander_creature(domain.deck, &mut staged.zones).is_none()
            {
                return Err(CastPlanningError::PaymentChanged);
            }

            let pre_resolution_graveyard_count = staged.zones.graveyard.len();
            let pre_resolution_source_name_matches =
                exact_known_graveyard_name_match_count(domain.deck, &staged.zones, card_index)
                    .ok_or(CastPlanningError::PaymentChanged)?;
            staged.hand.swap_remove(position);
            apply_planning_controller_spell_cast_triggers(domain, &mut staged, card_index)?;
            // The physical spell reaches the graveyard after resolving (or
            // being countered), but the threshold snapshot intentionally
            // excludes that source object.
            staged
                .zones
                .record_cast(domain.deck, card_index, domain.turn);
            if let Some(output) = atomic_mana_output_for_graveyard_snapshot(
                &transaction,
                pre_resolution_graveyard_count,
                pre_resolution_source_name_matches,
            ) && !add_fixed_mana(&mut staged.mana_pool, output)
            {
                return Err(CastPlanningError::PaymentChanged);
            }
            if matches!(
                transaction,
                TypedAtomicTransaction::TemporaryLandSacrificeManaGrant { .. }
            ) {
                staged.zones.land_sacrifice_mana_grant_turn = Some(domain.turn);
            }
            synchronize_turn_pool_with_battlefield(&mut staged.mana_pool, &staged.zones);
            staged.mana_pool.refresh_battlefield_sources(
                domain.deck,
                &staged.zones,
                active_ability_context(domain.deck, &staged.zones),
            );
            apply_planning_atomic_tutor_resolution(domain, &mut staged, &transaction);
            staged.planned_casts.push(card_index);
            staged.planned_actions.push(action);
            *state = staged;
            Ok(())
        }
        (TurnAction::Cast(_), Some(_)) | (TurnAction::CastAlternativeCost(_), Some(_)) => {
            Err(CastPlanningError::PaymentChanged)
        }
        (TurnAction::Cast(_), None) if card.ability_program.atomic_transaction.is_some() => {
            Err(CastPlanningError::PaymentChanged)
        }
        (TurnAction::Cast(_), None)
            if compile_typed_first_use_self_transfer_tutor(card).is_some() =>
        {
            let mut staged = state.clone();
            pay_planning_cost_and_reconcile(
                domain,
                &mut staged,
                card_index,
                domain
                    .mana_access
                    .and_then(|access| access.cost(card_index)),
                card.mana_value.ceil().max(0.0) as u8,
                domain.additional_generic_per_cast,
                0,
            )?;
            staged.hand.swap_remove(position);
            apply_planning_controller_spell_cast_triggers(domain, &mut staged, card_index)?;
            staged
                .zones
                .record_cast(domain.deck, card_index, domain.turn);
            staged.mana_pool.refresh_battlefield_sources(
                domain.deck,
                &staged.zones,
                active_ability_context(domain.deck, &staged.zones),
            );
            staged.planned_casts.push(card_index);
            staged.planned_actions.push(action);
            *state = staged;
            Ok(())
        }
        (TurnAction::Cast(_), None)
            if card.ability_program.self_transfer_tutor_permanent.is_some() =>
        {
            Err(CastPlanningError::PaymentChanged)
        }
        (TurnAction::Cast(_) | TurnAction::CastAlternativeCost(_), None) => {
            let mut staged = state.clone();
            let payment_choice = action
                .spell_payment_choice()
                .ok_or(CastPlanningError::PaymentChanged)?;
            pay_planning_cost_choice_and_reconcile(
                domain,
                &mut staged,
                card_index,
                domain
                    .mana_access
                    .and_then(|access| access.cost(card_index)),
                card.mana_value.ceil().max(0.0) as u8,
                domain.additional_generic_per_cast,
                0,
                payment_choice,
                combat_state,
            )?;
            staged.hand.swap_remove(position);
            apply_planning_controller_spell_cast_triggers(domain, &mut staged, card_index)?;
            staged
                .zones
                .record_cast(domain.deck, card_index, domain.turn);
            apply_conservative_planning_mana_effects(
                card_index,
                card,
                domain.mana_access,
                &staged.zones,
                &mut staged.mana_pool,
            )?;
            staged.mana_pool.refresh_battlefield_sources(
                domain.deck,
                &staged.zones,
                active_ability_context(domain.deck, &staged.zones),
            );
            apply_planning_spell_tutor_resolution(domain, &mut staged, card);
            if compile_typed_burst_card_access_program(card)
                == Some(TypedBurstCardAccessProgram::WholeHandDiscardThenDraw)
            {
                // The seven replacement cards are hidden until the real
                // runtime resolves the spell. Preserve the observable
                // discard, then stop this speculative branch at the
                // observation boundary instead of scoring phantom suffix
                // actions from the pre-Wheel hand.
                staged
                    .zones
                    .record_discards(domain.deck, std::mem::take(&mut staged.hand));
            }
            staged.planned_casts.push(card_index);
            staged.planned_actions.push(action);
            *state = staged;
            Ok(())
        }
    }
}

fn known_card_copy_count(hand: &[usize], zones: &KnownLineZoneState, card_index: usize) -> usize {
    hand.iter()
        .filter(|candidate| **candidate == card_index)
        .count()
        .saturating_add(
            zones
                .battlefield
                .iter()
                .filter(|presence| presence.card_index == card_index)
                .count(),
        )
        .saturating_add(
            zones
                .graveyard
                .iter()
                .filter(|candidate| **candidate == card_index)
                .count(),
        )
        .saturating_add(
            zones
                .exile
                .iter()
                .filter(|candidate| **candidate == card_index)
                .count(),
        )
}

fn planner_known_card_copy_count(state: &CastPlanningState, card_index: usize) -> usize {
    known_card_copy_count(&state.hand, &state.zones, card_index)
}

fn planner_library_has_copy(
    deck: &CompiledDeck,
    state: &CastPlanningState,
    card_index: usize,
) -> bool {
    planner_library_copy_count(deck, state, card_index) > 0
}

fn planner_library_copy_count(
    deck: &CompiledDeck,
    state: &CastPlanningState,
    card_index: usize,
) -> usize {
    deck.cards.get(card_index).map_or(0, |card| {
        if card.is_commander {
            0
        } else {
            usize::from(card.quantity)
                .saturating_sub(planner_known_card_copy_count(state, card_index))
        }
    })
}

fn best_planning_tutor_target(
    domain: &CastPlanningDomain<'_>,
    state: &CastPlanningState,
    mut target_matches: impl FnMut(&CompiledCard) -> bool,
) -> Option<usize> {
    domain
        .deck
        .cards
        .iter()
        .enumerate()
        .filter(|(card_index, card)| {
            planner_library_has_copy(domain.deck, state, *card_index) && target_matches(card)
        })
        .map(|(card_index, card)| {
            (
                card_index,
                exact_tutor_reviewed_route_rank_with_library(
                    domain.deck,
                    card_index,
                    &state.hand,
                    &state.zones,
                    domain.turn,
                    domain.mana_access,
                    &state.mana_pool,
                    domain.additional_generic_per_cast,
                    |index| planner_library_copy_count(domain.deck, state, index),
                ),
                tutor_target_score(domain.deck, card, &state.hand, &state.zones, domain.turn),
                card.normalized_name.as_str(),
            )
        })
        .max_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| right.3.cmp(left.3))
                .then_with(|| right.0.cmp(&left.0))
        })
        .map(|(card_index, _, _, _)| card_index)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UniformRandomDiscardTutorChoice {
    target_index: usize,
    expectation: StochasticPlannerExpectation,
    searched_card_survival_outcomes: u16,
    target_score: i32,
}

#[allow(clippy::too_many_arguments)]
fn uniform_random_discard_outcome_expectation(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand_after_source_cast: &[usize],
    zones_after_source_cast: &KnownLineZoneState,
    turn: u8,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
    planned_casts: &[usize],
    development: i64,
    searched_target: Option<usize>,
) -> Option<(StochasticPlannerExpectation, u16)> {
    let mut resulting_hand = hand_after_source_cast.to_vec();
    let searched_position = searched_target.map(|target_index| {
        resulting_hand.push(target_index);
        resulting_hand.len() - 1
    });
    let mut searched_card_survival_outcomes = 0u16;

    if resulting_hand.is_empty() {
        let (value, _) = planner_value_with_development(
            deck,
            mana_access,
            &resulting_hand,
            zones_after_source_cast,
            turn,
            mana_pool,
            future_additional_generic_per_cast,
            planned_casts,
            development,
        );
        return stochastic_expectation_from_outcomes([value]).map(|expectation| (expectation, 0));
    }

    let mut outcomes = Vec::with_capacity(resulting_hand.len());
    for discard_position in 0..resulting_hand.len() {
        let mut outcome_hand = resulting_hand.clone();
        let discarded = outcome_hand.remove(discard_position);
        let mut outcome_zones = zones_after_source_cast.clone();
        outcome_zones.record_discard(deck, discarded);
        let (value, _) = planner_value_with_development(
            deck,
            mana_access,
            &outcome_hand,
            &outcome_zones,
            turn,
            mana_pool,
            future_additional_generic_per_cast,
            planned_casts,
            development,
        );
        outcomes.push(value);
        if searched_position.is_some_and(|position| position != discard_position) {
            searched_card_survival_outcomes = searched_card_survival_outcomes.saturating_add(1);
        }
    }
    stochastic_expectation_from_outcomes(outcomes)
        .map(|expectation| (expectation, searched_card_survival_outcomes))
}

/// Choose a public search identity by enumerating the exact uniform discard
/// outcomes from the resulting hand. Candidate enumeration is by compiled
/// card identity and an observable library multiset, never by hidden library
/// position. The selected card is not assumed to survive in planner state.
#[allow(clippy::too_many_arguments)]
fn best_uniform_random_discard_tutor_target(
    tutor: &ProgramTutorEffect,
    deck: &CompiledDeck,
    hand_after_source_cast: &[usize],
    zones_after_source_cast: &KnownLineZoneState,
    turn: u8,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
    planned_casts: &[usize],
    development: i64,
    available_library_copies: impl Fn(usize) -> usize,
) -> Option<UniformRandomDiscardTutorChoice> {
    if tutor.from != ProgramZone::Library || tutor.destination != ProgramZone::Hand {
        return None;
    }

    deck.cards
        .iter()
        .enumerate()
        .filter(|(card_index, card)| {
            available_library_copies(*card_index) > 0 && program_tutor_matches(&tutor.filter, card)
        })
        .filter_map(|(target_index, target)| {
            let (expectation, searched_card_survival_outcomes) =
                uniform_random_discard_outcome_expectation(
                    deck,
                    mana_access,
                    hand_after_source_cast,
                    zones_after_source_cast,
                    turn,
                    mana_pool,
                    future_additional_generic_per_cast,
                    planned_casts,
                    development,
                    Some(target_index),
                )?;

            Some(UniformRandomDiscardTutorChoice {
                target_index,
                expectation,
                searched_card_survival_outcomes,
                target_score: tutor_target_score(
                    deck,
                    target,
                    hand_after_source_cast,
                    zones_after_source_cast,
                    turn,
                ),
            })
        })
        .max_by(|left, right| {
            compare_stochastic_planner_expectations(left.expectation, right.expectation)
                .then_with(|| {
                    left.searched_card_survival_outcomes
                        .cmp(&right.searched_card_survival_outcomes)
                })
                .then_with(|| left.target_score.cmp(&right.target_score))
                // Equal public values choose a stable public identity instead
                // of whichever copy happens to occur first in hidden order.
                .then_with(|| {
                    let left_name = &deck.cards[left.target_index].normalized_name;
                    let right_name = &deck.cards[right.target_index].normalized_name;
                    right_name.cmp(left_name)
                })
                .then_with(|| right.target_index.cmp(&left.target_index))
        })
}

fn apply_planning_first_use_self_transfer_tutor(
    domain: &CastPlanningDomain<'_>,
    state: &mut CastPlanningState,
    source_card_index: usize,
    source_sequence: u16,
) -> Result<(), CastPlanningError> {
    let tutor = state
        .zones
        .battlefield
        .iter()
        .find(|presence| {
            presence.card_index == source_card_index && presence.sequence == source_sequence
        })
        .and_then(|presence| domain.deck.cards.get(presence.card_index))
        .and_then(compile_typed_first_use_self_transfer_tutor)
        .ok_or(CastPlanningError::PaymentChanged)?;
    let current_route_potential = planning_reviewed_sequence_potential(
        domain.deck,
        domain.mana_access,
        &state.hand,
        &state.zones,
        domain.turn,
        &state.mana_pool,
        domain.additional_generic_per_cast,
    );
    if !pay_first_use_self_transfer_tutor_activation(&mut state.mana_pool, &tutor) {
        return Err(CastPlanningError::PaymentChanged);
    }
    reconcile_spent_planning_permanent_mana(domain.deck, state)?;
    settle_planning_payment(state)?;
    let target_index = best_planning_tutor_target(domain, state, |_| true)
        .ok_or(CastPlanningError::PaymentChanged)?;
    let target = domain
        .deck
        .cards
        .get(target_index)
        .ok_or(CastPlanningError::CardDisappeared)?;
    let mut post_search_hand = state.hand.clone();
    post_search_hand.push(target_index);
    let post_route_potential = planning_reviewed_sequence_potential(
        domain.deck,
        domain.mana_access,
        &post_search_hand,
        &state.zones,
        domain.turn,
        &state.mana_pool,
        domain.additional_generic_per_cast,
    );
    if post_route_potential <= current_route_potential
        && tutor_target_score(domain.deck, target, &state.hand, &state.zones, domain.turn) < 4_000
    {
        return Err(CastPlanningError::PaymentChanged);
    }

    state.hand = post_search_hand;
    let removed_card = state
        .zones
        .remove_permanent_sequence(domain.deck, source_sequence, false)
        .ok_or(CastPlanningError::CardDisappeared)?;
    if removed_card != source_card_index {
        return Err(CastPlanningError::CardDisappeared);
    }
    synchronize_turn_pool_with_battlefield(&mut state.mana_pool, &state.zones);
    state.mana_pool.refresh_battlefield_sources(
        domain.deck,
        &state.zones,
        active_ability_context(domain.deck, &state.zones),
    );
    Ok(())
}

fn apply_planning_spell_tutor_resolution(
    domain: &CastPlanningDomain<'_>,
    state: &mut CastPlanningState,
    tutor: &CompiledCard,
) {
    if !tutor.effects.tutor.is_executable_on_spell_resolution() {
        return;
    }
    for instruction in &tutor.effects.tutor.instructions {
        if instruction.source != TutorSourceZone::Library
            || instruction.destination != TutorDestination::Hand
        {
            continue;
        }
        for _ in 0..instruction.quantity.min(8) {
            let Some(target_index) = best_planning_tutor_target(domain, state, |candidate| {
                instruction.target.matches(candidate.effects.card_types)
            }) else {
                break;
            };
            // This is an observable search choice, not hidden-order
            // speculation. Runtime removes the same physical target from the
            // actual library before any queued cast is committed.
            state.hand.push(target_index);
        }
    }
}

fn apply_planning_atomic_tutor_resolution(
    domain: &CastPlanningDomain<'_>,
    state: &mut CastPlanningState,
    transaction: &TypedAtomicTransaction,
) {
    match transaction {
        TypedAtomicTransaction::SacrificeTutor { tutor } => {
            if tutor.from != ProgramZone::Library || tutor.destination != ProgramZone::Hand {
                return;
            }
            let Some(target_index) = best_planning_tutor_target(domain, state, |candidate| {
                program_tutor_matches(&tutor.filter, candidate)
            }) else {
                return;
            };
            state.hand.push(target_index);
        }
        TypedAtomicTransaction::SearchRandomDiscardShuffle { tutor } => {
            let development = planning_action_development(domain, state);
            state.stochastic_planner_expectation = best_uniform_random_discard_tutor_target(
                tutor,
                domain.deck,
                &state.hand,
                &state.zones,
                domain.turn,
                domain.mana_access,
                &state.mana_pool,
                domain.additional_generic_per_cast,
                &state.planned_casts,
                development,
                |card_index| planner_library_copy_count(domain.deck, state, card_index),
            )
            .map(|choice| choice.expectation)
            .or_else(|| {
                (tutor.from == ProgramZone::Library && tutor.destination == ProgramZone::Hand)
                    .then(|| {
                        uniform_random_discard_outcome_expectation(
                            domain.deck,
                            domain.mana_access,
                            &state.hand,
                            &state.zones,
                            domain.turn,
                            &state.mana_pool,
                            domain.additional_generic_per_cast,
                            &state.planned_casts,
                            development,
                            None,
                        )
                        .map(|(expectation, _)| expectation)
                    })
                    .flatten()
            });
        }
        _ => {}
    }
}

fn apply_planning_conditional_mana_cast(
    domain: &CastPlanningDomain<'_>,
    state: &mut CastPlanningState,
    card_index: usize,
    hand_position: usize,
    kind: TypedConditionalManaSource,
    action: TurnAction,
) -> Result<(), CastPlanningError> {
    let card = domain
        .deck
        .cards
        .get(card_index)
        .ok_or(CastPlanningError::CardDisappeared)?;
    let mut staged = state.clone();
    pay_planning_cost_and_reconcile(
        domain,
        &mut staged,
        card_index,
        domain
            .mana_access
            .and_then(|access| access.cost(card_index)),
        card.mana_value.ceil().max(0.0) as u8,
        domain.additional_generic_per_cast,
        0,
    )?;
    if hand_position >= staged.hand.len() || staged.hand[hand_position] != card_index {
        return Err(CastPlanningError::CardDisappeared);
    }
    staged.hand.swap_remove(hand_position);
    apply_planning_controller_spell_cast_triggers(domain, &mut staged, card_index)?;
    let entry = resolve_typed_permanent_entry(
        kind,
        domain.deck,
        card_index,
        &mut staged.hand,
        domain.mana_access,
        &mut staged.zones,
        domain.turn,
        &staged.mana_pool,
        domain.additional_generic_per_cast,
    );
    if !entry.entered_battlefield {
        return Err(CastPlanningError::PaymentChanged);
    }
    if kind == TypedConditionalManaSource::ImprintLinkedCardColors
        && (entry.moved_hand_card.is_none()
            || entry.linked_colors.is_none_or(ManaColorMask::is_empty))
    {
        // Casting Chrome Mox with no usable imprint is legal, but it is not a
        // mana-development action. Reject that speculative branch so the
        // runtime cannot commit a zero-output Mox merely because the artifact
        // itself has a static role score.
        return Err(CastPlanningError::PaymentChanged);
    }
    let source = typed_battlefield_mana_source(
        domain.deck,
        card_index,
        kind,
        entry.linked_colors,
        domain.turn,
    )
    .ok_or(CastPlanningError::PaymentChanged)?;
    add_typed_source_to_current_pool(
        source,
        domain.deck,
        &staged.zones,
        domain.turn,
        &mut staged.mana_pool,
    );
    staged.mana_pool.refresh_battlefield_sources(
        domain.deck,
        &staged.zones,
        active_ability_context(domain.deck, &staged.zones),
    );
    staged.planned_casts.push(card_index);
    staged.planned_actions.push(action);
    *state = staged;
    Ok(())
}

fn combat_state_progress_basis_points(state: &CommanderCombatState) -> i64 {
    OpponentId::ALL
        .into_iter()
        .map(|opponent| {
            let opponent = state.opponent(opponent);
            if opponent.has_left_game() {
                return 10_000;
            }
            let life_remaining = opponent.life_total().clamp(0, 40);
            let life_progress = (40 - life_remaining) * 10_000 / 40;
            let commander_progress =
                i64::from(opponent.commander_combat_damage().min(21)) * 10_000 / 21;
            life_progress.max(commander_progress)
        })
        .sum()
}

fn projected_combat_planner_value(
    deck: &CompiledDeck,
    state: &CastPlanningState,
    turn: u8,
    combat_state: &CommanderCombatState,
) -> (i64, bool) {
    let mut projected_zones = state.zones.clone();
    let mut projected_pool = state.mana_pool.clone();
    let _ = activate_equipment_for_combat(deck, &mut projected_zones, &mut projected_pool, turn);
    let _ = activate_counter_threshold_token_abilities(
        deck,
        &mut projected_zones,
        &mut projected_pool,
        turn,
    );

    let mut after_current_combat = combat_state.clone();
    let current_attack = plan_combat_attack(
        deck,
        &projected_zones,
        turn,
        &after_current_combat,
        &projected_pool,
    );
    let presents_table_lethal = current_attack
        .as_ref()
        .is_some_and(PresentedAttack::presents_table_lethal);
    if let Some(attack) = current_attack.as_ref() {
        let _ = resolve_all_connected_combat_damage(&mut after_current_combat, attack);
    }
    let current_progress = combat_state_progress_basis_points(&after_current_combat);

    let mut after_next_combat = after_current_combat.clone();
    if !after_next_combat.evaluate_terminal().is_table_win() {
        // Reusable mana creatures untap before the following combat. The
        // empty pool therefore excludes no attackers, while exact tap costs
        // recorded in zone state are cleared by the real next-turn boundary.
        let mut next_zones = projected_zones;
        next_zones.tapped_creatures_this_turn.clear();
        if let Some(next_attack) = plan_combat_attack(
            deck,
            &next_zones,
            turn.saturating_add(1),
            &after_next_combat,
            &TurnManaPool::default(),
        ) {
            let _ = resolve_all_connected_combat_damage(&mut after_next_combat, &next_attack);
        }
    }
    let next_progress = combat_state_progress_basis_points(&after_next_combat);
    (
        current_progress
            .saturating_mul(30_001)
            .saturating_add(next_progress),
        presents_table_lethal,
    )
}

struct CombatAwareCastPlanningDomain<'domain, 'deck> {
    base: &'domain CastPlanningDomain<'deck>,
    combat_state: &'domain CommanderCombatState,
}

impl TurnPlanningDomain for CombatAwareCastPlanningDomain<'_, '_> {
    type ObservableState = CastPlanningState;
    type Action = TurnAction;
    type StateKey = CastPlanningStateKey;
    type ActionKey = TurnAction;
    type Endpoint = bool;
    type Error = CastPlanningError;

    fn legal_actions(&self, state: &Self::ObservableState) -> Vec<Self::Action> {
        self.base.legal_actions(state)
    }

    fn is_executable_action(&self, state: &Self::ObservableState, action: &Self::Action) -> bool {
        self.base.is_executable_action(state, action)
    }

    fn apply_action(
        &self,
        state: &mut Self::ObservableState,
        action: &Self::Action,
    ) -> Result<(), Self::Error> {
        apply_planning_action_with_combat_state(self.base, state, *action, Some(self.combat_state))
    }

    fn action_error_is_recoverable(&self, error: &Self::Error) -> bool {
        self.base.action_error_is_recoverable(error)
    }

    fn canonical_state_key(&self, state: &Self::ObservableState) -> Self::StateKey {
        self.base.canonical_state_key(state)
    }

    fn action_tie_break_key(&self, action: &Self::Action) -> Self::ActionKey {
        self.base.action_tie_break_key(action)
    }

    fn value_vector(&self, state: &Self::ObservableState) -> PlannerValue {
        self.evaluate_state(state).value
    }

    fn dominance_vector(&self, state: &Self::ObservableState) -> PlannerValue {
        self.evaluate_state(state).dominance
    }

    fn terminal_endpoint(&self, state: &Self::ObservableState) -> Option<Self::Endpoint> {
        self.base.terminal_endpoint(state)
    }

    fn evaluate_state(
        &self,
        state: &Self::ObservableState,
    ) -> crate::turn_planner::PlannerStateEvaluation<Self::Endpoint> {
        let mut evaluation = self.base.planner_state_evaluation(state);
        if state.stochastic_planner_expectation.is_none() {
            let (combat_value, presents_table_lethal) = projected_combat_planner_value(
                self.base.deck,
                state,
                self.base.turn,
                self.combat_state,
            );
            evaluation.value.credible_executable_threat = evaluation
                .value
                .credible_executable_threat
                .max(i64::from(presents_table_lethal));
            evaluation.value.card_mana_development = combat_value
                .saturating_mul(1_000_000)
                .saturating_add(evaluation.value.card_mana_development);
            evaluation.dominance = evaluation.value;
        }
        evaluation
    }

    fn current_turn_complete(&self, state: &Self::ObservableState) -> bool {
        self.base.current_turn_complete(state)
    }

    fn conservative_next_turn_state(
        &self,
        state: &Self::ObservableState,
    ) -> Result<Option<Self::ObservableState>, Self::Error> {
        self.base.conservative_next_turn_state(state)
    }
}

fn plan_hand_action_order(
    domain: &CastPlanningDomain<'_>,
    hand: &[usize],
    mana_pool: &TurnManaPool,
) -> Vec<TurnAction> {
    let initial = CastPlanningState {
        hand: hand.to_vec(),
        mana_pool: mana_pool.clone(),
        zones: domain.zones.clone(),
        player_life: domain.player_life,
        spells_cast_this_turn: domain.spells_cast_this_turn,
        planned_casts: Vec::new(),
        planned_actions: Vec::new(),
        stochastic_planner_expectation: None,
    };
    if let Some(certified) =
        certified_reviewed_graveyard_storm_primer_continuation(domain, initial.clone())
    {
        return certified;
    }
    let cancellation = AtomicBool::new(false);
    plan_turn(
        domain,
        initial,
        PlannerConfig {
            beam_width: CAST_PLANNER_BEAM_WIDTH,
            max_node_expansions: CAST_PLANNER_MAX_EXPANSIONS,
            max_actions: CAST_PLANNER_MAX_ACTIONS,
            horizon: PlanningHorizon::CurrentTurnOnly,
        },
        &cancellation,
    )
    .map(|result| result.best.actions)
    .unwrap_or_default()
}

fn plan_hand_action_order_with_combat(
    domain: &CastPlanningDomain<'_>,
    hand: &[usize],
    mana_pool: &TurnManaPool,
    combat_state: &CommanderCombatState,
) -> Vec<TurnAction> {
    let initial = CastPlanningState {
        hand: hand.to_vec(),
        mana_pool: mana_pool.clone(),
        zones: domain.zones.clone(),
        player_life: domain.player_life,
        spells_cast_this_turn: domain.spells_cast_this_turn,
        planned_casts: Vec::new(),
        planned_actions: Vec::new(),
        stochastic_planner_expectation: None,
    };
    if let Some(certified) =
        certified_reviewed_graveyard_storm_primer_continuation(domain, initial.clone())
    {
        return certified;
    }
    let cancellation = AtomicBool::new(false);
    let combat_domain = CombatAwareCastPlanningDomain {
        base: domain,
        combat_state,
    };
    plan_turn(
        &combat_domain,
        initial,
        PlannerConfig {
            beam_width: CAST_PLANNER_BEAM_WIDTH,
            max_node_expansions: CAST_PLANNER_MAX_EXPANSIONS,
            max_actions: CAST_PLANNER_MAX_ACTIONS,
            horizon: PlanningHorizon::CurrentTurnOnly,
        },
        &cancellation,
    )
    .map(|result| result.best.actions)
    .unwrap_or_default()
}

fn certified_reviewed_graveyard_storm_primer_continuation(
    domain: &CastPlanningDomain<'_>,
    initial: CastPlanningState,
) -> Option<Vec<TurnAction>> {
    const MAX_CERTIFICATE_ACTIONS: usize = 10;
    const MAX_CERTIFICATE_NODES: usize = 2_048;

    let programs = domain
        .deck
        .known_lines
        .iter()
        .filter_map(|line| compile_graveyard_storm_program(line, domain.deck))
        .collect::<Vec<_>>();
    if programs.is_empty() || domain.rule_of_law_cap_active() {
        return None;
    }
    let reviewed_response_is_public =
        |state: &CastPlanningState, program: GraveyardStormProgram| {
            state.zones.graveyard.contains(&program.mana_source)
                && state.zones.graveyard.contains(&program.mill_spell)
                && state.zones.spells_cast_this_turn.iter().any(|spell| {
                    domain.deck.cards.get(spell.card_index).is_some_and(|card| {
                        matches!(
                            compile_typed_atomic_transaction(card),
                            Some(TypedAtomicTransaction::SearchRandomDiscardShuffle { .. })
                        )
                    })
                })
        };
    let endpoint = |state: &CastPlanningState, actions: &[TurnAction]| {
        programs.iter().any(|program| {
            let response_in_current_suffix = actions.iter().any(|action| {
                matches!(
                    action,
                    TurnAction::CastReviewedRandomDiscardWithManaResponse {
                        mana_source_card_index,
                        permission_source_card_index,
                        mill_target_card_index,
                        ..
                    } if *mana_source_card_index == program.mana_source
                        && *permission_source_card_index == program.permission_source
                        && *mill_target_card_index == program.mill_spell
                )
            });
            let response_was_observed = reviewed_response_is_public(state, *program);
            let tutor_in_current_suffix = actions.iter().any(|action| {
                matches!(action, TurnAction::ActivateFirstUseSelfTransferTutor { .. })
            });
            let tutor_was_observed = response_was_observed
                && active_first_use_self_transfer_tutor(domain.deck, &state.zones).is_none();

            battlefield_contains(&state.zones, program.permission_source)
                && state.zones.graveyard.contains(&program.mana_source)
                && state.zones.graveyard.contains(&program.mill_spell)
                && (response_in_current_suffix || response_was_observed)
                && (tutor_in_current_suffix || tutor_was_observed)
        })
    };
    let setup_cast = |card_index: usize| {
        let Some(card) = domain.deck.cards.get(card_index) else {
            return false;
        };
        programs.iter().any(|program| {
            card_index == program.permission_source || card_index == program.mana_source
        }) || compile_typed_first_use_self_transfer_tutor(card).is_some()
            || matches!(
                compile_typed_atomic_transaction(card),
                Some(TypedAtomicTransaction::NameLinkedGraveyardRitual { .. })
            )
            || card.effects.mana_production_kind == ManaProductionKind::OneShotActivated
                && card.effects.mana_produced.conservative_value(1) > 0
    };

    let mut frontier = VecDeque::from([(initial, Vec::<TurnAction>::new())]);
    let mut expanded = 0usize;
    while let Some((state, actions)) = frontier.pop_front() {
        if endpoint(&state, &actions) {
            return Some(actions);
        }
        if actions.len() >= MAX_CERTIFICATE_ACTIONS || expanded >= MAX_CERTIFICATE_NODES {
            continue;
        }
        expanded = expanded.saturating_add(1);

        let mut candidate_actions = domain
            .legal_actions(&state)
            .into_iter()
            .filter(|action| {
                matches!(
                    action,
                    TurnAction::CastReviewedRandomDiscardWithManaResponse { .. }
                        | TurnAction::ActivateFirstUseSelfTransferTutor { .. }
                ) || matches!(
                    action,
                    TurnAction::Cast(card_index) | TurnAction::CastAlternativeCost(card_index)
                        if setup_cast(*card_index)
                )
            })
            .collect::<Vec<_>>();
        // The ordinary planner deliberately holds isolated combo pieces when
        // their payoff is not visible in that plan's local cast history. A
        // replan after Wishclaw's observable search starts with an empty
        // history even though the exact typed prefix is already present in
        // public zones. Admit only the certificate's reviewed setup casts
        // here; `apply_planning_action` remains the exact payment/zone gate and
        // the endpoint still requires the complete Gamble/LED/Wishclaw proof.
        candidate_actions.extend(
            state
                .hand
                .iter()
                .copied()
                .filter(|card_index| setup_cast(*card_index))
                .map(TurnAction::Cast),
        );
        candidate_actions.sort_unstable();
        candidate_actions.dedup();
        for action in candidate_actions {
            let mut next = state.clone();
            match apply_planning_action(domain, &mut next, action) {
                Ok(()) => {}
                Err(CastPlanningError::PaymentChanged) => continue,
                Err(CastPlanningError::CardDisappeared) => return None,
            }
            let mut next_actions = actions.clone();
            next_actions.push(action);
            frontier.push_back((next, next_actions));
        }
    }
    None
}

fn reviewed_primer_window_preempts_eager_tutor(
    domain: &CastPlanningDomain<'_>,
    hand: &[usize],
    mana_pool: &TurnManaPool,
) -> bool {
    let state = CastPlanningState {
        hand: hand.to_vec(),
        mana_pool: mana_pool.clone(),
        zones: domain.zones.clone(),
        player_life: domain.player_life,
        spells_cast_this_turn: domain.spells_cast_this_turn,
        planned_casts: Vec::new(),
        planned_actions: Vec::new(),
        stochastic_planner_expectation: None,
    };
    let actions = domain.legal_actions(&state);
    actions.iter().copied().any(|action| {
        matches!(
            action,
            TurnAction::CastReviewedRandomDiscardWithManaResponse { .. }
        )
    }) || actions.into_iter().any(|action| {
        matches!(
            action,
            TurnAction::Cast(_) | TurnAction::CastAlternativeCost(_)
        ) && !reviewed_primer_window_actions_after_mana_source_cast(
            domain,
            &state,
            action.card_index(),
        )
        .is_empty()
    })
}

fn hand_plan_completes_credible_executable_route(
    domain: &CastPlanningDomain<'_>,
    hand: &[usize],
    mana_pool: &TurnManaPool,
) -> bool {
    let planned_actions = plan_hand_action_order(domain, hand, mana_pool);
    if planned_actions.is_empty() {
        return false;
    }
    let mut final_state = CastPlanningState {
        hand: hand.to_vec(),
        mana_pool: mana_pool.clone(),
        zones: domain.zones.clone(),
        player_life: domain.player_life,
        spells_cast_this_turn: domain.spells_cast_this_turn,
        planned_casts: Vec::new(),
        planned_actions: Vec::new(),
        stochastic_planner_expectation: None,
    };
    for action in planned_actions {
        if apply_planning_action(domain, &mut final_state, action).is_err() {
            return false;
        }
    }
    let (_, completed_threat, completed_conversion) = planning_route_value(
        domain.deck,
        &final_state.zones,
        domain.turn,
        &final_state.planned_casts,
    );
    completed_threat || completed_conversion
}

fn apply_conservative_planning_mana_effects(
    card_index: usize,
    card: &CompiledCard,
    mana_access: Option<&ManaAccessProfile>,
    zones: &KnownLineZoneState,
    pool: &mut TurnManaPool,
) -> Result<(), CastPlanningError> {
    if exact_discard_sacrifice_mana_ability(card).is_some() {
        // The whole-hand discard and self-sacrifice are a distinct optional
        // battlefield action. Casting the permanent cannot silently activate
        // it or install spendable mana that bypasses those exact costs.
        return Ok(());
    }
    let source_profile = mana_access.and_then(|access| access.source(card_index));
    let source_colors = source_profile
        .map(|source| source.colors)
        .unwrap_or_else(|| {
            if card.effects.mana_production_kind == ManaProductionKind::NonRefreshingActivated {
                ManaColorMask::COLORLESS
            } else {
                ManaColorMask::NONE
            }
        });
    let mana_output = card.effects.mana_produced.conservative_value(1).min(8);
    if card.effects.mana_production_kind == ManaProductionKind::OneShotActivated {
        let source = zones
            .battlefield
            .last()
            .filter(|presence| presence.card_index == card_index)
            .ok_or(CastPlanningError::CardDisappeared)?;
        pool.sources.push(PoolManaSource {
            colors: source_colors,
            remaining: mana_output,
            sacrifice_on_first_spend: Some(SacrificeOnFirstSpend {
                card_index,
                sequence: source.sequence,
            }),
            ..PoolManaSource::default()
        });
    } else if card.effects.mana_production_kind == ManaProductionKind::ReusableActivated {
        // A reusable permanent must remain an exact physical source in the
        // speculative pool. The EOT planner can then carry that same public
        // object into next turn instead of losing it as anonymous floating
        // mana. Summoning-sick creatures and exact tapped entrants are marked
        // used for this turn so the dynamic refresh below cannot untap them;
        // planner_future_mana_sources intentionally rebuilds them next turn.
        if let Some(source) = source_profile.filter(|source| {
            mana_source_profile_is_exact_for_trajectory(source)
                && !source.colors.is_empty()
                && mana_output > 0
        }) {
            let available_now = reusable_nonland_source_is_available_on_entry(card, source);
            let origin_sequence = zones
                .battlefield
                .iter()
                .rev()
                .find(|presence| presence.card_index == card_index)
                .map(|presence| presence.sequence);
            pool.sources.push(PoolManaSource {
                colors: source.colors,
                remaining: if available_now { mana_output } else { 0 },
                origin_card_index: Some(card_index),
                origin_sequence,
                base_capacity: mana_output,
                activation_used: !available_now,
                ..PoolManaSource::default()
            });
        }
    } else if matches!(
        card.effects.mana_production_kind,
        ManaProductionKind::SpellResolution | ManaProductionKind::NonRefreshingActivated
    ) {
        pool.add_floating(source_colors, mana_output);
    }
    pool.add_treasures(immediate_effect_value(
        card,
        card.effects.treasure_tokens,
        1,
    ));
    Ok(())
}

fn planning_route_value(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    planned_casts: &[usize],
) -> (i64, bool, bool) {
    let mut best_progress = 0i64;
    let mut completed_threat = false;
    let mut completed_conversion = false;
    for line in &deck.known_lines {
        if line.cards.is_empty()
            || line.simulation_requirements.iter().any(|requirement| {
                matches!(
                    requirement,
                    LineRequirement::Unmodeled
                        | LineRequirement::TotalExecutionMana
                        | LineRequirement::CombatAccess
                )
            })
        {
            continue;
        }
        let required =
            line.cards
                .iter()
                .fold(HashMap::<String, usize>::new(), |mut counts, name| {
                    *counts
                        .entry(crate::parser::normalize_card_name(name))
                        .or_default() += 1;
                    counts
                });
        let mut present = 0usize;
        for (normalized_name, required_count) in required {
            let already_usable = zones.usable_count(deck, &normalized_name, turn);
            let newly_cast = planned_casts
                .iter()
                .filter(|card_index| {
                    deck.cards
                        .get(**card_index)
                        .is_some_and(|card| card.normalized_name == normalized_name)
                })
                .count();
            // Typed cards are represented in both the speculative zone ledger
            // and the ordered cast list; legacy or ambiguous inputs may only
            // have the latter. `max`, rather than addition, preserves either
            // witness without double-crediting one physical object.
            present += required_count.min(already_usable.max(newly_cast));
        }
        let total = line.cards.len().max(1);
        let progress = (present as i64 * 10_000 / total as i64)
            + i64::from(line.table_lethal_if_resolved) * 500;
        best_progress = best_progress.max(progress);
        if present == total
            && planning_line_sequence_is_credible(line, deck, zones, turn, planned_casts)
        {
            completed_threat = true;
            completed_conversion |= line.table_lethal_if_resolved;
        }
    }
    (best_progress, completed_threat, completed_conversion)
}

fn planning_line_is_completed(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    planned_casts: &[usize],
) -> bool {
    if line.cards.is_empty()
        || line.simulation_requirements.iter().any(|requirement| {
            matches!(
                requirement,
                LineRequirement::Unmodeled
                    | LineRequirement::TotalExecutionMana
                    | LineRequirement::CombatAccess
            )
        })
    {
        return false;
    }
    let required = line
        .cards
        .iter()
        .fold(HashMap::<String, usize>::new(), |mut counts, name| {
            *counts
                .entry(crate::parser::normalize_card_name(name))
                .or_default() += 1;
            counts
        });
    required
        .into_iter()
        .all(|(normalized_name, required_count)| {
            let already_usable = zones.usable_count(deck, &normalized_name, turn);
            let newly_cast = planned_casts
                .iter()
                .filter(|card_index| {
                    deck.cards
                        .get(**card_index)
                        .is_some_and(|card| card.normalized_name == normalized_name)
                })
                .count();
            already_usable.max(newly_cast) >= required_count
        })
        && planning_line_sequence_is_credible(line, deck, zones, turn, planned_casts)
}

#[allow(clippy::too_many_arguments)]
fn planning_reviewed_sequence_potential(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: &TurnManaPool,
    additional_generic_per_cast: u8,
) -> i64 {
    let empty_library_potential = deck
        .known_lines
        .iter()
        .filter(|line| reviewed_empty_library_sequence(line))
        .map(|line| {
            if !reviewed_sequence_zone_order_is_still_credible(line, deck, zones, turn) {
                return 0;
            }
            let total = line.cards.len().max(1);
            let accessible =
                named_line_piece_access_count(line, deck, hand, zones, turn).min(total);
            if accessible < total {
                return accessible as i64 * 10_000 / total as i64;
            }

            // Measure the exact colored/payment distance without inventing a
            // future source or declaring an endpoint. A flexible hypothetical
            // unit is used only as a monotone deficit probe; the real branch
            // still has to produce and spend actual mana before the existing
            // executor may cast either package member.
            for flexible_deficit in 0..=8u8 {
                let mut candidate_pool = mana_pool.clone();
                candidate_pool.add_floating(
                    ManaColorMask::ANY_COLOR | ManaColorMask::COLORLESS,
                    flexible_deficit,
                );
                if reviewed_sequence_package_is_jointly_payable(
                    line,
                    deck,
                    hand,
                    zones,
                    turn,
                    &candidate_pool,
                    mana_access,
                    additional_generic_per_cast,
                ) {
                    let executed =
                        named_line_piece_executed_count(line, deck, zones, turn).min(total);
                    return 60_000 + executed as i64 * 10_000 / total as i64
                        - i64::from(flexible_deficit) * 5_000;
                }
            }
            10_000
        })
        .max()
        .unwrap_or_default();
    empty_library_potential.max(planning_graveyard_storm_access_potential(deck, hand, zones))
}

fn planning_graveyard_storm_access_potential(
    deck: &CompiledDeck,
    hand: &[usize],
    zones: &KnownLineZoneState,
) -> i64 {
    deck.known_lines
        .iter()
        .filter_map(|line| {
            graveyard_storm_planning_access(line, deck, hand, zones, |card_index| {
                deck.cards.get(card_index).map_or(0, |card| {
                    usize::from(card.quantity)
                        .saturating_sub(known_card_copy_count(hand, zones, card_index))
                })
            })
        })
        .map(|access| access.supported_count() as i64 * 10_000 / 3)
        .max()
        .unwrap_or_default()
}

fn named_line_piece_access_count(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
) -> usize {
    let mut required = HashMap::<String, usize>::new();
    for name in &line.cards {
        *required
            .entry(crate::parser::normalize_card_name(name))
            .or_default() += 1;
    }
    required
        .into_iter()
        .map(|(normalized_name, required_count)| {
            let usable = zones.usable_count(deck, &normalized_name, turn);
            let held = hand
                .iter()
                .filter(|card_index| {
                    deck.cards
                        .get(**card_index)
                        .is_some_and(|card| card.normalized_name == normalized_name)
                })
                .count();
            required_count.min(usable.saturating_add(held))
        })
        .sum()
}

fn reviewed_sequence_zone_order_is_still_credible(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
) -> bool {
    let first_line_spell_sequence = zones
        .spells_cast_this_turn
        .iter()
        .filter(|cast| cast.turn == turn)
        .filter_map(|cast| {
            deck.cards.get(cast.card_index).and_then(|card| {
                line.cards
                    .iter()
                    .any(|name| crate::parser::normalize_card_name(name) == card.normalized_name)
                    .then_some(cast.sequence)
            })
        })
        .min();
    let Some(first_line_spell_sequence) = first_line_spell_sequence else {
        return true;
    };

    line.cards.iter().all(|name| {
        let normalized = crate::parser::normalize_card_name(name);
        let Some(card) = unique_card_by_normalized_name(deck, &normalized) else {
            return false;
        };
        if !matches!(
            modeled_line_card_kind(card),
            Some(ModeledLineCardKind::Permanent)
        ) {
            return true;
        }
        zones.battlefield.iter().any(|presence| {
            presence.sequence < first_line_spell_sequence
                && deck
                    .cards
                    .get(presence.card_index)
                    .is_some_and(|candidate| {
                        candidate.normalized_name == normalized && presence.entered_turn <= turn
                    })
        })
    })
}

fn named_line_piece_executed_count(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
) -> usize {
    let mut required = HashMap::<String, usize>::new();
    for name in &line.cards {
        *required
            .entry(crate::parser::normalize_card_name(name))
            .or_default() += 1;
    }
    required
        .into_iter()
        .map(|(normalized_name, required_count)| {
            required_count.min(zones.usable_count(deck, &normalized_name, turn))
        })
        .sum()
}

fn planning_line_sequence_is_credible(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    planned_casts: &[usize],
) -> bool {
    if line
        .simulation_requirements
        .contains(&LineRequirement::ExecutableGraveyardStormLoop)
    {
        // The ordinary hand-cast planner cannot pay escape costs or execute
        // the discard/sacrifice/Storm transaction. Only the atomic route
        // executor may mark this line complete.
        return false;
    }
    if !line
        .simulation_requirements
        .contains(&LineRequirement::ReviewedEmptyLibrarySequence)
    {
        return true;
    }
    let first_planned_spell = planned_casts.iter().position(|card_index| {
        deck.cards.get(*card_index).is_some_and(|card| {
            line.cards.iter().any(|name| {
                crate::parser::normalize_card_name(name) == card.normalized_name
                    && matches!(
                        modeled_line_card_kind(card),
                        Some(ModeledLineCardKind::Spell)
                    )
            })
        })
    });
    line.cards.iter().all(|name| {
        let normalized = crate::parser::normalize_card_name(name);
        let Some(card) = unique_card_by_normalized_name(deck, &normalized) else {
            return false;
        };
        if !matches!(
            modeled_line_card_kind(card),
            Some(ModeledLineCardKind::Permanent)
        ) {
            return true;
        }
        let planned_permanent_position = planned_casts.iter().position(|card_index| {
            deck.cards
                .get(*card_index)
                .is_some_and(|candidate| candidate.normalized_name == normalized)
        });
        if let Some(permanent_position) = planned_permanent_position {
            first_planned_spell.is_none_or(|spell_position| permanent_position < spell_position)
        } else {
            zones.battlefield.iter().any(|presence| {
                presence.entered_turn == turn
                    && deck
                        .cards
                        .get(presence.card_index)
                        .is_some_and(|candidate| candidate.normalized_name == normalized)
            })
        }
    })
}

fn planning_card_development_value(
    card_index: usize,
    card: &CompiledCard,
    policy: PilotPolicy,
    mana_access: Option<&ManaAccessProfile>,
    ability_context: ActiveAbilityContext,
) -> i64 {
    // CurrentTurnOnly floating mana has no end-state value. A ritual, exact
    // hand-mana object, or immediately consumed mana permanent therefore earns
    // no development credit by itself; it is selected only when it unlocks a
    // higher-value same-turn action or reviewed route.
    if card_is_current_turn_consumable_mana(card) {
        // Casting a whole-hand-discard source before a random discard protects
        // that physical source without activating it. One minimal point
        // breaks the otherwise false tie against casting Gamble first; the
        // later queued action still has to pay the exact discard and
        // self-sacrifice costs.
        return i64::from(exact_discard_sacrifice_mana_ability(card).is_some());
    }
    let executable_mana_role = card_has_executable_planner_mana_role(card);
    let executable_tutor_role = card_has_executable_planner_tutor_role(card);
    let policy_bias = match policy {
        PilotPolicy::Race => {
            i64::from(
                card.has(role::COMBO_PIECE)
                    || card.has(role::FAST_MANA) && executable_mana_role
                    || card.has(role::TUTOR) && executable_tutor_role,
            ) * 18
        }
        PilotPolicy::Balanced => i64::from(card.has(role::ENGINE | role::DRAW)) * 12,
        PilotPolicy::Protect => i64::from(card.has(role::ENGINE | role::PROTECTION)) * 10,
    };
    let is_mana_source = card.has(role::MANA_SOURCE) && executable_mana_role
        || !matches!(
            card.effects.mana_production_kind,
            ManaProductionKind::None | ManaProductionKind::Unsupported
        )
        || mana_access
            .and_then(|access| access.source(card_index))
            .is_some();
    // This is only a planner preference for a conservative next-turn
    // resource. The executor still requires the creature to have entered
    // before the current turn before it may be reserved as an attacker, so
    // this never invents haste or a same-turn tap. Mana-source Dwarves are
    // excluded because the runtime already reserves them for their typed mana
    // source and cannot also attack with the same object.
    let future_dwarf_treasure_development = i64::from(
        ability_context.dwarf_treasure_per_tap > 0
            && card.effects.card_types.is_creature
            && card_has_subtype(card, "Dwarf")
            && !card_has_keyword(card, "Defender")
            && !is_mana_source,
    ) * 32;
    policy_bias
        + i64::from(card.has(role::RAMP | role::FAST_MANA) && executable_mana_role) * 42
        + i64::from(card.has(role::TUTOR) && executable_tutor_role) * 36
        + i64::from(card.has(role::DRAW)) * 28
        + i64::from(card.has(role::ENGINE)) * 25
        + i64::from(card.has(role::ENABLER | role::PAYOFF)) * 20
        + i64::from(card.has(role::COMBO_PIECE | role::WIN_CONDITION)) * 16
        + i64::from(immediate_cards_drawn(card)) * 10
        + match compile_typed_burst_card_access_program(card) {
            Some(TypedBurstCardAccessProgram::WholeHandDiscardThenDraw) => 70,
            Some(TypedBurstCardAccessProgram::RepeatableTopCardReveal) => 90,
            None => 0,
        }
        + i64::from(compile_typed_necropotence_lifecycle(card).is_some()) * 85
        + future_dwarf_treasure_development
}

fn mana_color_code(colors: ManaColorMask) -> u8 {
    [
        ManaColorMask::WHITE,
        ManaColorMask::BLUE,
        ManaColorMask::BLACK,
        ManaColorMask::RED,
        ManaColorMask::GREEN,
        ManaColorMask::COLORLESS,
    ]
    .into_iter()
    .enumerate()
    .fold(0u8, |bits, (index, color)| {
        bits | (u8::from(colors.intersects(color)) << index)
    })
}

fn mana_behavior_code(behavior: BattlefieldManaBehavior) -> u16 {
    match behavior {
        BattlefieldManaBehavior::Fixed => 0,
        BattlefieldManaBehavior::AnyColorAmongControlledLegendaryCreaturesAndPlaneswalkers => 1,
        BattlefieldManaBehavior::AnyColorWithMetalcraft => 2,
        BattlefieldManaBehavior::LinkedCardColors(colors) => {
            0x100 | u16::from(mana_color_code(colors))
        }
    }
}

fn compile_line_activation_cost(line: &crate::domain::KnownLine) -> CompiledLineActivationCost {
    if line
        .simulation_requirements
        .contains(&LineRequirement::TotalExecutionMana)
    {
        return CompiledLineActivationCost::Unmodeled;
    }
    let named_cards_pay_printed_costs = line
        .simulation_requirements
        .contains(&LineRequirement::NamedCardsPayPrintedCosts);

    let mut explicit_costs = line
        .simulation_requirements
        .iter()
        .filter_map(|requirement| match requirement {
            LineRequirement::AdditionalActivationMana { cost } => Some(*cost),
            _ => None,
        });
    let explicit_cost = explicit_costs.next();
    if explicit_costs.next().is_some() {
        return CompiledLineActivationCost::Unmodeled;
    }

    match (
        named_cards_pay_printed_costs,
        line.mana_needed.as_deref(),
        explicit_cost,
    ) {
        (true, _, None) => CompiledLineActivationCost::None,
        (true, _, Some(_)) => CompiledLineActivationCost::Unmodeled,
        (false, None, None) => CompiledLineActivationCost::None,
        (false, reported, Some(cost))
            if reported.is_none_or(|reported| reported.trim() == cost.trim()) =>
        {
            CompiledLineActivationCost::Additional(parse_mana_cost(Some(cost)))
        }
        // A raw catalog value without an explicit basis, or a disagreement
        // between reported and activation mana, cannot be safely sequenced.
        _ => CompiledLineActivationCost::Unmodeled,
    }
}

fn source_option_count(colors: ManaColorMask) -> u8 {
    [
        ManaColorMask::WHITE,
        ManaColorMask::BLUE,
        ManaColorMask::BLACK,
        ManaColorMask::RED,
        ManaColorMask::GREEN,
        ManaColorMask::COLORLESS,
    ]
    .into_iter()
    .filter(|color| colors.intersects(*color))
    .count() as u8
}

fn source_damage_for_spend(source: PoolManaSource, accepted_colors: Option<ManaColorMask>) -> u8 {
    if source.first_spend_recorded {
        return 0;
    }
    let can_choose_damage_free_mode = source
        .colors
        .intersects(source.damage_free_colors_on_first_spend)
        && accepted_colors
            .is_none_or(|colors| colors.intersects(source.damage_free_colors_on_first_spend));
    if can_choose_damage_free_mode {
        0
    } else {
        source.source_damage_on_first_spend
    }
}

/// CR 601.2b announces a monocolored-hybrid choice before CR 601.2f applies
/// cost increases and reductions. Expand those announced choices into exact
/// nonhybrid faces so a generic reduction can legally reduce the generic half
/// of a symbol such as {2/W}.
fn adjusted_announced_cost_faces(
    face: &ManaCostFace,
    additional_generic: u8,
    generic_reduction: u16,
) -> Vec<ManaCostFace> {
    let mut monocolored_hybrid_positions = face
        .pips
        .iter()
        .enumerate()
        .filter_map(|(position, pip)| {
            (pip.is_hybrid && pip.generic_value.is_some() && source_option_count(pip.colors) == 1)
                .then_some(position)
        })
        .collect::<Vec<_>>();
    // Oracle mana costs are bounded in practice, but fail closed instead of
    // allowing an adversarial profile to explode this exact branch set.
    if monocolored_hybrid_positions.len() > 12 {
        return Vec::new();
    }
    monocolored_hybrid_positions.reverse();

    let mut variants = vec![face.clone()];
    for position in monocolored_hybrid_positions {
        let mut next = Vec::with_capacity(variants.len().saturating_mul(2));
        for variant in variants {
            let Some(pip) = variant.pips.get(position) else {
                continue;
            };
            let generic_alternative = pip.generic_value.unwrap_or(2);

            let mut colored = variant.clone();
            if let Some(colored_pip) = colored.pips.get_mut(position) {
                colored_pip.generic_value = None;
            }
            next.push(colored);

            let mut generic = variant;
            generic.pips.remove(position);
            generic.generic_value = generic.generic_value.saturating_add(generic_alternative);
            next.push(generic);
        }
        variants = next;
    }

    for variant in &mut variants {
        variant.generic_value = variant
            .generic_value
            .saturating_add(u16::from(additional_generic))
            .saturating_sub(generic_reduction);
    }
    variants
}

fn pay_requirements(
    pool: &mut TurnManaPool,
    requirements: &[(ManaColorMask, u8)],
    position: usize,
    generic_due: u8,
    tap_constraint: Option<&TapCandidatePaymentConstraint>,
) -> bool {
    let damage_ceiling = pool.pending_source_damage;
    let mut damage_free_candidate = pool.clone();
    if pay_requirements_with_damage_ceiling(
        &mut damage_free_candidate,
        requirements,
        position,
        generic_due,
        Some(damage_ceiling),
        tap_constraint,
    ) {
        *pool = damage_free_candidate;
        return true;
    }
    pay_requirements_with_damage_ceiling(
        pool,
        requirements,
        position,
        generic_due,
        None,
        tap_constraint,
    )
}

fn pay_requirements_with_damage_ceiling(
    pool: &mut TurnManaPool,
    requirements: &[(ManaColorMask, u8)],
    position: usize,
    generic_due: u8,
    damage_ceiling: Option<u8>,
    tap_constraint: Option<&TapCandidatePaymentConstraint>,
) -> bool {
    if position == requirements.len() {
        let mut candidate = pool.clone();
        if !pay_generic_with_constraint(&mut candidate, generic_due, tap_constraint)
            || damage_ceiling.is_some_and(|ceiling| candidate.pending_source_damage > ceiling)
            || tap_constraint.is_some_and(|constraint| !constraint.admits(&candidate))
        {
            return false;
        }
        *pool = candidate;
        return true;
    }
    let (colors, generic_alternative) = requirements[position];
    let mut source_indices = (0..pool.sources.len())
        .filter(|source_index| {
            let source = &pool.sources[*source_index];
            source.remaining > 0 && source.colors.intersects(colors)
        })
        .collect::<Vec<_>>();
    source_indices.sort_by_key(|source_index| {
        let source = &pool.sources[*source_index];
        (
            source_damage_for_spend(*source, Some(colors)),
            tap_constraint.is_some_and(|constraint| constraint.source_is_candidate(source)),
            source.is_treasure,
            source_option_count(source.colors),
            Reverse(source.remaining),
        )
    });
    for source_index in source_indices {
        let source = pool.sources[source_index];
        if damage_ceiling.is_some_and(|ceiling| {
            pool.pending_source_damage
                .saturating_add(source_damage_for_spend(source, Some(colors)))
                > ceiling
        }) {
            continue;
        }
        let color_choices = if source.same_type_coupled && source_option_count(source.colors) > 1 {
            [
                ManaColorMask::WHITE,
                ManaColorMask::BLUE,
                ManaColorMask::BLACK,
                ManaColorMask::RED,
                ManaColorMask::GREEN,
                ManaColorMask::COLORLESS,
            ]
            .into_iter()
            .filter(|color| source.colors.intersects(*color) && colors.intersects(*color))
            .collect::<Vec<_>>()
        } else {
            vec![source.colors]
        };
        for chosen_color in color_choices {
            let mut candidate = pool.clone();
            if candidate.sources[source_index].same_type_coupled {
                // A modifier such as Kinnan adds the same type of mana that
                // the permanent produced. Once one colored unit is committed,
                // every coupled unit from that activation is locked to it.
                candidate.sources[source_index].colors = chosen_color;
            }
            let accepted_colors = if candidate.sources[source_index].same_type_coupled {
                chosen_color
            } else {
                colors
            };
            candidate.spend_from_source(source_index, 1, Some(accepted_colors));
            if pay_requirements_with_damage_ceiling(
                &mut candidate,
                requirements,
                position + 1,
                generic_due,
                damage_ceiling,
                tap_constraint,
            ) {
                *pool = candidate;
                return true;
            }
        }
    }
    if generic_alternative > 0 {
        let next_generic = generic_due.saturating_add(generic_alternative);
        return pay_requirements_with_damage_ceiling(
            pool,
            requirements,
            position + 1,
            next_generic,
            damage_ceiling,
            tap_constraint,
        );
    }
    false
}

fn pay_generic_with_constraint(
    pool: &mut TurnManaPool,
    amount: u8,
    tap_constraint: Option<&TapCandidatePaymentConstraint>,
) -> bool {
    let Some(tap_constraint) = tap_constraint else {
        return pool.pay_generic(amount);
    };
    if amount == 0 {
        return tap_constraint.admits(pool);
    }
    if pool.total() < amount {
        return false;
    }

    let mut source_indices = (0..pool.sources.len())
        .filter(|source_index| pool.sources[*source_index].remaining > 0)
        .collect::<Vec<_>>();
    source_indices.sort_by_key(|source_index| {
        let source = &pool.sources[*source_index];
        (
            source_damage_for_spend(*source, None),
            tap_constraint.source_is_candidate(source),
            Reverse(source.remaining),
            source_option_count(source.colors),
            source.is_treasure,
        )
    });
    for source_index in source_indices {
        let mut candidate = pool.clone();
        candidate.spend_from_source(source_index, 1, None);
        if pay_generic_with_constraint(&mut candidate, amount - 1, Some(tap_constraint)) {
            *pool = candidate;
            return true;
        }
    }
    false
}

fn source_weights(source: &ManaSourceProfile, required_colors: &[ManaColorMask]) -> Vec<f32> {
    required_colors
        .iter()
        .map(|color| {
            if source.colors.intersects(*color) {
                source.reliability
            } else {
                0.0
            }
        })
        .collect()
}

fn normalize_mana_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Debug, Default)]
struct ScenarioAggregate {
    threat_turns: Vec<u8>,
    win_attempt_turns: Vec<u8>,
    model_pace_turns: Vec<u8>,
    generic_conversion_milestone_turns: Vec<u8>,
    resolved_table_win_turns: Vec<u8>,
    explicit_route_attempt_turns: HashMap<usize, Vec<u8>>,
    generic_milestone_kind_counts: HashMap<GenericMilestoneKind, u32>,
    early_turn_blocker_counts:
        HashMap<(u8, Option<usize>, Option<u8>, ExplicitAttemptBlockerReason), u32>,
    first_attempt_opportunities: u32,
    stopped_attempts: u32,
    recovered_attempts: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct EpisodeTimingProvenance {
    first_explicit_route_index: Option<usize>,
    first_generic_milestone_turn: Option<u8>,
    first_generic_milestone_kind: Option<GenericMilestoneKind>,
    early_turn_blockers: [Option<EpisodeAttemptBlocker>; EARLY_ATTEMPT_DIAGNOSTIC_HORIZON as usize],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EpisodeAttemptBlocker {
    line_index: Option<usize>,
    missing_card_position: Option<u8>,
    reason: ExplicitAttemptBlockerReason,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct EpisodeOutcome {
    threat_turn: Option<u8>,
    first_win_attempt_turn: Option<u8>,
    resolved_table_win_turn: Option<u8>,
    timing_provenance: EpisodeTimingProvenance,
    first_attempt_opportunity: bool,
    first_attempt_stopped: bool,
    recovered: bool,
    #[allow(dead_code)]
    final_life: f32,
    #[allow(dead_code)]
    player_died: bool,
}

/// A resolved-table-win timestamp may only be supplied by a strict typed
/// executor. Structural line metadata, catalog lethality, and heuristic
/// engine/combat states never manufacture this proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StrictTableWinResolutionProof {
    line_index: usize,
    turn: u8,
}

/// Ephemeral receipt for an exact Oracle/exiler sequence. It contributes an
/// attempt while interaction is pending, but only its private finalizer can
/// promote the episode to a resolved table win.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingStrictWinCandidate {
    line_index: usize,
    turn: u8,
    oracle_sequence: u16,
    exile_spell_sequence: u16,
    library_receipt: ReviewedLibraryExileReceipt,
}

impl PendingStrictWinCandidate {
    fn finalize(
        self,
        ready_line_index: Option<usize>,
        turn: u8,
    ) -> Option<StrictTableWinResolutionProof> {
        (ready_line_index == Some(self.line_index)
            && turn == self.turn
            && self.exile_spell_sequence == self.oracle_sequence.saturating_add(1)
            && self.library_receipt.exiled_library_cards
                == self.library_receipt.starting_library_cards
            && self.library_receipt.remaining_library_cards == 0)
            .then_some(StrictTableWinResolutionProof {
                line_index: self.line_index,
                turn: self.turn,
            })
    }
}

/// Chooses the single explicit endpoint presented to the interaction model.
///
/// A combat attack has already crossed the rules-backed presentation boundary,
/// while a strict line is still a staged transaction until its private
/// finalizer produces a proof. Structural line readiness remains attempt-only
/// fallback evidence. Keeping this priority here prevents a pending line from
/// discarding an independently presented lethal attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExplicitAttemptSelection {
    CombatDamage,
    Strict(StrictTableWinResolutionProof),
    StructuralLine(usize),
}

impl ExplicitAttemptSelection {
    fn route_index(self) -> usize {
        match self {
            Self::CombatDamage => COMBAT_DAMAGE_ROUTE_INDEX,
            Self::Strict(proof) => proof.line_index,
            Self::StructuralLine(line_index) => line_index,
        }
    }
}

fn select_explicit_attempt(
    combat_table_lethal_ready: bool,
    strict_resolution_proof: Option<StrictTableWinResolutionProof>,
    structural_table_lethal_line: Option<usize>,
) -> Option<ExplicitAttemptSelection> {
    if combat_table_lethal_ready {
        Some(ExplicitAttemptSelection::CombatDamage)
    } else if let Some(proof) = strict_resolution_proof {
        Some(ExplicitAttemptSelection::Strict(proof))
    } else {
        structural_table_lethal_line.map(ExplicitAttemptSelection::StructuralLine)
    }
}

fn stage_reviewed_empty_library_win_candidate(
    deck: &CompiledDeck,
    library_order: &[usize],
    next_draw_position: usize,
    zones: &KnownLineZoneState,
    turn: u8,
) -> Option<PendingStrictWinCandidate> {
    for (line_index, line) in deck.known_lines.iter().enumerate() {
        let Some(program) = compile_reviewed_empty_library_win_program(line_index, line, deck)
        else {
            continue;
        };
        let Some(oracle) = zones
            .battlefield
            .iter()
            .filter(|presence| {
                presence.card_index == program.oracle_card_index && presence.entered_turn == turn
            })
            .max_by_key(|presence| presence.sequence)
        else {
            continue;
        };
        let Some(exile_spell) = zones
            .spells_cast_this_turn
            .iter()
            .filter(|cast| {
                cast.card_index == program.exile_spell_card_index
                    && cast.turn == turn
                    && cast.sequence == oracle.sequence.saturating_add(1)
            })
            .min_by_key(|cast| cast.sequence)
        else {
            continue;
        };

        let mut staged_library = library_order.to_vec();
        let mut staged_exile = zones.exile.clone();
        let Some(library_receipt) = execute_reviewed_library_exile_transaction(
            program,
            deck,
            &mut staged_library,
            next_draw_position,
            &mut staged_exile,
        ) else {
            continue;
        };
        return Some(PendingStrictWinCandidate {
            line_index,
            turn,
            oracle_sequence: oracle.sequence,
            exile_spell_sequence: exile_spell.sequence,
            library_receipt,
        });
    }
    None
}

#[derive(Debug, Default, Clone)]
struct IsolatedScenarioTrace {
    events: ScenarioEventCounters,
    recovery_turn: Option<u8>,
    opportunity_mask: ScenarioOpportunityMask,
}

#[derive(Debug, Default, Clone, Copy)]
struct ScenarioOpportunityMask(u16);

impl ScenarioOpportunityMask {
    fn bit(scenario: InteractionScenario) -> u16 {
        match scenario {
            InteractionScenario::TargetedPermanentRemoval => 1 << 0,
            InteractionScenario::CommanderRemovalRecast => 1 << 1,
            InteractionScenario::FirstRelevantSpellCountered => 1 << 2,
            InteractionScenario::CreatureWipe => 1 << 3,
            InteractionScenario::GraveyardShutdown => 1 << 4,
            InteractionScenario::GenericTaxStax => 1 << 5,
            InteractionScenario::RuleOfLawCap => 1 << 6,
            InteractionScenario::FirstWinAttemptStopped => 1 << 7,
        }
    }

    fn record(&mut self, scenario: InteractionScenario) {
        self.0 |= Self::bit(scenario);
    }

    fn contains(self, scenario: InteractionScenario) -> bool {
        self.0 & Self::bit(scenario) != 0
    }

    fn proves_absent(self, scenario: InteractionScenario) -> bool {
        matches!(
            scenario,
            InteractionScenario::TargetedPermanentRemoval
                | InteractionScenario::CommanderRemovalRecast
                | InteractionScenario::FirstRelevantSpellCountered
                | InteractionScenario::CreatureWipe
                | InteractionScenario::GraveyardShutdown
                | InteractionScenario::GenericTaxStax
                | InteractionScenario::RuleOfLawCap
                | InteractionScenario::FirstWinAttemptStopped
        ) && !self.contains(scenario)
    }
}

#[derive(Debug)]
struct EpisodeSimulationResult {
    outcome: EpisodeOutcome,
    scenario_trace: IsolatedScenarioTrace,
}

#[derive(Debug)]
struct PreparedEpisode {
    opening: LondonHandSample,
    rng_after_opening: ChaCha8Rng,
    opponent_timeline: OpponentEventTimeline,
    table_activity_timeline: TableActivityTimeline,
}

#[derive(Debug)]
struct PreparedBaselineEpisode {
    preparation: PreparedEpisode,
    outcome: EpisodeOutcome,
    opportunity_mask: ScenarioOpportunityMask,
}

#[derive(Debug)]
struct SimulationRuntimeModel {
    line_activation_costs: Vec<CompiledLineActivationCost>,
}

impl SimulationRuntimeModel {
    fn compile(deck: &CompiledDeck) -> Self {
        Self {
            line_activation_costs: deck
                .known_lines
                .iter()
                .map(compile_line_activation_cost)
                .collect(),
        }
    }
}

#[derive(Debug)]
enum ConditionWorkResult {
    Interfered {
        outcome: EpisodeOutcome,
    },
    Scenario {
        collector_index: usize,
        episode: ScenarioEpisodeInput,
    },
}

#[derive(Debug)]
struct ScenarioSuiteCollector {
    scenario: InteractionScenario,
    applicability: ScenarioApplicability,
    episodes: Vec<ScenarioEpisodeInput>,
}

#[derive(Debug)]
struct IsolatedScenarioRuntime {
    scenario: Option<InteractionScenario>,
    trace: IsolatedScenarioTrace,
    applied_turn: Option<u8>,
}

impl IsolatedScenarioRuntime {
    fn new(scenario: Option<InteractionScenario>) -> Self {
        Self {
            scenario,
            trace: IsolatedScenarioTrace::default(),
            applied_turn: None,
        }
    }

    fn is(&self, scenario: InteractionScenario) -> bool {
        self.scenario == Some(scenario)
    }

    fn applied(&self) -> bool {
        self.applied_turn.is_some()
    }

    fn observe_opportunity(&mut self, scenario: InteractionScenario) {
        self.trace.opportunity_mask.record(scenario);
    }

    fn activate(&mut self, turn: u8, affected_events: u32) -> bool {
        debug_assert!(
            self.scenario
                .is_some_and(|scenario| self.trace.opportunity_mask.contains(scenario)),
            "an isolated scenario must record its checkpoint before activation"
        );
        if self.applied() {
            return false;
        }
        self.trace.events.checkpoint_matches = 1;
        self.trace.events.opportunities = 1;
        self.trace.events.directive_attempts = 1;
        self.trace.events.directive_applied = 1;
        self.trace.events.affected_game_events = affected_events.max(1);
        self.applied_turn = Some(turn);
        true
    }

    fn add_affected_event(&mut self) {
        if self.applied() {
            self.trace.events.affected_game_events =
                self.trace.events.affected_game_events.saturating_add(1);
        }
    }

    fn recover(&mut self, turn: u8) {
        if self.applied() && self.trace.recovery_turn.is_none() {
            self.trace.recovery_turn = Some(turn);
        }
    }

    fn finish(self, outcome: EpisodeOutcome) -> EpisodeSimulationResult {
        EpisodeSimulationResult {
            outcome,
            scenario_trace: self.trace,
        }
    }
}

fn rule_of_law_blocks_next_spell(
    isolated: &mut IsolatedScenarioRuntime,
    turn: u8,
    spells_cast_this_turn: u8,
) -> bool {
    if spells_cast_this_turn < 1 {
        return false;
    }
    isolated.observe_opportunity(InteractionScenario::RuleOfLawCap);
    if !isolated.is(InteractionScenario::RuleOfLawCap) {
        return false;
    }
    if !isolated.applied() {
        isolated.activate(turn, 1);
    } else {
        isolated.add_affected_event();
    }
    true
}

pub fn simulate_opening_hands_with_mana(
    deck: &CompiledDeck,
    mana: &ManaModel,
    options: &AnalysisOptions,
    seed: u64,
    cancellation: &AtomicBool,
    report_progress: impl FnMut(u32, u32),
) -> Result<OpeningHandReport, SimulationError> {
    simulate_opening_hands_inner(
        deck,
        Some(mana),
        options,
        seed,
        cancellation,
        report_progress,
    )
}

fn simulate_opening_hands_inner(
    deck: &CompiledDeck,
    mana: Option<&ManaModel>,
    options: &AnalysisOptions,
    seed: u64,
    cancellation: &AtomicBool,
    report_progress: impl FnMut(u32, u32),
) -> Result<OpeningHandReport, SimulationError> {
    let simulations = options.opening_hand_simulations.clamp(100, 100_000);
    simulate_opening_hands_inner_with_worker_count(
        deck,
        mana,
        options,
        seed,
        cancellation,
        episode_worker_count(simulations),
        report_progress,
    )
}

fn simulate_opening_hands_inner_with_worker_count(
    deck: &CompiledDeck,
    mana: Option<&ManaModel>,
    options: &AnalysisOptions,
    seed: u64,
    cancellation: &AtomicBool,
    worker_count: usize,
    mut report_progress: impl FnMut(u32, u32),
) -> Result<OpeningHandReport, SimulationError> {
    if deck.library.len() < 7 {
        return Err(SimulationError::LibraryTooSmall);
    }

    let mana_access = mana.map(|model| ManaAccessProfile::compile(deck, model));
    let simulations = options.opening_hand_simulations.clamp(100, 100_000);
    let mut keepable_seven = 0u32;
    let mut keepable_after = 0u32;
    let mut total_mulligans = 0u64;
    let mut total_cards_kept = 0u64;
    let mut two_land = 0u32;
    let mut three_land_by_turn_three = 0u32;
    let mut ramp_access = 0u32;
    let mut engine_access = 0u32;
    let mut opening_color_coverage = 0.0f64;
    let mut turn_three_color_coverage = 0.0f64;
    let mut candidate_cohort = OpeningCandidateCohort::new(seed, simulations, deck.library.len());
    let reporting_interval = (simulations / 20).max(25);
    let mut batch_start = 0u32;
    while batch_start < simulations {
        if cancellation.load(Ordering::Relaxed) {
            return Err(SimulationError::Cancelled);
        }
        let batch_count = (simulations - batch_start).min(OPENING_EPISODE_BATCH_SIZE);
        let batch_results = ordered_parallel_episode_map(
            batch_count,
            worker_count,
            cancellation,
            |batch_completed| {
                let completed = batch_start + batch_completed;
                if completed.is_multiple_of(reporting_interval) || completed == simulations {
                    report_progress(completed, simulations);
                }
            },
            |batch_index| {
                let simulation_index = batch_start + batch_index;
                Ok(simulate_opening_episode(
                    deck,
                    mana_access.as_ref(),
                    options,
                    seed,
                    simulation_index,
                ))
            },
        )?;

        for result in batch_results {
            for (attempt, order) in result.candidate_orders.iter().enumerate() {
                candidate_cohort.record(deck, result.simulation_index, attempt as u8, order);
            }
            keepable_seven += u32::from(result.initial_keepable);
            keepable_after += u32::from(result.accepted_by_policy);
            total_mulligans += u64::from(result.paid_mulligans);
            total_cards_kept += result.cards_kept as u64;
            opening_color_coverage += f64::from(result.kept_evaluation.color_coverage);
            two_land += u32::from(result.kept_evaluation.lands >= 2);
            ramp_access += u32::from(result.kept_evaluation.ramp > 0);
            engine_access += u32::from(
                result.kept_evaluation.command_zone_plan_access
                    || result.kept_evaluation.engines > 0
                    || result.kept_evaluation.tutors > 0,
            );
            turn_three_color_coverage += f64::from(result.turn_three_evaluation.color_coverage);
            three_land_by_turn_three += u32::from(result.turn_three_evaluation.lands >= 3);
        }
        batch_start += batch_count;
    }

    let denominator = simulations as f32;
    let keep_rate = keepable_after as f32 / denominator;
    let confidence_margin = 1.96 * (keep_rate * (1.0 - keep_rate) / denominator).sqrt();
    let mut mana_report = mana.map(analysis_report).unwrap_or_default();
    if mana_access
        .as_ref()
        .is_some_and(|profile| !profile.required_colors.is_empty())
    {
        mana_report.average_opening_color_coverage =
            (opening_color_coverage / simulations as f64) as f32;
        mana_report.average_turn_three_color_coverage =
            (turn_three_color_coverage / simulations as f64) as f32;
    }

    Ok(OpeningHandReport {
        simulations,
        candidate_cohort_version: OPENING_CANDIDATE_COHORT_VERSION.into(),
        candidate_cohort_sha256: candidate_cohort.finish(),
        keepable_seven_rate: keepable_seven as f32 / denominator,
        keepable_after_mulligans_rate: keep_rate,
        average_mulligans: total_mulligans as f32 / denominator,
        average_cards_kept: total_cards_kept as f32 / denominator,
        two_land_rate: two_land as f32 / denominator,
        three_land_by_turn_three_rate: three_land_by_turn_three as f32 / denominator,
        ramp_access_rate: ramp_access as f32 / denominator,
        engine_access_rate: engine_access as f32 / denominator,
        confidence_margin,
        policy_fidelity: crate::domain::SimulationFidelity::LegacyHeuristic,
        mana: mana_report,
    })
}

fn bounded_episode_worker_count(available_parallelism: usize, episode_count: u32) -> usize {
    let reserved_capacity = if available_parallelism <= 8 { 1 } else { 2 };
    available_parallelism
        .saturating_sub(reserved_capacity)
        .clamp(1, MAX_EPISODE_WORKERS)
        .min(episode_count.max(1) as usize)
}

fn episode_worker_count(episode_count: u32) -> usize {
    let available_parallelism = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    bounded_episode_worker_count(available_parallelism, episode_count)
}

/// Evaluate independent, deterministically indexed episodes concurrently,
/// then restore strict index order before the caller aggregates any results.
///
/// Progress is emitted by the coordinator as a deterministic completed count;
/// worker scheduling can therefore affect only elapsed time, never seeds,
/// aggregation order, or report contents.
fn ordered_parallel_episode_map<T>(
    episode_count: u32,
    worker_count: usize,
    cancellation: &AtomicBool,
    mut report_completed: impl FnMut(u32),
    evaluate: impl Fn(u32) -> Result<T, SimulationError> + Sync,
) -> Result<Vec<T>, SimulationError>
where
    T: Send,
{
    if episode_count == 0 {
        return Ok(Vec::new());
    }
    let worker_count = worker_count.clamp(1, episode_count as usize);
    if worker_count == 1 {
        let mut ordered = Vec::with_capacity(episode_count as usize);
        for episode_index in 0..episode_count {
            if cancellation.load(Ordering::Relaxed) {
                return Err(SimulationError::Cancelled);
            }
            ordered.push(evaluate(episode_index)?);
            report_completed(episode_index + 1);
        }
        return Ok(ordered);
    }

    let next_episode = AtomicU32::new(0);
    std::thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel::<(u32, Result<T, SimulationError>)>();

        for _ in 0..worker_count {
            let next_episode = &next_episode;
            let sender = sender.clone();
            let evaluate = &evaluate;
            scope.spawn(move || {
                loop {
                    if cancellation.load(Ordering::Relaxed) {
                        break;
                    }
                    let episode_index = next_episode.fetch_add(1, Ordering::Relaxed);
                    if episode_index >= episode_count {
                        break;
                    }
                    let result = evaluate(episode_index);
                    if sender.send((episode_index, result)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);

        let mut ordered = (0..episode_count).map(|_| None).collect::<Vec<Option<T>>>();
        let mut completed = 0u32;
        while completed < episode_count {
            if cancellation.load(Ordering::Relaxed) {
                return Err(SimulationError::Cancelled);
            }
            let (episode_index, result) =
                receiver.recv().map_err(|_| SimulationError::Cancelled)?;
            ordered[episode_index as usize] = Some(result?);
            completed += 1;
            report_completed(completed);
        }

        Ok(ordered
            .into_iter()
            .map(|episode| episode.expect("every indexed episode was received"))
            .collect())
    })
}

pub fn simulate_win_speed_with_mana(
    deck: &CompiledDeck,
    mana: &ManaModel,
    options: &AnalysisOptions,
    seed: u64,
    cancellation: &AtomicBool,
    report_progress: impl FnMut(bool, u32, u32),
) -> Result<WinSpeedReport, SimulationError> {
    let mana_access = ManaAccessProfile::compile(deck, mana);
    simulate_win_speed_inner(
        deck,
        Some(&mana_access),
        options,
        seed,
        cancellation,
        report_progress,
    )
}

fn simulate_win_speed_inner(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    options: &AnalysisOptions,
    seed: u64,
    cancellation: &AtomicBool,
    report_progress: impl FnMut(bool, u32, u32),
) -> Result<WinSpeedReport, SimulationError> {
    let simulations = options.game_simulations.clamp(100, 50_000);
    simulate_win_speed_inner_with_worker_count(
        deck,
        mana_access,
        options,
        seed,
        cancellation,
        episode_worker_count(simulations),
        report_progress,
    )
}

fn simulate_win_speed_inner_with_worker_count(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    options: &AnalysisOptions,
    seed: u64,
    cancellation: &AtomicBool,
    worker_count: usize,
    mut report_progress: impl FnMut(bool, u32, u32),
) -> Result<WinSpeedReport, SimulationError> {
    if deck.library.len() < 7 {
        return Err(SimulationError::LibraryTooSmall);
    }
    let simulations = options.game_simulations.clamp(100, 50_000);
    let runtime_model = SimulationRuntimeModel::compile(deck);
    let mut baseline = ScenarioAggregate::default();
    let mut interfered = ScenarioAggregate::default();
    let mut interfered_outcomes = Vec::with_capacity(simulations as usize);
    let scenario_simulations = simulations.min(MAX_INTERACTION_SCENARIO_EPISODES);
    let mut scenario_collectors = InteractionScenario::ALL
        .into_iter()
        .map(|scenario| ScenarioSuiteCollector {
            scenario,
            applicability: scenario_applicability(deck, scenario),
            episodes: Vec::with_capacity(scenario_simulations as usize),
        })
        .collect::<Vec<_>>();
    let reporting_interval = (simulations / 20).max(25);

    let baseline_episodes = ordered_parallel_episode_map(
        simulations,
        worker_count,
        cancellation,
        |completed| {
            if completed.is_multiple_of(reporting_interval) || completed == simulations {
                report_progress(false, completed, simulations);
            }
        },
        |simulation_index| {
            let episode_seed =
                derive_episode_seed(seed, WIN_SPEED_EPISODE_SEED_DOMAIN, simulation_index);
            let preparation = prepare_episode(deck, mana_access, options, episode_seed);
            let baseline_result = simulate_prepared_episode_condition(
                deck,
                mana_access,
                options,
                InteractionProfile::None,
                None,
                &preparation,
                &runtime_model,
            );
            Ok(PreparedBaselineEpisode {
                preparation,
                outcome: baseline_result.outcome,
                opportunity_mask: baseline_result.scenario_trace.opportunity_mask,
            })
        },
    )?;
    let baseline_outcomes = baseline_episodes
        .iter()
        .map(|episode| episode.outcome)
        .collect::<Vec<_>>();
    for outcome in &baseline_outcomes {
        collect_outcome(&mut baseline, *outcome);
    }

    let applicable_scenarios = scenario_collectors
        .iter()
        .enumerate()
        .filter_map(|(collector_index, collector)| {
            matches!(collector.applicability, ScenarioApplicability::Applicable)
                .then_some((collector_index, collector.scenario))
        })
        .collect::<Vec<_>>();
    let scenario_work_items =
        scenario_simulations.saturating_mul(applicable_scenarios.len() as u32);
    let condition_work_items = simulations.saturating_add(scenario_work_items);
    let mut last_condition_progress = 0u32;
    let condition_results = ordered_parallel_episode_map(
        condition_work_items,
        worker_count,
        cancellation,
        |completed| {
            let virtual_completed = ((u64::from(completed) * u64::from(simulations))
                / u64::from(condition_work_items)) as u32;
            if virtual_completed == simulations
                || virtual_completed.saturating_sub(last_condition_progress) >= reporting_interval
            {
                last_condition_progress = virtual_completed;
                report_progress(true, virtual_completed, simulations);
            }
        },
        |work_index| {
            if work_index >= simulations {
                let scenario_work_index = work_index - simulations;
                let scenario_slot = scenario_work_index % applicable_scenarios.len() as u32;
                let simulation_index = scenario_work_index / applicable_scenarios.len() as u32;
                let (collector_index, scenario) = applicable_scenarios[scenario_slot as usize];
                let baseline_episode = &baseline_episodes[simulation_index as usize];
                let episode_seed =
                    derive_episode_seed(seed, WIN_SPEED_EPISODE_SEED_DOMAIN, simulation_index);
                let stressed = if baseline_episode.opportunity_mask.proves_absent(scenario) {
                    EpisodeSimulationResult {
                        outcome: baseline_episode.outcome,
                        scenario_trace: IsolatedScenarioTrace::default(),
                    }
                } else {
                    simulate_prepared_episode_condition(
                        deck,
                        mana_access,
                        options,
                        InteractionProfile::None,
                        Some(scenario),
                        &baseline_episode.preparation,
                        &runtime_model,
                    )
                };
                return Ok(ConditionWorkResult::Scenario {
                    collector_index,
                    episode: scenario_episode_input(
                        simulation_index,
                        episode_seed,
                        options.maximum_turn,
                        ScenarioApplicability::Applicable,
                        baseline_episode.outcome,
                        stressed,
                    ),
                });
            }

            let simulation_index = work_index;
            let baseline_episode = &baseline_episodes[simulation_index as usize];
            let outcome = simulate_prepared_episode_condition(
                deck,
                mana_access,
                options,
                options.interaction_profile,
                None,
                &baseline_episode.preparation,
                &runtime_model,
            )
            .outcome;
            Ok(ConditionWorkResult::Interfered { outcome })
        },
    )?;
    for result in condition_results {
        match result {
            ConditionWorkResult::Interfered { outcome } => {
                collect_outcome(&mut interfered, outcome);
                interfered_outcomes.push(outcome);
            }
            ConditionWorkResult::Scenario {
                collector_index,
                episode,
            } => {
                scenario_collectors[collector_index].episodes.push(episode);
            }
        }
    }
    for collector in &mut scenario_collectors {
        if matches!(collector.applicability, ScenarioApplicability::Applicable) {
            continue;
        }
        for simulation_index in 0..scenario_simulations {
            let episode_seed =
                derive_episode_seed(seed, WIN_SPEED_EPISODE_SEED_DOMAIN, simulation_index);
            let baseline_outcome = baseline_episodes[simulation_index as usize].outcome;
            collector.episodes.push(scenario_episode_input(
                simulation_index,
                episode_seed,
                options.maximum_turn,
                collector.applicability.clone(),
                baseline_outcome,
                EpisodeSimulationResult {
                    outcome: baseline_outcome,
                    scenario_trace: IsolatedScenarioTrace::default(),
                },
            ));
        }
    }

    let baseline_distribution = distribution(&baseline.threat_turns, simulations);
    let interfered_distribution = distribution(&interfered.threat_turns, simulations);
    let baseline_win_attempt = distribution(&baseline.win_attempt_turns, simulations);
    let interfered_win_attempt = distribution(&interfered.win_attempt_turns, simulations);
    let baseline_model_pace = distribution(&baseline.model_pace_turns, simulations);
    let interfered_model_pace = distribution(&interfered.model_pace_turns, simulations);
    let baseline_generic_conversion_milestone =
        distribution(&baseline.generic_conversion_milestone_turns, simulations);
    let interfered_generic_conversion_milestone =
        distribution(&interfered.generic_conversion_milestone_turns, simulations);
    let baseline_resolved_table_win = distribution(&baseline.resolved_table_win_turns, simulations);
    let interfered_resolved_table_win =
        distribution(&interfered.resolved_table_win_turns, simulations);
    let paired_threat_delay =
        paired_delay_report(&baseline_outcomes, &interfered_outcomes, |outcome| {
            outcome.threat_turn
        });
    let paired_win_attempt_delay =
        paired_delay_report(&baseline_outcomes, &interfered_outcomes, |outcome| {
            outcome.first_win_attempt_turn
        });
    let paired_resolved_table_win_delay =
        paired_delay_report(&baseline_outcomes, &interfered_outcomes, |outcome| {
            outcome.resolved_table_win_turn
        });
    let median_delay = paired_threat_delay.median;
    let win_attempt_median_delay = paired_win_attempt_delay.median;
    let resolved_table_win_median_delay = paired_resolved_table_win_delay.median;
    let cumulative_threat_rate =
        cumulative_rates(&baseline.threat_turns, simulations, options.maximum_turn);
    let cumulative_interfered_threat_rate =
        cumulative_rates(&interfered.threat_turns, simulations, options.maximum_turn);
    let cumulative_win_attempt_rate = cumulative_rates(
        &baseline.win_attempt_turns,
        simulations,
        options.maximum_turn,
    );
    let cumulative_interfered_win_attempt_rate = cumulative_rates(
        &interfered.win_attempt_turns,
        simulations,
        options.maximum_turn,
    );
    let cumulative_generic_conversion_milestone_rate = cumulative_rates(
        &baseline.generic_conversion_milestone_turns,
        simulations,
        options.maximum_turn,
    );
    let cumulative_interfered_generic_conversion_milestone_rate = cumulative_rates(
        &interfered.generic_conversion_milestone_turns,
        simulations,
        options.maximum_turn,
    );
    let cumulative_resolved_table_win_rate = cumulative_rates(
        &baseline.resolved_table_win_turns,
        simulations,
        options.maximum_turn,
    );
    let cumulative_interfered_resolved_table_win_rate = cumulative_rates(
        &interfered.resolved_table_win_turns,
        simulations,
        options.maximum_turn,
    );
    let attempt_denominator = interfered.first_attempt_opportunities.max(1) as f32;
    let recovery_by_max_turn_rate = (interfered.stopped_attempts > 0)
        .then_some(interfered.recovered_attempts as f32 / interfered.stopped_attempts as f32);
    let interaction_scenarios = finalize_scenario_suite(scenario_collectors, seed)?;
    let attempt_provenance = build_attempt_provenance_report(
        deck,
        &baseline,
        &interfered,
        simulations,
        options.maximum_turn,
    );

    let report = WinSpeedReport {
        simulations,
        fidelity: crate::domain::SimulationFidelity::LegacyHeuristic,
        fidelity_message: "Deterministic legacy trajectory estimate using a bounded observable-state planner, typed effect/ability programs where supported, and reviewed structural line witnesses. Only recognized table-lethal routes populate the explicit attempt endpoint; broad engine/combat density is serialized separately as a generic milestone. Resolved table wins remain censored without strict typed execution proof. This is not a complete Magic execution trace.".into(),
        coverage_manifest_sha256: None,
        timing_endpoint_version: Some(TIMING_ENDPOINT_VERSION.into()),
        baseline: baseline_distribution,
        interfered: interfered_distribution,
        baseline_win_attempt,
        interfered_win_attempt,
        baseline_model_pace,
        interfered_model_pace,
        baseline_resolved_table_win: Some(baseline_resolved_table_win),
        interfered_resolved_table_win: Some(interfered_resolved_table_win),
        median_delay,
        win_attempt_median_delay,
        resolved_table_win_median_delay,
        paired_threat_delay,
        paired_win_attempt_delay,
        paired_resolved_table_win_delay: Some(paired_resolved_table_win_delay),
        first_attempt_opportunities: interfered.first_attempt_opportunities,
        first_attempt_stopped_rate: interfered.stopped_attempts as f32 / attempt_denominator,
        recovery_opportunities: interfered.stopped_attempts,
        recovered_attempts: interfered.recovered_attempts,
        recovery_by_max_turn_rate,
        cumulative_threat_rate,
        cumulative_interfered_threat_rate,
        cumulative_win_attempt_rate,
        cumulative_interfered_win_attempt_rate,
        baseline_generic_conversion_milestone,
        interfered_generic_conversion_milestone,
        cumulative_generic_conversion_milestone_rate,
        cumulative_interfered_generic_conversion_milestone_rate,
        attempt_provenance,
        early_turn_evaluation: None,
        cumulative_resolved_table_win_rate: Some(cumulative_resolved_table_win_rate),
        cumulative_interfered_resolved_table_win_rate: Some(
            cumulative_interfered_resolved_table_win_rate,
        ),
        interaction_scenarios,
        stress_tests: build_stress_tests(deck, win_attempt_median_delay),
    };
    Ok(report)
}

fn scenario_episode_input(
    simulation_index: u32,
    episode_seed: u64,
    maximum_turn: u8,
    applicability: ScenarioApplicability,
    baseline: EpisodeOutcome,
    stressed: EpisodeSimulationResult,
) -> ScenarioEpisodeInput {
    let events = stressed.scenario_trace.events;
    let effectful = events.directive_applied > events.directive_no_ops;
    let recovery = effectful.then(|| {
        stressed.scenario_trace.recovery_turn.map_or(
            RecoveryObservation::RightCensored {
                at_turn: u16::from(maximum_turn),
            },
            |turn| RecoveryObservation::Recovered {
                turn: u16::from(turn),
            },
        )
    });
    ScenarioEpisodeInput {
        episode_id: format!("{simulation_index:06}-{episode_seed:016x}"),
        episode_seed,
        turn_cap: u16::from(maximum_turn),
        applicability,
        baseline: scenario_outcome_input(baseline, maximum_turn),
        stressed: scenario_outcome_input(stressed.outcome, maximum_turn),
        events,
        recovery,
    }
}

fn scenario_outcome_input(outcome: EpisodeOutcome, maximum_turn: u8) -> EpisodeOutcomeInput {
    EpisodeOutcomeInput {
        credible_threat: outcome.threat_turn.map_or(
            CensoredTurn::RightCensored {
                at_turn: u16::from(maximum_turn),
            },
            |turn| CensoredTurn::Observed {
                turn: u16::from(turn),
            },
        ),
        first_win_attempt: outcome.first_win_attempt_turn.map_or(
            CensoredTurn::RightCensored {
                at_turn: u16::from(maximum_turn),
            },
            |turn| CensoredTurn::Observed {
                turn: u16::from(turn),
            },
        ),
        resolved_table_win: Some(outcome.resolved_table_win_turn.map_or(
            CensoredTurn::RightCensored {
                at_turn: u16::from(maximum_turn),
            },
            |turn| CensoredTurn::Observed {
                turn: u16::from(turn),
            },
        )),
    }
}

fn finalize_scenario_suite(
    collectors: Vec<ScenarioSuiteCollector>,
    seed: u64,
) -> Result<Vec<CompactScenarioReport>, ScenarioReportError> {
    std::thread::scope(|scope| {
        let handles = collectors
            .into_iter()
            .map(|collector| {
                scope.spawn(move || {
                    let report = build_scenario_report(ScenarioReportInput::new(
                        collector.scenario,
                        ScenarioExecutionSource::ResponsePressure,
                        collector.episodes,
                    ))?;
                    Ok::<_, ScenarioReportError>(compact_scenario_report(
                        &report,
                        seed,
                        INTERACTION_SCENARIO_SEED_DERIVATION_VERSION,
                    ))
                })
            })
            .collect::<Vec<_>>();
        let mut compact_reports = Vec::with_capacity(handles.len());
        for handle in handles {
            compact_reports.push(
                handle
                    .join()
                    .expect("scenario report worker completed without panicking")?,
            );
        }
        Ok(compact_reports)
    })
}

pub fn estimate_commander_on_curve(
    deck: &CompiledDeck,
    opening: &OpeningHandReport,
    mana: &ManaAnalysisReport,
) -> f32 {
    let commander_cost = deck
        .commanders
        .iter()
        .filter_map(|index| deck.cards.get(*index))
        .map(|card| card.mana_value)
        .fold(f32::INFINITY, f32::min);
    if !commander_cost.is_finite() {
        return 0.0;
    }

    let land_base = opening.three_land_by_turn_three_rate;
    let ramp_help = opening.ramp_access_rate * if commander_cost >= 4.0 { 0.34 } else { 0.12 };
    let curve_penalty = ((commander_cost - 4.0).max(0.0) * 0.085).min(0.42);
    let raw_rate = (land_base + ramp_help - curve_penalty).clamp(0.0, 1.0);
    if mana.reliability_band == ManaReliabilityBand::Unknown {
        return raw_rate;
    }
    let color_support = (mana.reliability_score * 0.56
        + mana.average_turn_three_color_coverage * 0.44)
        .clamp(0.0, 1.0);
    (raw_rate * (0.58 + color_support * 0.42)).clamp(0.0, 1.0)
}

fn evaluate_cards(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    card_indices: &[usize],
) -> HandEvaluation {
    let mut evaluation = HandEvaluation::default();
    let remaining_library = remaining_library_counts(deck, card_indices);
    for index in card_indices {
        let card = &deck.cards[*index];
        if card.has(role::LAND) {
            evaluation.lands = evaluation.lands.saturating_add(1);
        }
        if card.has(role::RAMP)
            && card.mana_value <= 3.0
            && card_has_executable_opening_mana_role(card)
        {
            evaluation.ramp = evaluation.ramp.saturating_add(1);
        }
        if card.has(role::FAST_MANA)
            && (card.mana_value <= 1.0
                || matches!(
                    compile_typed_atomic_transaction(card),
                    Some(TypedAtomicTransaction::HandMana { .. })
                ))
            && card_has_executable_opening_mana_role(card)
        {
            evaluation.fast_mana = evaluation.fast_mana.saturating_add(1);
        }
        if card_is_executable_early_acceleration(card) {
            evaluation.executable_one_land_acceleration = evaluation
                .executable_one_land_acceleration
                .saturating_add(1);
            if card_is_executable_zero_land_acceleration(card) {
                evaluation.executable_zero_land_acceleration = evaluation
                    .executable_zero_land_acceleration
                    .saturating_add(1);
            }
        }
        if card.has(role::ENGINE | role::ENABLER) {
            evaluation.engines = evaluation.engines.saturating_add(1);
        }
        if card.has(role::DRAW) && card.mana_value <= 3.0 {
            evaluation.draw = evaluation.draw.saturating_add(1);
        }
        if card.has(role::TUTOR) && tutor_has_legal_target(deck, card, &remaining_library) {
            evaluation.tutors = evaluation.tutors.saturating_add(1);
        }
        if card_is_hand_mulligan_plan(deck, card, &remaining_library) {
            evaluation.cedh_hand_plans = evaluation.cedh_hand_plans.saturating_add(1);
        }
        if card_is_independent_hand_mulligan_plan(card) {
            evaluation.independent_hand_plans = evaluation.independent_hand_plans.saturating_add(1);
        }
        if !card.has(role::LAND)
            && card.mana_value <= 3.0
            && card_is_executable_opening_action(card)
        {
            evaluation.early_actions = evaluation.early_actions.saturating_add(1);
        }
        if card_is_meaningful_early_action(card) {
            evaluation.meaningful_early_actions =
                evaluation.meaningful_early_actions.saturating_add(1);
        }
    }

    let route_assessment =
        assess_reviewed_opening_routes(deck, mana, card_indices, &remaining_library);
    evaluation.reviewed_route_catalog_present = route_assessment.has_reviewed_routes;
    evaluation.explicit_route_access =
        route_assessment.direct_complete || route_assessment.tutor_complete;
    evaluation.route_relevant_tutors = card_indices
        .iter()
        .filter(|index| route_assessment.relevant_hand_tutors.contains(index))
        .count()
        .min(usize::from(u8::MAX)) as u8;

    // Comprehensive Rules 903.6 makes the selected commanders available from
    // the command zone. Only a real engine or executable card-access/search
    // ability is a plan here: an enabler role by itself (for example a free
    // body with no engine text) must not turn a pile of mana into a keep. When
    // reviewed routes exist, a tutor-only commander must also reach the exact
    // currently missing named piece; broad "some legal target" credit is not
    // enough.
    evaluation.command_zone_plan_access = deck.commanders.iter().any(|index| {
        deck.cards.get(*index).is_some_and(|card| {
            // Guaranteed command-zone access is not the same as an early
            // keepable plan. A three- or five-mana value commander cannot
            // justify an otherwise disconnected competitive opening merely
            // because it eventually draws cards. The bounded opening search
            // separately proves accelerated low-cost commander paths.
            card.mana_value <= 2.0
                && card_is_relevant_command_zone_mulligan_plan(
                    deck,
                    card,
                    *index,
                    &remaining_library,
                    &route_assessment,
                )
        })
    });
    let has_opening_plan = evaluation.independent_hand_plans > 0
        || evaluation.route_relevant_tutors > 0
        || evaluation.command_zone_plan_access
        || !route_assessment.has_reviewed_routes && evaluation.cedh_hand_plans > 0;
    if evaluation.lands <= 1 && has_opening_plan {
        let (direct_one_land, accelerated_one_land, zero_land) = opening_plan_paths(
            deck,
            mana,
            card_indices,
            &remaining_library,
            &route_assessment,
        );
        evaluation.directly_payable_one_land_plan = direct_one_land;
        evaluation.accelerated_one_land_plan = accelerated_one_land;
        evaluation.accelerated_zero_land_plan = zero_land;
    }
    if let Some((average, floor)) = mana.and_then(|profile| profile.coverage(card_indices)) {
        evaluation.color_requirements_known = true;
        evaluation.color_coverage = average;
        evaluation.color_floor = floor;
    }
    apply_effective_hand_strength(&mut evaluation, &route_assessment);
    evaluation
}

fn reviewed_line_is_opening_route(line: &crate::domain::KnownLine) -> bool {
    line.table_lethal_if_resolved
        && !line.cards.is_empty()
        && !line.simulation_requirements.iter().any(|requirement| {
            matches!(
                requirement,
                LineRequirement::Unmodeled
                    | LineRequirement::TotalExecutionMana
                    | LineRequirement::CombatAccess
            )
        })
}

fn assess_reviewed_opening_routes(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    card_indices: &[usize],
    remaining_library: &HashMap<usize, u16>,
) -> ReviewedOpeningRouteAssessment {
    let mut assessment = ReviewedOpeningRouteAssessment::default();
    let mut hand_counts = HashMap::<String, usize>::new();
    for card_index in card_indices {
        if let Some(card) = deck.cards.get(*card_index) {
            *hand_counts.entry(card.normalized_name.clone()).or_default() += 1;
        }
    }
    let mut command_zone_counts = HashMap::<String, usize>::new();
    for card_index in &deck.commanders {
        if let Some(card) = deck.cards.get(*card_index) {
            *command_zone_counts
                .entry(card.normalized_name.clone())
                .or_default() += 1;
        }
    }

    for line in deck
        .known_lines
        .iter()
        .filter(|line| reviewed_line_is_opening_route(line))
    {
        assessment.has_reviewed_routes = true;
        let mut required = HashMap::<String, usize>::new();
        for name in &line.cards {
            *required
                .entry(crate::parser::normalize_card_name(name))
                .or_default() += 1;
        }
        let total_required = required.values().copied().sum::<usize>();
        if total_required == 0 {
            continue;
        }

        let mut accessible = 0usize;
        let mut missing = Vec::<String>::new();
        for (name, quantity) in &required {
            let in_hand = hand_counts.get(name).copied().unwrap_or_default();
            let in_command_zone = command_zone_counts.get(name).copied().unwrap_or_default();
            let available = (*quantity).min(in_hand.saturating_add(in_command_zone));
            accessible = accessible.saturating_add(available);
            missing.extend(std::iter::repeat_n(
                name.clone(),
                quantity.saturating_sub(available),
            ));
        }
        assessment.best_direct_fraction = assessment
            .best_direct_fraction
            .max(accessible as f32 / total_required as f32);
        if missing.is_empty() {
            assessment.direct_complete = true;
            continue;
        }
        if missing.len() != 1 {
            continue;
        }

        let missing_name = &missing[0];
        let target_indices = remaining_library
            .iter()
            .filter_map(|(card_index, count)| {
                (*count > 0
                    && deck
                        .cards
                        .get(*card_index)
                        .is_some_and(|card| card.normalized_name == *missing_name))
                .then_some(*card_index)
            })
            .collect::<Vec<_>>();
        if target_indices.is_empty() {
            continue;
        }

        for (tutor_position, tutor_index) in card_indices.iter().copied().enumerate() {
            let reaches_missing = deck.cards.get(tutor_index).is_some_and(|tutor| {
                target_indices.iter().copied().any(|target_index| {
                    deck.cards
                        .get(target_index)
                        .is_some_and(|target| typed_tutor_reaches_specific_card(tutor, target))
                })
            });
            if reaches_missing
                && opening_hand_plan_is_payable(deck, mana, card_indices, tutor_position)
            {
                assessment.tutor_complete = true;
                assessment.relevant_hand_tutors.insert(tutor_index);
            }
        }
        for tutor_index in deck.commanders.iter().copied() {
            let reaches_missing = deck.cards.get(tutor_index).is_some_and(|tutor| {
                target_indices.iter().copied().any(|target_index| {
                    deck.cards
                        .get(target_index)
                        .is_some_and(|target| typed_tutor_reaches_specific_card(tutor, target))
                })
            });
            if reaches_missing {
                assessment.tutor_complete = true;
                assessment.relevant_command_zone_tutors.insert(tutor_index);
            }
        }
    }

    if let Some(relevant_tutors) =
        reviewed_graveyard_storm_primer_opening_witness(deck, mana, card_indices, remaining_library)
    {
        assessment.has_reviewed_routes = true;
        assessment.tutor_complete = true;
        assessment.best_direct_fraction = 1.0;
        assessment.relevant_hand_tutors.extend(relevant_tutors);
    }
    assessment
}

/// Recognize the exact resource proof behind the primer's seven-card Breach
/// opening before the London policy decides whether to keep it. This is not a
/// name allowlist: every member is selected by the same typed roots used by
/// turn planning and runtime, and the complete mana sequence is paid on a
/// private pool. The real forced-opening episode regression remains the final
/// authority that the selected hand survives land choice, priority, searches,
/// zone movement, and the six-card escape-fodder boundary.
fn reviewed_graveyard_storm_primer_opening_witness(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    remaining_library: &HashMap<usize, u16>,
) -> Option<HashSet<usize>> {
    let mana_access = mana_access?;
    if hand.len() < 7 {
        return None;
    }

    let distinct_hand_cards = hand.iter().copied().collect::<HashSet<_>>();
    for line in &deck.known_lines {
        let Some(program) = compile_graveyard_storm_program(line, deck) else {
            continue;
        };
        if !hand.contains(&program.mana_source)
            || remaining_library
                .get(&program.permission_source)
                .copied()
                .unwrap_or_default()
                == 0
            || remaining_library
                .get(&program.mill_spell)
                .copied()
                .unwrap_or_default()
                == 0
        {
            continue;
        }

        let search_sources = distinct_hand_cards
            .iter()
            .copied()
            .filter(|card_index| {
                matches!(
                    deck.cards
                        .get(*card_index)
                        .and_then(compile_typed_atomic_transaction),
                    Some(TypedAtomicTransaction::SearchRandomDiscardShuffle { tutor })
                        if deck.cards.get(program.mill_spell).is_some_and(
                            |target| program_tutor_matches(&tutor.filter, target)
                        )
                )
            })
            .collect::<Vec<_>>();
        let tutor_sources = distinct_hand_cards
            .iter()
            .copied()
            .filter_map(|card_index| {
                deck.cards
                    .get(card_index)
                    .and_then(compile_typed_first_use_self_transfer_tutor)
                    .map(|tutor| (card_index, tutor))
            })
            .collect::<Vec<_>>();
        let rituals = distinct_hand_cards
            .iter()
            .copied()
            .filter_map(|card_index| {
                let TypedAtomicTransaction::NameLinkedGraveyardRitual {
                    base,
                    per_match,
                    opponent_matching_card_floor,
                } = deck
                    .cards
                    .get(card_index)
                    .and_then(compile_typed_atomic_transaction)?
                else {
                    return None;
                };
                (base
                    == (FixedManaProfile {
                        red: 2,
                        ..FixedManaProfile::default()
                    })
                    && per_match
                        == (FixedManaProfile {
                            red: 1,
                            ..FixedManaProfile::default()
                        })
                    && opponent_matching_card_floor == 0)
                    .then_some((card_index, base))
            })
            .collect::<Vec<_>>();
        let one_shot_black_sources = distinct_hand_cards
            .iter()
            .copied()
            .filter(|card_index| {
                let Some(card) = deck.cards.get(*card_index) else {
                    return false;
                };
                let Some(source) = mana_access.source(*card_index) else {
                    return false;
                };
                card.effects.mana_production_kind == ManaProductionKind::OneShotActivated
                    && card.effects.mana_produced.conservative_value(1) == 1
                    && card_has_executable_opening_mana_role(card)
                    && !source.unknown
                    && (source.any_color || source.colors.intersects(ManaColorMask::BLACK))
            })
            .collect::<Vec<_>>();
        let fetches = distinct_hand_cards
            .iter()
            .copied()
            .filter_map(|card_index| {
                deck.cards
                    .get(card_index)
                    .and_then(reviewed_fetchland)
                    .map(|fetch| (card_index, fetch))
            })
            .collect::<Vec<_>>();

        for search_source in &search_sources {
            for (tutor_source, tutor) in &tutor_sources {
                for (ritual, ritual_base) in &rituals {
                    for one_shot_black in &one_shot_black_sources {
                        for (fetch, descriptor) in &fetches {
                            let setup_cards = [
                                program.mana_source,
                                *search_source,
                                *tutor_source,
                                *ritual,
                                *one_shot_black,
                                *fetch,
                            ];
                            if setup_cards.into_iter().collect::<HashSet<_>>().len() != 6
                                || hand.len() <= setup_cards.len()
                            {
                                continue;
                            }

                            let mut fetch_targets = remaining_library
                                .iter()
                                .filter_map(|(card_index, count)| {
                                    let candidate = (*count > 0)
                                        .then(|| deck.cards.get(*card_index))
                                        .flatten()?;
                                    if !library_card_has_basic_land_subtype(
                                        candidate,
                                        descriptor.first_subtype,
                                    ) && !library_card_has_basic_land_subtype(
                                        candidate,
                                        descriptor.second_subtype,
                                    ) {
                                        return None;
                                    }
                                    let source = battlefield_land_source(
                                        deck,
                                        Some(mana_access),
                                        *card_index,
                                        1,
                                    );
                                    (source.available_from_turn <= 1
                                        && source.reliability >= 0.999
                                        && source.colors.intersects(ManaColorMask::RED))
                                    .then_some((*card_index, source))
                                })
                                .collect::<Vec<_>>();
                            fetch_targets.sort_unstable_by_key(|(card_index, _)| *card_index);

                            for (_, land_source) in fetch_targets {
                                let mut pool = TurnManaPool::default();
                                pool.add_floating(land_source.colors, land_source.capacity);
                                let exact_payment = |pool: &mut TurnManaPool, card_index| {
                                    let Some(cost) = mana_access.cost(card_index) else {
                                        return false;
                                    };
                                    activation_cost_is_exactly_modeled(cost)
                                        && pool.pay(Some(cost), 0, 0)
                                };
                                if !exact_payment(&mut pool, *ritual)
                                    || !add_fixed_mana(&mut pool, *ritual_base)
                                    || !exact_payment(&mut pool, *one_shot_black)
                                {
                                    continue;
                                }
                                pool.add_floating(ManaColorMask::BLACK, 1);
                                if !exact_payment(&mut pool, program.mana_source)
                                    || !exact_payment(&mut pool, *tutor_source)
                                    || !exact_payment(&mut pool, *search_source)
                                {
                                    continue;
                                }
                                pool.add_floating(ManaColorMask::RED, program.mana_amount);
                                if !pay_first_use_self_transfer_tutor_activation(&mut pool, tutor)
                                    || !pay_exact_printed_cost(
                                        &mut pool,
                                        mana_access,
                                        program.permission_source,
                                        0,
                                    )
                                {
                                    continue;
                                }

                                return Some(HashSet::from([*search_source, *tutor_source]));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn typed_tutor_reaches_specific_card(tutor: &CompiledCard, target: &CompiledCard) -> bool {
    if matches!(
        compile_typed_atomic_transaction(tutor),
        Some(
            TypedAtomicTransaction::SearchRandomDiscardShuffle { .. }
                | TypedAtomicTransaction::OpponentChoiceSearchSplit
        )
    ) {
        // The linked random discard is the authoritative resolution shape.
        // A simultaneously populated legacy tutor descriptor must never
        // upgrade stochastic access to certainty.
        return false;
    }
    let legacy_match = tutor.effects.tutor.is_executable_on_spell_resolution()
        && tutor.effects.tutor.instructions.iter().any(|instruction| {
            instruction.source == TutorSourceZone::Library
                && instruction.quantity > 0
                && instruction.destination != TutorDestination::None
                && instruction.target.matches(target.effects.card_types)
                && (!matches!(
                    instruction.destination,
                    TutorDestination::BattlefieldTapped | TutorDestination::BattlefieldUntapped
                ) || target.effects.card_types.is_land)
        });
    if legacy_match {
        return true;
    }

    if compile_typed_atomic_transaction(tutor).is_some_and(|transaction| match transaction {
        // A sacrifice tutor has no opening-state witness until a real creature
        // has reached the battlefield. The full trajectory executor can earn
        // that route later; opening EHS must not assume the additional cost.
        TypedAtomicTransaction::SacrificeTutor { .. }
        // The searched object and every retained hand object are still at
        // uniform discard risk. Runtime and turn planning can value that
        // chance, but opening EHS must not promote it to certain completion.
        | TypedAtomicTransaction::SearchRandomDiscardShuffle { .. }
        | TypedAtomicTransaction::OpponentChoiceSearchSplit => false,
        TypedAtomicTransaction::BargainSearchCastOrHand { .. } => !target.is_commander,
        _ => false,
    }) {
        return true;
    }

    if compile_typed_first_use_self_transfer_tutor(tutor).is_some() && !target.is_commander {
        return true;
    }

    tutor.ability_program.executable_abilities().any(|ability| {
        ability.timing == AbilityTiming::SpellResolution
            && ability.effects.iter().any(|effect| match effect {
                AbilityEffect::Tutor(program) => {
                    program.from == ProgramZone::Library
                        && program.destination != ProgramZone::Library
                        && program_tutor_matches(&program.filter, target)
                }
                AbilityEffect::VariableCreatureTutor(program) => {
                    program.from_library
                        && program.destination != ProgramZone::Library
                        && target.effects.card_types.is_creature
                }
                _ => false,
            })
    })
}

fn apply_effective_hand_strength(
    evaluation: &mut HandEvaluation,
    route: &ReviewedOpeningRouteAssessment,
) {
    let independent_plan_strength = if evaluation.independent_hand_plans > 0 {
        0.86
    } else if evaluation.command_zone_plan_access {
        0.82
    } else if !route.has_reviewed_routes && evaluation.cedh_hand_plans > 0 {
        0.76
    } else {
        0.0
    };
    let reviewed_route_strength = if route.direct_complete {
        1.0
    } else if route.tutor_complete {
        0.90
    } else {
        // Partial possession is useful for deterministic London bottoming, but
        // it is deliberately capped far below a joint route. In particular,
        // two pieces of a three-card line are not silently promoted to a
        // complete opening plan.
        (route.best_direct_fraction * 0.50).min(0.49)
    };
    let route_strength = reviewed_route_strength.max(independent_plan_strength);

    let mana_readiness = if evaluation.directly_payable_one_land_plan {
        1.0
    } else if evaluation.accelerated_one_land_plan {
        0.90
    } else if evaluation.accelerated_zero_land_plan {
        0.80
    } else {
        match evaluation.lands {
            2..=3 => 0.74,
            1 => 0.42,
            4 => 0.52,
            0 if evaluation.executable_zero_land_acceleration >= 2 => 0.36,
            _ => 0.18,
        }
    };
    let color_viability = if evaluation.color_requirements_known {
        (evaluation.color_coverage * 0.70 + evaluation.color_floor * 0.30).clamp(0.0, 1.0)
    } else {
        // Unknown exact pips are neutral rather than perfect. This preserves
        // fail-closed behavior while allowing exact route/tutor evidence to
        // remain useful in tests and partially resolved local card data.
        0.50
    };

    evaluation.effective_hand_strength_assessed = true;
    evaluation.effective_route_strength = route_strength;
    evaluation.effective_mana_readiness = mana_readiness;
    evaluation.effective_color_viability = color_viability;
    evaluation.effective_hand_strength =
        (route_strength * 0.60 + mana_readiness * 0.25 + color_viability * 0.15).clamp(0.0, 1.0);
}

fn card_is_meaningful_early_action(card: &CompiledCard) -> bool {
    let nonfunctional_role = card.has(
        role::DRAW
            | role::REMOVAL
            | role::COUNTERSPELL
            | role::BOARD_WIPE
            | role::PROTECTION
            | role::ENGINE
            | role::ENABLER
            | role::PAYOFF
            | role::WIN_CONDITION
            | role::COMBO_PIECE
            | role::STAX,
    );
    let executable_functional_role = card.has(role::RAMP | role::FAST_MANA)
        && card_has_executable_opening_mana_role(card)
        || card.has(role::TUTOR) && card_has_executable_planner_tutor_role(card);
    !card.has(role::LAND)
        && card.mana_value <= 3.0
        && (nonfunctional_role || executable_functional_role)
}

fn card_is_executable_opening_action(card: &CompiledCard) -> bool {
    if card.ability_program.necropotence_lifecycle.is_some() {
        return compile_typed_necropotence_lifecycle(card).is_some();
    }
    !card.has(role::RAMP | role::FAST_MANA | role::TUTOR)
        || card.has(role::RAMP | role::FAST_MANA) && card_has_executable_opening_mana_role(card)
        || card.has(role::TUTOR) && card_has_executable_planner_tutor_role(card)
        || card_has_other_executable_opening_development(card)
}

fn card_has_executable_draw_access(card: &CompiledCard) -> bool {
    immediate_cards_drawn(card) > 0
        || compile_typed_burst_card_access_program(card).is_some()
        || compile_typed_necropotence_lifecycle(card).is_some()
        || card.ability_program.executable_abilities().any(|ability| {
            ability.effects.iter().any(|effect| match effect {
                AbilityEffect::Draw(access) => access.count > 0,
                AbilityEffect::LookAtTopAndSelect(selection) => {
                    selection.selection_count > 0 && selection.destination != ProgramZone::Library
                }
                AbilityEffect::ExhaustiveTopCardAccess(access) => {
                    access.land_destination != ProgramZone::Library
                        || access.nonland_destination != ProgramZone::Library
                }
                _ => false,
            })
        })
}

fn card_changes_observable_plan(card: &CompiledCard) -> bool {
    immediate_cards_drawn(card) > 0
        || card.effects.tutor.is_executable_on_spell_resolution()
        || compile_typed_burst_card_access_program(card).is_some()
        || compile_typed_atomic_transaction(card).is_some()
        || compile_typed_conditional_mana_source(card).is_some()
        || exact_discard_sacrifice_mana_ability(card).is_some()
        || compile_typed_first_use_self_transfer_tutor(card).is_some()
        || compile_typed_necropotence_lifecycle(card).is_some()
}

/// A strategic role is not execution authority. Only mana lifecycles that the
/// cast executor can actually install or resolve receive RAMP/FAST_MANA
/// development credit.
fn card_has_executable_planner_mana_role(card: &CompiledCard) -> bool {
    card.effects.mana_produced.conservative_value(1) > 0
        && matches!(
            card.effects.mana_production_kind,
            ManaProductionKind::SpellResolution
                | ManaProductionKind::ReusableActivated
                | ManaProductionKind::OneShotActivated
                | ManaProductionKind::NonRefreshingActivated
        )
        || immediate_effect_value(card, card.effects.treasure_tokens, 1) > 0
        || card.effects.tutor.is_executable_on_spell_resolution()
            && immediate_effect_value(card, card.effects.lands_to_battlefield, 1) > 0
        || compile_typed_atomic_transaction(card)
            .is_some_and(|transaction| transaction.is_mana_development())
        || compile_typed_conditional_mana_source(card).is_some()
        || exact_discard_sacrifice_mana_ability(card).is_some()
}

/// Opening-hand credit is narrower than full turn-planner executability. The
/// compact opening solver can execute hand mana and the exact graveyard
/// rituals below, but it has no battlefield-object ledger for sacrifice
/// rituals or temporary land-sacrifice grants such as Rain of Filth. Those
/// transactions remain executable in real turns without being treated as
/// independent mulligan acceleration.
fn card_has_executable_opening_mana_role(card: &CompiledCard) -> bool {
    match compile_typed_atomic_transaction(card) {
        Some(
            TypedAtomicTransaction::HandMana { .. }
            | TypedAtomicTransaction::NameLinkedGraveyardRitual { .. }
            | TypedAtomicTransaction::ThresholdRitual { .. },
        ) => true,
        Some(transaction) if transaction.is_mana_development() => false,
        _ => card_has_executable_planner_mana_role(card),
    }
}

/// Exact mana lifecycles that cannot leave a reusable next-turn resource in
/// the bounded model. This is capability-based so renamed cards inherit the
/// same conservation policy without any card-name allowlist.
fn card_is_current_turn_consumable_mana(card: &CompiledCard) -> bool {
    compile_typed_atomic_transaction(card)
        .is_some_and(|transaction| transaction.is_mana_development())
        || card.effects.mana_produced.conservative_value(1) > 0
            && matches!(
                card.effects.mana_production_kind,
                ManaProductionKind::SpellResolution
                    | ManaProductionKind::OneShotActivated
                    | ManaProductionKind::NonRefreshingActivated
            )
}

/// Match the activated Treasure tutor shape consumed by
/// `active_resource_tutor`. Broader Tutor role annotations remain report-only
/// until an executor exists for their complete costs and destinations.
fn card_has_executable_resource_tutor_ability(card: &CompiledCard) -> bool {
    card.ability_program.executable_abilities().any(|ability| {
        if !matches!(ability.timing, AbilityTiming::Activated { .. })
            || ability.costs.len() != 1
            || ability.effects.len() != 1
        {
            return false;
        }
        let AbilityCost::SacrificeResource {
            resource: ResourceKind::Treasure,
            count,
        } = &ability.costs[0]
        else {
            return false;
        };
        let AbilityEffect::Tutor(tutor) = &ability.effects[0] else {
            return false;
        };
        tutor.from == ProgramZone::Library
            && tutor.destination == ProgramZone::Battlefield
            && *count > 0
            && *count <= u16::from(u8::MAX)
    })
}

fn card_has_executable_planner_tutor_role(card: &CompiledCard) -> bool {
    card.effects.tutor.is_executable_on_spell_resolution()
        || card_has_executable_resource_tutor_ability(card)
        || compile_typed_atomic_transaction(card).is_some_and(|transaction| transaction.is_tutor())
        || compile_typed_first_use_self_transfer_tutor(card).is_some()
}

fn card_has_other_executable_goldfish_development(card: &CompiledCard) -> bool {
    immediate_cards_drawn(card) > 0
        || compile_typed_burst_card_access_program(card).is_some()
        || compile_typed_atomic_transaction(card).is_some()
        || compile_typed_conditional_mana_source(card).is_some()
        || compile_typed_first_use_self_transfer_tutor(card).is_some()
        || compile_typed_necropotence_lifecycle(card).is_some()
        || immediate_creature_tokens(card) > 0
        || immediate_extra_turns(card) > 0
        || card.effects.recursion
        || card_has_persistent_body(card)
            && card.has(
                role::ENGINE
                    | role::ENABLER
                    | role::PAYOFF
                    | role::WIN_CONDITION
                    | role::COMBO_PIECE
                    | role::PROTECTION
                    | role::CREATURE,
            )
}

fn card_has_other_executable_opening_development(card: &CompiledCard) -> bool {
    immediate_cards_drawn(card) > 0
        || compile_typed_burst_card_access_program(card).is_some()
        || compile_typed_atomic_transaction(card).is_some_and(|transaction| {
            !transaction.is_mana_development() && !transaction.is_tutor()
        })
        || compile_typed_conditional_mana_source(card).is_some()
        || compile_typed_first_use_self_transfer_tutor(card).is_some()
        || compile_typed_necropotence_lifecycle(card).is_some()
        || immediate_creature_tokens(card) > 0
        || immediate_extra_turns(card) > 0
        || card.effects.recursion
        || card_has_persistent_body(card)
            && card.has(
                role::ENGINE
                    | role::ENABLER
                    | role::PAYOFF
                    | role::WIN_CONDITION
                    | role::COMBO_PIECE
                    | role::PROTECTION
                    | role::CREATURE,
            )
}

fn planning_card_advances_credible_route(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    planned_casts: &[usize],
    card_index: usize,
) -> bool {
    let before = planning_route_value(deck, zones, turn, planned_casts);
    let mut with_card = planned_casts.to_vec();
    with_card.push(card_index);
    let after = planning_route_value(deck, zones, turn, &with_card);
    (after.2, after.1, after.0) > (before.2, before.1, before.0)
}

/// Hold a role-only mana/tutor card when casting it would be a no-op in the
/// current runtime. A card may still be legal when another typed effect or an
/// executable reviewed route gives the physical cast a modeled purpose; only
/// the unsupported functional-role bonus remains suppressed.
fn functional_role_card_is_only_noop(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    planned_casts: &[usize],
    card_index: usize,
    card: &CompiledCard,
) -> bool {
    let has_functional_role = card.has(role::RAMP | role::FAST_MANA | role::TUTOR);
    let has_executable_functional_role = card.has(role::RAMP | role::FAST_MANA)
        && card_has_executable_planner_mana_role(card)
        || card.has(role::TUTOR) && card_has_executable_planner_tutor_role(card);
    has_functional_role
        && !has_executable_functional_role
        && !card_has_other_executable_goldfish_development(card)
        && !planning_card_advances_credible_route(deck, zones, turn, planned_casts, card_index)
}

fn card_is_hand_mulligan_plan(
    deck: &CompiledDeck,
    card: &CompiledCard,
    remaining_library: &HashMap<usize, u16>,
) -> bool {
    card_is_independent_hand_mulligan_plan(card)
        || card.has(role::TUTOR) && card_has_executable_tutor_access(deck, card, remaining_library)
}

fn card_is_independent_hand_mulligan_plan(card: &CompiledCard) -> bool {
    card.has(role::ENGINE | role::DRAW) && card_has_executable_draw_access(card)
}

fn card_is_command_zone_mulligan_plan(
    deck: &CompiledDeck,
    card: &CompiledCard,
    remaining_library: &HashMap<usize, u16>,
) -> bool {
    card_has_executable_draw_access(card)
        || card_has_executable_tutor_access(deck, card, remaining_library)
        || card.ability_program.executable_abilities().any(|ability| {
            ability.effects.iter().any(|effect| match effect {
                AbilityEffect::AddMana(mana) => mana.amount > 0,
                AbilityEffect::VariableCreatureTutor(tutor) => {
                    tutor.from_library && tutor.destination != ProgramZone::Library
                }
                AbilityEffect::CreateToken(token) => token.count > 0,
                AbilityEffect::GrantCastPermission(_) => true,
                AbilityEffect::ModifyNonlandMana(modifier) => modifier.additional_amount > 0,
                _ => false,
            })
        })
}

fn card_is_independent_command_zone_mulligan_plan(card: &CompiledCard) -> bool {
    card_has_executable_draw_access(card)
        || card.ability_program.executable_abilities().any(|ability| {
            ability.effects.iter().any(|effect| {
                matches!(
                    effect,
                    AbilityEffect::AddMana(mana) if mana.amount > 0
                ) || matches!(
                    effect,
                    AbilityEffect::CreateToken(token) if token.count > 0
                ) || matches!(effect, AbilityEffect::GrantCastPermission(_))
                    || matches!(
                        effect,
                        AbilityEffect::ModifyNonlandMana(modifier)
                            if modifier.additional_amount > 0
                    )
            })
        })
}

fn card_is_relevant_command_zone_mulligan_plan(
    deck: &CompiledDeck,
    card: &CompiledCard,
    card_index: usize,
    remaining_library: &HashMap<usize, u16>,
    route: &ReviewedOpeningRouteAssessment,
) -> bool {
    card_is_independent_command_zone_mulligan_plan(card)
        || route.relevant_command_zone_tutors.contains(&card_index)
        || !route.has_reviewed_routes
            && card_is_command_zone_mulligan_plan(deck, card, remaining_library)
}

fn card_has_executable_tutor_access(
    deck: &CompiledDeck,
    tutor: &CompiledCard,
    remaining_library: &HashMap<usize, u16>,
) -> bool {
    tutor_has_legal_target(deck, tutor, remaining_library)
        || compile_typed_atomic_transaction(tutor).is_some_and(|transaction| {
            transaction.is_tutor()
                && remaining_library
                    .iter()
                    .any(|(card_index, count)| *count > 0 && deck.cards.get(*card_index).is_some())
        })
        || compile_typed_first_use_self_transfer_tutor(tutor).is_some()
            && remaining_library.iter().any(|(card_index, count)| {
                *count > 0
                    && deck
                        .cards
                        .get(*card_index)
                        .is_some_and(|candidate| !candidate.is_commander)
            })
        || tutor.ability_program.executable_abilities().any(|ability| {
            ability.effects.iter().any(|effect| {
                let AbilityEffect::Tutor(program) = effect else {
                    return false;
                };
                program.from == ProgramZone::Library
                    && program.destination != ProgramZone::Library
                    && remaining_library.iter().any(|(card_index, count)| {
                        *count > 0
                            && deck.cards.get(*card_index).is_some_and(|candidate| {
                                program_tutor_matches(&program.filter, candidate)
                            })
                    })
            })
        })
}

fn opening_plan_paths(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    card_indices: &[usize],
    remaining_library: &HashMap<usize, u16>,
    route: &ReviewedOpeningRouteAssessment,
) -> (bool, bool, bool) {
    let lands = card_indices
        .iter()
        .enumerate()
        .filter_map(|(position, index)| {
            deck.cards
                .get(*index)
                .is_some_and(|card| card.has(role::LAND))
                .then_some(position)
        })
        .collect::<Vec<_>>();
    let hand_plans = card_indices
        .iter()
        .enumerate()
        .filter_map(|(position, index)| {
            deck.cards
                .get(*index)
                .is_some_and(|card| {
                    card_is_independent_hand_mulligan_plan(card)
                        || route.relevant_hand_tutors.contains(index)
                        || !route.has_reviewed_routes
                            && card_is_hand_mulligan_plan(deck, card, remaining_library)
                })
                .then_some(position)
        })
        .collect::<Vec<_>>();
    let commander_plans = deck
        .commanders
        .iter()
        .copied()
        .filter(|index| {
            deck.cards.get(*index).is_some_and(|card| {
                card_is_relevant_command_zone_mulligan_plan(
                    deck,
                    card,
                    *index,
                    remaining_library,
                    route,
                )
            })
        })
        .collect::<Vec<_>>();
    if hand_plans.is_empty() && commander_plans.is_empty() {
        return (false, false, false);
    }

    let direct_one_land = lands.len() == 1
        && opening_plan_is_directly_payable(
            deck,
            mana,
            card_indices,
            &hand_plans,
            &commander_plans,
            lands[0],
        );
    let accelerated_one_land = if lands.len() == 1 {
        opening_plan_is_castable_after_acceleration(
            deck,
            mana,
            card_indices,
            &hand_plans,
            &commander_plans,
            Some(lands[0]),
            1,
        )
    } else {
        false
    };
    let zero_land = if lands.is_empty() {
        opening_plan_is_castable_after_acceleration(
            deck,
            mana,
            card_indices,
            &hand_plans,
            &commander_plans,
            None,
            2,
        )
    } else {
        false
    };
    (direct_one_land, accelerated_one_land, zero_land)
}

fn opening_hand_plan_is_payable(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    card_indices: &[usize],
    plan_position: usize,
) -> bool {
    let Some(plan_index) = card_indices.get(plan_position).copied() else {
        return false;
    };
    if deck.cards.get(plan_index).is_none() {
        return false;
    }

    let mut pool = TurnManaPool::default();
    let mut board = OpeningBoardState::default();
    let mut used_positions = 0u128;
    let land_count = card_indices
        .iter()
        .filter(|card_index| {
            deck.cards
                .get(**card_index)
                .is_some_and(|card| card.has(role::LAND))
        })
        .count();
    let horizon_turn = u8::try_from(land_count).unwrap_or(u8::MAX).max(1);
    for (position, card_index) in card_indices.iter().copied().enumerate() {
        let Some(card) = deck.cards.get(card_index) else {
            continue;
        };
        if !card.has(role::LAND) {
            continue;
        }
        add_opening_land_mana(&mut pool, deck, mana, card_index, horizon_turn);
        update_opening_board_for_permanent(card, &mut board);
        if position < 128 {
            used_positions |= 1u128 << position;
        }
    }
    let has_amber = card_indices.iter().copied().any(|card_index| {
        deck.cards.get(card_index).is_some_and(|card| {
            compile_typed_conditional_mana_source(card)
                == Some(TypedConditionalManaSource::ControlledLegendaryColors)
        })
    });
    let has_opal = card_indices.iter().copied().any(|card_index| {
        deck.cards.get(card_index).is_some_and(|card| {
            compile_typed_conditional_mana_source(card)
                == Some(TypedConditionalManaSource::MetalcraftAnyColor)
        })
    });
    let acceleration_positions = card_indices
        .iter()
        .enumerate()
        .filter_map(|(position, card_index)| {
            (position != plan_position
                && (position >= 128 || used_positions & (1u128 << position) == 0))
                .then(|| deck.cards.get(*card_index))
                .flatten()
                .is_some_and(|card| {
                    card_is_executable_early_acceleration(card)
                        || has_amber && card_enables_opening_amber(card)
                        || has_opal && card.effects.card_types.is_artifact
                })
                .then_some(position)
        })
        .collect::<Vec<_>>();

    opening_plan_search(
        deck,
        mana,
        card_indices,
        &[plan_position],
        &[],
        &acceleration_positions,
        None,
        horizon_turn,
        &pool,
        &[],
        used_positions,
        0,
        0,
        board,
        0,
        0,
        &mut HashSet::new(),
    )
}

fn opening_plan_is_directly_payable(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    card_indices: &[usize],
    hand_plan_positions: &[usize],
    commander_plan_indices: &[usize],
    land_position: usize,
) -> bool {
    let Some(card_index) = card_indices.get(land_position).copied() else {
        return false;
    };
    let mut pool = TurnManaPool::default();
    add_opening_land_mana(&mut pool, deck, mana, card_index, 1);
    let used_positions = if land_position < 128 {
        1u128 << land_position
    } else {
        0
    };
    opening_plan_payable(
        deck,
        mana,
        card_indices,
        hand_plan_positions,
        commander_plan_indices,
        used_positions,
        0,
        &pool,
    )
}

#[allow(clippy::too_many_arguments)]
fn opening_plan_is_castable_after_acceleration(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    card_indices: &[usize],
    hand_plan_positions: &[usize],
    commander_plan_indices: &[usize],
    land_position: Option<usize>,
    required_acceleration: u8,
) -> bool {
    let has_amber = card_indices.iter().copied().any(|card_index| {
        deck.cards.get(card_index).is_some_and(|card| {
            compile_typed_conditional_mana_source(card)
                == Some(TypedConditionalManaSource::ControlledLegendaryColors)
        })
    });
    let has_opal = card_indices.iter().copied().any(|card_index| {
        deck.cards.get(card_index).is_some_and(|card| {
            compile_typed_conditional_mana_source(card)
                == Some(TypedConditionalManaSource::MetalcraftAnyColor)
        })
    });
    let candidate_positions = card_indices
        .iter()
        .enumerate()
        .filter_map(|(position, index)| {
            (Some(position) != land_position)
                .then(|| deck.cards.get(*index))
                .flatten()
                .is_some_and(|card| {
                    card_is_executable_early_acceleration(card)
                        || has_amber && card_enables_opening_amber(card)
                        || has_opal && card.effects.card_types.is_artifact
                })
                .then_some(position)
        })
        .collect::<Vec<_>>();
    if candidate_positions
        .iter()
        .filter(|position| {
            deck.cards
                .get(card_indices[**position])
                .is_some_and(card_is_executable_early_acceleration)
        })
        .count()
        < usize::from(required_acceleration)
    {
        return false;
    }

    let mut pool = TurnManaPool::default();
    let mut board = OpeningBoardState::default();
    let mut used_positions = 0u128;
    if let Some(position) = land_position {
        add_opening_land_mana(&mut pool, deck, mana, card_indices[position], 1);
        if position < 128 {
            used_positions |= 1u128 << position;
        }
        if deck.cards[card_indices[position]]
            .effects
            .card_types
            .is_artifact
        {
            board.artifacts = 1;
        }
    }
    let mut visited = HashSet::new();
    opening_plan_search(
        deck,
        mana,
        card_indices,
        hand_plan_positions,
        commander_plan_indices,
        &candidate_positions,
        land_position,
        1,
        &pool,
        &[],
        used_positions,
        0,
        0,
        board,
        0,
        required_acceleration,
        &mut visited,
    )
}

fn card_enables_opening_amber(card: &CompiledCard) -> bool {
    type_line_has_subtype(&card.type_line, "Legendary")
        && (card.effects.card_types.is_creature
            || type_line_has_subtype(&card.type_line, "Planeswalker"))
        && !printed_card_colors(card).is_empty()
}

fn update_opening_board_for_permanent(card: &CompiledCard, board: &mut OpeningBoardState) {
    if card.effects.card_types.is_artifact {
        board.artifacts = board.artifacts.saturating_add(1).min(3);
    }
    if card_enables_opening_amber(card) {
        board.legendary_creature_or_planeswalker_colors |= printed_card_colors(card);
    }
}

fn activate_available_opening_sources(
    persistent_sources: &[OpeningPersistentManaSource],
    turn: u8,
    board: OpeningBoardState,
    pool: &mut TurnManaPool,
    activated_source_positions: &mut u128,
) {
    for source in persistent_sources {
        if source.origin_position >= 128
            || *activated_source_positions & (1u128 << source.origin_position) != 0
            || turn < source.available_from_turn
        {
            continue;
        }
        let colors = match source.behavior {
            OpeningManaBehavior::Fixed(colors) => colors,
            OpeningManaBehavior::ControlledLegendaryColors => {
                board.legendary_creature_or_planeswalker_colors
            }
            OpeningManaBehavior::MetalcraftAnyColor => {
                if usize::from(board.artifacts) + usize::from(pool.remaining_treasures()) >= 3 {
                    ManaColorMask::ANY_COLOR
                } else {
                    ManaColorMask::NONE
                }
            }
        };
        if colors.is_empty() || source.capacity == 0 {
            continue;
        }
        pool.add_floating(colors, source.capacity);
        *activated_source_positions |= 1u128 << source.origin_position;
    }
}

#[allow(clippy::too_many_arguments)]
fn opening_search_state_key(
    turn: u8,
    pool: &TurnManaPool,
    persistent_sources: &[OpeningPersistentManaSource],
    used_positions: u128,
    activated_source_positions: u128,
    cast_commanders: u8,
    board: OpeningBoardState,
    acceleration_casts: u8,
) -> String {
    let mut mana = pool
        .sources
        .iter()
        .map(|source| {
            (
                mana_color_code(source.colors),
                source.remaining,
                source.is_treasure,
            )
        })
        .collect::<Vec<_>>();
    mana.sort_unstable();
    let mut persistent = persistent_sources
        .iter()
        .map(|source| {
            let behavior = match source.behavior {
                OpeningManaBehavior::Fixed(colors) => 0x100u16 | u16::from(mana_color_code(colors)),
                OpeningManaBehavior::ControlledLegendaryColors => 1,
                OpeningManaBehavior::MetalcraftAnyColor => 2,
            };
            (
                source.origin_position,
                behavior,
                source.capacity,
                source.available_from_turn,
            )
        })
        .collect::<Vec<_>>();
    persistent.sort_unstable();
    format!(
        "{turn}|{used_positions}|{activated_source_positions}|{cast_commanders}|{}|{}|{acceleration_casts}|{mana:?}|{persistent:?}",
        board.artifacts,
        mana_color_code(board.legendary_creature_or_planeswalker_colors),
    )
}

fn cast_commander_is_opening_plan(
    deck: &CompiledDeck,
    commander_plan_indices: &[usize],
    cast_commanders: u8,
) -> bool {
    deck.commanders
        .iter()
        .copied()
        .enumerate()
        .take(8)
        .any(|(slot, card_index)| {
            cast_commanders & (1u8 << slot) != 0 && commander_plan_indices.contains(&card_index)
        })
}

#[allow(clippy::too_many_arguments)]
fn opening_plan_search(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    card_indices: &[usize],
    hand_plan_positions: &[usize],
    commander_plan_indices: &[usize],
    acceleration_positions: &[usize],
    land_position: Option<usize>,
    turn: u8,
    pool: &TurnManaPool,
    persistent_sources: &[OpeningPersistentManaSource],
    used_positions: u128,
    activated_source_positions: u128,
    cast_commanders: u8,
    board: OpeningBoardState,
    acceleration_casts: u8,
    required_acceleration: u8,
    visited: &mut HashSet<String>,
) -> bool {
    let mut active_pool = pool.clone();
    let mut active_sources = activated_source_positions;
    activate_available_opening_sources(
        persistent_sources,
        turn,
        board,
        &mut active_pool,
        &mut active_sources,
    );
    let state_key = opening_search_state_key(
        turn,
        &active_pool,
        persistent_sources,
        used_positions,
        active_sources,
        cast_commanders,
        board,
        acceleration_casts,
    );
    if !visited.insert(state_key) {
        return false;
    }
    if acceleration_casts >= required_acceleration
        && (cast_commander_is_opening_plan(deck, commander_plan_indices, cast_commanders)
            || opening_plan_payable(
                deck,
                mana,
                card_indices,
                hand_plan_positions,
                commander_plan_indices,
                used_positions,
                cast_commanders,
                &active_pool,
            ))
    {
        return true;
    }

    for position in acceleration_positions {
        if *position >= 128 || used_positions & (1u128 << *position) != 0 {
            continue;
        }
        let card_index = card_indices[*position];
        let Some(card) = deck.cards.get(card_index) else {
            continue;
        };
        let mut paid_pool = active_pool.clone();
        let atomic_transaction = compile_typed_atomic_transaction(card);
        let conditional_source = compile_typed_conditional_mana_source(card);
        let entry_choices = if let Some(kind) = conditional_source {
            if !paid_pool.pay(
                mana.and_then(|profile| profile.cost(card_index)),
                card.mana_value.ceil().max(0.0) as u8,
                0,
            ) {
                continue;
            }
            match kind {
                TypedConditionalManaSource::ImprintLinkedCardColors => {
                    let choices = card_indices
                        .iter()
                        .copied()
                        .enumerate()
                        .filter(|(candidate_position, _)| {
                            *candidate_position != *position
                                && *candidate_position < 128
                                && used_positions & (1u128 << *candidate_position) == 0
                        })
                        .filter_map(|(candidate_position, candidate_index)| {
                            let candidate = deck.cards.get(candidate_index)?;
                            let colors = printed_hand_card_colors(candidate)?;
                            (entry_filter_matches(
                                deck,
                                candidate_index,
                                EntryLinkedCardFilter::NonartifactNonlandCard,
                            ) && !colors.is_empty())
                            .then_some((candidate_position, colors))
                        })
                        .map(|(imprint_position, imprint_colors)| {
                            (
                                Some(imprint_position),
                                Some(OpeningPersistentManaSource {
                                    origin_position: *position,
                                    behavior: OpeningManaBehavior::Fixed(imprint_colors),
                                    capacity: 1,
                                    available_from_turn: turn,
                                }),
                            )
                        })
                        .collect::<Vec<_>>();
                    if choices.is_empty() {
                        continue;
                    }
                    choices
                }
                TypedConditionalManaSource::DiscardLandOrFailEntry => {
                    let choices = card_indices
                        .iter()
                        .copied()
                        .enumerate()
                        .filter_map(|(candidate_position, candidate_index)| {
                            (candidate_position != *position
                                && candidate_position < 128
                                && used_positions & (1u128 << candidate_position) == 0
                                && Some(candidate_position) != land_position
                                && entry_filter_matches(
                                    deck,
                                    candidate_index,
                                    EntryLinkedCardFilter::LandCard,
                                ))
                            .then_some((
                                Some(candidate_position),
                                Some(OpeningPersistentManaSource {
                                    origin_position: *position,
                                    behavior: OpeningManaBehavior::Fixed(ManaColorMask::ANY_COLOR),
                                    capacity: 1,
                                    available_from_turn: turn,
                                }),
                            ))
                        })
                        .collect::<Vec<_>>();
                    if choices.is_empty() {
                        continue;
                    }
                    choices
                }
                TypedConditionalManaSource::ControlledLegendaryColors => vec![(
                    None,
                    Some(OpeningPersistentManaSource {
                        origin_position: *position,
                        behavior: OpeningManaBehavior::ControlledLegendaryColors,
                        capacity: 1,
                        available_from_turn: turn,
                    }),
                )],
                TypedConditionalManaSource::MetalcraftAnyColor => vec![(
                    None,
                    Some(OpeningPersistentManaSource {
                        origin_position: *position,
                        behavior: OpeningManaBehavior::MetalcraftAnyColor,
                        capacity: 1,
                        available_from_turn: turn,
                    }),
                )],
                TypedConditionalManaSource::FixedWithSourceDamage { .. }
                | TypedConditionalManaSource::ColorlessOrAnyColorWithSourceDamage { .. } => {
                    continue;
                }
            }
        } else {
            match atomic_transaction.as_ref() {
                Some(TypedAtomicTransaction::HandMana { output }) => {
                    if !add_fixed_mana(&mut paid_pool, *output) {
                        continue;
                    }
                }
                Some(
                    transaction @ (TypedAtomicTransaction::NameLinkedGraveyardRitual { .. }
                    | TypedAtomicTransaction::ThresholdRitual { .. }),
                ) => {
                    if !paid_pool.pay(
                        mana.and_then(|profile| profile.cost(card_index)),
                        card.mana_value.ceil().max(0.0) as u8,
                        0,
                    ) {
                        continue;
                    }
                    // The compact opening predicate has no graveyard ledger.
                    // Both known-name matches and unavailable opponent matches
                    // therefore remain at the explicit conservative zero floor.
                    let Some(output) = atomic_mana_output_for_graveyard_snapshot(transaction, 0, 0)
                    else {
                        continue;
                    };
                    if !add_fixed_mana(&mut paid_pool, output) {
                        continue;
                    }
                }
                // This compact opening search does not have a battlefield-object
                // ledger. A creature-sacrifice ritual therefore stays fail-closed
                // here even though the full turn planner can execute it.
                Some(
                    TypedAtomicTransaction::SacrificeRitual { .. }
                    | TypedAtomicTransaction::SacrificeTutor { .. }
                    | TypedAtomicTransaction::SearchRandomDiscardShuffle { .. }
                    | TypedAtomicTransaction::TemporaryLandSacrificeManaGrant { .. }
                    | TypedAtomicTransaction::BargainSearchCastOrHand { .. }
                    | TypedAtomicTransaction::OpponentChoiceSearchSplit,
                ) => continue,
                None if card.ability_program.atomic_transaction.is_some() => continue,
                None => {
                    if !paid_pool.pay(
                        mana.and_then(|profile| profile.cost(card_index)),
                        card.mana_value.ceil().max(0.0) as u8,
                        0,
                    ) {
                        continue;
                    }
                }
            }
            vec![(None, None)]
        };

        let next_casts = acceleration_casts
            .saturating_add(u8::from(card_is_executable_early_acceleration(card)));
        for (additional_used_position, typed_persistent_source) in entry_choices {
            let mut next_pool = paid_pool.clone();
            let mut next_board = board;
            let mut next_used = used_positions | (1u128 << *position);
            if let Some(entry_cost_position) = additional_used_position {
                next_used |= 1u128 << entry_cost_position;
            }
            if next_casts >= required_acceleration && hand_plan_positions.contains(position) {
                return true;
            }

            let mut next_persistent = persistent_sources.to_vec();
            if let Some(source) = typed_persistent_source {
                next_persistent.push(source);
            } else if atomic_transaction.is_none() && conditional_source.is_none() {
                let output = card.effects.mana_produced.conservative_value(1).min(8);
                let colors =
                    opening_source_colors(mana, card_index, card.effects.mana_production_kind);
                match card.effects.mana_production_kind {
                    ManaProductionKind::SpellResolution
                    | ManaProductionKind::OneShotActivated
                    | ManaProductionKind::NonRefreshingActivated => {
                        next_pool.add_floating(colors, output);
                    }
                    ManaProductionKind::ReusableActivated => {
                        next_persistent.push(OpeningPersistentManaSource {
                            origin_position: *position,
                            behavior: OpeningManaBehavior::Fixed(colors),
                            capacity: output,
                            available_from_turn: if card.has(role::CREATURE) {
                                turn.saturating_add(1)
                            } else {
                                turn
                            },
                        });
                    }
                    ManaProductionKind::None | ManaProductionKind::Unsupported => {
                        if card_is_executable_early_acceleration(card) {
                            continue;
                        }
                    }
                }
            }
            let mut next_active_sources = active_sources;
            let transient_artifact = atomic_transaction.is_none()
                && conditional_source.is_none()
                && card.effects.card_types.is_artifact
                && card.effects.mana_production_kind == ManaProductionKind::OneShotActivated;
            if transient_artifact {
                let mut activation_board = next_board;
                update_opening_board_for_permanent(card, &mut activation_board);
                activate_available_opening_sources(
                    &next_persistent,
                    turn,
                    activation_board,
                    &mut next_pool,
                    &mut next_active_sources,
                );
            } else if conditional_source.is_some()
                || atomic_transaction.is_none()
                    && card_has_persistent_body(card)
                    && card.effects.mana_production_kind != ManaProductionKind::OneShotActivated
            {
                update_opening_board_for_permanent(card, &mut next_board);
            }
            if opening_plan_search(
                deck,
                mana,
                card_indices,
                hand_plan_positions,
                commander_plan_indices,
                acceleration_positions,
                land_position,
                turn,
                &next_pool,
                &next_persistent,
                next_used,
                next_active_sources,
                cast_commanders,
                next_board,
                next_casts,
                required_acceleration,
                visited,
            ) {
                return true;
            }
        }
    }

    for (commander_slot, card_index) in deck.commanders.iter().copied().enumerate().take(8) {
        let commander_bit = 1u8 << commander_slot;
        if cast_commanders & commander_bit != 0 {
            continue;
        }
        let Some(card) = deck.cards.get(card_index) else {
            continue;
        };
        if !(card_enables_opening_amber(card)
            || card.effects.card_types.is_artifact
                && persistent_sources
                    .iter()
                    .any(|source| source.behavior == OpeningManaBehavior::MetalcraftAnyColor))
        {
            continue;
        }
        let mut commander_pool = active_pool.clone();
        if !commander_pool.pay(
            mana.and_then(|profile| profile.cost(card_index)),
            card.mana_value.ceil().max(0.0) as u8,
            0,
        ) {
            continue;
        }
        let mut commander_board = board;
        update_opening_board_for_permanent(card, &mut commander_board);
        if opening_plan_search(
            deck,
            mana,
            card_indices,
            hand_plan_positions,
            commander_plan_indices,
            acceleration_positions,
            land_position,
            turn,
            &commander_pool,
            persistent_sources,
            used_positions,
            active_sources,
            cast_commanders | commander_bit,
            commander_board,
            acceleration_casts,
            required_acceleration,
            visited,
        ) {
            return true;
        }
    }

    if turn >= 2 {
        return false;
    }
    let mut next_turn_pool = TurnManaPool::default();
    if let Some(position) = land_position {
        add_opening_land_mana(
            &mut next_turn_pool,
            deck,
            mana,
            card_indices[position],
            turn.saturating_add(1),
        );
    }
    opening_plan_search(
        deck,
        mana,
        card_indices,
        hand_plan_positions,
        commander_plan_indices,
        acceleration_positions,
        land_position,
        turn.saturating_add(1),
        &next_turn_pool,
        persistent_sources,
        used_positions,
        0,
        cast_commanders,
        board,
        acceleration_casts,
        required_acceleration,
        visited,
    )
}

#[allow(clippy::too_many_arguments)]
fn opening_plan_payable(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    card_indices: &[usize],
    hand_plan_positions: &[usize],
    commander_plan_indices: &[usize],
    used_positions: u128,
    cast_commanders: u8,
    pool: &TurnManaPool,
) -> bool {
    hand_plan_positions
        .iter()
        .filter(|position| **position >= 128 || used_positions & (1u128 << **position) == 0)
        .filter_map(|position| card_indices.get(*position).copied())
        .chain(commander_plan_indices.iter().copied().filter(|card_index| {
            deck.commanders
                .iter()
                .position(|commander| commander == card_index)
                .is_none_or(|slot| slot >= 8 || cast_commanders & (1u8 << slot) == 0)
        }))
        .any(|card_index| {
            let Some(card) = deck.cards.get(card_index) else {
                return false;
            };
            let mut candidate = pool.clone();
            if let Some(tutor) = compile_typed_first_use_self_transfer_tutor(card) {
                return candidate.pay(
                    mana.and_then(|profile| profile.cost(card_index)),
                    card.mana_value.ceil().max(0.0) as u8,
                    0,
                ) && pay_first_use_self_transfer_tutor_activation(&mut candidate, &tutor);
            }
            if card.ability_program.self_transfer_tutor_permanent.is_some() {
                return false;
            }
            if card.ability_program.necropotence_lifecycle.is_some()
                && compile_typed_necropotence_lifecycle(card).is_none()
            {
                return false;
            }
            match compile_typed_atomic_transaction(card) {
                Some(TypedAtomicTransaction::SacrificeTutor { .. }) => false,
                Some(
                    TypedAtomicTransaction::SearchRandomDiscardShuffle { .. }
                    | TypedAtomicTransaction::BargainSearchCastOrHand { .. },
                ) => candidate.pay(
                    mana.and_then(|profile| profile.cost(card_index)),
                    card.mana_value.ceil().max(0.0) as u8,
                    0,
                ),
                Some(_) => false,
                None if card.ability_program.atomic_transaction.is_some() => false,
                None => candidate.pay(
                    mana.and_then(|profile| profile.cost(card_index)),
                    card.mana_value.ceil().max(0.0) as u8,
                    0,
                ),
            }
        })
}

fn add_opening_land_mana(
    pool: &mut TurnManaPool,
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    card_index: usize,
    turn: u8,
) {
    let Some(card) = deck.cards.get(card_index) else {
        return;
    };
    if !card.has(role::LAND) {
        return;
    }
    match compile_typed_conditional_mana_source(card) {
        Some(TypedConditionalManaSource::FixedWithSourceDamage { output, .. }) => {
            // This compact keep predicate proves mana access, not a life-total
            // trajectory. The full simulator executes the linked self-damage.
            let _ = add_fixed_mana(pool, output);
            return;
        }
        Some(TypedConditionalManaSource::ColorlessOrAnyColorWithSourceDamage { .. }) => {
            pool.add_floating(ManaColorMask::ANY_COLOR | ManaColorMask::COLORLESS, 1);
            return;
        }
        _ => {}
    }
    let source = mana.and_then(|profile| profile.source(card_index));
    let available = turn >= 2
        || !matches!(
            source.map(|source| source.enters_tapped),
            Some(EntersTapped::Always | EntersTapped::Conditional | EntersTapped::Unknown)
        );
    if available {
        pool.add_floating(
            source
                .map(|source| source.colors)
                .unwrap_or(ManaColorMask::ANY_COLOR),
            1,
        );
    }
}

fn opening_source_colors(
    mana: Option<&ManaAccessProfile>,
    card_index: usize,
    production_kind: ManaProductionKind,
) -> ManaColorMask {
    mana.and_then(|profile| profile.source(card_index))
        .map(|source| source.colors)
        .unwrap_or_else(|| {
            if production_kind == ManaProductionKind::NonRefreshingActivated {
                ManaColorMask::COLORLESS
            } else {
                ManaColorMask::ANY_COLOR
            }
        })
}

fn card_is_executable_early_acceleration(card: &CompiledCard) -> bool {
    if card.has(role::LAND) {
        return false;
    }
    if let Some(kind) = compile_typed_conditional_mana_source(card) {
        return !matches!(
            kind,
            TypedConditionalManaSource::FixedWithSourceDamage { .. }
                | TypedConditionalManaSource::ColorlessOrAnyColorWithSourceDamage { .. }
        );
    }
    match compile_typed_atomic_transaction(card) {
        Some(TypedAtomicTransaction::HandMana { .. }) => true,
        Some(
            TypedAtomicTransaction::NameLinkedGraveyardRitual { .. }
            | TypedAtomicTransaction::ThresholdRitual { .. },
        ) => card.mana_value <= 1.0,
        Some(
            TypedAtomicTransaction::SacrificeRitual { .. }
            | TypedAtomicTransaction::SacrificeTutor { .. }
            | TypedAtomicTransaction::SearchRandomDiscardShuffle { .. }
            | TypedAtomicTransaction::TemporaryLandSacrificeManaGrant { .. }
            | TypedAtomicTransaction::BargainSearchCastOrHand { .. }
            | TypedAtomicTransaction::OpponentChoiceSearchSplit,
        ) => false,
        None if card.ability_program.atomic_transaction.is_some() => false,
        None => {
            card.mana_value <= 1.0
                && card.effects.mana_produced.conservative_value(1) > 0
                && matches!(
                    card.effects.mana_production_kind,
                    ManaProductionKind::SpellResolution
                        | ManaProductionKind::ReusableActivated
                        | ManaProductionKind::OneShotActivated
                        | ManaProductionKind::NonRefreshingActivated
                )
        }
    }
}

fn card_is_executable_zero_land_acceleration(card: &CompiledCard) -> bool {
    if let Some(kind) = compile_typed_conditional_mana_source(card) {
        return matches!(
            kind,
            TypedConditionalManaSource::ImprintLinkedCardColors
                | TypedConditionalManaSource::ControlledLegendaryColors
                | TypedConditionalManaSource::MetalcraftAnyColor
        );
    }
    if matches!(
        compile_typed_atomic_transaction(card),
        Some(TypedAtomicTransaction::HandMana { .. })
    ) {
        return true;
    }
    card_is_executable_early_acceleration(card)
        && card.mana_value <= 0.0
        && (matches!(
            card.effects.mana_production_kind,
            ManaProductionKind::SpellResolution | ManaProductionKind::OneShotActivated
        ) || card.effects.mana_production_kind == ManaProductionKind::ReusableActivated
            && !card.effects.card_types.is_creature)
}

fn remaining_library_counts(
    deck: &CompiledDeck,
    cards_outside_library: &[usize],
) -> HashMap<usize, u16> {
    let mut remaining = HashMap::<usize, u16>::new();
    for card_index in &deck.library {
        let count = remaining.entry(*card_index).or_default();
        *count = count.saturating_add(1);
    }
    for card_index in cards_outside_library {
        if let Some(count) = remaining.get_mut(card_index) {
            *count = count.saturating_sub(1);
        }
    }
    remaining
}

fn tutor_has_legal_target(
    deck: &CompiledDeck,
    tutor: &CompiledCard,
    remaining_library: &HashMap<usize, u16>,
) -> bool {
    let legacy_target = tutor.effects.tutor.is_executable_on_spell_resolution()
        && tutor.effects.tutor.instructions.iter().any(|instruction| {
            instruction.source == TutorSourceZone::Library
                && instruction.quantity > 0
                && instruction.destination != TutorDestination::None
                && remaining_library.iter().any(|(card_index, count)| {
                    *count > 0
                        && deck.cards.get(*card_index).is_some_and(|candidate| {
                            instruction.target.matches(candidate.effects.card_types)
                                && (!matches!(
                                    instruction.destination,
                                    TutorDestination::BattlefieldTapped
                                        | TutorDestination::BattlefieldUntapped
                                ) || candidate.effects.card_types.is_land)
                        })
                })
        });
    legacy_target
        || compile_typed_atomic_transaction(tutor).is_some_and(|transaction| {
            transaction.is_tutor()
                && remaining_library
                    .iter()
                    .any(|(card_index, count)| *count > 0 && deck.cards.get(*card_index).is_some())
        })
        || compile_typed_first_use_self_transfer_tutor(tutor).is_some()
            && remaining_library.iter().any(|(card_index, count)| {
                *count > 0
                    && deck
                        .cards
                        .get(*card_index)
                        .is_some_and(|candidate| !candidate.is_commander)
            })
}

fn should_keep(
    hand: HandEvaluation,
    policy: MulliganPolicy,
    paid_mulligans: u8,
    declared_intent: DeckIntent,
) -> bool {
    if uses_competitive_search_envelope(policy, declared_intent) {
        return should_keep_cedh(hand, policy, paid_mulligans);
    }
    if paid_mulligans >= 3 {
        return hand.lands >= 1 && hand.lands <= 5;
    }
    let colors_are_viable = if !hand.color_requirements_known || paid_mulligans >= 2 {
        true
    } else {
        let threshold = match policy {
            MulliganPolicy::Conservative => 0.42,
            MulliganPolicy::Balanced => 0.58,
            MulliganPolicy::Aggressive => 0.70,
        };
        hand.color_coverage >= threshold
            && (hand.color_floor > 0.0 || paid_mulligans.saturating_add(1) >= 2)
    };
    match policy {
        MulliganPolicy::Conservative => {
            (2..=5).contains(&hand.lands)
                && (hand.early_actions > 0 || hand.draw > 0 || hand.ramp > 0)
                && colors_are_viable
        }
        MulliganPolicy::Balanced => {
            (2..=4).contains(&hand.lands)
                && (hand.ramp > 0
                    || hand.draw > 0
                    || hand.engines > 0
                    || hand.tutors > 0
                    || hand.early_actions >= 2)
                && colors_are_viable
        }
        MulliganPolicy::Aggressive => {
            (2..=3).contains(&hand.lands)
                && (hand.ramp > 0 || hand.engines > 0 || hand.tutors > 0)
                && hand.early_actions > 0
                && colors_are_viable
        }
    }
}

fn uses_competitive_search_envelope(policy: MulliganPolicy, declared_intent: DeckIntent) -> bool {
    matches!(policy, MulliganPolicy::Aggressive) || matches!(declared_intent, DeckIntent::Cedh)
}

fn should_keep_cedh(hand: HandEvaluation, policy: MulliganPolicy, paid_mulligans: u8) -> bool {
    debug_assert!(hand.effective_hand_strength_assessed);
    let plan_access = hand.explicit_route_access && hand.effective_hand_strength >= 0.55
        || hand.command_zone_plan_access
        || hand.independent_hand_plans > 0
        || !hand.reviewed_route_catalog_present && hand.cedh_hand_plans > 0;
    let directly_payable_one_land = hand.lands == 1 && hand.directly_payable_one_land_plan;
    let accelerated_one_land = hand.lands == 1 && hand.accelerated_one_land_plan;
    let zero_land_burst = hand.lands == 0 && hand.accelerated_zero_land_plan;
    let color_threshold = match policy {
        MulliganPolicy::Conservative => 0.38,
        MulliganPolicy::Balanced => 0.46,
        MulliganPolicy::Aggressive => 0.52,
    };
    let colors_are_viable = !hand.color_requirements_known
        || hand.color_coverage >= color_threshold
        || paid_mulligans >= 3;
    // Exact payment from the actual opening land is stronger evidence than
    // deck-wide average color coverage. This admits Island + {U} while the
    // payment witness still rejects Island + {W}.
    let plan_colors_are_viable = colors_are_viable || directly_payable_one_land;

    if paid_mulligans >= 4 {
        return plan_colors_are_viable
            && ((2..=4).contains(&hand.lands)
                || hand.lands == 1
                    && (plan_access
                        || hand.meaningful_early_actions > 0
                        || hand.executable_one_land_acceleration > 0)
                || accelerated_one_land
                || directly_payable_one_land
                || zero_land_burst);
    }

    if paid_mulligans == 3 {
        return plan_colors_are_viable
            && ((2..=4).contains(&hand.lands)
                && (plan_access || hand.meaningful_early_actions > 0 || hand.ramp > 0)
                || directly_payable_one_land
                || accelerated_one_land
                || zero_land_burst);
    }

    // Initial seven through a five-card hand is the competitive search
    // window. Mana and cheap spell counts alone do not constitute a plan:
    // there must be executable engine/draw/tutor access in hand or from the
    // command zone. Only a proven accelerated resource sequence may waive the
    // normal two-land floor.
    match policy {
        MulliganPolicy::Conservative => {
            plan_colors_are_viable
                && ((2..=4).contains(&hand.lands) && plan_access
                    || directly_payable_one_land
                    || accelerated_one_land)
        }
        MulliganPolicy::Balanced => {
            plan_colors_are_viable
                && ((2..=3).contains(&hand.lands) && plan_access
                    || directly_payable_one_land
                    || accelerated_one_land)
        }
        MulliganPolicy::Aggressive => {
            plan_colors_are_viable
                && ((2..=3).contains(&hand.lands) && plan_access
                    || directly_payable_one_land
                    || accelerated_one_land
                    || zero_land_burst)
        }
    }
}

fn generate_opening_candidate_orders(deck: &CompiledDeck, rng: &mut ChaCha8Rng) -> Vec<Vec<usize>> {
    (0..=8)
        .map(|_| {
            let mut order = deck.library.clone();
            order.shuffle(rng);
            order
        })
        .collect()
}

fn simulate_opening_episode(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    options: &AnalysisOptions,
    seed: u64,
    simulation_index: u32,
) -> OpeningEpisodeResult {
    let episode_seed = derive_episode_seed(seed, 0x4841_4e44, simulation_index);
    let mut rng = ChaCha8Rng::seed_from_u64(episode_seed);
    let candidate_orders = generate_opening_candidate_orders(deck, &mut rng);
    let sample = sample_london_hand_from_candidate_orders(deck, mana, options, &candidate_orders);
    let kept_evaluation = evaluate_cards(deck, mana, &sample.hand);
    let mut drawn_through_turn_three = sample.hand.clone();
    drawn_through_turn_three.extend(sample.draw_order.iter().take(3).copied());
    let turn_three_evaluation = evaluate_cards(deck, mana, &drawn_through_turn_three);

    OpeningEpisodeResult {
        simulation_index,
        candidate_orders,
        initial_keepable: sample.initial_keepable,
        accepted_by_policy: sample.accepted_by_policy,
        paid_mulligans: sample.paid_mulligans,
        cards_kept: sample.hand.len(),
        kept_evaluation,
        turn_three_evaluation,
    }
}

fn sample_london_hand_from_candidate_orders(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    options: &AnalysisOptions,
    candidate_orders: &[Vec<usize>],
) -> LondonHandSample {
    debug_assert_eq!(candidate_orders.len(), 9);
    let mut initial_keepable = false;
    for (attempt, order) in (0..=8u8).zip(candidate_orders) {
        let seven = order[..7].to_vec();
        let evaluation = evaluate_cards(deck, mana, &seven);
        let paid_mulligans = attempt.saturating_sub(1);
        let accepted_by_policy = should_keep(
            evaluation,
            options.mulligan_policy,
            paid_mulligans,
            options.declared_intent,
        );
        if attempt == 0 {
            initial_keepable = accepted_by_policy;
        }
        if accepted_by_policy || attempt == 8 {
            let (hand, bottomed) = choose_london_bottoms(
                deck,
                mana,
                seven,
                paid_mulligans,
                options.mulligan_policy,
                options.declared_intent,
            );
            let mut draw_order = order.iter().skip(7).copied().collect::<Vec<_>>();
            draw_order.extend(bottomed);
            return LondonHandSample {
                hand,
                draw_order,
                initial_keepable,
                accepted_by_policy,
                paid_mulligans,
            };
        }
    }
    unreachable!("the final zero-card London mulligan is always retained")
}

fn sample_london_hand(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    options: &AnalysisOptions,
    rng: &mut ChaCha8Rng,
    simulation_index: u32,
    candidate_cohort: Option<&mut OpeningCandidateCohort>,
) -> LondonHandSample {
    if let Some(cohort) = candidate_cohort {
        let candidate_orders = generate_opening_candidate_orders(deck, rng);
        for (attempt, order) in candidate_orders.iter().enumerate() {
            cohort.record(deck, simulation_index, attempt as u8, order);
        }
        return sample_london_hand_from_candidate_orders(deck, mana, options, &candidate_orders);
    }
    let mut initial_keepable = false;
    // Attempt 0 is the initial seven; attempt 1 is Commander's free
    // multiplayer mulligan; attempts 2..=8 pay one through seven London
    // mulligans. The policy usually keeps much earlier, but the rules path
    // must remain valid all the way to a zero-card hand.
    for attempt in 0..=8u8 {
        let mut order = deck.library.clone();
        order.shuffle(rng);
        let seven = order[..7].to_vec();
        let evaluation = evaluate_cards(deck, mana, &seven);
        let paid_mulligans = attempt.saturating_sub(1);
        let accepted_by_policy = should_keep(
            evaluation,
            options.mulligan_policy,
            paid_mulligans,
            options.declared_intent,
        );
        if attempt == 0 {
            initial_keepable = accepted_by_policy;
        }
        if accepted_by_policy || attempt == 8 {
            let (hand, bottomed) = choose_london_bottoms(
                deck,
                mana,
                seven,
                paid_mulligans,
                options.mulligan_policy,
                options.declared_intent,
            );
            let mut draw_order = order.into_iter().skip(7).collect::<Vec<_>>();
            draw_order.extend(bottomed);
            return LondonHandSample {
                hand,
                draw_order,
                initial_keepable,
                accepted_by_policy,
                paid_mulligans,
            };
        }
    }
    unreachable!("the final zero-card London mulligan is always retained")
}

fn choose_london_bottoms(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    hand: Vec<usize>,
    paid_mulligans: u8,
    policy: MulliganPolicy,
    declared_intent: DeckIntent,
) -> (Vec<usize>, Vec<usize>) {
    choose_route_aware_hand_reductions(
        deck,
        mana,
        hand,
        usize::from(paid_mulligans),
        paid_mulligans,
        policy,
        declared_intent,
    )
}

#[allow(clippy::too_many_arguments)]
fn choose_route_aware_hand_reductions(
    deck: &CompiledDeck,
    mana: Option<&ManaAccessProfile>,
    mut hand: Vec<usize>,
    removal_count: usize,
    quality_paid_mulligans: u8,
    policy: MulliganPolicy,
    declared_intent: DeckIntent,
) -> (Vec<usize>, Vec<usize>) {
    let mut removed_cards = Vec::new();
    for _ in 0..removal_count.min(hand.len()) {
        let current_lands = evaluate_cards(deck, mana, &hand).lands;
        let mut best_position = 0usize;
        let mut best_score = f32::NEG_INFINITY;
        let mut best_tiebreak = f32::NEG_INFINITY;
        for position in 0..hand.len() {
            let mut candidate = hand.clone();
            let removed = candidate.remove(position);
            let evaluation = evaluate_cards(deck, mana, &candidate);
            let score =
                london_hand_quality(evaluation, policy, quality_paid_mulligans, declared_intent);
            let card = &deck.cards[removed];
            let target_lands = match (declared_intent, policy) {
                (DeckIntent::Cedh, _) | (_, MulliganPolicy::Aggressive) => 2,
                _ => 3,
            };
            let land_excess = card.has(role::LAND) && current_lands > target_lands;
            let executable_functional_card = card.has(role::FAST_MANA)
                && card_has_executable_opening_mana_role(card)
                || card.has(role::TUTOR) && card_has_executable_planner_tutor_role(card);
            let tiebreak = card.mana_value + if land_excess { 8.0 } else { 0.0 }
                - if executable_functional_card {
                    3.0
                } else if card.has(role::COMBO_PIECE | role::WIN_CONDITION) {
                    2.0
                } else {
                    0.0
                };
            if score > best_score || score == best_score && tiebreak > best_tiebreak {
                best_position = position;
                best_score = score;
                best_tiebreak = tiebreak;
            }
        }
        removed_cards.push(hand.remove(best_position));
    }
    (hand, removed_cards)
}

fn london_hand_quality(
    hand: HandEvaluation,
    policy: MulliganPolicy,
    paid_mulligans: u8,
    declared_intent: DeckIntent,
) -> f32 {
    let competitive_search = uses_competitive_search_envelope(policy, declared_intent);
    let target_lands = match (declared_intent, policy) {
        (DeckIntent::Cedh, _) | (_, MulliganPolicy::Aggressive) => 2.0,
        _ => 3.0,
    };
    let keep_bonus = if should_keep(hand, policy, paid_mulligans, declared_intent) {
        100.0
    } else {
        0.0
    };
    let tutor_quality_count =
        if hand.effective_hand_strength_assessed && hand.reviewed_route_catalog_present {
            hand.route_relevant_tutors
        } else {
            hand.tutors
        };
    let hand_plan_quality_count =
        if hand.effective_hand_strength_assessed && hand.reviewed_route_catalog_present {
            hand.independent_hand_plans
                .saturating_add(hand.route_relevant_tutors)
        } else {
            hand.cedh_hand_plans
        };
    keep_bonus - (hand.lands as f32 - target_lands).abs() * 12.0
        + hand.color_coverage * 28.0
        + hand.fast_mana.min(3) as f32 * if competitive_search { 12.0 } else { 7.0 }
        + hand.ramp.min(2) as f32 * 7.0
        + hand.draw.min(2) as f32 * 6.0
        + tutor_quality_count.min(2) as f32 * 8.0
        + hand.engines.min(2) as f32 * 4.0
        + hand_plan_quality_count.min(2) as f32 * if competitive_search { 8.0 } else { 0.0 }
        + if competitive_search && hand.effective_hand_strength_assessed {
            hand.effective_hand_strength * 32.0
        } else {
            0.0
        }
        + if competitive_search && hand.explicit_route_access {
            18.0
        } else {
            0.0
        }
        + hand.early_actions.min(3) as f32 * 2.0
}

fn line_permanents_ready_before_overrun_cast(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    conversion_index: usize,
    turn: u8,
) -> bool {
    let Some(conversion) = deck.cards.get(conversion_index) else {
        return false;
    };
    let mut required = HashMap::<String, usize>::new();
    let mut skipped_conversion = false;
    for name in &line.cards {
        let normalized = crate::parser::normalize_card_name(name);
        if !skipped_conversion && normalized == conversion.normalized_name {
            skipped_conversion = true;
            continue;
        }
        let Some(card) = unique_card_by_normalized_name(deck, &normalized) else {
            return false;
        };
        if !matches!(
            modeled_line_card_kind(card),
            Some(ModeledLineCardKind::Permanent)
        ) {
            return false;
        }
        *required.entry(normalized).or_default() += 1;
    }
    skipped_conversion
        && required
            .into_iter()
            .all(|(normalized, count)| zones.usable_count(deck, &normalized, turn) >= count)
}

fn typed_overrun_cast_line_ready(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    conversion_index: usize,
    turn: u8,
    precombat: bool,
) -> bool {
    if !precombat
        || !deck
            .cards
            .get(conversion_index)
            .is_some_and(card_has_executable_variable_creature_overrun)
        || bounded_attack_capable_battlefield_count(deck, zones, turn) < 2
    {
        return false;
    }
    deck.known_lines.iter().any(|line| {
        line.simulation_requirements
            .contains(&LineRequirement::ExecutableInfiniteManaCreatureOverrunAttempt)
            && line.cards.iter().any(|name| {
                crate::parser::normalize_card_name(name)
                    == deck.cards[conversion_index].normalized_name
            })
            && line_has_executable_infinite_mana_cycle(line, deck)
            && line_permanents_ready_before_overrun_cast(line, deck, zones, conversion_index, turn)
    })
}

fn pay_exact_variable_green_overrun_cost(
    mana_pool: &mut TurnManaPool,
    cost: Option<&ManaCostProfile>,
) -> bool {
    let Some(cost) = cost.filter(|cost| cost.faces.len() == 1 && cost.confidence >= 0.999) else {
        return false;
    };
    let face = &cost.faces[0];
    if face.generic_value != 0 || face.pips.len() != 3 {
        return false;
    }
    let variable_count = face.pips.iter().filter(|pip| pip.is_variable).count();
    let green_count = face
        .pips
        .iter()
        .filter(|pip| {
            !pip.is_variable
                && !pip.is_hybrid
                && !pip.is_phyrexian
                && !pip.is_snow
                && !pip.is_unknown
                && !pip.is_colorless
                && pip.generic_value.is_none()
                && pip.colors == ManaColorMask::GREEN
        })
        .count();
    if variable_count != 1 || green_count != 2 {
        return false;
    }

    // The structurally proven permanent cycle supplies any chosen X. Only the
    // two fixed green pips are removed from the finite turn pool.
    let mut fixed_face = face.clone();
    fixed_face.pips.retain(|pip| !pip.is_variable);
    let mut paid = mana_pool.clone();
    if paid.pay_face(&fixed_face) {
        paid.resolve_pending_tap_triggers();
        *mana_pool = paid;
        true
    } else {
        false
    }
}

#[allow(clippy::too_many_arguments)]
fn select_typed_overrun_cast_from_hand(
    deck: &CompiledDeck,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
    precombat: bool,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &TurnManaPool,
    library_order: &[usize],
    next_draw_position: usize,
) -> Option<usize> {
    best_overrun_creature_target_position(deck, library_order, next_draw_position)?;
    hand.iter()
        .copied()
        .filter(|card_index| {
            typed_overrun_cast_line_ready(deck, zones, *card_index, turn, precombat)
        })
        .filter(|card_index| {
            let mut paid = mana_pool.clone();
            pay_exact_variable_green_overrun_cost(
                &mut paid,
                mana_access.and_then(|access| access.cost(*card_index)),
            )
        })
        .min_by(|left, right| {
            deck.cards[*left]
                .normalized_name
                .cmp(&deck.cards[*right].normalized_name)
                .then_with(|| left.cmp(right))
        })
}

fn best_overrun_creature_target_position(
    deck: &CompiledDeck,
    library_order: &[usize],
    next_draw_position: usize,
) -> Option<usize> {
    library_order
        .iter()
        .copied()
        .enumerate()
        .skip(next_draw_position)
        .filter_map(|(position, card_index)| {
            let card = deck.cards.get(card_index)?;
            card_is_bounded_attack_capable(card).then_some((
                position,
                card.normalized_name.as_str(),
                card_index,
            ))
        })
        .min_by(|left, right| left.1.cmp(right.1).then_with(|| left.2.cmp(&right.2)))
        .map(|(position, _, _)| position)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UnboundedColorlessManaCapability {
    source_line_index: usize,
}

impl UnboundedColorlessManaCapability {
    fn pays_complete_ability_cost(&self, cost: &ProgramManaCost) -> bool {
        let ProgramManaCost::PrintedSymbols { profile, .. } = cost else {
            return false;
        };
        profile.white == 0
            && profile.blue == 0
            && profile.black == 0
            && profile.red == 0
            && profile.green == 0
            && profile.variable_x == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnboundedExhaustiveAccessSource {
    card_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UnboundedExhaustiveAccessResult {
    source_line_index: usize,
    access_source_index: usize,
    conversion_index: usize,
    activations: u16,
}

fn revalidated_unbounded_colorless_capability(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    creature_count: u8,
    mana_sources: &[BattlefieldManaSource],
    turn: u8,
    activation_costs: &[CompiledLineActivationCost],
    mana_pool: &TurnManaPool,
) -> Option<(UnboundedColorlessManaCapability, TurnManaPool)> {
    deck.known_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            line.is_infinite
                && !line.table_lethal_if_resolved
                && line.outcome == crate::domain::KnownLineOutcome::InfiniteMana
                && line
                    .simulation_requirements
                    .contains(&LineRequirement::ReviewedInfiniteManaLoop)
        })
        .find_map(|(line_index, line)| {
            if !line_pieces_usable_together(line, deck, zones, turn)
                || !line_requirements_met(line, deck, zones, creature_count, mana_sources, turn)
                || !line_has_executable_infinite_mana_cycle(line, deck)
            {
                return None;
            }

            let mut staged_pool = mana_pool.clone();
            match activation_costs.get(line_index) {
                Some(CompiledLineActivationCost::None) => {}
                Some(CompiledLineActivationCost::Additional(cost))
                    if activation_cost_is_exactly_modeled(cost)
                        && staged_pool.pay(Some(cost), 0, 0) => {}
                Some(
                    CompiledLineActivationCost::Additional(_)
                    | CompiledLineActivationCost::Unmodeled,
                )
                | None => return None,
            }

            Some((
                UnboundedColorlessManaCapability {
                    source_line_index: line_index,
                },
                staged_pool,
            ))
        })
}

fn exact_unbounded_exhaustive_access_source(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    capability: UnboundedColorlessManaCapability,
    turn: u8,
) -> Option<UnboundedExhaustiveAccessSource> {
    zones
        .battlefield
        .iter()
        .filter(|presence| presence.entered_turn <= turn)
        .filter_map(|presence| {
            let card = deck.cards.get(presence.card_index)?;
            if card
                .ability_program
                .unsupported_abilities()
                .next()
                .is_some()
            {
                return None;
            }
            card.ability_program
                .executable_abilities()
                .filter_map(|ability| {
                    if !matches!(ability.timing, AbilityTiming::Activated { .. })
                        || ability.costs.len() != 1
                        || ability.preconditions
                            != [AbilityPrecondition::SourceZone(ProgramZone::Battlefield)]
                        || ability.effects.len() != 1
                    {
                        return None;
                    }
                    let AbilityCost::Mana(cost) = &ability.costs[0] else {
                        return None;
                    };
                    let AbilityEffect::ExhaustiveTopCardAccess(effect) = &ability.effects[0] else {
                        return None;
                    };
                    (capability.pays_complete_ability_cost(cost)
                        && effect.scry_count == 1
                        && effect.reveal
                        && effect.land_destination == ProgramZone::Battlefield
                        && effect.land_enters_tapped
                        && effect.nonland_destination == ProgramZone::Hand)
                        .then_some(UnboundedExhaustiveAccessSource {
                            card_index: presence.card_index,
                        })
                })
                .next()
        })
        .min_by(|left, right| {
            deck.cards[left.card_index]
                .normalized_name
                .cmp(&deck.cards[right.card_index].normalized_name)
                .then_with(|| left.card_index.cmp(&right.card_index))
        })
}

#[allow(clippy::too_many_arguments)]
fn execute_unbounded_exhaustive_access_to_typed_conversion(
    deck: &CompiledDeck,
    hand: &mut Vec<usize>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    zones: &mut KnownLineZoneState,
    mana_sources: &mut Vec<BattlefieldManaSource>,
    turn: u8,
    precombat: bool,
    creature_count: u8,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &mut TurnManaPool,
    activation_costs: &[CompiledLineActivationCost],
) -> Option<UnboundedExhaustiveAccessResult> {
    if select_typed_overrun_cast_from_hand(
        deck,
        hand,
        zones,
        turn,
        precombat,
        mana_access,
        mana_pool,
        library_order,
        next_draw_position,
    )
    .is_some()
    {
        return None;
    }

    let (capability, mut staged_pool) = revalidated_unbounded_colorless_capability(
        deck,
        zones,
        creature_count,
        mana_sources,
        turn,
        activation_costs,
        mana_pool,
    )?;
    let access_source = exact_unbounded_exhaustive_access_source(deck, zones, capability, turn)?;
    let mut staged_hand = hand.clone();
    let mut staged_library = library_order.clone();
    let mut staged_zones = zones.clone();
    let mut staged_mana_sources = mana_sources.clone();
    let mut activations = 0u16;

    while next_draw_position < staged_library.len() {
        // With repeatable, unbounded access, leaving the scryed card on top and
        // resolving the complete reveal branch visits each real library object
        // once in stable order. No hidden position is inspected to choose a
        // branch, and the transaction is committed only after a typed,
        // presently payable conversion has actually reached hand.
        let card_index = staged_library.remove(next_draw_position);
        let card = deck.cards.get(card_index)?;
        if card_has_visible_multiface_identity(card) {
            // The compiled root does not retain the face identity needed to
            // apply library characteristics for a multifaced card. Treating a
            // land back face as the library object's active face could create
            // both mana and an attacker, so the complete transaction fails
            // closed until face-bound execution exists.
            return None;
        }
        activations = activations.checked_add(1)?;
        if card.effects.card_types.is_land {
            let source = battlefield_tutored_land_source(deck, mana_access, card_index, turn, true);
            staged_mana_sources.push(source);
            staged_zones.record_put_onto_battlefield(deck, card_index, turn);
        } else {
            staged_hand.push(card_index);
        }

        let Some(conversion_index) = select_typed_overrun_cast_from_hand(
            deck,
            &staged_hand,
            &staged_zones,
            turn,
            precombat,
            mana_access,
            &staged_pool,
            &staged_library,
            next_draw_position,
        ) else {
            continue;
        };

        *hand = staged_hand;
        *library_order = staged_library;
        *zones = staged_zones;
        *mana_sources = staged_mana_sources;
        *mana_pool = std::mem::take(&mut staged_pool);
        return Some(UnboundedExhaustiveAccessResult {
            source_line_index: capability.source_line_index,
            access_source_index: access_source.card_index,
            conversion_index,
            activations,
        });
    }

    None
}

#[allow(clippy::too_many_arguments)]
fn enqueue_unbounded_exhaustive_access_conversion(
    deck: &CompiledDeck,
    hand: &mut Vec<usize>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    zones: &mut KnownLineZoneState,
    mana_sources: &mut Vec<BattlefieldManaSource>,
    turn: u8,
    precombat: bool,
    creature_count: u8,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &mut TurnManaPool,
    activation_costs: &[CompiledLineActivationCost],
    planned_cast_queue: &mut VecDeque<TurnAction>,
) -> bool {
    let Some(access) = execute_unbounded_exhaustive_access_to_typed_conversion(
        deck,
        hand,
        library_order,
        next_draw_position,
        zones,
        mana_sources,
        turn,
        precombat,
        creature_count,
        mana_access,
        mana_pool,
        activation_costs,
    ) else {
        return false;
    };

    debug_assert!(deck.known_lines.get(access.source_line_index).is_some());
    debug_assert!(
        zones
            .battlefield
            .iter()
            .any(|presence| presence.card_index == access.access_source_index)
    );
    debug_assert!(access.activations > 0);
    planned_cast_queue.clear();
    planned_cast_queue.push_back(TurnAction::Cast(access.conversion_index));
    true
}

#[allow(clippy::too_many_arguments)]
fn resolve_typed_overrun_creature_tutor(
    deck: &CompiledDeck,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    turn: u8,
    mana_access: Option<&ManaAccessProfile>,
    mana_sources: &mut Vec<BattlefieldManaSource>,
    mana_pool: &mut TurnManaPool,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
    engine_count: &mut u8,
    enabler_count: &mut u8,
    payoff_count: &mut u8,
    creature_count: &mut u8,
    protection_count: &mut u8,
) -> bool {
    let Some(target_position) =
        best_overrun_creature_target_position(deck, library_order, next_draw_position)
    else {
        return false;
    };
    let target_index = library_order.remove(target_position);
    let Some(target) = deck.cards.get(target_index) else {
        return false;
    };

    let unseen_start = next_draw_position.min(library_order.len());
    library_order[unseen_start..].shuffle(rng);
    zones.record_put_onto_battlefield(deck, target_index, turn);
    apply_roles(
        target.roles,
        engine_count,
        enabler_count,
        payoff_count,
        creature_count,
        protection_count,
    );
    install_tutored_permanent_runtime(
        deck,
        target_index,
        mana_access,
        turn,
        mana_sources,
        mana_pool,
        rng,
        None,
        zones,
    );
    true
}

fn prepare_episode(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    options: &AnalysisOptions,
    seed: u64,
) -> PreparedEpisode {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let opening = sample_london_hand(deck, mana_access, options, &mut rng, 0, None);
    PreparedEpisode {
        opening,
        rng_after_opening: rng,
        opponent_timeline: OpponentEventTimeline::for_episode(seed, options.maximum_turn),
        table_activity_timeline: TableActivityTimeline::for_episode(seed, options.maximum_turn),
    }
}

fn simulate_prepared_episode_condition(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    options: &AnalysisOptions,
    profile: InteractionProfile,
    isolated_scenario: Option<InteractionScenario>,
    preparation: &PreparedEpisode,
    runtime_model: &SimulationRuntimeModel,
) -> EpisodeSimulationResult {
    let mut rng = preparation.rng_after_opening.clone();
    let opening = preparation.opening.clone();
    let mut order = opening.draw_order;
    let mut hand = opening.hand;
    let mut position = 0usize;
    let parameters = interaction_parameters(profile);
    let opponent_timeline = &preparation.opponent_timeline;
    let table_activity_timeline = &preparation.table_activity_timeline;

    let mut mana_sources = Vec::<BattlefieldManaSource>::new();
    let mut engine_count = 0u8;
    let mut enabler_count = 0u8;
    let mut payoff_count = 0u8;
    let mut creature_count = 0u8;
    let mut protection_count = 0u8;
    let mut recursion_count = 0u8;
    let mut treasure_reserve = 0u8;
    let mut pending_engine_recovery = 0u8;
    let mut extra_turn_credit = 0u8;
    let mut commanders_cast = HashSet::<usize>::new();
    let mut commander_cast_counts = HashMap::<usize, u8>::new();
    let mut line_zones = KnownLineZoneState::default();
    let mut pending_delayed_card_access = Vec::<PendingDelayedCardAccess>::new();
    let mut player_life = COMMANDER_STARTING_LIFE;
    let mut combat_state = CommanderCombatState::new();
    let mut first_attempt_opportunity = false;
    let mut first_attempt_stopped = false;
    let mut first_win_attempt_turn = None;
    let mut timing_provenance = EpisodeTimingProvenance::default();
    let mut recovered_attempt = false;
    let mut may_attempt_after = 0u8;
    let mut first_credible_threat_turn = None;
    let mut isolated = IsolatedScenarioRuntime::new(isolated_scenario);

    for turn in 1..=options.maximum_turn {
        let opponent_rolls = opponent_timeline
            .turn(turn)
            .expect("opponent event timeline covers every simulated turn");
        let mut spells_cast_this_turn = 0u8;
        line_zones.begin_turn();
        engine_count = engine_count.saturating_add(pending_engine_recovery);
        pending_engine_recovery = 0;

        let (reserved_attackers, attack_treasures) =
            reserve_dwarf_attack_taps(deck, &line_zones, &mana_sources, turn);
        // Dwarf attack-tap resources force the remaining modeled actions into
        // the postcombat main phase. With no such reservation, the normal
        // planning window remains the precombat main phase and may support an
        // explicitly typed haste-overrun attempt.
        let cast_phase_is_precombat = reserved_attackers.is_empty();
        let unreserved_mana_sources = (!reserved_attackers.is_empty()).then(|| {
            mana_sources
                .iter()
                .copied()
                .filter(|source| {
                    source
                        .card_index
                        .is_none_or(|card_index| !reserved_attackers.contains(&card_index))
                })
                .collect::<Vec<_>>()
        });
        let sources_for_pool = unreserved_mana_sources
            .as_deref()
            .unwrap_or(mana_sources.as_slice());
        let ability_context = active_ability_context(deck, &line_zones);
        let mut mana_pool = TurnManaPool::from_battlefield_with_ability_context(
            sources_for_pool,
            turn,
            deck,
            ability_context,
            &line_zones,
        );
        mana_pool.add_treasures(treasure_reserve);
        if !resolve_controller_upkeep_creature_triggers(
            deck,
            &mut line_zones,
            turn,
            &mut player_life,
        ) {
            return isolated.finish(EpisodeOutcome {
                threat_turn: first_credible_threat_turn,
                first_win_attempt_turn,
                resolved_table_win_turn: None,
                timing_provenance,
                first_attempt_opportunity,
                first_attempt_stopped,
                recovered: recovered_attempt,
                final_life: player_life,
                player_died: true,
            });
        }
        let upkeep_removed =
            resolve_cumulative_upkeep_triggers(deck, &mut line_zones, &mut mana_pool);
        remove_persistent_contributions(
            deck,
            &upkeep_removed,
            &mut commanders_cast,
            &mut engine_count,
            &mut enabler_count,
            &mut payoff_count,
            &mut creature_count,
            &mut protection_count,
            &mut recursion_count,
        );
        synchronize_mana_sources_with_battlefield(&mut mana_sources, &line_zones);
        synchronize_turn_pool_with_battlefield(&mut mana_pool, &line_zones);
        if !mana_pool.settle_pending_source_damage(&mut player_life) {
            return isolated.finish(EpisodeOutcome {
                threat_turn: first_credible_threat_turn,
                first_win_attempt_turn,
                resolved_table_win_turn: None,
                timing_provenance,
                first_attempt_opportunity,
                first_attempt_stopped,
                recovered: recovered_attempt,
                final_life: player_life,
                player_died: true,
            });
        }

        // Untap/upkeep is complete. Instant top-library tutors are resolved in
        // the prior opponent end step below, so their reusable payment sources
        // have untapped before this draw. Do not spend freshly untapped
        // combo-turn mana in a fictitious upkeep window.
        execute_modeled_draw_step(deck, &mut hand, &order, &mut position, &line_zones);
        let mut deferred_land_play = false;
        if let Some(hand_position) = best_land_position(
            deck,
            mana_access,
            &hand,
            &order,
            position,
            turn,
            &mana_pool,
            &line_zones,
        ) {
            if should_defer_only_land_to_diamond(
                deck,
                mana_access,
                &hand,
                hand_position,
                turn,
                &mana_pool,
                &line_zones,
            ) {
                deferred_land_play = true;
            } else {
                let land_index = hand.swap_remove(hand_position);
                let land_resolution = execute_land_play(
                    deck,
                    mana_access,
                    land_index,
                    turn,
                    &hand,
                    &mut order,
                    position,
                    &mut rng,
                    &mut line_zones,
                    &mut mana_sources,
                    &mut mana_pool,
                    &mut player_life,
                );
                if land_resolution.player_died {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
            }
        }

        mana_pool.add_treasures(attack_treasures);
        mana_pool.refresh_battlefield_sources(
            deck,
            &line_zones,
            active_ability_context(deck, &line_zones),
        );
        let pressure = if parameters.mana_pressure > 0 && turn <= 4 {
            parameters.mana_pressure
        } else {
            0
        };
        mana_pool.apply_pressure(pressure);
        mana_pool.resolve_pending_tap_triggers();
        mana_pool.refresh_battlefield_sources(
            deck,
            &line_zones,
            active_ability_context(deck, &line_zones),
        );
        if !mana_pool.settle_pending_source_damage(&mut player_life) {
            return isolated.finish(EpisodeOutcome {
                threat_turn: first_credible_threat_turn,
                first_win_attempt_turn,
                resolved_table_win_turn: None,
                timing_provenance,
                first_attempt_opportunity,
                first_attempt_stopped,
                recovered: recovered_attempt,
                final_life: player_life,
                player_died: true,
            });
        }

        // The command zone is public, but casting from it is not automatically
        // better than a line already executable from the current hand. The
        // eager commander pass used to spend that line's mana before the
        // bounded planner could act. Defer priority commanders for this turn
        // whenever the same fail-closed planner can already complete a
        // credible reviewed threat/conversion with the unspent pool.
        let hand_domain_before_commander = CastPlanningDomain {
            deck,
            mana_access,
            zones: &line_zones,
            turn,
            policy: options.pilot_policy,
            additional_generic_per_cast: 0,
            player_life,
            spells_cast_this_turn,
            opponent_end_step: Some(OpponentEndStepPlanningContext {
                maximum_turn: options.maximum_turn,
                additional_generic_per_cast: u8::from(
                    isolated.is(InteractionScenario::GenericTaxStax) && isolated.applied(),
                ),
                // A pending isolated response is applied only at the real
                // cast checkpoint. Letting scenario identity suppress this
                // public plan beforehand can change the paired trajectory
                // without an opportunity or effectful intervention.
                first_relevant_spell_will_be_countered: false,
                rule_of_law_cap_active: isolated.is(InteractionScenario::RuleOfLawCap)
                    && isolated.applied(),
                mana_sources: mana_sources.clone(),
            }),
        };
        let has_uncast_priority_commander = deck.commanders.iter().copied().any(|card_index| {
            !commanders_cast.contains(&card_index)
                && deck
                    .cards
                    .get(card_index)
                    .is_some_and(|card| commander_is_priority(card, options.pilot_policy))
        });
        let defer_priority_commanders_for_hand_route = has_uncast_priority_commander
            && hand_plan_completes_credible_executable_route(
                &hand_domain_before_commander,
                &hand,
                &mana_pool,
            );

        // Each selected commander is an independent command-zone object. Try
        // every distinct commander once per turn, cheapest first, so partners
        // can both be deployed when the remaining pool pays for them. The
        // per-turn attempted set prevents a counter/removal directive from
        // creating an immediate zero-cost recast loop.
        let mut commanders_attempted_this_turn = HashSet::<usize>::new();
        while let Some(commander_index) = deck
            .commanders
            .iter()
            .copied()
            .filter(|_| !defer_priority_commanders_for_hand_route)
            .filter(|index| !commanders_cast.contains(index))
            .filter(|index| !commanders_attempted_this_turn.contains(index))
            .filter(|index| {
                deck.cards
                    .get(*index)
                    .is_some_and(|card| commander_is_priority(card, options.pilot_policy))
            })
            .min_by(|left, right| {
                let total_cost = |index: usize| {
                    deck.cards[index].mana_value
                        + f32::from(
                            commander_cast_counts
                                .get(&index)
                                .copied()
                                .unwrap_or_default()
                                .saturating_mul(2),
                        )
                };
                total_cost(*left)
                    .total_cmp(&total_cost(*right))
                    .then_with(|| left.cmp(right))
            })
        {
            commanders_attempted_this_turn.insert(commander_index);
            if rule_of_law_blocks_next_spell(&mut isolated, turn, spells_cast_this_turn) {
                break;
            }
            let commander = &deck.cards[commander_index];
            let commander_cost = commander.mana_value.ceil() as u8;
            let commander_tax = commander_cast_counts
                .get(&commander_index)
                .copied()
                .unwrap_or_default()
                .saturating_mul(2);
            let reserved_mana = commander_mana_reserve(commander, options.pilot_policy);
            let live_eot_domain = CastPlanningDomain {
                deck,
                mana_access,
                zones: &line_zones,
                turn,
                policy: options.pilot_policy,
                additional_generic_per_cast: u8::from(
                    isolated.is(InteractionScenario::GenericTaxStax) && isolated.applied(),
                ),
                player_life,
                spells_cast_this_turn,
                opponent_end_step: Some(OpponentEndStepPlanningContext {
                    maximum_turn: options.maximum_turn,
                    additional_generic_per_cast: u8::from(
                        isolated.is(InteractionScenario::GenericTaxStax) && isolated.applied(),
                    ),
                    first_relevant_spell_will_be_countered: false,
                    rule_of_law_cap_active: isolated.is(InteractionScenario::RuleOfLawCap)
                        && isolated.applied(),
                    mana_sources: mana_sources.clone(),
                }),
            };
            let preserve_eot_tutor =
                planner_prefers_opponent_end_step_top_tutor(&live_eot_domain, &hand, &mana_pool);
            let Some(commander_payment) = commander_payment_at_generic_tax_checkpoint(
                &mut isolated,
                turn,
                &live_eot_domain,
                &hand,
                &mana_pool,
                commander_index,
                mana_access.and_then(|access| access.cost(commander_index)),
                commander_cost,
                commander_tax,
                reserved_mana,
                preserve_eot_tutor,
            ) else {
                continue;
            };
            mana_pool = commander_payment.mana_pool;
            line_zones = commander_payment.zones;
            {
                if !mana_pool.settle_pending_source_damage(&mut player_life) {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
                let cast_count = commander_cast_counts.entry(commander_index).or_default();
                *cast_count = cast_count.saturating_add(1);
                spells_cast_this_turn = spells_cast_this_turn.saturating_add(1);
                if !apply_controller_spell_cast_triggers(
                    deck,
                    &mut line_zones,
                    commander_index,
                    turn,
                    spells_cast_this_turn,
                    &mut mana_pool,
                    &mut player_life,
                ) {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
                let relevant_commander_spell = scenario_relevant_spell(commander);
                if relevant_commander_spell {
                    isolated.observe_opportunity(InteractionScenario::FirstRelevantSpellCountered);
                }
                if isolated.is(InteractionScenario::FirstRelevantSpellCountered)
                    && !isolated.applied()
                    && relevant_commander_spell
                {
                    isolated.activate(turn, 1);
                } else {
                    let commander_recast_recovery = isolated
                        .is(InteractionScenario::CommanderRemovalRecast)
                        && isolated.applied();
                    let counter_recovery = isolated
                        .is(InteractionScenario::FirstRelevantSpellCountered)
                        && isolated.applied();
                    let graveyard_action = scenario_executable_graveyard_action(commander);
                    if graveyard_action {
                        isolated.observe_opportunity(InteractionScenario::GraveyardShutdown);
                    }
                    let graveyard_suppressed =
                        isolated.is(InteractionScenario::GraveyardShutdown) && graveyard_action;
                    if graveyard_suppressed && !isolated.applied() {
                        isolated.activate(turn, 1);
                    }
                    commanders_cast.insert(commander_index);
                    // A resolved commander is public battlefield state before
                    // its static/triggered mana effects and before the active
                    // player receives priority. This can immediately enable a
                    // pre-existing Mox Amber or alter Metalcraft.
                    line_zones.record_cast(deck, commander_index, turn);
                    apply_roles(
                        commander.roles,
                        &mut engine_count,
                        &mut enabler_count,
                        &mut payoff_count,
                        &mut creature_count,
                        &mut protection_count,
                    );
                    let mana_sources_before = mana_sources.len();
                    let ability_context = active_ability_context(deck, &line_zones);
                    apply_cast_mana_effects(
                        deck,
                        commander_index,
                        commander,
                        mana_access,
                        turn,
                        &mut mana_sources,
                        &mut mana_pool,
                        &mut rng,
                        ability_context,
                        &line_zones,
                    );
                    mana_pool.refresh_battlefield_sources(
                        deck,
                        &line_zones,
                        active_ability_context(deck, &line_zones),
                    );
                    creature_count =
                        creature_count.saturating_add(immediate_creature_tokens(commander));
                    extra_turn_credit =
                        extra_turn_credit.saturating_add(immediate_extra_turns(commander));
                    if commander.effects.recursion && !graveyard_suppressed {
                        recursion_count = recursion_count.saturating_add(1);
                    }
                    resolve_immediate_spell_draws(commander, &mut hand, &order, &mut position);
                    if commander_recast_recovery || counter_recovery {
                        isolated.recover(turn);
                    }
                    if isolated.is(InteractionScenario::GenericTaxStax) {
                        isolated.recover(turn);
                    }
                    if isolated.is(InteractionScenario::RuleOfLawCap)
                        && isolated
                            .applied_turn
                            .is_some_and(|applied_turn| turn > applied_turn)
                    {
                        isolated.recover(turn);
                    }
                    if !commander_recast_recovery {
                        isolated.observe_opportunity(InteractionScenario::CommanderRemovalRecast);
                    }
                    if isolated.is(InteractionScenario::CommanderRemovalRecast)
                        && !commander_recast_recovery
                        && isolated.activate(turn, 1)
                    {
                        commanders_cast.remove(&commander_index);
                        remove_roles(
                            commander.roles,
                            &mut engine_count,
                            &mut enabler_count,
                            &mut payoff_count,
                            &mut creature_count,
                            &mut protection_count,
                        );
                        if commander.effects.recursion {
                            recursion_count = recursion_count.saturating_sub(1);
                        }
                        mana_sources.truncate(mana_sources_before);
                        line_zones.remove_named_permanent(deck, &commander.normalized_name, false);
                    }
                    apply_isolated_creature_wipe_if_ready(
                        &mut isolated,
                        deck,
                        turn,
                        &mut line_zones,
                        &mut commanders_cast,
                        &mut engine_count,
                        &mut enabler_count,
                        &mut payoff_count,
                        &mut creature_count,
                        &mut protection_count,
                        &mut recursion_count,
                    );
                    synchronize_mana_sources_with_battlefield(&mut mana_sources, &line_zones);
                    synchronize_turn_pool_with_battlefield(&mut mana_pool, &line_zones);
                }
            }
        }

        // Receding-horizon execution: the planner sees only the current hand,
        // battlefield-derived state, and remaining mana. Commit its first
        // cast, resolve the real draw/tutor outcome, then plan again. Hidden
        // library order is never passed into the speculative state.
        let mut committed_actions = 0usize;
        let mut planned_cast_queue = VecDeque::<TurnAction>::new();
        let mut executed_graveyard_storm_line = None;
        while committed_actions < CAST_PLANNER_MAX_COMMITTED_ACTIONS {
            if let Some(line_index) = execute_graveyard_storm_under_isolated_scenario(
                deck,
                &mut hand,
                &mut order,
                position,
                &mut line_zones,
                turn,
                mana_access,
                &mut mana_pool,
                &mut spells_cast_this_turn,
                &mut player_life,
                &mut isolated,
            ) {
                if player_life <= 0.0 {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
                if !mana_pool.settle_pending_source_damage(&mut player_life) {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
                executed_graveyard_storm_line = Some(line_index);
                break;
            }

            let activated_tax_for_future_spells =
                u8::from(isolated.is(InteractionScenario::GenericTaxStax) && isolated.applied());
            let bypass_eot_domain = CastPlanningDomain {
                deck,
                mana_access,
                zones: &line_zones,
                turn,
                policy: options.pilot_policy,
                additional_generic_per_cast: activated_tax_for_future_spells,
                player_life,
                spells_cast_this_turn,
                opponent_end_step: Some(OpponentEndStepPlanningContext {
                    maximum_turn: options.maximum_turn,
                    additional_generic_per_cast: activated_tax_for_future_spells,
                    first_relevant_spell_will_be_countered: false,
                    rule_of_law_cap_active: isolated.is(InteractionScenario::RuleOfLawCap)
                        && isolated.applied(),
                    mana_sources: mana_sources.clone(),
                }),
            };
            let preserve_eot_before_bypasses =
                planner_prefers_opponent_end_step_top_tutor(&bypass_eot_domain, &hand, &mana_pool);
            let reviewed_primer_window_preempts_eager_tutor =
                reviewed_primer_window_preempts_eager_tutor(&bypass_eot_domain, &hand, &mana_pool);
            let eager_first_use_preserves_eot = !reviewed_primer_window_preempts_eager_tutor
                && preserve_eot_before_bypasses
                && active_first_use_self_transfer_tutor(deck, &line_zones).is_some_and(
                    |(source_sequence, _, tutor)| {
                        let mut candidate_pool = mana_pool.clone();
                        let mut candidate_life = player_life;
                        let mut candidate_zones = line_zones.clone();
                        pay_first_use_self_transfer_tutor_activation(&mut candidate_pool, &tutor)
                            && candidate_pool.settle_pending_source_damage(&mut candidate_life)
                            && candidate_zones
                                .remove_permanent_sequence(deck, source_sequence, false)
                                .is_some()
                            && {
                                synchronize_turn_pool_with_battlefield(
                                    &mut candidate_pool,
                                    &candidate_zones,
                                );
                                let candidate_domain = CastPlanningDomain {
                                    deck,
                                    mana_access,
                                    zones: &candidate_zones,
                                    turn,
                                    policy: options.pilot_policy,
                                    additional_generic_per_cast: activated_tax_for_future_spells,
                                    player_life: candidate_life,
                                    spells_cast_this_turn,
                                    opponent_end_step: bypass_eot_domain.opponent_end_step.clone(),
                                };
                                planner_prefers_opponent_end_step_top_tutor(
                                    &candidate_domain,
                                    &hand,
                                    &candidate_pool,
                                )
                            }
                    },
                );
            if eot_reservation_allows_candidate(
                preserve_eot_before_bypasses,
                eager_first_use_preserves_eot,
            ) && !reviewed_primer_window_preempts_eager_tutor
                && let Some(resolution) = execute_first_use_self_transfer_tutor(
                    deck,
                    mana_access,
                    &mut order,
                    position,
                    &mut hand,
                    turn,
                    &mut mana_pool,
                    &mut rng,
                    &mut line_zones,
                    activated_tax_for_future_spells,
                )
            {
                if !mana_pool.settle_pending_source_damage(&mut player_life) {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
                finalize_first_use_self_transfer_tutor_runtime(
                    deck,
                    resolution,
                    &mut mana_sources,
                    &mut mana_pool,
                    &line_zones,
                    &mut engine_count,
                    &mut enabler_count,
                    &mut payoff_count,
                    &mut creature_count,
                    &mut protection_count,
                    &mut recursion_count,
                );
                planned_cast_queue.clear();
                committed_actions = committed_actions.saturating_add(1);
                continue;
            }

            let resource_eot_domain = CastPlanningDomain {
                deck,
                mana_access,
                zones: &line_zones,
                turn,
                policy: options.pilot_policy,
                additional_generic_per_cast: activated_tax_for_future_spells,
                player_life,
                spells_cast_this_turn,
                opponent_end_step: Some(OpponentEndStepPlanningContext {
                    maximum_turn: options.maximum_turn,
                    additional_generic_per_cast: activated_tax_for_future_spells,
                    first_relevant_spell_will_be_countered: false,
                    rule_of_law_cap_active: isolated.is(InteractionScenario::RuleOfLawCap)
                        && isolated.applied(),
                    mana_sources: mana_sources.clone(),
                }),
            };
            let preserve_eot_before_resource = planner_prefers_opponent_end_step_top_tutor(
                &resource_eot_domain,
                &hand,
                &mana_pool,
            );
            let staged_resource_tutor = stage_resource_tutor_ability(
                deck,
                mana_access,
                &order,
                position,
                &hand,
                turn,
                &mana_sources,
                &mana_pool,
                &rng,
                &line_zones,
            );
            let resource_tutor_preserves_eot =
                staged_resource_tutor.as_ref().is_some_and(|staged| {
                    let staged_eot_domain = CastPlanningDomain {
                        deck,
                        mana_access,
                        zones: &staged.zones,
                        turn,
                        policy: options.pilot_policy,
                        additional_generic_per_cast: activated_tax_for_future_spells,
                        player_life,
                        spells_cast_this_turn,
                        opponent_end_step: Some(OpponentEndStepPlanningContext {
                            maximum_turn: options.maximum_turn,
                            additional_generic_per_cast: activated_tax_for_future_spells,
                            first_relevant_spell_will_be_countered: false,
                            rule_of_law_cap_active: isolated.is(InteractionScenario::RuleOfLawCap)
                                && isolated.applied(),
                            mana_sources: staged.mana_sources.clone(),
                        }),
                    };
                    planner_prefers_opponent_end_step_top_tutor(
                        &staged_eot_domain,
                        &staged.hand,
                        &staged.mana_pool,
                    )
                });
            let commit_staged_resource_tutor =
                staged_resource_tutor.as_ref().is_some_and(|staged| {
                    should_commit_staged_resource_tutor(
                        deck,
                        turn,
                        &line_zones,
                        preserve_eot_before_resource,
                        resource_tutor_preserves_eot,
                        staged,
                    )
                });
            if commit_staged_resource_tutor && let Some(staged) = staged_resource_tutor {
                let target_index = staged.target_index;
                let target_entered_battlefield = staged.target_entered_battlefield;
                order = staged.library_order;
                hand = staged.hand;
                mana_sources = staged.mana_sources;
                mana_pool = staged.mana_pool;
                rng = staged.rng;
                line_zones = staged.zones;
                if !mana_pool.settle_pending_source_damage(&mut player_life) {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
                if target_entered_battlefield {
                    let target = &deck.cards[target_index];
                    apply_roles(
                        target.roles,
                        &mut engine_count,
                        &mut enabler_count,
                        &mut payoff_count,
                        &mut creature_count,
                        &mut protection_count,
                    );
                    if target.effects.recursion {
                        recursion_count = recursion_count.saturating_add(1);
                    }
                }
                planned_cast_queue.clear();
                committed_actions = committed_actions.saturating_add(1);
                continue;
            }
            // Rule of Law limits spells, not activated abilities. Let the
            // exhaustive-access bridge commit its real activations and expose
            // the conversion; the shared pre-cast checkpoint below records
            // and blocks an actual second-spell attempt.
            if enqueue_unbounded_exhaustive_access_conversion(
                deck,
                &mut hand,
                &mut order,
                position,
                &mut line_zones,
                &mut mana_sources,
                turn,
                cast_phase_is_precombat,
                creature_count,
                mana_access,
                &mut mana_pool,
                &runtime_model.line_activation_costs,
                &mut planned_cast_queue,
            ) {
                if !mana_pool.settle_pending_source_damage(&mut player_life) {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
                committed_actions = committed_actions.saturating_add(1);
            }
            let prospective_generic_tax =
                u8::from(isolated.is(InteractionScenario::GenericTaxStax) && isolated.applied());
            let planning_domain = CastPlanningDomain {
                deck,
                mana_access,
                zones: &line_zones,
                turn,
                policy: options.pilot_policy,
                additional_generic_per_cast: prospective_generic_tax,
                player_life,
                spells_cast_this_turn,
                opponent_end_step: Some(OpponentEndStepPlanningContext {
                    maximum_turn: options.maximum_turn,
                    additional_generic_per_cast: prospective_generic_tax,
                    first_relevant_spell_will_be_countered: false,
                    rule_of_law_cap_active: isolated.is(InteractionScenario::RuleOfLawCap)
                        && isolated.applied(),
                    mana_sources: mana_sources.clone(),
                }),
            };
            let planning_rule_of_law_cap_active = planning_domain.rule_of_law_cap_active();
            if planned_cast_queue.is_empty() {
                planned_cast_queue.extend(if cast_phase_is_precombat {
                    plan_hand_action_order_with_combat(
                        &planning_domain,
                        &hand,
                        &mana_pool,
                        &combat_state,
                    )
                } else {
                    plan_hand_action_order(&planning_domain, &hand, &mana_pool)
                });
            }
            if deferred_land_play
                && line_zones.land_sacrifice_mana_grant_turn == Some(turn)
                && !queued_plan_reserves_deferred_land_for_diamond(deck, &planned_cast_queue)
                && let Some(hand_position) = best_land_position(
                    deck,
                    mana_access,
                    &hand,
                    &order,
                    position,
                    turn,
                    &mana_pool,
                    &line_zones,
                )
            {
                // The land was held only to preserve a possible Mox Diamond
                // entry choice. Once the actual queue declines that line and
                // Rain is live, play the land now and replan so its newly
                // granted exact activation can be selected this turn.
                let land_index = hand.swap_remove(hand_position);
                let land_resolution = execute_land_play(
                    deck,
                    mana_access,
                    land_index,
                    turn,
                    &hand,
                    &mut order,
                    position,
                    &mut rng,
                    &mut line_zones,
                    &mut mana_sources,
                    &mut mana_pool,
                    &mut player_life,
                );
                if land_resolution.player_died {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
                deferred_land_play = false;
                planned_cast_queue.clear();
                continue;
            }
            let preserve_eot_after_planning =
                planner_prefers_opponent_end_step_top_tutor(&planning_domain, &hand, &mana_pool);
            if planned_cast_queue.is_empty()
                && !(planning_domain.rule_of_law_cap_active() && spells_cast_this_turn >= 1)
                && let Some(card_index) = select_typed_overrun_cast_from_hand(
                    deck,
                    &hand,
                    &line_zones,
                    turn,
                    cast_phase_is_precombat,
                    mana_access,
                    &mana_pool,
                    &order,
                    position,
                )
            {
                // This fallback exists only for a strict typed current-turn
                // conversion witness. It may outrank future access; ordinary
                // paid fallback development below may not.
                let certified_current_turn_conversion = typed_overrun_cast_line_ready(
                    deck,
                    &line_zones,
                    card_index,
                    turn,
                    cast_phase_is_precombat,
                );
                if !preserve_eot_after_planning || certified_current_turn_conversion {
                    planned_cast_queue.push_back(TurnAction::Cast(card_index));
                }
            }
            let Some(action) = planned_cast_queue.pop_front() else {
                let library_selection_preserves_eot = preserve_eot_after_planning
                    && active_library_selection_ability(deck, &line_zones).is_some_and(
                        |(oracle_cost, _)| {
                            let mut candidate_pool = mana_pool.clone();
                            let mut candidate_life = player_life;
                            candidate_pool.pay(Some(&parse_mana_cost(Some(&oracle_cost))), 0, 0)
                                && candidate_pool.settle_pending_source_damage(&mut candidate_life)
                                && {
                                    let candidate_domain = CastPlanningDomain {
                                        deck,
                                        mana_access,
                                        zones: &line_zones,
                                        turn,
                                        policy: options.pilot_policy,
                                        additional_generic_per_cast: planning_domain
                                            .additional_generic_per_cast,
                                        player_life: candidate_life,
                                        spells_cast_this_turn,
                                        opponent_end_step: planning_domain
                                            .opponent_end_step
                                            .clone(),
                                    };
                                    planner_prefers_opponent_end_step_top_tutor(
                                        &candidate_domain,
                                        &hand,
                                        &candidate_pool,
                                    )
                                }
                        },
                    );
                if eot_reservation_allows_candidate(
                    preserve_eot_after_planning,
                    library_selection_preserves_eot,
                ) && let Some(resolution) = execute_library_selection_ability(
                    deck,
                    &mut order,
                    position,
                    &hand,
                    turn,
                    &mut mana_pool,
                    &mut rng,
                    &mut line_zones,
                ) {
                    if !mana_pool.settle_pending_source_damage(&mut player_life) {
                        return isolated.finish(EpisodeOutcome {
                            threat_turn: first_credible_threat_turn,
                            first_win_attempt_turn,
                            resolved_table_win_turn: None,
                            timing_provenance,
                            first_attempt_opportunity,
                            first_attempt_stopped,
                            recovered: recovered_attempt,
                            final_life: player_life,
                            player_died: true,
                        });
                    }
                    if let Some(target_index) = resolution.selected_card {
                        let target = &deck.cards[target_index];
                        apply_roles(
                            target.roles,
                            &mut engine_count,
                            &mut enabler_count,
                            &mut payoff_count,
                            &mut creature_count,
                            &mut protection_count,
                        );
                        if target.effects.recursion {
                            recursion_count = recursion_count.saturating_add(1);
                        }
                        install_tutored_permanent_runtime(
                            deck,
                            target_index,
                            mana_access,
                            turn,
                            &mut mana_sources,
                            &mut mana_pool,
                            &mut rng,
                            Some(&mut hand),
                            &mut line_zones,
                        );
                        mana_pool.refresh_battlefield_sources(
                            deck,
                            &line_zones,
                            active_ability_context(deck, &line_zones),
                        );
                    }
                    committed_actions = committed_actions.saturating_add(1);
                    continue;
                }
                execute_necropotence_delayed_access_batch(
                    deck,
                    mana_access,
                    &hand,
                    &mut order,
                    position,
                    &line_zones,
                    &mana_sources,
                    turn,
                    mana_pool.remaining_treasures(),
                    &mut player_life,
                    &mut pending_delayed_card_access,
                );
                break;
            };
            if let TurnAction::ActivateGrantedLandMana {
                source_card_index,
                source_sequence,
            } = action
            {
                let Some(removed_index) = execute_granted_land_mana_action(
                    deck,
                    source_card_index,
                    source_sequence,
                    turn,
                    &mut line_zones,
                    &mut mana_pool,
                ) else {
                    planned_cast_queue.clear();
                    break;
                };
                if let Some(removed) = deck.cards.get(removed_index) {
                    remove_roles(
                        removed.roles,
                        &mut engine_count,
                        &mut enabler_count,
                        &mut payoff_count,
                        &mut creature_count,
                        &mut protection_count,
                    );
                    if removed.effects.recursion {
                        recursion_count = recursion_count.saturating_sub(1);
                    }
                    if removed.is_commander {
                        commanders_cast.remove(&removed_index);
                    }
                }
                synchronize_mana_sources_with_battlefield(&mut mana_sources, &line_zones);
                synchronize_turn_pool_with_battlefield(&mut mana_pool, &line_zones);
                mana_pool.refresh_battlefield_sources(
                    deck,
                    &line_zones,
                    active_ability_context(deck, &line_zones),
                );
                planned_cast_queue.clear();
                committed_actions = committed_actions.saturating_add(1);
                continue;
            }
            if let TurnAction::ActivateDiscardHandSacrificeMana {
                source_card_index,
                source_sequence,
                color,
            } = action
            {
                let Some(removed_index) = execute_discard_hand_sacrifice_mana_action(
                    deck,
                    source_card_index,
                    source_sequence,
                    color,
                    &mut hand,
                    &mut line_zones,
                    &mut mana_pool,
                ) else {
                    planned_cast_queue.clear();
                    break;
                };
                if let Some(removed) = deck.cards.get(removed_index) {
                    remove_roles(
                        removed.roles,
                        &mut engine_count,
                        &mut enabler_count,
                        &mut payoff_count,
                        &mut creature_count,
                        &mut protection_count,
                    );
                    if removed.effects.recursion {
                        recursion_count = recursion_count.saturating_sub(1);
                    }
                    if removed.is_commander {
                        commanders_cast.remove(&removed_index);
                    }
                }
                synchronize_mana_sources_with_battlefield(&mut mana_sources, &line_zones);
                synchronize_turn_pool_with_battlefield(&mut mana_pool, &line_zones);
                mana_pool.refresh_battlefield_sources(
                    deck,
                    &line_zones,
                    active_ability_context(deck, &line_zones),
                );
                planned_cast_queue.clear();
                committed_actions = committed_actions.saturating_add(1);
                continue;
            }
            match execute_queued_first_use_self_transfer_tutor_action(
                action,
                deck,
                mana_access,
                &mut order,
                position,
                &mut hand,
                turn,
                &mut mana_pool,
                &mut rng,
                &mut line_zones,
                prospective_generic_tax,
            ) {
                QueuedFirstUseSelfTransferTutorExecution::Committed(resolution) => {
                    if !mana_pool.settle_pending_source_damage(&mut player_life) {
                        return isolated.finish(EpisodeOutcome {
                            threat_turn: first_credible_threat_turn,
                            first_win_attempt_turn,
                            resolved_table_win_turn: None,
                            timing_provenance,
                            first_attempt_opportunity,
                            first_attempt_stopped,
                            recovered: recovered_attempt,
                            final_life: player_life,
                            player_died: true,
                        });
                    }
                    finalize_first_use_self_transfer_tutor_runtime(
                        deck,
                        resolution,
                        &mut mana_sources,
                        &mut mana_pool,
                        &line_zones,
                        &mut engine_count,
                        &mut enabler_count,
                        &mut payoff_count,
                        &mut creature_count,
                        &mut protection_count,
                        &mut recursion_count,
                    );
                    // Search, source transfer, and pool synchronization are
                    // newly observable. Any speculative suffix was planned
                    // against state that no longer exists.
                    planned_cast_queue.clear();
                    committed_actions = committed_actions.saturating_add(1);
                    continue;
                }
                QueuedFirstUseSelfTransferTutorExecution::Rejected => {
                    // The action was bound to a stale object or can no longer
                    // pay. The exact executor is transactional; stop this main
                    // phase instead of falling through to a hand-card lookup or
                    // repeatedly replanning the same rejected activation.
                    planned_cast_queue.clear();
                    break;
                }
                QueuedFirstUseSelfTransferTutorExecution::NotActivation => {}
            }
            let card_index = action.card_index();
            let Some(action_index) = hand.iter().position(|candidate| *candidate == card_index)
            else {
                break;
            };
            let card_index = hand[action_index];
            let card = &deck.cards[card_index];
            if matches!(action, TurnAction::ActivateHandMana(_)) {
                if execute_atomic_hand_mana_action(
                    deck,
                    card_index,
                    &mut hand,
                    &mut line_zones,
                    &mut mana_pool,
                ) {
                    committed_actions = committed_actions.saturating_add(1);
                    // The mana and hand/exile delta are newly observable.
                    // Replan rather than trusting any suffix chosen before it.
                    planned_cast_queue.clear();
                    continue;
                }
                break;
            }
            let typed_burst_card_access = compile_typed_burst_card_access_program(card);
            let atomic_transaction = compile_typed_atomic_transaction(card);
            let conditional_mana_source = compile_typed_conditional_mana_source(card);
            let first_use_self_transfer_tutor = compile_typed_first_use_self_transfer_tutor(card);
            let necropotence_lifecycle = compile_typed_necropotence_lifecycle(card);
            if card.ability_program.atomic_transaction.is_some() && atomic_transaction.is_none() {
                break;
            }
            if card.ability_program.entry_linked_permanent.is_some()
                && conditional_mana_source.is_none()
            {
                break;
            }
            if card.ability_program.self_transfer_tutor_permanent.is_some()
                && first_use_self_transfer_tutor.is_none()
            {
                break;
            }
            if card.ability_program.necropotence_lifecycle.is_some()
                && necropotence_lifecycle.is_none()
            {
                break;
            }
            let mut changes_observable_plan = card_changes_observable_plan(card)
                || atomic_transaction.is_some()
                || conditional_mana_source.is_some()
                || first_use_self_transfer_tutor.is_some()
                || necropotence_lifecycle.is_some();
            if card.has(role::LAND) {
                break;
            }
            if should_hold_reactive_card(card) {
                break;
            }
            if should_hold_reviewed_sequence_piece(
                deck,
                card_index,
                &hand,
                &line_zones,
                turn,
                &mana_pool,
                mana_access,
                prospective_generic_tax,
            ) {
                break;
            }
            if matches!(
                action,
                TurnAction::CastReviewedRandomDiscardWithManaResponse { .. }
            ) && !reviewed_primer_window_action_is_runtime_legal(
                action,
                deck,
                mana_access,
                &order,
                position,
                &hand,
                &line_zones,
                &mana_pool,
                player_life,
                prospective_generic_tax,
                planning_rule_of_law_cap_active,
            ) {
                planned_cast_queue.clear();
                break;
            }
            if rule_of_law_blocks_next_spell(&mut isolated, turn, spells_cast_this_turn) {
                // The blocked card remains in hand. Replan under the now-live
                // cap so legal nonspell actions (hand mana, Rain activations,
                // and battlefield tutor abilities) are not stranded.
                planned_cast_queue.clear();
                continue;
            }
            isolated.observe_opportunity(InteractionScenario::GenericTaxStax);
            let generic_tax = if isolated.is(InteractionScenario::GenericTaxStax) {
                isolated.activate(turn, 1);
                1
            } else {
                0
            };
            let cost = card.mana_value.ceil().max(0.0) as u8;
            let typed_overrun_cast =
                typed_overrun_cast_line_ready(
                    deck,
                    &line_zones,
                    card_index,
                    turn,
                    cast_phase_is_precombat,
                ) && best_overrun_creature_target_position(deck, &order, position).is_some();
            changes_observable_plan |= typed_overrun_cast;
            let mut atomic_commit = None;
            let paid = if let Some(transaction) = atomic_transaction.as_ref() {
                atomic_commit = commit_atomic_spell_initiation_with_choice(
                    transaction,
                    deck,
                    card_index,
                    &mut hand,
                    &mut line_zones,
                    &mut mana_pool,
                    mana_access,
                    generic_tax,
                    atomic_transaction_choice_for_action(action),
                );
                atomic_commit.is_some()
            } else if typed_overrun_cast {
                // The typed loop pays X and any generic response-pressure tax;
                // the finite pool must still pay the two printed green pips.
                pay_exact_variable_green_overrun_cost(
                    &mut mana_pool,
                    mana_access.and_then(|access| access.cost(card_index)),
                )
            } else {
                action.spell_payment_choice().is_some_and(|payment_choice| {
                    pay_spell_cost_choice(
                        deck,
                        &mut line_zones,
                        &mut mana_pool,
                        card_index,
                        mana_access.and_then(|access| access.cost(card_index)),
                        cost,
                        generic_tax,
                        0,
                        turn,
                        payment_choice,
                        Some(&combat_state),
                    )
                })
            };
            if !paid {
                break;
            }
            if !mana_pool.settle_pending_source_damage(&mut player_life) {
                return isolated.finish(EpisodeOutcome {
                    threat_turn: first_credible_threat_turn,
                    first_win_attempt_turn,
                    resolved_table_win_turn: None,
                    timing_provenance,
                    first_attempt_opportunity,
                    first_attempt_stopped,
                    recovered: recovered_attempt,
                    final_life: player_life,
                    player_died: true,
                });
            }
            if atomic_commit.is_none() {
                hand.swap_remove(action_index);
            }
            committed_actions = committed_actions.saturating_add(1);
            spells_cast_this_turn = spells_cast_this_turn.saturating_add(1);
            if !apply_controller_spell_cast_triggers(
                deck,
                &mut line_zones,
                card_index,
                turn,
                spells_cast_this_turn,
                &mut mana_pool,
                &mut player_life,
            ) {
                return isolated.finish(EpisodeOutcome {
                    threat_turn: first_credible_threat_turn,
                    first_win_attempt_turn,
                    resolved_table_win_turn: None,
                    timing_provenance,
                    first_attempt_opportunity,
                    first_attempt_stopped,
                    recovered: recovered_attempt,
                    final_life: player_life,
                    player_died: true,
                });
            }
            if let Some(commit) = atomic_commit
                && let Some(sacrificed_index) = commit.sacrificed_card
            {
                if let Some(sacrificed) = deck.cards.get(sacrificed_index) {
                    remove_roles(
                        sacrificed.roles,
                        &mut engine_count,
                        &mut enabler_count,
                        &mut payoff_count,
                        &mut creature_count,
                        &mut protection_count,
                    );
                    if sacrificed.effects.recursion {
                        recursion_count = recursion_count.saturating_sub(1);
                    }
                    if sacrificed.is_commander {
                        commanders_cast.remove(&sacrificed_index);
                    }
                }
                synchronize_mana_sources_with_battlefield(&mut mana_sources, &line_zones);
                synchronize_turn_pool_with_battlefield(&mut mana_pool, &line_zones);
            }
            if matches!(
                action,
                TurnAction::CastReviewedRandomDiscardWithManaResponse { .. }
            ) {
                // This reviewed action explicitly holds priority after casting
                // the search spell. LED's mana ability resolves immediately,
                // so its hand discard and sacrifice commit before opponents
                // receive the counter checkpoint below. The library search,
                // forced discard, and shuffle remain after that checkpoint.
                let Some(transaction) = atomic_transaction.as_ref() else {
                    planned_cast_queue.clear();
                    break;
                };
                let Some(sacrificed_card) = execute_reviewed_random_discard_mana_response(
                    action,
                    transaction,
                    deck,
                    &mut hand,
                    &mut mana_pool,
                    &mut line_zones,
                ) else {
                    planned_cast_queue.clear();
                    break;
                };
                if let Some(removed) = deck.cards.get(sacrificed_card) {
                    remove_roles(
                        removed.roles,
                        &mut engine_count,
                        &mut enabler_count,
                        &mut payoff_count,
                        &mut creature_count,
                        &mut protection_count,
                    );
                    if removed.effects.recursion {
                        recursion_count = recursion_count.saturating_sub(1);
                    }
                    if removed.is_commander {
                        commanders_cast.remove(&sacrificed_card);
                    }
                }
                synchronize_mana_sources_with_battlefield(&mut mana_sources, &line_zones);
                synchronize_turn_pool_with_battlefield(&mut mana_pool, &line_zones);
            }
            let relevant_spell = scenario_relevant_spell(card);
            if relevant_spell {
                isolated.observe_opportunity(InteractionScenario::FirstRelevantSpellCountered);
            }
            if isolated.is(InteractionScenario::FirstRelevantSpellCountered)
                && !isolated.applied()
                && relevant_spell
            {
                isolated.activate(turn, 1);
                if atomic_commit.is_some() {
                    // A countered atomic spell never resolves, so it can move
                    // directly from the stack to the graveyard here.
                    line_zones.record_cast(deck, card_index, turn);
                } else {
                    // The card has already left hand, but a countered spell
                    // never enters the battlefield and never performs its
                    // entry procedure. Preserve the physical object in the
                    // graveyard for threshold, recursion, and later lines.
                    line_zones.graveyard.push(card_index);
                    line_zones.advance_sequence();
                }
                planned_cast_queue.clear();
                continue;
            }

            let targeted_recovery = isolated.is(InteractionScenario::TargetedPermanentRemoval)
                && isolated.applied()
                && scenario_targetable_permanent(card);
            let counter_recovery = isolated.is(InteractionScenario::FirstRelevantSpellCountered)
                && isolated.applied()
                && scenario_relevant_spell(card);
            let rule_recovery = isolated.is(InteractionScenario::RuleOfLawCap)
                && isolated
                    .applied_turn
                    .is_some_and(|applied_turn| turn > applied_turn);
            let graveyard_action = scenario_executable_graveyard_action(card);
            if graveyard_action {
                isolated.observe_opportunity(InteractionScenario::GraveyardShutdown);
            }
            let graveyard_suppressed =
                isolated.is(InteractionScenario::GraveyardShutdown) && graveyard_action;
            if graveyard_suppressed && !isolated.applied() {
                isolated.activate(turn, 1);
            }
            let mana_sources_before = mana_sources.len();
            let mut entry_resolution = TypedPermanentEntryResolution {
                entered_battlefield: true,
                ..TypedPermanentEntryResolution::default()
            };
            if typed_overrun_cast {
                line_zones.record_typed_overrun_cast(deck, card_index, turn);
            } else if let Some(kind) = conditional_mana_source.filter(|kind| kind.is_entry_linked())
            {
                entry_resolution = resolve_typed_permanent_entry(
                    kind,
                    deck,
                    card_index,
                    &mut hand,
                    mana_access,
                    &mut line_zones,
                    turn,
                    &mana_pool,
                    0,
                );
            } else if atomic_commit.is_none() {
                line_zones.record_cast(deck, card_index, turn);
            }
            let mut atomic_resolution = AtomicSpellResolution::default();
            let consumed_mana_source = if let (Some(transaction), Some(commit)) =
                (atomic_transaction.as_ref(), atomic_commit)
            {
                if matches!(
                    action,
                    TurnAction::CastReviewedRandomDiscardWithManaResponse { .. }
                ) {
                    let Some(resolution) =
                        execute_reviewed_random_discard_resolution_after_mana_response(
                            action,
                            transaction,
                            deck,
                            &mut order,
                            position,
                            &mut hand,
                            &mut rng,
                            &mut line_zones,
                        )
                    else {
                        planned_cast_queue.clear();
                        break;
                    };
                    atomic_resolution = resolution;
                } else {
                    atomic_resolution = execute_atomic_spell_resolution_with_choice(
                        transaction,
                        commit.pre_resolution_graveyard_count,
                        commit.pre_resolution_source_name_matches,
                        commit.choice,
                        deck,
                        mana_access,
                        &mut order,
                        position,
                        &mut hand,
                        turn,
                        &mut mana_pool,
                        &mut rng,
                        &mut line_zones,
                        generic_tax,
                    );
                }
                false
            } else if let Some(kind) = conditional_mana_source.filter(|kind| kind.is_entry_linked())
            {
                if entry_resolution.entered_battlefield
                    && let Some(source) = typed_battlefield_mana_source(
                        deck,
                        card_index,
                        kind,
                        entry_resolution.linked_colors,
                        turn,
                    )
                {
                    mana_sources.push(source);
                    add_typed_source_to_current_pool(
                        source,
                        deck,
                        &line_zones,
                        turn,
                        &mut mana_pool,
                    );
                }
                !entry_resolution.entered_battlefield
            } else {
                let ability_context = active_ability_context(deck, &line_zones);
                apply_cast_mana_effects(
                    deck,
                    card_index,
                    card,
                    mana_access,
                    turn,
                    &mut mana_sources,
                    &mut mana_pool,
                    &mut rng,
                    ability_context,
                    &line_zones,
                )
            };
            let mut nested_target_to_resolve = None;
            if let Some(target_index) = atomic_resolution.free_cast_offer {
                let target_supported = deck
                    .cards
                    .get(target_index)
                    .is_some_and(nested_free_cast_is_supported);
                let rule_blocked = target_supported
                    && rule_of_law_blocks_next_spell(&mut isolated, turn, spells_cast_this_turn);
                let mut cast_succeeded = false;
                if target_supported && !rule_blocked {
                    let mut candidate_pool = mana_pool.clone();
                    if candidate_pool.pay_with_generic_adjustment(
                        None,
                        0,
                        generic_tax,
                        generic_spell_cost_reduction(deck, &line_zones, target_index),
                        0,
                    ) {
                        mana_pool = candidate_pool;
                        if generic_tax > 0 {
                            isolated.add_affected_event();
                        }
                        if !mana_pool.settle_pending_source_damage(&mut player_life) {
                            return isolated.finish(EpisodeOutcome {
                                threat_turn: first_credible_threat_turn,
                                first_win_attempt_turn,
                                resolved_table_win_turn: None,
                                timing_provenance,
                                first_attempt_opportunity,
                                first_attempt_stopped,
                                recovered: recovered_attempt,
                                final_life: player_life,
                                player_died: true,
                            });
                        }
                        spells_cast_this_turn = spells_cast_this_turn.saturating_add(1);
                        if !apply_controller_spell_cast_triggers(
                            deck,
                            &mut line_zones,
                            target_index,
                            turn,
                            spells_cast_this_turn,
                            &mut mana_pool,
                            &mut player_life,
                        ) {
                            return isolated.finish(EpisodeOutcome {
                                threat_turn: first_credible_threat_turn,
                                first_win_attempt_turn,
                                resolved_table_win_turn: None,
                                timing_provenance,
                                first_attempt_opportunity,
                                first_attempt_stopped,
                                recovered: recovered_attempt,
                                final_life: player_life,
                                player_died: true,
                            });
                        }
                        nested_target_to_resolve = Some(target_index);
                        cast_succeeded = true;
                    }
                }
                if !cast_succeeded {
                    hand.push(target_index);
                }
            }
            if atomic_commit.is_some() {
                // Atomic source spells remain on the stack throughout their
                // ordered resolution. In particular, Beseech's offered card
                // is cast before Beseech itself reaches the graveyard.
                line_zones.record_cast(deck, card_index, turn);
            }
            if let Some(target_index) = nested_target_to_resolve {
                // Casting the offered spell happens during Beseech's
                // resolution, but that spell cannot resolve until Beseech has
                // finished and left the stack. Preserve that exact ordering
                // for graveyard-sensitive effects and ordered line evidence.
                let target = &deck.cards[target_index];
                line_zones.record_cast(deck, target_index, turn);
                let ability_context = active_ability_context(deck, &line_zones);
                let nested_consumed_source = apply_cast_mana_effects(
                    deck,
                    target_index,
                    target,
                    mana_access,
                    turn,
                    &mut mana_sources,
                    &mut mana_pool,
                    &mut rng,
                    ability_context,
                    &line_zones,
                );
                if nested_consumed_source {
                    line_zones.remove_named_permanent(deck, &target.normalized_name, true);
                } else if card_has_persistent_body(target) {
                    apply_roles(
                        target.roles,
                        &mut engine_count,
                        &mut enabler_count,
                        &mut payoff_count,
                        &mut creature_count,
                        &mut protection_count,
                    );
                }
                if target.effects.recursion {
                    recursion_count = recursion_count.saturating_add(1);
                }
                creature_count = creature_count.saturating_add(immediate_creature_tokens(target));
                extra_turn_credit = extra_turn_credit.saturating_add(immediate_extra_turns(target));
            }
            mana_pool.refresh_battlefield_sources(
                deck,
                &line_zones,
                active_ability_context(deck, &line_zones),
            );
            if consumed_mana_source {
                changes_observable_plan = true;
                line_zones.remove_named_permanent(deck, &card.normalized_name, true);
            }
            if typed_overrun_cast {
                resolve_typed_overrun_creature_tutor(
                    deck,
                    &mut order,
                    position,
                    turn,
                    mana_access,
                    &mut mana_sources,
                    &mut mana_pool,
                    &mut rng,
                    &mut line_zones,
                    &mut engine_count,
                    &mut enabler_count,
                    &mut payoff_count,
                    &mut creature_count,
                    &mut protection_count,
                );
            } else if atomic_transaction.is_none() && conditional_mana_source.is_none() {
                execute_tutor_on_resolution(
                    deck,
                    card,
                    mana_access,
                    &mut order,
                    position,
                    &mut hand,
                    turn,
                    &mut mana_sources,
                    &mut mana_pool,
                    &mut rng,
                    &mut line_zones,
                    &mut engine_count,
                    &mut enabler_count,
                    &mut payoff_count,
                    &mut creature_count,
                    &mut protection_count,
                    0,
                );
                player_life -= f32::from(card.effects.tutor.life_loss_after_resolution);
                if player_life <= 0.0 {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
            }
            if let Some(program) = typed_burst_card_access {
                let resolution = execute_typed_burst_card_access(
                    program,
                    deck,
                    &mut hand,
                    &mut order,
                    position,
                    &mut line_zones,
                    &mut player_life,
                );
                if resolution.player_died {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
            } else {
                resolve_immediate_spell_draws(card, &mut hand, &order, &mut position);
            }
            if card.effects.recursion && !graveyard_suppressed {
                recursion_count = recursion_count.saturating_add(1);
            }
            creature_count = creature_count.saturating_add(immediate_creature_tokens(card));
            extra_turn_credit = extra_turn_credit.saturating_add(immediate_extra_turns(card));
            if entry_resolution.entered_battlefield
                && !consumed_mana_source
                && card_has_persistent_body(card)
            {
                apply_roles(
                    card.roles,
                    &mut engine_count,
                    &mut enabler_count,
                    &mut payoff_count,
                    &mut creature_count,
                    &mut protection_count,
                );
            }

            if targeted_recovery || counter_recovery || rule_recovery {
                isolated.recover(turn);
            }
            if isolated.is(InteractionScenario::GenericTaxStax) {
                isolated.recover(turn);
            }
            let targeted_permanent_checkpoint = !targeted_recovery
                && entry_resolution.entered_battlefield
                && scenario_targetable_permanent(card);
            if targeted_permanent_checkpoint {
                isolated.observe_opportunity(InteractionScenario::TargetedPermanentRemoval);
            }
            if isolated.is(InteractionScenario::TargetedPermanentRemoval)
                && targeted_permanent_checkpoint
                && isolated.activate(turn, 1)
            {
                if !consumed_mana_source {
                    remove_roles(
                        card.roles,
                        &mut engine_count,
                        &mut enabler_count,
                        &mut payoff_count,
                        &mut creature_count,
                        &mut protection_count,
                    );
                }
                if card.effects.recursion {
                    recursion_count = recursion_count.saturating_sub(1);
                }
                mana_sources.truncate(mana_sources_before);
                line_zones.remove_named_permanent(deck, &card.normalized_name, false);
                planned_cast_queue.clear();
            }
            if changes_observable_plan {
                planned_cast_queue.clear();
            }
            apply_isolated_creature_wipe_if_ready(
                &mut isolated,
                deck,
                turn,
                &mut line_zones,
                &mut commanders_cast,
                &mut engine_count,
                &mut enabler_count,
                &mut payoff_count,
                &mut creature_count,
                &mut protection_count,
                &mut recursion_count,
            );
            synchronize_mana_sources_with_battlefield(&mut mana_sources, &line_zones);
            synchronize_turn_pool_with_battlefield(&mut mana_pool, &line_zones);
        }

        if deferred_land_play
            && let Some(hand_position) = best_land_position(
                deck,
                mana_access,
                &hand,
                &order,
                position,
                turn,
                &mana_pool,
                &line_zones,
            )
        {
            // A single weak land was held back so the bounded planner could
            // choose Mox Diamond's real discard line. If it declined that
            // line, preserve the normal land drop for future turns.
            let land_index = hand.swap_remove(hand_position);
            let land_resolution = execute_land_play(
                deck,
                mana_access,
                land_index,
                turn,
                &hand,
                &mut order,
                position,
                &mut rng,
                &mut line_zones,
                &mut mana_sources,
                &mut mana_pool,
                &mut player_life,
            );
            if land_resolution.player_died {
                return isolated.finish(EpisodeOutcome {
                    threat_turn: first_credible_threat_turn,
                    first_win_attempt_turn,
                    resolved_table_win_turn: None,
                    timing_provenance,
                    first_attempt_opportunity,
                    first_attempt_stopped,
                    recovered: recovered_attempt,
                    final_life: player_life,
                    player_died: true,
                });
            }
        }

        if engine_count > 0 && opponent_rolls.disrupts_engine(profile) {
            let hand_protection = payable_hand_protection(deck, &hand, mana_access, &mana_pool);
            if (protection_count > 0 || hand_protection.is_some())
                && opponent_rolls.protection_prevents_engine_disruption()
            {
                if protection_count > 0 {
                    protection_count = protection_count.saturating_sub(1);
                } else {
                    consume_paid_hand_protection(
                        &mut hand,
                        &mut mana_pool,
                        &mut treasure_reserve,
                        hand_protection,
                    );
                }
            } else {
                engine_count = engine_count.saturating_sub(1);
                line_zones.remove_one_permanent_with_role(deck, role::ENGINE);
                synchronize_mana_sources_with_battlefield(&mut mana_sources, &line_zones);
                synchronize_turn_pool_with_battlefield(&mut mana_pool, &line_zones);
                if recursion_count > 0 {
                    recursion_count = recursion_count.saturating_sub(1);
                    pending_engine_recovery = pending_engine_recovery.saturating_add(1);
                }
                if deck.synergy.commander_dependence > 0.62
                    && !commanders_cast.is_empty()
                    && opponent_rolls.hits_commander()
                {
                    let removed_commander = commanders_cast
                        .iter()
                        .copied()
                        .filter(|index| {
                            deck.cards
                                .get(*index)
                                .is_some_and(|card| card.has(role::ENGINE))
                        })
                        .min()
                        .or_else(|| commanders_cast.iter().copied().min());
                    if let Some(commander_index) = removed_commander {
                        commanders_cast.remove(&commander_index);
                        if let Some(commander) = deck.cards.get(commander_index) {
                            line_zones.remove_named_permanent(
                                deck,
                                &commander.normalized_name,
                                false,
                            );
                            synchronize_mana_sources_with_battlefield(
                                &mut mana_sources,
                                &line_zones,
                            );
                            synchronize_turn_pool_with_battlefield(&mut mana_pool, &line_zones);
                        }
                    }
                }
            }
        }
        if !mana_pool.settle_pending_source_damage(&mut player_life) {
            return isolated.finish(EpisodeOutcome {
                threat_turn: first_credible_threat_turn,
                first_win_attempt_turn,
                resolved_table_win_turn: None,
                timing_provenance,
                first_attempt_opportunity,
                first_attempt_stopped,
                recovered: recovered_attempt,
                final_life: player_life,
                player_died: true,
            });
        }
        if turn >= 5 && creature_count >= 3 && opponent_rolls.wipes_board(profile) {
            destroy_all_creatures_and_remove_persistent_contributions(
                deck,
                &mut line_zones,
                &mut commanders_cast,
                &mut engine_count,
                &mut enabler_count,
                &mut payoff_count,
                &mut creature_count,
                &mut protection_count,
                &mut recursion_count,
            );
            synchronize_mana_sources_with_battlefield(&mut mana_sources, &line_zones);
            synchronize_turn_pool_with_battlefield(&mut mana_pool, &line_zones);
        }

        let entry_draws = line_zones.take_pending_card_draws();
        draw_bounded_cards(&mut hand, &order, &mut position, entry_draws);

        if cast_phase_is_precombat {
            // Commit combat-relevant attachment choices before spending
            // otherwise-unclaimed mana on threshold token activations. The
            // newly created tokens are summoning sick this turn, so activating
            // first could both consume a legal equip payment and tempt the
            // attachment chooser toward a body that cannot attack.
            let equipment_resolution =
                activate_equipment_for_combat(deck, &mut line_zones, &mut mana_pool, turn);
            remove_persistent_contributions(
                deck,
                &equipment_resolution.removed_card_indices,
                &mut commanders_cast,
                &mut engine_count,
                &mut enabler_count,
                &mut payoff_count,
                &mut creature_count,
                &mut protection_count,
                &mut recursion_count,
            );
            creature_count = creature_count.saturating_sub(
                u8::try_from(equipment_resolution.removed_creature_tokens).unwrap_or(u8::MAX),
            );
            let activated_tokens = activate_counter_threshold_token_abilities(
                deck,
                &mut line_zones,
                &mut mana_pool,
                turn,
            );
            creature_count =
                creature_count.saturating_add(u8::try_from(activated_tokens).unwrap_or(u8::MAX));
            if !mana_pool.settle_pending_source_damage(&mut player_life) {
                return isolated.finish(EpisodeOutcome {
                    threat_turn: first_credible_threat_turn,
                    first_win_attempt_turn,
                    resolved_table_win_turn: None,
                    timing_provenance,
                    first_attempt_opportunity,
                    first_attempt_stopped,
                    recovered: recovered_attempt,
                    final_life: player_life,
                    player_died: true,
                });
            }
            let equipment_draws = line_zones.take_pending_card_draws();
            draw_bounded_cards(&mut hand, &order, &mut position, equipment_draws);
        }

        // Combat is resolved from exact attack-capable objects and tracked
        // opponent life/commander damage. Nonlethal attacks advance public
        // table state immediately. A table-lethal presentation is held at the
        // response checkpoint below, where it may be stopped before any
        // connected damage is supplied to the terminal evaluator.
        let combat_attack = plan_combat_attack(deck, &line_zones, turn, &combat_state, &mana_pool);
        if let Some(attack) = combat_attack.as_ref() {
            record_nonvigilance_attack_taps(deck, &mut line_zones, &mut mana_pool, attack);
            let attack_draws = combat_attack_draw_count(deck, &line_zones, attack);
            draw_bounded_cards(&mut hand, &order, &mut position, attack_draws);
        }
        let combat_table_lethal_ready = combat_attack
            .as_ref()
            .is_some_and(PresentedAttack::presents_table_lethal);
        if let Some(attack) = combat_attack.as_ref()
            && !combat_table_lethal_ready
        {
            let resolved_table_win = resolve_all_connected_combat_damage(&mut combat_state, attack);
            debug_assert!(
                !resolved_table_win,
                "a nonlethal presented attack cannot resolve a table win"
            );
            let damage_draws = combat_damage_draw_count(deck, &line_zones, attack);
            draw_bounded_cards(&mut hand, &order, &mut position, damage_draws);
            player_life += combat_lifelink_gain(deck, &line_zones, attack);
        }

        // Select at most one line for the turn. Its exact modeled activation
        // cost is committed to the remaining post-cast pool once, so the same
        // mana cannot make both the generic and table-lethal checks pass.
        let pre_line_selection_pool = mana_pool.clone();
        let pending_strict_win_candidate =
            stage_reviewed_empty_library_win_candidate(deck, &order, position, &line_zones, turn);
        let ready_line_index = pending_strict_win_candidate
            .map(|candidate| candidate.line_index)
            .or(executed_graveyard_storm_line)
            .or_else(|| {
                if combat_table_lethal_ready {
                    return None;
                }
                select_ready_known_line(
                    deck,
                    &line_zones,
                    creature_count,
                    &mana_sources,
                    turn,
                    &runtime_model.line_activation_costs,
                    &mut mana_pool,
                )
            });
        if !mana_pool.settle_pending_source_damage(&mut player_life) {
            return isolated.finish(EpisodeOutcome {
                threat_turn: first_credible_threat_turn,
                first_win_attempt_turn,
                resolved_table_win_turn: None,
                timing_provenance,
                first_attempt_opportunity,
                first_attempt_stopped,
                recovered: recovered_attempt,
                final_life: player_life,
                player_died: true,
            });
        }
        treasure_reserve = mana_pool.remaining_treasures();
        let combo_ready = ready_line_index.is_some();
        let table_lethal_combo_ready = ready_line_index.is_some_and(|index| {
            deck.known_lines
                .get(index)
                .is_some_and(|line| line.table_lethal_if_resolved)
        });
        let minimum_engine_turn = match options.pilot_policy {
            PilotPolicy::Race => 3,
            PilotPolicy::Balanced => 4,
            PilotPolicy::Protect => 5,
        };
        let minimum_combat_turn = match options.pilot_policy {
            PilotPolicy::Race => 4,
            PilotPolicy::Balanced => 5,
            PilotPolicy::Protect => 6,
        };
        let development_turn = turn.saturating_add(extra_turn_credit);
        let engine_threat = engine_count > 0
            && payoff_count > 0
            && (enabler_count > 0 || deck.synergy.cohesion_score >= 72)
            && development_turn >= minimum_engine_turn;
        let combat_threat =
            creature_count >= 5 && payoff_count > 0 && development_turn >= minimum_combat_turn;
        let credible_threat =
            combo_ready || engine_threat || combat_threat || combat_table_lethal_ready;

        if credible_threat && first_credible_threat_turn.is_none() {
            first_credible_threat_turn = Some(turn);
        }
        if credible_threat
            && isolated.is(InteractionScenario::GraveyardShutdown)
            && isolated.applied()
        {
            isolated.recover(turn);
        }

        // A documented table-lethal line can present an explicit attempt
        // immediately, but structural readiness is not proof that the
        // conversion resolved. Broad engine/combat density remains useful as
        // a separately serialized development milestone; it must never write
        // into the explicit attempt endpoint.
        let developed_since_threat =
            first_credible_threat_turn.is_some_and(|threat_turn| turn > threat_turn);
        let engine_attempt = engine_threat
            && developed_since_threat
            && engine_count
                .saturating_add(enabler_count)
                .saturating_add(payoff_count)
                >= 5;
        let combat_attempt = combat_threat && developed_since_threat && creature_count >= 7;
        let generic_milestone_kind = match (engine_attempt, combat_attempt) {
            (true, true) => Some(GenericMilestoneKind::EngineAndCombat),
            (true, false) => Some(GenericMilestoneKind::Engine),
            (false, true) => Some(GenericMilestoneKind::Combat),
            (false, false) => None,
        };
        if timing_provenance.first_generic_milestone_turn.is_none()
            && let Some(kind) = generic_milestone_kind
        {
            timing_provenance.first_generic_milestone_turn = Some(turn);
            timing_provenance.first_generic_milestone_kind = Some(kind);
        }
        let strict_resolution_proof = pending_strict_win_candidate
            .and_then(|candidate| candidate.finalize(ready_line_index, turn));
        let structural_table_lethal_line = ready_line_index.filter(|_| table_lethal_combo_ready);
        let explicit_attempt_selection = select_explicit_attempt(
            combat_table_lethal_ready,
            strict_resolution_proof,
            structural_table_lethal_line,
        );
        let explicit_attempt_route_index =
            explicit_attempt_selection.map(ExplicitAttemptSelection::route_index);
        let explicit_win_attempt = explicit_attempt_selection.is_some();

        if !explicit_win_attempt
            && first_win_attempt_turn.is_none()
            && turn <= EARLY_ATTEMPT_DIAGNOSTIC_HORIZON
        {
            timing_provenance.early_turn_blockers[usize::from(turn - 1)] =
                Some(diagnose_explicit_attempt_blocker(
                    deck,
                    mana_access,
                    &hand,
                    &line_zones,
                    creature_count,
                    &mana_sources,
                    turn,
                    &runtime_model.line_activation_costs,
                    &pre_line_selection_pool,
                    u8::from(
                        isolated.is(InteractionScenario::GenericTaxStax) && isolated.applied(),
                    ),
                    may_attempt_after,
                ));
        }

        if explicit_win_attempt && first_credible_threat_turn.is_some() && turn >= may_attempt_after
        {
            let is_first_win_attempt = first_win_attempt_turn.is_none();
            if is_first_win_attempt {
                // The recognized route was presented even when the
                // response-pressure model stops it below. Record both its
                // timestamp and route provenance before interaction.
                first_win_attempt_turn = Some(turn);
                timing_provenance.first_explicit_route_index = explicit_attempt_route_index;
                first_attempt_opportunity = true;
                isolated.observe_opportunity(InteractionScenario::FirstWinAttemptStopped);

                let isolated_first_attempt_stop = isolated
                    .is(InteractionScenario::FirstWinAttemptStopped)
                    && !isolated.applied()
                    && isolated.activate(turn, 1);
                let hand_protection = payable_hand_protection(deck, &hand, mana_access, &mana_pool);
                let effective_protection =
                    protection_count.saturating_add(u8::from(hand_protection.is_some()));
                let opponent_would_interact = opponent_rolls.stops_attempt(profile, 0);
                let opponent_stops_attempt =
                    opponent_rolls.stops_attempt(profile, effective_protection);
                if opponent_would_interact {
                    consume_paid_hand_protection(
                        &mut hand,
                        &mut mana_pool,
                        &mut treasure_reserve,
                        hand_protection,
                    );
                }
                if !mana_pool.settle_pending_source_damage(&mut player_life) {
                    return isolated.finish(EpisodeOutcome {
                        threat_turn: first_credible_threat_turn,
                        first_win_attempt_turn,
                        resolved_table_win_turn: None,
                        timing_provenance,
                        first_attempt_opportunity,
                        first_attempt_stopped,
                        recovered: recovered_attempt,
                        final_life: player_life,
                        player_died: true,
                    });
                }
                if isolated_first_attempt_stop || opponent_stops_attempt {
                    first_attempt_stopped = true;
                    let has_recursion = recursion_count > 0;
                    if has_recursion {
                        recursion_count = recursion_count.saturating_sub(1);
                    }
                    may_attempt_after = turn.saturating_add(
                        if matches!(profile, InteractionProfile::HighPower) && !has_recursion {
                            2
                        } else {
                            1
                        },
                    );
                    if !has_recursion {
                        engine_count = engine_count.saturating_sub(1);
                        payoff_count = payoff_count.saturating_sub(1);
                    }
                    protection_count = protection_count.saturating_sub(1);
                    // The attempt was stopped, so the episode continues into
                    // the end step. Temporary graveyard-cast permission must
                    // expire before the next turn.
                    if line_zones.controller_is_monarch() {
                        draw_bounded_cards(&mut hand, &order, &mut position, 1);
                    }
                    deliver_due_delayed_card_access(
                        &mut hand,
                        &mut pending_delayed_card_access,
                        turn,
                    );
                    let removed = resolve_beginning_of_end_step_self_sacrifices(
                        deck,
                        &mut line_zones,
                        &mut mana_sources,
                    );
                    remove_persistent_contributions(
                        deck,
                        &removed,
                        &mut commanders_cast,
                        &mut engine_count,
                        &mut enabler_count,
                        &mut payoff_count,
                        &mut creature_count,
                        &mut protection_count,
                        &mut recursion_count,
                    );
                    discard_to_maximum_hand_size(
                        deck,
                        mana_access,
                        &mut hand,
                        order.get(position..).unwrap_or_default(),
                        &mut line_zones,
                        &mana_sources,
                        turn,
                        treasure_reserve,
                    );
                    if turn < options.maximum_turn {
                        let activity = table_activity_timeline
                            .turn(turn)
                            .expect("table activity timeline covers every simulated turn");
                        if !apply_table_turn_activity_with_end_steps(
                            deck,
                            &mut line_zones,
                            activity,
                            &combat_state,
                            &mut hand,
                            &order,
                            &mut position,
                            &mut treasure_reserve,
                            &mut player_life,
                        ) {
                            return isolated.finish(EpisodeOutcome {
                                threat_turn: first_credible_threat_turn,
                                first_win_attempt_turn,
                                resolved_table_win_turn: None,
                                timing_provenance,
                                first_attempt_opportunity,
                                first_attempt_stopped,
                                recovered: recovered_attempt,
                                final_life: player_life,
                                player_died: true,
                            });
                        }
                        if execute_opponent_end_step_top_tutor_before_next_turn(
                            deck,
                            mana_access,
                            &mut hand,
                            &mut order,
                            &mut position,
                            turn,
                            &mana_pool,
                            &mana_sources,
                            &mut rng,
                            &mut line_zones,
                            &mut treasure_reserve,
                            &mut creature_count,
                            &mut player_life,
                            &mut isolated,
                        ) {
                            return isolated.finish(EpisodeOutcome {
                                threat_turn: first_credible_threat_turn,
                                first_win_attempt_turn,
                                resolved_table_win_turn: None,
                                timing_provenance,
                                first_attempt_opportunity,
                                first_attempt_stopped,
                                recovered: recovered_attempt,
                                final_life: player_life,
                                player_died: true,
                            });
                        }
                    }
                    continue;
                }
            }

            if first_attempt_stopped && !is_first_win_attempt {
                recovered_attempt = true;
            }
            if first_attempt_stopped
                && !is_first_win_attempt
                && isolated.is(InteractionScenario::FirstWinAttemptStopped)
                && isolated.applied()
            {
                isolated.recover(turn);
            }
            if matches!(
                isolated.scenario,
                Some(
                    InteractionScenario::TargetedPermanentRemoval
                        | InteractionScenario::FirstRelevantSpellCountered
                        | InteractionScenario::GraveyardShutdown
                        | InteractionScenario::GenericTaxStax
                        | InteractionScenario::RuleOfLawCap
                )
            ) && isolated.applied()
            {
                isolated.recover(turn);
            }

            let typed_table_win_resolved = matches!(
                explicit_attempt_selection,
                Some(ExplicitAttemptSelection::Strict(_))
            );
            let combat_table_win_resolved = matches!(
                explicit_attempt_selection,
                Some(ExplicitAttemptSelection::CombatDamage)
            ) && combat_attack.as_ref().is_some_and(|attack| {
                resolve_all_connected_combat_damage(&mut combat_state, attack)
            });
            if typed_table_win_resolved || combat_table_win_resolved {
                return isolated.finish(EpisodeOutcome {
                    threat_turn: first_credible_threat_turn,
                    first_win_attempt_turn,
                    resolved_table_win_turn: Some(turn),
                    timing_provenance,
                    first_attempt_opportunity,
                    first_attempt_stopped,
                    recovered: recovered_attempt,
                    final_life: player_life,
                    player_died: false,
                });
            }
            return isolated.finish(EpisodeOutcome {
                threat_turn: first_credible_threat_turn,
                first_win_attempt_turn,
                resolved_table_win_turn: None,
                timing_provenance,
                first_attempt_opportunity,
                first_attempt_stopped,
                recovered: recovered_attempt,
                final_life: player_life,
                player_died: false,
            });
        }
        if line_zones.controller_is_monarch() {
            draw_bounded_cards(&mut hand, &order, &mut position, 1);
        }
        deliver_due_delayed_card_access(&mut hand, &mut pending_delayed_card_access, turn);
        let removed =
            resolve_beginning_of_end_step_self_sacrifices(deck, &mut line_zones, &mut mana_sources);
        remove_persistent_contributions(
            deck,
            &removed,
            &mut commanders_cast,
            &mut engine_count,
            &mut enabler_count,
            &mut payoff_count,
            &mut creature_count,
            &mut protection_count,
            &mut recursion_count,
        );
        discard_to_maximum_hand_size(
            deck,
            mana_access,
            &mut hand,
            order.get(position..).unwrap_or_default(),
            &mut line_zones,
            &mana_sources,
            turn,
            treasure_reserve,
        );
        if turn < options.maximum_turn {
            let activity = table_activity_timeline
                .turn(turn)
                .expect("table activity timeline covers every simulated turn");
            if !apply_table_turn_activity_with_end_steps(
                deck,
                &mut line_zones,
                activity,
                &combat_state,
                &mut hand,
                &order,
                &mut position,
                &mut treasure_reserve,
                &mut player_life,
            ) {
                return isolated.finish(EpisodeOutcome {
                    threat_turn: first_credible_threat_turn,
                    first_win_attempt_turn,
                    resolved_table_win_turn: None,
                    timing_provenance,
                    first_attempt_opportunity,
                    first_attempt_stopped,
                    recovered: recovered_attempt,
                    final_life: player_life,
                    player_died: true,
                });
            }
            if execute_opponent_end_step_top_tutor_before_next_turn(
                deck,
                mana_access,
                &mut hand,
                &mut order,
                &mut position,
                turn,
                &mana_pool,
                &mana_sources,
                &mut rng,
                &mut line_zones,
                &mut treasure_reserve,
                &mut creature_count,
                &mut player_life,
                &mut isolated,
            ) {
                return isolated.finish(EpisodeOutcome {
                    threat_turn: first_credible_threat_turn,
                    first_win_attempt_turn,
                    resolved_table_win_turn: None,
                    timing_provenance,
                    first_attempt_opportunity,
                    first_attempt_stopped,
                    recovered: recovered_attempt,
                    final_life: player_life,
                    player_died: true,
                });
            }
        }
    }

    isolated.finish(EpisodeOutcome {
        threat_turn: first_credible_threat_turn,
        first_win_attempt_turn,
        resolved_table_win_turn: None,
        timing_provenance,
        first_attempt_opportunity,
        first_attempt_stopped,
        recovered: recovered_attempt,
        final_life: player_life,
        player_died: false,
    })
}

fn select_ready_known_line(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    creature_count: u8,
    mana_sources: &[BattlefieldManaSource],
    turn: u8,
    activation_costs: &[CompiledLineActivationCost],
    mana_pool: &mut TurnManaPool,
) -> Option<usize> {
    // Prefer a documented table conversion, but retain catalog order within
    // each class so paired simulations stay deterministic.
    for require_table_lethal in [true, false] {
        for (line_index, line) in deck.known_lines.iter().enumerate() {
            if line.table_lethal_if_resolved != require_table_lethal
                || !line_pieces_usable_together(line, deck, zones, turn)
                || !line_requirements_met(line, deck, zones, creature_count, mana_sources, turn)
            {
                continue;
            }

            let mut paid_pool = mana_pool.clone();
            match activation_costs.get(line_index) {
                Some(CompiledLineActivationCost::None) => {}
                Some(CompiledLineActivationCost::Additional(cost))
                    if activation_cost_is_exactly_modeled(cost)
                        && paid_pool.pay(Some(cost), 0, 0) => {}
                Some(
                    CompiledLineActivationCost::Additional(_)
                    | CompiledLineActivationCost::Unmodeled,
                )
                | None => continue,
            }

            // Commit once only after every piece and prerequisite has passed.
            // Both threat and win-attempt logic consume this one selection.
            *mana_pool = paid_pool;
            return Some(line_index);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn diagnose_explicit_attempt_blocker(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    zones: &KnownLineZoneState,
    creature_count: u8,
    mana_sources: &[BattlefieldManaSource],
    turn: u8,
    activation_costs: &[CompiledLineActivationCost],
    mana_pool: &TurnManaPool,
    additional_generic_per_cast: u8,
    may_attempt_after: u8,
) -> EpisodeAttemptBlocker {
    let mut best = None::<(u16, usize, Option<u8>, ExplicitAttemptBlockerReason)>;
    for (line_index, line) in deck
        .known_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.table_lethal_if_resolved)
    {
        let usable_piece_count = named_line_piece_access_count(line, deck, hand, zones, turn)
            .min(usize::from(u8::MAX)) as u16;
        let missing_card_position =
            first_missing_named_line_card_position(line, deck, hand, zones, turn);
        let (stage, reason) = if !line_pieces_usable_together(line, deck, zones, turn) {
            if missing_card_position.is_some() {
                (1u16, ExplicitAttemptBlockerReason::MissingNamedPieces)
            } else if reviewed_empty_library_sequence(line)
                && line
                    .simulation_requirements
                    .contains(&LineRequirement::SingletonLibrary)
                && deck.cards.iter().any(|card| card.quantity > 1)
            {
                (2, ExplicitAttemptBlockerReason::UnmetPrerequisite)
            } else if reviewed_empty_library_sequence(line)
                && reviewed_sequence_package_is_jointly_payable(
                    line,
                    deck,
                    hand,
                    zones,
                    turn,
                    mana_pool,
                    mana_access,
                    additional_generic_per_cast,
                )
            {
                if turn < may_attempt_after {
                    (5, ExplicitAttemptBlockerReason::DeferredAfterStoppedAttempt)
                } else {
                    (5, ExplicitAttemptBlockerReason::ReadyButNotSelected)
                }
            } else if reviewed_empty_library_sequence(line) {
                (4, ExplicitAttemptBlockerReason::InsufficientNamedCardMana)
            } else {
                (
                    1,
                    ExplicitAttemptBlockerReason::NamedPiecesNotUsableTogether,
                )
            }
        } else if !line_requirements_met(line, deck, zones, creature_count, mana_sources, turn) {
            let unsupported = line_has_intrinsically_unsupported_requirement(line, deck);
            (
                2,
                if unsupported {
                    ExplicitAttemptBlockerReason::UnsupportedRequirement
                } else {
                    ExplicitAttemptBlockerReason::UnmetPrerequisite
                },
            )
        } else {
            match activation_costs.get(line_index) {
                Some(CompiledLineActivationCost::None) if turn < may_attempt_after => {
                    (5, ExplicitAttemptBlockerReason::DeferredAfterStoppedAttempt)
                }
                Some(CompiledLineActivationCost::None) => {
                    // A ready explicit route is normally selected immediately.
                    // Preserve a conservative diagnostic if a future selector
                    // policy chooses a competing line instead.
                    (5, ExplicitAttemptBlockerReason::ReadyButNotSelected)
                }
                Some(CompiledLineActivationCost::Additional(cost))
                    if !activation_cost_is_exactly_modeled(cost) =>
                {
                    (3, ExplicitAttemptBlockerReason::UnsupportedActivationCost)
                }
                Some(CompiledLineActivationCost::Additional(cost)) => {
                    let mut candidate = mana_pool.clone();
                    if candidate.pay(Some(cost), 0, 0) {
                        if turn < may_attempt_after {
                            (5, ExplicitAttemptBlockerReason::DeferredAfterStoppedAttempt)
                        } else {
                            (5, ExplicitAttemptBlockerReason::ReadyButNotSelected)
                        }
                    } else {
                        (4, ExplicitAttemptBlockerReason::InsufficientActivationMana)
                    }
                }
                Some(CompiledLineActivationCost::Unmodeled) | None => {
                    (3, ExplicitAttemptBlockerReason::UnsupportedActivationCost)
                }
            }
        };
        let progress = stage
            .saturating_mul(256)
            .saturating_add(usable_piece_count.min(255));
        if best.is_none_or(|(best_progress, best_index, _, _)| {
            progress > best_progress || progress == best_progress && line_index < best_index
        }) {
            best = Some((progress, line_index, missing_card_position, reason));
        }
    }

    best.map_or(
        if deck_has_executable_combat_route(deck) {
            EpisodeAttemptBlocker {
                line_index: Some(COMBAT_DAMAGE_ROUTE_INDEX),
                missing_card_position: None,
                reason: if turn < may_attempt_after {
                    ExplicitAttemptBlockerReason::DeferredAfterStoppedAttempt
                } else {
                    ExplicitAttemptBlockerReason::UnmetPrerequisite
                },
            }
        } else {
            EpisodeAttemptBlocker {
                line_index: None,
                missing_card_position: None,
                reason: ExplicitAttemptBlockerReason::NoRecognizedExplicitRoute,
            }
        },
        |(_, line_index, missing_card_position, reason)| EpisodeAttemptBlocker {
            line_index: Some(line_index),
            missing_card_position,
            reason,
        },
    )
}

fn first_missing_named_line_card_position(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
) -> Option<u8> {
    let mut required_seen = HashMap::<String, usize>::new();
    line.cards.iter().enumerate().find_map(|(position, name)| {
        let normalized = crate::parser::normalize_card_name(name);
        let required = required_seen.entry(normalized.clone()).or_default();
        *required = required.saturating_add(1);
        let held = hand
            .iter()
            .filter(|card_index| {
                deck.cards
                    .get(**card_index)
                    .is_some_and(|card| card.normalized_name == normalized)
            })
            .count();
        (zones
            .usable_count(deck, &normalized, turn)
            .saturating_add(held)
            < *required)
            .then_some(position.min(usize::from(u8::MAX)) as u8)
    })
}

fn line_has_intrinsically_unsupported_requirement(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
) -> bool {
    line.simulation_requirements
        .iter()
        .any(|requirement| match requirement {
            LineRequirement::TotalExecutionMana
            | LineRequirement::CombatAccess
            | LineRequirement::Unmodeled => true,
            LineRequirement::ReviewedInfiniteManaLoop => {
                !line_has_executable_infinite_mana_cycle(line, deck)
            }
            LineRequirement::ExecutableArtifactTapTreasureLoop => {
                !line_has_executable_artifact_tap_treasure_cycle(line, deck)
            }
            LineRequirement::ExecutableMaskwoodArtifactDwarfTreasureLoop => {
                !line_has_executable_maskwood_artifact_dwarf_treasure_cycle(line, deck)
            }
            LineRequirement::AdditionalCreature { .. }
            | LineRequirement::NonlandManaCapacity { .. }
            | LineRequirement::AdditionalActivationMana { .. }
            | LineRequirement::NamedCardsPayPrintedCosts
            | LineRequirement::ReviewedEmptyLibrarySequence
            | LineRequirement::ExecutableGraveyardStormLoop
            | LineRequirement::ExecutableInfiniteManaCreatureOverrunAttempt
            | LineRequirement::ExternalEnabler
            | LineRequirement::SingletonLibrary
            | LineRequirement::GraveyardSetup { .. } => false,
        })
}

fn activation_cost_is_exactly_modeled(cost: &ManaCostProfile) -> bool {
    cost.faces.len() == 1
        && cost.confidence >= 0.999
        && cost.faces[0]
            .pips
            .iter()
            .all(|pip| !pip.is_unknown && !pip.is_variable && !pip.is_phyrexian && !pip.is_snow)
}

fn line_pieces_usable_together(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
) -> bool {
    if line.cards.is_empty() {
        return false;
    }

    let required = line
        .cards
        .iter()
        .fold(HashMap::<String, usize>::new(), |mut counts, name| {
            let count = counts
                .entry(crate::parser::normalize_card_name(name))
                .or_default();
            *count = count.saturating_add(1);
            counts
        });
    let requires_same_turn_permanent_entry = line
        .simulation_requirements
        .contains(&LineRequirement::ReviewedEmptyLibrarySequence);
    let mut first_selected_spell_sequence = None::<u16>;

    for (normalized_name, required_count) in &required {
        let Some(card) = unique_card_by_normalized_name(deck, normalized_name) else {
            return false;
        };
        match modeled_line_card_kind(card) {
            Some(ModeledLineCardKind::Spell) => {
                let mut cast_sequences = zones
                    .spells_cast_this_turn
                    .iter()
                    .filter(|cast| {
                        cast.turn == turn
                            && deck.cards.get(cast.card_index).is_some_and(|candidate| {
                                candidate.normalized_name == *normalized_name
                            })
                    })
                    .map(|cast| cast.sequence)
                    .collect::<Vec<_>>();
                if cast_sequences.len() < *required_count {
                    return false;
                }
                cast_sequences.sort_unstable();
                // Select the latest sufficient executions. If an earlier copy
                // was cast before the other pieces existed, it cannot make a
                // later valid execution disappear.
                let selected_start = cast_sequences.len() - *required_count;
                let earliest_for_name = cast_sequences[selected_start];
                first_selected_spell_sequence = Some(
                    first_selected_spell_sequence
                        .map_or(earliest_for_name, |current| current.min(earliest_for_name)),
                );
            }
            Some(ModeledLineCardKind::Permanent) => {}
            None => return false,
        }
    }

    for (normalized_name, required_count) in &required {
        let Some(card) = unique_card_by_normalized_name(deck, normalized_name) else {
            return false;
        };
        if !matches!(
            modeled_line_card_kind(card),
            Some(ModeledLineCardKind::Permanent)
        ) {
            continue;
        }
        let available = zones
            .battlefield
            .iter()
            .filter(|presence| {
                presence.entered_turn <= turn
                    && (!requires_same_turn_permanent_entry || presence.entered_turn == turn)
                    && first_selected_spell_sequence
                        .is_none_or(|spell_sequence| presence.sequence < spell_sequence)
                    && deck
                        .cards
                        .get(presence.card_index)
                        .is_some_and(|candidate| candidate.normalized_name == *normalized_name)
            })
            .count();
        if available < *required_count {
            return false;
        }
    }

    true
}

fn line_has_executable_infinite_mana_cycle(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
) -> bool {
    let members = executable_line_members(line, deck);
    if members.len() != line.cards.len()
        || members.iter().any(|card| {
            card.ability_program
                .unsupported_abilities()
                .next()
                .is_some()
        })
    {
        return false;
    }
    let has_nonland_plus_one = members.iter().any(|card| {
        card.ability_program.executable_abilities().any(|ability| {
            matches!(
                ability.timing,
                AbilityTiming::Triggered {
                    event: crate::ability_program::TriggerEvent {
                        kind: TriggerEventKind::PermanentTappedForMana,
                        actor: ControllerRelation::You,
                        ..
                    }
                }
            ) && ability.effects.iter().any(|effect| {
                matches!(
                    effect,
                    AbilityEffect::ModifyNonlandMana(modifier)
                        if modifier.additional_amount >= 1
                            && modifier.kind == ProgramManaKind::AnyTypeProducedByTriggeringPermanent
                )
            })
        })
    });
    let has_repeatable_source = members.iter().any(|card| {
        if card.effects.card_types.is_land {
            return false;
        }
        let mut taps_for_at_least_three = false;
        let mut pays_three_to_untap_self = false;
        for ability in card.ability_program.executable_abilities() {
            if matches!(
                ability.timing,
                AbilityTiming::Activated { .. }
            ) && ability.costs == [AbilityCost::TapSelf]
                && ability.effects.iter().any(|effect| {
                    matches!(
                        effect,
                        AbilityEffect::AddMana(mana)
                            if mana.amount >= 3
                                && matches!(mana.kind, ProgramManaKind::Fixed(profile) if profile.colorless >= 3)
                    )
                })
            {
                taps_for_at_least_three = true;
            }
            if matches!(
                ability.timing,
                AbilityTiming::Activated { .. }
            ) && ability.costs.len() == 1
                && matches!(
                    &ability.costs[0],
                    AbilityCost::Mana(ProgramManaCost::PrintedSymbols { profile, .. })
                        if profile.generic == 3
                            && profile.white == 0
                            && profile.blue == 0
                            && profile.black == 0
                            && profile.red == 0
                            && profile.green == 0
                            && profile.colorless == 0
                            && profile.variable_x == 0
                )
                && ability
                    .effects
                    .contains(&AbilityEffect::Untap(TargetSelector::SelfPermanent))
            {
                pays_three_to_untap_self = true;
            }
        }
        taps_for_at_least_three && pays_three_to_untap_self
    });
    has_nonland_plus_one && has_repeatable_source
}

fn card_has_executable_variable_creature_overrun(card: &CompiledCard) -> bool {
    let is_sorcery = card
        .type_line
        .split(|character: char| !character.is_alphabetic())
        .any(|word| word.eq_ignore_ascii_case("Sorcery"));
    if !is_sorcery
        || card
            .ability_program
            .unsupported_abilities()
            .next()
            .is_some()
    {
        return false;
    }

    let mut tutor_count = 0;
    let mut overrun_count = 0;
    for ability in card.ability_program.executable_abilities() {
        if ability.timing != AbilityTiming::SpellResolution {
            return false;
        }
        for effect in &ability.effects {
            match effect {
                AbilityEffect::VariableCreatureTutor(tutor)
                    if tutor.from_library
                        && tutor.from_graveyard
                        && tutor.destination == ProgramZone::Battlefield
                        && tutor.mana_value_at_most_x
                        && tutor.shuffle_if_library_searched =>
                {
                    tutor_count += 1;
                }
                AbilityEffect::VariableCreatureOverrun(overrun)
                    if overrun.minimum_x == 10
                        && overrun.creatures_you_control
                        && overrun.power_bonus_equals_x
                        && overrun.toughness_bonus_equals_x
                        && overrun.grants_haste
                        && overrun.until_end_of_turn =>
                {
                    overrun_count += 1;
                }
                _ => return false,
            }
        }
    }
    tutor_count == 1 && overrun_count == 1
}

fn card_is_bounded_attack_capable(card: &CompiledCard) -> bool {
    card.effects.card_types.is_creature
        && !card_has_keyword(card, "Defender")
        && !card.ability_program.abilities.iter().any(|ability| {
            let oracle = match ability {
                crate::ability_program::AbilityCompilation::Executable(ability) => {
                    &ability.normalized_oracle
                }
                crate::ability_program::AbilityCompilation::Unsupported(ability) => {
                    &ability.normalized_oracle
                }
            };
            oracle_prevents_source_from_attacking(oracle)
        })
}

fn oracle_prevents_source_from_attacking(oracle: &str) -> bool {
    oracle
        .replace(['’', '‘'], "'")
        .to_ascii_lowercase()
        .split(['.', '\n'])
        .filter_map(|clause| {
            let clause = clause.trim();
            clause
                .find("can't attack")
                .map(|marker| (clause[..marker].trim(), clause[marker + 12..].trim()))
        })
        .any(|(subject, restriction)| {
            // These clauses constrain attacks *toward* this permanent's
            // controller, not attacks made by the source creature itself.
            let opponent_facing_scope = subject.starts_with("other ")
                || subject.contains("your opponents control")
                || restriction == "you"
                || restriction.starts_with("you ")
                || restriction.starts_with("a planeswalker you control")
                || restriction.starts_with("planeswalkers you control");
            !opponent_facing_scope
        })
}

fn bounded_attack_capable_battlefield_count(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
) -> usize {
    zones
        .battlefield
        .iter()
        .filter(|presence| presence.entered_turn <= turn)
        .filter(|presence| {
            deck.cards
                .get(presence.card_index)
                .is_some_and(card_is_bounded_attack_capable)
        })
        .count()
}

fn line_has_executable_infinite_mana_creature_overrun_attempt(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
) -> bool {
    if !line_has_executable_infinite_mana_cycle(line, deck)
        || bounded_attack_capable_battlefield_count(deck, zones, turn) < 3
    {
        return false;
    }

    let conversions = line
        .cards
        .iter()
        .filter_map(|name| {
            let normalized = crate::parser::normalize_card_name(name);
            unique_card_by_normalized_name(deck, &normalized)
        })
        .filter(|card| card_has_executable_variable_creature_overrun(card))
        .collect::<Vec<_>>();
    conversions.len() == 1
        && zones
            .typed_overrun_casts_this_turn
            .iter()
            .filter(|card_index| {
                deck.cards
                    .get(**card_index)
                    .is_some_and(|card| card.normalized_name == conversions[0].normalized_name)
            })
            .count()
            == 1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GraveyardStormProgram {
    permission_source: usize,
    mana_source: usize,
    mill_spell: usize,
    exile_count: u8,
    mana_amount: u8,
    mill_per_resolution: u8,
}

impl GraveyardStormProgram {
    fn members(self) -> [usize; 3] {
        [self.permission_source, self.mana_source, self.mill_spell]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GraveyardStormPlanningAccess {
    program: GraveyardStormProgram,
    supported: [bool; 3],
}

impl GraveyardStormPlanningAccess {
    fn supported_count(self) -> usize {
        self.supported
            .into_iter()
            .filter(|supported| *supported)
            .count()
    }

    fn missing_members(self) -> impl Iterator<Item = usize> {
        self.program
            .members()
            .into_iter()
            .zip(self.supported)
            .filter_map(|(card_index, supported)| (!supported).then_some(card_index))
    }
}

fn compile_graveyard_storm_program(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
) -> Option<GraveyardStormProgram> {
    if line.cards.len() != 3
        || !line
            .simulation_requirements
            .contains(&LineRequirement::ExecutableGraveyardStormLoop)
        || !line
            .simulation_requirements
            .contains(&LineRequirement::NamedCardsPayPrintedCosts)
        || line.simulation_requirements.iter().any(|requirement| {
            !matches!(
                requirement,
                LineRequirement::NamedCardsPayPrintedCosts
                    | LineRequirement::ExecutableGraveyardStormLoop
                    | LineRequirement::GraveyardSetup { .. }
            )
        })
    {
        return None;
    }

    let mut members = Vec::with_capacity(3);
    for name in &line.cards {
        let normalized = crate::parser::normalize_card_name(name);
        let mut matches = deck
            .cards
            .iter()
            .enumerate()
            .filter(|(_, card)| card.normalized_name == normalized);
        let (index, _) = matches.next()?;
        if matches.next().is_some() || members.contains(&index) {
            return None;
        }
        members.push(index);
    }

    let permission_sources = members
        .iter()
        .filter_map(|index| {
            exact_escape_permission_and_lifetime(&deck.cards[*index]).map(|count| (*index, count))
        })
        .collect::<Vec<_>>();
    let mana_sources = members
        .iter()
        .filter_map(|index| {
            exact_discard_sacrifice_mana_ability(&deck.cards[*index]).map(|amount| (*index, amount))
        })
        .collect::<Vec<_>>();
    let mill_spells = members
        .iter()
        .filter_map(|index| {
            exact_mill_storm_spell(&deck.cards[*index]).map(|count| (*index, count))
        })
        .collect::<Vec<_>>();
    if permission_sources.len() != 1 || mana_sources.len() != 1 || mill_spells.len() != 1 {
        return None;
    }

    let program = GraveyardStormProgram {
        permission_source: permission_sources[0].0,
        exile_count: permission_sources[0].1,
        mana_source: mana_sources[0].0,
        mana_amount: mana_sources[0].1,
        mill_spell: mill_spells[0].0,
        mill_per_resolution: mill_spells[0].1,
    };
    (program.permission_source != program.mana_source
        && program.permission_source != program.mill_spell
        && program.mana_source != program.mill_spell)
        .then_some(program)
}

fn graveyard_storm_planning_access(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    hand: &[usize],
    zones: &KnownLineZoneState,
    available_library_copies: impl Fn(usize) -> usize,
) -> Option<GraveyardStormPlanningAccess> {
    let program = compile_graveyard_storm_program(line, deck)?;
    let hand_contains = |card_index| hand.contains(&card_index);
    let battlefield_contains_index = |card_index| {
        zones
            .battlefield
            .iter()
            .any(|presence| presence.card_index == card_index)
    };
    let graveyard_contains = |card_index| zones.graveyard.contains(&card_index);
    let mill_spell_survives_known_zones = graveyard_contains(program.mill_spell)
        || hand_contains(program.mill_spell) && !active_necropotence_lifecycle(deck, zones);
    let access = GraveyardStormPlanningAccess {
        program,
        supported: [
            battlefield_contains_index(program.permission_source)
                || hand_contains(program.permission_source),
            battlefield_contains_index(program.mana_source)
                || hand_contains(program.mana_source)
                || graveyard_contains(program.mana_source),
            mill_spell_survives_known_zones,
        ],
    };
    access
        .missing_members()
        .all(|card_index| available_library_copies(card_index) > 0)
        .then_some(access)
}

fn plain_executable_ability_root_is_complete(card: &CompiledCard, expected_count: usize) -> bool {
    let program = &card.ability_program;
    program.abilities.len() == expected_count
        && program.executable_abilities().count() == expected_count
        && program.unsupported_abilities().next().is_none()
        && program.necropotence_lifecycle.is_none()
        && program.self_transfer_tutor_permanent.is_none()
        && program.entry_linked_permanent.is_none()
        && program.atomic_transaction.is_none()
        && program.face_programs.is_empty()
}

fn exact_escape_permission_and_lifetime(card: &CompiledCard) -> Option<u8> {
    if !plain_executable_ability_root_is_complete(card, 2) {
        return None;
    }
    let expected_filter = ProgramObjectFilter {
        nonland: true,
        ..ProgramObjectFilter::default()
    };
    let mut exile_count = None;
    let mut lifetime_count = 0u8;
    for ability in card.ability_program.executable_abilities() {
        if ability.timing == AbilityTiming::StaticModifier
            && ability.costs.is_empty()
            && ability.preconditions == [AbilityPrecondition::SourceZone(ProgramZone::Battlefield)]
            && let [AbilityEffect::GrantCastPermission(grant)] = ability.effects.as_slice()
            && grant.from == ProgramZone::Graveyard
            && grant.owner == ControllerRelation::You
            && grant.filter == expected_filter
            && grant.mechanic == CastPermissionKind::Escape
            && let [
                AbilityCost::Mana(ProgramManaCost::GrantedCardPrintedManaCost),
                AbilityCost::ExileFromGraveyard { count, other: true },
            ] = grant.alternative_cost.as_slice()
            && *count == 3
            && exile_count.replace(*count as u8).is_some()
        {
            return None;
        }
        if matches!(
            &ability.timing,
            AbilityTiming::Triggered { event }
                if event.kind == TriggerEventKind::BeginningOfEndStep
                    && event.actor == ControllerRelation::Any
        ) && ability.costs.is_empty()
            && ability.effects == [AbilityEffect::SacrificeSelf]
        {
            lifetime_count = lifetime_count.saturating_add(1);
        }
    }
    (lifetime_count == 1)
        .then_some(exile_count?)
        .filter(|count| *count == 3)
}

fn exact_discard_sacrifice_mana_ability(card: &CompiledCard) -> Option<u8> {
    if !plain_executable_ability_root_is_complete(card, 1) {
        return None;
    }
    let mut amount = None;
    for ability in card.ability_program.executable_abilities() {
        if matches!(ability.timing, AbilityTiming::Activated { .. })
            && ability.costs
                == [
                    AbilityCost::Discard(DiscardCost::EntireHand),
                    AbilityCost::SacrificeSelf,
                ]
            && ability.preconditions == [AbilityPrecondition::SourceZone(ProgramZone::Battlefield)]
            && let [AbilityEffect::AddMana(mana)] = ability.effects.as_slice()
            && mana.amount == 3
            && mana.kind == ProgramManaKind::AnyOneColor
            && amount.replace(mana.amount as u8).is_some()
        {
            return None;
        }
    }
    amount
}

fn execute_discard_hand_sacrifice_mana_action(
    deck: &CompiledDeck,
    source_card_index: usize,
    source_sequence: u16,
    color: FiveColorManaChoice,
    hand: &mut Vec<usize>,
    zones: &mut KnownLineZoneState,
    mana_pool: &mut TurnManaPool,
) -> Option<usize> {
    let presence = zones.battlefield.iter().find(|presence| {
        presence.card_index == source_card_index && presence.sequence == source_sequence
    })?;
    let amount = deck
        .cards
        .get(presence.card_index)
        .and_then(exact_discard_sacrifice_mana_ability)?;
    if amount != 3 {
        return None;
    }

    let mut staged_hand = hand.clone();
    let mut staged_zones = zones.clone();
    let mut staged_pool = mana_pool.clone();
    staged_zones.record_discards(deck, std::mem::take(&mut staged_hand));
    let removed = staged_zones.remove_permanent_sequence(deck, source_sequence, true)?;
    if removed != source_card_index {
        return None;
    }
    synchronize_turn_pool_with_battlefield(&mut staged_pool, &staged_zones);
    staged_pool.add_floating(color.mask(), amount);

    *hand = staged_hand;
    *zones = staged_zones;
    *mana_pool = staged_pool;
    Some(removed)
}

#[allow(clippy::too_many_arguments)]
fn reviewed_primer_window_action_is_runtime_legal(
    action: TurnAction,
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    library_order: &[usize],
    next_draw_position: usize,
    hand: &[usize],
    zones: &KnownLineZoneState,
    mana_pool: &TurnManaPool,
    player_life: f32,
    additional_generic_per_cast: u8,
    rule_of_law_cap_active: bool,
) -> bool {
    matches!(
        action,
        TurnAction::CastReviewedRandomDiscardWithManaResponse { .. }
    ) && reviewed_graveyard_storm_primer_window_actions(
        deck,
        mana_access,
        hand,
        zones,
        mana_pool,
        player_life,
        additional_generic_per_cast,
        rule_of_law_cap_active,
        |card_index| {
            library_order
                .iter()
                .copied()
                .skip(next_draw_position)
                .filter(|candidate| *candidate == card_index)
                .count()
        },
    )
    .contains(&action)
}

#[allow(clippy::too_many_arguments)]
fn execute_reviewed_random_discard_mana_response(
    action: TurnAction,
    transaction: &TypedAtomicTransaction,
    deck: &CompiledDeck,
    hand: &mut Vec<usize>,
    mana_pool: &mut TurnManaPool,
    zones: &mut KnownLineZoneState,
) -> Option<usize> {
    let TurnAction::CastReviewedRandomDiscardWithManaResponse {
        mana_source_card_index,
        mana_source_sequence,
        tutor_source_card_index,
        tutor_source_sequence,
        permission_source_card_index,
        mill_target_card_index,
        color,
        ..
    } = action
    else {
        return None;
    };
    let TypedAtomicTransaction::SearchRandomDiscardShuffle { tutor } = transaction else {
        return None;
    };
    if active_necropotence_lifecycle(deck, zones)
        || !deck
            .cards
            .get(mill_target_card_index)
            .is_some_and(|target| program_tutor_matches(&tutor.filter, target))
        || !zones.battlefield.iter().any(|presence| {
            presence.card_index == mana_source_card_index
                && presence.sequence == mana_source_sequence
        })
        || !zones.battlefield.iter().any(|presence| {
            presence.card_index == tutor_source_card_index
                && presence.sequence == tutor_source_sequence
                && deck
                    .cards
                    .get(presence.card_index)
                    .and_then(compile_typed_first_use_self_transfer_tutor)
                    .is_some()
        })
        || !deck.known_lines.iter().any(|line| {
            compile_graveyard_storm_program(line, deck).is_some_and(|program| {
                program.mana_source == mana_source_card_index
                    && program.permission_source == permission_source_card_index
                    && program.mill_spell == mill_target_card_index
            })
        })
    {
        return None;
    }
    let sacrificed = execute_discard_hand_sacrifice_mana_action(
        deck,
        mana_source_card_index,
        mana_source_sequence,
        color,
        hand,
        zones,
        mana_pool,
    )?;
    (sacrificed == mana_source_card_index && hand.is_empty()).then_some(sacrificed)
}

#[allow(clippy::too_many_arguments)]
fn execute_reviewed_random_discard_resolution_after_mana_response(
    action: TurnAction,
    transaction: &TypedAtomicTransaction,
    deck: &CompiledDeck,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    hand: &mut Vec<usize>,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
) -> Option<AtomicSpellResolution> {
    let TurnAction::CastReviewedRandomDiscardWithManaResponse {
        mana_source_card_index,
        mana_source_sequence,
        tutor_source_card_index,
        tutor_source_sequence,
        permission_source_card_index,
        mill_target_card_index,
        ..
    } = action
    else {
        return None;
    };
    let TypedAtomicTransaction::SearchRandomDiscardShuffle { tutor } = transaction else {
        return None;
    };
    if active_necropotence_lifecycle(deck, zones)
        || !hand.is_empty()
        || zones.battlefield.iter().any(|presence| {
            presence.card_index == mana_source_card_index
                && presence.sequence == mana_source_sequence
        })
        || !zones.battlefield.iter().any(|presence| {
            presence.card_index == tutor_source_card_index
                && presence.sequence == tutor_source_sequence
                && deck
                    .cards
                    .get(presence.card_index)
                    .and_then(compile_typed_first_use_self_transfer_tutor)
                    .is_some()
        })
        || !deck
            .cards
            .get(mill_target_card_index)
            .is_some_and(|target| program_tutor_matches(&tutor.filter, target))
        || !deck.known_lines.iter().any(|line| {
            compile_graveyard_storm_program(line, deck).is_some_and(|program| {
                program.mana_source == mana_source_card_index
                    && program.permission_source == permission_source_card_index
                    && program.mill_spell == mill_target_card_index
            })
        })
    {
        return None;
    }
    let target_position = library_order
        .iter()
        .copied()
        .enumerate()
        .skip(next_draw_position)
        .find_map(|(position, candidate)| {
            (candidate == mill_target_card_index).then_some(position)
        })?;

    let mut staged_library = library_order.clone();
    let mut staged_hand = hand.clone();
    let mut staged_rng = rng.clone();
    let mut staged_zones = zones.clone();
    let searched_card = staged_library.remove(target_position);
    if searched_card != mill_target_card_index {
        return None;
    }
    staged_hand.push(searched_card);
    let discarded_card = staged_hand.pop()?;
    if discarded_card != mill_target_card_index || !staged_hand.is_empty() {
        return None;
    }
    staged_zones.record_discard(deck, discarded_card);
    if !staged_zones.graveyard.contains(&discarded_card) {
        return None;
    }
    let unseen_start = next_draw_position.min(staged_library.len());
    staged_library[unseen_start..].shuffle(&mut staged_rng);

    *library_order = staged_library;
    *hand = staged_hand;
    *rng = staged_rng;
    *zones = staged_zones;
    Some(AtomicSpellResolution {
        searched_card: Some(searched_card),
        discarded_card: Some(discarded_card),
        ..AtomicSpellResolution::default()
    })
}

fn exact_mill_storm_spell(card: &CompiledCard) -> Option<u8> {
    if !plain_executable_ability_root_is_complete(card, 2) {
        return None;
    }
    let mut mill_count = None;
    let mut storm_count = 0u8;
    for ability in card.ability_program.executable_abilities() {
        if ability.timing == AbilityTiming::SpellResolution
            && ability.costs.is_empty()
            && ability.preconditions == [AbilityPrecondition::SourceZone(ProgramZone::Stack)]
            && let [AbilityEffect::Mill(mill)] = ability.effects.as_slice()
            && mill.player == PlayerSelector::TargetPlayer
            && mill.count == 3
            && mill_count.replace(mill.count as u8).is_some()
        {
            return None;
        }
        if matches!(
            &ability.timing,
            AbilityTiming::Triggered { event }
                if event.kind == TriggerEventKind::ThisSpellCast
                    && event.actor == ControllerRelation::You
        ) && ability.costs.is_empty()
            && let [AbilityEffect::CopyThisSpell(copy)] = ability.effects.as_slice()
            && copy.count == SpellCopyCount::EachSpellCastBeforeThisSpellThisTurn
            && copy.target_choice == CopyTargetChoice::MayChooseNewTargets
        {
            storm_count = storm_count.saturating_add(1);
        }
    }
    (storm_count == 1)
        .then_some(mill_count?)
        .filter(|count| *count == 3)
}

#[allow(clippy::too_many_arguments)]
fn execute_ready_graveyard_storm_line(
    deck: &CompiledDeck,
    hand: &mut Vec<usize>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    zones: &mut KnownLineZoneState,
    turn: u8,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &mut TurnManaPool,
    spells_cast_this_turn: &mut u8,
    player_life: &mut f32,
    additional_generic_per_cast: u8,
    rule_of_law: bool,
    graveyard_shutdown: bool,
) -> Option<usize> {
    for require_table_lethal in [true, false] {
        for (line_index, line) in deck.known_lines.iter().enumerate() {
            if line.table_lethal_if_resolved != require_table_lethal {
                continue;
            }
            if execute_graveyard_storm_transaction(
                line,
                deck,
                hand,
                library_order,
                next_draw_position,
                zones,
                turn,
                mana_access,
                mana_pool,
                spells_cast_this_turn,
                player_life,
                additional_generic_per_cast,
                rule_of_law,
                graveyard_shutdown,
            ) {
                return Some(line_index);
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn execute_graveyard_storm_under_isolated_scenario(
    deck: &CompiledDeck,
    hand: &mut Vec<usize>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    zones: &mut KnownLineZoneState,
    turn: u8,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &mut TurnManaPool,
    spells_cast_this_turn: &mut u8,
    player_life: &mut f32,
    isolated: &mut IsolatedScenarioRuntime,
) -> Option<usize> {
    let rule_of_law = isolated.is(InteractionScenario::RuleOfLawCap);
    let graveyard_shutdown = isolated.is(InteractionScenario::GraveyardShutdown);
    let generic_tax = isolated.is(InteractionScenario::GenericTaxStax);
    if rule_of_law || graveyard_shutdown || generic_tax && !isolated.applied() {
        let mut preview_hand = hand.clone();
        let mut preview_library = library_order.clone();
        let mut preview_zones = zones.clone();
        let mut preview_pool = mana_pool.clone();
        let mut preview_spell_count = *spells_cast_this_turn;
        let mut preview_life = *player_life;
        execute_ready_graveyard_storm_line(
            deck,
            &mut preview_hand,
            &mut preview_library,
            next_draw_position,
            &mut preview_zones,
            turn,
            mana_access,
            &mut preview_pool,
            &mut preview_spell_count,
            &mut preview_life,
            0,
            false,
            false,
        )?;
        isolated.observe_opportunity(InteractionScenario::GenericTaxStax);
        isolated.observe_opportunity(InteractionScenario::RuleOfLawCap);
        isolated.observe_opportunity(InteractionScenario::GraveyardShutdown);
        if !isolated.applied() {
            isolated.activate(turn, 1);
        } else {
            isolated.add_affected_event();
        }
        if !generic_tax {
            return None;
        }
    }

    let result = execute_ready_graveyard_storm_line(
        deck,
        hand,
        library_order,
        next_draw_position,
        zones,
        turn,
        mana_access,
        mana_pool,
        spells_cast_this_turn,
        player_life,
        u8::from(generic_tax && isolated.applied()),
        rule_of_law,
        graveyard_shutdown,
    );
    if result.is_some() {
        isolated.observe_opportunity(InteractionScenario::GenericTaxStax);
        isolated.observe_opportunity(InteractionScenario::RuleOfLawCap);
        isolated.observe_opportunity(InteractionScenario::GraveyardShutdown);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn execute_graveyard_storm_transaction(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    hand: &mut Vec<usize>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    zones: &mut KnownLineZoneState,
    turn: u8,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &mut TurnManaPool,
    spells_cast_this_turn: &mut u8,
    player_life: &mut f32,
    additional_generic_per_cast: u8,
    rule_of_law: bool,
    graveyard_shutdown: bool,
) -> bool {
    if rule_of_law || graveyard_shutdown {
        return false;
    }
    let Some(program) = compile_graveyard_storm_program(line, deck) else {
        return false;
    };
    let Some(mana_access) = mana_access else {
        return false;
    };

    let mut candidate_hand = hand.clone();
    let mut candidate_library = library_order.clone();
    let mut candidate_zones = zones.clone();
    let mut candidate_pool = mana_pool.clone();
    let mut candidate_spell_count = *spells_cast_this_turn;
    let mut candidate_life = *player_life;
    if !stage_graveyard_storm_permanents(
        deck,
        program,
        &mut candidate_hand,
        &mut candidate_zones,
        turn,
        mana_access,
        &mut candidate_pool,
        &mut candidate_spell_count,
        &mut candidate_life,
        additional_generic_per_cast,
    ) {
        return false;
    }

    let self_target_resolutions = usize::from(
        u16::from(program.exile_count)
            .saturating_mul(2)
            .div_ceil(u16::from(program.mill_per_resolution)),
    );
    let Some(first_mill) = execute_graveyard_storm_iteration(
        deck,
        program,
        &mut candidate_hand,
        &mut candidate_library,
        next_draw_position,
        &mut candidate_zones,
        turn,
        mana_access,
        &mut candidate_pool,
        &mut candidate_spell_count,
        &mut candidate_life,
        additional_generic_per_cast,
        self_target_resolutions,
    ) else {
        return false;
    };
    let first_fodder = graveyard_storm_fodder_count(&candidate_zones, program);
    let first_mana = candidate_pool.total();

    let Some(second_mill) = execute_graveyard_storm_iteration(
        deck,
        program,
        &mut candidate_hand,
        &mut candidate_library,
        next_draw_position,
        &mut candidate_zones,
        turn,
        mana_access,
        &mut candidate_pool,
        &mut candidate_spell_count,
        &mut candidate_life,
        additional_generic_per_cast,
        self_target_resolutions,
    ) else {
        return false;
    };
    let second_fodder = graveyard_storm_fodder_count(&candidate_zones, program);
    if second_mill < first_mill
        || second_fodder < first_fodder
        || candidate_pool.total() < first_mana
    {
        return false;
    }

    // Prove one more complete recurrence on a private clone. The third
    // iteration is not committed; it establishes that the two real iterations
    // did not merely consume a finite pile and stop.
    let mut proof_hand = candidate_hand.clone();
    let mut proof_library = candidate_library.clone();
    let mut proof_zones = candidate_zones.clone();
    let mut proof_pool = candidate_pool.clone();
    let mut proof_spell_count = candidate_spell_count;
    let mut proof_life = candidate_life;
    if execute_graveyard_storm_iteration(
        deck,
        program,
        &mut proof_hand,
        &mut proof_library,
        next_draw_position,
        &mut proof_zones,
        turn,
        mana_access,
        &mut proof_pool,
        &mut proof_spell_count,
        &mut proof_life,
        additional_generic_per_cast,
        self_target_resolutions,
    )
    .is_none()
    {
        return false;
    }

    *hand = candidate_hand;
    *library_order = candidate_library;
    *zones = candidate_zones;
    *mana_pool = candidate_pool;
    *spells_cast_this_turn = candidate_spell_count;
    *player_life = candidate_life;
    true
}

#[allow(clippy::too_many_arguments)]
fn stage_graveyard_storm_permanents(
    deck: &CompiledDeck,
    program: GraveyardStormProgram,
    hand: &mut Vec<usize>,
    zones: &mut KnownLineZoneState,
    turn: u8,
    mana_access: &ManaAccessProfile,
    mana_pool: &mut TurnManaPool,
    spells_cast_this_turn: &mut u8,
    player_life: &mut f32,
    additional_generic_per_cast: u8,
) -> bool {
    if !battlefield_contains(zones, program.permission_source)
        && !cast_permanent_from_hand_exact(
            deck,
            program.permission_source,
            hand,
            zones,
            turn,
            mana_access,
            mana_pool,
            spells_cast_this_turn,
            player_life,
            additional_generic_per_cast,
        )
    {
        return false;
    }
    if !battlefield_contains(zones, program.permission_source) {
        return false;
    }

    if battlefield_contains(zones, program.mana_source) {
        return true;
    }
    if hand.contains(&program.mana_source) {
        return cast_permanent_from_hand_exact(
            deck,
            program.mana_source,
            hand,
            zones,
            turn,
            mana_access,
            mana_pool,
            spells_cast_this_turn,
            player_life,
            additional_generic_per_cast,
        );
    }
    if zones.graveyard.contains(&program.mana_source) {
        return escape_cast_exact(
            deck,
            program,
            program.mana_source,
            zones,
            turn,
            mana_access,
            mana_pool,
            spells_cast_this_turn,
            player_life,
            additional_generic_per_cast,
        );
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn cast_permanent_from_hand_exact(
    deck: &CompiledDeck,
    card_index: usize,
    hand: &mut Vec<usize>,
    zones: &mut KnownLineZoneState,
    turn: u8,
    mana_access: &ManaAccessProfile,
    mana_pool: &mut TurnManaPool,
    spells_cast_this_turn: &mut u8,
    player_life: &mut f32,
    additional_generic_per_cast: u8,
) -> bool {
    let Some(position) = hand.iter().position(|candidate| *candidate == card_index) else {
        return false;
    };
    let Some(card) = deck.cards.get(card_index) else {
        return false;
    };
    if !matches!(
        modeled_line_card_kind(card),
        Some(ModeledLineCardKind::Permanent)
    ) || !pay_exact_printed_cost_with_context(
        mana_pool,
        deck,
        zones,
        mana_access,
        card_index,
        additional_generic_per_cast,
    ) {
        return false;
    }
    hand.swap_remove(position);
    *spells_cast_this_turn = spells_cast_this_turn.saturating_add(1);
    apply_controller_spell_cast_triggers(
        deck,
        zones,
        card_index,
        turn,
        *spells_cast_this_turn,
        mana_pool,
        player_life,
    );
    zones.record_cast(deck, card_index, turn);
    true
}

#[allow(clippy::too_many_arguments)]
fn execute_graveyard_storm_iteration(
    deck: &CompiledDeck,
    program: GraveyardStormProgram,
    hand: &mut Vec<usize>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    zones: &mut KnownLineZoneState,
    turn: u8,
    mana_access: &ManaAccessProfile,
    mana_pool: &mut TurnManaPool,
    spells_cast_this_turn: &mut u8,
    player_life: &mut f32,
    additional_generic_per_cast: u8,
    self_target_resolutions: usize,
) -> Option<usize> {
    let position = zones
        .battlefield
        .iter()
        .position(|presence| presence.card_index == program.mana_source)?;
    let removed = zones.battlefield.remove(position);
    zones.advance_sequence();
    zones.graveyard.push(removed.card_index);
    zones.record_discards(deck, std::mem::take(hand));

    for color in [
        ManaColorMask::WHITE,
        ManaColorMask::BLUE,
        ManaColorMask::BLACK,
        ManaColorMask::RED,
        ManaColorMask::GREEN,
    ] {
        let mut color_zones = zones.clone();
        let mut color_pool = mana_pool.clone();
        let mut color_spell_count = *spells_cast_this_turn;
        let mut color_life = *player_life;
        color_pool.add_floating(color, program.mana_amount);
        // The LED on the battlefield is the copy established before this
        // iteration (including the primer's first escape after Breach).
        // Sacrifice it, escape Brain Freeze with the other three initial
        // fodder cards, resolve the mill, and only then use replenished fodder
        // to establish LED for the next iteration. Re-escaping LED before
        // Brain Freeze would charge the first loop twice and incorrectly
        // require nine setup cards instead of the primer's exact six.
        if !escape_cast_exact(
            deck,
            program,
            program.mill_spell,
            &mut color_zones,
            turn,
            mana_access,
            &mut color_pool,
            &mut color_spell_count,
            &mut color_life,
            additional_generic_per_cast,
        ) {
            continue;
        }

        let resolutions = usize::from(color_spell_count);
        // Only enough copies target us to replace the six cards paid to escape
        // the two spells. The reviewed optional target-choice clause lets
        // every remaining copy target opponents. That proves attempt
        // authority, never resolved-table-win authority.
        if resolutions <= self_target_resolutions {
            continue;
        }
        let mill_count =
            usize::from(program.mill_per_resolution).saturating_mul(self_target_resolutions);
        let remaining_library = library_order.len().saturating_sub(next_draw_position);
        if mill_count == 0 || remaining_library < mill_count {
            continue;
        }
        let mut color_library = library_order.clone();
        for _ in 0..mill_count {
            let card_index = color_library.remove(next_draw_position);
            color_zones.graveyard.push(card_index);
        }
        if !escape_cast_exact(
            deck,
            program,
            program.mana_source,
            &mut color_zones,
            turn,
            mana_access,
            &mut color_pool,
            &mut color_spell_count,
            &mut color_life,
            additional_generic_per_cast,
        ) {
            continue;
        }
        *zones = color_zones;
        *mana_pool = color_pool;
        *spells_cast_this_turn = color_spell_count;
        *player_life = color_life;
        *library_order = color_library;
        return Some(mill_count);
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn escape_cast_exact(
    deck: &CompiledDeck,
    program: GraveyardStormProgram,
    card_index: usize,
    zones: &mut KnownLineZoneState,
    turn: u8,
    mana_access: &ManaAccessProfile,
    mana_pool: &mut TurnManaPool,
    spells_cast_this_turn: &mut u8,
    player_life: &mut f32,
    additional_generic_per_cast: u8,
) -> bool {
    if !battlefield_contains(zones, program.permission_source) {
        return false;
    }
    let Some(graveyard_position) = zones
        .graveyard
        .iter()
        .position(|candidate| *candidate == card_index)
    else {
        return false;
    };
    let mut candidate_zones = zones.clone();
    candidate_zones.graveyard.remove(graveyard_position);
    let mut candidate_pool = mana_pool.clone();
    if !pay_exact_printed_cost_with_context(
        &mut candidate_pool,
        deck,
        &candidate_zones,
        mana_access,
        card_index,
        additional_generic_per_cast,
    ) || !exile_graveyard_fodder(&mut candidate_zones, program)
    {
        return false;
    }
    *spells_cast_this_turn = spells_cast_this_turn.saturating_add(1);
    apply_controller_spell_cast_triggers(
        deck,
        &mut candidate_zones,
        card_index,
        turn,
        *spells_cast_this_turn,
        &mut candidate_pool,
        player_life,
    );
    candidate_zones.record_cast(deck, card_index, turn);
    *zones = candidate_zones;
    *mana_pool = candidate_pool;
    true
}

fn pay_exact_printed_cost(
    mana_pool: &mut TurnManaPool,
    mana_access: &ManaAccessProfile,
    card_index: usize,
    additional_generic_per_cast: u8,
) -> bool {
    let Some(cost) = mana_access.cost(card_index) else {
        return false;
    };
    activation_cost_is_exactly_modeled(cost)
        && mana_pool.pay_with_additional_generic(Some(cost), 0, additional_generic_per_cast, 0)
}

fn pay_exact_printed_cost_with_context(
    mana_pool: &mut TurnManaPool,
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    mana_access: &ManaAccessProfile,
    card_index: usize,
    additional_generic_per_cast: u8,
) -> bool {
    let Some(cost) = mana_access.cost(card_index) else {
        return false;
    };
    activation_cost_is_exactly_modeled(cost)
        && mana_pool.pay_with_generic_adjustment(
            Some(cost),
            0,
            additional_generic_per_cast,
            generic_spell_cost_reduction(deck, zones, card_index),
            0,
        )
}

fn exile_graveyard_fodder(zones: &mut KnownLineZoneState, program: GraveyardStormProgram) -> bool {
    let mut candidate = zones.clone();
    for _ in 0..program.exile_count {
        let Some(position) = candidate.graveyard.iter().position(|card_index| {
            *card_index != program.permission_source
                && *card_index != program.mana_source
                && *card_index != program.mill_spell
        }) else {
            return false;
        };
        let exiled = candidate.graveyard.remove(position);
        candidate.exile.push(exiled);
    }
    *zones = candidate;
    true
}

fn graveyard_storm_fodder_count(
    zones: &KnownLineZoneState,
    program: GraveyardStormProgram,
) -> usize {
    zones
        .graveyard
        .iter()
        .filter(|card_index| {
            **card_index != program.permission_source
                && **card_index != program.mana_source
                && **card_index != program.mill_spell
        })
        .count()
}

fn battlefield_contains(zones: &KnownLineZoneState, card_index: usize) -> bool {
    zones
        .battlefield
        .iter()
        .any(|presence| presence.card_index == card_index)
}

fn deliver_due_delayed_card_access(
    hand: &mut Vec<usize>,
    pending: &mut Vec<PendingDelayedCardAccess>,
    turn: u8,
) -> usize {
    let mut retained = Vec::with_capacity(pending.len());
    let mut delivered = 0usize;
    for delayed in pending.drain(..) {
        if delayed.due_turn <= turn {
            // Each queue entry is the exact physical face-down object moved by
            // one activation. Equal compiled card indices remain distinct
            // entries, so an older unrelated exiled copy can never be taken.
            hand.push(delayed.card_index);
            delivered = delivered.saturating_add(1);
        } else {
            retained.push(delayed);
        }
    }
    *pending = retained;
    delivered
}

fn cleanup_card_preservation_score(card: &CompiledCard) -> i64 {
    i64::from(card.has(role::PROTECTION | role::COUNTERSPELL)) * 100
        + i64::from(card_has_executable_planner_tutor_role(card)) * 85
        + i64::from(card.has(role::FAST_MANA) && card_has_executable_planner_mana_role(card)) * 75
        + i64::from(card_has_executable_draw_access(card)) * 65
        + i64::from(card.has(role::COMBO_PIECE | role::WIN_CONDITION)) * 60
        + i64::from(card.has(role::ENGINE | role::ENABLER | role::PAYOFF)) * 45
}

#[allow(clippy::too_many_arguments)]
fn discard_to_maximum_hand_size(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &mut Vec<usize>,
    actual_unseen_library: &[usize],
    zones: &mut KnownLineZoneState,
    mana_sources: &[BattlefieldManaSource],
    turn: u8,
    retained_treasures: u8,
) -> Vec<usize> {
    if hand.len() <= MAXIMUM_CLEANUP_HAND_SIZE {
        return Vec::new();
    }
    let actual_remaining_library = exact_card_multiset(actual_unseen_library);
    let (projected_zones, projected_pool) =
        projected_next_turn_pool(deck, zones, mana_sources, turn, retained_treasures);
    let projected_turn = turn.saturating_add(1);
    let mut discarded = Vec::with_capacity(hand.len() - MAXIMUM_CLEANUP_HAND_SIZE);
    while hand.len() > MAXIMUM_CLEANUP_HAND_SIZE {
        let mut best: Option<(usize, DelayedAccessPublicHandQuality, i64, f32, usize)> = None;
        for position in 0..hand.len() {
            let mut candidate_hand = hand.clone();
            let removed_index = candidate_hand.remove(position);
            let quality = delayed_access_public_hand_quality(
                deck,
                mana_access,
                &candidate_hand,
                &actual_remaining_library,
                &projected_zones,
                projected_turn,
                &projected_pool,
            );
            let card = &deck.cards[removed_index];
            let preservation = cleanup_card_preservation_score(card);
            let mana_value = card.mana_value;
            let replace = best.as_ref().is_none_or(
                |(_, best_quality, best_preservation, best_mana_value, best_index)| {
                    quality > *best_quality
                        || quality == *best_quality
                            && (preservation < *best_preservation
                                || preservation == *best_preservation
                                    && (mana_value.total_cmp(best_mana_value).is_gt()
                                        || mana_value.total_cmp(best_mana_value).is_eq()
                                            && removed_index < *best_index))
                },
            );
            if replace {
                best = Some((position, quality, preservation, mana_value, removed_index));
            }
        }
        let (position, _, _, _, _) =
            best.expect("a nonempty oversized hand always has a cleanup candidate");
        discarded.push(hand.remove(position));
    }
    zones.record_discards(deck, discarded.iter().copied());
    discarded
}

fn resolve_beginning_of_end_step_self_sacrifices(
    deck: &CompiledDeck,
    zones: &mut KnownLineZoneState,
    mana_sources: &mut Vec<BattlefieldManaSource>,
) -> Vec<usize> {
    let sacrifices = zones
        .battlefield
        .iter()
        .filter_map(|presence| {
            deck.cards
                .get(presence.card_index)
                .is_some_and(|card| {
                    card.ability_program.executable_abilities().any(|ability| {
                        matches!(
                            &ability.timing,
                            AbilityTiming::Triggered { event }
                                if event.kind == TriggerEventKind::BeginningOfEndStep
                        ) && ability.costs.is_empty()
                            && ability.effects == [AbilityEffect::SacrificeSelf]
                    })
                })
                .then_some(presence.sequence)
        })
        .collect::<Vec<_>>();
    let mut removed = Vec::new();
    for sequence in sacrifices {
        if let Some(card_indices) =
            zones.remove_permanent_sequence_with_attached_auras(deck, sequence, true)
        {
            removed.extend(card_indices);
        }
    }
    synchronize_mana_sources_with_battlefield(mana_sources, zones);
    removed
}

fn line_has_executable_artifact_tap_treasure_cycle(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
) -> bool {
    let members = executable_line_members(line, deck);
    if members.len() != line.cards.len() {
        return false;
    }
    let has_dwarf_tap_treasure_trigger = members.iter().any(|card| {
        card.ability_program.executable_abilities().any(|ability| {
            matches!(
                &ability.timing,
                AbilityTiming::Triggered { event }
                    if event.kind == TriggerEventKind::PermanentBecomesTapped
                        && event.object_filter.subtype.as_deref() == Some("Dwarf")
            ) && ability.effects.iter().any(|effect| {
                matches!(
                    effect,
                    AbilityEffect::CreateToken(token)
                        if token.kind == TokenKind::Treasure && token.count >= 1
                )
            })
        })
    });
    let has_artifact_tap_untap_engine = members.iter().any(|card| {
        card.ability_program.executable_abilities().any(|ability| {
            matches!(ability.timing, AbilityTiming::Activated { .. })
                && ability.costs.iter().any(|cost| {
                    matches!(
                        cost,
                        AbilityCost::TapPermanents {
                            filter,
                            count,
                            exclude_source: false,
                        } if *count == 2
                            && filter.card_type == Some(ProgramCardType::Artifact)
                    )
                })
                && ability.effects.iter().any(|effect| {
                    matches!(
                        effect,
                        AbilityEffect::Untap(TargetSelector::Target(filter))
                            if filter.card_type == Some(ProgramCardType::Artifact)
                    )
                })
        })
    });
    let has_artifact_dwarf = members
        .iter()
        .any(|card| card.effects.card_types.is_artifact && card_has_subtype(card, "Dwarf"));
    has_dwarf_tap_treasure_trigger && has_artifact_tap_untap_engine && has_artifact_dwarf
}

fn line_has_executable_maskwood_artifact_dwarf_treasure_cycle(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
) -> bool {
    let members = executable_line_members(line, deck);
    if members.len() != line.cards.len() {
        return false;
    }
    let has_dwarf_tap_treasure_trigger = members.iter().any(|card| {
        card.ability_program.executable_abilities().any(|ability| {
            matches!(
                &ability.timing,
                AbilityTiming::Triggered { event }
                    if event.kind == TriggerEventKind::PermanentBecomesTapped
                        && event.actor == ControllerRelation::You
                        && event.object_filter.subtype.as_deref() == Some("Dwarf")
                        && event.object_filter.controller == Some(ControllerRelation::You)
            ) && ability.effects.iter().any(|effect| {
                matches!(
                    effect,
                    AbilityEffect::CreateToken(token)
                        if token.kind == TokenKind::Treasure && token.count >= 1
                )
            })
        })
    });
    let has_artifact_entry_optional_self_untap = members.iter().any(|card| {
        card.effects.card_types.is_artifact
            && card.effects.card_types.is_creature
            && card.ability_program.executable_abilities().any(|ability| {
                matches!(
                    &ability.timing,
                    AbilityTiming::Triggered { event }
                        if event.kind == TriggerEventKind::PermanentEntersBattlefield
                            && event.actor == ControllerRelation::Any
                            && event.object_filter.card_type
                                == Some(ProgramCardType::Artifact)
                            && event.object_filter.controller.is_none()
                ) && ability.effects
                    == [AbilityEffect::OptionalUntap(TargetSelector::SelfPermanent)]
            })
    });
    let has_tap_untapped_dwarf_activation = members.iter().any(|card| {
        card.ability_program.executable_abilities().any(|ability| {
            matches!(ability.timing, AbilityTiming::Activated { .. })
                && ability.costs.iter().any(|cost| {
                    matches!(
                        cost,
                        AbilityCost::TapPermanents {
                            filter,
                            count,
                            exclude_source: false,
                        } if *count == 1
                            && filter.subtype.as_deref() == Some("Dwarf")
                            && filter.controller == Some(ControllerRelation::You)
                    )
                })
                && ability.effects.iter().any(|effect| {
                    matches!(
                        effect,
                        AbilityEffect::ModifyPowerToughnessUntilEndOfTurn(modifier)
                            if modifier.power_delta == 2
                                && modifier.toughness_delta == 0
                                && matches!(
                                    &modifier.target,
                                    TargetSelector::Target(filter)
                                        if filter.card_type
                                            == Some(ProgramCardType::Creature)
                                )
                    )
                })
        })
    });
    let has_exact_all_creature_types_static = members.iter().any(|card| {
        card.effects.card_types.is_artifact
            && card.ability_program.executable_abilities().any(|ability| {
                ability.timing == AbilityTiming::StaticModifier
                    && ability.effects.iter().any(|effect| {
                        matches!(
                            effect,
                            AbilityEffect::GrantAllCreatureTypes(scope)
                                if scope.creatures_you_control
                                    && scope.creature_spells_you_control
                                    && scope.nonbattlefield_creature_cards_you_own
                        )
                    })
            })
    });

    has_dwarf_tap_treasure_trigger
        && has_artifact_entry_optional_self_untap
        && has_tap_untapped_dwarf_activation
        && has_exact_all_creature_types_static
}

fn executable_line_members<'a>(
    line: &crate::domain::KnownLine,
    deck: &'a CompiledDeck,
) -> Vec<&'a CompiledCard> {
    line.cards
        .iter()
        .filter_map(|name| {
            let normalized = crate::parser::normalize_card_name(name);
            unique_card_by_normalized_name(deck, &normalized)
        })
        .collect()
}

fn line_requirements_met(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    creature_count: u8,
    mana_sources: &[BattlefieldManaSource],
    turn: u8,
) -> bool {
    let line_names = line
        .cards
        .iter()
        .map(|name| crate::parser::normalize_card_name(name))
        .collect::<HashSet<_>>();
    line.simulation_requirements.iter().all(|requirement| {
        match requirement {
            LineRequirement::AdditionalCreature { count } => {
                let line_creatures = line
                    .cards
                    .iter()
                    .filter(|name| {
                        let normalized = crate::parser::normalize_card_name(name);
                        unique_card_by_normalized_name(deck, &normalized)
                            .is_some_and(|card| card.has(role::CREATURE))
                    })
                    .fold(0u8, |total, _| total.saturating_add(1));
                creature_count >= line_creatures.saturating_add(*count)
            }
            LineRequirement::NonlandManaCapacity { minimum } => {
                mana_sources
                    .iter()
                    .filter(|source| !source.is_land && source.available_from_turn <= turn)
                    .fold(0u8, |total, source| total.saturating_add(source.capacity))
                    >= *minimum
            }
            // The explicit additional cost is validated and committed exactly
            // once by `select_ready_known_line`.
            LineRequirement::AdditionalActivationMana { .. } => true,
            // A total execution value cannot be subtracted from already-paid
            // casts without a full staged sequence.
            LineRequirement::TotalExecutionMana => false,
            LineRequirement::NamedCardsPayPrintedCosts
            | LineRequirement::ReviewedEmptyLibrarySequence => true,
            LineRequirement::ReviewedInfiniteManaLoop => {
                line_has_executable_infinite_mana_cycle(line, deck)
            }
            // A structural graveyard/escape/Storm witness is not action
            // authority. Only the atomic transaction executor may promote
            // this requirement to a threat or win attempt.
            LineRequirement::ExecutableGraveyardStormLoop => false,
            LineRequirement::ExecutableArtifactTapTreasureLoop => {
                line_has_executable_artifact_tap_treasure_cycle(line, deck)
            }
            LineRequirement::ExecutableMaskwoodArtifactDwarfTreasureLoop => {
                line_has_executable_maskwood_artifact_dwarf_treasure_cycle(line, deck)
            }
            LineRequirement::ExecutableInfiniteManaCreatureOverrunAttempt => {
                line_has_executable_infinite_mana_creature_overrun_attempt(line, deck, zones, turn)
            }
            LineRequirement::ExternalEnabler => deck.cards.iter().any(|card| {
                !line_names.contains(&card.normalized_name)
                    && card.has(role::ENABLER | role::PAYOFF)
                    && zones.usable_count(deck, &card.normalized_name, turn) > 0
            }),
            LineRequirement::SingletonLibrary => deck.cards.iter().all(|card| card.quantity <= 1),
            LineRequirement::GraveyardSetup { minimum_cast_cards } => {
                // Require setup to exist at the start of the turn. Line spells
                // cast during this turn cannot bootstrap their own escape/
                // graveyard prerequisite after the fact.
                zones.graveyard_cards_at_turn_start >= u16::from(*minimum_cast_cards)
            }
            // There is no opponent battlefield, blocker, life-total, haste,
            // or combat-damage model. Therefore combat access cannot be
            // established and combat-dependent catalog lines receive no
            // simulated readiness credit.
            LineRequirement::CombatAccess => false,
            LineRequirement::Unmodeled => false,
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn best_land_position(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    library_order: &[usize],
    next_draw_position: usize,
    turn: u8,
    mana_pool: &TurnManaPool,
    zones: &KnownLineZoneState,
) -> Option<usize> {
    hand.iter()
        .enumerate()
        .filter(|(_, index)| deck.cards[**index].has(role::LAND))
        .max_by(|(_, left), (_, right)| {
            let rank = |card_index| {
                let source = contextual_land_source(
                    deck,
                    mana_access,
                    hand,
                    library_order,
                    next_draw_position,
                    card_index,
                    turn,
                    mana_pool,
                    zones,
                );
                let route_rank = source.map_or(0, |source| {
                    reviewed_sequence_payable_count_with_land_source(
                        deck,
                        mana_access,
                        hand,
                        turn,
                        mana_pool,
                        zones,
                        source,
                    )
                });
                let immediate = source.map_or(0.0, |source| {
                    immediate_hand_cast_score_for_source(
                        deck,
                        mana_access,
                        hand,
                        card_index,
                        turn,
                        mana_pool,
                        zones,
                        source,
                    )
                });
                (
                    route_rank,
                    u8::from(source.is_some_and(|source| {
                        battlefield_source_is_trajectory_available(&source, turn)
                    })),
                    land_play_score_for_card(deck, mana_access, card_index) + immediate,
                )
            };
            let left_rank = rank(**left);
            let right_rank = rank(**right);
            left_rank
                .0
                .cmp(&right_rank.0)
                .then_with(|| left_rank.1.cmp(&right_rank.1))
                .then_with(|| left_rank.2.total_cmp(&right_rank.2))
        })
        .map(|(position, _)| position)
}

#[allow(clippy::too_many_arguments)]
fn contextual_land_source(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    library_order: &[usize],
    next_draw_position: usize,
    land_index: usize,
    turn: u8,
    mana_pool: &TurnManaPool,
    zones: &KnownLineZoneState,
) -> Option<BattlefieldManaSource> {
    if let Some(descriptor) = deck.cards.get(land_index).and_then(reviewed_fetchland) {
        let target_position = reviewed_fetchland_target_position(
            deck,
            mana_access,
            descriptor,
            hand,
            library_order,
            next_draw_position,
            turn,
            mana_pool,
            zones,
        )?;
        let target_index = *library_order.get(target_position)?;
        return Some(battlefield_land_source(
            deck,
            mana_access,
            target_index,
            turn,
        ));
    }
    Some(battlefield_land_source(deck, mana_access, land_index, turn))
}

fn reviewed_sequence_payable_count_with_land_source(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    turn: u8,
    mana_pool: &TurnManaPool,
    zones: &KnownLineZoneState,
    source: BattlefieldManaSource,
) -> usize {
    let Some(candidate_pool) = turn_pool_with_land_source(deck, zones, turn, mana_pool, source)
    else {
        return 0;
    };
    deck.known_lines
        .iter()
        .filter(|line| reviewed_empty_library_sequence(line))
        .filter(|line| {
            reviewed_sequence_package_is_jointly_payable(
                line,
                deck,
                hand,
                zones,
                turn,
                &candidate_pool,
                mana_access,
                0,
            )
        })
        .count()
}

fn turn_pool_with_land_source(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: &TurnManaPool,
    source: BattlefieldManaSource,
) -> Option<TurnManaPool> {
    if !battlefield_source_is_trajectory_available(&source, turn) {
        return None;
    }
    let mut candidate = mana_pool.clone();
    add_battlefield_source_to_current_pool(source, deck, zones, turn, &mut candidate);
    Some(candidate)
}

#[allow(clippy::too_many_arguments)]
fn immediate_hand_cast_score_for_source(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    land_index: usize,
    turn: u8,
    mana_pool: &TurnManaPool,
    zones: &KnownLineZoneState,
    source: BattlefieldManaSource,
) -> f32 {
    let Some(pool) = turn_pool_with_land_source(deck, zones, turn, mana_pool, source) else {
        return 0.0;
    };
    hand.iter()
        .copied()
        .filter(|card_index| *card_index != land_index)
        .filter_map(|card_index| {
            let card = deck.cards.get(card_index)?;
            if card.has(role::LAND) || card.mana_value <= 0.0 || should_hold_reactive_card(card) {
                return None;
            }
            let mut candidate = pool.clone();
            candidate
                .pay(
                    mana_access.and_then(|access| access.cost(card_index)),
                    card.mana_value.ceil().max(0.0) as u8,
                    0,
                )
                .then_some(
                    20.0 + if card.has(role::WIN_CONDITION | role::COMBO_PIECE) {
                        8.0
                    } else if card.has(role::TUTOR | role::ENGINE | role::ENABLER) {
                        5.0
                    } else {
                        1.0
                    },
                )
        })
        .fold(0.0, f32::max)
}

fn should_defer_only_land_to_diamond(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    land_position: usize,
    turn: u8,
    mana_pool: &TurnManaPool,
    zones: &KnownLineZoneState,
) -> bool {
    let lands = hand
        .iter()
        .copied()
        .filter(|card_index| {
            deck.cards
                .get(*card_index)
                .is_some_and(|card| card.has(role::LAND))
        })
        .collect::<Vec<_>>();
    if lands.len() != 1 || hand.get(land_position).copied() != lands.first().copied() {
        return false;
    }
    let has_diamond = hand.iter().copied().any(|card_index| {
        deck.cards.get(card_index).is_some_and(|card| {
            compile_typed_conditional_mana_source(card)
                == Some(TypedConditionalManaSource::DiscardLandOrFailEntry)
        })
    });
    if !has_diamond {
        return false;
    }
    let land_index = lands[0];
    let land_source = battlefield_land_source(deck, mana_access, land_index, turn);
    if land_source.available_from_turn > turn {
        return true;
    }

    let land_fit = immediate_hand_cast_score_for_source(
        deck,
        mana_access,
        hand,
        land_index,
        turn,
        mana_pool,
        zones,
        land_source,
    );
    let any_color_fit = hand
        .iter()
        .copied()
        .filter(|card_index| *card_index != land_index)
        .filter_map(|card_index| {
            let card = deck.cards.get(card_index)?;
            if card.has(role::LAND)
                || card.mana_value <= 0.0
                || compile_typed_conditional_mana_source(card)
                    == Some(TypedConditionalManaSource::DiscardLandOrFailEntry)
                || should_hold_reactive_card(card)
            {
                return None;
            }
            let mut candidate = mana_pool.clone();
            candidate.add_floating(ManaColorMask::ANY_COLOR, 1);
            candidate
                .pay(
                    mana_access.and_then(|access| access.cost(card_index)),
                    card.mana_value.ceil().max(0.0) as u8,
                    0,
                )
                .then_some(21.0)
        })
        .fold(0.0, f32::max);
    any_color_fit > land_fit
}

fn queued_plan_reserves_deferred_land_for_diamond(
    deck: &CompiledDeck,
    planned_actions: &VecDeque<TurnAction>,
) -> bool {
    planned_actions.iter().copied().any(|action| {
        let TurnAction::Cast(card_index) = action else {
            return false;
        };
        deck.cards.get(card_index).is_some_and(|card| {
            compile_typed_conditional_mana_source(card)
                == Some(TypedConditionalManaSource::DiscardLandOrFailEntry)
        })
    })
}

fn land_play_score_for_card(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    card_index: usize,
) -> f32 {
    let base = land_play_score(mana_access.and_then(|access| access.source(card_index)));
    let Some(card) = deck.cards.get(card_index) else {
        return base;
    };
    match compile_typed_conditional_mana_source(card) {
        Some(TypedConditionalManaSource::FixedWithSourceDamage { output, damage }) => {
            let additional_capacity = fixed_mana_profile_output(output)
                .map(|(_, capacity)| capacity.saturating_sub(1))
                .unwrap_or_default();
            base + f32::from(additional_capacity) * 1.75 - f32::from(damage) * 0.05
        }
        Some(TypedConditionalManaSource::ColorlessOrAnyColorWithSourceDamage { damage }) => {
            // Do not prefer the damaging colored mode merely because the
            // source also advertises a sixth, damage-free colorless option.
            // Exact immediate/route payment still promotes this land when
            // the real cost contains {C}.
            base - f32::from(damage) * 0.05
        }
        _ => base,
    }
}

fn land_play_score(source: Option<&ManaSourceProfile>) -> f32 {
    let Some(source) = source else {
        return 0.0;
    };
    let untapped_bonus = match source.enters_tapped {
        EntersTapped::UntappedByDefault | EntersTapped::NotApplicable => 2.0,
        EntersTapped::Conditional => 0.8,
        EntersTapped::Unknown => 0.4,
        EntersTapped::Always => 0.0,
    };
    untapped_bonus + source.reliability + source_option_count(source.colors) as f32 * 0.08
}

fn fixed_mana_profile_output(output: FixedManaProfile) -> Option<(ManaColorMask, u8)> {
    let components = [
        (ManaColorMask::WHITE, output.white),
        (ManaColorMask::BLUE, output.blue),
        (ManaColorMask::BLACK, output.black),
        (ManaColorMask::RED, output.red),
        (ManaColorMask::GREEN, output.green),
        (ManaColorMask::COLORLESS, output.colorless),
    ];
    let mut colors = ManaColorMask::NONE;
    let mut total = 0u16;
    for (color, amount) in components {
        if amount > 0 {
            colors |= color;
        }
        total = total.checked_add(amount)?;
    }
    (total > 0 && total <= u16::from(u8::MAX)).then_some((colors, total as u8))
}

fn typed_battlefield_mana_source(
    deck: &CompiledDeck,
    card_index: usize,
    kind: TypedConditionalManaSource,
    linked_colors: Option<ManaColorMask>,
    turn: u8,
) -> Option<BattlefieldManaSource> {
    let card = deck.cards.get(card_index)?;
    let (
        colors,
        capacity,
        behavior,
        source_damage_on_first_spend,
        damage_free_colors_on_first_spend,
    ) = match kind {
        TypedConditionalManaSource::ImprintLinkedCardColors => {
            let colors = linked_colors.unwrap_or(ManaColorMask::NONE);
            (
                colors,
                1,
                BattlefieldManaBehavior::LinkedCardColors(colors),
                0,
                ManaColorMask::NONE,
            )
        }
        TypedConditionalManaSource::DiscardLandOrFailEntry => (
            ManaColorMask::ANY_COLOR,
            1,
            BattlefieldManaBehavior::Fixed,
            0,
            ManaColorMask::NONE,
        ),
        TypedConditionalManaSource::ControlledLegendaryColors => (
            ManaColorMask::NONE,
            1,
            BattlefieldManaBehavior::AnyColorAmongControlledLegendaryCreaturesAndPlaneswalkers,
            0,
            ManaColorMask::NONE,
        ),
        TypedConditionalManaSource::MetalcraftAnyColor => (
            ManaColorMask::NONE,
            1,
            BattlefieldManaBehavior::AnyColorWithMetalcraft,
            0,
            ManaColorMask::NONE,
        ),
        TypedConditionalManaSource::FixedWithSourceDamage { output, damage } => {
            let (colors, capacity) = fixed_mana_profile_output(output)?;
            (
                colors,
                capacity,
                BattlefieldManaBehavior::Fixed,
                u8::try_from(damage).ok()?,
                ManaColorMask::NONE,
            )
        }
        TypedConditionalManaSource::ColorlessOrAnyColorWithSourceDamage { damage } => (
            ManaColorMask::ANY_COLOR | ManaColorMask::COLORLESS,
            1,
            BattlefieldManaBehavior::Fixed,
            u8::try_from(damage).ok()?,
            ManaColorMask::COLORLESS,
        ),
    };
    let is_land = card.effects.card_types.is_land;
    Some(BattlefieldManaSource {
        colors,
        capacity,
        reliability: 1.0,
        available_from_turn: if card.effects.card_types.is_creature {
            turn.saturating_add(1)
        } else {
            turn
        },
        is_land,
        card_index: Some(card_index),
        behavior,
        source_damage_on_first_spend,
        damage_free_colors_on_first_spend,
    })
}

fn add_typed_source_to_current_pool(
    source: BattlefieldManaSource,
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    pool: &mut TurnManaPool,
) {
    add_battlefield_source_to_current_pool(source, deck, zones, turn, pool);
}

fn add_battlefield_source_to_current_pool(
    source: BattlefieldManaSource,
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    turn: u8,
    pool: &mut TurnManaPool,
) {
    if turn < source.available_from_turn {
        return;
    }
    let origin_sequence = source.card_index.and_then(|card_index| {
        zones
            .battlefield
            .iter()
            .rev()
            .find(|presence| {
                presence.card_index == card_index
                    && !pool
                        .sources
                        .iter()
                        .any(|existing| existing.origin_sequence == Some(presence.sequence))
            })
            .map(|presence| presence.sequence)
    });
    let is_dwarf = origin_sequence.is_some_and(|sequence| {
        creature_object_has_effective_subtype(deck, zones, sequence, source.card_index, "Dwarf")
    });
    let context = active_ability_context(deck, zones);
    pool.sources.push(PoolManaSource {
        colors: source.colors,
        remaining: 0,
        is_treasure: false,
        treasure_on_first_spend: if is_dwarf {
            context.dwarf_treasure_per_tap
        } else {
            0
        },
        first_spend_recorded: false,
        origin_card_index: source.card_index,
        origin_sequence,
        physically_tapped: false,
        behavior: source.behavior,
        base_capacity: source.capacity,
        is_land: source.is_land,
        activation_used: false,
        source_damage_on_first_spend: source.source_damage_on_first_spend,
        damage_free_colors_on_first_spend: source.damage_free_colors_on_first_spend,
        same_type_coupled: false,
        sacrifice_on_first_spend: None,
    });
    pool.refresh_battlefield_sources(deck, zones, context);
}

fn reviewed_fetchland(card: &CompiledCard) -> Option<ReviewedFetchland> {
    if !card.effects.card_types.is_land
        || card.ability_program.abilities.len() != 1
        || card.ability_program.entry_linked_permanent.is_some()
        || card.ability_program.atomic_transaction.is_some()
        || !card.ability_program.face_programs.is_empty()
    {
        return None;
    }

    // The reviewed lifecycle is the exact allied/enemy fetchland activation:
    // tapping, paying one life, and sacrificing the source are all mandatory.
    // Omitting the `{T}, ` prefix previously made real Scryfall Oracle text
    // fail recognition because the activation cost was incomplete.
    const PREFIX: &str = "{t}, pay 1 life, sacrifice this permanent: search your library for ";
    const SUFFIX: &str = " card, put it onto the battlefield, then shuffle";
    let mut descriptor = None;
    for ability in &card.ability_program.abilities {
        let normalized_oracle = match ability {
            AbilityCompilation::Executable(ability) => ability.normalized_oracle.as_str(),
            AbilityCompilation::Unsupported(ability) => ability.normalized_oracle.as_str(),
        };
        let lower = normalized_oracle
            .trim()
            .trim_end_matches('.')
            .to_ascii_lowercase();
        let Some(subtypes_with_article) = lower
            .strip_prefix(PREFIX)
            .and_then(|text| text.strip_suffix(SUFFIX))
        else {
            continue;
        };
        let Some(subtypes) = subtypes_with_article
            .strip_prefix("a ")
            .or_else(|| subtypes_with_article.strip_prefix("an "))
        else {
            continue;
        };
        let Some((first, second)) = subtypes.split_once(" or ") else {
            continue;
        };
        let candidate = ReviewedFetchland {
            first_subtype: BasicLandSubtype::parse(first)?,
            second_subtype: BasicLandSubtype::parse(second)?,
        };
        if candidate.first_subtype == candidate.second_subtype
            || descriptor.replace(candidate).is_some()
        {
            return None;
        }
    }
    descriptor
}

fn library_card_has_basic_land_subtype(card: &CompiledCard, subtype: BasicLandSubtype) -> bool {
    let hand_characteristics = card.hand_zone_characteristics();
    if hand_characteristics.exact {
        return hand_characteristics.card_types.is_land
            && type_line_has_subtype(&hand_characteristics.type_line, subtype.type_name());
    }
    if card_has_visible_multiface_identity(card) {
        return false;
    }
    card.effects.card_types.is_land && type_line_has_subtype(&card.type_line, subtype.type_name())
}

#[allow(clippy::too_many_arguments)]
fn reviewed_fetchland_target_position(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    descriptor: ReviewedFetchland,
    hand: &[usize],
    library_order: &[usize],
    next_draw_position: usize,
    turn: u8,
    mana_pool: &TurnManaPool,
    zones: &KnownLineZoneState,
) -> Option<usize> {
    library_order
        .iter()
        .copied()
        .enumerate()
        .skip(next_draw_position)
        .filter(|(_, card_index)| {
            deck.cards.get(*card_index).is_some_and(|candidate| {
                library_card_has_basic_land_subtype(candidate, descriptor.first_subtype)
                    || library_card_has_basic_land_subtype(candidate, descriptor.second_subtype)
            })
        })
        .max_by(
            |(left_position, left_index), (right_position, right_index)| {
                let rank = |card_index| {
                    let source = battlefield_land_source(deck, mana_access, card_index, turn);
                    let route_rank = reviewed_sequence_payable_count_with_land_source(
                        deck,
                        mana_access,
                        hand,
                        turn,
                        mana_pool,
                        zones,
                        source,
                    );
                    let immediate = immediate_hand_cast_score_for_source(
                        deck,
                        mana_access,
                        hand,
                        card_index,
                        turn,
                        mana_pool,
                        zones,
                        source,
                    );
                    let current_turn = if source.available_from_turn <= turn {
                        100.0
                    } else {
                        0.0
                    };
                    (
                        route_rank,
                        current_turn
                            + immediate
                            + land_play_score_for_card(deck, mana_access, card_index)
                            + f32::from(source_option_count(source.colors)) * 0.1,
                    )
                };
                let left_rank = rank(*left_index);
                let right_rank = rank(*right_index);
                left_rank
                    .0
                    .cmp(&right_rank.0)
                    .then_with(|| left_rank.1.total_cmp(&right_rank.1))
                    // For equal targets, the earliest physical library object is
                    // selected before the unseen suffix is randomized.
                    .then_with(|| right_position.cmp(left_position))
            },
        )
        .map(|(position, _)| position)
}

#[allow(clippy::too_many_arguments)]
fn resolve_reviewed_fetchland_activation(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    descriptor: ReviewedFetchland,
    fetch_sequence: u16,
    turn: u8,
    hand: &[usize],
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
    mana_sources: &mut Vec<BattlefieldManaSource>,
    mana_pool: &mut TurnManaPool,
    player_life: &mut f32,
) -> ReviewedFetchlandResolution {
    if *player_life <= 0.0 {
        return ReviewedFetchlandResolution::default();
    }

    let Some(target_position) = reviewed_fetchland_target_position(
        deck,
        mana_access,
        descriptor,
        hand,
        library_order,
        next_draw_position,
        turn,
        mana_pool,
        zones,
    ) else {
        // Activating is optional. With no legal target and no modeled shuffle
        // payoff, sacrificing the land and paying life is strictly worse than
        // retaining the exact battlefield object for later turns.
        return ReviewedFetchlandResolution::default();
    };
    if zones
        .remove_permanent_sequence(deck, fetch_sequence, true)
        .is_none()
    {
        return ReviewedFetchlandResolution::default();
    }

    *player_life -= 1.0;
    let searched_target = Some(library_order.remove(target_position));
    shuffle_unseen_library(library_order, next_draw_position, rng);

    if let Some(target_index) = searched_target {
        zones.record_put_onto_battlefield(deck, target_index, turn);
        let source = battlefield_land_source(deck, mana_access, target_index, turn);
        mana_sources.push(source);
        if battlefield_source_is_trajectory_available(&source, turn) {
            add_battlefield_source_to_current_pool(source, deck, zones, turn, mana_pool);
        }
    }

    ReviewedFetchlandResolution {
        searched_target,
        player_died: *player_life <= 0.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_land_play(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    land_index: usize,
    turn: u8,
    hand: &[usize],
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
    mana_sources: &mut Vec<BattlefieldManaSource>,
    mana_pool: &mut TurnManaPool,
    player_life: &mut f32,
) -> ReviewedFetchlandResolution {
    zones.record_land_play(deck, land_index, turn);
    if let Some(descriptor) = deck.cards.get(land_index).and_then(reviewed_fetchland)
        && let Some(fetch_sequence) = zones
            .battlefield
            .last()
            .filter(|presence| presence.card_index == land_index)
            .map(|presence| presence.sequence)
    {
        return resolve_reviewed_fetchland_activation(
            deck,
            mana_access,
            descriptor,
            fetch_sequence,
            turn,
            hand,
            library_order,
            next_draw_position,
            rng,
            zones,
            mana_sources,
            mana_pool,
            player_life,
        );
    }

    let source = battlefield_land_source(deck, mana_access, land_index, turn);
    mana_sources.push(source);
    if battlefield_source_is_trajectory_available(&source, turn) {
        add_battlefield_source_to_current_pool(source, deck, zones, turn, mana_pool);
    }
    ReviewedFetchlandResolution {
        searched_target: None,
        player_died: false,
    }
}

fn battlefield_land_source(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    card_index: usize,
    turn: u8,
) -> BattlefieldManaSource {
    if let Some(kind) = deck
        .cards
        .get(card_index)
        .and_then(compile_typed_conditional_mana_source)
        && matches!(
            kind,
            TypedConditionalManaSource::FixedWithSourceDamage { .. }
                | TypedConditionalManaSource::ColorlessOrAnyColorWithSourceDamage { .. }
        )
        && let Some(source) = typed_battlefield_mana_source(deck, card_index, kind, None, turn)
    {
        return source;
    }
    let source = mana_access.and_then(|access| access.source(card_index));
    let colors = source
        .map(|source| source.colors)
        .unwrap_or(ManaColorMask::NONE);
    let reliability = if source.is_some_and(mana_source_profile_is_exact_for_trajectory) {
        1.0
    } else {
        0.0
    };
    let available_from_turn = match source.map(|source| source.enters_tapped) {
        Some(EntersTapped::Always | EntersTapped::Conditional | EntersTapped::Unknown) => {
            turn.saturating_add(1)
        }
        _ => turn,
    };
    BattlefieldManaSource {
        colors,
        capacity: 1,
        reliability,
        available_from_turn,
        is_land: true,
        card_index: Some(card_index),
        ..BattlefieldManaSource::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_cast_mana_effects(
    deck: &CompiledDeck,
    card_index: usize,
    card: &CompiledCard,
    mana_access: Option<&ManaAccessProfile>,
    turn: u8,
    battlefield: &mut Vec<BattlefieldManaSource>,
    pool: &mut TurnManaPool,
    _rng: &mut ChaCha8Rng,
    ability_context: ActiveAbilityContext,
    zones: &KnownLineZoneState,
) -> bool {
    if exact_discard_sacrifice_mana_ability(card).is_some() {
        // This source remains a battlefield object until its typed queued
        // action pays the entire-hand discard and self-sacrifice atomically.
        return false;
    }
    if let Some(kind) = compile_typed_conditional_mana_source(card)
        && !kind.is_entry_linked()
        && !card.effects.card_types.is_land
        && let Some(source) = typed_battlefield_mana_source(deck, card_index, kind, None, turn)
    {
        battlefield.push(source);
        add_typed_source_to_current_pool(source, deck, zones, turn, pool);
        return false;
    }
    let source_profile = mana_access.and_then(|access| access.source(card_index));
    let source_colors = source_profile
        .map(|source| source.colors)
        .unwrap_or_else(|| {
            if card.effects.mana_production_kind == ManaProductionKind::NonRefreshingActivated {
                ManaColorMask::COLORLESS
            } else {
                ManaColorMask::NONE
            }
        });
    let source_reliability = if card.effects.mana_production_kind
        == ManaProductionKind::NonRefreshingActivated
        || source_profile.is_some_and(mana_source_profile_is_exact_for_trajectory)
    {
        1.0
    } else {
        0.0
    };

    let mana_output = card.effects.mana_produced.conservative_value(1).min(8);
    let consumed_mana_source = match card.effects.mana_production_kind {
        ManaProductionKind::None | ManaProductionKind::Unsupported => false,
        ManaProductionKind::SpellResolution => {
            // The spell's printed mana cost was paid by the action loop. The
            // descriptor admits only unconditional Add-mana resolution text
            // with no unmodeled additional cost.
            pool.add_floating(source_colors, mana_output);
            false
        }
        ManaProductionKind::ReusableActivated => {
            // Tapping a noncreature permanent is immediately legal only when
            // its exact source profile does not say it enters tapped. Creature
            // sources wait because this model has no haste proof.
            let delayed_on_entry = card.has(role::CREATURE)
                || source_profile.is_some_and(|source| {
                    !reusable_nonland_source_is_available_on_entry(card, source)
                });
            let available_from_turn = if delayed_on_entry {
                turn.saturating_add(1)
            } else {
                turn
            };
            let source = BattlefieldManaSource {
                colors: source_colors,
                capacity: mana_output,
                reliability: source_reliability,
                available_from_turn,
                is_land: false,
                card_index: Some(card_index),
                ..BattlefieldManaSource::default()
            };
            battlefield.push(source);
            if available_from_turn == turn && source_reliability >= 0.999 {
                add_battlefield_source_to_current_pool(source, deck, zones, turn, pool);
            }
            false
        }
        ManaProductionKind::OneShotActivated => {
            // Compilation only admits a self-sacrifice activation whose other
            // timing/costs are already proven legal now. Credit the burst once
            // and tell the zone tracker to move the permanent to graveyard;
            // never install it as a reusable battlefield source.
            pool.add_floating(source_colors, mana_output);
            true
        }
        ManaProductionKind::NonRefreshingActivated => {
            // The complete reviewed lifecycle proves a legal first tap but
            // also forbids a normal untap. Credit that activation now while
            // retaining the artifact in the zone tracker. Deliberately omit a
            // battlefield mana-source entry so turn construction cannot
            // refresh it until the upkeep untap lifecycle is modeled exactly.
            if source_reliability >= 0.999 {
                pool.add_floating_from_nonland_permanent(
                    source_colors,
                    mana_output,
                    card_has_subtype(card, "Dwarf"),
                    ability_context,
                );
            }
            false
        }
    };

    let treasures = immediate_effect_value(card, card.effects.treasure_tokens, 1).min(8);
    pool.add_treasures(treasures);
    consumed_mana_source
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FirstUseSelfTransferTutorResolution {
    source_card: usize,
    selected_card: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueuedFirstUseSelfTransferTutorExecution {
    NotActivation,
    Rejected,
    Committed(FirstUseSelfTransferTutorResolution),
}

fn active_first_use_self_transfer_tutor(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
) -> Option<(u16, usize, TypedFirstUseSelfTransferTutor)> {
    zones
        .battlefield
        .iter()
        .filter_map(|presence| {
            let card = deck.cards.get(presence.card_index)?;
            let tutor = compile_typed_first_use_self_transfer_tutor(card)?;
            Some((
                presence.sequence,
                presence.card_index,
                tutor,
                card.normalized_name.as_str(),
            ))
        })
        .min_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.3.cmp(right.3))
                .then_with(|| left.1.cmp(&right.1))
        })
        .map(|(sequence, card_index, tutor, _)| (sequence, card_index, tutor))
}

/// Execute one complete first-controller activation. Every mutable component,
/// including RNG, is staged so an unpaid activation, empty library, or
/// non-improving public target leaves the episode byte-for-byte unchanged.
#[allow(clippy::too_many_arguments)]
fn execute_first_use_self_transfer_tutor(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    hand: &mut Vec<usize>,
    turn: u8,
    mana_pool: &mut TurnManaPool,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
    future_additional_generic_per_cast: u8,
) -> Option<FirstUseSelfTransferTutorResolution> {
    let (source_sequence, source_card, _) = active_first_use_self_transfer_tutor(deck, zones)?;
    execute_first_use_self_transfer_tutor_from_source(
        deck,
        mana_access,
        library_order,
        next_draw_position,
        hand,
        turn,
        mana_pool,
        rng,
        zones,
        future_additional_generic_per_cast,
        source_card,
        source_sequence,
    )
}

/// Execute the exact battlefield object selected by a queued planner action.
/// Source identity is validated before any payment, search, shuffle, or zone
/// mutation, and every mutable component remains staged through resolution.
#[allow(clippy::too_many_arguments)]
fn execute_first_use_self_transfer_tutor_from_source(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    hand: &mut Vec<usize>,
    turn: u8,
    mana_pool: &mut TurnManaPool,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
    future_additional_generic_per_cast: u8,
    source_card: usize,
    source_sequence: u16,
) -> Option<FirstUseSelfTransferTutorResolution> {
    let tutor = zones
        .battlefield
        .iter()
        .find(|presence| presence.card_index == source_card && presence.sequence == source_sequence)
        .and_then(|presence| deck.cards.get(presence.card_index))
        .and_then(compile_typed_first_use_self_transfer_tutor)?;

    let mut staged_pool = mana_pool.clone();
    if !pay_first_use_self_transfer_tutor_activation(&mut staged_pool, &tutor) {
        return None;
    }
    let instruction = TutorInstruction {
        source: TutorSourceZone::Library,
        target: crate::effects::TutorTarget::AnyCard,
        destination: TutorDestination::Hand,
        quantity: 1,
        reveal: false,
        shuffle_after: true,
    };
    let target_position = best_tutor_target_position(
        deck,
        instruction,
        library_order,
        next_draw_position,
        hand,
        zones,
        turn,
        mana_access,
        &staged_pool,
        future_additional_generic_per_cast,
        false,
    )?;
    let target_index = *library_order.get(target_position)?;
    let target = deck.cards.get(target_index)?;
    let current_route_potential = planning_reviewed_sequence_potential(
        deck,
        mana_access,
        hand,
        zones,
        turn,
        mana_pool,
        future_additional_generic_per_cast,
    );
    let mut staged_hand = hand.clone();
    staged_hand.push(target_index);
    let post_route_potential = planning_reviewed_sequence_potential(
        deck,
        mana_access,
        &staged_hand,
        zones,
        turn,
        &staged_pool,
        future_additional_generic_per_cast,
    );
    if post_route_potential <= current_route_potential
        && tutor_target_score(deck, target, hand, zones, turn) < 4_000
    {
        return None;
    }

    let mut staged_library = library_order.clone();
    let removed_target = staged_library.remove(target_position);
    debug_assert_eq!(removed_target, target_index);
    let mut staged_rng = rng.clone();
    let unseen_start = next_draw_position.min(staged_library.len());
    staged_library[unseen_start..].shuffle(&mut staged_rng);
    let mut staged_zones = zones.clone();
    let source_position = staged_zones.battlefield.iter().position(|presence| {
        presence.card_index == source_card && presence.sequence == source_sequence
    })?;
    let removed_source = staged_zones.battlefield.remove(source_position);
    debug_assert_eq!(removed_source.card_index, source_card);
    debug_assert_eq!(removed_source.sequence, source_sequence);
    staged_zones.advance_sequence();

    *mana_pool = staged_pool;
    *library_order = staged_library;
    *hand = staged_hand;
    *rng = staged_rng;
    *zones = staged_zones;
    Some(FirstUseSelfTransferTutorResolution {
        source_card,
        selected_card: target_index,
    })
}

#[allow(clippy::too_many_arguments)]
fn execute_queued_first_use_self_transfer_tutor_action(
    action: TurnAction,
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    hand: &mut Vec<usize>,
    turn: u8,
    mana_pool: &mut TurnManaPool,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
    future_additional_generic_per_cast: u8,
) -> QueuedFirstUseSelfTransferTutorExecution {
    let TurnAction::ActivateFirstUseSelfTransferTutor {
        source_card_index,
        source_sequence,
    } = action
    else {
        return QueuedFirstUseSelfTransferTutorExecution::NotActivation;
    };
    execute_first_use_self_transfer_tutor_from_source(
        deck,
        mana_access,
        library_order,
        next_draw_position,
        hand,
        turn,
        mana_pool,
        rng,
        zones,
        future_additional_generic_per_cast,
        source_card_index,
        source_sequence,
    )
    .map_or(
        QueuedFirstUseSelfTransferTutorExecution::Rejected,
        QueuedFirstUseSelfTransferTutorExecution::Committed,
    )
}

#[allow(clippy::too_many_arguments)]
fn finalize_first_use_self_transfer_tutor_runtime(
    deck: &CompiledDeck,
    resolution: FirstUseSelfTransferTutorResolution,
    mana_sources: &mut Vec<BattlefieldManaSource>,
    mana_pool: &mut TurnManaPool,
    zones: &KnownLineZoneState,
    engine_count: &mut u8,
    enabler_count: &mut u8,
    payoff_count: &mut u8,
    creature_count: &mut u8,
    protection_count: &mut u8,
    recursion_count: &mut u8,
) {
    if let Some(source) = deck.cards.get(resolution.source_card) {
        remove_roles(
            source.roles,
            engine_count,
            enabler_count,
            payoff_count,
            creature_count,
            protection_count,
        );
        if source.effects.recursion {
            *recursion_count = recursion_count.saturating_sub(1);
        }
    }
    debug_assert!(deck.cards.get(resolution.selected_card).is_some());
    synchronize_mana_sources_with_battlefield(mana_sources, zones);
    synchronize_turn_pool_with_battlefield(mana_pool, zones);
}

fn active_resource_tutor(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
) -> Option<(u8, ProgramTutorEffect)> {
    zones
        .battlefield
        .iter()
        .filter_map(|presence| deck.cards.get(presence.card_index))
        .flat_map(|card| card.ability_program.executable_abilities())
        .filter_map(|ability| {
            if !matches!(ability.timing, AbilityTiming::Activated { .. })
                || ability.costs.len() != 1
                || ability.effects.len() != 1
            {
                return None;
            }
            let AbilityCost::SacrificeResource {
                resource: ResourceKind::Treasure,
                count,
            } = &ability.costs[0]
            else {
                return None;
            };
            let AbilityEffect::Tutor(tutor) = &ability.effects[0] else {
                return None;
            };
            (tutor.from == ProgramZone::Library
                && tutor.destination == ProgramZone::Battlefield
                && *count > 0
                && *count <= u16::from(u8::MAX))
            .then(|| (*count as u8, tutor.clone()))
        })
        .min_by(|left, right| left.0.cmp(&right.0))
}

#[derive(Debug, Clone)]
struct StagedResourceTutorActivation {
    target_index: usize,
    target_entered_battlefield: bool,
    library_order: Vec<usize>,
    hand: Vec<usize>,
    mana_sources: Vec<BattlefieldManaSource>,
    mana_pool: TurnManaPool,
    rng: ChaCha8Rng,
    zones: KnownLineZoneState,
}

/// Stage the complete deterministic resource-tutor transaction on clones.
/// Target identity is still selected by the executor's public, stable ranking;
/// hidden order and RNG are consumed only inside the staged copy until the
/// caller explicitly commits it.
#[allow(clippy::too_many_arguments)]
fn stage_resource_tutor_ability(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    library_order: &[usize],
    next_draw_position: usize,
    hand: &[usize],
    turn: u8,
    mana_sources: &[BattlefieldManaSource],
    mana_pool: &TurnManaPool,
    rng: &ChaCha8Rng,
    zones: &KnownLineZoneState,
) -> Option<StagedResourceTutorActivation> {
    let mut staged_order = library_order.to_vec();
    let mut staged_hand = hand.to_vec();
    let mut staged_sources = mana_sources.to_vec();
    let mut staged_pool = mana_pool.clone();
    let mut staged_rng = rng.clone();
    let mut staged_zones = zones.clone();
    let target_index = execute_resource_tutor_ability(
        deck,
        &mut staged_order,
        next_draw_position,
        hand,
        turn,
        &mut staged_pool,
        &mut staged_rng,
        &mut staged_zones,
    )?;
    let target_entered_battlefield = install_tutored_permanent_runtime(
        deck,
        target_index,
        mana_access,
        turn,
        &mut staged_sources,
        &mut staged_pool,
        &mut staged_rng,
        Some(&mut staged_hand),
        &mut staged_zones,
    );
    synchronize_mana_sources_with_battlefield(&mut staged_sources, &staged_zones);
    synchronize_turn_pool_with_battlefield(&mut staged_pool, &staged_zones);
    staged_pool.refresh_battlefield_sources(
        deck,
        &staged_zones,
        active_ability_context(deck, &staged_zones),
    );
    Some(StagedResourceTutorActivation {
        target_index,
        target_entered_battlefield,
        library_order: staged_order,
        hand: staged_hand,
        mana_sources: staged_sources,
        mana_pool: staged_pool,
        rng: staged_rng,
        zones: staged_zones,
    })
}

fn staged_resource_tutor_completes_current_turn_conversion(
    deck: &CompiledDeck,
    turn: u8,
    zones_before: &KnownLineZoneState,
    staged: &StagedResourceTutorActivation,
) -> bool {
    if !staged.target_entered_battlefield {
        return false;
    }
    let Some(target) = deck.cards.get(staged.target_index) else {
        return false;
    };
    deck.known_lines.iter().any(|line| {
        line.table_lethal_if_resolved
            && line
                .cards
                .iter()
                .any(|name| crate::parser::normalize_card_name(name) == target.normalized_name)
            && !planning_line_is_completed(line, deck, zones_before, turn, &[])
            && planning_line_is_completed(line, deck, &staged.zones, turn, &[])
    })
}

fn should_commit_staged_resource_tutor(
    deck: &CompiledDeck,
    turn: u8,
    zones_before: &KnownLineZoneState,
    preserve_eot_before: bool,
    preserves_eot_after: bool,
    staged: &StagedResourceTutorActivation,
) -> bool {
    eot_reservation_allows_candidate(preserve_eot_before, preserves_eot_after)
        || staged_resource_tutor_completes_current_turn_conversion(deck, turn, zones_before, staged)
}

#[allow(clippy::too_many_arguments)]
fn execute_resource_tutor_ability(
    deck: &CompiledDeck,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    hand: &[usize],
    turn: u8,
    mana_pool: &mut TurnManaPool,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
) -> Option<usize> {
    let (treasure_cost, tutor) = active_resource_tutor(deck, zones)?;
    if mana_pool.remaining_treasures() < treasure_cost {
        return None;
    }
    let target_position = library_order
        .iter()
        .copied()
        .enumerate()
        .skip(next_draw_position)
        .filter(|(_, card_index)| {
            deck.cards
                .get(*card_index)
                .is_some_and(|card| program_tutor_matches(&tutor.filter, card))
        })
        .map(|(position, card_index)| {
            let card = &deck.cards[card_index];
            (
                position,
                card_index,
                tutor_target_score(deck, card, hand, zones, turn),
                card.normalized_name.as_str(),
            )
        })
        .filter(|(_, _, score, _)| *score >= 4_000)
        .max_by(|left, right| {
            left.2
                .cmp(&right.2)
                // Equal-value searches must not depend on the hidden order.
                .then_with(|| right.3.cmp(left.3))
                .then_with(|| right.1.cmp(&left.1))
        })
        .map(|(position, _, _, _)| position)?;

    let mut paid_pool = mana_pool.clone();
    if !paid_pool.spend_treasures(treasure_cost) {
        return None;
    }
    let target_index = library_order.remove(target_position);
    if tutor.shuffle_after {
        let unseen_start = next_draw_position.min(library_order.len());
        library_order[unseen_start..].shuffle(rng);
    }
    *mana_pool = paid_pool;
    zones.record_put_onto_battlefield(deck, target_index, turn);
    Some(target_index)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LibrarySelectionResolution {
    selected_card: Option<usize>,
}

fn active_library_selection_ability(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
) -> Option<(String, ProgramLibrarySelectionEffect)> {
    zones
        .battlefield
        .iter()
        .filter_map(|presence| deck.cards.get(presence.card_index))
        .flat_map(|card| card.ability_program.executable_abilities())
        .filter_map(|ability| {
            if !matches!(ability.timing, AbilityTiming::Activated { .. })
                || ability.costs.len() != 1
                || ability.effects.len() != 1
            {
                return None;
            }
            let AbilityCost::Mana(ProgramManaCost::PrintedSymbols { oracle, .. }) =
                &ability.costs[0]
            else {
                return None;
            };
            let AbilityEffect::LookAtTopAndSelect(selection) = &ability.effects[0] else {
                return None;
            };
            (selection.look_count > 0
                && selection.selection_count == 1
                && selection.destination == ProgramZone::Battlefield
                && selection.remainder == LibraryRemainderPlacement::BottomInRandomOrder)
                .then(|| (oracle.clone(), selection.clone()))
        })
        .min_by(|left, right| left.0.cmp(&right.0))
}

/// Commits the activation using only public state and the known remaining
/// multiset. The order of the unseen library is not inspected until after the
/// exact mana cost has been paid. This lets a pilot decide from hit odds while
/// preserving the real information boundary of a top-N activation.
#[allow(clippy::too_many_arguments)]
fn execute_library_selection_ability(
    deck: &CompiledDeck,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    hand: &[usize],
    turn: u8,
    mana_pool: &mut TurnManaPool,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
) -> Option<LibrarySelectionResolution> {
    let (oracle_cost, selection) = active_library_selection_ability(deck, zones)?;
    let unseen_start = next_draw_position.min(library_order.len());
    let unseen_len = library_order.len().saturating_sub(unseen_start);
    if unseen_len == 0 {
        return None;
    }

    // The controller knows the submitted singleton list plus every card in
    // hand/public zones, so the remaining multiset and its aggregate hit rate
    // are observable. Only positional order remains hidden.
    let remaining_legal_targets = library_order[unseen_start..]
        .iter()
        .filter(|card_index| {
            deck.cards
                .get(**card_index)
                .is_some_and(|card| program_object_filter_matches(&selection.filter, card))
        })
        .count();
    if remaining_legal_targets == 0 {
        return None;
    }

    let parsed_cost = parse_mana_cost(Some(&oracle_cost));
    let mut paid_pool = mana_pool.clone();
    if !paid_pool.pay(Some(&parsed_cost), 0, 0) {
        return None;
    }
    *mana_pool = paid_pool;

    let reveal_count = usize::from(selection.look_count).min(unseen_len);
    let mut revealed = library_order
        .drain(unseen_start..unseen_start + reveal_count)
        .collect::<Vec<_>>();
    let selected_position = revealed
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, card_index)| {
            deck.cards
                .get(*card_index)
                .is_some_and(|card| program_object_filter_matches(&selection.filter, card))
        })
        .map(|(position, card_index)| {
            let card = &deck.cards[card_index];
            (
                position,
                card_index,
                tutor_target_score(deck, card, hand, zones, turn),
                card.normalized_name.as_str(),
            )
        })
        .max_by(|left, right| {
            left.2
                .cmp(&right.2)
                .then_with(|| right.3.cmp(left.3))
                .then_with(|| right.1.cmp(&left.1))
        })
        .map(|(position, _, _, _)| position);
    let selected_card = selected_position.map(|position| revealed.remove(position));

    // The compiler currently admits only this exact remainder instruction.
    // Randomizing before appending makes these cards the true library bottom
    // without disturbing the still-unseen prefix above them.
    revealed.shuffle(rng);
    library_order.extend(revealed);
    if let Some(card_index) = selected_card {
        zones.record_put_onto_battlefield(deck, card_index, turn);
    }

    Some(LibrarySelectionResolution { selected_card })
}

fn program_tutor_matches(filter: &TutorFilter, card: &CompiledCard) -> bool {
    let TutorFilter::AnyOf(filters) = filter;
    filters
        .iter()
        .any(|filter| program_object_filter_matches(filter, card))
}

fn program_object_filter_matches(filter: &ProgramObjectFilter, card: &CompiledCard) -> bool {
    let matches_type = |card_type| {
        let card_types = card.effects.card_types;
        match card_type {
            ProgramCardType::Artifact => card_types.is_artifact,
            ProgramCardType::Creature => card_types.is_creature,
            ProgramCardType::Dragon => card_has_subtype(card, "Dragon"),
            ProgramCardType::Land => card_types.is_land,
            ProgramCardType::Permanent => {
                card_types.is_land
                    || card_types.is_creature
                    || card_types.is_artifact
                    || card_types.is_enchantment
            }
            ProgramCardType::Spell => card_types.is_instant || card_types.is_sorcery,
            ProgramCardType::Card => true,
        }
    };
    filter.card_type.is_none_or(matches_type)
        && (filter.any_of_card_types.is_empty()
            || filter
                .any_of_card_types
                .iter()
                .any(|card_type| match card_type {
                    SpecificCardType::Artifact => card.effects.card_types.is_artifact,
                    SpecificCardType::Enchantment => card.effects.card_types.is_enchantment,
                }))
        && filter
            .excluded_card_type
            .is_none_or(|card_type| !matches_type(card_type))
        && (!filter.nonland || !card.effects.card_types.is_land)
        && filter
            .subtype
            .as_deref()
            .is_none_or(|subtype| card_has_subtype(card, subtype))
        && filter
            .excluded_subtype
            .as_deref()
            .is_none_or(|subtype| !card_has_subtype(card, subtype))
        && filter
            .controller
            .is_none_or(|controller| controller == ControllerRelation::You)
}

#[allow(clippy::too_many_arguments)]
fn install_tutored_permanent_runtime(
    deck: &CompiledDeck,
    card_index: usize,
    mana_access: Option<&ManaAccessProfile>,
    turn: u8,
    mana_sources: &mut Vec<BattlefieldManaSource>,
    mana_pool: &mut TurnManaPool,
    _rng: &mut ChaCha8Rng,
    mut hand: Option<&mut Vec<usize>>,
    zones: &mut KnownLineZoneState,
) -> bool {
    let Some(card) = deck.cards.get(card_index) else {
        return false;
    };
    if let Some(kind) = compile_typed_conditional_mana_source(card) {
        let linked_colors = match kind {
            TypedConditionalManaSource::ImprintLinkedCardColors => hand.as_mut().and_then(|hand| {
                select_imprint_hand_position(deck, mana_access, hand, zones, turn, mana_pool, 0)
                    .map(|position| {
                        let linked_index = hand.swap_remove(position);
                        zones.exile.push(linked_index);
                        zones.advance_sequence();
                        printed_hand_card_colors(&deck.cards[linked_index])
                            .unwrap_or(ManaColorMask::NONE)
                    })
            }),
            TypedConditionalManaSource::DiscardLandOrFailEntry => {
                let discarded = hand.as_mut().and_then(|hand| {
                    select_land_discard_hand_position(
                        deck,
                        mana_access,
                        hand,
                        zones,
                        turn,
                        mana_pool,
                        0,
                    )
                    .map(|position| hand.swap_remove(position))
                });
                let Some(discarded) = discarded else {
                    // The replacement effect puts the source into its owner's
                    // graveyard instead of letting it enter.
                    zones.remove_named_permanent(deck, &card.normalized_name, true);
                    return false;
                };
                zones.record_discard(deck, discarded);
                None
            }
            _ => None,
        };
        if let Some(source) =
            typed_battlefield_mana_source(deck, card_index, kind, linked_colors, turn)
        {
            mana_sources.push(source);
            add_typed_source_to_current_pool(source, deck, zones, turn, mana_pool);
        }
        return true;
    }
    if card.effects.mana_production_kind == ManaProductionKind::NonRefreshingActivated {
        let source_profile = mana_access.and_then(|access| access.source(card_index));
        let colors = source_profile
            .map(|source| source.colors)
            .unwrap_or(ManaColorMask::COLORLESS);
        let capacity = card.effects.mana_produced.conservative_value(1).min(8);
        // This enum is emitted only for the complete reviewed lifecycle
        // (currently Mana Vault-shaped text). The effect compiler, rather
        // than the reporting-weight profile, is the execution authority.
        mana_pool.add_floating_from_nonland_permanent(
            colors,
            capacity,
            card_has_subtype(card, "Dwarf"),
            active_ability_context(deck, zones),
        );
        return true;
    }
    if card.effects.mana_production_kind != ManaProductionKind::ReusableActivated {
        return true;
    }
    let source_profile = mana_access.and_then(|access| access.source(card_index));
    let colors = source_profile
        .map(|source| source.colors)
        .unwrap_or_default();
    let reliability = if source_profile.is_some_and(mana_source_profile_is_exact_for_trajectory) {
        1.0
    } else {
        0.0
    };
    let capacity = card.effects.mana_produced.conservative_value(1).min(8);
    let delayed_on_entry = card.has(role::CREATURE)
        || source_profile
            .is_some_and(|source| !reusable_nonland_source_is_available_on_entry(card, source));
    let available_from_turn = if delayed_on_entry {
        turn.saturating_add(1)
    } else {
        turn
    };
    let source = BattlefieldManaSource {
        colors,
        capacity,
        reliability,
        available_from_turn,
        is_land: false,
        card_index: Some(card_index),
        ..BattlefieldManaSource::default()
    };
    mana_sources.push(source);
    if available_from_turn == turn && reliability >= 0.999 {
        add_battlefield_source_to_current_pool(source, deck, zones, turn, mana_pool);
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn execute_tutor_on_resolution(
    deck: &CompiledDeck,
    tutor: &CompiledCard,
    mana_access: Option<&ManaAccessProfile>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    hand: &mut Vec<usize>,
    turn: u8,
    mana_sources: &mut Vec<BattlefieldManaSource>,
    mana_pool: &mut TurnManaPool,
    rng: &mut ChaCha8Rng,
    line_zones: &mut KnownLineZoneState,
    engine_count: &mut u8,
    enabler_count: &mut u8,
    payoff_count: &mut u8,
    creature_count: &mut u8,
    protection_count: &mut u8,
    future_additional_generic_per_cast: u8,
) {
    if !tutor.effects.tutor.is_executable_on_spell_resolution() {
        return;
    }

    for instruction in &tutor.effects.tutor.instructions {
        if instruction.source != TutorSourceZone::Library {
            continue;
        }
        let library_top_reaches_next_modeled_draw = instruction.destination
            == TutorDestination::LibraryTop
            && !active_necropotence_lifecycle(deck, line_zones);
        let next_turn_pool = library_top_reaches_next_modeled_draw.then(|| {
            let mut future_pool =
                future_untapped_mana_pool(deck, line_zones, mana_sources, turn.saturating_add(1));
            future_pool.add_treasures(mana_pool.remaining_treasures());
            future_pool
        });
        let decision_turn = if library_top_reaches_next_modeled_draw {
            turn.saturating_add(1)
        } else {
            turn
        };
        let decision_pool = next_turn_pool.unwrap_or_else(|| mana_pool.clone());
        let mut searched_to_library_top = Vec::new();
        for _ in 0..instruction.quantity.min(8) {
            // When a search places several cards on top, only the first
            // selected target is ranked as the next modeled draw. The chosen
            // order is committed below so that target remains physically on
            // top; later targets cannot all masquerade as immediately held.
            let target_is_next_modeled_draw =
                library_top_reaches_next_modeled_draw && searched_to_library_top.is_empty();
            let Some(target_position) = best_tutor_target_position(
                deck,
                *instruction,
                library_order,
                next_draw_position,
                hand,
                line_zones,
                decision_turn,
                mana_access,
                &decision_pool,
                future_additional_generic_per_cast,
                target_is_next_modeled_draw,
            ) else {
                break;
            };
            let target_index = library_order.remove(target_position);
            let Some(target) = deck.cards.get(target_index) else {
                continue;
            };
            match instruction.destination {
                TutorDestination::None => {}
                TutorDestination::Hand => hand.push(target_index),
                TutorDestination::LibraryTop => {
                    searched_to_library_top.push(target_index);
                }
                TutorDestination::BattlefieldTapped | TutorDestination::BattlefieldUntapped => {
                    // The current typed executor only compiles direct land
                    // searches to the battlefield. This guard keeps malformed
                    // or hand-authored descriptors from bypassing a permanent's
                    // mana cost or inventing its ETB behavior.
                    if !target.effects.card_types.is_land {
                        library_order
                            .insert(next_draw_position.min(library_order.len()), target_index);
                        continue;
                    }
                    let force_tapped =
                        instruction.destination == TutorDestination::BattlefieldTapped;
                    let source = battlefield_tutored_land_source(
                        deck,
                        mana_access,
                        target_index,
                        turn,
                        force_tapped,
                    );
                    mana_sources.push(source);
                    line_zones.record_put_onto_battlefield(
                        deck,
                        target_index,
                        source.available_from_turn,
                    );
                    if battlefield_source_is_trajectory_available(&source, turn) {
                        add_battlefield_source_to_current_pool(
                            source, deck, line_zones, turn, mana_pool,
                        );
                    }
                    apply_roles(
                        target.roles,
                        engine_count,
                        enabler_count,
                        payoff_count,
                        creature_count,
                        protection_count,
                    );
                }
            }
        }
        if instruction.shuffle_after {
            let unseen_start = next_draw_position.min(library_order.len());
            library_order[unseen_start..].shuffle(rng);
        }
        let unseen_start = next_draw_position.min(library_order.len());
        for target_index in searched_to_library_top.into_iter().rev() {
            library_order.insert(unseen_start, target_index);
        }
    }
}

fn exact_instant_library_top_tutor(card: &CompiledCard) -> Option<TutorInstruction> {
    if !card.effects.card_types.is_instant
        || !card.effects.tutor.is_executable_on_spell_resolution()
    {
        return None;
    }
    let [instruction] = card.effects.tutor.instructions.as_slice() else {
        return None;
    };
    (instruction.source == TutorSourceZone::Library
        && instruction.destination == TutorDestination::LibraryTop
        && instruction.quantity == 1)
        .then_some(*instruction)
}

/// Mana that can still be used in the preceding opponent end step. Floating
/// mana from the active player's main phase has already emptied; an unspent
/// Treasure persists, and an exact battlefield source remains available only
/// when it was not activated earlier in the turn.
fn opponent_end_step_payment_pool(
    turn_pool: &TurnManaPool,
    zones: &KnownLineZoneState,
    treasure_reserve: u8,
) -> TurnManaPool {
    let mut remaining_battlefield_copies = zones
        .battlefield
        .iter()
        .filter(|presence| {
            !zones
                .tapped_creatures_this_turn
                .contains(&presence.sequence)
        })
        .fold(HashMap::<usize, usize>::new(), |mut counts, presence| {
            *counts.entry(presence.card_index).or_default() += 1;
            counts
        });
    let mut persistent_sources = Vec::new();
    for source in turn_pool.sources.iter().copied().filter(|source| {
        !source.is_treasure
            && source.origin_card_index.is_some()
            && !source.activation_used
            && !source.physically_tapped
            && source.remaining > 0
    }) {
        let Some(card_index) = source.origin_card_index else {
            continue;
        };
        if let Some(sequence) = source.origin_sequence {
            if !zones
                .battlefield
                .iter()
                .any(|presence| presence.card_index == card_index && presence.sequence == sequence)
                || zones.tapped_creatures_this_turn.contains(&sequence)
            {
                continue;
            }
            persistent_sources.push(source);
            continue;
        }
        let Some(remaining) = remaining_battlefield_copies.get_mut(&card_index) else {
            continue;
        };
        if *remaining == 0 {
            continue;
        }
        *remaining -= 1;
        persistent_sources.push(source);
    }
    let mut pool = TurnManaPool {
        sources: persistent_sources,
        pending_triggered_treasures: 0,
        pending_source_damage: 0,
    };
    pool.add_treasures(treasure_reserve);
    pool
}

fn future_untapped_mana_pool(
    deck: &CompiledDeck,
    zones: &KnownLineZoneState,
    mana_sources: &[BattlefieldManaSource],
    turn: u8,
) -> TurnManaPool {
    TurnManaPool::from_battlefield_with_ability_context(
        mana_sources,
        turn,
        deck,
        active_ability_context(deck, zones),
        zones,
    )
}

#[derive(Debug, Clone, PartialEq)]
struct OpponentEndStepTopTutorProjection {
    tutor_index: usize,
    target_index: usize,
    payment_choice: SpellPaymentChoice,
    hand_after_cast: Vec<usize>,
    zones_after_resolution: KnownLineZoneState,
    pool_after_cast_triggers: TurnManaPool,
    life_after_resolution: f32,
    route_potential: i64,
    target_score: i32,
}

/// Resolve the modeled draw step. Exact instant top-library tutors use the
/// prior opponent end-step window, where their payment belongs, rather than
/// consuming mana after the active player's untap.
fn execute_modeled_draw_step(
    deck: &CompiledDeck,
    hand: &mut Vec<usize>,
    library_order: &[usize],
    next_draw_position: &mut usize,
    zones: &KnownLineZoneState,
) {
    if active_necropotence_lifecycle(deck, zones) {
        return;
    }
    if *next_draw_position < library_order.len() {
        hand.push(library_order[*next_draw_position]);
        *next_draw_position += 1;
    }
}

/// Build the complete public transaction for the best exact instant-speed
/// LibraryTop tutor. Candidate identities come only from the observable
/// remaining-library multiset. The helper is pure: it stages exact payment,
/// source damage, the first-spell controller triggers, resolution life loss,
/// and the certain next draw without mutating a real or speculative zone.
#[allow(clippy::too_many_arguments)]
fn best_opponent_end_step_top_tutor_projection(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &[usize],
    available_library_copies: &HashMap<usize, u16>,
    turn: u8,
    next_turn: u8,
    end_step_pool: &TurnManaPool,
    future_untapped_pool: &TurnManaPool,
    zones: &KnownLineZoneState,
    player_life: f32,
    additional_generic_per_cast: u8,
    pending_first_relevant_spell_counter: bool,
) -> Option<OpponentEndStepTopTutorProjection> {
    let mana_access = mana_access?;
    if active_necropotence_lifecycle(deck, zones) {
        return None;
    }
    let mut current_future_pool = future_untapped_pool.clone();
    current_future_pool.add_treasures(end_step_pool.remaining_treasures());
    let current_route_potential = planning_reviewed_sequence_potential(
        deck,
        Some(mana_access),
        hand,
        zones,
        next_turn,
        &current_future_pool,
        additional_generic_per_cast,
    );

    let mut distinct_tutors = hand.to_vec();
    distinct_tutors.sort_unstable();
    distinct_tutors.dedup();
    distinct_tutors
        .into_iter()
        .flat_map(|tutor_index| {
            [SpellPaymentChoice::Printed, SpellPaymentChoice::Alternative]
                .into_iter()
                .filter_map(move |payment_choice| {
                    let tutor = deck.cards.get(tutor_index)?;
                    let instruction = exact_instant_library_top_tutor(tutor)?;
                    if pending_first_relevant_spell_counter && scenario_relevant_spell(tutor) {
                        return None;
                    }
                    let cost = mana_access.cost(tutor_index);
                    if payment_choice == SpellPaymentChoice::Printed
                        && cost.is_none_or(|cost| !activation_cost_is_exactly_modeled(cost))
                    {
                        return None;
                    }
                    let mut paid_pool = end_step_pool.clone();
                    let mut post_resolution_zones = zones.clone();
                    if !pay_spell_cost_choice(
                        deck,
                        &mut post_resolution_zones,
                        &mut paid_pool,
                        tutor_index,
                        cost,
                        tutor.mana_value.ceil().max(0.0) as u8,
                        additional_generic_per_cast,
                        0,
                        turn,
                        payment_choice,
                        None,
                    ) {
                        return None;
                    }
                    let hand_position = hand
                        .iter()
                        .position(|candidate| *candidate == tutor_index)?;
                    let mut post_cast_hand = hand.to_vec();
                    post_cast_hand.swap_remove(hand_position);
                    let mut candidate_window_pool = paid_pool.clone();
                    let mut candidate_life = player_life;
                    if !candidate_window_pool.settle_pending_source_damage(&mut candidate_life) {
                        return None;
                    }
                    if !apply_controller_spell_cast_triggers(
                        deck,
                        &mut post_resolution_zones,
                        tutor_index,
                        turn,
                        1,
                        &mut candidate_window_pool,
                        &mut candidate_life,
                    ) {
                        return None;
                    }
                    candidate_life -= f32::from(tutor.effects.tutor.life_loss_after_resolution);
                    if candidate_life <= 0.0 {
                        return None;
                    }
                    post_resolution_zones.record_cast(deck, tutor_index, turn);
                    let mut candidate_future_pool = future_untapped_pool.clone();
                    candidate_future_pool
                        .add_treasures(candidate_window_pool.remaining_treasures());
                    let target_index = best_tutor_target_identity(
                        deck,
                        instruction,
                        available_library_copies,
                        &post_cast_hand,
                        &post_resolution_zones,
                        next_turn,
                        Some(mana_access),
                        &candidate_future_pool,
                        additional_generic_per_cast,
                        true,
                    )?;
                    let target = deck.cards.get(target_index)?;
                    let target_score = tutor_target_score(
                        deck,
                        target,
                        &post_cast_hand,
                        &post_resolution_zones,
                        next_turn,
                    );
                    let hand_after_cast = post_cast_hand.clone();
                    let mut post_draw_hand = post_cast_hand;
                    post_draw_hand.push(target_index);
                    let route_potential = planning_reviewed_sequence_potential(
                        deck,
                        Some(mana_access),
                        &post_draw_hand,
                        &post_resolution_zones,
                        next_turn,
                        &candidate_future_pool,
                        additional_generic_per_cast,
                    );
                    (route_potential > current_route_potential || target_score >= 20_000).then_some(
                        OpponentEndStepTopTutorProjection {
                            tutor_index,
                            target_index,
                            payment_choice,
                            hand_after_cast,
                            zones_after_resolution: post_resolution_zones,
                            pool_after_cast_triggers: candidate_window_pool,
                            life_after_resolution: candidate_life,
                            route_potential,
                            target_score,
                        },
                    )
                })
        })
        .max_by(|left, right| {
            left.route_potential
                .cmp(&right.route_potential)
                .then_with(|| left.target_score.cmp(&right.target_score))
                .then_with(|| {
                    let left_name = &deck.cards[left.target_index].normalized_name;
                    let right_name = &deck.cards[right.target_index].normalized_name;
                    right_name.cmp(left_name)
                })
                .then_with(|| right.target_index.cmp(&left.target_index))
                .then_with(|| {
                    let left_name = &deck.cards[left.tutor_index].normalized_name;
                    let right_name = &deck.cards[right.tutor_index].normalized_name;
                    right_name.cmp(left_name)
                })
                .then_with(|| right.tutor_index.cmp(&left.tutor_index))
                .then_with(|| {
                    (left.payment_choice == SpellPaymentChoice::Printed)
                        .cmp(&(right.payment_choice == SpellPaymentChoice::Printed))
                })
        })
}

/// Revalidate a previously selected public identity transaction against the
/// exact live pool, hand, zones, life, and unseen multiset, then commit it
/// atomically. A stale source or target is a clean no-op.
#[allow(clippy::too_many_arguments)]
fn commit_opponent_end_step_top_tutor_projection(
    expected: &OpponentEndStepTopTutorProjection,
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &mut Vec<usize>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    turn: u8,
    next_turn: u8,
    end_step_pool: &mut TurnManaPool,
    future_untapped_pool: &TurnManaPool,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
    spells_cast_in_window: &mut u8,
    player_life: &mut f32,
    additional_generic_per_cast: u8,
    pending_first_relevant_spell_counter: bool,
) -> bool {
    if *spells_cast_in_window != 0 {
        return false;
    }
    let unseen_start = next_draw_position.min(library_order.len());
    let available_library_copies = exact_card_multiset(&library_order[unseen_start..]);
    let Some(revalidated) = best_opponent_end_step_top_tutor_projection(
        deck,
        mana_access,
        hand,
        &available_library_copies,
        turn,
        next_turn,
        end_step_pool,
        future_untapped_pool,
        zones,
        *player_life,
        additional_generic_per_cast,
        pending_first_relevant_spell_counter,
    ) else {
        return false;
    };
    if revalidated.tutor_index != expected.tutor_index
        || revalidated.target_index != expected.target_index
        || revalidated.payment_choice != expected.payment_choice
    {
        return false;
    }
    let Some(hand_position) = hand
        .iter()
        .position(|card_index| *card_index == revalidated.tutor_index)
    else {
        return false;
    };
    let Some(target_position) = library_order
        .iter()
        .enumerate()
        .skip(unseen_start)
        .find_map(|(position, card_index)| {
            (*card_index == revalidated.target_index).then_some(position)
        })
    else {
        return false;
    };

    let mut staged_hand = hand.clone();
    let removed_tutor = staged_hand.swap_remove(hand_position);
    if removed_tutor != revalidated.tutor_index || staged_hand != revalidated.hand_after_cast {
        return false;
    }
    let mut staged_library = library_order.clone();
    let removed_target = staged_library.remove(target_position);
    if removed_target != revalidated.target_index {
        return false;
    }
    let mut staged_rng = rng.clone();
    let staged_unseen_start = next_draw_position.min(staged_library.len());
    let instruction = exact_instant_library_top_tutor(&deck.cards[revalidated.tutor_index])
        .expect("the public transaction revalidated an exact top tutor");
    if instruction.shuffle_after {
        staged_library[staged_unseen_start..].shuffle(&mut staged_rng);
    }
    staged_library.insert(staged_unseen_start, removed_target);

    *hand = staged_hand;
    *library_order = staged_library;
    *rng = staged_rng;
    *end_step_pool = revalidated.pool_after_cast_triggers;
    *zones = revalidated.zones_after_resolution;
    *player_life = revalidated.life_after_resolution;
    *spells_cast_in_window = 1;
    true
}

/// Commit only the cast portion of an otherwise valid EOT tutor transaction.
/// The spell is paid for, leaves hand, produces controller cast triggers, and
/// reaches the graveyard, but its search and resolution life loss do not
/// happen because the isolated response counters it. Hidden library order and
/// RNG remain untouched.
#[allow(clippy::too_many_arguments)]
fn commit_countered_opponent_end_step_top_tutor(
    expected: &OpponentEndStepTopTutorProjection,
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &mut Vec<usize>,
    available_library_copies: &HashMap<usize, u16>,
    turn: u8,
    next_turn: u8,
    end_step_pool: &mut TurnManaPool,
    future_untapped_pool: &TurnManaPool,
    zones: &mut KnownLineZoneState,
    spells_cast_in_window: &mut u8,
    player_life: &mut f32,
    additional_generic_per_cast: u8,
) -> bool {
    if *spells_cast_in_window != 0 {
        return false;
    }
    let Some(revalidated) = best_opponent_end_step_top_tutor_projection(
        deck,
        mana_access,
        hand,
        available_library_copies,
        turn,
        next_turn,
        end_step_pool,
        future_untapped_pool,
        zones,
        *player_life,
        additional_generic_per_cast,
        false,
    ) else {
        return false;
    };
    if revalidated.tutor_index != expected.tutor_index
        || revalidated.target_index != expected.target_index
        || revalidated.payment_choice != expected.payment_choice
    {
        return false;
    }
    let Some(tutor) = deck.cards.get(revalidated.tutor_index) else {
        return false;
    };
    let cost = mana_access.and_then(|access| access.cost(revalidated.tutor_index));
    if revalidated.payment_choice == SpellPaymentChoice::Printed
        && cost.is_none_or(|cost| !activation_cost_is_exactly_modeled(cost))
    {
        return false;
    }
    let Some(hand_position) = hand
        .iter()
        .position(|card_index| *card_index == revalidated.tutor_index)
    else {
        return false;
    };

    let mut staged_pool = end_step_pool.clone();
    let mut staged_zones = zones.clone();
    if !pay_spell_cost_choice(
        deck,
        &mut staged_zones,
        &mut staged_pool,
        revalidated.tutor_index,
        cost,
        tutor.mana_value.ceil().max(0.0) as u8,
        additional_generic_per_cast,
        0,
        turn,
        revalidated.payment_choice,
        None,
    ) {
        return false;
    }
    let mut staged_life = *player_life;
    if !staged_pool.settle_pending_source_damage(&mut staged_life)
        || !apply_controller_spell_cast_triggers(
            deck,
            &mut staged_zones,
            revalidated.tutor_index,
            turn,
            1,
            &mut staged_pool,
            &mut staged_life,
        )
    {
        return false;
    }
    let mut staged_hand = hand.clone();
    let removed_tutor = staged_hand.swap_remove(hand_position);
    if removed_tutor != revalidated.tutor_index || staged_hand != revalidated.hand_after_cast {
        return false;
    }
    staged_zones.record_cast(deck, revalidated.tutor_index, turn);

    *hand = staged_hand;
    *end_step_pool = staged_pool;
    *zones = staged_zones;
    *player_life = staged_life;
    *spells_cast_in_window = 1;
    true
}

/// Cast at most one exact instant-speed top-library tutor in the preceding
/// opponent end step. `future_untapped_pool` contains only next-turn reusable
/// sources; each candidate adds the Treasures that remain after its real
/// end-step payment before evaluating the card that will be drawn.
///
/// Returns true only when the player dies while paying for or resolving the
/// selected spell.
#[allow(clippy::too_many_arguments)]
fn execute_opponent_end_step_top_tutor(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &mut Vec<usize>,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    turn: u8,
    next_turn: u8,
    end_step_pool: &mut TurnManaPool,
    future_untapped_pool: &TurnManaPool,
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
    spells_cast_in_window: &mut u8,
    player_life: &mut f32,
    isolated: &mut IsolatedScenarioRuntime,
) -> bool {
    if *spells_cast_in_window != 0 {
        return false;
    }
    let unseen_start = next_draw_position.min(library_order.len());
    let available_library_copies = exact_card_multiset(&library_order[unseen_start..]);
    let pending_generic_tax =
        isolated.is(InteractionScenario::GenericTaxStax) && !isolated.applied();
    let pending_first_relevant_spell_counter =
        isolated.is(InteractionScenario::FirstRelevantSpellCountered) && !isolated.applied();
    let mut additional_generic_per_cast =
        u8::from(isolated.is(InteractionScenario::GenericTaxStax) && isolated.applied());
    // Select the intended public cast before applying a pending isolated
    // response. This is the same checkpoint used by ordinary hand spells: a
    // tax that makes the spell unaffordable or a counter that stops it still
    // records an effectful intervention instead of silently changing a paired
    // trajectory.
    let Some(mut projection) = best_opponent_end_step_top_tutor_projection(
        deck,
        mana_access,
        hand,
        &available_library_copies,
        turn,
        next_turn,
        end_step_pool,
        future_untapped_pool,
        zones,
        *player_life,
        additional_generic_per_cast,
        false,
    ) else {
        return false;
    };
    isolated.observe_opportunity(InteractionScenario::GenericTaxStax);
    if deck
        .cards
        .get(projection.tutor_index)
        .is_some_and(scenario_relevant_spell)
    {
        isolated.observe_opportunity(InteractionScenario::FirstRelevantSpellCountered);
    }
    if pending_generic_tax {
        isolated.activate(turn, 1);
        additional_generic_per_cast = 1;
        let Some(taxed_projection) = best_opponent_end_step_top_tutor_projection(
            deck,
            mana_access,
            hand,
            &available_library_copies,
            turn,
            next_turn,
            end_step_pool,
            future_untapped_pool,
            zones,
            *player_life,
            additional_generic_per_cast,
            false,
        ) else {
            return false;
        };
        projection = taxed_projection;
    }
    if pending_first_relevant_spell_counter
        && deck
            .cards
            .get(projection.tutor_index)
            .is_some_and(scenario_relevant_spell)
    {
        if !commit_countered_opponent_end_step_top_tutor(
            &projection,
            deck,
            mana_access,
            hand,
            &available_library_copies,
            turn,
            next_turn,
            end_step_pool,
            future_untapped_pool,
            zones,
            spells_cast_in_window,
            player_life,
            additional_generic_per_cast,
        ) {
            return false;
        }
        isolated.activate(turn, 1);
        return *player_life <= 0.0;
    }
    if !commit_opponent_end_step_top_tutor_projection(
        &projection,
        deck,
        mana_access,
        hand,
        library_order,
        next_draw_position,
        turn,
        next_turn,
        end_step_pool,
        future_untapped_pool,
        rng,
        zones,
        spells_cast_in_window,
        player_life,
        additional_generic_per_cast,
        false,
    ) {
        return false;
    }
    let counter_recovery =
        isolated.is(InteractionScenario::FirstRelevantSpellCountered) && isolated.applied();
    let rule_recovery = isolated.is(InteractionScenario::RuleOfLawCap)
        && isolated
            .applied_turn
            .is_some_and(|applied_turn| turn > applied_turn);

    if counter_recovery || rule_recovery {
        isolated.recover(turn);
    }
    if isolated.is(InteractionScenario::GenericTaxStax) {
        isolated.recover(turn);
    }
    *player_life <= 0.0
}

#[allow(clippy::too_many_arguments)]
fn execute_opponent_end_step_top_tutor_before_next_turn(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    hand: &mut Vec<usize>,
    library_order: &mut Vec<usize>,
    next_draw_position: &mut usize,
    turn: u8,
    turn_pool: &TurnManaPool,
    mana_sources: &[BattlefieldManaSource],
    rng: &mut ChaCha8Rng,
    zones: &mut KnownLineZoneState,
    treasure_reserve: &mut u8,
    creature_count: &mut u8,
    player_life: &mut f32,
    isolated: &mut IsolatedScenarioRuntime,
) -> bool {
    let next_turn = turn.saturating_add(1);
    let mut end_step_pool = opponent_end_step_payment_pool(turn_pool, zones, *treasure_reserve);
    let future_untapped_pool = future_untapped_mana_pool(deck, zones, mana_sources, next_turn);
    let mut spells_cast_in_window = 0;
    let player_died = execute_opponent_end_step_top_tutor(
        deck,
        mana_access,
        hand,
        library_order,
        *next_draw_position,
        turn,
        next_turn,
        &mut end_step_pool,
        &future_untapped_pool,
        rng,
        zones,
        &mut spells_cast_in_window,
        player_life,
        isolated,
    );
    if player_died {
        *treasure_reserve = end_step_pool.remaining_treasures();
        return true;
    }

    // Normal-priority token activations may be made in the final opponent end
    // step after the last quest-counter trigger has resolved. Those creatures
    // have been controlled since before the next turn begins and may attack
    // then; using only the active player's precombat main phase delayed them
    // by a full turn.
    let created_tokens =
        activate_counter_threshold_token_abilities(deck, zones, &mut end_step_pool, turn);
    *creature_count =
        creature_count.saturating_add(u8::try_from(created_tokens).unwrap_or(u8::MAX));
    if !end_step_pool.settle_pending_source_damage(player_life) {
        *treasure_reserve = end_step_pool.remaining_treasures();
        return true;
    }
    let entry_draws = zones.take_pending_card_draws();
    draw_bounded_cards(hand, library_order, next_draw_position, entry_draws);
    *treasure_reserve = end_step_pool.remaining_treasures();
    false
}

fn planner_public_library_multiset(
    deck: &CompiledDeck,
    state: &CastPlanningState,
) -> HashMap<usize, u16> {
    deck.cards
        .iter()
        .enumerate()
        .filter_map(|(card_index, _)| {
            let copies = planner_library_copy_count(deck, state, card_index)
                .min(usize::from(u16::MAX)) as u16;
            (copies > 0).then_some((card_index, copies))
        })
        .collect()
}

/// Merge fixed live battlefield sources with exact reusable sources created by
/// a speculative child. The latter already carry their public origin,
/// capacity, color behavior, and damage lifecycle in the planner pool; no card
/// identity is re-interpreted to invent an unavailable source.
fn planner_future_mana_sources(
    zones: &KnownLineZoneState,
    fixed_sources: &[BattlefieldManaSource],
    state_pool: &TurnManaPool,
    next_turn: u8,
) -> Vec<BattlefieldManaSource> {
    let mut sources = fixed_sources.to_vec();
    synchronize_mana_sources_with_battlefield(&mut sources, zones);
    let mut unmatched_fixed = sources.iter().filter_map(|source| source.card_index).fold(
        HashMap::<usize, usize>::new(),
        |mut counts, card_index| {
            *counts.entry(card_index).or_default() += 1;
            counts
        },
    );
    let battlefield_counts =
        zones
            .battlefield
            .iter()
            .fold(HashMap::<usize, usize>::new(), |mut counts, presence| {
                *counts.entry(presence.card_index).or_default() += 1;
                counts
            });
    let mut represented_pool_copies = HashMap::<usize, usize>::new();
    for source in state_pool.sources.iter().filter(|source| {
        !source.is_treasure && source.origin_card_index.is_some() && source.base_capacity > 0
    }) {
        let card_index = source
            .origin_card_index
            .expect("filtered planner source has a public origin");
        let represented = represented_pool_copies.entry(card_index).or_default();
        if *represented
            >= battlefield_counts
                .get(&card_index)
                .copied()
                .unwrap_or_default()
        {
            continue;
        }
        *represented += 1;
        if unmatched_fixed
            .get_mut(&card_index)
            .is_some_and(|remaining| {
                if *remaining > 0 {
                    *remaining -= 1;
                    true
                } else {
                    false
                }
            })
        {
            continue;
        }
        sources.push(BattlefieldManaSource {
            colors: source.colors,
            capacity: source.base_capacity,
            reliability: 1.0,
            available_from_turn: next_turn,
            is_land: source.is_land,
            card_index: Some(card_index),
            behavior: source.behavior,
            source_damage_on_first_spend: source.source_damage_on_first_spend,
            damage_free_colors_on_first_spend: source.damage_free_colors_on_first_spend,
        });
    }
    sources
}

/// Project only the public, certain next-draw value of an exact opponent-EOT
/// top tutor. Oversized pre-cleanup hands fail closed; for admitted states,
/// beginning-EOT self-sacrifices are staged on clones before payment and
/// future mana are constructed. No hidden order or speculative zone mutation
/// escapes.
fn projected_opponent_end_step_top_tutor(
    domain: &CastPlanningDomain<'_>,
    state: &CastPlanningState,
) -> Option<OpponentEndStepTopTutorProjection> {
    let context = domain.opponent_end_step.as_ref()?;
    if domain.turn >= context.maximum_turn
        || state.stochastic_planner_expectation.is_some()
        || state.hand.len() > MAXIMUM_CLEANUP_HAND_SIZE
        || state.mana_pool.pending_triggered_treasures > 0
        || state.mana_pool.pending_source_damage > 0
        || active_necropotence_lifecycle(domain.deck, &state.zones)
    {
        return None;
    }
    let mana_access = domain.mana_access?;
    if !state.hand.iter().copied().any(|card_index| {
        domain.deck.cards.get(card_index).is_some_and(|card| {
            exact_instant_library_top_tutor(card).is_some()
                && mana_access
                    .cost(card_index)
                    .is_some_and(activation_cost_is_exactly_modeled)
                && !(context.first_relevant_spell_will_be_countered
                    && scenario_relevant_spell(card))
        })
    }) {
        // Deterministic planner evaluation visits many states that cannot
        // possibly use this window. Reject those before cloning zones/sources
        // or reconstructing the public remaining-library multiset.
        return None;
    }

    let mut projected_zones = state.zones.clone();
    let next_turn = domain.turn.saturating_add(1);
    let mut projected_sources = planner_future_mana_sources(
        &projected_zones,
        &context.mana_sources,
        &state.mana_pool,
        next_turn,
    );
    resolve_beginning_of_end_step_self_sacrifices(
        domain.deck,
        &mut projected_zones,
        &mut projected_sources,
    );
    let mut projected_turn_pool = state.mana_pool.clone();
    synchronize_turn_pool_with_battlefield(&mut projected_turn_pool, &projected_zones);
    synchronize_mana_sources_with_battlefield(&mut projected_sources, &projected_zones);

    let end_step_pool = opponent_end_step_payment_pool(
        &projected_turn_pool,
        &projected_zones,
        projected_turn_pool.remaining_treasures(),
    );
    let future_untapped_pool =
        future_untapped_mana_pool(domain.deck, &projected_zones, &projected_sources, next_turn);
    let available_library_copies = planner_public_library_multiset(domain.deck, state);
    best_opponent_end_step_top_tutor_projection(
        domain.deck,
        domain.mana_access,
        &state.hand,
        &available_library_copies,
        domain.turn,
        next_turn,
        &end_step_pool,
        &future_untapped_pool,
        &projected_zones,
        state.player_life,
        context.additional_generic_per_cast,
        context.first_relevant_spell_will_be_countered,
    )
}

fn projected_opponent_end_step_top_tutor_route_value(
    domain: &CastPlanningDomain<'_>,
    state: &CastPlanningState,
) -> Option<i64> {
    projected_opponent_end_step_top_tutor(domain, state)
        .map(|projection| projection.route_potential)
}

fn planner_prefers_opponent_end_step_top_tutor(
    domain: &CastPlanningDomain<'_>,
    hand: &[usize],
    mana_pool: &TurnManaPool,
) -> bool {
    let state = CastPlanningState {
        hand: hand.to_vec(),
        mana_pool: mana_pool.clone(),
        zones: domain.zones.clone(),
        player_life: domain.player_life,
        spells_cast_this_turn: domain.spells_cast_this_turn,
        planned_casts: Vec::new(),
        planned_actions: Vec::new(),
        stochastic_planner_expectation: None,
    };
    let Some(projected_route) = projected_opponent_end_step_top_tutor_route_value(domain, &state)
    else {
        return false;
    };
    let (current_value, _) = planner_value_with_development(
        domain.deck,
        domain.mana_access,
        hand,
        domain.zones,
        domain.turn,
        mana_pool,
        domain.additional_generic_per_cast,
        &[],
        0,
    );
    projected_route > current_value.route_deficit_reduction
}

fn eot_reservation_allows_candidate(
    preserve_eot_before_candidate: bool,
    preserves_eot_after_candidate: bool,
) -> bool {
    !preserve_eot_before_candidate || preserves_eot_after_candidate
}

#[derive(Debug, Clone)]
struct StagedCommanderPayment {
    mana_pool: TurnManaPool,
    zones: KnownLineZoneState,
    payment_choice: SpellPaymentChoice,
}

#[allow(clippy::too_many_arguments)]
fn stage_commander_payment_candidate(
    domain: &CastPlanningDomain<'_>,
    hand: &[usize],
    mana_pool: &TurnManaPool,
    commander_index: usize,
    commander_cost: Option<&ManaCostProfile>,
    commander_mana_value: u8,
    commander_tax: u8,
    generic_tax: u8,
    reserved_mana: u8,
    preserve_eot_before_candidate: bool,
    payment_choice: SpellPaymentChoice,
) -> Option<StagedCommanderPayment> {
    let mut candidate_pool = mana_pool.clone();
    let mut candidate_zones = domain.zones.clone();
    if !pay_spell_cost_choice(
        domain.deck,
        &mut candidate_zones,
        &mut candidate_pool,
        commander_index,
        commander_cost,
        commander_mana_value,
        commander_tax.saturating_add(generic_tax),
        reserved_mana,
        domain.turn,
        payment_choice,
        None,
    ) {
        return None;
    }
    let preserves_eot_after_candidate = preserve_eot_before_candidate && {
        let mut settled_pool = candidate_pool.clone();
        let mut settled_life = domain.player_life;
        settled_pool.settle_pending_source_damage(&mut settled_life) && {
            let mut opponent_end_step = domain.opponent_end_step.clone();
            if let Some(context) = opponent_end_step.as_mut() {
                context.additional_generic_per_cast =
                    context.additional_generic_per_cast.max(generic_tax);
            }
            let post_payment_domain = CastPlanningDomain {
                deck: domain.deck,
                mana_access: domain.mana_access,
                zones: &candidate_zones,
                turn: domain.turn,
                policy: domain.policy,
                additional_generic_per_cast: domain.additional_generic_per_cast.max(generic_tax),
                player_life: settled_life,
                spells_cast_this_turn: domain.spells_cast_this_turn,
                opponent_end_step,
            };
            planner_prefers_opponent_end_step_top_tutor(&post_payment_domain, hand, &settled_pool)
        }
    };
    eot_reservation_allows_candidate(preserve_eot_before_candidate, preserves_eot_after_candidate)
        .then_some(StagedCommanderPayment {
            mana_pool: candidate_pool,
            zones: candidate_zones,
            payment_choice,
        })
}

#[allow(clippy::too_many_arguments)]
fn best_staged_commander_payment_candidate(
    domain: &CastPlanningDomain<'_>,
    hand: &[usize],
    mana_pool: &TurnManaPool,
    commander_index: usize,
    commander_cost: Option<&ManaCostProfile>,
    commander_mana_value: u8,
    commander_tax: u8,
    generic_tax: u8,
    reserved_mana: u8,
    preserve_eot_before_candidate: bool,
) -> Option<StagedCommanderPayment> {
    [SpellPaymentChoice::Printed, SpellPaymentChoice::Alternative]
        .into_iter()
        .filter_map(|payment_choice| {
            stage_commander_payment_candidate(
                domain,
                hand,
                mana_pool,
                commander_index,
                commander_cost,
                commander_mana_value,
                commander_tax,
                generic_tax,
                reserved_mana,
                preserve_eot_before_candidate,
                payment_choice,
            )
        })
        .max_by_key(|candidate| {
            (
                combat_progress_after_current_attack(
                    domain.deck,
                    &candidate.zones,
                    &candidate.mana_pool,
                    domain.turn,
                    None,
                ),
                candidate.mana_pool.total(),
                candidate.payment_choice == SpellPaymentChoice::Printed,
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn commander_payment_at_generic_tax_checkpoint(
    isolated: &mut IsolatedScenarioRuntime,
    turn: u8,
    domain: &CastPlanningDomain<'_>,
    hand: &[usize],
    mana_pool: &TurnManaPool,
    commander_index: usize,
    commander_cost: Option<&ManaCostProfile>,
    commander_mana_value: u8,
    commander_tax: u8,
    reserved_mana: u8,
    preserve_eot_before_candidate: bool,
) -> Option<StagedCommanderPayment> {
    let pending_generic_tax =
        isolated.is(InteractionScenario::GenericTaxStax) && !isolated.applied();
    let active_generic_tax =
        u8::from(isolated.is(InteractionScenario::GenericTaxStax) && isolated.applied());
    let mut candidate = best_staged_commander_payment_candidate(
        domain,
        hand,
        mana_pool,
        commander_index,
        commander_cost,
        commander_mana_value,
        commander_tax,
        active_generic_tax,
        reserved_mana,
        preserve_eot_before_candidate,
    )?;
    isolated.observe_opportunity(InteractionScenario::GenericTaxStax);
    if pending_generic_tax {
        // Only a neutral, reservation-safe intended cast creates the real tax
        // checkpoint. Once reached, an unaffordable or newly reservation-
        // breaking taxed reprice remains an effectful intervention.
        isolated.activate(turn, 1);
        candidate = best_staged_commander_payment_candidate(
            domain,
            hand,
            mana_pool,
            commander_index,
            commander_cost,
            commander_mana_value,
            commander_tax,
            1,
            reserved_mana,
            preserve_eot_before_candidate,
        )?;
    }
    Some(candidate)
}

#[allow(clippy::too_many_arguments)]
fn best_tutor_target_position(
    deck: &CompiledDeck,
    instruction: TutorInstruction,
    library_order: &[usize],
    next_draw_position: usize,
    hand: &[usize],
    line_zones: &KnownLineZoneState,
    turn: u8,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
    library_top_is_next_modeled_draw: bool,
) -> Option<usize> {
    let unseen_start = next_draw_position.min(library_order.len());
    let available_library_copies = exact_card_multiset(&library_order[unseen_start..]);
    let target_index = best_tutor_target_identity(
        deck,
        instruction,
        &available_library_copies,
        hand,
        line_zones,
        turn,
        mana_access,
        mana_pool,
        future_additional_generic_per_cast,
        library_top_is_next_modeled_draw,
    )?;
    library_order
        .iter()
        .enumerate()
        .skip(unseen_start)
        .find_map(|(position, card_index)| (*card_index == target_index).then_some(position))
}

/// Choose an exact public search identity from the observable remaining
/// library multiset. Hidden positions never enter candidate enumeration or
/// tie-breaking; a runtime caller maps the chosen identity to one physical
/// unseen object only after the public choice has been made.
#[allow(clippy::too_many_arguments)]
fn best_tutor_target_identity(
    deck: &CompiledDeck,
    instruction: TutorInstruction,
    available_library_copies: &HashMap<usize, u16>,
    hand: &[usize],
    line_zones: &KnownLineZoneState,
    turn: u8,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
    library_top_is_next_modeled_draw: bool,
) -> Option<usize> {
    deck.cards
        .iter()
        .enumerate()
        .filter_map(|(card_index, candidate)| {
            if available_library_copies
                .get(&card_index)
                .copied()
                .unwrap_or_default()
                == 0
            {
                return None;
            }
            if !instruction.target.matches(candidate.effects.card_types) {
                return None;
            }
            if matches!(
                instruction.destination,
                TutorDestination::BattlefieldTapped | TutorDestination::BattlefieldUntapped
            ) && !candidate.effects.card_types.is_land
            {
                return None;
            }
            let route_rank = if instruction.destination == TutorDestination::Hand
                || instruction.destination == TutorDestination::LibraryTop
                    && library_top_is_next_modeled_draw
            {
                exact_tutor_reviewed_route_rank_with_library(
                    deck,
                    card_index,
                    hand,
                    line_zones,
                    turn,
                    mana_access,
                    mana_pool,
                    future_additional_generic_per_cast,
                    |index| {
                        available_library_copies
                            .get(&index)
                            .copied()
                            .map(usize::from)
                            .unwrap_or_default()
                    },
                )
            } else {
                ExactTutorReviewedRouteRank::default()
            };
            Some((
                card_index,
                route_rank,
                tutor_target_score(deck, candidate, hand, line_zones, turn),
                candidate.normalized_name.as_str(),
            ))
        })
        .max_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.cmp(&right.2))
                // Equal-value searches choose a stable public identity, not
                // whichever candidate happens to be earlier in hidden order.
                .then_with(|| right.3.cmp(left.3))
                .then_with(|| right.0.cmp(&left.0))
        })
        .map(|(card_index, _, _, _)| card_index)
}

/// Route-aware rank for an exact, observable library search.
///
/// The first three fields answer "how close is the earliest reviewed route
/// that this exact object advances?" The last two preserve optionality: a
/// shared hub is better than one branch when both make equal immediate
/// progress because it leaves more distinct live route cards as future outs.
/// Static role/card/MV scoring remains a caller-owned tie-break after this
/// complete key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct ExactTutorReviewedRouteRank {
    payable_completed_routes: u16,
    best_progress_basis_points: u16,
    best_payment_proximity: u8,
    distinct_remaining_outs: u16,
    advanced_routes: u16,
}

const EXACT_TUTOR_PAYMENT_PROBE_LIMIT: u8 = 24;

#[allow(clippy::too_many_arguments)]
fn exact_tutor_reviewed_route_rank_with_library(
    deck: &CompiledDeck,
    candidate_index: usize,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &TurnManaPool,
    future_additional_generic_per_cast: u8,
    available_library_copies: impl Fn(usize) -> usize,
) -> ExactTutorReviewedRouteRank {
    let Some(candidate) = deck.cards.get(candidate_index) else {
        return ExactTutorReviewedRouteRank::default();
    };
    let mut candidate_hand = hand.to_vec();
    candidate_hand.push(candidate_index);
    let mut rank = ExactTutorReviewedRouteRank::default();
    let mut remaining_out_indices = HashSet::<usize>::new();

    for line in deck
        .known_lines
        .iter()
        .filter(|line| reviewed_empty_library_sequence(line))
        .filter(|line| reviewed_sequence_zone_order_is_still_credible(line, deck, zones, turn))
    {
        let candidate_is_named = line
            .cards
            .iter()
            .any(|name| crate::parser::normalize_card_name(name) == candidate.normalized_name);
        if !candidate_is_named {
            continue;
        }
        let total = line.cards.len().max(1);
        let access_before = named_line_piece_access_count(line, deck, hand, zones, turn);
        let access_after =
            named_line_piece_access_count(line, deck, &candidate_hand, zones, turn).min(total);
        if access_after <= access_before {
            continue;
        }

        let mut required =
            line.cards
                .iter()
                .fold(HashMap::<String, usize>::new(), |mut counts, name| {
                    *counts
                        .entry(crate::parser::normalize_card_name(name))
                        .or_default() += 1;
                    counts
                });
        let mut hypothetical_complete_hand = candidate_hand.clone();
        let mut line_out_indices = Vec::new();
        let mut line_is_live = true;
        for (normalized_name, required_count) in required.drain() {
            let accessible = zones
                .usable_count(deck, &normalized_name, turn)
                .saturating_add(
                    candidate_hand
                        .iter()
                        .filter(|card_index| {
                            deck.cards
                                .get(**card_index)
                                .is_some_and(|card| card.normalized_name == normalized_name)
                        })
                        .count(),
                );
            let missing = required_count.saturating_sub(accessible);
            if missing == 0 {
                continue;
            }
            let mut matching_indices = deck
                .cards
                .iter()
                .enumerate()
                .filter(|(_, card)| card.normalized_name == normalized_name);
            let Some((missing_index, _)) = matching_indices.next() else {
                line_is_live = false;
                break;
            };
            if matching_indices.next().is_some() {
                line_is_live = false;
                break;
            }
            let remaining_copies = available_library_copies(missing_index)
                .saturating_sub(usize::from(missing_index == candidate_index));
            if remaining_copies < missing {
                line_is_live = false;
                break;
            }
            hypothetical_complete_hand.extend(std::iter::repeat_n(missing_index, missing));
            line_out_indices.push(missing_index);
        }
        if !line_is_live {
            continue;
        }

        let payment_deficit = (0..=EXACT_TUTOR_PAYMENT_PROBE_LIMIT).find(|flexible_deficit| {
            let mut candidate_pool = mana_pool.clone();
            candidate_pool.add_floating(
                ManaColorMask::ANY_COLOR | ManaColorMask::COLORLESS,
                *flexible_deficit,
            );
            reviewed_sequence_package_is_jointly_payable(
                line,
                deck,
                &hypothetical_complete_hand,
                zones,
                turn,
                &candidate_pool,
                mana_access,
                future_additional_generic_per_cast,
            )
        });
        let payment_proximity = payment_deficit
            .map(|deficit| {
                EXACT_TUTOR_PAYMENT_PROBE_LIMIT
                    .saturating_add(1)
                    .saturating_sub(deficit)
            })
            .unwrap_or_default();
        let progress = access_after
            .saturating_mul(10_000)
            .checked_div(total)
            .unwrap_or_default()
            .min(10_000) as u16;

        if progress > rank.best_progress_basis_points {
            rank.best_progress_basis_points = progress;
            rank.best_payment_proximity = payment_proximity;
        } else if progress == rank.best_progress_basis_points {
            rank.best_payment_proximity = rank.best_payment_proximity.max(payment_proximity);
        }
        if access_after == total && payment_deficit == Some(0) {
            rank.payable_completed_routes = rank.payable_completed_routes.saturating_add(1);
        }
        rank.advanced_routes = rank.advanced_routes.saturating_add(1);
        remaining_out_indices.extend(line_out_indices);
    }

    for line in &deck.known_lines {
        let Some(before) =
            graveyard_storm_planning_access(line, deck, hand, zones, &available_library_copies)
        else {
            continue;
        };
        if !before.program.members().contains(&candidate_index) {
            continue;
        }
        let Some(after) =
            graveyard_storm_planning_access(line, deck, &candidate_hand, zones, |card_index| {
                available_library_copies(card_index)
                    .saturating_sub(usize::from(card_index == candidate_index))
            })
        else {
            continue;
        };
        let access_before = before.supported_count();
        let access_after = after.supported_count();
        if access_after <= access_before {
            continue;
        }
        let progress = access_after
            .saturating_mul(10_000)
            .checked_div(3)
            .unwrap_or_default()
            .min(10_000) as u16;
        if progress > rank.best_progress_basis_points {
            rank.best_progress_basis_points = progress;
            // The progress-only probe deliberately does not claim that the
            // full escape recurrence is payable from planner state.
            rank.best_payment_proximity = 0;
        }
        rank.advanced_routes = rank.advanced_routes.saturating_add(1);
        remaining_out_indices.extend(after.missing_members());
    }

    rank.distinct_remaining_outs = remaining_out_indices.len().min(usize::from(u16::MAX)) as u16;
    rank
}

fn tutor_target_score(
    deck: &CompiledDeck,
    candidate: &CompiledCard,
    hand: &[usize],
    line_zones: &KnownLineZoneState,
    turn: u8,
) -> i32 {
    let line_score = deck
        .known_lines
        .iter()
        .filter_map(|line| {
            let required_copies = line
                .cards
                .iter()
                .filter(|name| {
                    crate::parser::normalize_card_name(name) == candidate.normalized_name
                })
                .count();
            if required_copies == 0 {
                return None;
            }
            let copies_in_hand = hand
                .iter()
                .filter(|index| {
                    deck.cards
                        .get(**index)
                        .is_some_and(|card| card.normalized_name == candidate.normalized_name)
                })
                .count();
            let usable_copies =
                line_zones.usable_count(deck, &candidate.normalized_name, turn) + copies_in_hand;
            if usable_copies >= required_copies {
                return None;
            }
            let other_pieces_available = line
                .cards
                .iter()
                .filter(|name| {
                    let normalized = crate::parser::normalize_card_name(name);
                    normalized != candidate.normalized_name
                        && (line_zones.usable_count(deck, &normalized, turn) > 0
                            || hand.iter().any(|index| {
                                deck.cards
                                    .get(*index)
                                    .is_some_and(|card| card.normalized_name == normalized)
                            }))
                })
                .count() as i32;
            Some(
                20_000
                    + i32::from(line.table_lethal_if_resolved) * 4_000
                    + other_pieces_available * 700
                    - i32::from(line.compactness) * 20,
            )
        })
        .max()
        .unwrap_or(0);
    let role_score = if candidate.has(role::COMBO_PIECE) {
        8_000
    } else if candidate.has(role::WIN_CONDITION) {
        7_000
    } else if candidate.has(role::PAYOFF) {
        6_000
    } else if candidate.has(role::ENGINE) {
        5_000
    } else if candidate.has(role::ENABLER) {
        4_500
    } else if candidate.has(role::RAMP | role::FAST_MANA)
        && card_has_executable_planner_mana_role(candidate)
    {
        3_000
    } else if candidate.has(role::DRAW) {
        2_500
    } else if candidate.has(role::PROTECTION) {
        1_500
    } else if candidate.has(role::LAND) {
        1_000
    } else {
        0
    };
    line_score
        .max(role_score)
        .saturating_sub((candidate.mana_value.max(0.0) * 10.0).round() as i32)
}

fn battlefield_tutored_land_source(
    deck: &CompiledDeck,
    mana_access: Option<&ManaAccessProfile>,
    card_index: usize,
    turn: u8,
    force_tapped: bool,
) -> BattlefieldManaSource {
    let mut source = battlefield_land_source(deck, mana_access, card_index, turn);
    if force_tapped {
        source.available_from_turn = turn.saturating_add(1);
    }
    source
}

fn immediate_effect_value(
    card: &CompiledCard,
    magnitude: EffectMagnitude,
    dynamic_fallback: u8,
) -> u8 {
    let value = magnitude.conservative_value(dynamic_fallback);
    if card.effects.repeatable && card.effects.conditional {
        0
    } else if card.effects.conditional {
        value.min(1)
    } else {
        value
    }
}

/// Returns only cards that a resolving spell explicitly draws.
///
/// `impulse_access` is descriptive metadata for look/reveal/exile-top text. It
/// does not prove that any card changes zones, much less that it enters the
/// hand. Exact typed executors own those movements. Treating scalar access as a
/// draw made pure reorder effects draw cards and made Ponder draw four.
fn immediate_cards_drawn(card: &CompiledCard) -> u8 {
    let typed_spell_draws = card
        .ability_program
        .executable_abilities()
        .filter(|ability| {
            ability.timing == AbilityTiming::SpellResolution
                && ability.costs.is_empty()
                && ability
                    .preconditions
                    .contains(&AbilityPrecondition::SourceZone(ProgramZone::Stack))
        })
        .flat_map(|ability| ability.effects.iter())
        .filter_map(|effect| match effect {
            AbilityEffect::Draw(draw)
                if !draw.optional && draw.unless_event_player_pays.is_none() =>
            {
                Some(draw.count)
            }
            _ => None,
        })
        .fold(0u16, u16::saturating_add)
        .min(8) as u8;
    if typed_spell_draws > 0 {
        return typed_spell_draws;
    }

    // A permanent's broad semantic DRAW role may describe an activated or
    // triggered ability. It is not evidence that casting the permanent draws
    // immediately. Retain the scalar compatibility fallback only for
    // resolving instants and sorceries whose exact draw program has not yet
    // been compiled.
    if card.effects.card_types.is_instant
        || card.effects.card_types.is_sorcery
        || card.has(role::INSTANT_SORCERY)
    {
        immediate_effect_value(card, card.effects.draw_cards, 1).min(8)
    } else {
        0
    }
}

fn resolve_immediate_spell_draws(
    card: &CompiledCard,
    hand: &mut Vec<usize>,
    library_order: &[usize],
    next_draw_position: &mut usize,
) {
    for _ in 0..immediate_cards_drawn(card) {
        if *next_draw_position >= library_order.len() {
            break;
        }
        hand.push(library_order[*next_draw_position]);
        *next_draw_position += 1;
    }
}

fn immediate_creature_tokens(card: &CompiledCard) -> u8 {
    immediate_effect_value(card, card.effects.creature_tokens, 1).min(12)
}

fn immediate_extra_turns(card: &CompiledCard) -> u8 {
    immediate_effect_value(card, card.effects.extra_turns, 1).min(2)
}

/// A reviewed Oracle-style spell is deliberately conserved until every named
/// member of at least one reviewed package is either usable this turn or in
/// hand and the remaining printed costs are jointly payable. This is a small,
/// versioned sequencing adapter, not a general combo executor.
#[allow(clippy::too_many_arguments)]
fn should_hold_reviewed_sequence_piece(
    deck: &CompiledDeck,
    card_index: usize,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: &TurnManaPool,
    mana_access: Option<&ManaAccessProfile>,
    additional_generic_per_cast: u8,
) -> bool {
    let Some(card) = deck.cards.get(card_index) else {
        return false;
    };
    if deck.known_lines.iter().any(|line| {
        compile_graveyard_storm_program(line, deck).is_some()
            && line
                .cards
                .iter()
                .any(|name| crate::parser::normalize_card_name(name) == card.normalized_name)
    }) {
        // The atomic executor stages every member together. Letting ordinary
        // planning spend the permission source or mill spell early can expire
        // the route. The exact whole-hand source is different: casting it does
        // not activate it, and putting the physical object onto the
        // battlefield before a random discard is required by the primer line.
        return exact_discard_sacrifice_mana_ability(card).is_none();
    }
    let candidate_lines = deck
        .known_lines
        .iter()
        .filter(|line| reviewed_empty_library_sequence(line))
        .filter(|line| {
            line.cards
                .iter()
                .any(|name| crate::parser::normalize_card_name(name) == card.normalized_name)
        })
        .collect::<Vec<_>>();
    !candidate_lines.is_empty()
        && !candidate_lines.iter().any(|line| {
            reviewed_sequence_package_is_jointly_payable(
                line,
                deck,
                hand,
                zones,
                turn,
                mana_pool,
                mana_access,
                additional_generic_per_cast,
            )
        })
}

fn reviewed_empty_library_sequence(line: &crate::domain::KnownLine) -> bool {
    line.simulation_requirements
        .contains(&LineRequirement::ReviewedEmptyLibrarySequence)
        && line
            .simulation_requirements
            .contains(&LineRequirement::NamedCardsPayPrintedCosts)
        && line.simulation_requirements.iter().all(|requirement| {
            matches!(
                requirement,
                LineRequirement::NamedCardsPayPrintedCosts
                    | LineRequirement::ReviewedEmptyLibrarySequence
                    | LineRequirement::SingletonLibrary
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn reviewed_sequence_package_is_jointly_payable(
    line: &crate::domain::KnownLine,
    deck: &CompiledDeck,
    hand: &[usize],
    zones: &KnownLineZoneState,
    turn: u8,
    mana_pool: &TurnManaPool,
    mana_access: Option<&ManaAccessProfile>,
    additional_generic_per_cast: u8,
) -> bool {
    if line.cards.is_empty()
        || (line
            .simulation_requirements
            .contains(&LineRequirement::SingletonLibrary)
            && deck.cards.iter().any(|card| card.quantity > 1))
    {
        return false;
    }
    let Some(mana_access) = mana_access else {
        // This reviewed route explicitly promises printed-cost execution.
        // Falling back to mana value would erase colored and non-generic
        // requirements, so absent cost data must fail closed.
        return false;
    };

    let mut required =
        line.cards
            .iter()
            .fold(HashMap::<String, usize>::new(), |mut counts, name| {
                *counts
                    .entry(crate::parser::normalize_card_name(name))
                    .or_default() += 1;
                counts
            });

    for (normalized_name, remaining) in &mut required {
        let Some(line_card) = unique_card_by_normalized_name(deck, normalized_name) else {
            return false;
        };
        let already_usable = match modeled_line_card_kind(line_card) {
            Some(ModeledLineCardKind::Permanent) => zones
                .battlefield
                .iter()
                .filter(|presence| {
                    presence.entered_turn == turn
                        && deck
                            .cards
                            .get(presence.card_index)
                            .is_some_and(|candidate| candidate.normalized_name == *normalized_name)
                })
                .count(),
            Some(ModeledLineCardKind::Spell) => zones
                .spells_cast_this_turn
                .iter()
                .filter(|cast| {
                    cast.turn == turn
                        && deck
                            .cards
                            .get(cast.card_index)
                            .is_some_and(|candidate| candidate.normalized_name == *normalized_name)
                })
                .count(),
            None => return false,
        };
        *remaining = remaining.saturating_sub(already_usable);
    }

    let mut remaining_hand_cards = Vec::new();
    for (normalized_name, required_count) in required {
        let matches = hand
            .iter()
            .copied()
            .filter(|index| {
                deck.cards
                    .get(*index)
                    .is_some_and(|candidate| candidate.normalized_name == normalized_name)
            })
            .take(required_count)
            .collect::<Vec<_>>();
        if matches.len() < required_count {
            return false;
        }
        remaining_hand_cards.extend(matches);
    }
    remaining_hand_cards.sort_by_key(|index| {
        match deck.cards.get(*index).and_then(modeled_line_card_kind) {
            Some(ModeledLineCardKind::Permanent) => 0,
            Some(ModeledLineCardKind::Spell) => 1,
            None => 2,
        }
    });

    let mut candidate_pool = mana_pool.clone();
    let mut candidate_zones = zones.clone();
    for card_index in remaining_hand_cards {
        let Some(cost) = mana_access
            .cost(card_index)
            .filter(|cost| activation_cost_is_exactly_modeled(cost))
        else {
            return false;
        };
        if !candidate_pool.pay_with_generic_adjustment(
            Some(cost),
            0,
            additional_generic_per_cast,
            generic_spell_cost_reduction(deck, &candidate_zones, card_index),
            0,
        ) {
            return false;
        }
        candidate_zones.record_cast(deck, card_index, turn);
    }
    true
}

/// Goldfish trajectories do not have legal opposing targets. Conserving a
/// purely reactive spell prevents counters, removal, wipes, and protection
/// from being converted into imaginary proactive board development. A broad
/// strategic role alone is not execution proof: only typed, target-independent
/// effects that this trajectory loop can actually resolve make the spell
/// eligible in a goldfish.
fn should_hold_reactive_card(card: &CompiledCard) -> bool {
    let reactive = card
        .has(role::REMOVAL | role::COUNTERSPELL | role::BOARD_WIPE | role::PROTECTION | role::STAX);
    let is_resolving_spell = card.effects.card_types.is_instant
        || card.effects.card_types.is_sorcery
        || card.has(role::INSTANT_SORCERY);
    let requires_reactive_target =
        card.effects.targeted_removal || card.has(role::COUNTERSPELL | role::PROTECTION);
    reactive
        && is_resolving_spell
        && (requires_reactive_target || !has_target_independent_executable_effect(card))
}

fn has_target_independent_executable_effect(card: &CompiledCard) -> bool {
    immediate_cards_drawn(card) > 0
        || compile_typed_necropotence_lifecycle(card).is_some()
        || card.effects.tutor.is_executable_on_spell_resolution()
        || immediate_effect_value(card, card.effects.lands_to_battlefield, 1) > 0
        || card.effects.mana_produced.conservative_value(1) > 0
            && matches!(
                card.effects.mana_production_kind,
                ManaProductionKind::SpellResolution
                    | ManaProductionKind::ReusableActivated
                    | ManaProductionKind::OneShotActivated
                    | ManaProductionKind::NonRefreshingActivated
            )
        || immediate_creature_tokens(card) > 0
        || immediate_effect_value(card, card.effects.treasure_tokens, 1) > 0
        || immediate_extra_turns(card) > 0
}

fn payable_hand_protection(
    deck: &CompiledDeck,
    hand: &[usize],
    mana_access: Option<&ManaAccessProfile>,
    mana_pool: &TurnManaPool,
) -> Option<(usize, TurnManaPool)> {
    hand.iter()
        .enumerate()
        .filter_map(|(hand_position, card_index)| {
            let card = deck.cards.get(*card_index)?;
            let is_reactive_spell = card.has(role::PROTECTION | role::COUNTERSPELL)
                && card.effects.card_types.is_instant;
            if !is_reactive_spell {
                return None;
            }
            let mut paid_pool = mana_pool.clone();
            let fallback = card.mana_value.ceil().max(0.0) as u8;
            paid_pool
                .pay(
                    mana_access.and_then(|access| access.cost(*card_index)),
                    fallback,
                    0,
                )
                .then_some((hand_position, *card_index, paid_pool))
        })
        .min_by(|(_, left_index, _), (_, right_index, _)| {
            deck.cards[*left_index]
                .mana_value
                .total_cmp(&deck.cards[*right_index].mana_value)
                .then_with(|| left_index.cmp(right_index))
        })
        .map(|(hand_position, _, paid_pool)| (hand_position, paid_pool))
}

fn consume_paid_hand_protection(
    hand: &mut Vec<usize>,
    mana_pool: &mut TurnManaPool,
    treasure_reserve: &mut u8,
    payment: Option<(usize, TurnManaPool)>,
) -> bool {
    let Some((hand_position, paid_pool)) = payment else {
        return false;
    };
    if hand_position >= hand.len() {
        return false;
    }
    hand.swap_remove(hand_position);
    *mana_pool = paid_pool;
    *treasure_reserve = mana_pool.remaining_treasures();
    true
}

/// Strategic role counters describe persistent battlefield state. Instants
/// and sorceries receive their typed resolution effects, but must never leave
/// permanent engine/protection/payoff counters behind after resolving.
fn card_has_persistent_body(card: &CompiledCard) -> bool {
    let types = card.effects.card_types;
    !types.is_land && !types.is_instant && !types.is_sorcery
}

fn commander_is_priority(card: &CompiledCard, policy: PilotPolicy) -> bool {
    match policy {
        PilotPolicy::Race => {
            card.has(role::ENABLER | role::PAYOFF | role::WIN_CONDITION | role::COMBO_PIECE)
        }
        PilotPolicy::Balanced => card.has(role::ENGINE | role::ENABLER | role::PAYOFF),
        PilotPolicy::Protect => {
            card.has(role::DRAW | role::PROTECTION | role::ENGINE | role::ENABLER)
        }
    }
}

fn commander_mana_reserve(card: &CompiledCard, policy: PilotPolicy) -> u8 {
    if matches!(policy, PilotPolicy::Protect)
        && card.has(
            role::ENGINE | role::ENABLER | role::PAYOFF | role::WIN_CONDITION | role::COMBO_PIECE,
        )
        && !card.has(role::DRAW | role::PROTECTION)
    {
        1
    } else {
        0
    }
}

fn apply_roles(
    roles: u32,
    engine_count: &mut u8,
    enabler_count: &mut u8,
    payoff_count: &mut u8,
    creature_count: &mut u8,
    protection_count: &mut u8,
) {
    if roles & role::ENGINE != 0 {
        *engine_count = engine_count.saturating_add(1);
    }
    if roles & role::ENABLER != 0 {
        *enabler_count = enabler_count.saturating_add(1);
    }
    if roles & role::PAYOFF != 0 {
        *payoff_count = payoff_count.saturating_add(1);
    }
    if roles & role::CREATURE != 0 {
        *creature_count = creature_count.saturating_add(1);
    }
    if roles & role::PROTECTION != 0 {
        *protection_count = protection_count.saturating_add(1);
    }
}

fn remove_roles(
    roles: u32,
    engine_count: &mut u8,
    enabler_count: &mut u8,
    payoff_count: &mut u8,
    creature_count: &mut u8,
    protection_count: &mut u8,
) {
    if roles & role::ENGINE != 0 {
        *engine_count = engine_count.saturating_sub(1);
    }
    if roles & role::ENABLER != 0 {
        *enabler_count = enabler_count.saturating_sub(1);
    }
    if roles & role::PAYOFF != 0 {
        *payoff_count = payoff_count.saturating_sub(1);
    }
    if roles & role::CREATURE != 0 {
        *creature_count = creature_count.saturating_sub(1);
    }
    if roles & role::PROTECTION != 0 {
        *protection_count = protection_count.saturating_sub(1);
    }
}

#[allow(clippy::too_many_arguments)]
fn remove_persistent_contributions(
    deck: &CompiledDeck,
    removed: &[usize],
    commanders_cast: &mut HashSet<usize>,
    engine_count: &mut u8,
    enabler_count: &mut u8,
    payoff_count: &mut u8,
    creature_count: &mut u8,
    protection_count: &mut u8,
    recursion_count: &mut u8,
) {
    for card_index in removed {
        let Some(card) = deck.cards.get(*card_index) else {
            continue;
        };
        remove_roles(
            card.roles,
            engine_count,
            enabler_count,
            payoff_count,
            creature_count,
            protection_count,
        );
        if card.effects.recursion {
            *recursion_count = recursion_count.saturating_sub(1);
        }
        commanders_cast.remove(card_index);
    }
}

#[allow(clippy::too_many_arguments)]
fn destroy_all_creatures_and_remove_persistent_contributions(
    deck: &CompiledDeck,
    line_zones: &mut KnownLineZoneState,
    commanders_cast: &mut HashSet<usize>,
    engine_count: &mut u8,
    enabler_count: &mut u8,
    payoff_count: &mut u8,
    creature_count: &mut u8,
    protection_count: &mut u8,
    recursion_count: &mut u8,
) -> CreatureDestructionResolution {
    let destruction = line_zones.destroy_all_creatures(deck);
    remove_persistent_contributions(
        deck,
        &destruction.removed_card_indices,
        commanders_cast,
        engine_count,
        enabler_count,
        payoff_count,
        creature_count,
        protection_count,
        recursion_count,
    );
    *creature_count = creature_count
        .saturating_sub(u8::try_from(destruction.removed_creature_tokens).unwrap_or(u8::MAX));
    destruction
}

fn scenario_targetable_permanent(card: &CompiledCard) -> bool {
    !card.is_commander
        && !card.has(role::LAND)
        && matches!(
            modeled_line_card_kind(card),
            Some(ModeledLineCardKind::Permanent)
        )
        && card.has(
            role::ENGINE
                | role::ENABLER
                | role::PAYOFF
                | role::COMBO_PIECE
                | role::STAX
                | role::CREATURE_MATTERS
                | role::ARTIFACT_MATTERS
                | role::ENCHANTMENT_MATTERS,
        )
}

fn scenario_relevant_spell(card: &CompiledCard) -> bool {
    !card.has(role::LAND)
        && card.has(
            role::RAMP
                | role::FAST_MANA
                | role::DRAW
                | role::TUTOR
                | role::ENGINE
                | role::ENABLER
                | role::PAYOFF
                | role::WIN_CONDITION
                | role::COMBO_PIECE
                | role::PROTECTION
                | role::CREATURE
                | role::ARTIFACT
                | role::ENCHANTMENT
                | role::INSTANT_SORCERY,
        )
}

fn scenario_graveyard_dependent(card: &CompiledCard) -> bool {
    card.effects.recursion || card.has(role::GRAVEYARD | role::RECURSION | role::DEATH_MATTERS)
}

fn scenario_executable_graveyard_action(card: &CompiledCard) -> bool {
    card.effects.recursion
        || card.ability_program.executable_abilities().any(|ability| {
            ability.effects.iter().any(|effect| {
                matches!(
                    effect,
                    AbilityEffect::GrantCastPermission(grant)
                        if grant.from == ProgramZone::Graveyard
                )
            })
        })
}

#[allow(clippy::too_many_arguments)]
fn apply_isolated_creature_wipe_if_ready(
    isolated: &mut IsolatedScenarioRuntime,
    deck: &CompiledDeck,
    turn: u8,
    line_zones: &mut KnownLineZoneState,
    commanders_cast: &mut HashSet<usize>,
    engine_count: &mut u8,
    enabler_count: &mut u8,
    payoff_count: &mut u8,
    creature_count: &mut u8,
    protection_count: &mut u8,
    recursion_count: &mut u8,
) {
    if *creature_count < 2 {
        return;
    }
    isolated.observe_opportunity(InteractionScenario::CreatureWipe);
    if !isolated.is(InteractionScenario::CreatureWipe) {
        return;
    }
    if isolated.applied() {
        isolated.recover(turn);
        return;
    }

    isolated.activate(turn, u32::from(*creature_count));
    destroy_all_creatures_and_remove_persistent_contributions(
        deck,
        line_zones,
        commanders_cast,
        engine_count,
        enabler_count,
        payoff_count,
        creature_count,
        protection_count,
        recursion_count,
    );
}

fn scenario_applicability(
    deck: &CompiledDeck,
    scenario: InteractionScenario,
) -> ScenarioApplicability {
    if deck.semantic_coverage < MIN_SCENARIO_SEMANTIC_COVERAGE {
        return ScenarioApplicability::Undetermined {
            reason: format!(
                "semantic coverage {:.3} is below the response-pressure scenario threshold {:.2}",
                deck.semantic_coverage, MIN_SCENARIO_SEMANTIC_COVERAGE
            ),
        };
    }
    if scenario == InteractionScenario::GraveyardShutdown {
        if deck
            .cards
            .iter()
            .any(|card| card.quantity > 0 && scenario_executable_graveyard_action(card))
        {
            return ScenarioApplicability::Applicable;
        }
        if deck
            .cards
            .iter()
            .any(|card| card.quantity > 0 && scenario_graveyard_dependent(card))
        {
            return ScenarioApplicability::Undetermined {
                reason: "graveyard dependency is present, but no graveyard action has an executable response-pressure effect".into(),
            };
        }
    }

    let applicable = match scenario {
        InteractionScenario::TargetedPermanentRemoval => deck
            .cards
            .iter()
            .any(|card| card.quantity > 0 && scenario_targetable_permanent(card)),
        InteractionScenario::CommanderRemovalRecast => !deck.commanders.is_empty(),
        InteractionScenario::FirstRelevantSpellCountered => deck
            .cards
            .iter()
            .any(|card| card.quantity > 0 && scenario_relevant_spell(card)),
        InteractionScenario::CreatureWipe => deck.cards.iter().any(|card| {
            card.quantity > 0
                && card.has(
                    role::CREATURE | role::TOKEN | role::TOKEN_MATTERS | role::CREATURE_MATTERS,
                )
        }),
        InteractionScenario::GraveyardShutdown => deck
            .cards
            .iter()
            .any(|card| card.quantity > 0 && scenario_executable_graveyard_action(card)),
        InteractionScenario::GenericTaxStax => deck
            .cards
            .iter()
            .any(|card| !card.has(role::LAND) && card.quantity > 0),
        InteractionScenario::RuleOfLawCap => {
            deck.cards
                .iter()
                .filter(|card| card.quantity > 0 && !card.has(role::LAND))
                .fold(0u32, |count, card| {
                    count.saturating_add(u32::from(card.quantity))
                })
                >= 2
        }
        InteractionScenario::FirstWinAttemptStopped => {
            deck.known_lines
                .iter()
                .any(|line| line.table_lethal_if_resolved)
                || deck_has_executable_combat_route(deck)
        }
    };
    if applicable {
        ScenarioApplicability::Applicable
    } else {
        ScenarioApplicability::NotApplicable {
            reason: match scenario {
                InteractionScenario::TargetedPermanentRemoval => {
                    InapplicabilityReason::NoEligibleNoncommanderPermanent
                }
                InteractionScenario::CommanderRemovalRecast => {
                    InapplicabilityReason::NoCommanderSubject
                }
                InteractionScenario::FirstRelevantSpellCountered => {
                    InapplicabilityReason::NoRelevantSpellClass
                }
                InteractionScenario::CreatureWipe => {
                    InapplicabilityReason::NoRelevantCreatureBoardPlan
                }
                InteractionScenario::GraveyardShutdown => {
                    InapplicabilityReason::NoGraveyardDependency
                }
                InteractionScenario::GenericTaxStax => InapplicabilityReason::NoTaxableActionClass,
                InteractionScenario::RuleOfLawCap => InapplicabilityReason::NoMultispellPlan,
                InteractionScenario::FirstWinAttemptStopped => {
                    InapplicabilityReason::NoRepresentableWinAttempt
                }
            },
        }
    }
}

fn collect_outcome(aggregate: &mut ScenarioAggregate, outcome: EpisodeOutcome) {
    let model_pace_turn = [
        outcome.first_win_attempt_turn,
        outcome.timing_provenance.first_generic_milestone_turn,
    ]
    .into_iter()
    .flatten()
    .min();
    if let Some(turn) = outcome.threat_turn {
        aggregate.threat_turns.push(turn);
    }
    if let Some(turn) = outcome.first_win_attempt_turn {
        debug_assert!(
            outcome
                .threat_turn
                .is_some_and(|threat_turn| turn >= threat_turn),
            "a modeled win attempt must not precede its credible threat"
        );
        debug_assert!(
            outcome
                .timing_provenance
                .first_explicit_route_index
                .is_some(),
            "an explicit win attempt must identify its recognized route"
        );
        aggregate.win_attempt_turns.push(turn);
        if let Some(line_index) = outcome.timing_provenance.first_explicit_route_index {
            aggregate
                .explicit_route_attempt_turns
                .entry(line_index)
                .or_default()
                .push(turn);
        }
    }
    if let Some(turn) = outcome.timing_provenance.first_generic_milestone_turn {
        aggregate.generic_conversion_milestone_turns.push(turn);
        if let Some(kind) = outcome.timing_provenance.first_generic_milestone_kind {
            *aggregate
                .generic_milestone_kind_counts
                .entry(kind)
                .or_default() += 1;
        }
    }
    if let Some(turn) = model_pace_turn {
        aggregate.model_pace_turns.push(turn);
    }
    for (position, blocker) in outcome
        .timing_provenance
        .early_turn_blockers
        .into_iter()
        .enumerate()
    {
        if let Some(blocker) = blocker {
            let turn = position.saturating_add(1).min(usize::from(u8::MAX)) as u8;
            *aggregate
                .early_turn_blocker_counts
                .entry((
                    turn,
                    blocker.line_index,
                    blocker.missing_card_position,
                    blocker.reason,
                ))
                .or_default() += 1;
        }
    }
    if let Some(turn) = outcome.resolved_table_win_turn {
        debug_assert!(
            outcome
                .first_win_attempt_turn
                .is_some_and(|attempt_turn| turn >= attempt_turn),
            "a resolved table win must not precede its modeled win attempt"
        );
        aggregate.resolved_table_win_turns.push(turn);
    }
    if outcome.first_attempt_stopped {
        aggregate.stopped_attempts += 1;
    }
    if outcome.first_attempt_opportunity {
        aggregate.first_attempt_opportunities += 1;
    }
    if outcome.recovered {
        aggregate.recovered_attempts += 1;
    }
}

fn build_attempt_provenance_report(
    deck: &CompiledDeck,
    baseline: &ScenarioAggregate,
    interfered: &ScenarioAggregate,
    simulations: u32,
    maximum_turn: u8,
) -> AttemptProvenanceReport {
    let mut explicit_routes = deck
        .known_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.table_lethal_if_resolved)
        .map(|(line_index, line)| {
            let baseline_turns = baseline
                .explicit_route_attempt_turns
                .get(&line_index)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let interfered_turns = interfered
                .explicit_route_attempt_turns
                .get(&line_index)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let denominator = simulations.max(1) as f32;
            ExplicitAttemptRouteReport {
                route_id: explicit_route_id(line),
                name: line.name.clone(),
                cards: line.cards.clone(),
                prerequisites: line.prerequisites.clone(),
                model_confidence: line.model_confidence,
                baseline_attempts: baseline_turns.len().min(u32::MAX as usize) as u32,
                interfered_attempts: interfered_turns.len().min(u32::MAX as usize) as u32,
                baseline_rate: baseline_turns.len() as f32 / denominator,
                interfered_rate: interfered_turns.len() as f32 / denominator,
                baseline_first_attempt: distribution(baseline_turns, simulations),
                interfered_first_attempt: distribution(interfered_turns, simulations),
                cumulative_baseline_attempt_rate: cumulative_rates(
                    baseline_turns,
                    simulations,
                    maximum_turn,
                ),
                cumulative_interfered_attempt_rate: cumulative_rates(
                    interfered_turns,
                    simulations,
                    maximum_turn,
                ),
            }
        })
        .collect::<Vec<_>>();
    if deck_has_executable_combat_route(deck) {
        let baseline_turns = baseline
            .explicit_route_attempt_turns
            .get(&COMBAT_DAMAGE_ROUTE_INDEX)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let interfered_turns = interfered
            .explicit_route_attempt_turns
            .get(&COMBAT_DAMAGE_ROUTE_INDEX)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let denominator = simulations.max(1) as f32;
        explicit_routes.push(ExplicitAttemptRouteReport {
            route_id: combat_damage_route_id().into(),
            name: combat_damage_route_name().into(),
            cards: deck
                .commanders
                .iter()
                .filter_map(|index| deck.cards.get(*index))
                .map(|card| card.name.clone())
                .collect(),
            prerequisites: vec![
                "Attack-capable creatures with known combat power".into(),
                "Assigned combat damage connects to every remaining opponent".into(),
                "Each opponent reaches 0 life or 21 combat damage from the tracked commander"
                    .into(),
            ],
            model_confidence: 0.75,
            baseline_attempts: baseline_turns.len().min(u32::MAX as usize) as u32,
            interfered_attempts: interfered_turns.len().min(u32::MAX as usize) as u32,
            baseline_rate: baseline_turns.len() as f32 / denominator,
            interfered_rate: interfered_turns.len() as f32 / denominator,
            baseline_first_attempt: distribution(baseline_turns, simulations),
            interfered_first_attempt: distribution(interfered_turns, simulations),
            cumulative_baseline_attempt_rate: cumulative_rates(
                baseline_turns,
                simulations,
                maximum_turn,
            ),
            cumulative_interfered_attempt_rate: cumulative_rates(
                interfered_turns,
                simulations,
                maximum_turn,
            ),
        });
    }

    let generic_milestone_kinds = [
        GenericMilestoneKind::Engine,
        GenericMilestoneKind::Combat,
        GenericMilestoneKind::EngineAndCombat,
    ]
    .into_iter()
    .map(|kind| {
        let baseline_episodes = baseline
            .generic_milestone_kind_counts
            .get(&kind)
            .copied()
            .unwrap_or_default();
        let interfered_episodes = interfered
            .generic_milestone_kind_counts
            .get(&kind)
            .copied()
            .unwrap_or_default();
        let denominator = simulations.max(1) as f32;
        GenericMilestoneKindReport {
            kind,
            baseline_episodes,
            interfered_episodes,
            baseline_rate: baseline_episodes as f32 / denominator,
            interfered_rate: interfered_episodes as f32 / denominator,
        }
    })
    .collect();

    let mut early_turn_blockers = Vec::new();
    append_early_turn_blocker_reports(
        &mut early_turn_blockers,
        TimingSampleKind::Baseline,
        deck,
        baseline,
        simulations,
    );
    append_early_turn_blocker_reports(
        &mut early_turn_blockers,
        TimingSampleKind::Interfered,
        deck,
        interfered,
        simulations,
    );

    AttemptProvenanceReport {
        explicit_routes,
        generic_milestone_kinds,
        early_failure_horizon: maximum_turn.min(EARLY_ATTEMPT_DIAGNOSTIC_HORIZON),
        early_turn_blockers,
    }
}

fn append_early_turn_blocker_reports(
    reports: &mut Vec<EarlyTurnAttemptBlockerReport>,
    sample: TimingSampleKind,
    deck: &CompiledDeck,
    aggregate: &ScenarioAggregate,
    simulations: u32,
) {
    let mut counts = aggregate
        .early_turn_blocker_counts
        .iter()
        .map(
            |((turn, line_index, missing_card_position, reason), episodes)| {
                (
                    *turn,
                    *line_index,
                    *missing_card_position,
                    *reason,
                    *episodes,
                )
            },
        )
        .collect::<Vec<_>>();
    counts.sort_by_key(|(turn, line_index, missing_card_position, reason, _)| {
        (
            *turn,
            line_index.unwrap_or(usize::MAX),
            missing_card_position.unwrap_or(u8::MAX),
            explicit_attempt_blocker_reason_rank(*reason),
        )
    });
    let denominator = simulations.max(1) as f32;
    reports.extend(counts.into_iter().map(
        |(turn, line_index, missing_card_position, reason, episodes)| {
            let line = line_index.and_then(|index| deck.known_lines.get(index));
            let is_combat_route = line_index == Some(COMBAT_DAMAGE_ROUTE_INDEX);
            EarlyTurnAttemptBlockerReport {
                sample,
                turn,
                route_id: if is_combat_route {
                    Some(combat_damage_route_id().into())
                } else {
                    line.map(explicit_route_id)
                },
                route_name: if is_combat_route {
                    Some(combat_damage_route_name().into())
                } else {
                    line.map(|line| line.name.clone())
                },
                blocked_card: line.and_then(|line| {
                    missing_card_position
                        .and_then(|position| line.cards.get(usize::from(position)))
                        .cloned()
                }),
                reason,
                episodes,
                rate: episodes as f32 / denominator,
            }
        },
    ));
}

fn combat_damage_route_id() -> &'static str {
    "rules-combat:life-or-commander-damage/v1"
}

fn combat_damage_route_name() -> &'static str {
    "Combat damage (40 life / 21 commander damage)"
}

fn explicit_route_id(line: &crate::domain::KnownLine) -> String {
    let normalized_name = crate::parser::normalize_card_name(&line.name).replace(' ', "-");
    let normalized_cards = line
        .cards
        .iter()
        .map(|card| crate::parser::normalize_card_name(card).replace(' ', "-"))
        .collect::<Vec<_>>()
        .join("+");
    format!(
        "known-line:{normalized_name}:{normalized_cards}:{}",
        line.compactness
    )
}

fn explicit_attempt_blocker_reason_rank(reason: ExplicitAttemptBlockerReason) -> u8 {
    match reason {
        ExplicitAttemptBlockerReason::NoRecognizedExplicitRoute => 0,
        ExplicitAttemptBlockerReason::MissingNamedPieces => 1,
        ExplicitAttemptBlockerReason::NamedPiecesNotUsableTogether => 2,
        ExplicitAttemptBlockerReason::InsufficientNamedCardMana => 3,
        ExplicitAttemptBlockerReason::UnsupportedRequirement => 4,
        ExplicitAttemptBlockerReason::UnmetPrerequisite => 5,
        ExplicitAttemptBlockerReason::UnsupportedActivationCost => 6,
        ExplicitAttemptBlockerReason::InsufficientActivationMana => 7,
        ExplicitAttemptBlockerReason::DeferredAfterStoppedAttempt => 8,
        ExplicitAttemptBlockerReason::ReadyButNotSelected => 9,
    }
}

fn paired_delay_report(
    baseline: &[EpisodeOutcome],
    stressed: &[EpisodeOutcome],
    endpoint: impl Fn(&EpisodeOutcome) -> Option<u8>,
) -> PairedTurnDelayReport {
    debug_assert_eq!(
        baseline.len(),
        stressed.len(),
        "paired scenarios must retain one outcome per shared episode seed"
    );
    let mut observed_delays = Vec::<i16>::new();
    let mut prevented_by_turn_cap = 0u32;
    let mut baseline_not_demonstrated = 0u32;
    let mut stressed_only = 0u32;

    for (base, disrupted) in baseline.iter().zip(stressed) {
        match (endpoint(base), endpoint(disrupted)) {
            (Some(base_turn), Some(disrupted_turn)) => {
                observed_delays.push(i16::from(disrupted_turn) - i16::from(base_turn));
            }
            (Some(_), None) => prevented_by_turn_cap += 1,
            (None, None) => baseline_not_demonstrated += 1,
            (None, Some(_)) => stressed_only += 1,
        }
    }
    observed_delays.sort_unstable();

    PairedTurnDelayReport {
        observed_pairs: observed_delays.len().min(u32::MAX as usize) as u32,
        prevented_by_turn_cap,
        baseline_not_demonstrated,
        stressed_only,
        median: signed_percentile(&observed_delays, 0.50),
        p10: signed_percentile(&observed_delays, 0.10),
        p90: signed_percentile(&observed_delays, 0.90),
    }
}

fn cumulative_rates(turns: &[u8], simulations: u32, maximum_turn: u8) -> Vec<TurnRate> {
    (1..=maximum_turn)
        .map(|turn| TurnRate {
            turn,
            rate: turns
                .iter()
                .filter(|demonstrated| **demonstrated <= turn)
                .count() as f32
                / simulations.max(1) as f32,
        })
        .collect()
}

fn distribution(turns: &[u8], simulations: u32) -> TurnDistribution {
    let simulations = simulations.max(1);
    let demonstrated_rate = turns.len() as f32 / simulations as f32;
    let right_censored_rate = (1.0 - demonstrated_rate).clamp(0.0, 1.0);
    if turns.is_empty() {
        return TurnDistribution {
            median: None,
            p10: None,
            p90: None,
            conditional_median: None,
            conditional_p10: None,
            conditional_p90: None,
            demonstrated_rate: 0.0,
            right_censored_rate: 1.0,
        };
    }
    let mut ordered = turns.to_vec();
    ordered.sort_unstable();
    TurnDistribution {
        median: population_percentile(&ordered, simulations, 0.50),
        p10: population_percentile(&ordered, simulations, 0.10),
        p90: population_percentile(&ordered, simulations, 0.90),
        conditional_median: percentile(&ordered, 0.50),
        conditional_p10: percentile(&ordered, 0.10),
        conditional_p90: percentile(&ordered, 0.90),
        demonstrated_rate,
        right_censored_rate,
    }
}

fn population_percentile(
    ordered_demonstrated_turns: &[u8],
    simulations: u32,
    probability: f32,
) -> Option<f32> {
    if ordered_demonstrated_turns.is_empty() || simulations == 0 {
        return None;
    }
    // Unobserved episodes are known only to occur after the turn cap. A
    // nearest-rank population quantile is identifiable exactly when enough
    // observed episodes exist to reach that rank.
    let rank = (probability.clamp(0.0, 1.0) * simulations as f32)
        .ceil()
        .max(1.0) as usize;
    ordered_demonstrated_turns
        .get(rank.saturating_sub(1))
        .copied()
        .map(f32::from)
}

fn percentile(ordered: &[u8], probability: f32) -> Option<f32> {
    if ordered.is_empty() {
        return None;
    }
    let position = probability * (ordered.len() - 1) as f32;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position - lower as f32;
    Some(ordered[lower] as f32 * (1.0 - fraction) + ordered[upper] as f32 * fraction)
}

fn signed_percentile(ordered: &[i16], probability: f32) -> Option<f32> {
    if ordered.is_empty() {
        return None;
    }
    let position = probability * (ordered.len() - 1) as f32;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let fraction = position - lower as f32;
    Some(ordered[lower] as f32 * (1.0 - fraction) + ordered[upper] as f32 * fraction)
}

fn build_stress_tests(deck: &CompiledDeck, observed_delay: Option<f32>) -> Vec<StressTestResult> {
    let commander_delay = 0.5 + deck.synergy.commander_dependence * 2.4;
    let graveyard_cards = deck
        .cards
        .iter()
        .filter(|card| card.has(role::GRAVEYARD | role::RECURSION))
        .map(|card| card.quantity as u32)
        .sum::<u32>();
    let creature_cards = deck
        .cards
        .iter()
        .filter(|card| card.has(role::CREATURE))
        .map(|card| card.quantity as u32)
        .sum::<u32>();

    vec![
        StressTestResult {
            name: "Commander removed once".into(),
            outcome: format!("Estimated +{commander_delay:.1} turns to rebuild"),
            severity: if commander_delay >= 2.0 {
                IssueSeverity::Warning
            } else {
                IssueSeverity::Info
            },
        },
        StressTestResult {
            name: "First modeled win attempt stopped".into(),
            outcome: match observed_delay {
                Some(delay) if delay > 0.0 => {
                    format!("Observed median attempt delay: +{delay:.1} turns")
                }
                Some(_) => "No measurable median attempt delay in this sample".into(),
                None => "No paired attempt-delay observation in this sample".into(),
            },
            severity: IssueSeverity::Info,
        },
        StressTestResult {
            name: "Board wipe on turn five".into(),
            outcome: if creature_cards >= 28 {
                "Material impact; the primary plan commits heavily to the battlefield".into()
            } else {
                "Moderate impact under the abstract battlefield model".into()
            },
            severity: if creature_cards >= 28 {
                IssueSeverity::Warning
            } else {
                IssueSeverity::Info
            },
        },
        StressTestResult {
            name: "Graveyard disabled".into(),
            outcome: if graveyard_cards >= 12 {
                "Primary plan is significantly impaired".into()
            } else if graveyard_cards >= 5 {
                "Several value lines are impaired".into()
            } else {
                "Low modeled dependency".into()
            },
            severity: if graveyard_cards >= 12 {
                IssueSeverity::Warning
            } else {
                IssueSeverity::Info
            },
        },
    ]
}

fn derive_episode_seed(master: u64, scenario: u64, simulation_index: u32) -> u64 {
    let mut value = master
        ^ scenario.rotate_left(17)
        ^ (simulation_index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}
