//! Exact standalone Affinity and Ward programs that are outside the earlier
//! official keyword runtime.
//!
//! Recognition is deliberately narrow. A clause is accepted only when the
//! complete Oracle line supplies an exact Affinity filter or a complete Ward
//! cost. Granted abilities, duration modifiers, compound keyword lists, and
//! reminder-only mentions stay rejected. The runtime is not connected to the
//! production simulator until that simulator can supply every state boundary
//! represented here.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const RESIDUAL_COST_KEYWORD_COMPILER_VERSION: &str = "residual-cost-keyword-compiler-0.2";
pub const RESIDUAL_COST_KEYWORD_RUNTIME_VERSION: &str = "residual-cost-keyword-runtime-0.2";
pub const RESIDUAL_COST_KEYWORD_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:101.2-3,104.3d,106.3,106.6,107.1b,107.4,117.3,118.3-5,118.7,119.4,122.1,601.2b,601.2f-i,603.2,603.3b,608.2b,608.2h,609.3,701.6,701.59,701.67-68,702.21,702.41,704.5c,704.5g";

pub const fn residual_cost_keyword_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncarnationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StackObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StackIncarnationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StackObjectRef {
    pub stack_id: StackObjectId,
    pub incarnation_id: StackIncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbilityInstanceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CostDeterminationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaymentId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaUnitId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetEventId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TriggerBatchId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WardTriggerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Stack,
    Command,
    OutsideGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardType {
    Artifact,
    Battle,
    Creature,
    Enchantment,
    Instant,
    Land,
    Planeswalker,
    Sorcery,
    Kindred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Supertype {
    Basic,
    Legendary,
    Snow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

impl ManaColor {
    fn stable_id(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Blue => "blue",
            Self::Black => "black",
            Self::Red => "red",
            Self::Green => "green",
            Self::Colorless => "colorless",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ManaRequirement {
    pub generic: u32,
    pub white: u32,
    pub blue: u32,
    pub black: u32,
    pub red: u32,
    pub green: u32,
    pub colorless: u32,
    pub snow: u32,
}

impl ManaRequirement {
    fn checked_add(&self, other: &Self) -> Option<Self> {
        Some(Self {
            generic: self.generic.checked_add(other.generic)?,
            white: self.white.checked_add(other.white)?,
            blue: self.blue.checked_add(other.blue)?,
            black: self.black.checked_add(other.black)?,
            red: self.red.checked_add(other.red)?,
            green: self.green.checked_add(other.green)?,
            colorless: self.colorless.checked_add(other.colorless)?,
            snow: self.snow.checked_add(other.snow)?,
        })
    }

    fn stable_id(&self) -> String {
        format!(
            "generic={};w={};u={};b={};r={};g={};c={};s={}",
            self.generic,
            self.white,
            self.blue,
            self.black,
            self.red,
            self.green,
            self.colorless,
            self.snow
        )
    }

    fn required_unit_count(&self) -> u32 {
        self.generic
            .saturating_add(self.white)
            .saturating_add(self.blue)
            .saturating_add(self.black)
            .saturating_add(self.red)
            .saturating_add(self.green)
            .saturating_add(self.colorless)
            .saturating_add(self.snow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaUnit {
    pub id: ManaUnitId,
    pub color: ManaColor,
    pub from_snow_source: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectCharacteristics {
    pub card_types: BTreeSet<CardType>,
    pub supertypes: BTreeSet<Supertype>,
    pub subtypes: BTreeSet<String>,
    pub mana_value: u32,
    pub power: i32,
    pub is_token: bool,
    pub has_affinity: bool,
}

impl ObjectCharacteristics {
    fn has_type(&self, card_type: CardType) -> bool {
        self.card_types.contains(&card_type)
    }

    fn has_subtype(&self, subtype: &str) -> bool {
        self.subtypes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(subtype))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameObject {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub characteristics: ObjectCharacteristics,
    pub tapped: bool,
    pub can_receive_minus_one_minus_one_counters: bool,
    pub minus_one_minus_one_counters: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackObjectKind {
    Spell,
    Ability { source: Option<ObjectRef> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackObjectStatus {
    OnStack,
    Countered,
    Resolved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackObject {
    pub stack_ref: StackObjectRef,
    pub controller: PlayerId,
    pub kind: StackObjectKind,
    pub counterable: bool,
    pub status: StackObjectStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub life: u32,
    pub poison_counters: u32,
    pub mana_pool: BTreeMap<ManaUnitId, ManaUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AffinityFilter {
    CardType(CardType),
    AllCardTypes(BTreeSet<CardType>),
    CreatureType(String),
    PermanentSubtype(String),
    BasicLandType(String),
    HistoricPermanent,
    OutlawPermanent,
    SnowLand,
    TokenPermanent,
    PermanentWithAffinity,
}

impl AffinityFilter {
    fn stable_id(&self) -> String {
        match self {
            Self::CardType(card_type) => format!("card-type/{card_type:?}"),
            Self::AllCardTypes(types) => format!(
                "all-card-types/{}",
                types
                    .iter()
                    .map(|card_type| format!("{card_type:?}"))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::CreatureType(subtype) => {
                format!("creature-type/{}", subtype.to_ascii_lowercase())
            }
            Self::PermanentSubtype(subtype) => {
                format!("permanent-subtype/{}", subtype.to_ascii_lowercase())
            }
            Self::BasicLandType(subtype) => {
                format!("basic-land-type/{}", subtype.to_ascii_lowercase())
            }
            Self::HistoricPermanent => "historic-permanent".to_owned(),
            Self::OutlawPermanent => "outlaw-permanent".to_owned(),
            Self::SnowLand => "snow-land".to_owned(),
            Self::TokenPermanent => "token-permanent".to_owned(),
            Self::PermanentWithAffinity => "permanent-with-affinity".to_owned(),
        }
    }

    pub fn matches(&self, object: &GameObject, controller: PlayerId) -> bool {
        if object.zone != Zone::Battlefield || object.controller != controller {
            return false;
        }
        let characteristics = &object.characteristics;
        match self {
            Self::CardType(card_type) => characteristics.has_type(*card_type),
            Self::AllCardTypes(types) => types
                .iter()
                .all(|card_type| characteristics.has_type(*card_type)),
            // Kindred and type-changing effects can put a creature type on a
            // permanent that is not currently a creature.
            Self::CreatureType(subtype) => characteristics.has_subtype(subtype),
            Self::PermanentSubtype(subtype) => characteristics.has_subtype(subtype),
            Self::BasicLandType(subtype) => {
                characteristics.has_type(CardType::Land) && characteristics.has_subtype(subtype)
            }
            Self::HistoricPermanent => {
                characteristics.has_type(CardType::Artifact)
                    || characteristics.supertypes.contains(&Supertype::Legendary)
                    || characteristics.has_subtype("Saga")
            }
            Self::OutlawPermanent => ["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"]
                .iter()
                .any(|subtype| characteristics.has_subtype(subtype)),
            Self::SnowLand => {
                characteristics.has_type(CardType::Land)
                    && characteristics.supertypes.contains(&Supertype::Snow)
            }
            Self::TokenPermanent => characteristics.is_token,
            Self::PermanentWithAffinity => characteristics.has_affinity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffinityProgram {
    pub filter: AffinityFilter,
    pub generic_reduction_per_counted_permanent: u32,
    pub applies_while_source_is_a_spell_on_stack: bool,
    pub count_at_total_cost_determination: bool,
    pub applies_after_cost_increases: bool,
    pub applies_to_alternative_and_additional_costs: bool,
    pub cannot_reduce_nongeneric_requirements: bool,
    pub generic_floor_is_zero: bool,
    pub multiple_instances_apply_separately: bool,
}

impl AffinityProgram {
    fn stable_id(&self) -> String {
        format!(
            "affinity/v1;filter={};per-count={};spell-stack={};count-during-total={};after-increases={};alternate-additional={};nongeneric-preserved={};floor-zero={};multiple-separate={}",
            self.filter.stable_id(),
            self.generic_reduction_per_counted_permanent,
            self.applies_while_source_is_a_spell_on_stack,
            self.count_at_total_cost_determination,
            self.applies_after_cost_increases,
            self.applies_to_alternative_and_additional_costs,
            self.cannot_reduce_nongeneric_requirements,
            self.generic_floor_is_zero,
            self.multiple_instances_apply_separately
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WardDiscardFilter {
    AnyCard,
    EnchantmentInstantOrSorcery,
}

impl WardDiscardFilter {
    fn stable_id(&self) -> &'static str {
        match self {
            Self::AnyCard => "any-card",
            Self::EnchantmentInstantOrSorcery => "enchantment-instant-or-sorcery",
        }
    }

    fn matches(&self, object: &GameObject, player: PlayerId) -> bool {
        if object.zone != Zone::Hand || object.owner != player {
            return false;
        }
        match self {
            Self::AnyCard => true,
            Self::EnchantmentInstantOrSorcery => {
                [CardType::Enchantment, CardType::Instant, CardType::Sorcery]
                    .iter()
                    .any(|card_type| object.characteristics.has_type(*card_type))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WardSacrificeFilter {
    AnyPermanent,
    NonlandPermanent,
    Creature,
    Food,
    LegendaryArtifactOrCreature,
    PermanentWithMinimumManaValue(u32),
}

impl WardSacrificeFilter {
    fn stable_id(&self) -> String {
        match self {
            Self::AnyPermanent => "any-permanent".to_owned(),
            Self::NonlandPermanent => "nonland-permanent".to_owned(),
            Self::Creature => "creature".to_owned(),
            Self::Food => "food".to_owned(),
            Self::LegendaryArtifactOrCreature => "legendary-and-artifact-or-creature".to_owned(),
            Self::PermanentWithMinimumManaValue(value) => {
                format!("permanent-mana-value-at-least/{value}")
            }
        }
    }

    fn matches(&self, object: &GameObject, player: PlayerId) -> bool {
        if object.zone != Zone::Battlefield || object.controller != player {
            return false;
        }
        match self {
            Self::AnyPermanent => true,
            Self::NonlandPermanent => !object.characteristics.has_type(CardType::Land),
            Self::Creature => object.characteristics.has_type(CardType::Creature),
            Self::Food => object.characteristics.has_subtype("Food"),
            Self::LegendaryArtifactOrCreature => {
                object
                    .characteristics
                    .supertypes
                    .contains(&Supertype::Legendary)
                    && (object.characteristics.has_type(CardType::Artifact)
                        || object.characteristics.has_type(CardType::Creature))
            }
            Self::PermanentWithMinimumManaValue(value) => {
                object.characteristics.mana_value >= *value
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WardCost {
    Mana(ManaRequirement),
    PayLife(u32),
    PayLifeEqualToProtectedPower,
    Discard {
        amount: u32,
        filter: WardDiscardFilter,
        random: bool,
    },
    Sacrifice {
        amount: u32,
        filter: WardSacrificeFilter,
    },
    CollectEvidence {
        minimum_total_mana_value: u32,
    },
    Blight {
        amount: u32,
    },
    GetPoisonCounters(u32),
    Waterbend(ManaRequirement),
    All(Vec<WardCost>),
    OneOf(Vec<WardCost>),
}

impl WardCost {
    fn stable_id(&self) -> String {
        match self {
            Self::Mana(requirement) => format!("mana/{}", requirement.stable_id()),
            Self::PayLife(amount) => format!("pay-life/{amount}"),
            Self::PayLifeEqualToProtectedPower => {
                "pay-life/equal-protected-power/current-or-resolution-lki".to_owned()
            }
            Self::Discard {
                amount,
                filter,
                random,
            } => format!("discard/{amount}/{}/random={random}", filter.stable_id()),
            Self::Sacrifice { amount, filter } => {
                format!("sacrifice/{amount}/{}", filter.stable_id())
            }
            Self::CollectEvidence {
                minimum_total_mana_value,
            } => format!("collect-evidence/{minimum_total_mana_value}"),
            Self::Blight { amount } => format!("blight/{amount}"),
            Self::GetPoisonCounters(amount) => format!("get-poison/{amount}"),
            Self::Waterbend(requirement) => {
                format!("waterbend/{}", requirement.stable_id())
            }
            Self::All(costs) => format!(
                "all/{}",
                costs
                    .iter()
                    .map(Self::stable_id)
                    .collect::<Vec<_>>()
                    .join("+")
            ),
            Self::OneOf(costs) => format!(
                "one-of/{}",
                costs
                    .iter()
                    .map(Self::stable_id)
                    .collect::<Vec<_>>()
                    .join("|")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WardProgram {
    pub cost: WardCost,
    pub triggers_on_becoming_target: bool,
    pub requires_opponent_controlled_spell_or_ability: bool,
    pub trigger_controller_is_protected_object_controller: bool,
    pub counters_that_stack_object_unless_paid: bool,
    pub each_instance_triggers_separately: bool,
}

impl WardProgram {
    fn stable_id(&self) -> String {
        format!(
            "ward/v1;cost={};becomes-target={};opponent-source={};trigger-controller=protected-controller:{};counter-that-stack-object-unless-paid={};instances-separate={}",
            self.cost.stable_id(),
            self.triggers_on_becoming_target,
            self.requires_opponent_controlled_spell_or_ability,
            self.trigger_controller_is_protected_object_controller,
            self.counters_that_stack_object_unless_paid,
            self.each_instance_triggers_separately
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidualCostKeywordKind {
    Affinity(AffinityProgram),
    Ward(WardProgram),
}

impl ResidualCostKeywordKind {
    pub const fn family(&self) -> ResidualCostKeywordFamily {
        match self {
            Self::Affinity(_) => ResidualCostKeywordFamily::Affinity,
            Self::Ward(_) => ResidualCostKeywordFamily::Ward,
        }
    }

    fn stable_id(&self) -> String {
        match self {
            Self::Affinity(program) => program.stable_id(),
            Self::Ward(program) => program.stable_id(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResidualCostKeywordFamily {
    Affinity,
    Ward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidualCostKeywordProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    kind: ResidualCostKeywordKind,
}

impl ResidualCostKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &ResidualCostKeywordKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        residual_cost_keyword_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarlierClauseOwner {
    OfficialKeywordRuntimeAffinityForArtifacts,
    OfficialKeywordRuntimeManaWard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidualCostClauseClassification {
    Program(ResidualCostKeywordProgram),
    EarlierOwner {
        family: ResidualCostKeywordFamily,
        owner: EarlierClauseOwner,
    },
    Rejected,
}

pub fn compile_residual_cost_keyword_program(
    exact_source: &str,
    normalized_source: &str,
) -> Option<ResidualCostKeywordProgram> {
    match classify_residual_cost_keyword_clause(exact_source, normalized_source) {
        ResidualCostClauseClassification::Program(program) => Some(program),
        ResidualCostClauseClassification::EarlierOwner { .. }
        | ResidualCostClauseClassification::Rejected => None,
    }
}

pub fn classify_residual_cost_keyword_clause(
    exact_source: &str,
    normalized_source: &str,
) -> ResidualCostClauseClassification {
    if !is_complete_single_line(exact_source) || !is_complete_single_line(normalized_source) {
        return ResidualCostClauseClassification::Rejected;
    }

    if is_earlier_affinity_clause(exact_source) {
        return ResidualCostClauseClassification::EarlierOwner {
            family: ResidualCostKeywordFamily::Affinity,
            owner: EarlierClauseOwner::OfficialKeywordRuntimeAffinityForArtifacts,
        };
    }
    if let Some(program) = parse_residual_affinity(exact_source) {
        return build_program(
            exact_source,
            normalized_source,
            ResidualCostKeywordKind::Affinity(program),
        );
    }

    if is_earlier_mana_ward_clause(exact_source) {
        return ResidualCostClauseClassification::EarlierOwner {
            family: ResidualCostKeywordFamily::Ward,
            owner: EarlierClauseOwner::OfficialKeywordRuntimeManaWard,
        };
    }
    if let Some(program) = parse_residual_ward(exact_source, normalized_source) {
        return build_program(
            exact_source,
            normalized_source,
            ResidualCostKeywordKind::Ward(program),
        );
    }

    ResidualCostClauseClassification::Rejected
}

fn build_program(
    exact_source: &str,
    normalized_source: &str,
    kind: ResidualCostKeywordKind,
) -> ResidualCostClauseClassification {
    let semantic_digest = semantic_digest(exact_source, &kind);
    ResidualCostClauseClassification::Program(ResidualCostKeywordProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        semantic_digest,
        kind,
    })
}

fn is_complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source.trim() == source
        && !source.contains('\n')
        && !source.contains('\r')
}

fn split_core_and_reminder(source: &str) -> Option<(&str, Option<&str>)> {
    if !source.ends_with(')') {
        return Some((source, None));
    }
    let separator = source.rfind(" (")?;
    let core = &source[..separator];
    let reminder = &source[separator + 2..source.len() - 1];
    if core.is_empty() || reminder.is_empty() || reminder.contains('(') || reminder.contains(')') {
        return None;
    }
    Some((core, Some(reminder)))
}

fn is_earlier_affinity_clause(source: &str) -> bool {
    matches!(
        source,
        "Affinity for artifacts"
            | "Affinity for artifacts (This spell costs {1} less to cast for each artifact you control.)"
    )
}

fn parse_residual_affinity(source: &str) -> Option<AffinityProgram> {
    let (core, reminder) = split_core_and_reminder(source)?;
    let label = core.strip_prefix("Affinity for ")?;
    if label.is_empty() || label == "artifacts" {
        return None;
    }
    let (filter, expected_counted_phrase, uses_card_noun) = affinity_filter(label)?;
    let reminder = reminder?;
    let expected = if uses_card_noun {
        "This card costs {1} less to cast for each permanent you control with affinity.".to_string()
    } else {
        format!(
            "This spell costs {{1}} less to cast for each {expected_counted_phrase} you control."
        )
    };
    if reminder != expected {
        return None;
    }
    Some(AffinityProgram {
        filter,
        generic_reduction_per_counted_permanent: 1,
        applies_while_source_is_a_spell_on_stack: true,
        count_at_total_cost_determination: true,
        applies_after_cost_increases: true,
        applies_to_alternative_and_additional_costs: true,
        cannot_reduce_nongeneric_requirements: true,
        generic_floor_is_zero: true,
        multiple_instances_apply_separately: true,
    })
}

fn affinity_filter(label: &str) -> Option<(AffinityFilter, String, bool)> {
    let card_type = |card_type, phrase: &'static str| {
        Some((
            AffinityFilter::CardType(card_type),
            phrase.to_owned(),
            false,
        ))
    };
    let creature_subtype = |subtype: &'static str, phrase: &'static str| {
        Some((
            AffinityFilter::CreatureType(subtype.to_owned()),
            phrase.to_owned(),
            false,
        ))
    };
    let permanent_subtype = |subtype: &'static str, phrase: &'static str| {
        Some((
            AffinityFilter::PermanentSubtype(subtype.to_owned()),
            phrase.to_owned(),
            false,
        ))
    };
    let basic_land_type = |subtype: &'static str, phrase: &'static str| {
        Some((
            AffinityFilter::BasicLandType(subtype.to_owned()),
            phrase.to_owned(),
            false,
        ))
    };

    match label {
        "Affinity" => Some((
            AffinityFilter::PermanentWithAffinity,
            "permanent with affinity".to_owned(),
            true,
        )),
        "artifact creatures" => {
            let mut types = BTreeSet::new();
            types.insert(CardType::Artifact);
            types.insert(CardType::Creature);
            Some((
                AffinityFilter::AllCardTypes(types),
                "artifact creature".to_owned(),
                false,
            ))
        }
        "creatures" => card_type(CardType::Creature, "creature"),
        "enchantments" => card_type(CardType::Enchantment, "enchantment"),
        "planeswalkers" => card_type(CardType::Planeswalker, "planeswalker"),
        "tokens" => Some((AffinityFilter::TokenPermanent, "token".to_owned(), false)),
        "snow lands" => Some((AffinityFilter::SnowLand, "snow land".to_owned(), false)),
        "historic permanents" => Some((
            AffinityFilter::HistoricPermanent,
            "artifact, legendary, and/or Saga permanent".to_owned(),
            false,
        )),
        "outlaws" => Some((
            AffinityFilter::OutlawPermanent,
            "Assassin, Mercenary, Pirate, Rogue, and/or Warlock".to_owned(),
            false,
        )),
        "Allies" => creature_subtype("Ally", "Ally"),
        "Birds" => creature_subtype("Bird", "Bird"),
        "Cats" => creature_subtype("Cat", "Cat"),
        "Citizens" => creature_subtype("Citizen", "Citizen"),
        "Daleks" => creature_subtype("Dalek", "Dalek"),
        "Elves" => creature_subtype("Elf", "Elf"),
        "Frogs" => creature_subtype("Frog", "Frog"),
        "Humans" => creature_subtype("Human", "Human"),
        "Knights" => creature_subtype("Knight", "Knight"),
        "Lizards" => creature_subtype("Lizard", "Lizard"),
        "Phyrexians" => creature_subtype("Phyrexian", "Phyrexian"),
        "Slivers" => creature_subtype("Sliver", "Sliver"),
        "Spirits" => creature_subtype("Spirit", "Spirit"),
        "Equipment" => permanent_subtype("Equipment", "Equipment"),
        "Foods" => permanent_subtype("Food", "Food"),
        "Gates" => permanent_subtype("Gate", "Gate"),
        "Towns" => permanent_subtype("Town", "Town"),
        "Plains" => basic_land_type("Plains", "Plains"),
        "Islands" => basic_land_type("Island", "Island"),
        "Swamps" => basic_land_type("Swamp", "Swamp"),
        "Mountains" => basic_land_type("Mountain", "Mountain"),
        "Forests" => basic_land_type("Forest", "Forest"),
        _ => generic_subtype_affinity_filter(label),
    }
}

fn generic_subtype_affinity_filter(label: &str) -> Option<(AffinityFilter, String, bool)> {
    let singular = if let Some(stem) = label.strip_suffix("ies") {
        format!("{stem}y")
    } else if let Some(stem) = label.strip_suffix("ves") {
        format!("{stem}f")
    } else {
        label.strip_suffix('s')?.to_owned()
    };
    if singular.is_empty()
        || !singular
            .split_whitespace()
            .all(|word| word.chars().next().is_some_and(char::is_uppercase))
        || !singular.chars().all(|character| {
            character.is_alphabetic()
                || character.is_whitespace()
                || matches!(character, '-' | '\u{2019}' | '\'')
        })
    {
        return None;
    }
    Some((
        AffinityFilter::PermanentSubtype(singular.clone()),
        singular,
        false,
    ))
}

fn is_earlier_mana_ward_clause(source: &str) -> bool {
    let Some((core, reminder)) = split_core_and_reminder(source) else {
        return false;
    };
    let Some(mana_text) = core.strip_prefix("Ward ") else {
        return false;
    };
    let Some(mana) = parse_mana_requirement(mana_text) else {
        return false;
    };
    if mana.required_unit_count() == 0 {
        return false;
    }
    reminder.is_none_or(|reminder| {
        validate_standard_ward_reminder(reminder, &format!("pays {mana_text}"))
    })
}

fn parse_residual_ward(source: &str, normalized_source: &str) -> Option<WardProgram> {
    let (core, reminder) = split_core_and_reminder(source)?;
    let cost_text = core.strip_prefix("Ward\u{2014}")?.strip_suffix('.')?.trim();
    let (normalized_core, _) = split_core_and_reminder(normalized_source)?;
    let normalized_cost_text = normalized_core
        .strip_suffix('.')?
        .trim()
        .strip_prefix("Ward\u{2014}")
        .or_else(|| {
            normalized_core
                .strip_suffix('.')?
                .trim()
                .strip_prefix("ward\u{2014}")
        })?
        .trim();
    let cost = parse_ward_cost(cost_text, normalized_cost_text)?;
    if !validate_ward_reminder(reminder, cost_text, &cost) {
        return None;
    }
    Some(WardProgram {
        cost,
        triggers_on_becoming_target: true,
        requires_opponent_controlled_spell_or_ability: true,
        trigger_controller_is_protected_object_controller: true,
        counters_that_stack_object_unless_paid: true,
        each_instance_triggers_separately: true,
    })
}

fn parse_ward_cost(source: &str, normalized_source: &str) -> Option<WardCost> {
    if let Some((mana, life)) = source.split_once(", Pay ") {
        let mana = parse_mana_requirement(mana)?;
        let life = life.strip_suffix(" life")?.parse::<u32>().ok()?;
        return (life > 0).then_some(WardCost::All(vec![
            WardCost::Mana(mana),
            WardCost::PayLife(life),
        ]));
    }
    if source == "Discard a card or pay {2}" {
        return Some(WardCost::OneOf(vec![
            WardCost::Discard {
                amount: 1,
                filter: WardDiscardFilter::AnyCard,
                random: false,
            },
            WardCost::Mana(parse_mana_requirement("{2}")?),
        ]));
    }
    if source == "Discard a card" {
        return Some(WardCost::Discard {
            amount: 1,
            filter: WardDiscardFilter::AnyCard,
            random: false,
        });
    }
    if source == "Discard a card at random" {
        return Some(WardCost::Discard {
            amount: 1,
            filter: WardDiscardFilter::AnyCard,
            random: true,
        });
    }
    if source == "Discard an enchantment, instant, or sorcery card" {
        return Some(WardCost::Discard {
            amount: 1,
            filter: WardDiscardFilter::EnchantmentInstantOrSorcery,
            random: false,
        });
    }
    if let Some(amount) = source
        .strip_prefix("Pay ")
        .and_then(|tail| tail.strip_suffix(" life"))
        .and_then(|amount| amount.parse::<u32>().ok())
    {
        return (amount > 0).then_some(WardCost::PayLife(amount));
    }
    let literal_self_reference = source == "Pay life equal to this creature's power";
    let named_self_reference = source.starts_with("Pay life equal to ")
        && source.ends_with("'s power")
        && source["Pay life equal to ".len()..source.len() - "'s power".len()]
            .chars()
            .all(|character| {
                character.is_alphanumeric()
                    || character.is_whitespace()
                    || matches!(character, ',' | '-' | '\'')
            });
    let normalized_self_reference = matches!(
        normalized_source.to_ascii_lowercase().as_str(),
        "pay life equal to this creature's power"
            | "pay life equal to this permanent's power"
            | "pay life equal to this object's power"
    );
    if (literal_self_reference || named_self_reference) && normalized_self_reference {
        return Some(WardCost::PayLifeEqualToProtectedPower);
    }
    if source == "Sacrifice a creature" {
        return Some(WardCost::Sacrifice {
            amount: 1,
            filter: WardSacrificeFilter::Creature,
        });
    }
    if source == "Sacrifice a Food" {
        return Some(WardCost::Sacrifice {
            amount: 1,
            filter: WardSacrificeFilter::Food,
        });
    }
    if source == "Sacrifice a legendary artifact or legendary creature" {
        return Some(WardCost::Sacrifice {
            amount: 1,
            filter: WardSacrificeFilter::LegendaryArtifactOrCreature,
        });
    }
    if source == "Sacrifice a permanent with mana value 1 or greater" {
        return Some(WardCost::Sacrifice {
            amount: 1,
            filter: WardSacrificeFilter::PermanentWithMinimumManaValue(1),
        });
    }
    if source == "Sacrifice two permanents" {
        return Some(WardCost::Sacrifice {
            amount: 2,
            filter: WardSacrificeFilter::AnyPermanent,
        });
    }
    if source == "Sacrifice three nonland permanents" {
        return Some(WardCost::Sacrifice {
            amount: 3,
            filter: WardSacrificeFilter::NonlandPermanent,
        });
    }
    if source == "Collect evidence 4" {
        return Some(WardCost::CollectEvidence {
            minimum_total_mana_value: 4,
        });
    }
    if source == "Blight 2" {
        return Some(WardCost::Blight { amount: 2 });
    }
    if source == "Get five poison counters" {
        return Some(WardCost::GetPoisonCounters(5));
    }
    if source == "You get two poison counters" {
        return Some(WardCost::GetPoisonCounters(2));
    }
    source
        .strip_prefix("Waterbend ")
        .and_then(parse_mana_requirement)
        .map(WardCost::Waterbend)
}

fn validate_ward_reminder(reminder: Option<&str>, cost_text: &str, cost: &WardCost) -> bool {
    match cost {
        WardCost::CollectEvidence {
            minimum_total_mana_value: 4,
        } => {
            reminder
                == Some(
                    "Whenever this creature becomes the target of a spell or ability an opponent controls, counter it unless that player exiles cards with total mana value 4 or greater from their graveyard.",
                )
        }
        WardCost::Blight { amount: 2 } => {
            reminder
                == Some("To blight 2, a player puts two -1/-1 counters on a creature they control.")
        }
        WardCost::GetPoisonCounters(5) => {
            reminder == Some("A player with ten or more poison counters loses the game.")
        }
        WardCost::Waterbend(requirement)
            if *requirement
                == (ManaRequirement {
                    generic: 4,
                    ..ManaRequirement::default()
                }) =>
        {
            reminder
                == Some(
                    "Whenever this creature becomes the target of a spell or ability an opponent controls, counter it unless that player pays {4}. They can tap their artifacts and creatures to help. Each one pays for {1}.",
                )
        }
        WardCost::Discard {
            amount: 1,
            filter: WardDiscardFilter::AnyCard,
            random: false,
        } => reminder
            .is_none_or(|reminder| validate_standard_ward_reminder(reminder, "discards a card")),
        WardCost::OneOf(_) => reminder.is_some_and(|reminder| {
            validate_standard_ward_reminder(reminder, "discards a card or pays {2}")
        }),
        WardCost::PayLife(amount) => reminder.is_none_or(|reminder| {
            validate_standard_ward_reminder(reminder, &format!("pays {amount} life"))
        }),
        _ => {
            let _ = cost_text;
            reminder.is_none()
        }
    }
}

fn validate_standard_ward_reminder(reminder: &str, payment_phrase: &str) -> bool {
    const PREFIX: &str = "Whenever this ";
    const MIDDLE: &str = " becomes the target of a spell or ability an opponent controls, counter it unless that player ";
    let Some(rest) = reminder.strip_prefix(PREFIX) else {
        return false;
    };
    let Some((object_noun, tail)) = rest.split_once(MIDDLE) else {
        return false;
    };
    matches!(object_noun, "creature" | "enchantment" | "permanent")
        && tail == format!("{payment_phrase}.")
}

fn parse_mana_requirement(source: &str) -> Option<ManaRequirement> {
    if source.is_empty() {
        return None;
    }
    let mut requirement = ManaRequirement::default();
    let mut remaining = source;
    while !remaining.is_empty() {
        let body = remaining.strip_prefix('{')?;
        let close = body.find('}')?;
        let symbol = &body[..close];
        remaining = &body[close + 1..];
        match symbol {
            "W" => requirement.white = requirement.white.checked_add(1)?,
            "U" => requirement.blue = requirement.blue.checked_add(1)?,
            "B" => requirement.black = requirement.black.checked_add(1)?,
            "R" => requirement.red = requirement.red.checked_add(1)?,
            "G" => requirement.green = requirement.green.checked_add(1)?,
            "C" => requirement.colorless = requirement.colorless.checked_add(1)?,
            "S" => requirement.snow = requirement.snow.checked_add(1)?,
            _ => {
                let generic = symbol.parse::<u32>().ok()?;
                requirement.generic = requirement.generic.checked_add(generic)?;
            }
        }
    }
    (requirement.required_unit_count() > 0).then_some(requirement)
}

fn semantic_digest(source: &str, kind: &ResidualCostKeywordKind) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"residual-cost-keyword-semantic\0");
    hasher.update(RESIDUAL_COST_KEYWORD_COMPILER_VERSION.as_bytes());
    hasher.update(b"\0");
    hasher.update(RESIDUAL_COST_KEYWORD_RUNTIME_VERSION.as_bytes());
    hasher.update(b"\0");
    hasher.update(RESIDUAL_COST_KEYWORD_RULES_CONTEXT_VERSION.as_bytes());
    hasher.update(b"\0");
    hasher.update(source.as_bytes());
    hasher.update(b"\0");
    hasher.update(kind.stable_id().as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseManaCostChoice {
    Printed(ManaRequirement),
    Alternative(ManaRequirement),
}

impl BaseManaCostChoice {
    fn requirement(&self) -> &ManaRequirement {
        match self {
            Self::Printed(requirement) | Self::Alternative(requirement) => requirement,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostDeterminationPhase {
    Reductions,
    MinimumsAndFinalization,
    ReadyForPayment,
    Paid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffinityInstance {
    pub ability_instance_id: AbilityInstanceId,
    pub source_spell: StackObjectRef,
    pub controller: PlayerId,
    pub program: ResidualCostKeywordProgram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffinityReductionEvidence {
    pub cost_determination_id: CostDeterminationId,
    pub ability_instance_id: AbilityInstanceId,
    pub source_spell: StackObjectRef,
    pub program_semantic_digest: String,
    pub counted_permanents: Vec<ObjectRef>,
    pub generic_before: u32,
    pub requested_reduction: u32,
    pub applied_reduction: u32,
    pub generic_after: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCostFrame {
    pub cost_determination_id: CostDeterminationId,
    pub source_spell: StackObjectRef,
    pub caster: PlayerId,
    pub selected_base_cost: BaseManaCostChoice,
    pub additional_mana_costs: Vec<ManaRequirement>,
    pub cost_increases: Vec<ManaRequirement>,
    pub current_total: ManaRequirement,
    pub phase: CostDeterminationPhase,
    pub affinity_evidence: Vec<AffinityReductionEvidence>,
    applied_affinity_instances: BTreeSet<AbilityInstanceId>,
}

impl SpellCostFrame {
    pub fn begin_reductions(
        cost_determination_id: CostDeterminationId,
        source_spell: StackObjectRef,
        caster: PlayerId,
        selected_base_cost: BaseManaCostChoice,
        additional_mana_costs: Vec<ManaRequirement>,
        cost_increases: Vec<ManaRequirement>,
    ) -> Result<Self, ResidualCostRuntimeError> {
        let mut current_total = selected_base_cost.requirement().clone();
        for component in additional_mana_costs.iter().chain(cost_increases.iter()) {
            current_total = current_total
                .checked_add(component)
                .ok_or(ResidualCostRuntimeError::ManaRequirementOverflow)?;
        }
        Ok(Self {
            cost_determination_id,
            source_spell,
            caster,
            selected_base_cost,
            additional_mana_costs,
            cost_increases,
            current_total,
            phase: CostDeterminationPhase::Reductions,
            affinity_evidence: Vec::new(),
            applied_affinity_instances: BTreeSet::new(),
        })
    }

    pub fn finalize_reductions(&mut self) -> Result<(), ResidualCostRuntimeError> {
        if self.phase != CostDeterminationPhase::Reductions {
            return Err(ResidualCostRuntimeError::WrongCostDeterminationPhase);
        }
        self.phase = CostDeterminationPhase::MinimumsAndFinalization;
        self.phase = CostDeterminationPhase::ReadyForPayment;
        Ok(())
    }
}

pub fn apply_affinity_reduction(
    state: &ResidualCostGameState,
    frame: &mut SpellCostFrame,
    instance: &AffinityInstance,
) -> Result<AffinityReductionEvidence, ResidualCostRuntimeError> {
    if frame.phase != CostDeterminationPhase::Reductions {
        return Err(ResidualCostRuntimeError::WrongCostDeterminationPhase);
    }
    if frame.source_spell != instance.source_spell || frame.caster != instance.controller {
        return Err(ResidualCostRuntimeError::WrongControllerOrSource);
    }
    let stack_object = state
        .stack
        .get(&instance.source_spell.stack_id)
        .ok_or(ResidualCostRuntimeError::MissingStackObject)?;
    if stack_object.stack_ref != instance.source_spell
        || stack_object.controller != instance.controller
        || stack_object.kind != StackObjectKind::Spell
        || stack_object.status != StackObjectStatus::OnStack
    {
        return Err(ResidualCostRuntimeError::AffinitySourceNotCurrentSpell);
    }
    let ResidualCostKeywordKind::Affinity(program) = instance.program.kind() else {
        return Err(ResidualCostRuntimeError::WrongProgramKind);
    };
    if frame
        .applied_affinity_instances
        .contains(&instance.ability_instance_id)
    {
        return Err(ResidualCostRuntimeError::AbilityAlreadyApplied);
    }
    let mut counted_permanents = state
        .objects
        .values()
        .filter(|object| program.filter.matches(object, frame.caster))
        .map(|object| object.object_ref)
        .collect::<Vec<_>>();
    counted_permanents.sort();
    counted_permanents.dedup();
    let count = u32::try_from(counted_permanents.len())
        .map_err(|_| ResidualCostRuntimeError::CountOverflow)?;
    let requested_reduction = count
        .checked_mul(program.generic_reduction_per_counted_permanent)
        .ok_or(ResidualCostRuntimeError::CountOverflow)?;
    let generic_before = frame.current_total.generic;
    let applied_reduction = generic_before.min(requested_reduction);
    frame
        .applied_affinity_instances
        .insert(instance.ability_instance_id);
    frame.current_total.generic = generic_before - applied_reduction;
    let evidence = AffinityReductionEvidence {
        cost_determination_id: frame.cost_determination_id,
        ability_instance_id: instance.ability_instance_id,
        source_spell: instance.source_spell,
        program_semantic_digest: instance.program.semantic_digest().to_owned(),
        counted_permanents,
        generic_before,
        requested_reduction,
        applied_reduction,
        generic_after: frame.current_total.generic,
    };
    frame.affinity_evidence.push(evidence.clone());
    Ok(evidence)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPaymentEvidence {
    pub payment_id: PaymentId,
    pub payer: PlayerId,
    pub mana_units: Vec<ManaUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCostPaymentReceipt {
    pub payment_id: PaymentId,
    pub cost_determination_id: CostDeterminationId,
    pub payer: PlayerId,
    pub paid_requirement: ManaRequirement,
    pub mana_units: Vec<ManaUnitId>,
    pub affinity_evidence: Vec<AffinityReductionEvidence>,
}

pub fn pay_final_spell_mana_cost(
    state: &mut ResidualCostGameState,
    frame: &mut SpellCostFrame,
    payment: ManaPaymentEvidence,
) -> Result<SpellCostPaymentReceipt, ResidualCostRuntimeError> {
    if frame.phase != CostDeterminationPhase::ReadyForPayment {
        return Err(ResidualCostRuntimeError::WrongCostDeterminationPhase);
    }
    if payment.payer != frame.caster {
        return Err(ResidualCostRuntimeError::WrongControllerOrSource);
    }
    if state.used_payment_ids.contains(&payment.payment_id) {
        return Err(ResidualCostRuntimeError::PaymentAlreadyUsed);
    }
    let mut staged = state.clone();
    consume_mana(
        &mut staged,
        payment.payer,
        &frame.current_total,
        &payment.mana_units,
    )?;
    staged.used_payment_ids.insert(payment.payment_id);
    *state = staged;
    frame.phase = CostDeterminationPhase::Paid;
    Ok(SpellCostPaymentReceipt {
        payment_id: payment.payment_id,
        cost_determination_id: frame.cost_determination_id,
        payer: payment.payer,
        paid_requirement: frame.current_total.clone(),
        mana_units: payment.mana_units,
        affinity_evidence: frame.affinity_evidence.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WardAbilityInstance {
    pub ability_instance_id: AbilityInstanceId,
    pub protected_object: ObjectRef,
    pub controller: PlayerId,
    pub program: ResidualCostKeywordProgram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetEvent {
    pub event_id: TargetEventId,
    pub source: StackObjectRef,
    pub target: ObjectRef,
    pub target_was_newly_chosen: bool,
    pub target_choice_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WardTriggerOrder {
    pub batch_id: TriggerBatchId,
    pub ordered_triggers: Vec<(AbilityInstanceId, WardTriggerId)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingWardTrigger {
    pub trigger_id: WardTriggerId,
    pub trigger_batch_id: TriggerBatchId,
    pub trigger_ordinal: u32,
    pub ability_instance_id: AbilityInstanceId,
    pub controller: PlayerId,
    pub payer: PlayerId,
    pub protected_object: ObjectRef,
    pub source: StackObjectRef,
    pub target_event_id: TargetEventId,
    pub program_semantic_digest: String,
    pub cost: WardCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneMovePayment {
    pub before: ObjectRef,
    pub destination_incarnation: IncarnationId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomSelectionProof {
    pub eligible_cards_in_stable_order: Vec<ObjectRef>,
    pub selected_cards: Vec<ObjectRef>,
    pub entropy_commitment: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WardPaymentAtom {
    Mana {
        mana_units: Vec<ManaUnitId>,
    },
    PayLife(u32),
    PayLifeEqualToProtectedPower {
        amount: u32,
        /// Required only when the protected object is no longer the same
        /// battlefield object as the one that triggered ward. This must be
        /// that object's power immediately before it left the battlefield.
        last_known_power_at_resolution: Option<i32>,
    },
    Discard {
        cards: Vec<ZoneMovePayment>,
        random_selection: Option<RandomSelectionProof>,
    },
    Sacrifice {
        permanents: Vec<ZoneMovePayment>,
    },
    CollectEvidence {
        cards: Vec<ZoneMovePayment>,
    },
    Blight {
        creature: ObjectRef,
        amount: u32,
    },
    GetPoisonCounters(u32),
    Waterbend {
        mana_units: Vec<ManaUnitId>,
        tapped_permanents: Vec<ObjectRef>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WardPaymentEvidence {
    pub payment_id: PaymentId,
    pub payer: PlayerId,
    pub chosen_alternative: Option<usize>,
    pub atoms: Vec<WardPaymentAtom>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WardResolution {
    Paid {
        trigger_id: WardTriggerId,
        payment_id: PaymentId,
        payer: PlayerId,
        payment: WardPaymentEvidence,
        source: StackObjectRef,
        program_semantic_digest: String,
    },
    Countered {
        trigger_id: WardTriggerId,
        source: StackObjectRef,
        program_semantic_digest: String,
    },
    CounterAttemptPrevented {
        trigger_id: WardTriggerId,
        source: StackObjectRef,
        program_semantic_digest: String,
    },
    SourceNoLongerOnStack {
        trigger_id: WardTriggerId,
        source: StackObjectRef,
        program_semantic_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResidualCostGameState {
    pub players: BTreeMap<PlayerId, PlayerState>,
    pub objects: BTreeMap<ObjectId, GameObject>,
    pub stack: BTreeMap<StackObjectId, StackObject>,
    installed_ward: BTreeMap<AbilityInstanceId, WardAbilityInstance>,
    pending_ward: BTreeMap<WardTriggerId, PendingWardTrigger>,
    seen_target_events: BTreeSet<TargetEventId>,
    resolved_ward: BTreeSet<WardTriggerId>,
    used_payment_ids: BTreeSet<PaymentId>,
}

impl ResidualCostGameState {
    pub fn install_ward(
        &mut self,
        instance: WardAbilityInstance,
    ) -> Result<(), ResidualCostRuntimeError> {
        let object = self.current_object(instance.protected_object)?;
        if object.zone != Zone::Battlefield || object.controller != instance.controller {
            return Err(ResidualCostRuntimeError::WrongControllerOrSource);
        }
        if !matches!(instance.program.kind(), ResidualCostKeywordKind::Ward(_)) {
            return Err(ResidualCostRuntimeError::WrongProgramKind);
        }
        if self
            .installed_ward
            .contains_key(&instance.ability_instance_id)
        {
            return Err(ResidualCostRuntimeError::AbilityAlreadyApplied);
        }
        self.installed_ward
            .insert(instance.ability_instance_id, instance);
        Ok(())
    }

    pub fn create_ward_triggers(
        &mut self,
        event: TargetEvent,
        order: WardTriggerOrder,
    ) -> Result<Vec<PendingWardTrigger>, ResidualCostRuntimeError> {
        let mut staged = self.clone();
        let pending = staged.create_ward_triggers_inner(event, order)?;
        *self = staged;
        Ok(pending)
    }

    fn create_ward_triggers_inner(
        &mut self,
        event: TargetEvent,
        order: WardTriggerOrder,
    ) -> Result<Vec<PendingWardTrigger>, ResidualCostRuntimeError> {
        if !event.target_was_newly_chosen || !event.target_choice_complete {
            return Err(ResidualCostRuntimeError::InvalidTargetEvent);
        }
        if !self.seen_target_events.insert(event.event_id) {
            return Err(ResidualCostRuntimeError::TargetEventAlreadyHandled);
        }
        let source = self
            .stack
            .get(&event.source.stack_id)
            .ok_or(ResidualCostRuntimeError::MissingStackObject)?;
        if source.stack_ref != event.source || source.status != StackObjectStatus::OnStack {
            return Err(ResidualCostRuntimeError::InvalidTargetEvent);
        }
        let target = self.current_object(event.target)?.clone();
        if target.zone != Zone::Battlefield {
            return Err(ResidualCostRuntimeError::InvalidTargetEvent);
        }
        if source.controller == target.controller {
            return Err(ResidualCostRuntimeError::WardRequiresOpponentSource);
        }

        let eligible = self
            .installed_ward
            .values()
            .filter(|instance| {
                instance.protected_object == event.target
                    && instance.controller == target.controller
            })
            .map(|instance| instance.ability_instance_id)
            .collect::<BTreeSet<_>>();
        let ordered = order
            .ordered_triggers
            .iter()
            .map(|(instance, _)| *instance)
            .collect::<BTreeSet<_>>();
        let trigger_ids = order
            .ordered_triggers
            .iter()
            .map(|(_, trigger)| *trigger)
            .collect::<BTreeSet<_>>();
        if eligible.is_empty()
            || eligible != ordered
            || trigger_ids.len() != order.ordered_triggers.len()
            || order.ordered_triggers.len() != eligible.len()
        {
            return Err(ResidualCostRuntimeError::InvalidTriggerOrder);
        }

        let mut pending = Vec::with_capacity(order.ordered_triggers.len());
        for (ordinal, (ability_instance_id, trigger_id)) in
            order.ordered_triggers.iter().copied().enumerate()
        {
            if self.pending_ward.contains_key(&trigger_id)
                || self.resolved_ward.contains(&trigger_id)
            {
                return Err(ResidualCostRuntimeError::TriggerAlreadyExists);
            }
            let instance = self
                .installed_ward
                .get(&ability_instance_id)
                .ok_or(ResidualCostRuntimeError::MissingWardInstance)?;
            let ResidualCostKeywordKind::Ward(program) = instance.program.kind() else {
                return Err(ResidualCostRuntimeError::WrongProgramKind);
            };
            let trigger = PendingWardTrigger {
                trigger_id,
                trigger_batch_id: order.batch_id,
                trigger_ordinal: u32::try_from(ordinal)
                    .map_err(|_| ResidualCostRuntimeError::CountOverflow)?,
                ability_instance_id,
                controller: instance.controller,
                payer: source.controller,
                protected_object: instance.protected_object,
                source: event.source,
                target_event_id: event.event_id,
                program_semantic_digest: instance.program.semantic_digest().to_owned(),
                cost: program.cost.clone(),
            };
            self.pending_ward.insert(trigger_id, trigger.clone());
            pending.push(trigger);
        }
        Ok(pending)
    }

    pub fn pending_ward_trigger(&self, trigger: WardTriggerId) -> Option<&PendingWardTrigger> {
        self.pending_ward.get(&trigger)
    }

    fn current_object(
        &self,
        object_ref: ObjectRef,
    ) -> Result<&GameObject, ResidualCostRuntimeError> {
        let object = self
            .objects
            .get(&object_ref.object_id)
            .ok_or(ResidualCostRuntimeError::MissingObject)?;
        if object.object_ref != object_ref {
            return Err(ResidualCostRuntimeError::ObjectIncarnationChanged);
        }
        Ok(object)
    }
}

pub fn resolve_ward_trigger(
    state: &mut ResidualCostGameState,
    trigger_id: WardTriggerId,
    payment: Option<WardPaymentEvidence>,
) -> Result<WardResolution, ResidualCostRuntimeError> {
    if state.resolved_ward.contains(&trigger_id) {
        return Err(ResidualCostRuntimeError::TriggerAlreadyResolved);
    }
    let pending = state
        .pending_ward
        .get(&trigger_id)
        .cloned()
        .ok_or(ResidualCostRuntimeError::MissingWardTrigger)?;

    let current_source = state.stack.get(&pending.source.stack_id);
    if current_source.is_none_or(|source| {
        source.stack_ref != pending.source || source.status != StackObjectStatus::OnStack
    }) {
        if payment.is_some() {
            return Err(ResidualCostRuntimeError::PaymentNotApplicable);
        }
        state.pending_ward.remove(&trigger_id);
        state.resolved_ward.insert(trigger_id);
        return Ok(WardResolution::SourceNoLongerOnStack {
            trigger_id,
            source: pending.source,
            program_semantic_digest: pending.program_semantic_digest,
        });
    }

    if let Some(payment) = payment {
        if payment.payer != pending.payer {
            return Err(ResidualCostRuntimeError::WrongWardPayer);
        }
        if state.used_payment_ids.contains(&payment.payment_id) {
            return Err(ResidualCostRuntimeError::PaymentAlreadyUsed);
        }
        let mut staged = state.clone();
        apply_ward_payment(&mut staged, &pending, &payment)?;
        staged.used_payment_ids.insert(payment.payment_id);
        staged.pending_ward.remove(&trigger_id);
        staged.resolved_ward.insert(trigger_id);
        *state = staged;
        return Ok(WardResolution::Paid {
            trigger_id,
            payment_id: payment.payment_id,
            payer: payment.payer,
            payment,
            source: pending.source,
            program_semantic_digest: pending.program_semantic_digest,
        });
    }

    let source = state
        .stack
        .get_mut(&pending.source.stack_id)
        .ok_or(ResidualCostRuntimeError::MissingStackObject)?;
    let resolution = if source.counterable {
        source.status = StackObjectStatus::Countered;
        WardResolution::Countered {
            trigger_id,
            source: pending.source,
            program_semantic_digest: pending.program_semantic_digest,
        }
    } else {
        WardResolution::CounterAttemptPrevented {
            trigger_id,
            source: pending.source,
            program_semantic_digest: pending.program_semantic_digest,
        }
    };
    state.pending_ward.remove(&trigger_id);
    state.resolved_ward.insert(trigger_id);
    Ok(resolution)
}

fn apply_ward_payment(
    state: &mut ResidualCostGameState,
    pending: &PendingWardTrigger,
    payment: &WardPaymentEvidence,
) -> Result<(), ResidualCostRuntimeError> {
    let mut cursor = 0usize;
    apply_ward_cost(state, pending, payment, &pending.cost, &mut cursor, true)?;
    if cursor != payment.atoms.len() {
        return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
    }
    Ok(())
}

fn apply_ward_cost(
    state: &mut ResidualCostGameState,
    pending: &PendingWardTrigger,
    payment: &WardPaymentEvidence,
    cost: &WardCost,
    cursor: &mut usize,
    root: bool,
) -> Result<(), ResidualCostRuntimeError> {
    match cost {
        WardCost::All(costs) => {
            if root && payment.chosen_alternative.is_some() {
                return Err(ResidualCostRuntimeError::InvalidAlternativeChoice);
            }
            for cost in costs {
                apply_ward_cost(state, pending, payment, cost, cursor, false)?;
            }
            Ok(())
        }
        WardCost::OneOf(costs) => {
            let index = payment
                .chosen_alternative
                .ok_or(ResidualCostRuntimeError::InvalidAlternativeChoice)?;
            let cost = costs
                .get(index)
                .ok_or(ResidualCostRuntimeError::InvalidAlternativeChoice)?;
            apply_ward_cost(state, pending, payment, cost, cursor, false)
        }
        WardCost::Mana(requirement) => {
            if root && payment.chosen_alternative.is_some() {
                return Err(ResidualCostRuntimeError::InvalidAlternativeChoice);
            }
            let WardPaymentAtom::Mana { mana_units } = next_payment_atom(payment, cursor)? else {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            };
            consume_mana(state, pending.payer, requirement, mana_units)
        }
        WardCost::PayLife(amount) => {
            if root && payment.chosen_alternative.is_some() {
                return Err(ResidualCostRuntimeError::InvalidAlternativeChoice);
            }
            let WardPaymentAtom::PayLife(paid) = next_payment_atom(payment, cursor)? else {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            };
            if paid != amount {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            }
            pay_life(state, pending.payer, *amount)
        }
        WardCost::PayLifeEqualToProtectedPower => {
            if root && payment.chosen_alternative.is_some() {
                return Err(ResidualCostRuntimeError::InvalidAlternativeChoice);
            }
            let WardPaymentAtom::PayLifeEqualToProtectedPower {
                amount: paid,
                last_known_power_at_resolution,
            } = next_payment_atom(payment, cursor)?
            else {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            };
            let current_power = state
                .objects
                .get(&pending.protected_object.object_id)
                .filter(|object| {
                    object.object_ref == pending.protected_object
                        && object.zone == Zone::Battlefield
                })
                .map(|object| object.characteristics.power);
            let relevant_power = match (current_power, last_known_power_at_resolution) {
                (Some(power), None) => power,
                (Some(_), Some(_)) => {
                    return Err(ResidualCostRuntimeError::UnexpectedLastKnownInformation);
                }
                (None, Some(power)) => *power,
                (None, None) => {
                    return Err(ResidualCostRuntimeError::MissingLastKnownInformation);
                }
            };
            let required = u32::try_from(relevant_power.max(0))
                .map_err(|_| ResidualCostRuntimeError::CountOverflow)?;
            if *paid != required {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            }
            pay_life(state, pending.payer, required)
        }
        WardCost::Discard {
            amount,
            filter,
            random,
        } => {
            if root && payment.chosen_alternative.is_some() {
                return Err(ResidualCostRuntimeError::InvalidAlternativeChoice);
            }
            let WardPaymentAtom::Discard {
                cards,
                random_selection,
            } = next_payment_atom(payment, cursor)?
            else {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            };
            validate_selection_count(cards.len(), *amount)?;
            validate_unique_moves(cards)?;
            for card in cards {
                let object = state.current_object(card.before)?;
                if !filter.matches(object, pending.payer) {
                    return Err(ResidualCostRuntimeError::InvalidPaymentObject);
                }
            }
            if *random {
                validate_random_selection(state, pending.payer, filter, cards, random_selection)?;
            } else if random_selection.is_some() {
                return Err(ResidualCostRuntimeError::UnexpectedRandomSelection);
            }
            for card in cards {
                move_payment_object(state, card, Zone::Graveyard)?;
            }
            Ok(())
        }
        WardCost::Sacrifice { amount, filter } => {
            if root && payment.chosen_alternative.is_some() {
                return Err(ResidualCostRuntimeError::InvalidAlternativeChoice);
            }
            let WardPaymentAtom::Sacrifice { permanents } = next_payment_atom(payment, cursor)?
            else {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            };
            validate_selection_count(permanents.len(), *amount)?;
            validate_unique_moves(permanents)?;
            for permanent in permanents {
                let object = state.current_object(permanent.before)?;
                if !filter.matches(object, pending.payer) {
                    return Err(ResidualCostRuntimeError::InvalidPaymentObject);
                }
            }
            for permanent in permanents {
                move_payment_object(state, permanent, Zone::Graveyard)?;
            }
            Ok(())
        }
        WardCost::CollectEvidence {
            minimum_total_mana_value,
        } => {
            if root && payment.chosen_alternative.is_some() {
                return Err(ResidualCostRuntimeError::InvalidAlternativeChoice);
            }
            let WardPaymentAtom::CollectEvidence { cards } = next_payment_atom(payment, cursor)?
            else {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            };
            if cards.is_empty() {
                return Err(ResidualCostRuntimeError::InsufficientEvidence);
            }
            validate_unique_moves(cards)?;
            let mut total = 0u32;
            for card in cards {
                let object = state.current_object(card.before)?;
                if object.zone != Zone::Graveyard || object.owner != pending.payer {
                    return Err(ResidualCostRuntimeError::InvalidPaymentObject);
                }
                total = total
                    .checked_add(object.characteristics.mana_value)
                    .ok_or(ResidualCostRuntimeError::CountOverflow)?;
            }
            if total < *minimum_total_mana_value {
                return Err(ResidualCostRuntimeError::InsufficientEvidence);
            }
            for card in cards {
                move_payment_object(state, card, Zone::Exile)?;
            }
            Ok(())
        }
        WardCost::Blight { amount } => {
            if root && payment.chosen_alternative.is_some() {
                return Err(ResidualCostRuntimeError::InvalidAlternativeChoice);
            }
            let WardPaymentAtom::Blight {
                creature,
                amount: paid,
            } = next_payment_atom(payment, cursor)?
            else {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            };
            if paid != amount {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            }
            let object = state
                .objects
                .get_mut(&creature.object_id)
                .ok_or(ResidualCostRuntimeError::MissingObject)?;
            if object.object_ref != *creature
                || object.zone != Zone::Battlefield
                || object.controller != pending.payer
                || !object.characteristics.has_type(CardType::Creature)
                || !object.can_receive_minus_one_minus_one_counters
            {
                return Err(ResidualCostRuntimeError::InvalidPaymentObject);
            }
            object.minus_one_minus_one_counters = object
                .minus_one_minus_one_counters
                .checked_add(*amount)
                .ok_or(ResidualCostRuntimeError::CountOverflow)?;
            Ok(())
        }
        WardCost::GetPoisonCounters(amount) => {
            if root && payment.chosen_alternative.is_some() {
                return Err(ResidualCostRuntimeError::InvalidAlternativeChoice);
            }
            let WardPaymentAtom::GetPoisonCounters(received) = next_payment_atom(payment, cursor)?
            else {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            };
            if received != amount {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            }
            let player = state
                .players
                .get_mut(&pending.payer)
                .ok_or(ResidualCostRuntimeError::MissingPlayer)?;
            player.poison_counters = player
                .poison_counters
                .checked_add(*amount)
                .ok_or(ResidualCostRuntimeError::CountOverflow)?;
            Ok(())
        }
        WardCost::Waterbend(requirement) => {
            if root && payment.chosen_alternative.is_some() {
                return Err(ResidualCostRuntimeError::InvalidAlternativeChoice);
            }
            let WardPaymentAtom::Waterbend {
                mana_units,
                tapped_permanents,
            } = next_payment_atom(payment, cursor)?
            else {
                return Err(ResidualCostRuntimeError::PaymentAtomMismatch);
            };
            let mut unique = tapped_permanents.iter().copied().collect::<BTreeSet<_>>();
            if unique.len() != tapped_permanents.len() {
                return Err(ResidualCostRuntimeError::DuplicatePaymentObject);
            }
            let tapped_count =
                u32::try_from(unique.len()).map_err(|_| ResidualCostRuntimeError::CountOverflow)?;
            if tapped_count > requirement.generic {
                return Err(ResidualCostRuntimeError::ExcessWaterbendPayment);
            }
            for permanent in &unique {
                let object = state.current_object(*permanent)?;
                if object.zone != Zone::Battlefield
                    || object.controller != pending.payer
                    || object.tapped
                    || !(object.characteristics.has_type(CardType::Artifact)
                        || object.characteristics.has_type(CardType::Creature))
                {
                    return Err(ResidualCostRuntimeError::InvalidPaymentObject);
                }
            }
            let mut remaining = requirement.clone();
            remaining.generic -= tapped_count;
            consume_mana(state, pending.payer, &remaining, mana_units)?;
            for permanent in std::mem::take(&mut unique) {
                let object = state
                    .objects
                    .get_mut(&permanent.object_id)
                    .ok_or(ResidualCostRuntimeError::MissingObject)?;
                object.tapped = true;
            }
            Ok(())
        }
    }
}

fn next_payment_atom<'a>(
    payment: &'a WardPaymentEvidence,
    cursor: &mut usize,
) -> Result<&'a WardPaymentAtom, ResidualCostRuntimeError> {
    let atom = payment
        .atoms
        .get(*cursor)
        .ok_or(ResidualCostRuntimeError::PaymentAtomMismatch)?;
    *cursor = cursor
        .checked_add(1)
        .ok_or(ResidualCostRuntimeError::CountOverflow)?;
    Ok(atom)
}

fn validate_selection_count(actual: usize, expected: u32) -> Result<(), ResidualCostRuntimeError> {
    if u32::try_from(actual).ok() != Some(expected) {
        return Err(ResidualCostRuntimeError::InvalidSelectionCount);
    }
    Ok(())
}

fn validate_unique_moves(moves: &[ZoneMovePayment]) -> Result<(), ResidualCostRuntimeError> {
    let objects = moves
        .iter()
        .map(|zone_move| zone_move.before)
        .collect::<BTreeSet<_>>();
    if objects.len() != moves.len()
        || moves
            .iter()
            .any(|zone_move| zone_move.before.incarnation_id == zone_move.destination_incarnation)
    {
        return Err(ResidualCostRuntimeError::DuplicatePaymentObject);
    }
    Ok(())
}

fn validate_random_selection(
    state: &ResidualCostGameState,
    payer: PlayerId,
    filter: &WardDiscardFilter,
    cards: &[ZoneMovePayment],
    proof: &Option<RandomSelectionProof>,
) -> Result<(), ResidualCostRuntimeError> {
    let proof = proof
        .as_ref()
        .ok_or(ResidualCostRuntimeError::MissingRandomSelection)?;
    if proof.entropy_commitment.trim().is_empty() {
        return Err(ResidualCostRuntimeError::MissingRandomSelection);
    }
    let mut eligible = state
        .objects
        .values()
        .filter(|object| filter.matches(object, payer))
        .map(|object| object.object_ref)
        .collect::<Vec<_>>();
    eligible.sort();
    let selected = cards.iter().map(|card| card.before).collect::<Vec<_>>();
    if proof.eligible_cards_in_stable_order != eligible || proof.selected_cards != selected {
        return Err(ResidualCostRuntimeError::InvalidRandomSelection);
    }
    Ok(())
}

fn move_payment_object(
    state: &mut ResidualCostGameState,
    zone_move: &ZoneMovePayment,
    destination: Zone,
) -> Result<(), ResidualCostRuntimeError> {
    let object = state
        .objects
        .get_mut(&zone_move.before.object_id)
        .ok_or(ResidualCostRuntimeError::MissingObject)?;
    if object.object_ref != zone_move.before
        || zone_move.destination_incarnation == zone_move.before.incarnation_id
    {
        return Err(ResidualCostRuntimeError::ObjectIncarnationChanged);
    }
    object.zone = destination;
    object.object_ref.incarnation_id = zone_move.destination_incarnation;
    object.tapped = false;
    Ok(())
}

fn pay_life(
    state: &mut ResidualCostGameState,
    player: PlayerId,
    amount: u32,
) -> Result<(), ResidualCostRuntimeError> {
    let player = state
        .players
        .get_mut(&player)
        .ok_or(ResidualCostRuntimeError::MissingPlayer)?;
    if player.life < amount {
        return Err(ResidualCostRuntimeError::CannotPayLife);
    }
    player.life -= amount;
    Ok(())
}

fn consume_mana(
    state: &mut ResidualCostGameState,
    player: PlayerId,
    requirement: &ManaRequirement,
    unit_ids: &[ManaUnitId],
) -> Result<(), ResidualCostRuntimeError> {
    let player = state
        .players
        .get_mut(&player)
        .ok_or(ResidualCostRuntimeError::MissingPlayer)?;
    let unique = unit_ids.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != unit_ids.len()
        || u32::try_from(unit_ids.len()).ok() != Some(requirement.required_unit_count())
    {
        return Err(ResidualCostRuntimeError::InvalidManaPayment);
    }
    let units = unit_ids
        .iter()
        .map(|id| {
            player
                .mana_pool
                .get(id)
                .copied()
                .ok_or(ResidualCostRuntimeError::InvalidManaPayment)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut unused = units.iter().collect::<BTreeSet<_>>();
    consume_exact_color(&mut unused, ManaColor::White, requirement.white)?;
    consume_exact_color(&mut unused, ManaColor::Blue, requirement.blue)?;
    consume_exact_color(&mut unused, ManaColor::Black, requirement.black)?;
    consume_exact_color(&mut unused, ManaColor::Red, requirement.red)?;
    consume_exact_color(&mut unused, ManaColor::Green, requirement.green)?;
    consume_exact_color(&mut unused, ManaColor::Colorless, requirement.colorless)?;
    for _ in 0..requirement.snow {
        let unit = unused
            .iter()
            .copied()
            .find(|unit| unit.from_snow_source)
            .ok_or(ResidualCostRuntimeError::InvalidManaPayment)?;
        unused.remove(unit);
    }
    if u32::try_from(unused.len()).ok() != Some(requirement.generic) {
        return Err(ResidualCostRuntimeError::InvalidManaPayment);
    }
    for id in unit_ids {
        player.mana_pool.remove(id);
    }
    Ok(())
}

fn consume_exact_color(
    unused: &mut BTreeSet<&ManaUnit>,
    color: ManaColor,
    amount: u32,
) -> Result<(), ResidualCostRuntimeError> {
    for _ in 0..amount {
        let unit = unused
            .iter()
            .copied()
            .find(|unit| unit.color == color)
            .ok_or(ResidualCostRuntimeError::InvalidManaPayment)?;
        unused.remove(unit);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidualCostRuntimeError {
    WrongProgramKind,
    WrongControllerOrSource,
    MissingObject,
    ObjectIncarnationChanged,
    MissingPlayer,
    MissingStackObject,
    AffinitySourceNotCurrentSpell,
    WrongCostDeterminationPhase,
    ManaRequirementOverflow,
    CountOverflow,
    AbilityAlreadyApplied,
    InvalidManaPayment,
    InvalidTargetEvent,
    TargetEventAlreadyHandled,
    WardRequiresOpponentSource,
    InvalidTriggerOrder,
    TriggerAlreadyExists,
    MissingWardInstance,
    MissingWardTrigger,
    TriggerAlreadyResolved,
    WrongWardPayer,
    PaymentAlreadyUsed,
    PaymentNotApplicable,
    PaymentAtomMismatch,
    InvalidAlternativeChoice,
    InvalidPaymentObject,
    InvalidSelectionCount,
    DuplicatePaymentObject,
    MissingRandomSelection,
    UnexpectedRandomSelection,
    InvalidRandomSelection,
    InsufficientEvidence,
    CannotPayLife,
    MissingLastKnownInformation,
    UnexpectedLastKnownInformation,
    ExcessWaterbendPayment,
}

impl fmt::Display for ResidualCostRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ResidualCostRuntimeError {}
