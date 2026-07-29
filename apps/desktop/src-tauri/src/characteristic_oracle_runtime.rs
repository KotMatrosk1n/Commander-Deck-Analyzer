//! Exact Oracle ownership for behavior already supplied by characteristics.
//!
//! This compiler does not create new gameplay behavior. It proves that one
//! occurrence of an Oracle clause agrees with an already compiled printed
//! combat keyword or devotion based toughness characteristic.

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
