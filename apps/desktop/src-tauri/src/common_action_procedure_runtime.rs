//! Exact, content keyed Oracle programs for common game procedures.
//!
//! This module owns only complete standalone instructions and a small reviewed
//! set of complete trigger envelopes. It does not promote a clause merely
//! because the clause contains a familiar action word. Compounds, replacement
//! effects, grants, event references, named-source wrappers, variable values
//! without complete resolution evidence, and unreviewed trigger envelopes fail
//! closed.
//!
//! Program identity contains exact Oracle content, the complete action and
//! timing contract, and versioned rules semantics. Card names, identifiers,
//! database rows, clause addresses, snapshot hashes, timestamps, and unrelated
//! metadata are deliberately absent.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const COMMON_ACTION_PROCEDURE_COMPILER_VERSION: &str = "common-action-procedure-compiler-0.1";
pub const COMMON_ACTION_PROCEDURE_RUNTIME_VERSION: &str = "common-action-procedure-runtime-0.1";
pub const COMMON_ACTION_PROCEDURE_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:101.3,101.4,109.5,111.10f,117.3b,121,122,\
     400.2-7,400.11,603,608.2,701.16,701.34,701.41,701.44,701.47-49,701.54,\
     725,726,800-810";

pub const fn common_action_production_adapter_connected() -> bool {
    false
}

pub type PlayerId = u16;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type EventId = u64;
pub type TriggerId = u64;
pub type CardId = u64;
pub type DungeonId = u64;
pub type RoomId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommonActionFamily {
    GainEnergy,
    TakeInitiative,
    Explore,
    Learn,
    Investigate,
    Support,
    RingTemptsYou,
    VentureIntoDungeon,
    BecomeMonarch,
    Proliferate,
    Amass,
}

