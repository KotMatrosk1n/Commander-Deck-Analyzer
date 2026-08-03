//! Exact graveyard, hand, and library keyword programs.
//!
//! This module is intentionally isolated from the production compiler. It
//! accepts only complete Oracle lines whose costs, zones, timing, targets, and
//! resolution instructions are fully represented below. Recognition does not
//! make a program production live.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const ZONE_KEYWORD_COMPILER_VERSION: &str = "graveyard-hand-library-keyword-compiler-0.1";
pub const ZONE_KEYWORD_RUNTIME_VERSION: &str = "graveyard-hand-library-keyword-runtime-0.1";
pub const ZONE_KEYWORD_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:108.3,111,113.6,115,118,119,400.2,400.7,401,602,608,609.3,614,616,701.7,701.8,701.16,701.20,702.52,702.53,702.57,702.59,702.77,702.97,702.128,702.129,704,707";

const PRIOR_CHANNEL_BOSS: &str = "Channel \u{2014} {1}{G}, Discard this card: Destroy target artifact, enchantment, or nonbasic land an opponent controls. That player may search their library for a land card with a basic land type, put it onto the battlefield, then shuffle. This ability costs {1} less to activate for each legendary creature you control.";
const PRIOR_CHANNEL_BOUNCE: &str = "Channel \u{2014} {3}{U}, Discard this card: Return target artifact, creature, enchantment, or planeswalker to its owner's hand. This ability costs {1} less to activate for each legendary creature you control.";

pub const fn zone_keyword_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceKind {
    Creature,
    Land,
    Other,
}

impl SourceKind {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Creature => "creature",
            Self::Land => "land",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSemanticContext<'a> {
    pub type_line: &'a str,
    pub mana_value: Option<u32>,
}

