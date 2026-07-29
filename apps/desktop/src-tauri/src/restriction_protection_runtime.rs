//! Exact restriction and protection programs for the reviewed Alela families.
//!
//! Card names are not classifier inputs. Every program is selected from the
//! complete normalized Oracle root plus an exact source type requirement.

use crate::ability_program::{
    AbilityCompilation, EXECUTABLE_ABILITY_PROGRAM_VERSION, ExecutableAbilityProgramV1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RestrictionProtectionCardInput<'a> {
    pub type_line: &'a str,
    pub oracle_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceZone {
    Battlefield,
    Stack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceCardType {
    Artifact,
    Creature,
    Enchantment,
    Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceSubtype {
    Aura,
    Equipment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceRequirement {
    pub zone: SourceZone,
    pub card_type: SourceCardType,
    pub subtype: Option<SourceSubtype>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControllerScope {
    You,
    Opponents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectDuration {
    WhileSourceRemainsOnBattlefield,
    DuringYourTurn,
    UntilEndOfTurn,
    WhileAuraRemainsAttached,
    UntilYourNextTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Keyword {
    Hexproof,
    Indestructible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PermanentKind {
    Artifact,
    Creature,
    Enchantment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OracleOwnership {
    CompleteRoot { clause_count: u16 },
    ExactClauseSet { clause_indices: Vec<u16> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttackTaxSubject {
    EachCreatureAttackingYou,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttackTaxPayer {
    AttackingCreatureController,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttackTaxProgram {
    pub source: SourceRequirement,
    pub protected_player: ControllerScope,
    pub subject: AttackTaxSubject,
    pub payer: AttackTaxPayer,
    pub generic_mana_per_attacker: u16,
    pub duration: EffectDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProhibitedOpponentAction {
    CastSpells,
    ActivateAbilitiesOf(PermanentKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpponentTurnRestrictionProgram {
    pub source: SourceRequirement,
    pub active_during: EffectDuration,
    pub restricted_players: ControllerScope,
    pub prohibited_actions: Vec<ProhibitedOpponentAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttackTriggerSubject {
    OneOrMoreCreaturesYouControlAttack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GrantedSubject {
    ThoseAttackingCreatures,
    OtherFlyingCreaturesYouControl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeywordGrantProgram {
    pub source: SourceRequirement,
    pub trigger: Option<AttackTriggerSubject>,
    pub subject: GrantedSubject,
    pub keyword: Keyword,
    pub duration: EffectDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuraTarget {
    Creature,
    CreatureOrPlaneswalker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuraContinuousEffect {
    LoseAllAbilities,
    SetBasePowerAndToughness {
        power: i16,
        toughness: i16,
    },
    ModifyPowerAndToughness {
        power_delta: i16,
        toughness_delta: i16,
    },
    CannotAttack,
    CannotBlock,
    ActivatedAbilitiesCannotBeActivated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuraRestrictionProgram {
    pub source: SourceRequirement,
    pub legal_target: AuraTarget,
    pub effects: Vec<AuraContinuousEffect>,
    pub duration: EffectDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActivatedAbilityCost {
    pub generic_mana: u16,
    pub tap_source: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeywordRemovalSubject {
    PermanentsYourOpponentsControl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeywordRemovalProgram {
    pub source: SourceRequirement,
    pub cost: ActivatedAbilityCost,
    pub subject: KeywordRemovalSubject,
    pub removed_keywords: Vec<Keyword>,
    pub duration: EffectDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProtectionScope {
    Everything,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PhaseReturnTiming {
    BeforeYourNextUntap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpellDestination {
    Exile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompleteProtectionStep {
    LockLifeTotal {
        player: ControllerScope,
        duration: EffectDuration,
    },
    GrantProtection {
        player: ControllerScope,
        from: ProtectionScope,
        duration: EffectDuration,
    },
    PhaseOutControlledPermanents {
        controller: ControllerScope,
        return_timing: PhaseReturnTiming,
    },
    MoveSourceSpell {
        from: SourceZone,
        destination: SpellDestination,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompleteProtectionProgram {
    pub source: SourceRequirement,
    pub ordered_steps: Vec<CompleteProtectionStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RestrictionProtectionProgram {
    AttackTax(AttackTaxProgram),
    OpponentTurnRestriction(OpponentTurnRestrictionProgram),
    KeywordGrant(KeywordGrantProgram),
    AuraRestriction(AuraRestrictionProgram),
    KeywordRemoval(KeywordRemovalProgram),
    CompleteProtection(CompleteProtectionProgram),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledRestrictionProtection {
    pub ownership: OracleOwnership,
    pub program: RestrictionProtectionProgram,
}

pub(crate) fn compile_restriction_protection_runtime(
    input: RestrictionProtectionCardInput<'_>,
) -> Option<CompiledRestrictionProtection> {
    let clauses = normalize_oracle_root(input.oracle_text)?;

    compile_ghostly_prison(input.type_line, &clauses)
        .or_else(|| compile_grand_abolisher(input.type_line, &clauses))
        .or_else(|| compile_angelic_guardian(input.type_line, &clauses))
        .or_else(|| compile_sephara(input.type_line, &clauses))
        .or_else(|| compile_kasmina_transmutation(input.type_line, &clauses))
        .or_else(|| compile_negative_aura(input.type_line, &clauses))
        .or_else(|| compile_nahiri_binding(input.type_line, &clauses))
        .or_else(|| compile_shadowspear(input.type_line, &clauses))
        .or_else(|| compile_complete_turn_protection(input.type_line, &clauses))
}

pub(crate) fn compile_restriction_protection_from_program(
    type_line: &str,
    program: &ExecutableAbilityProgramV1,
) -> Option<CompiledRestrictionProtection> {
    if program.version != EXECUTABLE_ABILITY_PROGRAM_VERSION
        || !program.face_programs.is_empty()
        || program.necropotence_lifecycle.is_some()
        || program.self_transfer_tutor_permanent.is_some()
        || program.entry_linked_permanent.is_some()
        || program.atomic_transaction.is_some()
        || program.graveyard_reclamation.is_some()
    {
        return None;
    }
    let mut clauses = program
        .abilities
        .iter()
        .map(|compilation| match compilation {
            AbilityCompilation::Executable(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
            AbilityCompilation::Unsupported(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
        })
        .collect::<Vec<_>>();
    clauses.sort_by_key(|(clause_index, _)| *clause_index);
    if clauses.is_empty()
        || clauses
            .iter()
            .enumerate()
            .any(|(expected, (actual, _))| expected != *actual)
    {
        return None;
    }
    let normalized_oracle = clauses
        .iter()
        .map(|(_, clause)| *clause)
        .collect::<Vec<_>>()
        .join("\n");
    compile_restriction_protection_runtime(RestrictionProtectionCardInput {
        type_line,
        oracle_text: &normalized_oracle,
    })
}

fn compile_ghostly_prison(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledRestrictionProtection> {
    if !source_type_matches(type_line, SourceCardType::Enchantment, None)
        || clauses
            != [
                "creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you",
            ]
    {
        return None;
    }

    Some(CompiledRestrictionProtection {
        ownership: complete_root(clauses),
        program: RestrictionProtectionProgram::AttackTax(AttackTaxProgram {
            source: battlefield_source(SourceCardType::Enchantment, None),
            protected_player: ControllerScope::You,
            subject: AttackTaxSubject::EachCreatureAttackingYou,
            payer: AttackTaxPayer::AttackingCreatureController,
            generic_mana_per_attacker: 2,
            duration: EffectDuration::WhileSourceRemainsOnBattlefield,
        }),
    })
}

fn compile_grand_abolisher(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledRestrictionProtection> {
    if !source_type_matches(type_line, SourceCardType::Creature, None)
        || clauses
            != [
                "during your turn, your opponents can't cast spells or activate abilities of artifacts, creatures, or enchantments",
            ]
    {
        return None;
    }

    Some(CompiledRestrictionProtection {
        ownership: complete_root(clauses),
        program: RestrictionProtectionProgram::OpponentTurnRestriction(
            OpponentTurnRestrictionProgram {
                source: battlefield_source(SourceCardType::Creature, None),
                active_during: EffectDuration::DuringYourTurn,
                restricted_players: ControllerScope::Opponents,
                prohibited_actions: vec![
                    ProhibitedOpponentAction::CastSpells,
                    ProhibitedOpponentAction::ActivateAbilitiesOf(PermanentKind::Artifact),
                    ProhibitedOpponentAction::ActivateAbilitiesOf(PermanentKind::Creature),
                    ProhibitedOpponentAction::ActivateAbilitiesOf(PermanentKind::Enchantment),
                ],
            },
        ),
    })
}

fn compile_angelic_guardian(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledRestrictionProtection> {
    if !source_type_matches(type_line, SourceCardType::Creature, None)
        || clauses
            != [
                "flying",
                "whenever one or more creatures you control attack, they gain indestructible until end of turn",
            ]
    {
        return None;
    }

    Some(CompiledRestrictionProtection {
        ownership: OracleOwnership::ExactClauseSet {
            clause_indices: vec![1],
        },
        program: RestrictionProtectionProgram::KeywordGrant(KeywordGrantProgram {
            source: battlefield_source(SourceCardType::Creature, None),
            trigger: Some(AttackTriggerSubject::OneOrMoreCreaturesYouControlAttack),
            subject: GrantedSubject::ThoseAttackingCreatures,
            keyword: Keyword::Indestructible,
            duration: EffectDuration::UntilEndOfTurn,
        }),
    })
}

fn compile_sephara(type_line: &str, clauses: &[String]) -> Option<CompiledRestrictionProtection> {
    if !source_type_matches(type_line, SourceCardType::Creature, None)
        || clauses
            != [
                "you may pay {w} and tap four untapped creatures you control with flying rather than pay this spell's mana cost",
                "flying, lifelink",
                "other creatures you control with flying have indestructible",
            ]
    {
        return None;
    }

    Some(CompiledRestrictionProtection {
        ownership: OracleOwnership::ExactClauseSet {
            clause_indices: vec![2],
        },
        program: RestrictionProtectionProgram::KeywordGrant(KeywordGrantProgram {
            source: battlefield_source(SourceCardType::Creature, None),
            trigger: None,
            subject: GrantedSubject::OtherFlyingCreaturesYouControl,
            keyword: Keyword::Indestructible,
            duration: EffectDuration::WhileSourceRemainsOnBattlefield,
        }),
    })
}

fn compile_kasmina_transmutation(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledRestrictionProtection> {
    if !source_type_matches(
        type_line,
        SourceCardType::Enchantment,
        Some(SourceSubtype::Aura),
    ) || clauses
        != [
            "enchant creature",
            "enchanted creature loses all abilities and has base power and toughness 1/1",
        ]
    {
        return None;
    }

    Some(CompiledRestrictionProtection {
        ownership: complete_root(clauses),
        program: RestrictionProtectionProgram::AuraRestriction(AuraRestrictionProgram {
            source: battlefield_source(SourceCardType::Enchantment, Some(SourceSubtype::Aura)),
            legal_target: AuraTarget::Creature,
            effects: vec![
                AuraContinuousEffect::LoseAllAbilities,
                AuraContinuousEffect::SetBasePowerAndToughness {
                    power: 1,
                    toughness: 1,
                },
            ],
            duration: EffectDuration::WhileAuraRemainsAttached,
        }),
    })
}

fn compile_nahiri_binding(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledRestrictionProtection> {
    if !source_type_matches(
        type_line,
        SourceCardType::Enchantment,
        Some(SourceSubtype::Aura),
    ) || clauses
        != [
            "enchant creature or planeswalker",
            "enchanted permanent can't attack or block, and its activated abilities can't be activated",
        ]
    {
        return None;
    }

    Some(CompiledRestrictionProtection {
        ownership: complete_root(clauses),
        program: RestrictionProtectionProgram::AuraRestriction(AuraRestrictionProgram {
            source: battlefield_source(SourceCardType::Enchantment, Some(SourceSubtype::Aura)),
            legal_target: AuraTarget::CreatureOrPlaneswalker,
            effects: vec![
                AuraContinuousEffect::CannotAttack,
                AuraContinuousEffect::CannotBlock,
                AuraContinuousEffect::ActivatedAbilitiesCannotBeActivated,
            ],
            duration: EffectDuration::WhileAuraRemainsAttached,
        }),
    })
}

fn compile_negative_aura(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledRestrictionProtection> {
    if !source_type_matches(
        type_line,
        SourceCardType::Enchantment,
        Some(SourceSubtype::Aura),
    ) || clauses != ["enchant creature", "enchanted creature gets -3/-3"]
    {
        return None;
    }

    Some(CompiledRestrictionProtection {
        ownership: complete_root(clauses),
        program: RestrictionProtectionProgram::AuraRestriction(AuraRestrictionProgram {
            source: battlefield_source(SourceCardType::Enchantment, Some(SourceSubtype::Aura)),
            legal_target: AuraTarget::Creature,
            effects: vec![AuraContinuousEffect::ModifyPowerAndToughness {
                power_delta: -3,
                toughness_delta: -3,
            }],
            duration: EffectDuration::WhileAuraRemainsAttached,
        }),
    })
}

fn compile_shadowspear(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledRestrictionProtection> {
    if !source_type_matches(
        type_line,
        SourceCardType::Artifact,
        Some(SourceSubtype::Equipment),
    ) || clauses
        != [
            "equipped creature gets +1/+1 and has trample and lifelink",
            "{1}: permanents your opponents control lose hexproof and indestructible until end of turn",
            "equip {2}",
        ]
    {
        return None;
    }

    Some(CompiledRestrictionProtection {
        ownership: OracleOwnership::ExactClauseSet {
            clause_indices: vec![1],
        },
        program: RestrictionProtectionProgram::KeywordRemoval(KeywordRemovalProgram {
            source: battlefield_source(SourceCardType::Artifact, Some(SourceSubtype::Equipment)),
            cost: ActivatedAbilityCost {
                generic_mana: 1,
                tap_source: false,
            },
            subject: KeywordRemovalSubject::PermanentsYourOpponentsControl,
            removed_keywords: vec![Keyword::Hexproof, Keyword::Indestructible],
            duration: EffectDuration::UntilEndOfTurn,
        }),
    })
}

fn compile_complete_turn_protection(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledRestrictionProtection> {
    if !source_type_matches(type_line, SourceCardType::Instant, None)
        || clauses.len() != 2
        || clauses[0]
            != "until your next turn, your life total can't change and you gain protection from everything. all permanents you control phase out"
        || !is_named_source_exile_clause(&clauses[1])
    {
        return None;
    }

    Some(CompiledRestrictionProtection {
        ownership: complete_root(clauses),
        program: RestrictionProtectionProgram::CompleteProtection(CompleteProtectionProgram {
            source: SourceRequirement {
                zone: SourceZone::Stack,
                card_type: SourceCardType::Instant,
                subtype: None,
            },
            ordered_steps: vec![
                CompleteProtectionStep::LockLifeTotal {
                    player: ControllerScope::You,
                    duration: EffectDuration::UntilYourNextTurn,
                },
                CompleteProtectionStep::GrantProtection {
                    player: ControllerScope::You,
                    from: ProtectionScope::Everything,
                    duration: EffectDuration::UntilYourNextTurn,
                },
                CompleteProtectionStep::PhaseOutControlledPermanents {
                    controller: ControllerScope::You,
                    return_timing: PhaseReturnTiming::BeforeYourNextUntap,
                },
                CompleteProtectionStep::MoveSourceSpell {
                    from: SourceZone::Stack,
                    destination: SpellDestination::Exile,
                },
            ],
        }),
    })
}

fn complete_root(clauses: &[String]) -> OracleOwnership {
    OracleOwnership::CompleteRoot {
        clause_count: u16::try_from(clauses.len()).expect("reviewed roots fit in u16"),
    }
}

fn battlefield_source(
    card_type: SourceCardType,
    subtype: Option<SourceSubtype>,
) -> SourceRequirement {
    SourceRequirement {
        zone: SourceZone::Battlefield,
        card_type,
        subtype,
    }
}

fn source_type_matches(
    type_line: &str,
    card_type: SourceCardType,
    subtype: Option<SourceSubtype>,
) -> bool {
    let required_type = match card_type {
        SourceCardType::Artifact => "artifact",
        SourceCardType::Creature => "creature",
        SourceCardType::Enchantment => "enchantment",
        SourceCardType::Instant => "instant",
    };
    if !has_type_line_word(type_line, required_type) {
        return false;
    }
    let required_subtype = match subtype {
        Some(SourceSubtype::Aura) => Some("aura"),
        Some(SourceSubtype::Equipment) => Some("equipment"),
        None => None,
    };
    required_subtype.is_none_or(|word| has_type_line_word(type_line, word))
}

fn has_type_line_word(type_line: &str, expected: &str) -> bool {
    type_line
        .split(|character: char| !character.is_alphabetic())
        .any(|word| word.eq_ignore_ascii_case(expected))
}

fn is_named_source_exile_clause(clause: &str) -> bool {
    if matches!(clause, "exile this spell" | "exile this permanent") {
        return true;
    }
    let Some(reference) = clause.strip_prefix("exile ") else {
        return false;
    };
    let words = reference.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() || words.len() > 8 {
        return false;
    }
    const NON_NAME_STARTS: &[&str] = &[
        "a",
        "all",
        "an",
        "any",
        "each",
        "it",
        "permanent",
        "permanents",
        "spell",
        "target",
        "that",
        "the",
        "them",
        "this",
        "those",
        "you",
        "your",
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

fn normalize_oracle_root(oracle_text: &str) -> Option<Vec<String>> {
    let normalized_text = oracle_text.trim().replace('’', "'");
    let mut clauses = Vec::new();
    for raw_clause in normalized_text.lines() {
        let collapsed = raw_clause
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        if collapsed.is_empty() {
            continue;
        }
        let without_reminders = remove_reviewed_reminder_text(&collapsed);
        let clause = without_reminders
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let clause = clause.strip_suffix('.').unwrap_or(&clause).trim();
        if clause.is_empty() {
            return None;
        }
        clauses.push(clause.to_string());
    }
    (!clauses.is_empty()).then_some(clauses)
}

fn remove_reviewed_reminder_text(value: &str) -> String {
    const REMINDERS: &[&str] = &[
        " (this creature can't be blocked except by creatures with flying or reach.)",
        " (this permanent can't be blocked except by creatures with flying or reach.)",
        " (damage and effects that say \"destroy\" don't destroy them.)",
        " (while they're phased out, they're treated as though they don't exist. they phase in before you untap during your untap step.)",
    ];
    REMINDERS.iter().fold(value.to_string(), |text, reminder| {
        text.replace(reminder, "")
    })
}
