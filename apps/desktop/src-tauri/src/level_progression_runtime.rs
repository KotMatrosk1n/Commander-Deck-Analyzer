//! Complete face programs for the Level up keyword and its level symbols.
//!
//! A face is accepted only when its full Oracle block is a canonical leveler
//! face and every base or level-band ability has a typed child program. The
//! production adapter remains disconnected until the main simulator can
//! provide complete activation, stack-resolution, payment, counter, layer,
//! incarnation, and state based action evidence required by this runtime.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fmt;

use sha2::{Digest, Sha256};

pub const LEVEL_PROGRESSION_COMPILER_VERSION: &str = "level-progression-compiler-0.2";
pub const LEVEL_PROGRESSION_RUNTIME_VERSION: &str = "level-progression-runtime-0.2";
pub const LEVEL_PROGRESSION_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:117.3b,117.5,602.2,608.2,613.1,613.4,613.5,704.5f,704.5g,701.27,702.87";

pub const fn level_progression_production_adapter_connected() -> bool {
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
pub enum FacePermanentKind {
    Creature,
    Land,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelProgressionFaceInput {
    pub exact_oracle_text: String,
    pub exact_layout: String,
    pub exact_type_line: String,
    pub printed_power: Option<i32>,
    pub printed_toughness: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelProgressionProgram {
    exact_oracle_text: String,
    exact_layout: String,
    exact_type_line: String,
    permanent_kind: FacePermanentKind,
    printed_power_toughness: Option<PowerToughness>,
    level_up: LevelUpAbilityProgram,
    base_children: Vec<TypedLevelChild>,
    bands: Vec<LevelBandProgram>,
    semantic_sha256: String,
}

impl LevelProgressionProgram {
    pub fn exact_oracle_text(&self) -> &str {
        &self.exact_oracle_text
    }

    pub fn exact_layout(&self) -> &str {
        &self.exact_layout
    }

    pub fn exact_type_line(&self) -> &str {
        &self.exact_type_line
    }

    pub fn permanent_kind(&self) -> FacePermanentKind {
        self.permanent_kind
    }

    pub fn printed_power_toughness(&self) -> Option<PowerToughness> {
        self.printed_power_toughness
    }

    pub fn level_up(&self) -> &LevelUpAbilityProgram {
        &self.level_up
    }

    pub fn base_children(&self) -> &[TypedLevelChild] {
        &self.base_children
    }

    pub fn bands(&self) -> &[LevelBandProgram] {
        &self.bands
    }

    pub fn semantic_sha256(&self) -> &str {
        &self.semantic_sha256
    }

    pub fn covered_clause_count(&self) -> usize {
        self.exact_oracle_text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()
    }

    pub fn has_exact_contract(&self) -> bool {
        !self.exact_oracle_text.trim().is_empty()
            && self.exact_layout == "leveler"
            && !self.exact_type_line.trim().is_empty()
            && !self.bands.is_empty()
            && is_sha256_hex(&self.semantic_sha256)
            && self.semantic_sha256 == semantic_digest(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelUpAbilityProgram {
    pub exact_source: String,
    pub exact_cost: String,
    pub cost: ActivationCost,
    pub source_must_be_controlled_battlefield_permanent: bool,
    pub sorcery_timing_only: bool,
    pub puts_one_level_counter: bool,
    pub uses_stack: bool,
    pub repeatable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationCost {
    pub exact: String,
    pub components: Vec<CostComponent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostComponent {
    Mana(ManaCost),
    TapSource,
    UntapSource,
    PayLife(u32),
    DiscardCard,
    SacrificeSource,
    SacrificeCreature,
    RemoveCounterFromSource { count: u32, counter_name: String },
    ExileCardFromYourGraveyard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaCost {
    pub exact: String,
    pub symbols: Vec<ManaSymbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaSymbol {
    Generic(u32),
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
    Snow,
    VariableX,
    Hybrid(String, String),
    Phyrexian(String),
    TwoBrid(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerToughness {
    pub power: i32,
    pub toughness: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelRange {
    Inclusive { minimum: u32, maximum: u32 },
    AtLeast { minimum: u32 },
}

impl LevelRange {
    pub fn contains(self, counters: u32) -> bool {
        match self {
            Self::Inclusive { minimum, maximum } => (minimum..=maximum).contains(&counters),
            Self::AtLeast { minimum } => counters >= minimum,
        }
    }

    fn minimum(self) -> u32 {
        match self {
            Self::Inclusive { minimum, .. } | Self::AtLeast { minimum } => minimum,
        }
    }

    fn maximum(self) -> Option<u32> {
        match self {
            Self::Inclusive { maximum, .. } => Some(maximum),
            Self::AtLeast { .. } => None,
        }
    }

    fn stable_id(self) -> String {
        match self {
            Self::Inclusive { minimum, maximum } => format!("{minimum}-{maximum}"),
            Self::AtLeast { minimum } => format!("{minimum}+"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelBandProgram {
    pub exact_header: String,
    pub range: LevelRange,
    pub exact_power_toughness: Option<String>,
    pub power_toughness: Option<PowerToughness>,
    pub children: Vec<TypedLevelChild>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedLevelChild {
    pub exact_source: String,
    pub kind: LevelChildKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LevelChildKind {
    KeywordLine(Vec<KeywordAbility>),
    Activated(ActivatedAbilityProgram),
    Triggered(TriggeredAbilityProgram),
    Static(StaticAbilityProgram),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordAbility {
    Flying,
    FirstStrike,
    DoubleStrike,
    Lifelink,
    Indestructible,
    Vigilance,
    Deathtouch,
    Shroud,
    Trample,
    Islandwalk,
    Protection(ProtectionQuality),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectionQuality {
    Instants,
    Everything,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivatedAbilityProgram {
    pub cost: ActivationCost,
    pub effect: ActivatedEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivatedEffect {
    DealDamageToAnyTarget {
        amount: u32,
    },
    AddMana {
        choices: Vec<ManaProductionChoice>,
        amount_per_choice: u32,
        then_scry: Option<u32>,
    },
    DrawThenDiscard {
        draw: u32,
        discard: u32,
    },
    DrawCards(u32),
    TargetCreaturePowerToughnessUntilEndOfTurn {
        power: i32,
        toughness: i32,
    },
    RegenerateSource,
    SourcePowerToughnessUntilEndOfTurn {
        power: i32,
        toughness: i32,
    },
    CreateCreatureTokens {
        count: u32,
        power: i32,
        toughness: i32,
        color: String,
        subtype: String,
    },
    CopyTargetInstantOrSorcery {
        copies: u32,
        may_choose_new_targets: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManaProductionChoice {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggeredAbilityProgram {
    pub event: TriggerEvent,
    pub intervening_condition: Option<TriggerCondition>,
    pub effect: TriggeredEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerEvent {
    BeginningOfEachEndStep,
    SourceAttacks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerCondition {
    NotYourTurn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggeredEffect {
    TakeExtraTurnAfterThisOne,
    DealDamageToEachCreatureDefendingPlayerControls { amount: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticAbilityProgram {
    Unblockable,
    BlockableOnlyByBlackCreatures,
    OtherControlledCreaturesGet {
        subtype: Option<String>,
        power: i32,
        toughness: i32,
    },
    ControlledSubtypeCreaturesHaveManaAbility {
        subtype: String,
        produced: Vec<ManaProductionChoice>,
    },
    PreventDamageToYouOrControlledCreature {
        amount: u32,
    },
    ActivateOpponentArtifactAbilitiesAsThoughControlled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LevelProgressionCompileError {
    EmptyOracleText,
    GrantedOrModifierReference,
    NonCanonicalOrSilverBorderLevelUp,
    UnsupportedLayout(String),
    UnsupportedTypeLine(String),
    MissingPrintedCharacteristics,
    MalformedLevelUpLine,
    UnsupportedActivationCost(String),
    DetachedLevelBand(String),
    MalformedLevelHeader(String),
    MalformedPowerToughness(String),
    MissingBandPowerToughness(String),
    UnexpectedBandPowerToughness(String),
    OverlappingOrDiscontinuousBands,
    MissingOpenEndedFinalBand,
    UnsupportedChildAbility(String),
}

impl fmt::Display for LevelProgressionCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LevelProgressionCompileError {}

pub fn compile_level_progression_face(
    input: LevelProgressionFaceInput,
) -> Result<LevelProgressionProgram, LevelProgressionCompileError> {
    if input.exact_oracle_text.trim().is_empty() {
        return Err(LevelProgressionCompileError::EmptyOracleText);
    }

    let exact_lines = input
        .exact_oracle_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let first_line = exact_lines
        .first()
        .ok_or(LevelProgressionCompileError::EmptyOracleText)?;

    if !first_line.starts_with("Level up ") {
        if first_line.starts_with("LEVEL ") {
            return Err(LevelProgressionCompileError::DetachedLevelBand(
                first_line.clone(),
            ));
        }
        if exact_lines.iter().any(|line| line.starts_with("LEVEL ")) {
            return Err(LevelProgressionCompileError::NonCanonicalOrSilverBorderLevelUp);
        }
        if contains_keyword_word(&input.exact_oracle_text, "level up") {
            return Err(LevelProgressionCompileError::GrantedOrModifierReference);
        }
        return Err(LevelProgressionCompileError::MalformedLevelUpLine);
    }

    if input.exact_layout != "leveler" {
        return Err(LevelProgressionCompileError::NonCanonicalOrSilverBorderLevelUp);
    }

    let permanent_kind = permanent_kind(&input.exact_type_line)?;
    let printed_power_toughness = match permanent_kind {
        FacePermanentKind::Creature => Some(PowerToughness {
            power: input
                .printed_power
                .ok_or(LevelProgressionCompileError::MissingPrintedCharacteristics)?,
            toughness: input
                .printed_toughness
                .ok_or(LevelProgressionCompileError::MissingPrintedCharacteristics)?,
        }),
        FacePermanentKind::Land => {
            if input.printed_power.is_some() || input.printed_toughness.is_some() {
                return Err(LevelProgressionCompileError::UnexpectedBandPowerToughness(
                    input.exact_type_line.clone(),
                ));
            }
            None
        }
    };

    let level_up = parse_level_up_ability(first_line)?;
    let first_header = exact_lines
        .iter()
        .position(|line| line.starts_with("LEVEL "))
        .ok_or_else(|| {
            LevelProgressionCompileError::DetachedLevelBand(
                "the canonical Level up face has no level symbol".into(),
            )
        })?;

    let base_children = exact_lines[1..first_header]
        .iter()
        .map(|line| compile_typed_child(line))
        .collect::<Result<Vec<_>, _>>()?;
    let bands = parse_level_bands(&exact_lines[first_header..], permanent_kind)?;
    validate_level_bands(&bands)?;

    let mut program = LevelProgressionProgram {
        exact_oracle_text: input.exact_oracle_text,
        exact_layout: input.exact_layout,
        exact_type_line: input.exact_type_line,
        permanent_kind,
        printed_power_toughness,
        level_up,
        base_children,
        bands,
        semantic_sha256: String::new(),
    };
    program.semantic_sha256 = semantic_digest(&program);
    if !program.has_exact_contract() {
        return Err(LevelProgressionCompileError::MalformedLevelUpLine);
    }
    Ok(program)
}

fn permanent_kind(
    exact_type_line: &str,
) -> Result<FacePermanentKind, LevelProgressionCompileError> {
    let card_types = exact_type_line
        .split(['\u{2014}', '-'])
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .map(|word| word.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    if card_types.contains("creature") {
        Ok(FacePermanentKind::Creature)
    } else if card_types == BTreeSet::from(["land".to_owned()]) {
        Ok(FacePermanentKind::Land)
    } else {
        Err(LevelProgressionCompileError::UnsupportedTypeLine(
            exact_type_line.to_owned(),
        ))
    }
}

fn parse_level_up_ability(
    exact_line: &str,
) -> Result<LevelUpAbilityProgram, LevelProgressionCompileError> {
    let after_prefix = exact_line
        .strip_prefix("Level up ")
        .ok_or(LevelProgressionCompileError::MalformedLevelUpLine)?;
    let (exact_cost, reminder) = after_prefix
        .split_once(" (")
        .ok_or(LevelProgressionCompileError::MalformedLevelUpLine)?;
    let expected_reminder =
        format!("{exact_cost}: Put a level counter on this. Level up only as a sorcery.)");
    if exact_cost.is_empty() || reminder != expected_reminder {
        return Err(LevelProgressionCompileError::MalformedLevelUpLine);
    }
    let cost = parse_activation_cost(exact_cost)?;
    Ok(LevelUpAbilityProgram {
        exact_source: exact_line.to_owned(),
        exact_cost: exact_cost.to_owned(),
        cost,
        source_must_be_controlled_battlefield_permanent: true,
        sorcery_timing_only: true,
        puts_one_level_counter: true,
        uses_stack: true,
        repeatable: true,
    })
}

fn parse_activation_cost(exact_cost: &str) -> Result<ActivationCost, LevelProgressionCompileError> {
    let raw_components = split_cost_components(exact_cost);
    let mut components = Vec::with_capacity(raw_components.len());
    for raw in raw_components {
        let component = if raw == "{T}" {
            CostComponent::TapSource
        } else if raw == "{Q}" {
            CostComponent::UntapSource
        } else if is_mana_cost(raw) {
            CostComponent::Mana(parse_mana_cost(raw)?)
        } else if let Some(amount) = raw
            .strip_prefix("Pay ")
            .and_then(|rest| rest.strip_suffix(" life"))
            .and_then(|value| value.parse::<u32>().ok())
        {
            CostComponent::PayLife(amount)
        } else if raw == "Discard a card" {
            CostComponent::DiscardCard
        } else if raw == "Sacrifice this permanent" {
            CostComponent::SacrificeSource
        } else if raw == "Sacrifice a creature" {
            CostComponent::SacrificeCreature
        } else if raw == "Exile a card from your graveyard" {
            CostComponent::ExileCardFromYourGraveyard
        } else if let Some(rest) = raw.strip_prefix("Remove a ")
            && let Some(counter_name) = rest.strip_suffix(" counter from this")
            && !counter_name.trim().is_empty()
        {
            CostComponent::RemoveCounterFromSource {
                count: 1,
                counter_name: counter_name.to_owned(),
            }
        } else {
            return Err(LevelProgressionCompileError::UnsupportedActivationCost(
                raw.to_owned(),
            ));
        };
        components.push(component);
    }
    if components.is_empty() {
        return Err(LevelProgressionCompileError::UnsupportedActivationCost(
            exact_cost.to_owned(),
        ));
    }
    Ok(ActivationCost {
        exact: exact_cost.to_owned(),
        components,
    })
}

fn split_cost_components(exact_cost: &str) -> Vec<&str> {
    let mut components = Vec::new();
    let mut start = 0usize;
    let mut brace_depth = 0usize;
    let bytes = exact_cost.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => brace_depth += 1,
            b'}' => brace_depth = brace_depth.saturating_sub(1),
            b',' if brace_depth == 0 && bytes.get(index + 1).copied() == Some(b' ') => {
                components.push(exact_cost[start..index].trim());
                index += 1;
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    components.push(exact_cost[start..].trim());
    components
}

fn is_mana_cost(source: &str) -> bool {
    if source.is_empty() {
        return false;
    }
    let mut rest = source;
    while let Some(after_open) = rest.strip_prefix('{') {
        let Some(close) = after_open.find('}') else {
            return false;
        };
        if close == 0 {
            return false;
        }
        rest = &after_open[close + 1..];
    }
    rest.is_empty()
}

fn parse_mana_cost(exact: &str) -> Result<ManaCost, LevelProgressionCompileError> {
    let mut symbols = Vec::new();
    let mut rest = exact;
    while let Some(after_open) = rest.strip_prefix('{') {
        let close = after_open.find('}').ok_or_else(|| {
            LevelProgressionCompileError::UnsupportedActivationCost(exact.to_owned())
        })?;
        let token = &after_open[..close];
        let symbol = match token {
            "W" => ManaSymbol::White,
            "U" => ManaSymbol::Blue,
            "B" => ManaSymbol::Black,
            "R" => ManaSymbol::Red,
            "G" => ManaSymbol::Green,
            "C" => ManaSymbol::Colorless,
            "S" => ManaSymbol::Snow,
            "X" => ManaSymbol::VariableX,
            _ if token.chars().all(|character| character.is_ascii_digit()) => {
                ManaSymbol::Generic(token.parse().map_err(|_| {
                    LevelProgressionCompileError::UnsupportedActivationCost(exact.to_owned())
                })?)
            }
            _ if token.ends_with("/P") => {
                let color = token.trim_end_matches("/P");
                if !matches!(color, "W" | "U" | "B" | "R" | "G") {
                    return Err(LevelProgressionCompileError::UnsupportedActivationCost(
                        exact.to_owned(),
                    ));
                }
                ManaSymbol::Phyrexian(color.to_owned())
            }
            _ if token.starts_with("2/") => {
                let color = token.trim_start_matches("2/");
                if !matches!(color, "W" | "U" | "B" | "R" | "G") {
                    return Err(LevelProgressionCompileError::UnsupportedActivationCost(
                        exact.to_owned(),
                    ));
                }
                ManaSymbol::TwoBrid(color.to_owned())
            }
            _ if token.split('/').count() == 2 => {
                let mut halves = token.split('/');
                let left = halves.next().unwrap_or_default();
                let right = halves.next().unwrap_or_default();
                if !matches!(left, "W" | "U" | "B" | "R" | "G" | "C")
                    || !matches!(right, "W" | "U" | "B" | "R" | "G" | "C")
                {
                    return Err(LevelProgressionCompileError::UnsupportedActivationCost(
                        exact.to_owned(),
                    ));
                }
                ManaSymbol::Hybrid(left.to_owned(), right.to_owned())
            }
            _ => {
                return Err(LevelProgressionCompileError::UnsupportedActivationCost(
                    exact.to_owned(),
                ));
            }
        };
        symbols.push(symbol);
        rest = &after_open[close + 1..];
    }
    if !rest.is_empty() || symbols.is_empty() {
        return Err(LevelProgressionCompileError::UnsupportedActivationCost(
            exact.to_owned(),
        ));
    }
    Ok(ManaCost {
        exact: exact.to_owned(),
        symbols,
    })
}

fn parse_level_bands(
    lines: &[String],
    kind: FacePermanentKind,
) -> Result<Vec<LevelBandProgram>, LevelProgressionCompileError> {
    let mut bands = Vec::new();
    let mut cursor = 0usize;
    while cursor < lines.len() {
        let exact_header = lines[cursor].clone();
        let range = parse_level_header(&exact_header)?;
        cursor += 1;

        let (exact_power_toughness, power_toughness) = if cursor < lines.len()
            && !lines[cursor].starts_with("LEVEL ")
            && parse_power_toughness(&lines[cursor]).is_some()
        {
            let exact = lines[cursor].clone();
            let parsed = parse_power_toughness(&exact);
            cursor += 1;
            (Some(exact), parsed)
        } else {
            (None, None)
        };

        match (kind, power_toughness) {
            (FacePermanentKind::Creature, None) => {
                return Err(LevelProgressionCompileError::MissingBandPowerToughness(
                    exact_header,
                ));
            }
            (FacePermanentKind::Land, Some(_)) => {
                return Err(LevelProgressionCompileError::UnexpectedBandPowerToughness(
                    exact_header,
                ));
            }
            _ => {}
        }

        let child_start = cursor;
        while cursor < lines.len() && !lines[cursor].starts_with("LEVEL ") {
            cursor += 1;
        }
        let children = lines[child_start..cursor]
            .iter()
            .map(|line| compile_typed_child(line))
            .collect::<Result<Vec<_>, _>>()?;
        bands.push(LevelBandProgram {
            exact_header,
            range,
            exact_power_toughness,
            power_toughness,
            children,
        });
    }
    Ok(bands)
}

fn parse_level_header(exact: &str) -> Result<LevelRange, LevelProgressionCompileError> {
    let range = exact
        .strip_prefix("LEVEL ")
        .ok_or_else(|| LevelProgressionCompileError::MalformedLevelHeader(exact.to_owned()))?;
    if let Some(minimum) = range
        .strip_suffix('+')
        .and_then(|value| value.parse::<u32>().ok())
        && minimum > 0
    {
        return Ok(LevelRange::AtLeast { minimum });
    }
    if let Some((minimum, maximum)) = range.split_once('-')
        && let (Ok(minimum), Ok(maximum)) = (minimum.parse::<u32>(), maximum.parse::<u32>())
        && minimum > 0
        && maximum >= minimum
    {
        return Ok(LevelRange::Inclusive { minimum, maximum });
    }
    Err(LevelProgressionCompileError::MalformedLevelHeader(
        exact.to_owned(),
    ))
}

fn parse_power_toughness(exact: &str) -> Option<PowerToughness> {
    let (power, toughness) = exact.split_once('/')?;
    Some(PowerToughness {
        power: power.parse().ok()?,
        toughness: toughness.parse().ok()?,
    })
}

fn validate_level_bands(bands: &[LevelBandProgram]) -> Result<(), LevelProgressionCompileError> {
    if bands.is_empty()
        || !matches!(
            bands.last().map(|band| band.range),
            Some(LevelRange::AtLeast { .. })
        )
    {
        return Err(LevelProgressionCompileError::MissingOpenEndedFinalBand);
    }
    for pair in bands.windows(2) {
        let Some(previous_maximum) = pair[0].range.maximum() else {
            return Err(LevelProgressionCompileError::OverlappingOrDiscontinuousBands);
        };
        if previous_maximum.checked_add(1) != Some(pair[1].range.minimum()) {
            return Err(LevelProgressionCompileError::OverlappingOrDiscontinuousBands);
        }
    }
    Ok(())
}

fn compile_typed_child(exact: &str) -> Result<TypedLevelChild, LevelProgressionCompileError> {
    let kind = if let Some(keywords) = parse_keyword_line(exact) {
        LevelChildKind::KeywordLine(keywords)
    } else if let Some(activated) = parse_activated_ability(exact)? {
        LevelChildKind::Activated(activated)
    } else if let Some(triggered) = parse_triggered_ability(exact) {
        LevelChildKind::Triggered(triggered)
    } else if let Some(static_ability) = parse_static_ability(exact) {
        LevelChildKind::Static(static_ability)
    } else {
        return Err(LevelProgressionCompileError::UnsupportedChildAbility(
            exact.to_owned(),
        ));
    };
    Ok(TypedLevelChild {
        exact_source: exact.to_owned(),
        kind,
    })
}

fn parse_keyword_line(exact: &str) -> Option<Vec<KeywordAbility>> {
    let without_reminder = match exact {
        "Islandwalk (This creature can't be blocked as long as defending player controls an Island.)" => {
            "Islandwalk"
        }
        "Shroud (This creature can't be the target of spells or abilities.)" => "Shroud",
        _ => exact,
    };
    let mut keywords = Vec::new();
    for part in without_reminder.split(", ") {
        let keyword = match part {
            "Flying" => KeywordAbility::Flying,
            "First strike" => KeywordAbility::FirstStrike,
            "Double strike" => KeywordAbility::DoubleStrike,
            "Lifelink" => KeywordAbility::Lifelink,
            "indestructible" | "Indestructible" => KeywordAbility::Indestructible,
            "Vigilance" | "vigilance" => KeywordAbility::Vigilance,
            "Deathtouch" | "deathtouch" => KeywordAbility::Deathtouch,
            "Shroud" => KeywordAbility::Shroud,
            "Trample" | "trample" => KeywordAbility::Trample,
            "Islandwalk" => KeywordAbility::Islandwalk,
            "Protection from instants" => KeywordAbility::Protection(ProtectionQuality::Instants),
            "Protection from everything" => {
                KeywordAbility::Protection(ProtectionQuality::Everything)
            }
            _ => return None,
        };
        keywords.push(keyword);
    }
    (!keywords.is_empty()).then_some(keywords)
}

fn parse_activated_ability(
    exact: &str,
) -> Result<Option<ActivatedAbilityProgram>, LevelProgressionCompileError> {
    let Some((cost_source, effect_source)) = exact.split_once(": ") else {
        return Ok(None);
    };
    if !cost_source.starts_with('{') {
        return Ok(None);
    }
    let cost = parse_activation_cost(cost_source)?;
    let effect = if let Some(amount) = effect_source
        .strip_prefix("This creature deals ")
        .and_then(|rest| rest.strip_suffix(" damage to any target."))
        .and_then(|value| value.parse::<u32>().ok())
    {
        ActivatedEffect::DealDamageToAnyTarget { amount }
    } else if let Some(effect) = parse_add_mana_effect(effect_source) {
        effect
    } else if effect_source == "Draw a card, then discard a card." {
        ActivatedEffect::DrawThenDiscard {
            draw: 1,
            discard: 1,
        }
    } else if effect_source == "Draw a card." {
        ActivatedEffect::DrawCards(1)
    } else if let Some(pair) = effect_source
        .strip_prefix("Target creature gets ")
        .and_then(|rest| rest.strip_suffix(" until end of turn."))
        .and_then(parse_signed_power_toughness)
    {
        ActivatedEffect::TargetCreaturePowerToughnessUntilEndOfTurn {
            power: pair.power,
            toughness: pair.toughness,
        }
    } else if effect_source == "Regenerate this creature." {
        ActivatedEffect::RegenerateSource
    } else if let Some(pair) = effect_source
        .strip_prefix("This creature gets ")
        .and_then(|rest| rest.strip_suffix(" until end of turn."))
        .and_then(parse_signed_power_toughness)
    {
        ActivatedEffect::SourcePowerToughnessUntilEndOfTurn {
            power: pair.power,
            toughness: pair.toughness,
        }
    } else if let Some(effect) = parse_create_token_effect(effect_source) {
        effect
    } else if let Some(effect) = parse_copy_spell_effect(effect_source) {
        effect
    } else {
        return Err(LevelProgressionCompileError::UnsupportedChildAbility(
            exact.to_owned(),
        ));
    };
    Ok(Some(ActivatedAbilityProgram { cost, effect }))
}

fn parse_add_mana_effect(source: &str) -> Option<ActivatedEffect> {
    let (mana_sentence, then_scry) = if let Some(prefix) = source.strip_suffix(". Scry 1.") {
        (prefix, Some(1))
    } else {
        (source.strip_suffix('.')?, None)
    };
    let choices_source = mana_sentence.strip_prefix("Add ")?;
    let normalized_choices = choices_source.replace(", or ", ", ").replace(" or ", ", ");
    let raw_choices = normalized_choices.split(", ").collect::<Vec<_>>();
    let mut choices = Vec::new();
    let mut repeated_choice = None;
    for raw in raw_choices {
        if raw.starts_with('{') && raw.matches('{').count() > 1 {
            let mana = parse_produced_mana_sequence(raw)?;
            if mana.iter().all(|choice| *choice == mana[0]) {
                repeated_choice = Some((mana[0], mana.len() as u32));
            } else {
                return None;
            }
        } else {
            choices.push(parse_mana_production_choice(raw)?);
        }
    }
    let (choices, amount_per_choice) = if let Some((choice, amount)) = repeated_choice {
        if !choices.is_empty() {
            return None;
        }
        (vec![choice], amount)
    } else {
        (choices, 1)
    };
    (!choices.is_empty()).then_some(ActivatedEffect::AddMana {
        choices,
        amount_per_choice,
        then_scry,
    })
}

fn parse_produced_mana_sequence(source: &str) -> Option<Vec<ManaProductionChoice>> {
    let mut rest = source;
    let mut result = Vec::new();
    while let Some(after_open) = rest.strip_prefix('{') {
        let close = after_open.find('}')?;
        result.push(parse_mana_production_choice(&after_open[..close])?);
        rest = &after_open[close + 1..];
    }
    (rest.is_empty() && !result.is_empty()).then_some(result)
}

fn parse_mana_production_choice(source: &str) -> Option<ManaProductionChoice> {
    let token = source
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(source);
    match token {
        "W" => Some(ManaProductionChoice::White),
        "U" => Some(ManaProductionChoice::Blue),
        "B" => Some(ManaProductionChoice::Black),
        "R" => Some(ManaProductionChoice::Red),
        "G" => Some(ManaProductionChoice::Green),
        "C" => Some(ManaProductionChoice::Colorless),
        _ => None,
    }
}

fn parse_signed_power_toughness(source: &str) -> Option<PowerToughness> {
    let (power, toughness) = source.split_once('/')?;
    Some(PowerToughness {
        power: power.parse().ok()?,
        toughness: toughness.parse().ok()?,
    })
}

fn parse_create_token_effect(source: &str) -> Option<ActivatedEffect> {
    let body = source.strip_prefix("Create ")?;
    let body = body
        .strip_suffix(" creature token.")
        .or_else(|| body.strip_suffix(" creature tokens."))?;
    let (count, body) = if let Some(rest) = body.strip_prefix("a ") {
        (1, rest)
    } else {
        let rest = body.strip_prefix("two ")?;
        (2, rest)
    };
    let mut words = body.split_whitespace();
    let pair = parse_power_toughness(words.next()?)?;
    let color = words.next()?.to_owned();
    let subtype = words.next()?.to_owned();
    if words.next().is_some() {
        return None;
    }
    Some(ActivatedEffect::CreateCreatureTokens {
        count,
        power: pair.power,
        toughness: pair.toughness,
        color,
        subtype,
    })
}

fn parse_copy_spell_effect(source: &str) -> Option<ActivatedEffect> {
    let (copies, suffix) =
        if let Some(suffix) = source.strip_prefix("Copy target instant or sorcery spell twice. ") {
            (2, suffix)
        } else {
            let suffix = source.strip_prefix("Copy target instant or sorcery spell. ")?;
            (1, suffix)
        };
    let expected = if copies == 1 {
        "You may choose new targets for the copy."
    } else {
        "You may choose new targets for the copies."
    };
    (suffix == expected).then_some(ActivatedEffect::CopyTargetInstantOrSorcery {
        copies,
        may_choose_new_targets: true,
    })
}

fn parse_triggered_ability(exact: &str) -> Option<TriggeredAbilityProgram> {
    if exact
        == "At the beginning of each end step, if it's not your turn, take an extra turn after this one."
    {
        return Some(TriggeredAbilityProgram {
            event: TriggerEvent::BeginningOfEachEndStep,
            intervening_condition: Some(TriggerCondition::NotYourTurn),
            effect: TriggeredEffect::TakeExtraTurnAfterThisOne,
        });
    }
    let amount = exact
        .strip_prefix("Whenever this creature attacks, it deals ")
        .and_then(|rest| rest.strip_suffix(" damage to each creature defending player controls."))
        .and_then(|value| value.parse::<u32>().ok())?;
    Some(TriggeredAbilityProgram {
        event: TriggerEvent::SourceAttacks,
        intervening_condition: None,
        effect: TriggeredEffect::DealDamageToEachCreatureDefendingPlayerControls { amount },
    })
}

fn parse_static_ability(exact: &str) -> Option<StaticAbilityProgram> {
    match exact {
        "This creature can't be blocked." => return Some(StaticAbilityProgram::Unblockable),
        "This creature can't be blocked except by black creatures." => {
            return Some(StaticAbilityProgram::BlockableOnlyByBlackCreatures);
        }
        "You may activate abilities of artifacts your opponents control as though you control them." =>
        {
            return Some(StaticAbilityProgram::ActivateOpponentArtifactAbilitiesAsThoughControlled);
        }
        _ => {}
    }

    if let Some(rest) = exact.strip_prefix("Other ")
        && let Some((scope, modifier)) = rest.split_once(" you control get ")
        && let Some(pair) = modifier
            .strip_suffix('.')
            .and_then(parse_signed_power_toughness)
    {
        let subtype = if scope == "creatures" {
            None
        } else {
            scope
                .strip_suffix(" creatures")
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        };
        if scope == "creatures" || subtype.is_some() {
            return Some(StaticAbilityProgram::OtherControlledCreaturesGet {
                subtype,
                power: pair.power,
                toughness: pair.toughness,
            });
        }
    }

    if let Some((scope, quoted)) = exact.split_once(" you control have \"")
        && let Some(ability_source) = quoted.strip_suffix('"')
        && let Some((cost_source, effect_source)) = ability_source.split_once(": ")
        && cost_source == "{T}"
        && let Some(ActivatedEffect::AddMana {
            choices,
            amount_per_choice,
            then_scry: None,
        }) = parse_add_mana_effect(effect_source)
        && amount_per_choice == 2
    {
        return Some(
            StaticAbilityProgram::ControlledSubtypeCreaturesHaveManaAbility {
                subtype: scope.to_owned(),
                produced: choices,
            },
        );
    }

    if let Some(amount) = exact
        .strip_prefix("If a source would deal damage to you or a creature you control, prevent ")
        .and_then(|rest| rest.strip_suffix(" of that damage."))
        .and_then(|value| value.parse::<u32>().ok())
    {
        return Some(StaticAbilityProgram::PreventDamageToYouOrControlledCreature { amount });
    }
    None
}

fn semantic_digest(program: &LevelProgressionProgram) -> String {
    let mut fields = Vec::<(String, String)>::new();
    fields.push((
        "compiler".into(),
        LEVEL_PROGRESSION_COMPILER_VERSION.to_owned(),
    ));
    fields.push((
        "runtime".into(),
        LEVEL_PROGRESSION_RUNTIME_VERSION.to_owned(),
    ));
    fields.push((
        "rules".into(),
        LEVEL_PROGRESSION_RULES_CONTEXT_VERSION.to_owned(),
    ));
    fields.push(("oracle".into(), program.exact_oracle_text.clone()));
    fields.push(("layout".into(), program.exact_layout.clone()));
    fields.push(("type".into(), program.exact_type_line.clone()));
    fields.push((
        "kind".into(),
        match program.permanent_kind {
            FacePermanentKind::Creature => "creature",
            FacePermanentKind::Land => "land",
        }
        .to_owned(),
    ));
    fields.push((
        "printed-pt".into(),
        program
            .printed_power_toughness
            .map(|pair| format!("{}/{}", pair.power, pair.toughness))
            .unwrap_or_else(|| "none".into()),
    ));
    fields.push(("level-up".into(), level_up_stable_id(&program.level_up)));
    for (index, child) in program.base_children.iter().enumerate() {
        fields.push((format!("base-child-{index}"), child_stable_id(child)));
    }
    for (index, band) in program.bands.iter().enumerate() {
        fields.push((format!("band-{index}"), band_stable_id(band)));
    }
    sha256_hex(length_delimited_payload(&fields).as_bytes())
}

fn level_up_stable_id(program: &LevelUpAbilityProgram) -> String {
    format!(
        "source={};cost={};battlefield={};sorcery={};one={};stack={};repeatable={}",
        program.exact_source,
        cost_stable_id(&program.cost),
        program.source_must_be_controlled_battlefield_permanent,
        program.sorcery_timing_only,
        program.puts_one_level_counter,
        program.uses_stack,
        program.repeatable
    )
}

fn band_stable_id(band: &LevelBandProgram) -> String {
    let children = band
        .children
        .iter()
        .map(child_stable_id)
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "header={};range={};exact-pt={:?};pt={:?};children={children}",
        band.exact_header,
        band.range.stable_id(),
        band.exact_power_toughness,
        band.power_toughness
    )
}

fn child_stable_id(child: &TypedLevelChild) -> String {
    format!("source={};kind={:?}", child.exact_source, child.kind)
}

fn cost_stable_id(cost: &ActivationCost) -> String {
    format!("exact={};components={:?}", cost.exact, cost.components)
}

fn length_delimited_payload(fields: &[(String, String)]) -> String {
    let mut payload = String::new();
    for (key, value) in fields {
        payload.push_str(&format!("{}:{}{}:{};", key.len(), key, value.len(), value));
    }
    payload
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn contains_keyword_word(source: &str, keyword: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    let keyword = keyword.to_ascii_lowercase();
    let mut cursor = 0usize;
    while let Some(relative) = lower[cursor..].find(&keyword) {
        let start = cursor + relative;
        let end = start + keyword.len();
        let left_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
        let right_ok = end == lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
        if left_ok && right_ok {
            return true;
        }
        cursor = end;
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelProgressionObjectState {
    pub object: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub level_counters: u32,
    pub marked_damage: u32,
    pub was_dealt_deathtouch_damage: bool,
    pub indestructible: bool,
    pub copied_program_sha256: String,
    pub counter_history: Vec<LevelCounterChangeReceipt>,
}

pub fn new_level_progression_object(
    program: &LevelProgressionProgram,
    object: ObjectRef,
    owner: PlayerId,
    controller: PlayerId,
    zone: Zone,
) -> Result<LevelProgressionObjectState, LevelProgressionRuntimeError> {
    if !program.has_exact_contract() {
        return Err(LevelProgressionRuntimeError::InvalidProgramContract);
    }
    Ok(LevelProgressionObjectState {
        object,
        owner,
        controller,
        zone,
        level_counters: 0,
        marked_damage: 0,
        was_dealt_deathtouch_damage: false,
        indestructible: false,
        copied_program_sha256: program.semantic_sha256.clone(),
        counter_history: Vec::new(),
    })
}

pub fn copy_level_progression_object(
    program: &LevelProgressionProgram,
    copied: &LevelProgressionObjectState,
    new_object: ObjectRef,
    new_controller: PlayerId,
) -> Result<LevelProgressionObjectState, LevelProgressionRuntimeError> {
    verify_program_object(program, copied)?;
    Ok(LevelProgressionObjectState {
        object: new_object,
        owner: copied.owner,
        controller: new_controller,
        zone: copied.zone,
        level_counters: 0,
        marked_damage: 0,
        was_dealt_deathtouch_damage: false,
        // Counters, damage, and ordinary continuous effects are not copiable
        // values. The exact face program is copied, then the new object begins
        // with its own counter and effect history.
        indestructible: false,
        copied_program_sha256: program.semantic_sha256.clone(),
        counter_history: Vec::new(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SorceryTimingEvidence {
    pub active_player: PlayerId,
    pub phase: TurnPhase,
    pub stack_is_empty: bool,
    pub actor_has_priority: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnPhase {
    Beginning,
    PrecombatMain,
    Combat,
    PostcombatMain,
    Ending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostComponentPaymentReceipt {
    pub component_index: usize,
    pub exact_component: String,
    pub completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactCostPaymentEvidence {
    pub payer: PlayerId,
    pub exact_cost: String,
    pub components: Vec<CostComponentPaymentReceipt>,
    pub payment_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterReplacementStep {
    pub replacement_effect_sha256: String,
    pub incoming: u32,
    pub outgoing: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelCounterPlacementEvidence {
    pub event_id: u64,
    pub target: ObjectRef,
    pub requested: u32,
    pub replacement_steps: Vec<CounterReplacementStep>,
    pub placed: u32,
    pub before_total: u32,
    pub after_total: u32,
    pub replacement_trace_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelUpActivationEvidence {
    pub activation_id: u64,
    pub stack_object_id: u64,
    pub actor: PlayerId,
    pub source: ObjectRef,
    pub timing: SorceryTimingEvidence,
    pub level_up_ability_present: bool,
    pub payment: ExactCostPaymentEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingLevelUpAbility {
    pub runtime_version: &'static str,
    pub stack_object_id: u64,
    pub controller: PlayerId,
    pub source: ObjectRef,
    pub requested_level_counters: u32,
    pub copied_from_stack_object_id: Option<u64>,
    pub program_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelUpActivationReceipt {
    pub runtime_version: &'static str,
    pub activation_id: u64,
    pub stack_object_id: u64,
    pub actor: PlayerId,
    pub source: ObjectRef,
    pub exact_cost: String,
    pub before_level_counters: u32,
    pub stack_object_created: bool,
    pub state_unchanged_until_resolution: bool,
    pub repeatable: bool,
    pub program_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelUpActivationOutcome {
    pub pending: PendingLevelUpAbility,
    pub receipt: LevelUpActivationReceipt,
}

pub fn activate_level_up_ability(
    program: &LevelProgressionProgram,
    state: &LevelProgressionObjectState,
    evidence: LevelUpActivationEvidence,
) -> Result<LevelUpActivationOutcome, LevelProgressionRuntimeError> {
    verify_program_object(program, state)?;
    if state.object != evidence.source {
        return Err(LevelProgressionRuntimeError::WrongObjectIncarnation);
    }
    if state.zone != Zone::Battlefield
        || state.controller != evidence.actor
        || evidence.timing.active_player != evidence.actor
        || !matches!(
            evidence.timing.phase,
            TurnPhase::PrecombatMain | TurnPhase::PostcombatMain
        )
        || !evidence.timing.stack_is_empty
        || !evidence.timing.actor_has_priority
    {
        return Err(LevelProgressionRuntimeError::IllegalLevelUpTiming);
    }
    if !evidence.level_up_ability_present {
        return Err(LevelProgressionRuntimeError::LevelUpAbilityMissing);
    }
    validate_payment(&program.level_up.cost, evidence.actor, &evidence.payment)?;

    let pending = PendingLevelUpAbility {
        runtime_version: LEVEL_PROGRESSION_RUNTIME_VERSION,
        stack_object_id: evidence.stack_object_id,
        controller: evidence.actor,
        source: state.object,
        requested_level_counters: 1,
        copied_from_stack_object_id: None,
        program_sha256: program.semantic_sha256.clone(),
    };
    let receipt = LevelUpActivationReceipt {
        runtime_version: LEVEL_PROGRESSION_RUNTIME_VERSION,
        activation_id: evidence.activation_id,
        stack_object_id: evidence.stack_object_id,
        actor: evidence.actor,
        source: state.object,
        exact_cost: program.level_up.exact_cost.clone(),
        before_level_counters: state.level_counters,
        stack_object_created: true,
        state_unchanged_until_resolution: true,
        repeatable: true,
        program_sha256: program.semantic_sha256.clone(),
    };
    Ok(LevelUpActivationOutcome { pending, receipt })
}

pub fn copy_pending_level_up_ability(
    program: &LevelProgressionProgram,
    original: &PendingLevelUpAbility,
    new_stack_object_id: u64,
    new_controller: PlayerId,
) -> Result<PendingLevelUpAbility, LevelProgressionRuntimeError> {
    verify_pending_level_up_ability(program, original)?;
    if new_stack_object_id == original.stack_object_id {
        return Err(LevelProgressionRuntimeError::DuplicateStackObject);
    }
    Ok(PendingLevelUpAbility {
        runtime_version: LEVEL_PROGRESSION_RUNTIME_VERSION,
        stack_object_id: new_stack_object_id,
        controller: new_controller,
        source: original.source,
        requested_level_counters: original.requested_level_counters,
        copied_from_stack_object_id: Some(original.stack_object_id),
        program_sha256: program.semantic_sha256.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LevelUpResolutionSourceEvidence {
    CurrentBattlefieldObject {
        source: ObjectRef,
    },
    SourceNoLongerOnBattlefield {
        source: ObjectRef,
        zone_change_tracking_complete: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelUpResolutionEvidence {
    pub resolution_id: u64,
    pub stack_object_id: u64,
    pub source: LevelUpResolutionSourceEvidence,
    pub placement: Option<LevelCounterPlacementEvidence>,
    pub resolution_tracking_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelUpResolutionReceipt {
    pub runtime_version: &'static str,
    pub resolution_id: u64,
    pub stack_object_id: u64,
    pub controller: PlayerId,
    pub source: ObjectRef,
    pub source_was_current_battlefield_object: bool,
    pub requested_level_counters: u32,
    pub placed_level_counters: u32,
    pub before_level_counters: Option<u32>,
    pub after_level_counters: Option<u32>,
    pub copied_from_stack_object_id: Option<u64>,
    pub program_sha256: String,
}

pub fn resolve_level_up_ability(
    program: &LevelProgressionProgram,
    mut state: Option<&mut LevelProgressionObjectState>,
    pending: &PendingLevelUpAbility,
    evidence: LevelUpResolutionEvidence,
) -> Result<LevelUpResolutionReceipt, LevelProgressionRuntimeError> {
    verify_pending_level_up_ability(program, pending)?;
    if !evidence.resolution_tracking_complete || evidence.stack_object_id != pending.stack_object_id
    {
        return Err(LevelProgressionRuntimeError::IncompleteResolutionEvidence);
    }

    match evidence.source {
        LevelUpResolutionSourceEvidence::CurrentBattlefieldObject { source } => {
            let state = state
                .as_deref_mut()
                .ok_or(LevelProgressionRuntimeError::IncompleteResolutionEvidence)?;
            verify_program_object(program, state)?;
            if source != pending.source || state.object != pending.source {
                return Err(LevelProgressionRuntimeError::WrongObjectIncarnation);
            }
            if state.zone != Zone::Battlefield {
                return Err(LevelProgressionRuntimeError::SourceNoLongerOnBattlefield);
            }
            let placement = evidence
                .placement
                .ok_or(LevelProgressionRuntimeError::IncompleteResolutionEvidence)?;
            validate_placement(state, &placement, pending.requested_level_counters)?;
            let before = state.level_counters;
            state.level_counters = placement.after_total;
            let placed = placement.placed;
            state.counter_history.push(LevelCounterChangeReceipt {
                runtime_version: LEVEL_PROGRESSION_RUNTIME_VERSION,
                event_id: placement.event_id,
                target: state.object,
                kind: LevelCounterChangeKind::LevelUpResolution,
                requested: pending.requested_level_counters,
                actual_change: i64::from(placed),
                before_total: before,
                after_total: state.level_counters,
                replacement_steps: placement.replacement_steps,
                program_sha256: program.semantic_sha256.clone(),
            });
            Ok(LevelUpResolutionReceipt {
                runtime_version: LEVEL_PROGRESSION_RUNTIME_VERSION,
                resolution_id: evidence.resolution_id,
                stack_object_id: pending.stack_object_id,
                controller: pending.controller,
                source: pending.source,
                source_was_current_battlefield_object: true,
                requested_level_counters: pending.requested_level_counters,
                placed_level_counters: placed,
                before_level_counters: Some(before),
                after_level_counters: Some(state.level_counters),
                copied_from_stack_object_id: pending.copied_from_stack_object_id,
                program_sha256: program.semantic_sha256.clone(),
            })
        }
        LevelUpResolutionSourceEvidence::SourceNoLongerOnBattlefield {
            source,
            zone_change_tracking_complete,
        } => {
            if source != pending.source
                || !zone_change_tracking_complete
                || state.is_some()
                || evidence.placement.is_some()
            {
                return Err(LevelProgressionRuntimeError::IncompleteResolutionEvidence);
            }
            Ok(LevelUpResolutionReceipt {
                runtime_version: LEVEL_PROGRESSION_RUNTIME_VERSION,
                resolution_id: evidence.resolution_id,
                stack_object_id: pending.stack_object_id,
                controller: pending.controller,
                source: pending.source,
                source_was_current_battlefield_object: false,
                requested_level_counters: pending.requested_level_counters,
                placed_level_counters: 0,
                before_level_counters: None,
                after_level_counters: None,
                copied_from_stack_object_id: pending.copied_from_stack_object_id,
                program_sha256: program.semantic_sha256.clone(),
            })
        }
    }
}

fn validate_payment(
    cost: &ActivationCost,
    actor: PlayerId,
    evidence: &ExactCostPaymentEvidence,
) -> Result<(), LevelProgressionRuntimeError> {
    if evidence.payer != actor
        || evidence.exact_cost != cost.exact
        || !evidence.payment_complete
        || evidence.components.len() != cost.components.len()
    {
        return Err(LevelProgressionRuntimeError::IncompleteCostPayment);
    }
    for (index, (component, receipt)) in
        cost.components.iter().zip(&evidence.components).enumerate()
    {
        if receipt.component_index != index
            || receipt.exact_component != cost_component_exact(component)
            || !receipt.completed
        {
            return Err(LevelProgressionRuntimeError::IncompleteCostPayment);
        }
    }
    Ok(())
}

fn cost_component_exact(component: &CostComponent) -> String {
    match component {
        CostComponent::Mana(cost) => cost.exact.clone(),
        CostComponent::TapSource => "{T}".into(),
        CostComponent::UntapSource => "{Q}".into(),
        CostComponent::PayLife(amount) => format!("Pay {amount} life"),
        CostComponent::DiscardCard => "Discard a card".into(),
        CostComponent::SacrificeSource => "Sacrifice this permanent".into(),
        CostComponent::SacrificeCreature => "Sacrifice a creature".into(),
        CostComponent::RemoveCounterFromSource {
            count: 1,
            counter_name,
        } => format!("Remove a {counter_name} counter from this"),
        CostComponent::RemoveCounterFromSource {
            count,
            counter_name,
        } => format!("Remove {count} {counter_name} counters from this"),
        CostComponent::ExileCardFromYourGraveyard => "Exile a card from your graveyard".into(),
    }
}

fn validate_placement(
    state: &LevelProgressionObjectState,
    evidence: &LevelCounterPlacementEvidence,
    expected_requested: u32,
) -> Result<(), LevelProgressionRuntimeError> {
    if evidence.target != state.object {
        return Err(LevelProgressionRuntimeError::WrongObjectIncarnation);
    }
    if evidence.requested != expected_requested
        || evidence.before_total != state.level_counters
        || !evidence.replacement_trace_complete
        || evidence.after_total
            != evidence
                .before_total
                .checked_add(evidence.placed)
                .ok_or(LevelProgressionRuntimeError::CounterOverflow)?
    {
        return Err(LevelProgressionRuntimeError::IncompleteCounterEvidence);
    }
    let mut value = evidence.requested;
    for step in &evidence.replacement_steps {
        if !is_sha256_hex(&step.replacement_effect_sha256) || step.incoming != value {
            return Err(LevelProgressionRuntimeError::IncompleteCounterEvidence);
        }
        value = step.outgoing;
    }
    if value != evidence.placed {
        return Err(LevelProgressionRuntimeError::IncompleteCounterEvidence);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelCounterChangeKind {
    LevelUpResolution,
    Proliferate,
    OtherPlacement,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelCounterChangeReceipt {
    pub runtime_version: &'static str,
    pub event_id: u64,
    pub target: ObjectRef,
    pub kind: LevelCounterChangeKind,
    pub requested: u32,
    pub actual_change: i64,
    pub before_total: u32,
    pub after_total: u32,
    pub replacement_steps: Vec<CounterReplacementStep>,
    pub program_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProliferateLevelCounterEvidence {
    pub chooser: PlayerId,
    pub target_selected: bool,
    pub target_had_level_counter: bool,
    pub placement: LevelCounterPlacementEvidence,
}

pub fn proliferate_level_counter(
    program: &LevelProgressionProgram,
    state: &mut LevelProgressionObjectState,
    evidence: ProliferateLevelCounterEvidence,
) -> Result<LevelCounterChangeReceipt, LevelProgressionRuntimeError> {
    verify_program_object(program, state)?;
    if !evidence.target_selected || !evidence.target_had_level_counter || state.level_counters == 0
    {
        return Err(LevelProgressionRuntimeError::IllegalProliferateSelection);
    }
    validate_placement(state, &evidence.placement, 1)?;
    let receipt = LevelCounterChangeReceipt {
        runtime_version: LEVEL_PROGRESSION_RUNTIME_VERSION,
        event_id: evidence.placement.event_id,
        target: state.object,
        kind: LevelCounterChangeKind::Proliferate,
        requested: 1,
        actual_change: evidence.placement.placed as i64,
        before_total: state.level_counters,
        after_total: evidence.placement.after_total,
        replacement_steps: evidence.placement.replacement_steps,
        program_sha256: program.semantic_sha256.clone(),
    };
    state.level_counters = receipt.after_total;
    state.counter_history.push(receipt.clone());
    Ok(receipt)
}

pub fn apply_external_level_counter_placement(
    program: &LevelProgressionProgram,
    state: &mut LevelProgressionObjectState,
    evidence: LevelCounterPlacementEvidence,
) -> Result<LevelCounterChangeReceipt, LevelProgressionRuntimeError> {
    verify_program_object(program, state)?;
    let requested = evidence.requested;
    validate_placement(state, &evidence, requested)?;
    let receipt = LevelCounterChangeReceipt {
        runtime_version: LEVEL_PROGRESSION_RUNTIME_VERSION,
        event_id: evidence.event_id,
        target: state.object,
        kind: LevelCounterChangeKind::OtherPlacement,
        requested,
        actual_change: evidence.placed as i64,
        before_total: state.level_counters,
        after_total: evidence.after_total,
        replacement_steps: evidence.replacement_steps,
        program_sha256: program.semantic_sha256.clone(),
    };
    state.level_counters = receipt.after_total;
    state.counter_history.push(receipt.clone());
    Ok(receipt)
}

pub fn remove_level_counters(
    program: &LevelProgressionProgram,
    state: &mut LevelProgressionObjectState,
    event_id: u64,
    target: ObjectRef,
    requested: u32,
    removed: u32,
    evidence_complete: bool,
) -> Result<LevelCounterChangeReceipt, LevelProgressionRuntimeError> {
    verify_program_object(program, state)?;
    if target != state.object {
        return Err(LevelProgressionRuntimeError::WrongObjectIncarnation);
    }
    if !evidence_complete || removed > requested || removed > state.level_counters {
        return Err(LevelProgressionRuntimeError::IncompleteCounterEvidence);
    }
    let before = state.level_counters;
    state.level_counters -= removed;
    let receipt = LevelCounterChangeReceipt {
        runtime_version: LEVEL_PROGRESSION_RUNTIME_VERSION,
        event_id,
        target,
        kind: LevelCounterChangeKind::Remove,
        requested,
        actual_change: -(i64::from(removed)),
        before_total: before,
        after_total: state.level_counters,
        replacement_steps: Vec::new(),
        program_sha256: program.semantic_sha256.clone(),
    };
    state.counter_history.push(receipt.clone());
    Ok(receipt)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityLayerEvidence {
    pub level_up_ability_present: bool,
    pub level_symbol_abilities_present: bool,
    pub printed_base_children_present: bool,
    pub layer_six_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowerToughnessSetSource {
    ActiveLevelBand {
        exact_header: String,
        program_sha256: String,
    },
    External {
        effect_sha256: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedPowerToughnessSetEffect {
    pub dependency_and_timestamp_order: u32,
    pub source: PowerToughnessSetSource,
    pub value: PowerToughness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerToughnessModifier {
    pub power: i32,
    pub toughness: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerToughnessLayerEvidence {
    pub copied_base: Option<PowerToughness>,
    pub ordered_layer_7b_set_effects: Vec<OrderedPowerToughnessSetEffect>,
    pub ordered_layer_7c_modifiers: Vec<PowerToughnessModifier>,
    pub counter_power_toughness_modifier: PowerToughnessModifier,
    pub switch_power_toughness_in_layer_7d: bool,
    pub dependency_and_timestamp_order_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousLevelEvaluation {
    pub object: ObjectRef,
    pub level_counters: u32,
    pub active_band_index: Option<usize>,
    pub active_exact_header: Option<String>,
    pub level_up_action_available: bool,
    pub active_base_children: Vec<TypedLevelChild>,
    pub active_band_children: Vec<TypedLevelChild>,
    pub level_symbol_power_toughness_setting: Option<PowerToughness>,
    pub final_power_toughness: Option<PowerToughness>,
    pub program_sha256: String,
}

pub fn evaluate_level_progression_continuous_state(
    program: &LevelProgressionProgram,
    state: &LevelProgressionObjectState,
    abilities: &AbilityLayerEvidence,
    layers: &PowerToughnessLayerEvidence,
) -> Result<ContinuousLevelEvaluation, LevelProgressionRuntimeError> {
    verify_program_object(program, state)?;
    if !abilities.layer_six_complete || !layers.dependency_and_timestamp_order_complete {
        return Err(LevelProgressionRuntimeError::IncompleteLayerEvidence);
    }
    let active_band_index =
        if state.zone == Zone::Battlefield && abilities.level_symbol_abilities_present {
            program
                .bands
                .iter()
                .position(|band| band.range.contains(state.level_counters))
        } else {
            None
        };
    let active_band = active_band_index.map(|index| &program.bands[index]);
    let level_setting = active_band.and_then(|band| band.power_toughness);
    validate_layer_7b_evidence(program, active_band, layers)?;

    let final_power_toughness = match program.permanent_kind {
        FacePermanentKind::Land => {
            if layers.copied_base.is_some()
                || !layers.ordered_layer_7b_set_effects.is_empty()
                || !layers.ordered_layer_7c_modifiers.is_empty()
                || layers.counter_power_toughness_modifier
                    != (PowerToughnessModifier {
                        power: 0,
                        toughness: 0,
                    })
                || layers.switch_power_toughness_in_layer_7d
            {
                return Err(LevelProgressionRuntimeError::IncompleteLayerEvidence);
            }
            None
        }
        FacePermanentKind::Creature => {
            let mut value = layers
                .copied_base
                .or(program.printed_power_toughness)
                .ok_or(LevelProgressionRuntimeError::IncompleteLayerEvidence)?;
            for effect in &layers.ordered_layer_7b_set_effects {
                value = effect.value;
            }
            for modifier in &layers.ordered_layer_7c_modifiers {
                value.power = value
                    .power
                    .checked_add(modifier.power)
                    .ok_or(LevelProgressionRuntimeError::PowerToughnessOverflow)?;
                value.toughness = value
                    .toughness
                    .checked_add(modifier.toughness)
                    .ok_or(LevelProgressionRuntimeError::PowerToughnessOverflow)?;
            }
            value.power = value
                .power
                .checked_add(layers.counter_power_toughness_modifier.power)
                .ok_or(LevelProgressionRuntimeError::PowerToughnessOverflow)?;
            value.toughness = value
                .toughness
                .checked_add(layers.counter_power_toughness_modifier.toughness)
                .ok_or(LevelProgressionRuntimeError::PowerToughnessOverflow)?;
            if layers.switch_power_toughness_in_layer_7d {
                std::mem::swap(&mut value.power, &mut value.toughness);
            }
            Some(value)
        }
    };

    Ok(ContinuousLevelEvaluation {
        object: state.object,
        level_counters: state.level_counters,
        active_band_index,
        active_exact_header: active_band.map(|band| band.exact_header.clone()),
        level_up_action_available: state.zone == Zone::Battlefield
            && abilities.level_up_ability_present,
        active_base_children: if state.zone == Zone::Battlefield
            && abilities.printed_base_children_present
        {
            program.base_children.clone()
        } else {
            Vec::new()
        },
        active_band_children: active_band
            .map(|band| band.children.clone())
            .unwrap_or_default(),
        level_symbol_power_toughness_setting: level_setting,
        final_power_toughness,
        program_sha256: program.semantic_sha256.clone(),
    })
}

fn validate_layer_7b_evidence(
    program: &LevelProgressionProgram,
    active_band: Option<&LevelBandProgram>,
    layers: &PowerToughnessLayerEvidence,
) -> Result<(), LevelProgressionRuntimeError> {
    if layers.ordered_layer_7b_set_effects.windows(2).any(|pair| {
        pair[0].dependency_and_timestamp_order >= pair[1].dependency_and_timestamp_order
    }) {
        return Err(LevelProgressionRuntimeError::IncompleteLayerEvidence);
    }
    let mut level_effects = Vec::new();
    for effect in &layers.ordered_layer_7b_set_effects {
        match &effect.source {
            PowerToughnessSetSource::ActiveLevelBand {
                exact_header,
                program_sha256,
            } => level_effects.push((effect, exact_header, program_sha256)),
            PowerToughnessSetSource::External { effect_sha256 } => {
                if !is_sha256_hex(effect_sha256) {
                    return Err(LevelProgressionRuntimeError::IncompleteLayerEvidence);
                }
            }
        }
    }
    match active_band.and_then(|band| band.power_toughness) {
        Some(expected) => {
            if level_effects.len() != 1
                || level_effects[0].0.value != expected
                || level_effects[0].1 != &active_band.unwrap().exact_header
                || level_effects[0].2 != &program.semantic_sha256
            {
                return Err(LevelProgressionRuntimeError::IncompleteLayerEvidence);
            }
        }
        None if !level_effects.is_empty() => {
            return Err(LevelProgressionRuntimeError::IncompleteLayerEvidence);
        }
        None => {}
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateBasedAction {
    PutIntoOwnersGraveyardForZeroOrLessToughness,
    DestroyForLethalDamage,
    DestroyForDeathtouchDamage,
}

pub fn evaluate_level_progression_state_based_actions(
    program: &LevelProgressionProgram,
    state: &LevelProgressionObjectState,
    continuous: &ContinuousLevelEvaluation,
) -> Result<Vec<StateBasedAction>, LevelProgressionRuntimeError> {
    verify_program_object(program, state)?;
    if continuous.object != state.object
        || continuous.level_counters != state.level_counters
        || continuous.program_sha256 != program.semantic_sha256
    {
        return Err(LevelProgressionRuntimeError::StaleContinuousEvaluation);
    }
    if state.zone != Zone::Battlefield || program.permanent_kind != FacePermanentKind::Creature {
        return Ok(Vec::new());
    }
    let pair = continuous
        .final_power_toughness
        .ok_or(LevelProgressionRuntimeError::IncompleteLayerEvidence)?;
    if pair.toughness <= 0 {
        return Ok(vec![
            StateBasedAction::PutIntoOwnersGraveyardForZeroOrLessToughness,
        ]);
    }
    if !state.indestructible && state.was_dealt_deathtouch_damage && state.marked_damage > 0 {
        return Ok(vec![StateBasedAction::DestroyForDeathtouchDamage]);
    }
    if !state.indestructible && i64::from(state.marked_damage) >= i64::from(pair.toughness) {
        return Ok(vec![StateBasedAction::DestroyForLethalDamage]);
    }
    Ok(Vec::new())
}

fn verify_program_object(
    program: &LevelProgressionProgram,
    state: &LevelProgressionObjectState,
) -> Result<(), LevelProgressionRuntimeError> {
    if !program.has_exact_contract() {
        return Err(LevelProgressionRuntimeError::InvalidProgramContract);
    }
    if state.copied_program_sha256 != program.semantic_sha256 {
        return Err(LevelProgressionRuntimeError::WrongProgramForObject);
    }
    Ok(())
}

fn verify_pending_level_up_ability(
    program: &LevelProgressionProgram,
    pending: &PendingLevelUpAbility,
) -> Result<(), LevelProgressionRuntimeError> {
    if !program.has_exact_contract() {
        return Err(LevelProgressionRuntimeError::InvalidProgramContract);
    }
    if pending.runtime_version != LEVEL_PROGRESSION_RUNTIME_VERSION
        || pending.program_sha256 != program.semantic_sha256
        || pending.requested_level_counters != 1
        || pending.copied_from_stack_object_id == Some(pending.stack_object_id)
    {
        return Err(LevelProgressionRuntimeError::InvalidPendingAbility);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LevelProgressionRuntimeError {
    InvalidProgramContract,
    InvalidPendingAbility,
    WrongProgramForObject,
    WrongObjectIncarnation,
    DuplicateStackObject,
    IllegalLevelUpTiming,
    LevelUpAbilityMissing,
    IncompleteCostPayment,
    IncompleteResolutionEvidence,
    SourceNoLongerOnBattlefield,
    IncompleteCounterEvidence,
    CounterOverflow,
    IllegalProliferateSelection,
    IncompleteLayerEvidence,
    PowerToughnessOverflow,
    StaleContinuousEvaluation,
}

impl fmt::Display for LevelProgressionRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LevelProgressionRuntimeError {}
