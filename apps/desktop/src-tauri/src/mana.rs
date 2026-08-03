//! Deterministic, rules-aware-enough mana modeling primitives.
//!
//! This module intentionally stops short of claiming perfect mana math. Oracle
//! text contains open-ended effects, so every source carries explicit
//! conditional/unknown flags and a confidence estimate. The simulation layer
//! can consume the structured result later without reparsing card text in its
//! hot path.

use std::collections::HashMap;
use std::ops::{BitAnd, BitOr, BitOrAssign};

use crate::domain::{
    CardDefinition, DeckEntry, ManaAnalysisReport, ManaColorSourceReport, ManaReliabilityBand,
};

pub(crate) const MANA_MODEL_VERSION: &str = "oracle-sources-0.1";

const WHITE_BIT: u8 = 1 << 0;
const BLUE_BIT: u8 = 1 << 1;
const BLACK_BIT: u8 = 1 << 2;
const RED_BIT: u8 = 1 << 3;
const GREEN_BIT: u8 = 1 << 4;
const COLORLESS_BIT: u8 = 1 << 5;
const COLOR_BITS: [u8; 6] = [
    WHITE_BIT,
    BLUE_BIT,
    BLACK_BIT,
    RED_BIT,
    GREEN_BIT,
    COLORLESS_BIT,
];

/// Compact W/U/B/R/G/C bit mask. "Any color" deliberately excludes C because
/// colorless is not a color in Magic rules terminology.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ManaColorMask(u8);

impl ManaColorMask {
    pub const NONE: Self = Self(0);
    pub const WHITE: Self = Self(WHITE_BIT);
    pub const BLUE: Self = Self(BLUE_BIT);
    pub const BLACK: Self = Self(BLACK_BIT);
    pub const RED: Self = Self(RED_BIT);
    pub const GREEN: Self = Self(GREEN_BIT);
    pub const COLORLESS: Self = Self(COLORLESS_BIT);
    pub const ANY_COLOR: Self = Self(WHITE_BIT | BLUE_BIT | BLACK_BIT | RED_BIT | GREEN_BIT);

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    fn from_symbol(symbol: &str) -> Self {
        match symbol.trim().to_ascii_uppercase().as_str() {
            "W" => Self::WHITE,
            "U" => Self::BLUE,
            "B" => Self::BLACK,
            "R" => Self::RED,
            "G" => Self::GREEN,
            "C" => Self::COLORLESS,
            _ => Self::NONE,
        }
    }
}

