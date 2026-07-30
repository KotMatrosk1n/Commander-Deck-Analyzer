//! Content and semantic-context keyed programs for a bounded creature-entry
//! and +1/+1-counter keyword batch.
//!
//! Existing entry-choice owners retain Bloodthirst, Riot, and Unleash.
//! Existing official-keyword owners retain canonical Evolve and canonical
//! Renown 1. This module accepts only complete unowned clauses and is not
//! connected to the production simulator.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const CREATURE_COUNTER_COMPILER_VERSION: &str = "creature-counter-compiler-0.1";
pub const CREATURE_COUNTER_RUNTIME_VERSION: &str = "creature-counter-runtime-0.1";
pub const CREATURE_COUNTER_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:107.3,115,117,118,122,400.7,602,603,608,614,616,701.37,701.39,701.46,702.38,702.43,702.44,702.54,702.58,702.82,702.100,702.104,702.112,702.123";

const EVOLVE_CANONICAL: &str = "Evolve (Whenever a creature you control enters, if that creature has greater power or toughness than this creature, put a +1/+1 counter on this creature.)";
const RENOWN_ONE_CANONICAL: &str = "Renown 1 (When this creature deals combat damage to a player, if it isn't renowned, put a +1/+1 counter on it and it becomes renowned.)";
const UNLEASH_CANONICAL: &str = "Unleash (You may have this creature enter with a +1/+1 counter on it. It can't block as long as it has a +1/+1 counter on it.)";
const RIOT_CANONICAL: &str =
    "Riot (This creature enters with your choice of a +1/+1 counter or haste.)";
const RIOT_ADDITIONAL_CANONICAL: &str =
    "Riot (This creature enters with your choice of an additional +1/+1 counter or haste.)";
const BLOODTHIRST_X_CANONICAL: &str = "Bloodthirst X (This creature enters with X +1/+1 counters on it, where X is the damage dealt to your opponents this turn.)";

pub const fn creature_counter_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
}

