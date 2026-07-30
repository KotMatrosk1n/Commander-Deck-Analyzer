use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::domain::{CardDefinition, DeckEntry, LineRequirement, WinSpeedReport};
use crate::effects::EffectMagnitude;
use crate::parser::normalize_card_name;
use crate::semantics::CompiledDeck;

pub(crate) const BUNDLED_POLICY_JSON: &str = include_str!("../data/commander-policy.json");
pub const BUNDLED_POLICY_VERSION: &str = "2026.02.09-r2";

#[derive(Debug, Error)]
pub enum PolicyPackageError {
    #[error("The Commander policy package is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("The Commander policy package is invalid: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommanderPolicyPackage {
    pub schema_version: u16,
    pub package_version: String,
    pub effective_date: String,
    pub verified_at: String,
    pub status: String,
    pub sources: Vec<PolicySource>,
    pub format_rules: CommanderFormatRules,
    pub bracket_policy: BracketPolicy,
    pub game_changers: Vec<String>,
    pub named_banned_cards: Vec<String>,
    pub conditional_bans: Vec<ConditionalBan>,
    pub categorical_bans: Vec<CategoricalBan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicySource {
    pub title: String,
    pub url: String,
    pub covers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommanderFormatRules {
    pub deck_size: u16,
    pub singleton: bool,
    pub basic_lands_exempt: bool,
    pub color_identity_required: bool,
    pub minimum_commanders: u8,
    pub maximum_commanders: u8,
    pub legendary_creatures_eligible: bool,
    pub explicit_oracle_permission_eligible: bool,
    pub legendary_vehicle_or_spacecraft_needs_printed_power_toughness: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BracketPolicy {
    pub game_changer_limits: Vec<GameChangerLimit>,
    pub floor_when_at_least_one: u8,
    pub floor_when_more_than: u16,
    pub floor_above_limit: u8,
    pub intent_only_brackets: Vec<u8>,
    #[serde(default)]
    pub guidance: BracketGuidancePolicy,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BracketGuidancePolicy {
    #[serde(default)]
    pub source_urls: Vec<String>,
    #[serde(default)]
    pub expected_minimum_turns: Vec<BracketTurnExpectation>,
    #[serde(default)]
    pub mass_land_denial_floor: Option<u8>,
    #[serde(default)]
    pub two_card_game_ending_floor: Option<u8>,
    #[serde(default)]
    pub early_two_card_game_ending_floor: Option<u8>,
    #[serde(default)]
    pub early_game_through_turn: Option<u8>,
    #[serde(default)]
    pub frequent_early_win_attempt_rate: Option<f32>,
    #[serde(default)]
    pub extra_turn_card_floor: Option<u8>,
    #[serde(default)]
    pub extra_turn_chain_floor: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BracketTurnExpectation {
    pub bracket: u8,
    pub minimum_turns_played: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameChangerLimit {
    pub bracket: u8,
    pub maximum: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConditionalBan {
    pub name: String,
    pub condition: ConditionalBanCondition,
    pub note: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConditionalBanCondition {
    CompanionOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CategoricalBan {
    pub category: String,
    pub source_url: String,
    pub machine_check: CategoryMachineCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CategoryMachineCheck {
    TypeLineContains { value: String },
    OracleTextContains { value: String },
    NameList { names: Vec<String> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LegalityStatus {
    Legal,
    Illegal,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolicyViolation {
    pub code: String,
    pub card_name: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ColorIdentityViolation {
    pub card_name: String,
    pub card_colors: Vec<String>,
    pub commander_colors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateViolation {
    pub card_name: String,
    pub quantity: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommanderEligibility {
    pub card_name: String,
    pub status: LegalityStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum IntentAssessmentStatus {
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntentBracketAssessment {
    pub bracket: u8,
    pub status: IntentAssessmentStatus,
    pub inferred: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BracketPolicySignalKind {
    DeterministicFloor,
    ModeledGuidance,
    ManualReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BracketPolicySignal {
    pub code: String,
    pub kind: BracketPolicySignalKind,
    pub recommended_floor: Option<u8>,
    pub title: String,
    pub detail: String,
    pub cards: Vec<String>,
    pub source_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyEvaluation {
    pub package_version: String,
    pub effective_date: String,
    pub legality: LegalityStatus,
    pub deck_card_count: u32,
    pub format_violations: Vec<PolicyViolation>,
    pub color_identity_violations: Vec<ColorIdentityViolation>,
    pub duplicate_violations: Vec<DuplicateViolation>,
    pub commander_eligibility: Vec<CommanderEligibility>,
    pub unresolved_cards: Vec<String>,
    pub game_changer_count: u16,
    pub game_changers: Vec<String>,
    /// A deterministic minimum derived from machine-checkable bracket
    /// restrictions. `None` does not establish Bracket 1 or 2.
    pub policy_floor: Option<u8>,
    pub policy_floor_reason: String,
    #[serde(default)]
    pub bracket_signals: Vec<BracketPolicySignal>,
    /// Brackets 1 and 5 depend on player/pod intent and are never inferred.
    pub intent_assessments: Vec<IntentBracketAssessment>,
    pub manual_review_reasons: Vec<String>,
}

impl Default for PolicyEvaluation {
    fn default() -> Self {
        Self {
            package_version: "unavailable".into(),
            effective_date: String::new(),
            legality: LegalityStatus::Unknown,
            deck_card_count: 0,
            format_violations: Vec::new(),
            color_identity_violations: Vec::new(),
            duplicate_violations: Vec::new(),
            commander_eligibility: Vec::new(),
            unresolved_cards: Vec::new(),
            game_changer_count: 0,
            game_changers: Vec::new(),
            policy_floor: None,
            policy_floor_reason: "Commander policy evaluation was unavailable.".into(),
            bracket_signals: Vec::new(),
            intent_assessments: Vec::new(),
            manual_review_reasons: vec!["Commander policy evaluation was unavailable.".into()],
        }
    }
}

#[derive(Debug)]
struct AggregatedEntry {
    display_name: String,
    quantity: u16,
}

pub fn bundled_policy() -> Result<CommanderPolicyPackage, PolicyPackageError> {
    let package = serde_json::from_str::<CommanderPolicyPackage>(BUNDLED_POLICY_JSON)?;
    package.validate()?;
    if package.package_version != BUNDLED_POLICY_VERSION {
        return Err(PolicyPackageError::Invalid(format!(
            "bundled version constant {BUNDLED_POLICY_VERSION} does not match package {}",
            package.package_version
        )));
    }
    Ok(package)
}

impl CommanderPolicyPackage {
    pub fn validate(&self) -> Result<(), PolicyPackageError> {
        const MAXIMUM_LIST_ENTRIES: usize = 5_000;
        const MAXIMUM_SOURCES: usize = 64;

        if self.schema_version != 1 {
            return Err(PolicyPackageError::Invalid(format!(
                "unsupported schema version {}",
                self.schema_version
            )));
        }
        if self.package_version.trim().is_empty()
            || self.effective_date.trim().is_empty()
            || self.verified_at.trim().is_empty()
        {
            return Err(PolicyPackageError::Invalid(
                "version and date fields must not be empty".into(),
            ));
        }
        if self.package_version.len() > 128
            || !self
                .package_version
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".-_+".contains(character))
        {
            return Err(PolicyPackageError::Invalid(format!(
                "package version {:?} must be at most 128 ASCII letters, numbers, dots, dashes, underscores, or plus signs",
                self.package_version,
            )));
        }
        let effective_date = parse_policy_date("effectiveDate", &self.effective_date)?;
        let verified_at = parse_policy_date("verifiedAt", &self.verified_at)?;
        if verified_at < effective_date {
            return Err(PolicyPackageError::Invalid(
                "verifiedAt cannot be earlier than effectiveDate".into(),
            ));
        }
        if self.status.trim().is_empty() || self.status.len() > 64 {
            return Err(PolicyPackageError::Invalid(
                "status must contain 1 to 64 characters".into(),
            ));
        }
        if self.format_rules.deck_size == 0
            || self.format_rules.minimum_commanders == 0
            || self.format_rules.minimum_commanders > self.format_rules.maximum_commanders
        {
            return Err(PolicyPackageError::Invalid(
                "Commander deck and commander-count limits are inconsistent".into(),
            ));
        }
        if !(1..=5).contains(&self.bracket_policy.floor_when_at_least_one)
            || !(1..=5).contains(&self.bracket_policy.floor_above_limit)
            || self.bracket_policy.floor_when_at_least_one >= self.bracket_policy.floor_above_limit
        {
            return Err(PolicyPackageError::Invalid(
                "Game Changer bracket floors are inconsistent".into(),
            ));
        }
        let brackets = self
            .bracket_policy
            .game_changer_limits
            .iter()
            .map(|limit| limit.bracket)
            .collect::<BTreeSet<_>>();
        if brackets != BTreeSet::from([1, 2, 3, 4, 5]) {
            return Err(PolicyPackageError::Invalid(
                "Game Changer limits must define all five brackets".into(),
            ));
        }
        if self.bracket_policy.intent_only_brackets != [1, 5] {
            return Err(PolicyPackageError::Invalid(
                "Brackets 1 and 5 must remain intent-only".into(),
            ));
        }
        validate_bracket_guidance(&self.bracket_policy.guidance)?;
        ensure_unique_nonempty("Game Changers", &self.game_changers)?;
        ensure_unique_nonempty("named banned cards", &self.named_banned_cards)?;
        if self.game_changers.len() > MAXIMUM_LIST_ENTRIES
            || self.named_banned_cards.len() > MAXIMUM_LIST_ENTRIES
            || self.conditional_bans.len() > MAXIMUM_LIST_ENTRIES
            || self.categorical_bans.len() > MAXIMUM_LIST_ENTRIES
        {
            return Err(PolicyPackageError::Invalid(
                "policy card and ban lists may contain at most 5,000 entries each".into(),
            ));
        }
        if self.sources.is_empty() || self.sources.len() > MAXIMUM_SOURCES {
            return Err(PolicyPackageError::Invalid(
                "policy packages must contain 1 to 64 sources".into(),
            ));
        }
        for source in &self.sources {
            validate_source(source)?;
        }
        let mut conditional_names = BTreeSet::new();
        for ban in &self.conditional_bans {
            if ban.name.trim().is_empty() || ban.note.trim().is_empty() {
                return Err(PolicyPackageError::Invalid(
                    "conditional bans require nonempty names and notes".into(),
                ));
            }
            if !conditional_names.insert(normalize_card_name(&ban.name)) {
                return Err(PolicyPackageError::Invalid(format!(
                    "conditional bans contains duplicate card name {:?}",
                    ban.name
                )));
            }
        }
        let mut category_names = BTreeSet::new();
        for ban in &self.categorical_bans {
            if ban.category.trim().is_empty() || !category_names.insert(ban.category.trim()) {
                return Err(PolicyPackageError::Invalid(
                    "categorical ban names must be nonempty and unique".into(),
                ));
            }
            validate_https_url("categorical ban sourceUrl", &ban.source_url)?;
            if let CategoryMachineCheck::NameList { names } = &ban.machine_check {
                ensure_unique_nonempty(&ban.category, names)?;
            }
        }
        Ok(())
    }
}

fn validate_bracket_guidance(guidance: &BracketGuidancePolicy) -> Result<(), PolicyPackageError> {
    for source_url in &guidance.source_urls {
        validate_https_url("bracket guidance source URL", source_url)?;
    }
    if guidance.source_urls.len() > 16 {
        return Err(PolicyPackageError::Invalid(
            "bracket guidance may cite at most 16 source URLs".into(),
        ));
    }
    if !guidance.expected_minimum_turns.is_empty() {
        let brackets = guidance
            .expected_minimum_turns
            .iter()
            .map(|expectation| expectation.bracket)
            .collect::<BTreeSet<_>>();
        if brackets != BTreeSet::from([1, 2, 3, 4, 5]) || guidance.expected_minimum_turns.len() != 5
        {
            return Err(PolicyPackageError::Invalid(
                "turn expectations must define each bracket exactly once".into(),
            ));
        }
        if guidance.expected_minimum_turns.iter().any(|expectation| {
            expectation
                .minimum_turns_played
                .is_some_and(|turns| turns == 0 || turns > 30)
        }) {
            return Err(PolicyPackageError::Invalid(
                "turn expectations must be between 1 and 30 turns, or null".into(),
            ));
        }
    }
    for (label, floor) in [
        ("massLandDenialFloor", guidance.mass_land_denial_floor),
        (
            "twoCardGameEndingFloor",
            guidance.two_card_game_ending_floor,
        ),
        (
            "earlyTwoCardGameEndingFloor",
            guidance.early_two_card_game_ending_floor,
        ),
        ("extraTurnCardFloor", guidance.extra_turn_card_floor),
        ("extraTurnChainFloor", guidance.extra_turn_chain_floor),
    ] {
        if floor.is_some_and(|value| !(1..=5).contains(&value)) {
            return Err(PolicyPackageError::Invalid(format!(
                "{label} must be a bracket from 1 through 5"
            )));
        }
    }
    if guidance
        .early_game_through_turn
        .is_some_and(|turn| turn == 0 || turn > 30)
    {
        return Err(PolicyPackageError::Invalid(
            "earlyGameThroughTurn must be between 1 and 30".into(),
        ));
    }
    if guidance
        .frequent_early_win_attempt_rate
        .is_some_and(|rate| !rate.is_finite() || !(0.0..=1.0).contains(&rate))
    {
        return Err(PolicyPackageError::Invalid(
            "frequentEarlyWinAttemptRate must be finite and between 0 and 1".into(),
        ));
    }
    if matches!(
        (
            guidance.two_card_game_ending_floor,
            guidance.early_two_card_game_ending_floor,
        ),
        (Some(general), Some(early)) if early < general
    ) {
        return Err(PolicyPackageError::Invalid(
            "the early two-card game-ending floor cannot be lower than the general floor".into(),
        ));
    }
    Ok(())
}

fn parse_policy_date(field: &str, value: &str) -> Result<NaiveDate, PolicyPackageError> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, value)| index != 4 && index != 7 && !value.is_ascii_digit())
    {
        return Err(PolicyPackageError::Invalid(format!(
            "{field} must use the exact YYYY-MM-DD format"
        )));
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        PolicyPackageError::Invalid(format!("{field} must use the exact YYYY-MM-DD format"))
    })
}

fn validate_source(source: &PolicySource) -> Result<(), PolicyPackageError> {
    if source.title.trim().is_empty()
        || source.covers.is_empty()
        || source.covers.iter().any(|item| item.trim().is_empty())
    {
        return Err(PolicyPackageError::Invalid(
            "every policy source requires a title and at least one nonempty coverage label".into(),
        ));
    }
    validate_https_url("policy source URL", &source.url)
}

fn validate_https_url(field: &str, value: &str) -> Result<(), PolicyPackageError> {
    let parsed = Url::parse(value)
        .map_err(|_| PolicyPackageError::Invalid(format!("{field} is not a valid URL")))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(PolicyPackageError::Invalid(format!(
            "{field} must be an HTTPS URL with a host and no embedded credentials"
        )));
    }
    Ok(())
}

/// Evaluates rules that can be established from a parsed deck and locally
/// resolved card definitions. It does not estimate deck power and deliberately
/// does not infer the intent-dependent Exhibition or cEDH brackets.
pub fn evaluate_commander_policy(
    policy: &CommanderPolicyPackage,
    entries: &[DeckEntry],
    definitions: &[CardDefinition],
    selected_commanders: &[String],
) -> PolicyEvaluation {
    let definition_index = build_definition_index(definitions);
    let aggregated = aggregate_entries(entries);
    let deck_card_count = aggregated
        .values()
        .map(|entry| u32::from(entry.quantity))
        .sum();
    let commander_names = effective_commanders(entries, selected_commanders);

    let mut format_violations = Vec::new();
    let mut seen_format_violations = BTreeSet::new();
    let mut manual_review_reasons = BTreeSet::new();
    let mut unresolved_cards = BTreeSet::new();
    let mut has_unknown = false;

    if deck_card_count != u32::from(policy.format_rules.deck_size) {
        push_format_violation(
            &mut format_violations,
            &mut seen_format_violations,
            "deckSize",
            None,
            format!(
                "Commander decks require exactly {} cards; this list contains {deck_card_count}.",
                policy.format_rules.deck_size
            ),
        );
    }

    if commander_names.is_empty() {
        has_unknown = true;
        manual_review_reasons.insert(
            "No commander was selected, so commander eligibility and color identity cannot be established."
                .into(),
        );
    } else if commander_names.len() > usize::from(policy.format_rules.maximum_commanders) {
        push_format_violation(
            &mut format_violations,
            &mut seen_format_violations,
            "commanderCount",
            None,
            format!(
                "At most {} commanders may be selected.",
                policy.format_rules.maximum_commanders
            ),
        );
    }

    for commander in &commander_names {
        if !aggregated
            .values()
            .any(|entry| names_overlap(&entry.display_name, commander))
        {
            push_format_violation(
                &mut format_violations,
                &mut seen_format_violations,
                "commanderMissingFromDeck",
                Some(commander.clone()),
                format!("{commander} is selected as a commander but is not in the 100-card list."),
            );
        }
    }

    let named_bans = official_name_index(&policy.named_banned_cards);
    for entry in aggregated.values() {
        let definition = find_definition(&definition_index, &entry.display_name);
        let named_ban = find_official_name(&named_bans, &entry.display_name)
            .or_else(|| definition.and_then(|card| find_official_name(&named_bans, &card.name)));
        let mut has_specific_ban = false;

        if let Some(banned_name) = named_ban {
            has_specific_ban = true;
            push_format_violation(
                &mut format_violations,
                &mut seen_format_violations,
                "bannedCard",
                Some(entry.display_name.clone()),
                format!("{banned_name} is banned in Commander."),
            );
        }

        if let Some(card) = definition {
            for category in &policy.categorical_bans {
                if category_matches(category, card) {
                    has_specific_ban = true;
                    push_format_violation(
                        &mut format_violations,
                        &mut seen_format_violations,
                        "categoricallyBannedCard",
                        Some(entry.display_name.clone()),
                        format!(
                            "{} is banned under the “{}” category.",
                            entry.display_name, category.category
                        ),
                    );
                }
            }

            if !has_specific_ban {
                match local_commander_legality(card) {
                    LegalityStatus::Illegal => push_format_violation(
                        &mut format_violations,
                        &mut seen_format_violations,
                        "notCommanderLegal",
                        Some(entry.display_name.clone()),
                        format!(
                            "{} is marked not legal in Commander by the exact local card-data status.",
                            entry.display_name
                        ),
                    ),
                    LegalityStatus::Unknown => {
                        has_unknown = true;
                        manual_review_reasons.insert(format!(
                            "{} does not have an authoritative Commander-legality value in the local card data and requires review or a card-data refresh.",
                            entry.display_name
                        ));
                    }
                    LegalityStatus::Legal => {}
                }
            }
        } else {
            unresolved_cards.insert(entry.display_name.clone());
            has_unknown = true;
            manual_review_reasons.insert(format!(
                "{} is unresolved, so its legality, color identity, and deck-construction text require review.",
                entry.display_name
            ));
        }
    }

    let mut commander_eligibility = Vec::new();
    let mut commander_definitions = Vec::new();
    for commander in &commander_names {
        let definition = find_definition(&definition_index, commander);
        commander_definitions.push(definition);
        let eligibility = evaluate_commander_eligibility(policy, commander, definition);
        match eligibility.status {
            LegalityStatus::Illegal => push_format_violation(
                &mut format_violations,
                &mut seen_format_violations,
                "ineligibleCommander",
                Some(commander.clone()),
                eligibility.reason.clone(),
            ),
            LegalityStatus::Unknown => {
                has_unknown = true;
                manual_review_reasons.insert(eligibility.reason.clone());
            }
            LegalityStatus::Legal => {}
        }
        commander_eligibility.push(eligibility);
    }

    if commander_names.len() == 2
        && commander_definitions
            .iter()
            .all(|definition| definition.is_some())
        && !commander_pair_is_supported(
            commander_definitions[0].expect("checked above"),
            commander_definitions[1].expect("checked above"),
        )
    {
        has_unknown = true;
        manual_review_reasons.insert(
            "Two commanders were selected, but their partner-style pairing could not be proven from the available card fields."
                .into(),
        );
    }

    let mut duplicate_violations = Vec::new();
    if policy.format_rules.singleton {
        for entry in aggregated.values().filter(|entry| entry.quantity > 1) {
            match find_definition(&definition_index, &entry.display_name) {
                Some(card) if card_allows_multiple_copies(policy, card) => {}
                Some(_) => duplicate_violations.push(DuplicateViolation {
                    card_name: entry.display_name.clone(),
                    quantity: entry.quantity,
                }),
                None => {
                    has_unknown = true;
                    manual_review_reasons.insert(format!(
                        "{} appears {} times, but its card data is unresolved, so a deck-construction exception cannot be checked.",
                        entry.display_name, entry.quantity
                    ));
                }
            }
        }
    }

    let mut color_identity_violations = Vec::new();
    if policy.format_rules.color_identity_required
        && !commander_names.is_empty()
        && commander_definitions
            .iter()
            .all(|definition| definition.is_some())
    {
        let commander_colors = commander_definitions
            .iter()
            .flatten()
            .flat_map(|card| normalized_colors(&card.color_identity))
            .collect::<BTreeSet<_>>();

        for entry in aggregated.values() {
            let Some(card) = find_definition(&definition_index, &entry.display_name) else {
                continue;
            };
            let card_colors = normalized_colors(&card.color_identity);
            if card_colors
                .iter()
                .any(|color| !commander_colors.contains(color))
            {
                color_identity_violations.push(ColorIdentityViolation {
                    card_name: entry.display_name.clone(),
                    card_colors: card_colors.into_iter().collect(),
                    commander_colors: commander_colors.iter().cloned().collect(),
                });
            }
        }
    } else if policy.format_rules.color_identity_required && !commander_names.is_empty() {
        has_unknown = true;
        manual_review_reasons.insert(
            "At least one commander is unresolved, so the complete commander color identity cannot be established."
                .into(),
        );
    }

    let game_changer_names = official_name_index(&policy.game_changers);
    let mut found_game_changers = BTreeSet::new();
    let mut game_changer_count = 0u16;
    for entry in aggregated.values() {
        let definition = find_definition(&definition_index, &entry.display_name);
        let official_name =
            find_official_name(&game_changer_names, &entry.display_name).or_else(|| {
                definition.and_then(|card| find_official_name(&game_changer_names, &card.name))
            });
        if let Some(official_name) = official_name {
            game_changer_count = game_changer_count.saturating_add(entry.quantity);
            found_game_changers.insert(official_name.to_string());
        }
    }

    let (policy_floor, policy_floor_reason) =
        game_changer_floor(&policy.bracket_policy, game_changer_count);
    let game_changers = found_game_changers.into_iter().collect::<Vec<_>>();
    let bracket_signals = policy_floor
        .map(|floor| {
            vec![BracketPolicySignal {
                code: "gameChangers".into(),
                kind: BracketPolicySignalKind::DeterministicFloor,
                recommended_floor: Some(floor),
                title: format!("Game Changer floor: Bracket {floor}"),
                detail: policy_floor_reason.clone(),
                cards: game_changers.clone(),
                source_urls: policy.bracket_policy.guidance.source_urls.clone(),
            }]
        })
        .unwrap_or_default();
    let intent_assessments = policy
        .bracket_policy
        .intent_only_brackets
        .iter()
        .map(|bracket| IntentBracketAssessment {
            bracket: *bracket,
            status: IntentAssessmentStatus::Unknown,
            inferred: false,
            reason: match bracket {
                1 => "Bracket 1 depends on a deliberately thematic Exhibition intent and pod-approved flexibility.".into(),
                5 => "Bracket 5 depends on cEDH metagame and competitive intent, not card composition alone.".into(),
                _ => "This bracket is configured as intent-only and is not inferred.".into(),
            },
        })
        .collect();

    let definitely_illegal = !format_violations.is_empty()
        || !duplicate_violations.is_empty()
        || !color_identity_violations.is_empty();
    let legality = if definitely_illegal {
        LegalityStatus::Illegal
    } else if has_unknown {
        LegalityStatus::Unknown
    } else {
        LegalityStatus::Legal
    };

    PolicyEvaluation {
        package_version: policy.package_version.clone(),
        effective_date: policy.effective_date.clone(),
        legality,
        deck_card_count,
        format_violations,
        color_identity_violations,
        duplicate_violations,
        commander_eligibility,
        unresolved_cards: unresolved_cards.into_iter().collect(),
        game_changer_count,
        game_changers,
        policy_floor,
        policy_floor_reason,
        bracket_signals,
        intent_assessments,
        manual_review_reasons: manual_review_reasons.into_iter().collect(),
    }
}

/// Adds bracket-content and turn-expectation signals that require compiled
/// Oracle semantics or simulation output. Deterministic restrictions may raise
/// `policy_floor`; intent- or model-dependent observations remain visibly
/// classified as guidance or manual review.
pub fn apply_compiled_bracket_guidance(
    policy: &CommanderPolicyPackage,
    deck: &CompiledDeck,
    win_speed: &WinSpeedReport,
    evaluation: &mut PolicyEvaluation,
) {
    let guidance = &policy.bracket_policy.guidance;
    let sources = guidance.source_urls.clone();

    let mut definite_mass_land_denial = BTreeSet::new();
    let mut uncertain_mass_land_denial = BTreeSet::new();
    for card in deck
        .cards
        .iter()
        .filter(|card| card.effects.mass_land_denial)
    {
        if card.semantic_confidence >= 0.72 {
            definite_mass_land_denial.insert(card.name.clone());
        } else {
            uncertain_mass_land_denial.insert(card.name.clone());
        }
    }
    if let Some(floor) = guidance.mass_land_denial_floor
        && !definite_mass_land_denial.is_empty()
    {
        let cards = definite_mass_land_denial.into_iter().collect::<Vec<_>>();
        let detail = format!(
            "{} match{} the versioned mass-land-denial definition, which is excluded from Brackets 1\u{2013}3.",
            cards.join(" · "),
            if cards.len() == 1 { "es" } else { "" },
        );
        raise_deterministic_floor(evaluation, floor, &detail);
        evaluation.bracket_signals.push(BracketPolicySignal {
            code: "massLandDenial".into(),
            kind: BracketPolicySignalKind::DeterministicFloor,
            recommended_floor: Some(floor),
            title: format!("Mass land denial: Bracket {floor} floor"),
            detail,
            cards,
            source_urls: sources.clone(),
        });
    }
    if !uncertain_mass_land_denial.is_empty() {
        let cards = uncertain_mass_land_denial.into_iter().collect::<Vec<_>>();
        let detail = format!(
            "Possible mass-land-denial text was found on {}, but semantic confidence is too low for an automatic floor.",
            cards.join(" · ")
        );
        push_manual_review(evaluation, detail.clone());
        evaluation.bracket_signals.push(BracketPolicySignal {
            code: "massLandDenialUncertain".into(),
            kind: BracketPolicySignalKind::ManualReview,
            recommended_floor: guidance.mass_land_denial_floor,
            title: "Possible mass land denial".into(),
            detail,
            cards,
            source_urls: sources.clone(),
        });
    }

    let two_card_game_enders = deck
        .known_lines
        .iter()
        .filter(|line| {
            line.compactness == 2 && line.table_lethal_if_resolved && line.model_confidence >= 0.85
        })
        .collect::<Vec<_>>();
    if !two_card_game_enders.is_empty() {
        let line_names = two_card_game_enders
            .iter()
            .map(|line| line.name.clone())
            .collect::<Vec<_>>();
        let cards = two_card_game_enders
            .iter()
            .flat_map(|line| line.cards.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let detail = format!(
            "Documented two-card game-ending line{} detected: {}. Lower-bracket guidance excludes intentional game-ending combos, but a decklist cannot prove intent.",
            if line_names.len() == 1 { "" } else { "s" },
            line_names.join(" · "),
        );
        push_manual_review(evaluation, detail.clone());
        evaluation.bracket_signals.push(BracketPolicySignal {
            code: "twoCardGameEndingLine".into(),
            kind: BracketPolicySignalKind::ModeledGuidance,
            recommended_floor: guidance.two_card_game_ending_floor,
            title: "Two-card game-ending line".into(),
            detail,
            cards,
            source_urls: sources.clone(),
        });

        let modeled_line_present = two_card_game_enders.iter().any(|line| {
            !line.simulation_requirements.iter().any(|requirement| {
                matches!(
                    requirement,
                    LineRequirement::Unmodeled
                        | LineRequirement::TotalExecutionMana
                        | LineRequirement::CombatAccess
                )
            })
        });
        if modeled_line_present
            && let (Some(turn), Some(rate_threshold), Some(floor)) = (
                guidance.early_game_through_turn,
                guidance.frequent_early_win_attempt_rate,
                guidance.early_two_card_game_ending_floor,
            )
        {
            let early_rate =
                cumulative_rate_at_or_before(&win_speed.cumulative_win_attempt_rate, turn);
            if early_rate >= rate_threshold {
                let detail = format!(
                    "The bounded whole-deck model reaches a baseline win attempt by turn {turn} in {:.0}% of trials while a machine-checkable two-card line is present. This is Bracket {floor} guidance, not proof that the line caused every attempt.",
                    early_rate * 100.0,
                );
                push_manual_review(evaluation, detail.clone());
                evaluation.bracket_signals.push(BracketPolicySignal {
                    code: "frequentEarlyGameEndingLine".into(),
                    kind: BracketPolicySignalKind::ModeledGuidance,
                    recommended_floor: Some(floor),
                    title: "Frequent early win-attempt pattern".into(),
                    detail,
                    cards: two_card_game_enders
                        .iter()
                        .flat_map(|line| line.cards.iter().cloned())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                    source_urls: sources.clone(),
                });
            }
        }
    }

    let extra_turn_cards = deck
        .cards
        .iter()
        .filter(|card| card.effects.extra_turns != EffectMagnitude::None)
        .collect::<Vec<_>>();
    if !extra_turn_cards.is_empty() {
        let cards = extra_turn_cards
            .iter()
            .map(|card| card.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let detail = format!(
            "{} extra-turn card{} detected. Bracket 1 excludes extra-turn cards; Brackets 2\u{2013}3 describe low quantities that are not intended to chain or loop.",
            cards.len(),
            if cards.len() == 1 { "" } else { "s" },
        );
        push_manual_review(evaluation, detail.clone());
        evaluation.bracket_signals.push(BracketPolicySignal {
            code: "extraTurnCards".into(),
            kind: BracketPolicySignalKind::ManualReview,
            recommended_floor: guidance.extra_turn_card_floor,
            title: "Extra-turn intent check".into(),
            detail,
            cards: cards.clone(),
            source_urls: sources.clone(),
        });

        let repeatable = extra_turn_cards
            .iter()
            .filter(|card| card.effects.repeatable)
            .map(|card| card.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if !repeatable.is_empty() {
            let detail = format!(
                "Repeatable extra-turn text appears on {}. The model cannot prove a chain or loop, so player intent and actual recursion paths require review.",
                repeatable.join(" · "),
            );
            push_manual_review(evaluation, detail.clone());
            evaluation.bracket_signals.push(BracketPolicySignal {
                code: "repeatableExtraTurns".into(),
                kind: BracketPolicySignalKind::ManualReview,
                recommended_floor: guidance.extra_turn_chain_floor,
                title: "Possible extra-turn chain".into(),
                detail,
                cards: repeatable,
                source_urls: sources.clone(),
            });
        }
    }

    if !guidance.expected_minimum_turns.is_empty() {
        let expectations = guidance
            .expected_minimum_turns
            .iter()
            .map(|expectation| {
                expectation.minimum_turns_played.map_or_else(
                    || format!("B{} any turn", expectation.bracket),
                    |turns| format!("B{} at least {turns}", expectation.bracket),
                )
            })
            .collect::<Vec<_>>()
            .join(" · ");
        let analyzed_turn_cap = win_speed
            .cumulative_win_attempt_rate
            .last()
            .map(|entry| entry.turn)
            .unwrap_or(6);
        let modeled = win_speed.baseline_win_attempt.median.map_or_else(
            || "No baseline win-attempt median was demonstrated by the turn cap.".into(),
            |median| {
                format!(
                    "The bounded baseline win-attempt median is turn {median:.1}; {:.0}% of trials reach an attempt by turn {analyzed_turn_cap}.",
                    cumulative_rate_at_or_before(
                        &win_speed.cumulative_win_attempt_rate,
                        analyzed_turn_cap,
                    ) * 100.0
                )
            },
        );
        evaluation.bracket_signals.push(BracketPolicySignal {
            code: "turnExpectations".into(),
            kind: BracketPolicySignalKind::ModeledGuidance,
            recommended_floor: None,
            title: "Official turn-expectation context".into(),
            detail: format!(
                "Published expectations: {expectations}. {modeled} These are descriptive expectations, not automatic legality checks."
            ),
            cards: Vec::new(),
            source_urls: sources,
        });
    }
}

fn cumulative_rate_at_or_before(rates: &[crate::domain::TurnRate], turn: u8) -> f32 {
    rates
        .iter()
        .filter(|entry| entry.turn <= turn)
        .max_by_key(|entry| entry.turn)
        .map(|entry| entry.rate)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0)
}

fn raise_deterministic_floor(evaluation: &mut PolicyEvaluation, floor: u8, reason: &str) {
    let previous = evaluation.policy_floor;
    evaluation.policy_floor = Some(previous.map_or(floor, |current| current.max(floor)));
    evaluation.policy_floor_reason = match previous {
        None => reason.into(),
        Some(_) if evaluation.policy_floor_reason.trim().is_empty() => reason.into(),
        Some(_) => format!("{} {reason}", evaluation.policy_floor_reason),
    };
}

fn push_manual_review(evaluation: &mut PolicyEvaluation, reason: String) {
    if !evaluation
        .manual_review_reasons
        .iter()
        .any(|existing| existing == &reason)
    {
        evaluation.manual_review_reasons.push(reason);
        evaluation.manual_review_reasons.sort();
    }
}

fn ensure_unique_nonempty(label: &str, names: &[String]) -> Result<(), PolicyPackageError> {
    if names.is_empty() {
        return Err(PolicyPackageError::Invalid(format!(
            "{label} must not be empty"
        )));
    }
    let normalized = names
        .iter()
        .map(|name| normalize_card_name(name))
        .collect::<BTreeSet<_>>();
    if normalized.len() != names.len() || normalized.contains("") {
        return Err(PolicyPackageError::Invalid(format!(
            "{label} contain duplicate or empty card names"
        )));
    }
    Ok(())
}

fn aggregate_entries(entries: &[DeckEntry]) -> BTreeMap<String, AggregatedEntry> {
    let mut aggregated = BTreeMap::<String, AggregatedEntry>::new();
    for entry in entries {
        let key = normalize_card_name(&entry.name);
        if key.is_empty() {
            continue;
        }
        aggregated
            .entry(key)
            .and_modify(|existing| {
                existing.quantity = existing.quantity.saturating_add(entry.quantity);
            })
            .or_insert_with(|| AggregatedEntry {
                display_name: entry.name.trim().to_string(),
                quantity: entry.quantity,
            });
    }
    aggregated
}

fn effective_commanders(entries: &[DeckEntry], selected_commanders: &[String]) -> Vec<String> {
    let source = if selected_commanders
        .iter()
        .any(|name| !name.trim().is_empty())
    {
        selected_commanders
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    } else {
        entries
            .iter()
            .filter(|entry| entry.is_commander)
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>()
    };

    source
        .into_iter()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

fn build_definition_index(definitions: &[CardDefinition]) -> HashMap<String, &CardDefinition> {
    let mut index = HashMap::new();
    for definition in definitions {
        if !definition.normalized_name.is_empty() {
            index.insert(definition.normalized_name.clone(), definition);
        }
        for key in card_name_keys(&definition.name) {
            index.entry(key).or_insert(definition);
        }
    }
    index
}

fn find_definition<'a>(
    index: &'a HashMap<String, &'a CardDefinition>,
    name: &str,
) -> Option<&'a CardDefinition> {
    card_name_keys(name)
        .into_iter()
        .find_map(|key| index.get(&key).copied())
}

fn card_name_keys(name: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let full = normalize_card_name(name);
    if !full.is_empty() {
        keys.push(full);
    }
    for face in name.split("//") {
        let key = normalize_card_name(face);
        if !key.is_empty() && !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

fn names_overlap(left: &str, right: &str) -> bool {
    let left = card_name_keys(left).into_iter().collect::<BTreeSet<_>>();
    card_name_keys(right)
        .into_iter()
        .any(|key| left.contains(&key))
}

fn official_name_index(names: &[String]) -> HashMap<String, &str> {
    let mut index = HashMap::new();
    for name in names {
        for key in card_name_keys(name) {
            index.insert(key, name.as_str());
        }
    }
    index
}

fn find_official_name<'a>(index: &'a HashMap<String, &'a str>, candidate: &str) -> Option<&'a str> {
    card_name_keys(candidate)
        .into_iter()
        .find_map(|key| index.get(&key).copied())
}

fn push_format_violation(
    violations: &mut Vec<PolicyViolation>,
    seen: &mut BTreeSet<(String, String)>,
    code: &str,
    card_name: Option<String>,
    message: String,
) {
    let key = (
        code.to_string(),
        card_name
            .as_deref()
            .map(normalize_card_name)
            .unwrap_or_default(),
    );
    if seen.insert(key) {
        violations.push(PolicyViolation {
            code: code.into(),
            card_name,
            message,
        });
    }
}

fn category_matches(category: &CategoricalBan, card: &CardDefinition) -> bool {
    match &category.machine_check {
        CategoryMachineCheck::TypeLineContains { value } => card
            .type_line
            .to_lowercase()
            .contains(&value.to_lowercase()),
        CategoryMachineCheck::OracleTextContains { value } => {
            contains_word_case_insensitive(&card.oracle_text, value)
        }
        CategoryMachineCheck::NameList { names } => {
            let names = official_name_index(names);
            find_official_name(&names, &card.name).is_some()
        }
    }
}

fn contains_word_case_insensitive(text: &str, word: &str) -> bool {
    let expected = word.to_lowercase();
    text.split(|character: char| !character.is_alphanumeric())
        .any(|candidate| candidate.to_lowercase() == expected)
}

fn local_commander_legality(card: &CardDefinition) -> LegalityStatus {
    match card
        .commander_legality
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("legal" | "restricted") => LegalityStatus::Legal,
        Some("not_legal" | "banned") => LegalityStatus::Illegal,
        Some(_) => LegalityStatus::Unknown,
        // Before schema 5, the database retained only a collapsed boolean.
        // A true value is sufficient positive evidence. A false value is not
        // sufficient negative evidence because same-named tokens and missing
        // legality fields were both collapsed to false.
        None if card.legal_commander => LegalityStatus::Legal,
        None => LegalityStatus::Unknown,
    }
}

fn evaluate_commander_eligibility(
    policy: &CommanderPolicyPackage,
    commander_name: &str,
    definition: Option<&CardDefinition>,
) -> CommanderEligibility {
    let Some(card) = definition else {
        return CommanderEligibility {
            card_name: commander_name.into(),
            status: LegalityStatus::Unknown,
            reason: format!(
                "{commander_name} is unresolved, so commander eligibility cannot be checked."
            ),
        };
    };

    match local_commander_legality(card) {
        LegalityStatus::Illegal => {
            return CommanderEligibility {
                card_name: commander_name.into(),
                status: LegalityStatus::Illegal,
                reason: format!("{commander_name} is not legal in the Commander format."),
            };
        }
        LegalityStatus::Unknown => {
            return CommanderEligibility {
                card_name: commander_name.into(),
                status: LegalityStatus::Unknown,
                reason: format!(
                    "{commander_name} does not have an authoritative Commander-legality value in the local card data."
                ),
            };
        }
        LegalityStatus::Legal => {}
    }

    let type_line = card.type_line.to_lowercase();
    let oracle_text = card.oracle_text.to_lowercase();
    if policy.format_rules.legendary_creatures_eligible && type_line.contains("legendary creature")
    {
        return CommanderEligibility {
            card_name: commander_name.into(),
            status: LegalityStatus::Legal,
            reason: "Legendary creatures are eligible commanders.".into(),
        };
    }
    if policy.format_rules.explicit_oracle_permission_eligible
        && oracle_text.contains("can be your commander")
    {
        return CommanderEligibility {
            card_name: commander_name.into(),
            status: LegalityStatus::Legal,
            reason: "The card's Oracle text explicitly permits it to be a commander.".into(),
        };
    }
    if policy
        .format_rules
        .legendary_vehicle_or_spacecraft_needs_printed_power_toughness
        && type_line.contains("legendary")
        && (type_line.contains("vehicle") || type_line.contains("spacecraft"))
    {
        return CommanderEligibility {
            card_name: commander_name.into(),
            status: LegalityStatus::Unknown,
            reason: format!(
                "{commander_name} is a legendary Vehicle or Spacecraft; printed power/toughness is required, but that field is not available in the current local card model."
            ),
        };
    }

    CommanderEligibility {
        card_name: commander_name.into(),
        status: LegalityStatus::Illegal,
        reason: format!(
            "{commander_name} is neither a legendary creature nor a card with explicit commander permission."
        ),
    }
}

fn commander_pair_is_supported(first: &CardDefinition, second: &CardDefinition) -> bool {
    let first_text = first.oracle_text.to_lowercase();
    let second_text = second.oracle_text.to_lowercase();
    let first_type = first.type_line.to_lowercase();
    let second_type = second.type_line.to_lowercase();

    let has_keyword = |card: &CardDefinition, keyword: &str| {
        card.keywords
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(keyword))
    };
    let has_standalone_partner = |card: &CardDefinition| {
        has_keyword(card, "Partner")
            || card
                .oracle_text
                .lines()
                .any(|line| line.trim().eq_ignore_ascii_case("partner"))
    };

    (has_standalone_partner(first) && has_standalone_partner(second))
        || (has_keyword(first, "Friends forever") && has_keyword(second, "Friends forever"))
        || (first_text.contains("friends forever") && second_text.contains("friends forever"))
        || (first_text.contains("choose a background") && second_type.contains("background"))
        || (second_text.contains("choose a background") && first_type.contains("background"))
        || (first_text.contains("doctor's companion") && second_type.contains("doctor"))
        || (second_text.contains("doctor's companion") && first_type.contains("doctor"))
        || first_text.contains(&format!("partner with {}", second.name.to_lowercase()))
        || second_text.contains(&format!("partner with {}", first.name.to_lowercase()))
}

fn card_allows_multiple_copies(policy: &CommanderPolicyPackage, card: &CardDefinition) -> bool {
    if policy.format_rules.basic_lands_exempt
        && card.type_line.to_lowercase().contains("basic land")
    {
        return true;
    }
    let oracle_text = card.oracle_text.to_lowercase();
    oracle_text.contains("a deck can have any number of cards named")
        || (oracle_text.contains("a deck can have up to") && oracle_text.contains("cards named"))
}

fn normalized_colors(colors: &[String]) -> BTreeSet<String> {
    colors
        .iter()
        .map(|color| color.trim().to_ascii_uppercase())
        .filter(|color| matches!(color.as_str(), "W" | "U" | "B" | "R" | "G"))
        .collect()
}

fn game_changer_floor(policy: &BracketPolicy, count: u16) -> (Option<u8>, String) {
    if count == 0 {
        return (
            None,
            "No Game Changer floor applies. This does not establish Bracket 1 or 2.".into(),
        );
    }
    if count > policy.floor_when_more_than {
        return (
            Some(policy.floor_above_limit),
            format!(
                "{count} Game Changers exceed the Bracket 3 limit of {}, imposing a Bracket {} floor.",
                policy.floor_when_more_than, policy.floor_above_limit
            ),
        );
    }
    (
        Some(policy.floor_when_at_least_one),
        format!(
            "{count} Game Changer{} impose{} a Bracket {} floor.",
            if count == 1 { "" } else { "s" },
            if count == 1 { "s" } else { "" },
            policy.floor_when_at_least_one
        ),
    )
}
