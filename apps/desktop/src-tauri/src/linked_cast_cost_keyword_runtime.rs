//! Content and semantic-context keyed programs for residual Evoke plus Blitz,
//! Spectacle, Surge, and Prowl.
//!
//! Canonical reminder-bearing Evoke and Dash clauses remain owned by
//! `mechanic_runtime`. This module accepts only genuine residual Evoke clauses
//! and complete clauses for the otherwise unowned families. Its transaction
//! runtime is deliberately not connected to production.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const LINKED_CAST_COST_COMPILER_VERSION: &str = "linked-cast-cost-compiler-0.1";
pub const LINKED_CAST_COST_RUNTIME_VERSION: &str = "linked-cast-cost-runtime-0.1";
pub const LINKED_CAST_COST_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:108.3,117,400.7,601.2,603.7,607,609.3,614.1,616.1,702.74,702.76,702.109,702.117,702.137,702.152";

const EVOKE_REMINDER: &str =
    "You may cast this spell for its evoke cost. If you do, it's sacrificed when it enters.";
const DASH_REMINDER: &str = "You may cast this spell for its dash cost. If you do, it gains haste, and it's returned from the battlefield to its owner's hand at the beginning of the next end step.";
const BLITZ_REMINDER: &str = "If you cast this spell for its blitz cost, it gains haste and \"When this creature dies, draw a card.\" Sacrifice it at the beginning of the next end step.";
const SPECTACLE_REMINDER: &str = "You may cast this spell for its spectacle cost rather than its mana cost if an opponent lost life this turn.";
const SURGE_REMINDER: &str = "You may cast this spell for its surge cost if you or a teammate has cast another spell this turn.";

