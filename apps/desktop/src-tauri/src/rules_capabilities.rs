//! Reviewed Comprehensive Rules capabilities used by the semantic compiler.
//!
//! This module is deliberately a closed registry. Finding a word in Oracle
//! text is not enough to make a mechanic executable: the active Comprehensive
//! Rules snapshot must also contain the reviewed rule heading. Even then, this
//! first capability layer contributes strategic role metadata only. Every
//! mechanic below retains an explicit report-only clause until its costs,
//! choices, zones, events, targets, and state changes have a typed executor.

use crate::comprehensive_rules::ComprehensiveRulesSnapshot;
use crate::semantics::role;

pub(crate) const RULE_CAPABILITY_MODEL_VERSION: &str = "cr-capabilities-0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RulesMechanicKind {
    KeywordAction,
    KeywordAbility,
    CounterMechanic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OracleMatchKind {
    WholeTerm,
    WordSuffix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RulesMechanicCapability {
    pub name: &'static str,
    pub kind: RulesMechanicKind,
    pub rule_id: &'static str,
    pub rule_heading: &'static str,
    pub strategic_role_mask: u32,
    oracle_markers: &'static [&'static str],
    match_kind: OracleMatchKind,
    pub report_only_reason: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecognizedRulesMechanic {
    pub name: &'static str,
    pub rule_id: &'static str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RulesCapabilityApplication {
    pub added_role_bits: u32,
    pub recognized_mechanics: Vec<RecognizedRulesMechanic>,
    pub unsupported_clauses: Vec<String>,
}

macro_rules! capability {
    (
        $name:literal,
        $kind:ident,
        $rule_id:literal,
        $heading:literal,
        $roles:expr,
        [$($marker:literal),+ $(,)?],
        $reason:literal
    ) => {
        RulesMechanicCapability {
            name: $name,
            kind: RulesMechanicKind::$kind,
            rule_id: $rule_id,
            rule_heading: $heading,
            strategic_role_mask: $roles,
            oracle_markers: &[$($marker),+],
            match_kind: OracleMatchKind::WholeTerm,
            report_only_reason: $reason,
        }
    };
    (
        suffix $name:literal,
        $kind:ident,
        $rule_id:literal,
        $heading:literal,
        $roles:expr,
        [$($marker:literal),+ $(,)?],
        $reason:literal
    ) => {
        RulesMechanicCapability {
            name: $name,
            kind: RulesMechanicKind::$kind,
            rule_id: $rule_id,
            rule_heading: $heading,
            strategic_role_mask: $roles,
            oracle_markers: &[$($marker),+],
            match_kind: OracleMatchKind::WordSuffix,
            report_only_reason: $reason,
        }
    };
}

/// Closed, reviewed mappings for the June 19, 2026 Comprehensive Rules.
///
/// Role masks are intentionally strategic metadata, not executable effects.
/// In particular, a payoff role does not establish a win and an enabler role
/// does not establish that a cost, target, or prerequisite can be satisfied.
pub(crate) const REVIEWED_RULE_CAPABILITIES: &[RulesMechanicCapability] = &[
    capability!(
        "Investigate",
        KeywordAction,
        "701.16",
        "Investigate",
        role::DRAW | role::ENABLER | role::TOKEN,
        ["investigate", "investigates", "investigated"],
        "Clue creation, activation payment, sacrifice, and the resulting draw are not fully executed."
    ),
    capability!(
        "Mill",
        KeywordAction,
        "701.17",
        "Mill",
        role::GRAVEYARD | role::ENABLER,
        ["mill", "mills", "milled"],
        "Library-to-graveyard object movement, affected players, and library depletion are not executed."
    ),
    capability!(
        "Surveil",
        KeywordAction,
        "701.25",
        "Surveil",
        role::GRAVEYARD | role::ENABLER,
        ["surveil", "surveils", "surveilled"],
        "Library inspection, ordering choices, and chosen graveyard movement are not executed."
    ),
    capability!(
        "Proliferate",
        CounterMechanic,
        "701.34",
        "Proliferate",
        role::ENABLER | role::PAYOFF,
        ["proliferate", "proliferates", "proliferated"],
        "Permanent/player selection and typed counter state are not executed."
    ),
    capability!(
        "Populate",
        KeywordAction,
        "701.36",
        "Populate",
        role::TOKEN | role::TOKEN_MATTERS | role::ENABLER,
        ["populate", "populates", "populated"],
        "Token selection and copying of the chosen token's characteristics are not executed."
    ),
    capability!(
        "Explore",
        CounterMechanic,
        "701.44",
        "Explore",
        role::CREATURE_MATTERS | role::GRAVEYARD | role::ENABLER,
        ["explore", "explores", "explored"],
        "Reveal, land-to-hand or graveyard choice, and +1/+1 counter placement are not executed."
    ),
    capability!(
        "Amass",
        CounterMechanic,
        "701.47",
        "Amass",
        role::TOKEN | role::TOKEN_MATTERS | role::CREATURE_MATTERS | role::ENABLER,
        ["amass", "amasses", "amassed"],
        "Army selection or creation, subtype changes, and +1/+1 counter placement are not executed."
    ),
    capability!(
        "Connive",
        CounterMechanic,
        "701.50",
        "Connive",
        role::DRAW | role::GRAVEYARD | role::CREATURE_MATTERS | role::ENABLER,
        ["connive", "connives", "connived"],
        "Draw/discard choices, discarded-card typing, and +1/+1 counter placement are not executed."
    ),
    capability!(
        "Discover",
        KeywordAction,
        "701.57",
        "Discover",
        role::SPELL_MATTERS | role::DRAW | role::ENABLER,
        ["discover", "discovers", "discovered"],
        "Exile traversal, mana-value filtering, free casting, and library ordering are not executed."
    ),
    capability!(
        suffix "Cycling",
        KeywordAbility,
        "702.29",
        "Cycling",
        role::DRAW | role::ENABLER,
        ["cycling"],
        "Cycling activation costs, discard movement, timing, and the resulting draw are not executed."
    ),
    capability!(
        "Flashback",
        KeywordAbility,
        "702.34",
        "Flashback",
        role::GRAVEYARD | role::RECURSION | role::SPELL_MATTERS | role::ENABLER,
        ["flashback"],
        "Casting from the graveyard, alternate payment, and the exile replacement are not executed."
    ),
    capability!(
        "Storm",
        KeywordAbility,
        "702.40",
        "Storm",
        role::SPELL_MATTERS | role::PAYOFF,
        ["storm"],
        "Spells-cast history, stack copies, target choices, and copy resolution are not executed."
    ),
    capability!(
        "Affinity",
        KeywordAbility,
        "702.41",
        "Affinity",
        role::ENABLER,
        ["affinity"],
        "The affinity quality, object count, and generic cost reduction are not executed."
    ),
    capability!(
        "Convoke",
        KeywordAbility,
        "702.51",
        "Convoke",
        role::CREATURE_MATTERS | role::ENABLER,
        ["convoke"],
        "Creature selection, tapping, colored payment choices, and staged cost payment are not executed."
    ),
    capability!(
        "Dredge",
        KeywordAbility,
        "702.52",
        "Dredge",
        role::GRAVEYARD | role::RECURSION | role::ENABLER,
        ["dredge"],
        "The draw replacement, library movement, and card return from the graveyard are not executed."
    ),
    capability!(
        "Delve",
        KeywordAbility,
        "702.66",
        "Delve",
        role::GRAVEYARD | role::ENABLER,
        ["delve"],
        "Graveyard-card selection, exile payment, and generic cost reduction are not executed."
    ),
    capability!(
        "Persist",
        CounterMechanic,
        "702.79",
        "Persist",
        role::RECURSION | role::DEATH_MATTERS | role::CREATURE_MATTERS,
        ["persist"],
        "Death triggers, last-known counter state, battlefield return, and -1/-1 counters are not executed."
    ),
    capability!(
        "Retrace",
        KeywordAbility,
        "702.81",
        "Retrace",
        role::GRAVEYARD | role::RECURSION | role::SPELL_MATTERS | role::ENABLER,
        ["retrace"],
        "Casting from the graveyard and discarding a land as an additional cost are not executed."
    ),
    capability!(
        "Devour",
        KeywordAbility,
        "702.82",
        "Devour",
        role::SACRIFICE | role::CREATURE_MATTERS | role::PAYOFF,
        ["devour"],
        "The enters replacement, sacrifice choices, and resulting +1/+1 counters are not executed."
    ),
    capability!(
        "Unearth",
        KeywordAbility,
        "702.84",
        "Unearth",
        role::GRAVEYARD | role::RECURSION | role::CREATURE_MATTERS,
        ["unearth"],
        "Graveyard activation, haste, delayed exile, and zone-change replacement are not executed."
    ),
    capability!(
        "Cascade",
        KeywordAbility,
        "702.85",
        "Cascade",
        role::SPELL_MATTERS | role::DRAW | role::ENABLER,
        ["cascade"],
        "Exile traversal, mana-value comparison, free casting, and random library ordering are not executed."
    ),
    capability!(
        "Infect",
        KeywordAbility,
        "702.90",
        "Infect",
        role::CREATURE_MATTERS | role::PAYOFF,
        ["infect"],
        "Combat, damage replacement, poison counters, -1/-1 counters, and player loss are not executed."
    ),
    capability!(
        "Undying",
        CounterMechanic,
        "702.93",
        "Undying",
        role::RECURSION | role::DEATH_MATTERS | role::CREATURE_MATTERS,
        ["undying"],
        "Death triggers, last-known counter state, battlefield return, and +1/+1 counters are not executed."
    ),
    capability!(
        "Prowess",
        KeywordAbility,
        "702.108",
        "Prowess",
        role::SPELL_MATTERS | role::CREATURE_MATTERS | role::PAYOFF,
        ["prowess"],
        "Noncreature-spell cast events and temporary power/toughness changes are not executed."
    ),
    capability!(
        "Exploit",
        KeywordAbility,
        "702.110",
        "Exploit",
        role::SACRIFICE | role::CREATURE_MATTERS | role::ENABLER,
        ["exploit"],
        "The enters trigger, creature choice, sacrifice event, and exploit-linked triggers are not executed."
    ),
    capability!(
        "Myriad",
        KeywordAbility,
        "702.116",
        "Myriad",
        role::TOKEN | role::TOKEN_MATTERS | role::CREATURE_MATTERS | role::ENABLER,
        ["myriad"],
        "Attack triggers, opponent enumeration, tapped attacking token copies, and delayed exile are not executed."
    ),
    capability!(
        "Improvise",
        KeywordAbility,
        "702.126",
        "Improvise",
        role::ARTIFACT_MATTERS | role::ENABLER,
        ["improvise"],
        "Artifact selection, tapping, and staged generic-mana payment are not executed."
    ),
    capability!(
        "Embalm",
        KeywordAbility,
        "702.128",
        "Embalm",
        role::GRAVEYARD | role::RECURSION | role::TOKEN | role::CREATURE_MATTERS,
        ["embalm"],
        "Graveyard activation, exile cost, token-copy characteristics, and sorcery timing are not executed."
    ),
    capability!(
        "Eternalize",
        KeywordAbility,
        "702.129",
        "Eternalize",
        role::GRAVEYARD | role::RECURSION | role::TOKEN | role::CREATURE_MATTERS,
        ["eternalize"],
        "Graveyard activation, exile cost, modified token-copy characteristics, and sorcery timing are not executed."
    ),
    capability!(
        "Jump-start",
        KeywordAbility,
        "702.133",
        "Jump-Start",
        role::GRAVEYARD | role::RECURSION | role::SPELL_MATTERS | role::ENABLER,
        ["jump-start", "jump start"],
        "Casting from the graveyard, discard payment, and the exile replacement are not executed."
    ),
    capability!(
        "Escape",
        KeywordAbility,
        "702.138",
        "Escape",
        role::GRAVEYARD | role::RECURSION | role::ENABLER,
        ["escape"],
        "Casting from the graveyard, alternate costs, graveyard exile choices, and escape modifications are not executed."
    ),
    capability!(
        "Encore",
        KeywordAbility,
        "702.141",
        "Encore",
        role::GRAVEYARD
            | role::RECURSION
            | role::TOKEN
            | role::TOKEN_MATTERS
            | role::CREATURE_MATTERS,
        ["encore"],
        "Graveyard activation, opponent enumeration, token copies, attack requirements, and delayed sacrifice are not executed."
    ),
    capability!(
        "Casualty",
        KeywordAbility,
        "702.153",
        "Casualty",
        role::SACRIFICE | role::SPELL_MATTERS | role::ENABLER,
        ["casualty"],
        "Additional sacrifice costs, power checks, stack copies, targets, and copy resolution are not executed."
    ),
    capability!(
        "Toxic",
        KeywordAbility,
        "702.164",
        "Toxic",
        role::CREATURE_MATTERS | role::PAYOFF,
        ["toxic"],
        "Combat damage to players, poison counters, and state-based player loss are not executed."
    ),
    // Representative +1/+1-counter and counter-transfer mechanics. The
    // current strategic vocabulary has no generic COUNTER_MATTERS bit, so
    // these conservatively use creature/enabler/payoff evidence while typed
    // counter state remains report-only.
    capability!(
        "Monstrosity",
        CounterMechanic,
        "701.37",
        "Monstrosity",
        role::CREATURE_MATTERS | role::PAYOFF,
        ["monstrosity", "monstrous"],
        "Monstrous state, activation payment, and +1/+1 counter placement are not executed."
    ),
    capability!(
        "Bolster",
        CounterMechanic,
        "701.39",
        "Bolster",
        role::CREATURE_MATTERS | role::ENABLER,
        ["bolster", "bolsters", "bolstered"],
        "Least-toughness comparison, creature choice, and +1/+1 counter placement are not executed."
    ),
    capability!(
        "Support",
        CounterMechanic,
        "701.41",
        "Support",
        role::CREATURE_MATTERS | role::ENABLER,
        ["support"],
        "Distinct target selection and +1/+1 counter placement are not executed."
    ),
    capability!(
        "Adapt",
        CounterMechanic,
        "701.46",
        "Adapt",
        role::CREATURE_MATTERS | role::ENABLER | role::PAYOFF,
        ["adapt"],
        "Counter-state preconditions, activation payment, and +1/+1 counter placement are not executed."
    ),
    capability!(
        "Modular",
        CounterMechanic,
        "702.43",
        "Modular",
        role::ARTIFACT_MATTERS | role::CREATURE_MATTERS | role::DEATH_MATTERS | role::ENABLER,
        ["modular"],
        "Enters counters, death triggers, artifact-creature targeting, and counter transfer are not executed."
    ),
    capability!(
        "Graft",
        CounterMechanic,
        "702.58",
        "Graft",
        role::CREATURE_MATTERS | role::ENABLER,
        ["graft"],
        "Enters counters, creature-entry triggers, target choice, and counter transfer are not executed."
    ),
    capability!(
        "Level Up",
        CounterMechanic,
        "702.87",
        "Level Up",
        role::CREATURE_MATTERS | role::PAYOFF,
        ["level up"],
        "Sorcery-speed activation payment, level counters, and level-based characteristics are not executed."
    ),
    capability!(
        "Scavenge",
        CounterMechanic,
        "702.97",
        "Scavenge",
        role::GRAVEYARD | role::CREATURE_MATTERS | role::ENABLER,
        ["scavenge"],
        "Graveyard activation, exile cost, targeting, and power-derived +1/+1 counters are not executed."
    ),
    capability!(
        "Unleash",
        CounterMechanic,
        "702.98",
        "Unleash",
        role::CREATURE_MATTERS | role::PAYOFF,
        ["unleash"],
        "The enters choice, +1/+1 counter placement, and blocking restriction are not executed."
    ),
    capability!(
        "Evolve",
        CounterMechanic,
        "702.100",
        "Evolve",
        role::CREATURE_MATTERS | role::PAYOFF,
        ["evolve"],
        "Creature-entry events, intervening power/toughness checks, and +1/+1 counters are not executed."
    ),
    capability!(
        "Outlast",
        CounterMechanic,
        "702.107",
        "Outlast",
        role::CREATURE_MATTERS | role::ENABLER,
        ["outlast"],
        "Sorcery-speed activation payment, tapping, and +1/+1 counter placement are not executed."
    ),
    capability!(
        "Renown",
        CounterMechanic,
        "702.112",
        "Renown",
        role::CREATURE_MATTERS | role::PAYOFF,
        ["renown", "renowned"],
        "Combat damage, renowned state, and +1/+1 counter placement are not executed."
    ),
    capability!(
        "Fabricate",
        CounterMechanic,
        "702.123",
        "Fabricate",
        role::TOKEN | role::CREATURE_MATTERS | role::ENABLER,
        ["fabricate"],
        "The enters choice between +1/+1 counters and Servo token creation is not executed."
    ),
    capability!(
        "Mentor",
        CounterMechanic,
        "702.134",
        "Mentor",
        role::CREATURE_MATTERS | role::PAYOFF,
        ["mentor"],
        "Attack triggers, lesser-power targeting, resolution checks, and +1/+1 counters are not executed."
    ),
    capability!(
        "Riot",
        CounterMechanic,
        "702.136",
        "Riot",
        role::CREATURE_MATTERS | role::ENABLER,
        ["riot"],
        "The enters choice between a +1/+1 counter and haste is not executed."
    ),
    capability!(
        "Training",
        CounterMechanic,
        "702.149",
        "Training",
        role::CREATURE_MATTERS | role::PAYOFF,
        ["training"],
        "Attack pairing, power comparison, and +1/+1 counter placement are not executed."
    ),
    capability!(
        "Backup",
        CounterMechanic,
        "702.165",
        "Backup",
        role::CREATURE_MATTERS | role::ENABLER,
        ["backup"],
        "The enters trigger, target choice, +1/+1 counters, and temporary ability grant are not executed."
    ),
];

/// Applies reviewed CR-backed strategic metadata to one Oracle text box.
///
/// A mechanic contributes roles only when its heading is verified in the
/// active rules snapshot. Both verified-but-unexecuted mechanics and text that
/// matches a now-unverified rule mapping remain visible as coverage gaps.
pub(crate) fn apply_rules_capabilities(
    oracle_text: &str,
    scryfall_keywords: &[String],
    rules: &ComprehensiveRulesSnapshot,
) -> RulesCapabilityApplication {
    let mut searchable_text = oracle_text.to_string();
    for keyword in scryfall_keywords {
        searchable_text.push('\n');
        searchable_text.push_str(keyword);
    }
    let normalized = normalize_oracle_for_capabilities(&searchable_text);
    let normalized_oracle = normalize_oracle_for_capabilities(oracle_text);
    let mut application = RulesCapabilityApplication::default();

    for capability in matching_capabilities(&normalized) {
        if rules.has_rule_heading(capability.rule_id, capability.rule_heading) {
            let contributes_roles = capability_has_positive_context(capability, &normalized_oracle);
            let contributed_role_mask = if contributes_roles {
                capability.strategic_role_mask
            } else {
                0
            };
            application.added_role_bits |= contributed_role_mask;
            application
                .recognized_mechanics
                .push(RecognizedRulesMechanic {
                    name: capability.name,
                    rule_id: capability.rule_id,
                });
            if contributes_roles {
                application.unsupported_clauses.push(format!(
                    "{} [CR {}] is recognized as strategic evidence, but remains report-only: {}",
                    capability.name, capability.rule_id, capability.report_only_reason
                ));
            } else {
                application.unsupported_clauses.push(format!(
                    "{} [CR {}] is mentioned, but its positive ownership or action was not established; no strategic roles were added and the reference remains report-only.",
                    capability.name, capability.rule_id
                ));
            }
        } else {
            application.unsupported_clauses.push(format!(
                "{} matched Oracle text, but CR {} ({}) was not verified in the active Comprehensive Rules snapshot.",
                capability.name, capability.rule_id, capability.rule_heading
            ));
        }
    }

    for keyword in scryfall_keywords {
        if keyword_has_reviewed_mapping(keyword) {
            continue;
        }
        if let Some((rule_id, heading)) = rules.keyword_rule(keyword) {
            application.unsupported_clauses.push(format!(
                "{heading} [CR {rule_id}] is present in the active rules and on this card, but this app version has no reviewed strategic or executable capability for it."
            ));
        } else {
            application.unsupported_clauses.push(format!(
                "Scryfall keyword “{keyword}” has no reviewed capability and was not verified as a 701/702 heading in the active Comprehensive Rules snapshot."
            ));
        }
    }

    application.unsupported_clauses.sort();
    application.unsupported_clauses.dedup();
    application
}

fn keyword_has_reviewed_mapping(keyword: &str) -> bool {
    let normalized_keyword = normalize_oracle_for_capabilities(keyword);
    REVIEWED_RULE_CAPABILITIES
        .iter()
        .any(|capability| capability_matches(capability, &normalized_keyword))
}

pub(crate) fn capability_compatibility(rules: &ComprehensiveRulesSnapshot) -> (usize, Vec<String>) {
    let mut changed = REVIEWED_RULE_CAPABILITIES
        .iter()
        .filter(|capability| !rules.has_rule_heading(capability.rule_id, capability.rule_heading))
        .map(|capability| capability.rule_id.to_string())
        .collect::<Vec<_>>();
    changed.sort();
    changed.dedup();
    (REVIEWED_RULE_CAPABILITIES.len(), changed)
}

fn matching_capabilities(
    normalized_oracle: &str,
) -> impl Iterator<Item = &'static RulesMechanicCapability> {
    REVIEWED_RULE_CAPABILITIES
        .iter()
        .filter(|capability| capability_matches(capability, normalized_oracle))
}