impl BitOr for ManaColorMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ManaColorMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ManaColorMask {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPip {
    pub raw: String,
    /// Possible colored payments represented by this symbol. Hybrid
    /// alternatives are a union, not simultaneous requirements.
    pub colors: ManaColorMask,
    pub generic_value: Option<u16>,
    pub is_colorless: bool,
    pub is_snow: bool,
    pub is_variable: bool,
    pub is_hybrid: bool,
    pub is_phyrexian: bool,
    pub is_unknown: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ManaCostFace {
    pub raw: String,
    pub pips: Vec<ManaPip>,
    pub colors: ManaColorMask,
    /// Potential appearances by W/U/B/R/G/C. A hybrid W/U symbol adds one to
    /// both potential-color buckets; consumers must inspect `pips` when they
    /// need minimum rather than potential requirements.
    pub pip_appearances: [u16; 6],
    pub generic_value: u16,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ManaCostProfile {
    /// Split and modal double-faced costs remain separate instead of being
    /// incorrectly added together.
    pub faces: Vec<ManaCostFace>,
    pub colors: ManaColorMask,
    pub confidence: f32,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntersTapped {
    NotApplicable,
    UntappedByDefault,
    Always,
    Conditional,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ManaSourceProfile {
    pub name: String,
    pub quantity: u16,
    pub is_land: bool,
    pub is_temporary: bool,
    pub colors: ManaColorMask,
    pub any_color: bool,
    pub commander_identity_limited: bool,
    pub conditional: bool,
    pub unknown: bool,
    pub enters_tapped: EntersTapped,
    /// Availability weight, not a probability of drawing the card.
    pub reliability: f32,
    /// Confidence that the Oracle-text classification itself is representative.
    pub confidence: f32,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardManaProfile {
    pub name: String,
    pub quantity: u16,
    pub is_commander: bool,
    pub cost: ManaCostProfile,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CommanderManaRequirements {
    pub commanders: Vec<String>,
    pub color_identity: ManaColorMask,
    pub casting_colors: ManaColorMask,
    pub pip_appearances: [u16; 6],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorSourceSummary {
    pub color: ManaColorMask,
    pub exact_sources: u32,
    pub conditional_sources: u32,
    pub tapped_sources: u32,
    pub weighted_source_equivalents: f32,
    pub demand_pip_appearances: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SourceReliabilityBand {
    #[default]
    Unknown,
    Fragile,
    Mixed,
    Supported,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SourceReliabilitySummary {
    pub band: SourceReliabilityBand,
    pub score: f32,
    pub required_colors: ManaColorMask,
    pub colors: Vec<ColorSourceSummary>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ManaModel {
    pub cards: Vec<CardManaProfile>,
    pub sources: Vec<ManaSourceProfile>,
    pub commander_requirements: CommanderManaRequirements,
    pub land_source_count: u32,
    pub nonland_source_count: u32,
    pub conditional_source_count: u32,
    pub unknown_source_count: u32,
    pub enters_tapped_land_count: u32,
    pub reliability: SourceReliabilitySummary,
    pub confidence: f32,
    pub notes: Vec<String>,
}

/// Parses printed mana symbols while retaining hybrid/Phyrexian alternatives
/// and separate split faces.
pub fn parse_mana_cost(cost: Option<&str>) -> ManaCostProfile {
    let Some(cost) = cost.map(str::trim).filter(|cost| !cost.is_empty()) else {
        return ManaCostProfile {
            faces: Vec::new(),
            colors: ManaColorMask::NONE,
            confidence: 1.0,
            notes: Vec::new(),
        };
    };

    let mut notes = Vec::new();
    let mut colors = ManaColorMask::NONE;
    let mut faces = Vec::new();
    let mut saw_hybrid = false;
    let mut saw_phyrexian = false;
    let mut saw_unknown = false;

    for raw_face in cost.split("//") {
        let raw_face = raw_face.trim();
        let (tokens, malformed) = braced_tokens(raw_face);
        let mut pips = Vec::new();
        let mut face_colors = ManaColorMask::NONE;
        let mut pip_appearances = [0u16; 6];
        let mut generic_value = 0u16;
        let mut unknown_count = usize::from(malformed);

        for token in tokens {
            let pip = parse_pip(&token);
            for (index, bit) in COLOR_BITS.iter().enumerate() {
                if pip.colors.intersects(ManaColorMask(*bit)) || (index == 5 && pip.is_colorless) {
                    pip_appearances[index] = pip_appearances[index].saturating_add(1);
                }
            }
            if !pip.is_hybrid {
                generic_value = generic_value.saturating_add(pip.generic_value.unwrap_or_default());
            }
            face_colors |= pip.colors;
            if pip.is_colorless {
                face_colors |= ManaColorMask::COLORLESS;
            }
            saw_hybrid |= pip.is_hybrid;
            saw_phyrexian |= pip.is_phyrexian;
            saw_unknown |= pip.is_unknown;
            unknown_count += usize::from(pip.is_unknown);
            pips.push(pip);
        }

        let face_confidence = (1.0 - unknown_count as f32 * 0.18).clamp(0.25, 1.0);
        colors |= face_colors;
        faces.push(ManaCostFace {
            raw: raw_face.to_string(),
            pips,
            colors: face_colors,
            pip_appearances,
            generic_value,
            confidence: face_confidence,
        });
    }

    if faces.len() > 1 {
        notes.push("Split-face costs are modeled as alternatives, not added together.".into());
    }
    if saw_hybrid {
        notes.push("Hybrid symbols retain their alternative color payments.".into());
    }
    if saw_phyrexian {
        notes.push("Phyrexian symbols retain the non-mana payment alternative.".into());
    }
    if saw_unknown {
        notes.push("At least one nonstandard mana symbol needs a manual annotation.".into());
    }
    let confidence = faces
        .iter()
        .map(|face| face.confidence)
        .fold(1.0f32, f32::min);

    ManaCostProfile {
        faces,
        colors,
        confidence,
        notes,
    }
}

/// Classifies a card that directly produces mana or, for lands only, searches
/// for a land. Search/fetch lands are potential sources and are always marked
/// conditional.
pub fn classify_mana_source(
    card: &CardDefinition,
    quantity: u16,
    commander_identity: Option<ManaColorMask>,
) -> Option<ManaSourceProfile> {
    let type_line = card.type_line.to_ascii_lowercase();
    let oracle = normalize_oracle(&card.oracle_text);
    let is_land = contains_type_word(&type_line, "land");
    let is_temporary =
        contains_type_word(&type_line, "instant") || contains_type_word(&type_line, "sorcery");
    let mut colors = colors_from_basic_land_types(&type_line);
    let mut any_color = false;
    let mut commander_identity_limited = false;
    let mut conditional = false;
    let mut unknown = false;
    let mut notes = Vec::new();

    let add_clauses = oracle
        .split(['.', '\n', ';'])
        .filter_map(|clause| {
            let index = clause.find("add")?;
            Some((&clause[..index], clause[index + "add".len()..].trim()))
        })
        .collect::<Vec<_>>();

    for (activation, output) in &add_clauses {
        let output_symbols = braced_tokens(output).0;
        let mut recognized_output = false;
        for symbol in output_symbols {
            let pip = parse_pip(&symbol);
            if !pip.colors.is_empty() {
                colors |= pip.colors;
                recognized_output = true;
            }
            if pip.is_colorless {
                colors |= ManaColorMask::COLORLESS;
                recognized_output = true;
            }
        }

        let commander_clause = output.contains("commander's color identity")
            || output.contains("commander’s color identity");
        let flexible_color = output.contains("mana of any color")
            || output.contains("any combination of colors")
            || output.contains("chosen color");
        if flexible_color {
            any_color = true;
            recognized_output = true;
            if commander_clause {
                commander_identity_limited = true;
                match commander_identity {
                    Some(identity) => colors |= identity & ManaColorMask::ANY_COLOR,
                    None => {
                        colors |= ManaColorMask::ANY_COLOR;
                        conditional = true;
                        unknown = true;
                        push_note(
                            &mut notes,
                            "Commander-identity output is unresolved without a commander.",
                        );
                    }
                }
            } else {
                colors |= ManaColorMask::ANY_COLOR;
            }
        }

        if activation_has_mana_payment(activation)
            || contains_conditional_language(activation)
            || contains_conditional_language(output)
        {
            conditional = true;
        }
        if !recognized_output {
            unknown = true;
            conditional = true;
            push_note(
                &mut notes,
                "An Add-mana clause could not be reduced to known color symbols.",
            );
        }
    }

    let is_fetch = is_land && oracle.contains("search your library");
    if is_fetch {
        conditional = true;
        let searched_colors = colors_from_basic_land_types(&oracle);
        if searched_colors.is_empty() && oracle.contains("basic land card") {
            colors |= ManaColorMask::ANY_COLOR;
            any_color = true;
        } else {
            colors |= searched_colors;
        }
        push_note(
            &mut notes,
            "Land-search output is conditional on a legal target remaining in the library.",
        );
    }

    if oracle.contains("sacrifice")
        || oracle.contains("activate only")
        || oracle.contains("spend this mana only")
        || oracle.contains("could produce")
        || oracle.contains("equal to")
        || oracle.contains("for each")
    {
        conditional = true;
    }

    let has_basic_type_ability = !colors_from_basic_land_types(&type_line).is_empty();
    if add_clauses.is_empty() && !is_fetch && !has_basic_type_ability {
        return None;
    }

    let enters_tapped = classify_enters_tapped(&type_line, &oracle);
    if matches!(
        enters_tapped,
        EntersTapped::Always | EntersTapped::Conditional
    ) {
        push_note(
            &mut notes,
            "Enters-tapped timing is approximated from current Oracle text.",
        );
    }
    if conditional {
        push_note(
            &mut notes,
            "Conditional sources are not counted as fully reliable colored sources.",
        );
    }

    let mut reliability = 1.0f32;
    reliability *= match enters_tapped {
        EntersTapped::NotApplicable | EntersTapped::UntappedByDefault => 1.0,
        EntersTapped::Always => 0.72,
        EntersTapped::Conditional => 0.84,
        EntersTapped::Unknown => 0.76,
    };
    if conditional {
        reliability *= 0.66;
    }
    if unknown {
        reliability *= 0.42;
    }
    if is_temporary {
        reliability *= 0.55;
        push_note(
            &mut notes,
            "One-shot mana is tracked separately from persistent sources.",
        );
    }
    if colors.is_empty() {
        reliability *= 0.25;
    }
    let mut confidence = 1.0f32;
    if conditional {
        confidence -= 0.12;
    }
    if is_fetch {
        confidence -= 0.08;
    }
    if unknown {
        confidence -= 0.45;
    }

    Some(ManaSourceProfile {
        name: card.name.clone(),
        quantity,
        is_land,
        is_temporary,
        colors,
        any_color,
        commander_identity_limited,
        conditional,
        unknown,
        enters_tapped,
        reliability: reliability.clamp(0.0, 1.0),
        confidence: confidence.clamp(0.15, 1.0),
        notes,
    })
}
/// Builds the same deterministic model while honoring the commanders selected
/// in the UI even when the imported list did not include a Commander section.
pub fn build_mana_model_with_commanders(
    entries: &[DeckEntry],
    definitions: &HashMap<String, CardDefinition>,
    commander_names: &[String],
) -> ManaModel {
    let selected_commanders = commander_names
        .iter()
        .map(|name| normalize_name(name))
        .collect::<std::collections::HashSet<_>>();
    let mut commander_requirements = CommanderManaRequirements::default();
    let mut unresolved_quantity = 0u32;
    let total_quantity = entries
        .iter()
        .map(|entry| entry.quantity as u32)
        .sum::<u32>();

    for entry in entries.iter().filter(|entry| {
        entry.is_commander || selected_commanders.contains(&normalize_name(&entry.name))
    }) {
        commander_requirements.commanders.push(entry.name.clone());
        let Some(card) = find_definition(definitions, &entry.name) else {
            unresolved_quantity += entry.quantity as u32;
            continue;
        };
        commander_requirements.color_identity |= mask_from_identity(&card.color_identity);
        let cost = parse_mana_cost(card.mana_cost.as_deref());
        commander_requirements.casting_colors |= cost.colors;
        for face in &cost.faces {
            for (index, appearances) in face.pip_appearances.iter().enumerate() {
                commander_requirements.pip_appearances[index] =
                    commander_requirements.pip_appearances[index].saturating_add(*appearances);
            }
        }
    }

    let commander_identity = (!commander_requirements.commanders.is_empty())
        .then_some(commander_requirements.color_identity);
    let mut cards = Vec::new();
    let mut sources = Vec::new();
    let mut demand_pips = [0u32; 6];
    let mut cost_confidence_weight = 0.0f32;
    let mut resolved_quantity = 0u32;

    for entry in entries {
        let Some(card) = find_definition(definitions, &entry.name) else {
            if !entry.is_commander {
                unresolved_quantity += entry.quantity as u32;
            }
            continue;
        };
        let cost = parse_mana_cost(card.mana_cost.as_deref());
        resolved_quantity += entry.quantity as u32;
        cost_confidence_weight += cost.confidence * entry.quantity as f32;
        for face in &cost.faces {
            for (index, appearances) in face.pip_appearances.iter().enumerate() {
                demand_pips[index] =
                    demand_pips[index].saturating_add(*appearances as u32 * entry.quantity as u32);
            }
        }
        if let Some(source) = classify_mana_source(card, entry.quantity, commander_identity) {
            sources.push(source);
        }
        cards.push(CardManaProfile {
            name: card.name.clone(),
            quantity: entry.quantity,
            is_commander: entry.is_commander
                || selected_commanders.contains(&normalize_name(&entry.name)),
            cost,
        });
    }

    let mut land_source_count = 0u32;
    let mut nonland_source_count = 0u32;
    let mut conditional_source_count = 0u32;
    let mut unknown_source_count = 0u32;
    let mut enters_tapped_land_count = 0u32;
    for source in &sources {
        let quantity = source.quantity as u32;
        if source.is_land {
            land_source_count += quantity;
        } else {
            nonland_source_count += quantity;
        }
        if source.conditional {
            conditional_source_count += quantity;
        }
        if source.unknown {
            unknown_source_count += quantity;
        }
        if source.is_land
            && matches!(
                source.enters_tapped,
                EntersTapped::Always | EntersTapped::Conditional
            )
        {
            enters_tapped_land_count += quantity;
        }
    }

    let required_colors =
        demand_pips
            .iter()
            .enumerate()
            .fold(ManaColorMask::NONE, |mut mask, (index, count)| {
                if *count > 0 {
                    mask |= ManaColorMask(COLOR_BITS[index]);
                }
                mask
            })
            | commander_requirements.color_identity
            | commander_requirements.casting_colors;
    let reliability = summarize_source_reliability(&sources, required_colors, demand_pips);

    let identity_confidence = if total_quantity == 0 {
        0.0
    } else {
        resolved_quantity as f32 / total_quantity as f32
    };
    let cost_confidence = if resolved_quantity == 0 {
        0.0
    } else {
        cost_confidence_weight / resolved_quantity as f32
    };
    let source_parse_confidence = if sources.is_empty() {
        0.0
    } else {
        sources
            .iter()
            .map(|source| source.confidence * source.quantity as f32)
            .sum::<f32>()
            / sources
                .iter()
                .map(|source| source.quantity as f32)
                .sum::<f32>()
                .max(1.0)
    };
    let confidence =
        (identity_confidence * 0.50 + cost_confidence * 0.25 + source_parse_confidence * 0.25)
            .clamp(0.0, 1.0);
    let mut notes = Vec::new();
    if commander_requirements.commanders.is_empty() {
        notes.push(
            "No commander entry was marked; commander-identity sources remain conditional.".into(),
        );
    }
    if unresolved_quantity > 0 {
        notes.push(format!(
            "{unresolved_quantity} card slot(s) were unresolved and omitted from mana math."
        ));
    }
    if conditional_source_count > 0 {
        notes.push(format!(
            "{conditional_source_count} source slot(s) depend on board state, targets, or activation conditions."
        ));
    }
    if unknown_source_count > 0 {
        notes.push(format!(
            "{unknown_source_count} source slot(s) need a manual mana annotation."
        ));
    }
    if enters_tapped_land_count > 0 {
        notes.push(format!(
            "{enters_tapped_land_count} land slot(s) are modeled as always or conditionally entering tapped."
        ));
    }

    ManaModel {
        cards,
        sources,
        commander_requirements,
        land_source_count,
        nonland_source_count,
        conditional_source_count,
        unknown_source_count,
        enters_tapped_land_count,
        reliability,
        confidence,
        notes,
    }
}

/// Converts detailed parser state into the stable, serializable report surface.
/// Only required colors are included so off-color reminder text does not make
/// the mana base appear more complex than the deck's actual casting demand.
pub fn analysis_report(model: &ManaModel) -> ManaAnalysisReport {
    let colors = model
        .reliability
        .colors
        .iter()
        .filter(|summary| model.reliability.required_colors.intersects(summary.color))
        .map(|summary| ManaColorSourceReport {
            color: color_label(summary.color).into(),
            exact_sources: summary.exact_sources,
            conditional_sources: summary.conditional_sources,
            tapped_sources: summary.tapped_sources,
            weighted_source_equivalents: summary.weighted_source_equivalents,
            demand_pip_appearances: summary.demand_pip_appearances,
        })
        .collect();

    ManaAnalysisReport {
        reliability_band: match model.reliability.band {
            SourceReliabilityBand::Unknown => ManaReliabilityBand::Unknown,
            SourceReliabilityBand::Fragile => ManaReliabilityBand::Fragile,
            SourceReliabilityBand::Mixed => ManaReliabilityBand::Mixed,
            SourceReliabilityBand::Supported => ManaReliabilityBand::Supported,
        },
        reliability_score: model.reliability.score,
        model_confidence: model.confidence,
        land_source_count: model.land_source_count,
        nonland_source_count: model.nonland_source_count,
        conditional_source_count: model.conditional_source_count,
        unknown_source_count: model.unknown_source_count,
        enters_tapped_land_count: model.enters_tapped_land_count,
        colors,
        notes: model.notes.clone(),
        ..Default::default()
    }
}

fn parse_pip(raw: &str) -> ManaPip {
    let normalized = raw.trim().to_ascii_uppercase();
    let parts = normalized.split('/').collect::<Vec<_>>();
    let is_hybrid = parts.len() > 1;
    let is_phyrexian = parts.contains(&"P");
    let mut colors = ManaColorMask::NONE;
    let mut generic_value = None;
    let mut is_colorless = false;
    let mut is_snow = false;
    let mut is_variable = false;
    let mut recognized = false;

    for part in &parts {
        let color = ManaColorMask::from_symbol(part);
        if !color.is_empty() {
            if color == ManaColorMask::COLORLESS {
                is_colorless = true;
            } else {
                colors |= color;
            }
            recognized = true;
        } else if let Ok(value) = part.parse::<u16>() {
            generic_value = Some(value);
            recognized = true;
        } else if *part == "S" {
            is_snow = true;
            recognized = true;
        } else if matches!(*part, "X" | "Y" | "Z") {
            is_variable = true;
            recognized = true;
        } else if *part == "P" {
            recognized = true;
        }
    }

    ManaPip {
        raw: raw.to_string(),
        colors,
        generic_value,
        is_colorless,
        is_snow,
        is_variable,
        is_hybrid,
        is_phyrexian,
        is_unknown: !recognized || parts.iter().any(|part| part.is_empty()),
    }
}

fn braced_tokens(text: &str) -> (Vec<String>, bool) {
    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    let mut malformed = false;
    while let Some(relative_start) = text[cursor..].find('{') {
        let start = cursor + relative_start + 1;
        let Some(relative_end) = text[start..].find('}') else {
            malformed = true;
            break;
        };
        let end = start + relative_end;
        tokens.push(text[start..end].to_string());
        cursor = end + 1;
    }
    (tokens, malformed)
}

fn normalize_oracle(text: &str) -> String {
    text.to_lowercase()
        .replace("enters the battlefield", "enters")
}

fn contains_type_word(text: &str, word: &str) -> bool {
    text.split(|character: char| !character.is_alphabetic())
        .any(|part| part == word)
}

fn colors_from_basic_land_types(text: &str) -> ManaColorMask {
    let mut colors = ManaColorMask::NONE;
    for word in text.split(|character: char| !character.is_alphabetic()) {
        colors |= match word {
            "plains" => ManaColorMask::WHITE,
            "island" => ManaColorMask::BLUE,
            "swamp" => ManaColorMask::BLACK,
            "mountain" => ManaColorMask::RED,
            "forest" => ManaColorMask::GREEN,
            _ => ManaColorMask::NONE,
        };
    }
    colors
}

fn mask_from_identity(identity: &[String]) -> ManaColorMask {
    identity
        .iter()
        .fold(ManaColorMask::NONE, |mut mask, color| {
            mask |= ManaColorMask::from_symbol(color);
            mask
        })
}

fn color_label(color: ManaColorMask) -> &'static str {
    match color {
        ManaColorMask::WHITE => "W",
        ManaColorMask::BLUE => "U",
        ManaColorMask::BLACK => "B",
        ManaColorMask::RED => "R",
        ManaColorMask::GREEN => "G",
        ManaColorMask::COLORLESS => "C",
        _ => "?",
    }
}

fn classify_enters_tapped(type_line: &str, oracle: &str) -> EntersTapped {
    if contains_type_word(type_line, "instant") || contains_type_word(type_line, "sorcery") {
        return EntersTapped::NotApplicable;
    }
    let Some(sentence) = oracle
        .split(['.', '\n'])
        .find(|sentence| sentence.contains("enters tapped"))
    else {
        return EntersTapped::UntappedByDefault;
    };
    if sentence.contains("unless")
        || sentence.contains("if ")
        || sentence.contains("as long as")
        || sentence.contains("you may")
    {
        EntersTapped::Conditional
    } else if sentence.contains("enters tapped") {
        EntersTapped::Always
    } else {
        EntersTapped::Unknown
    }
}

fn activation_has_mana_payment(activation: &str) -> bool {
    braced_tokens(activation).0.iter().any(|symbol| {
        let symbol = symbol.trim().to_ascii_uppercase();
        symbol != "T" && symbol != "Q" && symbol != "E"
    })
}

fn contains_conditional_language(clause: &str) -> bool {
    [
        " if ",
        "unless",
        "only ",
        "for each",
        "equal to",
        "could produce",
        "chosen color",
        "among ",
        "as long as",
    ]
    .iter()
    .any(|marker| clause.contains(marker))
}

fn push_note(notes: &mut Vec<String>, note: &str) {
    if !notes.iter().any(|existing| existing == note) {
        notes.push(note.to_string());
    }
}

fn find_definition<'a>(
    definitions: &'a HashMap<String, CardDefinition>,
    name: &str,
) -> Option<&'a CardDefinition> {
    let normalized = normalize_name(name);
    definitions.get(&normalized).or_else(|| {
        definitions
            .values()
            .find(|card| normalize_name(&card.name) == normalized)
    })
}

fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn summarize_source_reliability(
    sources: &[ManaSourceProfile],
    required_colors: ManaColorMask,
    demand_pips: [u32; 6],
) -> SourceReliabilitySummary {
    let colors = COLOR_BITS
        .iter()
        .enumerate()
        .map(|(index, bit)| {
            let color = ManaColorMask(*bit);
            let matching = sources
                .iter()
                .filter(|source| source.colors.intersects(color))
                .collect::<Vec<_>>();
            ColorSourceSummary {
                color,
                exact_sources: matching
                    .iter()
                    .filter(|source| !source.conditional && !source.unknown)
                    .map(|source| source.quantity as u32)
                    .sum(),
                conditional_sources: matching
                    .iter()
                    .filter(|source| source.conditional || source.unknown)
                    .map(|source| source.quantity as u32)
                    .sum(),
                tapped_sources: matching
                    .iter()
                    .filter(|source| {
                        matches!(
                            source.enters_tapped,
                            EntersTapped::Always | EntersTapped::Conditional
                        )
                    })
                    .map(|source| source.quantity as u32)
                    .sum(),
                weighted_source_equivalents: matching
                    .iter()
                    .map(|source| source.reliability * source.quantity as f32)
                    .sum(),
                demand_pip_appearances: demand_pips[index],
            }
        })
        .collect::<Vec<_>>();

    let required = colors
        .iter()
        .filter(|summary| required_colors.intersects(summary.color))
        .collect::<Vec<_>>();
    if required.is_empty() {
        return SourceReliabilitySummary {
            band: SourceReliabilityBand::Unknown,
            score: 0.0,
            required_colors,
            colors,
        };
    }

    // This is deliberately a conservative heuristic, not a hypergeometric
    // casting-probability claim. It creates a stable ranking until the
    // simulator consumes turn-by-turn colored source states.
    let ratios = required
        .iter()
        .map(|summary| {
            let demand_target = 8.0 + (summary.demand_pip_appearances as f32).sqrt().min(10.0);
            (summary.weighted_source_equivalents / demand_target).clamp(0.0, 1.0)
        })
        .collect::<Vec<_>>();
    let minimum = ratios.iter().copied().fold(1.0f32, f32::min);
    let average = ratios.iter().sum::<f32>() / ratios.len() as f32;
    let score = (minimum * 0.65 + average * 0.35).clamp(0.0, 1.0);
    let band = if score >= 0.86 {
        SourceReliabilityBand::Supported
    } else if score >= 0.62 {
        SourceReliabilityBand::Mixed
    } else {
        SourceReliabilityBand::Fragile
    };

    SourceReliabilitySummary {
        band,
        score,
        required_colors,
        colors,
    }
}