impl CardType {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Artifact => "artifact",
            Self::Battle => "battle",
            Self::Creature => "creature",
            Self::Enchantment => "enchantment",
            Self::Instant => "instant",
            Self::Kindred => "kindred",
            Self::Land => "land",
            Self::Planeswalker => "planeswalker",
            Self::Sorcery => "sorcery",
        }
    }

    fn is_permanent(self) -> bool {
        matches!(
            self,
            Self::Artifact
                | Self::Battle
                | Self::Creature
                | Self::Enchantment
                | Self::Land
                | Self::Planeswalker
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSemanticContext {
    pub card_types: BTreeSet<CardType>,
    pub subtypes: BTreeSet<String>,
}

impl SourceSemanticContext {
    pub fn from_type_line(type_line: &str) -> Option<Self> {
        if type_line.is_empty()
            || type_line.trim() != type_line
            || collapse_whitespace(type_line) != type_line
        {
            return None;
        }
        let (head, tail) = type_line
            .split_once(" \u{2014} ")
            .or_else(|| type_line.split_once(" \u{fffd} "))
            .or_else(|| type_line.split_once(" - "))
            .map_or((type_line, ""), |parts| parts);
        let mut card_types = BTreeSet::new();
        for word in head.split_ascii_whitespace() {
            if let Some(card_type) = match word {
                "Artifact" => Some(CardType::Artifact),
                "Battle" => Some(CardType::Battle),
                "Creature" => Some(CardType::Creature),
                "Enchantment" => Some(CardType::Enchantment),
                "Instant" => Some(CardType::Instant),
                "Kindred" | "Tribal" => Some(CardType::Kindred),
                "Land" => Some(CardType::Land),
                "Planeswalker" => Some(CardType::Planeswalker),
                "Sorcery" => Some(CardType::Sorcery),
                _ => None,
            } {
                card_types.insert(card_type);
            }
        }
        if card_types.is_empty() {
            return None;
        }
        let subtypes = tail
            .split_ascii_whitespace()
            .map(normalize_subtype)
            .collect::<Option<BTreeSet<_>>>()?;
        Some(Self {
            card_types,
            subtypes,
        })
    }

    fn is_creature(&self) -> bool {
        self.card_types.contains(&CardType::Creature)
    }

    fn is_artifact_permanent(&self) -> bool {
        self.card_types.contains(&CardType::Artifact)
            && self
                .card_types
                .iter()
                .any(|card_type| card_type.is_permanent())
    }

    fn is_permanent(&self) -> bool {
        self.card_types
            .iter()
            .any(|card_type| card_type.is_permanent())
    }

    fn stable_id(&self) -> String {
        format!(
            "types={};subtypes={}",
            self.card_types
                .iter()
                .map(|card_type| card_type.stable_id())
                .collect::<Vec<_>>()
                .join(","),
            self.subtypes
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

fn normalize_subtype(source: &str) -> Option<String> {
    if source.is_empty()
        || !source
            .chars()
            .all(|character| character.is_alphabetic() || character == '-')
    {
        return None;
    }
    Some(source.to_ascii_lowercase())
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

    fn is_colored(self) -> bool {
        self != Self::Colorless
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
            Self::Generic(value) => format!("generic/{value}"),
            Self::White => "w".into(),
            Self::Blue => "u".into(),
            Self::Black => "b".into(),
            Self::Red => "r".into(),
            Self::Green => "g".into(),
            Self::Colorless => "c".into(),
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

    fn contains_x(&self) -> bool {
        self.symbols.contains(&ManaSymbol::VariableX)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterAmount {
    Fixed(u32),
    BoundX,
    Sunburst,
}

impl CounterAmount {
    fn stable_id(self) -> String {
        match self {
            Self::Fixed(amount) => format!("fixed/{amount}"),
            Self::BoundX => "bound-x".into(),
            Self::Sunburst => "colors-spent-to-cast".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevourQuality {
    Creature,
    CardType(CardType),
    Subtype(String),
}

impl DevourQuality {
    fn stable_id(&self) -> String {
        match self {
            Self::Creature => "creature".into(),
            Self::CardType(card_type) => format!("card-type/{}", card_type.stable_id()),
            Self::Subtype(subtype) => format!("subtype/{subtype}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreatureCounterKeywordKind {
    Adapt {
        activation_cost: ManaCost,
        counters: u32,
    },
    Amplify {
        counters_per_revealed_card: u32,
        matching_creature_types: BTreeSet<String>,
    },
    Bolster {
        counters: u32,
    },
    Devour {
        counters_per_sacrifice: u32,
        quality: DevourQuality,
    },
    ResidualEvolve,
    Fabricate {
        counters_or_tokens: u32,
    },
    Graft {
        counters: u32,
    },
    Modular {
        counters: CounterAmount,
    },
    Monstrosity {
        activation_cost: ManaCost,
        counters: CounterAmount,
    },
    ResidualRenown {
        counters: u32,
    },
    Tribute {
        counters: u32,
    },
}

impl CreatureCounterKeywordKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Adapt { .. } => "Adapt",
            Self::Amplify { .. } => "Amplify",
            Self::Bolster { .. } => "Bolster",
            Self::Devour { .. } => "Devour",
            Self::ResidualEvolve => "Evolve",
            Self::Fabricate { .. } => "Fabricate",
            Self::Graft { .. } => "Graft",
            Self::Modular { .. } => "Modular",
            Self::Monstrosity { .. } => "Monstrosity",
            Self::ResidualRenown { .. } => "Renown",
            Self::Tribute { .. } => "Tribute",
        }
    }

    fn stable_id(&self) -> String {
        match self {
            Self::Adapt {
                activation_cost,
                counters,
            } => format!(
                "adapt/v1;cost={};resolution=if-no-plus-one-counters-put/{counters};stack=true;targets=false",
                activation_cost.stable_id()
            ),
            Self::Amplify {
                counters_per_revealed_card,
                matching_creature_types,
            } => format!(
                "amplify/v1;per-card={counters_per_revealed_card};types={};entry-replacement=true;reveal=hand-sharing-type;exclude-self-and-simultaneous-entry",
                matching_creature_types.iter().map(String::as_str).collect::<Vec<_>>().join(",")
            ),
            Self::Bolster { counters } => format!(
                "bolster/v1;counters={counters};choice=controlled-creature-tied-for-least-effective-toughness;targets=false"
            ),
            Self::Devour {
                counters_per_sacrifice,
                quality,
            } => format!(
                "devour/v1;per-sacrifice={counters_per_sacrifice};quality={};entry-replacement=true;choice=any-number-controlled-permanents",
                quality.stable_id()
            ),
            Self::ResidualEvolve => "residual-evolve/v1;trigger=controlled-creature-entry;intervening-comparison=effective-power-or-toughness;resolution-counter=1;targets=false".into(),
            Self::Fabricate { counters_or_tokens } => format!(
                "fabricate/v1;trigger=source-entry;resolution-choice=put/{counters_or_tokens}-counters-on-source-or-create/{counters_or_tokens}-servo-tokens;targets=false"
            ),
            Self::Graft { counters } => format!(
                "graft/v1;entry-counters={counters};trigger=another-creature-entry-if-source-has-counter;resolution=optional-move-one-counter;targets=false"
            ),
            Self::Modular { counters } => format!(
                "modular/v1;entry-counters={};death-trigger=optional-put-lki-plus-one-counter-count-on-target-artifact-creature",
                counters.stable_id()
            ),
            Self::Monstrosity {
                activation_cost,
                counters,
            } => format!(
                "monstrosity/v1;cost={};amount={};resolution=if-not-monstrous-put-counters-and-designate;stack=true;targets=false",
                activation_cost.stable_id(),
                counters.stable_id()
            ),
            Self::ResidualRenown { counters } => format!(
                "residual-renown/v1;counters={counters};trigger=combat-damage-to-player;intervening-if=not-renowned;designation=incarnation"
            ),
            Self::Tribute { counters } => format!(
                "tribute/v1;counters={counters};entry-replacement=true;choose-opponent=controller;counter-choice=chosen-opponent;paid=actual-specified-counters-entered"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatureCounterKeywordProgram {
    exact_source: String,
    source_context: SourceSemanticContext,
    kind: CreatureCounterKeywordKind,
    semantic_digest: String,
}

impl CreatureCounterKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn source_context(&self) -> &SourceSemanticContext {
        &self.source_context
    }

    pub fn kind(&self) -> &CreatureCounterKeywordKind {
        &self.kind
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub const fn production_adapter_connected(&self) -> bool {
        creature_counter_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotCandidateClass {
    SupportedProgram,
    ExistingEntryChoiceOwner,
    ExistingOfficialKeywordOwner,
    DeliberateExclusion,
}

pub fn classify_creature_counter_snapshot_candidate(
    exact_source: &str,
    type_line: &str,
) -> Option<SnapshotCandidateClass> {
    candidate_family(exact_source)?;
    let Some(context) = SourceSemanticContext::from_type_line(type_line) else {
        return Some(SnapshotCandidateClass::DeliberateExclusion);
    };
    if existing_entry_choice_owner(exact_source, &context) {
        return Some(SnapshotCandidateClass::ExistingEntryChoiceOwner);
    }
    if existing_official_owner(exact_source, &context) {
        return Some(SnapshotCandidateClass::ExistingOfficialKeywordOwner);
    }
    if compile_creature_counter_keyword_program(exact_source, type_line).is_some() {
        return Some(SnapshotCandidateClass::SupportedProgram);
    }
    Some(SnapshotCandidateClass::DeliberateExclusion)
}

pub fn compile_creature_counter_keyword_program(
    exact_source: &str,
    type_line: &str,
) -> Option<CreatureCounterKeywordProgram> {
    if exact_source.is_empty()
        || exact_source.trim() != exact_source
        || exact_source.contains(['\r', '\n'])
        || collapse_whitespace(exact_source) != exact_source
    {
        return None;
    }
    let source_context = SourceSemanticContext::from_type_line(type_line)?;
    let kind = parse_adapt(exact_source, &source_context)
        .or_else(|| parse_amplify(exact_source, &source_context))
        .or_else(|| parse_bolster(exact_source))
        .or_else(|| parse_devour(exact_source, &source_context))
        .or_else(|| parse_residual_evolve(exact_source, &source_context))
        .or_else(|| parse_fabricate(exact_source, &source_context))
        .or_else(|| parse_graft(exact_source, &source_context))
        .or_else(|| parse_modular(exact_source, &source_context))
        .or_else(|| parse_monstrosity(exact_source, &source_context))
        .or_else(|| parse_residual_renown(exact_source, &source_context))
        .or_else(|| parse_tribute(exact_source, &source_context))?;
    let semantic_digest = creature_counter_semantic_digest(exact_source, &source_context, &kind);
    Some(CreatureCounterKeywordProgram {
        exact_source: exact_source.to_owned(),
        source_context,
        kind,
        semantic_digest,
    })
}

fn existing_entry_choice_owner(source: &str, context: &SourceSemanticContext) -> bool {
    if !context.is_creature() {
        return false;
    }
    source == UNLEASH_CANONICAL
        || source == RIOT_CANONICAL
        || source == RIOT_ADDITIONAL_CANONICAL
        || source == BLOODTHIRST_X_CANONICAL
        || parse_fixed_bloodthirst_owner(source).is_some()
}

fn parse_fixed_bloodthirst_owner(source: &str) -> Option<u32> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let amount = core.strip_prefix("Bloodthirst ")?.parse::<u32>().ok()?;
    if amount == 0 {
        return None;
    }
    let counter_words = count_words(amount)?;
    let expected = if amount == 1 {
        "If an opponent was dealt damage this turn, this creature enters with a +1/+1 counter on it.".to_owned()
    } else {
        format!(
            "If an opponent was dealt damage this turn, this creature enters with {counter_words} +1/+1 counters on it."
        )
    };
    (reminder == Some(expected.as_str())).then_some(amount)
}

fn existing_official_owner(source: &str, context: &SourceSemanticContext) -> bool {
    context.is_creature() && (source == EVOLVE_CANONICAL || source == RENOWN_ONE_CANONICAL)
}

fn parse_adapt(
    source: &str,
    context: &SourceSemanticContext,
) -> Option<CreatureCounterKeywordKind> {
    if !context.is_creature() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let (cost, action) = core.split_once(": Adapt ")?;
    let counters = action.strip_suffix('.')?.parse::<u32>().ok()?;
    if counters == 0 {
        return None;
    }
    if let Some(reminder) = reminder {
        let expected = adapt_reminder(counters)?;
        if reminder != expected {
            return None;
        }
    }
    Some(CreatureCounterKeywordKind::Adapt {
        activation_cost: parse_mana_cost(cost)?,
        counters,
    })
}

fn adapt_reminder(counters: u32) -> Option<String> {
    if counters == 1 {
        Some("If this creature has no +1/+1 counters on it, put a +1/+1 counter on it.".to_owned())
    } else {
        Some(format!(
            "If this creature has no +1/+1 counters on it, put {} +1/+1 counters on it.",
            count_words(counters)?
        ))
    }
}

fn parse_amplify(
    source: &str,
    context: &SourceSemanticContext,
) -> Option<CreatureCounterKeywordKind> {
    if !context.is_creature() || context.subtypes.is_empty() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let counters = core.strip_prefix("Amplify ")?.parse::<u32>().ok()?;
    if counters == 0 {
        return None;
    }
    let reminder = reminder?;
    let prefix = "As this creature enters, put ";
    let rest = reminder.strip_prefix(prefix)?;
    let counter_phrase = if counters == 1 {
        "a +1/+1 counter on it for each "
    } else {
        return parse_amplify_plural(counters, rest, context);
    };
    let types = rest
        .strip_prefix(counter_phrase)?
        .strip_suffix(" card you reveal in your hand.")?
        .split(" and/or ")
        .map(normalize_subtype)
        .collect::<Option<BTreeSet<_>>>()?;
    (types == context.subtypes).then_some(CreatureCounterKeywordKind::Amplify {
        counters_per_revealed_card: counters,
        matching_creature_types: types,
    })
}

fn parse_amplify_plural(
    counters: u32,
    reminder_rest: &str,
    context: &SourceSemanticContext,
) -> Option<CreatureCounterKeywordKind> {
    let prefix = format!("{} +1/+1 counters on it for each ", count_words(counters)?);
    let types = reminder_rest
        .strip_prefix(&prefix)?
        .strip_suffix(" card you reveal in your hand.")?
        .split(" and/or ")
        .map(normalize_subtype)
        .collect::<Option<BTreeSet<_>>>()?;
    (types == context.subtypes).then_some(CreatureCounterKeywordKind::Amplify {
        counters_per_revealed_card: counters,
        matching_creature_types: types,
    })
}

fn parse_bolster(source: &str) -> Option<CreatureCounterKeywordKind> {
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let counters = core
        .strip_prefix("Bolster ")?
        .strip_suffix('.')?
        .parse::<u32>()
        .ok()?;
    if counters == 0 {
        return None;
    }
    let expected = if counters == 1 {
        "Choose a creature with the least toughness among creatures you control and put a +1/+1 counter on it.".to_owned()
    } else {
        format!(
            "Choose a creature with the least toughness among creatures you control and put {} +1/+1 counters on it.",
            count_words(counters)?
        )
    };
    (reminder == Some(expected.as_str()))
        .then_some(CreatureCounterKeywordKind::Bolster { counters })
}

fn parse_devour(
    source: &str,
    context: &SourceSemanticContext,
) -> Option<CreatureCounterKeywordKind> {
    if !context.is_creature() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let headline = core.strip_prefix("Devour ")?;
    let (quality, counters) = if let Ok(counters) = headline.parse::<u32>() {
        (DevourQuality::Creature, counters)
    } else {
        let (quality, counters) = headline.rsplit_once(' ')?;
        let quality = match quality {
            "artifact" => DevourQuality::CardType(CardType::Artifact),
            "land" => DevourQuality::CardType(CardType::Land),
            "Food" => DevourQuality::Subtype("food".into()),
            _ => return None,
        };
        (quality, counters.parse::<u32>().ok()?)
    };
    if counters == 0 {
        return None;
    }
    let reminder = reminder?;
    if !devour_reminder_matches(reminder, counters, &quality) {
        return None;
    }
    Some(CreatureCounterKeywordKind::Devour {
        counters_per_sacrifice: counters,
        quality,
    })
}

fn devour_reminder_matches(reminder: &str, counters: u32, quality: &DevourQuality) -> bool {
    let sacrificed = match quality {
        DevourQuality::Creature => "creatures",
        DevourQuality::CardType(CardType::Artifact) => "artifacts",
        DevourQuality::CardType(CardType::Land) => "lands",
        DevourQuality::Subtype(subtype) if subtype == "food" => "Foods",
        _ => return false,
    };
    let prefix = format!("As this creature enters, you may sacrifice any number of {sacrificed}. ");
    let Some(rest) = reminder.strip_prefix(&prefix) else {
        return false;
    };
    let rest = rest
        .strip_prefix("It enters with ")
        .or_else(|| rest.strip_prefix("This creature enters with "));
    let Some(rest) = rest else {
        return false;
    };
    let expected = match counters {
        1 => "that many +1/+1 counters on it.".to_owned(),
        2 => "twice that many +1/+1 counters on it.".to_owned(),
        amount => format!(
            "{} times that many +1/+1 counters on it.",
            count_words(amount).unwrap_or_default()
        ),
    };
    rest == expected
}

fn parse_residual_evolve(
    source: &str,
    context: &SourceSemanticContext,
) -> Option<CreatureCounterKeywordKind> {
    (context.is_creature() && source == "Evolve")
        .then_some(CreatureCounterKeywordKind::ResidualEvolve)
}

fn parse_fabricate(
    source: &str,
    context: &SourceSemanticContext,
) -> Option<CreatureCounterKeywordKind> {
    if !context.is_creature() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let amount = core.strip_prefix("Fabricate ")?.parse::<u32>().ok()?;
    if amount == 0 {
        return None;
    }
    let expected = if amount == 1 {
        "When this creature enters, put a +1/+1 counter on it or create a 1/1 colorless Servo artifact creature token.".to_owned()
    } else {
        format!(
            "When this creature enters, put {} +1/+1 counters on it or create {} 1/1 colorless Servo artifact creature tokens.",
            count_words(amount)?,
            count_words(amount)?
        )
    };
    (reminder == Some(expected.as_str())).then_some(CreatureCounterKeywordKind::Fabricate {
        counters_or_tokens: amount,
    })
}

fn parse_graft(
    source: &str,
    context: &SourceSemanticContext,
) -> Option<CreatureCounterKeywordKind> {
    if !context.is_permanent() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let amount = core.strip_prefix("Graft ")?.parse::<u32>().ok()?;
    if amount == 0 {
        return None;
    }
    let subject = if context.is_creature() {
        "creature"
    } else if context.card_types.contains(&CardType::Land) {
        "land"
    } else {
        return None;
    };
    let first = if amount == 1 {
        format!("This {subject} enters with a +1/+1 counter on it.")
    } else {
        format!(
            "This {subject} enters with {} +1/+1 counters on it.",
            count_words(amount)?
        )
    };
    let second = if context.is_creature() {
        " Whenever another creature enters, you may move a +1/+1 counter from this creature onto it."
    } else {
        " Whenever a creature enters, you may move a +1/+1 counter from this land onto it."
    };
    let expected = format!("{first}{second}");
    (reminder == Some(expected.as_str()))
        .then_some(CreatureCounterKeywordKind::Graft { counters: amount })
}

fn parse_modular(
    source: &str,
    context: &SourceSemanticContext,
) -> Option<CreatureCounterKeywordKind> {
    if !context.is_artifact_permanent() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(source)?;
    if strip_keyword_dash(core, "Modular") == Some("Sunburst") {
        let expected = "This creature enters with a +1/+1 counter on it for each color of mana spent to cast it. When it dies, you may put its +1/+1 counters on target artifact creature.";
        return (context.is_creature() && reminder == Some(expected)).then_some(
            CreatureCounterKeywordKind::Modular {
                counters: CounterAmount::Sunburst,
            },
        );
    }
    let amount = core.strip_prefix("Modular ")?.parse::<u32>().ok()?;
    if amount == 0 {
        return None;
    }
    if let Some(reminder) = reminder {
        if !context.is_creature() {
            return None;
        }
        let entry = if amount == 1 {
            "This creature enters with a +1/+1 counter on it.".to_owned()
        } else {
            format!(
                "This creature enters with {} +1/+1 counters on it.",
                count_words(amount)?
            )
        };
        let expected = format!(
            "{entry} When it dies, you may put its +1/+1 counters on target artifact creature."
        );
        if reminder != expected {
            return None;
        }
    }
    Some(CreatureCounterKeywordKind::Modular {
        counters: CounterAmount::Fixed(amount),
    })
}

fn parse_monstrosity(
    source: &str,
    context: &SourceSemanticContext,
) -> Option<CreatureCounterKeywordKind> {
    if !context.is_creature() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let (cost, action) = core.split_once(": Monstrosity ")?;
    let amount_text = action.strip_suffix('.')?;
    let counters = if amount_text == "X" {
        CounterAmount::BoundX
    } else {
        let amount = amount_text.parse::<u32>().ok()?;
        if amount == 0 {
            return None;
        }
        CounterAmount::Fixed(amount)
    };
    let reminder = reminder?;
    let expected = match counters {
        CounterAmount::Fixed(1) => {
            "If this creature isn't monstrous, put a +1/+1 counter on it and it becomes monstrous."
                .to_owned()
        }
        CounterAmount::Fixed(amount) => format!(
            "If this creature isn't monstrous, put {} +1/+1 counters on it and it becomes monstrous.",
            count_words(amount)?
        ),
        CounterAmount::BoundX => {
            "If this creature isn't monstrous, put X +1/+1 counters on it and it becomes monstrous."
                .to_owned()
        }
        CounterAmount::Sunburst => return None,
    };
    if reminder != expected
        && !(matches!(counters, CounterAmount::Fixed(2))
            && reminder
                == "If this creature isn't monstrous, put two +1/+1 counters on it and it becomes monstrous. {S} can be paid with one mana from a snow source.")
    {
        return None;
    }
    Some(CreatureCounterKeywordKind::Monstrosity {
        activation_cost: parse_mana_cost(cost)?,
        counters,
    })
}

fn parse_residual_renown(
    source: &str,
    context: &SourceSemanticContext,
) -> Option<CreatureCounterKeywordKind> {
    if !context.is_creature() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let amount = core.strip_prefix("Renown ")?.parse::<u32>().ok()?;
    if amount <= 1 {
        return None;
    }
    let expected = format!(
        "When this creature deals combat damage to a player, if it isn't renowned, put {} +1/+1 counters on it and it becomes renowned.",
        count_words(amount)?
    );
    (reminder == Some(expected.as_str()))
        .then_some(CreatureCounterKeywordKind::ResidualRenown { counters: amount })
}

fn parse_tribute(
    source: &str,
    context: &SourceSemanticContext,
) -> Option<CreatureCounterKeywordKind> {
    if !context.is_creature() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(source)?;
    let amount = core.strip_prefix("Tribute ")?.parse::<u32>().ok()?;
    if amount == 0 {
        return None;
    }
    let expected = if amount == 1 {
        "As this creature enters, an opponent of your choice may put a +1/+1 counter on it."
            .to_owned()
    } else {
        format!(
            "As this creature enters, an opponent of your choice may put {} +1/+1 counters on it.",
            count_words(amount)?
        )
    };
    (reminder == Some(expected.as_str()))
        .then_some(CreatureCounterKeywordKind::Tribute { counters: amount })
}

fn candidate_family(source: &str) -> Option<&'static str> {
    let lower = source.to_ascii_lowercase();
    for (needle, family) in [
        ("adapt", "Adapt"),
        ("amplify", "Amplify"),
        ("bloodthirst", "Bloodthirst"),
        ("bolster", "Bolster"),
        ("devour", "Devour"),
        ("evolve", "Evolve"),
        ("fabricate", "Fabricate"),
        ("graft", "Graft"),
        ("modular", "Modular"),
        ("monstrosity", "Monstrosity"),
        ("renown", "Renown"),
        ("riot", "Riot"),
        ("tribute", "Tribute"),
        ("unleash", "Unleash"),
    ] {
        if contains_word(&lower, needle) {
            return Some(family);
        }
    }
    None
}

fn contains_word(source: &str, needle: &str) -> bool {
    source.match_indices(needle).any(|(start, _)| {
        let before = source[..start].chars().next_back();
        let after = source[start + needle.len()..].chars().next();
        before.is_none_or(|character| !character.is_alphanumeric())
            && after.is_none_or(|character| !character.is_alphanumeric())
    })
}

fn creature_counter_semantic_digest(
    exact_source: &str,
    context: &SourceSemanticContext,
    kind: &CreatureCounterKeywordKind,
) -> String {
    let context_id = context.stable_id();
    let kind_id = kind.stable_id();
    let mut hasher = Sha256::new();
    for component in [
        "creature-counter-content/v1",
        CREATURE_COUNTER_COMPILER_VERSION,
        CREATURE_COUNTER_RUNTIME_VERSION,
        CREATURE_COUNTER_RULES_CONTEXT_VERSION,
        exact_source,
        &context_id,
        &kind_id,
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    if source.is_empty() || source.trim() != source {
        return None;
    }
    let mut symbols = Vec::new();
    let mut offset = 0_usize;
    while offset < source.len() {
        if source.as_bytes().get(offset).copied()? != b'{' {
            return None;
        }
        let token_start = offset + 1;
        let token_end = token_start + source[token_start..].find('}')?;
        let token = &source[token_start..token_end];
        if token.is_empty() || token.contains('{') || token.trim() != token {
            return None;
        }
        let upper = token.to_ascii_uppercase();
        let symbol = if upper.bytes().all(|byte| byte.is_ascii_digit()) {
            if upper.len() > 1 && upper.starts_with('0') {
                return None;
            }
            ManaSymbol::Generic(upper.parse::<u32>().ok()?)
        } else {
            match upper.as_str() {
                "W" => ManaSymbol::White,
                "U" => ManaSymbol::Blue,
                "B" => ManaSymbol::Black,
                "R" => ManaSymbol::Red,
                "G" => ManaSymbol::Green,
                "C" => ManaSymbol::Colorless,
                "S" => ManaSymbol::Snow,
                "X" => ManaSymbol::VariableX,
                _ => {
                    let (first, second) = upper.split_once('/')?;
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

fn strip_keyword_dash<'a>(source: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = source.strip_prefix(keyword)?;
    rest.strip_prefix('\u{2014}')
        .or_else(|| rest.strip_prefix('\u{fffd}'))
}

fn count_words(value: u32) -> Option<&'static str> {
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

fn collapse_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TeamId(pub u16);

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
pub struct PendingTriggerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModularDeathEventId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplacementEffectId(pub u64);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManaUnit {
    pub id: ManaUnitId,
    pub color: ManaColor,
    pub from_snow_source: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub team: TeamId,
    pub mana_pool: BTreeMap<ManaUnitId, ManaUnit>,
}

impl PlayerState {
    pub fn new(team: TeamId) -> Self {
        Self {
            team,
            mana_pool: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedObject {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub controller: Option<PlayerId>,
    pub zone: Zone,
    pub card_types: BTreeSet<CardType>,
    pub subtypes: BTreeSet<String>,
    pub plus_one_counters: u32,
    pub effective_power: Option<i32>,
    pub effective_toughness: Option<i32>,
    pub is_token: bool,
}

impl TrackedObject {
    pub fn new(
        object_ref: ObjectRef,
        owner: PlayerId,
        zone: Zone,
        card_types: impl IntoIterator<Item = CardType>,
        subtypes: impl IntoIterator<Item = String>,
        printed_power_toughness: Option<(i32, i32)>,
    ) -> Self {
        Self {
            object_ref,
            owner,
            controller: matches!(zone, Zone::Battlefield | Zone::Stack).then_some(owner),
            zone,
            card_types: card_types.into_iter().collect(),
            subtypes: subtypes.into_iter().collect(),
            plus_one_counters: 0,
            effective_power: printed_power_toughness.map(|values| values.0),
            effective_toughness: printed_power_toughness.map(|values| values.1),
            is_token: false,
        }
    }

    fn is_creature(&self) -> bool {
        self.card_types.contains(&CardType::Creature)
    }

    fn is_artifact_creature(&self) -> bool {
        self.card_types.contains(&CardType::Artifact) && self.is_creature()
    }

    fn matches_context(&self, context: &SourceSemanticContext) -> bool {
        self.card_types == context.card_types && self.subtypes == context.subtypes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalZoneReplacement {
    pub effect_id: ReplacementEffectId,
    pub replaces_destination: Option<Zone>,
    pub destination: Zone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementApplicationEvidence {
    pub effect_id: ReplacementEffectId,
    pub destination_before: Zone,
    pub destination_after: Zone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneChangeEvidence {
    pub before: ObjectRef,
    pub after: ObjectRef,
    pub from: Zone,
    pub requested_destination: Zone,
    pub actual_destination: Zone,
    pub replacement_order: Vec<ReplacementEffectId>,
    pub replacements_applied: Vec<ReplacementApplicationEvidence>,
}

/// The caller's complete counter replacement engine supplies the final
/// quantity. The keyword runtime never assumes that the printed quantity is
/// the quantity actually placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterPlacementEvidence {
    pub object: ObjectRef,
    pub requested_counters: u32,
    pub counters_placed: u32,
    pub placement_action_possible: bool,
    pub replacement_effects_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPayment {
    pub x_value: u32,
    pub mana_units: Vec<ManaUnitId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivationWindow {
    pub priority_player: PlayerId,
    pub prohibitions_allow_activation: bool,
    pub external_cost_modifiers_complete: bool,
}

impl ActivationWindow {
    pub const fn fully_legal(player: PlayerId) -> Self {
        Self {
            priority_player: player,
            prohibitions_allow_activation: true,
            external_cost_modifiers_complete: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SacrificeChoice {
    pub permanent: ObjectRef,
    pub destination_replacements: Vec<ExternalZoneReplacement>,
    pub replacement_order: Vec<ReplacementEffectId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKeywordChoice {
    Amplify {
        revealed_cards: Vec<ObjectRef>,
        simultaneously_entering: BTreeSet<ObjectRef>,
        simultaneously_entering_census_complete: bool,
    },
    Devour {
        sacrifices: Vec<SacrificeChoice>,
    },
    Fabricate,
    Graft,
    Modular {
        colors_spent_to_cast: BTreeSet<ManaColor>,
        cast_payment_evidence_complete: bool,
    },
    Tribute {
        chosen_opponent: PlayerId,
        opponent_accepts: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryResolutionInput {
    pub controller: PlayerId,
    pub choice: EntryKeywordChoice,
    pub counter_placement: Option<CounterPlacementEvidence>,
    pub destination_replacements: Vec<ExternalZoneReplacement>,
    pub replacement_order: Vec<ReplacementEffectId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryResolutionEvidence {
    pub source_zone_change: ZoneChangeEvidence,
    pub permanent: Option<ObjectRef>,
    pub requested_counters: u32,
    pub counters_placed: u32,
    pub revealed_cards: Vec<ObjectRef>,
    pub sacrificed_permanents: Vec<ZoneChangeEvidence>,
    pub tribute_paid: Option<bool>,
    pub fabricate_trigger: Option<PendingTriggerId>,
    pub entry_observer_triggers: Vec<PendingTriggerId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FabricatePreference {
    PutCounters,
    CreateServos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FabricateResolution {
    Counters {
        requested: u32,
        placed: u32,
    },
    Servos {
        tokens: Vec<ObjectRef>,
        entry_observer_triggers: Vec<PendingTriggerId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BolsterResolution {
    NoControlledCreature,
    Counters {
        chosen: ObjectRef,
        requested: u32,
        placed: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationEvidence {
    pub trigger_id: PendingTriggerId,
    pub source: ObjectRef,
    pub controller: PlayerId,
    pub x_value: u32,
    pub mana_units_spent: Vec<ManaUnitId>,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CounterAbilityResolution {
    SourceNoLongerThatPermanent,
    ConditionNotMet,
    Applied {
        source: ObjectRef,
        requested: u32,
        placed: u32,
        became_monstrous: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraftResolution {
    Declined,
    SourceOrEnteredCreatureMissing,
    SourceHasNoCounter,
    DestinationCannotReceiveCounter,
    Moved {
        source: ObjectRef,
        entered_creature: ObjectRef,
        counters_removed: u32,
        counters_placed: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvolveResolution {
    SourceNoLongerEligible,
    ComparisonFalse,
    Resolved {
        source: ObjectRef,
        requested: u32,
        placed: u32,
        evolved: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenownResolution {
    SourceNoLongerEligible,
    AlreadyRenowned,
    BecameRenowned {
        source: ObjectRef,
        requested: u32,
        placed: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMoveEvidence {
    pub zone_change: ZoneChangeEvidence,
    pub modular_death_events: Vec<ModularDeathEventId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModularResolution {
    Declined,
    TargetIllegal,
    Counters {
        target: ObjectRef,
        requested: u32,
        placed: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PowerToughnessSnapshot {
    was_creature: bool,
    power: Option<i32>,
    toughness: Option<i32>,
}

impl PowerToughnessSnapshot {
    fn from_object(object: &TrackedObject) -> Self {
        Self {
            was_creature: object.is_creature(),
            power: object.effective_power,
            toughness: object.effective_toughness,
        }
    }
}

#[derive(Debug, Clone)]
struct FabricateTrigger {
    trigger_id: PendingTriggerId,
    controller: PlayerId,
    source: ObjectRef,
    amount: u32,
    semantic_digest: String,
}

#[derive(Debug, Clone)]
struct GraftTrigger {
    trigger_id: PendingTriggerId,
    controller: PlayerId,
    source: ObjectRef,
    entered_creature: ObjectRef,
    semantic_digest: String,
}

#[derive(Debug, Clone)]
struct EvolveTrigger {
    trigger_id: PendingTriggerId,
    controller: PlayerId,
    source: ObjectRef,
    entered_creature: ObjectRef,
    entered_lki: PowerToughnessSnapshot,
    semantic_digest: String,
}

#[derive(Debug, Clone)]
struct RenownTrigger {
    trigger_id: PendingTriggerId,
    controller: PlayerId,
    source: ObjectRef,
    amount: u32,
    semantic_digest: String,
}

#[derive(Debug, Clone)]
enum ActivatedCounterKind {
    Adapt { amount: u32 },
    Monstrosity { amount: u32 },
}

#[derive(Debug, Clone)]
struct ActivatedCounterTrigger {
    trigger_id: PendingTriggerId,
    controller: PlayerId,
    source: ObjectRef,
    kind: ActivatedCounterKind,
    semantic_digest: String,
}

#[derive(Debug, Clone)]
enum PendingTrigger {
    Fabricate(FabricateTrigger),
    Graft(GraftTrigger),
    Evolve(EvolveTrigger),
    Renown(RenownTrigger),
    Activated(ActivatedCounterTrigger),
}

#[derive(Debug, Clone)]
struct ModularDeathEvent {
    event_id: ModularDeathEventId,
    controller: PlayerId,
    source_lki: ObjectRef,
    plus_one_counters_lki: u32,
    semantic_digest: String,
}

#[derive(Debug, Clone)]
struct StackedModularTrigger {
    trigger_id: PendingTriggerId,
    controller: PlayerId,
    target: ObjectRef,
    plus_one_counters_lki: u32,
    semantic_digest: String,
}

#[derive(Debug, Clone)]
pub struct CreatureCounterRuntime {
    players: BTreeMap<PlayerId, PlayerState>,
    objects: BTreeMap<ObjectId, TrackedObject>,
    programs: BTreeMap<ObjectRef, Vec<CreatureCounterKeywordProgram>>,
    pending_triggers: BTreeMap<PendingTriggerId, PendingTrigger>,
    modular_death_events: BTreeMap<ModularDeathEventId, ModularDeathEvent>,
    stacked_modular: BTreeMap<PendingTriggerId, StackedModularTrigger>,
    monstrous: BTreeMap<ObjectRef, u32>,
    renowned: BTreeSet<ObjectRef>,
    devoured_counts: BTreeMap<ObjectRef, u32>,
    tribute_paid: BTreeMap<ObjectRef, bool>,
    next_trigger_id: u64,
    next_death_event_id: u64,
}

impl Default for CreatureCounterRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl CreatureCounterRuntime {
    pub fn new() -> Self {
        Self {
            players: BTreeMap::new(),
            objects: BTreeMap::new(),
            programs: BTreeMap::new(),
            pending_triggers: BTreeMap::new(),
            modular_death_events: BTreeMap::new(),
            stacked_modular: BTreeMap::new(),
            monstrous: BTreeMap::new(),
            renowned: BTreeSet::new(),
            devoured_counts: BTreeMap::new(),
            tribute_paid: BTreeMap::new(),
            next_trigger_id: 1,
            next_death_event_id: 1,
        }
    }

    pub fn insert_player(
        &mut self,
        player: PlayerId,
        state: PlayerState,
    ) -> Result<(), CreatureCounterRuntimeError> {
        if self.players.contains_key(&player) {
            return Err(CreatureCounterRuntimeError::DuplicatePlayer(player));
        }
        self.players.insert(player, state);
        Ok(())
    }

    pub fn insert_object(
        &mut self,
        object: TrackedObject,
    ) -> Result<(), CreatureCounterRuntimeError> {
        if self.objects.contains_key(&object.object_ref.object_id) {
            return Err(CreatureCounterRuntimeError::DuplicateObject(
                object.object_ref.object_id,
            ));
        }
        if !self.players.contains_key(&object.owner)
            || object
                .controller
                .is_some_and(|controller| !self.players.contains_key(&controller))
        {
            return Err(CreatureCounterRuntimeError::UnknownPlayer);
        }
        if object
            .subtypes
            .iter()
            .any(|subtype| normalize_subtype(subtype).as_deref() != Some(subtype))
        {
            return Err(CreatureCounterRuntimeError::InvalidCharacteristics);
        }
        self.objects.insert(object.object_ref.object_id, object);
        Ok(())
    }

    pub fn player(&self, player: PlayerId) -> Option<&PlayerState> {
        self.players.get(&player)
    }

    pub fn player_mut(&mut self, player: PlayerId) -> Option<&mut PlayerState> {
        self.players.get_mut(&player)
    }

    pub fn object(&self, object: ObjectRef) -> Option<&TrackedObject> {
        self.objects
            .get(&object.object_id)
            .filter(|tracked| tracked.object_ref == object)
    }

    pub fn current_object(&self, object_id: ObjectId) -> Option<&TrackedObject> {
        self.objects.get(&object_id)
    }

    pub fn is_monstrous(&self, permanent: ObjectRef) -> bool {
        self.monstrous.contains_key(&permanent)
    }

    pub fn monstrosity_x(&self, permanent: ObjectRef) -> Option<u32> {
        self.monstrous.get(&permanent).copied()
    }

    pub fn is_renowned(&self, permanent: ObjectRef) -> bool {
        self.renowned.contains(&permanent)
    }

    pub fn devoured_count(&self, permanent: ObjectRef) -> Option<u32> {
        self.devoured_counts.get(&permanent).copied()
    }

    pub fn was_tribute_paid(&self, permanent: ObjectRef) -> Option<bool> {
        self.tribute_paid.get(&permanent).copied()
    }

    pub fn unstacked_modular_death_events(&self) -> Vec<ModularDeathEventId> {
        self.modular_death_events.keys().copied().collect()
    }

    pub fn set_effective_power_toughness(
        &mut self,
        permanent: ObjectRef,
        power: i32,
        toughness: i32,
    ) -> Result<(), CreatureCounterRuntimeError> {
        let object = self.require_object_mut(permanent, Zone::Battlefield)?;
        if !object.is_creature() {
            return Err(CreatureCounterRuntimeError::NotCreature);
        }
        object.effective_power = Some(power);
        object.effective_toughness = Some(toughness);
        Ok(())
    }

    pub fn register_battlefield_program(
        &mut self,
        permanent: ObjectRef,
        program: &CreatureCounterKeywordProgram,
    ) -> Result<(), CreatureCounterRuntimeError> {
        let object = self.require_object(permanent, Zone::Battlefield)?;
        if !object.matches_context(program.source_context()) {
            return Err(CreatureCounterRuntimeError::SourceContextMismatch);
        }
        match program.kind() {
            CreatureCounterKeywordKind::Adapt { .. }
            | CreatureCounterKeywordKind::ResidualEvolve
            | CreatureCounterKeywordKind::Graft { .. }
            | CreatureCounterKeywordKind::Modular { .. }
            | CreatureCounterKeywordKind::Monstrosity { .. }
            | CreatureCounterKeywordKind::ResidualRenown { .. } => {}
            _ => return Err(CreatureCounterRuntimeError::WrongProgramKind),
        }
        self.programs
            .entry(permanent)
            .or_default()
            .push(program.clone());
        Ok(())
    }

    pub fn enter_with_keyword(
        &mut self,
        source: ObjectRef,
        program: &CreatureCounterKeywordProgram,
        input: EntryResolutionInput,
    ) -> Result<EntryResolutionEvidence, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.enter_with_keyword_inner(source, program, input)?;
        *self = staged;
        Ok(evidence)
    }

    fn enter_with_keyword_inner(
        &mut self,
        source: ObjectRef,
        program: &CreatureCounterKeywordProgram,
        input: EntryResolutionInput,
    ) -> Result<EntryResolutionEvidence, CreatureCounterRuntimeError> {
        if !self.players.contains_key(&input.controller) {
            return Err(CreatureCounterRuntimeError::UnknownPlayer);
        }
        let source_object = self
            .objects
            .get(&source.object_id)
            .filter(|object| object.object_ref == source)
            .cloned()
            .ok_or(CreatureCounterRuntimeError::MissingObject(source))?;
        if source_object.zone == Zone::Battlefield {
            return Err(CreatureCounterRuntimeError::WrongZone {
                expected: Zone::Stack,
                actual: Zone::Battlefield,
            });
        }
        if !source_object.matches_context(program.source_context()) {
            return Err(CreatureCounterRuntimeError::SourceContextMismatch);
        }
        let (actual_destination, _) = apply_zone_replacements(
            Zone::Battlefield,
            &input.destination_replacements,
            &input.replacement_order,
        )?;
        if actual_destination != Zone::Battlefield {
            if input.counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            let change = self.move_object(
                source,
                Zone::Battlefield,
                &input.destination_replacements,
                &input.replacement_order,
            )?;
            return Ok(EntryResolutionEvidence {
                source_zone_change: change,
                permanent: None,
                requested_counters: 0,
                counters_placed: 0,
                revealed_cards: Vec::new(),
                sacrificed_permanents: Vec::new(),
                tribute_paid: None,
                fabricate_trigger: None,
                entry_observer_triggers: Vec::new(),
            });
        }

        let predicted = predicted_next_ref(source)?;
        let mut revealed_cards = Vec::new();
        let mut sacrificed_permanents = Vec::new();
        let mut tribute_paid = None;
        let requested = match (program.kind(), &input.choice) {
            (
                CreatureCounterKeywordKind::Amplify {
                    counters_per_revealed_card,
                    matching_creature_types,
                },
                EntryKeywordChoice::Amplify {
                    revealed_cards: choices,
                    simultaneously_entering,
                    simultaneously_entering_census_complete,
                },
            ) => {
                if !simultaneously_entering_census_complete {
                    return Err(CreatureCounterRuntimeError::IncompleteSimultaneousEntryBoundary);
                }
                let mut unique = BTreeSet::new();
                for card_ref in choices {
                    if !unique.insert(*card_ref)
                        || card_ref.object_id == source.object_id
                        || simultaneously_entering.contains(card_ref)
                    {
                        return Err(CreatureCounterRuntimeError::InvalidAmplifyReveal);
                    }
                    let card = self.require_object(*card_ref, Zone::Hand)?;
                    if card.owner != input.controller
                        || card.subtypes.is_disjoint(matching_creature_types)
                    {
                        return Err(CreatureCounterRuntimeError::InvalidAmplifyReveal);
                    }
                }
                revealed_cards = choices.clone();
                u32::try_from(choices.len())
                    .ok()
                    .and_then(|count| count.checked_mul(*counters_per_revealed_card))
                    .ok_or(CreatureCounterRuntimeError::CounterQuantityOverflow)?
            }
            (
                CreatureCounterKeywordKind::Devour {
                    counters_per_sacrifice,
                    quality,
                },
                EntryKeywordChoice::Devour { sacrifices },
            ) => {
                let mut unique = BTreeSet::new();
                for sacrifice in sacrifices {
                    if !unique.insert(sacrifice.permanent.object_id)
                        || sacrifice.permanent.object_id == source.object_id
                    {
                        return Err(CreatureCounterRuntimeError::InvalidDevourSacrifice);
                    }
                    let permanent = self.require_object(sacrifice.permanent, Zone::Battlefield)?;
                    if permanent.controller != Some(input.controller)
                        || !devour_quality_matches(permanent, quality)
                    {
                        return Err(CreatureCounterRuntimeError::InvalidDevourSacrifice);
                    }
                }
                for sacrifice in sacrifices {
                    let change = self.move_object(
                        sacrifice.permanent,
                        Zone::Graveyard,
                        &sacrifice.destination_replacements,
                        &sacrifice.replacement_order,
                    )?;
                    sacrificed_permanents.push(change);
                }
                u32::try_from(sacrifices.len())
                    .ok()
                    .and_then(|count| count.checked_mul(*counters_per_sacrifice))
                    .ok_or(CreatureCounterRuntimeError::CounterQuantityOverflow)?
            }
            (CreatureCounterKeywordKind::Fabricate { .. }, EntryKeywordChoice::Fabricate) => 0,
            (CreatureCounterKeywordKind::Graft { counters }, EntryKeywordChoice::Graft) => {
                *counters
            }
            (
                CreatureCounterKeywordKind::Modular { counters },
                EntryKeywordChoice::Modular {
                    colors_spent_to_cast,
                    cast_payment_evidence_complete,
                },
            ) => match counters {
                CounterAmount::Fixed(amount) => {
                    if !colors_spent_to_cast.is_empty() {
                        return Err(CreatureCounterRuntimeError::UnexpectedCastColorEvidence);
                    }
                    *amount
                }
                CounterAmount::Sunburst => {
                    if !cast_payment_evidence_complete
                        || source_object.zone != Zone::Stack
                        || colors_spent_to_cast.iter().any(|color| !color.is_colored())
                    {
                        return Err(if !cast_payment_evidence_complete {
                            CreatureCounterRuntimeError::IncompleteCastPaymentBoundary
                        } else {
                            CreatureCounterRuntimeError::InvalidCastColorEvidence
                        });
                    }
                    u32::try_from(colors_spent_to_cast.len())
                        .map_err(|_| CreatureCounterRuntimeError::CounterQuantityOverflow)?
                }
                CounterAmount::BoundX => return Err(CreatureCounterRuntimeError::WrongProgramKind),
            },
            (
                CreatureCounterKeywordKind::Tribute { counters },
                EntryKeywordChoice::Tribute {
                    chosen_opponent,
                    opponent_accepts,
                },
            ) => {
                let controller_team = self
                    .players
                    .get(&input.controller)
                    .ok_or(CreatureCounterRuntimeError::UnknownPlayer)?
                    .team;
                if self
                    .players
                    .get(chosen_opponent)
                    .is_none_or(|player| player.team == controller_team)
                {
                    return Err(CreatureCounterRuntimeError::ChosenPlayerIsNotOpponent);
                }
                if *opponent_accepts { *counters } else { 0 }
            }
            _ => return Err(CreatureCounterRuntimeError::WrongEntryChoice),
        };
        let placed =
            self.validate_counter_evidence(predicted, requested, input.counter_placement)?;
        if let (
            CreatureCounterKeywordKind::Tribute { .. },
            EntryKeywordChoice::Tribute {
                opponent_accepts, ..
            },
        ) = (program.kind(), &input.choice)
        {
            tribute_paid = Some(*opponent_accepts && placed > 0);
        }
        let change = self.move_object(
            source,
            Zone::Battlefield,
            &input.destination_replacements,
            &input.replacement_order,
        )?;
        debug_assert_eq!(change.after, predicted);
        {
            let permanent = self
                .objects
                .get_mut(&source.object_id)
                .expect("moved object remains tracked");
            permanent.controller = Some(input.controller);
        }
        self.add_placed_counters(change.after, placed)?;
        if matches!(program.kind(), CreatureCounterKeywordKind::Devour { .. }) {
            self.devoured_counts.insert(
                change.after,
                u32::try_from(sacrificed_permanents.len())
                    .map_err(|_| CreatureCounterRuntimeError::CounterQuantityOverflow)?,
            );
        }
        if let Some(paid) = tribute_paid {
            self.tribute_paid.insert(change.after, paid);
        }

        let mut fabricate_trigger = None;
        match program.kind() {
            CreatureCounterKeywordKind::Fabricate { counters_or_tokens } => {
                let trigger_id = self.next_trigger_id()?;
                self.pending_triggers.insert(
                    trigger_id,
                    PendingTrigger::Fabricate(FabricateTrigger {
                        trigger_id,
                        controller: input.controller,
                        source: change.after,
                        amount: *counters_or_tokens,
                        semantic_digest: program.semantic_digest().to_owned(),
                    }),
                );
                fabricate_trigger = Some(trigger_id);
            }
            CreatureCounterKeywordKind::Graft { .. }
            | CreatureCounterKeywordKind::Modular { .. } => {
                self.programs
                    .entry(change.after)
                    .or_default()
                    .push(program.clone());
            }
            _ => {}
        }
        let entry_observer_triggers = if self
            .object(change.after)
            .is_some_and(TrackedObject::is_creature)
        {
            self.observe_creature_entered_inner(change.after)?
        } else {
            Vec::new()
        };
        Ok(EntryResolutionEvidence {
            source_zone_change: change.clone(),
            permanent: Some(change.after),
            requested_counters: requested,
            counters_placed: placed,
            revealed_cards,
            sacrificed_permanents,
            tribute_paid,
            fabricate_trigger,
            entry_observer_triggers,
        })
    }

    pub fn observe_creature_entered(
        &mut self,
        entered_creature: ObjectRef,
    ) -> Result<Vec<PendingTriggerId>, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let triggers = staged.observe_creature_entered_inner(entered_creature)?;
        *self = staged;
        Ok(triggers)
    }

    fn observe_creature_entered_inner(
        &mut self,
        entered_creature: ObjectRef,
    ) -> Result<Vec<PendingTriggerId>, CreatureCounterRuntimeError> {
        let entered = self
            .require_object(entered_creature, Zone::Battlefield)?
            .clone();
        if !entered.is_creature() {
            return Err(CreatureCounterRuntimeError::NotCreature);
        }
        let entered_controller = entered
            .controller
            .ok_or(CreatureCounterRuntimeError::InvalidObjectState)?;
        let entered_lki = PowerToughnessSnapshot::from_object(&entered);
        let bindings = self
            .programs
            .iter()
            .flat_map(|(source, programs)| {
                programs
                    .iter()
                    .cloned()
                    .map(|program| (*source, program))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut created = Vec::new();
        for (source_ref, program) in bindings {
            if source_ref == entered_creature {
                continue;
            }
            let Some(source) = self.object(source_ref).cloned() else {
                continue;
            };
            if source.zone != Zone::Battlefield {
                continue;
            }
            match program.kind() {
                CreatureCounterKeywordKind::Graft { .. } if source.plus_one_counters > 0 => {
                    let trigger_id = self.next_trigger_id()?;
                    self.pending_triggers.insert(
                        trigger_id,
                        PendingTrigger::Graft(GraftTrigger {
                            trigger_id,
                            controller: source
                                .controller
                                .ok_or(CreatureCounterRuntimeError::InvalidObjectState)?,
                            source: source_ref,
                            entered_creature,
                            semantic_digest: program.semantic_digest().to_owned(),
                        }),
                    );
                    created.push(trigger_id);
                }
                CreatureCounterKeywordKind::ResidualEvolve
                    if source.controller == Some(entered_controller)
                        && evolve_comparison(&source, entered_lki)? =>
                {
                    let trigger_id = self.next_trigger_id()?;
                    self.pending_triggers.insert(
                        trigger_id,
                        PendingTrigger::Evolve(EvolveTrigger {
                            trigger_id,
                            controller: source
                                .controller
                                .ok_or(CreatureCounterRuntimeError::InvalidObjectState)?,
                            source: source_ref,
                            entered_creature,
                            entered_lki,
                            semantic_digest: program.semantic_digest().to_owned(),
                        }),
                    );
                    created.push(trigger_id);
                }
                _ => {}
            }
        }
        Ok(created)
    }

    pub fn resolve_fabricate(
        &mut self,
        trigger_id: PendingTriggerId,
        preference: FabricatePreference,
        counter_placement: Option<CounterPlacementEvidence>,
        servo_token_ids: Vec<ObjectId>,
        token_creation_replacements_complete: bool,
    ) -> Result<FabricateResolution, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let result = staged.resolve_fabricate_inner(
            trigger_id,
            preference,
            counter_placement,
            servo_token_ids,
            token_creation_replacements_complete,
        )?;
        *self = staged;
        Ok(result)
    }

    fn resolve_fabricate_inner(
        &mut self,
        trigger_id: PendingTriggerId,
        preference: FabricatePreference,
        counter_placement: Option<CounterPlacementEvidence>,
        servo_token_ids: Vec<ObjectId>,
        token_creation_replacements_complete: bool,
    ) -> Result<FabricateResolution, CreatureCounterRuntimeError> {
        let pending = self
            .pending_triggers
            .get(&trigger_id)
            .cloned()
            .ok_or(CreatureCounterRuntimeError::MissingPendingTrigger)?;
        let PendingTrigger::Fabricate(trigger) = pending else {
            return Err(CreatureCounterRuntimeError::WrongTriggerKind);
        };
        let source_available = self
            .object(trigger.source)
            .is_some_and(|source| source.zone == Zone::Battlefield);
        if preference == FabricatePreference::PutCounters && source_available {
            if !servo_token_ids.is_empty() {
                return Err(CreatureCounterRuntimeError::UnexpectedTokenIds);
            }
            let placed =
                self.validate_counter_evidence(trigger.source, trigger.amount, counter_placement)?;
            self.add_placed_counters(trigger.source, placed)?;
            self.pending_triggers.remove(&trigger_id);
            return Ok(FabricateResolution::Counters {
                requested: trigger.amount,
                placed,
            });
        }
        if counter_placement.is_some() {
            return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
        }
        if !token_creation_replacements_complete {
            return Err(CreatureCounterRuntimeError::IncompleteTokenCreationBoundary);
        }
        if servo_token_ids.len()
            != usize::try_from(trigger.amount)
                .map_err(|_| CreatureCounterRuntimeError::CounterQuantityOverflow)?
        {
            return Err(CreatureCounterRuntimeError::WrongTokenCount);
        }
        let unique = servo_token_ids.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != servo_token_ids.len()
            || servo_token_ids
                .iter()
                .any(|object_id| self.objects.contains_key(object_id))
        {
            return Err(CreatureCounterRuntimeError::DuplicateObjectId);
        }
        let mut tokens = Vec::new();
        let mut observer_triggers = Vec::new();
        for object_id in servo_token_ids {
            let token_ref = ObjectRef {
                object_id,
                incarnation_id: IncarnationId(1),
            };
            let mut token = TrackedObject::new(
                token_ref,
                trigger.controller,
                Zone::Battlefield,
                [CardType::Artifact, CardType::Creature],
                ["servo".to_owned()],
                Some((1, 1)),
            );
            token.controller = Some(trigger.controller);
            token.is_token = true;
            self.objects.insert(object_id, token);
            observer_triggers.extend(self.observe_creature_entered_inner(token_ref)?);
            tokens.push(token_ref);
        }
        self.pending_triggers.remove(&trigger_id);
        Ok(FabricateResolution::Servos {
            tokens,
            entry_observer_triggers: observer_triggers,
        })
    }

    pub fn resolve_bolster(
        &mut self,
        controller: PlayerId,
        program: &CreatureCounterKeywordProgram,
        chosen: Option<ObjectRef>,
        counter_placement: Option<CounterPlacementEvidence>,
    ) -> Result<BolsterResolution, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let result =
            staged.resolve_bolster_inner(controller, program, chosen, counter_placement)?;
        *self = staged;
        Ok(result)
    }

    fn resolve_bolster_inner(
        &mut self,
        controller: PlayerId,
        program: &CreatureCounterKeywordProgram,
        chosen: Option<ObjectRef>,
        counter_placement: Option<CounterPlacementEvidence>,
    ) -> Result<BolsterResolution, CreatureCounterRuntimeError> {
        let CreatureCounterKeywordKind::Bolster { counters } = program.kind() else {
            return Err(CreatureCounterRuntimeError::WrongProgramKind);
        };
        if !self.players.contains_key(&controller) {
            return Err(CreatureCounterRuntimeError::UnknownPlayer);
        }
        let controlled_creatures = self
            .objects
            .values()
            .filter(|object| {
                object.zone == Zone::Battlefield
                    && object.controller == Some(controller)
                    && object.is_creature()
            })
            .collect::<Vec<_>>();
        if controlled_creatures
            .iter()
            .any(|object| object.effective_toughness.is_none())
        {
            return Err(CreatureCounterRuntimeError::MissingEffectiveToughness);
        }
        let eligible = controlled_creatures
            .into_iter()
            .map(|object| {
                (
                    object.object_ref,
                    object
                        .effective_toughness
                        .expect("checked complete toughness"),
                )
            })
            .collect::<Vec<_>>();
        let Some(minimum) = eligible.iter().map(|(_, toughness)| *toughness).min() else {
            if chosen.is_some() || counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedBolsterChoice);
            }
            return Ok(BolsterResolution::NoControlledCreature);
        };
        let chosen = chosen.ok_or(CreatureCounterRuntimeError::MissingBolsterChoice)?;
        if !eligible
            .iter()
            .any(|(candidate, toughness)| *candidate == chosen && *toughness == minimum)
        {
            return Err(CreatureCounterRuntimeError::InvalidBolsterChoice);
        }
        let placed = self.validate_counter_evidence(chosen, *counters, counter_placement)?;
        self.add_placed_counters(chosen, placed)?;
        Ok(BolsterResolution::Counters {
            chosen,
            requested: *counters,
            placed,
        })
    }

    pub fn activate_counter_ability(
        &mut self,
        controller: PlayerId,
        source: ObjectRef,
        program: &CreatureCounterKeywordProgram,
        window: ActivationWindow,
        payment: ManaPayment,
    ) -> Result<ActivationEvidence, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let evidence =
            staged.activate_counter_ability_inner(controller, source, program, window, payment)?;
        *self = staged;
        Ok(evidence)
    }

    fn activate_counter_ability_inner(
        &mut self,
        controller: PlayerId,
        source: ObjectRef,
        program: &CreatureCounterKeywordProgram,
        window: ActivationWindow,
        payment: ManaPayment,
    ) -> Result<ActivationEvidence, CreatureCounterRuntimeError> {
        if window.priority_player != controller
            || !window.prohibitions_allow_activation
            || !window.external_cost_modifiers_complete
        {
            return Err(CreatureCounterRuntimeError::ActivationNotLegal);
        }
        let permanent = self.require_object(source, Zone::Battlefield)?;
        if permanent.controller != Some(controller)
            || !permanent.matches_context(program.source_context())
        {
            return Err(CreatureCounterRuntimeError::SourceContextMismatch);
        }
        let (cost, kind) = match program.kind() {
            CreatureCounterKeywordKind::Adapt {
                activation_cost,
                counters,
            } => (
                activation_cost,
                ActivatedCounterKind::Adapt { amount: *counters },
            ),
            CreatureCounterKeywordKind::Monstrosity {
                activation_cost,
                counters,
            } => {
                let amount = match counters {
                    CounterAmount::Fixed(amount) => *amount,
                    CounterAmount::BoundX => payment.x_value,
                    CounterAmount::Sunburst => {
                        return Err(CreatureCounterRuntimeError::WrongProgramKind);
                    }
                };
                (
                    activation_cost,
                    ActivatedCounterKind::Monstrosity { amount },
                )
            }
            _ => return Err(CreatureCounterRuntimeError::WrongProgramKind),
        };
        self.pay_mana(controller, cost, &payment)?;
        let trigger_id = self.next_trigger_id()?;
        self.pending_triggers.insert(
            trigger_id,
            PendingTrigger::Activated(ActivatedCounterTrigger {
                trigger_id,
                controller,
                source,
                kind,
                semantic_digest: program.semantic_digest().to_owned(),
            }),
        );
        Ok(ActivationEvidence {
            trigger_id,
            source,
            controller,
            x_value: payment.x_value,
            mana_units_spent: payment.mana_units,
            semantic_digest: program.semantic_digest().to_owned(),
        })
    }

    pub fn resolve_counter_ability(
        &mut self,
        trigger_id: PendingTriggerId,
        counter_placement: Option<CounterPlacementEvidence>,
    ) -> Result<CounterAbilityResolution, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let result = staged.resolve_counter_ability_inner(trigger_id, counter_placement)?;
        *self = staged;
        Ok(result)
    }

    fn resolve_counter_ability_inner(
        &mut self,
        trigger_id: PendingTriggerId,
        counter_placement: Option<CounterPlacementEvidence>,
    ) -> Result<CounterAbilityResolution, CreatureCounterRuntimeError> {
        let pending = self
            .pending_triggers
            .get(&trigger_id)
            .cloned()
            .ok_or(CreatureCounterRuntimeError::MissingPendingTrigger)?;
        let PendingTrigger::Activated(trigger) = pending else {
            return Err(CreatureCounterRuntimeError::WrongTriggerKind);
        };
        let Some(source) = self.object(trigger.source).cloned() else {
            if counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.pending_triggers.remove(&trigger_id);
            return Ok(CounterAbilityResolution::SourceNoLongerThatPermanent);
        };
        if source.zone != Zone::Battlefield {
            if counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.pending_triggers.remove(&trigger_id);
            return Ok(CounterAbilityResolution::SourceNoLongerThatPermanent);
        }
        let (condition, amount, is_monstrous) = match trigger.kind {
            ActivatedCounterKind::Adapt { amount } => {
                (source.plus_one_counters == 0, amount, false)
            }
            ActivatedCounterKind::Monstrosity { amount } => {
                (!self.monstrous.contains_key(&trigger.source), amount, true)
            }
        };
        if !condition {
            if counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.pending_triggers.remove(&trigger_id);
            return Ok(CounterAbilityResolution::ConditionNotMet);
        }
        let placed = self.validate_counter_evidence(trigger.source, amount, counter_placement)?;
        self.add_placed_counters(trigger.source, placed)?;
        if is_monstrous {
            self.monstrous.insert(trigger.source, amount);
        }
        self.pending_triggers.remove(&trigger_id);
        Ok(CounterAbilityResolution::Applied {
            source: trigger.source,
            requested: amount,
            placed,
            became_monstrous: is_monstrous,
        })
    }

    pub fn resolve_graft(
        &mut self,
        trigger_id: PendingTriggerId,
        move_counter: bool,
        destination_placement: Option<CounterPlacementEvidence>,
    ) -> Result<GraftResolution, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let result = staged.resolve_graft_inner(trigger_id, move_counter, destination_placement)?;
        *self = staged;
        Ok(result)
    }

    fn resolve_graft_inner(
        &mut self,
        trigger_id: PendingTriggerId,
        move_counter: bool,
        destination_placement: Option<CounterPlacementEvidence>,
    ) -> Result<GraftResolution, CreatureCounterRuntimeError> {
        let pending = self
            .pending_triggers
            .get(&trigger_id)
            .cloned()
            .ok_or(CreatureCounterRuntimeError::MissingPendingTrigger)?;
        let PendingTrigger::Graft(trigger) = pending else {
            return Err(CreatureCounterRuntimeError::WrongTriggerKind);
        };
        if !move_counter {
            if destination_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.pending_triggers.remove(&trigger_id);
            return Ok(GraftResolution::Declined);
        }
        let source_available = self
            .object(trigger.source)
            .is_some_and(|object| object.zone == Zone::Battlefield);
        let entered_available = self
            .object(trigger.entered_creature)
            .is_some_and(|object| object.zone == Zone::Battlefield);
        if !source_available || !entered_available {
            if destination_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.pending_triggers.remove(&trigger_id);
            return Ok(GraftResolution::SourceOrEnteredCreatureMissing);
        }
        if self
            .object(trigger.source)
            .expect("source checked")
            .plus_one_counters
            == 0
        {
            if destination_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.pending_triggers.remove(&trigger_id);
            return Ok(GraftResolution::SourceHasNoCounter);
        }
        let placed =
            self.validate_counter_evidence(trigger.entered_creature, 1, destination_placement)?;
        if destination_placement.is_some_and(|evidence| !evidence.placement_action_possible) {
            self.pending_triggers.remove(&trigger_id);
            return Ok(GraftResolution::DestinationCannotReceiveCounter);
        }
        self.remove_one_counter(trigger.source)?;
        self.add_placed_counters(trigger.entered_creature, placed)?;
        self.pending_triggers.remove(&trigger_id);
        Ok(GraftResolution::Moved {
            source: trigger.source,
            entered_creature: trigger.entered_creature,
            counters_removed: 1,
            counters_placed: placed,
        })
    }

    pub fn resolve_evolve(
        &mut self,
        trigger_id: PendingTriggerId,
        counter_placement: Option<CounterPlacementEvidence>,
    ) -> Result<EvolveResolution, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let result = staged.resolve_evolve_inner(trigger_id, counter_placement)?;
        *self = staged;
        Ok(result)
    }

    fn resolve_evolve_inner(
        &mut self,
        trigger_id: PendingTriggerId,
        counter_placement: Option<CounterPlacementEvidence>,
    ) -> Result<EvolveResolution, CreatureCounterRuntimeError> {
        let pending = self
            .pending_triggers
            .get(&trigger_id)
            .cloned()
            .ok_or(CreatureCounterRuntimeError::MissingPendingTrigger)?;
        let PendingTrigger::Evolve(trigger) = pending else {
            return Err(CreatureCounterRuntimeError::WrongTriggerKind);
        };
        let Some(source) = self.object(trigger.source).cloned() else {
            if counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.pending_triggers.remove(&trigger_id);
            return Ok(EvolveResolution::SourceNoLongerEligible);
        };
        if source.zone != Zone::Battlefield || !source.is_creature() {
            if counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.pending_triggers.remove(&trigger_id);
            return Ok(EvolveResolution::SourceNoLongerEligible);
        }
        let entered = self
            .object(trigger.entered_creature)
            .filter(|object| object.zone == Zone::Battlefield)
            .map(PowerToughnessSnapshot::from_object)
            .unwrap_or(trigger.entered_lki);
        if !evolve_comparison(&source, entered)? {
            if counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.pending_triggers.remove(&trigger_id);
            return Ok(EvolveResolution::ComparisonFalse);
        }
        let placed = self.validate_counter_evidence(trigger.source, 1, counter_placement)?;
        self.add_placed_counters(trigger.source, placed)?;
        self.pending_triggers.remove(&trigger_id);
        Ok(EvolveResolution::Resolved {
            source: trigger.source,
            requested: 1,
            placed,
            evolved: placed > 0,
        })
    }

    pub fn record_combat_damage_to_player(
        &mut self,
        source: ObjectRef,
        damaged_player: PlayerId,
        damage: u32,
    ) -> Result<Vec<PendingTriggerId>, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let created =
            staged.record_combat_damage_to_player_inner(source, damaged_player, damage)?;
        *self = staged;
        Ok(created)
    }

    fn record_combat_damage_to_player_inner(
        &mut self,
        source: ObjectRef,
        damaged_player: PlayerId,
        damage: u32,
    ) -> Result<Vec<PendingTriggerId>, CreatureCounterRuntimeError> {
        if damage == 0 || !self.players.contains_key(&damaged_player) {
            return Err(CreatureCounterRuntimeError::InvalidCombatDamageEvent);
        }
        let source_object = self.require_object(source, Zone::Battlefield)?.clone();
        if !source_object.is_creature() {
            return Err(CreatureCounterRuntimeError::NotCreature);
        }
        let source_controller = source_object
            .controller
            .ok_or(CreatureCounterRuntimeError::InvalidObjectState)?;
        if self
            .players
            .get(&damaged_player)
            .zip(self.players.get(&source_controller))
            .is_none_or(|(damaged, controller)| damaged.team == controller.team)
        {
            return Err(CreatureCounterRuntimeError::InvalidCombatDamageEvent);
        }
        let bindings = self.programs.get(&source).cloned().unwrap_or_default();
        let mut created = Vec::new();
        for program in bindings {
            let CreatureCounterKeywordKind::ResidualRenown { counters } = program.kind() else {
                continue;
            };
            if self.renowned.contains(&source) {
                continue;
            }
            let trigger_id = self.next_trigger_id()?;
            self.pending_triggers.insert(
                trigger_id,
                PendingTrigger::Renown(RenownTrigger {
                    trigger_id,
                    controller: source_controller,
                    source,
                    amount: *counters,
                    semantic_digest: program.semantic_digest().to_owned(),
                }),
            );
            created.push(trigger_id);
        }
        Ok(created)
    }

    pub fn resolve_renown(
        &mut self,
        trigger_id: PendingTriggerId,
        counter_placement: Option<CounterPlacementEvidence>,
    ) -> Result<RenownResolution, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let result = staged.resolve_renown_inner(trigger_id, counter_placement)?;
        *self = staged;
        Ok(result)
    }

    fn resolve_renown_inner(
        &mut self,
        trigger_id: PendingTriggerId,
        counter_placement: Option<CounterPlacementEvidence>,
    ) -> Result<RenownResolution, CreatureCounterRuntimeError> {
        let pending = self
            .pending_triggers
            .get(&trigger_id)
            .cloned()
            .ok_or(CreatureCounterRuntimeError::MissingPendingTrigger)?;
        let PendingTrigger::Renown(trigger) = pending else {
            return Err(CreatureCounterRuntimeError::WrongTriggerKind);
        };
        let eligible = self
            .object(trigger.source)
            .is_some_and(|source| source.zone == Zone::Battlefield && source.is_creature());
        if !eligible {
            if counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.pending_triggers.remove(&trigger_id);
            return Ok(RenownResolution::SourceNoLongerEligible);
        }
        if self.renowned.contains(&trigger.source) {
            if counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.pending_triggers.remove(&trigger_id);
            return Ok(RenownResolution::AlreadyRenowned);
        }
        let placed =
            self.validate_counter_evidence(trigger.source, trigger.amount, counter_placement)?;
        self.add_placed_counters(trigger.source, placed)?;
        self.renowned.insert(trigger.source);
        self.pending_triggers.remove(&trigger_id);
        Ok(RenownResolution::BecameRenowned {
            source: trigger.source,
            requested: trigger.amount,
            placed,
        })
    }

    pub fn apply_external_zone_change(
        &mut self,
        source: ObjectRef,
        requested_destination: Zone,
        replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<ExternalMoveEvidence, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let before_events = staged
            .modular_death_events
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let zone_change = staged.move_object(
            source,
            requested_destination,
            &replacements,
            &replacement_order,
        )?;
        let modular_death_events = staged
            .modular_death_events
            .keys()
            .copied()
            .filter(|event_id| !before_events.contains(event_id))
            .collect::<Vec<_>>();
        *self = staged;
        Ok(ExternalMoveEvidence {
            zone_change,
            modular_death_events,
        })
    }

    pub fn stack_modular_trigger(
        &mut self,
        death_event_id: ModularDeathEventId,
        target: ObjectRef,
    ) -> Result<PendingTriggerId, CreatureCounterRuntimeError> {
        let event = self
            .modular_death_events
            .get(&death_event_id)
            .cloned()
            .ok_or(CreatureCounterRuntimeError::MissingModularDeathEvent)?;
        if !self
            .object(target)
            .is_some_and(|object| object.zone == Zone::Battlefield && object.is_artifact_creature())
        {
            return Err(CreatureCounterRuntimeError::IllegalModularTarget);
        }
        let trigger_id = self.next_trigger_id()?;
        self.stacked_modular.insert(
            trigger_id,
            StackedModularTrigger {
                trigger_id,
                controller: event.controller,
                target,
                plus_one_counters_lki: event.plus_one_counters_lki,
                semantic_digest: event.semantic_digest,
            },
        );
        self.modular_death_events.remove(&death_event_id);
        Ok(trigger_id)
    }

    pub fn dismiss_modular_death_event(
        &mut self,
        death_event_id: ModularDeathEventId,
    ) -> Result<(), CreatureCounterRuntimeError> {
        self.modular_death_events
            .remove(&death_event_id)
            .map(|_| ())
            .ok_or(CreatureCounterRuntimeError::MissingModularDeathEvent)
    }

    pub fn resolve_modular(
        &mut self,
        trigger_id: PendingTriggerId,
        put_counters: bool,
        counter_placement: Option<CounterPlacementEvidence>,
    ) -> Result<ModularResolution, CreatureCounterRuntimeError> {
        let mut staged = self.clone();
        let result = staged.resolve_modular_inner(trigger_id, put_counters, counter_placement)?;
        *self = staged;
        Ok(result)
    }

    fn resolve_modular_inner(
        &mut self,
        trigger_id: PendingTriggerId,
        put_counters: bool,
        counter_placement: Option<CounterPlacementEvidence>,
    ) -> Result<ModularResolution, CreatureCounterRuntimeError> {
        let trigger = self
            .stacked_modular
            .get(&trigger_id)
            .cloned()
            .ok_or(CreatureCounterRuntimeError::MissingPendingTrigger)?;
        if !self
            .object(trigger.target)
            .is_some_and(|object| object.zone == Zone::Battlefield && object.is_artifact_creature())
        {
            if counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.stacked_modular.remove(&trigger_id);
            return Ok(ModularResolution::TargetIllegal);
        }
        if !put_counters {
            if counter_placement.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            self.stacked_modular.remove(&trigger_id);
            return Ok(ModularResolution::Declined);
        }
        let placed = self.validate_counter_evidence(
            trigger.target,
            trigger.plus_one_counters_lki,
            counter_placement,
        )?;
        self.add_placed_counters(trigger.target, placed)?;
        self.stacked_modular.remove(&trigger_id);
        Ok(ModularResolution::Counters {
            target: trigger.target,
            requested: trigger.plus_one_counters_lki,
            placed,
        })
    }

    fn validate_counter_evidence(
        &self,
        object: ObjectRef,
        requested: u32,
        evidence: Option<CounterPlacementEvidence>,
    ) -> Result<u32, CreatureCounterRuntimeError> {
        if requested == 0 {
            if evidence.is_some() {
                return Err(CreatureCounterRuntimeError::UnexpectedCounterEvidence);
            }
            return Ok(0);
        }
        let evidence = evidence.ok_or(CreatureCounterRuntimeError::MissingCounterEvidence)?;
        if evidence.object != object {
            return Err(CreatureCounterRuntimeError::CounterEvidenceObjectMismatch);
        }
        if evidence.requested_counters != requested {
            return Err(CreatureCounterRuntimeError::CounterRequestMismatch {
                expected: requested,
                actual: evidence.requested_counters,
            });
        }
        if !evidence.replacement_effects_complete {
            return Err(CreatureCounterRuntimeError::IncompleteCounterReplacementBoundary);
        }
        if !evidence.placement_action_possible && evidence.counters_placed != 0 {
            return Err(CreatureCounterRuntimeError::ImpossibleCounterPlacementResult);
        }
        Ok(evidence.counters_placed)
    }

    fn add_placed_counters(
        &mut self,
        object: ObjectRef,
        placed: u32,
    ) -> Result<(), CreatureCounterRuntimeError> {
        if placed == 0 {
            return Ok(());
        }
        let permanent = self.require_object_mut(object, Zone::Battlefield)?;
        permanent.plus_one_counters = permanent
            .plus_one_counters
            .checked_add(placed)
            .ok_or(CreatureCounterRuntimeError::CounterQuantityOverflow)?;
        let amount = i32::try_from(placed)
            .map_err(|_| CreatureCounterRuntimeError::CounterQuantityOverflow)?;
        if let Some(power) = permanent.effective_power.as_mut() {
            *power = power
                .checked_add(amount)
                .ok_or(CreatureCounterRuntimeError::CharacteristicOverflow)?;
        }
        if let Some(toughness) = permanent.effective_toughness.as_mut() {
            *toughness = toughness
                .checked_add(amount)
                .ok_or(CreatureCounterRuntimeError::CharacteristicOverflow)?;
        }
        Ok(())
    }

    fn remove_one_counter(&mut self, object: ObjectRef) -> Result<(), CreatureCounterRuntimeError> {
        let permanent = self.require_object_mut(object, Zone::Battlefield)?;
        if permanent.plus_one_counters == 0 {
            return Err(CreatureCounterRuntimeError::CounterNotPresent);
        }
        permanent.plus_one_counters -= 1;
        if let Some(power) = permanent.effective_power.as_mut() {
            *power = power
                .checked_sub(1)
                .ok_or(CreatureCounterRuntimeError::CharacteristicOverflow)?;
        }
        if let Some(toughness) = permanent.effective_toughness.as_mut() {
            *toughness = toughness
                .checked_sub(1)
                .ok_or(CreatureCounterRuntimeError::CharacteristicOverflow)?;
        }
        Ok(())
    }

    fn pay_mana(
        &mut self,
        player: PlayerId,
        cost: &ManaCost,
        payment: &ManaPayment,
    ) -> Result<(), CreatureCounterRuntimeError> {
        if !cost.contains_x() && payment.x_value != 0 {
            return Err(CreatureCounterRuntimeError::ManaPaymentMismatch);
        }
        let state = self
            .players
            .get(&player)
            .ok_or(CreatureCounterRuntimeError::UnknownPlayer)?;
        let mut seen = BTreeSet::new();
        let units = payment
            .mana_units
            .iter()
            .map(|unit_id| {
                if !seen.insert(*unit_id) {
                    return Err(CreatureCounterRuntimeError::DuplicateManaUnit(*unit_id));
                }
                state
                    .mana_pool
                    .get(unit_id)
                    .copied()
                    .ok_or(CreatureCounterRuntimeError::MissingManaUnit(*unit_id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !mana_units_match_cost(cost, payment.x_value, &units) {
            return Err(CreatureCounterRuntimeError::ManaPaymentMismatch);
        }
        let state = self.players.get_mut(&player).expect("validated mana payer");
        for unit_id in &payment.mana_units {
            state.mana_pool.remove(unit_id);
        }
        Ok(())
    }

    fn move_object(
        &mut self,
        source: ObjectRef,
        requested_destination: Zone,
        replacements: &[ExternalZoneReplacement],
        replacement_order: &[ReplacementEffectId],
    ) -> Result<ZoneChangeEvidence, CreatureCounterRuntimeError> {
        let current = self
            .objects
            .get(&source.object_id)
            .filter(|object| object.object_ref == source)
            .cloned()
            .ok_or(CreatureCounterRuntimeError::MissingObject(source))?;
        let (actual_destination, applications) =
            apply_zone_replacements(requested_destination, replacements, replacement_order)?;
        if actual_destination == current.zone {
            return Err(CreatureCounterRuntimeError::UnsupportedNoZoneChangeReplacement);
        }
        let after = predicted_next_ref(source)?;
        let modular_bindings =
            if current.zone == Zone::Battlefield && actual_destination == Zone::Graveyard {
                self.programs
                    .get(&source)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|program| {
                        matches!(program.kind(), CreatureCounterKeywordKind::Modular { .. })
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
        self.programs.remove(&source);
        self.monstrous.remove(&source);
        self.renowned.remove(&source);
        self.devoured_counts.remove(&source);
        self.tribute_paid.remove(&source);
        let object = self
            .objects
            .get_mut(&source.object_id)
            .expect("current object was cloned");
        object.object_ref = after;
        object.zone = actual_destination;
        object.controller = if matches!(actual_destination, Zone::Battlefield | Zone::Stack) {
            current.controller.or(Some(current.owner))
        } else {
            None
        };
        if current.zone == Zone::Battlefield && actual_destination != Zone::Battlefield {
            let removed = i32::try_from(object.plus_one_counters)
                .map_err(|_| CreatureCounterRuntimeError::CharacteristicOverflow)?;
            if let Some(power) = object.effective_power.as_mut() {
                *power = power
                    .checked_sub(removed)
                    .ok_or(CreatureCounterRuntimeError::CharacteristicOverflow)?;
            }
            if let Some(toughness) = object.effective_toughness.as_mut() {
                *toughness = toughness
                    .checked_sub(removed)
                    .ok_or(CreatureCounterRuntimeError::CharacteristicOverflow)?;
            }
            object.plus_one_counters = 0;
        }
        for program in modular_bindings {
            let event_id = self.next_death_event_id()?;
            self.modular_death_events.insert(
                event_id,
                ModularDeathEvent {
                    event_id,
                    controller: current
                        .controller
                        .ok_or(CreatureCounterRuntimeError::InvalidObjectState)?,
                    source_lki: source,
                    plus_one_counters_lki: current.plus_one_counters,
                    semantic_digest: program.semantic_digest().to_owned(),
                },
            );
        }
        Ok(ZoneChangeEvidence {
            before: source,
            after,
            from: current.zone,
            requested_destination,
            actual_destination,
            replacement_order: replacement_order.to_vec(),
            replacements_applied: applications,
        })
    }

    fn require_object(
        &self,
        object: ObjectRef,
        zone: Zone,
    ) -> Result<&TrackedObject, CreatureCounterRuntimeError> {
        let tracked = self
            .objects
            .get(&object.object_id)
            .filter(|tracked| tracked.object_ref == object)
            .ok_or(CreatureCounterRuntimeError::MissingObject(object))?;
        if tracked.zone != zone {
            return Err(CreatureCounterRuntimeError::WrongZone {
                expected: zone,
                actual: tracked.zone,
            });
        }
        Ok(tracked)
    }

    fn require_object_mut(
        &mut self,
        object: ObjectRef,
        zone: Zone,
    ) -> Result<&mut TrackedObject, CreatureCounterRuntimeError> {
        let tracked = self
            .objects
            .get_mut(&object.object_id)
            .filter(|tracked| tracked.object_ref == object)
            .ok_or(CreatureCounterRuntimeError::MissingObject(object))?;
        if tracked.zone != zone {
            return Err(CreatureCounterRuntimeError::WrongZone {
                expected: zone,
                actual: tracked.zone,
            });
        }
        Ok(tracked)
    }

    fn next_trigger_id(&mut self) -> Result<PendingTriggerId, CreatureCounterRuntimeError> {
        let id = PendingTriggerId(self.next_trigger_id);
        self.next_trigger_id = self
            .next_trigger_id
            .checked_add(1)
            .ok_or(CreatureCounterRuntimeError::IdentifierOverflow)?;
        Ok(id)
    }

    fn next_death_event_id(&mut self) -> Result<ModularDeathEventId, CreatureCounterRuntimeError> {
        let id = ModularDeathEventId(self.next_death_event_id);
        self.next_death_event_id = self
            .next_death_event_id
            .checked_add(1)
            .ok_or(CreatureCounterRuntimeError::IdentifierOverflow)?;
        Ok(id)
    }
}

fn predicted_next_ref(source: ObjectRef) -> Result<ObjectRef, CreatureCounterRuntimeError> {
    Ok(ObjectRef {
        object_id: source.object_id,
        incarnation_id: IncarnationId(
            source
                .incarnation_id
                .0
                .checked_add(1)
                .ok_or(CreatureCounterRuntimeError::IdentifierOverflow)?,
        ),
    })
}

fn devour_quality_matches(object: &TrackedObject, quality: &DevourQuality) -> bool {
    match quality {
        DevourQuality::Creature => object.card_types.contains(&CardType::Creature),
        DevourQuality::CardType(card_type) => object.card_types.contains(card_type),
        DevourQuality::Subtype(subtype) => object.subtypes.contains(subtype),
    }
}

fn evolve_comparison(
    source: &TrackedObject,
    entered: PowerToughnessSnapshot,
) -> Result<bool, CreatureCounterRuntimeError> {
    if !source.is_creature() || !entered.was_creature {
        return Ok(false);
    }
    match (
        source.effective_power,
        source.effective_toughness,
        entered.power,
        entered.toughness,
    ) {
        (Some(source_power), Some(source_toughness), Some(other_power), Some(other_toughness)) => {
            Ok(other_power > source_power || other_toughness > source_toughness)
        }
        _ => Err(CreatureCounterRuntimeError::MissingEffectivePowerToughness),
    }
}

fn apply_zone_replacements(
    requested_destination: Zone,
    replacements: &[ExternalZoneReplacement],
    replacement_order: &[ReplacementEffectId],
) -> Result<(Zone, Vec<ReplacementApplicationEvidence>), CreatureCounterRuntimeError> {
    let replacement_ids = replacements
        .iter()
        .map(|replacement| replacement.effect_id)
        .collect::<BTreeSet<_>>();
    let order_ids = replacement_order.iter().copied().collect::<BTreeSet<_>>();
    if replacement_ids.len() != replacements.len()
        || order_ids.len() != replacement_order.len()
        || replacement_ids != order_ids
    {
        return Err(CreatureCounterRuntimeError::ReplacementOrderMismatch);
    }
    let replacements = replacements
        .iter()
        .map(|replacement| (replacement.effect_id, replacement))
        .collect::<BTreeMap<_, _>>();
    let mut destination = requested_destination;
    let mut applications = Vec::new();
    for effect_id in replacement_order {
        let replacement = replacements
            .get(effect_id)
            .ok_or(CreatureCounterRuntimeError::ReplacementOrderMismatch)?;
        if replacement
            .replaces_destination
            .is_none_or(|expected| expected == destination)
        {
            let before = destination;
            destination = replacement.destination;
            applications.push(ReplacementApplicationEvidence {
                effect_id: *effect_id,
                destination_before: before,
                destination_after: destination,
            });
        }
    }
    Ok((destination, applications))
}

fn mana_units_match_cost(cost: &ManaCost, x_value: u32, units: &[ManaUnit]) -> bool {
    let mut generic = 0_u32;
    let mut specific = Vec::new();
    for symbol in &cost.symbols {
        match symbol {
            ManaSymbol::Generic(amount) => {
                let Some(next) = generic.checked_add(*amount) else {
                    return false;
                };
                generic = next;
            }
            ManaSymbol::VariableX => {
                let Some(next) = generic.checked_add(x_value) else {
                    return false;
                };
                generic = next;
            }
            symbol => specific.push(*symbol),
        }
    }
    let Ok(generic) = usize::try_from(generic) else {
        return false;
    };
    if units.len() != specific.len().saturating_add(generic) {
        return false;
    }
    fn recurse(
        index: usize,
        symbols: &[ManaSymbol],
        units: &[ManaUnit],
        used: &mut [bool],
    ) -> bool {
        if index == symbols.len() {
            return true;
        }
        for unit_index in 0..units.len() {
            if !used[unit_index] && mana_unit_matches(units[unit_index], symbols[index]) {
                used[unit_index] = true;
                if recurse(index + 1, symbols, units, used) {
                    return true;
                }
                used[unit_index] = false;
            }
        }
        false
    }
    recurse(0, &specific, units, &mut vec![false; units.len()])
}

fn mana_unit_matches(unit: ManaUnit, symbol: ManaSymbol) -> bool {
    match symbol {
        ManaSymbol::White => unit.color == ManaColor::White,
        ManaSymbol::Blue => unit.color == ManaColor::Blue,
        ManaSymbol::Black => unit.color == ManaColor::Black,
        ManaSymbol::Red => unit.color == ManaColor::Red,
        ManaSymbol::Green => unit.color == ManaColor::Green,
        ManaSymbol::Colorless => unit.color == ManaColor::Colorless,
        ManaSymbol::Snow => unit.from_snow_source,
        ManaSymbol::Hybrid(first, second) => unit.color == first || unit.color == second,
        ManaSymbol::Generic(_) | ManaSymbol::VariableX => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreatureCounterRuntimeError {
    DuplicatePlayer(PlayerId),
    DuplicateObject(ObjectId),
    DuplicateObjectId,
    UnknownPlayer,
    MissingObject(ObjectRef),
    WrongZone { expected: Zone, actual: Zone },
    InvalidCharacteristics,
    InvalidObjectState,
    SourceContextMismatch,
    WrongProgramKind,
    WrongEntryChoice,
    InvalidAmplifyReveal,
    IncompleteSimultaneousEntryBoundary,
    InvalidDevourSacrifice,
    UnexpectedCastColorEvidence,
    InvalidCastColorEvidence,
    IncompleteCastPaymentBoundary,
    ChosenPlayerIsNotOpponent,
    MissingCounterEvidence,
    UnexpectedCounterEvidence,
    CounterEvidenceObjectMismatch,
    CounterRequestMismatch { expected: u32, actual: u32 },
    IncompleteCounterReplacementBoundary,
    ImpossibleCounterPlacementResult,
    CounterQuantityOverflow,
    CharacteristicOverflow,
    CounterNotPresent,
    ReplacementOrderMismatch,
    UnsupportedNoZoneChangeReplacement,
    MissingPendingTrigger,
    WrongTriggerKind,
    UnexpectedTokenIds,
    IncompleteTokenCreationBoundary,
    WrongTokenCount,
    UnexpectedBolsterChoice,
    MissingBolsterChoice,
    InvalidBolsterChoice,
    MissingEffectiveToughness,
    MissingEffectivePowerToughness,
    ActivationNotLegal,
    MissingManaUnit(ManaUnitId),
    DuplicateManaUnit(ManaUnitId),
    ManaPaymentMismatch,
    NotCreature,
    InvalidCombatDamageEvent,
    MissingModularDeathEvent,
    IllegalModularTarget,
    IdentifierOverflow,
}

impl fmt::Display for CreatureCounterRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CreatureCounterRuntimeError {}