fn capability_matches(capability: &RulesMechanicCapability, normalized_oracle: &str) -> bool {
    capability
        .oracle_markers
        .iter()
        .any(|marker| match capability.match_kind {
            OracleMatchKind::WholeTerm => contains_ascii_term(normalized_oracle, marker),
            OracleMatchKind::WordSuffix => contains_ascii_word_suffix(normalized_oracle, marker),
        })
}

fn capability_has_positive_context(
    capability: &RulesMechanicCapability,
    normalized_oracle: &str,
) -> bool {
    normalized_oracle
        .split(['\n', ';', '.'])
        .map(str::trim)
        .filter(|clause| !clause.is_empty() && capability_matches(capability, clause))
        .any(|clause| {
            let padded = format!(" {clause} ");
            let is_hard_negation = [
                " can't ",
                " cannot ",
                " don't ",
                " doesn't ",
                " lose ",
                " loses ",
                " lost ",
                " may not ",
                " not have ",
                " prevent ",
                " prevents ",
            ]
            .iter()
            .any(|marker| padded.contains(marker));
            let grants_only_to_opponents = clause.contains("opponent")
                && [
                    " control have ",
                    " controls has ",
                    " control gain ",
                    " controls gains ",
                ]
                .iter()
                .any(|marker| padded.contains(marker));
            if is_hard_negation || grants_only_to_opponents {
                return false;
            }
            if clause_starts_with_capability(capability, clause) {
                return true;
            }
            if padded.contains(" instead ") || padded.contains(" without ") {
                return false;
            }

            let positive_subject = [
                " you ",
                " your ",
                " this ",
                " it ",
                " target creature ",
                " each player ",
                " each creature ",
                " under your control ",
            ]
            .iter()
            .any(|marker| padded.contains(marker));
            positive_subject || capability_follows_instruction_break(capability, clause)
        })
}

