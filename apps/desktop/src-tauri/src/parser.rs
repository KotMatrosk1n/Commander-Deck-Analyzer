use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::domain::{DeckEntry, DeckIssue, DeckParseResult, IssueSeverity};

const REQUIRED_CARD_COUNT: u32 = 100;

static QUANTITY_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?P<quantity>\d+)(?:\s*x\s+|\s+)(?P<name>\S.*)$")
        .expect("quantity regex is valid")
});
static NUMERIC_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[+-]?\d+(?:\s*[xX])?(?:\s+|$)").expect("valid regex"));
static HEADING_COUNT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*[\(\[]\s*\d+\s*[\)\]]\s*$").expect("heading regex is valid"));
static EXPORT_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\s+(?:\([A-Z0-9]{2,8}\)|\[[A-Z0-9]{2,8}\])\s+(?:[A-Z0-9-]+)(?:\s+\*[^*]+\*)?\s*$",
    )
    .expect("export suffix regex is valid")
});
static FOIL_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+\*(?:F|E|CMDR)\*\s*$").expect("valid regex"));

pub fn parse_decklist(input: &str) -> DeckParseResult {
    if input.trim().is_empty() {
        return DeckParseResult {
            entries: Vec::new(),
            card_count: 0,
            unique_card_count: 0,
            ignored_line_count: 0,
            commanders: Vec::new(),
            issues: vec![DeckIssue {
                severity: IssueSeverity::Info,
                code: "empty-deck".into(),
                message: "Paste a decklist, open a file, or import a supported URL.".into(),
                line_number: None,
                card_name: None,
            }],
            canonical_text: String::new(),
            is_commander_sized: false,
        };
    }

    let mut entries = Vec::new();
    let mut ignored_line_count = 0usize;
    let mut include_section = true;
    let mut commander_section = false;

    for (index, raw_line) in input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .enumerate()
    {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(section) = parse_section_heading(line) {
            include_section = !is_ignored_section(&section);
            commander_section = matches!(section.as_str(), "commander" | "commanders");
            ignored_line_count += 1;
            continue;
        }

        if !include_section || is_comment_or_metadata(line) {
            ignored_line_count += 1;
            continue;
        }

        if let Some(captures) = QUANTITY_LINE.captures(line) {
            let quantity = captures
                .name("quantity")
                .and_then(|value| value.as_str().parse::<u16>().ok())
                .unwrap_or(0);
            let raw_name = captures
                .name("name")
                .map(|value| value.as_str())
                .unwrap_or("");
            let name = clean_exported_card_name(raw_name);
            if quantity == 0 || name.is_empty() {
                ignored_line_count += 1;
                continue;
            }

            entries.push(DeckEntry {
                quantity,
                name,
                line_number,
                is_commander: commander_section,
            });
            continue;
        }

        if NUMERIC_PREFIX.is_match(line) {
            ignored_line_count += 1;
            continue;
        }

        let name = clean_exported_card_name(line);
        if !name.is_empty() {
            entries.push(DeckEntry {
                quantity: 1,
                name,
                line_number,
                is_commander: commander_section,
            });
        }
    }

    let card_count = entries.iter().fold(0u32, |total, entry| {
        total.saturating_add(entry.quantity as u32)
    });
    let unique_card_count = entries
        .iter()
        .map(|entry| normalize_card_name(&entry.name))
        .collect::<HashSet<_>>()
        .len();
    let commanders = entries
        .iter()
        .filter(|entry| entry.is_commander)
        .flat_map(|entry| std::iter::repeat_n(entry.name.clone(), entry.quantity as usize))
        .collect::<Vec<_>>();
    let mut issues = Vec::new();

    if card_count != REQUIRED_CARD_COUNT {
        issues.push(DeckIssue {
            severity: IssueSeverity::Error,
            code: "deck-size".into(),
            message: format!(
                "Commander decks normally contain exactly 100 cards including commanders; this list contains {card_count}."
            ),
            line_number: None,
            card_name: None,
        });
    }

    if commanders.is_empty() {
        issues.push(DeckIssue {
            severity: IssueSeverity::Warning,
            code: "commander-missing".into(),
            message: "No Commander section was found. Select the commander before analysis.".into(),
            line_number: None,
            card_name: None,
        });
    } else if commanders.len() > 2 {
        issues.push(DeckIssue {
            severity: IssueSeverity::Warning,
            code: "commander-count".into(),
            message: format!(
                "{} cards are marked as commanders. Verify partner, background, or Doctor's companion rules.",
                commanders.len()
            ),
            line_number: None,
            card_name: None,
        });
    }

    let mut canonical_quantities = BTreeMap::<String, u32>::new();
    for entry in &entries {
        *canonical_quantities
            .entry(normalize_card_name(&entry.name))
            .or_default() += entry.quantity as u32;
    }
    let canonical_text = canonical_quantities
        .iter()
        .map(|(name, quantity)| format!("{quantity} {name}"))
        .collect::<Vec<_>>()
        .join("\n");

    DeckParseResult {
        entries,
        card_count,
        unique_card_count,
        ignored_line_count,
        commanders,
        issues,
        canonical_text,
        is_commander_sized: card_count == REQUIRED_CARD_COUNT,
    }
}

pub fn normalize_card_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn parse_section_heading(line: &str) -> Option<String> {
    let mut heading = line.trim();
    heading = heading.trim_start_matches('#').trim();
    heading = heading.trim_start_matches("//").trim();
    let heading = HEADING_COUNT.replace(heading, "");
    let heading = heading.trim().trim_end_matches(':').trim().to_lowercase();

    if included_sections().contains(heading.as_str()) || is_ignored_section(&heading) {
        Some(heading)
    } else {
        None
    }
}

fn included_sections() -> &'static HashSet<&'static str> {
    static SECTIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
        [
            "commander",
            "commanders",
            "deck",
            "decklist",
            "mainboard",
            "main deck",
            "creature",
            "creatures",
            "artifact",
            "artifacts",
            "enchantment",
            "enchantments",
            "instant",
            "instants",
            "sorcery",
            "sorceries",
            "land",
            "lands",
            "planeswalker",
            "planeswalkers",
            "battle",
            "battles",
            "other",
        ]
        .into_iter()
        .collect()
    });
    &SECTIONS
}

fn is_ignored_section(heading: &str) -> bool {
    matches!(
        heading,
        "sideboard"
            | "side board"
            | "maybeboard"
            | "maybe board"
            | "considering"
            | "tokens"
            | "token"
            | "acquireboard"
            | "acquire board"
            | "companion"
            | "companions"
    )
}

fn is_comment_or_metadata(line: &str) -> bool {
    line.starts_with('#')
        || line.starts_with("//")
        || line.starts_with(';')
        || line.to_ascii_lowercase().starts_with("sb:")
        || [
            "total:",
            "card:",
            "cards:",
            "format:",
            "exported by:",
            "last updated:",
        ]
        .iter()
        .any(|prefix| line.to_ascii_lowercase().starts_with(prefix))
}

fn clean_exported_card_name(name: &str) -> String {
    let without_suffix = EXPORT_SUFFIX.replace(name.trim(), "");
    let without_foil = FOIL_SUFFIX.replace(without_suffix.trim(), "");
    without_foil.trim().to_string()
}
