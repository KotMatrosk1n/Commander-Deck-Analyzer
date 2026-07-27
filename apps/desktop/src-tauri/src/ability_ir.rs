//! Conservative, report-only card-function representation.
//!
//! This module turns reviewed Oracle-text signals into typed abilities,
//! costs, resources, and events. It intentionally does not execute an
//! ability in the bounded simulator. Unsupported or ambiguous clauses remain
//! visible through coverage counts instead of being guessed. Card names are
//! identity/display data only and never select a semantic profile.

use std::collections::{BTreeSet, HashSet};

use crate::domain::{
    CardDefinition, KnownLine, SynergyGraph, SynergyLink, SynergyRelation, SynergyResourceCoverage,
};
use crate::effects::{EffectDescriptor, EffectMagnitude};

pub(crate) const ABILITY_IR_VERSION: &str = "ability-ir-0.3";
pub(crate) const SYNERGY_GRAPH_VERSION: &str = "synergy-graph-0.3";
const MAX_DISPLAYED_LINKS: usize = 48;
const MAX_COMMANDER_LINKS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbilityKind {
    Spell,
    Activated,
    Triggered,
    Static,
    Replacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbilityTiming {
    Resolution,
    Activated,
    EntersBattlefield,
    Dies,
    Cast,
    Upkeep,
    Combat,
    Static,
    Replacement,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbilityCostKind {
    Mana,
    Tap,
    Sacrifice,
    Discard,
    Life,
    Exile,
    Additional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum SynergySignal {
    CreatureBody,
    CreatureToken,
    Token,
    Artifact,
    Enchantment,
    /// An instant or sorcery cast by the analyzed deck's controller.
    SpellCast,
    /// A spell cast by an opponent. The deck-only graph has no producer for
    /// this signal; retaining the scope in the IR prevents opponent-triggered
    /// cards from being joined to the analyzed deck's own spells.
    OpponentSpellCast,
    DeathEvent,
    SacrificeEvent,
    Graveyard,
    LandEntering,
    CardAccess,
    Mana,
    Counter,
    LifeGained,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DeathDependency {
    Generic,
    AnotherCreatureYouControl,
}

impl SynergySignal {
    fn label(self) -> &'static str {
        match self {
            Self::CreatureBody => "creature bodies",
            Self::CreatureToken => "creature tokens",
            Self::Token => "tokens",
            Self::Artifact => "artifacts",
            Self::Enchantment => "enchantments",
            Self::SpellCast => "instant/sorcery casts",
            Self::OpponentSpellCast => "opponent spell casts",
            Self::DeathEvent => "creature death events",
            Self::SacrificeEvent => "sacrifice events",
            Self::Graveyard => "graveyard resources",
            Self::LandEntering => "lands entering",
            Self::CardAccess => "card access",
            Self::Mana => "mana",
            Self::Counter => "counters",
            Self::LifeGained => "life-gain events",
        }
    }

    fn is_event(self) -> bool {
        matches!(
            self,
            Self::SpellCast
                | Self::OpponentSpellCast
                | Self::DeathEvent
                | Self::SacrificeEvent
                | Self::LandEntering
                | Self::LifeGained
        )
    }

    fn specificity(self) -> u8 {
        match self {
            Self::DeathEvent | Self::SacrificeEvent => 5,
            Self::CreatureToken
            | Self::Token
            | Self::SpellCast
            | Self::OpponentSpellCast
            | Self::Graveyard => 4,
            Self::Artifact
            | Self::Enchantment
            | Self::LandEntering
            | Self::Counter
            | Self::LifeGained => 3,
            Self::CreatureBody => 2,
            Self::CardAccess | Self::Mana => 1,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct AbilityNode {
    pub kind: AbilityKind,
    pub timing: AbilityTiming,
    pub costs: Vec<AbilityCostKind>,
    pub produces: BTreeSet<SynergySignal>,
    pub consumes: BTreeSet<SynergySignal>,
    pub evidence: String,
    pub confidence: f32,
    /// Until an executor explicitly supports this node, the IR is evidence
    /// for reports and scoring research only.
    pub report_only: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AbilityProfile {
    pub abilities: Vec<AbilityNode>,
    pub produces: BTreeSet<SynergySignal>,
    pub consumes: BTreeSet<SynergySignal>,
    death_dependencies: BTreeSet<DeathDependency>,
    pub confidence: f32,
    pub unsupported_clause_count: u32,
    pub is_land: bool,
}

impl AbilityProfile {
    pub fn unresolved() -> Self {
        Self {
            confidence: 0.0,
            unsupported_clause_count: 1,
            ..Self::default()
        }
    }
}

pub(crate) struct GraphCard<'a> {
    pub name: &'a str,
    pub normalized_name: &'a str,
    pub quantity: u16,
    pub is_commander: bool,
    pub profile: &'a AbilityProfile,
}

/// Compile a card into a closed, typed report representation. The existing
/// effect descriptor supplies only behavior that has already passed its
/// conservative parser; line-level Oracle inspection adds trigger and
/// replacement structure without making any node executable.
pub(crate) fn compile_ability_profile(
    card: &CardDefinition,
    effects: &EffectDescriptor,
) -> AbilityProfile {
    let lower_type = card.type_line.to_ascii_lowercase();
    let mut profile = AbilityProfile {
        confidence: effects.confidence,
        unsupported_clause_count: effects.unsupported_clauses.len() as u32,
        is_land: effects.card_types.is_land,
        ..AbilityProfile::default()
    };

    if effects.card_types.is_creature {
        profile.produces.insert(SynergySignal::CreatureBody);
    }
    if effects.card_types.is_artifact {
        profile.produces.insert(SynergySignal::Artifact);
    }
    if effects.card_types.is_enchantment {
        profile.produces.insert(SynergySignal::Enchantment);
    }
    if effects.card_types.is_instant || effects.card_types.is_sorcery {
        profile.produces.insert(SynergySignal::SpellCast);
    }
    if effects.card_types.is_land {
        profile.produces.insert(SynergySignal::LandEntering);
    }

    if magnitude_present(effects.creature_tokens) {
        profile.produces.extend([
            SynergySignal::CreatureToken,
            SynergySignal::CreatureBody,
            SynergySignal::Token,
        ]);
    }
    if magnitude_present(effects.treasure_tokens) {
        profile.produces.extend([
            SynergySignal::Token,
            SynergySignal::Artifact,
            SynergySignal::Mana,
        ]);
    }
    if magnitude_present(effects.mana_produced) || magnitude_present(effects.lands_to_battlefield) {
        profile.produces.insert(SynergySignal::Mana);
    }
    if magnitude_present(effects.lands_to_battlefield) {
        profile.produces.insert(SynergySignal::LandEntering);
    }
    if magnitude_present(effects.draw_cards)
        || magnitude_present(effects.impulse_access)
        || !effects.tutor.instructions.is_empty()
    {
        profile.produces.insert(SynergySignal::CardAccess);
    }
    if effects.recursion {
        profile.consumes.insert(SynergySignal::Graveyard);
        profile.produces.insert(SynergySignal::CardAccess);
    }

    let oracle_lines = card
        .oracle_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    for line in oracle_lines {
        if let Some(dependency) = death_dependency(&line.to_ascii_lowercase()) {
            profile.death_dependencies.insert(dependency);
        }
        let node = compile_ability_node(line, &lower_type, effects.confidence);
        profile.produces.extend(node.produces.iter().copied());
        profile.consumes.extend(node.consumes.iter().copied());
        profile.abilities.push(node);
    }

    // The descriptor can identify an additional or activated sacrifice even
    // when reminder text or modal formatting kept it out of a single line.
    if effects.requires_sacrifice {
        profile.produces.insert(SynergySignal::SacrificeEvent);
        let oracle = card.oracle_text.to_ascii_lowercase();
        if oracle.contains("sacrifice a creature")
            || oracle.contains("sacrifice another creature")
            || oracle.contains("sacrifice x creatures")
            || oracle.contains("sacrifice x squirrels")
        {
            profile.consumes.insert(SynergySignal::CreatureBody);
            profile.produces.insert(SynergySignal::DeathEvent);
        }
    }

    profile
}

fn compile_ability_node(line: &str, lower_type: &str, confidence: f32) -> AbilityNode {
    let lower = line.to_ascii_lowercase();
    let kind = ability_kind(&lower, lower_type);
    let timing = ability_timing(&lower, kind);
    let costs = ability_costs(&lower, kind);
    let mut produces = BTreeSet::new();
    let mut consumes = BTreeSet::new();

    if contains_any(&lower, &["create ", "created"]) && lower.contains(" token") {
        produces.insert(SynergySignal::Token);
        if lower.contains("creature token") {
            produces.insert(SynergySignal::CreatureToken);
            produces.insert(SynergySignal::CreatureBody);
        }
        if contains_any(
            &lower,
            &[
                "treasure token",
                "clue token",
                "food token",
                "blood token",
                "map token",
                "powerstone token",
            ],
        ) {
            produces.insert(SynergySignal::Artifact);
        }
        if lower.contains("treasure token") {
            produces.insert(SynergySignal::Mana);
        }
    }
    if contains_any(
        &lower,
        &[
            "draw a card",
            "draw two cards",
            "draw three cards",
            "draw cards equal",
            "you may play",
            "you may cast",
            "search your library",
        ],
    ) {
        produces.insert(SynergySignal::CardAccess);
    }
    if lower.contains("add {") || lower.contains("add one mana") || lower.contains("add two mana") {
        produces.insert(SynergySignal::Mana);
    }
    if lower.contains("put ") && lower.contains(" counter") {
        produces.insert(SynergySignal::Counter);
    }
    if lower.contains("gain ") && lower.contains(" life") {
        produces.insert(SynergySignal::LifeGained);
    }
    if contains_any(
        &lower,
        &[
            "mill ",
            "surveil ",
            "put the top card of your library into your graveyard",
            "put the top cards of your library into your graveyard",
            "discard a card",
            "discard your hand",
        ],
    ) {
        produces.insert(SynergySignal::Graveyard);
    }

    if token_dependency(&lower) {
        consumes.insert(SynergySignal::Token);
    }
    if creature_dependency(&lower) {
        consumes.insert(SynergySignal::CreatureBody);
    }
    if artifact_dependency(&lower) {
        consumes.insert(SynergySignal::Artifact);
    }
    if enchantment_dependency(&lower) {
        consumes.insert(SynergySignal::Enchantment);
    }
    consumes.extend(spell_dependencies(&lower));
    if death_dependency(&lower).is_some() {
        consumes.insert(SynergySignal::DeathEvent);
    }
    if sacrifice_dependency(&lower) {
        consumes.insert(SynergySignal::SacrificeEvent);
    }
    if graveyard_dependency(&lower) {
        consumes.insert(SynergySignal::Graveyard);
    }
    if land_entry_dependency(&lower) {
        consumes.insert(SynergySignal::LandEntering);
    }
    if counter_dependency(&lower) {
        consumes.insert(SynergySignal::Counter);
    }
    if life_gain_dependency(&lower) {
        consumes.insert(SynergySignal::LifeGained);
    }

    if costs.contains(&AbilityCostKind::Sacrifice) {
        produces.insert(SynergySignal::SacrificeEvent);
        if contains_any(
            &lower,
            &[
                "sacrifice a creature",
                "sacrifice another creature",
                "sacrifice x creatures",
                "sacrifice x squirrels",
            ],
        ) {
            consumes.insert(SynergySignal::CreatureBody);
            produces.insert(SynergySignal::DeathEvent);
        }
    }
    if costs.contains(&AbilityCostKind::Discard) {
        produces.insert(SynergySignal::Graveyard);
    }

    AbilityNode {
        kind,
        timing,
        costs,
        produces,
        consumes,
        evidence: bounded_evidence(line),
        confidence,
        report_only: true,
    }
}

fn ability_kind(lower: &str, lower_type: &str) -> AbilityKind {
    if lower.contains(" would ") && lower.contains(" instead") {
        AbilityKind::Replacement
    } else if lower.starts_with("when ")
        || lower.starts_with("whenever ")
        || lower.starts_with("at the beginning ")
    {
        AbilityKind::Triggered
    } else if lower.contains(':') {
        AbilityKind::Activated
    } else if lower_type.contains("instant") || lower_type.contains("sorcery") {
        AbilityKind::Spell
    } else {
        AbilityKind::Static
    }
}

fn ability_timing(lower: &str, kind: AbilityKind) -> AbilityTiming {
    if kind == AbilityKind::Replacement {
        AbilityTiming::Replacement
    } else if kind == AbilityKind::Activated {
        AbilityTiming::Activated
    } else if lower.contains("enters the battlefield") || lower.contains("enters under") {
        AbilityTiming::EntersBattlefield
    } else if lower.contains(" dies") || lower.contains(" die") {
        AbilityTiming::Dies
    } else if lower.contains("you cast") || lower.contains("a spell") {
        AbilityTiming::Cast
    } else if lower.contains("upkeep") {
        AbilityTiming::Upkeep
    } else if lower.contains("attack") || lower.contains("combat") {
        AbilityTiming::Combat
    } else if kind == AbilityKind::Spell {
        AbilityTiming::Resolution
    } else if kind == AbilityKind::Static {
        AbilityTiming::Static
    } else {
        AbilityTiming::Unknown
    }
}

fn ability_costs(lower: &str, kind: AbilityKind) -> Vec<AbilityCostKind> {
    let cost_text = if kind == AbilityKind::Activated {
        lower.split_once(':').map(|(cost, _)| cost).unwrap_or(lower)
    } else {
        lower
    };
    let mut costs = Vec::new();
    if kind == AbilityKind::Activated
        && cost_text.contains('{')
        && contains_any(
            cost_text,
            &[
                "{w}", "{u}", "{b}", "{r}", "{g}", "{c}", "{x}", "{0}", "{1}", "{2}", "{3}", "{4}",
                "{5}", "{6}", "{7}", "{8}", "{9}",
            ],
        )
    {
        costs.push(AbilityCostKind::Mana);
    }
    if cost_text.contains("{t}") {
        costs.push(AbilityCostKind::Tap);
    }
    if cost_text.contains("sacrifice ")
        || lower.contains("as an additional cost") && lower.contains("sacrifice ")
    {
        costs.push(AbilityCostKind::Sacrifice);
    }
    if cost_text.contains("discard ")
        || lower.contains("as an additional cost") && lower.contains("discard ")
    {
        costs.push(AbilityCostKind::Discard);
    }
    if cost_text.contains("pay ") && cost_text.contains(" life") {
        costs.push(AbilityCostKind::Life);
    }
    if cost_text.contains("exile ") {
        costs.push(AbilityCostKind::Exile);
    }
    if lower.contains("as an additional cost") {
        costs.push(AbilityCostKind::Additional);
    }
    costs.sort_by_key(|cost| *cost as u8);
    costs.dedup();
    costs
}

pub(crate) fn build_synergy_graph(
    cards: &[GraphCard<'_>],
    known_lines: &[KnownLine],
) -> SynergyGraph {
    let mut links = Vec::new();
    let mut resource_coverage = Vec::new();

    for signal in all_signals() {
        let producer_count = cards
            .iter()
            .filter(|card| card.profile.produces.contains(signal))
            .fold(0u16, |total, card| total.saturating_add(card.quantity));
        let consumer_count = cards
            .iter()
            .filter(|card| card.profile.consumes.contains(signal))
            .fold(0u16, |total, card| total.saturating_add(card.quantity));
        if producer_count > 0 && consumer_count > 0 {
            resource_coverage.push(SynergyResourceCoverage {
                resource: signal.label().into(),
                producer_count,
                consumer_count,
            });
        }
    }

    for source in cards {
        for target in cards {
            if source.normalized_name == target.normalized_name {
                continue;
            }
            for signal in source
                .profile
                .produces
                .intersection(&target.profile.consumes)
            {
                let relation = if signal.is_event() {
                    SynergyRelation::Triggers
                } else {
                    SynergyRelation::Provides
                };
                let confidence = (source.profile.confidence.min(target.profile.confidence)
                    * if signal.is_event() { 0.92 } else { 0.86 })
                .clamp(0.0, 0.96);
                links.push(SynergyLink {
                    source_card: source.name.into(),
                    target_card: target.name.into(),
                    relation,
                    resource: signal.label().into(),
                    confidence,
                    evidence: if relation == SynergyRelation::Triggers {
                        trigger_evidence(source, target, *signal)
                    } else {
                        format!(
                            "{} supplies {}; {} explicitly uses or rewards that resource.",
                            source.name,
                            signal.label(),
                            target.name
                        )
                    },
                });
            }
        }
    }

    for line in known_lines {
        for left in 0..line.cards.len() {
            for right in (left + 1)..line.cards.len() {
                links.push(SynergyLink {
                    source_card: line.cards[left].clone(),
                    target_card: line.cards[right].clone(),
                    relation: SynergyRelation::KnownCombination,
                    resource: "documented combination".into(),
                    confidence: line.model_confidence,
                    evidence: format!(
                        "Both cards are required by the documented “{}” line.",
                        line.name
                    ),
                });
            }
        }
    }

    links.sort_by(|left, right| {
        relation_rank(right.relation)
            .cmp(&relation_rank(left.relation))
            .then_with(|| {
                let left_commander = is_commander_link(left, cards);
                let right_commander = is_commander_link(right, cards);
                right_commander.cmp(&left_commander)
            })
            .then_with(|| {
                signal_specificity(&right.resource).cmp(&signal_specificity(&left.resource))
            })
            .then_with(|| right.confidence.total_cmp(&left.confidence))
            .then_with(|| left.source_card.cmp(&right.source_card))
            .then_with(|| left.target_card.cmp(&right.target_card))
            .then_with(|| left.resource.cmp(&right.resource))
    });
    links.dedup_by(|left, right| {
        left.source_card == right.source_card
            && left.target_card == right.target_card
            && left.relation == right.relation
            && left.resource == right.resource
    });

    let edge_count = links.len().min(u32::MAX as usize) as u32;
    let commander_links = links
        .iter()
        .filter(|link| is_commander_link(link, cards))
        .take(MAX_COMMANDER_LINKS)
        .cloned()
        .collect::<Vec<_>>();

    let connected_names = links
        .iter()
        .flat_map(|link| [&link.source_card, &link.target_card])
        .map(|name| normalize_for_graph(name))
        .collect::<HashSet<_>>();
    let nonland_names = cards
        .iter()
        .filter(|card| !card.profile.is_land)
        .map(|card| normalize_for_graph(card.name))
        .collect::<HashSet<_>>();
    let connected_card_count = nonland_names
        .iter()
        .filter(|name| connected_names.contains(*name))
        .count()
        .min(u16::MAX as usize) as u16;
    let graph_coverage = if nonland_names.is_empty() {
        0.0
    } else {
        connected_card_count as f32 / nonland_names.len() as f32
    };
    links.truncate(MAX_DISPLAYED_LINKS);

    SynergyGraph {
        model_version: SYNERGY_GRAPH_VERSION.into(),
        ability_model_version: ABILITY_IR_VERSION.into(),
        node_count: cards.len().min(u16::MAX as usize) as u16,
        connected_card_count,
        edge_count,
        displayed_edge_count: links.len().min(u16::MAX as usize) as u16,
        graph_coverage,
        unsupported_clause_count: cards
            .iter()
            .map(|card| card.profile.unsupported_clause_count * u32::from(card.quantity))
            .sum(),
        resources: resource_coverage,
        links,
        commander_links,
    }
}

fn trigger_evidence(
    source: &GraphCard<'_>,
    target: &GraphCard<'_>,
    signal: SynergySignal,
) -> String {
    if signal == SynergySignal::DeathEvent
        && target
            .profile
            .death_dependencies
            .contains(&DeathDependency::AnotherCreatureYouControl)
        && !target
            .profile
            .death_dependencies
            .contains(&DeathDependency::Generic)
    {
        return format!(
            "{} can create {}; {} can trigger when another creature you control dies. \
             This is conditional: {} must be on the battlefield, and a different creature \
             you control must die.",
            source.name,
            signal.label(),
            target.name,
            target.name
        );
    }

    format!(
        "{} can create {}; {} has an explicit matching trigger or dependency.",
        source.name,
        signal.label(),
        target.name
    )
}

fn all_signals() -> &'static [SynergySignal] {
    &[
        SynergySignal::CreatureBody,
        SynergySignal::CreatureToken,
        SynergySignal::Token,
        SynergySignal::Artifact,
        SynergySignal::Enchantment,
        SynergySignal::SpellCast,
        SynergySignal::OpponentSpellCast,
        SynergySignal::DeathEvent,
        SynergySignal::SacrificeEvent,
        SynergySignal::Graveyard,
        SynergySignal::LandEntering,
        SynergySignal::CardAccess,
        SynergySignal::Mana,
        SynergySignal::Counter,
        SynergySignal::LifeGained,
    ]
}

fn relation_rank(relation: SynergyRelation) -> u8 {
    match relation {
        SynergyRelation::KnownCombination => 3,
        SynergyRelation::Triggers => 2,
        SynergyRelation::Provides => 1,
    }
}

fn signal_specificity(label: &str) -> u8 {
    all_signals()
        .iter()
        .find(|signal| signal.label() == label)
        .map(|signal| signal.specificity())
        .unwrap_or(6)
}

fn is_commander_link(link: &SynergyLink, cards: &[GraphCard<'_>]) -> bool {
    cards.iter().any(|card| {
        card.is_commander
            && (card.name.eq_ignore_ascii_case(&link.source_card)
                || card.name.eq_ignore_ascii_case(&link.target_card))
    })
}

fn magnitude_present(magnitude: EffectMagnitude) -> bool {
    magnitude != EffectMagnitude::None
}

fn token_dependency(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "tokens you control",
            "token you control",
            "one or more tokens",
            "for each token",
            "number of tokens",
            "token enters",
            "tokens enter",
            "token dies",
            "tokens die",
            "tokens would be created",
            "populate",
        ],
    )
}

fn creature_dependency(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "creatures you control",
            "creature you control",
            "another creature",
            "creature spell you cast",
            "creature spells you cast",
            "whenever a creature enters",
            "for each creature you control",
            "number of creatures you control",
        ],
    )
}

fn artifact_dependency(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "artifacts you control",
            "artifact you control",
            "artifact spell you cast",
            "artifact spells you cast",
            "whenever an artifact",
            "for each artifact",
            "number of artifacts",
            "sacrifice an artifact",
            "affinity for artifacts",
            "metalcraft",
            "improvise",
        ],
    )
}

fn enchantment_dependency(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "enchantments you control",
            "enchantment you control",
            "enchantment spell you cast",
            "enchantment spells you cast",
            "whenever an enchantment",
            "for each enchantment",
            "number of enchantments",
            "constellation",
        ],
    )
}

