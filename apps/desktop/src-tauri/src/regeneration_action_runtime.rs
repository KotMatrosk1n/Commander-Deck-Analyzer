//! Exact regeneration action envelopes for reviewed Oracle clauses.
//!
//! The official keyword runtime owns the four complete standalone forms:
//! `Regenerate target creature.`, its canonical reminder variant,
//! `Regenerate target permanent.`, and `Regenerate each creature you control.`
//! This module owns complete activated and triggered regeneration actions plus
//! the old static destruction replacement wording. It preserves activation
//! costs, target selection, target legality, object incarnation, one shot
//! replacement expiry, static replacement lifetime, and effects that prohibit
//! regeneration.
//!
//! Ability grants, modal fragments, joke costs, clauses with trailing effects,
//! and compounds whose other instructions are not modeled remain rejected.
//! Program identity is derived from exact Oracle content, typed semantic
//! context, and versioned rules context. Card names, identifiers, database
//! rows, snapshot hashes, addresses, ordering, and timestamps are never inputs.
//! Recognition is not production execution coverage. No production adapter is
//! connected yet.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

use crate::keyword_rules_runtime::{
    CardType, CombatKeyword, KeywordAction, KeywordExecutionError, KeywordGameState,
    KeywordProgram, KeywordProgramInput, ManaColor, ManaCost, ManaPayment, ManaSymbol, ManaUnitId,
    ObjectId, PlayerId, ProtectionTarget, RegenerationChoice, RegenerationReplacement,
    SourceProfile, SymbolPayment, Zone, clear_end_of_turn_regeneration, compile_keyword_program,
    execute_keyword_action, remove_static_regeneration, resolve_destruction, targeting_is_legal,
};

pub const REGENERATION_ACTION_COMPILER_VERSION: &str = "regeneration-action-compiler-0.1";
pub const REGENERATION_ACTION_RUNTIME_VERSION: &str = "regeneration-action-runtime-0.1";
pub const REGENERATION_ACTION_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:107.2,117.12,119.4,122.1,400.7,601.2b,601.2f-h,602.2b,603.3d,608.2b,608.2h,609.3,614.1,614.6,616.1,701.19,701.21";

