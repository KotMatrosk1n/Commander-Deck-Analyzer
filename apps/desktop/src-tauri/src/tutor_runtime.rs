//! Exact tutor programs for the reviewed Alela tutor family.
//!
//! Classification is based on card type and complete Oracle structure. Card
//! names are deliberately absent from the input so alternate print names
//! cannot affect compilation.

use crate::ability_program::{
    AbilityCompilation, EXECUTABLE_ABILITY_PROGRAM_VERSION, ExecutableAbilityProgramV1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TutorCardInput<'a> {
    pub type_line: &'a str,
    pub oracle_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpellSpeed {
    Instant,
    Sorcery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TutorSourceZone {
    Library,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TutorDestinationZone {
    Hand,
    TopOfLibrary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchRequirement {
    Required,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchQuantity {
    ExactlyOne,
    UpTo(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchedCardPredicate {
    AnyCard,
    Enchantment,
    ArtifactOrEnchantment,
    BasicLand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TutorTransactionStep {
    RevealSearchedCards,
    MoveSearchedCardsTo(TutorDestinationZone),
    ShuffleLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TutorSearchTransaction {
    pub source_zone: TutorSourceZone,
    pub requirement: SearchRequirement,
    pub quantity: SearchQuantity,
    pub searched_card: SearchedCardPredicate,
    pub ordered_steps: Vec<TutorTransactionStep>,
}

impl TutorSearchTransaction {
    pub(crate) fn destination_zone(&self) -> Option<TutorDestinationZone> {
        self.ordered_steps.iter().find_map(|step| match step {
            TutorTransactionStep::MoveSearchedCardsTo(destination) => Some(*destination),
            _ => None,
        })
    }

    pub(crate) fn reveals_searched_cards(&self) -> bool {
        self.ordered_steps
            .contains(&TutorTransactionStep::RevealSearchedCards)
    }

    pub(crate) fn shuffles_library(&self) -> bool {
        self.ordered_steps
            .contains(&TutorTransactionStep::ShuffleLibrary)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifeLossTiming {
    AfterSearchTransaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LifeLoss {
    pub amount: u8,
    pub timing: LifeLossTiming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriggerTiming {
    BeginningOfYourUpkeep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InterveningIfCondition {
    OpponentControlsMoreLandsThanYou,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpellTutorProgram {
    pub speed: SpellSpeed,
    pub search: TutorSearchTransaction,
    pub life_loss: Option<LifeLoss>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TriggeredTutorProgram {
    pub timing: TriggerTiming,
    pub intervening_if: InterveningIfCondition,
    pub search: TutorSearchTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TutorRuntimeProgram {
    Spell(SpellTutorProgram),
    TriggeredPermanent(TriggeredTutorProgram),
}

pub(crate) fn compile_tutor_runtime(input: TutorCardInput<'_>) -> Option<TutorRuntimeProgram> {
    let normalized = normalize_oracle_text(input.oracle_text);
    let sentences = split_sentences(&normalized)?;

    compile_triggered_tutor(input.type_line, &sentences)
        .or_else(|| compile_spell_tutor(input.type_line, &sentences))
}

pub(crate) fn compile_tutor_runtime_from_program(
    type_line: &str,
    program: &ExecutableAbilityProgramV1,
) -> Option<TutorRuntimeProgram> {
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
    compile_tutor_runtime(TutorCardInput {
        type_line,
        oracle_text: &normalized_oracle,
    })
}

fn compile_spell_tutor(type_line: &str, sentences: &[&str]) -> Option<TutorRuntimeProgram> {
    let speed = spell_speed(type_line)?;
    let first = *sentences.first()?;
    let search = parse_search_instruction(first)?;
    let life_loss = match sentences {
        [_] => None,
        [_, "you lose 3 life"] => Some(LifeLoss {
            amount: 3,
            timing: LifeLossTiming::AfterSearchTransaction,
        }),
        _ => return None,
    };

    let is_reviewed_shape = matches!(
        (
            speed,
            search.requirement,
            search.quantity,
            search.searched_card,
            search.ordered_steps.as_slice(),
            life_loss,
        ),
        (
            SpellSpeed::Instant,
            SearchRequirement::Required,
            SearchQuantity::ExactlyOne,
            SearchedCardPredicate::ArtifactOrEnchantment,
            [
                TutorTransactionStep::RevealSearchedCards,
                TutorTransactionStep::ShuffleLibrary,
                TutorTransactionStep::MoveSearchedCardsTo(TutorDestinationZone::TopOfLibrary),
            ],
            None,
        ) | (
            SpellSpeed::Sorcery,
            SearchRequirement::Required,
            SearchQuantity::ExactlyOne,
            SearchedCardPredicate::Enchantment,
            [
                TutorTransactionStep::RevealSearchedCards,
                TutorTransactionStep::MoveSearchedCardsTo(TutorDestinationZone::Hand),
                TutorTransactionStep::ShuffleLibrary,
            ],
            None,
        ) | (
            SpellSpeed::Sorcery,
            SearchRequirement::Required,
            SearchQuantity::ExactlyOne,
            SearchedCardPredicate::AnyCard,
            [
                TutorTransactionStep::MoveSearchedCardsTo(TutorDestinationZone::Hand),
                TutorTransactionStep::ShuffleLibrary,
            ],
            Some(LifeLoss {
                amount: 3,
                timing: LifeLossTiming::AfterSearchTransaction,
            }),
        )
    );

    is_reviewed_shape.then_some(TutorRuntimeProgram::Spell(SpellTutorProgram {
        speed,
        search,
        life_loss,
    }))
}

fn compile_triggered_tutor(type_line: &str, sentences: &[&str]) -> Option<TutorRuntimeProgram> {
    if !has_card_type(type_line, "enchantment") {
        return None;
    }
    let [sentence] = sentences else {
        return None;
    };
    let search_text = sentence.strip_prefix(
        "at the beginning of your upkeep, if an opponent controls more lands than you, ",
    )?;
    let search = parse_search_instruction(search_text)?;
    if search
        != (TutorSearchTransaction {
            source_zone: TutorSourceZone::Library,
            requirement: SearchRequirement::Optional,
            quantity: SearchQuantity::UpTo(3),
            searched_card: SearchedCardPredicate::BasicLand,
            ordered_steps: vec![
                TutorTransactionStep::RevealSearchedCards,
                TutorTransactionStep::MoveSearchedCardsTo(TutorDestinationZone::Hand),
                TutorTransactionStep::ShuffleLibrary,
            ],
        })
    {
        return None;
    }

    Some(TutorRuntimeProgram::TriggeredPermanent(
        TriggeredTutorProgram {
            timing: TriggerTiming::BeginningOfYourUpkeep,
            intervening_if: InterveningIfCondition::OpponentControlsMoreLandsThanYou,
            search,
        },
    ))
}

fn parse_search_instruction(value: &str) -> Option<TutorSearchTransaction> {
    let (requirement, body) = if let Some(body) = value.strip_prefix("search your library for ") {
        (SearchRequirement::Required, body)
    } else if let Some(body) = value.strip_prefix("you may search your library for ") {
        (SearchRequirement::Optional, body)
    } else {
        return None;
    };

    let (quantity, searched_card, suffix) =
        if let Some(suffix) = body.strip_prefix("up to three basic land cards") {
            (
                SearchQuantity::UpTo(3),
                SearchedCardPredicate::BasicLand,
                suffix,
            )
        } else if let Some(suffix) = body.strip_prefix("an artifact or enchantment card") {
            (
                SearchQuantity::ExactlyOne,
                SearchedCardPredicate::ArtifactOrEnchantment,
                suffix,
            )
        } else if let Some(suffix) = body.strip_prefix("an enchantment card") {
            (
                SearchQuantity::ExactlyOne,
                SearchedCardPredicate::Enchantment,
                suffix,
            )
        } else if let Some(suffix) = body.strip_prefix("a card") {
            (
                SearchQuantity::ExactlyOne,
                SearchedCardPredicate::AnyCard,
                suffix,
            )
        } else {
            return None;
        };

    let ordered_steps = match suffix {
        ", reveal it, then shuffle and put that card on top" => vec![
            TutorTransactionStep::RevealSearchedCards,
            TutorTransactionStep::ShuffleLibrary,
            TutorTransactionStep::MoveSearchedCardsTo(TutorDestinationZone::TopOfLibrary),
        ],
        ", reveal it, put it into your hand, then shuffle" => vec![
            TutorTransactionStep::RevealSearchedCards,
            TutorTransactionStep::MoveSearchedCardsTo(TutorDestinationZone::Hand),
            TutorTransactionStep::ShuffleLibrary,
        ],
        ", put that card into your hand, then shuffle" => vec![
            TutorTransactionStep::MoveSearchedCardsTo(TutorDestinationZone::Hand),
            TutorTransactionStep::ShuffleLibrary,
        ],
        ", reveal them, put them into your hand, then shuffle" => vec![
            TutorTransactionStep::RevealSearchedCards,
            TutorTransactionStep::MoveSearchedCardsTo(TutorDestinationZone::Hand),
            TutorTransactionStep::ShuffleLibrary,
        ],
        _ => return None,
    };

    Some(TutorSearchTransaction {
        source_zone: TutorSourceZone::Library,
        requirement,
        quantity,
        searched_card,
        ordered_steps,
    })
}

fn spell_speed(type_line: &str) -> Option<SpellSpeed> {
    let instant = has_card_type(type_line, "instant");
    let sorcery = has_card_type(type_line, "sorcery");
    match (instant, sorcery) {
        (true, false) => Some(SpellSpeed::Instant),
        (false, true) => Some(SpellSpeed::Sorcery),
        _ => None,
    }
}

fn has_card_type(type_line: &str, expected: &str) -> bool {
    type_line
        .split(|character: char| !character.is_alphabetic())
        .any(|word| word.eq_ignore_ascii_case(expected))
}

fn normalize_oracle_text(oracle_text: &str) -> String {
    oracle_text
        .trim()
        .replace('’', "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn split_sentences(normalized: &str) -> Option<Vec<&str>> {
    if normalized.is_empty() {
        return None;
    }
    let mut sentences = normalized.split('.').collect::<Vec<_>>();
    if sentences.last().is_some_and(|sentence| sentence.is_empty()) {
        sentences.pop();
    }
    if sentences.is_empty() || sentences.iter().any(|sentence| sentence.trim().is_empty()) {
        return None;
    }
    Some(sentences.into_iter().map(str::trim).collect())
}
