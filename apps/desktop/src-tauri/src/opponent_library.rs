//! Bounded opponent-library state and rules-backed draw-loss receipts.
//!
//! Opponent card identities and hidden ordering are never invented. Each
//! library is represented by a conservative upper bound on physical cards
//! remaining. Proving enough mill to exhaust that upper bound proves the
//! unknown real library is empty as well.

use crate::combat_terminal::{OPPONENT_COUNT, OpponentId};

/// A one-commander deck begins with at most 99 library cards. Under CR 103.5,
/// a player may mulligan until their opening hand is zero cards; CR 103.5c
/// makes only the first multiplayer mulligan free. Repeated London mulligans
/// can therefore return every noncommander card to the library before the
/// game begins. Partner/background configurations start with fewer, so 99 is
/// the conservative post-opening upper bound for every legal configuration.
pub(crate) const COMMANDER_LIBRARY_UPPER_BOUND_AFTER_OPENING: u16 = 99;
pub(crate) const THREE_CARD_MILL_PACKET_SIZE: u16 = 3;
pub(crate) const BREACH_SELF_MILL_PACKETS_PER_ITERATION: u16 = 2;
pub(crate) const BREACH_SPELLS_PER_ITERATION: u16 = 2;
pub(crate) const MINIMUM_PROVEN_RECURRENCE_ITERATIONS: u16 = 3;
pub(crate) const MAXIMUM_TABLE_SATURATION_PACKETS: u16 = OPPONENT_COUNT as u16
    * COMMANDER_LIBRARY_UPPER_BOUND_AFTER_OPENING.div_ceil(THREE_CARD_MILL_PACKET_SIZE);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpponentLibraryRule {
    Cr104_2aAllOpponentsLeft,
    Cr104_3cRequiredDrawExceedsLibrary,
    Cr121_4SequentialMultiCardDraw,
    Cr704_5bDrawFromEmptyLibrary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FiniteMillRecurrenceReceipt {
    pub packet_size: u16,
    pub initial_spells_cast: u16,
    pub recurrence_iterations: u16,
    pub storm_spell_casts: u16,
    pub storm_copy_count: u16,
    pub self_mill_packets: u16,
    pub opponent_mill_packets: u16,
    pub self_library_cards_required: u16,
}

impl FiniteMillRecurrenceReceipt {
    fn balances(self) -> bool {
        let iterations = u64::from(self.recurrence_iterations);
        let initial_spells = u64::from(self.initial_spells_cast);
        let Some(opponent_packets_per_first_iteration) = initial_spells.checked_sub(1) else {
            return false;
        };
        let iteration_growth = iterations.saturating_mul(iterations.saturating_sub(1));
        let expected_copies = iterations
            .saturating_mul(initial_spells)
            .saturating_add(iteration_growth);
        let expected_opponent_packets = iterations
            .saturating_mul(opponent_packets_per_first_iteration)
            .saturating_add(iteration_growth);
        let expected_self_packets =
            iterations.saturating_mul(u64::from(BREACH_SELF_MILL_PACKETS_PER_ITERATION));
        let expected_self_cards =
            expected_self_packets.saturating_mul(u64::from(THREE_CARD_MILL_PACKET_SIZE));

        self.packet_size == THREE_CARD_MILL_PACKET_SIZE
            && self.recurrence_iterations >= MINIMUM_PROVEN_RECURRENCE_ITERATIONS
            && self.storm_spell_casts == self.recurrence_iterations
            && u64::from(self.storm_copy_count) == expected_copies
            && u64::from(self.self_mill_packets) == expected_self_packets
            && u64::from(self.opponent_mill_packets) == expected_opponent_packets
            && u64::from(self.self_library_cards_required) == expected_self_cards
    }
}

/// Compress the exact two-spell Breach recurrence into one finite proof.
///
/// Each iteration casts the storm mill spell and the mana source. Two
/// three-card packets target the controller to replenish the six physical
/// escape-fodder cards. Every remaining original/copy packet is available for
/// opponents. The loop is bounded by the controller's actual unseen library,
/// so this never relies on an infinite physical deck.
pub(crate) fn prove_finite_breach_mill_recurrence(
    initial_spells_cast: u16,
    available_self_library_cards: usize,
    required_opponent_mill_packets: u16,
) -> Option<FiniteMillRecurrenceReceipt> {
    if required_opponent_mill_packets == 0 {
        return None;
    }
    let cards_per_iteration =
        THREE_CARD_MILL_PACKET_SIZE.saturating_mul(BREACH_SELF_MILL_PACKETS_PER_ITERATION);
    let maximum_iterations = u16::try_from(
        available_self_library_cards
            .min(usize::from(u16::MAX))
            .checked_div(usize::from(cards_per_iteration))?,
    )
    .ok()?;

    let mut storm_copy_count = 0u32;
    let mut opponent_mill_packets = 0u32;
    for zero_based_iteration in 0..maximum_iterations {
        let resolutions = u32::from(initial_spells_cast)
            .saturating_add(
                u32::from(BREACH_SPELLS_PER_ITERATION)
                    .saturating_mul(u32::from(zero_based_iteration)),
            )
            .saturating_add(1);
        if resolutions <= u32::from(BREACH_SELF_MILL_PACKETS_PER_ITERATION) {
            return None;
        }
        storm_copy_count = storm_copy_count.saturating_add(resolutions.saturating_sub(1));
        opponent_mill_packets = opponent_mill_packets.saturating_add(
            resolutions.saturating_sub(u32::from(BREACH_SELF_MILL_PACKETS_PER_ITERATION)),
        );
        let recurrence_iterations = zero_based_iteration.saturating_add(1);
        if recurrence_iterations < MINIMUM_PROVEN_RECURRENCE_ITERATIONS
            || opponent_mill_packets < u32::from(required_opponent_mill_packets)
        {
            continue;
        }

        let storm_spell_casts = recurrence_iterations;
        let self_mill_packets =
            recurrence_iterations.saturating_mul(BREACH_SELF_MILL_PACKETS_PER_ITERATION);
        return Some(FiniteMillRecurrenceReceipt {
            packet_size: THREE_CARD_MILL_PACKET_SIZE,
            initial_spells_cast,
            recurrence_iterations,
            storm_spell_casts,
            storm_copy_count: u16::try_from(storm_copy_count).ok()?,
            self_mill_packets,
            opponent_mill_packets: u16::try_from(opponent_mill_packets).ok()?,
            self_library_cards_required: self_mill_packets
                .saturating_mul(THREE_CARD_MILL_PACKET_SIZE),
        });
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpponentDrawReceipt {
    pub opponent: OpponentId,
    pub cards_requested: u16,
    pub cards_drawn: u16,
    pub library_upper_bound_before: u16,
    pub library_upper_bound_after: u16,
    pub sequential_draw_rule: OpponentLibraryRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpponentDrawLossReceipt {
    pub opponent: OpponentId,
    pub cards_requested: u16,
    pub cards_drawn: u16,
    pub missing_draws: u16,
    pub library_upper_bound_before: u16,
    pub library_upper_bound_after: u16,
    pub required_draw_loss_rule: OpponentLibraryRule,
    pub sequential_draw_rule: OpponentLibraryRule,
    pub state_based_action_rule: OpponentLibraryRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpponentDrawResolution {
    Completed(OpponentDrawReceipt),
    Loses(OpponentDrawLossReceipt),
}

impl OpponentDrawResolution {
    pub(crate) const fn cards_drawn(self) -> u16 {
        match self {
            Self::Completed(receipt) => receipt.cards_drawn,
            Self::Loses(receipt) => receipt.cards_drawn,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpponentLibraryState {
    remaining_card_upper_bound: u16,
    draw_loss_receipt: Option<OpponentDrawLossReceipt>,
}

impl OpponentLibraryState {
    const fn with_upper_bound(remaining_card_upper_bound: u16) -> Self {
        Self {
            remaining_card_upper_bound,
            draw_loss_receipt: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpponentLibraryTable {
    opponents: [OpponentLibraryState; OPPONENT_COUNT],
}

impl Default for OpponentLibraryTable {
    fn default() -> Self {
        Self::after_opening()
    }
}

impl OpponentLibraryTable {
    pub(crate) fn after_opening() -> Self {
        Self::from_upper_bounds([COMMANDER_LIBRARY_UPPER_BOUND_AFTER_OPENING; OPPONENT_COUNT])
    }

    pub(crate) fn from_upper_bounds(upper_bounds: [u16; OPPONENT_COUNT]) -> Self {
        Self {
            opponents: upper_bounds.map(OpponentLibraryState::with_upper_bound),
        }
    }
    pub(crate) fn draw_loss_receipts(&self) -> [Option<OpponentDrawLossReceipt>; OPPONENT_COUNT] {
        self.opponents.map(|state| state.draw_loss_receipt)
    }

    /// Resolve one modeled instruction to draw one or more cards. CR 121.4
    /// treats a multi-card draw as sequential individual draws; CR 104.3c and
    /// CR 704.5b produce a loss receipt when the request exceeds the library.
    pub(crate) fn resolve_draw_requirement(
        &mut self,
        opponent: OpponentId,
        cards_requested: u16,
    ) -> Option<OpponentDrawResolution> {
        if cards_requested == 0 {
            return None;
        }
        let state = &mut self.opponents[opponent.index()];
        if state.draw_loss_receipt.is_some() {
            return None;
        }
        let library_upper_bound_before = state.remaining_card_upper_bound;
        let cards_drawn = library_upper_bound_before.min(cards_requested);
        state.remaining_card_upper_bound =
            state.remaining_card_upper_bound.saturating_sub(cards_drawn);
        if cards_drawn == cards_requested {
            return Some(OpponentDrawResolution::Completed(OpponentDrawReceipt {
                opponent,
                cards_requested,
                cards_drawn,
                library_upper_bound_before,
                library_upper_bound_after: state.remaining_card_upper_bound,
                sequential_draw_rule: OpponentLibraryRule::Cr121_4SequentialMultiCardDraw,
            }));
        }

        let receipt = OpponentDrawLossReceipt {
            opponent,
            cards_requested,
            cards_drawn,
            missing_draws: cards_requested.saturating_sub(cards_drawn),
            library_upper_bound_before,
            library_upper_bound_after: state.remaining_card_upper_bound,
            required_draw_loss_rule: OpponentLibraryRule::Cr104_3cRequiredDrawExceedsLibrary,
            sequential_draw_rule: OpponentLibraryRule::Cr121_4SequentialMultiCardDraw,
            state_based_action_rule: OpponentLibraryRule::Cr704_5bDrawFromEmptyLibrary,
        };
        state.draw_loss_receipt = Some(receipt);
        Some(OpponentDrawResolution::Loses(receipt))
    }

    /// Apply every opponent-directed packet from one finite recurrence.
    ///
    /// Required packets are assigned in stable opponent order. Any proven
    /// excess packets target the first active opponent after its library is
    /// already empty; they mill zero rather than inventing additional cards.
    /// Mutation is staged and committed only after the receipt balances.
    pub(crate) fn saturate_active_opponents(
        &mut self,
        active_opponents: [bool; OPPONENT_COUNT],
        recurrence: FiniteMillRecurrenceReceipt,
        committed_self_library_cards: u16,
    ) -> Option<OpponentLibrarySaturationReceipt> {
        if !recurrence.balances()
            || committed_self_library_cards != recurrence.self_library_cards_required
            || !active_opponents.into_iter().any(|active| active)
        {
            return None;
        }
        let library_upper_bounds_before =
            self.opponents.map(|state| state.remaining_card_upper_bound);
        let mut mill_packets_by_opponent = [0u16; OPPONENT_COUNT];
        let mut required_packets = 0u16;
        for opponent in OpponentId::ALL {
            if !active_opponents[opponent.index()] {
                continue;
            }
            let packets = self.opponents[opponent.index()]
                .remaining_card_upper_bound
                .div_ceil(THREE_CARD_MILL_PACKET_SIZE);
            mill_packets_by_opponent[opponent.index()] = packets;
            required_packets = required_packets.saturating_add(packets);
        }
        if recurrence.opponent_mill_packets < required_packets {
            return None;
        }

        let first_active = OpponentId::ALL
            .into_iter()
            .find(|opponent| active_opponents[opponent.index()])?;
        mill_packets_by_opponent[first_active.index()] =
            mill_packets_by_opponent[first_active.index()].saturating_add(
                recurrence
                    .opponent_mill_packets
                    .saturating_sub(required_packets),
            );
        if mill_packets_by_opponent
            .into_iter()
            .fold(0u16, u16::saturating_add)
            != recurrence.opponent_mill_packets
        {
            return None;
        }

        let mut staged = self.clone();
        for opponent in OpponentId::ALL {
            if active_opponents[opponent.index()] {
                staged.opponents[opponent.index()].remaining_card_upper_bound = 0;
            }
        }
        let library_upper_bounds_after = staged
            .opponents
            .map(|state| state.remaining_card_upper_bound);
        let receipt = OpponentLibrarySaturationReceipt {
            active_opponents,
            library_upper_bounds_before,
            library_upper_bounds_after,
            mill_packets_by_opponent,
            recurrence,
            committed_self_library_cards,
            draw_loss_rule: OpponentLibraryRule::Cr104_3cRequiredDrawExceedsLibrary,
            state_based_action_rule: OpponentLibraryRule::Cr704_5bDrawFromEmptyLibrary,
            table_win_rule: OpponentLibraryRule::Cr104_2aAllOpponentsLeft,
        };
        *self = staged;
        Some(receipt)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpponentLibrarySaturationReceipt {
    pub active_opponents: [bool; OPPONENT_COUNT],
    pub library_upper_bounds_before: [u16; OPPONENT_COUNT],
    pub library_upper_bounds_after: [u16; OPPONENT_COUNT],
    pub mill_packets_by_opponent: [u16; OPPONENT_COUNT],
    pub recurrence: FiniteMillRecurrenceReceipt,
    pub committed_self_library_cards: u16,
    pub draw_loss_rule: OpponentLibraryRule,
    pub state_based_action_rule: OpponentLibraryRule,
    pub table_win_rule: OpponentLibraryRule,
}
