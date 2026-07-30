//! Content keyed cast modification and spell copy keyword programs.
//!
//! This module owns complete standalone Oracle clauses for Buyback, Entwine,
//! Splice onto Arcane, Overload, Replicate, and residual Storm forms. The
//! compiler accepts only reviewed complete clauses and preserves their exact
//! costs. The runtime retains cast, stack, object incarnation, copied choice,
//! target, and trigger evidence. It is deliberately not connected to the
//! production simulation until those evidence boundaries are supplied there.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const CAST_MODIFIER_KEYWORD_COMPILER_VERSION: &str = "cast-modifier-keyword-compiler-0.1";
pub const CAST_MODIFIER_KEYWORD_RUNTIME_VERSION: &str = "cast-modifier-keyword-runtime-0.1";
pub const CAST_MODIFIER_KEYWORD_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:400.7,601.2b,601.2f,601.2h,608.2,608.2f,707.10,702.27,702.42,702.47,702.96,702.56,702.40";

const BUYBACK_MANA_REMINDER_PREFIX: &str = "You may pay an additional ";
const BUYBACK_MANA_REMINDER_SUFFIX: &str =
    " as you cast this spell. If you do, put this card into your hand as it resolves.";
const ENTWINE_BOTH_REMINDER: &str = "Choose both if you pay the entwine cost.";
const ENTWINE_ALL_REMINDER: &str = "Choose all if you pay the entwine cost.";
const ENTWINE_ALL_OF_THEM_REMINDER: &str = "Choose all of them if you pay the entwine cost.";
const SPLICE_REMINDER: &str = "As you cast an Arcane spell, you may reveal this card from your hand and pay its splice cost. If you do, add this card's effects to that spell.";
const OVERLOAD_REMINDER: &str = "You may cast this spell for its overload cost. If you do, change \"target\" in its text to \"each.\"";
const OVERLOAD_LONG_REMINDER: &str = "You may cast this spell for its overload cost. If you do, change its text by replacing all instances of \"target\" with \"each.\"";
const REPLICATE_REMINDER: &str = "When you cast this spell, copy it for each time you paid its replicate cost. You may choose new targets for the copies.";
const REPLICATE_NO_TARGET_REMINDER: &str =
    "When you cast this spell, copy it for each time you paid its replicate cost.";
const REPLICATE_SINGULAR_TARGET_REMINDER: &str = "When you cast this spell, copy it for each time you paid its replicate cost. You may choose new targets for the copy.";
const REPLICATE_TOKEN_REMINDER: &str = "When you cast this spell, copy it for each time you paid its replicate cost. You may choose new targets for the copies. Copies become tokens.";
const STORM_REMINDER: &str = "When you cast this spell, copy it for each spell cast before it this turn. You may choose new targets for the copies.";
const STORM_NO_TARGET_REMINDER: &str =
    "When you cast this spell, copy it for each spell cast before it this turn.";
const STORM_TOKEN_REMINDER: &str = "When you cast this spell, copy it for each spell cast before it this turn. Copies become tokens.";
const STORM_THE_TOKEN_REMINDER: &str = "When you cast this spell, copy it for each spell cast before it this turn. The copies become tokens.";
const STORM_TARGET_TOKEN_REMINDER: &str = "When you cast this spell, copy it for each spell cast before it this turn. You may choose new targets for the copies. Copies become tokens.";
const STORM_TARGET_ENTER_TOKEN_REMINDER: &str = "When you cast this spell, copy it for each spell cast before it this turn. You may choose new targets for the copies. The copies enter as tokens.";

