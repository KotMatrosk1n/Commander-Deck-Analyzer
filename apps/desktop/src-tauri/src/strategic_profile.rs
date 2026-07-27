//! Generic, report-only strategic posture, combo-family, and route classification.
//!
//! This module consumes only compiled roles, mana values, typed known-line
//! requirements, normalized card identities, and aggregate semantic coverage.
//! Names are treated only as opaque identities for connecting lines and as
//! display evidence; no score or rank interprets card, commander, strategy-plan,
//! or known-line text. It therefore describes the submitted list's structural
//! signature without asserting player intent, observed metagame performance,
//! or simulated timing.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::domain::{KnownLine, KnownLineOutcome, LineRequirement};
use crate::parser::normalize_card_name;
use crate::semantics::{CompiledCard, CompiledDeck, role};

pub(crate) const STRATEGIC_PROFILE_MODEL_VERSION: &str = "strategic-profile-0.4";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrategicProfileReport {
    pub model_version: String,
    pub use_policy: StrategicProfileUsePolicy,
    pub primary_posture: StrategicPosture,
    pub posture_ranking: Vec<RankedPosture>,
    pub primary_archetype: StrategicArchetype,
    pub archetype_ranking: Vec<RankedArchetype>,
    pub combo_family_ranking: Vec<RankedComboFamily>,
    pub combo_route_clusters: Vec<RankedComboRouteCluster>,
    pub evidence: StrategicEvidenceSummary,
    pub confidence: f32,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrategicProfileUsePolicy {
    pub disposition: StrategicProfileDisposition,
    pub affects_bracket_rating: bool,
    pub affects_simulation: bool,
    pub asserts_player_intent: bool,
}

impl Default for StrategicProfileUsePolicy {
    fn default() -> Self {
        Self {
            disposition: StrategicProfileDisposition::ReportOnly,
            affects_bracket_rating: false,
            affects_simulation: false,
            asserts_player_intent: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StrategicProfileDisposition {
    ReportOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum StrategicPosture {
    Turbo,
    Proactive,
    Adaptive,
    Reactive,
    Attrition,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum StrategicArchetype {
    TurboCombo,
    MidrangeCombo,
    BigManaCombo,
    ReactiveToolboxCombo,
    StaxCombo,
    EngineCombo,
    ProactiveMidrange,
    ReactiveControl,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum ComboFamily {
    CompactTableWin,
    GraveyardRecursion,
    SpellChain,
    PermanentEngine,
    InfiniteResource,
    BigManaPayoff,
    CombatConversion,
    TutorToolbox,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RankedPosture {
    pub posture: StrategicPosture,
    pub score: f32,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RankedArchetype {
    pub archetype: StrategicArchetype,
    pub score: f32,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RankedComboFamily {
    pub family: ComboFamily,
    pub score: f32,
    pub supporting_line_count: u16,
    pub evidence: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ComboRouteRank {
    Primary,
    Backup,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ComboRouteConversion {
    TableLethal,
    MixedTableLethalAndConversion,
    RequiresConversion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComboRouteCard {
    pub name: String,
    pub line_count: u16,
    pub appears_in_every_line: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RankedComboRouteCluster {
    /// Stable within this report and independent of card or line names.
    pub route_id: String,
    pub rank: ComboRouteRank,
    pub score: f32,
    pub line_count: u16,
    pub line_names: Vec<String>,
    /// Cards occurring in at least two lines in this route.
    pub central_cards: Vec<ComboRouteCard>,
    /// Cards occurring in exactly one line in this route.
    pub unique_cards: Vec<String>,
    pub outcomes: Vec<KnownLineOutcome>,
    pub best_confidence: f32,
    pub table_lethal_line_count: u16,
    pub conversion_required_line_count: u16,
    pub conversion: ComboRouteConversion,
    pub has_report_only_requirements: bool,
    pub report_only_line_count: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrategicEvidenceSummary {
    pub total_cards: u16,
    pub commander_slots: u16,
    pub land_slots: u16,
    pub nonland_slots: u16,
    pub fast_mana_slots: u16,
    pub ramp_slots: u16,
    pub tutor_slots: u16,
    pub draw_slots: u16,
    pub engine_slots: u16,
    pub interaction_slots: u16,
    pub protection_slots: u16,
    pub stax_slots: u16,
    pub payoff_slots: u16,
    pub combo_piece_slots: u16,
    pub graveyard_slots: u16,
    pub recursion_slots: u16,
    pub commander_engine_slots: u16,
    pub commander_tutor_slots: u16,
    pub known_line_count: u16,
    pub lethal_line_count: u16,
    pub compact_lethal_line_count: u16,
    pub report_only_line_count: u16,
    pub average_nonland_mana_value: f32,
    pub mean_card_semantic_confidence: f32,
    pub semantic_coverage: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct LineSignals {
    compact_table_win: f32,
    compact_table_win_count: u16,
    graveyard_recursion: f32,
    graveyard_recursion_count: u16,
    spell_chain: f32,
    spell_chain_count: u16,
    permanent_engine: f32,
    permanent_engine_count: u16,
    infinite_resource: f32,
    infinite_resource_count: u16,
    big_mana_payoff: f32,
    big_mana_payoff_count: u16,
    combat_conversion: f32,
    combat_conversion_count: u16,
}

impl LineSignals {
    fn diversity(self) -> f32 {
        let present = [
            self.compact_table_win,
            self.graveyard_recursion,
            self.spell_chain,
            self.permanent_engine,
            self.infinite_resource,
            self.big_mana_payoff,
            self.combat_conversion,
        ]
        .into_iter()
        .filter(|score| *score >= 0.25)
        .count();
        present as f32 / 7.0
    }
}

#[derive(Debug, Clone, Copy)]
struct NormalizedFeatures {
    fast_mana: f32,
    ramp: f32,
    tutors: f32,
    draw: f32,
    engines: f32,
    interaction: f32,
    protection: f32,
    stax: f32,
    payoffs: f32,
    combo_pieces: f32,
    graveyard: f32,
    recursion: f32,
    commander_engines: f32,
    commander_tutors: f32,
    low_curve: f32,
    high_curve: f32,
    combo_presence: f32,
}

/// Classify a compiled deck's structural strategic signature.
///
/// The result is deliberately report-only. It must not feed bracket scoring,
/// simulation decisions, or claims about the pilot's declared intent.
pub(crate) fn classify_strategic_profile(deck: &CompiledDeck) -> StrategicProfileReport {
    let evidence = summarize_evidence(deck);
    let line_signals = summarize_line_signals(deck);
    let normalized = normalize_features(&evidence);
    let combo_family_ranking = rank_combo_families(&evidence, normalized, line_signals);
    let combo_route_clusters = cluster_combo_routes(&deck.known_lines);
    let posture_ranking = rank_postures(&evidence, normalized, line_signals);
    let archetype_ranking = rank_archetypes(
        &evidence,
        normalized,
        &posture_ranking,
        &combo_family_ranking,
    );
    let primary_posture = posture_ranking
        .first()
        .map(|ranked| ranked.posture)
        .unwrap_or(StrategicPosture::Adaptive);
    let primary_archetype = archetype_ranking
        .first()
        .map(|ranked| ranked.archetype)
        .unwrap_or(StrategicArchetype::ProactiveMidrange);

    StrategicProfileReport {
        model_version: STRATEGIC_PROFILE_MODEL_VERSION.into(),
        use_policy: StrategicProfileUsePolicy::default(),
        primary_posture,
        posture_ranking,
        primary_archetype,
        archetype_ranking,
        combo_family_ranking,
        combo_route_clusters,
        confidence: profile_confidence(deck, &evidence),
        limitations: profile_limitations(deck, &evidence),
        evidence,
    }
}

fn summarize_evidence(deck: &CompiledDeck) -> StrategicEvidenceSummary {
    let total_cards = count_slots(&deck.cards, u32::MAX);
    let land_slots = count_slots(&deck.cards, role::LAND);
    let nonland_slots = total_cards.saturating_sub(land_slots);
    let known_line_count = bounded_count(deck.known_lines.len());
    let lethal_line_count = bounded_count(
        deck.known_lines
            .iter()
            .filter(|line| line_is_table_lethal(line))
            .count(),
    );
    let compact_lethal_line_count = bounded_count(
        deck.known_lines
            .iter()
            .filter(|line| line_is_table_lethal(line) && line.compactness <= 3)
            .count(),
    );
    let report_only_line_count = bounded_count(
        deck.known_lines
            .iter()
            .filter(|line| line_has_report_only_requirement(line))
            .count(),
    );

    StrategicEvidenceSummary {
        total_cards,
        commander_slots: deck
            .cards
            .iter()
            .filter(|card| card.is_commander)
            .fold(0u16, |total, card| total.saturating_add(card.quantity)),
        land_slots,
        nonland_slots,
        fast_mana_slots: count_slots(&deck.cards, role::FAST_MANA),
        ramp_slots: count_slots(&deck.cards, role::RAMP),
        tutor_slots: count_slots(&deck.cards, role::TUTOR),
        draw_slots: count_slots(&deck.cards, role::DRAW),
        engine_slots: count_slots(&deck.cards, role::ENGINE),
        interaction_slots: count_slots(
            &deck.cards,
            role::REMOVAL | role::COUNTERSPELL | role::BOARD_WIPE,
        ),
        protection_slots: count_slots(&deck.cards, role::PROTECTION),
        stax_slots: count_slots(&deck.cards, role::STAX),
        payoff_slots: count_slots(&deck.cards, role::PAYOFF | role::WIN_CONDITION),
        combo_piece_slots: count_slots(&deck.cards, role::COMBO_PIECE),
        graveyard_slots: count_slots(&deck.cards, role::GRAVEYARD),
        recursion_slots: count_slots(&deck.cards, role::RECURSION),
        commander_engine_slots: deck
            .cards
            .iter()
            .filter(|card| card.is_commander && card.has(role::ENGINE | role::DRAW | role::PAYOFF))
            .fold(0u16, |total, card| total.saturating_add(card.quantity)),
        commander_tutor_slots: deck
            .cards
            .iter()
            .filter(|card| card.is_commander && card.has(role::TUTOR))
            .fold(0u16, |total, card| total.saturating_add(card.quantity)),
        known_line_count,
        lethal_line_count,
        compact_lethal_line_count,
        report_only_line_count,
        average_nonland_mana_value: round_three(weighted_nonland_mana_value(&deck.cards)),
        mean_card_semantic_confidence: bounded_score(weighted_semantic_confidence(&deck.cards)),
        semantic_coverage: bounded_score(deck.semantic_coverage),
    }
}

fn normalize_features(evidence: &StrategicEvidenceSummary) -> NormalizedFeatures {
    let nonlands = evidence.nonland_slots.max(1);
    let commander_count = evidence.commander_slots.max(1);
    let combo_presence = (if evidence.lethal_line_count > 0 {
        0.65
    } else {
        0.0
    } + density(evidence.known_line_count, 3, 1.0) * 0.20
        + density(evidence.combo_piece_slots, nonlands, 0.10) * 0.15)
        .clamp(0.0, 1.0);

    NormalizedFeatures {
        fast_mana: density(evidence.fast_mana_slots, nonlands, 0.14),
        ramp: density(evidence.ramp_slots, nonlands, 0.25),
        tutors: density(evidence.tutor_slots, nonlands, 0.12),
        draw: density(evidence.draw_slots, nonlands, 0.15),
        engines: density(evidence.engine_slots, nonlands, 0.15),
        interaction: density(evidence.interaction_slots, nonlands, 0.28),
        protection: density(evidence.protection_slots, nonlands, 0.12),
        stax: density(evidence.stax_slots, nonlands, 0.10),
        payoffs: density(evidence.payoff_slots, nonlands, 0.14),
        combo_pieces: density(evidence.combo_piece_slots, nonlands, 0.10),
        graveyard: density(evidence.graveyard_slots, nonlands, 0.12),
        recursion: density(evidence.recursion_slots, nonlands, 0.10),
        commander_engines: (evidence.commander_engine_slots as f32 / commander_count as f32)
            .clamp(0.0, 1.0),
        commander_tutors: (evidence.commander_tutor_slots as f32 / commander_count as f32)
            .clamp(0.0, 1.0),
        low_curve: ((4.5 - evidence.average_nonland_mana_value) / 3.0).clamp(0.0, 1.0),
        high_curve: ((evidence.average_nonland_mana_value - 2.5) / 3.0).clamp(0.0, 1.0),
        combo_presence,
    }
}

fn rank_postures(
    evidence: &StrategicEvidenceSummary,
    features: NormalizedFeatures,
    lines: LineSignals,
) -> Vec<RankedPosture> {
    let line_diversity = lines.diversity();
    let turbo = features.fast_mana * 0.30
        + features.tutors * 0.23
        + lines.compact_table_win * 0.27
        + features.low_curve * 0.12
        + features.protection * 0.08;
    let proactive = features.combo_presence * 0.18
        + features.tutors * 0.18
        + features.ramp * 0.20
        + features.payoffs * 0.16
        + features.protection * 0.15
        + features.fast_mana * 0.13;
    let adaptive = features.engines * 0.27
        + features.draw * 0.23
        + features.tutors * 0.17
        + features.ramp * 0.13
        + features.commander_engines * 0.12
        + line_diversity * 0.08;
    let reactive = features.interaction * 0.45
        + features.protection * 0.16
        + features.stax * 0.14
        + features.tutors * 0.13
        + features.engines * 0.12;
    let attrition = features.stax * 0.30
        + features.engines * 0.25
        + features.recursion * 0.20
        + features.interaction * 0.15
        + features.high_curve * 0.10;

    let mut ranking = vec![
        RankedPosture {
            posture: StrategicPosture::Turbo,
            score: turbo,
            evidence: format!(
                "{} fast-mana, {} tutor, and {} compact lethal slots/lines.",
                evidence.fast_mana_slots, evidence.tutor_slots, evidence.compact_lethal_line_count
            ),
        },
        RankedPosture {
            posture: StrategicPosture::Proactive,
            score: proactive,
            evidence: format!(
                "{} lethal lines, {} payoffs, and {} protection slots.",
                evidence.lethal_line_count, evidence.payoff_slots, evidence.protection_slots
            ),
        },
        RankedPosture {
            posture: StrategicPosture::Adaptive,
            score: adaptive,
            evidence: format!(
                "{} engines, {} draw slots, and {} commander-engine slots.",
                evidence.engine_slots, evidence.draw_slots, evidence.commander_engine_slots
            ),
        },
        RankedPosture {
            posture: StrategicPosture::Reactive,
            score: reactive,
            evidence: format!(
                "{} interaction, {} protection, and {} stax slots.",
                evidence.interaction_slots, evidence.protection_slots, evidence.stax_slots
            ),
        },
        RankedPosture {
            posture: StrategicPosture::Attrition,
            score: attrition,
            evidence: format!(
                "{} stax, {} recursion, and {} engine slots.",
                evidence.stax_slots, evidence.recursion_slots, evidence.engine_slots
            ),
        },
    ];
    sort_postures(&mut ranking);
    ranking
}

fn rank_archetypes(
    evidence: &StrategicEvidenceSummary,
    features: NormalizedFeatures,
    postures: &[RankedPosture],
    combo_families: &[RankedComboFamily],
) -> Vec<RankedArchetype> {
    let posture = |kind| {
        postures
            .iter()
            .find(|ranked| ranked.posture == kind)
            .map(|ranked| ranked.score)
            .unwrap_or(0.0)
    };
    let family = |kind| {
        combo_families
            .iter()
            .find(|ranked| ranked.family == kind)
            .map(|ranked| ranked.score)
            .unwrap_or(0.0)
    };
    let turbo = posture(StrategicPosture::Turbo);
    let proactive = posture(StrategicPosture::Proactive);
    let adaptive = posture(StrategicPosture::Adaptive);
    let reactive = posture(StrategicPosture::Reactive);
    let compact = family(ComboFamily::CompactTableWin);
    let big_mana =
        family(ComboFamily::BigManaPayoff).max(family(ComboFamily::InfiniteResource) * 0.85);
    let toolbox = family(ComboFamily::TutorToolbox);
    let engine_family =
        family(ComboFamily::PermanentEngine).max(family(ComboFamily::InfiniteResource));

    let mut ranking = vec![
        RankedArchetype {
            archetype: StrategicArchetype::TurboCombo,
            score: turbo * 0.55 + compact * 0.25 + features.combo_presence * 0.20,
            evidence: format!(
                "{} compact lethal lines with {} fast-mana slots.",
                evidence.compact_lethal_line_count, evidence.fast_mana_slots
            ),
        },
        RankedArchetype {
            archetype: StrategicArchetype::MidrangeCombo,
            score: (adaptive * 0.35
                + features.engines * 0.20
                + features.ramp * 0.15
                + features.combo_presence * 0.30)
                * (1.0 - turbo * 0.20),
            evidence: format!(
                "{} engines support {} known combo lines.",
                evidence.engine_slots, evidence.known_line_count
            ),
        },
        RankedArchetype {
            archetype: StrategicArchetype::BigManaCombo,
            score: big_mana * 0.55
                + features.ramp * 0.18
                + adaptive * 0.15
                + features.combo_presence * 0.12,
            evidence: format!(
                "Average nonland mana value {:.2} with {} ramp slots.",
                evidence.average_nonland_mana_value, evidence.ramp_slots
            ),
        },
        RankedArchetype {
            archetype: StrategicArchetype::ReactiveToolboxCombo,
            score: reactive * 0.40
                + toolbox * 0.28
                + features.combo_presence * 0.17
                + features.tutors * 0.15
                + features.commander_tutors * 0.20,
            evidence: format!(
                "{} tutors ({} in the command zone) and {} interaction slots support {} lines.",
                evidence.tutor_slots,
                evidence.commander_tutor_slots,
                evidence.interaction_slots,
                evidence.known_line_count
            ),
        },
        RankedArchetype {
            archetype: StrategicArchetype::StaxCombo,
            score: features.stax * 0.35 + reactive * 0.25 + features.combo_presence * 0.40,
            evidence: format!(
                "{} stax slots coexist with {} lethal lines.",
                evidence.stax_slots, evidence.lethal_line_count
            ),
        },
        RankedArchetype {
            archetype: StrategicArchetype::EngineCombo,
            score: engine_family * 0.40 + features.engines * 0.20 + features.combo_presence * 0.40,
            evidence: format!(
                "{} engine slots and {} known lines form repeatable resources.",
                evidence.engine_slots, evidence.known_line_count
            ),
        },
        RankedArchetype {
            archetype: StrategicArchetype::ProactiveMidrange,
            score: (proactive * 0.45 + adaptive * 0.35 + features.engines * 0.20)
                * (1.0 - features.combo_presence * 0.70),
            evidence: format!(
                "{} engines, {} payoffs, and {} protection slots.",
                evidence.engine_slots, evidence.payoff_slots, evidence.protection_slots
            ),
        },
        RankedArchetype {
            archetype: StrategicArchetype::ReactiveControl,
            score: reactive * 0.55 + features.interaction * 0.30 + features.stax * 0.15,
            evidence: format!(
                "{} interaction and {} stax slots anchor the control plan.",
                evidence.interaction_slots, evidence.stax_slots
            ),
        },
    ];
    sort_archetypes(&mut ranking);
    ranking
}

fn rank_combo_families(
    evidence: &StrategicEvidenceSummary,
    features: NormalizedFeatures,
    lines: LineSignals,
) -> Vec<RankedComboFamily> {
    let diversity = lines.diversity();
    let known_line_density = density(evidence.known_line_count, 3, 1.0);
    let entries = [
        (
            ComboFamily::CompactTableWin,
            lines.compact_table_win * 0.80 + features.tutors * 0.12 + features.fast_mana * 0.08,
            lines.compact_table_win_count,
            format!(
                "{} compact lethal lines backed by {} tutors.",
                evidence.compact_lethal_line_count, evidence.tutor_slots
            ),
        ),
        (
            ComboFamily::GraveyardRecursion,
            lines.graveyard_recursion * 0.65
                + features.graveyard * 0.20
                + features.recursion * 0.15,
            lines.graveyard_recursion_count,
            format!(
                "{} graveyard and {} recursion slots.",
                evidence.graveyard_slots, evidence.recursion_slots
            ),
        ),
        (
            ComboFamily::SpellChain,
            lines.spell_chain * 0.65 + features.combo_pieces * 0.20 + features.tutors * 0.15,
            lines.spell_chain_count,
            "Known-line pieces and deck roles favor spell sequencing.".into(),
        ),
        (
            ComboFamily::PermanentEngine,
            lines.permanent_engine * 0.60 + features.engines * 0.25 + features.payoffs * 0.15,
            lines.permanent_engine_count,
            format!(
                "{} engine and {} payoff slots support permanent loops.",
                evidence.engine_slots, evidence.payoff_slots
            ),
        ),
        (
            ComboFamily::InfiniteResource,
            lines.infinite_resource * 0.78 + features.payoffs * 0.22,
            lines.infinite_resource_count,
            "Known-line outcomes include repeatable resource generation.".into(),
        ),
        (
            ComboFamily::BigManaPayoff,
            lines.big_mana_payoff * 0.70 + features.ramp * 0.20 + features.high_curve * 0.10,
            lines.big_mana_payoff_count,
            format!(
                "{} ramp slots support an average nonland mana value of {:.2}.",
                evidence.ramp_slots, evidence.average_nonland_mana_value
            ),
        ),
        (
            ComboFamily::CombatConversion,
            lines.combat_conversion * 0.75 + features.payoffs * 0.15 + features.protection * 0.10,
            lines.combat_conversion_count,
            "Known-line requirements explicitly include combat access.".into(),
        ),
        (
            ComboFamily::TutorToolbox,
            features.tutors * 0.45
                + features.interaction * 0.35
                + features.commander_tutors * 0.20
                + diversity * 0.15
                + known_line_density * 0.05,
            evidence.known_line_count,
            format!(
                "{} tutors ({} in the command zone) bridge {} interaction slots and {} line families.",
                evidence.tutor_slots,
                evidence.commander_tutor_slots,
                evidence.interaction_slots,
                (diversity * 7.0).round() as u8
            ),
        ),
    ];

    let mut ranking = entries
        .into_iter()
        .map(
            |(family, score, supporting_line_count, evidence)| RankedComboFamily {
                family,
                score,
                supporting_line_count,
                evidence,
            },
        )
        .collect::<Vec<_>>();
    sort_combo_families(&mut ranking);
    ranking
}

#[derive(Debug)]
struct UnrankedComboRouteCluster {
    first_line_index: usize,
    cluster: RankedComboRouteCluster,
}

#[derive(Debug)]
struct RouteCardAccumulator {
    display_name: String,
    line_count: u16,
    first_line_position: usize,
    first_card_position: usize,
}

fn cluster_combo_routes(lines: &[KnownLine]) -> Vec<RankedComboRouteCluster> {
    if lines.is_empty() {
        return Vec::new();
    }

    let line_identities = lines
        .iter()
        .map(unique_line_card_identities)
        .collect::<Vec<_>>();
    let mut parents = (0..lines.len()).collect::<Vec<_>>();
    let mut first_line_for_card = HashMap::<String, usize>::new();

    for (line_index, identities) in line_identities.iter().enumerate() {
        for identity in identities {
            if let Some(first_line_index) = first_line_for_card.get(identity).copied() {
                union_route_lines(&mut parents, line_index, first_line_index);
            } else {
                first_line_for_card.insert(identity.clone(), line_index);
            }
        }
    }

    let mut grouped_line_indices = BTreeMap::<usize, Vec<usize>>::new();
    for line_index in 0..lines.len() {
        let root = route_root(&mut parents, line_index);
        grouped_line_indices
            .entry(root)
            .or_default()
            .push(line_index);
    }

    let mut candidates = grouped_line_indices
        .into_values()
        .map(|line_indices| build_combo_route_cluster(lines, &line_indices))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .cluster
            .score
            .total_cmp(&left.cluster.score)
            .then_with(|| right.cluster.line_count.cmp(&left.cluster.line_count))
            .then_with(|| {
                right
                    .cluster
                    .table_lethal_line_count
                    .cmp(&left.cluster.table_lethal_line_count)
            })
            .then_with(|| {
                right
                    .cluster
                    .best_confidence
                    .total_cmp(&left.cluster.best_confidence)
            })
            .then_with(|| left.first_line_index.cmp(&right.first_line_index))
    });

    candidates
        .into_iter()
        .enumerate()
        .map(|(index, mut candidate)| {
            candidate.cluster.route_id = format!("route-{}", index + 1);
            candidate.cluster.rank = if index == 0 {
                ComboRouteRank::Primary
            } else {
                ComboRouteRank::Backup
            };
            candidate.cluster.score = bounded_score(candidate.cluster.score);
            candidate.cluster
        })
        .collect()
}

fn unique_line_card_identities(line: &KnownLine) -> Vec<String> {
    let mut seen = HashSet::new();
    line.cards
        .iter()
        .filter_map(|card_name| {
            let identity = normalize_card_name(card_name);
            if identity.is_empty() || !seen.insert(identity.clone()) {
                None
            } else {
                Some(identity)
            }
        })
        .collect()
}

fn route_root(parents: &mut [usize], index: usize) -> usize {
    let mut root = index;
    while parents[root] != root {
        root = parents[root];
    }

    let mut current = index;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

fn union_route_lines(parents: &mut [usize], left: usize, right: usize) {
    let left_root = route_root(parents, left);
    let right_root = route_root(parents, right);
    if left_root == right_root {
        return;
    }
    let root = left_root.min(right_root);
    let child = left_root.max(right_root);
    parents[child] = root;
}

fn build_combo_route_cluster(
    lines: &[KnownLine],
    line_indices: &[usize],
) -> UnrankedComboRouteCluster {
    let line_count = bounded_count(line_indices.len());
    let mut card_evidence = HashMap::<String, RouteCardAccumulator>::new();
    let mut outcomes = Vec::new();
    let mut best_confidence = 0.0f32;
    let mut table_lethal_line_count = 0u16;
    let mut report_only_line_count = 0u16;
    let mut compactness_total = 0.0f32;

    for (line_position, line_index) in line_indices.iter().copied().enumerate() {
        let line = &lines[line_index];
        let mut seen_in_line = HashSet::new();
        for (card_position, card_name) in line.cards.iter().enumerate() {
            let identity = normalize_card_name(card_name);
            if identity.is_empty() || !seen_in_line.insert(identity.clone()) {
                continue;
            }
            let entry = card_evidence
                .entry(identity)
                .or_insert_with(|| RouteCardAccumulator {
                    display_name: card_name.clone(),
                    line_count: 0,
                    first_line_position: line_position,
                    first_card_position: card_position,
                });
            entry.line_count = entry.line_count.saturating_add(1);
        }

        if !outcomes.contains(&line.outcome) {
            outcomes.push(line.outcome);
        }
        best_confidence = best_confidence.max(line.model_confidence.clamp(0.0, 1.0));
        if line_is_table_lethal(line) {
            table_lethal_line_count = table_lethal_line_count.saturating_add(1);
        }
        if line_has_report_only_requirement(line) {
            report_only_line_count = report_only_line_count.saturating_add(1);
        }
        compactness_total += route_compactness_score(line.compactness);
    }

    outcomes.sort_by_key(|outcome| route_outcome_order(*outcome));

    let mut central_cards = Vec::new();
    let mut unique_cards = Vec::new();
    for evidence in card_evidence.into_values() {
        if evidence.line_count >= 2 {
            central_cards.push((
                evidence.first_line_position,
                evidence.first_card_position,
                ComboRouteCard {
                    name: evidence.display_name,
                    line_count: evidence.line_count,
                    appears_in_every_line: evidence.line_count == line_count,
                },
            ));
        } else {
            unique_cards.push((
                evidence.first_line_position,
                evidence.first_card_position,
                evidence.display_name,
            ));
        }
    }
    central_cards.sort_by(|left, right| {
        right
            .2
            .line_count
            .cmp(&left.2.line_count)
            .then_with(|| left.0.cmp(&right.0))
            .then_with(|| left.1.cmp(&right.1))
    });
    unique_cards.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let central_cards = central_cards
        .into_iter()
        .map(|(_, _, evidence)| evidence)
        .collect::<Vec<_>>();
    let unique_cards = unique_cards
        .into_iter()
        .map(|(_, _, name)| name)
        .collect::<Vec<_>>();
    let conversion_required_line_count = line_count.saturating_sub(table_lethal_line_count);
    let conversion = match (
        table_lethal_line_count > 0,
        conversion_required_line_count > 0,
    ) {
        (true, false) => ComboRouteConversion::TableLethal,
        (true, true) => ComboRouteConversion::MixedTableLethalAndConversion,
        (false, _) => ComboRouteConversion::RequiresConversion,
    };
    let score = combo_route_score(
        line_count,
        &central_cards,
        table_lethal_line_count,
        best_confidence,
        compactness_total,
        report_only_line_count,
    );

    UnrankedComboRouteCluster {
        first_line_index: line_indices.first().copied().unwrap_or(usize::MAX),
        cluster: RankedComboRouteCluster {
            route_id: String::new(),
            rank: ComboRouteRank::Backup,
            score,
            line_count,
            line_names: line_indices
                .iter()
                .map(|index| lines[*index].name.clone())
                .collect(),
            central_cards,
            unique_cards,
            outcomes,
            best_confidence: bounded_score(best_confidence),
            table_lethal_line_count,
            conversion_required_line_count,
            conversion,
            has_report_only_requirements: report_only_line_count > 0,
            report_only_line_count,
        },
    }
}

fn combo_route_score(
    line_count: u16,
    central_cards: &[ComboRouteCard],
    table_lethal_line_count: u16,
    best_confidence: f32,
    compactness_total: f32,
    report_only_line_count: u16,
) -> f32 {
    let population = line_count.max(1) as f32;
    let line_count_strength = (line_count as f32 / 3.0).clamp(0.0, 1.0);
    let shared_strength = if line_count <= 1 || central_cards.is_empty() {
        0.0
    } else {
        let widest_shared_card = central_cards
            .iter()
            .map(|card| card.line_count)
            .max()
            .unwrap_or(0) as f32
            / population;
        let shared_breadth = (central_cards.len() as f32 / 3.0).clamp(0.0, 1.0);
        widest_shared_card * 0.75 + shared_breadth * 0.25
    };
    let lethal_fraction = table_lethal_line_count as f32 / population;
    let compactness = compactness_total / population;
    let report_only_fraction = report_only_line_count as f32 / population;

    // Route ranking describes deck construction rather than executor support:
    // repeated branches around a shared package are stronger structural
    // evidence than a single isolated line. Execution support remains visible
    // separately and report-only records retain a modest confidence penalty.
    (line_count_strength * 0.38
        + shared_strength * 0.37
        + lethal_fraction * 0.07
        + best_confidence.clamp(0.0, 1.0) * 0.08
        + compactness * 0.10
        - report_only_fraction * 0.04)
        .clamp(0.0, 1.0)
}

fn route_compactness_score(compactness: u8) -> f32 {
    match compactness {
        0..=2 => 1.0,
        3 => 0.80,
        4 => 0.55,
        5 => 0.30,
        _ => 0.10,
    }
}

fn route_outcome_order(outcome: KnownLineOutcome) -> u8 {
    match outcome {
        KnownLineOutcome::TableWin => 0,
        KnownLineOutcome::InfiniteMana => 1,
        KnownLineOutcome::InfiniteEngine => 2,
        KnownLineOutcome::Engine => 3,
    }
}

fn summarize_line_signals(deck: &CompiledDeck) -> LineSignals {
    let mut signals = LineSignals::default();
    for line in &deck.known_lines {
        let weight = line_weight(line);
        if weight <= 0.0 {
            continue;
        }
        let lethal = line_is_table_lethal(line);
        let compactness = match line.compactness {
            0..=2 => 1.0,
            3 => 0.78,
            4 => 0.38,
            _ => 0.12,
        };
        if lethal {
            signals.compact_table_win = signals.compact_table_win.max(weight * compactness);
            if line.compactness <= 3 {
                signals.compact_table_win_count = signals.compact_table_win_count.saturating_add(1);
            }
        }

        let graveyard_requirement = line
            .simulation_requirements
            .iter()
            .any(|requirement| matches!(requirement, LineRequirement::GraveyardSetup { .. }));
        let graveyard_piece_fraction =
            line_piece_fraction(deck, line, role::GRAVEYARD | role::RECURSION);
        if graveyard_requirement || graveyard_piece_fraction > 0.0 {
            let strength = if graveyard_requirement {
                1.0
            } else {
                graveyard_piece_fraction
            };
            signals.graveyard_recursion = signals.graveyard_recursion.max(weight * strength);
            signals.graveyard_recursion_count = signals.graveyard_recursion_count.saturating_add(1);
        }

        let spell_fraction =
            line_piece_fraction(deck, line, role::INSTANT_SORCERY | role::SPELL_MATTERS);
        if spell_fraction > 0.0 {
            signals.spell_chain = signals.spell_chain.max(weight * spell_fraction);
            signals.spell_chain_count = signals.spell_chain_count.saturating_add(1);
        }

        let permanent_fraction = line_piece_fraction(
            deck,
            line,
            role::CREATURE | role::ARTIFACT | role::ENCHANTMENT,
        );
        let engine_piece_fraction = line_piece_fraction(deck, line, role::ENGINE | role::ENABLER);
        let repeatable_engine_outcome = line.outcome == KnownLineOutcome::InfiniteEngine;
        if permanent_fraction > 0.0 && (repeatable_engine_outcome || engine_piece_fraction > 0.0) {
            let engine_strength = if repeatable_engine_outcome {
                permanent_fraction
            } else {
                permanent_fraction * engine_piece_fraction
            };
            signals.permanent_engine = signals.permanent_engine.max(weight * engine_strength);
            signals.permanent_engine_count = signals.permanent_engine_count.saturating_add(1);
        }

        if matches!(
            line.outcome,
            KnownLineOutcome::InfiniteMana | KnownLineOutcome::InfiniteEngine
        ) {
            signals.infinite_resource = signals.infinite_resource.max(weight);
            signals.infinite_resource_count = signals.infinite_resource_count.saturating_add(1);
        }

        let required_mana = line_required_mana(line);
        let average_piece_mana = line_piece_average_mana_value(deck, line);
        let big_mana_strength = if required_mana >= 7 {
            1.0
        } else if required_mana >= 5 {
            0.75
        } else if average_piece_mana_value_is_high(average_piece_mana) {
            0.60
        } else {
            0.0
        };
        if big_mana_strength > 0.0 {
            signals.big_mana_payoff = signals.big_mana_payoff.max(weight * big_mana_strength);
            signals.big_mana_payoff_count = signals.big_mana_payoff_count.saturating_add(1);
        }

        if line
            .simulation_requirements
            .contains(&LineRequirement::CombatAccess)
        {
            signals.combat_conversion = signals.combat_conversion.max(weight);
            signals.combat_conversion_count = signals.combat_conversion_count.saturating_add(1);
        }
    }
    signals
}

fn line_weight(line: &KnownLine) -> f32 {
    let report_only_penalty = if line_has_report_only_requirement(line) {
        0.75
    } else {
        1.0
    };
    line.model_confidence.clamp(0.0, 1.0) * report_only_penalty
}

fn line_has_report_only_requirement(line: &KnownLine) -> bool {
    line.simulation_requirements.iter().any(|requirement| {
        matches!(
            requirement,
            LineRequirement::TotalExecutionMana
                | LineRequirement::CombatAccess
                | LineRequirement::Unmodeled
        )
    })
}

fn line_is_table_lethal(line: &KnownLine) -> bool {
    line.table_lethal_if_resolved || line.outcome == KnownLineOutcome::TableWin
}

fn line_piece_fraction(deck: &CompiledDeck, line: &KnownLine, role_mask: u32) -> f32 {
    let mut matched = 0u16;
    let mut relevant = 0u16;
    for line_card in &line.cards {
        let normalized = normalize_card_name(line_card);
        if let Some(card) = deck
            .cards
            .iter()
            .find(|card| card.normalized_name == normalized)
        {
            matched = matched.saturating_add(1);
            if card.has(role_mask) {
                relevant = relevant.saturating_add(1);
            }
        }
    }
    if matched == 0 {
        0.0
    } else {
        relevant as f32 / matched as f32
    }
}

fn line_piece_average_mana_value(deck: &CompiledDeck, line: &KnownLine) -> f32 {
    let mut total = 0.0;
    let mut matched = 0u16;
    for line_card in &line.cards {
        let normalized = normalize_card_name(line_card);
        if let Some(card) = deck
            .cards
            .iter()
            .find(|card| card.normalized_name == normalized)
        {
            total += card.mana_value.max(0.0);
            matched = matched.saturating_add(1);
        }
    }
    if matched == 0 {
        0.0
    } else {
        total / matched as f32
    }
}

fn line_required_mana(line: &KnownLine) -> u16 {
    let printed = line
        .mana_needed
        .as_deref()
        .map(parse_mana_value)
        .unwrap_or(0);
    line.simulation_requirements
        .iter()
        .fold(printed, |highest, requirement| {
            let requirement_mana = match requirement {
                LineRequirement::NonlandManaCapacity { minimum } => *minimum as u16,
                LineRequirement::AdditionalActivationMana { cost } => parse_mana_value(cost),
                _ => 0,
            };
            highest.max(requirement_mana)
        })
}

fn parse_mana_value(cost: &str) -> u16 {
    cost.split('{')
        .skip(1)
        .filter_map(|part| part.split('}').next())
        .map(|symbol| {
            if symbol.eq_ignore_ascii_case("x") {
                0
            } else if let Ok(generic) = symbol.parse::<u16>() {
                generic
            } else {
                symbol
                    .split('/')
                    .find_map(|part| part.parse::<u16>().ok())
                    .unwrap_or(1)
            }
        })
        .fold(0u16, u16::saturating_add)
}

fn average_piece_mana_value_is_high(value: f32) -> bool {
    value >= 4.5
}

fn count_slots(cards: &[CompiledCard], mask: u32) -> u16 {
    cards
        .iter()
        .filter(|card| mask == u32::MAX || card.has(mask))
        .fold(0u16, |total, card| total.saturating_add(card.quantity))
}

fn weighted_nonland_mana_value(cards: &[CompiledCard]) -> f32 {
    let mut total = 0.0;
    let mut slots = 0u16;
    for card in cards.iter().filter(|card| !card.has(role::LAND)) {
        total += card.mana_value.max(0.0) * card.quantity as f32;
        slots = slots.saturating_add(card.quantity);
    }
    if slots == 0 {
        0.0
    } else {
        total / slots as f32
    }
}

fn weighted_semantic_confidence(cards: &[CompiledCard]) -> f32 {
    let mut total = 0.0;
    let mut slots = 0u16;
    for card in cards {
        total += card.semantic_confidence.clamp(0.0, 1.0) * card.quantity as f32;
        slots = slots.saturating_add(card.quantity);
    }
    if slots == 0 {
        0.0
    } else {
        total / slots as f32
    }
}

fn density(slots: u16, population: u16, target_share: f32) -> f32 {
    if population == 0 || target_share <= 0.0 {
        return 0.0;
    }
    (slots as f32 / population as f32 / target_share).clamp(0.0, 1.0)
}

fn profile_confidence(deck: &CompiledDeck, evidence: &StrategicEvidenceSummary) -> f32 {
    let mean_line_confidence = if deck.known_lines.is_empty() {
        evidence.semantic_coverage
    } else {
        deck.known_lines
            .iter()
            .map(|line| line.model_confidence.clamp(0.0, 1.0))
            .sum::<f32>()
            / deck.known_lines.len() as f32
    };
    bounded_score(
        (evidence.semantic_coverage * 0.55
            + evidence.mean_card_semantic_confidence * 0.25
            + mean_line_confidence * 0.15
            + (evidence.total_cards as f32 / 100.0).clamp(0.0, 1.0) * 0.05)
            .clamp(0.0, 1.0),
    )
}

fn profile_limitations(deck: &CompiledDeck, evidence: &StrategicEvidenceSummary) -> Vec<String> {
    let mut limitations = vec![
        "Structural classification is report-only and does not affect bracket scoring or simulation."
            .into(),
        "Card structure cannot establish pilot intent, mulligan policy, pod composition, or observed timing."
            .into(),
        "Combo routes are identity-overlap clusters; otherwise distinct plans sharing a bridge card may appear as one route."
            .into(),
    ];
    if evidence.semantic_coverage < 0.85 {
        limitations.push(format!(
            "Semantic coverage is {:.0}%; posture evidence is incomplete.",
            evidence.semantic_coverage * 100.0
        ));
    }
    if !deck.approximated_cards.is_empty() {
        limitations.push(format!(
            "{} cards use approximated semantics.",
            deck.approximated_cards.len()
        ));
    }
    if evidence.known_line_count == 0 {
        limitations.push(
            "No typed known line was available; combo-family ranks rely on role density.".into(),
        );
    } else if evidence.report_only_line_count > 0 {
        limitations.push(format!(
            "{} known lines contain report-only execution requirements.",
            evidence.report_only_line_count
        ));
    }
    limitations
}

fn bounded_count(value: usize) -> u16 {
    value.min(u16::MAX as usize) as u16
}

fn bounded_score(value: f32) -> f32 {
    (value.clamp(0.0, 1.0) * 1_000.0).round() / 1_000.0
}

fn round_three(value: f32) -> f32 {
    (value * 1_000.0).round() / 1_000.0
}

fn sort_postures(ranking: &mut [RankedPosture]) {
    ranking.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.posture.cmp(&right.posture))
    });
    for ranked in ranking {
        ranked.score = bounded_score(ranked.score);
    }
}

fn sort_archetypes(ranking: &mut [RankedArchetype]) {
    ranking.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.archetype.cmp(&right.archetype))
    });
    for ranked in ranking {
        ranked.score = bounded_score(ranked.score);
    }
}

fn sort_combo_families(ranking: &mut [RankedComboFamily]) {
    ranking.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.family.cmp(&right.family))
    });
    for ranked in ranking {
        ranked.score = bounded_score(ranked.score);
    }
}
