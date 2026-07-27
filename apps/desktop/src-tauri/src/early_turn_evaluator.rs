//! Deterministic early-turn cEDH route-skeleton evaluation.
//!
//! This module deliberately does not estimate a generic "engine attempt".
//! It starts from explicit, table-lethal known lines and exactly enumerates
//! phase-partitioned opening-hand-plus-natural-draw combinations. Cards within
//! each phase are unordered, while the opening candidate remains distinct from
//! later draws. The result answers
//! three narrower questions:
//!
//! 1. Were all named route pieces naturally visible by a turn?
//! 2. Could a typed library tutor substitute for the missing pieces?
//! 3. Did the visible cards contain a conservative scalar mana-capacity floor?
//!
//! None of those questions proves that a legal ordered line can be executed.
//! Colored payments, zone setup, priority, protection, and opponent responses
//! remain explicit blockers until the trajectory executor supplies witnesses.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ability_program::{
    AbilityCost, AbilityEffect, AbilityTiming, AtomicEffect, AtomicInitiation, CardType,
    FixedManaProfile, ManaKind, ObjectFilter, SelfTransferTutorResolutionStep, TutorFilter, Zone,
};
use crate::domain::{KnownLine, KnownLineOutcome, LineRequirement};
use crate::effects::{ManaProductionKind, TutorDestination, TutorSourceZone};
use crate::mana::{EntersTapped, ManaModel, ManaSourceProfile, parse_mana_cost};
use crate::parser::normalize_card_name;
use crate::semantics::{CompiledCard, CompiledDeck, role};

pub(crate) const EARLY_TURN_EVALUATOR_VERSION: &str = "early-turn-route-skeleton/v5";