impl SourceSemanticContext<'_> {
    pub fn source_kind(self) -> SourceKind {
        let card_types = self
            .type_line
            .split_once('\u{2014}')
            .map_or(self.type_line, |(types, _)| types);
        if card_types.split_whitespace().any(|word| word == "Creature") {
            SourceKind::Creature
        } else if card_types.split_whitespace().any(|word| word == "Land") {
            SourceKind::Land
        } else {
            SourceKind::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotCandidateClass {
    SupportedFamily,
    EarlierExactOwner,
    UnsupportedCompoundOrContextDependent,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaSymbol {
    Generic(u32),
    Colored(ManaColor),
    VariableX,
}

impl ManaSymbol {
    fn stable_id(self) -> String {
        match self {
            Self::Generic(value) => format!("generic/{value}"),
            Self::Colored(color) => color.stable_id().to_owned(),
            Self::VariableX => "x".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
        self.symbols
            .iter()
            .map(|symbol| symbol.stable_id())
            .collect::<Vec<_>>()
            .join(",")
    }

    fn x_symbols(&self) -> u32 {
        self.symbols
            .iter()
            .filter(|symbol| **symbol == ManaSymbol::VariableX)
            .count() as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceZone {
    Graveyard,
    Hand,
}

impl SourceZone {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Graveyard => "graveyard",
            Self::Hand => "hand",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramTiming {
    Sorcery,
    AnyPriority,
    UpkeepOncePerTurn,
    Triggered,
    DrawReplacement,
}

impl ProgramTiming {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Sorcery => "sorcery",
            Self::AnyPriority => "priority",
            Self::UpkeepOncePerTurn => "upkeep-once-per-turn",
            Self::Triggered => "triggered",
            Self::DrawReplacement => "draw-replacement",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberValue {
    Fixed(u32),
    ChosenX,
    SourcePower,
    LandsControlled,
}

impl NumberValue {
    fn stable_id(self) -> String {
        match self {
            Self::Fixed(value) => format!("fixed/{value}"),
            Self::ChosenX => "chosen-x".to_owned(),
            Self::SourcePower => "source-power".to_owned(),
            Self::LandsControlled => "lands-controlled".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GrantedKeyword {
    Deathtouch,
    DoubleStrike,
    FirstStrike,
    Flying,
    Haste,
    Reach,
    Shadow,
    Trample,
}

impl GrantedKeyword {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Deathtouch => "deathtouch",
            Self::DoubleStrike => "double-strike",
            Self::FirstStrike => "first-strike",
            Self::Flying => "flying",
            Self::Haste => "haste",
            Self::Reach => "reach",
            Self::Shadow => "shadow",
            Self::Trample => "trample",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PumpEffect {
    pub power: NumberValue,
    pub toughness: NumberValue,
    pub granted_keywords: BTreeSet<GrantedKeyword>,
    pub target_must_be_attacking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenCopyException {
    Embalm,
    Eternalize,
    EternalizeLandCreature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoverCost {
    Mana(ManaCost),
    HalfLifeRoundedUp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReinforceAmount {
    Fixed(u32),
    ChosenX,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecastCost {
    ManaAndReveal(ManaCost),
    TapTwoWhiteOrBlueCreaturesAndReveal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecastEffect {
    SetTargetCreatureColorsUntilEndOfTurn,
    TargetSmallCreatureCantBeBlockedThisTurn,
    GainLifeWhenTargetCreatureDealsDamageThisTurn,
    GrantTargetCreatureKeywordUntilEndOfTurn(GrantedKeyword),
    EachPlayerDrawsOne,
    TapTargetCreature { must_be_untapped: bool },
    CreateWhiteBlueBird,
    ReturnSmallCreatureCardFromGraveyard,
    PumpTargetCreature(PumpEffect),
    DrawOne,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelEffect {
    TargetCreatureCantBlockThisTurn,
    CreateGreenHumanMonk,
    PumpTargetCreature(PumpEffect),
    GrantTargetCreatureKeywordUntilEndOfTurn(GrantedKeyword),
    DrawOne,
    DealDamage {
        amount: NumberValue,
        target: DamageTarget,
    },
    ReturnTargetToHand(TargetFilter),
    BlinkTargetUntilNextEndStep(TargetFilter),
    GainLife(u32),
    SearchBasicPlainsToHandThenGainLife(u32),
    PutCountersOnUpToTwoControlledCreatures,
    ReturnTargetCardFromGraveyardToHand(TargetFilter),
    SearchBasicLandToBattlefieldTapped,
    ForceAllAbleCreaturesToBlockTarget,
    CreateRedDragon,
    CounterUnlessControllerPays {
        target: CounterTarget,
        generic_mana: u32,
    },
    CreateTwoPilotTokens,
    MillThenReturnCreatureOrPlaneswalker,
    CreateTwoHastySpirits,
    TapAndFreezeUpToTwoOpposingCreatures,
    DestroyTarget(TargetFilter),
    PutTargetOnLibraryTopOrBottom,
    TargetPlayerDiscards(u32),
    PutCounterOnEachControlledCreatureThenDraw,
    DamageEachCreature(TargetFilter, NumberValue),
    AnimateTargetLandWithCounters,
    GrantFlyingToXTargets,
    ReturnXNonlegendaryCardsFromGraveyard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageTarget {
    Any,
    Creature,
    AttackingOrBlockingCreature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterTarget {
    Spell,
    SpellOrAbility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFilter {
    AnyCard,
    ArtifactCreatureEnchantmentOrPlaneswalker,
    ArtifactOrCreature,
    Creature,
    CreatureWithFlying,
    CreatureWithoutFlying,
    NonlandPermanent,
    NonlegendaryCard,
    CreatureOrPlaneswalkerCard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneKeywordKind {
    Embalm {
        cost: ManaCost,
        token_exception: TokenCopyException,
    },
    Eternalize {
        mana_cost: ManaCost,
        discard_another_card: bool,
        token_exception: TokenCopyException,
    },
    Scavenge {
        cost: ManaCost,
    },
    Recover {
        cost: RecoverCost,
        trigger_requires_another_creature: bool,
    },
    Dredge {
        amount: u32,
    },
    Transmute {
        cost: ManaCost,
        target_mana_value: u32,
    },
    Reinforce {
        amount: ReinforceAmount,
        cost: ManaCost,
    },
    Forecast {
        cost: ForecastCost,
        effect: ForecastEffect,
    },
    Bloodrush {
        cost: ManaCost,
        pump: PumpEffect,
    },
    Channel {
        cost: ManaCost,
        timing: ProgramTiming,
        legendary_creature_reduction: bool,
        effect: ChannelEffect,
    },
}

impl ZoneKeywordKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Embalm { .. } => "Embalm",
            Self::Eternalize { .. } => "Eternalize",
            Self::Scavenge { .. } => "Scavenge",
            Self::Recover { .. } => "Recover",
            Self::Dredge { .. } => "Dredge",
            Self::Transmute { .. } => "Transmute",
            Self::Reinforce { .. } => "Reinforce",
            Self::Forecast { .. } => "Forecast",
            Self::Bloodrush { .. } => "Bloodrush",
            Self::Channel { .. } => "Channel",
        }
    }

    pub fn source_zone(&self) -> SourceZone {
        match self {
            Self::Embalm { .. }
            | Self::Eternalize { .. }
            | Self::Scavenge { .. }
            | Self::Recover { .. }
            | Self::Dredge { .. } => SourceZone::Graveyard,
            Self::Transmute { .. }
            | Self::Reinforce { .. }
            | Self::Forecast { .. }
            | Self::Bloodrush { .. }
            | Self::Channel { .. } => SourceZone::Hand,
        }
    }

    pub fn timing(&self) -> ProgramTiming {
        match self {
            Self::Embalm { .. }
            | Self::Eternalize { .. }
            | Self::Scavenge { .. }
            | Self::Transmute { .. }
            | Self::Reinforce { .. } => ProgramTiming::Sorcery,
            Self::Forecast { .. } => ProgramTiming::UpkeepOncePerTurn,
            Self::Bloodrush { .. } => ProgramTiming::AnyPriority,
            Self::Channel { timing, .. } => *timing,
            Self::Recover { .. } => ProgramTiming::Triggered,
            Self::Dredge { .. } => ProgramTiming::DrawReplacement,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneKeywordProgram {
    exact_source: String,
    kind: ZoneKeywordKind,
    relevant_context: String,
    semantic_digest: String,
}

impl ZoneKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn kind(&self) -> &ZoneKeywordKind {
        &self.kind
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn relevant_context(&self) -> &str {
        &self.relevant_context
    }

    pub const fn production_adapter_connected(&self) -> bool {
        zone_keyword_production_adapter_connected()
    }
}

pub fn classify_snapshot_candidate(
    exact_source: &str,
    context: SourceSemanticContext<'_>,
) -> Option<SnapshotCandidateClass> {
    candidate_family(exact_source)?;
    if is_prior_channel_owner(exact_source, context) {
        return Some(SnapshotCandidateClass::EarlierExactOwner);
    }
    if compile_zone_keyword_program(exact_source, context).is_some() {
        return Some(SnapshotCandidateClass::SupportedFamily);
    }
    Some(SnapshotCandidateClass::UnsupportedCompoundOrContextDependent)
}

pub fn compile_zone_keyword_program(
    exact_source: &str,
    context: SourceSemanticContext<'_>,
) -> Option<ZoneKeywordProgram> {
    if exact_source.is_empty()
        || exact_source.trim() != exact_source
        || exact_source.contains(['\r', '\n'])
        || collapse_whitespace(exact_source) != exact_source
        || is_prior_channel_owner(exact_source, context)
    {
        return None;
    }

    let (kind, relevant_context) = parse_embalm(exact_source, context)
        .or_else(|| parse_eternalize(exact_source, context))
        .or_else(|| parse_scavenge(exact_source, context))
        .or_else(|| parse_recover(exact_source))
        .or_else(|| parse_dredge(exact_source))
        .or_else(|| parse_transmute(exact_source, context))
        .or_else(|| parse_reinforce(exact_source))
        .or_else(|| parse_forecast(exact_source))
        .or_else(|| parse_bloodrush(exact_source))
        .or_else(|| parse_channel(exact_source))?;
    let semantic_digest = semantic_digest_with_versions(
        exact_source,
        &kind,
        &relevant_context,
        ZONE_KEYWORD_COMPILER_VERSION,
        ZONE_KEYWORD_RUNTIME_VERSION,
        ZONE_KEYWORD_RULES_CONTEXT_VERSION,
    );
    Some(ZoneKeywordProgram {
        exact_source: exact_source.to_owned(),
        kind,
        relevant_context,
        semantic_digest,
    })
}

fn is_prior_channel_owner(exact_source: &str, context: SourceSemanticContext<'_>) -> bool {
    context.source_kind() == SourceKind::Land
        && matches!(exact_source, PRIOR_CHANNEL_BOSS | PRIOR_CHANNEL_BOUNCE)
}

fn candidate_family(source: &str) -> Option<&'static str> {
    let lower = source.to_ascii_lowercase();
    [
        ("embalm", "Embalm"),
        ("eternalize", "Eternalize"),
        ("scavenge", "Scavenge"),
        ("recover", "Recover"),
        ("dredge", "Dredge"),
        ("transmute", "Transmute"),
        ("reinforce", "Reinforce"),
        ("forecast", "Forecast"),
        ("bloodrush", "Bloodrush"),
        ("channel", "Channel"),
    ]
    .into_iter()
    .find_map(|(word, family)| contains_word(&lower, word).then_some(family))
}

fn contains_word(source: &str, needle: &str) -> bool {
    source.match_indices(needle).any(|(start, _)| {
        let before = source[..start].chars().next_back();
        let end = start + needle.len();
        let after = source[end..].chars().next();
        before.is_none_or(|character| !character.is_alphanumeric())
            && after.is_none_or(|character| !character.is_alphanumeric())
    })
}

fn parse_embalm(
    source: &str,
    context: SourceSemanticContext<'_>,
) -> Option<(ZoneKeywordKind, String)> {
    if context.source_kind() != SourceKind::Creature {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let cost = parse_mana_cost(core.strip_prefix("Embalm ")?)?;
    if let Some(reminder) = reminder {
        let prefix = format!(
            "{}, Exile this card from your graveyard: Create a token that's a copy of it, except it's a white Zombie",
            cost.exact()
        );
        let remainder = reminder.strip_prefix(&prefix)?;
        let added_types =
            remainder.strip_suffix(" with no mana cost. Embalm only as a sorcery.")?;
        if !valid_copied_subtype_suffix(added_types) {
            return None;
        }
    }
    Some((
        ZoneKeywordKind::Embalm {
            cost,
            token_exception: TokenCopyException::Embalm,
        },
        format!("source-kind/creature;type-line/{}", context.type_line),
    ))
}

fn parse_eternalize(
    source: &str,
    context: SourceSemanticContext<'_>,
) -> Option<(ZoneKeywordKind, String)> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let (mana_text, discard_another_card) = if let Some(cost) = core.strip_prefix("Eternalize ") {
        (cost, false)
    } else {
        (
            core.strip_prefix("Eternalize\u{2014}")?
                .strip_suffix(", Discard a card.")?,
            true,
        )
    };
    let mana_cost = parse_mana_cost(mana_text)?;
    let expected_cost = if discard_another_card {
        format!(
            "{}, Discard a card, Exile this card from your graveyard:",
            mana_cost.exact()
        )
    } else {
        format!(
            "{}, Exile this card from your graveyard:",
            mana_cost.exact()
        )
    };
    let reminder = reminder?;
    if !reminder.starts_with(&expected_cost) {
        return None;
    }

    let (token_exception, relevant_context) = match context.source_kind() {
        SourceKind::Creature => {
            let copy_prefix = " Create a token that's a copy of it, except it's a 4/4 black Zombie";
            let remainder = reminder.strip_prefix(&format!("{expected_cost}{copy_prefix}"))?;
            let added_types =
                remainder.strip_suffix(" with no mana cost. Eternalize only as a sorcery.")?;
            if !valid_copied_subtype_suffix(added_types) {
                return None;
            }
            (
                TokenCopyException::Eternalize,
                format!("source-kind/creature;type-line/{}", context.type_line),
            )
        }
        SourceKind::Land
            if !discard_another_card
                && reminder
                    == format!(
                        "{expected_cost} Create a token that's a copy of it, except it's a 4/4 black Zombie creature and loses all other card types. Eternalize only as a sorcery. And don't forget it still enters tapped!"
                    ) =>
        {
            (
                TokenCopyException::EternalizeLandCreature,
                format!("source-kind/land;type-line/{}", context.type_line),
            )
        }
        _ => return None,
    };
    Some((
        ZoneKeywordKind::Eternalize {
            mana_cost,
            discard_another_card,
            token_exception,
        },
        relevant_context,
    ))
}

fn valid_copied_subtype_suffix(source: &str) -> bool {
    !source.is_empty()
        && source.starts_with(' ')
        && source
            .chars()
            .all(|character| character.is_alphabetic() || matches!(character, ' ' | '-' | '\''))
}

fn parse_scavenge(
    source: &str,
    _context: SourceSemanticContext<'_>,
) -> Option<(ZoneKeywordKind, String)> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let cost = parse_mana_cost(core.strip_prefix("Scavenge ")?)?;
    if let Some(reminder) = reminder {
        let expected = format!(
            "{}, Exile this card from your graveyard: Put a number of +1/+1 counters equal to this card's power on target creature. Scavenge only as a sorcery.",
            cost.exact()
        );
        if reminder != expected {
            return None;
        }
    }
    Some((ZoneKeywordKind::Scavenge { cost }, String::new()))
}

fn parse_recover(source: &str) -> Option<(ZoneKeywordKind, String)> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    if core == "Recover\u{2014}Pay half your life, rounded up." {
        let exact = "When another creature is put into your graveyard from the battlefield, you may pay half your life, rounded up. If you do, return this card from your graveyard to your hand. Otherwise, exile this card.";
        if reminder? != exact {
            return None;
        }
        return Some((
            ZoneKeywordKind::Recover {
                cost: RecoverCost::HalfLifeRoundedUp,
                trigger_requires_another_creature: true,
            },
            String::new(),
        ));
    }
    let cost = parse_mana_cost(core.strip_prefix("Recover ")?)?;
    if let Some(reminder) = reminder {
        let expected = format!(
            "When a creature is put into your graveyard from the battlefield, you may pay {}. If you do, return this card from your graveyard to your hand. Otherwise, exile this card.",
            cost.exact()
        );
        if reminder != expected {
            return None;
        }
    }
    Some((
        ZoneKeywordKind::Recover {
            cost: RecoverCost::Mana(cost),
            trigger_requires_another_creature: false,
        },
        String::new(),
    ))
}

fn parse_dredge(source: &str) -> Option<(ZoneKeywordKind, String)> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let amount = core.strip_prefix("Dredge ")?.parse::<u32>().ok()?;
    if amount == 0 {
        return None;
    }
    if let Some(reminder) = reminder {
        let card_count = number_word(amount)?;
        let noun = if amount == 1 { "card" } else { "cards" };
        let expected = format!(
            "If you would draw a card, you may mill {card_count} {noun} instead. If you do, return this card from your graveyard to your hand."
        );
        if reminder != expected {
            return None;
        }
    }
    Some((ZoneKeywordKind::Dredge { amount }, String::new()))
}

fn parse_transmute(
    source: &str,
    context: SourceSemanticContext<'_>,
) -> Option<(ZoneKeywordKind, String)> {
    let mana_value = context.mana_value?;
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let cost = parse_mana_cost(core.strip_prefix("Transmute ")?)?;
    let reminder = reminder?;
    let prefix = format!(
        "{}, Discard this card: Search your library for a card with ",
        cost.exact()
    );
    let target = reminder.strip_prefix(&prefix)?.strip_suffix(
        ", reveal it, put it into your hand, then shuffle. Transmute only as a sorcery.",
    )?;
    match target {
        "the same mana value as this card" => {}
        literal => {
            let literal = literal.strip_prefix("mana value ")?.parse::<u32>().ok()?;
            if literal != mana_value {
                return None;
            }
        }
    }
    Some((
        ZoneKeywordKind::Transmute {
            cost,
            target_mana_value: mana_value,
        },
        format!("source-mana-value/{mana_value}"),
    ))
}

fn parse_reinforce(source: &str) -> Option<(ZoneKeywordKind, String)> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let specification = core.strip_prefix("Reinforce ")?;
    let (amount_text, cost_text) = specification.split_once('\u{2014}')?;
    let amount = match amount_text {
        "X" => ReinforceAmount::ChosenX,
        digits => {
            let value = digits.parse::<u32>().ok()?;
            if value == 0 {
                return None;
            }
            ReinforceAmount::Fixed(value)
        }
    };
    let cost = parse_mana_cost(cost_text)?;
    if matches!(amount, ReinforceAmount::ChosenX) != (cost.x_symbols() > 0) {
        return None;
    }
    if let Some(reminder) = reminder {
        let counter_text = match amount {
            ReinforceAmount::Fixed(value) => {
                let word = number_word(value)?;
                let noun = if value == 1 { "counter" } else { "counters" };
                format!("{word} +1/+1 {noun}")
            }
            ReinforceAmount::ChosenX => "X +1/+1 counters".to_owned(),
        };
        let expected = format!(
            "{}, Discard this card: Put {counter_text} on target creature.",
            cost.exact()
        );
        if reminder != expected {
            return None;
        }
    }
    Some((ZoneKeywordKind::Reinforce { amount, cost }, String::new()))
}

fn parse_forecast(source: &str) -> Option<(ZoneKeywordKind, String)> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let ability = core.strip_prefix("Forecast \u{2014} ")?;
    let (cost_text, effect_text) = ability.split_once(": ")?;
    let cost = if cost_text
        == "Tap two untapped white and/or blue creatures you control, Reveal this card from your hand"
    {
        ForecastCost::TapTwoWhiteOrBlueCreaturesAndReveal
    } else {
        let mana_and_reveal = cost_text
            .strip_suffix(", Reveal this card from your hand")
            .or_else(|| cost_text.strip_suffix(", Reveal this creature from your hand"))?;
        ForecastCost::ManaAndReveal(parse_mana_cost(mana_and_reveal)?)
    };
    if reminder.is_none_or(|reminder| {
        reminder != "Activate only during your upkeep and only once each turn."
            && reminder != "Activate this ability only during your upkeep and only once each turn."
    }) {
        return None;
    }
    let effect = match effect_text {
        "Target creature becomes the color or colors of your choice until end of turn." => {
            ForecastEffect::SetTargetCreatureColorsUntilEndOfTurn
        }
        "Target creature with power 2 or less can't be blocked this turn." => {
            ForecastEffect::TargetSmallCreatureCantBeBlockedThisTurn
        }
        "Whenever target creature deals damage this turn, you gain that much life." => {
            ForecastEffect::GainLifeWhenTargetCreatureDealsDamageThisTurn
        }
        "Target creature gains shadow until end of turn." => {
            ForecastEffect::GrantTargetCreatureKeywordUntilEndOfTurn(GrantedKeyword::Shadow)
        }
        "Each player draws a card." => ForecastEffect::EachPlayerDrawsOne,
        "Tap target untapped creature." => ForecastEffect::TapTargetCreature {
            must_be_untapped: true,
        },
        "Create a 1/1 white and blue Bird creature token with flying." => {
            ForecastEffect::CreateWhiteBlueBird
        }
        "Return target creature card with mana value 1 or less from your graveyard to the battlefield." => {
            ForecastEffect::ReturnSmallCreatureCardFromGraveyard
        }
        "Target creature gets +1/+1 until end of turn." => {
            ForecastEffect::PumpTargetCreature(PumpEffect {
                power: NumberValue::Fixed(1),
                toughness: NumberValue::Fixed(1),
                granted_keywords: BTreeSet::new(),
                target_must_be_attacking: false,
            })
        }
        "Tap target creature." => ForecastEffect::TapTargetCreature {
            must_be_untapped: false,
        },
        "Draw a card." if matches!(cost, ForecastCost::TapTwoWhiteOrBlueCreaturesAndReveal) => {
            ForecastEffect::DrawOne
        }
        _ => return None,
    };
    Some((ZoneKeywordKind::Forecast { cost, effect }, String::new()))
}

fn parse_bloodrush(source: &str) -> Option<(ZoneKeywordKind, String)> {
    let ability = source.strip_prefix("Bloodrush \u{2014} ")?;
    let (cost_text, effect_text) = ability.split_once(": ")?;
    let cost = parse_mana_cost(cost_text.strip_suffix(", Discard this card")?)?;
    let pump = parse_target_creature_pump(effect_text, true)?;
    Some((ZoneKeywordKind::Bloodrush { cost, pump }, String::new()))
}

fn parse_channel(source: &str) -> Option<(ZoneKeywordKind, String)> {
    let ability = source.strip_prefix("Channel \u{2014} ")?;
    let (cost_text, mut effect_text) = ability.split_once(": ")?;
    let cost = parse_mana_cost(cost_text.strip_suffix(", Discard this card")?)?;
    let timing = if let Some(body) = effect_text.strip_suffix(" Activate only as a sorcery.") {
        effect_text = body;
        ProgramTiming::Sorcery
    } else {
        ProgramTiming::AnyPriority
    };
    let legendary_creature_reduction = if let Some(body) = effect_text.strip_suffix(
        " This ability costs {1} less to activate for each legendary creature you control.",
    ) {
        effect_text = body;
        true
    } else {
        false
    };
    let effect = parse_channel_effect(effect_text)?;
    Some((
        ZoneKeywordKind::Channel {
            cost,
            timing,
            legendary_creature_reduction,
            effect,
        },
        String::new(),
    ))
}

fn parse_channel_effect(source: &str) -> Option<ChannelEffect> {
    if source == "Target creature can't block this turn." {
        return Some(ChannelEffect::TargetCreatureCantBlockThisTurn);
    }
    if source == "Create a 1/1 green Human Monk creature token with \"{T}: Add {G}.\"" {
        return Some(ChannelEffect::CreateGreenHumanMonk);
    }
    if let Some(keyword) = source
        .strip_prefix("Target creature gains ")
        .and_then(|body| body.strip_suffix(" until end of turn."))
        .and_then(|keyword| match keyword {
            "first strike" => Some(GrantedKeyword::FirstStrike),
            "flying" => Some(GrantedKeyword::Flying),
            "haste" => Some(GrantedKeyword::Haste),
            _ => None,
        })
    {
        return Some(ChannelEffect::GrantTargetCreatureKeywordUntilEndOfTurn(
            keyword,
        ));
    }
    if let Some(pump) = parse_target_creature_pump(source, false) {
        return Some(ChannelEffect::PumpTargetCreature(pump));
    }
    match source {
        "Draw a card." => Some(ChannelEffect::DrawOne),
        "It deals 2 damage to any target." => Some(ChannelEffect::DealDamage {
            amount: NumberValue::Fixed(2),
            target: DamageTarget::Any,
        }),
        "Return target creature to its owner's hand." => {
            Some(ChannelEffect::ReturnTargetToHand(TargetFilter::Creature))
        }
        "Exile target artifact or creature. Return it to the battlefield under its owner's control at the beginning of the next end step." => {
            Some(ChannelEffect::BlinkTargetUntilNextEndStep(
                TargetFilter::ArtifactOrCreature,
            ))
        }
        "You gain 4 life." => Some(ChannelEffect::GainLife(4)),
        "Search your library for a basic Plains card, reveal it, put it into your hand, then shuffle. You gain 2 life." => {
            Some(ChannelEffect::SearchBasicPlainsToHandThenGainLife(2))
        }
        "Put a +1/+1 counter on each of up to two target creatures you control." => {
            Some(ChannelEffect::PutCountersOnUpToTwoControlledCreatures)
        }
        "Return target card from your graveyard to your hand." => Some(
            ChannelEffect::ReturnTargetCardFromGraveyardToHand(TargetFilter::AnyCard),
        ),
        "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle." => {
            Some(ChannelEffect::SearchBasicLandToBattlefieldTapped)
        }
        "All creatures able to block target creature this turn do so." => {
            Some(ChannelEffect::ForceAllAbleCreaturesToBlockTarget)
        }
        "Create a 4/4 red Dragon creature token with flying." => {
            Some(ChannelEffect::CreateRedDragon)
        }
        "Counter target spell or ability unless its controller pays {3}." => {
            Some(ChannelEffect::CounterUnlessControllerPays {
                target: CounterTarget::SpellOrAbility,
                generic_mana: 3,
            })
        }
        "Create two 1/1 colorless Pilot creature tokens with \"This token crews Vehicles as though its power were 2 greater.\"" => {
            Some(ChannelEffect::CreateTwoPilotTokens)
        }
        "Mill three cards, then return a creature or planeswalker card from your graveyard to your hand." => {
            Some(ChannelEffect::MillThenReturnCreatureOrPlaneswalker)
        }
        "Create two 1/1 colorless Spirit creature tokens. They gain haste until end of turn." => {
            Some(ChannelEffect::CreateTwoHastySpirits)
        }
        "It deals 4 damage to target creature." => Some(ChannelEffect::DealDamage {
            amount: NumberValue::Fixed(4),
            target: DamageTarget::Creature,
        }),
        "Counter target spell unless its controller pays {4}." => {
            Some(ChannelEffect::CounterUnlessControllerPays {
                target: CounterTarget::Spell,
                generic_mana: 4,
            })
        }
        "Tap up to two target creatures you don't control. Those creatures don't untap during their controller's next untap step." => {
            Some(ChannelEffect::TapAndFreezeUpToTwoOpposingCreatures)
        }
        "Destroy target creature with flying." => Some(ChannelEffect::DestroyTarget(
            TargetFilter::CreatureWithFlying,
        )),
        "The owner of target nonland permanent puts it on their choice of the top or bottom of their library." => {
            Some(ChannelEffect::PutTargetOnLibraryTopOrBottom)
        }
        "Target player discards four cards." => Some(ChannelEffect::TargetPlayerDiscards(4)),
        "Put a +1/+1 counter on each creature you control. Draw a card." => {
            Some(ChannelEffect::PutCounterOnEachControlledCreatureThenDraw)
        }
        "It deals X damage to each creature with flying." => {
            Some(ChannelEffect::DamageEachCreature(
                TargetFilter::CreatureWithFlying,
                NumberValue::ChosenX,
            ))
        }
        "Put X +1/+1 counters on target land you control. It becomes a 0/0 green Spirit creature with haste. It's still a land." => {
            Some(ChannelEffect::AnimateTargetLandWithCounters)
        }
        "It deals X damage to each creature without flying." => {
            Some(ChannelEffect::DamageEachCreature(
                TargetFilter::CreatureWithoutFlying,
                NumberValue::ChosenX,
            ))
        }
        "X target creatures gain flying until end of turn." => {
            Some(ChannelEffect::GrantFlyingToXTargets)
        }
        "Return X target nonlegendary cards from your graveyard to your hand." => {
            Some(ChannelEffect::ReturnXNonlegendaryCardsFromGraveyard)
        }
        "It deals 4 damage to target attacking or blocking creature." => {
            Some(ChannelEffect::DealDamage {
                amount: NumberValue::Fixed(4),
                target: DamageTarget::AttackingOrBlockingCreature,
            })
        }
        _ => None,
    }
}

fn parse_target_creature_pump(source: &str, attacking: bool) -> Option<PumpEffect> {
    let prefix = if attacking {
        "Target attacking creature gets +"
    } else {
        "Target creature gets +"
    };
    let body = source.strip_prefix(prefix)?;
    let (body, dynamic_lands) = if let Some(body) =
        body.strip_suffix(" until end of turn, where X is the number of lands you control.")
    {
        (body, true)
    } else {
        (body.strip_suffix(" until end of turn.")?, false)
    };
    let (numbers, grants) = body
        .split_once(" and gains ")
        .map_or((body, None), |(numbers, grants)| (numbers, Some(grants)));
    let (power, toughness) = numbers.split_once("/+")?;
    let power = if power == "X" {
        NumberValue::LandsControlled
    } else {
        NumberValue::Fixed(power.parse().ok()?)
    };
    let toughness = if toughness == "X" {
        NumberValue::LandsControlled
    } else {
        NumberValue::Fixed(toughness.parse().ok()?)
    };
    if matches!(power, NumberValue::LandsControlled)
        != matches!(toughness, NumberValue::LandsControlled)
    {
        return None;
    }
    if dynamic_lands != matches!(power, NumberValue::LandsControlled) {
        return None;
    }
    let mut granted_keywords = BTreeSet::new();
    if let Some(grants) = grants {
        for grant in grants.split(" and ") {
            granted_keywords.insert(match grant {
                "deathtouch" => GrantedKeyword::Deathtouch,
                "double strike" => GrantedKeyword::DoubleStrike,
                "first strike" => GrantedKeyword::FirstStrike,
                "flying" => GrantedKeyword::Flying,
                "haste" => GrantedKeyword::Haste,
                "reach" => GrantedKeyword::Reach,
                "shadow" => GrantedKeyword::Shadow,
                "trample" => GrantedKeyword::Trample,
                _ => return None,
            });
        }
    }
    Some(PumpEffect {
        power,
        toughness,
        granted_keywords,
        target_must_be_attacking: attacking,
    })
}

fn split_trailing_parenthetical(source: &str) -> Option<(&str, Option<&str>)> {
    if !source.ends_with(')') {
        return Some((source, None));
    }
    let mut depth = 0_i32;
    let mut opening = None;
    for (index, character) in source.char_indices().rev() {
        match character {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                if depth == 0 {
                    opening = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let opening = opening?;
    let core = source[..opening].strip_suffix(' ')?;
    let reminder = &source[opening + 1..source.len() - 1];
    if core.is_empty() || reminder.is_empty() {
        return None;
    }
    Some((core, Some(reminder)))
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    if source.is_empty() || source.trim() != source {
        return None;
    }
    let mut symbols = Vec::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        if source.as_bytes().get(cursor).copied()? != b'{' {
            return None;
        }
        let end = cursor + source[cursor..].find('}')?;
        let token = &source[cursor + 1..end];
        let symbol = match token {
            "W" => ManaSymbol::Colored(ManaColor::White),
            "U" => ManaSymbol::Colored(ManaColor::Blue),
            "B" => ManaSymbol::Colored(ManaColor::Black),
            "R" => ManaSymbol::Colored(ManaColor::Red),
            "G" => ManaSymbol::Colored(ManaColor::Green),
            "C" => ManaSymbol::Colored(ManaColor::Colorless),
            "X" => ManaSymbol::VariableX,
            digits => ManaSymbol::Generic(digits.parse::<u32>().ok()?),
        };
        symbols.push(symbol);
        cursor = end + 1;
    }
    Some(ManaCost {
        exact: source.to_owned(),
        symbols,
    })
}

fn number_word(value: u32) -> Option<&'static str> {
    match value {
        1 => Some("a"),
        2 => Some("two"),
        3 => Some("three"),
        4 => Some("four"),
        5 => Some("five"),
        6 => Some("six"),
        _ => None,
    }
}

fn collapse_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn semantic_digest_with_versions(
    exact_source: &str,
    kind: &ZoneKeywordKind,
    relevant_context: &str,
    compiler_version: &str,
    runtime_version: &str,
    rules_context_version: &str,
) -> String {
    let contract = format!(
        "family={};zone={};timing={}",
        kind.label(),
        kind.source_zone().stable_id(),
        kind.timing().stable_id()
    );
    let mut hasher = Sha256::new();
    for component in [
        "graveyard-hand-library-keyword-content/v1",
        compiler_version,
        runtime_version,
        rules_context_version,
        exact_source,
        relevant_context,
        &contract,
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncarnationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub id: ObjectId,
    pub incarnation: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Void,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Beginning,
    Upkeep,
    Draw,
    PrecombatMain,
    Combat,
    PostcombatMain,
    End,
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
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPool {
    amounts: BTreeMap<ManaColor, u32>,
}

impl ManaPool {
    pub fn empty() -> Self {
        Self {
            amounts: BTreeMap::new(),
        }
    }

    pub fn with(mut self, color: ManaColor, amount: u32) -> Self {
        self.amounts.insert(color, amount);
        self
    }

    pub fn amount(&self, color: ManaColor) -> u32 {
        self.amounts.get(&color).copied().unwrap_or(0)
    }

    fn subtract(&mut self, payment: &ManaPayment) -> Result<(), RuntimeError> {
        for color in [
            ManaColor::White,
            ManaColor::Blue,
            ManaColor::Black,
            ManaColor::Red,
            ManaColor::Green,
            ManaColor::Colorless,
        ] {
            let spend = payment.amount(color);
            let available = self.amount(color);
            if spend > available {
                return Err(RuntimeError::InsufficientMana);
            }
            self.amounts.insert(color, available - spend);
        }
        Ok(())
    }
}

impl Default for ManaPool {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPayment {
    amounts: BTreeMap<ManaColor, u32>,
}

impl ManaPayment {
    pub fn none() -> Self {
        Self {
            amounts: BTreeMap::new(),
        }
    }

    pub fn with(mut self, color: ManaColor, amount: u32) -> Self {
        self.amounts.insert(color, amount);
        self
    }

    pub fn amount(&self, color: ManaColor) -> u32 {
        self.amounts.get(&color).copied().unwrap_or(0)
    }

    fn total(&self) -> u32 {
        self.amounts.values().copied().sum()
    }
}

impl Default for ManaPayment {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameObject {
    pub id: ObjectId,
    pub incarnation: IncarnationId,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub printed_type_line: String,
    pub mana_value: u32,
    pub oracle_lines: BTreeSet<String>,
    pub card_types: BTreeSet<CardType>,
    pub subtypes: BTreeSet<String>,
    pub colors: BTreeSet<ManaColor>,
    pub mana_cost: Option<ManaCost>,
    pub base_power: Option<i32>,
    pub base_toughness: Option<i32>,
    pub power_modifier: i32,
    pub toughness_modifier: i32,
    pub plus_one_counters: u32,
    pub loyalty: Option<i32>,
    pub defense: Option<i32>,
    pub printed_keywords: BTreeSet<GrantedKeyword>,
    pub granted_keywords: BTreeSet<GrantedKeyword>,
    pub temporary_keywords: BTreeSet<GrantedKeyword>,
    pub temporary_colors: Option<BTreeSet<ManaColor>>,
    pub tapped: bool,
    pub attacking: bool,
    pub blocking: bool,
    pub token: bool,
    pub legendary: bool,
    pub basic: bool,
    pub basic_land_types: BTreeSet<String>,
    pub enters_tapped: bool,
    pub indestructible: bool,
    pub regeneration_shields: u32,
    pub damage_marked: u32,
    pub prevent_next_damage: u32,
    pub targetable: bool,
    pub cannot_block_this_turn: bool,
    pub cannot_be_blocked_this_turn: bool,
    pub must_be_blocked_this_turn: bool,
    pub skip_next_untap: bool,
    pub crew_power_bonus: i32,
    pub taps_for: Option<ManaColor>,
    intrinsic_card_types: BTreeSet<CardType>,
    intrinsic_subtypes: BTreeSet<String>,
    intrinsic_colors: BTreeSet<ManaColor>,
    intrinsic_mana_cost: Option<ManaCost>,
    intrinsic_mana_value: u32,
    intrinsic_base_power: Option<i32>,
    intrinsic_base_toughness: Option<i32>,
    intrinsic_keywords: BTreeSet<GrantedKeyword>,
}

impl GameObject {
    pub fn new_card(
        id: ObjectId,
        owner: PlayerId,
        zone: Zone,
        type_line: impl Into<String>,
        mana_value: u32,
    ) -> Self {
        Self {
            id,
            incarnation: IncarnationId(0),
            owner,
            controller: owner,
            zone,
            printed_type_line: type_line.into(),
            mana_value,
            oracle_lines: BTreeSet::new(),
            card_types: BTreeSet::new(),
            subtypes: BTreeSet::new(),
            colors: BTreeSet::new(),
            mana_cost: None,
            base_power: None,
            base_toughness: None,
            power_modifier: 0,
            toughness_modifier: 0,
            plus_one_counters: 0,
            loyalty: None,
            defense: None,
            printed_keywords: BTreeSet::new(),
            granted_keywords: BTreeSet::new(),
            temporary_keywords: BTreeSet::new(),
            temporary_colors: None,
            tapped: false,
            attacking: false,
            blocking: false,
            token: false,
            legendary: false,
            basic: false,
            basic_land_types: BTreeSet::new(),
            enters_tapped: false,
            indestructible: false,
            regeneration_shields: 0,
            damage_marked: 0,
            prevent_next_damage: 0,
            targetable: true,
            cannot_block_this_turn: false,
            cannot_be_blocked_this_turn: false,
            must_be_blocked_this_turn: false,
            skip_next_untap: false,
            crew_power_bonus: 0,
            taps_for: None,
            intrinsic_card_types: BTreeSet::new(),
            intrinsic_subtypes: BTreeSet::new(),
            intrinsic_colors: BTreeSet::new(),
            intrinsic_mana_cost: None,
            intrinsic_mana_value: mana_value,
            intrinsic_base_power: None,
            intrinsic_base_toughness: None,
            intrinsic_keywords: BTreeSet::new(),
        }
    }

    pub fn object_ref(&self) -> ObjectRef {
        ObjectRef {
            id: self.id,
            incarnation: self.incarnation,
        }
    }

    pub fn power(&self) -> Option<i32> {
        self.base_power
            .map(|power| power + self.power_modifier + self.plus_one_counters as i32)
    }

    pub fn toughness(&self) -> Option<i32> {
        self.base_toughness
            .map(|toughness| toughness + self.toughness_modifier + self.plus_one_counters as i32)
    }

    pub fn has_keyword(&self, keyword: GrantedKeyword) -> bool {
        self.printed_keywords.contains(&keyword)
            || self.granted_keywords.contains(&keyword)
            || self.temporary_keywords.contains(&keyword)
    }

    fn semantic_context(&self) -> SourceSemanticContext<'_> {
        SourceSemanticContext {
            type_line: &self.printed_type_line,
            mana_value: Some(self.mana_value),
        }
    }

    fn seal_intrinsic_values(&mut self) {
        self.intrinsic_card_types = self.card_types.clone();
        self.intrinsic_subtypes = self.subtypes.clone();
        self.intrinsic_colors = self.colors.clone();
        self.intrinsic_mana_cost = self.mana_cost.clone();
        self.intrinsic_mana_value = self.mana_value;
        self.intrinsic_base_power = self.base_power;
        self.intrinsic_base_toughness = self.base_toughness;
        self.intrinsic_keywords = self.printed_keywords.clone();
    }

    fn restore_intrinsic_values(&mut self) {
        self.card_types = self.intrinsic_card_types.clone();
        self.subtypes = self.intrinsic_subtypes.clone();
        self.colors = self.intrinsic_colors.clone();
        self.mana_cost = self.intrinsic_mana_cost.clone();
        self.mana_value = self.intrinsic_mana_value;
        self.base_power = self.intrinsic_base_power;
        self.base_toughness = self.intrinsic_base_toughness;
        self.printed_keywords = self.intrinsic_keywords.clone();
        self.granted_keywords.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub id: PlayerId,
    pub life: i32,
    pub mana_pool: ManaPool,
    pub in_game: bool,
    pub failed_draw_from_empty_library: bool,
    library: Vec<ObjectId>,
}

impl PlayerState {
    pub fn new(id: PlayerId, life: i32) -> Self {
        Self {
            id,
            life,
            mana_pool: ManaPool::empty(),
            in_game: true,
            failed_draw_from_empty_library: false,
            library: Vec::new(),
        }
    }

    pub fn library_top_first(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.library.iter().rev().copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackItemKind {
    Spell,
    ActivatedAbility,
    TriggeredAbility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackItem {
    pub id: u64,
    pub controller: PlayerId,
    pub kind: StackItemKind,
    pub targetable: bool,
    pub countered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    Object(ObjectRef),
    Player(PlayerId),
    Stack(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationRequest {
    pub actor: PlayerId,
    pub source: ObjectRef,
    pub mana_payment: ManaPayment,
    pub x_value: u32,
    pub targets: Vec<Target>,
    pub additional_discard: Option<ObjectRef>,
    pub tap_costs: Vec<ObjectRef>,
}

impl ActivationRequest {
    pub fn new(actor: PlayerId, source: ObjectRef) -> Self {
        Self {
            actor,
            source,
            mana_payment: ManaPayment::none(),
            x_value: 0,
            targets: Vec::new(),
            additional_discard: None,
            tap_costs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAbility {
    pub controller: PlayerId,
    pub program: ZoneKeywordProgram,
    pub source_at_activation: ObjectRef,
    pub source_snapshot: GameObject,
    pub targets: Vec<Target>,
    pub x_value: u32,
    pub copied: bool,
}

impl PendingAbility {
    pub fn copy_without_repaying_costs(&self) -> Self {
        let mut copied = self.clone();
        copied.copied = true;
        copied
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryPlacement {
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DredgeChoice {
    pub program: ZoneKeywordProgram,
    pub source: ObjectRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolutionChoices {
    pub selected_library_card: Option<ObjectId>,
    pub library_order_after_shuffle: Option<Vec<ObjectId>>,
    pub selected_graveyard_card: Option<ObjectId>,
    pub library_placement: Option<LibraryPlacement>,
    pub counter_payment: Option<ManaPayment>,
    pub discard_cards: Vec<ObjectRef>,
    pub chosen_colors: BTreeSet<ManaColor>,
    pub draw_replacements: BTreeMap<PlayerId, DredgeChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverTrigger {
    pub controller: PlayerId,
    pub program: ZoneKeywordProgram,
    pub source: ObjectRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverResolutionChoice {
    pub pay: bool,
    pub mana_payment: ManaPayment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedReturn {
    pub object: ObjectRef,
    pub controller_on_return: PlayerId,
    pub created_step_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageLifeTriggerWindow {
    pub creature: ObjectRef,
    pub beneficiary: PlayerId,
    pub turn: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    Revealed(ObjectRef),
    Shuffled(PlayerId),
    Drew {
        player: PlayerId,
        card: ObjectRef,
    },
    Milled {
        player: PlayerId,
        card: ObjectRef,
    },
    DamageToPlayer {
        player: PlayerId,
        amount: u32,
    },
    DamageToObject {
        object: ObjectRef,
        amount: u32,
    },
    ZoneChanged {
        old: ObjectRef,
        new: ObjectRef,
        owner: PlayerId,
        was_creature: bool,
        from: Zone,
        to: Zone,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionOutcome {
    Resolved,
    CounteredByRules,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    UnsupportedProgram,
    InvalidSource,
    InvalidTiming,
    InvalidTarget,
    InvalidTargetCount,
    InvalidCostChoice,
    InsufficientMana,
    IllegalManaPayment,
    InvalidHiddenZoneChoice,
    InvalidShuffle,
    ReplacementUnavailable,
    MissingResolutionChoice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    pub turn: u64,
    pub phase: Phase,
    pub active_player: PlayerId,
    pub priority_player: PlayerId,
    pub stack_empty: bool,
    pub step_sequence: u64,
    pub players: BTreeMap<PlayerId, PlayerState>,
    pub objects: BTreeMap<ObjectId, GameObject>,
    pub stack_items: BTreeMap<u64, StackItem>,
    pub player_order: Vec<PlayerId>,
    pub delayed_returns: Vec<DelayedReturn>,
    pub damage_life_windows: Vec<DamageLifeTriggerWindow>,
    pub events: Vec<GameEvent>,
    forecast_uses: BTreeSet<(ObjectRef, String, u64)>,
    next_object_id: u64,
}

impl GameState {
    pub fn new(player_order: Vec<PlayerId>, active_player: PlayerId) -> Self {
        let players = player_order
            .iter()
            .copied()
            .map(|player| (player, PlayerState::new(player, 40)))
            .collect();
        Self {
            turn: 1,
            phase: Phase::PrecombatMain,
            active_player,
            priority_player: active_player,
            stack_empty: true,
            step_sequence: 1,
            players,
            objects: BTreeMap::new(),
            stack_items: BTreeMap::new(),
            player_order,
            delayed_returns: Vec::new(),
            damage_life_windows: Vec::new(),
            events: Vec::new(),
            forecast_uses: BTreeSet::new(),
            next_object_id: 1,
        }
    }

    pub fn insert_object(&mut self, mut object: GameObject) -> Result<ObjectRef, RuntimeError> {
        if self.objects.contains_key(&object.id) || !self.players.contains_key(&object.owner) {
            return Err(RuntimeError::InvalidSource);
        }
        object.seal_intrinsic_values();
        self.next_object_id = self.next_object_id.max(object.id.0.saturating_add(1));
        let object_ref = object.object_ref();
        if object.zone == Zone::Library {
            self.players
                .get_mut(&object.owner)
                .ok_or(RuntimeError::InvalidSource)?
                .library
                .insert(0, object.id);
        }
        self.objects.insert(object.id, object);
        Ok(object_ref)
    }

    pub fn set_library_top_first(
        &mut self,
        player: PlayerId,
        top_first: &[ObjectId],
    ) -> Result<(), RuntimeError> {
        let expected = self
            .objects
            .values()
            .filter(|object| object.owner == player && object.zone == Zone::Library)
            .map(|object| object.id)
            .collect::<BTreeSet<_>>();
        if expected != top_first.iter().copied().collect() || top_first.len() != expected.len() {
            return Err(RuntimeError::InvalidShuffle);
        }
        let state = self
            .players
            .get_mut(&player)
            .ok_or(RuntimeError::InvalidSource)?;
        state.library = top_first.iter().rev().copied().collect();
        Ok(())
    }

    pub fn current_object(&self, reference: ObjectRef) -> Option<&GameObject> {
        self.objects
            .get(&reference.id)
            .filter(|object| object.incarnation == reference.incarnation)
    }

    pub fn activate(
        &mut self,
        program: &ZoneKeywordProgram,
        request: ActivationRequest,
    ) -> Result<PendingAbility, RuntimeError> {
        let mut staged = self.clone();
        let pending = staged.activate_inner(program, request)?;
        *self = staged;
        Ok(pending)
    }

    fn activate_inner(
        &mut self,
        program: &ZoneKeywordProgram,
        request: ActivationRequest,
    ) -> Result<PendingAbility, RuntimeError> {
        if matches!(
            program.kind(),
            ZoneKeywordKind::Recover { .. } | ZoneKeywordKind::Dredge { .. }
        ) {
            return Err(RuntimeError::UnsupportedProgram);
        }
        let source = self
            .current_object(request.source)
            .cloned()
            .ok_or(RuntimeError::InvalidSource)?;
        self.validate_program_source(program, &source)?;
        if source.owner != request.actor {
            return Err(RuntimeError::InvalidSource);
        }
        self.validate_timing(program.kind(), request.actor)?;
        self.validate_activation_targets(
            program.kind(),
            request.actor,
            request.x_value,
            &request.targets,
        )?;
        self.validate_nonmana_cost_choices(program, &request)?;

        let reduction = match program.kind() {
            ZoneKeywordKind::Channel {
                legendary_creature_reduction: true,
                ..
            } => self
                .objects
                .values()
                .filter(|object| {
                    object.zone == Zone::Battlefield
                        && object.controller == request.actor
                        && object.legendary
                        && object.card_types.contains(&CardType::Creature)
                })
                .count() as u32,
            _ => 0,
        };
        if let Some(cost) = activation_mana_cost(program.kind()) {
            self.pay_mana(
                request.actor,
                cost,
                request.x_value,
                reduction,
                &request.mana_payment,
            )?;
        } else if request.mana_payment.total() != 0 || request.x_value != 0 {
            return Err(RuntimeError::IllegalManaPayment);
        }

        match program.kind() {
            ZoneKeywordKind::Embalm { .. } | ZoneKeywordKind::Scavenge { .. } => {
                self.move_object(request.source, Zone::Exile, None)?;
            }
            ZoneKeywordKind::Eternalize {
                discard_another_card,
                ..
            } => {
                if *discard_another_card {
                    self.move_object(
                        request
                            .additional_discard
                            .ok_or(RuntimeError::InvalidCostChoice)?,
                        Zone::Graveyard,
                        None,
                    )?;
                }
                self.move_object(request.source, Zone::Exile, None)?;
            }
            ZoneKeywordKind::Transmute { .. }
            | ZoneKeywordKind::Reinforce { .. }
            | ZoneKeywordKind::Bloodrush { .. }
            | ZoneKeywordKind::Channel { .. } => {
                self.move_object(request.source, Zone::Graveyard, None)?;
            }
            ZoneKeywordKind::Forecast { cost, .. } => {
                match cost {
                    ForecastCost::ManaAndReveal(_) => {}
                    ForecastCost::TapTwoWhiteOrBlueCreaturesAndReveal => {
                        for reference in &request.tap_costs {
                            self.objects
                                .get_mut(&reference.id)
                                .ok_or(RuntimeError::InvalidCostChoice)?
                                .tapped = true;
                        }
                    }
                }
                self.events.push(GameEvent::Revealed(request.source));
                self.forecast_uses.insert((
                    request.source,
                    program.semantic_digest().to_owned(),
                    self.turn,
                ));
            }
            ZoneKeywordKind::Recover { .. } | ZoneKeywordKind::Dredge { .. } => {
                return Err(RuntimeError::UnsupportedProgram);
            }
        }

        Ok(PendingAbility {
            controller: request.actor,
            program: program.clone(),
            source_at_activation: request.source,
            source_snapshot: source,
            targets: request.targets,
            x_value: request.x_value,
            copied: false,
        })
    }

    fn validate_program_source(
        &self,
        program: &ZoneKeywordProgram,
        source: &GameObject,
    ) -> Result<(), RuntimeError> {
        let expected_zone = match program.kind().source_zone() {
            SourceZone::Graveyard => Zone::Graveyard,
            SourceZone::Hand => Zone::Hand,
        };
        if source.zone != expected_zone
            || !source.oracle_lines.contains(program.exact_source())
            || compile_zone_keyword_program(program.exact_source(), source.semantic_context())
                .is_none_or(|compiled| compiled.semantic_digest() != program.semantic_digest())
        {
            return Err(RuntimeError::InvalidSource);
        }
        Ok(())
    }

    fn validate_timing(&self, kind: &ZoneKeywordKind, actor: PlayerId) -> Result<(), RuntimeError> {
        match kind.timing() {
            ProgramTiming::Sorcery => {
                if self.active_player != actor
                    || !matches!(self.phase, Phase::PrecombatMain | Phase::PostcombatMain)
                    || !self.stack_empty
                    || self.priority_player != actor
                {
                    return Err(RuntimeError::InvalidTiming);
                }
            }
            ProgramTiming::AnyPriority => {
                if self.priority_player != actor {
                    return Err(RuntimeError::InvalidTiming);
                }
            }
            ProgramTiming::UpkeepOncePerTurn => {
                if self.active_player != actor
                    || self.phase != Phase::Upkeep
                    || self.priority_player != actor
                {
                    return Err(RuntimeError::InvalidTiming);
                }
            }
            ProgramTiming::Triggered | ProgramTiming::DrawReplacement => {
                return Err(RuntimeError::UnsupportedProgram);
            }
        }
        Ok(())
    }

    fn validate_nonmana_cost_choices(
        &self,
        program: &ZoneKeywordProgram,
        request: &ActivationRequest,
    ) -> Result<(), RuntimeError> {
        let kind = program.kind();
        match kind {
            ZoneKeywordKind::Eternalize {
                discard_another_card: true,
                ..
            } => {
                let discard = request
                    .additional_discard
                    .ok_or(RuntimeError::InvalidCostChoice)?;
                let object = self
                    .current_object(discard)
                    .ok_or(RuntimeError::InvalidCostChoice)?;
                if object.zone != Zone::Hand
                    || object.owner != request.actor
                    || discard == request.source
                {
                    return Err(RuntimeError::InvalidCostChoice);
                }
            }
            ZoneKeywordKind::Eternalize {
                discard_another_card: false,
                ..
            }
            | ZoneKeywordKind::Embalm { .. }
            | ZoneKeywordKind::Scavenge { .. }
            | ZoneKeywordKind::Transmute { .. }
            | ZoneKeywordKind::Reinforce { .. }
            | ZoneKeywordKind::Bloodrush { .. }
            | ZoneKeywordKind::Channel { .. } => {
                if request.additional_discard.is_some() || !request.tap_costs.is_empty() {
                    return Err(RuntimeError::InvalidCostChoice);
                }
            }
            ZoneKeywordKind::Forecast { cost, .. } => match cost {
                ForecastCost::ManaAndReveal(_) => {
                    if request.additional_discard.is_some() || !request.tap_costs.is_empty() {
                        return Err(RuntimeError::InvalidCostChoice);
                    }
                }
                ForecastCost::TapTwoWhiteOrBlueCreaturesAndReveal => {
                    if request.additional_discard.is_some()
                        || request.tap_costs.len() != 2
                        || request.tap_costs[0] == request.tap_costs[1]
                    {
                        return Err(RuntimeError::InvalidCostChoice);
                    }
                    for reference in &request.tap_costs {
                        let object = self
                            .current_object(*reference)
                            .ok_or(RuntimeError::InvalidCostChoice)?;
                        if object.zone != Zone::Battlefield
                            || object.controller != request.actor
                            || object.tapped
                            || !object.card_types.contains(&CardType::Creature)
                            || (!object.colors.contains(&ManaColor::White)
                                && !object.colors.contains(&ManaColor::Blue))
                        {
                            return Err(RuntimeError::InvalidCostChoice);
                        }
                    }
                }
            },
            ZoneKeywordKind::Recover { .. } | ZoneKeywordKind::Dredge { .. } => {
                return Err(RuntimeError::UnsupportedProgram);
            }
        }
        if matches!(kind, ZoneKeywordKind::Forecast { .. })
            && self.forecast_uses.contains(&(
                request.source,
                program.semantic_digest().to_owned(),
                self.turn,
            ))
        {
            return Err(RuntimeError::InvalidTiming);
        }
        Ok(())
    }

    fn pay_mana(
        &mut self,
        actor: PlayerId,
        cost: &ManaCost,
        x_value: u32,
        generic_reduction: u32,
        payment: &ManaPayment,
    ) -> Result<(), RuntimeError> {
        if cost.x_symbols() == 0 && x_value != 0 {
            return Err(RuntimeError::IllegalManaPayment);
        }
        let mut colored_required = BTreeMap::<ManaColor, u32>::new();
        let mut generic_required = 0_u32;
        for symbol in cost.symbols() {
            match symbol {
                ManaSymbol::Generic(amount) => {
                    generic_required = generic_required.saturating_add(*amount)
                }
                ManaSymbol::VariableX => {
                    generic_required = generic_required.saturating_add(x_value)
                }
                ManaSymbol::Colored(color) => {
                    *colored_required.entry(*color).or_default() += 1;
                }
            }
        }
        generic_required = generic_required.saturating_sub(generic_reduction);
        let colored_total: u32 = colored_required.values().copied().sum();
        if payment.total() != colored_total + generic_required {
            return Err(RuntimeError::IllegalManaPayment);
        }
        for (color, required) in colored_required {
            if payment.amount(color) < required {
                return Err(RuntimeError::IllegalManaPayment);
            }
        }
        self.players
            .get_mut(&actor)
            .ok_or(RuntimeError::InvalidSource)?
            .mana_pool
            .subtract(payment)
    }

    pub fn resolve_ability(
        &mut self,
        pending: PendingAbility,
        choices: ResolutionChoices,
    ) -> Result<ResolutionOutcome, RuntimeError> {
        let mut staged = self.clone();
        let outcome = staged.resolve_ability_inner(pending, choices)?;
        staged.run_state_based_actions()?;
        *self = staged;
        Ok(outcome)
    }

    fn resolve_ability_inner(
        &mut self,
        pending: PendingAbility,
        mut choices: ResolutionChoices,
    ) -> Result<ResolutionOutcome, RuntimeError> {
        let legal_targets = pending
            .targets
            .iter()
            .copied()
            .filter(|target| {
                self.target_is_legal(
                    pending.program.kind(),
                    pending.controller,
                    pending.x_value,
                    *target,
                )
            })
            .collect::<Vec<_>>();
        if !pending.targets.is_empty() && legal_targets.is_empty() {
            return Ok(ResolutionOutcome::CounteredByRules);
        }

        match pending.program.kind() {
            ZoneKeywordKind::Embalm {
                token_exception, ..
            }
            | ZoneKeywordKind::Eternalize {
                token_exception, ..
            } => {
                self.create_copy_token(
                    pending.controller,
                    &pending.source_snapshot,
                    *token_exception,
                )?;
            }
            ZoneKeywordKind::Scavenge { .. } => {
                let power = pending
                    .source_snapshot
                    .power()
                    .ok_or(RuntimeError::InvalidSource)?
                    .max(0) as u32;
                if let Some(Target::Object(target)) = legal_targets.first() {
                    self.objects
                        .get_mut(&target.id)
                        .ok_or(RuntimeError::InvalidTarget)?
                        .plus_one_counters += power;
                }
            }
            ZoneKeywordKind::Transmute {
                target_mana_value, ..
            } => {
                self.search_library(
                    pending.controller,
                    choices.selected_library_card,
                    choices.library_order_after_shuffle.take(),
                    true,
                    |object| object.mana_value == *target_mana_value,
                )?;
            }
            ZoneKeywordKind::Reinforce { amount, .. } => {
                let counters = match amount {
                    ReinforceAmount::Fixed(amount) => *amount,
                    ReinforceAmount::ChosenX => pending.x_value,
                };
                if let Some(Target::Object(target)) = legal_targets.first() {
                    self.objects
                        .get_mut(&target.id)
                        .ok_or(RuntimeError::InvalidTarget)?
                        .plus_one_counters += counters;
                }
            }
            ZoneKeywordKind::Forecast { effect, .. } => {
                self.resolve_forecast(pending.controller, effect, &legal_targets, &mut choices)?;
            }
            ZoneKeywordKind::Bloodrush { pump, .. } => {
                if let Some(Target::Object(target)) = legal_targets.first() {
                    self.apply_pump(*target, pending.controller, pump)?;
                }
            }
            ZoneKeywordKind::Channel { effect, .. } => {
                self.resolve_channel(
                    pending.controller,
                    effect,
                    pending.x_value,
                    &legal_targets,
                    &mut choices,
                )?;
            }
            ZoneKeywordKind::Recover { .. } | ZoneKeywordKind::Dredge { .. } => {
                return Err(RuntimeError::UnsupportedProgram);
            }
        }
        Ok(ResolutionOutcome::Resolved)
    }

    pub fn recover_trigger_for_event(
        &self,
        program: &ZoneKeywordProgram,
        source: ObjectRef,
        event: &GameEvent,
    ) -> Result<Option<RecoverTrigger>, RuntimeError> {
        let ZoneKeywordKind::Recover {
            trigger_requires_another_creature,
            ..
        } = program.kind()
        else {
            return Err(RuntimeError::UnsupportedProgram);
        };
        let source_object = self
            .current_object(source)
            .ok_or(RuntimeError::InvalidSource)?;
        self.validate_program_source(program, source_object)?;
        let GameEvent::ZoneChanged {
            old,
            owner,
            was_creature: true,
            from: Zone::Battlefield,
            to: Zone::Graveyard,
            ..
        } = event
        else {
            return Ok(None);
        };
        if source_object.owner != *owner
            || (*trigger_requires_another_creature && source.id == old.id)
        {
            return Ok(None);
        }
        Ok(Some(RecoverTrigger {
            controller: *owner,
            program: program.clone(),
            source,
        }))
    }

    pub fn resolve_recover(
        &mut self,
        trigger: RecoverTrigger,
        choice: RecoverResolutionChoice,
    ) -> Result<(), RuntimeError> {
        let mut staged = self.clone();
        let ZoneKeywordKind::Recover { cost, .. } = trigger.program.kind() else {
            return Err(RuntimeError::UnsupportedProgram);
        };
        if choice.pay {
            match cost {
                RecoverCost::Mana(cost) => {
                    staged.pay_mana(trigger.controller, cost, 0, 0, &choice.mana_payment)?;
                }
                RecoverCost::HalfLifeRoundedUp => {
                    if choice.mana_payment.total() != 0 {
                        return Err(RuntimeError::IllegalManaPayment);
                    }
                    let player = staged
                        .players
                        .get_mut(&trigger.controller)
                        .ok_or(RuntimeError::InvalidSource)?;
                    let payment = (player.life.max(0) + 1) / 2;
                    player.life -= payment;
                }
            }
            if staged
                .current_object(trigger.source)
                .is_some_and(|source| source.zone == Zone::Graveyard)
            {
                staged.move_object(trigger.source, Zone::Hand, None)?;
            }
        } else {
            if choice.mana_payment.total() != 0 {
                return Err(RuntimeError::IllegalManaPayment);
            }
            if staged
                .current_object(trigger.source)
                .is_some_and(|source| source.zone == Zone::Graveyard)
            {
                staged.move_object(trigger.source, Zone::Exile, None)?;
            }
        }
        staged.run_state_based_actions()?;
        *self = staged;
        Ok(())
    }

    pub fn resolve_draw(
        &mut self,
        player: PlayerId,
        dredge: Option<DredgeChoice>,
    ) -> Result<(), RuntimeError> {
        let mut staged = self.clone();
        staged.resolve_draw_inner(player, dredge)?;
        staged.run_state_based_actions()?;
        *self = staged;
        Ok(())
    }

    fn resolve_draw_inner(
        &mut self,
        player: PlayerId,
        dredge: Option<DredgeChoice>,
    ) -> Result<(), RuntimeError> {
        if let Some(choice) = dredge {
            let ZoneKeywordKind::Dredge { amount } = choice.program.kind() else {
                return Err(RuntimeError::UnsupportedProgram);
            };
            let source = self
                .current_object(choice.source)
                .ok_or(RuntimeError::ReplacementUnavailable)?;
            self.validate_program_source(&choice.program, source)
                .map_err(|_| RuntimeError::ReplacementUnavailable)?;
            if source.owner != player
                || self
                    .players
                    .get(&player)
                    .ok_or(RuntimeError::InvalidSource)?
                    .library
                    .len()
                    < *amount as usize
            {
                return Err(RuntimeError::ReplacementUnavailable);
            }
            for _ in 0..*amount {
                let card = self.pop_library_top(player)?;
                let new = self.move_object(card, Zone::Graveyard, None)?;
                self.events.push(GameEvent::Milled { player, card: new });
            }
            self.move_object(choice.source, Zone::Hand, None)?;
            return Ok(());
        }
        if self
            .players
            .get(&player)
            .ok_or(RuntimeError::InvalidSource)?
            .library
            .is_empty()
        {
            self.players
                .get_mut(&player)
                .ok_or(RuntimeError::InvalidSource)?
                .failed_draw_from_empty_library = true;
            return Ok(());
        }
        let card = self.pop_library_top(player)?;
        let drawn = self.move_object(card, Zone::Hand, None)?;
        self.events.push(GameEvent::Drew {
            player,
            card: drawn,
        });
        Ok(())
    }

    pub fn begin_end_step(&mut self) -> Result<(), RuntimeError> {
        self.step_sequence = self.step_sequence.saturating_add(1);
        self.phase = Phase::End;
        let pending = std::mem::take(&mut self.delayed_returns);
        for delayed in pending {
            if delayed.created_step_sequence < self.step_sequence
                && self
                    .current_object(delayed.object)
                    .is_some_and(|object| object.zone == Zone::Exile)
            {
                self.move_object(
                    delayed.object,
                    Zone::Battlefield,
                    Some(delayed.controller_on_return),
                )?;
            } else if delayed.created_step_sequence >= self.step_sequence {
                self.delayed_returns.push(delayed);
            }
        }
        self.run_state_based_actions()
    }

    pub fn cleanup_turn(&mut self) {
        for object in self.objects.values_mut() {
            if object.zone == Zone::Battlefield {
                object.power_modifier = 0;
                object.toughness_modifier = 0;
                object.temporary_keywords.clear();
                object.temporary_colors = None;
                object.damage_marked = 0;
                object.cannot_block_this_turn = false;
                object.cannot_be_blocked_this_turn = false;
                object.must_be_blocked_this_turn = false;
            }
        }
        self.damage_life_windows
            .retain(|window| window.turn > self.turn);
    }

    pub fn begin_untap_step(&mut self, player: PlayerId) {
        for object in self
            .objects
            .values_mut()
            .filter(|object| object.zone == Zone::Battlefield && object.controller == player)
        {
            if object.skip_next_untap {
                object.skip_next_untap = false;
            } else {
                object.tapped = false;
            }
        }
    }

    pub fn record_creature_damage_for_forecast(&mut self, creature: ObjectRef, amount: u32) {
        let beneficiaries = self
            .damage_life_windows
            .iter()
            .filter(|window| window.turn == self.turn && window.creature == creature)
            .map(|window| window.beneficiary)
            .collect::<Vec<_>>();
        for beneficiary in beneficiaries {
            if let Some(player) = self.players.get_mut(&beneficiary) {
                player.life = player.life.saturating_add(amount as i32);
            }
        }
    }

    fn validate_activation_targets(
        &self,
        kind: &ZoneKeywordKind,
        actor: PlayerId,
        x_value: u32,
        targets: &[Target],
    ) -> Result<(), RuntimeError> {
        let valid_count = match kind {
            ZoneKeywordKind::Embalm { .. }
            | ZoneKeywordKind::Eternalize { .. }
            | ZoneKeywordKind::Transmute { .. }
            | ZoneKeywordKind::Dredge { .. }
            | ZoneKeywordKind::Recover { .. } => targets.is_empty(),
            ZoneKeywordKind::Scavenge { .. }
            | ZoneKeywordKind::Reinforce { .. }
            | ZoneKeywordKind::Bloodrush { .. } => targets.len() == 1,
            ZoneKeywordKind::Forecast { effect, .. } => match effect {
                ForecastEffect::EachPlayerDrawsOne
                | ForecastEffect::CreateWhiteBlueBird
                | ForecastEffect::DrawOne => targets.is_empty(),
                _ => targets.len() == 1,
            },
            ZoneKeywordKind::Channel { effect, .. } => match effect {
                ChannelEffect::CreateGreenHumanMonk
                | ChannelEffect::DrawOne
                | ChannelEffect::GainLife(_)
                | ChannelEffect::SearchBasicPlainsToHandThenGainLife(_)
                | ChannelEffect::SearchBasicLandToBattlefieldTapped
                | ChannelEffect::CreateRedDragon
                | ChannelEffect::CreateTwoPilotTokens
                | ChannelEffect::MillThenReturnCreatureOrPlaneswalker
                | ChannelEffect::CreateTwoHastySpirits
                | ChannelEffect::PutCounterOnEachControlledCreatureThenDraw
                | ChannelEffect::DamageEachCreature(_, _) => targets.is_empty(),
                ChannelEffect::PutCountersOnUpToTwoControlledCreatures
                | ChannelEffect::TapAndFreezeUpToTwoOpposingCreatures => targets.len() <= 2,
                ChannelEffect::GrantFlyingToXTargets
                | ChannelEffect::ReturnXNonlegendaryCardsFromGraveyard => {
                    targets.len() == x_value as usize
                }
                _ => targets.len() == 1,
            },
        };
        if !valid_count {
            return Err(RuntimeError::InvalidTargetCount);
        }
        if targets.iter().copied().collect::<BTreeSet<_>>().len() != targets.len() {
            return Err(RuntimeError::InvalidTarget);
        }
        if targets
            .iter()
            .copied()
            .any(|target| !self.target_is_legal(kind, actor, x_value, target))
        {
            return Err(RuntimeError::InvalidTarget);
        }
        Ok(())
    }

    fn target_is_legal(
        &self,
        kind: &ZoneKeywordKind,
        actor: PlayerId,
        _x_value: u32,
        target: Target,
    ) -> bool {
        let battlefield_creature = |target: Target| {
            self.target_object(target).is_some_and(|object| {
                object.zone == Zone::Battlefield
                    && object.card_types.contains(&CardType::Creature)
                    && object.targetable
            })
        };
        match kind {
            ZoneKeywordKind::Scavenge { .. } | ZoneKeywordKind::Reinforce { .. } => {
                battlefield_creature(target)
            }
            ZoneKeywordKind::Bloodrush { .. } => self.target_object(target).is_some_and(|object| {
                object.zone == Zone::Battlefield
                    && object.card_types.contains(&CardType::Creature)
                    && object.attacking
                    && object.targetable
            }),
            ZoneKeywordKind::Forecast { effect, .. } => match effect {
                ForecastEffect::SetTargetCreatureColorsUntilEndOfTurn
                | ForecastEffect::GainLifeWhenTargetCreatureDealsDamageThisTurn
                | ForecastEffect::GrantTargetCreatureKeywordUntilEndOfTurn(_)
                | ForecastEffect::PumpTargetCreature(_) => battlefield_creature(target),
                ForecastEffect::TargetSmallCreatureCantBeBlockedThisTurn => {
                    self.target_object(target).is_some_and(|object| {
                        object.zone == Zone::Battlefield
                            && object.card_types.contains(&CardType::Creature)
                            && object.power().is_some_and(|power| power <= 2)
                            && object.targetable
                    })
                }
                ForecastEffect::TapTargetCreature { must_be_untapped } => {
                    self.target_object(target).is_some_and(|object| {
                        object.zone == Zone::Battlefield
                            && object.card_types.contains(&CardType::Creature)
                            && (!must_be_untapped || !object.tapped)
                            && object.targetable
                    })
                }
                ForecastEffect::ReturnSmallCreatureCardFromGraveyard => {
                    self.target_object(target).is_some_and(|object| {
                        object.zone == Zone::Graveyard
                            && object.owner == actor
                            && object.card_types.contains(&CardType::Creature)
                            && object.mana_value <= 1
                            && object.targetable
                    })
                }
                ForecastEffect::EachPlayerDrawsOne
                | ForecastEffect::CreateWhiteBlueBird
                | ForecastEffect::DrawOne => false,
            },
            ZoneKeywordKind::Channel { effect, .. } => {
                self.channel_target_is_legal(effect, actor, target)
            }
            ZoneKeywordKind::Embalm { .. }
            | ZoneKeywordKind::Eternalize { .. }
            | ZoneKeywordKind::Transmute { .. }
            | ZoneKeywordKind::Recover { .. }
            | ZoneKeywordKind::Dredge { .. } => false,
        }
    }

    fn channel_target_is_legal(
        &self,
        effect: &ChannelEffect,
        actor: PlayerId,
        target: Target,
    ) -> bool {
        match effect {
            ChannelEffect::TargetCreatureCantBlockThisTurn
            | ChannelEffect::PumpTargetCreature(_)
            | ChannelEffect::GrantTargetCreatureKeywordUntilEndOfTurn(_)
            | ChannelEffect::ForceAllAbleCreaturesToBlockTarget
            | ChannelEffect::GrantFlyingToXTargets => {
                self.target_object(target).is_some_and(|object| {
                    object.zone == Zone::Battlefield
                        && object.card_types.contains(&CardType::Creature)
                        && object.targetable
                })
            }
            ChannelEffect::DealDamage {
                target: DamageTarget::Any,
                ..
            } => match target {
                Target::Player(player) => {
                    self.players.get(&player).is_some_and(|state| state.in_game)
                }
                Target::Object(_) => self.target_object(target).is_some_and(|object| {
                    object.zone == Zone::Battlefield
                        && object.targetable
                        && (object.card_types.contains(&CardType::Creature)
                            || object.card_types.contains(&CardType::Planeswalker)
                            || object.card_types.contains(&CardType::Battle))
                }),
                Target::Stack(_) => false,
            },
            ChannelEffect::DealDamage {
                target: DamageTarget::Creature,
                ..
            } => self.target_object(target).is_some_and(|object| {
                object.zone == Zone::Battlefield
                    && object.card_types.contains(&CardType::Creature)
                    && object.targetable
            }),
            ChannelEffect::DealDamage {
                target: DamageTarget::AttackingOrBlockingCreature,
                ..
            } => self.target_object(target).is_some_and(|object| {
                object.zone == Zone::Battlefield
                    && object.card_types.contains(&CardType::Creature)
                    && (object.attacking || object.blocking)
                    && object.targetable
            }),
            ChannelEffect::ReturnTargetToHand(filter)
            | ChannelEffect::BlinkTargetUntilNextEndStep(filter)
            | ChannelEffect::DestroyTarget(filter) => {
                self.target_object(target).is_some_and(|object| {
                    object.zone == Zone::Battlefield
                        && object.targetable
                        && object_matches_filter(object, *filter)
                })
            }
            ChannelEffect::PutCountersOnUpToTwoControlledCreatures => {
                self.target_object(target).is_some_and(|object| {
                    object.zone == Zone::Battlefield
                        && object.controller == actor
                        && object.card_types.contains(&CardType::Creature)
                        && object.targetable
                })
            }
            ChannelEffect::ReturnTargetCardFromGraveyardToHand(filter) => {
                self.target_object(target).is_some_and(|object| {
                    object.zone == Zone::Graveyard
                        && object.owner == actor
                        && object.targetable
                        && object_matches_filter(object, *filter)
                })
            }
            ChannelEffect::ReturnXNonlegendaryCardsFromGraveyard => {
                self.target_object(target).is_some_and(|object| {
                    object.zone == Zone::Graveyard
                        && object.owner == actor
                        && object.targetable
                        && object_matches_filter(object, TargetFilter::NonlegendaryCard)
                })
            }
            ChannelEffect::CounterUnlessControllerPays { target: kind, .. } => {
                let Target::Stack(id) = target else {
                    return false;
                };
                self.stack_items.get(&id).is_some_and(|item| {
                    !item.countered
                        && item.targetable
                        && match kind {
                            CounterTarget::Spell => item.kind == StackItemKind::Spell,
                            CounterTarget::SpellOrAbility => true,
                        }
                })
            }
            ChannelEffect::TapAndFreezeUpToTwoOpposingCreatures => {
                self.target_object(target).is_some_and(|object| {
                    object.zone == Zone::Battlefield
                        && object.controller != actor
                        && object.card_types.contains(&CardType::Creature)
                        && object.targetable
                })
            }
            ChannelEffect::PutTargetOnLibraryTopOrBottom => {
                self.target_object(target).is_some_and(|object| {
                    object.zone == Zone::Battlefield
                        && !object.card_types.contains(&CardType::Land)
                        && object.targetable
                })
            }
            ChannelEffect::TargetPlayerDiscards(_) => {
                let Target::Player(player) = target else {
                    return false;
                };
                self.players.get(&player).is_some_and(|state| state.in_game)
            }
            ChannelEffect::AnimateTargetLandWithCounters => {
                self.target_object(target).is_some_and(|object| {
                    object.zone == Zone::Battlefield
                        && object.controller == actor
                        && object.card_types.contains(&CardType::Land)
                        && object.targetable
                })
            }
            ChannelEffect::CreateGreenHumanMonk
            | ChannelEffect::DrawOne
            | ChannelEffect::GainLife(_)
            | ChannelEffect::SearchBasicPlainsToHandThenGainLife(_)
            | ChannelEffect::SearchBasicLandToBattlefieldTapped
            | ChannelEffect::CreateRedDragon
            | ChannelEffect::CreateTwoPilotTokens
            | ChannelEffect::MillThenReturnCreatureOrPlaneswalker
            | ChannelEffect::CreateTwoHastySpirits
            | ChannelEffect::PutCounterOnEachControlledCreatureThenDraw
            | ChannelEffect::DamageEachCreature(_, _) => false,
        }
    }

    fn target_object(&self, target: Target) -> Option<&GameObject> {
        let Target::Object(reference) = target else {
            return None;
        };
        self.current_object(reference)
    }

    fn resolve_forecast(
        &mut self,
        controller: PlayerId,
        effect: &ForecastEffect,
        targets: &[Target],
        choices: &mut ResolutionChoices,
    ) -> Result<(), RuntimeError> {
        match effect {
            ForecastEffect::SetTargetCreatureColorsUntilEndOfTurn => {
                if choices.chosen_colors.is_empty()
                    || choices.chosen_colors.contains(&ManaColor::Colorless)
                {
                    return Err(RuntimeError::MissingResolutionChoice);
                }
                if let Some(Target::Object(target)) = targets.first() {
                    self.objects
                        .get_mut(&target.id)
                        .ok_or(RuntimeError::InvalidTarget)?
                        .temporary_colors = Some(choices.chosen_colors.clone());
                }
            }
            ForecastEffect::TargetSmallCreatureCantBeBlockedThisTurn => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.objects
                        .get_mut(&target.id)
                        .ok_or(RuntimeError::InvalidTarget)?
                        .cannot_be_blocked_this_turn = true;
                }
            }
            ForecastEffect::GainLifeWhenTargetCreatureDealsDamageThisTurn => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.damage_life_windows.push(DamageLifeTriggerWindow {
                        creature: *target,
                        beneficiary: controller,
                        turn: self.turn,
                    });
                }
            }
            ForecastEffect::GrantTargetCreatureKeywordUntilEndOfTurn(keyword) => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.objects
                        .get_mut(&target.id)
                        .ok_or(RuntimeError::InvalidTarget)?
                        .temporary_keywords
                        .insert(*keyword);
                }
            }
            ForecastEffect::EachPlayerDrawsOne => {
                let order = self.turn_order_from_active();
                for player in order {
                    if self.players.get(&player).is_some_and(|state| state.in_game) {
                        self.resolve_draw_inner(player, choices.draw_replacements.remove(&player))?;
                    }
                }
            }
            ForecastEffect::TapTargetCreature { .. } => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.objects
                        .get_mut(&target.id)
                        .ok_or(RuntimeError::InvalidTarget)?
                        .tapped = true;
                }
            }
            ForecastEffect::CreateWhiteBlueBird => {
                self.create_simple_token(
                    controller,
                    BTreeSet::from([ManaColor::White, ManaColor::Blue]),
                    BTreeSet::from([CardType::Creature]),
                    BTreeSet::from(["Bird".to_owned()]),
                    1,
                    1,
                    BTreeSet::from([GrantedKeyword::Flying]),
                )?;
            }
            ForecastEffect::ReturnSmallCreatureCardFromGraveyard => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.move_object(*target, Zone::Battlefield, Some(controller))?;
                }
            }
            ForecastEffect::PumpTargetCreature(pump) => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.apply_pump(*target, controller, pump)?;
                }
            }
            ForecastEffect::DrawOne => {
                self.resolve_draw_inner(controller, choices.draw_replacements.remove(&controller))?;
            }
        }
        Ok(())
    }

    fn resolve_channel(
        &mut self,
        controller: PlayerId,
        effect: &ChannelEffect,
        x_value: u32,
        targets: &[Target],
        choices: &mut ResolutionChoices,
    ) -> Result<(), RuntimeError> {
        match effect {
            ChannelEffect::TargetCreatureCantBlockThisTurn => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.objects
                        .get_mut(&target.id)
                        .ok_or(RuntimeError::InvalidTarget)?
                        .cannot_block_this_turn = true;
                }
            }
            ChannelEffect::CreateGreenHumanMonk => {
                let token = self.create_simple_token(
                    controller,
                    BTreeSet::from([ManaColor::Green]),
                    BTreeSet::from([CardType::Creature]),
                    BTreeSet::from(["Human".to_owned(), "Monk".to_owned()]),
                    1,
                    1,
                    BTreeSet::new(),
                )?;
                self.objects
                    .get_mut(&token.id)
                    .ok_or(RuntimeError::InvalidSource)?
                    .taps_for = Some(ManaColor::Green);
            }
            ChannelEffect::PumpTargetCreature(pump) => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.apply_pump(*target, controller, pump)?;
                }
            }
            ChannelEffect::GrantTargetCreatureKeywordUntilEndOfTurn(keyword) => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.objects
                        .get_mut(&target.id)
                        .ok_or(RuntimeError::InvalidTarget)?
                        .temporary_keywords
                        .insert(*keyword);
                }
            }
            ChannelEffect::DrawOne => {
                self.resolve_draw_inner(controller, choices.draw_replacements.remove(&controller))?;
            }
            ChannelEffect::DealDamage { amount, .. } => {
                let amount = self.number_value(*amount, controller, x_value, None)?;
                if let Some(target) = targets.first() {
                    self.deal_damage(*target, amount)?;
                }
            }
            ChannelEffect::ReturnTargetToHand(_) => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.move_object(*target, Zone::Hand, None)?;
                }
            }
            ChannelEffect::BlinkTargetUntilNextEndStep(_) => {
                if let Some(Target::Object(target)) = targets.first() {
                    let owner = self
                        .current_object(*target)
                        .ok_or(RuntimeError::InvalidTarget)?
                        .owner;
                    let exiled = self.move_object(*target, Zone::Exile, None)?;
                    self.delayed_returns.push(DelayedReturn {
                        object: exiled,
                        controller_on_return: owner,
                        created_step_sequence: self.step_sequence,
                    });
                }
            }
            ChannelEffect::GainLife(amount) => {
                let player = self
                    .players
                    .get_mut(&controller)
                    .ok_or(RuntimeError::InvalidSource)?;
                player.life = player.life.saturating_add(*amount as i32);
            }
            ChannelEffect::SearchBasicPlainsToHandThenGainLife(amount) => {
                self.search_library(
                    controller,
                    choices.selected_library_card,
                    choices.library_order_after_shuffle.take(),
                    true,
                    |object| {
                        object.basic
                            && object.card_types.contains(&CardType::Land)
                            && object.basic_land_types.contains("Plains")
                    },
                )?;
                let player = self
                    .players
                    .get_mut(&controller)
                    .ok_or(RuntimeError::InvalidSource)?;
                player.life = player.life.saturating_add(*amount as i32);
            }
            ChannelEffect::PutCountersOnUpToTwoControlledCreatures => {
                for target in targets {
                    if let Target::Object(reference) = target {
                        self.objects
                            .get_mut(&reference.id)
                            .ok_or(RuntimeError::InvalidTarget)?
                            .plus_one_counters += 1;
                    }
                }
            }
            ChannelEffect::ReturnTargetCardFromGraveyardToHand(_) => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.move_object(*target, Zone::Hand, None)?;
                }
            }
            ChannelEffect::SearchBasicLandToBattlefieldTapped => {
                let selected = self.search_library_to_zone(
                    controller,
                    choices.selected_library_card,
                    choices.library_order_after_shuffle.take(),
                    false,
                    Zone::Battlefield,
                    |object| object.basic && object.card_types.contains(&CardType::Land),
                )?;
                if let Some(reference) = selected {
                    self.objects
                        .get_mut(&reference.id)
                        .ok_or(RuntimeError::InvalidHiddenZoneChoice)?
                        .tapped = true;
                }
            }
            ChannelEffect::ForceAllAbleCreaturesToBlockTarget => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.objects
                        .get_mut(&target.id)
                        .ok_or(RuntimeError::InvalidTarget)?
                        .must_be_blocked_this_turn = true;
                }
            }
            ChannelEffect::CreateRedDragon => {
                self.create_simple_token(
                    controller,
                    BTreeSet::from([ManaColor::Red]),
                    BTreeSet::from([CardType::Creature]),
                    BTreeSet::from(["Dragon".to_owned()]),
                    4,
                    4,
                    BTreeSet::from([GrantedKeyword::Flying]),
                )?;
            }
            ChannelEffect::CounterUnlessControllerPays { generic_mana, .. } => {
                let Some(Target::Stack(id)) = targets.first() else {
                    return Err(RuntimeError::InvalidTarget);
                };
                let target_controller = self
                    .stack_items
                    .get(id)
                    .ok_or(RuntimeError::InvalidTarget)?
                    .controller;
                if let Some(payment) = choices.counter_payment.take() {
                    let cost = ManaCost {
                        exact: format!("{{{generic_mana}}}"),
                        symbols: vec![ManaSymbol::Generic(*generic_mana)],
                    };
                    self.pay_mana(target_controller, &cost, 0, 0, &payment)?;
                } else {
                    self.stack_items
                        .get_mut(id)
                        .ok_or(RuntimeError::InvalidTarget)?
                        .countered = true;
                }
            }
            ChannelEffect::CreateTwoPilotTokens => {
                for _ in 0..2 {
                    let token = self.create_simple_token(
                        controller,
                        BTreeSet::new(),
                        BTreeSet::from([CardType::Creature]),
                        BTreeSet::from(["Pilot".to_owned()]),
                        1,
                        1,
                        BTreeSet::new(),
                    )?;
                    self.objects
                        .get_mut(&token.id)
                        .ok_or(RuntimeError::InvalidSource)?
                        .crew_power_bonus = 2;
                }
            }
            ChannelEffect::MillThenReturnCreatureOrPlaneswalker => {
                for _ in 0..3 {
                    if self
                        .players
                        .get(&controller)
                        .ok_or(RuntimeError::InvalidSource)?
                        .library
                        .is_empty()
                    {
                        break;
                    }
                    let card = self.pop_library_top(controller)?;
                    let milled = self.move_object(card, Zone::Graveyard, None)?;
                    self.events.push(GameEvent::Milled {
                        player: controller,
                        card: milled,
                    });
                }
                let available = self.objects.values().any(|object| {
                    object.owner == controller
                        && object.zone == Zone::Graveyard
                        && (object.card_types.contains(&CardType::Creature)
                            || object.card_types.contains(&CardType::Planeswalker))
                });
                match (available, choices.selected_graveyard_card) {
                    (false, None) => {}
                    (true, Some(id))
                        if self.objects.get(&id).is_some_and(|object| {
                            object.owner == controller
                                && object.zone == Zone::Graveyard
                                && (object.card_types.contains(&CardType::Creature)
                                    || object.card_types.contains(&CardType::Planeswalker))
                        }) =>
                    {
                        let reference = self
                            .objects
                            .get(&id)
                            .ok_or(RuntimeError::InvalidHiddenZoneChoice)?
                            .object_ref();
                        self.move_object(reference, Zone::Hand, None)?;
                    }
                    _ => return Err(RuntimeError::MissingResolutionChoice),
                }
            }
            ChannelEffect::CreateTwoHastySpirits => {
                for _ in 0..2 {
                    let token = self.create_simple_token(
                        controller,
                        BTreeSet::new(),
                        BTreeSet::from([CardType::Creature]),
                        BTreeSet::from(["Spirit".to_owned()]),
                        1,
                        1,
                        BTreeSet::new(),
                    )?;
                    self.objects
                        .get_mut(&token.id)
                        .ok_or(RuntimeError::InvalidSource)?
                        .temporary_keywords
                        .insert(GrantedKeyword::Haste);
                }
            }
            ChannelEffect::TapAndFreezeUpToTwoOpposingCreatures => {
                for target in targets {
                    if let Target::Object(reference) = target {
                        let object = self
                            .objects
                            .get_mut(&reference.id)
                            .ok_or(RuntimeError::InvalidTarget)?;
                        object.tapped = true;
                        object.skip_next_untap = true;
                    }
                }
            }
            ChannelEffect::DestroyTarget(_) => {
                if let Some(Target::Object(target)) = targets.first() {
                    self.destroy_object(*target)?;
                }
            }
            ChannelEffect::PutTargetOnLibraryTopOrBottom => {
                if let Some(Target::Object(target)) = targets.first() {
                    let placement = choices
                        .library_placement
                        .ok_or(RuntimeError::MissingResolutionChoice)?;
                    self.move_object_to_library(*target, placement)?;
                }
            }
            ChannelEffect::TargetPlayerDiscards(amount) => {
                let Some(Target::Player(player)) = targets.first() else {
                    return Err(RuntimeError::InvalidTarget);
                };
                self.discard_chosen_cards(*player, *amount as usize, &choices.discard_cards)?;
            }
            ChannelEffect::PutCounterOnEachControlledCreatureThenDraw => {
                for object in self.objects.values_mut().filter(|object| {
                    object.zone == Zone::Battlefield
                        && object.controller == controller
                        && object.card_types.contains(&CardType::Creature)
                }) {
                    object.plus_one_counters += 1;
                }
                self.resolve_draw_inner(controller, choices.draw_replacements.remove(&controller))?;
            }
            ChannelEffect::DamageEachCreature(filter, amount) => {
                let amount = self.number_value(*amount, controller, x_value, None)?;
                let targets = self
                    .objects
                    .values()
                    .filter(|object| {
                        object.zone == Zone::Battlefield && object_matches_filter(object, *filter)
                    })
                    .map(|object| object.object_ref())
                    .collect::<Vec<_>>();
                for target in targets {
                    self.deal_damage(Target::Object(target), amount)?;
                }
            }
            ChannelEffect::AnimateTargetLandWithCounters => {
                if let Some(Target::Object(target)) = targets.first() {
                    let object = self
                        .objects
                        .get_mut(&target.id)
                        .ok_or(RuntimeError::InvalidTarget)?;
                    object.plus_one_counters += x_value;
                    object.card_types.insert(CardType::Creature);
                    object.subtypes.insert("Spirit".to_owned());
                    object.colors = BTreeSet::from([ManaColor::Green]);
                    object.base_power = Some(0);
                    object.base_toughness = Some(0);
                    object.granted_keywords.insert(GrantedKeyword::Haste);
                }
            }
            ChannelEffect::GrantFlyingToXTargets => {
                for target in targets {
                    if let Target::Object(reference) = target {
                        self.objects
                            .get_mut(&reference.id)
                            .ok_or(RuntimeError::InvalidTarget)?
                            .temporary_keywords
                            .insert(GrantedKeyword::Flying);
                    }
                }
            }
            ChannelEffect::ReturnXNonlegendaryCardsFromGraveyard => {
                for target in targets {
                    if let Target::Object(reference) = target {
                        self.move_object(*reference, Zone::Hand, None)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn apply_pump(
        &mut self,
        target: ObjectRef,
        controller: PlayerId,
        pump: &PumpEffect,
    ) -> Result<(), RuntimeError> {
        let power = self.number_value(pump.power, controller, 0, None)? as i32;
        let toughness = self.number_value(pump.toughness, controller, 0, None)? as i32;
        let object = self
            .objects
            .get_mut(&target.id)
            .ok_or(RuntimeError::InvalidTarget)?;
        object.power_modifier += power;
        object.toughness_modifier += toughness;
        object
            .temporary_keywords
            .extend(pump.granted_keywords.iter().copied());
        Ok(())
    }

    fn number_value(
        &self,
        value: NumberValue,
        controller: PlayerId,
        x_value: u32,
        source_power: Option<i32>,
    ) -> Result<u32, RuntimeError> {
        match value {
            NumberValue::Fixed(value) => Ok(value),
            NumberValue::ChosenX => Ok(x_value),
            NumberValue::SourcePower => source_power
                .map(|power| power.max(0) as u32)
                .ok_or(RuntimeError::InvalidSource),
            NumberValue::LandsControlled => Ok(self
                .objects
                .values()
                .filter(|object| {
                    object.zone == Zone::Battlefield
                        && object.controller == controller
                        && object.card_types.contains(&CardType::Land)
                })
                .count() as u32),
        }
    }

    fn create_copy_token(
        &mut self,
        controller: PlayerId,
        source: &GameObject,
        exception: TokenCopyException,
    ) -> Result<ObjectRef, RuntimeError> {
        let mut token = source.clone();
        token.restore_intrinsic_values();
        token.id = ObjectId(self.next_object_id);
        self.next_object_id = self.next_object_id.saturating_add(1);
        token.incarnation = IncarnationId(0);
        token.owner = controller;
        token.controller = controller;
        token.zone = Zone::Battlefield;
        token.token = true;
        token.mana_cost = None;
        token.mana_value = 0;
        token.plus_one_counters = 0;
        token.power_modifier = 0;
        token.toughness_modifier = 0;
        token.damage_marked = 0;
        token.temporary_keywords.clear();
        token.temporary_colors = None;
        token.attacking = false;
        token.blocking = false;
        token.skip_next_untap = false;
        token.subtypes.insert("Zombie".to_owned());
        match exception {
            TokenCopyException::Embalm => {
                token.colors = BTreeSet::from([ManaColor::White]);
            }
            TokenCopyException::Eternalize => {
                token.colors = BTreeSet::from([ManaColor::Black]);
                token.base_power = Some(4);
                token.base_toughness = Some(4);
            }
            TokenCopyException::EternalizeLandCreature => {
                token.colors = BTreeSet::from([ManaColor::Black]);
                token.card_types = BTreeSet::from([CardType::Creature]);
                token.base_power = Some(4);
                token.base_toughness = Some(4);
            }
        }
        token.tapped = token.enters_tapped;
        token.seal_intrinsic_values();
        let reference = token.object_ref();
        self.objects.insert(token.id, token);
        Ok(reference)
    }

    fn create_simple_token(
        &mut self,
        controller: PlayerId,
        colors: BTreeSet<ManaColor>,
        card_types: BTreeSet<CardType>,
        subtypes: BTreeSet<String>,
        power: i32,
        toughness: i32,
        keywords: BTreeSet<GrantedKeyword>,
    ) -> Result<ObjectRef, RuntimeError> {
        if !self.players.contains_key(&controller) {
            return Err(RuntimeError::InvalidSource);
        }
        let id = ObjectId(self.next_object_id);
        self.next_object_id = self.next_object_id.saturating_add(1);
        let mut token = GameObject::new_card(id, controller, Zone::Battlefield, "Token", 0);
        token.token = true;
        token.colors = colors;
        token.card_types = card_types;
        token.subtypes = subtypes;
        token.base_power = Some(power);
        token.base_toughness = Some(toughness);
        token.printed_keywords = keywords;
        token.seal_intrinsic_values();
        let reference = token.object_ref();
        self.objects.insert(id, token);
        Ok(reference)
    }

    fn search_library<F>(
        &mut self,
        player: PlayerId,
        selected: Option<ObjectId>,
        order_after_shuffle: Option<Vec<ObjectId>>,
        reveal: bool,
        predicate: F,
    ) -> Result<Option<ObjectRef>, RuntimeError>
    where
        F: Fn(&GameObject) -> bool,
    {
        self.search_library_to_zone(
            player,
            selected,
            order_after_shuffle,
            reveal,
            Zone::Hand,
            predicate,
        )
    }

    fn search_library_to_zone<F>(
        &mut self,
        player: PlayerId,
        selected: Option<ObjectId>,
        order_after_shuffle: Option<Vec<ObjectId>>,
        reveal: bool,
        destination: Zone,
        predicate: F,
    ) -> Result<Option<ObjectRef>, RuntimeError>
    where
        F: Fn(&GameObject) -> bool,
    {
        let selected_ref = if let Some(id) = selected {
            let object = self
                .objects
                .get(&id)
                .ok_or(RuntimeError::InvalidHiddenZoneChoice)?;
            if object.owner != player || object.zone != Zone::Library || !predicate(object) {
                return Err(RuntimeError::InvalidHiddenZoneChoice);
            }
            let old = object.object_ref();
            let moved = self.move_object(old, destination, None)?;
            if reveal {
                self.events.push(GameEvent::Revealed(moved));
            }
            Some(moved)
        } else {
            None
        };
        self.shuffle_library(player, order_after_shuffle)?;
        Ok(selected_ref)
    }

    fn shuffle_library(
        &mut self,
        player: PlayerId,
        top_first: Option<Vec<ObjectId>>,
    ) -> Result<(), RuntimeError> {
        let current = self
            .players
            .get(&player)
            .ok_or(RuntimeError::InvalidSource)?
            .library
            .clone();
        if current.len() > 1 {
            let top_first = top_first.ok_or(RuntimeError::InvalidShuffle)?;
            if top_first.len() != current.len()
                || top_first.iter().copied().collect::<BTreeSet<_>>()
                    != current.iter().copied().collect()
            {
                return Err(RuntimeError::InvalidShuffle);
            }
            self.players
                .get_mut(&player)
                .ok_or(RuntimeError::InvalidSource)?
                .library = top_first.into_iter().rev().collect();
        } else if let Some(order) = top_first
            && (order.len() != current.len()
                || order.iter().copied().collect::<BTreeSet<_>>()
                    != current.iter().copied().collect())
        {
            return Err(RuntimeError::InvalidShuffle);
        }
        self.events.push(GameEvent::Shuffled(player));
        Ok(())
    }

    fn discard_chosen_cards(
        &mut self,
        player: PlayerId,
        amount: usize,
        choices: &[ObjectRef],
    ) -> Result<(), RuntimeError> {
        let hand = self
            .objects
            .values()
            .filter(|object| object.owner == player && object.zone == Zone::Hand)
            .map(|object| object.object_ref())
            .collect::<BTreeSet<_>>();
        let required = amount.min(hand.len());
        if choices.len() != required
            || choices.iter().copied().collect::<BTreeSet<_>>().len() != choices.len()
            || choices.iter().any(|choice| !hand.contains(choice))
        {
            return Err(RuntimeError::InvalidHiddenZoneChoice);
        }
        for choice in choices {
            self.move_object(*choice, Zone::Graveyard, None)?;
        }
        Ok(())
    }

    fn move_object_to_library(
        &mut self,
        reference: ObjectRef,
        placement: LibraryPlacement,
    ) -> Result<ObjectRef, RuntimeError> {
        let owner = self
            .current_object(reference)
            .ok_or(RuntimeError::InvalidTarget)?
            .owner;
        let moved = self.move_object(reference, Zone::Library, None)?;
        let library = &mut self
            .players
            .get_mut(&owner)
            .ok_or(RuntimeError::InvalidSource)?
            .library;
        library.retain(|id| *id != moved.id);
        match placement {
            LibraryPlacement::Top => library.push(moved.id),
            LibraryPlacement::Bottom => library.insert(0, moved.id),
        }
        Ok(moved)
    }

    fn pop_library_top(&mut self, player: PlayerId) -> Result<ObjectRef, RuntimeError> {
        let id = self
            .players
            .get_mut(&player)
            .ok_or(RuntimeError::InvalidSource)?
            .library
            .pop()
            .ok_or(RuntimeError::InvalidHiddenZoneChoice)?;
        self.objects
            .get(&id)
            .map(GameObject::object_ref)
            .ok_or(RuntimeError::InvalidHiddenZoneChoice)
    }

    fn move_object(
        &mut self,
        reference: ObjectRef,
        destination: Zone,
        controller: Option<PlayerId>,
    ) -> Result<ObjectRef, RuntimeError> {
        let (owner, old_zone, was_creature) = {
            let object = self
                .current_object(reference)
                .ok_or(RuntimeError::InvalidSource)?;
            (
                object.owner,
                object.zone,
                object.card_types.contains(&CardType::Creature),
            )
        };
        if old_zone == Zone::Library {
            self.players
                .get_mut(&owner)
                .ok_or(RuntimeError::InvalidSource)?
                .library
                .retain(|id| *id != reference.id);
        }
        let object = self
            .objects
            .get_mut(&reference.id)
            .ok_or(RuntimeError::InvalidSource)?;
        object.restore_intrinsic_values();
        object.incarnation = IncarnationId(object.incarnation.0.saturating_add(1));
        object.zone = destination;
        object.controller = if destination == Zone::Battlefield {
            controller.unwrap_or(owner)
        } else {
            owner
        };
        object.plus_one_counters = 0;
        object.power_modifier = 0;
        object.toughness_modifier = 0;
        object.damage_marked = 0;
        object.prevent_next_damage = 0;
        object.temporary_keywords.clear();
        object.temporary_colors = None;
        object.tapped = destination == Zone::Battlefield && object.enters_tapped;
        object.attacking = false;
        object.blocking = false;
        object.cannot_block_this_turn = false;
        object.cannot_be_blocked_this_turn = false;
        object.must_be_blocked_this_turn = false;
        object.skip_next_untap = false;
        let new = object.object_ref();
        if destination == Zone::Library {
            self.players
                .get_mut(&owner)
                .ok_or(RuntimeError::InvalidSource)?
                .library
                .insert(0, reference.id);
        }
        self.events.push(GameEvent::ZoneChanged {
            old: reference,
            new,
            owner,
            was_creature,
            from: old_zone,
            to: destination,
        });
        Ok(new)
    }

    fn destroy_object(&mut self, reference: ObjectRef) -> Result<(), RuntimeError> {
        let object = self
            .objects
            .get_mut(&reference.id)
            .ok_or(RuntimeError::InvalidTarget)?;
        if object.incarnation != reference.incarnation || object.zone != Zone::Battlefield {
            return Ok(());
        }
        if object.indestructible {
            return Ok(());
        }
        if object.regeneration_shields > 0 {
            object.regeneration_shields -= 1;
            object.tapped = true;
            object.attacking = false;
            object.blocking = false;
            object.damage_marked = 0;
            return Ok(());
        }
        self.move_object(reference, Zone::Graveyard, None)?;
        Ok(())
    }

    fn deal_damage(&mut self, target: Target, amount: u32) -> Result<(), RuntimeError> {
        match target {
            Target::Player(player) => {
                let state = self
                    .players
                    .get_mut(&player)
                    .ok_or(RuntimeError::InvalidTarget)?;
                state.life = state.life.saturating_sub(amount as i32);
                self.events
                    .push(GameEvent::DamageToPlayer { player, amount });
            }
            Target::Object(reference) => {
                let object = self
                    .objects
                    .get_mut(&reference.id)
                    .ok_or(RuntimeError::InvalidTarget)?;
                let prevented = object.prevent_next_damage.min(amount);
                object.prevent_next_damage -= prevented;
                let dealt = amount - prevented;
                if object.card_types.contains(&CardType::Planeswalker) {
                    if let Some(loyalty) = object.loyalty.as_mut() {
                        *loyalty = loyalty.saturating_sub(dealt as i32);
                    }
                } else if object.card_types.contains(&CardType::Battle) {
                    if let Some(defense) = object.defense.as_mut() {
                        *defense = defense.saturating_sub(dealt as i32);
                    }
                } else {
                    object.damage_marked = object.damage_marked.saturating_add(dealt);
                }
                self.events.push(GameEvent::DamageToObject {
                    object: reference,
                    amount: dealt,
                });
            }
            Target::Stack(_) => return Err(RuntimeError::InvalidTarget),
        }
        Ok(())
    }

    fn run_state_based_actions(&mut self) -> Result<(), RuntimeError> {
        for player in self.players.values_mut() {
            if player.life <= 0 || player.failed_draw_from_empty_library {
                player.in_game = false;
            }
        }
        for _ in 0..16 {
            let mut move_to_graveyard = Vec::new();
            let mut destroy = Vec::new();
            let mut tokens_to_void = Vec::new();
            for object in self.objects.values() {
                if object.token && !matches!(object.zone, Zone::Battlefield | Zone::Void) {
                    tokens_to_void.push(object.object_ref());
                }
                if object.zone != Zone::Battlefield {
                    continue;
                }
                if object.card_types.contains(&CardType::Creature) {
                    if object.toughness().is_some_and(|toughness| toughness <= 0) {
                        move_to_graveyard.push(object.object_ref());
                    } else if !object.indestructible
                        && object.toughness().is_some_and(|toughness| {
                            toughness > 0 && object.damage_marked >= toughness as u32
                        })
                    {
                        destroy.push(object.object_ref());
                    }
                }
                if object.loyalty.is_some_and(|loyalty| loyalty <= 0)
                    || object.defense.is_some_and(|defense| defense <= 0)
                {
                    move_to_graveyard.push(object.object_ref());
                }
            }
            if move_to_graveyard.is_empty() && destroy.is_empty() && tokens_to_void.is_empty() {
                return Ok(());
            }
            for reference in move_to_graveyard {
                if self
                    .current_object(reference)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
                {
                    self.move_object(reference, Zone::Graveyard, None)?;
                }
            }
            for reference in destroy {
                self.destroy_object(reference)?;
            }
            for reference in tokens_to_void {
                if self.current_object(reference).is_some() {
                    self.move_object(reference, Zone::Void, None)?;
                }
            }
        }
        Ok(())
    }

    fn turn_order_from_active(&self) -> Vec<PlayerId> {
        let Some(active_index) = self
            .player_order
            .iter()
            .position(|player| *player == self.active_player)
        else {
            return self.player_order.clone();
        };
        self.player_order[active_index..]
            .iter()
            .chain(self.player_order[..active_index].iter())
            .copied()
            .collect()
    }
}

fn activation_mana_cost(kind: &ZoneKeywordKind) -> Option<&ManaCost> {
    match kind {
        ZoneKeywordKind::Embalm { cost, .. }
        | ZoneKeywordKind::Scavenge { cost }
        | ZoneKeywordKind::Transmute { cost, .. }
        | ZoneKeywordKind::Reinforce { cost, .. }
        | ZoneKeywordKind::Bloodrush { cost, .. }
        | ZoneKeywordKind::Channel { cost, .. } => Some(cost),
        ZoneKeywordKind::Eternalize { mana_cost, .. } => Some(mana_cost),
        ZoneKeywordKind::Forecast {
            cost: ForecastCost::ManaAndReveal(cost),
            ..
        } => Some(cost),
        ZoneKeywordKind::Forecast {
            cost: ForecastCost::TapTwoWhiteOrBlueCreaturesAndReveal,
            ..
        }
        | ZoneKeywordKind::Recover { .. }
        | ZoneKeywordKind::Dredge { .. } => None,
    }
}

fn object_matches_filter(object: &GameObject, filter: TargetFilter) -> bool {
    match filter {
        TargetFilter::AnyCard => true,
        TargetFilter::ArtifactCreatureEnchantmentOrPlaneswalker => {
            object.card_types.contains(&CardType::Artifact)
                || object.card_types.contains(&CardType::Creature)
                || object.card_types.contains(&CardType::Enchantment)
                || object.card_types.contains(&CardType::Planeswalker)
        }
        TargetFilter::ArtifactOrCreature => {
            object.card_types.contains(&CardType::Artifact)
                || object.card_types.contains(&CardType::Creature)
        }
        TargetFilter::Creature => object.card_types.contains(&CardType::Creature),
        TargetFilter::CreatureWithFlying => {
            object.card_types.contains(&CardType::Creature)
                && object.has_keyword(GrantedKeyword::Flying)
        }
        TargetFilter::CreatureWithoutFlying => {
            object.card_types.contains(&CardType::Creature)
                && !object.has_keyword(GrantedKeyword::Flying)
        }
        TargetFilter::NonlandPermanent => {
            object.zone == Zone::Battlefield && !object.card_types.contains(&CardType::Land)
        }
        TargetFilter::NonlegendaryCard => !object.legendary,
        TargetFilter::CreatureOrPlaneswalkerCard => {
            object.card_types.contains(&CardType::Creature)
                || object.card_types.contains(&CardType::Planeswalker)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileDiagnostic {
    NotCandidate,
    EarlierOwner,
    Unsupported,
}

impl fmt::Display for CompileDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCandidate => write!(formatter, "not an assigned keyword candidate"),
            Self::EarlierOwner => write!(formatter, "an earlier exact owner retains this clause"),
            Self::Unsupported => write!(formatter, "compound or context dependent source"),
        }
    }
}
