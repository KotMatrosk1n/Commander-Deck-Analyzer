use crate::ability_program::AbilityCompilation;
use crate::domain::{KnownLine, KnownLineOutcome, LineRequirement};
use crate::semantics::{CompiledCard, CompiledDeck};
use std::collections::HashSet;

const ORACLE_CURRENT_CORE: &str = "when this permanent enters, look at the top x cards of your library, where x is your devotion to blue. put up to one of them on top of your library and the rest on the bottom of your library in a random order. if x is greater than or equal to the number of cards in your library, you win the game";
const ORACLE_LEGACY_CORE: &str = "when this permanent enters the battlefield, look at the top x cards of your library, where x is your devotion to blue. put up to one of them on top of your library and the rest on the bottom of your library in a random order. if x is greater than or equal to the number of cards in your library, you win the game";
const ORACLE_DEVOTION_REMINDER: &str =
    " (each {u} in the mana costs of permanents you control counts toward your devotion to blue.)";
const CONSULTATION_ROOT: &str = "choose a card name. exile the top six cards of your library, then reveal cards from the top of your library until you reveal a card with the chosen name. put that card into your hand and exile all other cards revealed this way";
const PACT_ROOT: &str = "exile the top card of your library. you may put that card into your hand unless it has the same name as another card exiled this way. repeat this process until you put a card into your hand or you exile two cards with the same name, whichever comes first";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReviewedLibraryExileKind {
    Consultation,
    Pact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReviewedEmptyLibraryWinProgram {
    pub line_index: usize,
    pub oracle_card_index: usize,
    pub exile_spell_card_index: usize,
    pub exile_kind: ReviewedLibraryExileKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReviewedLibraryExileReceipt {
    pub starting_library_cards: usize,
    pub exiled_library_cards: usize,
    pub remaining_library_cards: usize,
}

fn complete_single_root(card: &CompiledCard) -> Option<String> {
    let program = &card.ability_program;
    if program.necropotence_lifecycle.is_some()
        || program.self_transfer_tutor_permanent.is_some()
        || program.entry_linked_permanent.is_some()
        || program.atomic_transaction.is_some()
        || !program.face_programs.is_empty()
    {
        return None;
    }
    let [ability] = program.abilities.as_slice() else {
        return None;
    };
    let normalized = match ability {
        AbilityCompilation::Executable(ability) => ability.normalized_oracle.as_str(),
        AbilityCompilation::Unsupported(ability) => ability.normalized_oracle.as_str(),
    };
    Some(
        normalized
            .trim()
            .to_ascii_lowercase()
            .trim_end_matches('.')
            .to_string(),
    )
}

fn exact_oracle_win_trigger(card: &CompiledCard) -> bool {
    if !card.effects.card_types.is_creature {
        return false;
    }
    let Some(mut root) = complete_single_root(card) else {
        return false;
    };
    if root.ends_with(ORACLE_DEVOTION_REMINDER) {
        root.truncate(root.len() - ORACLE_DEVOTION_REMINDER.len());
        root = root.trim_end_matches('.').to_string();
    }
    matches!(root.as_str(), ORACLE_CURRENT_CORE | ORACLE_LEGACY_CORE)
}

fn exact_consultation_exiler(card: &CompiledCard) -> bool {
    card.effects.card_types.is_instant
        && complete_single_root(card).as_deref() == Some(CONSULTATION_ROOT)
}

fn exact_pact_exiler(card: &CompiledCard) -> bool {
    card.effects.card_types.is_instant && complete_single_root(card).as_deref() == Some(PACT_ROOT)
}

fn exact_requirements(line: &KnownLine, expected: &[LineRequirement]) -> bool {
    line.simulation_requirements.len() == expected.len()
        && expected.iter().all(|requirement| {
            line.simulation_requirements
                .iter()
                .filter(|candidate| *candidate == requirement)
                .count()
                == expected
                    .iter()
                    .filter(|candidate| *candidate == requirement)
                    .count()
        })
}

pub(crate) fn compile_reviewed_empty_library_win_program(
    line_index: usize,
    line: &KnownLine,
    deck: &CompiledDeck,
) -> Option<ReviewedEmptyLibraryWinProgram> {
    if !line.table_lethal_if_resolved
        || line.outcome != KnownLineOutcome::TableWin
        || line.cards.len() != 2
        || line.compactness != 2
    {
        return None;
    }

    let mut members = Vec::with_capacity(2);
    for name in &line.cards {
        let normalized = crate::parser::normalize_card_name(name);
        let mut matches = deck
            .cards
            .iter()
            .enumerate()
            .filter(|(_, card)| card.normalized_name == normalized);
        let (card_index, _) = matches.next()?;
        if matches.next().is_some() || members.contains(&card_index) {
            return None;
        }
        members.push(card_index);
    }

    let oracle_members = members
        .iter()
        .copied()
        .filter(|card_index| exact_oracle_win_trigger(&deck.cards[*card_index]))
        .collect::<Vec<_>>();
    if oracle_members.len() != 1 {
        return None;
    }
    let oracle_card_index = oracle_members[0];
    let exile_spell_card_index = members
        .iter()
        .copied()
        .find(|card_index| *card_index != oracle_card_index)?;
    let exile_kind = if exact_consultation_exiler(&deck.cards[exile_spell_card_index])
        && exact_requirements(
            line,
            &[
                LineRequirement::NamedCardsPayPrintedCosts,
                LineRequirement::ReviewedEmptyLibrarySequence,
            ],
        ) {
        ReviewedLibraryExileKind::Consultation
    } else if exact_pact_exiler(&deck.cards[exile_spell_card_index])
        && exact_requirements(
            line,
            &[
                LineRequirement::SingletonLibrary,
                LineRequirement::NamedCardsPayPrintedCosts,
                LineRequirement::ReviewedEmptyLibrarySequence,
            ],
        )
    {
        ReviewedLibraryExileKind::Pact
    } else {
        return None;
    };

    Some(ReviewedEmptyLibraryWinProgram {
        line_index,
        oracle_card_index,
        exile_spell_card_index,
        exile_kind,
    })
}

/// Execute only the reviewed "decline/name absent" branch that exhausts the
/// actual remaining library. The caller supplies a clone because this receipt
/// is still a pending win candidate until table interaction is resolved.
pub(crate) fn execute_reviewed_library_exile_transaction(
    program: ReviewedEmptyLibraryWinProgram,
    deck: &CompiledDeck,
    library_order: &mut Vec<usize>,
    next_draw_position: usize,
    exile: &mut Vec<usize>,
) -> Option<ReviewedLibraryExileReceipt> {
    if next_draw_position > library_order.len() {
        return None;
    }
    let remaining = &library_order[next_draw_position..];
    match program.exile_kind {
        ReviewedLibraryExileKind::Consultation => {
            let source_name = &deck
                .cards
                .get(program.exile_spell_card_index)?
                .normalized_name;
            if remaining.iter().any(|card_index| {
                deck.cards
                    .get(*card_index)
                    .is_some_and(|card| card.normalized_name == *source_name)
            }) {
                return None;
            }
        }
        ReviewedLibraryExileKind::Pact => {
            let mut names = HashSet::with_capacity(remaining.len());
            for card_index in remaining {
                let normalized_name = &deck.cards.get(*card_index)?.normalized_name;
                if !names.insert(normalized_name.as_str()) {
                    return None;
                }
            }
        }
    }

    let mut staged_library = library_order.clone();
    let mut staged_exile = exile.clone();
    let starting_library_cards = staged_library.len() - next_draw_position;
    let moved = staged_library
        .drain(next_draw_position..)
        .collect::<Vec<_>>();
    let exiled_library_cards = moved.len();
    staged_exile.extend(moved);
    let remaining_library_cards = staged_library.len() - next_draw_position;
    if exiled_library_cards != starting_library_cards || remaining_library_cards != 0 {
        return None;
    }

    *library_order = staged_library;
    *exile = staged_exile;
    Some(ReviewedLibraryExileReceipt {
        starting_library_cards,
        exiled_library_cards,
        remaining_library_cards,
    })
}
