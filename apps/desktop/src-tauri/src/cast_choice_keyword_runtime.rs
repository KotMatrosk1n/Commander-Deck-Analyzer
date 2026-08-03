//! Exact, content keyed cast cost and cast choice programs.
//!
//! This module owns only reviewed standalone Oracle clauses that do not have
//! an earlier exact owner. Multikicker remains with the official Kicker
//! runtime, and Ravenous remains with the entry choice runtime. Grants,
//! compounds, reminderless forms, named text, and clauses whose required face
//! context is incomplete remain rejected.
//!
//! Program identity uses exact Oracle content, relevant derived type or face
//! context, and versioned rules contracts. It never uses card names, card
//! identifiers, database rows, addresses, snapshot metadata, or memory
//! location. The transaction runtime is not connected to production.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const CAST_CHOICE_KEYWORD_COMPILER_VERSION: &str = "cast-choice-keyword-compiler-0.1";
pub const CAST_CHOICE_KEYWORD_RUNTIME_VERSION: &str = "cast-choice-keyword-runtime-0.1";
pub const CAST_CHOICE_KEYWORD_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:107.3,111.12,115,116,118.7-9,122.1-2,207.2c,\
     305.2,400.7,601.2a-h,603.3d,603.6,603.11,607,608.2b,612,701.9,701.18,701.20,\
     701.21,701.28,702.33,702.48,702.94,702.113,702.119,702.120,702.132,702.148,\
     702.153,702.157,702.160,702.162,702.173,702.175,702.176,702.187,702.188,707,718";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type CastId = u64;
pub type AbilityInstanceId = u64;
pub type TriggerId = u64;
pub type EventId = u64;
pub type ManaUnitId = u64;
pub type TurnId = u64;

pub const fn cast_choice_keyword_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CastChoiceKeywordFamily {
    Multikicker,
    Casualty,
    Cleave,
    Emerge,
    Escalate,
    Offering,
    Prototype,
    Squad,
    Assist,
    Awaken,
    Strive,
    Freerunning,
    Impending,
    MoreThanMeetsTheEye,
    Offspring,
    WebSlinging,
    Mayhem,
    Miracle,
    Ravenous,
}