fn clause_starts_with_capability(capability: &RulesMechanicCapability, clause: &str) -> bool {
    match capability.match_kind {
        OracleMatchKind::WholeTerm => capability.oracle_markers.iter().any(|marker| {
            clause.starts_with(marker)
                && is_term_boundary(
                    clause.as_bytes().get(marker.len()).copied(),
                    clause.len() == marker.len(),
                )
        }),
        OracleMatchKind::WordSuffix => clause.split_whitespace().take(3).any(|word| {
            word.trim_matches(|character: char| !character.is_ascii_alphanumeric())
                .ends_with(capability.oracle_markers[0])
        }),
    }
}

fn capability_follows_instruction_break(
    capability: &RulesMechanicCapability,
    clause: &str,
) -> bool {
    capability.oracle_markers.iter().any(|marker| {
        [", ", " then "].iter().any(|separator| {
            clause
                .match_indices(separator)
                .map(|(index, _)| index + separator.len())
                .any(|start| {
                    let remainder = &clause[start..];
                    match capability.match_kind {
                        OracleMatchKind::WholeTerm => remainder.starts_with(marker),
                        OracleMatchKind::WordSuffix => remainder
                            .split_whitespace()
                            .next()
                            .is_some_and(|word| word.ends_with(marker)),
                    }
                })
        })
    })
}

fn normalize_oracle_for_capabilities(oracle_text: &str) -> String {
    oracle_text
        .replace('’', "'")
        .replace(['\u{2014}', '\u{2013}', '−', '‑'], "-")
        .to_ascii_lowercase()
}

fn contains_ascii_term(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(start, _)| {
        let end = start + needle.len();
        is_term_boundary(
            haystack.as_bytes().get(start.wrapping_sub(1)).copied(),
            start == 0,
        ) && is_term_boundary(haystack.as_bytes().get(end).copied(), end == haystack.len())
    })
}

fn contains_ascii_word_suffix(haystack: &str, suffix: &str) -> bool {
    haystack
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|word| word.ends_with(suffix))
}

fn is_term_boundary(byte: Option<u8>, at_edge: bool) -> bool {
    at_edge || byte.is_none_or(|value| !value.is_ascii_alphanumeric())
}
