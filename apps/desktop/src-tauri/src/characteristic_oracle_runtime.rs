//! Exact Oracle ownership for behavior already supplied by characteristics.
//!
//! This compiler does not create new gameplay behavior. It proves that one
//! occurrence of an Oracle clause agrees with an already compiled printed
//! combat keyword or devotion based toughness characteristic.

pub(crate) const STRUCTURAL_CHARACTERISTIC_RUNTIME_VERSION: &str =
    "structural-characteristic-runtime/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StandardSupertype {
    Basic,
    Legendary,
    Ongoing,
    Snow,
    World,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StandardCardType {
    Artifact,
    Battle,
    Conspiracy,
    Creature,
    Dungeon,
    Enchantment,
    Instant,
    Kindred,
    Land,
    Phenomenon,
    Plane,
    Planeswalker,
    Scheme,
    Sorcery,
    Vanguard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeLineRulesScope {
    Comprehensive,
    HistoricalAlias,
    Supplemental,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactTypeLineProcedure {
    pub standard_supertypes: Vec<StandardSupertype>,
    pub standard_card_types: Vec<StandardCardType>,
    pub subtypes: Vec<String>,
    pub supplemental_type_words: Vec<String>,
    pub rules_scope: TypeLineRulesScope,
}

impl ExactTypeLineProcedure {
    pub(crate) fn has_supertype(&self, supertype: StandardSupertype) -> bool {
        self.standard_supertypes.contains(&supertype)
    }

    pub(crate) fn has_card_type(&self, card_type: StandardCardType) -> bool {
        self.standard_card_types.contains(&card_type)
    }

    pub(crate) fn canonical_evidence_payload(&self) -> String {
        let supertypes = self
            .standard_supertypes
            .iter()
            .map(|value| match value {
                StandardSupertype::Basic => "basic",
                StandardSupertype::Legendary => "legendary",
                StandardSupertype::Ongoing => "ongoing",
                StandardSupertype::Snow => "snow",
                StandardSupertype::World => "world",
            })
            .collect::<Vec<_>>()
            .join(",");
        let card_types = self
            .standard_card_types
            .iter()
            .map(|value| match value {
                StandardCardType::Artifact => "artifact",
                StandardCardType::Battle => "battle",
                StandardCardType::Conspiracy => "conspiracy",
                StandardCardType::Creature => "creature",
                StandardCardType::Dungeon => "dungeon",
                StandardCardType::Enchantment => "enchantment",
                StandardCardType::Instant => "instant",
                StandardCardType::Kindred => "kindred",
                StandardCardType::Land => "land",
                StandardCardType::Phenomenon => "phenomenon",
                StandardCardType::Plane => "plane",
                StandardCardType::Planeswalker => "planeswalker",
                StandardCardType::Scheme => "scheme",
                StandardCardType::Sorcery => "sorcery",
                StandardCardType::Vanguard => "vanguard",
            })
            .collect::<Vec<_>>()
            .join(",");
        let scope = match self.rules_scope {
            TypeLineRulesScope::Comprehensive => "comprehensive",
            TypeLineRulesScope::HistoricalAlias => "historical-alias",
            TypeLineRulesScope::Supplemental => "supplemental",
        };
        format!(
            "type-line:{scope}:supertypes={supertypes}:types={card_types}:subtypes={}:supplemental={}",
            self.subtypes.join(","),
            self.supplemental_type_words.join(",")
        )
    }
}

pub(crate) fn compile_exact_type_line_procedure(type_line: &str) -> Option<ExactTypeLineProcedure> {
    let type_line = type_line.trim();
    if type_line.is_empty() || type_line.contains("//") {
        return None;
    }
    let (type_segment, subtype_segment) = split_type_line(type_line)?;
    let type_words = type_segment.split_whitespace().collect::<Vec<_>>();
    if type_words.is_empty() {
        return None;
    }

    let historical_summon = type_words[0].eq_ignore_ascii_case("summon");
    let mut standard_supertypes = Vec::new();
    let mut standard_card_types = Vec::new();
    let mut supplemental_type_words = Vec::new();
    let mut historical_alias = false;
    let mut inline_historical_subtypes = Vec::new();

    if historical_summon {
        standard_card_types.push(StandardCardType::Creature);
        historical_alias = true;
        inline_historical_subtypes.extend(
            type_words
                .iter()
                .skip(1)
                .map(|word| word.to_ascii_lowercase()),
        );
    } else {
        for word in type_words {
            let lower = word.to_ascii_lowercase();
            let supertype = match lower.as_str() {
                "basic" => Some(StandardSupertype::Basic),
                "legendary" => Some(StandardSupertype::Legendary),
                "ongoing" => Some(StandardSupertype::Ongoing),
                "snow" => Some(StandardSupertype::Snow),
                "world" => Some(StandardSupertype::World),
                _ => None,
            };
            if let Some(supertype) = supertype {
                if standard_supertypes.contains(&supertype) {
                    return None;
                }
                standard_supertypes.push(supertype);
                continue;
            }

            let card_type = match lower.as_str() {
                "artifact" => Some(StandardCardType::Artifact),
                "battle" => Some(StandardCardType::Battle),
                "conspiracy" => Some(StandardCardType::Conspiracy),
                "creature" => Some(StandardCardType::Creature),
                "dungeon" => Some(StandardCardType::Dungeon),
                "enchantment" => Some(StandardCardType::Enchantment),
                "instant" => Some(StandardCardType::Instant),
                "kindred" => Some(StandardCardType::Kindred),
                "land" => Some(StandardCardType::Land),
                "phenomenon" => Some(StandardCardType::Phenomenon),
                "plane" => Some(StandardCardType::Plane),
                "planeswalker" => Some(StandardCardType::Planeswalker),
                "scheme" => Some(StandardCardType::Scheme),
                "sorcery" => Some(StandardCardType::Sorcery),
                "vanguard" => Some(StandardCardType::Vanguard),
                "tribal" => {
                    historical_alias = true;
                    Some(StandardCardType::Kindred)
                }
                _ => None,
            };
            if let Some(card_type) = card_type {
                if standard_card_types.contains(&card_type) {
                    return None;
                }
                standard_card_types.push(card_type);
            } else {
                supplemental_type_words.push(lower);
            }
        }
    }

    let mut subtypes = inline_historical_subtypes;
    if let Some(subtype_segment) = subtype_segment {
        for subtype in subtype_segment.split_whitespace() {
            if subtype.is_empty()
                || subtype.contains("//")
                || subtype.contains('\u{2013}')
                || subtype.contains('\u{2014}')
            {
                return None;
            }
            subtypes.push(subtype.to_ascii_lowercase());
        }
    }
    standard_supertypes.sort();
    standard_card_types.sort();
    supplemental_type_words.sort();
    let rules_scope = if !supplemental_type_words.is_empty() || standard_card_types.is_empty() {
        TypeLineRulesScope::Supplemental
    } else if historical_alias {
        TypeLineRulesScope::HistoricalAlias
    } else {
        TypeLineRulesScope::Comprehensive
    };
    Some(ExactTypeLineProcedure {
        standard_supertypes,
        standard_card_types,
        subtypes,
        supplemental_type_words,
        rules_scope,
    })
}

fn split_type_line(type_line: &str) -> Option<(&str, Option<&str>)> {
    let delimiters = type_line
        .char_indices()
        .filter(|(_, character)| matches!(character, '\u{2013}' | '\u{2014}'))
        .collect::<Vec<_>>();
    match delimiters.as_slice() {
        [] => {
            if let Some((type_segment, subtype_segment)) = type_line.split_once(" - ") {
                if type_segment.contains(" - ")
                    || subtype_segment.contains(" - ")
                    || type_segment.trim().is_empty()
                    || subtype_segment.trim().is_empty()
                {
                    return None;
                }
                Some((type_segment.trim(), Some(subtype_segment.trim())))
            } else {
                Some((type_line, None))
            }
        }
        [(index, character)] => {
            let type_segment = type_line[..*index].trim();
            let subtype_segment = type_line[index + character.len_utf8()..].trim();
            (!type_segment.is_empty() && !subtype_segment.is_empty())
                .then_some((type_segment, Some(subtype_segment)))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExactManaValueProcedure {
    pub half_units: u32,
}

impl ExactManaValueProcedure {
    pub(crate) fn as_number(self) -> f64 {
        f64::from(self.half_units) / 2.0
    }

    pub(crate) fn canonical_evidence_payload(self) -> String {
        format!("mana-value:half-units={}", self.half_units)
    }
}

pub(crate) fn compile_exact_mana_value_procedure(
    mana_value: f32,
) -> Option<ExactManaValueProcedure> {
    if !mana_value.is_finite() || mana_value < 0.0 {
        return None;
    }
    let half_units = f64::from(mana_value) * 2.0;
    if half_units.fract() != 0.0 || half_units > f64::from(u32::MAX) {
        return None;
    }
    Some(ExactManaValueProcedure {
        half_units: half_units as u32,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CharacteristicColor {
    White,
    Blue,
    Black,
    Red,
    Green,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExactColorSetProcedure {
    pub mask: u8,
}

impl ExactColorSetProcedure {
    pub(crate) fn canonical_evidence_payload(self, subject: &str) -> String {
        format!("{subject}:mask={:02x}", self.mask)
    }
}

pub(crate) fn compile_exact_color_set_procedure(
    colors: &[String],
) -> Option<ExactColorSetProcedure> {
    let mut mask = 0u8;
    for color in colors {
        let color = match color.trim().to_ascii_uppercase().as_str() {
            "W" => CharacteristicColor::White,
            "U" => CharacteristicColor::Blue,
            "B" => CharacteristicColor::Black,
            "R" => CharacteristicColor::Red,
            "G" => CharacteristicColor::Green,
            _ => return None,
        };
        let bit = characteristic_color_bit(color);
        if mask & bit != 0 {
            return None;
        }
        mask |= bit;
    }
    Some(ExactColorSetProcedure { mask })
}

fn characteristic_color_bit(color: CharacteristicColor) -> u8 {
    match color {
        CharacteristicColor::White => 1 << 0,
        CharacteristicColor::Blue => 1 << 1,
        CharacteristicColor::Black => 1 << 2,
        CharacteristicColor::Red => 1 << 3,
        CharacteristicColor::Green => 1 << 4,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExactRational {
    pub numerator: i64,
    pub denominator: u32,
}

impl ExactRational {
    pub(crate) fn new(numerator: i64, denominator: u32) -> Option<Self> {
        if denominator == 0 {
            return None;
        }
        let divisor = greatest_common_divisor(numerator.unsigned_abs(), u64::from(denominator));
        let reduced_denominator = u64::from(denominator) / divisor;
        Some(Self {
            numerator: numerator / i64::try_from(divisor).ok()?,
            denominator: u32::try_from(reduced_denominator).ok()?,
        })
    }

    pub(crate) fn from_integer(value: i64) -> Self {
        Self {
            numerator: value,
            denominator: 1,
        }
    }

    fn checked_add(self, other: Self) -> Option<Self> {
        let left = self.numerator.checked_mul(i64::from(other.denominator))?;
        let right = other.numerator.checked_mul(i64::from(self.denominator))?;
        let denominator = self.denominator.checked_mul(other.denominator)?;
        Self::new(left.checked_add(right)?, denominator)
    }

    fn checked_sub(self, other: Self) -> Option<Self> {
        self.checked_add(Self {
            numerator: other.numerator.checked_neg()?,
            denominator: other.denominator,
        })
    }

    fn checked_mul(self, other: Self) -> Option<Self> {
        Self::new(
            self.numerator.checked_mul(other.numerator)?,
            self.denominator.checked_mul(other.denominator)?,
        )
    }
}

fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrintedStatVariable {
    OracleStar,
    OracleChoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrintedStatProcedure {
    Fixed(ExactRational),
    AugmentDelta(ExactRational),
    Variable {
        variable: PrintedStatVariable,
        offset: i16,
    },
    ConstantMinusVariable {
        constant: i16,
        variable: PrintedStatVariable,
    },
    VariableSquared {
        variable: PrintedStatVariable,
    },
    Infinite,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PrintedStatInputs {
    pub oracle_star: Option<ExactRational>,
    pub oracle_choice: Option<ExactRational>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum EvaluatedPrintedStat {
    Finite(ExactRational),
    Infinite,
}

impl PrintedStatProcedure {
    pub(crate) fn required_variable(self) -> Option<PrintedStatVariable> {
        match self {
            Self::Variable { variable, .. }
            | Self::ConstantMinusVariable { variable, .. }
            | Self::VariableSquared { variable } => Some(variable),
            Self::Fixed(_) | Self::AugmentDelta(_) | Self::Infinite => None,
        }
    }

    pub(crate) fn evaluate(self, inputs: PrintedStatInputs) -> Option<EvaluatedPrintedStat> {
        let variable = |variable| match variable {
            PrintedStatVariable::OracleStar => inputs.oracle_star,
            PrintedStatVariable::OracleChoice => inputs.oracle_choice,
        };
        let value = match self {
            Self::Fixed(value) | Self::AugmentDelta(value) => value,
            Self::Variable {
                variable: key,
                offset,
            } => variable(key)?.checked_add(ExactRational::from_integer(i64::from(offset)))?,
            Self::ConstantMinusVariable {
                constant,
                variable: key,
            } => ExactRational::from_integer(i64::from(constant)).checked_sub(variable(key)?)?,
            Self::VariableSquared { variable: key } => {
                let value = variable(key)?;
                value.checked_mul(value)?
            }
            Self::Infinite => return Some(EvaluatedPrintedStat::Infinite),
        };
        Some(EvaluatedPrintedStat::Finite(value))
    }

    pub(crate) fn canonical_evidence_payload(self, subject: &str) -> String {
        let procedure = match self {
            Self::Fixed(value) => format!("fixed={}", rational_evidence(value)),
            Self::AugmentDelta(value) => format!("augment-delta={}", rational_evidence(value)),
            Self::Variable { variable, offset } => {
                format!("variable={}:offset={offset}", stat_variable_tag(variable))
            }
            Self::ConstantMinusVariable { constant, variable } => format!(
                "constant={constant}:minus-variable={}",
                stat_variable_tag(variable)
            ),
            Self::VariableSquared { variable } => {
                format!("variable-squared={}", stat_variable_tag(variable))
            }
            Self::Infinite => "infinite".into(),
        };
        format!("{subject}:{procedure}")
    }
}

fn stat_variable_tag(variable: PrintedStatVariable) -> &'static str {
    match variable {
        PrintedStatVariable::OracleStar => "oracle-star",
        PrintedStatVariable::OracleChoice => "oracle-choice",
    }
}

fn rational_evidence(value: ExactRational) -> String {
    format!("{}/{}", value.numerator, value.denominator)
}

pub(crate) fn compile_exact_printed_stat_procedure(
    layout: &str,
    printed_value: &str,
) -> Option<PrintedStatProcedure> {
    let value = printed_value.trim();
    if value.is_empty() {
        return None;
    }
    if value == "\u{221e}" {
        return Some(PrintedStatProcedure::Infinite);
    }
    if value == "?" {
        return Some(PrintedStatProcedure::Variable {
            variable: PrintedStatVariable::OracleChoice,
            offset: 0,
        });
    }
    if value == "*" {
        return Some(PrintedStatProcedure::Variable {
            variable: PrintedStatVariable::OracleStar,
            offset: 0,
        });
    }
    if value == "*\u{b2}" {
        return Some(PrintedStatProcedure::VariableSquared {
            variable: PrintedStatVariable::OracleStar,
        });
    }
    if let Some(offset) = value.strip_prefix("*+") {
        return Some(PrintedStatProcedure::Variable {
            variable: PrintedStatVariable::OracleStar,
            offset: parse_canonical_i16(offset)?,
        });
    }
    if let Some(offset) = value.strip_suffix("+*") {
        return Some(PrintedStatProcedure::Variable {
            variable: PrintedStatVariable::OracleStar,
            offset: parse_canonical_i16(offset)?,
        });
    }
    if let Some(constant) = value.strip_suffix("-*") {
        return Some(PrintedStatProcedure::ConstantMinusVariable {
            constant: parse_canonical_i16(constant)?,
            variable: PrintedStatVariable::OracleStar,
        });
    }

    let rational = parse_exact_half_integer(value)?;
    if layout.trim().eq_ignore_ascii_case("augment")
        && (value.starts_with('+') || value.starts_with('-'))
    {
        Some(PrintedStatProcedure::AugmentDelta(rational))
    } else {
        Some(PrintedStatProcedure::Fixed(rational))
    }
}

fn parse_exact_half_integer(value: &str) -> Option<ExactRational> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let (negative, unsigned) = if let Some(unsigned) = value.strip_prefix('-') {
        (true, unsigned)
    } else if let Some(unsigned) = value.strip_prefix('+') {
        (false, unsigned)
    } else {
        (false, value)
    };
    if unsigned.is_empty() {
        return None;
    }
    let (whole, fractional_half) = if let Some((whole, fraction)) = unsigned.split_once('.') {
        if fraction != "5" || whole.contains('.') {
            return None;
        }
        (if whole.is_empty() { "0" } else { whole }, true)
    } else {
        (unsigned, false)
    };
    if !whole.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let whole = whole.parse::<i64>().ok()?;
    let magnitude = whole
        .checked_mul(2)?
        .checked_add(i64::from(fractional_half))?;
    ExactRational::new(if negative { -magnitude } else { magnitude }, 2)
}

fn parse_canonical_i16(value: &str) -> Option<i16> {
    if value.is_empty() || !value.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let parsed = value.parse::<i16>().ok()?;
    (parsed.to_string() == value).then_some(parsed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoyaltyInitializationProcedure {
    Fixed(u16),
    PaidX,
    Dice { count: u8, sides: u8, modifier: i16 },
    OracleDefined,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LoyaltyInitializationInputs {
    pub paid_x: Option<u16>,
    pub dice_total: Option<u16>,
    pub oracle_value: Option<u16>,
}

impl LoyaltyInitializationProcedure {
    pub(crate) fn initial_counters(self, inputs: LoyaltyInitializationInputs) -> Option<u16> {
        match self {
            Self::Fixed(value) => Some(value),
            Self::PaidX => inputs.paid_x,
            Self::OracleDefined => inputs.oracle_value,
            Self::Dice {
                count,
                sides,
                modifier,
            } => {
                let dice_total = inputs.dice_total?;
                let minimum = u16::from(count);
                let maximum = u16::from(count).checked_mul(u16::from(sides))?;
                if !(minimum..=maximum).contains(&dice_total) {
                    return None;
                }
                let total = i32::from(dice_total).checked_add(i32::from(modifier))?;
                u16::try_from(total).ok()
            }
        }
    }

    pub(crate) fn requires_live_input(self) -> bool {
        !matches!(self, Self::Fixed(_))
    }

    pub(crate) fn canonical_evidence_payload(self) -> String {
        match self {
            Self::Fixed(value) => format!("loyalty:fixed={value}"),
            Self::PaidX => "loyalty:paid-x".into(),
            Self::Dice {
                count,
                sides,
                modifier,
            } => format!("loyalty:dice={count}d{sides}:modifier={modifier}"),
            Self::OracleDefined => "loyalty:oracle-defined".into(),
        }
    }
}

pub(crate) fn compile_exact_loyalty_initialization_procedure(
    printed_loyalty: &str,
) -> Option<LoyaltyInitializationProcedure> {
    let value = printed_loyalty.trim();
    if value == "X" {
        return Some(LoyaltyInitializationProcedure::PaidX);
    }
    if value == "*" {
        return Some(LoyaltyInitializationProcedure::OracleDefined);
    }
    if let Some((dice, modifier)) = value.split_once('+')
        && let Some((count, sides)) = dice.split_once('d')
    {
        let count_source = count;
        let sides_source = sides;
        let modifier_source = modifier;
        let count = count_source.parse::<u8>().ok()?;
        let sides = sides_source.parse::<u8>().ok()?;
        let modifier = modifier_source.parse::<i16>().ok()?;
        if count == 0
            || sides < 2
            || count.to_string() != count_source
            || sides.to_string() != sides_source
            || modifier.to_string() != modifier_source
        {
            return None;
        }
        return Some(LoyaltyInitializationProcedure::Dice {
            count,
            sides,
            modifier,
        });
    }
    let value = value.parse::<u16>().ok()?;
    (value.to_string() == printed_loyalty.trim())
        .then_some(LoyaltyInitializationProcedure::Fixed(value))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DefenseInitializationProcedure {
    pub counters: u16,
}

impl DefenseInitializationProcedure {
    pub(crate) fn canonical_evidence_payload(self) -> String {
        format!("defense:fixed={}", self.counters)
    }
}

pub(crate) fn compile_exact_defense_initialization_procedure(
    printed_defense: &str,
) -> Option<DefenseInitializationProcedure> {
    let printed_defense = printed_defense.trim();
    let counters = printed_defense.parse::<u16>().ok()?;
    (counters.to_string() == printed_defense).then_some(DefenseInitializationProcedure { counters })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VanguardModifierProcedure {
    pub delta: i16,
}

impl VanguardModifierProcedure {
    pub(crate) fn apply(self, base: u16) -> Option<u16> {
        let adjusted = i32::from(base).checked_add(i32::from(self.delta))?;
        u16::try_from(adjusted).ok()
    }

    pub(crate) fn canonical_evidence_payload(self, subject: &str) -> String {
        format!("{subject}:delta={}", self.delta)
    }
}

pub(crate) fn compile_exact_vanguard_modifier_procedure(
    printed_modifier: &str,
) -> Option<VanguardModifierProcedure> {
    let printed_modifier = printed_modifier.trim();
    if printed_modifier.len() < 2
        || !matches!(printed_modifier.as_bytes()[0], b'+' | b'-')
        || !printed_modifier[1..]
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return None;
    }
    let delta = printed_modifier.parse::<i16>().ok()?;
    let canonical = if delta >= 0 {
        format!("+{delta}")
    } else {
        delta.to_string()
    };
    (canonical == printed_modifier).then_some(VanguardModifierProcedure { delta })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AttractionLightsProcedure {
    pub lit_roll_mask: u8,
}

impl AttractionLightsProcedure {
    pub(crate) fn canonical_evidence_payload(self) -> String {
        format!("attraction-lights:mask={:02x}", self.lit_roll_mask)
    }
}

pub(crate) fn compile_exact_attraction_lights_procedure(
    attraction_lights: &[u8],
) -> Option<AttractionLightsProcedure> {
    if attraction_lights.is_empty() {
        return None;
    }
    let mut mask = 0u8;
    let mut previous = 0u8;
    for &roll in attraction_lights {
        if !(1..=6).contains(&roll) || roll <= previous {
            return None;
        }
        mask |= 1 << (roll - 1);
        previous = roll;
    }
    Some(AttractionLightsProcedure {
        lit_roll_mask: mask,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SourceCardType {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CombatKeyword {
    Deathtouch,
    DoubleStrike,
    FirstStrike,
    Flying,
    Haste,
    Hexproof,
    Indestructible,
    Lifelink,
    Menace,
    Reach,
    Shroud,
    Trample,
    Vigilance,
    Defender,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DevotionColor {
    White,
    Blue,
    Black,
    Red,
    Green,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompiledDynamicCharacteristic {
    ToughnessEqualsDevotion(DevotionColor),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledCharacteristicView {
    pub source_card_types: Vec<SourceCardType>,
    pub printed_combat_keywords: Vec<CombatKeyword>,
    pub dynamic_characteristic: Option<CompiledDynamicCharacteristic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CharacteristicOwnershipRequest {
    PrintedCombatKeyword(CombatKeyword),
    ToughnessEqualsDevotion(DevotionColor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CharacteristicOracleInput<'a> {
    pub face_index: u16,
    pub type_line: &'a str,
    pub oracle_text: &'a str,
    pub printed_toughness: Option<&'a str>,
    pub request: CharacteristicOwnershipRequest,
    pub compiled: &'a CompiledCharacteristicView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OracleClauseOccurrence {
    pub face_index: u16,
    pub clause_index: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReviewedReminder {
    Devotion(DevotionColor),
    Flying,
    Lifelink,
    Vigilance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReminderContract {
    Absent,
    Reviewed(ReviewedReminder),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PureCombatKeywordProgram {
    pub source_card_types: Vec<SourceCardType>,
    pub complete_clause_keywords: Vec<CombatKeyword>,
    pub owned_keyword: CombatKeyword,
    pub reminder: ReminderContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DevotionToughnessProgram {
    pub source_card_types: Vec<SourceCardType>,
    pub color: DevotionColor,
    pub printed_toughness_is_star: bool,
    pub reminder: ReminderContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CharacteristicOracleProgram {
    PureCombatKeyword(PureCombatKeywordProgram),
    DevotionToughness(DevotionToughnessProgram),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledCharacteristicOracle {
    pub ownership: OracleClauseOccurrence,
    pub program: CharacteristicOracleProgram,
}

pub(crate) fn compile_characteristic_oracle_ownership(
    input: CharacteristicOracleInput<'_>,
) -> Option<Vec<CompiledCharacteristicOracle>> {
    let source_card_types = compile_exact_source_types(input.type_line)?;
    if source_card_types != canonical_source_types(&input.compiled.source_card_types)?
        || !source_card_types.contains(&SourceCardType::Creature)
    {
        return None;
    }
    let clauses = normalized_oracle_clauses(input.oracle_text)?;

    match input.request {
        CharacteristicOwnershipRequest::PrintedCombatKeyword(keyword) => {
            compile_keyword_occurrences(input, source_card_types, &clauses, keyword)
        }
        CharacteristicOwnershipRequest::ToughnessEqualsDevotion(color) => {
            compile_devotion_occurrence(input, source_card_types, &clauses, color)
                .map(|program| vec![program])
        }
    }
}

fn compile_keyword_occurrences(
    input: CharacteristicOracleInput<'_>,
    source_card_types: Vec<SourceCardType>,
    clauses: &[String],
    owned_keyword: CombatKeyword,
) -> Option<Vec<CompiledCharacteristicOracle>> {
    let compiled_keywords = canonical_combat_keywords(&input.compiled.printed_combat_keywords)?;
    if !compiled_keywords.contains(&owned_keyword) {
        return None;
    }

    let mut programs = Vec::new();
    for (clause_index, clause) in clauses.iter().enumerate() {
        let Some((clause_keywords, reminder)) = parse_pure_keyword_clause(clause) else {
            continue;
        };
        if !clause_keywords.contains(&owned_keyword) {
            continue;
        }
        if clause_keywords
            .iter()
            .any(|keyword| !compiled_keywords.contains(keyword))
        {
            return None;
        }
        programs.push(CompiledCharacteristicOracle {
            ownership: OracleClauseOccurrence {
                face_index: input.face_index,
                clause_index: u16::try_from(clause_index).ok()?,
            },
            program: CharacteristicOracleProgram::PureCombatKeyword(PureCombatKeywordProgram {
                source_card_types: source_card_types.clone(),
                complete_clause_keywords: clause_keywords,
                owned_keyword,
                reminder,
            }),
        });
    }
    (!programs.is_empty()).then_some(programs)
}

fn compile_devotion_occurrence(
    input: CharacteristicOracleInput<'_>,
    source_card_types: Vec<SourceCardType>,
    clauses: &[String],
    color: DevotionColor,
) -> Option<CompiledCharacteristicOracle> {
    if input.printed_toughness.map(str::trim) != Some("*")
        || input.compiled.dynamic_characteristic
            != Some(CompiledDynamicCharacteristic::ToughnessEqualsDevotion(
                color,
            ))
    {
        return None;
    }

    let matches = clauses
        .iter()
        .enumerate()
        .filter_map(|(clause_index, clause)| {
            parse_devotion_toughness_clause(clause)
                .filter(|(parsed_color, _)| *parsed_color == color)
                .map(|(_, reminder)| (clause_index, reminder))
        })
        .collect::<Vec<_>>();
    let [(clause_index, reminder)] = matches.as_slice() else {
        return None;
    };

    Some(CompiledCharacteristicOracle {
        ownership: OracleClauseOccurrence {
            face_index: input.face_index,
            clause_index: u16::try_from(*clause_index).ok()?,
        },
        program: CharacteristicOracleProgram::DevotionToughness(DevotionToughnessProgram {
            source_card_types,
            color,
            printed_toughness_is_star: true,
            reminder: *reminder,
        }),
    })
}

fn parse_pure_keyword_clause(clause: &str) -> Option<(Vec<CombatKeyword>, ReminderContract)> {
    for (keyword, reminder, exact_clause) in [
        (
            CombatKeyword::Flying,
            ReviewedReminder::Flying,
            "flying (this creature can't be blocked except by creatures with flying or reach.)",
        ),
        (
            CombatKeyword::Lifelink,
            ReviewedReminder::Lifelink,
            "lifelink (damage dealt by this creature also causes you to gain that much life.)",
        ),
        (
            CombatKeyword::Vigilance,
            ReviewedReminder::Vigilance,
            "vigilance (attacking doesn't cause this creature to tap.)",
        ),
    ] {
        if clause == exact_clause {
            return Some((vec![keyword], ReminderContract::Reviewed(reminder)));
        }
    }
    if clause.contains('(')
        || clause.contains(')')
        || clause.ends_with('.')
        || clause.contains(" and ")
    {
        return None;
    }
    let mut keywords = clause
        .split(", ")
        .map(parse_combat_keyword)
        .collect::<Option<Vec<_>>>()?;
    if keywords.is_empty() {
        return None;
    }
    let original_len = keywords.len();
    keywords.sort();
    keywords.dedup();
    if keywords.len() != original_len {
        return None;
    }
    Some((keywords, ReminderContract::Absent))
}

fn parse_combat_keyword(value: &str) -> Option<CombatKeyword> {
    match value {
        "deathtouch" => Some(CombatKeyword::Deathtouch),
        "double strike" => Some(CombatKeyword::DoubleStrike),
        "first strike" => Some(CombatKeyword::FirstStrike),
        "flying" => Some(CombatKeyword::Flying),
        "haste" => Some(CombatKeyword::Haste),
        "hexproof" => Some(CombatKeyword::Hexproof),
        "indestructible" => Some(CombatKeyword::Indestructible),
        "lifelink" => Some(CombatKeyword::Lifelink),
        "menace" => Some(CombatKeyword::Menace),
        "reach" => Some(CombatKeyword::Reach),
        "shroud" => Some(CombatKeyword::Shroud),
        "trample" => Some(CombatKeyword::Trample),
        "vigilance" => Some(CombatKeyword::Vigilance),
        "defender" => Some(CombatKeyword::Defender),
        _ => None,
    }
}

fn parse_devotion_toughness_clause(clause: &str) -> Option<(DevotionColor, ReminderContract)> {
    let body = if let Some(body) = clause.strip_prefix("this creature's ") {
        body
    } else if let Some(body) = clause.strip_prefix("this permanent's ") {
        body
    } else {
        let possessive = "'s toughness is equal to your devotion to ";
        let split = clause.find(possessive)?;
        let reference = &clause[..split];
        if !is_plausible_named_reference(reference) {
            return None;
        }
        &clause[split + 3..]
    };
    let prefix = "toughness is equal to your devotion to ";
    let remainder = body.strip_prefix(prefix)?;
    for color in [
        DevotionColor::White,
        DevotionColor::Blue,
        DevotionColor::Black,
        DevotionColor::Red,
        DevotionColor::Green,
    ] {
        let color_name = devotion_color_name(color);
        if remainder == format!("{color_name}.") {
            return Some((color, ReminderContract::Absent));
        }
        if remainder
            == format!(
                "{color_name}. (each {} in the mana costs of permanents you control counts toward your devotion to {color_name}.)",
                devotion_pip(color)
            )
        {
            return Some((
                color,
                ReminderContract::Reviewed(ReviewedReminder::Devotion(color)),
            ));
        }
    }
    None
}

fn devotion_color_name(color: DevotionColor) -> &'static str {
    match color {
        DevotionColor::White => "white",
        DevotionColor::Blue => "blue",
        DevotionColor::Black => "black",
        DevotionColor::Red => "red",
        DevotionColor::Green => "green",
    }
}

fn devotion_pip(color: DevotionColor) -> &'static str {
    match color {
        DevotionColor::White => "{w}",
        DevotionColor::Blue => "{u}",
        DevotionColor::Black => "{b}",
        DevotionColor::Red => "{r}",
        DevotionColor::Green => "{g}",
    }
}

fn is_plausible_named_reference(reference: &str) -> bool {
    let words = reference.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() || words.len() > 8 {
        return false;
    }
    const NON_NAME_STARTS: &[&str] = &[
        "a", "all", "an", "any", "each", "it", "source", "target", "that", "the", "them", "those",
        "you", "your",
    ];
    if NON_NAME_STARTS.contains(&words[0]) {
        return false;
    }
    reference.chars().all(|character| {
        character.is_alphanumeric()
            || character.is_whitespace()
            || matches!(character, '\'' | ',' | '-' | ':')
    })
}

fn canonical_combat_keywords(keywords: &[CombatKeyword]) -> Option<Vec<CombatKeyword>> {
    let mut canonical = keywords.to_vec();
    let original_len = canonical.len();
    canonical.sort();
    canonical.dedup();
    (canonical.len() == original_len).then_some(canonical)
}

fn canonical_source_types(types: &[SourceCardType]) -> Option<Vec<SourceCardType>> {
    let mut canonical = types.to_vec();
    let original_len = canonical.len();
    canonical.sort();
    canonical.dedup();
    (canonical.len() == original_len).then_some(canonical)
}

fn compile_exact_source_types(type_line: &str) -> Option<Vec<SourceCardType>> {
    if type_line.contains("//") {
        return None;
    }
    let normalized = type_line.replace(['\u{2013}', '\u{2014}'], " - ");
    let primary = normalized
        .split_once(" - ")
        .map_or(normalized.as_str(), |(primary, _)| primary)
        .trim();
    if primary.is_empty() {
        return None;
    }
    let mut types = Vec::new();
    for word in primary.split_whitespace() {
        let lower = word.to_ascii_lowercase();
        let parsed = match lower.as_str() {
            "artifact" => Some(SourceCardType::Artifact),
            "battle" => Some(SourceCardType::Battle),
            "creature" => Some(SourceCardType::Creature),
            "enchantment" => Some(SourceCardType::Enchantment),
            "instant" => Some(SourceCardType::Instant),
            "kindred" => Some(SourceCardType::Kindred),
            "land" => Some(SourceCardType::Land),
            "planeswalker" => Some(SourceCardType::Planeswalker),
            "sorcery" => Some(SourceCardType::Sorcery),
            "basic" | "legendary" | "ongoing" | "snow" | "world" => None,
            _ => return None,
        };
        if let Some(card_type) = parsed {
            types.push(card_type);
        }
    }
    if types.is_empty() {
        return None;
    }
    let original_len = types.len();
    types.sort();
    types.dedup();
    (types.len() == original_len).then_some(types)
}

fn normalized_oracle_clauses(oracle_text: &str) -> Option<Vec<String>> {
    let normalized_text = oracle_text.trim().replace('’', "'");
    let clauses = normalized_text
        .lines()
        .map(|clause| {
            clause
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase()
        })
        .filter(|clause| !clause.is_empty())
        .collect::<Vec<_>>();
    (!clauses.is_empty()).then_some(clauses)
}
