//! Exact reactive interaction programs used by the bounded simulation.
//!
//! This classifier accepts only complete, single-clause templates. Targeted
//! programs retain their full target filter so callers cannot propose them
//! without a legal opposing object.

use crate::ability_program::{
    AbilityCompilation, EXECUTABLE_ABILITY_PROGRAM_VERSION, ExecutableAbilityProgramV1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InteractionCardInput<'a> {
    pub name: &'a str,
    pub type_line: &'a str,
    pub oracle_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InteractionAction {
    Destroy,
    Exile,
    ReturnToOwnersHand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetController {
    Any,
    Opponent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PermanentType {
    Artifact,
    Creature,
    Enchantment,
    Planeswalker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PermanentTargetScope {
    pub any_of_types: Vec<PermanentType>,
    pub nonland: bool,
    pub controller: TargetController,
    pub maximum_mana_value: Option<u16>,
    pub must_have_dealt_damage_to_you_this_turn: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpellType {
    Creature,
    Enchantment,
    Instant,
    Sorcery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpellTargetScope {
    pub any_of_types: Vec<SpellType>,
    pub excluded_type: Option<SpellType>,
    pub maximum_mana_value: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum WipeScope {
    AllPermanents,
    AllNonlandPermanents,
    AllCreatures { maximum_mana_value: Option<u16> },
    AllArtifactsCreaturesAndEnchantments,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InteractionRuntimeProgram {
    TargetedPermanent {
        action: InteractionAction,
        target: PermanentTargetScope,
    },
    Counterspell {
        target: SpellTargetScope,
        unless_controller_pays_generic: Option<u16>,
    },
    DestroyAll {
        scope: WipeScope,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub(crate) struct PermanentTargetCandidate {
    pub controller_is_opponent: bool,
    pub is_artifact: bool,
    pub is_creature: bool,
    pub is_enchantment: bool,
    pub is_land: bool,
    pub is_planeswalker: bool,
    pub mana_value: u16,
    pub dealt_damage_to_you_this_turn: bool,
}

#[allow(dead_code)]
impl PermanentTargetCandidate {
    fn has_type(self, permanent_type: PermanentType) -> bool {
        match permanent_type {
            PermanentType::Artifact => self.is_artifact,
            PermanentType::Creature => self.is_creature,
            PermanentType::Enchantment => self.is_enchantment,
            PermanentType::Planeswalker => self.is_planeswalker,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SpellTargetCandidate {
    pub is_creature: bool,
    pub is_enchantment: bool,
    pub is_instant: bool,
    pub is_sorcery: bool,
    pub mana_value: u16,
}

impl SpellTargetCandidate {
    fn has_type(self, spell_type: SpellType) -> bool {
        match spell_type {
            SpellType::Creature => self.is_creature,
            SpellType::Enchantment => self.is_enchantment,
            SpellType::Instant => self.is_instant,
            SpellType::Sorcery => self.is_sorcery,
        }
    }
}

#[allow(dead_code)]
impl PermanentTargetScope {
    pub(crate) fn matches(&self, candidate: PermanentTargetCandidate) -> bool {
        if self.controller == TargetController::Opponent && !candidate.controller_is_opponent {
            return false;
        }
        if self.nonland && candidate.is_land {
            return false;
        }
        if self
            .maximum_mana_value
            .is_some_and(|maximum| candidate.mana_value > maximum)
        {
            return false;
        }
        if self.must_have_dealt_damage_to_you_this_turn && !candidate.dealt_damage_to_you_this_turn
        {
            return false;
        }
        self.any_of_types.is_empty()
            || self
                .any_of_types
                .iter()
                .copied()
                .any(|permanent_type| candidate.has_type(permanent_type))
    }
}

impl SpellTargetScope {
    pub(crate) fn matches(&self, candidate: SpellTargetCandidate) -> bool {
        if self
            .excluded_type
            .is_some_and(|spell_type| candidate.has_type(spell_type))
        {
            return false;
        }
        if self
            .maximum_mana_value
            .is_some_and(|maximum| candidate.mana_value > maximum)
        {
            return false;
        }
        self.any_of_types.is_empty()
            || self
                .any_of_types
                .iter()
                .copied()
                .any(|spell_type| candidate.has_type(spell_type))
    }
}

#[allow(dead_code)]
impl InteractionRuntimeProgram {
    pub(crate) fn requires_target(&self) -> bool {
        matches!(
            self,
            Self::TargetedPermanent { .. } | Self::Counterspell { .. }
        )
    }

    pub(crate) fn has_legal_permanent_target(
        &self,
        candidates: &[PermanentTargetCandidate],
    ) -> bool {
        let Self::TargetedPermanent { target, .. } = self else {
            return false;
        };
        candidates
            .iter()
            .copied()
            .any(|candidate| target.matches(candidate))
    }

    pub(crate) fn has_legal_spell_target(&self, candidates: &[SpellTargetCandidate]) -> bool {
        let Self::Counterspell { target, .. } = self else {
            return false;
        };
        candidates
            .iter()
            .copied()
            .any(|candidate| target.matches(candidate))
    }

    pub(crate) fn can_resolve_in_targetless_goldfish(&self) -> bool {
        !self.requires_target()
    }
}

pub(crate) fn compile_interaction_runtime(
    input: InteractionCardInput<'_>,
) -> Option<InteractionRuntimeProgram> {
    if !is_instant_or_sorcery(input.type_line) {
        return None;
    }
    let clauses = input
        .oracle_text
        .lines()
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .collect::<Vec<_>>();
    let [clause] = clauses.as_slice() else {
        return None;
    };
    let normalized = normalize_clause(clause, input.name);
    let lowercase = normalized.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);

    compile_targeted_permanent(lower)
        .or_else(|| compile_counterspell(lower))
        .or_else(|| compile_destroy_all(lower))
}

/// Rebuilds the exact normalized Oracle root retained by the ability compiler
/// and classifies it with the same interaction parser used by coverage.
///
/// The bounded simulator deliberately does not keep a second free-form Oracle
/// string on every compiled card. Requiring contiguous clause ordinals here
/// keeps this bridge occurrence-addressed and prevents a partial or special
/// root program from being treated as a complete interaction spell.
pub(crate) fn compile_interaction_runtime_from_program(
    type_line: &str,
    program: &ExecutableAbilityProgramV1,
) -> Option<InteractionRuntimeProgram> {
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
    compile_interaction_runtime(InteractionCardInput {
        name: "this spell",
        type_line,
        oracle_text: &normalized_oracle,
    })
}

fn compile_targeted_permanent(lower: &str) -> Option<InteractionRuntimeProgram> {
    let (action, target) = if let Some(target) = lower.strip_prefix("destroy target ") {
        (InteractionAction::Destroy, target)
    } else if let Some(target) = lower.strip_prefix("exile target ") {
        (InteractionAction::Exile, target)
    } else if let Some(target) = lower
        .strip_prefix("return target ")
        .and_then(|target| target.strip_suffix(" to its owner's hand"))
    {
        (InteractionAction::ReturnToOwnersHand, target)
    } else {
        return None;
    };
    let target = parse_permanent_target(target)?;
    Some(InteractionRuntimeProgram::TargetedPermanent { action, target })
}

fn parse_permanent_target(value: &str) -> Option<PermanentTargetScope> {
    let mut value = value;
    let must_have_dealt_damage_to_you_this_turn =
        if let Some(target) = value.strip_suffix(" that dealt damage to you this turn") {
            value = target;
            true
        } else {
            false
        };
    let controller = if let Some(target) = value
        .strip_suffix(" you don't control")
        .or_else(|| value.strip_suffix(" an opponent controls"))
    {
        value = target;
        TargetController::Opponent
    } else {
        TargetController::Any
    };
    let (any_of_types, nonland) = match value {
        "creature" => (vec![PermanentType::Creature], false),
        "creature or enchantment" => (
            vec![PermanentType::Creature, PermanentType::Enchantment],
            false,
        ),
        "artifact or enchantment" => (
            vec![PermanentType::Artifact, PermanentType::Enchantment],
            false,
        ),
        "creature or planeswalker" => (
            vec![PermanentType::Creature, PermanentType::Planeswalker],
            false,
        ),
        "nonland permanent" => (Vec::new(), true),
        "permanent" => (Vec::new(), false),
        _ => return None,
    };
    Some(PermanentTargetScope {
        any_of_types,
        nonland,
        controller,
        maximum_mana_value: None,
        must_have_dealt_damage_to_you_this_turn,
    })
}

fn compile_counterspell(lower: &str) -> Option<InteractionRuntimeProgram> {
    let body = lower.strip_prefix("counter target ")?;
    let (target_text, unless_controller_pays_generic) =
        if let Some((target, payment)) = body.split_once(" unless its controller pays ") {
            (target, Some(parse_generic_mana_payment(payment)?))
        } else {
            (body, None)
        };
    let target = match target_text {
        "spell" => SpellTargetScope {
            any_of_types: Vec::new(),
            excluded_type: None,
            maximum_mana_value: None,
        },
        "noncreature spell" => SpellTargetScope {
            any_of_types: Vec::new(),
            excluded_type: Some(SpellType::Creature),
            maximum_mana_value: None,
        },
        "instant or sorcery spell" => SpellTargetScope {
            any_of_types: vec![SpellType::Instant, SpellType::Sorcery],
            excluded_type: None,
            maximum_mana_value: None,
        },
        "enchantment, instant, or sorcery spell" => SpellTargetScope {
            any_of_types: vec![
                SpellType::Enchantment,
                SpellType::Instant,
                SpellType::Sorcery,
            ],
            excluded_type: None,
            maximum_mana_value: None,
        },
        "spell with mana value 1" => SpellTargetScope {
            any_of_types: Vec::new(),
            excluded_type: None,
            maximum_mana_value: Some(1),
        },
        _ => return None,
    };
    Some(InteractionRuntimeProgram::Counterspell {
        target,
        unless_controller_pays_generic,
    })
}

fn compile_destroy_all(lower: &str) -> Option<InteractionRuntimeProgram> {
    let scope = match lower {
        "destroy all permanents" => WipeScope::AllPermanents,
        "destroy all nonland permanents" => WipeScope::AllNonlandPermanents,
        "destroy all creatures" => WipeScope::AllCreatures {
            maximum_mana_value: None,
        },
        "destroy all creatures with mana value 3 or less" => WipeScope::AllCreatures {
            maximum_mana_value: Some(3),
        },
        "destroy all artifacts, creatures, and enchantments" => {
            WipeScope::AllArtifactsCreaturesAndEnchantments
        }
        _ => return None,
    };
    Some(InteractionRuntimeProgram::DestroyAll { scope })
}

fn parse_generic_mana_payment(value: &str) -> Option<u16> {
    let value = value.strip_prefix('{')?.strip_suffix('}')?;
    value.parse::<u16>().ok().filter(|amount| *amount > 0)
}

fn is_instant_or_sorcery(type_line: &str) -> bool {
    let mut is_instant = false;
    let mut is_sorcery = false;
    for word in type_line.split(|character: char| !character.is_alphabetic()) {
        is_instant |= word.eq_ignore_ascii_case("instant");
        is_sorcery |= word.eq_ignore_ascii_case("sorcery");
    }
    is_instant ^ is_sorcery
}

fn trim_terminal_period(value: &str) -> &str {
    value.strip_suffix('.').unwrap_or(value).trim()
}

fn normalize_clause(clause: &str, card_name: &str) -> String {
    let clause = clause.trim().replace('’', "'");
    let replaced = replace_ascii_case_insensitive(&clause, card_name.trim(), "this spell");
    replaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn replace_ascii_case_insensitive(source: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return source.to_string();
    }
    let lower_source = source.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative) = lower_source[cursor..].find(&lower_needle) {
        let start = cursor + relative;
        let end = start + needle.len();
        if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            return source.to_string();
        }
        output.push_str(&source[cursor..start]);
        output.push_str(replacement);
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    output
}