impl CommonActionFamily {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::GainEnergy => "gain-energy",
            Self::TakeInitiative => "take-initiative",
            Self::Explore => "explore",
            Self::Learn => "learn",
            Self::Investigate => "investigate",
            Self::Support => "support",
            Self::RingTemptsYou => "ring-tempts-you",
            Self::VentureIntoDungeon => "venture-into-dungeon",
            Self::BecomeMonarch => "become-monarch",
            Self::Proliferate => "proliferate",
            Self::Amass => "amass",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedAmount {
    Fixed(u32),
    EffectVariable,
}

impl ResolvedAmount {
    fn stable_id(self) -> String {
        match self {
            Self::Fixed(amount) => format!("fixed:{amount}"),
            Self::EffectVariable => "effect-variable".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreSubject {
    SourcePermanent,
    TargetCreatureYouControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportTargetPolicy {
    AnyCreature,
    OtherThanSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArmySubtype {
    Orc,
    Zombie,
}

impl ArmySubtype {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Orc => "orc",
            Self::Zombie => "zombie",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonActionKind {
    GainEnergy {
        amount: ResolvedAmount,
    },
    TakeInitiative,
    Explore {
        subject: ExploreSubject,
        repetitions: ResolvedAmount,
    },
    Learn,
    Investigate {
        repetitions: ResolvedAmount,
    },
    Support {
        maximum_targets: ResolvedAmount,
        target_policy: SupportTargetPolicy,
    },
    RingTemptsYou,
    VentureIntoDungeon,
    BecomeMonarch,
    Proliferate,
    Amass {
        subtype: ArmySubtype,
        amount: ResolvedAmount,
    },
}

impl CommonActionKind {
    pub const fn family(&self) -> CommonActionFamily {
        match self {
            Self::GainEnergy { .. } => CommonActionFamily::GainEnergy,
            Self::TakeInitiative => CommonActionFamily::TakeInitiative,
            Self::Explore { .. } => CommonActionFamily::Explore,
            Self::Learn => CommonActionFamily::Learn,
            Self::Investigate { .. } => CommonActionFamily::Investigate,
            Self::Support { .. } => CommonActionFamily::Support,
            Self::RingTemptsYou => CommonActionFamily::RingTemptsYou,
            Self::VentureIntoDungeon => CommonActionFamily::VentureIntoDungeon,
            Self::BecomeMonarch => CommonActionFamily::BecomeMonarch,
            Self::Proliferate => CommonActionFamily::Proliferate,
            Self::Amass { .. } => CommonActionFamily::Amass,
        }
    }

    fn stable_id(&self) -> String {
        match self {
            Self::GainEnergy { amount } => format!("gain-energy/{}", amount.stable_id()),
            Self::TakeInitiative => "take-initiative".into(),
            Self::Explore {
                subject,
                repetitions,
            } => format!("explore/{subject:?}/{}", repetitions.stable_id()),
            Self::Learn => "learn".into(),
            Self::Investigate { repetitions } => {
                format!("investigate/{}", repetitions.stable_id())
            }
            Self::Support {
                maximum_targets,
                target_policy,
            } => format!("support/{target_policy:?}/{}", maximum_targets.stable_id()),
            Self::RingTemptsYou => "ring-tempts-you".into(),
            Self::VentureIntoDungeon => "venture-into-dungeon".into(),
            Self::BecomeMonarch => "become-monarch".into(),
            Self::Proliferate => "proliferate".into(),
            Self::Amass { subtype, amount } => {
                format!("amass/{}/{}", subtype.stable_id(), amount.stable_id())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEnvelope {
    SourceEnters,
    SourceDies,
    SourceAttacks,
    SourceDealsCombatDamageToPlayer,
    ControllerUpkeepBegins,
    ControllerEndStepBegins,
}

impl TriggerEnvelope {
    fn stable_id(self) -> &'static str {
        match self {
            Self::SourceEnters => "source-enters",
            Self::SourceDies => "source-dies",
            Self::SourceAttacks => "source-attacks",
            Self::SourceDealsCombatDamageToPlayer => "source-combat-damage-player",
            Self::ControllerUpkeepBegins => "controller-upkeep-begins",
            Self::ControllerEndStepBegins => "controller-end-step-begins",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonActionTiming {
    ResolvingInstruction,
    Triggered(TriggerEnvelope),
}

impl CommonActionTiming {
    fn stable_id(self) -> &'static str {
        match self {
            Self::ResolvingInstruction => "resolution",
            Self::Triggered(envelope) => envelope.stable_id(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonActionProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    timing: CommonActionTiming,
    kind: CommonActionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonActionLeafProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    timing_context: CommonActionTiming,
    kind: CommonActionKind,
}

impl CommonActionLeafProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub const fn timing_context(&self) -> CommonActionTiming {
        self.timing_context
    }

    pub fn kind(&self) -> &CommonActionKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        common_action_production_adapter_connected()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedCommonActionComposition {
    pub timing: CommonActionTiming,
    pub action_byte_start: usize,
    pub action_byte_end: usize,
    pub leaf: CommonActionLeafProgram,
}

/// Compile one exact action leaf supplied by the lossless clause composition
/// layer. The timing context is relevant for pronouns such as "it" and for
/// support's source-exclusion contract, so it is part of semantic identity.
///
/// This function does not claim ownership of any surrounding trigger,
/// condition, cost, target declaration, replacement, or sibling instruction.
pub fn compile_common_action_leaf_program(
    exact_source: &str,
    normalized_source: &str,
    timing_context: CommonActionTiming,
) -> Option<CommonActionLeafProgram> {
    if !is_complete_single_line(exact_source)
        || !is_complete_single_line(normalized_source)
        || !normalized_is_content_derived(exact_source, normalized_source)
    {
        return None;
    }
    let kind = parse_reviewed_action(exact_source, timing_context)?;
    let semantic_digest = common_action_leaf_semantic_digest(exact_source, timing_context, &kind);
    Some(CommonActionLeafProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        semantic_digest,
        timing_context,
        kind,
    })
}

/// Decompose only a clause whose complete envelope is one of the reviewed
/// forms in this module. Compounds are rejected rather than substring matched.
pub fn decompose_reviewed_common_action_clause(
    exact_source: &str,
    normalized_source: &str,
) -> Option<ReviewedCommonActionComposition> {
    if !is_complete_single_line(exact_source)
        || !is_complete_single_line(normalized_source)
        || !normalized_is_content_derived(exact_source, normalized_source)
    {
        return None;
    }
    let (timing, action_source) = split_reviewed_envelope(exact_source)?;
    let action_byte_start = exact_source.len().checked_sub(action_source.len())?;
    let leaf = compile_common_action_leaf_program(action_source, action_source, timing)?;
    Some(ReviewedCommonActionComposition {
        timing,
        action_byte_start,
        action_byte_end: exact_source.len(),
        leaf,
    })
}

impl CommonActionProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub const fn timing(&self) -> CommonActionTiming {
        self.timing
    }

    pub fn kind(&self) -> &CommonActionKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        common_action_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarlierCommonActionOwner {
    OfficialInvestigateRuntime,
    ReviewedMonarchEntryRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonActionClauseClassification {
    Program(CommonActionProgram),
    EarlierOwner {
        family: CommonActionFamily,
        owner: EarlierCommonActionOwner,
    },
    Rejected,
}

pub fn compile_common_action_program(
    exact_source: &str,
    normalized_source: &str,
) -> Option<CommonActionProgram> {
    match classify_common_action_clause(exact_source, normalized_source) {
        CommonActionClauseClassification::Program(program) => Some(program),
        CommonActionClauseClassification::EarlierOwner { .. }
        | CommonActionClauseClassification::Rejected => None,
    }
}

pub fn classify_common_action_clause(
    exact_source: &str,
    normalized_source: &str,
) -> CommonActionClauseClassification {
    if !is_complete_single_line(exact_source)
        || !is_complete_single_line(normalized_source)
        || !normalized_is_content_derived(exact_source, normalized_source)
    {
        return CommonActionClauseClassification::Rejected;
    }

    let Some((timing, action_source)) = split_reviewed_envelope(exact_source) else {
        return CommonActionClauseClassification::Rejected;
    };
    let Some(kind) = parse_reviewed_action(action_source, timing) else {
        return CommonActionClauseClassification::Rejected;
    };

    if is_earlier_owned(exact_source, &kind, timing) {
        let owner = match kind {
            CommonActionKind::Investigate { .. } => {
                EarlierCommonActionOwner::OfficialInvestigateRuntime
            }
            CommonActionKind::BecomeMonarch => {
                EarlierCommonActionOwner::ReviewedMonarchEntryRuntime
            }
            _ => unreachable!("only reviewed earlier-owned families reach this branch"),
        };
        return CommonActionClauseClassification::EarlierOwner {
            family: kind.family(),
            owner,
        };
    }

    let semantic_digest = common_action_semantic_digest(exact_source, timing, &kind);
    CommonActionClauseClassification::Program(CommonActionProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        semantic_digest,
        timing,
        kind,
    })
}

fn is_earlier_owned(
    exact_source: &str,
    kind: &CommonActionKind,
    timing: CommonActionTiming,
) -> bool {
    match (kind, timing) {
        (
            CommonActionKind::Investigate {
                repetitions: ResolvedAmount::Fixed(1),
            },
            CommonActionTiming::ResolvingInstruction,
        ) => matches!(
            exact_source,
            "Investigate."
                | "Investigate"
                | "Investigate. (Create a Clue token. It's an artifact with \"{2}, Sacrifice this token: Draw a card.\")"
        ),
        (
            CommonActionKind::BecomeMonarch,
            CommonActionTiming::Triggered(TriggerEnvelope::SourceEnters),
        ) => true,
        _ => false,
    }
}

fn split_reviewed_envelope(source: &str) -> Option<(CommonActionTiming, &str)> {
    const PREFIXES: &[(&str, TriggerEnvelope)] = &[
        ("When this creature enters, ", TriggerEnvelope::SourceEnters),
        ("When this artifact enters, ", TriggerEnvelope::SourceEnters),
        (
            "When this enchantment enters, ",
            TriggerEnvelope::SourceEnters,
        ),
        (
            "When this Equipment enters, ",
            TriggerEnvelope::SourceEnters,
        ),
        ("When this Aura enters, ", TriggerEnvelope::SourceEnters),
        (
            "When this permanent enters, ",
            TriggerEnvelope::SourceEnters,
        ),
        ("When this creature dies, ", TriggerEnvelope::SourceDies),
        (
            "Whenever this creature attacks, ",
            TriggerEnvelope::SourceAttacks,
        ),
        (
            "Whenever this creature deals combat damage to a player, ",
            TriggerEnvelope::SourceDealsCombatDamageToPlayer,
        ),
        (
            "At the beginning of your upkeep, ",
            TriggerEnvelope::ControllerUpkeepBegins,
        ),
        (
            "At the beginning of your end step, ",
            TriggerEnvelope::ControllerEndStepBegins,
        ),
    ];
    for (prefix, envelope) in PREFIXES {
        if let Some(action) = source.strip_prefix(prefix) {
            return Some((CommonActionTiming::Triggered(*envelope), action));
        }
    }
    Some((CommonActionTiming::ResolvingInstruction, source))
}

fn parse_reviewed_action(source: &str, timing: CommonActionTiming) -> Option<CommonActionKind> {
    let source = lowercase_first(source);
    parse_gain_energy(&source)
        .or_else(|| parse_take_initiative(&source))
        .or_else(|| parse_explore(&source, timing))
        .or_else(|| parse_learn(&source))
        .or_else(|| parse_investigate(&source))
        .or_else(|| parse_support(&source, timing))
        .or_else(|| parse_ring_tempts_you(&source))
        .or_else(|| parse_venture(&source))
        .or_else(|| parse_become_monarch(&source))
        .or_else(|| parse_proliferate(&source))
        .or_else(|| parse_amass(&source))
}

fn parse_gain_energy(source: &str) -> Option<CommonActionKind> {
    let (core, reminder) = split_trailing_reminder(source);
    let symbols = strip_optional_terminal_period(core).strip_prefix("you get ")?;
    let amount = if symbols == "X {E}" {
        ResolvedAmount::EffectVariable
    } else {
        let count = parse_repeated_symbol(symbols, "{E}")?;
        ResolvedAmount::Fixed(count)
    };
    if let Some(reminder) = reminder
        && !energy_reminder_matches(amount, reminder)
    {
        return None;
    }
    Some(CommonActionKind::GainEnergy { amount })
}

fn parse_take_initiative(source: &str) -> Option<CommonActionKind> {
    (strip_optional_terminal_period(source) == "you take the initiative")
        .then_some(CommonActionKind::TakeInitiative)
}

fn parse_explore(source: &str, timing: CommonActionTiming) -> Option<CommonActionKind> {
    const SELF_REMINDER: &str = "Reveal the top card of your library. Put that card into your hand if it's a land. Otherwise, put a +1/+1 counter on this creature, then put the card back or put it into your graveyard.";
    const TARGET_REMINDER: &str = "Reveal the top card of your library. Put that card into your hand if it's a land. Otherwise, put a +1/+1 counter on the creature, then put the card back or put it into your graveyard.";
    let (core, reminder) = split_trailing_reminder(source);
    let subject = match strip_optional_terminal_period(core) {
        "it explores" if matches!(timing, CommonActionTiming::Triggered(_)) => {
            ExploreSubject::SourcePermanent
        }
        "this creature explores" => ExploreSubject::SourcePermanent,
        "target creature you control explores" => ExploreSubject::TargetCreatureYouControl,
        _ => return None,
    };
    if let Some(reminder) = reminder {
        let expected = match subject {
            ExploreSubject::SourcePermanent => SELF_REMINDER,
            ExploreSubject::TargetCreatureYouControl => TARGET_REMINDER,
        };
        if reminder != expected {
            return None;
        }
    }
    Some(CommonActionKind::Explore {
        subject,
        repetitions: ResolvedAmount::Fixed(1),
    })
}

fn parse_learn(source: &str) -> Option<CommonActionKind> {
    const REMINDER: &str = "You may reveal a Lesson card you own from outside the game and put it into your hand, or discard a card to draw a card.";
    let (core, reminder) = split_trailing_reminder(source);
    if strip_optional_terminal_period(core) != "learn"
        || reminder.is_some_and(|text| text != REMINDER)
    {
        return None;
    }
    Some(CommonActionKind::Learn)
}

fn parse_investigate(source: &str) -> Option<CommonActionKind> {
    const REMINDER: &str =
        "Create a Clue token. It's an artifact with \"{2}, Sacrifice this token: Draw a card.\"";
    let (core, reminder) = split_trailing_reminder(source);
    if reminder.is_some_and(|text| text != REMINDER) {
        return None;
    }
    let amount = match strip_optional_terminal_period(core) {
        "investigate" => ResolvedAmount::Fixed(1),
        _ => return None,
    };
    Some(CommonActionKind::Investigate {
        repetitions: amount,
    })
}

fn parse_support(source: &str, timing: CommonActionTiming) -> Option<CommonActionKind> {
    let (core, reminder) = split_trailing_reminder(source);
    let amount_text = strip_optional_terminal_period(core).strip_prefix("support ")?;
    let maximum_targets = parse_amount(amount_text)?;
    let Some(reminder) = reminder else {
        return None;
    };
    let amount_words = amount_words(maximum_targets)?;
    let other = reminder
        == format!("Put a +1/+1 counter on each of up to {amount_words} other target creatures.");
    let any = reminder
        == format!("Put a +1/+1 counter on each of up to {amount_words} target creatures.");
    let target_policy = match (other, any) {
        (true, false) => SupportTargetPolicy::OtherThanSource,
        (false, true) => SupportTargetPolicy::AnyCreature,
        _ => return None,
    };
    if matches!(timing, CommonActionTiming::ResolvingInstruction)
        && target_policy == SupportTargetPolicy::OtherThanSource
    {
        return None;
    }
    Some(CommonActionKind::Support {
        maximum_targets,
        target_policy,
    })
}

fn parse_ring_tempts_you(source: &str) -> Option<CommonActionKind> {
    (strip_optional_terminal_period(source) == "the Ring tempts you")
        .then_some(CommonActionKind::RingTemptsYou)
}

fn parse_venture(source: &str) -> Option<CommonActionKind> {
    const REMINDERS: &[&str] = &[
        "Enter the first room or advance to the next room.",
        "To venture into the dungeon, enter the first room or advance to the next room.",
    ];
    let (core, reminder) = split_trailing_reminder(source);
    if strip_optional_terminal_period(core) != "venture into the dungeon"
        || reminder.is_some_and(|text| !REMINDERS.contains(&text))
    {
        return None;
    }
    Some(CommonActionKind::VentureIntoDungeon)
}

fn parse_become_monarch(source: &str) -> Option<CommonActionKind> {
    (strip_optional_terminal_period(source) == "you become the monarch")
        .then_some(CommonActionKind::BecomeMonarch)
}

fn parse_proliferate(source: &str) -> Option<CommonActionKind> {
    const REMINDER: &str = "Choose any number of permanents and/or players, then give each another counter of each kind already there.";
    let (core, reminder) = split_trailing_reminder(source);
    (strip_optional_terminal_period(core) == "proliferate"
        && reminder.is_none_or(|text| text == REMINDER))
    .then_some(CommonActionKind::Proliferate)
}

fn parse_amass(source: &str) -> Option<CommonActionKind> {
    let (core, reminder) = split_trailing_reminder(source);
    let body = strip_optional_terminal_period(core).strip_prefix("amass ")?;
    let (subtype, amount_text) = if let Some(amount) = body.strip_prefix("Orcs ") {
        (ArmySubtype::Orc, amount)
    } else {
        let amount = body.strip_prefix("Zombies ")?;
        (ArmySubtype::Zombie, amount)
    };
    let amount = parse_amount(amount_text)?;
    let ResolvedAmount::Fixed(fixed) = amount else {
        return None;
    };
    let Some(reminder) = reminder else {
        return None;
    };
    let subtype_word = match subtype {
        ArmySubtype::Orc => "Orc",
        ArmySubtype::Zombie => "Zombie",
    };
    let subtype_article = match subtype {
        ArmySubtype::Orc => "an",
        ArmySubtype::Zombie => "a",
    };
    let counter_words = english_cardinal(fixed)?;
    let expected = if fixed == 1 {
        format!(
            "Put a +1/+1 counter on an Army you control. It's also {subtype_article} {subtype_word}. If you don't control an Army, create a 0/0 black {subtype_word} Army creature token first."
        )
    } else {
        format!(
            "Put {counter_words} +1/+1 counters on an Army you control. It's also {subtype_article} {subtype_word}. If you don't control an Army, create a 0/0 black {subtype_word} Army creature token first."
        )
    };
    let alternate = format!(
        "To amass {subtype_word}s {fixed}, {expected_start}",
        expected_start = lowercase_first(&expected)
    );
    if reminder != expected && reminder != alternate {
        return None;
    }
    Some(CommonActionKind::Amass { subtype, amount })
}

fn lowercase_first(source: &str) -> String {
    let mut characters = source.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    first.to_lowercase().chain(characters).collect()
}

fn split_trailing_reminder(source: &str) -> (&str, Option<&str>) {
    let without_close = source
        .strip_suffix(')')
        .or_else(|| source.strip_suffix(")."));
    let Some(without_close) = without_close else {
        return (source, None);
    };
    let Some(open) = without_close.rfind(" (") else {
        return (source, None);
    };
    (&source[..open], Some(&without_close[open + 2..]))
}

fn strip_optional_terminal_period(source: &str) -> &str {
    source.strip_suffix('.').unwrap_or(source)
}

fn parse_repeated_symbol(source: &str, symbol: &str) -> Option<u32> {
    if source.is_empty() {
        return None;
    }
    let mut remaining = source;
    let mut count = 0u32;
    while let Some(rest) = remaining.strip_prefix(symbol) {
        count = count.checked_add(1)?;
        remaining = rest;
    }
    (remaining.is_empty() && count > 0).then_some(count)
}

fn parse_amount(source: &str) -> Option<ResolvedAmount> {
    if source == "X" {
        return Some(ResolvedAmount::EffectVariable);
    }
    parse_positive_u32(source).map(ResolvedAmount::Fixed)
}

fn parse_positive_u32(source: &str) -> Option<u32> {
    if source.is_empty()
        || (source.len() > 1 && source.starts_with('0'))
        || !source.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    source.parse::<u32>().ok().filter(|amount| *amount > 0)
}

fn amount_words(amount: ResolvedAmount) -> Option<String> {
    match amount {
        ResolvedAmount::Fixed(amount) => english_cardinal(amount),
        ResolvedAmount::EffectVariable => Some("X".into()),
    }
}

fn english_cardinal(amount: u32) -> Option<String> {
    const WORDS: [&str; 13] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve",
    ];
    WORDS.get(amount as usize).map(|word| (*word).to_owned())
}

fn energy_reminder_matches(amount: ResolvedAmount, reminder: &str) -> bool {
    match amount {
        ResolvedAmount::Fixed(1) => reminder == "an energy counter",
        ResolvedAmount::Fixed(amount) => english_cardinal(amount)
            .is_some_and(|word| reminder == format!("{word} energy counters")),
        ResolvedAmount::EffectVariable => reminder == "energy counters",
    }
}

fn is_complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source.trim() == source
        && !source.contains(['\r', '\n'])
        && source.split_whitespace().collect::<Vec<_>>().join(" ") == source
}

pub fn reviewed_common_action_normalized_source(exact_source: &str) -> String {
    exact_source.to_owned()
}

fn normalized_is_content_derived(exact_source: &str, normalized_source: &str) -> bool {
    normalized_source == exact_source
        || normalized_source == reviewed_common_action_normalized_source(exact_source)
}

fn common_action_semantic_digest(
    exact_source: &str,
    timing: CommonActionTiming,
    kind: &CommonActionKind,
) -> String {
    let mut hasher = Sha256::new();
    for component in [
        "common-action-procedure-content/v1",
        COMMON_ACTION_PROCEDURE_COMPILER_VERSION,
        COMMON_ACTION_PROCEDURE_RUNTIME_VERSION,
        COMMON_ACTION_PROCEDURE_RULES_CONTEXT_VERSION,
        exact_source,
        timing.stable_id(),
        &kind.stable_id(),
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn common_action_leaf_semantic_digest(
    exact_source: &str,
    timing_context: CommonActionTiming,
    kind: &CommonActionKind,
) -> String {
    let mut hasher = Sha256::new();
    for component in [
        "common-action-leaf-content/v1",
        COMMON_ACTION_PROCEDURE_COMPILER_VERSION,
        COMMON_ACTION_PROCEDURE_RUNTIME_VERSION,
        COMMON_ACTION_PROCEDURE_RULES_CONTEXT_VERSION,
        exact_source,
        timing_context.stable_id(),
        &kind.stable_id(),
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEventKind {
    EntersBattlefield,
    Dies,
    DeclaredAttacker,
    CombatDamageToPlayer,
    UpkeepBegins,
    EndStepBegins,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonActionTriggerEvent {
    pub event_id: EventId,
    pub kind: TriggerEventKind,
    pub source: Option<ObjectRef>,
    pub source_controller_lki: Option<PlayerId>,
    pub active_player: Option<PlayerId>,
    pub damaged_player: Option<PlayerId>,
    pub event_group_complete: bool,
    pub controller_evidence_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingCommonAction {
    pub trigger_id: Option<TriggerId>,
    pub controller: PlayerId,
    pub source: Option<ObjectRef>,
    pub program_digest: String,
    pub kind: CommonActionKind,
}

pub fn create_common_action_trigger(
    trigger_id: TriggerId,
    program: &CommonActionProgram,
    bound_source: ObjectRef,
    event: &CommonActionTriggerEvent,
) -> Result<Option<PendingCommonAction>, CommonActionRuntimeError> {
    let CommonActionTiming::Triggered(envelope) = program.timing else {
        return Err(CommonActionRuntimeError::WrongProgramTiming);
    };
    if !event.event_group_complete || !event.controller_evidence_complete {
        return Err(CommonActionRuntimeError::IncompleteTriggerEvidence);
    }
    let expected_kind = match envelope {
        TriggerEnvelope::SourceEnters => TriggerEventKind::EntersBattlefield,
        TriggerEnvelope::SourceDies => TriggerEventKind::Dies,
        TriggerEnvelope::SourceAttacks => TriggerEventKind::DeclaredAttacker,
        TriggerEnvelope::SourceDealsCombatDamageToPlayer => TriggerEventKind::CombatDamageToPlayer,
        TriggerEnvelope::ControllerUpkeepBegins => TriggerEventKind::UpkeepBegins,
        TriggerEnvelope::ControllerEndStepBegins => TriggerEventKind::EndStepBegins,
    };
    if event.kind != expected_kind {
        return Ok(None);
    }

    let (source, controller) = match envelope {
        TriggerEnvelope::ControllerUpkeepBegins | TriggerEnvelope::ControllerEndStepBegins => {
            let controller = event
                .source_controller_lki
                .ok_or(CommonActionRuntimeError::MissingSourceController)?;
            if event.active_player != Some(controller) {
                return Ok(None);
            }
            (Some(bound_source), controller)
        }
        _ => {
            if event.source != Some(bound_source) {
                return Ok(None);
            }
            let controller = event
                .source_controller_lki
                .ok_or(CommonActionRuntimeError::MissingSourceController)?;
            (Some(bound_source), controller)
        }
    };
    Ok(Some(PendingCommonAction {
        trigger_id: Some(trigger_id),
        controller,
        source,
        program_digest: program.semantic_digest.clone(),
        kind: program.kind.clone(),
    }))
}

pub fn begin_resolving_common_action(
    program: &CommonActionProgram,
    controller: PlayerId,
    source: Option<ObjectRef>,
) -> Result<PendingCommonAction, CommonActionRuntimeError> {
    if program.timing != CommonActionTiming::ResolvingInstruction {
        return Err(CommonActionRuntimeError::WrongProgramTiming);
    }
    Ok(PendingCommonAction {
        trigger_id: None,
        controller,
        source,
        program_digest: program.semantic_digest.clone(),
        kind: program.kind.clone(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Command,
    OutsideGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CardType {
    Artifact,
    Creature,
    Dungeon,
    Enchantment,
    Instant,
    Land,
    Planeswalker,
    Sorcery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardRecord {
    pub card_id: CardId,
    pub owner: PlayerId,
    pub zone: Zone,
    pub card_types: BTreeSet<CardType>,
    pub subtypes: BTreeSet<String>,
    pub public_identity: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermanentRecord {
    pub object: ObjectRef,
    pub card_id: Option<CardId>,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub card_types: BTreeSet<CardType>,
    pub subtypes: BTreeSet<String>,
    pub counters: BTreeMap<String, u32>,
    pub is_token: bool,
}

impl PermanentRecord {
    pub fn is_creature(&self) -> bool {
        self.card_types.contains(&CardType::Creature)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerProcedureState {
    pub energy: u32,
    /// Top card is the final element.
    pub library: Vec<CardId>,
    pub hand: BTreeSet<CardId>,
    pub graveyard: BTreeSet<CardId>,
    pub outside_game: BTreeSet<CardId>,
    pub counters: BTreeMap<String, u32>,
    pub ring_temptation_count: u32,
    pub ring_bearer: Option<ObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DungeonDefinition {
    pub dungeon_id: DungeonId,
    pub owner: PlayerId,
    pub is_undercity: bool,
    pub top_room: RoomId,
    pub bottom_rooms: BTreeSet<RoomId>,
    pub outward_arrows: BTreeMap<RoomId, BTreeSet<RoomId>>,
    pub graph_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonProgress {
    pub dungeon_id: DungeonId,
    pub room_id: RoomId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonProcedureState {
    pub players: BTreeMap<PlayerId, PlayerProcedureState>,
    pub cards: BTreeMap<CardId, CardRecord>,
    pub permanents: BTreeMap<ObjectRef, PermanentRecord>,
    pub monarch: Option<PlayerId>,
    pub initiative: Option<PlayerId>,
    pub dungeons: BTreeMap<DungeonId, DungeonDefinition>,
    pub dungeon_progress: BTreeMap<PlayerId, DungeonProgress>,
    pub pending_undercity_ventures: Vec<PendingUndercityVenture>,
    pub completed_dungeons: BTreeMap<PlayerId, u32>,
    pub next_object_id: ObjectId,
    pub public_players_complete: bool,
    pub battlefield_complete: bool,
    pub counter_state_complete: bool,
    pub outside_game_complete: bool,
    pub dungeon_catalog_complete: bool,
    pub team_assignments: Option<BTreeMap<PlayerId, u16>>,
}

impl CommonProcedureState {
    fn player(&self, player: PlayerId) -> Result<&PlayerProcedureState, CommonActionRuntimeError> {
        self.players
            .get(&player)
            .ok_or(CommonActionRuntimeError::MissingPlayer(player))
    }

    fn player_mut(
        &mut self,
        player: PlayerId,
    ) -> Result<&mut PlayerProcedureState, CommonActionRuntimeError> {
        self.players
            .get_mut(&player)
            .ok_or(CommonActionRuntimeError::MissingPlayer(player))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingUndercityVenture {
    pub player: PlayerId,
    pub caused_by_trigger: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonActionRuntimeError {
    WrongProgramTiming,
    WrongActionFamily,
    IncompleteTriggerEvidence,
    MissingSourceController,
    MissingSource,
    MissingPlayer(PlayerId),
    MissingPermanent(ObjectRef),
    StaleObject(ObjectRef),
    WrongController {
        object: ObjectRef,
        expected: PlayerId,
        actual: PlayerId,
    },
    ObjectIsNotCreature(ObjectRef),
    AmountEvidenceMismatch,
    AmountOverflow,
    EnergyOverflow,
    IncompleteBattlefieldEvidence,
    IncompleteCounterEvidence,
    IncompleteOutsideGameEvidence,
    IncompleteDungeonCatalog,
    IncompleteLibraryEvidence,
    MissingLibraryCard(CardId),
    LibraryZoneMismatch(CardId),
    InvalidExploreChoice,
    MissingExploreChoice,
    CounterPlacementMismatch,
    InvalidLearnChoice,
    MissingLearnCard(CardId),
    CardOwnershipMismatch(CardId),
    CardZoneMismatch {
        card: CardId,
        expected: Zone,
        actual: Zone,
    },
    CardIsNotLesson(CardId),
    DuplicateTarget(ObjectRef),
    TooManyTargets {
        maximum: u32,
        actual: usize,
    },
    IllegalSupportTarget(ObjectRef),
    MissingRingBearerChoice,
    IllegalRingBearer(ObjectRef),
    RingTemptationOverflow,
    MissingDungeonChoice,
    IllegalDungeonChoice(DungeonId),
    MissingRoomChoice,
    IllegalRoomChoice(RoomId),
    MalformedDungeonGraph(DungeonId),
    DungeonCompletionOverflow,
    UnsupportedTeamProliferation,
    IllegalProliferateChoice,
    CounterOverflow,
    MissingArmyChoice,
    IllegalArmyChoice(ObjectRef),
    ObjectIdOverflow,
}

impl fmt::Display for CommonActionRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CommonActionRuntimeError {}

fn resolved_amount(
    expected: ResolvedAmount,
    supplied: Option<u32>,
) -> Result<u32, CommonActionRuntimeError> {
    match (expected, supplied) {
        (ResolvedAmount::Fixed(expected), None) => Ok(expected),
        (ResolvedAmount::Fixed(expected), Some(supplied)) if supplied == expected => Ok(expected),
        (ResolvedAmount::Fixed(_), Some(_)) => {
            Err(CommonActionRuntimeError::AmountEvidenceMismatch)
        }
        (ResolvedAmount::EffectVariable, Some(amount)) => Ok(amount),
        (ResolvedAmount::EffectVariable, None) => {
            Err(CommonActionRuntimeError::AmountEvidenceMismatch)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnergyReceipt {
    pub player: PlayerId,
    pub gained: u32,
    pub before: u32,
    pub after: u32,
    pub program_digest: String,
}

pub fn resolve_gain_energy(
    pending: &PendingCommonAction,
    dynamic_amount: Option<u32>,
    state: &mut CommonProcedureState,
) -> Result<EnergyReceipt, CommonActionRuntimeError> {
    let CommonActionKind::GainEnergy { amount } = pending.kind else {
        return Err(CommonActionRuntimeError::WrongActionFamily);
    };
    let gained = resolved_amount(amount, dynamic_amount)?;
    let player = state.player_mut(pending.controller)?;
    let before = player.energy;
    let after = before
        .checked_add(gained)
        .ok_or(CommonActionRuntimeError::EnergyOverflow)?;
    player.energy = after;
    Ok(EnergyReceipt {
        player: pending.controller,
        gained,
        before,
        after,
        program_digest: pending.program_digest.clone(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreNonlandChoice {
    LeaveOnTop,
    PutIntoGraveyard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExploreResolutionInput {
    pub target: ObjectRef,
    pub nonland_choice: Option<ExploreNonlandChoice>,
    pub counter_was_placed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExploreReceipt {
    LandToHand {
        player: PlayerId,
        target: ObjectRef,
        card: CardId,
    },
    Nonland {
        player: PlayerId,
        target: ObjectRef,
        card: CardId,
        counter_was_placed: bool,
        destination: Zone,
    },
    EmptyLibrary {
        player: PlayerId,
        target: ObjectRef,
    },
}

pub fn resolve_explore(
    pending: &PendingCommonAction,
    input: ExploreResolutionInput,
    state: &mut CommonProcedureState,
) -> Result<ExploreReceipt, CommonActionRuntimeError> {
    let CommonActionKind::Explore {
        subject,
        repetitions: ResolvedAmount::Fixed(1),
    } = pending.kind
    else {
        return Err(CommonActionRuntimeError::WrongActionFamily);
    };
    if !state.battlefield_complete {
        return Err(CommonActionRuntimeError::IncompleteBattlefieldEvidence);
    }
    let permanent = state
        .permanents
        .get(&input.target)
        .ok_or(CommonActionRuntimeError::MissingPermanent(input.target))?;
    if !permanent.is_creature() {
        return Err(CommonActionRuntimeError::ObjectIsNotCreature(input.target));
    }
    if permanent.controller != pending.controller {
        return Err(CommonActionRuntimeError::WrongController {
            object: input.target,
            expected: pending.controller,
            actual: permanent.controller,
        });
    }
    if subject == ExploreSubject::SourcePermanent && pending.source != Some(input.target) {
        return Err(CommonActionRuntimeError::StaleObject(input.target));
    }

    let Some(card_id) = state.player(pending.controller)?.library.last().copied() else {
        if input.nonland_choice.is_some() || input.counter_was_placed.is_some() {
            return Err(CommonActionRuntimeError::InvalidExploreChoice);
        }
        return Ok(ExploreReceipt::EmptyLibrary {
            player: pending.controller,
            target: input.target,
        });
    };
    let card = state
        .cards
        .get(&card_id)
        .ok_or(CommonActionRuntimeError::MissingLibraryCard(card_id))?;
    if card.owner != pending.controller || card.zone != Zone::Library {
        return Err(CommonActionRuntimeError::LibraryZoneMismatch(card_id));
    }
    let is_land = card.card_types.contains(&CardType::Land);
    if is_land {
        if input.nonland_choice.is_some() || input.counter_was_placed.is_some() {
            return Err(CommonActionRuntimeError::InvalidExploreChoice);
        }
        move_top_library_card(state, pending.controller, card_id, Zone::Hand)?;
        return Ok(ExploreReceipt::LandToHand {
            player: pending.controller,
            target: input.target,
            card: card_id,
        });
    }

    let choice = input
        .nonland_choice
        .ok_or(CommonActionRuntimeError::MissingExploreChoice)?;
    let counter_was_placed = input
        .counter_was_placed
        .ok_or(CommonActionRuntimeError::CounterPlacementMismatch)?;
    if counter_was_placed {
        add_counter_to_permanent(state, input.target, "+1/+1", 1)?;
    }
    let destination = match choice {
        ExploreNonlandChoice::LeaveOnTop => Zone::Library,
        ExploreNonlandChoice::PutIntoGraveyard => {
            move_top_library_card(state, pending.controller, card_id, Zone::Graveyard)?;
            Zone::Graveyard
        }
    };
    Ok(ExploreReceipt::Nonland {
        player: pending.controller,
        target: input.target,
        card: card_id,
        counter_was_placed,
        destination,
    })
}

fn move_top_library_card(
    state: &mut CommonProcedureState,
    player: PlayerId,
    card_id: CardId,
    destination: Zone,
) -> Result<(), CommonActionRuntimeError> {
    let popped = state
        .player_mut(player)?
        .library
        .pop()
        .ok_or(CommonActionRuntimeError::IncompleteLibraryEvidence)?;
    if popped != card_id {
        return Err(CommonActionRuntimeError::IncompleteLibraryEvidence);
    }
    let card = state
        .cards
        .get_mut(&card_id)
        .ok_or(CommonActionRuntimeError::MissingLibraryCard(card_id))?;
    card.zone = destination;
    match destination {
        Zone::Hand => {
            state.player_mut(player)?.hand.insert(card_id);
        }
        Zone::Graveyard => {
            state.player_mut(player)?.graveyard.insert(card_id);
        }
        _ => {}
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearnChoice {
    Decline,
    DiscardThenDraw { discard: CardId },
    RevealLesson { lesson: CardId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearnReceipt {
    Declined,
    DiscardedAndDrew {
        player: PlayerId,
        discarded: CardId,
        drawn: Option<CardId>,
    },
    RevealedLesson {
        player: PlayerId,
        lesson: CardId,
    },
}

pub fn resolve_learn(
    pending: &PendingCommonAction,
    choice: LearnChoice,
    state: &mut CommonProcedureState,
) -> Result<LearnReceipt, CommonActionRuntimeError> {
    if pending.kind != CommonActionKind::Learn {
        return Err(CommonActionRuntimeError::WrongActionFamily);
    }
    match choice {
        LearnChoice::Decline => Ok(LearnReceipt::Declined),
        LearnChoice::DiscardThenDraw { discard } => {
            if !state.player(pending.controller)?.hand.contains(&discard) {
                return Err(CommonActionRuntimeError::MissingLearnCard(discard));
            }
            let card = state
                .cards
                .get(&discard)
                .ok_or(CommonActionRuntimeError::MissingLearnCard(discard))?;
            if card.owner != pending.controller {
                return Err(CommonActionRuntimeError::CardOwnershipMismatch(discard));
            }
            state.player_mut(pending.controller)?.hand.remove(&discard);
            state
                .player_mut(pending.controller)?
                .graveyard
                .insert(discard);
            state
                .cards
                .get_mut(&discard)
                .expect("validated card remains present")
                .zone = Zone::Graveyard;
            let drawn = state.player(pending.controller)?.library.last().copied();
            if let Some(card_id) = drawn {
                move_top_library_card(state, pending.controller, card_id, Zone::Hand)?;
            }
            Ok(LearnReceipt::DiscardedAndDrew {
                player: pending.controller,
                discarded: discard,
                drawn,
            })
        }
        LearnChoice::RevealLesson { lesson } => {
            if !state.outside_game_complete {
                return Err(CommonActionRuntimeError::IncompleteOutsideGameEvidence);
            }
            if !state
                .player(pending.controller)?
                .outside_game
                .contains(&lesson)
            {
                return Err(CommonActionRuntimeError::MissingLearnCard(lesson));
            }
            let card = state
                .cards
                .get(&lesson)
                .ok_or(CommonActionRuntimeError::MissingLearnCard(lesson))?;
            if card.owner != pending.controller {
                return Err(CommonActionRuntimeError::CardOwnershipMismatch(lesson));
            }
            if card.zone != Zone::OutsideGame {
                return Err(CommonActionRuntimeError::CardZoneMismatch {
                    card: lesson,
                    expected: Zone::OutsideGame,
                    actual: card.zone,
                });
            }
            if !card.subtypes.contains("Lesson") {
                return Err(CommonActionRuntimeError::CardIsNotLesson(lesson));
            }
            state
                .player_mut(pending.controller)?
                .outside_game
                .remove(&lesson);
            state.player_mut(pending.controller)?.hand.insert(lesson);
            state
                .cards
                .get_mut(&lesson)
                .expect("validated lesson remains present")
                .zone = Zone::Hand;
            Ok(LearnReceipt::RevealedLesson {
                player: pending.controller,
                lesson,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigateReceipt {
    pub player: PlayerId,
    pub clue_tokens: Vec<ObjectRef>,
}

pub fn resolve_investigate(
    pending: &PendingCommonAction,
    dynamic_repetitions: Option<u32>,
    state: &mut CommonProcedureState,
) -> Result<InvestigateReceipt, CommonActionRuntimeError> {
    let CommonActionKind::Investigate { repetitions } = pending.kind else {
        return Err(CommonActionRuntimeError::WrongActionFamily);
    };
    let repetitions = resolved_amount(repetitions, dynamic_repetitions)?;
    let mut clue_tokens = Vec::new();
    for _ in 0..repetitions {
        let object = allocate_object(state)?;
        state.permanents.insert(
            object,
            PermanentRecord {
                object,
                card_id: None,
                owner: pending.controller,
                controller: pending.controller,
                card_types: BTreeSet::from([CardType::Artifact]),
                subtypes: BTreeSet::from(["Clue".into()]),
                counters: BTreeMap::new(),
                is_token: true,
            },
        );
        clue_tokens.push(object);
    }
    Ok(InvestigateReceipt {
        player: pending.controller,
        clue_tokens,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportResolutionInput {
    pub targets: Vec<ObjectRef>,
    pub dynamic_maximum: Option<u32>,
    /// The outcome after replacement effects, in target order.
    pub counters_placed: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportReceipt {
    pub player: PlayerId,
    pub targets: Vec<(ObjectRef, u32)>,
}

pub fn resolve_support(
    pending: &PendingCommonAction,
    input: SupportResolutionInput,
    state: &mut CommonProcedureState,
) -> Result<SupportReceipt, CommonActionRuntimeError> {
    let CommonActionKind::Support {
        maximum_targets,
        target_policy,
    } = pending.kind
    else {
        return Err(CommonActionRuntimeError::WrongActionFamily);
    };
    if !state.battlefield_complete {
        return Err(CommonActionRuntimeError::IncompleteBattlefieldEvidence);
    }
    let maximum = resolved_amount(maximum_targets, input.dynamic_maximum)?;
    if input.targets.len() > maximum as usize {
        return Err(CommonActionRuntimeError::TooManyTargets {
            maximum,
            actual: input.targets.len(),
        });
    }
    if input.counters_placed.len() != input.targets.len() {
        return Err(CommonActionRuntimeError::CounterPlacementMismatch);
    }
    let mut distinct = BTreeSet::new();
    for target in &input.targets {
        if !distinct.insert(*target) {
            return Err(CommonActionRuntimeError::DuplicateTarget(*target));
        }
        let permanent = state
            .permanents
            .get(target)
            .ok_or(CommonActionRuntimeError::MissingPermanent(*target))?;
        if !permanent.is_creature()
            || (target_policy == SupportTargetPolicy::OtherThanSource
                && pending.source == Some(*target))
        {
            return Err(CommonActionRuntimeError::IllegalSupportTarget(*target));
        }
    }
    for (target, placed) in input
        .targets
        .iter()
        .copied()
        .zip(input.counters_placed.iter().copied())
    {
        if placed > 1 {
            return Err(CommonActionRuntimeError::CounterPlacementMismatch);
        }
        if placed == 1 {
            add_counter_to_permanent(state, target, "+1/+1", 1)?;
        }
    }
    Ok(SupportReceipt {
        player: pending.controller,
        targets: input
            .targets
            .into_iter()
            .zip(input.counters_placed)
            .collect(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RingTemptationInput {
    /// `None` is legal only if the player controls no creature.
    pub chosen_bearer: Option<ObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingTemptationReceipt {
    pub player: PlayerId,
    pub temptation_count: u32,
    pub old_bearer: Option<ObjectRef>,
    pub new_bearer: Option<ObjectRef>,
    pub emblem_created: bool,
}

pub fn resolve_ring_tempts_you(
    pending: &PendingCommonAction,
    input: RingTemptationInput,
    state: &mut CommonProcedureState,
) -> Result<RingTemptationReceipt, CommonActionRuntimeError> {
    if pending.kind != CommonActionKind::RingTemptsYou {
        return Err(CommonActionRuntimeError::WrongActionFamily);
    }
    if !state.battlefield_complete {
        return Err(CommonActionRuntimeError::IncompleteBattlefieldEvidence);
    }
    let controlled_creatures = state
        .permanents
        .values()
        .filter(|permanent| permanent.controller == pending.controller && permanent.is_creature())
        .map(|permanent| permanent.object)
        .collect::<BTreeSet<_>>();
    match input.chosen_bearer {
        Some(chosen) if !controlled_creatures.contains(&chosen) => {
            return Err(CommonActionRuntimeError::IllegalRingBearer(chosen));
        }
        None if !controlled_creatures.is_empty() => {
            return Err(CommonActionRuntimeError::MissingRingBearerChoice);
        }
        _ => {}
    }
    let player = state.player_mut(pending.controller)?;
    let old_count = player.ring_temptation_count;
    let temptation_count = old_count
        .checked_add(1)
        .ok_or(CommonActionRuntimeError::RingTemptationOverflow)?;
    let old_bearer = player.ring_bearer;
    player.ring_temptation_count = temptation_count;
    player.ring_bearer = input.chosen_bearer;
    Ok(RingTemptationReceipt {
        player: pending.controller,
        temptation_count,
        old_bearer,
        new_bearer: input.chosen_bearer,
        emblem_created: old_count == 0,
    })
}

pub fn enforce_ring_bearer_control_boundary(state: &mut CommonProcedureState) {
    let players = state.players.keys().copied().collect::<Vec<_>>();
    for player in players {
        let bearer = state
            .players
            .get(&player)
            .and_then(|state| state.ring_bearer);
        if bearer.is_some_and(|object| {
            state
                .permanents
                .get(&object)
                .is_none_or(|permanent| permanent.controller != player)
        }) {
            state
                .players
                .get_mut(&player)
                .expect("known player")
                .ring_bearer = None;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignationReceipt {
    pub player: PlayerId,
    pub previous_holder: Option<PlayerId>,
    pub current_holder: PlayerId,
}

pub fn resolve_become_monarch(
    pending: &PendingCommonAction,
    state: &mut CommonProcedureState,
) -> Result<DesignationReceipt, CommonActionRuntimeError> {
    if pending.kind != CommonActionKind::BecomeMonarch {
        return Err(CommonActionRuntimeError::WrongActionFamily);
    }
    state.player(pending.controller)?;
    let previous_holder = state.monarch.replace(pending.controller);
    Ok(DesignationReceipt {
        player: pending.controller,
        previous_holder,
        current_holder: pending.controller,
    })
}

pub fn resolve_take_initiative(
    pending: &PendingCommonAction,
    state: &mut CommonProcedureState,
) -> Result<DesignationReceipt, CommonActionRuntimeError> {
    if pending.kind != CommonActionKind::TakeInitiative {
        return Err(CommonActionRuntimeError::WrongActionFamily);
    }
    state.player(pending.controller)?;
    let previous_holder = state.initiative.replace(pending.controller);
    state
        .pending_undercity_ventures
        .push(PendingUndercityVenture {
            player: pending.controller,
            caused_by_trigger: true,
        });
    Ok(DesignationReceipt {
        player: pending.controller,
        previous_holder,
        current_holder: pending.controller,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DungeonVentureKind {
    Ordinary,
    Undercity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonVentureChoice {
    pub dungeon: Option<DungeonId>,
    pub room: Option<RoomId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DungeonVentureReceipt {
    pub player: PlayerId,
    pub completed: Option<DungeonId>,
    pub entered_dungeon: Option<DungeonId>,
    pub entered_room: RoomId,
}

pub fn resolve_venture_into_dungeon(
    pending: &PendingCommonAction,
    choice: DungeonVentureChoice,
    state: &mut CommonProcedureState,
) -> Result<DungeonVentureReceipt, CommonActionRuntimeError> {
    if pending.kind != CommonActionKind::VentureIntoDungeon {
        return Err(CommonActionRuntimeError::WrongActionFamily);
    }
    resolve_venture(
        pending.controller,
        DungeonVentureKind::Ordinary,
        choice,
        state,
    )
}

pub fn resolve_pending_undercity_venture(
    choice: DungeonVentureChoice,
    state: &mut CommonProcedureState,
) -> Result<DungeonVentureReceipt, CommonActionRuntimeError> {
    let pending = state
        .pending_undercity_ventures
        .first()
        .copied()
        .ok_or(CommonActionRuntimeError::MissingDungeonChoice)?;
    let receipt = resolve_venture(pending.player, DungeonVentureKind::Undercity, choice, state)?;
    state.pending_undercity_ventures.remove(0);
    Ok(receipt)
}

fn resolve_venture(
    player: PlayerId,
    kind: DungeonVentureKind,
    choice: DungeonVentureChoice,
    state: &mut CommonProcedureState,
) -> Result<DungeonVentureReceipt, CommonActionRuntimeError> {
    state.player(player)?;
    if !state.dungeon_catalog_complete || !state.outside_game_complete {
        return Err(CommonActionRuntimeError::IncompleteDungeonCatalog);
    }
    for dungeon in state.dungeons.values() {
        validate_dungeon(dungeon)?;
    }

    let current = state.dungeon_progress.get(&player).copied();
    let mut completed = None;
    if let Some(progress) = current {
        let dungeon = state.dungeons.get(&progress.dungeon_id).ok_or(
            CommonActionRuntimeError::IllegalDungeonChoice(progress.dungeon_id),
        )?;
        if dungeon.bottom_rooms.contains(&progress.room_id) {
            completed = Some(progress.dungeon_id);
            state.dungeon_progress.remove(&player);
            let counter = state.completed_dungeons.entry(player).or_default();
            *counter = counter
                .checked_add(1)
                .ok_or(CommonActionRuntimeError::DungeonCompletionOverflow)?;
        } else {
            if choice.dungeon.is_some() {
                return Err(CommonActionRuntimeError::IllegalDungeonChoice(
                    choice.dungeon.expect("checked some"),
                ));
            }
            let room = choice
                .room
                .ok_or(CommonActionRuntimeError::MissingRoomChoice)?;
            if !dungeon
                .outward_arrows
                .get(&progress.room_id)
                .is_some_and(|rooms| rooms.contains(&room))
            {
                return Err(CommonActionRuntimeError::IllegalRoomChoice(room));
            }
            state.dungeon_progress.insert(
                player,
                DungeonProgress {
                    dungeon_id: progress.dungeon_id,
                    room_id: room,
                },
            );
            return Ok(DungeonVentureReceipt {
                player,
                completed: None,
                entered_dungeon: None,
                entered_room: room,
            });
        }
    }

    let dungeon_id = choice
        .dungeon
        .ok_or(CommonActionRuntimeError::MissingDungeonChoice)?;
    let dungeon = state
        .dungeons
        .get(&dungeon_id)
        .ok_or(CommonActionRuntimeError::IllegalDungeonChoice(dungeon_id))?;
    if dungeon.owner != player
        || match kind {
            DungeonVentureKind::Ordinary => dungeon.is_undercity,
            DungeonVentureKind::Undercity => !dungeon.is_undercity,
        }
        || choice.room.is_some_and(|room| room != dungeon.top_room)
    {
        return Err(CommonActionRuntimeError::IllegalDungeonChoice(dungeon_id));
    }
    let top_room = dungeon.top_room;
    state.dungeon_progress.insert(
        player,
        DungeonProgress {
            dungeon_id,
            room_id: top_room,
        },
    );
    Ok(DungeonVentureReceipt {
        player,
        completed,
        entered_dungeon: Some(dungeon_id),
        entered_room: top_room,
    })
}

fn validate_dungeon(dungeon: &DungeonDefinition) -> Result<(), CommonActionRuntimeError> {
    if !dungeon.graph_complete
        || dungeon.bottom_rooms.is_empty()
        || dungeon.bottom_rooms.contains(&dungeon.top_room)
        || dungeon.bottom_rooms.iter().any(|room| {
            dungeon
                .outward_arrows
                .get(room)
                .is_some_and(|next| !next.is_empty())
        })
    {
        return Err(CommonActionRuntimeError::MalformedDungeonGraph(
            dungeon.dungeon_id,
        ));
    }
    let mut visited = BTreeSet::new();
    let mut frontier = vec![dungeon.top_room];
    while let Some(room) = frontier.pop() {
        if !visited.insert(room) {
            continue;
        }
        if let Some(next) = dungeon.outward_arrows.get(&room) {
            frontier.extend(next.iter().copied());
        }
    }
    if !dungeon
        .bottom_rooms
        .iter()
        .all(|room| visited.contains(room))
    {
        return Err(CommonActionRuntimeError::MalformedDungeonGraph(
            dungeon.dungeon_id,
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CounterBearingChoice {
    Permanent(ObjectRef),
    Player(PlayerId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProliferateReceipt {
    pub chosen: Vec<CounterBearingChoice>,
    pub additions: Vec<(CounterBearingChoice, String)>,
}

pub fn resolve_proliferate(
    pending: &PendingCommonAction,
    chosen: Vec<CounterBearingChoice>,
    state: &mut CommonProcedureState,
) -> Result<ProliferateReceipt, CommonActionRuntimeError> {
    if pending.kind != CommonActionKind::Proliferate {
        return Err(CommonActionRuntimeError::WrongActionFamily);
    }
    if !state.battlefield_complete
        || !state.counter_state_complete
        || !state.public_players_complete
    {
        return Err(CommonActionRuntimeError::IncompleteCounterEvidence);
    }
    if state.team_assignments.is_some() {
        return Err(CommonActionRuntimeError::UnsupportedTeamProliferation);
    }
    let mut distinct = BTreeSet::new();
    let mut additions = Vec::new();
    for selection in &chosen {
        let key = match selection {
            CounterBearingChoice::Permanent(object) => format!("o:{object:?}"),
            CounterBearingChoice::Player(player) => format!("p:{player}"),
        };
        if !distinct.insert(key) {
            return Err(CommonActionRuntimeError::IllegalProliferateChoice);
        }
        let counters = match selection {
            CounterBearingChoice::Permanent(object) => state
                .permanents
                .get(object)
                .ok_or(CommonActionRuntimeError::MissingPermanent(*object))?
                .counters
                .clone(),
            CounterBearingChoice::Player(player) => state.player(*player)?.counters.clone(),
        };
        if counters.is_empty() {
            return Err(CommonActionRuntimeError::IllegalProliferateChoice);
        }
        additions.extend(
            counters
                .keys()
                .cloned()
                .map(|kind| (selection.clone(), kind)),
        );
    }
    for (selection, kind) in &additions {
        match selection {
            CounterBearingChoice::Permanent(object) => {
                add_counter_to_permanent(state, *object, kind, 1)?;
            }
            CounterBearingChoice::Player(player) => {
                let counter = state
                    .player_mut(*player)?
                    .counters
                    .get_mut(kind)
                    .expect("counter kind came from complete snapshot");
                *counter = counter
                    .checked_add(1)
                    .ok_or(CommonActionRuntimeError::CounterOverflow)?;
            }
        }
    }
    Ok(ProliferateReceipt { chosen, additions })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmassResolutionInput {
    pub chosen_army: Option<ObjectRef>,
    pub dynamic_amount: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmassReceipt {
    pub player: PlayerId,
    pub army: ObjectRef,
    pub created: bool,
    pub subtype: ArmySubtype,
    pub counters_added: u32,
}

pub fn resolve_amass(
    pending: &PendingCommonAction,
    input: AmassResolutionInput,
    state: &mut CommonProcedureState,
) -> Result<AmassReceipt, CommonActionRuntimeError> {
    let CommonActionKind::Amass { subtype, amount } = pending.kind else {
        return Err(CommonActionRuntimeError::WrongActionFamily);
    };
    if !state.battlefield_complete {
        return Err(CommonActionRuntimeError::IncompleteBattlefieldEvidence);
    }
    let amount = resolved_amount(amount, input.dynamic_amount)?;
    let mut armies = state
        .permanents
        .values()
        .filter(|permanent| {
            permanent.controller == pending.controller
                && permanent.is_creature()
                && permanent.subtypes.contains("Army")
        })
        .map(|permanent| permanent.object)
        .collect::<BTreeSet<_>>();
    let created = armies.is_empty();
    if created {
        let object = allocate_object(state)?;
        let subtype_text = match subtype {
            ArmySubtype::Orc => "Orc",
            ArmySubtype::Zombie => "Zombie",
        };
        state.permanents.insert(
            object,
            PermanentRecord {
                object,
                card_id: None,
                owner: pending.controller,
                controller: pending.controller,
                card_types: BTreeSet::from([CardType::Creature]),
                subtypes: BTreeSet::from(["Army".into(), subtype_text.into()]),
                counters: BTreeMap::new(),
                is_token: true,
            },
        );
        armies.insert(object);
    }
    let army = match (input.chosen_army, armies.len()) {
        (Some(chosen), _) if armies.contains(&chosen) => chosen,
        (None, 1) => *armies.iter().next().expect("one army"),
        (None, _) => return Err(CommonActionRuntimeError::MissingArmyChoice),
        (Some(chosen), _) => return Err(CommonActionRuntimeError::IllegalArmyChoice(chosen)),
    };
    let subtype_text = match subtype {
        ArmySubtype::Orc => "Orc",
        ArmySubtype::Zombie => "Zombie",
    };
    state
        .permanents
        .get_mut(&army)
        .expect("chosen Army remains present")
        .subtypes
        .insert(subtype_text.into());
    add_counter_to_permanent(state, army, "+1/+1", amount)?;
    Ok(AmassReceipt {
        player: pending.controller,
        army,
        created,
        subtype,
        counters_added: amount,
    })
}

fn allocate_object(
    state: &mut CommonProcedureState,
) -> Result<ObjectRef, CommonActionRuntimeError> {
    let object = ObjectRef {
        object_id: state.next_object_id,
        incarnation_id: 1,
    };
    state.next_object_id = state
        .next_object_id
        .checked_add(1)
        .ok_or(CommonActionRuntimeError::ObjectIdOverflow)?;
    Ok(object)
}

fn add_counter_to_permanent(
    state: &mut CommonProcedureState,
    target: ObjectRef,
    kind: &str,
    amount: u32,
) -> Result<(), CommonActionRuntimeError> {
    let counter = state
        .permanents
        .get_mut(&target)
        .ok_or(CommonActionRuntimeError::MissingPermanent(target))?
        .counters
        .entry(kind.to_owned())
        .or_default();
    *counter = counter
        .checked_add(amount)
        .ok_or(CommonActionRuntimeError::CounterOverflow)?;
    Ok(())
}
