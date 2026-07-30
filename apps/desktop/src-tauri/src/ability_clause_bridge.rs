//! Content keyed bridge for complete single-clause ability programs.
//!
//! The established ability compiler and simulator already own a number of
//! exact Oracle families that the bounded clause compiler does not. This
//! bridge retains the same typed program without treating unsupported or
//! card-level lifecycle candidates as complete single-clause programs.

use sha2::{Digest, Sha256};

use crate::ability_program::{
    AbilityCompilation, AbilityTiming, ActivationWindow, CardType, ControllerRelation,
    EXECUTABLE_ABILITY_PROGRAM_VERSION, ExecutableAbility, ObjectFilter, OracleCardInput,
    SpecificCardType, TriggerEvent, TriggerEventKind, compile_executable_ability_program,
};

pub const ABILITY_CLAUSE_BRIDGE_COMPILER_VERSION: &str = "ability-clause-bridge-compiler-0.2";
pub const ABILITY_CLAUSE_BRIDGE_RUNTIME_VERSION: &str = "ability-clause-bridge-runtime-0.2";

/// Public, content-derived timing contract for one bridged ability clause.
///
/// This mirrors the complete timing information retained by the established
/// ability program without exposing card names, snapshot coordinates, or
/// snapshot metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AbilityClauseTimingEnvelope {
    DeckConstruction,
    SpellResolution,
    AuraSpellTargeting,
    Activated {
        window: AbilityClauseActivationWindow,
    },
    Triggered {
        event: AbilityClauseTriggerEvent,
    },
    StaticModifier,
}