fn spell_dependencies(lower: &str) -> BTreeSet<SynergySignal> {
    let references_spell = contains_any(
        lower,
        &[
            "instant or sorcery spell",
            "instant and sorcery spell",
            "instant and sorcery spells",
            "instants and sorceries",
            "instant or sorcery card",
            "noncreature spell",
            "casts a spell",
            "casts their first spell",
            "spell your opponent casts",
            "spells your opponents cast",
            "magecraft",
            "copy target instant or sorcery",
        ],
    );
    if !references_spell {
        return BTreeSet::new();
    }

    let controller_cast = contains_any(
        lower,
        &[
            "whenever you cast",
            "when you cast",
            "if you cast",
            "the next time you cast",
            "spell you cast",
            "spells you cast",
            "you've cast",
            "you have cast",
            "magecraft",
            "copy target instant or sorcery",
        ],
    );
    let opponent_cast = contains_any(
        lower,
        &[
            "an opponent casts",
            "opponent casts",
            "opponents cast",
            "opponent has cast",
            "opponents have cast",
            "spells your opponents cast",
            "spell your opponent casts",
        ],
    );
    let any_player_cast = contains_any(
        lower,
        &[
            "you or an opponent",
            "a player casts",
            "each player casts",
            "any player casts",
        ],
    );

    let mut dependencies = BTreeSet::new();
    if controller_cast || any_player_cast {
        dependencies.insert(SynergySignal::SpellCast);
    }
    if opponent_cast || any_player_cast {
        dependencies.insert(SynergySignal::OpponentSpellCast);
    }
    dependencies
}

