//! Content keyed targeting restrictions and complete protection queries.
//!
//! The compiler in this module accepts only complete Shroud, Hexproof, and
//! protection clauses. It never consumes one keyword out of a compound
//! ability. Protection is evaluated through all four rules effects: targeting,
//! attachment legality, damage prevention, and blocking legality.
//!
//! No production adapter is connected yet. A recognized program remains
//! nonlive until the main engine supplies exact object incarnations, player
//! relationships, source characteristics, and attachment evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const TARGETING_PROTECTION_COMPILER_VERSION: &str = "targeting-protection-compiler-0.1";
pub const TARGETING_PROTECTION_RUNTIME_VERSION: &str = "targeting-protection-runtime-0.1";
pub const TARGETING_PROTECTION_RULES_CONTEXT_VERSION: &str =
    "magic-comprehensive-rules-2026-06-19:109.2,115.1,120.3,609.7,702.16,702.18,702.11";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;

/// These programs are deliberately nonlive until a production adapter
/// supplies the complete evidence required by every query in this module.
pub const fn targeting_protection_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
}

impl ManaColor {
    fn stable_id(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Blue => "blue",
            Self::Black => "black",
            Self::Red => "red",
            Self::Green => "green",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EffectSourceKind {
    Spell,
    ActivatedAbility,
    TriggeredAbility,
    StaticAbility,
    Object,
}

impl EffectSourceKind {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Spell => "spell",
            Self::ActivatedAbility => "activated-ability",
            Self::TriggeredAbility => "triggered-ability",
            Self::StaticAbility => "static-ability",
            Self::Object => "object",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProtectionQualitySpec {
    Everything,
    Color(ManaColor),
    EachColor,
    ChosenColor,
    ColorsOfPermanentsYouControl,
    EachOfProtectedObjectsColors,
    Colored,
    Colorless,
    Monocolored,
    Multicolored,
    EnemyColorPair,
    CardType(String),
    Subtype(String),
    Supertype(String),
    Named(String),
    ChosenPlayer,
    ManaValueAtMost(u32),
    ManaValueAtLeast(u32),
    NonSubtypeCreature(String),
    ModifiedCreature,
    PermanentWithCounter(String),
    Wordy,
    EffectKind(EffectSourceKind),
    CreatureWithAtLeastCreatureTypes(u32),
    CausesDieRoll,
}

impl ProtectionQualitySpec {
    fn stable_id(&self) -> String {
        match self {
            Self::Everything => "everything".into(),
            Self::Color(color) => format!("color/{}", color.stable_id()),
            Self::EachColor => "each-color".into(),
            Self::ChosenColor => "chosen-color".into(),
            Self::ColorsOfPermanentsYouControl => "colors-of-permanents-you-control".into(),
            Self::EachOfProtectedObjectsColors => "each-of-protected-objects-colors".into(),
            Self::Colored => "colored".into(),
            Self::Colorless => "colorless".into(),
            Self::Monocolored => "monocolored".into(),
            Self::Multicolored => "multicolored".into(),
            Self::EnemyColorPair => "enemy-color-pair".into(),
            Self::CardType(card_type) => {
                format!("card-type/{}", canonical_characteristic(card_type))
            }
            Self::Subtype(subtype) => {
                format!("subtype/{}", canonical_characteristic(subtype))
            }
            Self::Supertype(supertype) => {
                format!("supertype/{}", canonical_characteristic(supertype))
            }
            Self::Named(name) => format!("named/{}", canonical_characteristic(name)),
            Self::ChosenPlayer => "chosen-player".into(),
            Self::ManaValueAtMost(value) => format!("mana-value-at-most/{value}"),
            Self::ManaValueAtLeast(value) => format!("mana-value-at-least/{value}"),
            Self::NonSubtypeCreature(subtype) => {
                format!(
                    "creature-without-subtype/{}",
                    canonical_characteristic(subtype)
                )
            }
            Self::ModifiedCreature => "modified-creature".into(),
            Self::PermanentWithCounter(counter) => {
                format!(
                    "permanent-with-counter/{}",
                    canonical_characteristic(counter)
                )
            }
            Self::Wordy => "wordy/four-or-more-rules-text-lines".into(),
            Self::EffectKind(kind) => format!("effect-kind/{}", kind.stable_id()),
            Self::CreatureWithAtLeastCreatureTypes(amount) => {
                format!("creature-with-at-least-creature-types/{amount}")
            }
            Self::CausesDieRoll => "causes-die-roll".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetingProtectionKind {
    Shroud,
    Hexproof {
        /// Empty is ordinary Hexproof. Nonempty is Hexproof from the listed
        /// qualities.
        qualities: Vec<ProtectionQualitySpec>,
    },
    Protection {
        qualities: Vec<ProtectionQualitySpec>,
    },
}

impl TargetingProtectionKind {
    fn stable_id(&self) -> String {
        match self {
            Self::Shroud => "shroud".into(),
            Self::Hexproof { qualities } if qualities.is_empty() => "hexproof/all".into(),
            Self::Hexproof { qualities } => {
                format!("hexproof/{}", quality_list_stable_id(qualities))
            }
            Self::Protection { qualities } => {
                format!("protection/{}", quality_list_stable_id(qualities))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionRecipient {
    SourceObject,
    ControllerPlayer,
    EnchantedCreature,
}

impl ProtectionRecipient {
    fn stable_id(self) -> &'static str {
        match self {
            Self::SourceObject => "source-object",
            Self::ControllerPlayer => "controller-player",
            Self::EnchantedCreature => "enchanted-creature",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionDuration {
    WhileSourceAbilityApplies,
    UntilEndOfTurn,
}

impl ProtectionDuration {
    fn stable_id(self) -> &'static str {
        match self {
            Self::WhileSourceAbilityApplies => "while-source-ability-applies",
            Self::UntilEndOfTurn => "until-end-of-turn",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentExceptionPolicy {
    None,
    GrantingAuraOnly,
    ControlledAurasAndEquipmentAlreadyAttached,
}

impl AttachmentExceptionPolicy {
    fn stable_id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::GrantingAuraOnly => "granting-aura-only",
            Self::ControlledAurasAndEquipmentAlreadyAttached => {
                "controlled-auras-and-equipment-already-attached"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetingProtectionProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    kind: TargetingProtectionKind,
    recipient: ProtectionRecipient,
    duration: ProtectionDuration,
    attachment_exception: AttachmentExceptionPolicy,
}

impl TargetingProtectionProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &TargetingProtectionKind {
        &self.kind
    }

    pub fn recipient(&self) -> ProtectionRecipient {
        self.recipient
    }

    pub fn duration(&self) -> ProtectionDuration {
        self.duration
    }

    pub fn attachment_exception(&self) -> AttachmentExceptionPolicy {
        self.attachment_exception
    }

    pub const fn production_adapter_connected(&self) -> bool {
        targeting_protection_production_adapter_connected()
    }
}

/// Compile a complete clause. Whitespace trimming, compound keyword clauses,
/// extra effects, and unsupported quality prose all fail closed.
pub fn compile_targeting_protection_program(
    exact_source: &str,
) -> Option<TargetingProtectionProgram> {
    if exact_source.is_empty()
        || exact_source.trim() != exact_source
        || exact_source.contains('\n')
        || exact_source.contains('\r')
    {
        return None;
    }

    let compiled = compile_enchanted_creature_protection(exact_source)
        .or_else(|| compile_player_targeting_restriction(exact_source))
        .or_else(|| compile_singleton_keyword(exact_source))?;
    Some(build_program(exact_source, compiled))
}

#[derive(Debug)]
struct CompiledParts {
    kind: TargetingProtectionKind,
    recipient: ProtectionRecipient,
    duration: ProtectionDuration,
    attachment_exception: AttachmentExceptionPolicy,
}

fn build_program(exact_source: &str, compiled: CompiledParts) -> TargetingProtectionProgram {
    let normalized_source = normalize_oracle_phrase(exact_source);
    let semantic_digest = targeting_protection_semantic_digest(
        exact_source,
        &normalized_source,
        &compiled.kind,
        compiled.recipient,
        compiled.duration,
        compiled.attachment_exception,
    );
    TargetingProtectionProgram {
        exact_source: exact_source.to_owned(),
        normalized_source,
        semantic_digest,
        kind: compiled.kind,
        recipient: compiled.recipient,
        duration: compiled.duration,
        attachment_exception: compiled.attachment_exception,
    }
}

fn compile_singleton_keyword(source: &str) -> Option<CompiledParts> {
    let (core, reminder) = split_exact_trailing_parenthetical(source)?;
    let lower = core.to_ascii_lowercase();
    let kind = if lower == "shroud" {
        if reminder.is_some_and(|value| !canonical_shroud_reminder(value)) {
            return None;
        }
        TargetingProtectionKind::Shroud
    } else if lower == "hexproof" {
        if reminder.is_some_and(|value| !canonical_hexproof_reminder(value, None)) {
            return None;
        }
        TargetingProtectionKind::Hexproof {
            qualities: Vec::new(),
        }
    } else if let Some(quality_text) = lower.strip_prefix("hexproof from ") {
        let original_quality_text = core.get("Hexproof from ".len()..)?;
        let qualities = parse_quality_list(original_quality_text)?;
        if reminder.is_some_and(|value| !canonical_hexproof_reminder(value, Some(quality_text))) {
            return None;
        }
        TargetingProtectionKind::Hexproof { qualities }
    } else {
        let quality_text = lower.strip_prefix("protection from ")?;
        let qualities = parse_quality_list_with_original(core.get("Protection from ".len()..)?)?;
        if reminder.is_some_and(|value| !canonical_protection_reminder(value, quality_text)) {
            return None;
        }
        TargetingProtectionKind::Protection { qualities }
    };

    Some(CompiledParts {
        kind,
        recipient: ProtectionRecipient::SourceObject,
        duration: ProtectionDuration::WhileSourceAbilityApplies,
        attachment_exception: AttachmentExceptionPolicy::None,
    })
}

fn compile_player_targeting_restriction(source: &str) -> Option<CompiledParts> {
    let (core, reminder) = split_exact_trailing_parenthetical(source)?;
    let lower = core.to_ascii_lowercase();
    let (kind, duration) = match lower.as_str() {
        "you have shroud." => (
            TargetingProtectionKind::Shroud,
            ProtectionDuration::WhileSourceAbilityApplies,
        ),
        "you gain shroud until end of turn." => (
            TargetingProtectionKind::Shroud,
            ProtectionDuration::UntilEndOfTurn,
        ),
        "you have hexproof." => (
            TargetingProtectionKind::Hexproof {
                qualities: Vec::new(),
            },
            ProtectionDuration::WhileSourceAbilityApplies,
        ),
        _ => return None,
    };
    if let Some(reminder) = reminder {
        let normalized = normalize_oracle_phrase(reminder);
        let valid = match kind {
            TargetingProtectionKind::Shroud => {
                normalized == "you can't be the target of spells or abilities."
            }
            TargetingProtectionKind::Hexproof { .. } => {
                normalized
                    == "you can't be the target of spells or abilities your opponents control."
                    || normalized
                        == "you can't be the target of spells or abilities your opponents control, including aura spells."
            }
            TargetingProtectionKind::Protection { .. } => false,
        };
        if !valid {
            return None;
        }
    }
    Some(CompiledParts {
        kind,
        recipient: ProtectionRecipient::ControllerPlayer,
        duration,
        attachment_exception: AttachmentExceptionPolicy::None,
    })
}

fn compile_enchanted_creature_protection(source: &str) -> Option<CompiledParts> {
    let prefix = "Enchanted creature has protection from ";
    let body = source.strip_prefix(prefix)?;
    let (quality_text, attachment_exception) = if let Some(quality) =
        body.strip_suffix(". This effect doesn't remove this Aura.")
    {
        (quality, AttachmentExceptionPolicy::GrantingAuraOnly)
    } else {
        let quality = body.strip_suffix(
        ". This effect doesn't remove Auras and Equipment you control that are already attached to it.",
    )?;
        (
            quality,
            AttachmentExceptionPolicy::ControlledAurasAndEquipmentAlreadyAttached,
        )
    };
    let qualities = parse_quality_list_with_original(quality_text)?;
    Some(CompiledParts {
        kind: TargetingProtectionKind::Protection { qualities },
        recipient: ProtectionRecipient::EnchantedCreature,
        duration: ProtectionDuration::WhileSourceAbilityApplies,
        attachment_exception,
    })
}

fn split_exact_trailing_parenthetical(source: &str) -> Option<(&str, Option<&str>)> {
    let mut depth = 0u32;
    let mut open = None;
    let mut close = None;
    for (index, character) in source.char_indices() {
        match character {
            '(' => {
                if depth == 0 {
                    if open.is_some()
                        || index == 0
                        || !source[..index]
                            .chars()
                            .next_back()
                            .is_some_and(char::is_whitespace)
                    {
                        return None;
                    }
                    open = Some(index);
                }
                depth = depth.checked_add(1)?;
            }
            ')' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    close = Some(index);
                }
            }
            _ if close.is_some() => return None,
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    match (open, close) {
        (None, None) => Some((source, None)),
        (Some(open), Some(close)) if close + 1 == source.len() => {
            let core = source[..open].trim_end();
            let reminder = source[open + 1..close].trim();
            (!core.is_empty() && !reminder.is_empty()).then_some((core, Some(reminder)))
        }
        _ => None,
    }
}

fn parse_quality_list(quality_text: &str) -> Option<Vec<ProtectionQualitySpec>> {
    parse_quality_list_with_original(quality_text)
}

fn parse_quality_list_with_original(quality_text: &str) -> Option<Vec<ProtectionQualitySpec>> {
    if quality_text.is_empty()
        || quality_text.trim() != quality_text
        || quality_text.contains(';')
        || quality_text.contains('(')
        || quality_text.contains(')')
    {
        return None;
    }
    let separated = quality_text
        .replace(", and from ", "\u{0}")
        .replace(", from ", "\u{0}")
        .replace(" and from ", "\u{0}")
        .replace(", and ", "\u{0}")
        .replace(", ", "\u{0}")
        .replace(" and ", "\u{0}");
    let mut qualities = separated
        .split('\u{0}')
        .map(str::trim)
        .map(parse_quality)
        .collect::<Option<Vec<_>>>()?;
    if qualities.is_empty() {
        return None;
    }
    qualities.sort();
    qualities.dedup();
    Some(qualities)
}

fn parse_quality(quality: &str) -> Option<ProtectionQualitySpec> {
    let lower = quality.to_ascii_lowercase();
    let color = match lower.as_str() {
        "white" => Some(ManaColor::White),
        "blue" => Some(ManaColor::Blue),
        "black" => Some(ManaColor::Black),
        "red" => Some(ManaColor::Red),
        "green" => Some(ManaColor::Green),
        _ => None,
    };
    if let Some(color) = color {
        return Some(ProtectionQualitySpec::Color(color));
    }

    match lower.as_str() {
        "everything" => Some(ProtectionQualitySpec::Everything),
        "each color" => Some(ProtectionQualitySpec::EachColor),
        "the color of your choice" | "a color of your choice" | "the chosen color" => {
            Some(ProtectionQualitySpec::ChosenColor)
        }
        "the colors of permanents you control" | "colors of permanents you control" => {
            Some(ProtectionQualitySpec::ColorsOfPermanentsYouControl)
        }
        "each of its colors" => Some(ProtectionQualitySpec::EachOfProtectedObjectsColors),
        "colored" => Some(ProtectionQualitySpec::Colored),
        "colorless" => Some(ProtectionQualitySpec::Colorless),
        "monocolored" => Some(ProtectionQualitySpec::Monocolored),
        "multicolored" => Some(ProtectionQualitySpec::Multicolored),
        "enemy-colored multicolored" => Some(ProtectionQualitySpec::EnemyColorPair),
        "artifacts" | "artifact" => Some(ProtectionQualitySpec::CardType("artifact".into())),
        "battles" | "battle" => Some(ProtectionQualitySpec::CardType("battle".into())),
        "creatures" | "creature" => Some(ProtectionQualitySpec::CardType("creature".into())),
        "enchantments" | "enchantment" => {
            Some(ProtectionQualitySpec::CardType("enchantment".into()))
        }
        "instants" | "instant" => Some(ProtectionQualitySpec::CardType("instant".into())),
        "kindreds" | "kindred" => Some(ProtectionQualitySpec::CardType("kindred".into())),
        "lands" | "land" => Some(ProtectionQualitySpec::CardType("land".into())),
        "planeswalkers" | "planeswalker" => {
            Some(ProtectionQualitySpec::CardType("planeswalker".into()))
        }
        "sorceries" | "sorcery" => Some(ProtectionQualitySpec::CardType("sorcery".into())),
        "the chosen player" | "a player" => Some(ProtectionQualitySpec::ChosenPlayer),
        "snow" => Some(ProtectionQualitySpec::Supertype("snow".into())),
        "modified creatures" => Some(ProtectionQualitySpec::ModifiedCreature),
        "wordy" => Some(ProtectionQualitySpec::Wordy),
        "activated abilities" => Some(ProtectionQualitySpec::EffectKind(
            EffectSourceKind::ActivatedAbility,
        )),
        "triggered abilities" => Some(ProtectionQualitySpec::EffectKind(
            EffectSourceKind::TriggeredAbility,
        )),
        "spells" => Some(ProtectionQualitySpec::EffectKind(EffectSourceKind::Spell)),
        "creatures with two or more creature types" => {
            Some(ProtectionQualitySpec::CreatureWithAtLeastCreatureTypes(2))
        }
        "die rolls" => Some(ProtectionQualitySpec::CausesDieRoll),
        _ => parse_structured_or_subtype_quality(quality, &lower),
    }
}

fn parse_structured_or_subtype_quality(
    original: &str,
    lower: &str,
) -> Option<ProtectionQualitySpec> {
    if let Some(name) = lower.strip_prefix("cards named ") {
        let original_name = original.get("cards named ".len()..)?.trim();
        if !name.is_empty() && !original_name.is_empty() {
            return Some(ProtectionQualitySpec::Named(original_name.to_owned()));
        }
    }
    if let Some(value) = lower
        .strip_prefix("mana value ")
        .and_then(|suffix| suffix.strip_suffix(" or less"))
        .and_then(|value| value.parse::<u32>().ok())
    {
        return Some(ProtectionQualitySpec::ManaValueAtMost(value));
    }
    if let Some(value) = lower
        .strip_prefix("mana value ")
        .and_then(|suffix| suffix.strip_suffix(" or greater"))
        .and_then(|value| value.parse::<u32>().ok())
    {
        return Some(ProtectionQualitySpec::ManaValueAtLeast(value));
    }
    if let Some(subtype) = original
        .strip_prefix("non-")
        .and_then(|value| value.strip_suffix(" creatures"))
    {
        return canonical_subtype_phrase(subtype).map(ProtectionQualitySpec::NonSubtypeCreature);
    }
    if let Some(counter) = lower
        .strip_prefix("permanents with ")
        .and_then(|value| value.strip_suffix(" counters on them"))
        && !counter.is_empty()
    {
        return Some(ProtectionQualitySpec::PermanentWithCounter(
            counter.to_owned(),
        ));
    }
    canonical_subtype_phrase(original).map(ProtectionQualitySpec::Subtype)
}

fn canonical_subtype_phrase(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || !value.split_whitespace().all(|word| {
            word.chars().next().is_some_and(char::is_uppercase)
                && word
                    .chars()
                    .all(|character| character.is_alphabetic() || matches!(character, '-' | '\''))
        })
    {
        return None;
    }
    let words = value
        .split_whitespace()
        .map(singularize_subtype_word)
        .collect::<Vec<_>>();
    Some(words.join(" "))
}

fn singularize_subtype_word(word: &str) -> String {
    match word {
        "Elves" => "Elf".into(),
        "Dwarves" => "Dwarf".into(),
        "Werewolves" => "Werewolf".into(),
        "Allies" => "Ally".into(),
        "Mercenaries" => "Mercenary".into(),
        "Kavu" | "Elk" | "Arcane" => word.into(),
        _ if word.ends_with("ies") && word.len() > 3 => {
            format!("{}y", &word[..word.len() - 3])
        }
        _ if word.ends_with('s') && !word.ends_with("ss") => word[..word.len() - 1].to_owned(),
        _ => word.into(),
    }
}

fn canonical_shroud_reminder(reminder: &str) -> bool {
    matches!(
        normalize_oracle_phrase(reminder).as_str(),
        "a permanent with shroud can't be the target of spells or abilities."
            | "this creature can't be the target of spells or abilities."
            | "this enchantment can't be the target of spells or abilities."
            | "this permanent can't be the target of spells or abilities."
            | "this object can't be the target of spells or abilities."
    )
}

fn canonical_hexproof_reminder(reminder: &str, quality: Option<&str>) -> bool {
    let reminder = normalize_oracle_phrase(reminder);
    match quality {
        None => matches!(
            reminder.as_str(),
            "this creature can't be the target of spells or abilities your opponents control."
                | "this permanent can't be the target of spells or abilities your opponents control."
                | "this object can't be the target of spells or abilities your opponents control."
        ),
        Some(quality) => {
            let quality = reminder_quality_phrase(quality);
            ["creature", "permanent", "object"].into_iter().any(|noun| {
                reminder
                    == format!(
                        "this {noun} can't be the target of {quality} spells or abilities your opponents control."
                    )
            })
        }
    }
}

fn canonical_protection_reminder(reminder: &str, quality: &str) -> bool {
    let normalized = normalize_oracle_phrase(reminder);
    let lower_quality = normalize_oracle_phrase(quality);
    match lower_quality.as_str() {
        "modified creatures" => {
            return normalized
                == "modified creatures have a power, toughness, or ability different than their printed versions.";
        }
        "wordy" => {
            return normalized == "something is wordy if it has four or more lines of rules text.";
        }
        "enemy-colored multicolored" => {
            return normalized
                == "this creature can't be blocked, targeted, dealt damage, enchanted, or equipped by anything that's two enemy colors, such as blue and green.";
        }
        _ => {}
    }
    let quality = reminder_quality_phrase(quality);
    ["creature", "permanent", "object"].into_iter().any(|noun| {
        normalized
            == format!(
                "this {noun} can't be blocked, targeted, dealt damage, enchanted, or equipped by anything {quality}."
            )
            || normalized
                == format!(
                    "this {noun} can't be blocked, targeted, dealt damage, or enchanted by anything {quality}."
                )
    })
}

fn reminder_quality_phrase(quality: &str) -> String {
    normalize_oracle_phrase(quality)
        .replace(", and from ", " or ")
        .replace(", from ", " or ")
        .replace(" and from ", " or ")
        .replace(", and ", " or ")
        .replace(", ", " or ")
        .replace(" and ", " or ")
}

fn normalize_oracle_phrase(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            _ => character,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn canonical_characteristic(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn quality_list_stable_id(qualities: &[ProtectionQualitySpec]) -> String {
    qualities
        .iter()
        .map(ProtectionQualitySpec::stable_id)
        .collect::<Vec<_>>()
        .join("+")
}

fn targeting_protection_semantic_digest(
    exact_source: &str,
    normalized_source: &str,
    kind: &TargetingProtectionKind,
    recipient: ProtectionRecipient,
    duration: ProtectionDuration,
    attachment_exception: AttachmentExceptionPolicy,
) -> String {
    targeting_protection_semantic_digest_with_versions(
        TARGETING_PROTECTION_COMPILER_VERSION,
        TARGETING_PROTECTION_RUNTIME_VERSION,
        TARGETING_PROTECTION_RULES_CONTEXT_VERSION,
        exact_source,
        normalized_source,
        kind,
        recipient,
        duration,
        attachment_exception,
    )
}

#[allow(clippy::too_many_arguments)]
fn targeting_protection_semantic_digest_with_versions(
    compiler_version: &str,
    runtime_version: &str,
    rules_context_version: &str,
    exact_source: &str,
    normalized_source: &str,
    kind: &TargetingProtectionKind,
    recipient: ProtectionRecipient,
    duration: ProtectionDuration,
    attachment_exception: AttachmentExceptionPolicy,
) -> String {
    let kind = kind.stable_id();
    let mut hasher = Sha256::new();
    for component in [
        "targeting-protection-content/v1",
        compiler_version,
        runtime_version,
        rules_context_version,
        exact_source,
        normalized_source,
        kind.as_str(),
        recipient.stable_id(),
        duration.stable_id(),
        attachment_exception.stable_id(),
        "protection-effects/targeting+attachment+damage+blocking",
        "hexproof/opponents-only",
        "shroud/no-controller-exception",
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectedEntity {
    Object(ObjectRef),
    Player(PlayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectZone {
    Battlefield,
    Stack,
    Graveyard,
    Exile,
    Hand,
    Library,
    Command,
    OutsideGame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCharacteristics {
    pub name: String,
    pub colors: BTreeSet<ManaColor>,
    pub card_types: BTreeSet<String>,
    pub subtypes: BTreeSet<String>,
    pub supertypes: BTreeSet<String>,
    pub mana_value: Option<u32>,
    pub zone: ObjectZone,
    pub counters: BTreeMap<String, u32>,
    /// Exact current comparison against the printed power, toughness, and
    /// abilities. `None` means the caller did not supply enough evidence.
    pub modified_from_printed: Option<bool>,
    /// Physical Oracle rules text line count used by the "wordy" quality.
    pub rules_text_line_count: Option<u32>,
    pub causes_die_roll: Option<bool>,
}

impl SourceCharacteristics {
    fn has_card_type(&self, expected: &str) -> bool {
        set_contains_characteristic(&self.card_types, expected)
    }

    fn has_subtype(&self, expected: &str) -> bool {
        set_contains_characteristic(&self.subtypes, expected)
    }

    fn has_supertype(&self, expected: &str) -> bool {
        set_contains_characteristic(&self.supertypes, expected)
    }

    fn is_permanent(&self) -> bool {
        self.zone == ObjectZone::Battlefield
            && [
                "artifact",
                "battle",
                "creature",
                "enchantment",
                "land",
                "planeswalker",
            ]
            .into_iter()
            .any(|card_type| self.has_card_type(card_type))
    }
}

fn set_contains_characteristic(values: &BTreeSet<String>, expected: &str) -> bool {
    let expected = canonical_characteristic(expected);
    values
        .iter()
        .any(|value| canonical_characteristic(value) == expected)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectSourceSnapshot {
    /// `None` is permitted only for a player-originating rules action.
    pub source_object: Option<ObjectRef>,
    /// Controller of the spell or ability for Hexproof and player qualities.
    pub effect_controller: PlayerId,
    pub effect_kind: EffectSourceKind,
    /// Last known information must be supplied here when the source object is
    /// no longer in the zone where the event originated.
    pub characteristics: Option<SourceCharacteristics>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectionChoices {
    pub chosen_color: Option<ManaColor>,
    pub chosen_player: Option<PlayerId>,
}

impl ProtectionChoices {
    pub const fn none() -> Self {
        Self {
            chosen_color: None,
            chosen_player: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Aura,
    Equipment,
    Fortification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachmentSnapshot {
    pub source: ObjectRef,
    pub kind: AttachmentKind,
    pub controller: PlayerId,
    pub attached_to: ObjectRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectionInstallationInput {
    pub effect_controller: PlayerId,
    pub protected: ProtectedEntity,
    pub choices: ProtectionChoices,
    /// Required only by an enchanted-creature program.
    pub granting_aura: Option<AttachmentSnapshot>,
    /// Required evidence only by the explicit existing-attachment exception.
    pub controlled_auras_and_equipment_already_attached: Vec<AttachmentSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledTargetingProtection {
    semantic_digest: String,
    kind: TargetingProtectionKind,
    recipient: ProtectionRecipient,
    duration: ProtectionDuration,
    effect_controller: PlayerId,
    protected: ProtectedEntity,
    resolved_chosen_color: Option<ManaColor>,
    resolved_chosen_player: Option<PlayerId>,
    attachment_exceptions: BTreeSet<ObjectRef>,
}

impl InstalledTargetingProtection {
    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &TargetingProtectionKind {
        &self.kind
    }

    pub fn protected(&self) -> ProtectedEntity {
        self.protected
    }

    pub fn duration(&self) -> ProtectionDuration {
        self.duration
    }

    pub fn attachment_exceptions(&self) -> &BTreeSet<ObjectRef> {
        &self.attachment_exceptions
    }
}

pub fn install_targeting_protection(
    program: &TargetingProtectionProgram,
    input: ProtectionInstallationInput,
) -> Result<InstalledTargetingProtection, TargetingProtectionError> {
    validate_recipient(program.recipient, input.effect_controller, input.protected)?;
    let expects_color =
        program_kind_qualities(&program.kind).contains(&ProtectionQualitySpec::ChosenColor);
    let expects_player =
        program_kind_qualities(&program.kind).contains(&ProtectionQualitySpec::ChosenPlayer);
    if expects_color != input.choices.chosen_color.is_some()
        || expects_player != input.choices.chosen_player.is_some()
    {
        return Err(TargetingProtectionError::ProtectionChoiceMismatch);
    }

    let protected_object = match input.protected {
        ProtectedEntity::Object(object) => Some(object),
        ProtectedEntity::Player(_) => None,
    };
    let attachment_exceptions = match program.attachment_exception {
        AttachmentExceptionPolicy::None => {
            if input.granting_aura.is_some()
                || !input
                    .controlled_auras_and_equipment_already_attached
                    .is_empty()
            {
                return Err(TargetingProtectionError::UnexpectedAttachmentEvidence);
            }
            BTreeSet::new()
        }
        AttachmentExceptionPolicy::GrantingAuraOnly => {
            if !input
                .controlled_auras_and_equipment_already_attached
                .is_empty()
            {
                return Err(TargetingProtectionError::UnexpectedAttachmentEvidence);
            }
            let aura = input
                .granting_aura
                .ok_or(TargetingProtectionError::MissingGrantingAura)?;
            validate_exception_attachment(
                aura,
                AttachmentKind::Aura,
                input.effect_controller,
                protected_object.ok_or(TargetingProtectionError::RecipientMismatch)?,
            )?;
            BTreeSet::from([aura.source])
        }
        AttachmentExceptionPolicy::ControlledAurasAndEquipmentAlreadyAttached => {
            if input.granting_aura.is_some() {
                return Err(TargetingProtectionError::UnexpectedAttachmentEvidence);
            }
            let protected = protected_object.ok_or(TargetingProtectionError::RecipientMismatch)?;
            let mut exceptions = BTreeSet::new();
            for attachment in input.controlled_auras_and_equipment_already_attached {
                if !matches!(
                    attachment.kind,
                    AttachmentKind::Aura | AttachmentKind::Equipment
                ) {
                    return Err(TargetingProtectionError::InvalidAttachmentException);
                }
                if attachment.controller != input.effect_controller
                    || attachment.attached_to != protected
                {
                    return Err(TargetingProtectionError::InvalidAttachmentException);
                }
                exceptions.insert(attachment.source);
            }
            exceptions
        }
    };

    Ok(InstalledTargetingProtection {
        semantic_digest: program.semantic_digest.clone(),
        kind: program.kind.clone(),
        recipient: program.recipient,
        duration: program.duration,
        effect_controller: input.effect_controller,
        protected: input.protected,
        resolved_chosen_color: input.choices.chosen_color,
        resolved_chosen_player: input.choices.chosen_player,
        attachment_exceptions,
    })
}

fn validate_recipient(
    recipient: ProtectionRecipient,
    effect_controller: PlayerId,
    protected: ProtectedEntity,
) -> Result<(), TargetingProtectionError> {
    let valid = match (recipient, protected) {
        (ProtectionRecipient::SourceObject, ProtectedEntity::Object(_))
        | (ProtectionRecipient::EnchantedCreature, ProtectedEntity::Object(_)) => true,
        (ProtectionRecipient::ControllerPlayer, ProtectedEntity::Player(protected_player)) => {
            protected_player == effect_controller
        }
        _ => false,
    };
    valid
        .then_some(())
        .ok_or(TargetingProtectionError::RecipientMismatch)
}

fn validate_exception_attachment(
    attachment: AttachmentSnapshot,
    expected_kind: AttachmentKind,
    expected_controller: PlayerId,
    expected_recipient: ObjectRef,
) -> Result<(), TargetingProtectionError> {
    if attachment.kind != expected_kind
        || attachment.controller != expected_controller
        || attachment.attached_to != expected_recipient
    {
        return Err(TargetingProtectionError::InvalidAttachmentException);
    }
    Ok(())
}

fn program_kind_qualities(kind: &TargetingProtectionKind) -> &[ProtectionQualitySpec] {
    match kind {
        TargetingProtectionKind::Shroud => &[],
        TargetingProtectionKind::Hexproof { qualities }
        | TargetingProtectionKind::Protection { qualities } => qualities,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectionQueryContext {
    pub opponents_of_protected_controller: BTreeSet<PlayerId>,
    /// Current colors of the protected object. Required only by
    /// `EachOfProtectedObjectsColors`.
    pub protected_object_colors: Option<BTreeSet<ManaColor>>,
    /// Current union of colors among permanents controlled by each player.
    pub permanent_colors_by_controller: BTreeMap<PlayerId, BTreeSet<ManaColor>>,
}

impl ProtectionQueryContext {
    pub fn validate(&self, protected: ProtectedEntity) -> Result<(), TargetingProtectionError> {
        let protected_controller = match protected {
            ProtectedEntity::Player(player) => player,
            ProtectedEntity::Object(_) => return Ok(()),
        };
        if self
            .opponents_of_protected_controller
            .contains(&protected_controller)
        {
            return Err(TargetingProtectionError::InvalidOpponentRelationship);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetingDecision {
    Allowed,
    ForbiddenByShroud,
    ForbiddenByHexproof,
    ForbiddenByProtection,
}

pub fn targeting_decision(
    protection: &InstalledTargetingProtection,
    source: &EffectSourceSnapshot,
    context: &ProtectionQueryContext,
) -> Result<TargetingDecision, TargetingProtectionError> {
    context.validate(protection.protected)?;
    match &protection.kind {
        TargetingProtectionKind::Shroud => Ok(TargetingDecision::ForbiddenByShroud),
        TargetingProtectionKind::Hexproof { qualities } => {
            if !context
                .opponents_of_protected_controller
                .contains(&source.effect_controller)
            {
                return Ok(TargetingDecision::Allowed);
            }
            if qualities.is_empty() || any_quality_matches(protection, qualities, source, context)?
            {
                Ok(TargetingDecision::ForbiddenByHexproof)
            } else {
                Ok(TargetingDecision::Allowed)
            }
        }
        TargetingProtectionKind::Protection { qualities } => {
            if any_quality_matches(protection, qualities, source, context)? {
                Ok(TargetingDecision::ForbiddenByProtection)
            } else {
                Ok(TargetingDecision::Allowed)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentDecision {
    Allowed,
    AllowedByExplicitException,
    ForbiddenByProtection,
}

pub fn attachment_decision(
    protection: &InstalledTargetingProtection,
    attachment: AttachmentSnapshot,
    source: &EffectSourceSnapshot,
    context: &ProtectionQueryContext,
) -> Result<AttachmentDecision, TargetingProtectionError> {
    if source.source_object != Some(attachment.source) {
        return Err(TargetingProtectionError::AttachmentSourceMismatch);
    }
    let TargetingProtectionKind::Protection { qualities } = &protection.kind else {
        return Ok(AttachmentDecision::Allowed);
    };
    if !any_quality_matches(protection, qualities, source, context)? {
        return Ok(AttachmentDecision::Allowed);
    }
    if protection
        .attachment_exceptions
        .contains(&attachment.source)
    {
        return Ok(AttachmentDecision::AllowedByExplicitException);
    }
    Ok(AttachmentDecision::ForbiddenByProtection)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockingDecision {
    Allowed,
    ForbiddenByProtection,
}

pub fn blocking_decision(
    protection: &InstalledTargetingProtection,
    blocker: &EffectSourceSnapshot,
    context: &ProtectionQueryContext,
) -> Result<BlockingDecision, TargetingProtectionError> {
    let TargetingProtectionKind::Protection { qualities } = &protection.kind else {
        return Ok(BlockingDecision::Allowed);
    };
    let characteristics = blocker
        .characteristics
        .as_ref()
        .ok_or(TargetingProtectionError::MissingSourceCharacteristics)?;
    if !characteristics.has_card_type("creature") || characteristics.zone != ObjectZone::Battlefield
    {
        return Err(TargetingProtectionError::InvalidBlockingSource);
    }
    if any_quality_matches(protection, qualities, blocker, context)? {
        Ok(BlockingDecision::ForbiddenByProtection)
    } else {
        Ok(BlockingDecision::Allowed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamagePreventionDecision {
    NotApplicable,
    PreventAll { amount: u32 },
    CannotPrevent { amount: u32 },
}

pub fn damage_prevention_decision(
    protection: &InstalledTargetingProtection,
    source: &EffectSourceSnapshot,
    amount: u32,
    damage_can_be_prevented: bool,
    context: &ProtectionQueryContext,
) -> Result<DamagePreventionDecision, TargetingProtectionError> {
    let TargetingProtectionKind::Protection { qualities } = &protection.kind else {
        return Ok(DamagePreventionDecision::NotApplicable);
    };
    if amount == 0 || !any_quality_matches(protection, qualities, source, context)? {
        return Ok(DamagePreventionDecision::NotApplicable);
    }
    if damage_can_be_prevented {
        Ok(DamagePreventionDecision::PreventAll { amount })
    } else {
        Ok(DamagePreventionDecision::CannotPrevent { amount })
    }
}

fn any_quality_matches(
    protection: &InstalledTargetingProtection,
    qualities: &[ProtectionQualitySpec],
    source: &EffectSourceSnapshot,
    context: &ProtectionQueryContext,
) -> Result<bool, TargetingProtectionError> {
    for quality in qualities {
        if quality_matches(protection, quality, source, context)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn quality_matches(
    protection: &InstalledTargetingProtection,
    quality: &ProtectionQualitySpec,
    source: &EffectSourceSnapshot,
    context: &ProtectionQueryContext,
) -> Result<bool, TargetingProtectionError> {
    if *quality == ProtectionQualitySpec::Everything {
        return Ok(true);
    }
    if *quality == ProtectionQualitySpec::ChosenPlayer {
        return Ok(protection
            .resolved_chosen_player
            .ok_or(TargetingProtectionError::ProtectionChoiceMismatch)?
            == source.effect_controller);
    }
    if let ProtectionQualitySpec::EffectKind(expected) = quality {
        return Ok(source.effect_kind == *expected);
    }

    let characteristics = source
        .characteristics
        .as_ref()
        .ok_or(TargetingProtectionError::MissingSourceCharacteristics)?;
    let matched = match quality {
        ProtectionQualitySpec::Everything
        | ProtectionQualitySpec::ChosenPlayer
        | ProtectionQualitySpec::EffectKind(_) => unreachable!(),
        ProtectionQualitySpec::Color(color) => characteristics.colors.contains(color),
        ProtectionQualitySpec::EachColor => !characteristics.colors.is_empty(),
        ProtectionQualitySpec::ChosenColor => characteristics.colors.contains(
            &protection
                .resolved_chosen_color
                .ok_or(TargetingProtectionError::ProtectionChoiceMismatch)?,
        ),
        ProtectionQualitySpec::ColorsOfPermanentsYouControl => {
            let colors = context
                .permanent_colors_by_controller
                .get(&protection.effect_controller)
                .ok_or(TargetingProtectionError::MissingDynamicColorEvidence)?;
            !characteristics.colors.is_disjoint(colors)
        }
        ProtectionQualitySpec::EachOfProtectedObjectsColors => {
            let colors = context
                .protected_object_colors
                .as_ref()
                .ok_or(TargetingProtectionError::MissingProtectedObjectColors)?;
            !characteristics.colors.is_disjoint(colors)
        }
        ProtectionQualitySpec::Colored => !characteristics.colors.is_empty(),
        ProtectionQualitySpec::Colorless => characteristics.colors.is_empty(),
        ProtectionQualitySpec::Monocolored => characteristics.colors.len() == 1,
        ProtectionQualitySpec::Multicolored => characteristics.colors.len() > 1,
        ProtectionQualitySpec::EnemyColorPair => is_exact_enemy_color_pair(&characteristics.colors),
        ProtectionQualitySpec::CardType(card_type) => characteristics.has_card_type(card_type),
        ProtectionQualitySpec::Subtype(subtype) => characteristics.has_subtype(subtype),
        ProtectionQualitySpec::Supertype(supertype) => characteristics.has_supertype(supertype),
        ProtectionQualitySpec::Named(name) => {
            canonical_characteristic(&characteristics.name) == canonical_characteristic(name)
        }
        ProtectionQualitySpec::ManaValueAtMost(maximum) => {
            characteristics
                .mana_value
                .ok_or(TargetingProtectionError::MissingManaValue)?
                <= *maximum
        }
        ProtectionQualitySpec::ManaValueAtLeast(minimum) => {
            characteristics
                .mana_value
                .ok_or(TargetingProtectionError::MissingManaValue)?
                >= *minimum
        }
        ProtectionQualitySpec::NonSubtypeCreature(subtype) => {
            characteristics.has_card_type("creature") && !characteristics.has_subtype(subtype)
        }
        ProtectionQualitySpec::ModifiedCreature => {
            characteristics.has_card_type("creature")
                && characteristics
                    .modified_from_printed
                    .ok_or(TargetingProtectionError::MissingModifiedEvidence)?
        }
        ProtectionQualitySpec::PermanentWithCounter(counter) => {
            characteristics.is_permanent()
                && characteristics.counters.iter().any(|(name, amount)| {
                    canonical_characteristic(name) == canonical_characteristic(counter)
                        && *amount > 0
                })
        }
        ProtectionQualitySpec::Wordy => {
            characteristics
                .rules_text_line_count
                .ok_or(TargetingProtectionError::MissingRulesTextLineCount)?
                >= 4
        }
        ProtectionQualitySpec::CreatureWithAtLeastCreatureTypes(minimum) => {
            characteristics.has_card_type("creature")
                && characteristics.subtypes.len() >= *minimum as usize
        }
        ProtectionQualitySpec::CausesDieRoll => characteristics
            .causes_die_roll
            .ok_or(TargetingProtectionError::MissingDieRollEvidence)?,
    };
    Ok(matched)
}

fn is_exact_enemy_color_pair(colors: &BTreeSet<ManaColor>) -> bool {
    if colors.len() != 2 {
        return false;
    }
    let has = |left, right| colors.contains(&left) && colors.contains(&right);
    has(ManaColor::White, ManaColor::Black)
        || has(ManaColor::White, ManaColor::Red)
        || has(ManaColor::Blue, ManaColor::Red)
        || has(ManaColor::Blue, ManaColor::Green)
        || has(ManaColor::Black, ManaColor::Green)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetingProtectionError {
    RecipientMismatch,
    ProtectionChoiceMismatch,
    MissingGrantingAura,
    UnexpectedAttachmentEvidence,
    InvalidAttachmentException,
    AttachmentSourceMismatch,
    InvalidOpponentRelationship,
    MissingSourceCharacteristics,
    MissingManaValue,
    MissingModifiedEvidence,
    MissingRulesTextLineCount,
    MissingDieRollEvidence,
    MissingDynamicColorEvidence,
    MissingProtectedObjectColors,
    InvalidBlockingSource,
}

impl fmt::Display for TargetingProtectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::RecipientMismatch => {
                "the installed protection recipient does not match the clause"
            }
            Self::ProtectionChoiceMismatch => {
                "the supplied color or player choice does not match the protection qualities"
            }
            Self::MissingGrantingAura => "the explicit Aura exception requires the granting Aura",
            Self::UnexpectedAttachmentEvidence => {
                "attachment evidence was supplied to a clause that does not use it"
            }
            Self::InvalidAttachmentException => {
                "the attachment exception lacks exact controller, kind, or incarnation evidence"
            }
            Self::AttachmentSourceMismatch => {
                "the attachment and its effect source are different object incarnations"
            }
            Self::InvalidOpponentRelationship => {
                "the protected player is listed as their own opponent"
            }
            Self::MissingSourceCharacteristics => {
                "the protection quality requires source characteristics or last known information"
            }
            Self::MissingManaValue => "the protection quality requires the source mana value",
            Self::MissingModifiedEvidence => {
                "the modified quality requires an exact printed-characteristic comparison"
            }
            Self::MissingRulesTextLineCount => {
                "the wordy quality requires the physical Oracle rules text line count"
            }
            Self::MissingDieRollEvidence => {
                "the die-roll quality requires exact rules-action evidence"
            }
            Self::MissingDynamicColorEvidence => {
                "the dynamic color quality requires current controlled-permanent colors"
            }
            Self::MissingProtectedObjectColors => {
                "the self-color quality requires the protected object's current colors"
            }
            Self::InvalidBlockingSource => {
                "blocking protection requires a battlefield creature blocker"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for TargetingProtectionError {}