pub const fn cast_modifier_keyword_production_adapter_connected() -> bool {
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
pub struct CastEventId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TurnId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaymentId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaUnitId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbilityInstanceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetSlotId(pub u32);

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
            Self::White => "white",
            Self::Blue => "blue",
            Self::Black => "black",
            Self::Red => "red",
            Self::Green => "green",
            Self::Colorless => "colorless",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaSymbol {
    Generic(u32),
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
    VariableX,
}

impl ManaSymbol {
    fn stable_id(&self) -> String {
        match self {
            Self::Generic(amount) => format!("generic/{amount}"),
            Self::White => "white".to_owned(),
            Self::Blue => "blue".to_owned(),
            Self::Black => "black".to_owned(),
            Self::Red => "red".to_owned(),
            Self::Green => "green".to_owned(),
            Self::Colorless => "colorless".to_owned(),
            Self::VariableX => "x".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaCost {
    exact: String,
    symbols: Vec<ManaSymbol>,
}

impl ManaCost {
    pub fn exact(&self) -> &str {
        &self.exact
    }

    pub fn symbols(&self) -> &[ManaSymbol] {
        &self.symbols
    }

    fn stable_id(&self) -> String {
        format!(
            "exact={};symbols={}",
            self.exact,
            self.symbols
                .iter()
                .map(ManaSymbol::stable_id)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PermanentCostFilter {
    Land,
    Island,
    Mountain,
    BlueCreature,
    WhiteCreature,
    Dalek,
    Horror,
}

impl PermanentCostFilter {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Land => "land",
            Self::Island => "island",
            Self::Mountain => "mountain",
            Self::BlueCreature => "blue-creature",
            Self::WhiteCreature => "white-creature",
            Self::Dalek => "dalek",
            Self::Horror => "horror",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CastCostAtom {
    Mana(ManaCost),
    PayLife(u32),
    PayEnergy(u32),
    DiscardCards {
        amount: u32,
        random: bool,
    },
    SacrificePermanents {
        amount: u32,
        filter: PermanentCostFilter,
    },
    ExileCardsFromOwnGraveyard {
        amount: u32,
    },
    ReturnControlledCreatureToOwnersHand {
        filter: PermanentCostFilter,
    },
    TapUntappedControlledCreature {
        filter: PermanentCostFilter,
    },
    OpponentGainsLife(u32),
}

impl CastCostAtom {
    fn stable_id(&self) -> String {
        match self {
            Self::Mana(cost) => format!("mana/{}", cost.stable_id()),
            Self::PayLife(amount) => format!("pay-life/{amount}"),
            Self::PayEnergy(amount) => format!("pay-energy/{amount}"),
            Self::DiscardCards { amount, random } => {
                format!("discard/{amount}/random={random}")
            }
            Self::SacrificePermanents { amount, filter } => {
                format!("sacrifice/{amount}/{}", filter.stable_id())
            }
            Self::ExileCardsFromOwnGraveyard { amount } => {
                format!("exile-own-graveyard/{amount}")
            }
            Self::ReturnControlledCreatureToOwnersHand { filter } => {
                format!("return-controlled/{}/owners-hand", filter.stable_id())
            }
            Self::TapUntappedControlledCreature { filter } => {
                format!("tap-untapped-controlled/{}", filter.stable_id())
            }
            Self::OpponentGainsLife(amount) => format!("opponent-gains-life/{amount}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CastModifierCost {
    exact: String,
    atoms: Vec<CastCostAtom>,
}

impl CastModifierCost {
    pub fn exact(&self) -> &str {
        &self.exact
    }

    pub fn atoms(&self) -> &[CastCostAtom] {
        &self.atoms
    }

    fn stable_id(&self) -> String {
        format!(
            "exact={};atoms={}",
            self.exact,
            self.atoms
                .iter()
                .map(CastCostAtom::stable_id)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastModifierKeywordKind {
    Buyback {
        additional_cost: CastModifierCost,
    },
    Entwine {
        additional_cost: CastModifierCost,
    },
    SpliceOntoArcane {
        additional_cost: CastModifierCost,
    },
    Overload {
        alternative_cost: ManaCost,
    },
    Replicate {
        repeatable_additional_cost: CastModifierCost,
    },
    Storm,
}

impl CastModifierKeywordKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Buyback { .. } => "Buyback",
            Self::Entwine { .. } => "Entwine",
            Self::SpliceOntoArcane { .. } => "Splice onto Arcane",
            Self::Overload { .. } => "Overload",
            Self::Replicate { .. } => "Replicate",
            Self::Storm => "Storm",
        }
    }

    fn stable_id(&self) -> String {
        match self {
            Self::Buyback { additional_cost } => format!(
                "buyback/v1;cost={};additional=true;hand-replaces-resolving-stack-to-graveyard=true",
                additional_cost.stable_id()
            ),
            Self::Entwine { additional_cost } => format!(
                "entwine/v1;cost={};additional=true;paid-selects-all-modes=true",
                additional_cost.stable_id()
            ),
            Self::SpliceOntoArcane { additional_cost } => format!(
                "splice-onto-arcane/v1;cost={};additional=true;reveal-from-hand=true;copy-rules-text-except-splice=true;source-remains-in-hand=true",
                additional_cost.stable_id()
            ),
            Self::Overload { alternative_cost } => format!(
                "overload/v1;cost={};alternative=true;replace-every-target-word-with-each=true",
                alternative_cost.stable_id()
            ),
            Self::Replicate {
                repeatable_additional_cost,
            } => format!(
                "replicate/v1;cost={};repeatable-additional=true;cast-trigger=true;one-copy-per-payment=true;new-targets-per-copy=true",
                repeatable_additional_cost.stable_id()
            ),
            Self::Storm => "storm/v1;cast-trigger=true;copies=spells-cast-before-this-spell-this-turn;new-targets-per-copy=true".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastModifierKeywordProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    kind: CastModifierKeywordKind,
}

impl CastModifierKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &CastModifierKeywordKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        cast_modifier_keyword_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotCandidateClass {
    SupportedResidual,
    ExistingExactOwner,
    UnsupportedCompoundModifierOrReminder,
}

pub fn classify_snapshot_candidate(exact_source: &str) -> Option<SnapshotCandidateClass> {
    snapshot_family(exact_source)?;
    let Some(program) = compile_cast_modifier_keyword_program(exact_source) else {
        return Some(SnapshotCandidateClass::UnsupportedCompoundModifierOrReminder);
    };
    if existing_exact_owner(&program) {
        Some(SnapshotCandidateClass::ExistingExactOwner)
    } else {
        Some(SnapshotCandidateClass::SupportedResidual)
    }
}

fn snapshot_family(exact_source: &str) -> Option<&'static str> {
    if exact_source == "Buyback"
        || exact_source.starts_with("Buyback ")
        || exact_source.starts_with("Buyback\u{2014}")
    {
        Some("Buyback")
    } else if exact_source == "Entwine"
        || exact_source.starts_with("Entwine ")
        || exact_source.starts_with("Entwine\u{2014}")
    {
        Some("Entwine")
    } else if exact_source == "Splice onto Arcane"
        || exact_source.starts_with("Splice onto Arcane ")
        || exact_source.starts_with("Splice onto Arcane\u{2014}")
    {
        Some("Splice onto Arcane")
    } else if exact_source == "Overload"
        || exact_source.starts_with("Overload ")
        || exact_source.starts_with("Overload\u{2014}")
    {
        Some("Overload")
    } else if exact_source == "Replicate"
        || exact_source.starts_with("Replicate ")
        || exact_source.starts_with("Replicate\u{2014}")
    {
        Some("Replicate")
    } else if exact_source == "Storm" || exact_source.starts_with("Storm (") {
        Some("Storm")
    } else {
        None
    }
}

fn existing_exact_owner(program: &CastModifierKeywordProgram) -> bool {
    program.exact_source == format!("Storm ({STORM_REMINDER})")
}

/// Compiles one complete Oracle clause. Snapshot coordinates, card names,
/// Oracle IDs, row order, and snapshot hashes are intentionally not inputs.
pub fn compile_cast_modifier_keyword_program(
    exact_source: &str,
) -> Option<CastModifierKeywordProgram> {
    if !complete_single_line(exact_source) {
        return None;
    }
    // This value is derived from the clause itself. A caller-supplied
    // source-name normalization would make identity depend on the card name
    // and could change when the same clause moved in a future snapshot.
    let normalized_source = normalize_reviewed_clause(exact_source);
    let kind = parse_buyback(exact_source)
        .or_else(|| parse_entwine(exact_source))
        .or_else(|| parse_splice(exact_source))
        .or_else(|| parse_overload(exact_source))
        .or_else(|| parse_replicate(exact_source))
        .or_else(|| parse_storm(exact_source))?;
    let semantic_digest = semantic_digest(exact_source, &normalized_source, &kind);
    Some(CastModifierKeywordProgram {
        exact_source: exact_source.to_owned(),
        normalized_source,
        semantic_digest,
        kind,
    })
}

fn complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source == source.trim()
        && !source.contains(['\r', '\n'])
        && collapse_whitespace(source) == source
}

fn parse_buyback(source: &str) -> Option<CastModifierKeywordKind> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let cost = parse_keyword_cost(core, "Buyback", parse_buyback_nonmana_cost)?;
    let expected = match cost.atoms() {
        [CastCostAtom::Mana(mana)] => {
            format!("{BUYBACK_MANA_REMINDER_PREFIX}{}{BUYBACK_MANA_REMINDER_SUFFIX}", mana.exact())
        }
        [CastCostAtom::DiscardCards {
            amount: 2,
            random: false,
        }] => "You may discard two cards in addition to any other costs as you cast this spell. If you do, put this card into your hand as it resolves.".to_owned(),
        [
            CastCostAtom::PayLife(3),
            CastCostAtom::DiscardCards {
                amount: 1,
                random: true,
            },
        ] => "You may pay 3 life and discard a card at random in addition to any other costs as you cast this spell. If you do, put this card into your hand as it resolves.".to_owned(),
        [CastCostAtom::PayLife(4)] => "You may pay 4 life in addition to any other costs as you cast this spell. If you do, put this card into your hand as it resolves.".to_owned(),
        [CastCostAtom::SacrificePermanents {
            amount: 1,
            filter: PermanentCostFilter::Land,
        }] => "You may sacrifice a land in addition to any other costs as you cast this spell. If you do, put this card into your hand as it resolves.".to_owned(),
        [CastCostAtom::SacrificePermanents {
            amount: 3,
            filter: PermanentCostFilter::Island,
        }] => "You may sacrifice three Islands in addition to any other costs as you cast this spell. If you do, put this card into your hand as it resolves.".to_owned(),
        _ => return None,
    };
    (reminder == Some(expected.as_str())).then_some(CastModifierKeywordKind::Buyback {
        additional_cost: cost,
    })
}

fn parse_buyback_nonmana_cost(source: &str) -> Option<Vec<CastCostAtom>> {
    match source {
        "Discard two cards." => Some(vec![CastCostAtom::DiscardCards {
            amount: 2,
            random: false,
        }]),
        "Pay 3 life, Discard a card at random." => Some(vec![
            CastCostAtom::PayLife(3),
            CastCostAtom::DiscardCards {
                amount: 1,
                random: true,
            },
        ]),
        "Pay 4 life." => Some(vec![CastCostAtom::PayLife(4)]),
        "Sacrifice a land." => Some(vec![CastCostAtom::SacrificePermanents {
            amount: 1,
            filter: PermanentCostFilter::Land,
        }]),
        "Sacrifice three Islands." => Some(vec![CastCostAtom::SacrificePermanents {
            amount: 3,
            filter: PermanentCostFilter::Island,
        }]),
        _ => None,
    }
}

fn parse_entwine(source: &str) -> Option<CastModifierKeywordKind> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let cost = parse_keyword_cost(core, "Entwine", parse_entwine_nonmana_cost)?;
    let reminder = reminder?;
    [
        ENTWINE_BOTH_REMINDER,
        ENTWINE_ALL_REMINDER,
        ENTWINE_ALL_OF_THEM_REMINDER,
    ]
    .contains(&reminder)
    .then_some(CastModifierKeywordKind::Entwine {
        additional_cost: cost,
    })
}

fn parse_entwine_nonmana_cost(source: &str) -> Option<Vec<CastCostAtom>> {
    match source {
        "Sacrifice two lands." => Some(vec![CastCostAtom::SacrificePermanents {
            amount: 2,
            filter: PermanentCostFilter::Land,
        }]),
        "Sacrifice three lands." => Some(vec![CastCostAtom::SacrificePermanents {
            amount: 3,
            filter: PermanentCostFilter::Land,
        }]),
        _ => None,
    }
}

fn parse_splice(source: &str) -> Option<CastModifierKeywordKind> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let cost = parse_keyword_cost(core, "Splice onto Arcane", parse_splice_nonmana_cost)?;
    (reminder == Some(SPLICE_REMINDER)).then_some(CastModifierKeywordKind::SpliceOntoArcane {
        additional_cost: cost,
    })
}

fn parse_splice_nonmana_cost(source: &str) -> Option<Vec<CastCostAtom>> {
    match source {
        "An opponent gains 5 life." => Some(vec![CastCostAtom::OpponentGainsLife(5)]),
        "Exile four cards from your graveyard." => {
            Some(vec![CastCostAtom::ExileCardsFromOwnGraveyard { amount: 4 }])
        }
        "Return a blue creature you control to its owner's hand." => {
            Some(vec![CastCostAtom::ReturnControlledCreatureToOwnersHand {
                filter: PermanentCostFilter::BlueCreature,
            }])
        }
        "Sacrifice two Mountains." => Some(vec![CastCostAtom::SacrificePermanents {
            amount: 2,
            filter: PermanentCostFilter::Mountain,
        }]),
        "Tap an untapped white creature you control." => {
            Some(vec![CastCostAtom::TapUntappedControlledCreature {
                filter: PermanentCostFilter::WhiteCreature,
            }])
        }
        _ => None,
    }
}

fn parse_overload(source: &str) -> Option<CastModifierKeywordKind> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let cost_text = core.strip_prefix("Overload ")?;
    let alternative_cost = parse_mana_cost(cost_text)?;
    if let Some(reminder) = reminder
        && reminder != OVERLOAD_REMINDER
        && reminder != OVERLOAD_LONG_REMINDER
    {
        return None;
    }
    Some(CastModifierKeywordKind::Overload { alternative_cost })
}

fn parse_replicate(source: &str) -> Option<CastModifierKeywordKind> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let cost = parse_keyword_cost(core, "Replicate", parse_replicate_nonmana_cost)?;
    if let Some(reminder) = reminder
        && ![
            REPLICATE_REMINDER,
            REPLICATE_NO_TARGET_REMINDER,
            REPLICATE_SINGULAR_TARGET_REMINDER,
            REPLICATE_TOKEN_REMINDER,
        ]
        .contains(&reminder)
    {
        return None;
    }
    Some(CastModifierKeywordKind::Replicate {
        repeatable_additional_cost: cost,
    })
}

fn parse_replicate_nonmana_cost(source: &str) -> Option<Vec<CastCostAtom>> {
    match source {
        "Pay {E}{E}{E}." => Some(vec![CastCostAtom::PayEnergy(3)]),
        "Tap an untapped Dalek you control." => {
            Some(vec![CastCostAtom::TapUntappedControlledCreature {
                filter: PermanentCostFilter::Dalek,
            }])
        }
        "Tap an untapped Horror you control." => {
            Some(vec![CastCostAtom::TapUntappedControlledCreature {
                filter: PermanentCostFilter::Horror,
            }])
        }
        _ => None,
    }
}

fn parse_storm(source: &str) -> Option<CastModifierKeywordKind> {
    if source == "Storm" {
        return Some(CastModifierKeywordKind::Storm);
    }
    let (core, reminder) = split_trailing_parenthetical(source)?;
    if core != "Storm" {
        return None;
    }
    let reminder = reminder?;
    [
        STORM_REMINDER,
        STORM_NO_TARGET_REMINDER,
        STORM_TOKEN_REMINDER,
        STORM_THE_TOKEN_REMINDER,
        STORM_TARGET_TOKEN_REMINDER,
        STORM_TARGET_ENTER_TOKEN_REMINDER,
    ]
    .contains(&reminder)
    .then_some(CastModifierKeywordKind::Storm)
}

fn parse_keyword_cost(
    core: &str,
    keyword: &str,
    nonmana_parser: fn(&str) -> Option<Vec<CastCostAtom>>,
) -> Option<CastModifierCost> {
    if let Some(mana) = core.strip_prefix(&format!("{keyword} ")) {
        let mana = parse_mana_cost(mana)?;
        return Some(CastModifierCost {
            exact: mana.exact.clone(),
            atoms: vec![CastCostAtom::Mana(mana)],
        });
    }
    let nonmana = core.strip_prefix(&format!("{keyword}\u{2014}"))?;
    let atoms = nonmana_parser(nonmana)?;
    Some(CastModifierCost {
        exact: nonmana.to_owned(),
        atoms,
    })
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    if source.is_empty() || source.trim() != source {
        return None;
    }
    let mut symbols = Vec::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        if source.as_bytes().get(cursor).copied() != Some(b'{') {
            return None;
        }
        let relative_end = source[cursor + 1..].find('}')?;
        let end = cursor + 1 + relative_end;
        let token = &source[cursor + 1..end];
        if token.is_empty() || token.contains(['{', '}']) {
            return None;
        }
        let symbol = match token {
            "W" => ManaSymbol::White,
            "U" => ManaSymbol::Blue,
            "B" => ManaSymbol::Black,
            "R" => ManaSymbol::Red,
            "G" => ManaSymbol::Green,
            "C" => ManaSymbol::Colorless,
            "X" => ManaSymbol::VariableX,
            value
                if value.bytes().all(|byte| byte.is_ascii_digit())
                    && (value.len() == 1 || !value.starts_with('0')) =>
            {
                ManaSymbol::Generic(value.parse::<u32>().ok()?)
            }
            _ => return None,
        };
        symbols.push(symbol);
        cursor = end + 1;
    }
    (!symbols.is_empty()).then_some(ManaCost {
        exact: source.to_owned(),
        symbols,
    })
}

fn split_trailing_parenthetical(source: &str) -> Option<(&str, Option<&str>)> {
    if !source.ends_with(')') {
        if source.contains(['(', ')']) {
            return None;
        }
        return Some((source, None));
    }
    let mut depth = 0u32;
    let mut start = None;
    for (index, character) in source.char_indices().rev() {
        match character {
            ')' => depth = depth.checked_add(1)?,
            '(' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    start = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let start = start?;
    if start == 0 || !source[..start].ends_with(' ') {
        return None;
    }
    let core = source[..start].trim_end();
    let reminder = &source[start + 1..source.len() - 1];
    if core.is_empty() || reminder.is_empty() || core.contains(['(', ')']) {
        return None;
    }
    Some((core, Some(reminder)))
}

fn normalize_reviewed_clause(source: &str) -> String {
    let mut normalized = collapse_whitespace(&source.replace('\u{2019}', "'"));
    normalized = replace_ascii_case_insensitive(&normalized, "this permanent", "this object");
    normalized = replace_ascii_case_insensitive(&normalized, "this card", "this object");
    normalized = replace_ascii_case_insensitive(&normalized, "this spell", "this object spell");
    collapse_whitespace(&normalized)
}

fn collapse_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn replace_ascii_case_insensitive(source: &str, needle: &str, replacement: &str) -> String {
    let lower_source = source.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0usize;
    while let Some(relative) = lower_source[cursor..].find(&lower_needle) {
        let start = cursor + relative;
        let end = start + needle.len();
        output.push_str(&source[cursor..start]);
        output.push_str(replacement);
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    output
}

fn semantic_digest(
    exact_source: &str,
    normalized_source: &str,
    kind: &CastModifierKeywordKind,
) -> String {
    let mut hasher = Sha256::new();
    for component in [
        "cast-modifier-keyword-content/v1",
        CAST_MODIFIER_KEYWORD_COMPILER_VERSION,
        CAST_MODIFIER_KEYWORD_RUNTIME_VERSION,
        CAST_MODIFIER_KEYWORD_RULES_CONTEXT_VERSION,
        exact_source,
        normalized_source,
        &kind.stable_id(),
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostPaymentRole {
    Additional,
    Alternative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManaRequirement {
    Any,
    Colored(ManaColor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManaUnitPaymentEvidence {
    pub unit_id: ManaUnitId,
    pub produced: ManaColor,
    pub requirement_index: usize,
    pub spending_restriction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaCostPaymentEvidence {
    pub printed_cost: ManaCost,
    pub chosen_x: u32,
    /// Requirements after all increases, reductions, and alternative payment
    /// rules have been applied by the complete cost engine.
    pub effective_requirements: Vec<ManaRequirement>,
    pub units: Vec<ManaUnitPaymentEvidence>,
    /// Empty means no modifier changed the printed mana requirement.
    pub ordered_cost_modifier_semantic_digests: Vec<String>,
    pub modifier_ledger_complete: bool,
    pub spending_restrictions_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaidObjectEvidence {
    pub before: ObjectRef,
    pub after: ObjectRef,
    pub owner: PlayerId,
    pub controller_before: PlayerId,
    pub from_zone: Zone,
    /// The unmodified event required by the cost.
    pub requested_to_zone: Zone,
    /// The actual destination after replacement effects.
    pub to_zone: Zone,
    pub ordered_zone_change_replacement_semantic_digests: Vec<String>,
    pub zone_change_replacements_complete: bool,
    pub colors: BTreeSet<ManaColor>,
    pub card_types: BTreeSet<String>,
    pub subtypes: BTreeSet<String>,
    pub tapped_before: bool,
    pub tapped_after: bool,
    pub characteristics_complete: bool,
}

impl PaidObjectEvidence {
    fn has_type(&self, value: &str) -> bool {
        self.card_types
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(value))
    }

    fn has_subtype(&self, value: &str) -> bool {
        self.subtypes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(value))
    }

    fn matches_filter(&self, filter: PermanentCostFilter) -> bool {
        match filter {
            PermanentCostFilter::Land => self.has_type("land"),
            PermanentCostFilter::Island => self.has_subtype("island"),
            PermanentCostFilter::Mountain => self.has_subtype("mountain"),
            PermanentCostFilter::BlueCreature => {
                self.has_type("creature") && self.colors.contains(&ManaColor::Blue)
            }
            PermanentCostFilter::WhiteCreature => {
                self.has_type("creature") && self.colors.contains(&ManaColor::White)
            }
            PermanentCostFilter::Dalek => self.has_type("creature") && self.has_subtype("dalek"),
            PermanentCostFilter::Horror => self.has_type("creature") && self.has_subtype("horror"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostAtomPaymentEvidence {
    Mana(ManaCostPaymentEvidence),
    Life {
        player: PlayerId,
        before: i32,
        after: i32,
        amount: u32,
        payment_cannot_be_prevented: bool,
    },
    Energy {
        player: PlayerId,
        before: u32,
        after: u32,
        amount: u32,
    },
    Discard {
        cards: Vec<PaidObjectEvidence>,
        random_selection: bool,
        random_selection_complete: bool,
    },
    Sacrifice {
        permanents: Vec<PaidObjectEvidence>,
        selection_complete: bool,
    },
    ExileFromOwnGraveyard {
        cards: Vec<PaidObjectEvidence>,
        selection_complete: bool,
    },
    ReturnControlledToOwnersHand {
        permanent: PaidObjectEvidence,
    },
    TapUntappedControlled {
        permanent: PaidObjectEvidence,
    },
    OpponentGainsLife {
        opponent: PlayerId,
        before: i32,
        after: i32,
        amount: u32,
        opponent_relationship_checked: bool,
        life_change_replacements_complete: bool,
        ordered_life_change_replacement_semantic_digests: Vec<String>,
        final_life_change_semantic_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastCostPaymentEvidence {
    pub payment_id: PaymentId,
    pub payer: PlayerId,
    pub ability_instance_id: AbilityInstanceId,
    pub program_semantic_digest: String,
    pub role: CostPaymentRole,
    pub declared_cost: CastModifierCost,
    pub atoms: Vec<CostAtomPaymentEvidence>,
    pub all_cost_components_complete: bool,
}

fn validate_cost_payment(
    payment: &CastCostPaymentEvidence,
    payer: PlayerId,
    ability_instance_id: AbilityInstanceId,
    program: &CastModifierKeywordProgram,
    expected_cost: &CastModifierCost,
    expected_role: CostPaymentRole,
) -> Result<(), CastModifierRuntimeError> {
    if payment.payer != payer {
        return Err(CastModifierRuntimeError::WrongPayer {
            expected: payer,
            actual: payment.payer,
        });
    }
    if payment.ability_instance_id != ability_instance_id
        || payment.program_semantic_digest != program.semantic_digest()
    {
        return Err(CastModifierRuntimeError::PaymentBindingMismatch);
    }
    if payment.role != expected_role || payment.declared_cost != *expected_cost {
        return Err(CastModifierRuntimeError::WrongCostOrRole);
    }
    if !payment.all_cost_components_complete || payment.atoms.len() != expected_cost.atoms.len() {
        return Err(CastModifierRuntimeError::IncompleteCostPayment);
    }
    for (expected, actual) in expected_cost.atoms.iter().zip(&payment.atoms) {
        validate_cost_atom(expected, actual, payer)?;
    }
    Ok(())
}

fn validate_cost_atom(
    expected: &CastCostAtom,
    actual: &CostAtomPaymentEvidence,
    payer: PlayerId,
) -> Result<(), CastModifierRuntimeError> {
    match (expected, actual) {
        (CastCostAtom::Mana(cost), CostAtomPaymentEvidence::Mana(payment)) => {
            validate_mana_payment(cost, payment)
        }
        (
            CastCostAtom::PayLife(expected_amount),
            CostAtomPaymentEvidence::Life {
                player,
                before,
                after,
                amount,
                payment_cannot_be_prevented,
            },
        ) if *player == payer
            && amount == expected_amount
            && *payment_cannot_be_prevented
            && before.checked_sub(*expected_amount as i32) == Some(*after)
            && *after >= 0 =>
        {
            Ok(())
        }
        (
            CastCostAtom::PayEnergy(expected_amount),
            CostAtomPaymentEvidence::Energy {
                player,
                before,
                after,
                amount,
            },
        ) if *player == payer
            && amount == expected_amount
            && before.checked_sub(*expected_amount) == Some(*after) =>
        {
            Ok(())
        }
        (
            CastCostAtom::DiscardCards { amount, random },
            CostAtomPaymentEvidence::Discard {
                cards,
                random_selection,
                random_selection_complete,
            },
        ) if cards.len() == *amount as usize
            && random_selection == random
            && (!random || *random_selection_complete) =>
        {
            validate_distinct_paid_objects(cards)?;
            for card in cards {
                validate_zone_change(card, payer, Zone::Hand, Zone::Graveyard)?;
            }
            Ok(())
        }
        (
            CastCostAtom::SacrificePermanents { amount, filter },
            CostAtomPaymentEvidence::Sacrifice {
                permanents,
                selection_complete,
            },
        ) if permanents.len() == *amount as usize && *selection_complete => {
            validate_distinct_paid_objects(permanents)?;
            for permanent in permanents {
                validate_zone_change(permanent, payer, Zone::Battlefield, Zone::Graveyard)?;
                if !permanent.characteristics_complete || !permanent.matches_filter(*filter) {
                    return Err(CastModifierRuntimeError::CostPermanentFilterMismatch);
                }
            }
            Ok(())
        }
        (
            CastCostAtom::ExileCardsFromOwnGraveyard { amount },
            CostAtomPaymentEvidence::ExileFromOwnGraveyard {
                cards,
                selection_complete,
            },
        ) if cards.len() == *amount as usize && *selection_complete => {
            validate_distinct_paid_objects(cards)?;
            for card in cards {
                if card.owner != payer {
                    return Err(CastModifierRuntimeError::CostObjectOwnerMismatch);
                }
                validate_zone_change(card, card.controller_before, Zone::Graveyard, Zone::Exile)?;
            }
            Ok(())
        }
        (
            CastCostAtom::ReturnControlledCreatureToOwnersHand { filter },
            CostAtomPaymentEvidence::ReturnControlledToOwnersHand { permanent },
        ) => {
            validate_zone_change(permanent, payer, Zone::Battlefield, Zone::Hand)?;
            if !permanent.characteristics_complete || !permanent.matches_filter(*filter) {
                return Err(CastModifierRuntimeError::CostPermanentFilterMismatch);
            }
            Ok(())
        }
        (
            CastCostAtom::TapUntappedControlledCreature { filter },
            CostAtomPaymentEvidence::TapUntappedControlled { permanent },
        ) => {
            if permanent.controller_before != payer
                || permanent.from_zone != Zone::Battlefield
                || permanent.requested_to_zone != Zone::Battlefield
                || permanent.to_zone != Zone::Battlefield
                || !permanent
                    .ordered_zone_change_replacement_semantic_digests
                    .is_empty()
                || !permanent.zone_change_replacements_complete
                || permanent.before != permanent.after
                || permanent.tapped_before
                || !permanent.tapped_after
                || !permanent.characteristics_complete
                || !permanent.matches_filter(*filter)
            {
                return Err(CastModifierRuntimeError::InvalidTapCostEvidence);
            }
            Ok(())
        }
        (
            CastCostAtom::OpponentGainsLife(expected_amount),
            CostAtomPaymentEvidence::OpponentGainsLife {
                opponent,
                before,
                after,
                amount,
                opponent_relationship_checked,
                life_change_replacements_complete,
                ordered_life_change_replacement_semantic_digests,
                final_life_change_semantic_digest,
            },
        ) if *opponent != payer
            && amount == expected_amount
            && *opponent_relationship_checked
            && *life_change_replacements_complete
            && (if ordered_life_change_replacement_semantic_digests.is_empty() {
                before.checked_add(*expected_amount as i32) == Some(*after)
            } else {
                !final_life_change_semantic_digest.is_empty()
            }) =>
        {
            Ok(())
        }
        _ => Err(CastModifierRuntimeError::CostAtomMismatch),
    }
}

fn validate_mana_payment(
    expected: &ManaCost,
    payment: &ManaCostPaymentEvidence,
) -> Result<(), CastModifierRuntimeError> {
    if &payment.printed_cost != expected
        || !payment.modifier_ledger_complete
        || !payment.spending_restrictions_complete
    {
        return Err(CastModifierRuntimeError::IncompleteManaPayment);
    }
    if payment.ordered_cost_modifier_semantic_digests.is_empty()
        && payment.effective_requirements
            != resolve_printed_mana_requirements(expected, payment.chosen_x)?
    {
        return Err(CastModifierRuntimeError::IncorrectEffectiveManaCost);
    }
    if payment.units.len() != payment.effective_requirements.len() {
        return Err(CastModifierRuntimeError::IncorrectManaUnitCount);
    }
    let mut unit_ids = BTreeSet::new();
    let mut requirement_indices = BTreeSet::new();
    for unit in &payment.units {
        if !unit_ids.insert(unit.unit_id)
            || !requirement_indices.insert(unit.requirement_index)
            || !unit.spending_restriction_checked
        {
            return Err(CastModifierRuntimeError::DuplicateOrUncheckedManaUnit);
        }
        let Some(requirement) = payment.effective_requirements.get(unit.requirement_index) else {
            return Err(CastModifierRuntimeError::ManaRequirementIndexOutOfRange);
        };
        if matches!(requirement, ManaRequirement::Colored(color) if *color != unit.produced) {
            return Err(CastModifierRuntimeError::ManaColorMismatch);
        }
    }
    if requirement_indices.len() != payment.effective_requirements.len() {
        return Err(CastModifierRuntimeError::IncompleteManaRequirementAssignment);
    }
    Ok(())
}

fn resolve_printed_mana_requirements(
    cost: &ManaCost,
    chosen_x: u32,
) -> Result<Vec<ManaRequirement>, CastModifierRuntimeError> {
    let mut requirements = Vec::new();
    for symbol in cost.symbols() {
        match symbol {
            ManaSymbol::Generic(amount) => {
                requirements.extend(std::iter::repeat_n(ManaRequirement::Any, *amount as usize));
            }
            ManaSymbol::VariableX => {
                requirements.extend(std::iter::repeat_n(ManaRequirement::Any, chosen_x as usize))
            }
            ManaSymbol::White => requirements.push(ManaRequirement::Colored(ManaColor::White)),
            ManaSymbol::Blue => requirements.push(ManaRequirement::Colored(ManaColor::Blue)),
            ManaSymbol::Black => requirements.push(ManaRequirement::Colored(ManaColor::Black)),
            ManaSymbol::Red => requirements.push(ManaRequirement::Colored(ManaColor::Red)),
            ManaSymbol::Green => requirements.push(ManaRequirement::Colored(ManaColor::Green)),
            ManaSymbol::Colorless => {
                requirements.push(ManaRequirement::Colored(ManaColor::Colorless))
            }
        }
        if requirements.len() > 1_000_000 {
            return Err(CastModifierRuntimeError::ManaCostTooLarge);
        }
    }
    Ok(requirements)
}

fn validate_zone_change(
    object: &PaidObjectEvidence,
    expected_controller: PlayerId,
    from_zone: Zone,
    to_zone: Zone,
) -> Result<(), CastModifierRuntimeError> {
    if object.controller_before != expected_controller {
        return Err(CastModifierRuntimeError::CostObjectControllerMismatch);
    }
    if object.from_zone != from_zone
        || object.requested_to_zone != to_zone
        || !object.zone_change_replacements_complete
        || (object
            .ordered_zone_change_replacement_semantic_digests
            .is_empty()
            && object.to_zone != to_zone)
        || object.before.object_id != object.after.object_id
        || object.before.incarnation_id == object.after.incarnation_id
    {
        return Err(CastModifierRuntimeError::InvalidCostZoneChange);
    }
    Ok(())
}

fn validate_distinct_paid_objects(
    objects: &[PaidObjectEvidence],
) -> Result<(), CastModifierRuntimeError> {
    let mut ids = BTreeSet::new();
    if objects
        .iter()
        .any(|object| !ids.insert(object.before.object_id))
    {
        return Err(CastModifierRuntimeError::DuplicateCostObject);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellKind {
    InstantOrSorcery,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TargetRef {
    Player(PlayerId),
    Object(ObjectRef),
    StackObject(StackObjectId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetSelectionGrammar {
    Targeted {
        minimum: u32,
        maximum: Option<u32>,
        predicate_semantic_digest: String,
    },
    Each {
        predicate_semantic_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetWordBinding {
    pub byte_start: usize,
    pub slot_id: TargetSlotId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSlotProgram {
    pub slot_id: TargetSlotId,
    pub selection: TargetSelectionGrammar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionInstruction {
    pub exact_text: String,
    pub semantic_digest: String,
    pub target_words: Vec<TargetWordBinding>,
    pub target_slots: Vec<TargetSlotProgram>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAssignment {
    pub slot_id: TargetSlotId,
    pub targets: Vec<TargetRef>,
    pub legality_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopiedChoiceValue {
    Number(i64),
    Boolean(bool),
    SemanticOption(String),
    Object(ObjectRef),
    Player(PlayerId),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CopiableSpellChoices {
    pub selected_modes: Vec<ModeId>,
    pub x_values: BTreeMap<String, u32>,
    pub ordered_choices: Vec<CopiedChoiceValue>,
    pub divided_amounts: BTreeMap<String, Vec<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastOption {
    PrintedManaCost,
    OtherAlternativeCost,
    Overload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackSpell {
    pub stack_id: StackObjectId,
    pub physical_card: Option<ObjectRef>,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub spell_kind: SpellKind,
    pub cast_event_id: Option<CastEventId>,
    pub is_copy: bool,
    pub copy_parent: Option<StackObjectId>,
    pub cast_option: CastOption,
    pub resolution: Vec<ResolutionInstruction>,
    pub choices: CopiableSpellChoices,
    pub targets: Vec<TargetAssignment>,
    pub spliced_source_objects: BTreeSet<ObjectRef>,
    pub paid_modifier_bindings: BTreeMap<AbilityInstanceId, Vec<PaymentId>>,
}

impl StackSpell {
    fn validate_target_assignments(&self) -> Result<(), CastModifierRuntimeError> {
        let slots = self
            .resolution
            .iter()
            .flat_map(|instruction| instruction.target_slots.iter())
            .map(|slot| (slot.slot_id, slot))
            .collect::<BTreeMap<_, _>>();
        let slot_count = self
            .resolution
            .iter()
            .map(|instruction| instruction.target_slots.len())
            .sum::<usize>();
        if slots.len() != slot_count {
            return Err(CastModifierRuntimeError::InvalidTargetAssignment);
        }
        let mut assigned = BTreeMap::new();
        for assignment in &self.targets {
            if assigned.insert(assignment.slot_id, assignment).is_some()
                || !slots.contains_key(&assignment.slot_id)
                || !assignment.legality_checked
            {
                return Err(CastModifierRuntimeError::InvalidTargetAssignment);
            }
        }
        for (slot_id, slot) in slots {
            match &slot.selection {
                TargetSelectionGrammar::Targeted { .. } => {
                    let assignment = assigned
                        .get(&slot_id)
                        .ok_or(CastModifierRuntimeError::InvalidTargetAssignment)?;
                    validate_target_count(slot, assignment.targets.len())?;
                }
                TargetSelectionGrammar::Each { .. } => {
                    if assigned.contains_key(&slot_id) {
                        return Err(CastModifierRuntimeError::InvalidTargetAssignment);
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastEventEvidence {
    pub event_id: CastEventId,
    pub turn_id: TurnId,
    pub spell_cast_ordinal: u32,
    pub caster: PlayerId,
    pub spell: StackSpell,
    pub cast_process_complete: bool,
}

impl CastEventEvidence {
    fn validate(&self) -> Result<(), CastModifierRuntimeError> {
        if !self.cast_process_complete
            || self.spell.cast_event_id != Some(self.event_id)
            || self.spell.controller != self.caster
            || self.spell.is_copy
            || self.spell_cast_ordinal == 0
        {
            return Err(CastModifierRuntimeError::InvalidCastEvent);
        }
        self.spell.validate_target_assignments()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingBuybackReplacement {
    pub ability_instance_id: AbilityInstanceId,
    pub program_semantic_digest: String,
    pub payment_id: PaymentId,
    pub stack_id: StackObjectId,
    pub physical_spell_card: ObjectRef,
    pub owner: PlayerId,
}

pub fn pay_buyback(
    program: &CastModifierKeywordProgram,
    ability_instance_id: AbilityInstanceId,
    spell: &mut StackSpell,
    payment: CastCostPaymentEvidence,
) -> Result<PendingBuybackReplacement, CastModifierRuntimeError> {
    let CastModifierKeywordKind::Buyback { additional_cost } = program.kind() else {
        return Err(CastModifierRuntimeError::WrongProgramKind);
    };
    if spell.is_copy || spell.cast_event_id.is_none() {
        return Err(CastModifierRuntimeError::ModifierRequiresCastPhysicalSpell);
    }
    let physical_spell_card = spell
        .physical_card
        .ok_or(CastModifierRuntimeError::ModifierRequiresCastPhysicalSpell)?;
    validate_cost_payment(
        &payment,
        spell.controller,
        ability_instance_id,
        program,
        additional_cost,
        CostPaymentRole::Additional,
    )?;
    record_modifier_payment(spell, ability_instance_id, payment.payment_id, false)?;
    Ok(PendingBuybackReplacement {
        ability_instance_id,
        program_semantic_digest: program.semantic_digest().to_owned(),
        payment_id: payment.payment_id,
        stack_id: spell.stack_id,
        physical_spell_card,
        owner: spell.owner,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuybackReplacementInput {
    pub stack_id: StackObjectId,
    pub physical_spell_card: ObjectRef,
    pub spell_resolved: bool,
    /// The destination currently proposed by the replacement engine when it
    /// reaches this effect in the chosen ordering.
    pub current_destination: Zone,
    pub destination_incarnation_id: IncarnationId,
    pub replacement_pass_id: u64,
    pub application_ordinal: u32,
    pub prior_applied_replacement_semantic_digests: Vec<String>,
    pub ordering_complete_through_this_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuybackReplacementResolution {
    Applied {
        stack_id: StackObjectId,
        before: ObjectRef,
        after: ObjectRef,
        requested_destination: Zone,
        actual_destination: Zone,
        destination_player: PlayerId,
        replacement_pass_id: u64,
        application_ordinal: u32,
        prior_applied_replacement_semantic_digests: Vec<String>,
        program_semantic_digest: String,
    },
    NotApplicable {
        stack_id: StackObjectId,
        physical_spell_card: ObjectRef,
        current_destination: Zone,
        spell_resolved: bool,
    },
}

impl PendingBuybackReplacement {
    pub fn apply_at_ordered_stack_exit(
        self,
        input: BuybackReplacementInput,
    ) -> Result<BuybackReplacementResolution, CastModifierRuntimeError> {
        if input.stack_id != self.stack_id || input.physical_spell_card != self.physical_spell_card
        {
            return Err(CastModifierRuntimeError::StackSpellMismatch);
        }
        if !input.ordering_complete_through_this_effect
            || input.application_ordinal
                != input.prior_applied_replacement_semantic_digests.len() as u32
        {
            return Err(CastModifierRuntimeError::IncompleteReplacementOrdering);
        }
        if !input.spell_resolved || input.current_destination != Zone::Graveyard {
            return Ok(BuybackReplacementResolution::NotApplicable {
                stack_id: self.stack_id,
                physical_spell_card: self.physical_spell_card,
                current_destination: input.current_destination,
                spell_resolved: input.spell_resolved,
            });
        }
        if input.destination_incarnation_id == self.physical_spell_card.incarnation_id {
            return Err(CastModifierRuntimeError::ReusedObjectIncarnation);
        }
        Ok(BuybackReplacementResolution::Applied {
            stack_id: self.stack_id,
            before: self.physical_spell_card,
            after: ObjectRef {
                object_id: self.physical_spell_card.object_id,
                incarnation_id: input.destination_incarnation_id,
            },
            requested_destination: Zone::Graveyard,
            actual_destination: Zone::Hand,
            destination_player: self.owner,
            replacement_pass_id: input.replacement_pass_id,
            application_ordinal: input.application_ordinal,
            prior_applied_replacement_semantic_digests: input
                .prior_applied_replacement_semantic_digests,
            program_semantic_digest: self.program_semantic_digest,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintedModalChoiceEvidence {
    pub all_modes_in_printed_order: Vec<ModeId>,
    pub ordinary_minimum_modes: u32,
    pub ordinary_maximum_modes: u32,
    pub modal_grammar_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntwineCastReceipt {
    pub ability_instance_id: AbilityInstanceId,
    pub program_semantic_digest: String,
    pub payment_id: Option<PaymentId>,
    pub entwine_paid: bool,
    pub selected_modes: Vec<ModeId>,
}

pub fn apply_entwine_to_cast(
    program: &CastModifierKeywordProgram,
    ability_instance_id: AbilityInstanceId,
    spell: &mut StackSpell,
    modal: PrintedModalChoiceEvidence,
    payment: Option<CastCostPaymentEvidence>,
) -> Result<EntwineCastReceipt, CastModifierRuntimeError> {
    let CastModifierKeywordKind::Entwine { additional_cost } = program.kind() else {
        return Err(CastModifierRuntimeError::WrongProgramKind);
    };
    if spell.is_copy || spell.cast_event_id.is_none() || !modal.modal_grammar_complete {
        return Err(CastModifierRuntimeError::IncompleteModalCastEvidence);
    }
    let all_modes = unique_nonempty_modes(&modal.all_modes_in_printed_order)?;
    if all_modes.len() < 2
        || modal.ordinary_minimum_modes == 0
        || modal.ordinary_minimum_modes > modal.ordinary_maximum_modes
        || modal.ordinary_maximum_modes as usize > all_modes.len()
    {
        return Err(CastModifierRuntimeError::InvalidPrintedModalGrammar);
    }
    let payment_id = if let Some(payment) = payment {
        validate_cost_payment(
            &payment,
            spell.controller,
            ability_instance_id,
            program,
            additional_cost,
            CostPaymentRole::Additional,
        )?;
        record_modifier_payment(spell, ability_instance_id, payment.payment_id, false)?;
        spell.choices.selected_modes = modal.all_modes_in_printed_order.clone();
        Some(payment.payment_id)
    } else {
        let selected = &spell.choices.selected_modes;
        let selected_set = unique_nonempty_modes(selected)?;
        if selected.len() < modal.ordinary_minimum_modes as usize
            || selected.len() > modal.ordinary_maximum_modes as usize
            || !selected_set.is_subset(&all_modes)
        {
            return Err(CastModifierRuntimeError::InvalidOrdinaryModalChoice);
        }
        None
    };
    Ok(EntwineCastReceipt {
        ability_instance_id,
        program_semantic_digest: program.semantic_digest().to_owned(),
        payment_id,
        entwine_paid: payment_id.is_some(),
        selected_modes: spell.choices.selected_modes.clone(),
    })
}

fn unique_nonempty_modes(modes: &[ModeId]) -> Result<BTreeSet<ModeId>, CastModifierRuntimeError> {
    if modes.is_empty() {
        return Err(CastModifierRuntimeError::MissingModes);
    }
    let modes_set = modes.iter().copied().collect::<BTreeSet<_>>();
    if modes_set.len() != modes.len() {
        return Err(CastModifierRuntimeError::DuplicateMode);
    }
    Ok(modes_set)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpliceSourceEvidence {
    pub source_before_reveal: ObjectRef,
    pub source_after_reveal: ObjectRef,
    pub owner: PlayerId,
    pub zone_before_reveal: Zone,
    pub zone_after_reveal: Zone,
    pub exact_oracle_text: String,
    pub revealed_to_all_players: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplicePayload {
    /// One complete instruction per non-splice Oracle line, in printed order.
    pub instructions: Vec<ResolutionInstruction>,
    pub choices: CopiableSpellChoices,
    pub targets: Vec<TargetAssignment>,
    pub all_nonsplice_rules_text_compiled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpliceCastReceipt {
    pub ability_instance_id: AbilityInstanceId,
    pub program_semantic_digest: String,
    pub payment_id: PaymentId,
    pub revealed_source: ObjectRef,
    pub source_remained_in_hand: bool,
    pub appended_instruction_count: usize,
}

pub fn splice_onto_arcane_spell(
    program: &CastModifierKeywordProgram,
    ability_instance_id: AbilityInstanceId,
    spell: &mut StackSpell,
    base_spell_is_arcane: bool,
    source: SpliceSourceEvidence,
    payload: SplicePayload,
    payment: CastCostPaymentEvidence,
) -> Result<SpliceCastReceipt, CastModifierRuntimeError> {
    let CastModifierKeywordKind::SpliceOntoArcane { additional_cost } = program.kind() else {
        return Err(CastModifierRuntimeError::WrongProgramKind);
    };
    if spell.is_copy
        || spell.cast_event_id.is_none()
        || !base_spell_is_arcane
        || source.owner != spell.controller
        || source.zone_before_reveal != Zone::Hand
        || source.zone_after_reveal != Zone::Hand
        || source.source_before_reveal != source.source_after_reveal
        || !source.revealed_to_all_players
    {
        return Err(CastModifierRuntimeError::InvalidSpliceSource);
    }
    if spell
        .spliced_source_objects
        .contains(&source.source_before_reveal)
    {
        return Err(CastModifierRuntimeError::SameCardSplicedTwice);
    }
    if !payload.all_nonsplice_rules_text_compiled {
        return Err(CastModifierRuntimeError::IncompleteSplicePayload);
    }
    let expected_payload_lines = source
        .exact_oracle_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| *line != program.exact_source())
        .collect::<Vec<_>>();
    if expected_payload_lines.len()
        == source
            .exact_oracle_text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()
        || expected_payload_lines.len() != payload.instructions.len()
        || expected_payload_lines
            .iter()
            .zip(&payload.instructions)
            .any(|(expected, instruction)| *expected != instruction.exact_text)
    {
        return Err(CastModifierRuntimeError::SplicePayloadTextMismatch);
    }
    validate_cost_payment(
        &payment,
        spell.controller,
        ability_instance_id,
        program,
        additional_cost,
        CostPaymentRole::Additional,
    )?;
    validate_new_instruction_namespace(spell, &payload.instructions)?;
    validate_payload_targets(&payload.instructions, &payload.targets)?;
    let existing_modes = spell
        .choices
        .selected_modes
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let payload_modes = payload
        .choices
        .selected_modes
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if payload_modes.len() != payload.choices.selected_modes.len()
        || !existing_modes.is_disjoint(&payload_modes)
        || payload
            .choices
            .x_values
            .keys()
            .any(|key| spell.choices.x_values.contains_key(key))
        || payload
            .choices
            .divided_amounts
            .keys()
            .any(|key| spell.choices.divided_amounts.contains_key(key))
    {
        return Err(CastModifierRuntimeError::SpliceChoiceNamespaceCollision);
    }
    record_modifier_payment(spell, ability_instance_id, payment.payment_id, false)?;
    let appended_instruction_count = payload.instructions.len();
    spell.resolution.extend(payload.instructions);
    spell
        .choices
        .selected_modes
        .extend(payload.choices.selected_modes);
    spell
        .choices
        .ordered_choices
        .extend(payload.choices.ordered_choices);
    for (key, value) in payload.choices.x_values {
        let previous = spell.choices.x_values.insert(key, value);
        debug_assert!(previous.is_none());
    }
    for (key, value) in payload.choices.divided_amounts {
        let previous = spell.choices.divided_amounts.insert(key, value);
        debug_assert!(previous.is_none());
    }
    spell.targets.extend(payload.targets);
    spell
        .spliced_source_objects
        .insert(source.source_before_reveal);
    Ok(SpliceCastReceipt {
        ability_instance_id,
        program_semantic_digest: program.semantic_digest().to_owned(),
        payment_id: payment.payment_id,
        revealed_source: source.source_before_reveal,
        source_remained_in_hand: true,
        appended_instruction_count,
    })
}

fn validate_new_instruction_namespace(
    spell: &StackSpell,
    instructions: &[ResolutionInstruction],
) -> Result<(), CastModifierRuntimeError> {
    let existing = spell
        .resolution
        .iter()
        .flat_map(|instruction| instruction.target_slots.iter())
        .map(|slot| slot.slot_id)
        .collect::<BTreeSet<_>>();
    let mut added = BTreeSet::new();
    for slot in instructions
        .iter()
        .flat_map(|instruction| instruction.target_slots.iter())
    {
        if existing.contains(&slot.slot_id) || !added.insert(slot.slot_id) {
            return Err(CastModifierRuntimeError::SpliceTargetNamespaceCollision);
        }
    }
    Ok(())
}

fn validate_payload_targets(
    instructions: &[ResolutionInstruction],
    targets: &[TargetAssignment],
) -> Result<(), CastModifierRuntimeError> {
    let synthetic = StackSpell {
        stack_id: StackObjectId(0),
        physical_card: None,
        owner: PlayerId(0),
        controller: PlayerId(0),
        spell_kind: SpellKind::InstantOrSorcery,
        cast_event_id: None,
        is_copy: true,
        copy_parent: None,
        cast_option: CastOption::PrintedManaCost,
        resolution: instructions.to_vec(),
        choices: CopiableSpellChoices::default(),
        targets: targets.to_vec(),
        spliced_source_objects: BTreeSet::new(),
        paid_modifier_bindings: BTreeMap::new(),
    };
    synthetic.validate_target_assignments()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternativeCostSelectionEvidence {
    pub all_available_alternative_costs_enumerated: bool,
    pub chosen_only_overload: bool,
    pub mandatory_additional_costs_paid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverloadCastReceipt {
    pub ability_instance_id: AbilityInstanceId,
    pub program_semantic_digest: String,
    pub payment_id: PaymentId,
    pub transformed_target_word_count: usize,
}

pub fn cast_with_overload(
    program: &CastModifierKeywordProgram,
    ability_instance_id: AbilityInstanceId,
    spell: &mut StackSpell,
    selection: AlternativeCostSelectionEvidence,
    payment: CastCostPaymentEvidence,
) -> Result<OverloadCastReceipt, CastModifierRuntimeError> {
    let CastModifierKeywordKind::Overload { alternative_cost } = program.kind() else {
        return Err(CastModifierRuntimeError::WrongProgramKind);
    };
    if spell.is_copy
        || spell.cast_event_id.is_none()
        || spell.cast_option != CastOption::PrintedManaCost
        || !spell.targets.is_empty()
        || !selection.all_available_alternative_costs_enumerated
        || !selection.chosen_only_overload
        || !selection.mandatory_additional_costs_paid
    {
        return Err(CastModifierRuntimeError::InvalidOverloadCastEvidence);
    }
    let cost = CastModifierCost {
        exact: alternative_cost.exact.clone(),
        atoms: vec![CastCostAtom::Mana(alternative_cost.clone())],
    };
    validate_cost_payment(
        &payment,
        spell.controller,
        ability_instance_id,
        program,
        &cost,
        CostPaymentRole::Alternative,
    )?;

    let mut transformed = Vec::with_capacity(spell.resolution.len());
    let mut transformed_target_word_count = 0usize;
    for instruction in &spell.resolution {
        let transformed_instruction =
            overload_transform_instruction(instruction, program.semantic_digest())?;
        transformed_target_word_count += instruction.target_words.len();
        transformed.push(transformed_instruction);
    }
    if transformed_target_word_count == 0 {
        return Err(CastModifierRuntimeError::OverloadHasNoTargetWord);
    }
    record_modifier_payment(spell, ability_instance_id, payment.payment_id, false)?;
    spell.resolution = transformed;
    spell.cast_option = CastOption::Overload;
    Ok(OverloadCastReceipt {
        ability_instance_id,
        program_semantic_digest: program.semantic_digest().to_owned(),
        payment_id: payment.payment_id,
        transformed_target_word_count,
    })
}

fn overload_transform_instruction(
    instruction: &ResolutionInstruction,
    overload_program_digest: &str,
) -> Result<ResolutionInstruction, CastModifierRuntimeError> {
    let actual_target_words = standalone_word_offsets(&instruction.exact_text, "target");
    let mut bindings = instruction.target_words.clone();
    bindings.sort_by_key(|binding| binding.byte_start);
    if bindings
        .windows(2)
        .any(|pair| pair[0].byte_start == pair[1].byte_start)
        || bindings
            .iter()
            .map(|binding| binding.byte_start)
            .collect::<Vec<_>>()
            != actual_target_words
    {
        return Err(CastModifierRuntimeError::IncompleteOverloadTargetWordBinding);
    }
    let slot_map = instruction
        .target_slots
        .iter()
        .map(|slot| (slot.slot_id, slot))
        .collect::<BTreeMap<_, _>>();
    if slot_map.len() != instruction.target_slots.len()
        || bindings
            .iter()
            .any(|binding| !slot_map.contains_key(&binding.slot_id))
    {
        return Err(CastModifierRuntimeError::IncompleteOverloadTargetSlotBinding);
    }
    let bound_slot_ids = bindings
        .iter()
        .map(|binding| binding.slot_id)
        .collect::<BTreeSet<_>>();
    if bound_slot_ids.len() != instruction.target_slots.len() {
        return Err(CastModifierRuntimeError::IncompleteOverloadTargetSlotBinding);
    }

    let mut target_slots = Vec::with_capacity(instruction.target_slots.len());
    for slot in &instruction.target_slots {
        let TargetSelectionGrammar::Targeted {
            predicate_semantic_digest,
            ..
        } = &slot.selection
        else {
            return Err(CastModifierRuntimeError::OverloadSlotAlreadyNonTargeted);
        };
        target_slots.push(TargetSlotProgram {
            slot_id: slot.slot_id,
            selection: TargetSelectionGrammar::Each {
                predicate_semantic_digest: predicate_semantic_digest.clone(),
            },
        });
    }
    let exact_text =
        replace_words_at_offsets(&instruction.exact_text, &actual_target_words, "each")?;
    let target_words = Vec::new();
    let semantic_digest = transformed_instruction_digest(
        &instruction.semantic_digest,
        overload_program_digest,
        &exact_text,
        &target_slots,
    );
    Ok(ResolutionInstruction {
        exact_text,
        semantic_digest,
        target_words,
        target_slots,
    })
}

fn standalone_word_offsets(source: &str, needle: &str) -> Vec<usize> {
    let lower = source.to_ascii_lowercase();
    let needle = needle.to_ascii_lowercase();
    let mut offsets = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative) = lower[cursor..].find(&needle) {
        let start = cursor + relative;
        let end = start + needle.len();
        let left_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
        let right_ok = end == lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
        if left_ok && right_ok {
            offsets.push(start);
        }
        cursor = end;
    }
    offsets
}

fn replace_words_at_offsets(
    source: &str,
    offsets: &[usize],
    replacement: &str,
) -> Result<String, CastModifierRuntimeError> {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for start in offsets {
        let end = start
            .checked_add("target".len())
            .ok_or(CastModifierRuntimeError::InvalidTargetWordOffset)?;
        if *start < cursor
            || end > source.len()
            || !source.is_char_boundary(*start)
            || !source.is_char_boundary(end)
            || !source[*start..end].eq_ignore_ascii_case("target")
        {
            return Err(CastModifierRuntimeError::InvalidTargetWordOffset);
        }
        output.push_str(&source[cursor..*start]);
        if source.as_bytes()[*start].is_ascii_uppercase() {
            output.push_str("Each");
        } else {
            output.push_str(replacement);
        }
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

fn transformed_instruction_digest(
    original_digest: &str,
    overload_program_digest: &str,
    exact_text: &str,
    slots: &[TargetSlotProgram],
) -> String {
    let mut hasher = Sha256::new();
    let slot_contract = slots
        .iter()
        .map(|slot| match &slot.selection {
            TargetSelectionGrammar::Each {
                predicate_semantic_digest,
            } => format!("{}=each/{predicate_semantic_digest}", slot.slot_id.0),
            TargetSelectionGrammar::Targeted { .. } => {
                format!("{}=unexpected-target", slot.slot_id.0)
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    for component in [
        "overload-transformed-resolution/v1",
        CAST_MODIFIER_KEYWORD_RUNTIME_VERSION,
        original_digest,
        overload_program_digest,
        exact_text,
        &slot_contract,
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn record_modifier_payment(
    spell: &mut StackSpell,
    ability_instance_id: AbilityInstanceId,
    payment_id: PaymentId,
    repeatable: bool,
) -> Result<(), CastModifierRuntimeError> {
    let payments = spell
        .paid_modifier_bindings
        .entry(ability_instance_id)
        .or_default();
    if (!repeatable && !payments.is_empty()) || payments.contains(&payment_id) {
        return Err(CastModifierRuntimeError::DuplicateModifierPayment);
    }
    payments.push(payment_id);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CopyTriggerKind {
    Replicate,
    Storm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CopyTriggerKey {
    pub cast_event_id: CastEventId,
    pub ability_instance_id: AbilityInstanceId,
    pub kind: CopyTriggerKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CopyTriggerLedger {
    observed: BTreeSet<CopyTriggerKey>,
}

impl CopyTriggerLedger {
    pub fn has_observed(&self, key: CopyTriggerKey) -> bool {
        self.observed.contains(&key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSpellCopyTrigger {
    pub key: CopyTriggerKey,
    pub controller: PlayerId,
    pub program_semantic_digest: String,
    pub copy_count: u32,
    original_spell: StackSpell,
    pub supporting_payment_ids: Vec<PaymentId>,
    pub supporting_cast_event_ids: Vec<CastEventId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicatePaymentReceipt {
    pub ability_instance_id: AbilityInstanceId,
    pub program_semantic_digest: String,
    pub payment_id: PaymentId,
    pub payment_number_for_instance: u32,
}

pub fn pay_replicate_once(
    program: &CastModifierKeywordProgram,
    ability_instance_id: AbilityInstanceId,
    spell: &mut StackSpell,
    payment: CastCostPaymentEvidence,
) -> Result<ReplicatePaymentReceipt, CastModifierRuntimeError> {
    let CastModifierKeywordKind::Replicate {
        repeatable_additional_cost,
    } = program.kind()
    else {
        return Err(CastModifierRuntimeError::WrongProgramKind);
    };
    if spell.is_copy || spell.cast_event_id.is_none() {
        return Err(CastModifierRuntimeError::ModifierRequiresCastPhysicalSpell);
    }
    validate_cost_payment(
        &payment,
        spell.controller,
        ability_instance_id,
        program,
        repeatable_additional_cost,
        CostPaymentRole::Additional,
    )?;
    record_modifier_payment(spell, ability_instance_id, payment.payment_id, true)?;
    let payment_number_for_instance = u32::try_from(
        spell
            .paid_modifier_bindings
            .get(&ability_instance_id)
            .map(Vec::len)
            .unwrap_or_default(),
    )
    .map_err(|_| CastModifierRuntimeError::CopyCountOverflow)?;
    Ok(ReplicatePaymentReceipt {
        ability_instance_id,
        program_semantic_digest: program.semantic_digest().to_owned(),
        payment_id: payment.payment_id,
        payment_number_for_instance,
    })
}

pub fn begin_replicate_cast_trigger(
    program: &CastModifierKeywordProgram,
    ability_instance_id: AbilityInstanceId,
    cast: &CastEventEvidence,
    payments: Vec<CastCostPaymentEvidence>,
    ledger: &mut CopyTriggerLedger,
) -> Result<PendingSpellCopyTrigger, CastModifierRuntimeError> {
    let CastModifierKeywordKind::Replicate {
        repeatable_additional_cost,
    } = program.kind()
    else {
        return Err(CastModifierRuntimeError::WrongProgramKind);
    };
    cast.validate()?;
    let mut payment_ids = Vec::with_capacity(payments.len());
    let mut unique_payment_ids = BTreeSet::new();
    for payment in &payments {
        validate_cost_payment(
            payment,
            cast.caster,
            ability_instance_id,
            program,
            repeatable_additional_cost,
            CostPaymentRole::Additional,
        )?;
        if !unique_payment_ids.insert(payment.payment_id) {
            return Err(CastModifierRuntimeError::DuplicateModifierPayment);
        }
        payment_ids.push(payment.payment_id);
    }
    let recorded = cast
        .spell
        .paid_modifier_bindings
        .get(&ability_instance_id)
        .cloned()
        .unwrap_or_default();
    if recorded != payment_ids {
        return Err(CastModifierRuntimeError::ReplicatePaymentLedgerMismatch);
    }
    let copy_count = u32::try_from(payment_ids.len())
        .map_err(|_| CastModifierRuntimeError::CopyCountOverflow)?;
    let key = CopyTriggerKey {
        cast_event_id: cast.event_id,
        ability_instance_id,
        kind: CopyTriggerKind::Replicate,
    };
    if ledger.observed.contains(&key) {
        return Err(CastModifierRuntimeError::TriggerAlreadyObserved);
    }
    ledger.observed.insert(key);
    Ok(PendingSpellCopyTrigger {
        key,
        controller: cast.caster,
        program_semantic_digest: program.semantic_digest().to_owned(),
        copy_count,
        original_spell: cast.spell.clone(),
        supporting_payment_ids: payment_ids,
        supporting_cast_event_ids: vec![cast.event_id],
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastHistoryEntry {
    pub event_id: CastEventId,
    pub turn_id: TurnId,
    pub spell_cast_ordinal: u32,
    pub caster: PlayerId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastHistoryEvidence {
    pub turn_id: TurnId,
    pub entries_through_current_spell: Vec<CastHistoryEntry>,
    pub complete_from_turn_start_through_current_spell: bool,
}

impl CastHistoryEvidence {
    fn spells_before(
        &self,
        cast: &CastEventEvidence,
    ) -> Result<Vec<CastEventId>, CastModifierRuntimeError> {
        if !self.complete_from_turn_start_through_current_spell
            || self.turn_id != cast.turn_id
            || self.entries_through_current_spell.len() != cast.spell_cast_ordinal as usize
        {
            return Err(CastModifierRuntimeError::IncompleteCastHistory);
        }
        let mut event_ids = BTreeSet::new();
        for (index, entry) in self.entries_through_current_spell.iter().enumerate() {
            let expected_ordinal = u32::try_from(index + 1)
                .map_err(|_| CastModifierRuntimeError::CopyCountOverflow)?;
            if entry.turn_id != self.turn_id
                || entry.spell_cast_ordinal != expected_ordinal
                || !event_ids.insert(entry.event_id)
            {
                return Err(CastModifierRuntimeError::InvalidCastHistory);
            }
        }
        let Some(current) = self.entries_through_current_spell.last() else {
            return Err(CastModifierRuntimeError::IncompleteCastHistory);
        };
        if current.event_id != cast.event_id
            || current.spell_cast_ordinal != cast.spell_cast_ordinal
            || current.caster != cast.caster
        {
            return Err(CastModifierRuntimeError::CastHistoryCurrentSpellMismatch);
        }
        Ok(
            self.entries_through_current_spell[..self.entries_through_current_spell.len() - 1]
                .iter()
                .map(|entry| entry.event_id)
                .collect(),
        )
    }
}

pub fn begin_storm_cast_trigger(
    program: &CastModifierKeywordProgram,
    ability_instance_id: AbilityInstanceId,
    cast: &CastEventEvidence,
    history: &CastHistoryEvidence,
    ledger: &mut CopyTriggerLedger,
) -> Result<PendingSpellCopyTrigger, CastModifierRuntimeError> {
    if !matches!(program.kind(), CastModifierKeywordKind::Storm) {
        return Err(CastModifierRuntimeError::WrongProgramKind);
    }
    cast.validate()?;
    let supporting_cast_event_ids = history.spells_before(cast)?;
    let copy_count = u32::try_from(supporting_cast_event_ids.len())
        .map_err(|_| CastModifierRuntimeError::CopyCountOverflow)?;
    let key = CopyTriggerKey {
        cast_event_id: cast.event_id,
        ability_instance_id,
        kind: CopyTriggerKind::Storm,
    };
    if ledger.observed.contains(&key) {
        return Err(CastModifierRuntimeError::TriggerAlreadyObserved);
    }
    ledger.observed.insert(key);
    Ok(PendingSpellCopyTrigger {
        key,
        controller: cast.caster,
        program_semantic_digest: program.semantic_digest().to_owned(),
        copy_count,
        original_spell: cast.spell.clone(),
        supporting_payment_ids: Vec::new(),
        supporting_cast_event_ids,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyTargetChoice {
    pub stack_id: StackObjectId,
    /// Omitted slots keep the original spell's target assignment.
    pub new_targets_by_slot: BTreeMap<TargetSlotId, Vec<TargetRef>>,
    pub all_new_target_legality_checked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellCopyResolutionKind {
    OrdinarySpellCopy,
    PermanentSpellCopyBecomesToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedSpellCopy {
    pub spell: StackSpell,
    pub resolution_kind: SpellCopyResolutionKind,
    pub copied_modes: Vec<ModeId>,
    pub copied_x_values: BTreeMap<String, u32>,
    pub copied_ordered_choices: Vec<CopiedChoiceValue>,
    pub copied_divided_amounts: BTreeMap<String, Vec<u32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCopyTriggerResolution {
    pub key: CopyTriggerKey,
    pub program_semantic_digest: String,
    pub copies: Vec<CreatedSpellCopy>,
}

impl PendingSpellCopyTrigger {
    pub fn resolve(
        self,
        copy_choices: Vec<CopyTargetChoice>,
    ) -> Result<SpellCopyTriggerResolution, CastModifierRuntimeError> {
        if copy_choices.len() != self.copy_count as usize {
            return Err(CastModifierRuntimeError::WrongCopyCreationCount);
        }
        self.original_spell.validate_target_assignments()?;
        let original_target_map = self
            .original_spell
            .targets
            .iter()
            .map(|assignment| (assignment.slot_id, assignment.clone()))
            .collect::<BTreeMap<_, _>>();
        let slot_map = self
            .original_spell
            .resolution
            .iter()
            .flat_map(|instruction| instruction.target_slots.iter())
            .map(|slot| (slot.slot_id, slot.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut copy_ids = BTreeSet::new();
        let mut copies = Vec::with_capacity(copy_choices.len());
        for choice in copy_choices {
            if choice.stack_id == self.original_spell.stack_id
                || !copy_ids.insert(choice.stack_id)
                || !choice.all_new_target_legality_checked
            {
                return Err(CastModifierRuntimeError::InvalidCopyStackIdentity);
            }
            let mut targets = original_target_map.clone();
            for (slot_id, new_targets) in choice.new_targets_by_slot {
                let slot = slot_map
                    .get(&slot_id)
                    .ok_or(CastModifierRuntimeError::UnknownCopyTargetSlot)?;
                validate_target_count(slot, new_targets.len())?;
                targets.insert(
                    slot_id,
                    TargetAssignment {
                        slot_id,
                        targets: new_targets,
                        legality_checked: true,
                    },
                );
            }
            let mut spell = self.original_spell.clone();
            spell.stack_id = choice.stack_id;
            spell.physical_card = None;
            spell.cast_event_id = None;
            spell.is_copy = true;
            spell.copy_parent = Some(self.original_spell.stack_id);
            spell.targets = targets.into_values().collect();
            spell.targets.sort_by_key(|target| target.slot_id);
            let resolution_kind = match spell.spell_kind {
                SpellKind::InstantOrSorcery => SpellCopyResolutionKind::OrdinarySpellCopy,
                SpellKind::Permanent => SpellCopyResolutionKind::PermanentSpellCopyBecomesToken,
            };
            copies.push(CreatedSpellCopy {
                copied_modes: spell.choices.selected_modes.clone(),
                copied_x_values: spell.choices.x_values.clone(),
                copied_ordered_choices: spell.choices.ordered_choices.clone(),
                copied_divided_amounts: spell.choices.divided_amounts.clone(),
                spell,
                resolution_kind,
            });
        }
        Ok(SpellCopyTriggerResolution {
            key: self.key,
            program_semantic_digest: self.program_semantic_digest,
            copies,
        })
    }
}

fn validate_target_count(
    slot: &TargetSlotProgram,
    count: usize,
) -> Result<(), CastModifierRuntimeError> {
    match &slot.selection {
        TargetSelectionGrammar::Targeted {
            minimum, maximum, ..
        } => {
            if count < *minimum as usize || maximum.is_some_and(|maximum| count > maximum as usize)
            {
                return Err(CastModifierRuntimeError::InvalidNewTargetCount);
            }
            Ok(())
        }
        TargetSelectionGrammar::Each { .. } => {
            Err(CastModifierRuntimeError::CannotRetargetEachSelection)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastModifierRuntimeError {
    WrongProgramKind,
    WrongPayer {
        expected: PlayerId,
        actual: PlayerId,
    },
    PaymentBindingMismatch,
    WrongCostOrRole,
    IncompleteCostPayment,
    CostAtomMismatch,
    IncompleteManaPayment,
    IncorrectEffectiveManaCost,
    IncorrectManaUnitCount,
    DuplicateOrUncheckedManaUnit,
    ManaRequirementIndexOutOfRange,
    ManaColorMismatch,
    IncompleteManaRequirementAssignment,
    ManaCostTooLarge,
    CostPermanentFilterMismatch,
    CostObjectOwnerMismatch,
    CostObjectControllerMismatch,
    InvalidCostZoneChange,
    DuplicateCostObject,
    InvalidTapCostEvidence,
    ModifierRequiresCastPhysicalSpell,
    DuplicateModifierPayment,
    StackSpellMismatch,
    IncompleteReplacementOrdering,
    ReusedObjectIncarnation,
    IncompleteModalCastEvidence,
    InvalidPrintedModalGrammar,
    InvalidOrdinaryModalChoice,
    MissingModes,
    DuplicateMode,
    InvalidSpliceSource,
    SameCardSplicedTwice,
    IncompleteSplicePayload,
    SplicePayloadTextMismatch,
    SpliceTargetNamespaceCollision,
    SpliceChoiceNamespaceCollision,
    InvalidOverloadCastEvidence,
    OverloadHasNoTargetWord,
    IncompleteOverloadTargetWordBinding,
    IncompleteOverloadTargetSlotBinding,
    OverloadSlotAlreadyNonTargeted,
    InvalidTargetWordOffset,
    InvalidTargetAssignment,
    InvalidCastEvent,
    ReplicatePaymentLedgerMismatch,
    CopyCountOverflow,
    TriggerAlreadyObserved,
    IncompleteCastHistory,
    InvalidCastHistory,
    CastHistoryCurrentSpellMismatch,
    WrongCopyCreationCount,
    InvalidCopyStackIdentity,
    UnknownCopyTargetSlot,
    InvalidNewTargetCount,
    CannotRetargetEachSelection,
}

impl fmt::Display for CastModifierRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProgramKind => formatter.write_str("wrong cast modifier program kind"),
            Self::WrongPayer { expected, actual } => {
                write!(
                    formatter,
                    "wrong payer: expected {expected:?}, got {actual:?}"
                )
            }
            Self::PaymentBindingMismatch => {
                formatter.write_str("payment is bound to a different keyword instance or program")
            }
            Self::WrongCostOrRole => formatter.write_str("payment used the wrong cost or role"),
            Self::IncompleteCostPayment => {
                formatter.write_str("cost payment components are incomplete")
            }
            Self::CostAtomMismatch => formatter.write_str("cost atom evidence does not match"),
            Self::IncompleteManaPayment => formatter.write_str("mana payment is incomplete"),
            Self::IncorrectEffectiveManaCost => {
                formatter.write_str("effective mana cost does not follow the printed cost")
            }
            Self::IncorrectManaUnitCount => formatter.write_str("incorrect mana unit count"),
            Self::DuplicateOrUncheckedManaUnit => {
                formatter.write_str("mana unit is duplicate or unchecked")
            }
            Self::ManaRequirementIndexOutOfRange => {
                formatter.write_str("mana requirement index is out of range")
            }
            Self::ManaColorMismatch => formatter.write_str("mana color does not satisfy symbol"),
            Self::IncompleteManaRequirementAssignment => {
                formatter.write_str("not every mana requirement was assigned")
            }
            Self::ManaCostTooLarge => formatter.write_str("mana cost is too large"),
            Self::CostPermanentFilterMismatch => {
                formatter.write_str("cost permanent does not match its filter")
            }
            Self::CostObjectOwnerMismatch => formatter.write_str("cost object owner mismatch"),
            Self::CostObjectControllerMismatch => {
                formatter.write_str("cost object controller mismatch")
            }
            Self::InvalidCostZoneChange => formatter.write_str("invalid cost zone change"),
            Self::DuplicateCostObject => formatter.write_str("same object paid twice"),
            Self::InvalidTapCostEvidence => formatter.write_str("invalid tap cost evidence"),
            Self::ModifierRequiresCastPhysicalSpell => {
                formatter.write_str("modifier requires a cast physical spell")
            }
            Self::DuplicateModifierPayment => formatter.write_str("duplicate modifier payment"),
            Self::StackSpellMismatch => formatter.write_str("stack spell mismatch"),
            Self::IncompleteReplacementOrdering => {
                formatter.write_str("replacement ordering evidence is incomplete")
            }
            Self::ReusedObjectIncarnation => formatter.write_str("object incarnation was reused"),
            Self::IncompleteModalCastEvidence => {
                formatter.write_str("modal cast evidence is incomplete")
            }
            Self::InvalidPrintedModalGrammar => {
                formatter.write_str("printed modal grammar is invalid")
            }
            Self::InvalidOrdinaryModalChoice => {
                formatter.write_str("ordinary modal choice is invalid")
            }
            Self::MissingModes => formatter.write_str("modal spell has no modes"),
            Self::DuplicateMode => formatter.write_str("modal mode is duplicated"),
            Self::InvalidSpliceSource => formatter.write_str("invalid splice source evidence"),
            Self::SameCardSplicedTwice => {
                formatter.write_str("the same card cannot be spliced twice")
            }
            Self::IncompleteSplicePayload => formatter.write_str("splice payload is incomplete"),
            Self::SplicePayloadTextMismatch => {
                formatter.write_str("splice payload does not match source rules text")
            }
            Self::SpliceTargetNamespaceCollision => {
                formatter.write_str("splice target namespace collided")
            }
            Self::SpliceChoiceNamespaceCollision => {
                formatter.write_str("splice choice namespace collided")
            }
            Self::InvalidOverloadCastEvidence => {
                formatter.write_str("invalid overload cast evidence")
            }
            Self::OverloadHasNoTargetWord => {
                formatter.write_str("overload spell has no target word")
            }
            Self::IncompleteOverloadTargetWordBinding => {
                formatter.write_str("overload target word binding is incomplete")
            }
            Self::IncompleteOverloadTargetSlotBinding => {
                formatter.write_str("overload target slot binding is incomplete")
            }
            Self::OverloadSlotAlreadyNonTargeted => {
                formatter.write_str("overload slot is already non-targeted")
            }
            Self::InvalidTargetWordOffset => formatter.write_str("invalid target word offset"),
            Self::InvalidTargetAssignment => formatter.write_str("invalid target assignment"),
            Self::InvalidCastEvent => formatter.write_str("invalid cast event"),
            Self::ReplicatePaymentLedgerMismatch => {
                formatter.write_str("replicate payment ledger mismatch")
            }
            Self::CopyCountOverflow => formatter.write_str("copy count overflow"),
            Self::TriggerAlreadyObserved => formatter.write_str("trigger already observed"),
            Self::IncompleteCastHistory => formatter.write_str("cast history is incomplete"),
            Self::InvalidCastHistory => formatter.write_str("cast history is invalid"),
            Self::CastHistoryCurrentSpellMismatch => {
                formatter.write_str("cast history current spell mismatch")
            }
            Self::WrongCopyCreationCount => formatter.write_str("wrong copy creation count"),
            Self::InvalidCopyStackIdentity => formatter.write_str("invalid copy stack identity"),
            Self::UnknownCopyTargetSlot => formatter.write_str("unknown copy target slot"),
            Self::InvalidNewTargetCount => formatter.write_str("invalid new target count"),
            Self::CannotRetargetEachSelection => {
                formatter.write_str("an each selection cannot be retargeted")
            }
        }
    }
}

impl std::error::Error for CastModifierRuntimeError {}