pub const fn regeneration_action_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegenerationClauseFamily {
    ActivatedAction,
    TriggeredAction,
    StaticDestructionReplacement,
    StandaloneResolutionAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarlierRegenerationClauseOwner {
    OfficialKeywordRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegenerationClauseClassification {
    Program(RegenerationActionProgram),
    EarlierOwner {
        family: RegenerationClauseFamily,
        owner: EarlierRegenerationClauseOwner,
    },
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncarnationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectReference {
    pub object: ObjectId,
    pub incarnation: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipientSelectionTime {
    WhenAbilityIsPutOnStack,
    OnResolution,
    StaticReplacementEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipientCardinality {
    ExactlyOne,
    ZeroOrMore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequiredController {
    Any,
    EffectController,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegenerationTargetFilter {
    pub required_card_types: BTreeSet<CardType>,
    pub any_colors: BTreeSet<ManaColor>,
    pub any_subtypes: BTreeSet<String>,
    pub controller: RequiredController,
    pub excludes_source: bool,
    pub minimum_power: Option<i32>,
    pub required_counter: Option<String>,
}

impl RegenerationTargetFilter {
    fn battlefield_creature() -> Self {
        Self {
            required_card_types: BTreeSet::from([CardType::Creature]),
            any_colors: BTreeSet::new(),
            any_subtypes: BTreeSet::new(),
            controller: RequiredController::Any,
            excludes_source: false,
            minimum_power: None,
            required_counter: None,
        }
    }

    fn battlefield_permanent() -> Self {
        Self {
            required_card_types: BTreeSet::new(),
            any_colors: BTreeSet::new(),
            any_subtypes: BTreeSet::new(),
            controller: RequiredController::Any,
            excludes_source: false,
            minimum_power: None,
            required_counter: None,
        }
    }

    fn stable_id(&self) -> String {
        format!(
            "types={};colors={};subtypes={};controller={};another={};power={};counter={}",
            self.required_card_types
                .iter()
                .map(card_type_stable_id)
                .collect::<Vec<_>>()
                .join(","),
            self.any_colors
                .iter()
                .map(mana_color_stable_id)
                .collect::<Vec<_>>()
                .join(","),
            self.any_subtypes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(","),
            match self.controller {
                RequiredController::Any => "any",
                RequiredController::EffectController => "effect-controller",
            },
            self.excludes_source,
            self.minimum_power
                .map_or_else(|| "none".to_owned(), |power| power.to_string()),
            self.required_counter.as_deref().unwrap_or("none"),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegenerationRecipient {
    SourcePermanent {
        selection_time: RecipientSelectionTime,
    },
    EnchantedCreature {
        selection_time: RecipientSelectionTime,
    },
    EquippedCreature {
        selection_time: RecipientSelectionTime,
    },
    Target {
        filter: RegenerationTargetFilter,
        cardinality: RecipientCardinality,
        selection_time: RecipientSelectionTime,
    },
    ControlledSet {
        filter: RegenerationTargetFilter,
        cardinality: RecipientCardinality,
        selection_time: RecipientSelectionTime,
    },
}

impl RegenerationRecipient {
    fn stable_id(&self) -> String {
        match self {
            Self::SourcePermanent { selection_time } => {
                format!("source/{}", selection_time_stable_id(*selection_time))
            }
            Self::EnchantedCreature { selection_time } => {
                format!("enchanted/{}", selection_time_stable_id(*selection_time))
            }
            Self::EquippedCreature { selection_time } => {
                format!("equipped/{}", selection_time_stable_id(*selection_time))
            }
            Self::Target {
                filter,
                cardinality,
                selection_time,
            } => format!(
                "target/{}/{}/{}",
                filter.stable_id(),
                cardinality_stable_id(*cardinality),
                selection_time_stable_id(*selection_time)
            ),
            Self::ControlledSet {
                filter,
                cardinality,
                selection_time,
            } => format!(
                "controlled-set/{}/{}/{}",
                filter.stable_id(),
                cardinality_stable_id(*cardinality),
                selection_time_stable_id(*selection_time)
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderEvidence {
    Absent,
    CanonicalOneShot,
    CanonicalStatic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SacrificeRelation {
    Source,
    Another,
    Chosen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostObjectFilter {
    pub required_card_types: BTreeSet<CardType>,
    pub required_subtype: Option<String>,
}

impl CostObjectFilter {
    fn stable_id(&self) -> String {
        format!(
            "types={};subtype={}",
            self.required_card_types
                .iter()
                .map(card_type_stable_id)
                .collect::<Vec<_>>()
                .join(","),
            self.required_subtype.as_deref().unwrap_or("none")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegenerationActivationCost {
    Mana(ManaCost),
    TapSource,
    DiscardCards {
        count: usize,
        random: bool,
    },
    Sacrifice {
        count: usize,
        relation: SacrificeRelation,
        filter: CostObjectFilter,
    },
    PayLife(u32),
    RemoveCountersFromSource {
        count: u32,
        counter: String,
    },
    ExileCreatureCardsFromOwnGraveyard {
        count: usize,
        top_only: bool,
    },
    ReturnSourceToOwnersHand,
}

impl RegenerationActivationCost {
    fn stable_id(&self) -> String {
        match self {
            Self::Mana(cost) => format!(
                "mana/{}",
                cost.symbols
                    .iter()
                    .map(mana_symbol_stable_id)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::TapSource => "tap-source".to_owned(),
            Self::DiscardCards { count, random } => {
                format!("discard/{count}/random={random}")
            }
            Self::Sacrifice {
                count,
                relation,
                filter,
            } => format!(
                "sacrifice/{count}/{}/{}",
                match relation {
                    SacrificeRelation::Source => "source",
                    SacrificeRelation::Another => "another",
                    SacrificeRelation::Chosen => "chosen",
                },
                filter.stable_id()
            ),
            Self::PayLife(amount) => format!("pay-life/{amount}"),
            Self::RemoveCountersFromSource { count, counter } => {
                format!("remove-counter/{count}/{counter}")
            }
            Self::ExileCreatureCardsFromOwnGraveyard { count, top_only } => {
                format!("exile-graveyard-creature/{count}/top={top_only}")
            }
            Self::ReturnSourceToOwnersHand => "return-source-to-owner-hand".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationRestriction {
    None,
    ControlAnotherColorlessCreature,
    SourceBlockedOrWasBlockedByBlueCreatureThisTurn,
    ControlledCreaturesTotalPowerAtLeast(u32),
    OwnGraveyardHasAtLeast(u32),
}

impl ActivationRestriction {
    fn stable_id(self) -> String {
        match self {
            Self::None => "none".to_owned(),
            Self::ControlAnotherColorlessCreature => {
                "control-another-colorless-creature".to_owned()
            }
            Self::SourceBlockedOrWasBlockedByBlueCreatureThisTurn => {
                "source-blocked-or-was-blocked-by-blue-this-turn".to_owned()
            }
            Self::ControlledCreaturesTotalPowerAtLeast(amount) => {
                format!("controlled-creatures-total-power-at-least/{amount}")
            }
            Self::OwnGraveyardHasAtLeast(amount) => {
                format!("own-graveyard-at-least/{amount}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivatedRegenerationProgram {
    pub costs: Vec<RegenerationActivationCost>,
    pub recipient: RegenerationRecipient,
    pub restriction: ActivationRestriction,
    pub reminder: ReminderEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenerationTrigger {
    SourceTurnedFaceUp,
    SourceBecameBlocked,
    ControllerCastSpiritOrArcaneSpell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggeredRegenerationProgram {
    pub trigger: RegenerationTrigger,
    pub recipient: RegenerationRecipient,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticRegenerationProgram {
    pub recipient: RegenerationRecipient,
    pub replacement: RegenerationReplacement,
    pub reminder: ReminderEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionRegenerationProgram {
    pub recipient: RegenerationRecipient,
    pub reminder: ReminderEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegenerationActionKind {
    Activated(ActivatedRegenerationProgram),
    Triggered(TriggeredRegenerationProgram),
    StaticDestructionReplacement(StaticRegenerationProgram),
    StandaloneResolution(ResolutionRegenerationProgram),
}

impl RegenerationActionKind {
    pub const fn family(&self) -> RegenerationClauseFamily {
        match self {
            Self::Activated(_) => RegenerationClauseFamily::ActivatedAction,
            Self::Triggered(_) => RegenerationClauseFamily::TriggeredAction,
            Self::StaticDestructionReplacement(_) => {
                RegenerationClauseFamily::StaticDestructionReplacement
            }
            Self::StandaloneResolution(_) => RegenerationClauseFamily::StandaloneResolutionAction,
        }
    }

    fn stable_id(&self) -> String {
        match self {
            Self::Activated(program) => format!(
                "activated/costs={}/recipient={}/restriction={}/reminder={}",
                program
                    .costs
                    .iter()
                    .map(RegenerationActivationCost::stable_id)
                    .collect::<Vec<_>>()
                    .join("+"),
                program.recipient.stable_id(),
                program.restriction.stable_id(),
                reminder_stable_id(program.reminder),
            ),
            Self::Triggered(program) => format!(
                "triggered/trigger={}/recipient={}",
                trigger_stable_id(program.trigger),
                program.recipient.stable_id()
            ),
            Self::StaticDestructionReplacement(program) => format!(
                "static/recipient={}/replacement={}/reminder={}",
                program.recipient.stable_id(),
                replacement_stable_id(program.replacement),
                reminder_stable_id(program.reminder)
            ),
            Self::StandaloneResolution(program) => format!(
                "resolution/recipient={}/reminder={}",
                program.recipient.stable_id(),
                reminder_stable_id(program.reminder)
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegenerationActionProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    kind: RegenerationActionKind,
}

impl RegenerationActionProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &RegenerationActionKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        regeneration_action_production_adapter_connected()
    }
}

pub fn compile_regeneration_action_program(
    exact_source: &str,
    normalized_source: &str,
) -> Option<RegenerationActionProgram> {
    match classify_regeneration_action_clause(exact_source, normalized_source) {
        RegenerationClauseClassification::Program(program) => Some(program),
        RegenerationClauseClassification::EarlierOwner { .. }
        | RegenerationClauseClassification::Rejected => None,
    }
}

pub fn classify_regeneration_action_clause(
    exact_source: &str,
    normalized_source: &str,
) -> RegenerationClauseClassification {
    if !is_complete_single_line(exact_source)
        || !is_complete_single_line(normalized_source)
        || collapse_whitespace(exact_source) != normalized_source
    {
        return RegenerationClauseClassification::Rejected;
    }

    if is_earlier_owned_standalone(exact_source) {
        return RegenerationClauseClassification::EarlierOwner {
            family: RegenerationClauseFamily::StandaloneResolutionAction,
            owner: EarlierRegenerationClauseOwner::OfficialKeywordRuntime,
        };
    }

    let kind = parse_static_replacement(exact_source)
        .or_else(|| parse_triggered_action(exact_source))
        .or_else(|| parse_activated_action(exact_source));
    let Some(kind) = kind else {
        return RegenerationClauseClassification::Rejected;
    };
    let semantic_digest = regeneration_semantic_digest(exact_source, normalized_source, &kind);
    RegenerationClauseClassification::Program(RegenerationActionProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        semantic_digest,
        kind,
    })
}

pub fn contains_regeneration_lexeme(source: &str) -> bool {
    source.to_ascii_lowercase().contains("regenerat")
}

fn is_earlier_owned_standalone(source: &str) -> bool {
    matches!(
        source,
        "Regenerate target creature."
            | "Regenerate target creature. (The next time that creature would be destroyed this turn, instead tap it, remove it from combat, and heal all damage on it.)"
            | "Regenerate target permanent."
            | "Regenerate each creature you control."
    )
}

fn parse_static_replacement(source: &str) -> Option<RegenerationActionKind> {
    let reminder = match source {
        "If this creature would be destroyed, regenerate it." => ReminderEvidence::Absent,
        "If this creature would be destroyed, regenerate it. (Tap it, remove it from combat, and heal all damage on it.)" => {
            ReminderEvidence::CanonicalStatic
        }
        _ => return None,
    };
    Some(RegenerationActionKind::StaticDestructionReplacement(
        StaticRegenerationProgram {
            recipient: RegenerationRecipient::SourcePermanent {
                selection_time: RecipientSelectionTime::StaticReplacementEvent,
            },
            replacement: RegenerationReplacement::EveryDestructionWhileStaticEffectApplies,
            reminder,
        },
    ))
}

fn parse_triggered_action(source: &str) -> Option<RegenerationActionKind> {
    let (trigger, effect) = match source {
        "When this creature is turned face up, regenerate target creature." => (
            RegenerationTrigger::SourceTurnedFaceUp,
            "Regenerate target creature.",
        ),
        "Whenever this creature becomes blocked, regenerate it." => {
            (RegenerationTrigger::SourceBecameBlocked, "Regenerate it.")
        }
        "Whenever you cast a Spirit or Arcane spell, regenerate target creature." => (
            RegenerationTrigger::ControllerCastSpiritOrArcaneSpell,
            "Regenerate target creature.",
        ),
        "Whenever you cast a Spirit or Arcane spell, regenerate this creature." => (
            RegenerationTrigger::ControllerCastSpiritOrArcaneSpell,
            "Regenerate this creature.",
        ),
        _ => return None,
    };
    let (recipient, reminder) =
        parse_regeneration_effect(effect, RecipientSelectionTime::WhenAbilityIsPutOnStack)?;
    if reminder != ReminderEvidence::Absent {
        return None;
    }
    Some(RegenerationActionKind::Triggered(
        TriggeredRegenerationProgram { trigger, recipient },
    ))
}

fn parse_activated_action(source: &str) -> Option<RegenerationActionKind> {
    if source.contains('"')
        || source.starts_with('•')
        || source.contains(" When ")
        || source.contains(" Then ")
        || source.contains(". You ")
    {
        return None;
    }

    let (restriction_prefix, body) = if let Some(body) = source.strip_prefix("Formidable \u{2014} ")
    {
        (
            Some(ActivationRestriction::ControlledCreaturesTotalPowerAtLeast(
                8,
            )),
            body,
        )
    } else if let Some(body) = source.strip_prefix("Threshold \u{2014} ") {
        (Some(ActivationRestriction::OwnGraveyardHasAtLeast(7)), body)
    } else {
        (None, source)
    };

    let (cost_source, raw_effect) = body.split_once(": ")?;
    if cost_source.is_empty() || raw_effect.is_empty() || raw_effect.contains(": ") {
        return None;
    }

    let (effect, suffix_restriction) = if let Some(effect) =
        raw_effect.strip_suffix(" Activate only if you control another colorless creature.")
    {
        (
            effect,
            Some(ActivationRestriction::ControlAnotherColorlessCreature),
        )
    } else if let Some(effect) = raw_effect.strip_suffix(
        " Activate only if this creature blocked or was blocked by a blue creature this turn.",
    ) {
        (
            effect,
            Some(ActivationRestriction::SourceBlockedOrWasBlockedByBlueCreatureThisTurn),
        )
    } else if let Some(effect) = raw_effect
        .strip_suffix(" Activate only if creatures you control have total power 8 or greater.")
    {
        (
            effect,
            Some(ActivationRestriction::ControlledCreaturesTotalPowerAtLeast(
                8,
            )),
        )
    } else if let Some(effect) = raw_effect
        .strip_suffix(" Activate only if there are seven or more cards in your graveyard.")
    {
        (
            effect,
            Some(ActivationRestriction::OwnGraveyardHasAtLeast(7)),
        )
    } else {
        (raw_effect, None)
    };

    let restriction = match (restriction_prefix, suffix_restriction) {
        (None, None) => ActivationRestriction::None,
        (Some(prefix), Some(suffix)) if prefix == suffix => prefix,
        (Some(prefix), None) => prefix,
        (None, Some(suffix)) => suffix,
        (Some(_), Some(_)) => return None,
    };

    let (effect, snow_reminder) = if let Some(effect) =
        effect.strip_suffix(" ({S} can be paid with one mana from a snow source.)")
    {
        (effect, true)
    } else {
        (effect, false)
    };

    let costs = parse_activation_costs(cost_source)?;
    if snow_reminder
        != costs.iter().any(|cost| {
            matches!(
                cost,
                RegenerationActivationCost::Mana(ManaCost { symbols, .. })
                    if symbols.contains(&ManaSymbol::Snow)
            )
        })
    {
        return None;
    }
    let (recipient, reminder) =
        parse_regeneration_effect(effect, RecipientSelectionTime::WhenAbilityIsPutOnStack)?;
    Some(RegenerationActionKind::Activated(
        ActivatedRegenerationProgram {
            costs,
            recipient,
            restriction,
            reminder,
        },
    ))
}

fn parse_regeneration_effect(
    source: &str,
    selection_time: RecipientSelectionTime,
) -> Option<(RegenerationRecipient, ReminderEvidence)> {
    let (core, reminder) = strip_regeneration_reminder(source)?;
    let recipient = match core {
        "Regenerate this creature." | "Regenerate it." => {
            RegenerationRecipient::SourcePermanent { selection_time }
        }
        "Regenerate enchanted creature." => RegenerationRecipient::EnchantedCreature {
            selection_time: RecipientSelectionTime::OnResolution,
        },
        "Regenerate equipped creature." => RegenerationRecipient::EquippedCreature {
            selection_time: RecipientSelectionTime::OnResolution,
        },
        "Regenerate each creature you control." => {
            let mut filter = RegenerationTargetFilter::battlefield_creature();
            filter.controller = RequiredController::EffectController;
            RegenerationRecipient::ControlledSet {
                filter,
                cardinality: RecipientCardinality::ZeroOrMore,
                selection_time: RecipientSelectionTime::OnResolution,
            }
        }
        _ => {
            if let Some(subtype) = core
                .strip_prefix("Regenerate all ")
                .and_then(|rest| rest.strip_suffix(" creatures you control."))
                .and_then(normalize_subtype)
            {
                let mut filter = RegenerationTargetFilter::battlefield_creature();
                filter.controller = RequiredController::EffectController;
                filter.any_subtypes.insert(subtype);
                RegenerationRecipient::ControlledSet {
                    filter,
                    cardinality: RecipientCardinality::ZeroOrMore,
                    selection_time: RecipientSelectionTime::OnResolution,
                }
            } else {
                let target_phrase = core.strip_prefix("Regenerate ")?.strip_suffix('.')?;
                let filter = parse_target_filter(target_phrase)?;
                RegenerationRecipient::Target {
                    filter,
                    cardinality: RecipientCardinality::ExactlyOne,
                    selection_time,
                }
            }
        }
    };
    Some((recipient, reminder))
}

fn strip_regeneration_reminder(source: &str) -> Option<(&str, ReminderEvidence)> {
    const REMINDERS: [&str; 4] = [
        " (The next time this creature would be destroyed this turn, instead tap it, remove it from combat, and heal all damage on it.)",
        " (The next time it would be destroyed this turn, instead tap it, remove it from combat, and heal all damage on it.)",
        " (The next time that creature would be destroyed this turn, instead tap it, remove it from combat, and heal all damage on it.)",
        " (The next time the creature would be destroyed this turn, instead tap it, remove it from combat, and heal all damage on it.)",
    ];
    for reminder in REMINDERS {
        if let Some(core) = source.strip_suffix(reminder) {
            return Some((core, ReminderEvidence::CanonicalOneShot));
        }
    }
    if source.contains(" (") {
        return None;
    }
    Some((source, ReminderEvidence::Absent))
}

fn parse_target_filter(source: &str) -> Option<RegenerationTargetFilter> {
    let (another, source) = if let Some(source) = source.strip_prefix("another target ") {
        (true, source)
    } else {
        (false, source.strip_prefix("target ")?)
    };
    let (controlled, source) = if let Some(source) = source.strip_suffix(" you control") {
        (true, source)
    } else {
        (false, source)
    };

    let mut filter = if source == "creature" {
        RegenerationTargetFilter::battlefield_creature()
    } else if source == "permanent" {
        RegenerationTargetFilter::battlefield_permanent()
    } else if source == "artifact" {
        let mut filter = RegenerationTargetFilter::battlefield_permanent();
        filter.required_card_types.insert(CardType::Artifact);
        filter
    } else if source == "artifact creature" {
        let mut filter = RegenerationTargetFilter::battlefield_creature();
        filter.required_card_types.insert(CardType::Artifact);
        filter
    } else if let Some(amount) = source
        .strip_prefix("creature with power ")
        .and_then(|source| source.strip_suffix(" or greater"))
        .and_then(|amount| amount.parse::<i32>().ok())
        .filter(|amount| *amount >= 0)
    {
        let mut filter = RegenerationTargetFilter::battlefield_creature();
        filter.minimum_power = Some(amount);
        filter
    } else if let Some(counter) = source
        .strip_prefix("creature with a ")
        .and_then(|source| source.strip_suffix(" counter on it"))
        .filter(|counter| is_counter_name(counter))
    {
        let mut filter = RegenerationTargetFilter::battlefield_creature();
        filter.required_counter = Some(counter.to_owned());
        filter
    } else if let Some(colors) = source.strip_suffix(" creature") {
        let parsed = parse_color_disjunction(colors)?;
        let mut filter = RegenerationTargetFilter::battlefield_creature();
        filter.any_colors = parsed;
        filter
    } else {
        let subtypes = parse_subtype_disjunction(source)?;
        let mut filter = RegenerationTargetFilter::battlefield_permanent();
        filter.any_subtypes = subtypes;
        filter
    };
    filter.controller = if controlled {
        RequiredController::EffectController
    } else {
        RequiredController::Any
    };
    filter.excludes_source = another;
    Some(filter)
}

fn parse_color_disjunction(source: &str) -> Option<BTreeSet<ManaColor>> {
    let mut colors = BTreeSet::new();
    for token in source.replace(" or ", ", ").split(", ") {
        let color = match token {
            "white" => ManaColor::White,
            "blue" => ManaColor::Blue,
            "black" => ManaColor::Black,
            "red" => ManaColor::Red,
            "green" => ManaColor::Green,
            _ => return None,
        };
        colors.insert(color);
    }
    (!colors.is_empty()).then_some(colors)
}

fn parse_subtype_disjunction(source: &str) -> Option<BTreeSet<String>> {
    let replaced = source.replace(", or ", ", ").replace(" or ", ", ");
    let subtypes = replaced
        .split(", ")
        .map(normalize_subtype)
        .collect::<Option<BTreeSet<_>>>()?;
    (!subtypes.is_empty()).then_some(subtypes)
}

fn normalize_subtype(source: &str) -> Option<String> {
    if source.is_empty()
        || !source.chars().next()?.is_uppercase()
        || !source
            .chars()
            .all(|character| character.is_alphabetic() || character == '-')
    {
        return None;
    }
    Some(source.to_ascii_lowercase())
}

fn is_counter_name(source: &str) -> bool {
    !source.is_empty()
        && source
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "+-/' ".contains(character))
}

fn parse_activation_costs(source: &str) -> Option<Vec<RegenerationActivationCost>> {
    let mut costs = Vec::new();
    for component in source.split(", ") {
        let cost = if component == "{T}" {
            RegenerationActivationCost::TapSource
        } else if component.starts_with('{') && is_exact_mana_cost(component) {
            RegenerationActivationCost::Mana(parse_mana_cost(component)?)
        } else if component == "Discard a card" {
            RegenerationActivationCost::DiscardCards {
                count: 1,
                random: false,
            }
        } else if component == "Discard a card at random" {
            RegenerationActivationCost::DiscardCards {
                count: 1,
                random: true,
            }
        } else if let Some(amount) = component
            .strip_prefix("Pay ")
            .and_then(|source| source.strip_suffix(" life"))
            .and_then(parse_positive_u32)
        {
            RegenerationActivationCost::PayLife(amount)
        } else if component == "Return this enchantment to its owner's hand" {
            RegenerationActivationCost::ReturnSourceToOwnersHand
        } else if let Some(cost) = parse_remove_counter_cost(component) {
            cost
        } else if let Some(cost) = parse_exile_graveyard_cost(component) {
            cost
        } else {
            parse_sacrifice_cost(component)?
        };
        costs.push(cost);
    }
    (!costs.is_empty()).then_some(costs)
}

fn is_exact_mana_cost(source: &str) -> bool {
    let mut offset = 0usize;
    let bytes = source.as_bytes();
    while offset < bytes.len() {
        if bytes[offset] != b'{' {
            return false;
        }
        let Some(relative_end) = source[offset + 1..].find('}') else {
            return false;
        };
        let end = offset + 1 + relative_end;
        if end == offset + 1 {
            return false;
        }
        offset = end + 1;
    }
    true
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    let mut symbols = Vec::new();
    let mut offset = 0usize;
    while offset < source.len() {
        let start = offset + 1;
        let end = start + source[start..].find('}')?;
        let symbol = &source[start..end];
        symbols.push(parse_mana_symbol(symbol)?);
        offset = end + 1;
    }
    Some(ManaCost {
        raw: source.to_owned(),
        symbols,
    })
}

fn parse_mana_symbol(source: &str) -> Option<ManaSymbol> {
    if let Ok(amount) = source.parse::<u32>() {
        return Some(ManaSymbol::Generic(amount));
    }
    let color = |source| match source {
        "W" => Some(ManaColor::White),
        "U" => Some(ManaColor::Blue),
        "B" => Some(ManaColor::Black),
        "R" => Some(ManaColor::Red),
        "G" => Some(ManaColor::Green),
        _ => None,
    };
    match source {
        "C" => Some(ManaSymbol::Colorless),
        "S" => Some(ManaSymbol::Snow),
        "X" => Some(ManaSymbol::VariableX),
        _ => {
            let parts = source.split('/').collect::<Vec<_>>();
            match parts.as_slice() {
                [single] => color(single).map(ManaSymbol::Colored),
                [first, second] => Some(ManaSymbol::Hybrid(color(first)?, color(second)?)),
                _ => None,
            }
        }
    }
}

fn parse_remove_counter_cost(source: &str) -> Option<RegenerationActivationCost> {
    let source = source
        .strip_prefix("Remove ")
        .and_then(|source| source.strip_suffix(" from this creature"))?;
    let (count, source) = parse_leading_count(source)?;
    let counter = source
        .strip_suffix(" counter")
        .or_else(|| source.strip_suffix(" counters"))?;
    is_counter_name(counter).then(|| RegenerationActivationCost::RemoveCountersFromSource {
        count,
        counter: counter.to_owned(),
    })
}

fn parse_exile_graveyard_cost(source: &str) -> Option<RegenerationActivationCost> {
    if source == "Exile a creature card from your graveyard" {
        return Some(
            RegenerationActivationCost::ExileCreatureCardsFromOwnGraveyard {
                count: 1,
                top_only: false,
            },
        );
    }
    if source == "Exile two creature cards from your graveyard" {
        return Some(
            RegenerationActivationCost::ExileCreatureCardsFromOwnGraveyard {
                count: 2,
                top_only: false,
            },
        );
    }
    if source == "Exile the top creature card of your graveyard" {
        return Some(
            RegenerationActivationCost::ExileCreatureCardsFromOwnGraveyard {
                count: 1,
                top_only: true,
            },
        );
    }
    None
}

fn parse_sacrifice_cost(source: &str) -> Option<RegenerationActivationCost> {
    let source = source.strip_prefix("Sacrifice ")?;
    let (relation, filter) = if let Some(object) = source.strip_prefix("this ") {
        (SacrificeRelation::Source, parse_cost_object_filter(object)?)
    } else if let Some(object) = source.strip_prefix("another ") {
        (
            SacrificeRelation::Another,
            parse_cost_object_filter(object)?,
        )
    } else {
        let source = source
            .strip_prefix("a ")
            .or_else(|| source.strip_prefix("an "))
            .unwrap_or(source);
        (SacrificeRelation::Chosen, parse_cost_object_filter(source)?)
    };
    Some(RegenerationActivationCost::Sacrifice {
        count: 1,
        relation,
        filter,
    })
}

fn parse_cost_object_filter(source: &str) -> Option<CostObjectFilter> {
    let mut required_card_types = BTreeSet::new();
    let required_subtype = match source {
        "permanent" => None,
        "creature" => {
            required_card_types.insert(CardType::Creature);
            None
        }
        "artifact" => {
            required_card_types.insert(CardType::Artifact);
            None
        }
        "enchantment" => {
            required_card_types.insert(CardType::Enchantment);
            None
        }
        "Aura" => {
            required_card_types.insert(CardType::Enchantment);
            Some("aura".to_owned())
        }
        "Forest" => {
            required_card_types.insert(CardType::Land);
            Some("forest".to_owned())
        }
        subtype => {
            required_card_types.insert(CardType::Creature);
            Some(normalize_subtype(subtype)?)
        }
    };
    Some(CostObjectFilter {
        required_card_types,
        required_subtype,
    })
}

fn parse_leading_count(source: &str) -> Option<(u32, &str)> {
    for (word, count) in [
        ("a ", 1),
        ("one ", 1),
        ("two ", 2),
        ("three ", 3),
        ("four ", 4),
        ("five ", 5),
    ] {
        if let Some(rest) = source.strip_prefix(word) {
            return Some((count, rest));
        }
    }
    None
}

fn parse_positive_u32(source: &str) -> Option<u32> {
    source.parse::<u32>().ok().filter(|amount| *amount > 0)
}

fn regeneration_semantic_digest(
    exact_source: &str,
    normalized_source: &str,
    kind: &RegenerationActionKind,
) -> String {
    let mut digest = Sha256::new();
    for component in [
        "regeneration-action-program/v1",
        REGENERATION_ACTION_COMPILER_VERSION,
        REGENERATION_ACTION_RUNTIME_VERSION,
        REGENERATION_ACTION_RULES_CONTEXT_VERSION,
        exact_source,
        normalized_source,
        &kind.stable_id(),
    ] {
        digest.update((component.len() as u64).to_le_bytes());
        digest.update(component.as_bytes());
    }
    hex_digest(digest.finalize().as_slice())
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn is_complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source.trim() == source
        && !source.contains(['\r', '\n', '\0'])
        && collapse_whitespace(source) == source
}

fn collapse_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn card_type_stable_id(card_type: &CardType) -> &'static str {
    match card_type {
        CardType::Artifact => "artifact",
        CardType::Battle => "battle",
        CardType::Creature => "creature",
        CardType::Enchantment => "enchantment",
        CardType::Instant => "instant",
        CardType::Kindred => "kindred",
        CardType::Land => "land",
        CardType::Planeswalker => "planeswalker",
        CardType::Sorcery => "sorcery",
    }
}

fn mana_color_stable_id(color: &ManaColor) -> &'static str {
    match color {
        ManaColor::White => "white",
        ManaColor::Blue => "blue",
        ManaColor::Black => "black",
        ManaColor::Red => "red",
        ManaColor::Green => "green",
        ManaColor::Colorless => "colorless",
    }
}

fn mana_symbol_stable_id(symbol: &ManaSymbol) -> String {
    match symbol {
        ManaSymbol::Generic(amount) => format!("generic/{amount}"),
        ManaSymbol::Colored(color) => format!("colored/{}", mana_color_stable_id(color)),
        ManaSymbol::Colorless => "colorless".to_owned(),
        ManaSymbol::Snow => "snow".to_owned(),
        ManaSymbol::Hybrid(first, second) => format!(
            "hybrid/{}/{}",
            mana_color_stable_id(first),
            mana_color_stable_id(second)
        ),
        ManaSymbol::Phyrexian(color) => {
            format!("phyrexian/{}", mana_color_stable_id(color))
        }
        ManaSymbol::VariableX => "x".to_owned(),
    }
}

fn selection_time_stable_id(time: RecipientSelectionTime) -> &'static str {
    match time {
        RecipientSelectionTime::WhenAbilityIsPutOnStack => "stack",
        RecipientSelectionTime::OnResolution => "resolution",
        RecipientSelectionTime::StaticReplacementEvent => "replacement-event",
    }
}

fn cardinality_stable_id(cardinality: RecipientCardinality) -> &'static str {
    match cardinality {
        RecipientCardinality::ExactlyOne => "exactly-one",
        RecipientCardinality::ZeroOrMore => "zero-or-more",
    }
}

fn reminder_stable_id(reminder: ReminderEvidence) -> &'static str {
    match reminder {
        ReminderEvidence::Absent => "absent",
        ReminderEvidence::CanonicalOneShot => "canonical-one-shot",
        ReminderEvidence::CanonicalStatic => "canonical-static",
    }
}

fn trigger_stable_id(trigger: RegenerationTrigger) -> &'static str {
    match trigger {
        RegenerationTrigger::SourceTurnedFaceUp => "source-turned-face-up",
        RegenerationTrigger::SourceBecameBlocked => "source-became-blocked",
        RegenerationTrigger::ControllerCastSpiritOrArcaneSpell => {
            "controller-cast-spirit-or-arcane"
        }
    }
}

fn replacement_stable_id(replacement: RegenerationReplacement) -> &'static str {
    match replacement {
        RegenerationReplacement::NextDestructionThisTurn => "next-destruction-this-turn",
        RegenerationReplacement::EveryDestructionWhileStaticEffectApplies => {
            "every-destruction-while-static-applies"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegenerationStackActionId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RegenerationActivationPayment {
    pub mana_payment: ManaPayment,
    pub discarded_cards: Vec<ObjectReference>,
    pub sacrificed_permanents: Vec<ObjectReference>,
    pub exiled_graveyard_cards: Vec<ObjectReference>,
    pub random_discard_was_uniform: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegenerationTriggerEvent {
    SourceTurnedFaceUp {
        source: ObjectReference,
    },
    SourceBecameBlocked {
        source: ObjectReference,
    },
    SpellCast {
        player: PlayerId,
        spell: ObjectReference,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttachmentRelationship {
    Enchanted,
    Equipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingRecipient {
    Source(ObjectReference),
    Target {
        target: ObjectReference,
        filter: RegenerationTargetFilter,
    },
    Attachment {
        source: ObjectReference,
        relationship: AttachmentRelationship,
        last_known_recipient: Option<ObjectReference>,
    },
    ControlledSet(RegenerationTargetFilter),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRegenerationAction {
    pub id: RegenerationStackActionId,
    pub semantic_digest: String,
    pub controller: PlayerId,
    pub source: ObjectReference,
    targeting_source: SourceProfile,
    recipient: PendingRecipient,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegenerationActivationReceipt {
    pub action: PendingRegenerationAction,
    pub mana_spent: Vec<ManaUnitId>,
    pub life_paid: u32,
    pub discarded_cards: Vec<ObjectReference>,
    pub sacrificed_permanents: Vec<ObjectReference>,
    pub exiled_graveyard_cards: Vec<ObjectReference>,
    pub source_tapped: bool,
    pub counters_removed: BTreeMap<String, u32>,
    pub source_returned_to_hand: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegenerationResolutionOutcome {
    ReplacementCreated,
    ReplacementCreatedButCannotApplyWhileProhibited,
    RecipientNoLongerLegal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegenerationRecipientResolution {
    pub recipient: ObjectReference,
    pub outcome: RegenerationResolutionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegenerationResolutionReceipt {
    pub action_id: RegenerationStackActionId,
    pub semantic_digest: String,
    pub recipients: Vec<RegenerationRecipientResolution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegenerationDestructionOutcome {
    Regenerated {
        permanent: ObjectReference,
        removed_damage: u32,
        was_removed_from_combat: bool,
    },
    Destroyed {
        previous: ObjectReference,
        new_incarnation: ObjectReference,
        regeneration_was_prohibited: bool,
    },
    NotDestroyedIndestructible {
        permanent: ObjectReference,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegenerationRuntimeError {
    WrongProgramKind,
    ProgramVersionMismatch,
    MissingObject(ObjectId),
    StaleIncarnation(ObjectReference),
    ObjectNotOnBattlefield(ObjectReference),
    WrongController,
    MissingTarget,
    UnexpectedTarget,
    IllegalTarget(ObjectReference),
    MissingAttachment,
    ActivationRestrictionNotMet,
    MissingManaPayment {
        symbol_index: usize,
    },
    UnexpectedManaPayment {
        symbol_index: usize,
    },
    InvalidManaPayment {
        symbol_index: usize,
    },
    MissingManaUnit(ManaUnitId),
    DuplicateManaUnit(ManaUnitId),
    MissingDiscardPayment,
    InvalidDiscardPayment(ObjectReference),
    RandomDiscardEvidenceMissing,
    MissingSacrificePayment,
    InvalidSacrificePayment(ObjectReference),
    MissingExilePayment,
    InvalidExilePayment(ObjectReference),
    InsufficientLife,
    SourceAlreadyTapped,
    SourceCannotPayTapCost,
    InvalidSourceCost(ObjectReference),
    MissingSourceCounter {
        counter: String,
        required: u32,
        available: u32,
    },
    MissingPendingAction(RegenerationStackActionId),
    IncarnationOverflow(ObjectId),
    InconsistentZoneMembership {
        object: ObjectId,
        zone: Zone,
    },
    CoreKeyword(String),
}

impl fmt::Display for RegenerationRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RegenerationRuntimeError {}

impl From<KeywordExecutionError> for RegenerationRuntimeError {
    fn from(error: KeywordExecutionError) -> Self {
        Self::CoreKeyword(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RegenerationRuntimeState {
    pub game: KeywordGameState,
    incarnations: BTreeMap<ObjectId, IncarnationId>,
    cannot_regenerate_until_end_of_turn: BTreeSet<ObjectReference>,
    blocked_by_colors_this_turn: BTreeMap<ObjectReference, BTreeSet<ManaColor>>,
    pending: BTreeMap<RegenerationStackActionId, PendingRegenerationAction>,
    next_action_id: u64,
}

impl RegenerationRuntimeState {
    pub fn register_incarnation(
        &mut self,
        object: ObjectId,
        incarnation: IncarnationId,
    ) -> Result<ObjectReference, RegenerationRuntimeError> {
        let core = self
            .game
            .objects
            .get_mut(&object)
            .ok_or(RegenerationRuntimeError::MissingObject(object))?;
        if self.incarnations.get(&object).copied() != Some(incarnation) {
            core.regeneration_shields = 0;
            core.static_regeneration = false;
        }
        self.incarnations.insert(object, incarnation);
        Ok(ObjectReference {
            object,
            incarnation,
        })
    }

    pub fn current_reference(
        &self,
        object: ObjectId,
    ) -> Result<ObjectReference, RegenerationRuntimeError> {
        self.game
            .objects
            .get(&object)
            .ok_or(RegenerationRuntimeError::MissingObject(object))?;
        let incarnation = self.incarnations.get(&object).copied().ok_or(
            RegenerationRuntimeError::StaleIncarnation(ObjectReference {
                object,
                incarnation: IncarnationId(0),
            }),
        )?;
        Ok(ObjectReference {
            object,
            incarnation,
        })
    }

    pub fn pending(&self, id: RegenerationStackActionId) -> Option<&PendingRegenerationAction> {
        self.pending.get(&id)
    }

    pub fn note_source_blocked_by_color_this_turn(
        &mut self,
        source: ObjectReference,
        color: ManaColor,
    ) -> Result<(), RegenerationRuntimeError> {
        self.validate_current(source)?;
        self.blocked_by_colors_this_turn
            .entry(source)
            .or_default()
            .insert(color);
        Ok(())
    }

    pub fn prohibit_regeneration_until_end_of_turn(
        &mut self,
        permanent: ObjectReference,
    ) -> Result<(), RegenerationRuntimeError> {
        self.validate_battlefield(permanent)?;
        self.cannot_regenerate_until_end_of_turn.insert(permanent);
        Ok(())
    }

    pub fn regeneration_is_prohibited(&self, permanent: ObjectReference) -> bool {
        self.cannot_regenerate_until_end_of_turn
            .contains(&permanent)
    }

    pub fn end_turn_cleanup(&mut self) {
        clear_end_of_turn_regeneration(&mut self.game);
        self.cannot_regenerate_until_end_of_turn.clear();
        self.blocked_by_colors_this_turn.clear();
    }

    fn validate_current(&self, reference: ObjectReference) -> Result<(), RegenerationRuntimeError> {
        if !self.game.objects.contains_key(&reference.object) {
            return Err(RegenerationRuntimeError::MissingObject(reference.object));
        }
        if self.incarnations.get(&reference.object).copied() != Some(reference.incarnation) {
            return Err(RegenerationRuntimeError::StaleIncarnation(reference));
        }
        Ok(())
    }

    fn validate_battlefield(
        &self,
        reference: ObjectReference,
    ) -> Result<(), RegenerationRuntimeError> {
        self.validate_current(reference)?;
        if self.game.objects[&reference.object].zone != Zone::Battlefield {
            return Err(RegenerationRuntimeError::ObjectNotOnBattlefield(reference));
        }
        Ok(())
    }

    fn next_action_id(&mut self) -> Result<RegenerationStackActionId, RegenerationRuntimeError> {
        let id = RegenerationStackActionId(self.next_action_id);
        self.next_action_id = self.next_action_id.checked_add(1).ok_or(
            RegenerationRuntimeError::IncarnationOverflow(ObjectId(u64::MAX)),
        )?;
        Ok(id)
    }

    fn bump_incarnation(
        &mut self,
        object: ObjectId,
    ) -> Result<ObjectReference, RegenerationRuntimeError> {
        let current = self
            .incarnations
            .get(&object)
            .copied()
            .ok_or(RegenerationRuntimeError::MissingObject(object))?;
        let next = IncarnationId(
            current
                .0
                .checked_add(1)
                .ok_or(RegenerationRuntimeError::IncarnationOverflow(object))?,
        );
        self.incarnations.insert(object, next);
        if let Some(core) = self.game.objects.get_mut(&object) {
            core.regeneration_shields = 0;
            core.static_regeneration = false;
            core.damage_marked = 0;
            core.damaged_by_deathtouch_since_state_check = false;
            core.attacking = false;
            core.blocking = false;
        }
        Ok(ObjectReference {
            object,
            incarnation: next,
        })
    }
}

pub fn begin_regeneration_activation(
    state: &mut RegenerationRuntimeState,
    program: &RegenerationActionProgram,
    controller: PlayerId,
    source: ObjectReference,
    target: Option<ObjectReference>,
    payment: RegenerationActivationPayment,
) -> Result<RegenerationActivationReceipt, RegenerationRuntimeError> {
    let RegenerationActionKind::Activated(activated) = program.kind() else {
        return Err(RegenerationRuntimeError::WrongProgramKind);
    };
    verify_program_version(program)?;
    let before = state.clone();
    let result = (|| {
        state.validate_battlefield(source)?;
        if state.game.objects[&source.object].controller != controller {
            return Err(RegenerationRuntimeError::WrongController);
        }
        let targeting_source = SourceProfile::from_object(&state.game.objects[&source.object]);
        validate_activation_restriction(state, activated.restriction, controller, source)?;
        let pending_recipient = bind_recipient_for_stack(
            state,
            &activated.recipient,
            controller,
            source,
            &targeting_source,
            target,
        )?;
        let paid = pay_activation_costs(state, controller, source, &activated.costs, &payment)?;
        let id = state.next_action_id()?;
        let action = PendingRegenerationAction {
            id,
            semantic_digest: program.semantic_digest().to_owned(),
            controller,
            source,
            targeting_source,
            recipient: pending_recipient,
        };
        state.pending.insert(id, action.clone());
        Ok(RegenerationActivationReceipt {
            action,
            mana_spent: paid.mana_spent,
            life_paid: paid.life_paid,
            discarded_cards: paid.discarded_cards,
            sacrificed_permanents: paid.sacrificed_permanents,
            exiled_graveyard_cards: paid.exiled_graveyard_cards,
            source_tapped: paid.source_tapped,
            counters_removed: paid.counters_removed,
            source_returned_to_hand: paid.source_returned_to_hand,
        })
    })();
    if result.is_err() {
        *state = before;
    }
    result
}

pub fn begin_regeneration_trigger(
    state: &mut RegenerationRuntimeState,
    program: &RegenerationActionProgram,
    controller: PlayerId,
    source: ObjectReference,
    event: RegenerationTriggerEvent,
    target: Option<ObjectReference>,
) -> Result<PendingRegenerationAction, RegenerationRuntimeError> {
    let RegenerationActionKind::Triggered(triggered) = program.kind() else {
        return Err(RegenerationRuntimeError::WrongProgramKind);
    };
    verify_program_version(program)?;
    state.validate_battlefield(source)?;
    if !state.game.objects[&source.object]
        .effective_characteristics()
        .card_types
        .contains(&CardType::Creature)
    {
        return Err(RegenerationRuntimeError::IllegalTarget(source));
    }
    if state.game.objects[&source.object].controller != controller {
        return Err(RegenerationRuntimeError::WrongController);
    }
    let targeting_source = SourceProfile::from_object(&state.game.objects[&source.object]);
    validate_trigger_event(state, triggered.trigger, controller, source, &event)?;
    let recipient = bind_recipient_for_stack(
        state,
        &triggered.recipient,
        controller,
        source,
        &targeting_source,
        target,
    )?;
    let id = state.next_action_id()?;
    let action = PendingRegenerationAction {
        id,
        semantic_digest: program.semantic_digest().to_owned(),
        controller,
        source,
        targeting_source,
        recipient,
    };
    state.pending.insert(id, action.clone());
    Ok(action)
}

pub fn resolve_pending_regeneration(
    state: &mut RegenerationRuntimeState,
    id: RegenerationStackActionId,
) -> Result<RegenerationResolutionReceipt, RegenerationRuntimeError> {
    let before = state.clone();
    let result = (|| {
        let pending = state
            .pending
            .remove(&id)
            .ok_or(RegenerationRuntimeError::MissingPendingAction(id))?;
        let recipients = resolve_pending_recipients(state, &pending)?;
        let core_program = core_one_shot_regeneration_program()?;
        let mut resolutions = Vec::new();
        for recipient in recipients {
            let currently_prohibited = state.regeneration_is_prohibited(recipient);
            execute_keyword_action(
                &mut state.game,
                &core_program,
                KeywordAction::CreateRegenerationReplacement {
                    permanent: recipient.object,
                },
            )?;
            resolutions.push(RegenerationRecipientResolution {
                recipient,
                outcome: if currently_prohibited {
                    RegenerationResolutionOutcome::ReplacementCreatedButCannotApplyWhileProhibited
                } else {
                    RegenerationResolutionOutcome::ReplacementCreated
                },
            });
        }
        if resolutions.is_empty()
            && let PendingRecipient::Target { target, .. } = pending.recipient
        {
            resolutions.push(RegenerationRecipientResolution {
                recipient: target,
                outcome: RegenerationResolutionOutcome::RecipientNoLongerLegal,
            });
        }
        Ok(RegenerationResolutionReceipt {
            action_id: id,
            semantic_digest: pending.semantic_digest,
            recipients: resolutions,
        })
    })();
    if result.is_err() {
        *state = before;
    }
    result
}

pub fn install_static_regeneration_replacement(
    state: &mut RegenerationRuntimeState,
    program: &RegenerationActionProgram,
    source: ObjectReference,
) -> Result<(), RegenerationRuntimeError> {
    let RegenerationActionKind::StaticDestructionReplacement(static_program) = program.kind()
    else {
        return Err(RegenerationRuntimeError::WrongProgramKind);
    };
    verify_program_version(program)?;
    state.validate_battlefield(source)?;
    if !state.game.objects[&source.object]
        .effective_characteristics()
        .card_types
        .contains(&CardType::Creature)
    {
        return Err(RegenerationRuntimeError::IllegalTarget(source));
    }
    if !matches!(
        static_program.recipient,
        RegenerationRecipient::SourcePermanent { .. }
    ) || static_program.replacement
        != RegenerationReplacement::EveryDestructionWhileStaticEffectApplies
    {
        return Err(RegenerationRuntimeError::WrongProgramKind);
    }
    let core = core_static_regeneration_program()?;
    execute_keyword_action(
        &mut state.game,
        &core,
        KeywordAction::CreateRegenerationReplacement {
            permanent: source.object,
        },
    )?;
    Ok(())
}

pub fn remove_static_regeneration_replacement(
    state: &mut RegenerationRuntimeState,
    source: ObjectReference,
) -> Result<(), RegenerationRuntimeError> {
    state.validate_current(source)?;
    remove_static_regeneration(&mut state.game, source.object)?;
    Ok(())
}

pub fn resolve_regeneration_destruction(
    state: &mut RegenerationRuntimeState,
    permanent: ObjectReference,
    replacement_choice: Option<RegenerationChoice>,
) -> Result<RegenerationDestructionOutcome, RegenerationRuntimeError> {
    state.validate_battlefield(permanent)?;
    if state.game.objects[&permanent.object]
        .combat_keywords
        .contains(&CombatKeyword::Indestructible)
    {
        return Ok(RegenerationDestructionOutcome::NotDestroyedIndestructible { permanent });
    }
    let removed_damage = state.game.objects[&permanent.object].damage_marked;
    let was_removed_from_combat = {
        let object = &state.game.objects[&permanent.object];
        object.attacking || object.blocking
    };
    if state.regeneration_is_prohibited(permanent) {
        move_object_to_zone(state, permanent, Zone::Graveyard)?;
        let new_incarnation = state.current_reference(permanent.object)?;
        return Ok(RegenerationDestructionOutcome::Destroyed {
            previous: permanent,
            new_incarnation,
            regeneration_was_prohibited: true,
        });
    }

    let core = core_one_shot_regeneration_program()?;
    let receipt =
        resolve_destruction(&mut state.game, &core, permanent.object, replacement_choice)?;
    let regenerated =
        receipt.events.iter().any(|event| {
            matches!(
            event,
            crate::keyword_rules_runtime::KeywordEvidenceEvent::RegenerationReplacedDestruction {
                ..
            }
        )
        });
    if regenerated {
        return Ok(RegenerationDestructionOutcome::Regenerated {
            permanent,
            removed_damage,
            was_removed_from_combat,
        });
    }
    let new_incarnation = state.bump_incarnation(permanent.object)?;
    Ok(RegenerationDestructionOutcome::Destroyed {
        previous: permanent,
        new_incarnation,
        regeneration_was_prohibited: false,
    })
}

fn verify_program_version(
    program: &RegenerationActionProgram,
) -> Result<(), RegenerationRuntimeError> {
    let expected = regeneration_semantic_digest(
        program.exact_source(),
        program.normalized_source(),
        program.kind(),
    );
    if expected != program.semantic_digest() {
        return Err(RegenerationRuntimeError::ProgramVersionMismatch);
    }
    Ok(())
}

fn core_one_shot_regeneration_program() -> Result<KeywordProgram, RegenerationRuntimeError> {
    compile_keyword_program(KeywordProgramInput {
        face_index: 0,
        clause_index: 0,
        printed_keyword: "Regenerate",
        oracle_fragment: Some("Regenerate"),
    })
    .map_err(|error| RegenerationRuntimeError::CoreKeyword(error.to_string()))
}

fn core_static_regeneration_program() -> Result<KeywordProgram, RegenerationRuntimeError> {
    compile_keyword_program(KeywordProgramInput {
        face_index: 0,
        clause_index: 0,
        printed_keyword: "Regenerate",
        oracle_fragment: Some("If this permanent would be destroyed, regenerate it instead"),
    })
    .map_err(|error| RegenerationRuntimeError::CoreKeyword(error.to_string()))
}

fn bind_recipient_for_stack(
    state: &RegenerationRuntimeState,
    recipient: &RegenerationRecipient,
    controller: PlayerId,
    source: ObjectReference,
    targeting_source: &SourceProfile,
    target: Option<ObjectReference>,
) -> Result<PendingRecipient, RegenerationRuntimeError> {
    match recipient {
        RegenerationRecipient::SourcePermanent { .. } => {
            if target.is_some() {
                return Err(RegenerationRuntimeError::UnexpectedTarget);
            }
            Ok(PendingRecipient::Source(source))
        }
        RegenerationRecipient::Target { filter, .. } => {
            let target = target.ok_or(RegenerationRuntimeError::MissingTarget)?;
            validate_target(
                state,
                target,
                filter,
                controller,
                source,
                targeting_source,
                true,
            )?;
            Ok(PendingRecipient::Target {
                target,
                filter: filter.clone(),
            })
        }
        RegenerationRecipient::EnchantedCreature { .. } => {
            if target.is_some() {
                return Err(RegenerationRuntimeError::UnexpectedTarget);
            }
            validate_attachment_source(state, source, CardType::Enchantment, "Aura")?;
            let last_known_recipient = attached_object_reference(state, source)?;
            if let Some(recipient) = last_known_recipient {
                let filter = RegenerationTargetFilter::battlefield_creature();
                validate_target(
                    state,
                    recipient,
                    &filter,
                    controller,
                    source,
                    targeting_source,
                    false,
                )?;
            }
            Ok(PendingRecipient::Attachment {
                source,
                relationship: AttachmentRelationship::Enchanted,
                last_known_recipient,
            })
        }
        RegenerationRecipient::EquippedCreature { .. } => {
            if target.is_some() {
                return Err(RegenerationRuntimeError::UnexpectedTarget);
            }
            validate_attachment_source(state, source, CardType::Artifact, "Equipment")?;
            let last_known_recipient = attached_object_reference(state, source)?;
            if let Some(recipient) = last_known_recipient {
                let filter = RegenerationTargetFilter::battlefield_creature();
                validate_target(
                    state,
                    recipient,
                    &filter,
                    controller,
                    source,
                    targeting_source,
                    false,
                )?;
            }
            Ok(PendingRecipient::Attachment {
                source,
                relationship: AttachmentRelationship::Equipped,
                last_known_recipient,
            })
        }
        RegenerationRecipient::ControlledSet { filter, .. } => {
            if target.is_some() {
                return Err(RegenerationRuntimeError::UnexpectedTarget);
            }
            Ok(PendingRecipient::ControlledSet(filter.clone()))
        }
    }
}

fn validate_attachment_source(
    state: &RegenerationRuntimeState,
    source: ObjectReference,
    required_type: CardType,
    required_subtype: &str,
) -> Result<(), RegenerationRuntimeError> {
    state.validate_battlefield(source)?;
    let characteristics = state.game.objects[&source.object].effective_characteristics();
    if !characteristics.card_types.contains(&required_type)
        || !characteristics
            .subtypes
            .iter()
            .any(|subtype| subtype.eq_ignore_ascii_case(required_subtype))
    {
        return Err(RegenerationRuntimeError::InvalidSourceCost(source));
    }
    Ok(())
}

fn attached_object_reference(
    state: &RegenerationRuntimeState,
    source: ObjectReference,
) -> Result<Option<ObjectReference>, RegenerationRuntimeError> {
    state.validate_battlefield(source)?;
    match state.game.objects[&source.object].attached_to {
        Some(ProtectionTarget::Object(object)) => state.current_reference(object).map(Some),
        Some(ProtectionTarget::Player(_)) | None => Ok(None),
    }
}

fn resolve_pending_recipients(
    state: &RegenerationRuntimeState,
    pending: &PendingRegenerationAction,
) -> Result<Vec<ObjectReference>, RegenerationRuntimeError> {
    match &pending.recipient {
        PendingRecipient::Source(source) => {
            if state.validate_battlefield(*source).is_ok() {
                Ok(vec![*source])
            } else {
                Ok(Vec::new())
            }
        }
        PendingRecipient::Target { target, filter } => {
            if validate_target(
                state,
                *target,
                filter,
                pending.controller,
                pending.source,
                &pending.targeting_source,
                true,
            )
            .is_ok()
            {
                Ok(vec![*target])
            } else {
                Ok(Vec::new())
            }
        }
        PendingRecipient::Attachment {
            source,
            relationship: _,
            last_known_recipient,
        } => {
            let current = attached_object_reference(state, *source)
                .ok()
                .flatten()
                .or(*last_known_recipient);
            let Some(current) = current else {
                return Ok(Vec::new());
            };
            let filter = RegenerationTargetFilter::battlefield_creature();
            if validate_target(
                state,
                current,
                &filter,
                pending.controller,
                pending.source,
                &pending.targeting_source,
                false,
            )
            .is_ok()
            {
                Ok(vec![current])
            } else {
                Ok(Vec::new())
            }
        }
        PendingRecipient::ControlledSet(filter) => {
            let mut recipients = Vec::new();
            for object in state.game.objects.keys().copied() {
                let reference = state.current_reference(object)?;
                if validate_target(
                    state,
                    reference,
                    filter,
                    pending.controller,
                    pending.source,
                    &pending.targeting_source,
                    false,
                )
                .is_ok()
                {
                    recipients.push(reference);
                }
            }
            Ok(recipients)
        }
    }
}

fn validate_target(
    state: &RegenerationRuntimeState,
    target: ObjectReference,
    filter: &RegenerationTargetFilter,
    controller: PlayerId,
    source: ObjectReference,
    targeting_source: &SourceProfile,
    is_targeted: bool,
) -> Result<(), RegenerationRuntimeError> {
    state.validate_battlefield(target)?;
    if filter.excludes_source && target.object == source.object {
        return Err(RegenerationRuntimeError::IllegalTarget(target));
    }
    let object = &state.game.objects[&target.object];
    if filter.controller == RequiredController::EffectController && object.controller != controller
    {
        return Err(RegenerationRuntimeError::IllegalTarget(target));
    }
    let characteristics = object.effective_characteristics();
    let is_permanent = characteristics.card_types.iter().any(|card_type| {
        matches!(
            card_type,
            CardType::Artifact
                | CardType::Battle
                | CardType::Creature
                | CardType::Enchantment
                | CardType::Land
                | CardType::Planeswalker
        )
    });
    if !is_permanent
        || !filter
            .required_card_types
            .iter()
            .all(|card_type| characteristics.card_types.contains(card_type))
        || (!filter.any_colors.is_empty() && characteristics.colors.is_disjoint(&filter.any_colors))
        || (!filter.any_subtypes.is_empty()
            && !characteristics
                .subtypes
                .iter()
                .any(|subtype| filter.any_subtypes.contains(&subtype.to_ascii_lowercase())))
        || filter
            .minimum_power
            .is_some_and(|minimum| characteristics.power.unwrap_or(i32::MIN) < minimum)
        || filter
            .required_counter
            .as_ref()
            .is_some_and(|counter| object.counters.get(counter).copied().unwrap_or(0) == 0)
    {
        return Err(RegenerationRuntimeError::IllegalTarget(target));
    }
    if is_targeted
        && !targeting_is_legal(
            &state.game,
            ProtectionTarget::Object(target.object),
            targeting_source,
        )?
    {
        return Err(RegenerationRuntimeError::IllegalTarget(target));
    }
    Ok(())
}

fn validate_activation_restriction(
    state: &RegenerationRuntimeState,
    restriction: ActivationRestriction,
    controller: PlayerId,
    source: ObjectReference,
) -> Result<(), RegenerationRuntimeError> {
    let met = match restriction {
        ActivationRestriction::None => true,
        ActivationRestriction::ControlAnotherColorlessCreature => {
            state.game.objects.values().any(|object| {
                object.id != source.object
                    && object.zone == Zone::Battlefield
                    && object.controller == controller
                    && {
                        let characteristics = object.effective_characteristics();
                        characteristics.card_types.contains(&CardType::Creature)
                            && characteristics.colors.is_empty()
                    }
            })
        }
        ActivationRestriction::SourceBlockedOrWasBlockedByBlueCreatureThisTurn => state
            .blocked_by_colors_this_turn
            .get(&source)
            .is_some_and(|colors| colors.contains(&ManaColor::Blue)),
        ActivationRestriction::ControlledCreaturesTotalPowerAtLeast(minimum) => {
            state
                .game
                .objects
                .values()
                .filter(|object| {
                    object.zone == Zone::Battlefield
                        && object.controller == controller
                        && object
                            .effective_characteristics()
                            .card_types
                            .contains(&CardType::Creature)
                })
                .map(|object| object.effective_characteristics().power.unwrap_or(0).max(0) as u32)
                .sum::<u32>()
                >= minimum
        }
        ActivationRestriction::OwnGraveyardHasAtLeast(minimum) => state
            .game
            .players
            .get(&controller)
            .is_some_and(|player| player.graveyard.len() >= minimum as usize),
    };
    if met {
        Ok(())
    } else {
        Err(RegenerationRuntimeError::ActivationRestrictionNotMet)
    }
}

fn validate_trigger_event(
    state: &RegenerationRuntimeState,
    trigger: RegenerationTrigger,
    controller: PlayerId,
    source: ObjectReference,
    event: &RegenerationTriggerEvent,
) -> Result<(), RegenerationRuntimeError> {
    let valid = match (trigger, event) {
        (
            RegenerationTrigger::SourceTurnedFaceUp,
            RegenerationTriggerEvent::SourceTurnedFaceUp {
                source: event_source,
            },
        )
        | (
            RegenerationTrigger::SourceBecameBlocked,
            RegenerationTriggerEvent::SourceBecameBlocked {
                source: event_source,
            },
        ) => *event_source == source,
        (
            RegenerationTrigger::ControllerCastSpiritOrArcaneSpell,
            RegenerationTriggerEvent::SpellCast { player, spell },
        ) if *player == controller && state.validate_current(*spell).is_ok() => {
            let object = &state.game.objects[&spell.object];
            let characteristics = object.effective_characteristics();
            object.zone == Zone::Stack
                && characteristics
                    .subtypes
                    .iter()
                    .any(|subtype| matches!(subtype.as_str(), "Spirit" | "Arcane"))
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(RegenerationRuntimeError::ActivationRestrictionNotMet)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct PaidActivationCosts {
    mana_spent: Vec<ManaUnitId>,
    life_paid: u32,
    discarded_cards: Vec<ObjectReference>,
    sacrificed_permanents: Vec<ObjectReference>,
    exiled_graveyard_cards: Vec<ObjectReference>,
    source_tapped: bool,
    counters_removed: BTreeMap<String, u32>,
    source_returned_to_hand: bool,
}

fn pay_activation_costs(
    state: &mut RegenerationRuntimeState,
    player: PlayerId,
    source: ObjectReference,
    costs: &[RegenerationActivationCost],
    payment: &RegenerationActivationPayment,
) -> Result<PaidActivationCosts, RegenerationRuntimeError> {
    let expected_discards = costs
        .iter()
        .filter_map(|cost| match cost {
            RegenerationActivationCost::DiscardCards { count, .. } => Some(*count),
            _ => None,
        })
        .sum::<usize>();
    let expected_chosen_sacrifices = costs
        .iter()
        .filter_map(|cost| match cost {
            RegenerationActivationCost::Sacrifice {
                count,
                relation: SacrificeRelation::Another | SacrificeRelation::Chosen,
                ..
            } => Some(*count),
            _ => None,
        })
        .sum::<usize>();
    let expected_exiles = costs
        .iter()
        .filter_map(|cost| match cost {
            RegenerationActivationCost::ExileCreatureCardsFromOwnGraveyard { count, .. } => {
                Some(*count)
            }
            _ => None,
        })
        .sum::<usize>();
    if payment.discarded_cards.len() != expected_discards {
        return Err(RegenerationRuntimeError::MissingDiscardPayment);
    }
    if payment.sacrificed_permanents.len() != expected_chosen_sacrifices {
        return Err(RegenerationRuntimeError::MissingSacrificePayment);
    }
    if payment.exiled_graveyard_cards.len() != expected_exiles {
        return Err(RegenerationRuntimeError::MissingExilePayment);
    }
    let random_required = costs.iter().any(|cost| {
        matches!(
            cost,
            RegenerationActivationCost::DiscardCards { random: true, .. }
        )
    });
    if random_required && !payment.random_discard_was_uniform {
        return Err(RegenerationRuntimeError::RandomDiscardEvidenceMissing);
    }

    let mut paid = PaidActivationCosts::default();
    let mut discard_offset = 0usize;
    let mut sacrifice_offset = 0usize;
    let mut exile_offset = 0usize;
    let mana_cost_count = costs
        .iter()
        .filter(|cost| matches!(cost, RegenerationActivationCost::Mana(_)))
        .count();
    if mana_cost_count == 0 && !payment.mana_payment.symbols.is_empty() {
        return Err(RegenerationRuntimeError::UnexpectedManaPayment {
            symbol_index: *payment.mana_payment.symbols.keys().next().unwrap_or(&0),
        });
    }
    for cost in costs {
        match cost {
            RegenerationActivationCost::Mana(cost) => {
                paid.mana_spent = pay_mana_cost(state, player, cost, &payment.mana_payment)?;
            }
            RegenerationActivationCost::TapSource => {
                state.validate_battlefield(source)?;
                let object = state
                    .game
                    .objects
                    .get_mut(&source.object)
                    .ok_or(RegenerationRuntimeError::MissingObject(source.object))?;
                if object.tapped {
                    return Err(RegenerationRuntimeError::SourceAlreadyTapped);
                }
                if object
                    .effective_characteristics()
                    .card_types
                    .contains(&CardType::Creature)
                    && !object.controlled_since_turn_began
                    && !object.combat_keywords.contains(&CombatKeyword::Haste)
                {
                    return Err(RegenerationRuntimeError::SourceCannotPayTapCost);
                }
                object.tapped = true;
                paid.source_tapped = true;
            }
            RegenerationActivationCost::DiscardCards { count, .. } => {
                let selected = &payment.discarded_cards[discard_offset..discard_offset + count];
                for card in selected {
                    state.validate_current(*card)?;
                    let object = &state.game.objects[&card.object];
                    if object.owner != player
                        || object.zone != Zone::Hand
                        || !state.game.players[&player].hand.contains(&card.object)
                    {
                        return Err(RegenerationRuntimeError::InvalidDiscardPayment(*card));
                    }
                }
                for card in selected {
                    move_object_to_zone(state, *card, Zone::Graveyard)?;
                    paid.discarded_cards.push(*card);
                }
                discard_offset += count;
            }
            RegenerationActivationCost::Sacrifice {
                count,
                relation,
                filter,
            } => {
                let selected = match relation {
                    SacrificeRelation::Source => vec![source],
                    SacrificeRelation::Another | SacrificeRelation::Chosen => {
                        let selected = payment.sacrificed_permanents
                            [sacrifice_offset..sacrifice_offset + count]
                            .to_vec();
                        sacrifice_offset += count;
                        selected
                    }
                };
                for permanent in &selected {
                    state.validate_battlefield(*permanent)?;
                    let object = &state.game.objects[&permanent.object];
                    if object.controller != player
                        || (*relation == SacrificeRelation::Another
                            && permanent.object == source.object)
                        || (*relation == SacrificeRelation::Source
                            && permanent.object != source.object)
                        || !cost_object_matches(object, filter)
                    {
                        return Err(RegenerationRuntimeError::InvalidSacrificePayment(
                            *permanent,
                        ));
                    }
                }
                for permanent in selected {
                    move_object_to_zone(state, permanent, Zone::Graveyard)?;
                    paid.sacrificed_permanents.push(permanent);
                }
            }
            RegenerationActivationCost::PayLife(amount) => {
                let player_state = state
                    .game
                    .players
                    .get_mut(&player)
                    .ok_or(RegenerationRuntimeError::WrongController)?;
                let amount_i32 = i32::try_from(*amount)
                    .map_err(|_| RegenerationRuntimeError::InsufficientLife)?;
                if player_state.life < amount_i32 {
                    return Err(RegenerationRuntimeError::InsufficientLife);
                }
                player_state.life -= amount_i32;
                paid.life_paid = paid.life_paid.saturating_add(*amount);
            }
            RegenerationActivationCost::RemoveCountersFromSource { count, counter } => {
                state.validate_battlefield(source)?;
                let object = state
                    .game
                    .objects
                    .get_mut(&source.object)
                    .ok_or(RegenerationRuntimeError::MissingObject(source.object))?;
                if !object
                    .effective_characteristics()
                    .card_types
                    .contains(&CardType::Creature)
                {
                    return Err(RegenerationRuntimeError::InvalidSourceCost(source));
                }
                let available = object.counters.get(counter).copied().unwrap_or(0);
                if available < *count {
                    return Err(RegenerationRuntimeError::MissingSourceCounter {
                        counter: counter.clone(),
                        required: *count,
                        available,
                    });
                }
                if available == *count {
                    object.counters.remove(counter);
                } else {
                    object.counters.insert(counter.clone(), available - count);
                }
                paid.counters_removed.insert(counter.clone(), *count);
            }
            RegenerationActivationCost::ExileCreatureCardsFromOwnGraveyard { count, top_only } => {
                let selected = &payment.exiled_graveyard_cards[exile_offset..exile_offset + count];
                if *top_only {
                    let top_creature = state.game.players[&player]
                        .graveyard
                        .iter()
                        .rev()
                        .find(|object| {
                            state.game.objects[object]
                                .effective_characteristics()
                                .card_types
                                .contains(&CardType::Creature)
                        })
                        .copied();
                    if selected.first().map(|reference| reference.object) != top_creature {
                        return Err(RegenerationRuntimeError::InvalidExilePayment(selected[0]));
                    }
                }
                for card in selected {
                    state.validate_current(*card)?;
                    let object = &state.game.objects[&card.object];
                    if object.owner != player
                        || object.zone != Zone::Graveyard
                        || !object
                            .effective_characteristics()
                            .card_types
                            .contains(&CardType::Creature)
                    {
                        return Err(RegenerationRuntimeError::InvalidExilePayment(*card));
                    }
                }
                for card in selected {
                    move_object_to_zone(state, *card, Zone::Exile)?;
                    paid.exiled_graveyard_cards.push(*card);
                }
                exile_offset += count;
            }
            RegenerationActivationCost::ReturnSourceToOwnersHand => {
                state.validate_battlefield(source)?;
                if !state.game.objects[&source.object]
                    .effective_characteristics()
                    .card_types
                    .contains(&CardType::Enchantment)
                {
                    return Err(RegenerationRuntimeError::InvalidSourceCost(source));
                }
                move_object_to_zone(state, source, Zone::Hand)?;
                paid.source_returned_to_hand = true;
            }
        }
    }
    Ok(paid)
}

fn pay_mana_cost(
    state: &mut RegenerationRuntimeState,
    player: PlayerId,
    cost: &ManaCost,
    payment: &ManaPayment,
) -> Result<Vec<ManaUnitId>, RegenerationRuntimeError> {
    if payment.x_value.is_some() {
        return Err(RegenerationRuntimeError::UnexpectedManaPayment {
            symbol_index: cost.symbols.len(),
        });
    }
    let player_state = state
        .game
        .players
        .get(&player)
        .ok_or(RegenerationRuntimeError::WrongController)?;
    let available = player_state
        .mana_pool
        .iter()
        .map(|unit| (unit.id, unit))
        .collect::<BTreeMap<_, _>>();
    let mut used = BTreeSet::new();
    for (symbol_index, symbol) in cost.symbols.iter().enumerate() {
        let required = match symbol {
            ManaSymbol::Generic(amount) => *amount as usize,
            ManaSymbol::VariableX => {
                return Err(RegenerationRuntimeError::InvalidManaPayment { symbol_index });
            }
            _ => 1,
        };
        if required == 0 {
            if payment.symbols.contains_key(&symbol_index) {
                return Err(RegenerationRuntimeError::UnexpectedManaPayment { symbol_index });
            }
            continue;
        }
        let supplied = payment
            .symbols
            .get(&symbol_index)
            .ok_or(RegenerationRuntimeError::MissingManaPayment { symbol_index })?;
        let SymbolPayment::Mana(ids) = supplied else {
            return Err(RegenerationRuntimeError::InvalidManaPayment { symbol_index });
        };
        if ids.len() != required {
            return Err(RegenerationRuntimeError::InvalidManaPayment { symbol_index });
        }
        for id in ids {
            if !used.insert(*id) {
                return Err(RegenerationRuntimeError::DuplicateManaUnit(*id));
            }
            let unit = available
                .get(id)
                .ok_or(RegenerationRuntimeError::MissingManaUnit(*id))?;
            let legal = match symbol {
                ManaSymbol::Generic(_) => true,
                ManaSymbol::Colored(color) => unit.color == *color,
                ManaSymbol::Colorless => unit.color == ManaColor::Colorless,
                ManaSymbol::Snow => unit.from_snow_source,
                ManaSymbol::Hybrid(first, second) => unit.color == *first || unit.color == *second,
                ManaSymbol::Phyrexian(_) | ManaSymbol::VariableX => false,
            };
            if !legal {
                return Err(RegenerationRuntimeError::InvalidManaPayment { symbol_index });
            }
        }
    }
    if let Some(symbol_index) = payment
        .symbols
        .keys()
        .find(|symbol_index| **symbol_index >= cost.symbols.len())
    {
        return Err(RegenerationRuntimeError::UnexpectedManaPayment {
            symbol_index: *symbol_index,
        });
    }
    let player_state = state
        .game
        .players
        .get_mut(&player)
        .ok_or(RegenerationRuntimeError::WrongController)?;
    player_state
        .mana_pool
        .retain(|unit| !used.contains(&unit.id));
    Ok(used.into_iter().collect())
}

fn cost_object_matches(
    object: &crate::keyword_rules_runtime::KeywordObject,
    filter: &CostObjectFilter,
) -> bool {
    let characteristics = object.effective_characteristics();
    filter
        .required_card_types
        .iter()
        .all(|card_type| characteristics.card_types.contains(card_type))
        && filter.required_subtype.as_ref().is_none_or(|required| {
            characteristics
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case(required))
        })
}

fn move_object_to_zone(
    state: &mut RegenerationRuntimeState,
    reference: ObjectReference,
    destination: Zone,
) -> Result<(), RegenerationRuntimeError> {
    state.validate_current(reference)?;
    let (owner, current) = {
        let object = state
            .game
            .objects
            .get(&reference.object)
            .ok_or(RegenerationRuntimeError::MissingObject(reference.object))?;
        (object.owner, object.zone)
    };
    let player = state
        .game
        .players
        .get_mut(&owner)
        .ok_or(RegenerationRuntimeError::WrongController)?;
    let removed = match current {
        Zone::Library => remove_object_from_deque(&mut player.library, reference.object),
        Zone::Hand => remove_object_from_vec(&mut player.hand, reference.object),
        Zone::Graveyard => remove_object_from_vec(&mut player.graveyard, reference.object),
        Zone::Exile => remove_object_from_vec(&mut player.exile, reference.object),
        Zone::Command => remove_object_from_vec(&mut player.command, reference.object),
        Zone::Battlefield | Zone::Stack => true,
    };
    if !removed {
        return Err(RegenerationRuntimeError::InconsistentZoneMembership {
            object: reference.object,
            zone: current,
        });
    }
    match destination {
        Zone::Library => player.library.push_front(reference.object),
        Zone::Hand => player.hand.push(reference.object),
        Zone::Graveyard => player.graveyard.push(reference.object),
        Zone::Exile => player.exile.push(reference.object),
        Zone::Command => player.command.push(reference.object),
        Zone::Battlefield | Zone::Stack => {}
    }
    state
        .game
        .objects
        .get_mut(&reference.object)
        .ok_or(RegenerationRuntimeError::MissingObject(reference.object))?
        .zone = destination;
    state.bump_incarnation(reference.object)?;
    Ok(())
}

fn remove_object_from_vec(objects: &mut Vec<ObjectId>, object: ObjectId) -> bool {
    let Some(index) = objects.iter().position(|candidate| *candidate == object) else {
        return false;
    };
    objects.remove(index);
    true
}

fn remove_object_from_deque(
    objects: &mut std::collections::VecDeque<ObjectId>,
    object: ObjectId,
) -> bool {
    let Some(index) = objects.iter().position(|candidate| *candidate == object) else {
        return false;
    };
    objects.remove(index);
    true
}