impl AbilityClauseTimingEnvelope {
    pub fn family(&self) -> &'static str {
        match self {
            Self::DeckConstruction => "deck_construction",
            Self::SpellResolution => "spell_resolution",
            Self::AuraSpellTargeting => "aura_spell_targeting",
            Self::Activated { .. } => "activated",
            Self::Triggered { .. } => "triggered",
            Self::StaticModifier => "static_modifier",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AbilityClauseActivationWindow {
    NormalPriority,
    InstantSpeedOnly,
    SorcerySpeedOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AbilityClauseTriggerEvent {
    pub kind: AbilityClauseTriggerEventKind,
    pub actor: AbilityClauseControllerRelation,
    pub object_filter: AbilityClauseObjectFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AbilityClauseTriggerEventKind {
    BeginningOfUpkeep,
    BeginningOfEndStep,
    SpellCast,
    FirstFilteredSpellCastEachTurn,
    SecondSpellCastEachTurn,
    CardDraw,
    ThisSpellCast,
    PermanentEntersBattlefield,
    PermanentBecomesTapped,
    PermanentTappedForMana,
    EnchantedCreatureDealsDamageToOpponent,
    EquippedCreatureDies,
    CreatureDealsCombatDamageToPlayer,
    OneOrMoreCreaturesDealCombatDamageToPlayer,
    ChosenTypeCreatureEntersOrAttacks,
    OtherFlyingCreatureEntersBattlefield,
    SourceBecomesTargetByOpponentSpellOrAbility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AbilityClauseControllerRelation {
    You,
    Opponent,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct AbilityClauseObjectFilter {
    pub card_type: Option<AbilityClauseCardType>,
    pub any_of_card_types: Vec<AbilityClauseSpecificCardType>,
    pub excluded_card_type: Option<AbilityClauseCardType>,
    pub subtype: Option<String>,
    pub excluded_subtype: Option<String>,
    pub nonland: bool,
    pub controller: Option<AbilityClauseControllerRelation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AbilityClauseSpecificCardType {
    Artifact,
    Enchantment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AbilityClauseCardType {
    Artifact,
    Creature,
    Dragon,
    Land,
    Permanent,
    Spell,
    Card,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityClauseBridgeProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    timing: AbilityClauseTimingEnvelope,
    ability: ExecutableAbility,
}

impl AbilityClauseBridgeProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn timing(&self) -> &AbilityClauseTimingEnvelope {
        &self.timing
    }

    #[allow(dead_code)]
    pub(crate) fn ability(&self) -> &ExecutableAbility {
        &self.ability
    }
}

pub fn compile_ability_clause_bridge(
    exact_source: &str,
    source_name: &str,
    source_type_line: &str,
) -> Option<AbilityClauseBridgeProgram> {
    if exact_source.trim() != exact_source || exact_source.is_empty() {
        return None;
    }
    let compiled = compile_executable_ability_program(OracleCardInput {
        name: source_name,
        layout: "normal",
        type_line: source_type_line,
        oracle_text: exact_source,
        has_face_records: false,
    });
    if compiled.version != EXECUTABLE_ABILITY_PROGRAM_VERSION
        || compiled.necropotence_lifecycle.is_some()
        || compiled.self_transfer_tutor_permanent.is_some()
        || compiled.entry_linked_permanent.is_some()
        || compiled.atomic_transaction.is_some()
        || compiled.graveyard_reclamation.is_some()
        || !compiled.face_programs.is_empty()
    {
        return None;
    }
    let [AbilityCompilation::Executable(ability)] = compiled.abilities.as_slice() else {
        return None;
    };
    if ability.clause_index != 0
        || ability.normalized_oracle.trim().is_empty()
        || ability.effects.is_empty()
    {
        return None;
    }
    let timing = AbilityClauseTimingEnvelope::from(&ability.timing);
    let semantic_digest = ability_clause_semantic_digest(exact_source, ability, &timing);
    Some(AbilityClauseBridgeProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: ability.normalized_oracle.clone(),
        semantic_digest,
        timing,
        ability: ability.clone(),
    })
}

fn ability_clause_semantic_digest(
    exact_source: &str,
    ability: &ExecutableAbility,
    timing: &AbilityClauseTimingEnvelope,
) -> String {
    let components = [
        "ability-clause-bridge-content/v2".to_owned(),
        ABILITY_CLAUSE_BRIDGE_COMPILER_VERSION.to_owned(),
        ABILITY_CLAUSE_BRIDGE_RUNTIME_VERSION.to_owned(),
        EXECUTABLE_ABILITY_PROGRAM_VERSION.to_owned(),
        exact_source.to_owned(),
        ability.normalized_oracle.clone(),
        format!("timing:{timing:?}"),
        format!("costs:{:?}", ability.costs),
        format!("preconditions:{:?}", ability.preconditions),
        format!("effects:{:?}", ability.effects),
    ];
    let mut hasher = Sha256::new();
    for component in components {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

impl From<&AbilityTiming> for AbilityClauseTimingEnvelope {
    fn from(timing: &AbilityTiming) -> Self {
        match timing {
            AbilityTiming::DeckConstruction => Self::DeckConstruction,
            AbilityTiming::SpellResolution => Self::SpellResolution,
            AbilityTiming::AuraSpellTargeting => Self::AuraSpellTargeting,
            AbilityTiming::Activated { window } => Self::Activated {
                window: (*window).into(),
            },
            AbilityTiming::Triggered { event } => Self::Triggered {
                event: event.into(),
            },
            AbilityTiming::StaticModifier => Self::StaticModifier,
        }
    }
}

impl From<ActivationWindow> for AbilityClauseActivationWindow {
    fn from(window: ActivationWindow) -> Self {
        match window {
            ActivationWindow::NormalPriority => Self::NormalPriority,
            ActivationWindow::InstantSpeedOnly => Self::InstantSpeedOnly,
            ActivationWindow::SorcerySpeedOnly => Self::SorcerySpeedOnly,
        }
    }
}

impl From<&TriggerEvent> for AbilityClauseTriggerEvent {
    fn from(event: &TriggerEvent) -> Self {
        Self {
            kind: event.kind.into(),
            actor: event.actor.into(),
            object_filter: (&event.object_filter).into(),
        }
    }
}

impl From<TriggerEventKind> for AbilityClauseTriggerEventKind {
    fn from(kind: TriggerEventKind) -> Self {
        match kind {
            TriggerEventKind::BeginningOfUpkeep => Self::BeginningOfUpkeep,
            TriggerEventKind::BeginningOfEndStep => Self::BeginningOfEndStep,
            TriggerEventKind::SpellCast => Self::SpellCast,
            TriggerEventKind::FirstFilteredSpellCastEachTurn => {
                Self::FirstFilteredSpellCastEachTurn
            }
            TriggerEventKind::SecondSpellCastEachTurn => Self::SecondSpellCastEachTurn,
            TriggerEventKind::CardDraw => Self::CardDraw,
            TriggerEventKind::ThisSpellCast => Self::ThisSpellCast,
            TriggerEventKind::PermanentEntersBattlefield => Self::PermanentEntersBattlefield,
            TriggerEventKind::PermanentBecomesTapped => Self::PermanentBecomesTapped,
            TriggerEventKind::PermanentTappedForMana => Self::PermanentTappedForMana,
            TriggerEventKind::EnchantedCreatureDealsDamageToOpponent => {
                Self::EnchantedCreatureDealsDamageToOpponent
            }
            TriggerEventKind::EquippedCreatureDies => Self::EquippedCreatureDies,
            TriggerEventKind::CreatureDealsCombatDamageToPlayer => {
                Self::CreatureDealsCombatDamageToPlayer
            }
            TriggerEventKind::OneOrMoreCreaturesDealCombatDamageToPlayer => {
                Self::OneOrMoreCreaturesDealCombatDamageToPlayer
            }
            TriggerEventKind::ChosenTypeCreatureEntersOrAttacks => {
                Self::ChosenTypeCreatureEntersOrAttacks
            }
            TriggerEventKind::OtherFlyingCreatureEntersBattlefield => {
                Self::OtherFlyingCreatureEntersBattlefield
            }
            TriggerEventKind::SourceBecomesTargetByOpponentSpellOrAbility => {
                Self::SourceBecomesTargetByOpponentSpellOrAbility
            }
        }
    }
}

impl From<ControllerRelation> for AbilityClauseControllerRelation {
    fn from(relation: ControllerRelation) -> Self {
        match relation {
            ControllerRelation::You => Self::You,
            ControllerRelation::Opponent => Self::Opponent,
            ControllerRelation::Any => Self::Any,
        }
    }
}

impl From<&ObjectFilter> for AbilityClauseObjectFilter {
    fn from(filter: &ObjectFilter) -> Self {
        Self {
            card_type: filter.card_type.map(Into::into),
            any_of_card_types: filter
                .any_of_card_types
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
            excluded_card_type: filter.excluded_card_type.map(Into::into),
            subtype: filter.subtype.clone(),
            excluded_subtype: filter.excluded_subtype.clone(),
            nonland: filter.nonland,
            controller: filter.controller.map(Into::into),
        }
    }
}

impl From<SpecificCardType> for AbilityClauseSpecificCardType {
    fn from(card_type: SpecificCardType) -> Self {
        match card_type {
            SpecificCardType::Artifact => Self::Artifact,
            SpecificCardType::Enchantment => Self::Enchantment,
        }
    }
}

impl From<CardType> for AbilityClauseCardType {
    fn from(card_type: CardType) -> Self {
        match card_type {
            CardType::Artifact => Self::Artifact,
            CardType::Creature => Self::Creature,
            CardType::Dragon => Self::Dragon,
            CardType::Land => Self::Land,
            CardType::Permanent => Self::Permanent,
            CardType::Spell => Self::Spell,
            CardType::Card => Self::Card,
        }
    }
}