const OPENING_HAND_SIZE: u8 = 7;
const NATURAL_DRAWS_BEFORE_TURN_ONE: u8 = 1;
const NATURAL_DRAWS_BEFORE_TURN_TWO: u8 = 2;
const AGGRESSIVE_CANDIDATE_HANDS: u8 = 4;
const MAX_ROUTE_PIECE_SLOTS: usize = 12;
const MAX_TRACKED_MANA: u8 = 48;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EarlyTurnEvaluationReport {
    pub model_version: String,
    pub library_size: u16,
    pub known_line_count: u16,
    pub eligible_table_win_route_count: u16,
    pub omitted_non_table_win_line_count: u16,
    pub fixed_policy: EarlyTurnPolicy,
    pub routes: Vec<RouteReadinessReport>,
    pub blockers: Vec<EarlyTurnBlocker>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EarlyTurnPolicy {
    /// Cards in every London-mulligan candidate hand before bottoming.
    pub opening_hand_size: u8,
    /// Commander multiplayer's ordinary natural draws visible by turns one
    /// and two. The evaluator is intentionally fixed rather than exposing a
    /// user-selectable play-style setting.
    pub natural_draws_before_turn_one: u8,
    pub natural_draws_before_turn_two: u8,
    /// Fixed count used by the independent-reshuffle diagnostic envelope.
    /// This is not the production simulator's stop-on-keep London-mulligan
    /// probability.
    pub aggressive_candidate_hands: u8,
}

impl Default for EarlyTurnPolicy {
    fn default() -> Self {
        Self {
            opening_hand_size: OPENING_HAND_SIZE,
            natural_draws_before_turn_one: NATURAL_DRAWS_BEFORE_TURN_ONE,
            natural_draws_before_turn_two: NATURAL_DRAWS_BEFORE_TURN_TWO,
            aggressive_candidate_hands: AGGRESSIVE_CANDIDATE_HANDS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RouteReadinessReport {
    pub route_id: String,
    pub route_name: String,
    pub outcome: KnownLineOutcome,
    pub table_lethal_if_resolved: bool,
    pub model_confidence: f32,
    pub pieces: Vec<RoutePieceRequirement>,
    pub mana_demand: Option<ScalarManaDemand>,
    pub modeling_ceiling: RouteModelingCeiling,
    pub aggressive_mulligan: AggressiveMulliganEnvelope,
    pub turns: Vec<TurnRouteReadiness>,
    pub blockers: Vec<EarlyTurnBlocker>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoutePieceRequirement {
    pub card: String,
    pub normalized_card: String,
    pub required_copies: u8,
    pub command_zone_copies: u8,
    pub required_library_copies: u8,
    pub available_library_copies: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScalarManaDemand {
    pub amount: u8,
    pub includes_colored_or_colorless_pips: bool,
    pub basis: String,
    pub exact_printed_cost_coverage: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RouteModelingCeiling {
    Unavailable,
    RouteSkeletonOnly,
    RouteSkeletonWithScalarResourceFloor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AggressiveMulliganEnvelope {
    pub candidate_hands: u8,
    pub direct_skeleton_in_at_least_one_candidate: f64,
    pub typed_tutor_skeleton_in_at_least_one_candidate: f64,
    pub scalar_floor_skeleton_in_at_least_one_candidate: Option<f64>,
    pub caveat: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnRouteReadiness {
    pub turn: u8,
    pub visible_library_cards: u8,
    /// Decimal strings avoid silently losing exact phase-partitioned
    /// combination counts in a JavaScript consumer if the evaluator is later
    /// extended past turn two.
    pub total_combinations: String,
    pub direct_skeleton_combinations: String,
    pub typed_tutor_skeleton_combinations: String,
    pub strict_scalar_floor_combinations: Option<String>,
    pub conditional_scalar_ceiling_combinations: Option<String>,
    pub direct_skeleton_probability: f64,
    pub typed_tutor_skeleton_probability: f64,
    pub strict_scalar_floor_probability: Option<f64>,
    pub conditional_scalar_ceiling_probability: Option<f64>,
    /// Intentionally absent until an ordered trajectory supplies a legal,
    /// route-specific witness.
    pub executable_conversion_probability: Option<f64>,
    pub blockers: Vec<EarlyTurnBlocker>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EarlyTurnBlocker {
    pub category: EarlyTurnBlockerCategory,
    pub detail: String,
    pub affected_cards: Vec<String>,
    /// Exact probability mass only when the category is directly measurable
    /// from the combination enumeration. `None` means a modeling boundary,
    /// not zero impact.
    pub probability_mass: Option<f64>,
    pub prevents_executable_claim: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum EarlyTurnBlockerCategory {
    NoExplicitTableWinRoute,
    RouteTooLarge,
    MissingRouteCard,
    InsufficientRouteCopies,
    DirectPieceAccess,
    UnresolvedPieceAccess,
    TutorPaymentOrTiming,
    TutorShapeUnsupported,
    ManaDemandUnresolved,
    ScalarManaCapacity,
    ConditionalManaDependency,
    ColoredPaymentUnverified,
    CommandZoneCastUnverified,
    ZoneOrSequenceUnverified,
    PrerequisiteExecutionUnverified,
    UnsupportedCardFunction,
    InconsistentRouteMetadata,
}

/// Evaluates every explicit table-lethal known line under one fixed,
/// aggressive cEDH policy. There is intentionally no play-style parameter.
pub(crate) fn evaluate_early_turn_routes(
    deck: &CompiledDeck,
    mana_model: &ManaModel,
) -> EarlyTurnEvaluationReport {
    let fixed_policy = EarlyTurnPolicy::default();
    let mut routes = Vec::new();
    let mut report_blockers = Vec::new();
    let mut omitted_non_table_win_line_count = 0u16;

    for line in &deck.known_lines {
        if !line.table_lethal_if_resolved {
            omitted_non_table_win_line_count = omitted_non_table_win_line_count.saturating_add(1);
            continue;
        }
        match evaluate_route(deck, mana_model, line, &fixed_policy) {
            Ok(route) => routes.push(route),
            Err(blocker) => report_blockers.push(blocker),
        }
    }

    if routes.is_empty() {
        report_blockers.push(blocker(
            EarlyTurnBlockerCategory::NoExplicitTableWinRoute,
            "No explicit known line is marked table-lethal if resolved; generic engine density is not substituted for a win route.",
            Vec::new(),
            None,
            true,
        ));
    }

    routes.sort_by(|left, right| left.route_id.cmp(&right.route_id));
    report_blockers.sort_by(|left, right| {
        blocker_sort_key(left)
            .cmp(&blocker_sort_key(right))
            .then_with(|| left.detail.cmp(&right.detail))
    });

    let mut notes = vec![
        "Probabilities are exact over unordered library-card combinations; no random sampler is used.".into(),
        "Typed tutors widen card access only. Library-top tutors count for turn-two access only when present in the opening seven; their casting payment and the ordered top-deck sequence still require a witness.".into(),
        "The scalar mana floor is a fail-closed capacity diagnostic, not colored payment or legal sequencing proof.".into(),
        "Opponent interaction, protection, priority, and table conversion are intentionally outside this route-skeleton module.".into(),
    ];
    let exact_delayed_access_slots = deck
        .library
        .iter()
        .filter_map(|index| deck.cards.get(*index))
        .filter(|card| {
            card.ability_program
                .executable_necropotence_lifecycle()
                .is_some()
        })
        .count();
    if exact_delayed_access_slots > 0 {
        notes.push(format!(
            "{exact_delayed_access_slots} library card slot(s) have an exact typed delayed card-access/development lifecycle. Because that access is nonselective and arrives at the next end step, it does not masquerade as a tutor or widen named-route probabilities."
        ));
    }

    EarlyTurnEvaluationReport {
        model_version: EARLY_TURN_EVALUATOR_VERSION.into(),
        library_size: deck.library.len().min(u16::MAX as usize) as u16,
        known_line_count: deck.known_lines.len().min(u16::MAX as usize) as u16,
        eligible_table_win_route_count: routes.len().min(u16::MAX as usize) as u16,
        omitted_non_table_win_line_count,
        fixed_policy,
        routes,
        blockers: report_blockers,
        notes,
    }
}

fn evaluate_route(
    deck: &CompiledDeck,
    mana_model: &ManaModel,
    line: &KnownLine,
    policy: &EarlyTurnPolicy,
) -> Result<RouteReadinessReport, EarlyTurnBlocker> {
    let required_by_name = required_piece_counts(line);
    let required_slots = required_by_name
        .values()
        .map(|(_, count)| usize::from(*count))
        .sum::<usize>();
    if required_slots == 0 || required_slots > MAX_ROUTE_PIECE_SLOTS {
        return Err(blocker(
            EarlyTurnBlockerCategory::RouteTooLarge,
            format!(
                "The route has {required_slots} named piece slot(s); deterministic early-turn enumeration supports 1 to {MAX_ROUTE_PIECE_SLOTS}."
            ),
            line.cards.clone(),
            None,
            true,
        ));
    }

    let mut route_blockers = Vec::new();
    if line.outcome != KnownLineOutcome::TableWin {
        route_blockers.push(blocker(
            EarlyTurnBlockerCategory::InconsistentRouteMetadata,
            "The line is marked table-lethal if resolved but its typed outcome is not TableWin.",
            line.cards.clone(),
            None,
            true,
        ));
    }

    let mut pieces = Vec::new();
    for (normalized, (display, required)) in required_by_name {
        let command_zone_copies = deck
            .commanders
            .iter()
            .filter_map(|index| deck.cards.get(*index))
            .filter(|card| card.normalized_name == normalized)
            .count()
            .min(usize::from(required)) as u8;
        let required_library_copies = required.saturating_sub(command_zone_copies);
        let available_library_copies = deck
            .library
            .iter()
            .filter_map(|index| deck.cards.get(*index))
            .filter(|card| card.normalized_name == normalized)
            .count()
            .min(u16::MAX as usize) as u16;

        if command_zone_copies == 0 && available_library_copies == 0 {
            route_blockers.push(blocker(
                EarlyTurnBlockerCategory::MissingRouteCard,
                format!("The named route card “{display}” is not present in a modeled deck zone."),
                vec![display.clone()],
                None,
                true,
            ));
        } else if available_library_copies < u16::from(required_library_copies) {
            route_blockers.push(blocker(
                EarlyTurnBlockerCategory::InsufficientRouteCopies,
                format!(
                    "The route needs {required_library_copies} library copy/copies of “{display}”, but only {available_library_copies} are available."
                ),
                vec![display.clone()],
                None,
                true,
            ));
        }
        if command_zone_copies > 0 {
            route_blockers.push(blocker(
                EarlyTurnBlockerCategory::CommandZoneCastUnverified,
                format!(
                    "“{display}” is structurally available from the command zone, but casting it by the target turn is not proved by card access."
                ),
                vec![display.clone()],
                None,
                true,
            ));
        }

        pieces.push(RoutePieceRequirement {
            card: display,
            normalized_card: normalized,
            required_copies: required,
            command_zone_copies,
            required_library_copies,
            available_library_copies,
        });
    }
    pieces.sort_by(|left, right| left.normalized_card.cmp(&right.normalized_card));

    append_requirement_blockers(line, &mut route_blockers);
    append_card_function_blockers(deck, &pieces, &mut route_blockers);

    let mana_demand = derive_scalar_mana_demand(line, deck, mana_model, &mut route_blockers);
    let modeling_ceiling = if pieces
        .iter()
        .any(|piece| piece.available_library_copies < u16::from(piece.required_library_copies))
    {
        RouteModelingCeiling::Unavailable
    } else if mana_demand.is_some() {
        RouteModelingCeiling::RouteSkeletonWithScalarResourceFloor
    } else {
        RouteModelingCeiling::RouteSkeletonOnly
    };

    let route_context = build_route_context(deck, mana_model, &pieces);
    if route_context.unsupported_tutor_shapes > 0 {
        route_blockers.push(blocker(
            EarlyTurnBlockerCategory::TutorShapeUnsupported,
            format!(
                "{} potential tutor card slot(s) use a destination, multi-object instruction, or variable selection shape that this exact access enumerator deliberately excludes.",
                route_context.unsupported_tutor_shapes
            ),
            Vec::new(),
            None,
            true,
        ));
    }

    let opening = enumerate_phase_combinations(
        deck.library.len(),
        &route_context,
        policy.opening_hand_size,
        0,
        mana_demand.as_ref(),
        2,
    );
    let aggressive_mulligan = aggressive_mulligan_envelope(&opening, policy);

    let turn_specs = [
        (
            1,
            policy
                .opening_hand_size
                .saturating_add(policy.natural_draws_before_turn_one),
        ),
        (
            2,
            policy
                .opening_hand_size
                .saturating_add(policy.natural_draws_before_turn_two),
        ),
    ];
    let mut turns = Vec::new();
    for (turn, visible_cards) in turn_specs {
        let counts = enumerate_phase_combinations(
            deck.library.len(),
            &route_context,
            policy.opening_hand_size,
            visible_cards.saturating_sub(policy.opening_hand_size),
            mana_demand.as_ref(),
            turn,
        );
        turns.push(turn_readiness(
            turn,
            visible_cards,
            counts,
            mana_demand.as_ref(),
        ));
    }

    route_blockers.push(blocker(
        EarlyTurnBlockerCategory::ZoneOrSequenceUnverified,
        "Card access and scalar capacity do not establish starting zones, action order, priority, protection, or a resolved table conversion.",
        line.cards.clone(),
        None,
        true,
    ));
    deduplicate_blockers(&mut route_blockers);

    Ok(RouteReadinessReport {
        route_id: route_id(line),
        route_name: line.name.clone(),
        outcome: line.outcome,
        table_lethal_if_resolved: line.table_lethal_if_resolved,
        model_confidence: line.model_confidence,
        pieces,
        mana_demand,
        modeling_ceiling,
        aggressive_mulligan,
        turns,
        blockers: route_blockers,
    })
}

fn required_piece_counts(line: &KnownLine) -> BTreeMap<String, (String, u8)> {
    let mut required = BTreeMap::<String, (String, u8)>::new();
    for card in &line.cards {
        let normalized = normalize_card_name(card);
        let entry = required
            .entry(normalized)
            .or_insert_with(|| (card.clone(), 0));
        entry.1 = entry.1.saturating_add(1);
    }
    required
}

fn append_requirement_blockers(line: &KnownLine, blockers: &mut Vec<EarlyTurnBlocker>) {
    for requirement in &line.simulation_requirements {
        let (is_access_or_cost_metadata, detail) = match requirement {
            LineRequirement::NamedCardsPayPrintedCosts => (true, None),
            LineRequirement::AdditionalActivationMana { .. } => (true, None),
            LineRequirement::SingletonLibrary => (
                false,
                Some(
                    "The line requires a singleton-library state that card access alone cannot establish.",
                ),
            ),
            LineRequirement::AdditionalCreature { .. } => (
                false,
                Some("The line requires additional creature state beyond its named pieces."),
            ),
            LineRequirement::NonlandManaCapacity { .. } => (
                false,
                Some(
                    "The line requires a typed nonland-mana capacity state beyond the scalar floor.",
                ),
            ),
            LineRequirement::TotalExecutionMana => (
                false,
                Some(
                    "The catalog mana value is a reported total from external starting zones, not a machine-checked action sequence.",
                ),
            ),
            LineRequirement::ReviewedEmptyLibrarySequence => (
                false,
                Some(
                    "The line requires a reviewed same-turn permanent-entry and empty-library sequence.",
                ),
            ),
            LineRequirement::ReviewedInfiniteManaLoop
            | LineRequirement::ExecutableGraveyardStormLoop
            | LineRequirement::ExecutableArtifactTapTreasureLoop
            | LineRequirement::ExecutableMaskwoodArtifactDwarfTreasureLoop
            | LineRequirement::ExecutableInfiniteManaCreatureOverrunAttempt => (
                false,
                Some(
                    "The line depends on a typed runtime loop or conversion witness that this access enumerator does not execute.",
                ),
            ),
            LineRequirement::ExternalEnabler => (
                false,
                Some("The line requires an external enabling event beyond its named pieces."),
            ),
            LineRequirement::GraveyardSetup { .. } => (
                false,
                Some(
                    "The line requires graveyard setup that an unordered visible-card set cannot establish.",
                ),
            ),
            LineRequirement::CombatAccess => (
                false,
                Some("The line requires combat access and damage resolution."),
            ),
            LineRequirement::Unmodeled => (
                false,
                Some("At least one catalog prerequisite has no typed executable mapping."),
            ),
        };
        if !is_access_or_cost_metadata {
            blockers.push(blocker(
                EarlyTurnBlockerCategory::PrerequisiteExecutionUnverified,
                detail.expect("non-metadata requirements have a detail"),
                line.cards.clone(),
                None,
                true,
            ));
        }
    }
}

fn append_card_function_blockers(
    deck: &CompiledDeck,
    pieces: &[RoutePieceRequirement],
    blockers: &mut Vec<EarlyTurnBlocker>,
) {
    let required = pieces
        .iter()
        .map(|piece| piece.normalized_card.as_str())
        .collect::<HashSet<_>>();
    let affected = deck
        .cards
        .iter()
        .filter(|card| required.contains(card.normalized_name.as_str()))
        .filter(|card| {
            card.ability_program
                .unsupported_abilities()
                .next()
                .is_some()
                || card
                    .ability_program
                    .unsupported_entry_linked_permanent()
                    .is_some()
                || card
                    .ability_program
                    .unsupported_atomic_transaction()
                    .is_some()
                || card
                    .ability_program
                    .unsupported_necropotence_lifecycle()
                    .is_some()
                || card
                    .ability_program
                    .unsupported_self_transfer_tutor_permanent()
                    .is_some()
                || (!card.effects.unsupported_clauses.is_empty()
                    && !complete_typed_root_owns_scalar_parser_gaps(card))
        })
        .map(|card| card.name.clone())
        .collect::<Vec<_>>();
    if !affected.is_empty() {
        blockers.push(blocker(
            EarlyTurnBlockerCategory::UnsupportedCardFunction,
            "One or more named route cards retain unsupported Oracle clauses; their full route behavior cannot be inferred from card identity alone.",
            affected,
            None,
            true,
        ));
    }
}

fn complete_typed_root_owns_scalar_parser_gaps(card: &CompiledCard) -> bool {
    card.ability_program
        .executable_necropotence_lifecycle()
        .is_some()
        || card
            .ability_program
            .executable_self_transfer_tutor_permanent()
            .is_some()
}

#[derive(Debug, Clone)]
struct RouteContext {
    required_library_copies: Vec<u8>,
    categories: Vec<CardCategory>,
    tutor_profiles: Vec<TutorProfile>,
    unsupported_tutor_shapes: u16,
}

#[derive(Debug, Clone)]
struct CardCategory {
    fingerprint: CardFingerprint,
    tutor_profile: Option<usize>,
    multiplicity: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TutorProfile {
    immediate_mask: u16,
    library_top_mask: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CardFingerprint {
    piece_index: Option<u8>,
    immediate_tutor_mask: u16,
    library_top_tutor_mask: u16,
    exact_land: u8,
    conditional_land: u8,
    exact_one_shot_mana: u8,
    exact_reusable_mana: u8,
    conditional_one_shot_mana: u8,
    conditional_reusable_mana: u8,
}

fn build_route_context(
    deck: &CompiledDeck,
    mana_model: &ManaModel,
    pieces: &[RoutePieceRequirement],
) -> RouteContext {
    let piece_index = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| (piece.normalized_card.as_str(), index as u8))
        .collect::<HashMap<_, _>>();
    let target_cards = pieces
        .iter()
        .map(|piece| {
            deck.cards
                .iter()
                .find(|card| card.normalized_name == piece.normalized_card)
        })
        .collect::<Vec<_>>();
    let mana_sources = mana_model
        .sources
        .iter()
        .map(|source| (normalize_card_name(&source.name), source))
        .collect::<HashMap<_, _>>();

    let mut grouped = BTreeMap::<CardFingerprint, u16>::new();
    let mut unsupported_tutor_shapes = 0u16;
    for library_index in &deck.library {
        let Some(card) = deck.cards.get(*library_index) else {
            continue;
        };
        let route_piece_index = piece_index.get(card.normalized_name.as_str()).copied();
        let tutor = if route_piece_index.is_none() {
            typed_tutor_mask(card, &target_cards)
        } else {
            TutorMaskResult::default()
        };
        unsupported_tutor_shapes =
            unsupported_tutor_shapes.saturating_add(u16::from(tutor.unsupported_shape));
        let mana = classify_mana_capacity(card, mana_sources.get(&card.normalized_name).copied());
        let fingerprint = CardFingerprint {
            piece_index: route_piece_index,
            immediate_tutor_mask: tutor.immediate_mask,
            library_top_tutor_mask: tutor.library_top_mask,
            exact_land: mana.exact_land,
            conditional_land: mana.conditional_land,
            exact_one_shot_mana: mana.exact_one_shot,
            exact_reusable_mana: mana.exact_reusable,
            conditional_one_shot_mana: mana.conditional_one_shot,
            conditional_reusable_mana: mana.conditional_reusable,
        };
        let multiplicity = grouped.entry(fingerprint).or_default();
        *multiplicity = multiplicity.saturating_add(1);
    }

    let tutor_profiles = grouped
        .keys()
        .filter_map(|fingerprint| {
            (fingerprint.immediate_tutor_mask != 0 || fingerprint.library_top_tutor_mask != 0)
                .then_some(TutorProfile {
                    immediate_mask: fingerprint.immediate_tutor_mask,
                    library_top_mask: fingerprint.library_top_tutor_mask,
                })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let tutor_profile_index = tutor_profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| (*profile, index))
        .collect::<HashMap<_, _>>();
    let categories = grouped
        .into_iter()
        .map(|(fingerprint, multiplicity)| CardCategory {
            tutor_profile: tutor_profile_index
                .get(&TutorProfile {
                    immediate_mask: fingerprint.immediate_tutor_mask,
                    library_top_mask: fingerprint.library_top_tutor_mask,
                })
                .copied(),
            fingerprint,
            multiplicity,
        })
        .collect();

    RouteContext {
        required_library_copies: pieces
            .iter()
            .map(|piece| piece.required_library_copies)
            .collect(),
        categories,
        tutor_profiles,
        unsupported_tutor_shapes,
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct TutorMaskResult {
    immediate_mask: u16,
    library_top_mask: u16,
    unsupported_shape: bool,
}

fn typed_tutor_mask(
    card: &CompiledCard,
    target_cards: &[Option<&CompiledCard>],
) -> TutorMaskResult {
    let mut result = TutorMaskResult::default();
    if card
        .ability_program
        .executable_atomic_transaction()
        .is_some_and(|transaction| {
            transaction
                .resolution
                .iter()
                .any(|effect| matches!(effect, AtomicEffect::SearchToHand(_)))
                && transaction
                    .resolution
                    .iter()
                    .any(|effect| matches!(effect, AtomicEffect::RandomDiscard(_)))
        })
    {
        // The linked random discard is authoritative over every legacy tutor
        // descriptor on the same compiled card. No searched or retained card
        // is certain to survive this resolution.
        result.unsupported_shape = true;
        return result;
    }
    if card.effects.tutor.is_executable_on_spell_resolution() {
        for instruction in &card.effects.tutor.instructions {
            if instruction.source != TutorSourceZone::Library {
                continue;
            }
            if instruction.quantity != 1 {
                result.unsupported_shape = true;
                continue;
            }
            let destination_mask = match instruction.destination {
                TutorDestination::Hand
                | TutorDestination::BattlefieldUntapped
                | TutorDestination::BattlefieldTapped => &mut result.immediate_mask,
                TutorDestination::LibraryTop => &mut result.library_top_mask,
                TutorDestination::None => {
                    result.unsupported_shape = true;
                    continue;
                }
            };
            for (index, target) in target_cards.iter().enumerate() {
                if target
                    .is_some_and(|target| instruction.target.matches(target.effects.card_types))
                {
                    *destination_mask |= 1u16 << index;
                }
            }
        }
    } else if !card.effects.tutor.instructions.is_empty() {
        result.unsupported_shape = true;
    }

    for ability in card.ability_program.executable_abilities() {
        for effect in &ability.effects {
            match effect {
                AbilityEffect::Tutor(tutor)
                    if tutor.from == Zone::Library
                        && matches!(tutor.destination, Zone::Hand | Zone::Battlefield) =>
                {
                    result.immediate_mask |= tutor_filter_mask(&tutor.filter, target_cards);
                }
                AbilityEffect::Tutor(_) | AbilityEffect::VariableCreatureTutor(_) => {
                    result.unsupported_shape = true;
                }
                _ => {}
            }
        }
    }
    if let Some(transaction) = card.ability_program.executable_atomic_transaction() {
        match transaction.resolution.as_slice() {
            [AtomicEffect::SearchToHand(tutor)] => {
                if tutor.from == Zone::Library && tutor.destination == Zone::Hand {
                    result.immediate_mask |= tutor_filter_mask(&tutor.filter, target_cards);
                } else {
                    result.unsupported_shape = true;
                }
            }
            effects
                if effects
                    .iter()
                    .any(|effect| matches!(effect, AtomicEffect::SearchToHand(_))) =>
            {
                // A linked effect after the search can invalidate deterministic
                // access. In particular, a uniform random discard from the
                // resulting hand must remain stochastic instead of widening
                // the certain tutor skeleton.
                result.unsupported_shape = true;
            }
            _ => {}
        }
    }
    if let Some(permanent) = card
        .ability_program
        .executable_self_transfer_tutor_permanent()
    {
        match permanent.activation.resolution.as_slice() {
            [
                SelfTransferTutorResolutionStep::SearchToHand(tutor),
                SelfTransferTutorResolutionStep::OpponentGainsControlOfSource,
            ] if tutor.from == Zone::Library && tutor.destination == Zone::Hand => {
                result.immediate_mask |= tutor_filter_mask(&tutor.filter, target_cards);
            }
            _ => result.unsupported_shape = true,
        }
    } else if card.ability_program.self_transfer_tutor_permanent.is_some() {
        result.unsupported_shape = true;
    }
    if card
        .ability_program
        .executable_necropotence_lifecycle()
        .is_none()
        && card.ability_program.necropotence_lifecycle.is_some()
    {
        // A complete typed lifecycle is supported development/card access,
        // but it is deliberately not a named-card tutor: it moves the real
        // top card and delivers that same hidden object at the next end step.
        // Any recognized mutation must nevertheless fail closed instead of
        // being silently grouped with ordinary filler.
        result.unsupported_shape = true;
    }
    result
}

fn tutor_filter_mask(filter: &TutorFilter, target_cards: &[Option<&CompiledCard>]) -> u16 {
    let TutorFilter::AnyOf(filters) = filter;
    target_cards
        .iter()
        .enumerate()
        .fold(0u16, |mask, (index, target)| {
            if target.is_some_and(|target| {
                filters
                    .iter()
                    .any(|filter| object_filter_matches(filter, target))
            }) {
                mask | (1u16 << index)
            } else {
                mask
            }
        })
}

fn object_filter_matches(filter: &ObjectFilter, card: &CompiledCard) -> bool {
    let types = card.effects.card_types;
    if filter.nonland && types.is_land {
        return false;
    }
    if filter
        .card_type
        .is_some_and(|card_type| !program_card_type_matches(card_type, card))
    {
        return false;
    }
    if filter
        .excluded_card_type
        .is_some_and(|card_type| program_card_type_matches(card_type, card))
    {
        return false;
    }
    if let Some(subtype) = &filter.subtype {
        let type_line = card.type_line.to_ascii_lowercase();
        if !type_line
            .split(|character: char| !character.is_alphanumeric())
            .any(|word| word == subtype.to_ascii_lowercase())
        {
            return false;
        }
    }
    if let Some(excluded_subtype) = &filter.excluded_subtype {
        let type_line = card.type_line.to_ascii_lowercase();
        if type_line
            .split(|character: char| !character.is_alphanumeric())
            .any(|word| word == excluded_subtype.to_ascii_lowercase())
        {
            return false;
        }
    }
    true
}

fn program_card_type_matches(card_type: CardType, card: &CompiledCard) -> bool {
    let types = card.effects.card_types;
    match card_type {
        CardType::Artifact => types.is_artifact,
        CardType::Creature => types.is_creature,
        CardType::Dragon => {
            types.is_creature
                && card
                    .type_line
                    .split(|character: char| !character.is_alphanumeric())
                    .any(|word| word.eq_ignore_ascii_case("dragon"))
        }
        CardType::Land => types.is_land,
        CardType::Permanent => {
            types.is_artifact || types.is_creature || types.is_enchantment || types.is_land
        }
        CardType::Spell | CardType::Card => true,
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ManaCapacity {
    exact_land: u8,
    conditional_land: u8,
    exact_one_shot: u8,
    exact_reusable: u8,
    conditional_one_shot: u8,
    conditional_reusable: u8,
}

fn classify_mana_capacity(card: &CompiledCard, source: Option<&ManaSourceProfile>) -> ManaCapacity {
    let mut capacity = ManaCapacity::default();
    if let Some(source) = source {
        if source.is_land {
            if !source.conditional
                && !source.unknown
                && source.enters_tapped == EntersTapped::UntappedByDefault
            {
                capacity.exact_land = 1;
            } else {
                capacity.conditional_land = 1;
            }
            return capacity;
        }
    } else if card.effects.card_types.is_land {
        capacity.conditional_land = 1;
        return capacity;
    }

    if let Some((amount, reusable)) = strict_zero_input_mana(card) {
        if reusable {
            capacity.exact_reusable = amount.min(MAX_TRACKED_MANA);
        } else {
            capacity.exact_one_shot = amount.min(MAX_TRACKED_MANA);
        }
        return capacity;
    }

    let amount = card
        .effects
        .mana_produced
        .conservative_value(0)
        .max(u8::from(card.has(role::FAST_MANA) || card.has(role::RAMP)));
    if amount == 0 && source.is_none() {
        return capacity;
    }
    match card.effects.mana_production_kind {
        ManaProductionKind::ReusableActivated => {
            capacity.conditional_reusable = amount.clamp(1, MAX_TRACKED_MANA);
        }
        ManaProductionKind::SpellResolution
        | ManaProductionKind::OneShotActivated
        | ManaProductionKind::NonRefreshingActivated
        | ManaProductionKind::Unsupported
        | ManaProductionKind::None => {
            capacity.conditional_one_shot = amount.clamp(1, MAX_TRACKED_MANA);
        }
    }
    capacity
}

fn strict_zero_input_mana(card: &CompiledCard) -> Option<(u8, bool)> {
    if let Some(transaction) = card.ability_program.executable_atomic_transaction()
        && transaction.initiation == AtomicInitiation::HandManaAbility
    {
        let allowed_costs = transaction.initiation_costs.iter().all(|cost| {
            matches!(
                cost,
                crate::ability_program::AtomicCost::ExileSelf { from: Zone::Hand }
            )
        });
        if allowed_costs {
            let amount = transaction
                .resolution
                .iter()
                .filter_map(|effect| match effect {
                    AtomicEffect::AddFixedMana(profile) => Some(fixed_mana_total(*profile)),
                    AtomicEffect::ConditionalManaReplacement(effect) => Some(
                        fixed_mana_total(effect.default).min(fixed_mana_total(effect.replacement)),
                    ),
                    _ => None,
                })
                .sum::<u16>();
            if amount > 0 {
                return Some((amount.min(u16::from(u8::MAX)) as u8, false));
            }
        }
    }

    if card.mana_value > 0.0 || card.effects.card_types.is_creature {
        return None;
    }
    let mut best = None::<(u8, bool)>;
    for ability in card.ability_program.executable_abilities() {
        if !matches!(ability.timing, AbilityTiming::Activated { .. })
            || !ability.preconditions.is_empty()
            || ability.costs.is_empty()
            || !ability
                .costs
                .iter()
                .all(|cost| matches!(cost, AbilityCost::TapSelf | AbilityCost::SacrificeSelf))
            || !ability
                .costs
                .iter()
                .any(|cost| matches!(cost, AbilityCost::TapSelf))
        {
            continue;
        }
        let amount = ability
            .effects
            .iter()
            .filter_map(|effect| match effect {
                AbilityEffect::AddMana(mana) => scalar_mana_effect(mana.amount, mana.kind),
                AbilityEffect::AddManaAndSourceDamage(linked) => {
                    scalar_mana_effect(linked.mana.amount, linked.mana.kind)
                }
                _ => None,
            })
            .max()
            .unwrap_or_default();
        if amount == 0 {
            continue;
        }
        let reusable = !ability
            .costs
            .iter()
            .any(|cost| matches!(cost, AbilityCost::SacrificeSelf));
        let candidate = (amount, reusable);
        if best.is_none_or(|current| candidate.0 > current.0) {
            best = Some(candidate);
        }
    }
    best
}

fn scalar_mana_effect(amount: u16, kind: ManaKind) -> Option<u8> {
    let scalar = match kind {
        ManaKind::AnyOneColor => amount,
        ManaKind::Fixed(profile) => fixed_mana_total(profile),
        ManaKind::AnyColorAmongLegendaryCreaturesAndPlaneswalkersYouControl
        | ManaKind::AnyTypeProducedByTriggeringPermanent => return None,
    };
    Some(scalar.min(u16::from(u8::MAX)) as u8)
}

fn fixed_mana_total(profile: FixedManaProfile) -> u16 {
    profile
        .white
        .saturating_add(profile.blue)
        .saturating_add(profile.black)
        .saturating_add(profile.red)
        .saturating_add(profile.green)
        .saturating_add(profile.colorless)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EnumerationState {
    selected_opening: u8,
    selected_draws: u8,
    piece_counts: Vec<u8>,
    opening_tutor_counts: Vec<u8>,
    drawn_tutor_counts: Vec<u8>,
    exact_lands: u8,
    conditional_lands: u8,
    exact_one_shot_mana: u8,
    exact_reusable_mana: u8,
    conditional_one_shot_mana: u8,
    conditional_reusable_mana: u8,
}

#[derive(Debug, Clone, Default)]
struct EnumerationCounts {
    total: u128,
    direct: u128,
    tutor_assisted: u128,
    strict_scalar_floor: Option<u128>,
    conditional_scalar_ceiling: Option<u128>,
}

fn enumerate_visible_combinations(
    library_size: usize,
    context: &RouteContext,
    visible_cards: u8,
    mana_demand: Option<&ScalarManaDemand>,
    target_turn: u8,
) -> EnumerationCounts {
    enumerate_phase_combinations(
        library_size,
        context,
        visible_cards,
        0,
        mana_demand,
        target_turn,
    )
}

/// Exactly enumerates an opening candidate and the subsequent natural-draw
/// subset as distinct phases. Cards within either phase remain unordered.
/// This distinction is required for library-top tutors: an opening tutor can
/// set up a later natural draw, while a tutor first seen in the draw subset
/// cannot retroactively replace that same draw.
fn enumerate_phase_combinations(
    library_size: usize,
    context: &RouteContext,
    opening_cards: u8,
    natural_draws: u8,
    mana_demand: Option<&ScalarManaDemand>,
    target_turn: u8,
) -> EnumerationCounts {
    if usize::from(opening_cards).saturating_add(usize::from(natural_draws)) > library_size {
        return EnumerationCounts::default();
    }
    let initial = EnumerationState {
        selected_opening: 0,
        selected_draws: 0,
        piece_counts: vec![0; context.required_library_copies.len()],
        opening_tutor_counts: vec![0; context.tutor_profiles.len()],
        drawn_tutor_counts: vec![0; context.tutor_profiles.len()],
        exact_lands: 0,
        conditional_lands: 0,
        exact_one_shot_mana: 0,
        exact_reusable_mana: 0,
        conditional_one_shot_mana: 0,
        conditional_reusable_mana: 0,
    };
    let mut states = HashMap::<EnumerationState, u128>::from([(initial, 1)]);
    for category in &context.categories {
        let mut next = HashMap::<EnumerationState, u128>::new();
        for (state, weight) in states {
            let maximum_opening = u16::from(opening_cards.saturating_sub(state.selected_opening))
                .min(category.multiplicity);
            for chosen_opening in 0..=maximum_opening {
                let remaining_multiplicity = category.multiplicity.saturating_sub(chosen_opening);
                let maximum_drawn = u16::from(natural_draws.saturating_sub(state.selected_draws))
                    .min(remaining_multiplicity);
                for chosen_drawn in 0..=maximum_drawn {
                    let chosen_total = chosen_opening.saturating_add(chosen_drawn);
                    let mut updated = state.clone();
                    updated.selected_opening = updated
                        .selected_opening
                        .saturating_add(chosen_opening as u8);
                    updated.selected_draws =
                        updated.selected_draws.saturating_add(chosen_drawn as u8);
                    if let Some(index) = category.fingerprint.piece_index {
                        let index = usize::from(index);
                        updated.piece_counts[index] = updated.piece_counts[index]
                            .saturating_add(chosen_total as u8)
                            .min(context.required_library_copies[index]);
                    }
                    if let Some(profile) = category.tutor_profile {
                        updated.opening_tutor_counts[profile] = updated.opening_tutor_counts
                            [profile]
                            .saturating_add(chosen_opening as u8)
                            .min(MAX_ROUTE_PIECE_SLOTS as u8);
                        updated.drawn_tutor_counts[profile] = updated.drawn_tutor_counts[profile]
                            .saturating_add(chosen_drawn as u8)
                            .min(MAX_ROUTE_PIECE_SLOTS as u8);
                    }
                    add_capacity(
                        &mut updated.exact_lands,
                        category.fingerprint.exact_land,
                        chosen_total,
                        2,
                    );
                    add_capacity(
                        &mut updated.conditional_lands,
                        category.fingerprint.conditional_land,
                        chosen_total,
                        2,
                    );
                    add_capacity(
                        &mut updated.exact_one_shot_mana,
                        category.fingerprint.exact_one_shot_mana,
                        chosen_total,
                        MAX_TRACKED_MANA,
                    );
                    add_capacity(
                        &mut updated.exact_reusable_mana,
                        category.fingerprint.exact_reusable_mana,
                        chosen_total,
                        MAX_TRACKED_MANA,
                    );
                    add_capacity(
                        &mut updated.conditional_one_shot_mana,
                        category.fingerprint.conditional_one_shot_mana,
                        chosen_total,
                        MAX_TRACKED_MANA,
                    );
                    add_capacity(
                        &mut updated.conditional_reusable_mana,
                        category.fingerprint.conditional_reusable_mana,
                        chosen_total,
                        MAX_TRACKED_MANA,
                    );
                    let combinations = binomial(
                        u128::from(category.multiplicity),
                        u128::from(chosen_opening),
                    )
                    .saturating_mul(binomial(
                        u128::from(remaining_multiplicity),
                        u128::from(chosen_drawn),
                    ));
                    let contribution = weight.saturating_mul(combinations);
                    let accumulated = next.entry(updated).or_default();
                    *accumulated = accumulated.saturating_add(contribution);
                }
            }
        }
        states = next;
    }

    let mut counts = EnumerationCounts {
        strict_scalar_floor: mana_demand.map(|_| 0),
        conditional_scalar_ceiling: mana_demand.map(|_| 0),
        ..Default::default()
    };
    for (state, weight) in states.into_iter().filter(|(state, _)| {
        state.selected_opening == opening_cards && state.selected_draws == natural_draws
    }) {
        counts.total = counts.total.saturating_add(weight);
        let direct = state
            .piece_counts
            .iter()
            .zip(&context.required_library_copies)
            .all(|(visible, required)| visible >= required);
        if direct {
            counts.direct = counts.direct.saturating_add(weight);
        }
        let tutor_ready = direct
            || phase_aware_tutors_cover_missing(
                &state.piece_counts,
                &context.required_library_copies,
                &context.tutor_profiles,
                &state.opening_tutor_counts,
                &state.drawn_tutor_counts,
                target_turn >= 2,
            );
        if tutor_ready {
            counts.tutor_assisted = counts.tutor_assisted.saturating_add(weight);
        }
        if tutor_ready && let Some(demand) = mana_demand {
            let strict_capacity = cumulative_land_capacity(state.exact_lands, target_turn)
                .saturating_add(state.exact_one_shot_mana)
                .saturating_add(state.exact_reusable_mana.saturating_mul(target_turn));
            let conditional_capacity = cumulative_land_capacity(
                state.exact_lands.saturating_add(state.conditional_lands),
                target_turn,
            )
            .saturating_add(state.exact_one_shot_mana)
            .saturating_add(state.exact_reusable_mana.saturating_mul(target_turn))
            .saturating_add(state.conditional_one_shot_mana)
            .saturating_add(state.conditional_reusable_mana.saturating_mul(target_turn));
            if strict_capacity >= demand.amount
                && let Some(total) = &mut counts.strict_scalar_floor
            {
                *total = total.saturating_add(weight);
            }
            if conditional_capacity >= demand.amount
                && let Some(total) = &mut counts.conditional_scalar_ceiling
            {
                *total = total.saturating_add(weight);
            }
        }
    }

    debug_assert_eq!(
        counts.total,
        binomial(library_size as u128, u128::from(opening_cards)).saturating_mul(binomial(
            library_size.saturating_sub(usize::from(opening_cards)) as u128,
            u128::from(natural_draws),
        ))
    );
    counts
}

fn add_capacity(target: &mut u8, per_card: u8, chosen: u16, cap: u8) {
    *target = target
        .saturating_add(per_card.saturating_mul(chosen.min(u16::from(u8::MAX)) as u8))
        .min(cap);
}

fn cumulative_land_capacity(lands: u8, target_turn: u8) -> u8 {
    (1..=target_turn).fold(0u8, |total, turn| total.saturating_add(lands.min(turn)))
}

type PhaseAwareTutorMemo = HashMap<(Vec<u8>, Vec<u8>, Vec<u8>, bool), bool>;

fn phase_aware_tutors_cover_missing(
    visible: &[u8],
    required: &[u8],
    tutor_profiles: &[TutorProfile],
    opening_tutors: &[u8],
    drawn_tutors: &[u8],
    library_top_draw_available: bool,
) -> bool {
    let missing = visible
        .iter()
        .zip(required)
        .map(|(visible, required)| required.saturating_sub(*visible))
        .collect::<Vec<_>>();
    if missing.iter().all(|count| *count == 0) {
        return true;
    }
    let mut memo = PhaseAwareTutorMemo::new();
    phase_aware_tutor_assignment_exists(
        missing,
        tutor_profiles,
        opening_tutors.to_vec(),
        drawn_tutors.to_vec(),
        library_top_draw_available,
        &mut memo,
    )
}

fn phase_aware_tutor_assignment_exists(
    missing: Vec<u8>,
    tutor_profiles: &[TutorProfile],
    opening_tutors: Vec<u8>,
    drawn_tutors: Vec<u8>,
    library_top_draw_available: bool,
    memo: &mut PhaseAwareTutorMemo,
) -> bool {
    if missing.iter().all(|count| *count == 0) {
        return true;
    }
    let key = (
        missing.clone(),
        opening_tutors.clone(),
        drawn_tutors.clone(),
        library_top_draw_available,
    );
    if let Some(cached) = memo.get(&key) {
        return *cached;
    }
    let Some(piece_index) = missing.iter().position(|count| *count > 0) else {
        return true;
    };
    let piece_mask = 1u16 << piece_index;
    for (profile_index, profile) in tutor_profiles.iter().enumerate() {
        if drawn_tutors[profile_index] > 0 && profile.immediate_mask & piece_mask != 0 {
            let mut next_missing = missing.clone();
            let mut next_drawn = drawn_tutors.clone();
            next_missing[piece_index] -= 1;
            next_drawn[profile_index] -= 1;
            if phase_aware_tutor_assignment_exists(
                next_missing,
                tutor_profiles,
                opening_tutors.clone(),
                next_drawn,
                library_top_draw_available,
                memo,
            ) {
                memo.insert(key, true);
                return true;
            }
        }
        if opening_tutors[profile_index] == 0 {
            continue;
        }
        if profile.immediate_mask & piece_mask != 0 {
            let mut next_missing = missing.clone();
            let mut next_opening = opening_tutors.clone();
            next_missing[piece_index] -= 1;
            next_opening[profile_index] -= 1;
            if phase_aware_tutor_assignment_exists(
                next_missing,
                tutor_profiles,
                next_opening,
                drawn_tutors.clone(),
                library_top_draw_available,
                memo,
            ) {
                memo.insert(key, true);
                return true;
            }
        } else if library_top_draw_available && profile.library_top_mask & piece_mask != 0 {
            let mut next_missing = missing.clone();
            let mut next_opening = opening_tutors.clone();
            next_missing[piece_index] -= 1;
            next_opening[profile_index] -= 1;
            if phase_aware_tutor_assignment_exists(
                next_missing,
                tutor_profiles,
                next_opening,
                drawn_tutors.clone(),
                false,
                memo,
            ) {
                memo.insert(key, true);
                return true;
            }
        }
    }
    memo.insert(key, false);
    false
}

fn tutors_cover_missing(
    visible: &[u8],
    required: &[u8],
    tutor_masks: &[u16],
    tutor_counts: &[u8],
) -> bool {
    let missing = visible
        .iter()
        .zip(required)
        .map(|(visible, required)| required.saturating_sub(*visible))
        .collect::<Vec<_>>();
    if missing.iter().all(|count| *count == 0) {
        return true;
    }
    let mut memo = HashMap::<(Vec<u8>, Vec<u8>), bool>::new();
    tutor_assignment_exists(missing, tutor_masks, tutor_counts.to_vec(), &mut memo)
}

fn tutor_assignment_exists(
    missing: Vec<u8>,
    tutor_masks: &[u16],
    tutors: Vec<u8>,
    memo: &mut HashMap<(Vec<u8>, Vec<u8>), bool>,
) -> bool {
    if missing.iter().all(|count| *count == 0) {
        return true;
    }
    let key = (missing.clone(), tutors.clone());
    if let Some(cached) = memo.get(&key) {
        return *cached;
    }
    let Some(piece_index) = missing.iter().position(|count| *count > 0) else {
        return true;
    };
    for tutor_index in 0..tutor_masks.len() {
        if tutors[tutor_index] == 0 || tutor_masks[tutor_index] & (1u16 << piece_index) == 0 {
            continue;
        }
        let mut next_missing = missing.clone();
        let mut next_tutors = tutors.clone();
        next_missing[piece_index] -= 1;
        next_tutors[tutor_index] -= 1;
        if tutor_assignment_exists(next_missing, tutor_masks, next_tutors, memo) {
            memo.insert(key, true);
            return true;
        }
    }
    memo.insert(key, false);
    false
}

fn derive_scalar_mana_demand(
    line: &KnownLine,
    deck: &CompiledDeck,
    mana_model: &ManaModel,
    blockers: &mut Vec<EarlyTurnBlocker>,
) -> Option<ScalarManaDemand> {
    if line
        .simulation_requirements
        .contains(&LineRequirement::TotalExecutionMana)
    {
        let Some(raw) = line.mana_needed.as_deref() else {
            blockers.push(blocker(
                EarlyTurnBlockerCategory::ManaDemandUnresolved,
                "The route marks total execution mana but supplies no parseable mana string.",
                line.cards.clone(),
                None,
                true,
            ));
            return None;
        };
        let Some(parsed) = parsed_scalar_cost(raw) else {
            blockers.push(blocker(
                EarlyTurnBlockerCategory::ManaDemandUnresolved,
                format!(
                    "The reported total execution mana “{raw}” contains a symbol shape that cannot be reduced exactly."
                ),
                line.cards.clone(),
                None,
                true,
            ));
            return None;
        };
        if parsed.colored {
            blockers.push(blocker(
                EarlyTurnBlockerCategory::ColoredPaymentUnverified,
                "The reported total includes colored or colorless pips; the scalar floor does not prove the required colors.",
                line.cards.clone(),
                None,
                true,
            ));
        }
        return Some(ScalarManaDemand {
            amount: parsed.amount,
            includes_colored_or_colorless_pips: parsed.colored,
            basis: "Catalog-reported total execution mana from external starting zones.".into(),
            exact_printed_cost_coverage: false,
        });
    }

    let card_costs = mana_model
        .cards
        .iter()
        .map(|card| (normalize_card_name(&card.name), card))
        .collect::<HashMap<_, _>>();
    let mut amount = 0u8;
    let mut colored = false;
    let mut exact = true;
    for card_name in &line.cards {
        let normalized = normalize_card_name(card_name);
        let Some(compiled) = deck
            .cards
            .iter()
            .find(|card| card.normalized_name == normalized)
        else {
            exact = false;
            continue;
        };
        if compiled.effects.card_types.is_land {
            continue;
        }
        if let Some(mana_card) = card_costs.get(&normalized)
            && let Some(cost) = minimum_face_scalar_cost(&mana_card.cost)
        {
            amount = amount.saturating_add(cost.amount);
            colored |= cost.colored;
        } else {
            exact = false;
            amount = amount
                .saturating_add(compiled.mana_value.ceil().clamp(0.0, f32::from(u8::MAX)) as u8);
        }
    }

    for requirement in &line.simulation_requirements {
        if let LineRequirement::AdditionalActivationMana { cost } = requirement {
            let Some(parsed) = parsed_scalar_cost(cost) else {
                exact = false;
                continue;
            };
            amount = amount.saturating_add(parsed.amount);
            colored |= parsed.colored;
        }
    }
    if amount == 0 && !exact {
        blockers.push(blocker(
            EarlyTurnBlockerCategory::ManaDemandUnresolved,
            "Printed or activation mana could not be reduced to a scalar diagnostic.",
            line.cards.clone(),
            None,
            true,
        ));
        return None;
    }
    if colored {
        blockers.push(blocker(
            EarlyTurnBlockerCategory::ColoredPaymentUnverified,
            "Named route costs contain colored or colorless pips; the scalar capacity floor cannot prove legal colored payment.",
            line.cards.clone(),
            None,
            true,
        ));
    }
    if !exact {
        blockers.push(blocker(
            EarlyTurnBlockerCategory::ManaDemandUnresolved,
            "At least one route cost fell back to mana value, so alternate faces, X, and alternate costs remain unresolved.",
            line.cards.clone(),
            None,
            true,
        ));
    }
    Some(ScalarManaDemand {
        amount,
        includes_colored_or_colorless_pips: colored,
        basis: "Sum of minimum parsed printed costs for named nonland pieces plus typed additional activation mana.".into(),
        exact_printed_cost_coverage: exact,
    })
}

#[derive(Debug, Clone, Copy)]
struct ParsedScalarCost {
    amount: u8,
    colored: bool,
}

fn parsed_scalar_cost(raw: &str) -> Option<ParsedScalarCost> {
    minimum_face_scalar_cost(&parse_mana_cost(Some(raw)))
}

fn minimum_face_scalar_cost(profile: &crate::mana::ManaCostProfile) -> Option<ParsedScalarCost> {
    profile
        .faces
        .iter()
        .filter(|face| {
            face.confidence >= 0.999
                && face
                    .pips
                    .iter()
                    .all(|pip| !pip.is_unknown && !pip.is_variable && !pip.is_snow)
        })
        .map(|face| {
            let amount = face.pips.iter().fold(0u16, |total, pip| {
                total.saturating_add(pip.generic_value.unwrap_or(1))
            });
            ParsedScalarCost {
                amount: amount.min(u16::from(u8::MAX)) as u8,
                colored: face
                    .pips
                    .iter()
                    .any(|pip| !pip.colors.is_empty() || pip.is_colorless),
            }
        })
        .min_by_key(|cost| cost.amount)
}

fn aggressive_mulligan_envelope(
    opening: &EnumerationCounts,
    policy: &EarlyTurnPolicy,
) -> AggressiveMulliganEnvelope {
    let direct = probability(opening.direct, opening.total);
    let tutor = probability(opening.tutor_assisted, opening.total);
    let strict = opening
        .strict_scalar_floor
        .map(|count| probability(count, opening.total));
    AggressiveMulliganEnvelope {
        candidate_hands: policy.aggressive_candidate_hands,
        direct_skeleton_in_at_least_one_candidate: at_least_one_candidate(
            direct,
            policy.aggressive_candidate_hands,
        ),
        typed_tutor_skeleton_in_at_least_one_candidate: at_least_one_candidate(
            tutor,
            policy.aggressive_candidate_hands,
        ),
        scalar_floor_skeleton_in_at_least_one_candidate: strict.map(|probability| {
            at_least_one_candidate(probability, policy.aggressive_candidate_hands)
        }),
        caveat: "This four-candidate figure is a route-seeking envelope over independently reshuffled sevens, not the production mulligan probability. It does not model the simulator's stop-on-keep decisions, London bottoming, or correlations between a kept hand and later natural draws. Its scalar field asks only whether an opening candidate contains the by-turn-two capacity floor.".into(),
    }
}

fn turn_readiness(
    turn: u8,
    visible_cards: u8,
    counts: EnumerationCounts,
    mana_demand: Option<&ScalarManaDemand>,
) -> TurnRouteReadiness {
    let direct_probability = probability(counts.direct, counts.total);
    let tutor_probability = probability(counts.tutor_assisted, counts.total);
    let strict_probability = counts
        .strict_scalar_floor
        .map(|count| probability(count, counts.total));
    let conditional_probability = counts
        .conditional_scalar_ceiling
        .map(|count| probability(count, counts.total));
    let mut blockers = Vec::new();
    if direct_probability < 1.0 {
        blockers.push(blocker(
            EarlyTurnBlockerCategory::DirectPieceAccess,
            format!("At least one named library piece is not naturally visible by turn {turn}."),
            Vec::new(),
            Some((1.0 - direct_probability).clamp(0.0, 1.0)),
            false,
        ));
    }
    if tutor_probability > direct_probability {
        blockers.push(blocker(
            EarlyTurnBlockerCategory::TutorPaymentOrTiming,
            "Typed library search closes part of the access gap, but the tutor's payment and ordered resolution are not proved here.",
            Vec::new(),
            Some((tutor_probability - direct_probability).clamp(0.0, 1.0)),
            true,
        ));
    }
    if tutor_probability < 1.0 {
        blockers.push(blocker(
            EarlyTurnBlockerCategory::UnresolvedPieceAccess,
            format!(
                "Natural access plus eligible typed tutors still leaves at least one route piece unavailable by turn {turn}."
            ),
            Vec::new(),
            Some((1.0 - tutor_probability).clamp(0.0, 1.0)),
            false,
        ));
    }
    if mana_demand.is_some() {
        if let Some(strict) = strict_probability
            && strict < tutor_probability
        {
            blockers.push(blocker(
                EarlyTurnBlockerCategory::ScalarManaCapacity,
                "The visible route skeleton exceeds the conservative unconditional scalar mana floor in part of the combination space.",
                Vec::new(),
                Some((tutor_probability - strict).clamp(0.0, 1.0)),
                true,
            ));
        }
        if let (Some(strict), Some(conditional)) = (strict_probability, conditional_probability)
            && conditional > strict
        {
            blockers.push(blocker(
                EarlyTurnBlockerCategory::ConditionalManaDependency,
                "Conditional, tapped, cost-bearing, or incompletely typed mana sources widen the scalar ceiling but do not prove capacity.",
                Vec::new(),
                Some((conditional - strict).clamp(0.0, 1.0)),
                true,
            ));
        }
    }

    TurnRouteReadiness {
        turn,
        visible_library_cards: visible_cards,
        total_combinations: counts.total.to_string(),
        direct_skeleton_combinations: counts.direct.to_string(),
        typed_tutor_skeleton_combinations: counts.tutor_assisted.to_string(),
        strict_scalar_floor_combinations: counts.strict_scalar_floor.map(|count| count.to_string()),
        conditional_scalar_ceiling_combinations: counts
            .conditional_scalar_ceiling
            .map(|count| count.to_string()),
        direct_skeleton_probability: direct_probability,
        typed_tutor_skeleton_probability: tutor_probability,
        strict_scalar_floor_probability: strict_probability,
        conditional_scalar_ceiling_probability: conditional_probability,
        executable_conversion_probability: None,
        blockers,
    }
}

fn probability(numerator: u128, denominator: u128) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn at_least_one_candidate(single_candidate_probability: f64, candidates: u8) -> f64 {
    (1.0 - (1.0 - single_candidate_probability).powi(i32::from(candidates))).clamp(0.0, 1.0)
}

fn binomial(n: u128, k: u128) -> u128 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    (0..k).fold(1u128, |result, index| {
        result
            .saturating_mul(n - index)
            .checked_div(index + 1)
            .unwrap_or_default()
    })
}

fn route_id(line: &KnownLine) -> String {
    let mut members = line
        .cards
        .iter()
        .map(|card| normalize_card_name(card))
        .collect::<Vec<_>>();
    members.sort();
    let mut hasher = Sha256::new();
    hasher.update(if line.table_lethal_if_resolved {
        b"table-lethal".as_slice()
    } else {
        b"nonlethal".as_slice()
    });
    hasher.update(format!("{:?}", line.outcome));
    for member in members {
        hasher.update([0]);
        hasher.update(member.as_bytes());
    }
    let digest = hasher.finalize();
    digest[..12]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn blocker(
    category: EarlyTurnBlockerCategory,
    detail: impl Into<String>,
    mut affected_cards: Vec<String>,
    probability_mass: Option<f64>,
    prevents_executable_claim: bool,
) -> EarlyTurnBlocker {
    affected_cards.sort_by_key(|card| normalize_card_name(card));
    affected_cards.dedup_by(|left, right| normalize_card_name(left) == normalize_card_name(right));
    EarlyTurnBlocker {
        category,
        detail: detail.into(),
        affected_cards,
        probability_mass,
        prevents_executable_claim,
    }
}

fn deduplicate_blockers(blockers: &mut Vec<EarlyTurnBlocker>) {
    blockers.sort_by(|left, right| {
        blocker_sort_key(left)
            .cmp(&blocker_sort_key(right))
            .then_with(|| left.detail.cmp(&right.detail))
    });
    blockers.dedup_by(|left, right| {
        left.category == right.category
            && left.detail == right.detail
            && left.affected_cards == right.affected_cards
    });
}

fn blocker_sort_key(blocker: &EarlyTurnBlocker) -> (u8, bool) {
    (
        match blocker.category {
            EarlyTurnBlockerCategory::NoExplicitTableWinRoute => 0,
            EarlyTurnBlockerCategory::RouteTooLarge => 1,
            EarlyTurnBlockerCategory::MissingRouteCard => 2,
            EarlyTurnBlockerCategory::InsufficientRouteCopies => 3,
            EarlyTurnBlockerCategory::InconsistentRouteMetadata => 4,
            EarlyTurnBlockerCategory::DirectPieceAccess => 5,
            EarlyTurnBlockerCategory::UnresolvedPieceAccess => 6,
            EarlyTurnBlockerCategory::TutorPaymentOrTiming => 7,
            EarlyTurnBlockerCategory::TutorShapeUnsupported => 8,
            EarlyTurnBlockerCategory::ManaDemandUnresolved => 9,
            EarlyTurnBlockerCategory::ScalarManaCapacity => 10,
            EarlyTurnBlockerCategory::ConditionalManaDependency => 11,
            EarlyTurnBlockerCategory::ColoredPaymentUnverified => 12,
            EarlyTurnBlockerCategory::CommandZoneCastUnverified => 13,
            EarlyTurnBlockerCategory::ZoneOrSequenceUnverified => 14,
            EarlyTurnBlockerCategory::PrerequisiteExecutionUnverified => 15,
            EarlyTurnBlockerCategory::UnsupportedCardFunction => 16,
        },
        blocker.prevents_executable_claim,
    )
}