impl CastChoiceKeywordFamily {
    pub const fn printed_label(self) -> &'static str {
        match self {
            Self::Multikicker => "Multikicker",
            Self::Casualty => "Casualty",
            Self::Cleave => "Cleave",
            Self::Emerge => "Emerge",
            Self::Escalate => "Escalate",
            Self::Offering => "Offering",
            Self::Prototype => "Prototype",
            Self::Squad => "Squad",
            Self::Assist => "Assist",
            Self::Awaken => "Awaken",
            Self::Strive => "Strive",
            Self::Freerunning => "Freerunning",
            Self::Impending => "Impending",
            Self::MoreThanMeetsTheEye => "More Than Meets the Eye",
            Self::Offspring => "Offspring",
            Self::WebSlinging => "Web-slinging",
            Self::Mayhem => "Mayhem",
            Self::Miracle => "Miracle",
            Self::Ravenous => "Ravenous",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EarlierCastChoiceClauseOwner {
    OfficialKeywordKickerRuntime,
    EntryChoiceKeywordRuntime,
    OfficialKeywordSpreeRuntime,
    OfficialKeywordBargainRuntime,
    AlternateZoneCastKeywordRuntime,
    CastModifierKeywordRuntime,
    LinkedCastCostKeywordRuntime,
    ExtendedCastZoneKeywordRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
}

impl ManaColor {
    fn code(self) -> &'static str {
        match self {
            Self::White => "W",
            Self::Blue => "U",
            Self::Black => "B",
            Self::Red => "R",
            Self::Green => "G",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManaSymbol {
    Generic(u32),
    Colored(ManaColor),
    Colorless,
    Snow,
    VariableX,
    Hybrid(ManaColor, ManaColor),
    Phyrexian(ManaColor),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ManaCost {
    pub symbols: Vec<ManaSymbol>,
}

impl ManaCost {
    pub fn oracle_text(&self) -> String {
        let mut output = String::new();
        for symbol in &self.symbols {
            match symbol {
                ManaSymbol::Generic(amount) => output.push_str(&format!("{{{amount}}}")),
                ManaSymbol::Colored(color) => {
                    output.push('{');
                    output.push_str(color.code());
                    output.push('}');
                }
                ManaSymbol::Colorless => output.push_str("{C}"),
                ManaSymbol::Snow => output.push_str("{S}"),
                ManaSymbol::VariableX => output.push_str("{X}"),
                ManaSymbol::Hybrid(first, second) => {
                    output.push('{');
                    output.push_str(first.code());
                    output.push('/');
                    output.push_str(second.code());
                    output.push('}');
                }
                ManaSymbol::Phyrexian(color) => {
                    output.push('{');
                    output.push_str(color.code());
                    output.push_str("/P}");
                }
            }
        }
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonManaCost {
    DiscardCards(u32),
    TapUntappedCreatureYouControl,
    ExileCardsFromYourGraveyard(u32),
    ReturnBlueCreatureYouControlToOwnersHand,
    ReturnTappedCreatureYouControlToOwnersHand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostExpression {
    pub mana: Option<ManaCost>,
    pub nonmana: Vec<NonManaCost>,
}

impl CostExpression {
    fn mana(cost: ManaCost) -> Self {
        Self {
            mana: Some(cost),
            nonmana: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SacrificeQuality {
    Creature,
    Artifact,
    Subtype(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquadEntryKind {
    Creature,
    Enchantment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastChoiceKeywordKind {
    Casualty {
        minimum_power: u32,
        reminder_mentions_new_targets: bool,
    },
    Cleave {
        alternative_cost: ManaCost,
        removed_fragments: Vec<String>,
    },
    Emerge {
        alternative_cost: ManaCost,
        sacrifice_quality: SacrificeQuality,
    },
    Escalate {
        additional_cost_per_extra_mode: CostExpression,
        available_modes: u32,
    },
    Offering {
        sacrifice_quality: SacrificeQuality,
    },
    Prototype {
        alternative_cost: ManaCost,
        power: i32,
        toughness: i32,
    },
    Squad {
        repeatable_additional_cost: CostExpression,
        entry_kind: SquadEntryKind,
    },
    Assist,
    Awaken {
        counters: u32,
        alternative_cost: ManaCost,
    },
    Strive {
        additional_cost_per_extra_target: ManaCost,
    },
    Freerunning {
        alternative_cost: CostExpression,
    },
    Impending {
        time_counters: u32,
        alternative_cost: ManaCost,
    },
    MoreThanMeetsTheEye {
        alternative_cost: ManaCost,
    },
    Offspring {
        additional_cost: ManaCost,
    },
    WebSlinging {
        alternative_cost: ManaCost,
    },
    Mayhem {
        alternative_cost: Option<ManaCost>,
        permits_land_play: bool,
    },
    Miracle {
        alternative_cost: ManaCost,
    },
}

impl CastChoiceKeywordKind {
    pub const fn family(&self) -> CastChoiceKeywordFamily {
        match self {
            Self::Casualty { .. } => CastChoiceKeywordFamily::Casualty,
            Self::Cleave { .. } => CastChoiceKeywordFamily::Cleave,
            Self::Emerge { .. } => CastChoiceKeywordFamily::Emerge,
            Self::Escalate { .. } => CastChoiceKeywordFamily::Escalate,
            Self::Offering { .. } => CastChoiceKeywordFamily::Offering,
            Self::Prototype { .. } => CastChoiceKeywordFamily::Prototype,
            Self::Squad { .. } => CastChoiceKeywordFamily::Squad,
            Self::Assist => CastChoiceKeywordFamily::Assist,
            Self::Awaken { .. } => CastChoiceKeywordFamily::Awaken,
            Self::Strive { .. } => CastChoiceKeywordFamily::Strive,
            Self::Freerunning { .. } => CastChoiceKeywordFamily::Freerunning,
            Self::Impending { .. } => CastChoiceKeywordFamily::Impending,
            Self::MoreThanMeetsTheEye { .. } => CastChoiceKeywordFamily::MoreThanMeetsTheEye,
            Self::Offspring { .. } => CastChoiceKeywordFamily::Offspring,
            Self::WebSlinging { .. } => CastChoiceKeywordFamily::WebSlinging,
            Self::Mayhem { .. } => CastChoiceKeywordFamily::Mayhem,
            Self::Miracle { .. } => CastChoiceKeywordFamily::Miracle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastChoiceSourceContext {
    pub source_type_line: String,
    pub layout: String,
    pub face_index: u16,
    pub face_count: usize,
    /// Complete Oracle text for this face. Required only when semantics depend
    /// on text outside the keyword clause.
    pub complete_face_oracle_text: Option<String>,
    /// Complete count of printed modal choices, excluding reminder text.
    pub modal_mode_count: Option<u32>,
    /// Complete evidence about targets in the spell text outside the keyword
    /// reminder.
    pub spell_has_targets: Option<bool>,
    /// Complete evidence that the spell permits a variable number of targets.
    pub variable_target_count: Option<bool>,
}

pub fn derive_cast_choice_source_context(
    exact_source: &str,
    source_type_line: &str,
    layout: &str,
    face_index: u16,
    face_count: usize,
    clause_index: u16,
    complete_face_oracle_text: &str,
) -> Option<CastChoiceSourceContext> {
    let clauses = complete_face_oracle_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && *line != "//")
        .collect::<Vec<_>>();
    if clauses.get(usize::from(clause_index)).copied() != Some(exact_source) {
        return None;
    }
    let other_text = clauses
        .iter()
        .enumerate()
        .filter_map(|(index, clause)| (index != usize::from(clause_index)).then_some(*clause))
        .collect::<Vec<_>>()
        .join("\n");
    let other_lower = other_text.to_ascii_lowercase();
    Some(CastChoiceSourceContext {
        source_type_line: source_type_line.to_owned(),
        layout: layout.to_owned(),
        face_index,
        face_count,
        complete_face_oracle_text: Some(complete_face_oracle_text.to_owned()),
        modal_mode_count: Some(
            clauses
                .iter()
                .filter(|line| line.starts_with('\u{2022}'))
                .count()
                .try_into()
                .ok()?,
        ),
        spell_has_targets: Some(other_lower.contains("target")),
        variable_target_count: Some(
            other_lower.contains("any number of target")
                || (other_lower.contains("up to ") && other_lower.contains(" target")),
        ),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastChoiceKeywordProgram {
    exact_source: String,
    semantic_context: String,
    semantic_digest: String,
    kind: CastChoiceKeywordKind,
}

impl CastChoiceKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn semantic_context(&self) -> &str {
        &self.semantic_context
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &CastChoiceKeywordKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        cast_choice_keyword_production_adapter_connected()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastChoiceClauseClassification {
    Program(CastChoiceKeywordProgram),
    EarlierOwner {
        family: CastChoiceKeywordFamily,
        owner: EarlierCastChoiceClauseOwner,
    },
    Rejected,
}

pub fn compile_cast_choice_keyword_program(
    exact_source: &str,
    context: &CastChoiceSourceContext,
) -> Option<CastChoiceKeywordProgram> {
    match classify_cast_choice_keyword_clause(exact_source, context) {
        CastChoiceClauseClassification::Program(program) => Some(program),
        CastChoiceClauseClassification::EarlierOwner { .. }
        | CastChoiceClauseClassification::Rejected => None,
    }
}

pub fn classify_cast_choice_keyword_clause(
    exact_source: &str,
    context: &CastChoiceSourceContext,
) -> CastChoiceClauseClassification {
    if !is_complete_single_line(exact_source) {
        return CastChoiceClauseClassification::Rejected;
    }
    if parse_prior_multikicker(exact_source).is_some() {
        return CastChoiceClauseClassification::EarlierOwner {
            family: CastChoiceKeywordFamily::Multikicker,
            owner: EarlierCastChoiceClauseOwner::OfficialKeywordKickerRuntime,
        };
    }
    if exact_source
        == "Ravenous (This creature enters with X +1/+1 counters on it. If X is 5 or more, draw a \
           card when it enters.)"
    {
        return CastChoiceClauseClassification::EarlierOwner {
            family: CastChoiceKeywordFamily::Ravenous,
            owner: EarlierCastChoiceClauseOwner::EntryChoiceKeywordRuntime,
        };
    }

    let Some(kind) = parse_unowned_kind(exact_source, context) else {
        return CastChoiceClauseClassification::Rejected;
    };
    let Some(semantic_context) = reviewed_semantic_context(&kind, context) else {
        return CastChoiceClauseClassification::Rejected;
    };
    let semantic_digest = cast_choice_semantic_digest(exact_source, &semantic_context, &kind);
    CastChoiceClauseClassification::Program(CastChoiceKeywordProgram {
        exact_source: exact_source.to_owned(),
        semantic_context,
        semantic_digest,
        kind,
    })
}

fn parse_unowned_kind(
    exact_source: &str,
    context: &CastChoiceSourceContext,
) -> Option<CastChoiceKeywordKind> {
    parse_casualty(exact_source, context)
        .or_else(|| parse_cleave(exact_source, context))
        .or_else(|| parse_emerge(exact_source))
        .or_else(|| parse_escalate(exact_source, context))
        .or_else(|| parse_offering(exact_source))
        .or_else(|| parse_prototype(exact_source))
        .or_else(|| parse_squad(exact_source))
        .or_else(|| parse_assist(exact_source))
        .or_else(|| parse_awaken(exact_source))
        .or_else(|| parse_strive(exact_source, context))
        .or_else(|| parse_freerunning(exact_source))
        .or_else(|| parse_impending(exact_source))
        .or_else(|| parse_more_than_meets_the_eye(exact_source))
        .or_else(|| parse_offspring(exact_source))
        .or_else(|| parse_web_slinging(exact_source))
        .or_else(|| parse_mayhem(exact_source))
        .or_else(|| parse_miracle(exact_source))
}

fn parse_prior_multikicker(source: &str) -> Option<ManaCost> {
    let (core, reminder) = split_optional_reminder(source)?;
    let cost_text = core.strip_prefix("Multikicker ")?;
    let cost = parse_mana_cost(cost_text)?;
    if let Some(reminder) = reminder
        && reminder
            != format!(
                "You may pay an additional {} any number of times as you cast this spell.",
                cost.oracle_text()
            )
    {
        return None;
    }
    Some(cost)
}

fn parse_casualty(
    source: &str,
    context: &CastChoiceSourceContext,
) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let minimum_power = parse_positive_u32(core.strip_prefix("Casualty ")?)?;
    let prefix = format!(
        "As you cast this spell, you may sacrifice a creature with power {minimum_power} or \
         greater. When you do, copy this spell"
    );
    let reminder_mentions_new_targets = if reminder == format!("{prefix}.") {
        false
    } else if reminder == format!("{prefix} and you may choose a new target for the copy.") {
        true
    } else {
        return None;
    };
    if context.spell_has_targets? != reminder_mentions_new_targets {
        return None;
    }
    Some(CastChoiceKeywordKind::Casualty {
        minimum_power,
        reminder_mentions_new_targets,
    })
}

fn parse_cleave(source: &str, context: &CastChoiceSourceContext) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let alternative_cost = parse_mana_cost(core.strip_prefix("Cleave ")?)?;
    if reminder
        != "You may cast this spell for its cleave cost. If you do, remove the words in square \
            brackets."
    {
        return None;
    }
    let face_oracle = context.complete_face_oracle_text.as_deref()?;
    let removed_fragments = square_bracket_fragments(face_oracle, source)?;
    if removed_fragments.is_empty() {
        return None;
    }
    Some(CastChoiceKeywordKind::Cleave {
        alternative_cost,
        removed_fragments,
    })
}

fn parse_emerge(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let (quality, cost_text) = if let Some(rest) = core.strip_prefix("Emerge from artifact ") {
        (SacrificeQuality::Artifact, rest)
    } else {
        (SacrificeQuality::Creature, core.strip_prefix("Emerge ")?)
    };
    let alternative_cost = parse_mana_cost(cost_text)?;
    let expected = match quality {
        SacrificeQuality::Artifact => {
            "You may cast this spell by sacrificing an artifact and paying the emerge cost \
             reduced by that artifact's mana value."
                .to_string()
        }
        SacrificeQuality::Creature => {
            "You may cast this spell by sacrificing a creature and paying the emerge cost reduced \
             by that creature's mana value."
                .to_string()
        }
        SacrificeQuality::Subtype(_) => return None,
    };
    (reminder == expected).then_some(CastChoiceKeywordKind::Emerge {
        alternative_cost,
        sacrifice_quality: quality,
    })
}

fn parse_escalate(
    source: &str,
    context: &CastChoiceSourceContext,
) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    if reminder != "Pay this cost for each mode chosen beyond the first." {
        return None;
    }
    let cost_text = core
        .strip_prefix("Escalate ")
        .or_else(|| core.strip_prefix("Escalate\u{2014}"))?;
    let additional_cost_per_extra_mode = parse_cost_expression(cost_text)?;
    let available_modes = context.modal_mode_count.filter(|count| *count >= 2)?;
    Some(CastChoiceKeywordKind::Escalate {
        additional_cost_per_extra_mode,
        available_modes,
    })
}

fn parse_offering(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let quality_text = core.strip_suffix(" offering")?;
    if quality_text.is_empty() || quality_text.split_whitespace().count() != 1 {
        return None;
    }
    let sacrifice_quality = if quality_text == "Artifact" {
        SacrificeQuality::Artifact
    } else {
        SacrificeQuality::Subtype(quality_text.to_owned())
    };
    let subject = if quality_text == "Artifact" {
        "an artifact".to_owned()
    } else {
        format!("a {quality_text}")
    };
    let timing = if quality_text == "Artifact" {
        "as though it had flash"
    } else {
        "any time you could cast an instant"
    };
    let sacrificed_noun = if quality_text == "Artifact" {
        "artifact"
    } else {
        quality_text
    };
    let expected = format!(
        "You may cast this spell {timing} by sacrificing {subject} and paying the difference in \
         mana costs between this and the sacrificed {sacrificed_noun}. Mana cost includes \
         color.",
    );
    (reminder == expected).then_some(CastChoiceKeywordKind::Offering { sacrifice_quality })
}

fn parse_prototype(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    if reminder
        != "You may cast this spell with different mana cost, color, and size. It keeps its \
            abilities and types."
    {
        return None;
    }
    let rest = core.strip_prefix("Prototype ")?;
    let (cost_text, dimensions) = rest.split_once(" \u{2014} ")?;
    let (power, toughness) = parse_dimensions(dimensions)?;
    Some(CastChoiceKeywordKind::Prototype {
        alternative_cost: parse_mana_cost(cost_text)?,
        power,
        toughness,
    })
}

fn parse_squad(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let cost_text = core
        .strip_prefix("Squad ")
        .or_else(|| core.strip_prefix("Squad\u{2014}"))?;
    let (entry_kind, noun) = if reminder.contains("When this creature enters") {
        (SquadEntryKind::Creature, "creature")
    } else if reminder.contains("When this enchantment enters") {
        (SquadEntryKind::Enchantment, "enchantment")
    } else {
        return None;
    };
    let expected = format!(
        "As an additional cost to cast this spell, you may pay {} any number of times. When this \
         {noun} enters, create that many tokens that are copies of it.",
        if core.starts_with("Squad\u{2014}") {
            "its squad cost"
        } else {
            cost_text
        }
    );
    if reminder != expected {
        return None;
    }
    Some(CastChoiceKeywordKind::Squad {
        repeatable_additional_cost: parse_cost_expression(cost_text)?,
        entry_kind,
    })
}

fn parse_assist(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    if core != "Assist" || !reminder.starts_with("Another player can pay up to ") {
        return None;
    }
    let amount = reminder.strip_prefix("Another player can pay up to ")?;
    let amount = amount
        .strip_suffix(" of this spell's cost.")
        .or_else(|| amount.strip_suffix(" of this spell's cost. You choose the value of X."))?;
    if amount != "{X}" && parse_mana_cost(amount).is_none() {
        return None;
    }
    if reminder.ends_with(" You choose the value of X.") && amount != "{X}" {
        return None;
    }
    Some(CastChoiceKeywordKind::Assist)
}

fn parse_awaken(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let rest = core.strip_prefix("Awaken ")?;
    let (counters_text, cost_text) = rest.split_once('\u{2014}')?;
    let counters = parse_positive_u32(counters_text)?;
    let cost = parse_mana_cost(cost_text)?;
    let counter_word = english_cardinal(counters)?;
    let expected = format!(
        "If you cast this spell for {}, also put {counter_word} +1/+1 counters on target land you \
         control and it becomes a 0/0 Elemental creature with haste. It's still a land.",
        cost.oracle_text()
    );
    (reminder == expected).then_some(CastChoiceKeywordKind::Awaken {
        counters,
        alternative_cost: cost,
    })
}

fn parse_strive(source: &str, context: &CastChoiceSourceContext) -> Option<CastChoiceKeywordKind> {
    let cost_text = source
        .strip_prefix("Strive \u{2014} This spell costs ")?
        .strip_suffix(" more to cast for each target beyond the first.")?;
    if !context.spell_has_targets? || !context.variable_target_count? {
        return None;
    }
    Some(CastChoiceKeywordKind::Strive {
        additional_cost_per_extra_target: parse_mana_cost(cost_text)?,
    })
}

fn parse_freerunning(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let cost_text = core
        .strip_prefix("Freerunning ")
        .or_else(|| core.strip_prefix("Freerunning\u{2014}"))?;
    if reminder
        != "You may cast this spell for its freerunning cost if you dealt combat damage to a \
            player this turn with an Assassin or commander."
    {
        return None;
    }
    Some(CastChoiceKeywordKind::Freerunning {
        alternative_cost: parse_cost_expression(cost_text)?,
    })
}

fn parse_impending(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let rest = core.strip_prefix("Impending ")?;
    let (counters_text, cost_text) = rest.split_once('\u{2014}')?;
    let time_counters = parse_positive_u32(counters_text)?;
    let alternative_cost = parse_mana_cost(cost_text)?;
    let counter_word = english_cardinal(time_counters)?;
    let expected = format!(
        "If you cast this spell for its impending cost, it enters with {counter_word} time counters \
         and isn't a creature until the last is removed. At the beginning of your end step, remove \
         a time counter from it."
    );
    (reminder == expected).then_some(CastChoiceKeywordKind::Impending {
        time_counters,
        alternative_cost,
    })
}

fn parse_more_than_meets_the_eye(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let alternative_cost = parse_mana_cost(core.strip_prefix("More Than Meets the Eye ")?)?;
    let expected = format!(
        "You may cast this card converted for {}.",
        alternative_cost.oracle_text()
    );
    (reminder == expected)
        .then_some(CastChoiceKeywordKind::MoreThanMeetsTheEye { alternative_cost })
}

fn parse_offspring(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let additional_cost = parse_mana_cost(core.strip_prefix("Offspring ")?)?;
    let expected = format!(
        "You may pay an additional {} as you cast this spell. If you do, when this creature \
         enters, create a 1/1 token copy of it.",
        additional_cost.oracle_text()
    );
    (reminder == expected).then_some(CastChoiceKeywordKind::Offspring { additional_cost })
}

fn parse_web_slinging(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let alternative_cost = parse_mana_cost(core.strip_prefix("Web-slinging ")?)?;
    let expected = format!(
        "You may cast this spell for {} if you also return a tapped creature you control to its \
         owner's hand.",
        alternative_cost.oracle_text()
    );
    (reminder == expected).then_some(CastChoiceKeywordKind::WebSlinging { alternative_cost })
}

fn parse_mayhem(source: &str) -> Option<CastChoiceKeywordKind> {
    if source
        == "Mayhem (You may play this card from your graveyard if you discarded it this turn. \
           Timing rules still apply.)"
    {
        return Some(CastChoiceKeywordKind::Mayhem {
            alternative_cost: None,
            permits_land_play: true,
        });
    }
    let (core, reminder) = split_required_reminder(source)?;
    let alternative_cost = parse_mana_cost(core.strip_prefix("Mayhem ")?)?;
    let expected = format!(
        "You may cast this card from your graveyard for {} if you discarded it this turn. Timing \
         rules still apply.",
        alternative_cost.oracle_text()
    );
    let expected_without_timing = format!(
        "You may cast this card from your graveyard for {} if you discarded it this turn.",
        alternative_cost.oracle_text()
    );
    (reminder == expected || reminder == expected_without_timing).then_some(
        CastChoiceKeywordKind::Mayhem {
            alternative_cost: Some(alternative_cost),
            permits_land_play: false,
        },
    )
}

fn parse_miracle(source: &str) -> Option<CastChoiceKeywordKind> {
    let (core, reminder) = split_required_reminder(source)?;
    let alternative_cost = parse_mana_cost(core.strip_prefix("Miracle ")?)?;
    (reminder
        == "You may cast this card for its miracle cost when you draw it if it's the first card you \
            drew this turn.")
    .then_some(CastChoiceKeywordKind::Miracle { alternative_cost })
}

fn reviewed_semantic_context(
    kind: &CastChoiceKeywordKind,
    context: &CastChoiceSourceContext,
) -> Option<String> {
    let type_words = type_words(&context.source_type_line);
    let has_type = |word: &str| type_words.contains(word);
    let is_land = has_type("land");
    let is_spell_card = [
        "artifact",
        "battle",
        "creature",
        "enchantment",
        "instant",
        "kindred",
        "planeswalker",
        "sorcery",
    ]
    .into_iter()
    .any(has_type)
        && !is_land;
    let is_instant_or_sorcery = has_type("instant") || has_type("sorcery");
    let is_permanent_spell = [
        "artifact",
        "battle",
        "creature",
        "enchantment",
        "planeswalker",
    ]
    .into_iter()
    .any(has_type);
    match kind {
        CastChoiceKeywordKind::Casualty {
            reminder_mentions_new_targets,
            ..
        } if is_spell_card => Some(format!(
            "source=spell-card;targets={}",
            reminder_mentions_new_targets
        )),
        CastChoiceKeywordKind::Cleave {
            removed_fragments, ..
        } if is_instant_or_sorcery => Some(format!(
            "source=instant-or-sorcery;brackets={}",
            encode_components(removed_fragments.iter().map(String::as_str))
        )),
        CastChoiceKeywordKind::Emerge { .. } if has_type("creature") => {
            Some("source=creature-spell".to_owned())
        }
        CastChoiceKeywordKind::Escalate {
            available_modes, ..
        } if is_instant_or_sorcery => Some(format!(
            "source=modal-instant-or-sorcery;modes={available_modes}"
        )),
        CastChoiceKeywordKind::Offering { .. } if has_type("creature") => {
            Some("source=creature-spell".to_owned())
        }
        CastChoiceKeywordKind::Prototype { .. }
            if has_type("creature") && context.layout == "prototype" =>
        {
            Some("source=prototype-layout-creature".to_owned())
        }
        CastChoiceKeywordKind::Squad { entry_kind, .. }
            if (matches!(entry_kind, SquadEntryKind::Creature) && has_type("creature"))
                || (matches!(entry_kind, SquadEntryKind::Enchantment)
                    && has_type("enchantment")) =>
        {
            Some(format!("source=permanent-spell;entry-kind={entry_kind:?}"))
        }
        CastChoiceKeywordKind::Assist if is_spell_card => Some("source=spell-card".to_owned()),
        CastChoiceKeywordKind::Awaken { .. } if is_instant_or_sorcery => {
            Some("source=instant-or-sorcery".to_owned())
        }
        CastChoiceKeywordKind::Strive { .. } if is_instant_or_sorcery => {
            Some("source=variable-target-instant-or-sorcery".to_owned())
        }
        CastChoiceKeywordKind::Freerunning { .. } if is_spell_card => {
            Some("source=spell-card".to_owned())
        }
        CastChoiceKeywordKind::Impending { .. } if is_permanent_spell && has_type("creature") => {
            Some("source=creature-permanent-spell".to_owned())
        }
        CastChoiceKeywordKind::MoreThanMeetsTheEye { .. }
            if is_permanent_spell
                && context.layout == "transform"
                && context.face_count == 2
                && context.face_index == 0 =>
        {
            Some("source=front-face-two-face-transform-permanent".to_owned())
        }
        CastChoiceKeywordKind::Offspring { .. } if has_type("creature") => {
            Some("source=creature-spell".to_owned())
        }
        CastChoiceKeywordKind::WebSlinging { .. } if is_spell_card => {
            Some("source=spell-card".to_owned())
        }
        CastChoiceKeywordKind::Mayhem {
            alternative_cost: None,
            permits_land_play: true,
        } if is_land => Some("source=land-card;action=play".to_owned()),
        CastChoiceKeywordKind::Mayhem {
            alternative_cost: Some(_),
            permits_land_play: false,
        } if is_spell_card => Some("source=spell-card;action=cast".to_owned()),
        CastChoiceKeywordKind::Miracle { .. } if is_spell_card => {
            Some("source=nonland-spell-card".to_owned())
        }
        _ => None,
    }
}

fn cast_choice_semantic_digest(
    exact_source: &str,
    semantic_context: &str,
    kind: &CastChoiceKeywordKind,
) -> String {
    let semantics = canonical_semantics(kind);
    let mut hasher = Sha256::new();
    for component in [
        "cast-choice-keyword-content/v1",
        CAST_CHOICE_KEYWORD_COMPILER_VERSION,
        CAST_CHOICE_KEYWORD_RUNTIME_VERSION,
        CAST_CHOICE_KEYWORD_RULES_CONTEXT_VERSION,
        exact_source,
        semantic_context,
        &semantics,
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn canonical_semantics(kind: &CastChoiceKeywordKind) -> String {
    match kind {
        CastChoiceKeywordKind::Casualty {
            minimum_power,
            reminder_mentions_new_targets,
        } => format!(
            "additional=optional-sacrifice-creature-power>={minimum_power};on-cast=copy;\
             retarget={reminder_mentions_new_targets};instances=separate"
        ),
        CastChoiceKeywordKind::Cleave {
            alternative_cost,
            removed_fragments,
        } => format!(
            "alternative={};text-change=remove-square-brackets:{}",
            alternative_cost.oracle_text(),
            encode_components(removed_fragments.iter().map(String::as_str))
        ),
        CastChoiceKeywordKind::Emerge {
            alternative_cost,
            sacrifice_quality,
        } => format!(
            "alternative={}+sacrifice:{sacrifice_quality:?};reduce-generic=mana-value",
            alternative_cost.oracle_text()
        ),
        CastChoiceKeywordKind::Escalate {
            additional_cost_per_extra_mode,
            available_modes,
        } => format!(
            "modes={available_modes};additional-per-mode-beyond-first={additional_cost_per_extra_mode:?}"
        ),
        CastChoiceKeywordKind::Offering { sacrifice_quality } => format!(
            "additional=optional-sacrifice:{sacrifice_quality:?};flash-if-paid;\
             reduction=sacrificed-mana-cost"
        ),
        CastChoiceKeywordKind::Prototype {
            alternative_cost,
            power,
            toughness,
        } => format!(
            "prototype=alternative-characteristics;cost={};pt={power}/{toughness};copy-retains=true",
            alternative_cost.oracle_text()
        ),
        CastChoiceKeywordKind::Squad {
            repeatable_additional_cost,
            entry_kind,
        } => format!(
            "additional-repeatable={repeatable_additional_cost:?};linked-entry={entry_kind:?};\
             token-copies=payments;instances=separate"
        ),
        CastChoiceKeywordKind::Assist => {
            "payment-rule=chosen-other-player-may-pay-generic-total".to_owned()
        }
        CastChoiceKeywordKind::Awaken {
            counters,
            alternative_cost,
        } => format!(
            "alternative={};conditional-target=land-you-control;resolution=counter:{counters}+\
             0/0-elemental-haste-land",
            alternative_cost.oracle_text()
        ),
        CastChoiceKeywordKind::Strive {
            additional_cost_per_extra_target,
        } => format!(
            "cost-increase={}*targets-beyond-first;targets-before-total-cost",
            additional_cost_per_extra_target.oracle_text()
        ),
        CastChoiceKeywordKind::Freerunning { alternative_cost } => format!(
            "alternative={alternative_cost:?};condition=combat-damage-by-controlled-assassin-or-commander"
        ),
        CastChoiceKeywordKind::Impending {
            time_counters,
            alternative_cost,
        } => format!(
            "alternative={};entry=time-counters:{time_counters};noncreature-while-time;\
             controller-end-step-remove-one",
            alternative_cost.oracle_text()
        ),
        CastChoiceKeywordKind::MoreThanMeetsTheEye { alternative_cost } => format!(
            "alternative={};cast-converted=true;copy-retains-face=true",
            alternative_cost.oracle_text()
        ),
        CastChoiceKeywordKind::Offspring { additional_cost } => format!(
            "additional={};linked-entry=one-1/1-token-copy;instances=separate",
            additional_cost.oracle_text()
        ),
        CastChoiceKeywordKind::WebSlinging { alternative_cost } => format!(
            "alternative={}+return-tapped-controlled-creature",
            alternative_cost.oracle_text()
        ),
        CastChoiceKeywordKind::Mayhem {
            alternative_cost,
            permits_land_play,
        } => format!(
            "graveyard-after-discard-this-turn;alternative={alternative_cost:?};\
             permits-land-play={permits_land_play};normal-timing=true"
        ),
        CastChoiceKeywordKind::Miracle { alternative_cost } => format!(
            "first-draw-reveal-linked-trigger;alternative={};cast-on-trigger-resolution",
            alternative_cost.oracle_text()
        ),
    }
}

fn type_words(type_line: &str) -> BTreeSet<String> {
    type_line
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn is_complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source.trim() == source
        && !source.contains(['\r', '\n'])
        && source.split_whitespace().collect::<Vec<_>>().join(" ") == source
}

fn split_optional_reminder(source: &str) -> Option<(&str, Option<&str>)> {
    if let Some((core, rest)) = source.split_once(" (") {
        let reminder = rest.strip_suffix(')')?;
        if core.is_empty()
            || reminder.is_empty()
            || reminder.contains('(')
            || reminder.contains(')')
        {
            return None;
        }
        Some((core, Some(reminder)))
    } else {
        Some((source, None))
    }
}

fn split_required_reminder(source: &str) -> Option<(&str, &str)> {
    let (core, reminder) = split_optional_reminder(source)?;
    Some((core, reminder?))
}

fn parse_positive_u32(source: &str) -> Option<u32> {
    if source.is_empty()
        || (source.len() > 1 && source.starts_with('0'))
        || !source.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    source.parse::<u32>().ok().filter(|value| *value > 0)
}

fn parse_dimensions(source: &str) -> Option<(i32, i32)> {
    let (power, toughness) = source.split_once('/')?;
    Some((power.parse().ok()?, toughness.parse().ok()?))
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    if source.is_empty() {
        return None;
    }
    let mut symbols = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let remainder = &source[cursor..];
        let close = remainder.find('}')?;
        if !remainder.starts_with('{') || close < 2 {
            return None;
        }
        let body = &remainder[1..close];
        let symbol = match body {
            "W" => ManaSymbol::Colored(ManaColor::White),
            "U" => ManaSymbol::Colored(ManaColor::Blue),
            "B" => ManaSymbol::Colored(ManaColor::Black),
            "R" => ManaSymbol::Colored(ManaColor::Red),
            "G" => ManaSymbol::Colored(ManaColor::Green),
            "C" => ManaSymbol::Colorless,
            "S" => ManaSymbol::Snow,
            "X" => ManaSymbol::VariableX,
            _ => {
                if let Ok(amount) = body.parse::<u32>() {
                    if amount.to_string() != body {
                        return None;
                    }
                    ManaSymbol::Generic(amount)
                } else if let Some(color) = body.strip_suffix("/P").and_then(parse_color) {
                    ManaSymbol::Phyrexian(color)
                } else if let Some((first, second)) = body.split_once('/') {
                    ManaSymbol::Hybrid(parse_color(first)?, parse_color(second)?)
                } else {
                    return None;
                }
            }
        };
        symbols.push(symbol);
        cursor += close + 1;
    }
    Some(ManaCost { symbols })
}

fn parse_color(source: &str) -> Option<ManaColor> {
    match source {
        "W" => Some(ManaColor::White),
        "U" => Some(ManaColor::Blue),
        "B" => Some(ManaColor::Black),
        "R" => Some(ManaColor::Red),
        "G" => Some(ManaColor::Green),
        _ => None,
    }
}

fn parse_cost_expression(source: &str) -> Option<CostExpression> {
    if let Some(cost) = parse_mana_cost(source) {
        return Some(CostExpression::mana(cost));
    }
    match source {
        "Discard a card." | "Discard a card" => Some(CostExpression {
            mana: None,
            nonmana: vec![NonManaCost::DiscardCards(1)],
        }),
        "Tap an untapped creature you control." | "Tap an untapped creature you control" => {
            Some(CostExpression {
                mana: None,
                nonmana: vec![NonManaCost::TapUntappedCreatureYouControl],
            })
        }
        "Exile four cards from your graveyard." | "Exile four cards from your graveyard" => {
            Some(CostExpression {
                mana: None,
                nonmana: vec![NonManaCost::ExileCardsFromYourGraveyard(4)],
            })
        }
        "Return a blue creature you control to its owner's hand."
        | "Return a blue creature you control to its owner's hand" => Some(CostExpression {
            mana: None,
            nonmana: vec![NonManaCost::ReturnBlueCreatureYouControlToOwnersHand],
        }),
        _ => {
            let (mana, rest) = source.split_once(", ")?;
            let mana = parse_mana_cost(mana)?;
            match rest {
                "Discard a card." | "Discard a card" => Some(CostExpression {
                    mana: Some(mana),
                    nonmana: vec![NonManaCost::DiscardCards(1)],
                }),
                _ => None,
            }
        }
    }
}

fn square_bracket_fragments(face_oracle: &str, keyword_clause: &str) -> Option<Vec<String>> {
    let other_text = face_oracle
        .lines()
        .filter(|line| line.trim() != keyword_clause)
        .collect::<Vec<_>>()
        .join("\n");
    let mut fragments = Vec::new();
    let mut cursor = 0;
    while let Some(open_offset) = other_text[cursor..].find('[') {
        let open = cursor + open_offset;
        let close_offset = other_text[open + 1..].find(']')?;
        let close = open + 1 + close_offset;
        let fragment = &other_text[open + 1..close];
        if fragment.is_empty() || fragment.contains(['[', ']']) {
            return None;
        }
        fragments.push(fragment.to_owned());
        cursor = close + 1;
    }
    if other_text[cursor..].contains(']') {
        return None;
    }
    Some(fragments)
}

fn english_cardinal(value: u32) -> Option<&'static str> {
    match value {
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
        _ => None,
    }
}

fn encode_components<'a>(components: impl IntoIterator<Item = &'a str>) -> String {
    components
        .into_iter()
        .map(|component| format!("{}:{component}", component.len()))
        .collect::<Vec<_>>()
        .join("|")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

impl ObjectRef {
    fn next_incarnation(self) -> Result<Self, CastChoiceRuntimeError> {
        Ok(Self {
            object_id: self.object_id,
            incarnation_id: self
                .incarnation_id
                .checked_add(1)
                .ok_or(CastChoiceRuntimeError::IncarnationOverflow(self.object_id))?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardZone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Stack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManaKind {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

impl From<ManaColor> for ManaKind {
    fn from(value: ManaColor) -> Self {
        match value {
            ManaColor::White => Self::White,
            ManaColor::Blue => Self::Blue,
            ManaColor::Black => Self::Black,
            ManaColor::Red => Self::Red,
            ManaColor::Green => Self::Green,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaUnit {
    pub unit_id: ManaUnitId,
    pub controller: PlayerId,
    pub kind: ManaKind,
    pub from_snow_source: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerCastState {
    pub in_game: bool,
    pub life: i32,
    pub mana_pool: BTreeMap<ManaUnitId, ManaUnit>,
    pub hand: Vec<ObjectRef>,
    pub graveyard: Vec<ObjectRef>,
    pub exile: Vec<ObjectRef>,
    pub land_plays_remaining: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermanentCastState {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub is_creature: bool,
    pub is_artifact: bool,
    pub is_land: bool,
    pub is_blue: bool,
    pub subtypes: BTreeSet<String>,
    pub effective_power: Option<i32>,
    pub mana_value: Option<u32>,
    pub mana_cost: Option<ManaCost>,
    pub tapped: bool,
    pub can_be_sacrificed: Option<bool>,
    pub can_be_tapped_to_pay_cost: Option<bool>,
    pub can_be_returned_to_hand: Option<bool>,
    pub can_be_targeted_by_controller: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardObjectState {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: CardZone,
    pub discarded_this_turn: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatDamageHistoryEntry {
    pub turn_id: TurnId,
    pub source_controller: PlayerId,
    pub damaged_player: PlayerId,
    pub source_was_assassin: bool,
    pub source_was_commander: bool,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastRuntimeState {
    pub turn_id: TurnId,
    pub players: BTreeMap<PlayerId, PlayerCastState>,
    pub cards: BTreeMap<ObjectRef, CardObjectState>,
    pub permanents: BTreeMap<ObjectRef, PermanentCastState>,
    pub combat_damage_history: Vec<CombatDamageHistoryEntry>,
    pub players_complete: bool,
    pub zones_complete: bool,
    pub battlefield_complete: bool,
    pub mana_pools_complete: bool,
    pub combat_history_complete: bool,
    pub cost_restrictions_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedManaRequirement {
    pub generic: u32,
    pub colored: BTreeMap<ManaKind, u32>,
    pub snow: u32,
    pub hybrid: Vec<(ManaKind, ManaKind)>,
    pub phyrexian: Vec<ManaKind>,
}

impl ResolvedManaRequirement {
    fn zero() -> Self {
        Self {
            generic: 0,
            colored: BTreeMap::new(),
            snow: 0,
            hybrid: Vec::new(),
            phyrexian: Vec::new(),
        }
    }

    fn from_cost(cost: &ManaCost, x_value: Option<u32>) -> Result<Self, CastChoiceRuntimeError> {
        let mut requirement = Self::zero();
        for symbol in &cost.symbols {
            match symbol {
                ManaSymbol::Generic(amount) => {
                    requirement.generic = requirement.generic.checked_add(*amount).ok_or(
                        CastChoiceRuntimeError::ArithmeticOverflow("adding generic mana"),
                    )?;
                }
                ManaSymbol::Colored(color) => {
                    let kind = ManaKind::from(*color);
                    let current = requirement.colored.get(&kind).copied().unwrap_or_default();
                    requirement.colored.insert(
                        kind,
                        current.checked_add(1).ok_or(
                            CastChoiceRuntimeError::ArithmeticOverflow("adding colored mana"),
                        )?,
                    );
                }
                ManaSymbol::Colorless => {
                    let current = requirement
                        .colored
                        .get(&ManaKind::Colorless)
                        .copied()
                        .unwrap_or_default();
                    requirement.colored.insert(
                        ManaKind::Colorless,
                        current.checked_add(1).ok_or(
                            CastChoiceRuntimeError::ArithmeticOverflow("adding colorless mana"),
                        )?,
                    );
                }
                ManaSymbol::Snow => {
                    requirement.snow = requirement.snow.checked_add(1).ok_or(
                        CastChoiceRuntimeError::ArithmeticOverflow("adding snow mana"),
                    )?;
                }
                ManaSymbol::VariableX => {
                    requirement.generic = requirement
                        .generic
                        .checked_add(
                            x_value.ok_or(CastChoiceRuntimeError::MissingChoice("X value"))?,
                        )
                        .ok_or(CastChoiceRuntimeError::ArithmeticOverflow(
                            "adding variable mana",
                        ))?;
                }
                ManaSymbol::Hybrid(first, second) => requirement
                    .hybrid
                    .push((ManaKind::from(*first), ManaKind::from(*second))),
                ManaSymbol::Phyrexian(color) => requirement.phyrexian.push(ManaKind::from(*color)),
            }
        }
        Ok(requirement)
    }

    fn add_scaled(
        &mut self,
        cost: &ManaCost,
        times: u32,
        x_value: Option<u32>,
    ) -> Result<(), CastChoiceRuntimeError> {
        for _ in 0..times {
            let addition = Self::from_cost(cost, x_value)?;
            self.add(&addition)?;
        }
        Ok(())
    }

    fn add(&mut self, other: &Self) -> Result<(), CastChoiceRuntimeError> {
        self.generic = self.generic.checked_add(other.generic).ok_or(
            CastChoiceRuntimeError::ArithmeticOverflow("combining generic mana"),
        )?;
        self.snow =
            self.snow
                .checked_add(other.snow)
                .ok_or(CastChoiceRuntimeError::ArithmeticOverflow(
                    "combining snow mana",
                ))?;
        for (kind, amount) in &other.colored {
            let current = self.colored.get(kind).copied().unwrap_or_default();
            self.colored.insert(
                *kind,
                current
                    .checked_add(*amount)
                    .ok_or(CastChoiceRuntimeError::ArithmeticOverflow(
                        "combining colored mana",
                    ))?,
            );
        }
        self.hybrid.extend(other.hybrid.iter().copied());
        self.phyrexian.extend(other.phyrexian.iter().copied());
        Ok(())
    }

    fn reduce_generic(&mut self, amount: u32) {
        self.generic = self.generic.saturating_sub(amount);
    }

    fn reduce_by_mana_cost(&mut self, reduction: &ManaCost) -> Result<(), CastChoiceRuntimeError> {
        for symbol in &reduction.symbols {
            match symbol {
                ManaSymbol::Generic(amount) => self.reduce_generic(*amount),
                ManaSymbol::Colored(color) => {
                    self.reduce_specific_or_generic(ManaKind::from(*color), 1)
                }
                ManaSymbol::Colorless => self.reduce_specific_or_generic(ManaKind::Colorless, 1),
                ManaSymbol::Snow => self.reduce_generic(1),
                ManaSymbol::VariableX => {}
                ManaSymbol::Phyrexian(color) => {
                    self.reduce_specific_or_generic(ManaKind::from(*color), 1)
                }
                ManaSymbol::Hybrid(_, _) => {
                    return Err(CastChoiceRuntimeError::UnsupportedCostReduction(
                        reduction.oracle_text(),
                    ));
                }
            }
        }
        Ok(())
    }

    fn reduce_specific_or_generic(&mut self, kind: ManaKind, amount: u32) {
        let current = self.colored.get(&kind).copied().unwrap_or_default();
        let removed = current.min(amount);
        if removed == current {
            self.colored.remove(&kind);
        } else {
            self.colored.insert(kind, current - removed);
        }
        self.reduce_generic(amount - removed);
    }

    fn apply_external_adjustments(
        &mut self,
        generic_increase: u32,
        generic_reduction: u32,
    ) -> Result<(), CastChoiceRuntimeError> {
        self.generic = self.generic.checked_add(generic_increase).ok_or(
            CastChoiceRuntimeError::ArithmeticOverflow("applying generic cost increase"),
        )?;
        self.reduce_generic(generic_reduction);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPaymentPlan {
    pub selected_units: Vec<ManaUnitId>,
    pub hybrid_choices: Vec<ManaKind>,
    pub phyrexian_paid_with_life: Vec<bool>,
    pub assisting_player: Option<PlayerId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonManaPaymentPlan {
    pub casualty_sacrifice: Option<ObjectRef>,
    pub emerge_or_offering_sacrifice: Option<ObjectRef>,
    pub tapped_permanents: Vec<ObjectRef>,
    pub returned_permanents: Vec<ObjectRef>,
    pub discarded_cards: Vec<ObjectRef>,
    pub exiled_graveyard_cards: Vec<ObjectRef>,
}

impl NonManaPaymentPlan {
    pub fn none() -> Self {
        Self {
            casualty_sacrifice: None,
            emerge_or_offering_sacrifice: None,
            tapped_permanents: Vec::new(),
            returned_permanents: Vec::new(),
            discarded_cards: Vec::new(),
            exiled_graveyard_cards: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastChoiceDeclaration {
    Casualty {
        pay: bool,
    },
    Cleave {
        use_alternative: bool,
    },
    Emerge {
        use_alternative: bool,
    },
    Escalate {
        chosen_modes: u32,
    },
    Offering {
        use_offering: bool,
    },
    Prototype {
        cast_prototyped: bool,
    },
    Squad {
        times_paid: u32,
    },
    Assist {
        assisting_player: Option<PlayerId>,
    },
    Awaken {
        use_alternative: bool,
        awaken_target: Option<ObjectRef>,
    },
    Strive,
    Freerunning {
        use_alternative: bool,
    },
    Impending {
        use_alternative: bool,
    },
    MoreThanMeetsTheEye {
        cast_converted: bool,
    },
    Offspring {
        pay: bool,
    },
    WebSlinging {
        use_alternative: bool,
    },
    Mayhem {
        use_mayhem: bool,
    },
    Miracle {
        permission: Option<MiracleCastPermission>,
    },
}

impl CastChoiceDeclaration {
    pub const fn family(&self) -> CastChoiceKeywordFamily {
        match self {
            Self::Casualty { .. } => CastChoiceKeywordFamily::Casualty,
            Self::Cleave { .. } => CastChoiceKeywordFamily::Cleave,
            Self::Emerge { .. } => CastChoiceKeywordFamily::Emerge,
            Self::Escalate { .. } => CastChoiceKeywordFamily::Escalate,
            Self::Offering { .. } => CastChoiceKeywordFamily::Offering,
            Self::Prototype { .. } => CastChoiceKeywordFamily::Prototype,
            Self::Squad { .. } => CastChoiceKeywordFamily::Squad,
            Self::Assist { .. } => CastChoiceKeywordFamily::Assist,
            Self::Awaken { .. } => CastChoiceKeywordFamily::Awaken,
            Self::Strive => CastChoiceKeywordFamily::Strive,
            Self::Freerunning { .. } => CastChoiceKeywordFamily::Freerunning,
            Self::Impending { .. } => CastChoiceKeywordFamily::Impending,
            Self::MoreThanMeetsTheEye { .. } => CastChoiceKeywordFamily::MoreThanMeetsTheEye,
            Self::Offspring { .. } => CastChoiceKeywordFamily::Offspring,
            Self::WebSlinging { .. } => CastChoiceKeywordFamily::WebSlinging,
            Self::Mayhem { .. } => CastChoiceKeywordFamily::Mayhem,
            Self::Miracle { .. } => CastChoiceKeywordFamily::Miracle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastAttempt {
    pub cast_id: CastId,
    pub ability_instance_id: AbilityInstanceId,
    pub source: ObjectRef,
    pub controller: PlayerId,
    pub source_zone: CardZone,
    pub printed_mana_cost: ManaCost,
    pub x_value: Option<u32>,
    pub choice: CastChoiceDeclaration,
    /// Modes are chosen before targets and total cost.
    pub chosen_modes: Vec<u32>,
    /// Complete target identities chosen after alternative and additional cost
    /// choices and before total cost is determined.
    pub targets: Vec<ObjectRef>,
    pub normal_zone_permission: bool,
    pub normal_timing_permission: bool,
    /// Complete evidence that prohibitions and spell-specific casting
    /// restrictions permit this cast independently of zone and timing grants.
    pub casting_restrictions_satisfied: bool,
    pub another_alternative_method_selected: bool,
    /// The cost supplied by an earlier exact owner when that other alternative
    /// method is selected. Additional-cost families in this module can combine
    /// with it, while alternative-cost families reject the combination.
    pub another_alternative_cost: Option<ManaCost>,
    pub generic_cost_increase: u32,
    pub generic_cost_reduction: u32,
    pub cost_adjustments_complete: bool,
    pub mana_payment: ManaPaymentPlan,
    pub nonmana_payment: NonManaPaymentPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCharacteristicsReceipt {
    pub prototyped: bool,
    pub prototype_power: Option<i32>,
    pub prototype_toughness: Option<i32>,
    pub prototype_mana_cost: Option<ManaCost>,
    pub cleaved: bool,
    pub removed_text_fragments: Vec<String>,
    pub converted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedCastChoices {
    pub casualty_paid: bool,
    pub squad_times_paid: u32,
    pub offspring_paid: bool,
    pub impending_paid: bool,
    pub impending_time_counters: u32,
    pub awaken_paid: bool,
    pub awaken_target: Option<ObjectRef>,
    pub emerge_sacrifice: Option<ObjectRef>,
    pub offering_paid: bool,
    pub offering_sacrifice: Option<ObjectRef>,
    pub freerunning_paid: bool,
    pub web_slinging_paid: bool,
    pub web_slinging_return: Option<ObjectRef>,
    pub mayhem_paid: bool,
    pub miracle_paid: bool,
    pub assisting_player: Option<PlayerId>,
    pub assisted_generic_mana: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastReceipt {
    pub cast_id: CastId,
    pub ability_instance_id: AbilityInstanceId,
    pub program_digest: String,
    pub source_before_cast: ObjectRef,
    pub stack_object: ObjectRef,
    pub controller: PlayerId,
    pub family: CastChoiceKeywordFamily,
    pub x_value: Option<u32>,
    pub chosen_modes: Vec<u32>,
    pub targets: Vec<ObjectRef>,
    pub locked_mana_cost: ResolvedManaRequirement,
    pub characteristics: SpellCharacteristicsReceipt,
    pub linked: LinkedCastChoices,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastExecution {
    Spell(CastReceipt),
    LandPlayed {
        source_before_play: ObjectRef,
        battlefield_object: ObjectRef,
        controller: PlayerId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawEvent {
    pub event_id: EventId,
    pub turn_id: TurnId,
    pub player: PlayerId,
    pub card_in_hand: ObjectRef,
    pub draw_number_this_turn: u32,
    pub was_draw: bool,
    pub event_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiracleRevealTrigger {
    pub trigger_id: TriggerId,
    pub event_id: EventId,
    pub turn_id: TurnId,
    pub player: PlayerId,
    pub revealed_card: ObjectRef,
    pub program_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiracleCastPermission {
    pub trigger_id: TriggerId,
    pub turn_id: TurnId,
    pub player: PlayerId,
    pub revealed_card: ObjectRef,
    pub program_digest: String,
}

pub fn create_miracle_reveal_trigger(
    program: &CastChoiceKeywordProgram,
    trigger_id: TriggerId,
    event: &DrawEvent,
    reveal: bool,
) -> Result<Option<MiracleRevealTrigger>, CastChoiceRuntimeError> {
    if !matches!(program.kind(), CastChoiceKeywordKind::Miracle { .. }) {
        return Err(CastChoiceRuntimeError::WrongKeywordFamily);
    }
    if !event.event_complete {
        return Err(CastChoiceRuntimeError::IncompleteEvidence("draw event"));
    }
    if !event.was_draw || event.draw_number_this_turn != 1 || !reveal {
        return Ok(None);
    }
    Ok(Some(MiracleRevealTrigger {
        trigger_id,
        event_id: event.event_id,
        turn_id: event.turn_id,
        player: event.player,
        revealed_card: event.card_in_hand,
        program_digest: program.semantic_digest().to_owned(),
    }))
}

pub fn resolve_miracle_reveal_trigger(
    trigger: MiracleRevealTrigger,
    program: &CastChoiceKeywordProgram,
    state: &CastRuntimeState,
) -> Result<MiracleCastPermission, CastChoiceRuntimeError> {
    if trigger.program_digest != program.semantic_digest()
        || !matches!(program.kind(), CastChoiceKeywordKind::Miracle { .. })
    {
        return Err(CastChoiceRuntimeError::ProgramReceiptMismatch);
    }
    if trigger.turn_id != state.turn_id {
        return Err(CastChoiceRuntimeError::TurnBoundaryMismatch);
    }
    let card = state
        .cards
        .get(&trigger.revealed_card)
        .ok_or(CastChoiceRuntimeError::MissingObject(trigger.revealed_card))?;
    if card.zone != CardZone::Hand || card.controller != trigger.player {
        return Err(CastChoiceRuntimeError::MiracleCardNoLongerRevealedInHand);
    }
    Ok(MiracleCastPermission {
        trigger_id: trigger.trigger_id,
        turn_id: trigger.turn_id,
        player: trigger.player,
        revealed_card: trigger.revealed_card,
        program_digest: trigger.program_digest,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastChoiceRuntimeError {
    IncompleteEvidence(&'static str),
    WrongKeywordFamily,
    ProgramReceiptMismatch,
    MissingChoice(&'static str),
    UnexpectedChoice(&'static str),
    IllegalAlternativeCombination,
    IllegalSourceZone,
    TimingPermissionDenied,
    TurnBoundaryMismatch,
    MissingObject(ObjectRef),
    MissingPlayer(PlayerId),
    InvalidModeChoice,
    InvalidTargetChoice,
    InvalidPermanentPayment(ObjectRef),
    InvalidCardPayment(ObjectRef),
    DuplicatePaymentObject(ObjectRef),
    InvalidManaPayment(&'static str),
    UnsupportedCostReduction(String),
    ArithmeticOverflow(&'static str),
    IncarnationOverflow(ObjectId),
    ZoneObjectCollision(ObjectRef),
    FreerunningConditionNotMet,
    MayhemDiscardConditionNotMet,
    MiraclePermissionRequired,
    MiracleCardNoLongerRevealedInHand,
    LandPlayUnavailable,
    NormalLandPlayOutsideKeywordRuntime,
}

impl fmt::Display for CastChoiceRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompleteEvidence(field) => write!(formatter, "incomplete evidence: {field}"),
            Self::WrongKeywordFamily => write!(formatter, "keyword family does not match"),
            Self::ProgramReceiptMismatch => write!(formatter, "program and receipt do not match"),
            Self::MissingChoice(choice) => write!(formatter, "missing choice: {choice}"),
            Self::UnexpectedChoice(choice) => write!(formatter, "unexpected choice: {choice}"),
            Self::IllegalAlternativeCombination => {
                write!(formatter, "more than one alternative method was selected")
            }
            Self::IllegalSourceZone => write!(formatter, "source zone is not permitted"),
            Self::TimingPermissionDenied => write!(formatter, "casting timing is not permitted"),
            Self::TurnBoundaryMismatch => write!(formatter, "turn boundary mismatch"),
            Self::MissingObject(object) => write!(formatter, "missing object {object:?}"),
            Self::MissingPlayer(player) => write!(formatter, "missing player {player}"),
            Self::InvalidModeChoice => write!(formatter, "invalid mode choice"),
            Self::InvalidTargetChoice => write!(formatter, "invalid target choice"),
            Self::InvalidPermanentPayment(object) => {
                write!(formatter, "invalid permanent payment {object:?}")
            }
            Self::InvalidCardPayment(object) => {
                write!(formatter, "invalid card payment {object:?}")
            }
            Self::DuplicatePaymentObject(object) => {
                write!(formatter, "object paid more than once {object:?}")
            }
            Self::InvalidManaPayment(reason) => write!(formatter, "invalid mana payment: {reason}"),
            Self::UnsupportedCostReduction(cost) => {
                write!(formatter, "unsupported cost reduction {cost}")
            }
            Self::ArithmeticOverflow(operation) => {
                write!(formatter, "arithmetic overflow while {operation}")
            }
            Self::IncarnationOverflow(object) => {
                write!(formatter, "incarnation overflow for object {object}")
            }
            Self::ZoneObjectCollision(object) => {
                write!(formatter, "zone object collision {object:?}")
            }
            Self::FreerunningConditionNotMet => {
                write!(formatter, "freerunning combat damage condition was not met")
            }
            Self::MayhemDiscardConditionNotMet => {
                write!(formatter, "mayhem discard condition was not met")
            }
            Self::MiraclePermissionRequired => write!(formatter, "miracle permission is required"),
            Self::MiracleCardNoLongerRevealedInHand => {
                write!(formatter, "miracle card is no longer revealed in hand")
            }
            Self::LandPlayUnavailable => write!(formatter, "land play is unavailable"),
            Self::NormalLandPlayOutsideKeywordRuntime => {
                write!(
                    formatter,
                    "normal land play belongs to the normal land action runtime"
                )
            }
        }
    }
}

impl std::error::Error for CastChoiceRuntimeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedKeywordCost {
    mana: ResolvedManaRequirement,
    nonmana_costs: Vec<NonManaCost>,
    characteristics: SpellCharacteristicsReceipt,
    linked: LinkedCastChoices,
    grants_instant_timing: bool,
    uses_alternative_cost: bool,
    assisting_player: Option<PlayerId>,
    phyrexian_life_payment: u32,
}

pub fn execute_cast_choice(
    program: &CastChoiceKeywordProgram,
    attempt: &CastAttempt,
    state: &mut CastRuntimeState,
) -> Result<CastExecution, CastChoiceRuntimeError> {
    require_cast_state(state)?;
    if attempt.choice.family() != program.kind().family() {
        return Err(CastChoiceRuntimeError::WrongKeywordFamily);
    }
    if attempt.controller
        != state
            .cards
            .get(&attempt.source)
            .ok_or(CastChoiceRuntimeError::MissingObject(attempt.source))?
            .controller
        || state.cards[&attempt.source].zone != attempt.source_zone
    {
        return Err(CastChoiceRuntimeError::IllegalSourceZone);
    }
    if !attempt.cost_adjustments_complete {
        return Err(CastChoiceRuntimeError::IncompleteEvidence(
            "cost adjustments",
        ));
    }
    if !attempt.casting_restrictions_satisfied {
        return Err(CastChoiceRuntimeError::TimingPermissionDenied);
    }
    match (
        attempt.another_alternative_method_selected,
        attempt.another_alternative_cost.is_some(),
    ) {
        (true, false) => {
            return Err(CastChoiceRuntimeError::IncompleteEvidence(
                "other alternative cost",
            ));
        }
        (false, true) => {
            return Err(CastChoiceRuntimeError::UnexpectedChoice(
                "other alternative cost",
            ));
        }
        _ => {}
    }

    let prepared = prepare_keyword_cost(program, attempt, state)?;
    if prepared.uses_alternative_cost && attempt.another_alternative_method_selected {
        return Err(CastChoiceRuntimeError::IllegalAlternativeCombination);
    }

    if matches!(
        program.kind(),
        CastChoiceKeywordKind::Mayhem {
            alternative_cost: None,
            permits_land_play: true
        }
    ) {
        return if matches!(
            attempt.choice,
            CastChoiceDeclaration::Mayhem { use_mayhem: true }
        ) {
            execute_mayhem_land_play(attempt, state)
        } else {
            Err(CastChoiceRuntimeError::NormalLandPlayOutsideKeywordRuntime)
        };
    }

    validate_zone_and_timing(program, attempt, &prepared, state)?;
    validate_nonmana_payments(program, attempt, &prepared, state)?;
    let selected_mana = validate_mana_payment(attempt, &prepared, state)?;
    let assisted_generic_mana = u32::try_from(
        selected_mana
            .iter()
            .filter(|unit| unit.controller != attempt.controller)
            .count(),
    )
    .map_err(|_| CastChoiceRuntimeError::ArithmeticOverflow("counting assisted mana"))?;
    validate_payment_object_uniqueness(&attempt.nonmana_payment)?;
    let stack_object = attempt.source.next_incarnation()?;
    ensure_zone_ref_unused(stack_object, state)?;

    apply_nonmana_payments(attempt, &prepared, state)?;
    apply_mana_payment(
        attempt.controller,
        &selected_mana,
        prepared.phyrexian_life_payment,
        state,
    )?;
    move_source_to_stack(attempt, stack_object, state)?;

    let mut linked = prepared.linked;
    linked.assisting_player = prepared.assisting_player;
    linked.assisted_generic_mana = assisted_generic_mana;
    Ok(CastExecution::Spell(CastReceipt {
        cast_id: attempt.cast_id,
        ability_instance_id: attempt.ability_instance_id,
        program_digest: program.semantic_digest().to_owned(),
        source_before_cast: attempt.source,
        stack_object,
        controller: attempt.controller,
        family: program.kind().family(),
        x_value: attempt.x_value,
        chosen_modes: attempt.chosen_modes.clone(),
        targets: attempt.targets.clone(),
        locked_mana_cost: prepared.mana,
        characteristics: prepared.characteristics,
        linked,
    }))
}

fn require_cast_state(state: &CastRuntimeState) -> Result<(), CastChoiceRuntimeError> {
    for (complete, field) in [
        (state.players_complete, "players"),
        (state.zones_complete, "zones"),
        (state.battlefield_complete, "battlefield"),
        (state.mana_pools_complete, "mana pools"),
        (state.cost_restrictions_complete, "cost restrictions"),
    ] {
        if !complete {
            return Err(CastChoiceRuntimeError::IncompleteEvidence(field));
        }
    }
    Ok(())
}

fn prepare_keyword_cost(
    program: &CastChoiceKeywordProgram,
    attempt: &CastAttempt,
    state: &CastRuntimeState,
) -> Result<PreparedKeywordCost, CastChoiceRuntimeError> {
    let normal_characteristics = SpellCharacteristicsReceipt {
        prototyped: false,
        prototype_power: None,
        prototype_toughness: None,
        prototype_mana_cost: None,
        cleaved: false,
        removed_text_fragments: Vec::new(),
        converted: false,
    };
    let mut linked = LinkedCastChoices {
        casualty_paid: false,
        squad_times_paid: 0,
        offspring_paid: false,
        impending_paid: false,
        impending_time_counters: 0,
        awaken_paid: false,
        awaken_target: None,
        emerge_sacrifice: None,
        offering_paid: false,
        offering_sacrifice: None,
        freerunning_paid: false,
        web_slinging_paid: false,
        web_slinging_return: None,
        mayhem_paid: false,
        miracle_paid: false,
        assisting_player: None,
        assisted_generic_mana: 0,
    };
    let mut characteristics = normal_characteristics;
    let mut nonmana_costs = Vec::new();
    let mut grants_instant_timing = false;
    let mut uses_alternative_cost = false;
    let mut assisting_player = None;
    let mut base_cost = attempt
        .another_alternative_cost
        .clone()
        .unwrap_or_else(|| attempt.printed_mana_cost.clone());
    let mut additional_mana = Vec::<(ManaCost, u32)>::new();
    let mut emerge_reduction = None;
    let mut offering_reduction = None;

    match (program.kind(), &attempt.choice) {
        (CastChoiceKeywordKind::Casualty { .. }, CastChoiceDeclaration::Casualty { pay }) => {
            linked.casualty_paid = *pay;
        }
        (
            CastChoiceKeywordKind::Cleave {
                alternative_cost,
                removed_fragments,
            },
            CastChoiceDeclaration::Cleave { use_alternative },
        ) => {
            if *use_alternative {
                uses_alternative_cost = true;
                base_cost = alternative_cost.clone();
                characteristics.cleaved = true;
                characteristics.removed_text_fragments = removed_fragments.clone();
            }
        }
        (
            CastChoiceKeywordKind::Emerge {
                alternative_cost, ..
            },
            CastChoiceDeclaration::Emerge { use_alternative },
        ) => {
            if *use_alternative {
                uses_alternative_cost = true;
                base_cost = alternative_cost.clone();
                let sacrifice = attempt
                    .nonmana_payment
                    .emerge_or_offering_sacrifice
                    .ok_or(CastChoiceRuntimeError::MissingChoice("emerge sacrifice"))?;
                linked.emerge_sacrifice = Some(sacrifice);
                emerge_reduction = Some(
                    state
                        .permanents
                        .get(&sacrifice)
                        .and_then(|permanent| permanent.mana_value)
                        .ok_or(CastChoiceRuntimeError::IncompleteEvidence(
                            "sacrificed permanent mana value",
                        ))?,
                );
            }
        }
        (
            CastChoiceKeywordKind::Escalate {
                additional_cost_per_extra_mode,
                available_modes,
            },
            CastChoiceDeclaration::Escalate { chosen_modes },
        ) => {
            validate_modes(attempt, *chosen_modes, *available_modes)?;
            let beyond_first = chosen_modes.saturating_sub(1);
            add_cost_expression(
                additional_cost_per_extra_mode,
                beyond_first,
                &mut additional_mana,
                &mut nonmana_costs,
            )?;
        }
        (
            CastChoiceKeywordKind::Offering { .. },
            CastChoiceDeclaration::Offering { use_offering },
        ) => {
            if *use_offering {
                let sacrifice = attempt
                    .nonmana_payment
                    .emerge_or_offering_sacrifice
                    .ok_or(CastChoiceRuntimeError::MissingChoice("offering sacrifice"))?;
                offering_reduction = Some(
                    state
                        .permanents
                        .get(&sacrifice)
                        .and_then(|permanent| permanent.mana_cost.clone())
                        .ok_or(CastChoiceRuntimeError::IncompleteEvidence(
                            "sacrificed permanent mana cost",
                        ))?,
                );
                grants_instant_timing = true;
                linked.offering_paid = true;
                linked.offering_sacrifice = Some(sacrifice);
            }
        }
        (
            CastChoiceKeywordKind::Prototype {
                alternative_cost,
                power,
                toughness,
            },
            CastChoiceDeclaration::Prototype { cast_prototyped },
        ) => {
            if *cast_prototyped {
                uses_alternative_cost = true;
                base_cost = alternative_cost.clone();
                characteristics.prototyped = true;
                characteristics.prototype_power = Some(*power);
                characteristics.prototype_toughness = Some(*toughness);
                characteristics.prototype_mana_cost = Some(alternative_cost.clone());
            }
        }
        (
            CastChoiceKeywordKind::Squad {
                repeatable_additional_cost,
                ..
            },
            CastChoiceDeclaration::Squad { times_paid },
        ) => {
            linked.squad_times_paid = *times_paid;
            add_cost_expression(
                repeatable_additional_cost,
                *times_paid,
                &mut additional_mana,
                &mut nonmana_costs,
            )?;
        }
        (
            CastChoiceKeywordKind::Assist,
            CastChoiceDeclaration::Assist {
                assisting_player: selected,
            },
        ) => {
            if selected.is_some_and(|player| player == attempt.controller) {
                return Err(CastChoiceRuntimeError::UnexpectedChoice(
                    "assist requires another player",
                ));
            }
            if let Some(player) = selected
                && !state
                    .players
                    .get(player)
                    .is_some_and(|candidate| candidate.in_game)
            {
                return Err(CastChoiceRuntimeError::MissingPlayer(*player));
            }
            assisting_player = *selected;
        }
        (
            CastChoiceKeywordKind::Awaken {
                alternative_cost, ..
            },
            CastChoiceDeclaration::Awaken {
                use_alternative,
                awaken_target,
            },
        ) => {
            if *use_alternative {
                uses_alternative_cost = true;
                base_cost = alternative_cost.clone();
                let target =
                    awaken_target.ok_or(CastChoiceRuntimeError::MissingChoice("awaken target"))?;
                if !attempt.targets.contains(&target) {
                    return Err(CastChoiceRuntimeError::InvalidTargetChoice);
                }
                let target_state = state
                    .permanents
                    .get(&target)
                    .ok_or(CastChoiceRuntimeError::InvalidTargetChoice)?;
                if target_state.controller != attempt.controller || !target_state.is_land {
                    return Err(CastChoiceRuntimeError::InvalidTargetChoice);
                }
                match target_state.can_be_targeted_by_controller {
                    Some(true) => {}
                    Some(false) => return Err(CastChoiceRuntimeError::InvalidTargetChoice),
                    None => {
                        return Err(CastChoiceRuntimeError::IncompleteEvidence(
                            "awaken target legality",
                        ));
                    }
                }
                linked.awaken_paid = true;
                linked.awaken_target = Some(target);
            } else if awaken_target.is_some() {
                return Err(CastChoiceRuntimeError::UnexpectedChoice(
                    "awaken target without awaken cost",
                ));
            }
        }
        (
            CastChoiceKeywordKind::Strive {
                additional_cost_per_extra_target,
            },
            CastChoiceDeclaration::Strive,
        ) => {
            let beyond_first =
                u32::try_from(attempt.targets.len().saturating_sub(1)).map_err(|_| {
                    CastChoiceRuntimeError::ArithmeticOverflow("counting strive targets")
                })?;
            additional_mana.push((additional_cost_per_extra_target.clone(), beyond_first));
        }
        (
            CastChoiceKeywordKind::Freerunning { alternative_cost },
            CastChoiceDeclaration::Freerunning { use_alternative },
        ) => {
            if *use_alternative {
                if !state.combat_history_complete {
                    return Err(CastChoiceRuntimeError::IncompleteEvidence(
                        "combat damage history",
                    ));
                }
                let condition = state.combat_damage_history.iter().any(|entry| {
                    entry.turn_id == state.turn_id
                        && entry.source_controller == attempt.controller
                        && entry.amount > 0
                        && (entry.source_was_assassin || entry.source_was_commander)
                });
                if !condition {
                    return Err(CastChoiceRuntimeError::FreerunningConditionNotMet);
                }
                uses_alternative_cost = true;
                linked.freerunning_paid = true;
                base_cost = alternative_cost
                    .mana
                    .clone()
                    .unwrap_or(ManaCost { symbols: vec![] });
                nonmana_costs.extend(alternative_cost.nonmana.clone());
            }
        }
        (
            CastChoiceKeywordKind::Impending {
                alternative_cost,
                time_counters,
            },
            CastChoiceDeclaration::Impending { use_alternative },
        ) => {
            if *use_alternative {
                uses_alternative_cost = true;
                base_cost = alternative_cost.clone();
                linked.impending_paid = true;
                linked.impending_time_counters = *time_counters;
            }
        }
        (
            CastChoiceKeywordKind::MoreThanMeetsTheEye { alternative_cost },
            CastChoiceDeclaration::MoreThanMeetsTheEye { cast_converted },
        ) => {
            if *cast_converted {
                uses_alternative_cost = true;
                base_cost = alternative_cost.clone();
                characteristics.converted = true;
            }
        }
        (
            CastChoiceKeywordKind::Offspring { additional_cost },
            CastChoiceDeclaration::Offspring { pay },
        ) => {
            if *pay {
                additional_mana.push((additional_cost.clone(), 1));
                linked.offspring_paid = true;
            }
        }
        (
            CastChoiceKeywordKind::WebSlinging { alternative_cost },
            CastChoiceDeclaration::WebSlinging { use_alternative },
        ) => {
            if *use_alternative {
                uses_alternative_cost = true;
                linked.web_slinging_paid = true;
                base_cost = alternative_cost.clone();
                nonmana_costs.push(NonManaCost::ReturnTappedCreatureYouControlToOwnersHand);
                linked.web_slinging_return =
                    attempt.nonmana_payment.returned_permanents.first().copied();
            }
        }
        (
            CastChoiceKeywordKind::Mayhem {
                alternative_cost,
                permits_land_play: _,
            },
            CastChoiceDeclaration::Mayhem { use_mayhem },
        ) => {
            if *use_mayhem {
                let card = state
                    .cards
                    .get(&attempt.source)
                    .ok_or(CastChoiceRuntimeError::MissingObject(attempt.source))?;
                if card.discarded_this_turn != Some(true) {
                    return Err(CastChoiceRuntimeError::MayhemDiscardConditionNotMet);
                }
                if let Some(cost) = alternative_cost {
                    uses_alternative_cost = true;
                    linked.mayhem_paid = true;
                    base_cost = cost.clone();
                }
            }
        }
        (
            CastChoiceKeywordKind::Miracle { alternative_cost },
            CastChoiceDeclaration::Miracle { permission },
        ) => {
            let permission = permission
                .as_ref()
                .ok_or(CastChoiceRuntimeError::MiraclePermissionRequired)?;
            if permission.program_digest != program.semantic_digest()
                || permission.revealed_card != attempt.source
                || permission.player != attempt.controller
                || permission.turn_id != state.turn_id
            {
                return Err(CastChoiceRuntimeError::ProgramReceiptMismatch);
            }
            uses_alternative_cost = true;
            linked.miracle_paid = true;
            base_cost = alternative_cost.clone();
        }
        _ => return Err(CastChoiceRuntimeError::WrongKeywordFamily),
    }

    let mut mana = ResolvedManaRequirement::from_cost(&base_cost, attempt.x_value)?;
    for (cost, times) in additional_mana {
        mana.add_scaled(&cost, times, attempt.x_value)?;
    }
    mana.apply_external_adjustments(
        attempt.generic_cost_increase,
        attempt.generic_cost_reduction,
    )?;
    if let Some(reduction) = emerge_reduction {
        mana.reduce_generic(reduction);
    }
    if let Some(reduction) = offering_reduction {
        mana.reduce_by_mana_cost(&reduction)?;
    }

    let phyrexian_life_payment = u32::try_from(
        attempt
            .mana_payment
            .phyrexian_paid_with_life
            .iter()
            .filter(|paid| **paid)
            .count(),
    )
    .map_err(|_| CastChoiceRuntimeError::ArithmeticOverflow("counting Phyrexian life payments"))?
    .checked_mul(2)
    .ok_or(CastChoiceRuntimeError::ArithmeticOverflow(
        "calculating Phyrexian life payment",
    ))?;

    Ok(PreparedKeywordCost {
        mana,
        nonmana_costs,
        characteristics,
        linked,
        grants_instant_timing,
        uses_alternative_cost,
        assisting_player,
        phyrexian_life_payment,
    })
}

fn validate_modes(
    attempt: &CastAttempt,
    declared_count: u32,
    available_modes: u32,
) -> Result<(), CastChoiceRuntimeError> {
    if declared_count == 0
        || declared_count > available_modes
        || usize::try_from(declared_count).ok() != Some(attempt.chosen_modes.len())
    {
        return Err(CastChoiceRuntimeError::InvalidModeChoice);
    }
    let unique = attempt
        .chosen_modes
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if unique.len() != attempt.chosen_modes.len()
        || unique
            .iter()
            .any(|mode| *mode == 0 || *mode > available_modes)
    {
        return Err(CastChoiceRuntimeError::InvalidModeChoice);
    }
    Ok(())
}

fn add_cost_expression(
    expression: &CostExpression,
    times: u32,
    mana: &mut Vec<(ManaCost, u32)>,
    nonmana: &mut Vec<NonManaCost>,
) -> Result<(), CastChoiceRuntimeError> {
    if let Some(cost) = &expression.mana {
        mana.push((cost.clone(), times));
    }
    for _ in 0..times {
        nonmana.extend(expression.nonmana.clone());
    }
    Ok(())
}

fn validate_zone_and_timing(
    program: &CastChoiceKeywordProgram,
    attempt: &CastAttempt,
    prepared: &PreparedKeywordCost,
    state: &CastRuntimeState,
) -> Result<(), CastChoiceRuntimeError> {
    match program.kind() {
        CastChoiceKeywordKind::Mayhem { .. }
            if matches!(
                attempt.choice,
                CastChoiceDeclaration::Mayhem { use_mayhem: true }
            ) =>
        {
            if attempt.source_zone != CardZone::Graveyard {
                return Err(CastChoiceRuntimeError::IllegalSourceZone);
            }
            if !attempt.normal_timing_permission {
                return Err(CastChoiceRuntimeError::TimingPermissionDenied);
            }
        }
        CastChoiceKeywordKind::Miracle { .. } => {
            if attempt.source_zone != CardZone::Hand {
                return Err(CastChoiceRuntimeError::IllegalSourceZone);
            }
        }
        _ => {
            if !attempt.normal_zone_permission {
                return Err(CastChoiceRuntimeError::IllegalSourceZone);
            }
            if !attempt.normal_timing_permission && !prepared.grants_instant_timing {
                return Err(CastChoiceRuntimeError::TimingPermissionDenied);
            }
        }
    }
    let player = state
        .players
        .get(&attempt.controller)
        .ok_or(CastChoiceRuntimeError::MissingPlayer(attempt.controller))?;
    if !player.in_game {
        return Err(CastChoiceRuntimeError::MissingPlayer(attempt.controller));
    }
    Ok(())
}

fn validate_nonmana_payments(
    program: &CastChoiceKeywordProgram,
    attempt: &CastAttempt,
    prepared: &PreparedKeywordCost,
    state: &CastRuntimeState,
) -> Result<(), CastChoiceRuntimeError> {
    let mut required_taps = 0usize;
    let mut required_discards = 0usize;
    let mut required_exiles = 0usize;
    let mut required_return_blue = 0usize;
    let mut required_return_tapped = 0usize;
    for cost in &prepared.nonmana_costs {
        match cost {
            NonManaCost::TapUntappedCreatureYouControl => required_taps += 1,
            NonManaCost::DiscardCards(amount) => {
                required_discards = required_discards
                    .checked_add(usize::try_from(*amount).map_err(|_| {
                        CastChoiceRuntimeError::ArithmeticOverflow("counting discard payments")
                    })?)
                    .ok_or(CastChoiceRuntimeError::ArithmeticOverflow(
                        "counting discard payments",
                    ))?;
            }
            NonManaCost::ExileCardsFromYourGraveyard(amount) => {
                required_exiles = required_exiles
                    .checked_add(usize::try_from(*amount).map_err(|_| {
                        CastChoiceRuntimeError::ArithmeticOverflow("counting exile payments")
                    })?)
                    .ok_or(CastChoiceRuntimeError::ArithmeticOverflow(
                        "counting exile payments",
                    ))?;
            }
            NonManaCost::ReturnBlueCreatureYouControlToOwnersHand => required_return_blue += 1,
            NonManaCost::ReturnTappedCreatureYouControlToOwnersHand => required_return_tapped += 1,
        }
    }
    if attempt.nonmana_payment.tapped_permanents.len() != required_taps
        || attempt.nonmana_payment.discarded_cards.len() != required_discards
        || attempt.nonmana_payment.exiled_graveyard_cards.len() != required_exiles
        || attempt.nonmana_payment.returned_permanents.len()
            != required_return_blue + required_return_tapped
    {
        return Err(CastChoiceRuntimeError::MissingChoice(
            "complete nonmana payment",
        ));
    }

    for object in &attempt.nonmana_payment.tapped_permanents {
        let permanent = state
            .permanents
            .get(object)
            .ok_or(CastChoiceRuntimeError::InvalidPermanentPayment(*object))?;
        if permanent.controller != attempt.controller
            || !permanent.is_creature
            || permanent.tapped
            || permanent.can_be_tapped_to_pay_cost != Some(true)
        {
            return Err(CastChoiceRuntimeError::InvalidPermanentPayment(*object));
        }
    }
    for (index, object) in attempt
        .nonmana_payment
        .returned_permanents
        .iter()
        .enumerate()
    {
        let permanent = state
            .permanents
            .get(object)
            .ok_or(CastChoiceRuntimeError::InvalidPermanentPayment(*object))?;
        let needs_blue = index < required_return_blue;
        let needs_tapped = index >= required_return_blue;
        if permanent.controller != attempt.controller
            || !permanent.is_creature
            || (needs_blue && !permanent.is_blue)
            || (needs_tapped && !permanent.tapped)
            || permanent.can_be_returned_to_hand != Some(true)
        {
            return Err(CastChoiceRuntimeError::InvalidPermanentPayment(*object));
        }
    }
    for object in &attempt.nonmana_payment.discarded_cards {
        let card = state
            .cards
            .get(object)
            .ok_or(CastChoiceRuntimeError::InvalidCardPayment(*object))?;
        if card.zone != CardZone::Hand || card.controller != attempt.controller {
            return Err(CastChoiceRuntimeError::InvalidCardPayment(*object));
        }
    }
    for object in &attempt.nonmana_payment.exiled_graveyard_cards {
        let card = state
            .cards
            .get(object)
            .ok_or(CastChoiceRuntimeError::InvalidCardPayment(*object))?;
        if card.zone != CardZone::Graveyard || card.controller != attempt.controller {
            return Err(CastChoiceRuntimeError::InvalidCardPayment(*object));
        }
    }

    if let (
        CastChoiceKeywordKind::Casualty { minimum_power, .. },
        CastChoiceDeclaration::Casualty { pay },
    ) = (program.kind(), &attempt.choice)
    {
        match (*pay, attempt.nonmana_payment.casualty_sacrifice) {
            (false, None) => {}
            (true, Some(object)) => {
                let permanent = state
                    .permanents
                    .get(&object)
                    .ok_or(CastChoiceRuntimeError::InvalidPermanentPayment(object))?;
                let minimum = i32::try_from(*minimum_power).map_err(|_| {
                    CastChoiceRuntimeError::ArithmeticOverflow("converting casualty power")
                })?;
                if permanent.controller != attempt.controller
                    || !permanent.is_creature
                    || permanent
                        .effective_power
                        .is_none_or(|power| power < minimum)
                    || permanent.can_be_sacrificed != Some(true)
                {
                    return Err(CastChoiceRuntimeError::InvalidPermanentPayment(object));
                }
            }
            _ => {
                return Err(CastChoiceRuntimeError::MissingChoice("casualty sacrifice"));
            }
        }
    } else if attempt.nonmana_payment.casualty_sacrifice.is_some() {
        return Err(CastChoiceRuntimeError::UnexpectedChoice(
            "casualty sacrifice",
        ));
    }

    let selected_sacrifice = attempt.nonmana_payment.emerge_or_offering_sacrifice;
    match (program.kind(), &attempt.choice, selected_sacrifice) {
        (
            CastChoiceKeywordKind::Emerge {
                sacrifice_quality, ..
            },
            CastChoiceDeclaration::Emerge {
                use_alternative: true,
            },
            Some(object),
        ) => validate_sacrifice_quality(object, sacrifice_quality, attempt.controller, state)?,
        (
            CastChoiceKeywordKind::Offering { sacrifice_quality },
            CastChoiceDeclaration::Offering { use_offering: true },
            Some(object),
        ) => validate_sacrifice_quality(object, sacrifice_quality, attempt.controller, state)?,
        (
            CastChoiceKeywordKind::Emerge { .. },
            CastChoiceDeclaration::Emerge {
                use_alternative: false,
            },
            None,
        )
        | (
            CastChoiceKeywordKind::Offering { .. },
            CastChoiceDeclaration::Offering {
                use_offering: false,
            },
            None,
        ) => {}
        (_, _, None) => {}
        _ => {
            return Err(CastChoiceRuntimeError::UnexpectedChoice(
                "emerge or offering sacrifice",
            ));
        }
    }
    Ok(())
}

fn validate_sacrifice_quality(
    object: ObjectRef,
    quality: &SacrificeQuality,
    controller: PlayerId,
    state: &CastRuntimeState,
) -> Result<(), CastChoiceRuntimeError> {
    let permanent = state
        .permanents
        .get(&object)
        .ok_or(CastChoiceRuntimeError::InvalidPermanentPayment(object))?;
    let matches = match quality {
        SacrificeQuality::Creature => permanent.is_creature,
        SacrificeQuality::Artifact => permanent.is_artifact,
        SacrificeQuality::Subtype(subtype) => permanent
            .subtypes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(subtype)),
    };
    if permanent.controller != controller || permanent.can_be_sacrificed != Some(true) || !matches {
        return Err(CastChoiceRuntimeError::InvalidPermanentPayment(object));
    }
    Ok(())
}

fn validate_payment_object_uniqueness(
    payment: &NonManaPaymentPlan,
) -> Result<(), CastChoiceRuntimeError> {
    let objects = payment
        .casualty_sacrifice
        .into_iter()
        .chain(payment.emerge_or_offering_sacrifice)
        .chain(payment.tapped_permanents.iter().copied())
        .chain(payment.returned_permanents.iter().copied())
        .chain(payment.discarded_cards.iter().copied())
        .chain(payment.exiled_graveyard_cards.iter().copied())
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    for object in objects {
        if !seen.insert(object) {
            return Err(CastChoiceRuntimeError::DuplicatePaymentObject(object));
        }
    }
    Ok(())
}

fn validate_mana_payment(
    attempt: &CastAttempt,
    prepared: &PreparedKeywordCost,
    state: &CastRuntimeState,
) -> Result<Vec<ManaUnit>, CastChoiceRuntimeError> {
    if attempt.mana_payment.hybrid_choices.len() != prepared.mana.hybrid.len()
        || attempt.mana_payment.phyrexian_paid_with_life.len() != prepared.mana.phyrexian.len()
        || attempt.mana_payment.assisting_player != prepared.assisting_player
    {
        return Err(CastChoiceRuntimeError::InvalidManaPayment(
            "declared payment choices do not match the locked cost",
        ));
    }
    let mut selected_ids = BTreeSet::new();
    let mut units = Vec::new();
    for unit_id in &attempt.mana_payment.selected_units {
        if !selected_ids.insert(*unit_id) {
            return Err(CastChoiceRuntimeError::InvalidManaPayment(
                "mana unit selected more than once",
            ));
        }
        let unit = state
            .players
            .values()
            .find_map(|player| player.mana_pool.get(unit_id))
            .cloned()
            .ok_or(CastChoiceRuntimeError::InvalidManaPayment(
                "selected mana unit does not exist",
            ))?;
        if unit.controller != attempt.controller
            && Some(unit.controller) != prepared.assisting_player
        {
            return Err(CastChoiceRuntimeError::InvalidManaPayment(
                "mana came from an unauthorized player",
            ));
        }
        units.push(unit);
    }

    let mut restricted_slots = Vec::<RestrictedManaSlot>::new();
    for (kind, amount) in &prepared.mana.colored {
        for _ in 0..*amount {
            restricted_slots.push(RestrictedManaSlot::Specific(*kind));
        }
    }
    for _ in 0..prepared.mana.snow {
        restricted_slots.push(RestrictedManaSlot::Snow);
    }
    for ((first, second), chosen) in prepared
        .mana
        .hybrid
        .iter()
        .zip(&attempt.mana_payment.hybrid_choices)
    {
        if chosen != first && chosen != second {
            return Err(CastChoiceRuntimeError::InvalidManaPayment(
                "hybrid choice is not one of the printed halves",
            ));
        }
        restricted_slots.push(RestrictedManaSlot::Specific(*chosen));
    }
    for (kind, paid_with_life) in prepared
        .mana
        .phyrexian
        .iter()
        .zip(&attempt.mana_payment.phyrexian_paid_with_life)
    {
        if !paid_with_life {
            restricted_slots.push(RestrictedManaSlot::Specific(*kind));
        }
    }
    let restricted_indices = match_restricted_units(
        &restricted_slots,
        &units,
        attempt.controller,
        0,
        &mut BTreeSet::new(),
    )
    .ok_or(CastChoiceRuntimeError::InvalidManaPayment(
        "colored, colorless, snow, hybrid, or Phyrexian requirement was not paid",
    ))?;
    let remaining = units
        .iter()
        .enumerate()
        .filter(|(index, _)| !restricted_indices.contains(index))
        .map(|(_, unit)| unit)
        .collect::<Vec<_>>();
    if remaining.len()
        != usize::try_from(prepared.mana.generic).map_err(|_| {
            CastChoiceRuntimeError::ArithmeticOverflow("converting generic mana requirement")
        })?
    {
        return Err(CastChoiceRuntimeError::InvalidManaPayment(
            "generic mana payment does not equal the locked generic cost",
        ));
    }
    if units
        .iter()
        .enumerate()
        .filter(|(_, unit)| unit.controller != attempt.controller)
        .any(|(index, _)| restricted_indices.contains(&index))
    {
        return Err(CastChoiceRuntimeError::InvalidManaPayment(
            "assist may pay only generic mana",
        ));
    }
    let player = state
        .players
        .get(&attempt.controller)
        .ok_or(CastChoiceRuntimeError::MissingPlayer(attempt.controller))?;
    let life = i32::try_from(prepared.phyrexian_life_payment).map_err(|_| {
        CastChoiceRuntimeError::ArithmeticOverflow("converting Phyrexian life payment")
    })?;
    if player.life < life {
        return Err(CastChoiceRuntimeError::InvalidManaPayment(
            "not enough life for Phyrexian mana",
        ));
    }
    Ok(units)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestrictedManaSlot {
    Specific(ManaKind),
    Snow,
}

fn match_restricted_units(
    slots: &[RestrictedManaSlot],
    units: &[ManaUnit],
    caster: PlayerId,
    slot_index: usize,
    used: &mut BTreeSet<usize>,
) -> Option<BTreeSet<usize>> {
    if slot_index == slots.len() {
        return Some(used.clone());
    }
    for (index, unit) in units.iter().enumerate() {
        if used.contains(&index) || unit.controller != caster {
            continue;
        }
        let matched = match slots[slot_index] {
            RestrictedManaSlot::Specific(kind) => unit.kind == kind,
            RestrictedManaSlot::Snow => unit.from_snow_source,
        };
        if matched {
            used.insert(index);
            if let Some(solution) =
                match_restricted_units(slots, units, caster, slot_index + 1, used)
            {
                return Some(solution);
            }
            used.remove(&index);
        }
    }
    None
}

fn apply_nonmana_payments(
    attempt: &CastAttempt,
    _prepared: &PreparedKeywordCost,
    state: &mut CastRuntimeState,
) -> Result<(), CastChoiceRuntimeError> {
    let sacrifices = attempt
        .nonmana_payment
        .casualty_sacrifice
        .into_iter()
        .chain(attempt.nonmana_payment.emerge_or_offering_sacrifice)
        .collect::<Vec<_>>();
    let mut moves = Vec::<(ObjectRef, ObjectRef, PlayerId, CardZone, bool)>::new();
    for object in &sacrifices {
        let permanent = state
            .permanents
            .get(object)
            .ok_or(CastChoiceRuntimeError::InvalidPermanentPayment(*object))?;
        let destination = object.next_incarnation()?;
        ensure_zone_ref_unused(destination, state)?;
        moves.push((
            *object,
            destination,
            permanent.owner,
            CardZone::Graveyard,
            false,
        ));
    }
    for object in &attempt.nonmana_payment.returned_permanents {
        let permanent = state
            .permanents
            .get(object)
            .ok_or(CastChoiceRuntimeError::InvalidPermanentPayment(*object))?;
        let destination = object.next_incarnation()?;
        ensure_zone_ref_unused(destination, state)?;
        moves.push((*object, destination, permanent.owner, CardZone::Hand, false));
    }
    for object in &attempt.nonmana_payment.discarded_cards {
        let card = state
            .cards
            .get(object)
            .ok_or(CastChoiceRuntimeError::InvalidCardPayment(*object))?;
        let destination = object.next_incarnation()?;
        ensure_zone_ref_unused(destination, state)?;
        moves.push((*object, destination, card.owner, CardZone::Graveyard, true));
    }
    for object in &attempt.nonmana_payment.exiled_graveyard_cards {
        let card = state
            .cards
            .get(object)
            .ok_or(CastChoiceRuntimeError::InvalidCardPayment(*object))?;
        let destination = object.next_incarnation()?;
        ensure_zone_ref_unused(destination, state)?;
        moves.push((*object, destination, card.owner, CardZone::Exile, false));
    }
    let destination_refs = moves
        .iter()
        .map(|(_, destination, _, _, _)| *destination)
        .collect::<BTreeSet<_>>();
    if destination_refs.len() != moves.len() {
        return Err(CastChoiceRuntimeError::ZoneObjectCollision(
            moves
                .first()
                .map(|(_, destination, _, _, _)| *destination)
                .expect("duplicate move requires a move"),
        ));
    }
    if let Some((_, _, owner, _, _)) = moves
        .iter()
        .find(|(_, _, owner, _, _)| !state.players.contains_key(owner))
    {
        return Err(CastChoiceRuntimeError::MissingPlayer(*owner));
    }

    for object in &attempt.nonmana_payment.tapped_permanents {
        state
            .permanents
            .get_mut(object)
            .expect("tap payments were validated")
            .tapped = true;
    }
    for (old, new, owner, destination, was_discarded) in moves {
        if state.permanents.remove(&old).is_none() {
            state.cards.remove(&old);
            remove_zone_ref(old, state);
        } else {
            state.cards.remove(&old);
        }
        state.cards.insert(
            new,
            CardObjectState {
                object_ref: new,
                owner,
                controller: owner,
                zone: destination,
                discarded_this_turn: match destination {
                    CardZone::Graveyard => Some(was_discarded),
                    _ => None,
                },
            },
        );
        let player = state
            .players
            .get_mut(&owner)
            .ok_or(CastChoiceRuntimeError::MissingPlayer(owner))?;
        match destination {
            CardZone::Hand => player.hand.push(new),
            CardZone::Graveyard => player.graveyard.push(new),
            CardZone::Exile => player.exile.push(new),
            _ => {}
        }
    }
    Ok(())
}

fn apply_mana_payment(
    caster: PlayerId,
    units: &[ManaUnit],
    phyrexian_life_payment: u32,
    state: &mut CastRuntimeState,
) -> Result<(), CastChoiceRuntimeError> {
    for unit in units {
        let player = state
            .players
            .get_mut(&unit.controller)
            .ok_or(CastChoiceRuntimeError::MissingPlayer(unit.controller))?;
        player.mana_pool.remove(&unit.unit_id).ok_or(
            CastChoiceRuntimeError::InvalidManaPayment("mana unit disappeared before payment"),
        )?;
    }
    let life = i32::try_from(phyrexian_life_payment).map_err(|_| {
        CastChoiceRuntimeError::ArithmeticOverflow("converting Phyrexian life payment")
    })?;
    let player = state
        .players
        .get_mut(&caster)
        .ok_or(CastChoiceRuntimeError::MissingPlayer(caster))?;
    player.life =
        player
            .life
            .checked_sub(life)
            .ok_or(CastChoiceRuntimeError::ArithmeticOverflow(
                "paying life for Phyrexian mana",
            ))?;
    Ok(())
}

fn move_source_to_stack(
    attempt: &CastAttempt,
    stack_object: ObjectRef,
    state: &mut CastRuntimeState,
) -> Result<(), CastChoiceRuntimeError> {
    let old = state
        .cards
        .remove(&attempt.source)
        .ok_or(CastChoiceRuntimeError::MissingObject(attempt.source))?;
    remove_zone_ref(attempt.source, state);
    state.cards.insert(
        stack_object,
        CardObjectState {
            object_ref: stack_object,
            owner: old.owner,
            controller: attempt.controller,
            zone: CardZone::Stack,
            discarded_this_turn: None,
        },
    );
    Ok(())
}

fn execute_mayhem_land_play(
    attempt: &CastAttempt,
    state: &mut CastRuntimeState,
) -> Result<CastExecution, CastChoiceRuntimeError> {
    if attempt.source_zone != CardZone::Graveyard
        || !attempt.normal_timing_permission
        || attempt.another_alternative_method_selected
    {
        return Err(CastChoiceRuntimeError::TimingPermissionDenied);
    }
    let card = state
        .cards
        .get(&attempt.source)
        .ok_or(CastChoiceRuntimeError::MissingObject(attempt.source))?;
    if card.discarded_this_turn != Some(true) {
        return Err(CastChoiceRuntimeError::MayhemDiscardConditionNotMet);
    }
    let land_plays_remaining = state
        .players
        .get(&attempt.controller)
        .ok_or(CastChoiceRuntimeError::MissingPlayer(attempt.controller))?
        .land_plays_remaining;
    if !state.players[&attempt.controller].in_game {
        return Err(CastChoiceRuntimeError::MissingPlayer(attempt.controller));
    }
    if land_plays_remaining == 0 {
        return Err(CastChoiceRuntimeError::LandPlayUnavailable);
    }
    let battlefield_object = attempt.source.next_incarnation()?;
    ensure_zone_ref_unused(battlefield_object, state)?;
    state
        .players
        .get_mut(&attempt.controller)
        .ok_or(CastChoiceRuntimeError::MissingPlayer(attempt.controller))?
        .land_plays_remaining -= 1;
    let old = state
        .cards
        .remove(&attempt.source)
        .ok_or(CastChoiceRuntimeError::MissingObject(attempt.source))?;
    remove_zone_ref(attempt.source, state);
    state.cards.insert(
        battlefield_object,
        CardObjectState {
            object_ref: battlefield_object,
            owner: old.owner,
            controller: attempt.controller,
            zone: CardZone::Battlefield,
            discarded_this_turn: None,
        },
    );
    Ok(CastExecution::LandPlayed {
        source_before_play: attempt.source,
        battlefield_object,
        controller: attempt.controller,
    })
}

fn remove_zone_ref(object: ObjectRef, state: &mut CastRuntimeState) {
    for player in state.players.values_mut() {
        player.hand.retain(|candidate| *candidate != object);
        player.graveyard.retain(|candidate| *candidate != object);
        player.exile.retain(|candidate| *candidate != object);
    }
}

fn ensure_zone_ref_unused(
    object: ObjectRef,
    state: &CastRuntimeState,
) -> Result<(), CastChoiceRuntimeError> {
    if state.cards.contains_key(&object) || state.permanents.contains_key(&object) {
        Err(CastChoiceRuntimeError::ZoneObjectCollision(object))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CasualtyCopyTrigger {
    pub trigger_id: TriggerId,
    pub program_digest: String,
    pub original_spell: ObjectRef,
    pub controller: PlayerId,
    pub copied_x: Option<u32>,
    pub copied_modes: Vec<u32>,
    pub copied_targets: Vec<ObjectRef>,
    pub copied_characteristics: SpellCharacteristicsReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCopyReceipt {
    pub copy_object: ObjectRef,
    pub original_spell: ObjectRef,
    pub controller: PlayerId,
    pub x_value: Option<u32>,
    pub chosen_modes: Vec<u32>,
    pub targets: Vec<ObjectRef>,
    pub characteristics: SpellCharacteristicsReceipt,
    pub costs_were_not_paid_for_copy: bool,
}

pub fn create_casualty_copy_trigger(
    program: &CastChoiceKeywordProgram,
    receipt: &CastReceipt,
    trigger_id: TriggerId,
) -> Result<Option<CasualtyCopyTrigger>, CastChoiceRuntimeError> {
    if receipt.program_digest != program.semantic_digest()
        || receipt.family != CastChoiceKeywordFamily::Casualty
        || !matches!(program.kind(), CastChoiceKeywordKind::Casualty { .. })
    {
        return Err(CastChoiceRuntimeError::ProgramReceiptMismatch);
    }
    if !receipt.linked.casualty_paid {
        return Ok(None);
    }
    Ok(Some(CasualtyCopyTrigger {
        trigger_id,
        program_digest: receipt.program_digest.clone(),
        original_spell: receipt.stack_object,
        controller: receipt.controller,
        copied_x: receipt.x_value,
        copied_modes: receipt.chosen_modes.clone(),
        copied_targets: receipt.targets.clone(),
        copied_characteristics: receipt.characteristics.clone(),
    }))
}

pub fn resolve_casualty_copy_trigger(
    program: &CastChoiceKeywordProgram,
    trigger: CasualtyCopyTrigger,
    copy_object: ObjectRef,
    new_targets: Option<Vec<ObjectRef>>,
) -> Result<SpellCopyReceipt, CastChoiceRuntimeError> {
    let CastChoiceKeywordKind::Casualty {
        reminder_mentions_new_targets,
        ..
    } = program.kind()
    else {
        return Err(CastChoiceRuntimeError::WrongKeywordFamily);
    };
    if trigger.program_digest != program.semantic_digest() {
        return Err(CastChoiceRuntimeError::ProgramReceiptMismatch);
    }
    let targets = match (reminder_mentions_new_targets, new_targets) {
        (true, Some(targets)) if targets.len() == trigger.copied_targets.len() => targets,
        (true, None) => trigger.copied_targets,
        (false, None) => trigger.copied_targets,
        _ => return Err(CastChoiceRuntimeError::InvalidTargetChoice),
    };
    Ok(SpellCopyReceipt {
        copy_object,
        original_spell: trigger.original_spell,
        controller: trigger.controller,
        x_value: trigger.copied_x,
        chosen_modes: trigger.copied_modes,
        targets,
        characteristics: trigger.copied_characteristics,
        costs_were_not_paid_for_copy: true,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermanentEntryEvidence {
    pub permanent: ObjectRef,
    pub controller: PlayerId,
    pub came_from_resolving_stack_object: ObjectRef,
    pub current_copiable_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwakenTargetResolutionEvidence {
    pub target: ObjectRef,
    pub controller: Option<PlayerId>,
    pub is_land: Option<bool>,
    pub remains_a_legal_target: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedEntryEffect {
    SquadTokens {
        count: u32,
        copy_digest: String,
    },
    OffspringToken {
        power: i32,
        toughness: i32,
        copy_digest: String,
    },
    ImpendingState {
        time_counters: u32,
        is_creature_while_counters_remain: bool,
    },
    AwakenLand {
        target: ObjectRef,
        counters: u32,
        base_power: i32,
        base_toughness: i32,
        gains_elemental: bool,
        gains_haste: bool,
        remains_land: bool,
    },
}

pub fn resolve_entry_linked_keyword_effects(
    program: &CastChoiceKeywordProgram,
    receipt: &CastReceipt,
    entry: Option<&PermanentEntryEvidence>,
) -> Result<Vec<LinkedEntryEffect>, CastChoiceRuntimeError> {
    if receipt.program_digest != program.semantic_digest()
        || receipt.family != program.kind().family()
    {
        return Err(CastChoiceRuntimeError::ProgramReceiptMismatch);
    }
    let mut effects = Vec::new();
    match program.kind() {
        CastChoiceKeywordKind::Squad { .. } if receipt.linked.squad_times_paid > 0 => {
            let entry = validate_entry(receipt, entry)?;
            effects.push(LinkedEntryEffect::SquadTokens {
                count: receipt.linked.squad_times_paid,
                copy_digest: entry.current_copiable_digest.clone(),
            });
        }
        CastChoiceKeywordKind::Offspring { .. } if receipt.linked.offspring_paid => {
            let entry = validate_entry(receipt, entry)?;
            effects.push(LinkedEntryEffect::OffspringToken {
                power: 1,
                toughness: 1,
                copy_digest: entry.current_copiable_digest.clone(),
            });
        }
        CastChoiceKeywordKind::Impending { .. } if receipt.linked.impending_paid => {
            validate_entry(receipt, entry)?;
            effects.push(LinkedEntryEffect::ImpendingState {
                time_counters: receipt.linked.impending_time_counters,
                is_creature_while_counters_remain: false,
            });
        }
        _ => {}
    }
    Ok(effects)
}

pub fn resolve_awaken_keyword_effect(
    program: &CastChoiceKeywordProgram,
    receipt: &CastReceipt,
    evidence: Option<&AwakenTargetResolutionEvidence>,
) -> Result<Option<LinkedEntryEffect>, CastChoiceRuntimeError> {
    let CastChoiceKeywordKind::Awaken { counters, .. } = program.kind() else {
        return Err(CastChoiceRuntimeError::WrongKeywordFamily);
    };
    if receipt.program_digest != program.semantic_digest()
        || receipt.family != CastChoiceKeywordFamily::Awaken
    {
        return Err(CastChoiceRuntimeError::ProgramReceiptMismatch);
    }
    if !receipt.linked.awaken_paid {
        return Ok(None);
    }
    let target = receipt
        .linked
        .awaken_target
        .ok_or(CastChoiceRuntimeError::MissingChoice("awaken target"))?;
    let evidence = evidence.ok_or(CastChoiceRuntimeError::IncompleteEvidence(
        "awaken resolution target",
    ))?;
    if evidence.target != target {
        return Err(CastChoiceRuntimeError::ProgramReceiptMismatch);
    }
    let controller = evidence
        .controller
        .ok_or(CastChoiceRuntimeError::IncompleteEvidence(
            "awaken target controller",
        ))?;
    let is_land = evidence
        .is_land
        .ok_or(CastChoiceRuntimeError::IncompleteEvidence(
            "awaken target land type",
        ))?;
    let remains_legal =
        evidence
            .remains_a_legal_target
            .ok_or(CastChoiceRuntimeError::IncompleteEvidence(
                "awaken target legality",
            ))?;
    if controller != receipt.controller || !is_land || !remains_legal {
        return Ok(None);
    }
    Ok(Some(LinkedEntryEffect::AwakenLand {
        target,
        counters: *counters,
        base_power: 0,
        base_toughness: 0,
        gains_elemental: true,
        gains_haste: true,
        remains_land: true,
    }))
}

fn validate_entry<'a>(
    receipt: &CastReceipt,
    entry: Option<&'a PermanentEntryEvidence>,
) -> Result<&'a PermanentEntryEvidence, CastChoiceRuntimeError> {
    let entry = entry.ok_or(CastChoiceRuntimeError::IncompleteEvidence(
        "linked permanent entry",
    ))?;
    if entry.controller != receipt.controller
        || entry.came_from_resolving_stack_object != receipt.stack_object
    {
        return Err(CastChoiceRuntimeError::ProgramReceiptMismatch);
    }
    Ok(entry)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpendingPermanentState {
    pub permanent: ObjectRef,
    pub controller: PlayerId,
    pub time_counters: u32,
    pub impending_cost_was_paid: bool,
    pub creature_type_suppressed_by_impending: bool,
}

pub fn refresh_impending_static_effect(
    program: &CastChoiceKeywordProgram,
    permanent: &mut ImpendingPermanentState,
) -> Result<bool, CastChoiceRuntimeError> {
    if !matches!(program.kind(), CastChoiceKeywordKind::Impending { .. }) {
        return Err(CastChoiceRuntimeError::WrongKeywordFamily);
    }
    permanent.creature_type_suppressed_by_impending =
        permanent.impending_cost_was_paid && permanent.time_counters > 0;
    Ok(permanent.creature_type_suppressed_by_impending)
}

pub fn impending_end_step_trigger(
    program: &CastChoiceKeywordProgram,
    permanent: &mut ImpendingPermanentState,
    active_player: PlayerId,
) -> Result<bool, CastChoiceRuntimeError> {
    if !matches!(program.kind(), CastChoiceKeywordKind::Impending { .. }) {
        return Err(CastChoiceRuntimeError::WrongKeywordFamily);
    }
    if !permanent.impending_cost_was_paid
        || permanent.controller != active_player
        || permanent.time_counters == 0
    {
        return Ok(false);
    }
    permanent.time_counters -= 1;
    refresh_impending_static_effect(program, permanent)?;
    Ok(true)
}