fn death_dependency(lower: &str) -> Option<DeathDependency> {
    let references_death =
        lower.contains(" dies") || lower.contains(" die") || lower.contains("died this turn");
    let is_dependency = contains_any(
        lower,
        &["when ", "whenever ", "if ", "for each ", "died this turn"],
    );
    if !references_death || !is_dependency {
        return None;
    }

    // A generic death event does not prove an attachment-qualified subject.
    // Until the graph models face state and attachment identity, fail closed
    // instead of presenting an equipped/enchanted-creature trigger as a
    // two-card interaction.
    if contains_any(
        lower,
        &[
            "equipped creature",
            "enchanted creature",
            "creature equipped with",
            "creature enchanted by",
        ],
    ) {
        return None;
    }

    let requires_another_controlled_creature = contains_any(
        lower,
        &[
            "another creature you control",
            "other creature you control",
            "other creatures you control",
        ],
    ) && !contains_any(
        lower,
        &[
            " or another creature you control",
            " or other creature you control",
            " or other creatures you control",
        ],
    );

    Some(if requires_another_controlled_creature {
        DeathDependency::AnotherCreatureYouControl
    } else {
        DeathDependency::Generic
    })
}

fn sacrifice_dependency(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "whenever you sacrifice",
            "whenever a player sacrifices",
            "if you sacrificed",
            "for each permanent sacrificed",
        ],
    )
}

fn graveyard_dependency(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "from your graveyard",
            "in your graveyard",
            "cards in graveyards",
            "card in a graveyard",
            "cards in your graveyard",
            "return target",
        ],
    ) && lower.contains("graveyard")
}

fn land_entry_dependency(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "whenever a land enters",
            "whenever one or more lands enter",
            "landfall",
            "for each land that entered",
        ],
    )
}

fn counter_dependency(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "for each counter",
            "number of counters",
            "remove a counter",
            "one or more counters",
            "with a counter on",
        ],
    )
}

fn life_gain_dependency(lower: &str) -> bool {
    contains_any(
        lower,
        &[
            "whenever you gain life",
            "if you gained life",
            "each time you gain life",
            "for each life you gained",
        ],
    )
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn bounded_evidence(line: &str) -> String {
    let mut evidence = line.trim().replace('\n', " ");
    if evidence.chars().count() > 180 {
        evidence = evidence.chars().take(177).collect::<String>();
        evidence.push_str("...");
    }
    evidence
}

fn normalize_for_graph(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
