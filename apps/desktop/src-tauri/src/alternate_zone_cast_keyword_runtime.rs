//! Content keyed alternate-zone cast and play programs.
//!
//! This module owns complete standalone Oracle clauses for Suspend, Madness,
//! Unearth, Escape, and Flashback forms that the ordinary official-keyword
//! compiler cannot represent. Recognition is deliberately separate from
//! production coverage. The transactions below retain exact object
//! incarnations, choices, payments, triggers, replacements, and duration
//! boundaries, but the main simulation has no adapter for them yet.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const ALTERNATE_ZONE_CAST_COMPILER_VERSION: &str = "alternate-zone-cast-keyword-compiler-0.1";
pub const ALTERNATE_ZONE_CAST_RUNTIME_VERSION: &str = "alternate-zone-cast-keyword-runtime-0.1";
pub const ALTERNATE_ZONE_CAST_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:118.9,400.7,601.2,603.7,614.1,702.34,702.35,702.62,702.84,702.138";

pub const fn alternate_zone_cast_production_adapter_connected() -> bool {
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
pub struct ManaUnitId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingAbilityId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingTriggerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TurnId(pub u64);

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
            Self::White => "w",
            Self::Blue => "u",
            Self::Black => "b",
            Self::Red => "r",
            Self::Green => "g",
            Self::Colorless => "c",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardType {
    Artifact,
    Battle,
    Creature,
    Enchantment,
    Instant,
    Kindred,
    Land,
    Planeswalker,
    Sorcery,
    Other(String),
}

impl CardType {
    fn stable_id(&self) -> String {
        match self {
            Self::Artifact => "artifact".into(),
            Self::Battle => "battle".into(),
            Self::Creature => "creature".into(),
            Self::Enchantment => "enchantment".into(),
            Self::Instant => "instant".into(),
            Self::Kindred => "kindred".into(),
            Self::Land => "land".into(),
            Self::Planeswalker => "planeswalker".into(),
            Self::Sorcery => "sorcery".into(),
            Self::Other(value) => format!("other/{}", canonical_word(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSemanticContext {
    pub is_land: bool,
    pub is_creature: bool,
    pub is_instant_or_sorcery: bool,
    pub is_permanent_card: bool,
}

impl SourceSemanticContext {
    pub fn from_type_line(type_line: &str) -> Self {
        let card_types = card_types_from_type_line(type_line);
        let is_land = card_types.contains(&CardType::Land);
        let is_creature = card_types.contains(&CardType::Creature);
        let is_instant_or_sorcery =
            card_types.contains(&CardType::Instant) || card_types.contains(&CardType::Sorcery);
        Self {
            is_land,
            is_creature,
            is_instant_or_sorcery,
            is_permanent_card: !is_instant_or_sorcery,
        }
    }

    fn stable_id(self) -> String {
        format!(
            "land={};creature={};instant-or-sorcery={};permanent={}",
            self.is_land, self.is_creature, self.is_instant_or_sorcery, self.is_permanent_card
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaSymbol {
    Generic(u32),
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
    Snow,
    Hybrid(ManaColor, ManaColor),
    VariableX,
}

impl ManaSymbol {
    fn stable_id(self) -> String {
        match self {
            Self::Generic(amount) => format!("generic/{amount}"),
            Self::White => "white".into(),
            Self::Blue => "blue".into(),
            Self::Black => "black".into(),
            Self::Red => "red".into(),
            Self::Green => "green".into(),
            Self::Colorless => "colorless".into(),
            Self::Snow => "snow".into(),
            Self::Hybrid(first, second) => {
                format!("hybrid/{}/{}", first.stable_id(), second.stable_id())
            }
            Self::VariableX => "x".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaCost {
    pub exact: String,
    pub symbols: Vec<ManaSymbol>,
}

impl ManaCost {
    fn stable_id(&self) -> String {
        self.symbols
            .iter()
            .map(|symbol| symbol.stable_id())
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableConstraint {
    None,
    AtLeastOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Amount {
    Exact(u32),
    VariableX,
}

impl Amount {
    fn stable_id(self) -> String {
        match self {
            Self::Exact(amount) => format!("exact/{amount}"),
            Self::VariableX => "x".into(),
        }
    }

    fn resolve(self, x_value: u32) -> u32 {
        match self {
            Self::Exact(amount) => amount,
            Self::VariableX => x_value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermanentFilter {
    AnyCreature,
    WhiteCreature,
    Mountain,
    Land,
    Planeswalker,
}

impl PermanentFilter {
    fn stable_id(self) -> &'static str {
        match self {
            Self::AnyCreature => "creature",
            Self::WhiteCreature => "white-creature",
            Self::Mountain => "mountain",
            Self::Land => "land",
            Self::Planeswalker => "planeswalker",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraveyardCardFilter {
    Any,
    Blue,
}

impl GraveyardCardFilter {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Blue => "blue",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdditionalCost {
    PayLife(u32),
    PayEnergy(u32),
    PayRepeatedColorless(u32),
    ExileCardsFromYourGraveyard {
        amount: Amount,
        filter: GraveyardCardFilter,
        other_than_source: bool,
    },
    ExileAnyNumberFromYourGraveyardWithCombinedCardTypesAtLeast {
        distinct_card_types: u32,
        other_than_source: bool,
    },
    ExileControlledPermanent {
        amount: u32,
        filter: PermanentFilter,
    },
    SacrificeControlledPermanent {
        amount: Amount,
        filter: PermanentFilter,
    },
    TapUntappedControlledPermanent {
        amount: u32,
        filter: PermanentFilter,
    },
    DiscardCards {
        amount: Amount,
    },
    RemoveLoyaltyCountersFromControlledPlaneswalkers {
        amount: Amount,
        minimum_one: bool,
    },
    BeholdSubtype {
        amount: u32,
        subtype: String,
    },
}

impl AdditionalCost {
    fn stable_id(&self) -> String {
        match self {
            Self::PayLife(amount) => format!("pay-life/{amount}"),
            Self::PayEnergy(amount) => format!("pay-energy/{amount}"),
            Self::PayRepeatedColorless(amount) => format!("pay-colorless/{amount}"),
            Self::ExileCardsFromYourGraveyard {
                amount,
                filter,
                other_than_source,
            } => format!(
                "exile-graveyard/{}/{}/other={other_than_source}",
                amount.stable_id(),
                filter.stable_id()
            ),
            Self::ExileAnyNumberFromYourGraveyardWithCombinedCardTypesAtLeast {
                distinct_card_types,
                other_than_source,
            } => format!(
                "exile-graveyard-any/card-types-at-least/{distinct_card_types}/other={other_than_source}"
            ),
            Self::ExileControlledPermanent { amount, filter } => {
                format!("exile-controlled/{amount}/{}", filter.stable_id())
            }
            Self::SacrificeControlledPermanent { amount, filter } => format!(
                "sacrifice-controlled/{}/{}",
                amount.stable_id(),
                filter.stable_id()
            ),
            Self::TapUntappedControlledPermanent { amount, filter } => {
                format!("tap-controlled/{amount}/{}", filter.stable_id())
            }
            Self::DiscardCards { amount } => format!("discard/{}", amount.stable_id()),
            Self::RemoveLoyaltyCountersFromControlledPlaneswalkers {
                amount,
                minimum_one,
            } => format!(
                "remove-loyalty/{}/minimum-one={minimum_one}",
                amount.stable_id()
            ),
            Self::BeholdSubtype { amount, subtype } => {
                format!("behold/{amount}/{}", canonical_word(subtype))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternativeCost {
    pub mana: Option<ManaCost>,
    pub additional: Vec<AdditionalCost>,
    pub x_constraint: VariableConstraint,
    /// Ordinary additional costs imposed by the spell or game still apply.
    pub retains_other_cast_costs: bool,
}

impl AlternativeCost {
    fn stable_id(&self) -> String {
        let mana = self
            .mana
            .as_ref()
            .map(ManaCost::stable_id)
            .unwrap_or_else(|| "none".into());
        let additional = self
            .additional
            .iter()
            .map(AdditionalCost::stable_id)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "mana={mana};additional={additional};x={:?};retains-other={}",
            self.x_constraint, self.retains_other_cast_costs
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuspendCounterAmount {
    Fixed(u32),
    ChosenXAtLeastOne,
}

impl SuspendCounterAmount {
    fn stable_id(self) -> String {
        match self {
            Self::Fixed(amount) => format!("fixed/{amount}"),
            Self::ChosenXAtLeastOne => "x-at-least-one".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspendProgram {
    pub counters: SuspendCounterAmount,
    pub special_action_cost: AlternativeCost,
    pub only_from_hand: bool,
    pub upkeep_removes_one_time_counter: bool,
    pub last_counter_requires_play_if_able: bool,
    pub waives_mana_cost: bool,
    pub creature_spell_and_resulting_permanent_have_haste_until_control_lost: bool,
    pub printed_cast_unavailable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MadnessPlayKind {
    CastSpell,
    PlayLand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MadnessProgram {
    pub alternative_cost: AlternativeCost,
    pub play_kind: MadnessPlayKind,
    pub discard_destination_replaced_with_exile: bool,
    pub discarded_card_triggered_once: bool,
    pub graveyard_if_not_played: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnearthProgram {
    pub activation_cost: AlternativeCost,
    pub only_from_owners_graveyard: bool,
    pub sorcery_timing_only: bool,
    pub return_to_battlefield: bool,
    pub grants_haste: bool,
    pub exile_at_next_end_step: bool,
    pub battlefield_exit_replaced_with_exile: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscapeProgram {
    pub alternative_cost: AlternativeCost,
    pub only_from_owners_graveyard: bool,
    pub ordinary_stack_exit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlashbackCostModifier {
    None,
    ReduceGenericByGreatestOwnedCommanderManaValueOnBattlefieldOrCommandZone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidualFlashbackProgram {
    pub alternative_cost: AlternativeCost,
    pub modifier: FlashbackCostModifier,
    pub only_from_owners_graveyard: bool,
    pub every_stack_exit_replaced_with_exile: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlternateZoneKeywordKind {
    Suspend(SuspendProgram),
    Madness(MadnessProgram),
    Unearth(UnearthProgram),
    Escape(EscapeProgram),
    ResidualFlashback(ResidualFlashbackProgram),
}

impl AlternateZoneKeywordKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Suspend(_) => "Suspend",
            Self::Madness(_) => "Madness",
            Self::Unearth(_) => "Unearth",
            Self::Escape(_) => "Escape",
            Self::ResidualFlashback(_) => "Flashback",
        }
    }

    fn stable_id(&self) -> String {
        match self {
            Self::Suspend(program) => format!(
                "suspend;counters={};cost={};hand={};upkeep={};last-cast={};waive={};haste={};printed-unavailable={}",
                program.counters.stable_id(),
                program.special_action_cost.stable_id(),
                program.only_from_hand,
                program.upkeep_removes_one_time_counter,
                program.last_counter_requires_play_if_able,
                program.waives_mana_cost,
                program.creature_spell_and_resulting_permanent_have_haste_until_control_lost,
                program.printed_cast_unavailable
            ),
            Self::Madness(program) => format!(
                "madness;cost={};kind={:?};replacement={};trigger={};graveyard={}",
                program.alternative_cost.stable_id(),
                program.play_kind,
                program.discard_destination_replaced_with_exile,
                program.discarded_card_triggered_once,
                program.graveyard_if_not_played
            ),
            Self::Unearth(program) => format!(
                "unearth;cost={};owner-graveyard={};sorcery={};return={};haste={};end-step={};replacement={}",
                program.activation_cost.stable_id(),
                program.only_from_owners_graveyard,
                program.sorcery_timing_only,
                program.return_to_battlefield,
                program.grants_haste,
                program.exile_at_next_end_step,
                program.battlefield_exit_replaced_with_exile
            ),
            Self::Escape(program) => format!(
                "escape;cost={};owner-graveyard={};ordinary-exit={}",
                program.alternative_cost.stable_id(),
                program.only_from_owners_graveyard,
                program.ordinary_stack_exit
            ),
            Self::ResidualFlashback(program) => format!(
                "flashback;cost={};modifier={:?};owner-graveyard={};replacement={}",
                program.alternative_cost.stable_id(),
                program.modifier,
                program.only_from_owners_graveyard,
                program.every_stack_exit_replaced_with_exile
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternateZoneKeywordProgram {
    exact_source: String,
    source_context: SourceSemanticContext,
    semantic_digest: String,
    kind: AlternateZoneKeywordKind,
}

impl AlternateZoneKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub const fn source_context(&self) -> SourceSemanticContext {
        self.source_context
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &AlternateZoneKeywordKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        alternate_zone_cast_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotCandidateClass {
    SupportedFamily,
    StandardFlashbackOwnedByOfficialKeywordRuntime,
    UnsupportedCompoundOrModifier,
}

pub fn classify_snapshot_candidate(
    exact_source: &str,
    source_type_line: &str,
) -> Option<SnapshotCandidateClass> {
    let keyword_candidate = ["Suspend ", "Madness ", "Unearth ", "Escape", "Flashback "]
        .iter()
        .any(|prefix| exact_source.starts_with(prefix))
        || ["Suspend", "Madness", "Unearth", "Escape", "Flashback"]
            .iter()
            .any(|prefix| {
                exact_source.starts_with(&format!("{prefix}\u{fffd}"))
                    || exact_source.starts_with(&format!("{prefix}\u{2014}"))
            })
        || exact_source.starts_with("Commander Suspend ");
    if !keyword_candidate {
        return None;
    }
    if compile_alternate_zone_cast_keyword_program(exact_source, source_type_line).is_some() {
        return Some(SnapshotCandidateClass::SupportedFamily);
    }
    if standard_flashback_owned_by_official_keyword_runtime(exact_source) {
        return Some(SnapshotCandidateClass::StandardFlashbackOwnedByOfficialKeywordRuntime);
    }
    Some(SnapshotCandidateClass::UnsupportedCompoundOrModifier)
}

fn standard_flashback_owned_by_official_keyword_runtime(exact_source: &str) -> bool {
    let Some((core, reminder)) = split_trailing_parenthetical(exact_source) else {
        return false;
    };
    let Some(cost) = core.strip_prefix("Flashback ") else {
        return false;
    };
    if reminder.is_some_and(|reminder| {
        reminder
            != "You may cast this card from your graveyard for its flashback cost. Then exile it."
    }) {
        return false;
    }
    parse_mana_cost(cost).is_some()
}

/// Returns a complete, content-keyed program or `None`. Snapshot coordinates,
/// card names, Oracle IDs, and snapshot hashes are intentionally not inputs.
pub fn compile_alternate_zone_cast_keyword_program(
    exact_source: &str,
    source_type_line: &str,
) -> Option<AlternateZoneKeywordProgram> {
    if !complete_single_line(exact_source) || source_type_line.trim().is_empty() {
        return None;
    }
    let source_context = SourceSemanticContext::from_type_line(source_type_line);
    let kind = parse_suspend(exact_source, source_context)
        .or_else(|| parse_madness(exact_source, source_context))
        .or_else(|| parse_unearth(exact_source, source_context))
        .or_else(|| parse_escape(exact_source, source_context))
        .or_else(|| parse_residual_flashback(exact_source, source_context))?;
    let semantic_digest = semantic_digest(exact_source, source_context, &kind);
    Some(AlternateZoneKeywordProgram {
        exact_source: exact_source.to_owned(),
        source_context,
        semantic_digest,
        kind,
    })
}

fn complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source == source.trim()
        && !source.contains('\n')
        && !source.contains('\r')
}

fn semantic_digest(
    exact_source: &str,
    source_context: SourceSemanticContext,
    kind: &AlternateZoneKeywordKind,
) -> String {
    let mut hasher = Sha256::new();
    for component in [
        "alternate-zone-cast-keyword-content/v1".to_owned(),
        ALTERNATE_ZONE_CAST_COMPILER_VERSION.to_owned(),
        ALTERNATE_ZONE_CAST_RUNTIME_VERSION.to_owned(),
        ALTERNATE_ZONE_CAST_RULES_CONTEXT_VERSION.to_owned(),
        exact_source.to_owned(),
        source_context.stable_id(),
        kind.stable_id(),
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn parse_suspend(
    exact_source: &str,
    _source_context: SourceSemanticContext,
) -> Option<AlternateZoneKeywordKind> {
    let remainder = exact_source.strip_prefix("Suspend ")?;
    let (header, reminder) = split_trailing_parenthetical(remainder)?;
    let (counter_text, cost_text) = split_keyword_dash(header)?;

    let (counters, x_constraint, header_without_period) = if counter_text == "X" {
        let cost_text = cost_text.strip_suffix(". X can't be 0.")?;
        (
            SuspendCounterAmount::ChosenXAtLeastOne,
            VariableConstraint::AtLeastOne,
            cost_text,
        )
    } else {
        (
            SuspendCounterAmount::Fixed(counter_text.parse::<u32>().ok()?),
            VariableConstraint::None,
            cost_text,
        )
    };
    let mana = parse_mana_cost(header_without_period)?;
    match counters {
        SuspendCounterAmount::ChosenXAtLeastOne => {
            if !mana.symbols.contains(&ManaSymbol::VariableX) {
                return None;
            }
        }
        SuspendCounterAmount::Fixed(0) => return None,
        SuspendCounterAmount::Fixed(_) => {
            if mana.symbols.contains(&ManaSymbol::VariableX) {
                return None;
            }
        }
    }

    let printed_cast_unavailable = if let Some(reminder) = reminder {
        if reminder_declares_source_has_no_mana_cost_and_must_be_suspended(reminder) {
            true
        } else {
            validate_suspend_reminder(reminder, counters, &mana)?;
            false
        }
    } else {
        false
    };

    Some(AlternateZoneKeywordKind::Suspend(SuspendProgram {
        counters,
        special_action_cost: AlternativeCost {
            mana: Some(mana),
            additional: Vec::new(),
            x_constraint,
            retains_other_cast_costs: false,
        },
        only_from_hand: true,
        upkeep_removes_one_time_counter: true,
        last_counter_requires_play_if_able: true,
        waives_mana_cost: true,
        creature_spell_and_resulting_permanent_have_haste_until_control_lost: true,
        printed_cast_unavailable,
    }))
}

fn validate_suspend_reminder(
    reminder: &str,
    counters: SuspendCounterAmount,
    mana: &ManaCost,
) -> Option<()> {
    let counter_phrase = match counters {
        SuspendCounterAmount::Fixed(1) => "a time counter".to_owned(),
        SuspendCounterAmount::Fixed(amount) => {
            format!("{} time counters", number_word(amount)?)
        }
        SuspendCounterAmount::ChosenXAtLeastOne => "X time counters".to_owned(),
    };
    let permission_prefixes = [
        "Rather than cast this card from your hand, you may pay ",
        "Rather than cast this card from your hand, pay ",
    ];
    let prefix = permission_prefixes
        .iter()
        .find(|prefix| reminder.starts_with(**prefix))?;
    let suffix_without_haste = format!(
        "{} and exile it with {counter_phrase} on it. At the beginning of your upkeep, remove a time counter. When the last is removed, you may cast it without paying its mana cost.",
        mana.exact
    );
    let suffix_with_haste = format!("{suffix_without_haste} It has haste.");
    let body = &reminder[prefix.len()..];
    (body == suffix_without_haste || body == suffix_with_haste).then_some(())
}

fn parse_madness(
    exact_source: &str,
    source_context: SourceSemanticContext,
) -> Option<AlternateZoneKeywordKind> {
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    let cost_text = if let Some(value) = core.strip_prefix("Madness ") {
        value
    } else {
        strip_dash_prefix(core, "Madness")?
    };
    let alternative_cost = if cost_text == "Pay six {C}." {
        AlternativeCost {
            mana: None,
            additional: vec![AdditionalCost::PayRepeatedColorless(6)],
            x_constraint: VariableConstraint::None,
            retains_other_cast_costs: true,
        }
    } else {
        parse_mana_and_simple_life_cost(cost_text, true)?
    };
    if let Some(reminder) = reminder {
        if source_context.is_land {
            if reminder
                != "If you discard this card, discard it into exile. When you do, play it for its madness cost or put it into your graveyard. You can play a land only during your turn and only if you have an available land play remaining."
            {
                return None;
            }
        } else if reminder
            != "If you discard this card, discard it into exile. When you do, cast it for its madness cost or put it into your graveyard."
        {
            return None;
        }
    }
    Some(AlternateZoneKeywordKind::Madness(MadnessProgram {
        alternative_cost,
        play_kind: if source_context.is_land {
            MadnessPlayKind::PlayLand
        } else {
            MadnessPlayKind::CastSpell
        },
        discard_destination_replaced_with_exile: true,
        discarded_card_triggered_once: true,
        graveyard_if_not_played: true,
    }))
}

fn parse_unearth(
    exact_source: &str,
    source_context: SourceSemanticContext,
) -> Option<AlternateZoneKeywordKind> {
    if !source_context.is_permanent_card {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    let cost_text = if let Some(value) = core.strip_prefix("Unearth ") {
        value
    } else {
        strip_dash_prefix(core, "Unearth")?
    };
    let activation_cost = if cost_text == "Pay eight {E}." {
        AlternativeCost {
            mana: None,
            additional: vec![AdditionalCost::PayEnergy(8)],
            x_constraint: VariableConstraint::None,
            retains_other_cast_costs: false,
        }
    } else {
        AlternativeCost {
            mana: Some(parse_mana_cost(cost_text)?),
            additional: Vec::new(),
            x_constraint: VariableConstraint::None,
            retains_other_cast_costs: false,
        }
    };
    if let Some(reminder) = reminder {
        let expected_mana = activation_cost.mana.as_ref().map(|cost| {
            format!(
                "{}: Return this card from your graveyard to the battlefield.",
                cost.exact
            )
        });
        let prefix = if activation_cost.additional == [AdditionalCost::PayEnergy(8)] {
            "Pay eight energy counters: Return this card from your graveyard to the battlefield."
                .to_owned()
        } else {
            expected_mana?
        };
        let without_haste = format!(
            "{prefix} Exile it at the beginning of the next end step or if it would leave the battlefield. Unearth only as a sorcery."
        );
        let with_haste = format!(
            "{prefix} It gains haste. Exile it at the beginning of the next end step or if it would leave the battlefield. Unearth only as a sorcery."
        );
        if reminder != without_haste && reminder != with_haste {
            return None;
        }
    }
    Some(AlternateZoneKeywordKind::Unearth(UnearthProgram {
        activation_cost,
        only_from_owners_graveyard: true,
        sorcery_timing_only: true,
        return_to_battlefield: true,
        grants_haste: true,
        exile_at_next_end_step: true,
        battlefield_exit_replaced_with_exile: true,
    }))
}

fn parse_escape(
    exact_source: &str,
    source_context: SourceSemanticContext,
) -> Option<AlternateZoneKeywordKind> {
    if source_context.is_land {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    let cost_text = strip_dash_prefix(core, "Escape")?;
    if let Some(reminder) = reminder
        && reminder != "You may cast this card from your graveyard for its escape cost."
    {
        return None;
    }
    let mut parts = cost_text.trim_end_matches('.').split(", ");
    let mana = parse_mana_cost(parts.next()?)?;
    let rest = parts.collect::<Vec<_>>();
    if rest.is_empty() {
        return None;
    }
    let mut additional = Vec::new();
    for part in rest {
        if part == "Exile a land you control" {
            additional.push(AdditionalCost::ExileControlledPermanent {
                amount: 1,
                filter: PermanentFilter::Land,
            });
            continue;
        }
        if part
            == "Exile any number of other cards from your graveyard with four or more card types among them"
        {
            additional.push(
                AdditionalCost::ExileAnyNumberFromYourGraveyardWithCombinedCardTypesAtLeast {
                    distinct_card_types: 4,
                    other_than_source: true,
                },
            );
            continue;
        }
        let amount_text = part
            .strip_prefix("Exile ")?
            .strip_suffix(" other cards from your graveyard")?;
        additional.push(AdditionalCost::ExileCardsFromYourGraveyard {
            amount: Amount::Exact(parse_number_word(amount_text)?),
            filter: GraveyardCardFilter::Any,
            other_than_source: true,
        });
    }
    Some(AlternateZoneKeywordKind::Escape(EscapeProgram {
        alternative_cost: AlternativeCost {
            mana: Some(mana),
            additional,
            x_constraint: VariableConstraint::None,
            retains_other_cast_costs: true,
        },
        only_from_owners_graveyard: true,
        ordinary_stack_exit: true,
    }))
}

fn parse_residual_flashback(
    exact_source: &str,
    source_context: SourceSemanticContext,
) -> Option<AlternateZoneKeywordKind> {
    if !source_context.is_instant_or_sorcery {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    let (cost_text, used_dash) = if let Some(value) = core.strip_prefix("Flashback ") {
        (value, false)
    } else {
        (strip_dash_prefix(core, "Flashback")?, true)
    };

    let canonical_reminder =
        "You may cast this card from your graveyard for its flashback cost. Then exile it.";
    let additional_cost_reminder = "You may cast this card from your graveyard for its flashback cost and any additional costs. Then exile it.";
    let behold_reminder = "You may cast this card from your graveyard for its flashback cost. Then exile it. To behold an Elemental, choose an Elemental you control or reveal an Elemental card from your hand.";
    let insight_reminder = reminder.is_some_and(insight_self_exclusion_reminder);
    if reminder.is_some_and(|value| {
        ![
            canonical_reminder,
            additional_cost_reminder,
            behold_reminder,
        ]
        .contains(&value)
            && !insight_self_exclusion_reminder(value)
    }) {
        return None;
    }

    const COMMANDER_REDUCTION: &str = ". This spell costs {X} less to cast this way, where X is the greatest mana value of a commander you own on the battlefield or in the command zone.";
    let (cost_text, modifier) = if let Some(value) = cost_text.strip_suffix(COMMANDER_REDUCTION) {
        (
            value,
            FlashbackCostModifier::ReduceGenericByGreatestOwnedCommanderManaValueOnBattlefieldOrCommandZone,
        )
    } else {
        (cost_text, FlashbackCostModifier::None)
    };

    let pure_mana = parse_mana_cost(cost_text);
    if !used_dash
        && modifier == FlashbackCostModifier::None
        && pure_mana.is_some()
        && reminder != Some(additional_cost_reminder)
    {
        // The ordinary official-keyword compiler owns this complete form.
        return None;
    }

    let alternative_cost = if let Some(mana) = pure_mana {
        AlternativeCost {
            mana: Some(mana),
            additional: Vec::new(),
            x_constraint: VariableConstraint::None,
            retains_other_cast_costs: true,
        }
    } else {
        parse_flashback_extended_cost(cost_text)?
    };

    if reminder == Some(behold_reminder)
        && !alternative_cost.additional.iter().any(|cost| {
            matches!(
                cost,
                AdditionalCost::BeholdSubtype {
                    amount: 3,
                    subtype
                } if subtype == "Elemental"
            )
        })
    {
        return None;
    }
    if insight_reminder
        && !alternative_cost.additional.iter().any(|cost| {
            matches!(
                cost,
                AdditionalCost::ExileCardsFromYourGraveyard {
                    amount: Amount::VariableX,
                    filter: GraveyardCardFilter::Blue,
                    other_than_source: true
                }
            )
        })
    {
        return None;
    }

    Some(AlternateZoneKeywordKind::ResidualFlashback(
        ResidualFlashbackProgram {
            alternative_cost,
            modifier,
            only_from_owners_graveyard: true,
            every_stack_exit_replaced_with_exile: true,
        },
    ))
}

fn reminder_declares_source_has_no_mana_cost_and_must_be_suspended(reminder: &str) -> bool {
    reminder
        .strip_suffix(" has no mana cost and must be suspended.")
        .is_some_and(valid_printed_referent)
}

fn insight_self_exclusion_reminder(reminder: &str) -> bool {
    const PREFIX: &str = "You may cast this card from your graveyard for its flashback cost, then exile it. You can't exile ";
    const SUFFIX: &str = " to pay for its own flashback cost.";
    reminder
        .strip_prefix(PREFIX)
        .and_then(|value| value.strip_suffix(SUFFIX))
        .is_some_and(valid_printed_referent)
}

fn valid_printed_referent(value: &str) -> bool {
    !value.is_empty()
        && value == value.trim()
        && !value.contains(['\n', '\r', '.', '(', ')'])
        && value.chars().any(char::is_alphanumeric)
}

fn parse_flashback_extended_cost(source: &str) -> Option<AlternativeCost> {
    let source = source.trim_end_matches('.');
    let (mana, additional_text) = if source.starts_with('{') {
        let (mana_text, additional_text) = source.split_once(", ")?;
        (Some(parse_mana_cost(mana_text)?), additional_text)
    } else {
        (None, source)
    };
    let (additional, x_constraint) = match additional_text {
        "Pay 3 life" => (AdditionalCost::PayLife(3), VariableConstraint::None),
        "Sacrifice a creature" => (
            AdditionalCost::SacrificeControlledPermanent {
                amount: Amount::Exact(1),
                filter: PermanentFilter::AnyCreature,
            },
            VariableConstraint::None,
        ),
        "Sacrifice three creatures" => (
            AdditionalCost::SacrificeControlledPermanent {
                amount: Amount::Exact(3),
                filter: PermanentFilter::AnyCreature,
            },
            VariableConstraint::None,
        ),
        "Sacrifice a Mountain" => (
            AdditionalCost::SacrificeControlledPermanent {
                amount: Amount::Exact(1),
                filter: PermanentFilter::Mountain,
            },
            VariableConstraint::None,
        ),
        "Tap an untapped white creature you control" => (
            AdditionalCost::TapUntappedControlledPermanent {
                amount: 1,
                filter: PermanentFilter::WhiteCreature,
            },
            VariableConstraint::None,
        ),
        "Tap three untapped creatures you control" => (
            AdditionalCost::TapUntappedControlledPermanent {
                amount: 3,
                filter: PermanentFilter::AnyCreature,
            },
            VariableConstraint::None,
        ),
        "Tap three untapped white creatures you control" => (
            AdditionalCost::TapUntappedControlledPermanent {
                amount: 3,
                filter: PermanentFilter::WhiteCreature,
            },
            VariableConstraint::None,
        ),
        "Behold three Elementals" => (
            AdditionalCost::BeholdSubtype {
                amount: 3,
                subtype: "Elemental".into(),
            },
            VariableConstraint::None,
        ),
        "Exile X blue cards from your graveyard" => (
            AdditionalCost::ExileCardsFromYourGraveyard {
                amount: Amount::VariableX,
                filter: GraveyardCardFilter::Blue,
                other_than_source: true,
            },
            VariableConstraint::None,
        ),
        "Exile X cards from your graveyard" => (
            AdditionalCost::ExileCardsFromYourGraveyard {
                amount: Amount::VariableX,
                filter: GraveyardCardFilter::Any,
                other_than_source: true,
            },
            VariableConstraint::None,
        ),
        "Discard X cards" => (
            AdditionalCost::DiscardCards {
                amount: Amount::VariableX,
            },
            VariableConstraint::None,
        ),
        "Sacrifice X Mountains" => (
            AdditionalCost::SacrificeControlledPermanent {
                amount: Amount::VariableX,
                filter: PermanentFilter::Mountain,
            },
            VariableConstraint::None,
        ),
        "Remove X loyalty counters from among planeswalkers you control. If you cast this spell this way, X can't be 0" => {
            (
                AdditionalCost::RemoveLoyaltyCountersFromControlledPlaneswalkers {
                    amount: Amount::VariableX,
                    minimum_one: true,
                },
                VariableConstraint::AtLeastOne,
            )
        }
        _ => return None,
    };
    Some(AlternativeCost {
        mana,
        additional: vec![additional],
        x_constraint,
        retains_other_cast_costs: true,
    })
}

fn parse_mana_and_simple_life_cost(
    source: &str,
    retains_other_cast_costs: bool,
) -> Option<AlternativeCost> {
    let source = source.trim_end_matches('.');
    let (mana_text, life) = if let Some((mana, life)) = source.split_once(", Pay ") {
        let life = life.strip_suffix(" life")?.parse::<u32>().ok()?;
        (mana, Some(life))
    } else {
        (source, None)
    };
    let mana = parse_mana_cost(mana_text)?;
    Some(AlternativeCost {
        mana: Some(mana),
        additional: life.into_iter().map(AdditionalCost::PayLife).collect(),
        x_constraint: VariableConstraint::None,
        retains_other_cast_costs,
    })
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    let mut symbols = Vec::new();
    let mut offset = 0;
    while offset < source.len() {
        let remaining = &source[offset..];
        let character = remaining.chars().next()?;
        if character.is_whitespace() {
            offset += character.len_utf8();
            continue;
        }
        if character != '{' {
            return None;
        }
        let token_start = offset + 1;
        let token_end = token_start + source[token_start..].find('}')?;
        if source[token_start..token_end].contains('{') {
            return None;
        }
        let token = source[token_start..token_end].trim().to_ascii_uppercase();
        let symbol = if token.bytes().all(|byte| byte.is_ascii_digit()) {
            ManaSymbol::Generic(token.parse::<u32>().ok()?)
        } else {
            match token.as_str() {
                "W" => ManaSymbol::White,
                "U" => ManaSymbol::Blue,
                "B" => ManaSymbol::Black,
                "R" => ManaSymbol::Red,
                "G" => ManaSymbol::Green,
                "C" => ManaSymbol::Colorless,
                "S" => ManaSymbol::Snow,
                "X" => ManaSymbol::VariableX,
                _ => {
                    let (first, second) = token.split_once('/')?;
                    ManaSymbol::Hybrid(parse_mana_color(first)?, parse_mana_color(second)?)
                }
            }
        };
        symbols.push(symbol);
        offset = token_end + 1;
    }
    (!symbols.is_empty()).then(|| ManaCost {
        exact: source.to_owned(),
        symbols,
    })
}

fn parse_mana_color(source: &str) -> Option<ManaColor> {
    match source {
        "W" => Some(ManaColor::White),
        "U" => Some(ManaColor::Blue),
        "B" => Some(ManaColor::Black),
        "R" => Some(ManaColor::Red),
        "G" => Some(ManaColor::Green),
        "C" => Some(ManaColor::Colorless),
        _ => None,
    }
}

fn split_keyword_dash(source: &str) -> Option<(&str, &str)> {
    for separator in ['\u{fffd}', '\u{2014}'] {
        if let Some((before, after)) = source.split_once(separator) {
            let before = before.trim();
            let after = after.trim();
            if !before.is_empty() && !after.is_empty() {
                return Some((before, after));
            }
        }
    }
    None
}

fn strip_dash_prefix<'a>(source: &'a str, keyword: &str) -> Option<&'a str> {
    let suffix = source.strip_prefix(keyword)?;
    let suffix = suffix.trim_start();
    for separator in ['\u{fffd}', '\u{2014}'] {
        if let Some(value) = suffix.strip_prefix(separator) {
            let value = value.trim();
            return (!value.is_empty()).then_some(value);
        }
    }
    None
}

/// Returns `(core, reminder)`. Parentheses are accepted only as one complete
/// trailing group so a compound clause can never be consumed partially.
fn split_trailing_parenthetical(source: &str) -> Option<(&str, Option<&str>)> {
    let mut depth = 0_u32;
    let mut outer_start = None;
    for (index, character) in source.char_indices() {
        match character {
            '(' => {
                if depth == 0 {
                    if outer_start.is_some() {
                        return None;
                    }
                    outer_start = Some(index);
                }
                depth = depth.checked_add(1)?;
            }
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 && index + 1 != source.len() {
                    return None;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    let Some(start) = outer_start else {
        return Some((source, None));
    };
    if !source[..start].ends_with(' ') || !source.ends_with(')') {
        return None;
    }
    let core = source[..start].trim_end();
    let reminder = &source[start + 1..source.len() - 1];
    (!core.is_empty() && !reminder.is_empty()).then_some((core, Some(reminder)))
}

fn number_word(amount: u32) -> Option<&'static str> {
    match amount {
        1 => Some("one"),
        2 => Some("two"),
        3 => Some("three"),
        4 => Some("four"),
        5 => Some("five"),
        6 => Some("six"),
        7 => Some("seven"),
        8 => Some("eight"),
        9 => Some("nine"),
        10 => Some("ten"),
        11 => Some("eleven"),
        12 => Some("twelve"),
        13 => Some("thirteen"),
        14 => Some("fourteen"),
        15 => Some("fifteen"),
        16 => Some("sixteen"),
        17 => Some("seventeen"),
        _ => None,
    }
}

fn parse_number_word(source: &str) -> Option<u32> {
    (1..=17).find(|amount| number_word(*amount) == Some(source))
}

fn canonical_word(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn card_types_from_type_line(type_line: &str) -> BTreeSet<CardType> {
    let head = type_line
        .split(['\u{fffd}', '\u{2014}'])
        .next()
        .unwrap_or(type_line);
    head.split_whitespace()
        .map(|token| match token.to_ascii_lowercase().as_str() {
            "artifact" => CardType::Artifact,
            "battle" => CardType::Battle,
            "creature" => CardType::Creature,
            "enchantment" => CardType::Enchantment,
            "instant" => CardType::Instant,
            "kindred" => CardType::Kindred,
            "land" => CardType::Land,
            "planeswalker" => CardType::Planeswalker,
            "sorcery" => CardType::Sorcery,
            other => CardType::Other(other.to_owned()),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManaUnit {
    pub id: ManaUnitId,
    pub color: ManaColor,
    pub snow_source: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub life: i32,
    pub energy: u32,
    pub land_plays_remaining: u32,
    pub mana_pool: BTreeMap<ManaUnitId, ManaUnit>,
}

impl PlayerState {
    pub fn new(life: i32) -> Self {
        Self {
            life,
            energy: 0,
            land_plays_remaining: 1,
            mana_pool: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedCard {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub controller: Option<PlayerId>,
    pub zone: Zone,
    pub card_types: BTreeSet<CardType>,
    pub subtypes: BTreeSet<String>,
    pub colors: BTreeSet<ManaColor>,
    pub mana_value: u32,
    pub is_commander: bool,
    pub tapped: bool,
    pub counters: BTreeMap<String, u32>,
    pub cast_method: Option<CastMethod>,
}

impl TrackedCard {
    pub fn card(
        object_ref: ObjectRef,
        owner: PlayerId,
        zone: Zone,
        card_types: impl IntoIterator<Item = CardType>,
    ) -> Self {
        Self {
            object_ref,
            owner,
            controller: matches!(zone, Zone::Battlefield | Zone::Stack).then_some(owner),
            zone,
            card_types: card_types.into_iter().collect(),
            subtypes: BTreeSet::new(),
            colors: BTreeSet::new(),
            mana_value: 0,
            is_commander: false,
            tapped: false,
            counters: BTreeMap::new(),
            cast_method: None,
        }
    }

    pub fn source_context(&self) -> SourceSemanticContext {
        let is_land = self.card_types.contains(&CardType::Land);
        let is_creature = self.card_types.contains(&CardType::Creature);
        let is_instant_or_sorcery = self.card_types.contains(&CardType::Instant)
            || self.card_types.contains(&CardType::Sorcery);
        SourceSemanticContext {
            is_land,
            is_creature,
            is_instant_or_sorcery,
            is_permanent_card: !is_instant_or_sorcery,
        }
    }

    fn matches_filter(&self, filter: PermanentFilter) -> bool {
        match filter {
            PermanentFilter::AnyCreature => self.card_types.contains(&CardType::Creature),
            PermanentFilter::WhiteCreature => {
                self.card_types.contains(&CardType::Creature)
                    && self.colors.contains(&ManaColor::White)
            }
            PermanentFilter::Mountain => {
                self.card_types.contains(&CardType::Land)
                    && self
                        .subtypes
                        .iter()
                        .any(|subtype| subtype.eq_ignore_ascii_case("Mountain"))
            }
            PermanentFilter::Land => self.card_types.contains(&CardType::Land),
            PermanentFilter::Planeswalker => self.card_types.contains(&CardType::Planeswalker),
        }
    }

    fn has_subtype(&self, subtype: &str) -> bool {
        self.subtypes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(subtype))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastMethod {
    Suspend { semantic_digest: String },
    Madness { semantic_digest: String },
    Escape { semantic_digest: String },
    Flashback { semantic_digest: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Beginning,
    PrecombatMain,
    Combat,
    PostcombatMain,
    Ending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriorityWindow {
    pub turn: TurnId,
    pub active_player: PlayerId,
    pub phase: Phase,
    pub stack_empty: bool,
    pub priority_player: PlayerId,
}

impl PriorityWindow {
    pub fn is_sorcery_timing_for(self, player: PlayerId) -> bool {
        self.active_player == player
            && self.priority_player == player
            && self.stack_empty
            && matches!(self.phase, Phase::PrecombatMain | Phase::PostcombatMain)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastLegality {
    pub timing_and_permissions_allow_cast: bool,
    pub required_targets_available: bool,
    pub prohibitions_allow_cast: bool,
    pub all_external_additional_costs_satisfied: bool,
}

impl CastLegality {
    pub const fn fully_legal() -> Self {
        Self {
            timing_and_permissions_allow_cast: true,
            required_targets_available: true,
            prohibitions_allow_cast: true,
            all_external_additional_costs_satisfied: true,
        }
    }

    fn permits_cast(self, retains_other_costs: bool) -> bool {
        self.timing_and_permissions_allow_cast
            && self.required_targets_available
            && self.prohibitions_allow_cast
            && (!retains_other_costs || self.all_external_additional_costs_satisfied)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeholdChoice {
    ControlledPermanent(ObjectRef),
    RevealFromHand(ObjectRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoyaltyCounterPayment {
    pub planeswalker: ObjectRef,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CostPayment {
    pub x_value: u32,
    pub mana_units: Vec<ManaUnitId>,
    pub life: u32,
    pub energy: u32,
    pub exile_from_graveyard: Vec<ObjectRef>,
    pub exile_controlled_permanents: Vec<ObjectRef>,
    pub sacrifice_permanents: Vec<ObjectRef>,
    pub tap_permanents: Vec<ObjectRef>,
    pub discard_cards: Vec<ObjectRef>,
    pub remove_loyalty: Vec<LoyaltyCounterPayment>,
    pub behold: Vec<BeholdChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostEvidence {
    pub x_value: u32,
    pub mana_units_spent: Vec<ManaUnitId>,
    pub life_paid: u32,
    pub energy_paid: u32,
    pub cards_exiled_from_graveyard: Vec<ObjectRef>,
    pub controlled_permanents_exiled: Vec<ObjectRef>,
    pub permanents_sacrificed: Vec<ObjectRef>,
    pub permanents_tapped: Vec<ObjectRef>,
    pub cards_discarded: Vec<ObjectRef>,
    pub loyalty_removed: Vec<LoyaltyCounterPayment>,
    pub behold_choices: Vec<BeholdChoice>,
    pub generic_reduction_applied: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneChangeEvidence {
    pub before: ObjectRef,
    pub after: ObjectRef,
    pub from: Zone,
    pub requested_destination: Zone,
    pub actual_destination: Zone,
    pub replacement_applied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastEvidence {
    pub player: PlayerId,
    pub source_zone_change: ZoneChangeEvidence,
    pub cost: Option<CostEvidence>,
    pub method: CastMethod,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspendSpecialActionEvidence {
    pub player: PlayerId,
    pub source_zone_change: ZoneChangeEvidence,
    pub cost: CostEvidence,
    pub time_counters: u32,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspendUpkeepTrigger {
    pub id: PendingTriggerId,
    pub turn: TurnId,
    pub owner: PlayerId,
    pub suspended_card: ObjectRef,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuspendUpkeepResolution {
    CounterRemoved {
        card: ObjectRef,
        remaining: u32,
    },
    LastCounterRemoved {
        card: ObjectRef,
        cast_trigger: PendingTriggerId,
    },
    SourceMissing {
        card: ObjectRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
// This public receipt intentionally keeps its existing value layout.
#[allow(clippy::large_enum_variant)]
pub enum SuspendLastCounterResolution {
    Cast(CastEvidence),
    LandPlayed {
        player: PlayerId,
        zone_change: ZoneChangeEvidence,
    },
    CouldNotPlay {
        card: ObjectRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MadnessDiscardEvidence {
    pub player: PlayerId,
    pub replacement_zone_change: ZoneChangeEvidence,
    pub trigger: PendingTriggerId,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
// This public choice intentionally keeps its existing value layout.
#[allow(clippy::large_enum_variant)]
pub enum MadnessTriggerChoice {
    Play {
        payment: CostPayment,
        legality: CastLegality,
    },
    PutIntoGraveyard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MadnessResolutionEvidence {
    Cast(CastEvidence),
    LandPlayed {
        player: PlayerId,
        zone_change: ZoneChangeEvidence,
        cost: CostEvidence,
    },
    PutIntoGraveyard(ZoneChangeEvidence),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnearthActivationEvidence {
    pub id: PendingAbilityId,
    pub player: PlayerId,
    pub source: ObjectRef,
    pub cost: CostEvidence,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnearthResolutionEvidence {
    pub source_zone_change: ZoneChangeEvidence,
    pub haste_granted: bool,
    pub replacement_installed: bool,
    pub delayed_end_step_trigger: PendingTriggerId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnearthAbilityResolution {
    Returned(UnearthResolutionEvidence),
    SourceMissing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnearthDelayedResolution {
    Exiled(ZoneChangeEvidence),
    SourceNoLongerThatPermanent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SuspendedCard {
    object_ref: ObjectRef,
    owner: PlayerId,
    time_counters: u32,
    semantic_digest: String,
    source_context: SourceSemanticContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingMadness {
    card: ObjectRef,
    player: PlayerId,
    program: AlternateZoneKeywordProgram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingSuspendCast {
    card: ObjectRef,
    owner: PlayerId,
    semantic_digest: String,
    source_context: SourceSemanticContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingUnearth {
    source: ObjectRef,
    player: PlayerId,
    program: AlternateZoneKeywordProgram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnearthedPermanent {
    permanent: ObjectRef,
    controller: PlayerId,
    semantic_digest: String,
    end_step_trigger: PendingTriggerId,
    due_turn: TurnId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DelayedUnearthExile {
    permanent: ObjectRef,
    controller: PlayerId,
    semantic_digest: String,
    due_turn: TurnId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SuspendHasteGrant {
    object_ref: ObjectRef,
    controller: PlayerId,
    semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternateZoneCastRuntime {
    players: BTreeMap<PlayerId, PlayerState>,
    objects: BTreeMap<ObjectId, TrackedCard>,
    suspended: BTreeMap<ObjectId, SuspendedCard>,
    suspend_upkeep_triggers: BTreeMap<PendingTriggerId, SuspendUpkeepTrigger>,
    suspend_upkeep_created: BTreeSet<(TurnId, ObjectId)>,
    suspend_cast_triggers: BTreeMap<PendingTriggerId, PendingSuspendCast>,
    madness_triggers: BTreeMap<PendingTriggerId, PendingMadness>,
    unearth_abilities: BTreeMap<PendingAbilityId, PendingUnearth>,
    unearthed: BTreeMap<ObjectId, UnearthedPermanent>,
    unearth_delayed_exile: BTreeMap<PendingTriggerId, DelayedUnearthExile>,
    suspend_haste: BTreeMap<ObjectId, SuspendHasteGrant>,
    next_pending_id: u64,
}

impl Default for AlternateZoneCastRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl AlternateZoneCastRuntime {
    pub fn new() -> Self {
        Self {
            players: BTreeMap::new(),
            objects: BTreeMap::new(),
            suspended: BTreeMap::new(),
            suspend_upkeep_triggers: BTreeMap::new(),
            suspend_upkeep_created: BTreeSet::new(),
            suspend_cast_triggers: BTreeMap::new(),
            madness_triggers: BTreeMap::new(),
            unearth_abilities: BTreeMap::new(),
            unearthed: BTreeMap::new(),
            unearth_delayed_exile: BTreeMap::new(),
            suspend_haste: BTreeMap::new(),
            next_pending_id: 1,
        }
    }

    pub fn insert_player(
        &mut self,
        player: PlayerId,
        state: PlayerState,
    ) -> Result<(), AlternateZoneRuntimeError> {
        if self.players.insert(player, state).is_some() {
            return Err(AlternateZoneRuntimeError::DuplicatePlayer(player));
        }
        Ok(())
    }

    pub fn insert_object(&mut self, object: TrackedCard) -> Result<(), AlternateZoneRuntimeError> {
        validate_tracked_card(&object)?;
        if self
            .objects
            .insert(object.object_ref.object_id, object.clone())
            .is_some()
        {
            return Err(AlternateZoneRuntimeError::DuplicateObject(
                object.object_ref.object_id,
            ));
        }
        Ok(())
    }

    pub fn player(&self, player: PlayerId) -> Result<&PlayerState, AlternateZoneRuntimeError> {
        self.players
            .get(&player)
            .ok_or(AlternateZoneRuntimeError::MissingPlayer(player))
    }

    pub fn player_mut(
        &mut self,
        player: PlayerId,
    ) -> Result<&mut PlayerState, AlternateZoneRuntimeError> {
        self.players
            .get_mut(&player)
            .ok_or(AlternateZoneRuntimeError::MissingPlayer(player))
    }

    pub fn object(&self, object: ObjectId) -> Result<&TrackedCard, AlternateZoneRuntimeError> {
        self.objects
            .get(&object)
            .ok_or(AlternateZoneRuntimeError::MissingObject(object))
    }

    pub fn has_haste_from_suspend(&self, object: ObjectRef) -> bool {
        self.suspend_haste
            .get(&object.object_id)
            .is_some_and(|grant| grant.object_ref == object)
    }

    pub fn is_unearthed(&self, object: ObjectRef) -> bool {
        self.unearthed
            .get(&object.object_id)
            .is_some_and(|record| record.permanent == object)
    }

    pub fn has_haste_from_unearth(&self, object: ObjectRef) -> bool {
        self.is_unearthed(object)
    }

    pub fn suspend_from_hand(
        &mut self,
        program: &AlternateZoneKeywordProgram,
        player: PlayerId,
        source: ObjectRef,
        priority: PriorityWindow,
        cast_timing_legal_from_hand: bool,
        payment: CostPayment,
    ) -> Result<SuspendSpecialActionEvidence, AlternateZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.suspend_from_hand_inner(
            program,
            player,
            source,
            priority,
            cast_timing_legal_from_hand,
            payment,
        )?;
        *self = staged;
        Ok(evidence)
    }

    fn suspend_from_hand_inner(
        &mut self,
        program: &AlternateZoneKeywordProgram,
        player: PlayerId,
        source: ObjectRef,
        priority: PriorityWindow,
        cast_timing_legal_from_hand: bool,
        payment: CostPayment,
    ) -> Result<SuspendSpecialActionEvidence, AlternateZoneRuntimeError> {
        let AlternateZoneKeywordKind::Suspend(suspend) = program.kind() else {
            return Err(AlternateZoneRuntimeError::ProgramActionMismatch);
        };
        if priority.priority_player != player || !cast_timing_legal_from_hand {
            return Err(AlternateZoneRuntimeError::IllegalSuspendSpecialAction);
        }
        self.require_source(program, source, player, Zone::Hand)?;
        let time_counters = match suspend.counters {
            SuspendCounterAmount::Fixed(amount) => amount,
            SuspendCounterAmount::ChosenXAtLeastOne => {
                if payment.x_value == 0 {
                    return Err(AlternateZoneRuntimeError::VariableXBelowMinimum);
                }
                payment.x_value
            }
        };
        let cost = self.pay_cost(
            &suspend.special_action_cost,
            FlashbackCostModifier::None,
            player,
            source.object_id,
            &payment,
        )?;
        let change = self.move_object(source, Zone::Exile, player, false)?;
        self.object_mut(change.after)?
            .counters
            .insert("time".into(), time_counters);
        self.suspended.insert(
            source.object_id,
            SuspendedCard {
                object_ref: change.after,
                owner: player,
                time_counters,
                semantic_digest: program.semantic_digest().to_owned(),
                source_context: program.source_context(),
            },
        );
        Ok(SuspendSpecialActionEvidence {
            player,
            source_zone_change: change,
            cost,
            time_counters,
            semantic_digest: program.semantic_digest().to_owned(),
        })
    }

    pub fn begin_upkeep(
        &mut self,
        turn: TurnId,
        active_player: PlayerId,
    ) -> Vec<SuspendUpkeepTrigger> {
        let candidates = self
            .suspended
            .values()
            .filter(|record| {
                record.owner == active_player
                    && record.time_counters > 0
                    && !self
                        .suspend_upkeep_created
                        .contains(&(turn, record.object_ref.object_id))
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut triggers = Vec::new();
        for record in candidates {
            self.suspend_upkeep_created
                .insert((turn, record.object_ref.object_id));
            let id = PendingTriggerId(self.allocate_pending_id());
            let trigger = SuspendUpkeepTrigger {
                id,
                turn,
                owner: active_player,
                suspended_card: record.object_ref,
                semantic_digest: record.semantic_digest,
            };
            self.suspend_upkeep_triggers.insert(id, trigger.clone());
            triggers.push(trigger);
        }
        triggers
    }

    pub fn resolve_suspend_upkeep_trigger(
        &mut self,
        trigger: PendingTriggerId,
    ) -> Result<SuspendUpkeepResolution, AlternateZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.resolve_suspend_upkeep_trigger_inner(trigger)?;
        *self = staged;
        Ok(evidence)
    }

    fn resolve_suspend_upkeep_trigger_inner(
        &mut self,
        trigger: PendingTriggerId,
    ) -> Result<SuspendUpkeepResolution, AlternateZoneRuntimeError> {
        let pending = self
            .suspend_upkeep_triggers
            .remove(&trigger)
            .ok_or(AlternateZoneRuntimeError::MissingPendingTrigger(trigger))?;
        let source_is_current = self
            .objects
            .get(&pending.suspended_card.object_id)
            .is_some_and(|object| {
                object.object_ref == pending.suspended_card && object.zone == Zone::Exile
            });
        if !source_is_current {
            self.suspended.remove(&pending.suspended_card.object_id);
            return Ok(SuspendUpkeepResolution::SourceMissing {
                card: pending.suspended_card,
            });
        }
        let record_is_current = self
            .suspended
            .get(&pending.suspended_card.object_id)
            .is_some_and(|record| {
                record.object_ref == pending.suspended_card && record.time_counters > 0
            });
        if !record_is_current {
            self.suspended.remove(&pending.suspended_card.object_id);
            return Ok(SuspendUpkeepResolution::SourceMissing {
                card: pending.suspended_card,
            });
        }
        let record = self
            .suspended
            .get_mut(&pending.suspended_card.object_id)
            .expect("checked suspended record");
        record.time_counters -= 1;
        let card = record.object_ref;
        let remaining = record.time_counters;
        if remaining == 0 {
            self.object_mut(card)?.counters.remove("time");
        } else {
            self.object_mut(card)?
                .counters
                .insert("time".into(), remaining);
        }
        if remaining > 0 {
            return Ok(SuspendUpkeepResolution::CounterRemoved { card, remaining });
        }
        let record = self
            .suspended
            .remove(&pending.suspended_card.object_id)
            .expect("checked suspended record");
        let cast_trigger = PendingTriggerId(self.allocate_pending_id());
        self.suspend_cast_triggers.insert(
            cast_trigger,
            PendingSuspendCast {
                card: record.object_ref,
                owner: record.owner,
                semantic_digest: record.semantic_digest,
                source_context: record.source_context,
            },
        );
        Ok(SuspendUpkeepResolution::LastCounterRemoved {
            card: record.object_ref,
            cast_trigger,
        })
    }

    pub fn resolve_suspend_last_counter_trigger(
        &mut self,
        trigger: PendingTriggerId,
        play_legality: CastLegality,
    ) -> Result<SuspendLastCounterResolution, AlternateZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.resolve_suspend_last_counter_trigger_inner(trigger, play_legality)?;
        *self = staged;
        Ok(evidence)
    }

    fn resolve_suspend_last_counter_trigger_inner(
        &mut self,
        trigger: PendingTriggerId,
        play_legality: CastLegality,
    ) -> Result<SuspendLastCounterResolution, AlternateZoneRuntimeError> {
        let pending = self
            .suspend_cast_triggers
            .remove(&trigger)
            .ok_or(AlternateZoneRuntimeError::MissingPendingTrigger(trigger))?;
        if self
            .objects
            .get(&pending.card.object_id)
            .is_none_or(|object| object.object_ref != pending.card || object.zone != Zone::Exile)
        {
            return Ok(SuspendLastCounterResolution::CouldNotPlay { card: pending.card });
        }
        if !play_legality.permits_cast(false) {
            return Ok(SuspendLastCounterResolution::CouldNotPlay { card: pending.card });
        }
        if pending.source_context.is_land {
            let player_state = self.player_mut(pending.owner)?;
            if player_state.land_plays_remaining == 0 {
                return Ok(SuspendLastCounterResolution::CouldNotPlay { card: pending.card });
            }
            player_state.land_plays_remaining -= 1;
            let zone_change =
                self.move_object(pending.card, Zone::Battlefield, pending.owner, false)?;
            return Ok(SuspendLastCounterResolution::LandPlayed {
                player: pending.owner,
                zone_change,
            });
        }
        let change = self.move_object(pending.card, Zone::Stack, pending.owner, false)?;
        let method = CastMethod::Suspend {
            semantic_digest: pending.semantic_digest.clone(),
        };
        self.object_mut(change.after)?.cast_method = Some(method.clone());
        if pending.source_context.is_creature {
            self.suspend_haste.insert(
                change.after.object_id,
                SuspendHasteGrant {
                    object_ref: change.after,
                    controller: pending.owner,
                    semantic_digest: pending.semantic_digest,
                },
            );
        }
        Ok(SuspendLastCounterResolution::Cast(CastEvidence {
            player: pending.owner,
            source_zone_change: change,
            cost: None,
            method,
        }))
    }

    pub fn discard_with_madness(
        &mut self,
        program: &AlternateZoneKeywordProgram,
        player: PlayerId,
        source: ObjectRef,
    ) -> Result<MadnessDiscardEvidence, AlternateZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.discard_with_madness_inner(program, player, source)?;
        *self = staged;
        Ok(evidence)
    }

    fn discard_with_madness_inner(
        &mut self,
        program: &AlternateZoneKeywordProgram,
        player: PlayerId,
        source: ObjectRef,
    ) -> Result<MadnessDiscardEvidence, AlternateZoneRuntimeError> {
        if !matches!(program.kind(), AlternateZoneKeywordKind::Madness(_)) {
            return Err(AlternateZoneRuntimeError::ProgramActionMismatch);
        }
        self.require_source(program, source, player, Zone::Hand)?;
        let replacement_zone_change = self.move_object(source, Zone::Exile, player, true)?;
        let replacement_zone_change = ZoneChangeEvidence {
            requested_destination: Zone::Graveyard,
            actual_destination: Zone::Exile,
            replacement_applied: true,
            ..replacement_zone_change
        };
        let trigger = PendingTriggerId(self.allocate_pending_id());
        self.madness_triggers.insert(
            trigger,
            PendingMadness {
                card: replacement_zone_change.after,
                player,
                program: program.clone(),
            },
        );
        Ok(MadnessDiscardEvidence {
            player,
            replacement_zone_change,
            trigger,
            semantic_digest: program.semantic_digest().to_owned(),
        })
    }

    pub fn resolve_madness_trigger(
        &mut self,
        trigger: PendingTriggerId,
        choice: MadnessTriggerChoice,
    ) -> Result<MadnessResolutionEvidence, AlternateZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.resolve_madness_trigger_inner(trigger, choice)?;
        *self = staged;
        Ok(evidence)
    }

    fn resolve_madness_trigger_inner(
        &mut self,
        trigger: PendingTriggerId,
        choice: MadnessTriggerChoice,
    ) -> Result<MadnessResolutionEvidence, AlternateZoneRuntimeError> {
        let pending = self
            .madness_triggers
            .remove(&trigger)
            .ok_or(AlternateZoneRuntimeError::MissingPendingTrigger(trigger))?;
        self.require_exact_object(pending.card, Zone::Exile)?;
        let AlternateZoneKeywordKind::Madness(program) = pending.program.kind() else {
            return Err(AlternateZoneRuntimeError::ProgramActionMismatch);
        };
        match choice {
            MadnessTriggerChoice::PutIntoGraveyard => {
                let change =
                    self.move_object(pending.card, Zone::Graveyard, pending.player, false)?;
                Ok(MadnessResolutionEvidence::PutIntoGraveyard(change))
            }
            MadnessTriggerChoice::Play { payment, legality } => {
                if !legality.permits_cast(program.alternative_cost.retains_other_cast_costs) {
                    return Err(AlternateZoneRuntimeError::CastIsNotLegal);
                }
                match program.play_kind {
                    MadnessPlayKind::PlayLand => {
                        let player_state = self.player_mut(pending.player)?;
                        if player_state.land_plays_remaining == 0 {
                            return Err(AlternateZoneRuntimeError::NoLandPlayRemaining);
                        }
                        let cost = self.pay_cost(
                            &program.alternative_cost,
                            FlashbackCostModifier::None,
                            pending.player,
                            pending.card.object_id,
                            &payment,
                        )?;
                        self.player_mut(pending.player)?.land_plays_remaining -= 1;
                        let zone_change = self.move_object(
                            pending.card,
                            Zone::Battlefield,
                            pending.player,
                            false,
                        )?;
                        Ok(MadnessResolutionEvidence::LandPlayed {
                            player: pending.player,
                            zone_change,
                            cost,
                        })
                    }
                    MadnessPlayKind::CastSpell => {
                        let change =
                            self.move_object(pending.card, Zone::Stack, pending.player, false)?;
                        let cost = self.pay_cost(
                            &program.alternative_cost,
                            FlashbackCostModifier::None,
                            pending.player,
                            pending.card.object_id,
                            &payment,
                        )?;
                        let method = CastMethod::Madness {
                            semantic_digest: pending.program.semantic_digest().to_owned(),
                        };
                        self.object_mut(change.after)?.cast_method = Some(method.clone());
                        Ok(MadnessResolutionEvidence::Cast(CastEvidence {
                            player: pending.player,
                            source_zone_change: change,
                            cost: Some(cost),
                            method,
                        }))
                    }
                }
            }
        }
    }

    pub fn activate_unearth(
        &mut self,
        program: &AlternateZoneKeywordProgram,
        player: PlayerId,
        source: ObjectRef,
        priority: PriorityWindow,
        payment: CostPayment,
    ) -> Result<UnearthActivationEvidence, AlternateZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.activate_unearth_inner(program, player, source, priority, payment)?;
        *self = staged;
        Ok(evidence)
    }

    fn activate_unearth_inner(
        &mut self,
        program: &AlternateZoneKeywordProgram,
        player: PlayerId,
        source: ObjectRef,
        priority: PriorityWindow,
        payment: CostPayment,
    ) -> Result<UnearthActivationEvidence, AlternateZoneRuntimeError> {
        let AlternateZoneKeywordKind::Unearth(unearth) = program.kind() else {
            return Err(AlternateZoneRuntimeError::ProgramActionMismatch);
        };
        if !priority.is_sorcery_timing_for(player) {
            return Err(AlternateZoneRuntimeError::SorceryTimingRequired);
        }
        self.require_source(program, source, player, Zone::Graveyard)?;
        let cost = self.pay_cost(
            &unearth.activation_cost,
            FlashbackCostModifier::None,
            player,
            source.object_id,
            &payment,
        )?;
        let id = PendingAbilityId(self.allocate_pending_id());
        self.unearth_abilities.insert(
            id,
            PendingUnearth {
                source,
                player,
                program: program.clone(),
            },
        );
        Ok(UnearthActivationEvidence {
            id,
            player,
            source,
            cost,
            semantic_digest: program.semantic_digest().to_owned(),
        })
    }

    pub fn resolve_unearth(
        &mut self,
        ability: PendingAbilityId,
        current_turn: TurnId,
    ) -> Result<UnearthAbilityResolution, AlternateZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.resolve_unearth_inner(ability, current_turn)?;
        *self = staged;
        Ok(evidence)
    }

    fn resolve_unearth_inner(
        &mut self,
        ability: PendingAbilityId,
        current_turn: TurnId,
    ) -> Result<UnearthAbilityResolution, AlternateZoneRuntimeError> {
        let pending = self
            .unearth_abilities
            .remove(&ability)
            .ok_or(AlternateZoneRuntimeError::MissingPendingAbility(ability))?;
        if self
            .objects
            .get(&pending.source.object_id)
            .is_none_or(|object| {
                object.object_ref != pending.source || object.zone != Zone::Graveyard
            })
        {
            return Ok(UnearthAbilityResolution::SourceMissing);
        }
        let change = self.move_object(pending.source, Zone::Battlefield, pending.player, false)?;
        let delayed_end_step_trigger = PendingTriggerId(self.allocate_pending_id());
        self.unearthed.insert(
            change.after.object_id,
            UnearthedPermanent {
                permanent: change.after,
                controller: pending.player,
                semantic_digest: pending.program.semantic_digest().to_owned(),
                end_step_trigger: delayed_end_step_trigger,
                due_turn: current_turn,
            },
        );
        self.unearth_delayed_exile.insert(
            delayed_end_step_trigger,
            DelayedUnearthExile {
                permanent: change.after,
                controller: pending.player,
                semantic_digest: pending.program.semantic_digest().to_owned(),
                due_turn: current_turn,
            },
        );
        Ok(UnearthAbilityResolution::Returned(
            UnearthResolutionEvidence {
                source_zone_change: change,
                haste_granted: true,
                replacement_installed: true,
                delayed_end_step_trigger,
            },
        ))
    }

    pub fn move_unearthed_permanent(
        &mut self,
        permanent: ObjectRef,
        requested_destination: Zone,
    ) -> Result<ZoneChangeEvidence, AlternateZoneRuntimeError> {
        let record = self
            .unearthed
            .get(&permanent.object_id)
            .filter(|record| record.permanent == permanent)
            .cloned()
            .ok_or(AlternateZoneRuntimeError::NotUnearthed(permanent))?;
        self.move_battlefield_object_with_unearth_replacement(
            record.permanent,
            requested_destination,
            record.controller,
        )
    }

    pub fn resolve_unearth_end_step_trigger(
        &mut self,
        trigger: PendingTriggerId,
        current_turn: TurnId,
    ) -> Result<UnearthDelayedResolution, AlternateZoneRuntimeError> {
        let Some(delayed) = self.unearth_delayed_exile.get(&trigger).cloned() else {
            return Err(AlternateZoneRuntimeError::MissingPendingTrigger(trigger));
        };
        if current_turn < delayed.due_turn {
            return Err(AlternateZoneRuntimeError::TriggerNotDue);
        }
        self.unearth_delayed_exile.remove(&trigger);
        if self
            .objects
            .get(&delayed.permanent.object_id)
            .is_none_or(|object| {
                object.object_ref != delayed.permanent || object.zone != Zone::Battlefield
            })
        {
            self.unearthed.remove(&delayed.permanent.object_id);
            return Ok(UnearthDelayedResolution::SourceNoLongerThatPermanent);
        }
        let change = self.move_battlefield_object_with_unearth_replacement(
            delayed.permanent,
            Zone::Exile,
            delayed.controller,
        )?;
        Ok(UnearthDelayedResolution::Exiled(change))
    }

    pub fn cast_with_escape(
        &mut self,
        program: &AlternateZoneKeywordProgram,
        player: PlayerId,
        source: ObjectRef,
        payment: CostPayment,
        legality: CastLegality,
    ) -> Result<CastEvidence, AlternateZoneRuntimeError> {
        let AlternateZoneKeywordKind::Escape(escape) = program.kind() else {
            return Err(AlternateZoneRuntimeError::ProgramActionMismatch);
        };
        self.cast_from_graveyard(
            program,
            player,
            source,
            &escape.alternative_cost,
            FlashbackCostModifier::None,
            payment,
            legality,
            |digest| CastMethod::Escape {
                semantic_digest: digest,
            },
        )
    }

    pub fn cast_with_residual_flashback(
        &mut self,
        program: &AlternateZoneKeywordProgram,
        player: PlayerId,
        source: ObjectRef,
        payment: CostPayment,
        legality: CastLegality,
    ) -> Result<CastEvidence, AlternateZoneRuntimeError> {
        let AlternateZoneKeywordKind::ResidualFlashback(flashback) = program.kind() else {
            return Err(AlternateZoneRuntimeError::ProgramActionMismatch);
        };
        self.cast_from_graveyard(
            program,
            player,
            source,
            &flashback.alternative_cost,
            flashback.modifier.clone(),
            payment,
            legality,
            |digest| CastMethod::Flashback {
                semantic_digest: digest,
            },
        )
    }

    // The explicit parameters keep each cast receipt input distinct.
    #[allow(clippy::too_many_arguments)]
    fn cast_from_graveyard(
        &mut self,
        program: &AlternateZoneKeywordProgram,
        player: PlayerId,
        source: ObjectRef,
        cost: &AlternativeCost,
        modifier: FlashbackCostModifier,
        payment: CostPayment,
        legality: CastLegality,
        method: impl FnOnce(String) -> CastMethod,
    ) -> Result<CastEvidence, AlternateZoneRuntimeError> {
        if !legality.permits_cast(cost.retains_other_cast_costs) {
            return Err(AlternateZoneRuntimeError::CastIsNotLegal);
        }
        let mut staged = self.clone();
        staged.require_source(program, source, player, Zone::Graveyard)?;
        // CR 601.2a moves the card to the stack before costs are paid. The
        // cloned transaction makes this entire sequence atomic on failure.
        let change = staged.move_object(source, Zone::Stack, player, false)?;
        let cost_evidence = staged.pay_cost(cost, modifier, player, source.object_id, &payment)?;
        let method = method(program.semantic_digest().to_owned());
        staged.object_mut(change.after)?.cast_method = Some(method.clone());
        *self = staged;
        Ok(CastEvidence {
            player,
            source_zone_change: change,
            cost: Some(cost_evidence),
            method,
        })
    }

    pub fn leave_stack_after_flashback(
        &mut self,
        source: ObjectRef,
        requested_destination: Zone,
    ) -> Result<ZoneChangeEvidence, AlternateZoneRuntimeError> {
        if requested_destination == Zone::Stack {
            return Err(AlternateZoneRuntimeError::InvalidDestination);
        }
        let object = self.require_exact_object(source, Zone::Stack)?;
        if !matches!(object.cast_method, Some(CastMethod::Flashback { .. })) {
            return Err(AlternateZoneRuntimeError::ProgramActionMismatch);
        }
        self.move_object(
            source,
            Zone::Exile,
            object.controller.unwrap_or(object.owner),
            true,
        )
        .map(|change| ZoneChangeEvidence {
            requested_destination,
            replacement_applied: requested_destination != Zone::Exile,
            ..change
        })
    }

    pub fn resolve_suspended_creature_to_battlefield(
        &mut self,
        source: ObjectRef,
    ) -> Result<ZoneChangeEvidence, AlternateZoneRuntimeError> {
        let object = self.require_exact_object(source, Zone::Stack)?;
        let controller = object.controller.unwrap_or(object.owner);
        if !matches!(object.cast_method, Some(CastMethod::Suspend { .. })) {
            return Err(AlternateZoneRuntimeError::ProgramActionMismatch);
        }
        let grant = self
            .suspend_haste
            .get(&source.object_id)
            .filter(|grant| grant.object_ref == source)
            .cloned();
        let change = self.move_object(source, Zone::Battlefield, controller, false)?;
        if let Some(mut grant) = grant {
            grant.object_ref = change.after;
            self.suspend_haste.insert(source.object_id, grant);
        }
        Ok(change)
    }

    pub fn change_control(
        &mut self,
        permanent: ObjectRef,
        new_controller: PlayerId,
    ) -> Result<(), AlternateZoneRuntimeError> {
        let object = self.require_exact_object_mut(permanent, Zone::Battlefield)?;
        object.controller = Some(new_controller);
        if self
            .suspend_haste
            .get(&permanent.object_id)
            .is_some_and(|grant| {
                grant.object_ref == permanent && grant.controller != new_controller
            })
        {
            self.suspend_haste.remove(&permanent.object_id);
        }
        Ok(())
    }

    fn pay_cost(
        &mut self,
        cost: &AlternativeCost,
        modifier: FlashbackCostModifier,
        player: PlayerId,
        source_object: ObjectId,
        payment: &CostPayment,
    ) -> Result<CostEvidence, AlternateZoneRuntimeError> {
        if cost.x_constraint == VariableConstraint::AtLeastOne && payment.x_value == 0 {
            return Err(AlternateZoneRuntimeError::VariableXBelowMinimum);
        }
        if !cost_uses_x(cost) && payment.x_value != 0 {
            return Err(AlternateZoneRuntimeError::UnexpectedVariableX);
        }
        let generic_reduction = self.generic_reduction(player, modifier);
        let repeated_colorless = cost.additional.iter().find_map(|additional| {
            if let AdditionalCost::PayRepeatedColorless(amount) = additional {
                Some(*amount)
            } else {
                None
            }
        });
        let mana_units_spent = if let Some(mana) = &cost.mana {
            self.pay_mana(
                player,
                mana,
                payment.x_value,
                generic_reduction,
                &payment.mana_units,
            )?
        } else if repeated_colorless.is_none() {
            if !payment.mana_units.is_empty() {
                return Err(AlternateZoneRuntimeError::UnexpectedPaymentEvidence("mana"));
            }
            Vec::new()
        } else {
            Vec::new()
        };

        let mut expected_life = 0_u32;
        let mut expected_energy = 0_u32;
        let mut expected_colorless = 0_u32;
        let mut used_graveyard = false;
        let mut used_exile_controlled = false;
        let mut used_sacrifice = false;
        let mut used_tap = false;
        let mut used_discard = false;
        let mut used_loyalty = false;
        let mut used_behold = false;
        let mut cards_exiled_from_graveyard = Vec::new();
        let mut controlled_permanents_exiled = Vec::new();
        let mut permanents_sacrificed = Vec::new();
        let mut permanents_tapped = Vec::new();
        let mut cards_discarded = Vec::new();
        let mut loyalty_removed = Vec::new();
        let mut behold_choices = Vec::new();

        for additional in &cost.additional {
            match additional {
                AdditionalCost::PayLife(amount) => expected_life += *amount,
                AdditionalCost::PayEnergy(amount) => expected_energy += *amount,
                AdditionalCost::PayRepeatedColorless(amount) => expected_colorless += *amount,
                AdditionalCost::ExileCardsFromYourGraveyard {
                    amount,
                    filter,
                    other_than_source,
                } => {
                    if used_graveyard {
                        return Err(AlternateZoneRuntimeError::UnsupportedCombinedCost);
                    }
                    used_graveyard = true;
                    let expected = amount.resolve(payment.x_value) as usize;
                    if payment.exile_from_graveyard.len() != expected {
                        return Err(AlternateZoneRuntimeError::WrongSelectionCount {
                            category: "graveyard exile",
                            expected,
                            actual: payment.exile_from_graveyard.len(),
                        });
                    }
                    self.validate_graveyard_cards(
                        player,
                        source_object,
                        &payment.exile_from_graveyard,
                        *filter,
                        *other_than_source,
                    )?;
                    cards_exiled_from_graveyard = self.move_many(
                        &payment.exile_from_graveyard,
                        Zone::Graveyard,
                        Zone::Exile,
                        player,
                    )?;
                }
                AdditionalCost::ExileAnyNumberFromYourGraveyardWithCombinedCardTypesAtLeast {
                    distinct_card_types,
                    other_than_source,
                } => {
                    if used_graveyard {
                        return Err(AlternateZoneRuntimeError::UnsupportedCombinedCost);
                    }
                    used_graveyard = true;
                    self.validate_graveyard_cards(
                        player,
                        source_object,
                        &payment.exile_from_graveyard,
                        GraveyardCardFilter::Any,
                        *other_than_source,
                    )?;
                    let card_types = payment
                        .exile_from_graveyard
                        .iter()
                        .flat_map(|object| {
                            self.objects
                                .get(&object.object_id)
                                .into_iter()
                                .flat_map(|card| {
                                    card.card_types.iter().filter(countable_card_type).cloned()
                                })
                        })
                        .collect::<BTreeSet<_>>();
                    if card_types.len() < *distinct_card_types as usize {
                        return Err(AlternateZoneRuntimeError::CombinedCardTypesTooSmall {
                            required: *distinct_card_types,
                            actual: card_types.len() as u32,
                        });
                    }
                    cards_exiled_from_graveyard = self.move_many(
                        &payment.exile_from_graveyard,
                        Zone::Graveyard,
                        Zone::Exile,
                        player,
                    )?;
                }
                AdditionalCost::ExileControlledPermanent { amount, filter } => {
                    if used_exile_controlled {
                        return Err(AlternateZoneRuntimeError::UnsupportedCombinedCost);
                    }
                    used_exile_controlled = true;
                    self.validate_controlled_permanents(
                        player,
                        &payment.exile_controlled_permanents,
                        *amount as usize,
                        *filter,
                        false,
                    )?;
                    controlled_permanents_exiled = self.move_many(
                        &payment.exile_controlled_permanents,
                        Zone::Battlefield,
                        Zone::Exile,
                        player,
                    )?;
                }
                AdditionalCost::SacrificeControlledPermanent { amount, filter } => {
                    if used_sacrifice {
                        return Err(AlternateZoneRuntimeError::UnsupportedCombinedCost);
                    }
                    used_sacrifice = true;
                    self.validate_controlled_permanents(
                        player,
                        &payment.sacrifice_permanents,
                        amount.resolve(payment.x_value) as usize,
                        *filter,
                        false,
                    )?;
                    permanents_sacrificed = self.move_many(
                        &payment.sacrifice_permanents,
                        Zone::Battlefield,
                        Zone::Graveyard,
                        player,
                    )?;
                }
                AdditionalCost::TapUntappedControlledPermanent { amount, filter } => {
                    if used_tap {
                        return Err(AlternateZoneRuntimeError::UnsupportedCombinedCost);
                    }
                    used_tap = true;
                    self.validate_controlled_permanents(
                        player,
                        &payment.tap_permanents,
                        *amount as usize,
                        *filter,
                        true,
                    )?;
                    for object_ref in &payment.tap_permanents {
                        self.object_mut(*object_ref)?.tapped = true;
                    }
                    permanents_tapped = payment.tap_permanents.clone();
                }
                AdditionalCost::DiscardCards { amount } => {
                    if used_discard {
                        return Err(AlternateZoneRuntimeError::UnsupportedCombinedCost);
                    }
                    used_discard = true;
                    let expected = amount.resolve(payment.x_value) as usize;
                    if payment.discard_cards.len() != expected {
                        return Err(AlternateZoneRuntimeError::WrongSelectionCount {
                            category: "discard",
                            expected,
                            actual: payment.discard_cards.len(),
                        });
                    }
                    self.validate_owned_cards(player, &payment.discard_cards, Zone::Hand, None)?;
                    cards_discarded = self.move_many(
                        &payment.discard_cards,
                        Zone::Hand,
                        Zone::Graveyard,
                        player,
                    )?;
                }
                AdditionalCost::RemoveLoyaltyCountersFromControlledPlaneswalkers {
                    amount,
                    minimum_one,
                } => {
                    if used_loyalty {
                        return Err(AlternateZoneRuntimeError::UnsupportedCombinedCost);
                    }
                    used_loyalty = true;
                    let expected = amount.resolve(payment.x_value);
                    if *minimum_one && expected == 0 {
                        return Err(AlternateZoneRuntimeError::VariableXBelowMinimum);
                    }
                    self.remove_loyalty(player, &payment.remove_loyalty, expected)?;
                    loyalty_removed = payment.remove_loyalty.clone();
                }
                AdditionalCost::BeholdSubtype { amount, subtype } => {
                    if used_behold {
                        return Err(AlternateZoneRuntimeError::UnsupportedCombinedCost);
                    }
                    used_behold = true;
                    self.validate_behold(player, &payment.behold, *amount as usize, subtype)?;
                    behold_choices = payment.behold.clone();
                }
            }
        }

        if payment.life != expected_life {
            return Err(AlternateZoneRuntimeError::WrongLifePayment {
                expected: expected_life,
                actual: payment.life,
            });
        }
        if payment.energy != expected_energy {
            return Err(AlternateZoneRuntimeError::WrongEnergyPayment {
                expected: expected_energy,
                actual: payment.energy,
            });
        }
        if !used_graveyard && !payment.exile_from_graveyard.is_empty() {
            return Err(AlternateZoneRuntimeError::UnexpectedPaymentEvidence(
                "graveyard exile",
            ));
        }
        if !used_exile_controlled && !payment.exile_controlled_permanents.is_empty() {
            return Err(AlternateZoneRuntimeError::UnexpectedPaymentEvidence(
                "controlled permanent exile",
            ));
        }
        if !used_sacrifice && !payment.sacrifice_permanents.is_empty() {
            return Err(AlternateZoneRuntimeError::UnexpectedPaymentEvidence(
                "sacrifice",
            ));
        }
        if !used_tap && !payment.tap_permanents.is_empty() {
            return Err(AlternateZoneRuntimeError::UnexpectedPaymentEvidence("tap"));
        }
        if !used_discard && !payment.discard_cards.is_empty() {
            return Err(AlternateZoneRuntimeError::UnexpectedPaymentEvidence(
                "discard",
            ));
        }
        if !used_loyalty && !payment.remove_loyalty.is_empty() {
            return Err(AlternateZoneRuntimeError::UnexpectedPaymentEvidence(
                "loyalty",
            ));
        }
        if !used_behold && !payment.behold.is_empty() {
            return Err(AlternateZoneRuntimeError::UnexpectedPaymentEvidence(
                "behold",
            ));
        }

        if expected_life > 0 {
            let player_state = self.player_mut(player)?;
            if player_state.life <= expected_life as i32 {
                return Err(AlternateZoneRuntimeError::CannotPayLife);
            }
            player_state.life -= expected_life as i32;
        }
        if expected_energy > 0 {
            let player_state = self.player_mut(player)?;
            if player_state.energy < expected_energy {
                return Err(AlternateZoneRuntimeError::InsufficientEnergy);
            }
            player_state.energy -= expected_energy;
        }
        if expected_colorless > 0 {
            self.pay_repeated_colorless(player, expected_colorless, &payment.mana_units)?;
        }

        Ok(CostEvidence {
            x_value: payment.x_value,
            mana_units_spent: if expected_colorless > 0 {
                payment.mana_units.clone()
            } else {
                mana_units_spent
            },
            life_paid: expected_life,
            energy_paid: expected_energy,
            cards_exiled_from_graveyard,
            controlled_permanents_exiled,
            permanents_sacrificed,
            permanents_tapped,
            cards_discarded,
            loyalty_removed,
            behold_choices,
            generic_reduction_applied: generic_reduction,
        })
    }

    fn pay_mana(
        &mut self,
        player: PlayerId,
        cost: &ManaCost,
        x_value: u32,
        generic_reduction: u32,
        selected: &[ManaUnitId],
    ) -> Result<Vec<ManaUnitId>, AlternateZoneRuntimeError> {
        let mut fixed = Vec::<ManaRequirement>::new();
        let mut generic = 0_u32;
        for symbol in &cost.symbols {
            match symbol {
                ManaSymbol::Generic(amount) => generic = generic.saturating_add(*amount),
                ManaSymbol::VariableX => generic = generic.saturating_add(x_value),
                ManaSymbol::White => fixed.push(ManaRequirement::Color(ManaColor::White)),
                ManaSymbol::Blue => fixed.push(ManaRequirement::Color(ManaColor::Blue)),
                ManaSymbol::Black => fixed.push(ManaRequirement::Color(ManaColor::Black)),
                ManaSymbol::Red => fixed.push(ManaRequirement::Color(ManaColor::Red)),
                ManaSymbol::Green => fixed.push(ManaRequirement::Color(ManaColor::Green)),
                ManaSymbol::Colorless => fixed.push(ManaRequirement::Color(ManaColor::Colorless)),
                ManaSymbol::Snow => fixed.push(ManaRequirement::Snow),
                ManaSymbol::Hybrid(first, second) => {
                    fixed.push(ManaRequirement::Hybrid(*first, *second))
                }
            }
        }
        generic = generic.saturating_sub(generic_reduction);
        let expected = fixed.len() + generic as usize;
        if selected.len() != expected {
            return Err(AlternateZoneRuntimeError::WrongSelectionCount {
                category: "mana",
                expected,
                actual: selected.len(),
            });
        }
        require_distinct(selected.iter().copied(), "mana")?;
        let player_state = self.player(player)?;
        let units = selected
            .iter()
            .map(|id| {
                player_state
                    .mana_pool
                    .get(id)
                    .copied()
                    .ok_or(AlternateZoneRuntimeError::MissingManaUnit(*id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !assign_fixed_mana(&fixed, &units, 0, &mut BTreeSet::new()) {
            return Err(AlternateZoneRuntimeError::ManaPaymentDoesNotMatchCost);
        }
        let player_state = self.player_mut(player)?;
        for id in selected {
            player_state.mana_pool.remove(id);
        }
        Ok(selected.to_vec())
    }

    fn pay_repeated_colorless(
        &mut self,
        player: PlayerId,
        amount: u32,
        selected: &[ManaUnitId],
    ) -> Result<(), AlternateZoneRuntimeError> {
        if selected.len() != amount as usize {
            return Err(AlternateZoneRuntimeError::WrongSelectionCount {
                category: "colorless mana",
                expected: amount as usize,
                actual: selected.len(),
            });
        }
        require_distinct(selected.iter().copied(), "colorless mana")?;
        let player_state = self.player(player)?;
        if selected.iter().any(|id| {
            player_state
                .mana_pool
                .get(id)
                .is_none_or(|unit| unit.color != ManaColor::Colorless)
        }) {
            return Err(AlternateZoneRuntimeError::ManaPaymentDoesNotMatchCost);
        }
        let player_state = self.player_mut(player)?;
        for id in selected {
            player_state.mana_pool.remove(id);
        }
        Ok(())
    }

    fn generic_reduction(&self, player: PlayerId, modifier: FlashbackCostModifier) -> u32 {
        match modifier {
            FlashbackCostModifier::None => 0,
            FlashbackCostModifier::ReduceGenericByGreatestOwnedCommanderManaValueOnBattlefieldOrCommandZone => self
                .objects
                .values()
                .filter(|object| {
                    object.owner == player
                        && object.is_commander
                        && matches!(object.zone, Zone::Battlefield | Zone::Command)
                })
                .map(|object| object.mana_value)
                .max()
                .unwrap_or(0),
        }
    }

    fn validate_graveyard_cards(
        &self,
        player: PlayerId,
        source_object: ObjectId,
        selected: &[ObjectRef],
        filter: GraveyardCardFilter,
        other_than_source: bool,
    ) -> Result<(), AlternateZoneRuntimeError> {
        require_distinct(selected.iter().copied(), "graveyard cards")?;
        for object_ref in selected {
            if other_than_source && object_ref.object_id == source_object {
                return Err(AlternateZoneRuntimeError::SourceUsedToPayItsOwnCost);
            }
            let object = self.require_exact_object(*object_ref, Zone::Graveyard)?;
            if object.owner != player {
                return Err(AlternateZoneRuntimeError::WrongOwner);
            }
            if filter == GraveyardCardFilter::Blue && !object.colors.contains(&ManaColor::Blue) {
                return Err(AlternateZoneRuntimeError::SelectionDoesNotMatchFilter);
            }
        }
        Ok(())
    }

    fn validate_controlled_permanents(
        &self,
        player: PlayerId,
        selected: &[ObjectRef],
        expected: usize,
        filter: PermanentFilter,
        require_untapped: bool,
    ) -> Result<(), AlternateZoneRuntimeError> {
        if selected.len() != expected {
            return Err(AlternateZoneRuntimeError::WrongSelectionCount {
                category: "controlled permanents",
                expected,
                actual: selected.len(),
            });
        }
        require_distinct(selected.iter().copied(), "controlled permanents")?;
        for object_ref in selected {
            let object = self.require_exact_object(*object_ref, Zone::Battlefield)?;
            if object.controller != Some(player) {
                return Err(AlternateZoneRuntimeError::WrongController);
            }
            if !object.matches_filter(filter) {
                return Err(AlternateZoneRuntimeError::SelectionDoesNotMatchFilter);
            }
            if require_untapped && object.tapped {
                return Err(AlternateZoneRuntimeError::PermanentAlreadyTapped(
                    *object_ref,
                ));
            }
        }
        Ok(())
    }

    fn validate_owned_cards(
        &self,
        player: PlayerId,
        selected: &[ObjectRef],
        zone: Zone,
        filter: Option<PermanentFilter>,
    ) -> Result<(), AlternateZoneRuntimeError> {
        require_distinct(selected.iter().copied(), "cards")?;
        for object_ref in selected {
            let object = self.require_exact_object(*object_ref, zone)?;
            if object.owner != player {
                return Err(AlternateZoneRuntimeError::WrongOwner);
            }
            if filter.is_some_and(|filter| !object.matches_filter(filter)) {
                return Err(AlternateZoneRuntimeError::SelectionDoesNotMatchFilter);
            }
        }
        Ok(())
    }

    fn remove_loyalty(
        &mut self,
        player: PlayerId,
        selections: &[LoyaltyCounterPayment],
        expected: u32,
    ) -> Result<(), AlternateZoneRuntimeError> {
        require_distinct(
            selections.iter().map(|payment| payment.planeswalker),
            "planeswalkers",
        )?;
        let actual = selections.iter().map(|payment| payment.amount).sum::<u32>();
        if actual != expected || selections.iter().any(|payment| payment.amount == 0) {
            return Err(AlternateZoneRuntimeError::WrongLoyaltyPayment { expected, actual });
        }
        for payment in selections {
            let object = self.require_exact_object(payment.planeswalker, Zone::Battlefield)?;
            if object.controller != Some(player)
                || !object.card_types.contains(&CardType::Planeswalker)
            {
                return Err(AlternateZoneRuntimeError::SelectionDoesNotMatchFilter);
            }
            let loyalty = object.counters.get("loyalty").copied().unwrap_or(0);
            if loyalty < payment.amount {
                return Err(AlternateZoneRuntimeError::InsufficientLoyalty);
            }
        }
        for payment in selections {
            let object = self.object_mut(payment.planeswalker)?;
            *object.counters.entry("loyalty".into()).or_default() -= payment.amount;
        }
        Ok(())
    }

    fn validate_behold(
        &self,
        player: PlayerId,
        selections: &[BeholdChoice],
        expected: usize,
        subtype: &str,
    ) -> Result<(), AlternateZoneRuntimeError> {
        if selections.len() != expected {
            return Err(AlternateZoneRuntimeError::WrongSelectionCount {
                category: "behold",
                expected,
                actual: selections.len(),
            });
        }
        for selection in selections {
            let (object_ref, zone) = match selection {
                BeholdChoice::ControlledPermanent(object) => (*object, Zone::Battlefield),
                BeholdChoice::RevealFromHand(object) => (*object, Zone::Hand),
            };
            let object = self.require_exact_object(object_ref, zone)?;
            match selection {
                BeholdChoice::ControlledPermanent(_) if object.controller != Some(player) => {
                    return Err(AlternateZoneRuntimeError::WrongController);
                }
                BeholdChoice::RevealFromHand(_) if object.owner != player => {
                    return Err(AlternateZoneRuntimeError::WrongOwner);
                }
                _ => {}
            }
            if !object.has_subtype(subtype) {
                return Err(AlternateZoneRuntimeError::SelectionDoesNotMatchFilter);
            }
        }
        Ok(())
    }

    fn move_many(
        &mut self,
        selected: &[ObjectRef],
        expected_zone: Zone,
        destination: Zone,
        actor: PlayerId,
    ) -> Result<Vec<ObjectRef>, AlternateZoneRuntimeError> {
        let mut after = Vec::new();
        for object_ref in selected {
            self.require_exact_object(*object_ref, expected_zone)?;
            let change = if expected_zone == Zone::Battlefield {
                self.move_battlefield_object_with_unearth_replacement(
                    *object_ref,
                    destination,
                    actor,
                )?
            } else {
                self.move_object(*object_ref, destination, actor, false)?
            };
            after.push(change.after);
        }
        Ok(after)
    }

    fn move_battlefield_object_with_unearth_replacement(
        &mut self,
        permanent: ObjectRef,
        requested_destination: Zone,
        actor: PlayerId,
    ) -> Result<ZoneChangeEvidence, AlternateZoneRuntimeError> {
        self.require_exact_object(permanent, Zone::Battlefield)?;
        let replacement_active = self
            .unearthed
            .get(&permanent.object_id)
            .is_some_and(|record| record.permanent == permanent);
        let replacement_applied = replacement_active && requested_destination != Zone::Exile;
        let actual_destination = if replacement_active {
            Zone::Exile
        } else {
            requested_destination
        };
        let change = self.move_object(permanent, actual_destination, actor, replacement_applied)?;
        if replacement_active {
            self.unearthed.remove(&permanent.object_id);
        }
        Ok(ZoneChangeEvidence {
            requested_destination,
            actual_destination,
            replacement_applied,
            ..change
        })
    }

    fn require_source(
        &self,
        program: &AlternateZoneKeywordProgram,
        source: ObjectRef,
        player: PlayerId,
        zone: Zone,
    ) -> Result<&TrackedCard, AlternateZoneRuntimeError> {
        let object = self.require_exact_object(source, zone)?;
        if object.owner != player {
            return Err(AlternateZoneRuntimeError::WrongOwner);
        }
        if object.source_context() != program.source_context() {
            return Err(AlternateZoneRuntimeError::SourceSemanticContextMismatch);
        }
        Ok(object)
    }

    fn require_exact_object(
        &self,
        object_ref: ObjectRef,
        zone: Zone,
    ) -> Result<&TrackedCard, AlternateZoneRuntimeError> {
        let object = self.objects.get(&object_ref.object_id).ok_or(
            AlternateZoneRuntimeError::MissingObject(object_ref.object_id),
        )?;
        if object.object_ref != object_ref {
            return Err(AlternateZoneRuntimeError::StaleIncarnation(object_ref));
        }
        if object.zone != zone {
            return Err(AlternateZoneRuntimeError::WrongZone {
                object: object_ref,
                expected: zone,
                actual: object.zone,
            });
        }
        Ok(object)
    }

    fn require_exact_object_mut(
        &mut self,
        object_ref: ObjectRef,
        zone: Zone,
    ) -> Result<&mut TrackedCard, AlternateZoneRuntimeError> {
        let object = self.objects.get_mut(&object_ref.object_id).ok_or(
            AlternateZoneRuntimeError::MissingObject(object_ref.object_id),
        )?;
        if object.object_ref != object_ref {
            return Err(AlternateZoneRuntimeError::StaleIncarnation(object_ref));
        }
        if object.zone != zone {
            return Err(AlternateZoneRuntimeError::WrongZone {
                object: object_ref,
                expected: zone,
                actual: object.zone,
            });
        }
        Ok(object)
    }

    fn object_mut(
        &mut self,
        object_ref: ObjectRef,
    ) -> Result<&mut TrackedCard, AlternateZoneRuntimeError> {
        let object = self.objects.get_mut(&object_ref.object_id).ok_or(
            AlternateZoneRuntimeError::MissingObject(object_ref.object_id),
        )?;
        if object.object_ref != object_ref {
            return Err(AlternateZoneRuntimeError::StaleIncarnation(object_ref));
        }
        Ok(object)
    }

    fn move_object(
        &mut self,
        object_ref: ObjectRef,
        destination: Zone,
        actor: PlayerId,
        replacement_applied: bool,
    ) -> Result<ZoneChangeEvidence, AlternateZoneRuntimeError> {
        let object = self.objects.get_mut(&object_ref.object_id).ok_or(
            AlternateZoneRuntimeError::MissingObject(object_ref.object_id),
        )?;
        if object.object_ref != object_ref {
            return Err(AlternateZoneRuntimeError::StaleIncarnation(object_ref));
        }
        let from = object.zone;
        let next = ObjectRef {
            object_id: object_ref.object_id,
            incarnation_id: IncarnationId(
                object_ref
                    .incarnation_id
                    .0
                    .checked_add(1)
                    .ok_or(AlternateZoneRuntimeError::IncarnationOverflow)?,
            ),
        };
        object.object_ref = next;
        object.zone = destination;
        object.controller = matches!(destination, Zone::Battlefield | Zone::Stack).then_some(actor);
        object.tapped = false;
        object.counters.clear();
        if destination != Zone::Stack {
            object.cast_method = None;
        }
        Ok(ZoneChangeEvidence {
            before: object_ref,
            after: next,
            from,
            requested_destination: destination,
            actual_destination: destination,
            replacement_applied,
        })
    }

    fn allocate_pending_id(&mut self) -> u64 {
        let id = self.next_pending_id;
        self.next_pending_id = self.next_pending_id.saturating_add(1);
        id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManaRequirement {
    Color(ManaColor),
    Snow,
    Hybrid(ManaColor, ManaColor),
}

fn cost_uses_x(cost: &AlternativeCost) -> bool {
    cost.mana
        .as_ref()
        .is_some_and(|mana| mana.symbols.contains(&ManaSymbol::VariableX))
        || cost.additional.iter().any(|additional| {
            matches!(
                additional,
                AdditionalCost::ExileCardsFromYourGraveyard {
                    amount: Amount::VariableX,
                    ..
                } | AdditionalCost::SacrificeControlledPermanent {
                    amount: Amount::VariableX,
                    ..
                } | AdditionalCost::DiscardCards {
                    amount: Amount::VariableX,
                } | AdditionalCost::RemoveLoyaltyCountersFromControlledPlaneswalkers {
                    amount: Amount::VariableX,
                    ..
                }
            )
        })
}

fn countable_card_type(card_type: &&CardType) -> bool {
    !matches!(card_type, CardType::Other(_))
}

fn assign_fixed_mana(
    requirements: &[ManaRequirement],
    units: &[ManaUnit],
    offset: usize,
    used: &mut BTreeSet<usize>,
) -> bool {
    if offset == requirements.len() {
        return true;
    }
    for (index, unit) in units.iter().enumerate() {
        if used.contains(&index) || !mana_unit_matches(unit, requirements[offset]) {
            continue;
        }
        used.insert(index);
        if assign_fixed_mana(requirements, units, offset + 1, used) {
            return true;
        }
        used.remove(&index);
    }
    false
}

fn mana_unit_matches(unit: &ManaUnit, requirement: ManaRequirement) -> bool {
    match requirement {
        ManaRequirement::Color(color) => unit.color == color,
        ManaRequirement::Snow => unit.snow_source,
        ManaRequirement::Hybrid(first, second) => unit.color == first || unit.color == second,
    }
}

fn require_distinct<T: Ord + Copy>(
    values: impl IntoIterator<Item = T>,
    category: &'static str,
) -> Result<(), AlternateZoneRuntimeError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(AlternateZoneRuntimeError::DuplicateSelection(category));
        }
    }
    Ok(())
}

fn validate_tracked_card(object: &TrackedCard) -> Result<(), AlternateZoneRuntimeError> {
    match object.zone {
        Zone::Battlefield | Zone::Stack if object.controller.is_none() => Err(
            AlternateZoneRuntimeError::MissingController(object.object_ref),
        ),
        Zone::Battlefield | Zone::Stack => Ok(()),
        _ if object.controller.is_some() => Err(
            AlternateZoneRuntimeError::ControllerOutsideControlledZone(object.object_ref),
        ),
        _ if object.tapped => Err(AlternateZoneRuntimeError::TappedOutsideBattlefield(
            object.object_ref,
        )),
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlternateZoneRuntimeError {
    DuplicatePlayer(PlayerId),
    MissingPlayer(PlayerId),
    DuplicateObject(ObjectId),
    MissingObject(ObjectId),
    MissingController(ObjectRef),
    ControllerOutsideControlledZone(ObjectRef),
    TappedOutsideBattlefield(ObjectRef),
    StaleIncarnation(ObjectRef),
    IncarnationOverflow,
    WrongZone {
        object: ObjectRef,
        expected: Zone,
        actual: Zone,
    },
    WrongOwner,
    WrongController,
    SourceSemanticContextMismatch,
    ProgramActionMismatch,
    IllegalSuspendSpecialAction,
    SorceryTimingRequired,
    CastIsNotLegal,
    NoLandPlayRemaining,
    VariableXBelowMinimum,
    UnexpectedVariableX,
    MissingPendingTrigger(PendingTriggerId),
    MissingPendingAbility(PendingAbilityId),
    TriggerNotDue,
    NotUnearthed(ObjectRef),
    InvalidDestination,
    MissingManaUnit(ManaUnitId),
    ManaPaymentDoesNotMatchCost,
    WrongSelectionCount {
        category: &'static str,
        expected: usize,
        actual: usize,
    },
    DuplicateSelection(&'static str),
    SelectionDoesNotMatchFilter,
    SourceUsedToPayItsOwnCost,
    CombinedCardTypesTooSmall {
        required: u32,
        actual: u32,
    },
    PermanentAlreadyTapped(ObjectRef),
    WrongLifePayment {
        expected: u32,
        actual: u32,
    },
    CannotPayLife,
    WrongEnergyPayment {
        expected: u32,
        actual: u32,
    },
    InsufficientEnergy,
    WrongLoyaltyPayment {
        expected: u32,
        actual: u32,
    },
    InsufficientLoyalty,
    UnexpectedPaymentEvidence(&'static str),
    UnsupportedCombinedCost,
}

impl fmt::Display for AlternateZoneRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AlternateZoneRuntimeError {}