pub const fn linked_cast_cost_production_adapter_connected() -> bool {
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
            Self::Artifact | Self::Battle | Self::Creature | Self::Enchantment | Self::Planeswalker
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSemanticContext {
    pub card_types: BTreeSet<CardType>,
    pub creature_types: BTreeSet<String>,
}

impl SourceSemanticContext {
    pub fn from_type_line(type_line: &str) -> Option<Self> {
        if type_line.trim() != type_line
            || type_line.is_empty()
            || collapse_whitespace(type_line) != type_line
        {
            return None;
        }
        let (type_part, subtype_part) = type_line
            .split_once(" \u{2014} ")
            .or_else(|| type_line.split_once(" \u{fffd} "))
            .or_else(|| type_line.split_once(" - "))
            .map_or((type_line, ""), |(types, subtypes)| (types, subtypes));
        let mut card_types = BTreeSet::new();
        for token in type_part.split_ascii_whitespace() {
            let card_type = match token {
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
            };
            if let Some(card_type) = card_type {
                card_types.insert(card_type);
            }
        }
        if card_types.is_empty() {
            return None;
        }
        let creature_types = subtype_part
            .split_ascii_whitespace()
            .map(normalize_creature_type)
            .collect::<Option<BTreeSet<_>>>()?;
        Some(Self {
            card_types,
            creature_types,
        })
    }

    fn is_creature_spell(&self) -> bool {
        self.card_types.contains(&CardType::Creature)
    }

    fn is_nonland_spell(&self) -> bool {
        !self.card_types.contains(&CardType::Land)
            && self.card_types.iter().any(|card_type| {
                matches!(
                    card_type,
                    CardType::Artifact
                        | CardType::Battle
                        | CardType::Creature
                        | CardType::Enchantment
                        | CardType::Instant
                        | CardType::Kindred
                        | CardType::Planeswalker
                        | CardType::Sorcery
                )
            })
    }

    fn stable_id(&self) -> String {
        format!(
            "types={};creature-types={}",
            self.card_types
                .iter()
                .map(|card_type| card_type.stable_id())
                .collect::<Vec<_>>()
                .join(","),
            self.creature_types
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

fn normalize_creature_type(source: &str) -> Option<String> {
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
            Self::Snow => "s".into(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdditionalCastCost {
    None,
    PayLife(u32),
    DiscardOneCard,
    ExileOneCardFromHandWithColor(ManaColor),
}

impl AdditionalCastCost {
    fn stable_id(&self) -> String {
        match self {
            Self::None => "none".into(),
            Self::PayLife(amount) => format!("pay-life/{amount}"),
            Self::DiscardOneCard => "discard-one-card".into(),
            Self::ExileOneCardFromHandWithColor(color) => {
                format!("exile-one-{}-card-from-hand", color.stable_id())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternativeCastCost {
    pub mana: Option<ManaCost>,
    pub additional: AdditionalCastCost,
}

impl AlternativeCastCost {
    fn stable_id(&self) -> String {
        format!(
            "mana={};additional={}",
            self.mana
                .as_ref()
                .map_or_else(|| "none".into(), |mana| mana.stable_id()),
            self.additional.stable_id()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidualEvokeProgram {
    pub alternative_cost: AlternativeCastCost,
    pub cast_from_normally_permitted_zone: bool,
    pub sacrifice_trigger_on_entry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlitzProgram {
    pub alternative_cost: AlternativeCastCost,
    pub cast_from_normally_permitted_zone: bool,
    pub grants_haste: bool,
    pub grants_death_draw: bool,
    pub schedules_next_end_step_sacrifice: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpectacleProgram {
    pub alternative_cost: ManaCost,
    pub requires_opponent_lost_life_this_turn: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurgeProgram {
    pub alternative_cost: ManaCost,
    pub requires_you_or_teammate_cast_another_spell_this_turn: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProwlProgram {
    pub alternative_cost: ManaCost,
    pub qualifying_creature_types: BTreeSet<String>,
    pub requires_combat_damage_to_player_this_turn: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedCastCostKind {
    ResidualEvoke(ResidualEvokeProgram),
    Blitz(BlitzProgram),
    Spectacle(SpectacleProgram),
    Surge(SurgeProgram),
    Prowl(ProwlProgram),
}

impl LinkedCastCostKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ResidualEvoke(_) => "Evoke",
            Self::Blitz(_) => "Blitz",
            Self::Spectacle(_) => "Spectacle",
            Self::Surge(_) => "Surge",
            Self::Prowl(_) => "Prowl",
        }
    }

    fn alternative_cost(&self) -> AlternativeCastCost {
        match self {
            Self::ResidualEvoke(program) => program.alternative_cost.clone(),
            Self::Blitz(program) => program.alternative_cost.clone(),
            Self::Spectacle(program) => AlternativeCastCost {
                mana: Some(program.alternative_cost.clone()),
                additional: AdditionalCastCost::None,
            },
            Self::Surge(program) => AlternativeCastCost {
                mana: Some(program.alternative_cost.clone()),
                additional: AdditionalCastCost::None,
            },
            Self::Prowl(program) => AlternativeCastCost {
                mana: Some(program.alternative_cost.clone()),
                additional: AdditionalCastCost::None,
            },
        }
    }

    fn stable_id(&self) -> String {
        match self {
            Self::ResidualEvoke(program) => format!(
                "residual-evoke/v1;cost={};zone=externally-permitted;entry-trigger=sacrifice-self",
                program.alternative_cost.stable_id()
            ),
            Self::Blitz(program) => format!(
                "blitz/v1;cost={};zone=externally-permitted;haste=true;death-draw=true;delayed=next-end-step-sacrifice",
                program.alternative_cost.stable_id()
            ),
            Self::Spectacle(program) => format!(
                "spectacle/v1;cost={};condition=opponent-lost-life-this-turn",
                program.alternative_cost.stable_id()
            ),
            Self::Surge(program) => format!(
                "surge/v1;cost={};condition=self-or-teammate-cast-another-spell-this-turn",
                program.alternative_cost.stable_id()
            ),
            Self::Prowl(program) => format!(
                "prowl/v1;cost={};condition=combat-damage-to-player-by-controlled-source-sharing-spell-creature-type;types={}",
                program.alternative_cost.stable_id(),
                program
                    .qualifying_creature_types
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedCastCostProgram {
    exact_source: String,
    source_context: SourceSemanticContext,
    kind: LinkedCastCostKind,
    semantic_digest: String,
}

impl LinkedCastCostProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn source_context(&self) -> &SourceSemanticContext {
        &self.source_context
    }

    pub fn kind(&self) -> &LinkedCastCostKind {
        &self.kind
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub const fn production_adapter_connected(&self) -> bool {
        linked_cast_cost_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotCandidateClass {
    SupportedResidual,
    ExistingMechanicRuntimeOwner,
    UnsupportedGrantModifierReferenceCompoundOrIncomplete,
}

pub fn classify_linked_cast_cost_snapshot_candidate(
    exact_source: &str,
    source_type_line: &str,
) -> Option<SnapshotCandidateClass> {
    candidate_family(exact_source)?;
    let context = SourceSemanticContext::from_type_line(source_type_line)?;
    if is_existing_mechanic_runtime_owner(exact_source, &context) {
        return Some(SnapshotCandidateClass::ExistingMechanicRuntimeOwner);
    }
    if compile_linked_cast_cost_keyword_program(exact_source, source_type_line).is_some() {
        return Some(SnapshotCandidateClass::SupportedResidual);
    }
    Some(SnapshotCandidateClass::UnsupportedGrantModifierReferenceCompoundOrIncomplete)
}

pub fn compile_linked_cast_cost_keyword_program(
    exact_source: &str,
    source_type_line: &str,
) -> Option<LinkedCastCostProgram> {
    if exact_source.is_empty()
        || exact_source.trim() != exact_source
        || exact_source.contains(['\r', '\n'])
        || collapse_whitespace(exact_source) != exact_source
    {
        return None;
    }
    let source_context = SourceSemanticContext::from_type_line(source_type_line)?;
    let kind = parse_residual_evoke(exact_source, &source_context)
        .or_else(|| parse_blitz(exact_source, &source_context))
        .or_else(|| parse_spectacle(exact_source, &source_context))
        .or_else(|| parse_surge(exact_source, &source_context))
        .or_else(|| parse_prowl(exact_source, &source_context))?;
    let semantic_digest = linked_cast_cost_semantic_digest(exact_source, &source_context, &kind);
    Some(LinkedCastCostProgram {
        exact_source: exact_source.to_owned(),
        source_context,
        kind,
        semantic_digest,
    })
}

fn is_existing_mechanic_runtime_owner(exact_source: &str, context: &SourceSemanticContext) -> bool {
    let Some((core, reminder)) = split_trailing_parenthetical(exact_source) else {
        return false;
    };
    (context.is_creature_spell()
        && core
            .strip_prefix("Evoke ")
            .and_then(parse_mana_cost)
            .is_some()
        && reminder == Some(EVOKE_REMINDER))
        || (context.is_creature_spell()
            && core
                .strip_prefix("Dash ")
                .and_then(parse_mana_cost)
                .is_some()
            && reminder == Some(DASH_REMINDER))
}

fn parse_residual_evoke(
    exact_source: &str,
    context: &SourceSemanticContext,
) -> Option<LinkedCastCostKind> {
    if !context.is_creature_spell() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    let alternative_cost = if let Some(cost) = core.strip_prefix("Evoke ") {
        if reminder.is_some() {
            return None;
        }
        AlternativeCastCost {
            mana: Some(parse_mana_cost(cost)?),
            additional: AdditionalCastCost::None,
        }
    } else if let Some(color) = strip_keyword_dash(core, "Evoke")
        .and_then(|rest| rest.strip_prefix("Exile a "))
        .and_then(|rest| rest.strip_suffix(" card from your hand."))
        .and_then(parse_color_word)
    {
        if reminder.is_some() {
            return None;
        }
        AlternativeCastCost {
            mana: None,
            additional: AdditionalCastCost::ExileOneCardFromHandWithColor(color),
        }
    } else {
        let rest = strip_keyword_dash(core, "Evoke")?.strip_suffix('.')?;
        let (mana, life) = rest.split_once(", Pay ")?;
        let life = life.strip_suffix(" life")?.parse::<u32>().ok()?;
        if life == 0 || reminder.is_some() {
            return None;
        }
        AlternativeCastCost {
            mana: Some(parse_mana_cost(mana)?),
            additional: AdditionalCastCost::PayLife(life),
        }
    };
    Some(LinkedCastCostKind::ResidualEvoke(ResidualEvokeProgram {
        alternative_cost,
        cast_from_normally_permitted_zone: true,
        sacrifice_trigger_on_entry: true,
    }))
}

fn parse_blitz(exact_source: &str, context: &SourceSemanticContext) -> Option<LinkedCastCostKind> {
    if !context.is_creature_spell() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    let alternative_cost = if let Some(cost) = core.strip_prefix("Blitz ") {
        if reminder.is_some_and(|reminder| reminder != BLITZ_REMINDER) {
            return None;
        }
        AlternativeCastCost {
            mana: Some(parse_mana_cost(cost)?),
            additional: AdditionalCastCost::None,
        }
    } else {
        let rest = strip_keyword_dash(core, "Blitz")?.strip_suffix('.')?;
        if reminder != Some(BLITZ_REMINDER) {
            return None;
        }
        if let Some((mana, life)) = rest.split_once(", Pay ") {
            let life = life.strip_suffix(" life")?.parse::<u32>().ok()?;
            if life == 0 {
                return None;
            }
            AlternativeCastCost {
                mana: Some(parse_mana_cost(mana)?),
                additional: AdditionalCastCost::PayLife(life),
            }
        } else {
            let mana = rest.strip_suffix(", Discard a card")?;
            AlternativeCastCost {
                mana: Some(parse_mana_cost(mana)?),
                additional: AdditionalCastCost::DiscardOneCard,
            }
        }
    };
    Some(LinkedCastCostKind::Blitz(BlitzProgram {
        alternative_cost,
        cast_from_normally_permitted_zone: true,
        grants_haste: true,
        grants_death_draw: true,
        schedules_next_end_step_sacrifice: true,
    }))
}

fn parse_spectacle(
    exact_source: &str,
    context: &SourceSemanticContext,
) -> Option<LinkedCastCostKind> {
    if !context.is_nonland_spell() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    if reminder != Some(SPECTACLE_REMINDER) {
        return None;
    }
    Some(LinkedCastCostKind::Spectacle(SpectacleProgram {
        alternative_cost: parse_mana_cost(core.strip_prefix("Spectacle ")?)?,
        requires_opponent_lost_life_this_turn: true,
    }))
}

fn parse_surge(exact_source: &str, context: &SourceSemanticContext) -> Option<LinkedCastCostKind> {
    if !context.is_nonland_spell() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    if reminder != Some(SURGE_REMINDER) {
        return None;
    }
    Some(LinkedCastCostKind::Surge(SurgeProgram {
        alternative_cost: parse_mana_cost(core.strip_prefix("Surge ")?)?,
        requires_you_or_teammate_cast_another_spell_this_turn: true,
    }))
}

fn parse_prowl(exact_source: &str, context: &SourceSemanticContext) -> Option<LinkedCastCostKind> {
    if !context.is_nonland_spell() || context.creature_types.is_empty() {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    let reminder = reminder?;
    let prefix = if let Some(rest) = reminder.strip_prefix(
        "You may cast this for its prowl cost if you dealt combat damage to a player this turn with a ",
    ) {
        rest
    } else {
        reminder.strip_prefix(
            "You may cast this spell for its prowl cost if you dealt combat damage to a player this turn with a ",
        )?
    };
    let types = prefix
        .strip_suffix('.')?
        .split(" or ")
        .map(normalize_creature_type)
        .collect::<Option<BTreeSet<_>>>()?;
    if types != context.creature_types {
        return None;
    }
    Some(LinkedCastCostKind::Prowl(ProwlProgram {
        alternative_cost: parse_mana_cost(core.strip_prefix("Prowl ")?)?,
        qualifying_creature_types: types,
        requires_combat_damage_to_player_this_turn: true,
    }))
}

fn candidate_family(source: &str) -> Option<&'static str> {
    let lower = source.to_ascii_lowercase();
    for (needle, family) in [
        ("evoke", "Evoke"),
        ("dash", "Dash"),
        ("blitz", "Blitz"),
        ("spectacle", "Spectacle"),
        ("surge", "Surge"),
        ("prowl", "Prowl"),
    ] {
        if contains_word(&lower, needle) {
            return Some(family);
        }
    }
    None
}

fn strip_keyword_dash<'a>(source: &'a str, keyword: &str) -> Option<&'a str> {
    source
        .strip_prefix(keyword)?
        .strip_prefix('\u{2014}')
        .or_else(|| source.strip_prefix(keyword)?.strip_prefix('\u{fffd}'))
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

fn linked_cast_cost_semantic_digest(
    exact_source: &str,
    context: &SourceSemanticContext,
    kind: &LinkedCastCostKind,
) -> String {
    let kind_contract = kind.stable_id();
    let context_contract = context.stable_id();
    let mut hasher = Sha256::new();
    for component in [
        "linked-cast-cost-content/v1",
        LINKED_CAST_COST_COMPILER_VERSION,
        LINKED_CAST_COST_RUNTIME_VERSION,
        LINKED_CAST_COST_RULES_CONTEXT_VERSION,
        exact_source,
        &context_contract,
        &kind_contract,
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

fn parse_color_word(source: &str) -> Option<ManaColor> {
    match source {
        "white" => Some(ManaColor::White),
        "blue" => Some(ManaColor::Blue),
        "black" => Some(ManaColor::Black),
        "red" => Some(ManaColor::Red),
        "green" => Some(ManaColor::Green),
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
pub struct TurnId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingTriggerId(pub u64);

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
    pub life: i32,
    pub team: TeamId,
    pub mana_pool: BTreeMap<ManaUnitId, ManaUnit>,
    /// Top card first. Every entry is checked against the current object
    /// incarnation when a draw resolves.
    pub library_order: Vec<ObjectId>,
    pub cards_drawn: u32,
    pub failed_draws: u32,
}

impl PlayerState {
    pub fn new(life: i32, team: TeamId) -> Self {
        Self {
            life,
            team,
            mana_pool: BTreeMap::new(),
            library_order: Vec::new(),
            cards_drawn: 0,
            failed_draws: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastMethod {
    ResidualEvoke { semantic_digest: String },
    Blitz { semantic_digest: String },
    Spectacle { semantic_digest: String },
    Surge { semantic_digest: String },
    Prowl { semantic_digest: String },
}

impl CastMethod {
    fn semantic_digest(&self) -> &str {
        match self {
            Self::ResidualEvoke { semantic_digest }
            | Self::Blitz { semantic_digest }
            | Self::Spectacle { semantic_digest }
            | Self::Surge { semantic_digest }
            | Self::Prowl { semantic_digest } => semantic_digest,
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
    pub creature_types: BTreeSet<String>,
    pub colors: BTreeSet<ManaColor>,
    pub printed_mana_cost: Option<ManaCost>,
    pub cast_method: Option<CastMethod>,
    pub is_copy: bool,
    pub copied_from: Option<ObjectRef>,
}

impl TrackedCard {
    pub fn card(
        object_ref: ObjectRef,
        owner: PlayerId,
        zone: Zone,
        card_types: impl IntoIterator<Item = CardType>,
        creature_types: impl IntoIterator<Item = String>,
        colors: impl IntoIterator<Item = ManaColor>,
        printed_mana_cost: Option<ManaCost>,
    ) -> Self {
        Self {
            object_ref,
            owner,
            controller: matches!(zone, Zone::Battlefield | Zone::Stack).then_some(owner),
            zone,
            card_types: card_types.into_iter().collect(),
            creature_types: creature_types.into_iter().collect(),
            colors: colors.into_iter().collect(),
            printed_mana_cost,
            cast_method: None,
            is_copy: false,
            copied_from: None,
        }
    }

    fn is_permanent_spell(&self) -> bool {
        self.card_types
            .iter()
            .any(|card_type| card_type.is_permanent())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastPermissionEvidence {
    pub source_zone: Zone,
    /// Evidence from the caller's normal cast-permission engine. This module
    /// never infers that a card may be cast from an unusual zone.
    pub rules_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastWindow {
    pub turn: TurnId,
    pub priority_player: PlayerId,
    pub normal_timing_allows_cast: bool,
    pub required_targets_available: bool,
    pub prohibitions_allow_cast: bool,
    pub external_additional_costs_satisfied: bool,
}

impl CastWindow {
    pub const fn fully_legal(turn: TurnId, player: PlayerId) -> Self {
        Self {
            turn,
            priority_player: player,
            normal_timing_allows_cast: true,
            required_targets_available: true,
            prohibitions_allow_cast: true,
            external_additional_costs_satisfied: true,
        }
    }

    fn permits_cast(self, player: PlayerId) -> bool {
        self.priority_player == player
            && self.normal_timing_allows_cast
            && self.required_targets_available
            && self.prohibitions_allow_cast
            && self.external_additional_costs_satisfied
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ManaPayment {
    pub x_value: u32,
    pub mana_units: Vec<ManaUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalZoneReplacement {
    pub effect_id: ReplacementEffectId,
    pub replaces_destination: Option<Zone>,
    pub destination: Zone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastPayment {
    pub mana: ManaPayment,
    pub life: u32,
    pub additional_card: Option<ObjectRef>,
    pub additional_card_replacements: Vec<ExternalZoneReplacement>,
    pub additional_card_replacement_order: Vec<ReplacementEffectId>,
}

impl CastPayment {
    pub fn mana(mana_units: Vec<ManaUnitId>) -> Self {
        Self {
            mana: ManaPayment {
                x_value: 0,
                mana_units,
            },
            life: 0,
            additional_card: None,
            additional_card_replacements: Vec::new(),
            additional_card_replacement_order: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPaymentEvidence {
    pub exact_cost: String,
    pub x_value: u32,
    pub mana_units_spent: Vec<ManaUnitId>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastCostEvidence {
    pub mana: Option<ManaPaymentEvidence>,
    pub life_paid: u32,
    pub additional_card: Option<ZoneChangeEvidence>,
    pub external_additional_costs_satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastEvidence {
    pub caster: PlayerId,
    pub turn: TurnId,
    pub source_zone_change: ZoneChangeEvidence,
    pub stack_object: ObjectRef,
    pub method: CastMethod,
    pub cost: CastCostEvidence,
    pub delayed_blitz_sacrifice_scheduled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermanentResolutionEvidence {
    pub stack_zone_change: ZoneChangeEvidence,
    pub permanent: Option<ObjectRef>,
    pub evoke_trigger: Option<PendingTriggerId>,
    pub blitz_marker_attached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifeLossEvent {
    pub turn: TurnId,
    pub player: PlayerId,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCastEvent {
    pub turn: TurnId,
    pub caster: PlayerId,
    pub stack_object: ObjectRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatDamageEvent {
    pub turn: TurnId,
    pub source: ObjectRef,
    pub source_controller: PlayerId,
    pub source_creature_types: BTreeSet<String>,
    pub damaged_player: PlayerId,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvokeSacrificeTrigger {
    pub trigger_id: PendingTriggerId,
    pub controller: PlayerId,
    pub expected_permanent: ObjectRef,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlitzSacrificeTrigger {
    pub trigger_id: PendingTriggerId,
    pub controller: PlayerId,
    pub expected_permanent: ObjectRef,
    pub cast_turn: TurnId,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlitzDeathDrawTrigger {
    pub trigger_id: PendingTriggerId,
    pub controller: PlayerId,
    pub source_before_death: ObjectRef,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingLinkedTrigger {
    EvokeSacrifice(EvokeSacrificeTrigger),
    BlitzSacrifice(BlitzSacrificeTrigger),
    BlitzDeathDraw(BlitzDeathDrawTrigger),
}

impl PendingLinkedTrigger {
    pub fn trigger_id(&self) -> PendingTriggerId {
        match self {
            Self::EvokeSacrifice(trigger) => trigger.trigger_id,
            Self::BlitzSacrifice(trigger) => trigger.trigger_id,
            Self::BlitzDeathDraw(trigger) => trigger.trigger_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SacrificeResolution {
    SourceNoLongerThatPermanent,
    Moved(ZoneChangeEvidence),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawResolution {
    Drew(ZoneChangeEvidence),
    DrawReplaced(ZoneChangeEvidence),
    EmptyLibrary,
}

#[derive(Debug, Clone)]
struct BlitzMarker {
    controller: PlayerId,
    semantic_digest: String,
}

#[derive(Debug, Clone)]
struct BlitzDelayedSacrifice {
    controller: PlayerId,
    expected_permanent: ObjectRef,
    cast_turn: TurnId,
    semantic_digest: String,
}

#[derive(Debug, Clone)]
pub struct LinkedCastCostRuntime {
    players: BTreeMap<PlayerId, PlayerState>,
    objects: BTreeMap<ObjectId, TrackedCard>,
    life_loss_events: Vec<LifeLossEvent>,
    spell_cast_events: Vec<SpellCastEvent>,
    combat_damage_events: Vec<CombatDamageEvent>,
    blitz_markers: BTreeMap<ObjectRef, BlitzMarker>,
    blitz_delayed_sacrifices: BTreeMap<ObjectRef, BlitzDelayedSacrifice>,
    pending_triggers: BTreeMap<PendingTriggerId, PendingLinkedTrigger>,
    next_trigger_id: u64,
}

impl Default for LinkedCastCostRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkedCastCostRuntime {
    pub fn new() -> Self {
        Self {
            players: BTreeMap::new(),
            objects: BTreeMap::new(),
            life_loss_events: Vec::new(),
            spell_cast_events: Vec::new(),
            combat_damage_events: Vec::new(),
            blitz_markers: BTreeMap::new(),
            blitz_delayed_sacrifices: BTreeMap::new(),
            pending_triggers: BTreeMap::new(),
            next_trigger_id: 1,
        }
    }

    pub fn insert_player(
        &mut self,
        player: PlayerId,
        state: PlayerState,
    ) -> Result<(), LinkedCastCostRuntimeError> {
        if state.life < 0 {
            return Err(LinkedCastCostRuntimeError::InvalidPlayerState);
        }
        if self.players.contains_key(&player) {
            return Err(LinkedCastCostRuntimeError::DuplicatePlayer(player));
        }
        self.players.insert(player, state);
        Ok(())
    }

    pub fn insert_object(&mut self, object: TrackedCard) -> Result<(), LinkedCastCostRuntimeError> {
        if self.objects.contains_key(&object.object_ref.object_id) {
            return Err(LinkedCastCostRuntimeError::DuplicateObject(
                object.object_ref.object_id,
            ));
        }
        if !self.players.contains_key(&object.owner)
            || object
                .controller
                .is_some_and(|controller| !self.players.contains_key(&controller))
        {
            return Err(LinkedCastCostRuntimeError::UnknownPlayer);
        }
        if object.cast_method.is_some() {
            return Err(LinkedCastCostRuntimeError::InvalidObjectState);
        }
        if object.creature_types.iter().any(|creature_type| {
            normalize_creature_type(creature_type).as_deref() != Some(creature_type)
        }) {
            return Err(LinkedCastCostRuntimeError::InvalidCharacteristics);
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

    pub fn object(&self, object: ObjectRef) -> Option<&TrackedCard> {
        self.objects
            .get(&object.object_id)
            .filter(|tracked| tracked.object_ref == object)
    }

    pub fn current_object(&self, object_id: ObjectId) -> Option<&TrackedCard> {
        self.objects.get(&object_id)
    }

    pub fn pending_trigger(&self, trigger_id: PendingTriggerId) -> Option<&PendingLinkedTrigger> {
        self.pending_triggers.get(&trigger_id)
    }

    pub fn life_loss_events(&self) -> &[LifeLossEvent] {
        &self.life_loss_events
    }

    pub fn spell_cast_events(&self) -> &[SpellCastEvent] {
        &self.spell_cast_events
    }

    pub fn combat_damage_events(&self) -> &[CombatDamageEvent] {
        &self.combat_damage_events
    }

    pub fn record_life_loss(
        &mut self,
        turn: TurnId,
        player: PlayerId,
        amount: u32,
    ) -> Result<(), LinkedCastCostRuntimeError> {
        if amount == 0 {
            return Err(LinkedCastCostRuntimeError::InvalidEvent);
        }
        let state = self
            .players
            .get_mut(&player)
            .ok_or(LinkedCastCostRuntimeError::UnknownPlayer)?;
        state.life = state
            .life
            .checked_sub(
                i32::try_from(amount).map_err(|_| LinkedCastCostRuntimeError::InvalidEvent)?,
            )
            .ok_or(LinkedCastCostRuntimeError::InvalidEvent)?;
        self.life_loss_events.push(LifeLossEvent {
            turn,
            player,
            amount,
        });
        Ok(())
    }

    pub fn record_prior_spell_cast(
        &mut self,
        turn: TurnId,
        caster: PlayerId,
        stack_object: ObjectRef,
    ) -> Result<(), LinkedCastCostRuntimeError> {
        if !self.players.contains_key(&caster) {
            return Err(LinkedCastCostRuntimeError::UnknownPlayer);
        }
        self.spell_cast_events.push(SpellCastEvent {
            turn,
            caster,
            stack_object,
        });
        Ok(())
    }

    pub fn record_combat_damage_to_player(
        &mut self,
        turn: TurnId,
        source: ObjectRef,
        damaged_player: PlayerId,
        amount: u32,
    ) -> Result<(), LinkedCastCostRuntimeError> {
        if amount == 0 || !self.players.contains_key(&damaged_player) {
            return Err(LinkedCastCostRuntimeError::InvalidEvent);
        }
        let source_object = self.require_exact_object(source, Zone::Battlefield)?;
        let source_controller = source_object
            .controller
            .ok_or(LinkedCastCostRuntimeError::InvalidEvent)?;
        if !source_object.card_types.contains(&CardType::Creature) {
            return Err(LinkedCastCostRuntimeError::InvalidEvent);
        }
        self.combat_damage_events.push(CombatDamageEvent {
            turn,
            source,
            source_controller,
            source_creature_types: source_object.creature_types.clone(),
            damaged_player,
            amount,
        });
        Ok(())
    }

    pub fn cast_with_linked_cost(
        &mut self,
        caster: PlayerId,
        source: ObjectRef,
        program: &LinkedCastCostProgram,
        permission: CastPermissionEvidence,
        window: CastWindow,
        payment: CastPayment,
    ) -> Result<CastEvidence, LinkedCastCostRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged
            .cast_with_linked_cost_inner(caster, source, program, permission, window, payment)?;
        *self = staged;
        Ok(evidence)
    }

    fn cast_with_linked_cost_inner(
        &mut self,
        caster: PlayerId,
        source: ObjectRef,
        program: &LinkedCastCostProgram,
        permission: CastPermissionEvidence,
        window: CastWindow,
        payment: CastPayment,
    ) -> Result<CastEvidence, LinkedCastCostRuntimeError> {
        if !self.players.contains_key(&caster) {
            return Err(LinkedCastCostRuntimeError::UnknownPlayer);
        }
        if !window.permits_cast(caster) {
            return Err(LinkedCastCostRuntimeError::CastIsNotLegal);
        }
        if permission.rules_digest.trim().is_empty()
            || permission.source_zone == Zone::Stack
            || permission.source_zone == Zone::Battlefield
        {
            return Err(LinkedCastCostRuntimeError::MissingNormalCastPermission);
        }
        let source_object = self
            .require_exact_object(source, permission.source_zone)?
            .clone();
        if source_object.is_copy {
            return Err(LinkedCastCostRuntimeError::CopiesAreNotCastByThisRuntime);
        }
        if source_object.card_types != program.source_context.card_types
            || source_object.creature_types != program.source_context.creature_types
        {
            return Err(LinkedCastCostRuntimeError::SourceContextMismatch);
        }
        self.require_condition(caster, program, window.turn)?;
        let cost = self.pay_alternative_cost(caster, source, program.kind(), &payment, window)?;
        let change = self.move_object(source, Zone::Stack, &[], &[])?;
        let method = cast_method(program);
        {
            let stack = self
                .objects
                .get_mut(&source.object_id)
                .expect("moved source remains tracked");
            stack.controller = Some(caster);
            stack.cast_method = Some(method.clone());
        }
        self.spell_cast_events.push(SpellCastEvent {
            turn: window.turn,
            caster,
            stack_object: change.after,
        });
        let delayed_blitz_sacrifice_scheduled =
            matches!(program.kind(), LinkedCastCostKind::Blitz(_));
        if delayed_blitz_sacrifice_scheduled {
            let expected_permanent = ObjectRef {
                object_id: change.after.object_id,
                incarnation_id: IncarnationId(
                    change
                        .after
                        .incarnation_id
                        .0
                        .checked_add(1)
                        .ok_or(LinkedCastCostRuntimeError::IncarnationOverflow)?,
                ),
            };
            self.blitz_delayed_sacrifices.insert(
                expected_permanent,
                BlitzDelayedSacrifice {
                    controller: caster,
                    expected_permanent,
                    cast_turn: window.turn,
                    semantic_digest: program.semantic_digest().to_owned(),
                },
            );
        }
        Ok(CastEvidence {
            caster,
            turn: window.turn,
            source_zone_change: change.clone(),
            stack_object: change.after,
            method,
            cost,
            delayed_blitz_sacrifice_scheduled,
        })
    }

    fn require_condition(
        &self,
        caster: PlayerId,
        program: &LinkedCastCostProgram,
        turn: TurnId,
    ) -> Result<(), LinkedCastCostRuntimeError> {
        let caster_team = self
            .players
            .get(&caster)
            .ok_or(LinkedCastCostRuntimeError::UnknownPlayer)?
            .team;
        let satisfied = match program.kind() {
            LinkedCastCostKind::ResidualEvoke(_) | LinkedCastCostKind::Blitz(_) => true,
            LinkedCastCostKind::Spectacle(_) => self.life_loss_events.iter().any(|event| {
                event.turn == turn
                    && self
                        .players
                        .get(&event.player)
                        .is_some_and(|state| state.team != caster_team)
            }),
            LinkedCastCostKind::Surge(_) => self.spell_cast_events.iter().any(|event| {
                event.turn == turn
                    && self
                        .players
                        .get(&event.caster)
                        .is_some_and(|state| state.team == caster_team)
            }),
            LinkedCastCostKind::Prowl(prowl) => self.combat_damage_events.iter().any(|event| {
                event.turn == turn
                    && event.source_controller == caster
                    && !event
                        .source_creature_types
                        .is_disjoint(&prowl.qualifying_creature_types)
            }),
        };
        if satisfied {
            Ok(())
        } else {
            Err(LinkedCastCostRuntimeError::AlternativeCostConditionNotMet)
        }
    }

    fn pay_alternative_cost(
        &mut self,
        caster: PlayerId,
        source: ObjectRef,
        kind: &LinkedCastCostKind,
        payment: &CastPayment,
        window: CastWindow,
    ) -> Result<CastCostEvidence, LinkedCastCostRuntimeError> {
        let cost = kind.alternative_cost();
        let mana = match &cost.mana {
            Some(mana_cost) => Some(self.pay_mana(caster, mana_cost, &payment.mana)?),
            None => {
                if payment.mana.x_value != 0 || !payment.mana.mana_units.is_empty() {
                    return Err(LinkedCastCostRuntimeError::ManaPaymentMismatch);
                }
                None
            }
        };
        let expected_life = match cost.additional {
            AdditionalCastCost::PayLife(amount) => amount,
            _ => 0,
        };
        if payment.life != expected_life {
            return Err(LinkedCastCostRuntimeError::LifePaymentMismatch);
        }
        if expected_life != 0 {
            let player = self
                .players
                .get_mut(&caster)
                .ok_or(LinkedCastCostRuntimeError::UnknownPlayer)?;
            let payment_i32 = i32::try_from(expected_life)
                .map_err(|_| LinkedCastCostRuntimeError::CannotPayLife)?;
            if player.life < payment_i32 {
                return Err(LinkedCastCostRuntimeError::CannotPayLife);
            }
            player.life -= payment_i32;
        }

        let additional_card = match cost.additional {
            AdditionalCastCost::None | AdditionalCastCost::PayLife(_) => {
                if payment.additional_card.is_some()
                    || !payment.additional_card_replacements.is_empty()
                    || !payment.additional_card_replacement_order.is_empty()
                {
                    return Err(LinkedCastCostRuntimeError::UnexpectedAdditionalCard);
                }
                None
            }
            AdditionalCastCost::DiscardOneCard => {
                let additional = payment
                    .additional_card
                    .ok_or(LinkedCastCostRuntimeError::MissingAdditionalCard)?;
                if additional.object_id == source.object_id {
                    return Err(LinkedCastCostRuntimeError::SourceCannotPayItsOwnCost);
                }
                let card = self.require_exact_object(additional, Zone::Hand)?;
                if card.owner != caster || card.is_copy {
                    return Err(LinkedCastCostRuntimeError::InvalidAdditionalCard);
                }
                Some(self.move_object(
                    additional,
                    Zone::Graveyard,
                    &payment.additional_card_replacements,
                    &payment.additional_card_replacement_order,
                )?)
            }
            AdditionalCastCost::ExileOneCardFromHandWithColor(color) => {
                let additional = payment
                    .additional_card
                    .ok_or(LinkedCastCostRuntimeError::MissingAdditionalCard)?;
                if additional.object_id == source.object_id {
                    return Err(LinkedCastCostRuntimeError::SourceCannotPayItsOwnCost);
                }
                let card = self.require_exact_object(additional, Zone::Hand)?;
                if card.owner != caster || card.is_copy || !card.colors.contains(&color) {
                    return Err(LinkedCastCostRuntimeError::InvalidAdditionalCard);
                }
                Some(self.move_object(
                    additional,
                    Zone::Exile,
                    &payment.additional_card_replacements,
                    &payment.additional_card_replacement_order,
                )?)
            }
        };
        Ok(CastCostEvidence {
            mana,
            life_paid: expected_life,
            additional_card,
            external_additional_costs_satisfied: window.external_additional_costs_satisfied,
        })
    }

    pub fn resolve_spell_as_permanent(
        &mut self,
        stack_object: ObjectRef,
        destination_replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<PermanentResolutionEvidence, LinkedCastCostRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.resolve_spell_as_permanent_inner(
            stack_object,
            destination_replacements,
            replacement_order,
        )?;
        *self = staged;
        Ok(evidence)
    }

    fn resolve_spell_as_permanent_inner(
        &mut self,
        stack_object: ObjectRef,
        destination_replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<PermanentResolutionEvidence, LinkedCastCostRuntimeError> {
        let spell = self
            .require_exact_object(stack_object, Zone::Stack)?
            .clone();
        if !spell.is_permanent_spell() {
            return Err(LinkedCastCostRuntimeError::NotAPermanentSpell);
        }
        let method = spell.cast_method.clone();
        let controller = spell
            .controller
            .ok_or(LinkedCastCostRuntimeError::InvalidObjectState)?;
        let change = self.move_object(
            stack_object,
            Zone::Battlefield,
            &destination_replacements,
            &replacement_order,
        )?;
        let permanent = (change.actual_destination == Zone::Battlefield).then_some(change.after);
        let mut evoke_trigger = None;
        let mut blitz_marker_attached = false;
        if let Some(permanent) = permanent {
            match method {
                Some(CastMethod::ResidualEvoke { semantic_digest }) => {
                    let trigger_id = self.next_trigger_id()?;
                    self.pending_triggers.insert(
                        trigger_id,
                        PendingLinkedTrigger::EvokeSacrifice(EvokeSacrificeTrigger {
                            trigger_id,
                            controller,
                            expected_permanent: permanent,
                            semantic_digest,
                        }),
                    );
                    evoke_trigger = Some(trigger_id);
                }
                Some(CastMethod::Blitz { semantic_digest }) => {
                    self.blitz_markers.insert(
                        permanent,
                        BlitzMarker {
                            controller,
                            semantic_digest,
                        },
                    );
                    blitz_marker_attached = true;
                }
                Some(
                    CastMethod::Spectacle { .. }
                    | CastMethod::Surge { .. }
                    | CastMethod::Prowl { .. },
                )
                | None => {}
            }
        }
        Ok(PermanentResolutionEvidence {
            stack_zone_change: change,
            permanent,
            evoke_trigger,
            blitz_marker_attached,
        })
    }

    pub fn leave_stack(
        &mut self,
        stack_object: ObjectRef,
        requested_destination: Zone,
        destination_replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<ZoneChangeEvidence, LinkedCastCostRuntimeError> {
        if matches!(requested_destination, Zone::Stack | Zone::Battlefield) {
            return Err(LinkedCastCostRuntimeError::InvalidDestination);
        }
        let mut staged = self.clone();
        staged.require_exact_object(stack_object, Zone::Stack)?;
        let evidence = staged.move_object(
            stack_object,
            requested_destination,
            &destination_replacements,
            &replacement_order,
        )?;
        *self = staged;
        Ok(evidence)
    }

    pub fn begin_end_step(
        &mut self,
        turn: TurnId,
    ) -> Result<Vec<BlitzSacrificeTrigger>, LinkedCastCostRuntimeError> {
        let mut staged = self.clone();
        let triggers = staged.begin_end_step_inner(turn)?;
        *self = staged;
        Ok(triggers)
    }

    fn begin_end_step_inner(
        &mut self,
        turn: TurnId,
    ) -> Result<Vec<BlitzSacrificeTrigger>, LinkedCastCostRuntimeError> {
        let due = self
            .blitz_delayed_sacrifices
            .iter()
            .filter(|(_, delayed)| delayed.cast_turn.0 <= turn.0)
            .map(|(key, delayed)| (*key, delayed.clone()))
            .collect::<Vec<_>>();
        let mut triggers = Vec::with_capacity(due.len());
        for (key, delayed) in due {
            self.blitz_delayed_sacrifices.remove(&key);
            let trigger_id = self.next_trigger_id()?;
            let trigger = BlitzSacrificeTrigger {
                trigger_id,
                controller: delayed.controller,
                expected_permanent: delayed.expected_permanent,
                cast_turn: delayed.cast_turn,
                semantic_digest: delayed.semantic_digest,
            };
            self.pending_triggers.insert(
                trigger_id,
                PendingLinkedTrigger::BlitzSacrifice(trigger.clone()),
            );
            triggers.push(trigger);
        }
        Ok(triggers)
    }

    pub fn resolve_evoke_sacrifice(
        &mut self,
        trigger_id: PendingTriggerId,
        destination_replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<SacrificeResolution, LinkedCastCostRuntimeError> {
        let mut staged = self.clone();
        let resolution = staged.resolve_sacrifice_trigger_inner(
            trigger_id,
            false,
            destination_replacements,
            replacement_order,
        )?;
        *self = staged;
        Ok(resolution)
    }

    pub fn resolve_blitz_sacrifice(
        &mut self,
        trigger_id: PendingTriggerId,
        destination_replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<SacrificeResolution, LinkedCastCostRuntimeError> {
        let mut staged = self.clone();
        let resolution = staged.resolve_sacrifice_trigger_inner(
            trigger_id,
            true,
            destination_replacements,
            replacement_order,
        )?;
        *self = staged;
        Ok(resolution)
    }

    fn resolve_sacrifice_trigger_inner(
        &mut self,
        trigger_id: PendingTriggerId,
        blitz: bool,
        destination_replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<SacrificeResolution, LinkedCastCostRuntimeError> {
        let pending = self
            .pending_triggers
            .get(&trigger_id)
            .cloned()
            .ok_or(LinkedCastCostRuntimeError::MissingPendingTrigger)?;
        let expected = match (blitz, pending) {
            (false, PendingLinkedTrigger::EvokeSacrifice(trigger)) => trigger.expected_permanent,
            (true, PendingLinkedTrigger::BlitzSacrifice(trigger)) => trigger.expected_permanent,
            _ => return Err(LinkedCastCostRuntimeError::WrongTriggerKind),
        };
        self.pending_triggers.remove(&trigger_id);
        if self
            .object(expected)
            .is_none_or(|object| object.zone != Zone::Battlefield)
        {
            return Ok(SacrificeResolution::SourceNoLongerThatPermanent);
        }
        let change = self.move_object(
            expected,
            Zone::Graveyard,
            &destination_replacements,
            &replacement_order,
        )?;
        Ok(SacrificeResolution::Moved(change))
    }

    pub fn resolve_blitz_death_draw(
        &mut self,
        trigger_id: PendingTriggerId,
        destination_replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<DrawResolution, LinkedCastCostRuntimeError> {
        let mut staged = self.clone();
        let resolution = staged.resolve_blitz_death_draw_inner(
            trigger_id,
            destination_replacements,
            replacement_order,
        )?;
        *self = staged;
        Ok(resolution)
    }

    fn resolve_blitz_death_draw_inner(
        &mut self,
        trigger_id: PendingTriggerId,
        destination_replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<DrawResolution, LinkedCastCostRuntimeError> {
        let pending = self
            .pending_triggers
            .get(&trigger_id)
            .cloned()
            .ok_or(LinkedCastCostRuntimeError::MissingPendingTrigger)?;
        let PendingLinkedTrigger::BlitzDeathDraw(trigger) = pending else {
            return Err(LinkedCastCostRuntimeError::WrongTriggerKind);
        };
        let player = trigger.controller;
        let top_id = self
            .players
            .get(&player)
            .ok_or(LinkedCastCostRuntimeError::UnknownPlayer)?
            .library_order
            .first()
            .copied();
        self.pending_triggers.remove(&trigger_id);
        let Some(top_id) = top_id else {
            self.players
                .get_mut(&player)
                .expect("validated player")
                .failed_draws += 1;
            return Ok(DrawResolution::EmptyLibrary);
        };
        let top = self
            .objects
            .get(&top_id)
            .ok_or(LinkedCastCostRuntimeError::InvalidLibraryOrder)?;
        if top.owner != player || top.zone != Zone::Library {
            return Err(LinkedCastCostRuntimeError::InvalidLibraryOrder);
        }
        let top_ref = top.object_ref;
        let change = self.move_object(
            top_ref,
            Zone::Hand,
            &destination_replacements,
            &replacement_order,
        )?;
        let state = self.players.get_mut(&player).expect("validated player");
        state.library_order.remove(0);
        if change.actual_destination == Zone::Hand {
            state.cards_drawn += 1;
            Ok(DrawResolution::Drew(change))
        } else {
            Ok(DrawResolution::DrawReplaced(change))
        }
    }

    pub fn blitz_grants_haste(&self, permanent: ObjectRef) -> bool {
        self.object(permanent)
            .is_some_and(|object| object.zone == Zone::Battlefield)
            && self.blitz_markers.contains_key(&permanent)
    }

    pub fn change_controller(
        &mut self,
        permanent: ObjectRef,
        controller: PlayerId,
    ) -> Result<(), LinkedCastCostRuntimeError> {
        if !self.players.contains_key(&controller) {
            return Err(LinkedCastCostRuntimeError::UnknownPlayer);
        }
        let object = self.require_exact_object_mut(permanent, Zone::Battlefield)?;
        object.controller = Some(controller);
        if let Some(marker) = self.blitz_markers.get_mut(&permanent) {
            marker.controller = controller;
        }
        Ok(())
    }

    pub fn apply_external_zone_change(
        &mut self,
        source: ObjectRef,
        requested_destination: Zone,
        destination_replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<ZoneChangeEvidence, LinkedCastCostRuntimeError> {
        let mut staged = self.clone();
        staged
            .objects
            .get(&source.object_id)
            .filter(|object| object.object_ref == source)
            .ok_or(LinkedCastCostRuntimeError::MissingObject(source))?;
        let evidence = staged.move_object(
            source,
            requested_destination,
            &destination_replacements,
            &replacement_order,
        )?;
        *self = staged;
        Ok(evidence)
    }

    pub fn copy_spell(
        &mut self,
        source_spell: ObjectRef,
        copy_id: ObjectId,
    ) -> Result<ObjectRef, LinkedCastCostRuntimeError> {
        let source = self
            .require_exact_object(source_spell, Zone::Stack)?
            .clone();
        if self.objects.contains_key(&copy_id) {
            return Err(LinkedCastCostRuntimeError::DuplicateObject(copy_id));
        }
        let copy_ref = ObjectRef {
            object_id: copy_id,
            incarnation_id: IncarnationId(1),
        };
        self.objects.insert(
            copy_id,
            TrackedCard {
                object_ref: copy_ref,
                owner: source.owner,
                controller: source.controller,
                zone: Zone::Stack,
                card_types: source.card_types,
                creature_types: source.creature_types,
                colors: source.colors,
                printed_mana_cost: source.printed_mana_cost,
                cast_method: None,
                is_copy: true,
                copied_from: Some(source_spell),
            },
        );
        Ok(copy_ref)
    }

    fn pay_mana(
        &mut self,
        player: PlayerId,
        cost: &ManaCost,
        payment: &ManaPayment,
    ) -> Result<ManaPaymentEvidence, LinkedCastCostRuntimeError> {
        if !cost.contains_x() && payment.x_value != 0 {
            return Err(LinkedCastCostRuntimeError::ManaPaymentMismatch);
        }
        let state = self
            .players
            .get(&player)
            .ok_or(LinkedCastCostRuntimeError::UnknownPlayer)?;
        let mut seen = BTreeSet::new();
        let units = payment
            .mana_units
            .iter()
            .map(|id| {
                if !seen.insert(*id) {
                    return Err(LinkedCastCostRuntimeError::DuplicateManaUnit(*id));
                }
                state
                    .mana_pool
                    .get(id)
                    .copied()
                    .ok_or(LinkedCastCostRuntimeError::MissingManaUnit(*id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !mana_units_match_cost(cost, payment.x_value, &units) {
            return Err(LinkedCastCostRuntimeError::ManaPaymentMismatch);
        }
        let state = self
            .players
            .get_mut(&player)
            .expect("player validated before payment");
        for id in &payment.mana_units {
            state.mana_pool.remove(id);
        }
        Ok(ManaPaymentEvidence {
            exact_cost: cost.exact.clone(),
            x_value: payment.x_value,
            mana_units_spent: payment.mana_units.clone(),
        })
    }

    fn move_object(
        &mut self,
        source: ObjectRef,
        requested_destination: Zone,
        replacements: &[ExternalZoneReplacement],
        replacement_order: &[ReplacementEffectId],
    ) -> Result<ZoneChangeEvidence, LinkedCastCostRuntimeError> {
        let current = self
            .objects
            .get(&source.object_id)
            .filter(|object| object.object_ref == source)
            .cloned()
            .ok_or(LinkedCastCostRuntimeError::MissingObject(source))?;
        let (actual_destination, applications) =
            apply_replacements(requested_destination, replacements, replacement_order)?;
        let next_incarnation = current
            .object_ref
            .incarnation_id
            .0
            .checked_add(1)
            .ok_or(LinkedCastCostRuntimeError::IncarnationOverflow)?;
        let after = ObjectRef {
            object_id: source.object_id,
            incarnation_id: IncarnationId(next_incarnation),
        };
        let dying_blitz_marker = self.blitz_markers.remove(&source);
        let tracked = self
            .objects
            .get_mut(&source.object_id)
            .expect("current object was cloned");
        tracked.object_ref = after;
        tracked.zone = actual_destination;
        tracked.cast_method = None;
        tracked.controller = if matches!(actual_destination, Zone::Battlefield | Zone::Stack) {
            current.controller.or(Some(current.owner))
        } else {
            None
        };
        if current.zone == Zone::Battlefield
            && actual_destination == Zone::Graveyard
            && let Some(marker) = dying_blitz_marker
        {
            let controller = current.controller.unwrap_or(marker.controller);
            let trigger_id = self.next_trigger_id()?;
            self.pending_triggers.insert(
                trigger_id,
                PendingLinkedTrigger::BlitzDeathDraw(BlitzDeathDrawTrigger {
                    trigger_id,
                    controller,
                    source_before_death: source,
                    semantic_digest: marker.semantic_digest,
                }),
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

    fn require_exact_object(
        &self,
        object: ObjectRef,
        zone: Zone,
    ) -> Result<&TrackedCard, LinkedCastCostRuntimeError> {
        let tracked = self
            .objects
            .get(&object.object_id)
            .filter(|tracked| tracked.object_ref == object)
            .ok_or(LinkedCastCostRuntimeError::MissingObject(object))?;
        if tracked.zone != zone {
            return Err(LinkedCastCostRuntimeError::WrongZone {
                expected: zone,
                actual: tracked.zone,
            });
        }
        Ok(tracked)
    }

    fn require_exact_object_mut(
        &mut self,
        object: ObjectRef,
        zone: Zone,
    ) -> Result<&mut TrackedCard, LinkedCastCostRuntimeError> {
        let tracked = self
            .objects
            .get_mut(&object.object_id)
            .filter(|tracked| tracked.object_ref == object)
            .ok_or(LinkedCastCostRuntimeError::MissingObject(object))?;
        if tracked.zone != zone {
            return Err(LinkedCastCostRuntimeError::WrongZone {
                expected: zone,
                actual: tracked.zone,
            });
        }
        Ok(tracked)
    }

    fn next_trigger_id(&mut self) -> Result<PendingTriggerId, LinkedCastCostRuntimeError> {
        let trigger_id = PendingTriggerId(self.next_trigger_id);
        self.next_trigger_id = self
            .next_trigger_id
            .checked_add(1)
            .ok_or(LinkedCastCostRuntimeError::TriggerIdOverflow)?;
        Ok(trigger_id)
    }
}

fn cast_method(program: &LinkedCastCostProgram) -> CastMethod {
    let semantic_digest = program.semantic_digest().to_owned();
    match program.kind() {
        LinkedCastCostKind::ResidualEvoke(_) => CastMethod::ResidualEvoke { semantic_digest },
        LinkedCastCostKind::Blitz(_) => CastMethod::Blitz { semantic_digest },
        LinkedCastCostKind::Spectacle(_) => CastMethod::Spectacle { semantic_digest },
        LinkedCastCostKind::Surge(_) => CastMethod::Surge { semantic_digest },
        LinkedCastCostKind::Prowl(_) => CastMethod::Prowl { semantic_digest },
    }
}

fn apply_replacements(
    requested_destination: Zone,
    replacements: &[ExternalZoneReplacement],
    replacement_order: &[ReplacementEffectId],
) -> Result<(Zone, Vec<ReplacementApplicationEvidence>), LinkedCastCostRuntimeError> {
    let replacement_ids = replacements
        .iter()
        .map(|replacement| replacement.effect_id)
        .collect::<BTreeSet<_>>();
    let order_ids = replacement_order.iter().copied().collect::<BTreeSet<_>>();
    if replacement_ids.len() != replacements.len()
        || order_ids.len() != replacement_order.len()
        || replacement_ids != order_ids
    {
        return Err(LinkedCastCostRuntimeError::ReplacementOrderMismatch);
    }
    let by_id = replacements
        .iter()
        .map(|replacement| (replacement.effect_id, replacement))
        .collect::<BTreeMap<_, _>>();
    let mut destination = requested_destination;
    let mut applications = Vec::new();
    for effect_id in replacement_order {
        let replacement = by_id
            .get(effect_id)
            .ok_or(LinkedCastCostRuntimeError::ReplacementOrderMismatch)?;
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
            other => specific.push(*other),
        }
    }
    let Ok(generic_count) = usize::try_from(generic) else {
        return false;
    };
    if units.len() != specific.len().saturating_add(generic_count) {
        return false;
    }
    fn match_specific(
        index: usize,
        symbols: &[ManaSymbol],
        units: &[ManaUnit],
        used: &mut [bool],
    ) -> bool {
        if index == symbols.len() {
            return true;
        }
        for unit_index in 0..units.len() {
            if !used[unit_index] && mana_unit_matches_symbol(units[unit_index], symbols[index]) {
                used[unit_index] = true;
                if match_specific(index + 1, symbols, units, used) {
                    return true;
                }
                used[unit_index] = false;
            }
        }
        false
    }
    match_specific(0, &specific, units, &mut vec![false; units.len()])
}

fn mana_unit_matches_symbol(unit: ManaUnit, symbol: ManaSymbol) -> bool {
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
pub enum LinkedCastCostRuntimeError {
    DuplicatePlayer(PlayerId),
    DuplicateObject(ObjectId),
    InvalidPlayerState,
    InvalidCharacteristics,
    InvalidObjectState,
    UnknownPlayer,
    MissingObject(ObjectRef),
    WrongZone { expected: Zone, actual: Zone },
    MissingNormalCastPermission,
    CastIsNotLegal,
    CopiesAreNotCastByThisRuntime,
    SourceContextMismatch,
    AlternativeCostConditionNotMet,
    MissingManaUnit(ManaUnitId),
    DuplicateManaUnit(ManaUnitId),
    ManaPaymentMismatch,
    LifePaymentMismatch,
    CannotPayLife,
    UnexpectedAdditionalCard,
    MissingAdditionalCard,
    InvalidAdditionalCard,
    SourceCannotPayItsOwnCost,
    ReplacementOrderMismatch,
    IncarnationOverflow,
    TriggerIdOverflow,
    NotAPermanentSpell,
    InvalidDestination,
    MissingPendingTrigger,
    WrongTriggerKind,
    InvalidLibraryOrder,
    InvalidEvent,
}

impl fmt::Display for LinkedCastCostRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LinkedCastCostRuntimeError {}
