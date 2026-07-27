//! Typed, name-independent turn and event lifecycle state.
//!
//! The caller owns rules interpretation and supplies stable player/object
//! identities plus a typed counter key. This module does not parse Oracle text
//! and has no card, deck, commander, or display-name fields.
//!
//! Monarch ownership and counters persist across turns. Life lost and recorded
//! end-step occurrences are turn-scoped and reset only after a checked turn
//! advance. Active-player sets are supplied by the caller, so eliminated
//! players cannot contribute end-step events unless the caller incorrectly
//! includes them.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControllerCondition {
    IsMonarch,
    LostNoLifeThisTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnEventOperation {
    LifeLoss,
    CounterAddition,
    EndStepOccurrence,
    EndStepQuery,
    TurnAdvance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjectRegistrationResult {
    Registered,
    AlreadyRegistered,
}

/// Persistent and current-turn state parameterized by caller-owned identity
/// types. `C` should be a closed enum supplied by the typed compiler/runtime,
/// not an Oracle-text or card-name string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TurnEventState<P, O, C>
where
    P: Copy + Ord + Debug,
    O: Copy + Ord + Debug,
    C: Clone + Ord,
{
    known_players: BTreeSet<P>,
    known_objects: BTreeSet<O>,
    monarch: Option<P>,
    life_lost_this_turn: BTreeMap<P, u32>,
    counters: BTreeMap<O, BTreeMap<C, u32>>,
    end_step_occurrences_this_turn: BTreeMap<P, u32>,
    turn_sequence: u64,
}

impl<P, O, C> TurnEventState<P, O, C>
where
    P: Copy + Ord + Debug,
    O: Copy + Ord + Debug,
    C: Clone + Ord,
{
    pub(crate) fn new(
        players: impl IntoIterator<Item = P>,
        objects: impl IntoIterator<Item = O>,
    ) -> Result<Self, TurnEventStateError<P, O>> {
        let mut known_players = BTreeSet::new();
        for player in players {
            if !known_players.insert(player) {
                return Err(TurnEventStateError::DuplicatePlayer { player });
            }
        }
        let mut known_objects = BTreeSet::new();
        for object in objects {
            if !known_objects.insert(object) {
                return Err(TurnEventStateError::DuplicateObject { object });
            }
        }
        Ok(Self {
            known_players,
            known_objects,
            monarch: None,
            life_lost_this_turn: BTreeMap::new(),
            counters: BTreeMap::new(),
            end_step_occurrences_this_turn: BTreeMap::new(),
            turn_sequence: 0,
        })
    }

    pub(crate) const fn turn_sequence(&self) -> u64 {
        self.turn_sequence
    }

    pub(crate) fn monarch(&self) -> Option<P> {
        self.monarch
    }

    pub(crate) fn set_monarch(
        &mut self,
        player: P,
    ) -> Result<Option<P>, TurnEventStateError<P, O>> {
        self.require_player(player)?;
        Ok(self.monarch.replace(player))
    }

    pub(crate) fn clear_monarch(&mut self) -> Option<P> {
        self.monarch.take()
    }

    /// Register a newly created game object. Re-registering the same stable
    /// identity is idempotent; callers that reuse an identity after a zone
    /// change must unregister the old object first.
    pub(crate) fn register_object(&mut self, object: O) -> ObjectRegistrationResult {
        if self.known_objects.insert(object) {
            ObjectRegistrationResult::Registered
        } else {
            ObjectRegistrationResult::AlreadyRegistered
        }
    }

    /// Remove a game object and all counters attached to that object's prior
    /// battlefield existence.
    pub(crate) fn unregister_object(&mut self, object: O) -> bool {
        self.counters.remove(&object);
        self.known_objects.remove(&object)
    }

    pub(crate) fn object_is_registered(&self, object: O) -> bool {
        self.known_objects.contains(&object)
    }

    pub(crate) fn controller_condition(
        &self,
        controller: P,
        condition: ControllerCondition,
    ) -> Result<bool, TurnEventStateError<P, O>> {
        self.require_player(controller)?;
        Ok(match condition {
            ControllerCondition::IsMonarch => self.monarch == Some(controller),
            ControllerCondition::LostNoLifeThisTurn => {
                self.life_lost_this_turn
                    .get(&controller)
                    .copied()
                    .unwrap_or_default()
                    == 0
            }
        })
    }

    pub(crate) fn controller_is_monarch(
        &self,
        controller: P,
    ) -> Result<bool, TurnEventStateError<P, O>> {
        self.controller_condition(controller, ControllerCondition::IsMonarch)
    }

    pub(crate) fn controller_lost_no_life_this_turn(
        &self,
        controller: P,
    ) -> Result<bool, TurnEventStateError<P, O>> {
        self.controller_condition(controller, ControllerCondition::LostNoLifeThisTurn)
    }

    pub(crate) fn record_life_loss(
        &mut self,
        player: P,
        amount: u32,
    ) -> Result<u32, TurnEventStateError<P, O>> {
        self.require_player(player)?;
        let current = self.life_lost_this_turn(player)?;
        let updated =
            current
                .checked_add(amount)
                .ok_or(TurnEventStateError::ArithmeticOverflow {
                    operation: TurnEventOperation::LifeLoss,
                })?;
        if updated == 0 {
            self.life_lost_this_turn.remove(&player);
        } else {
            self.life_lost_this_turn.insert(player, updated);
        }
        Ok(updated)
    }

    pub(crate) fn life_lost_this_turn(&self, player: P) -> Result<u32, TurnEventStateError<P, O>> {
        self.require_player(player)?;
        Ok(self
            .life_lost_this_turn
            .get(&player)
            .copied()
            .unwrap_or_default())
    }

    pub(crate) fn add_counters(
        &mut self,
        object: O,
        counter: C,
        amount: u32,
    ) -> Result<u32, TurnEventStateError<P, O>> {
        self.require_object(object)?;
        let current = self.counter_count(object, &counter)?;
        let updated =
            current
                .checked_add(amount)
                .ok_or(TurnEventStateError::ArithmeticOverflow {
                    operation: TurnEventOperation::CounterAddition,
                })?;
        if updated > 0 {
            self.counters
                .entry(object)
                .or_default()
                .insert(counter, updated);
        }
        Ok(updated)
    }

    pub(crate) fn counter_count(
        &self,
        object: O,
        counter: &C,
    ) -> Result<u32, TurnEventStateError<P, O>> {
        self.require_object(object)?;
        Ok(self
            .counters
            .get(&object)
            .and_then(|counters| counters.get(counter))
            .copied()
            .unwrap_or_default())
    }

    pub(crate) fn counter_threshold_activation_eligible(
        &self,
        object: O,
        counter: &C,
        threshold: u32,
    ) -> Result<bool, TurnEventStateError<P, O>> {
        Ok(self.counter_count(object, counter)? >= threshold)
    }

    /// Record one end-step event only if the turn player appears in the
    /// caller-supplied active-player set.
    pub(crate) fn record_active_end_step(
        &mut self,
        turn_player: P,
        active_players: impl IntoIterator<Item = P>,
    ) -> Result<u32, TurnEventStateError<P, O>> {
        let active = self.validate_active_players(active_players)?;
        if !active.contains(&turn_player) {
            return Err(TurnEventStateError::InactiveTurnPlayer {
                player: turn_player,
            });
        }
        self.increment_end_step(turn_player)
    }

    /// Deterministic bounded helper for one complete set of active opponents'
    /// end steps. It records exactly one occurrence for every active player
    /// other than `controller`. The mutation is atomic if any counter would
    /// overflow.
    pub(crate) fn record_active_opponent_end_steps(
        &mut self,
        controller: P,
        active_players: impl IntoIterator<Item = P>,
    ) -> Result<u32, TurnEventStateError<P, O>> {
        self.require_player(controller)?;
        let active = self.validate_active_players(active_players)?;
        if !active.contains(&controller) {
            return Err(TurnEventStateError::InactiveController { controller });
        }
        let opponents = active
            .into_iter()
            .filter(|player| *player != controller)
            .collect::<Vec<_>>();
        let mut staged = Vec::with_capacity(opponents.len());
        for opponent in opponents {
            let current = self
                .end_step_occurrences_this_turn
                .get(&opponent)
                .copied()
                .unwrap_or_default();
            let updated =
                current
                    .checked_add(1)
                    .ok_or(TurnEventStateError::ArithmeticOverflow {
                        operation: TurnEventOperation::EndStepOccurrence,
                    })?;
            staged.push((opponent, updated));
        }
        let recorded =
            u32::try_from(staged.len()).map_err(|_| TurnEventStateError::ArithmeticOverflow {
                operation: TurnEventOperation::EndStepOccurrence,
            })?;
        for (opponent, updated) in &staged {
            self.end_step_occurrences_this_turn
                .insert(*opponent, *updated);
        }
        Ok(recorded)
    }

    pub(crate) fn end_step_occurrences_for(
        &self,
        turn_player: P,
    ) -> Result<u32, TurnEventStateError<P, O>> {
        self.require_player(turn_player)?;
        Ok(self
            .end_step_occurrences_this_turn
            .get(&turn_player)
            .copied()
            .unwrap_or_default())
    }

    pub(crate) fn active_opponent_end_step_occurrences(
        &self,
        controller: P,
    ) -> Result<u32, TurnEventStateError<P, O>> {
        self.require_player(controller)?;
        self.end_step_occurrences_this_turn
            .iter()
            .filter(|(player, _)| **player != controller)
            .try_fold(0u32, |total, (_, count)| {
                total
                    .checked_add(*count)
                    .ok_or(TurnEventStateError::ArithmeticOverflow {
                        operation: TurnEventOperation::EndStepQuery,
                    })
            })
    }

    /// Advance the state only after the sequence increment succeeds. Monarch
    /// and object counters persist; all per-turn observations are cleared.
    pub(crate) fn start_next_turn(&mut self) -> Result<u64, TurnEventStateError<P, O>> {
        let next =
            self.turn_sequence
                .checked_add(1)
                .ok_or(TurnEventStateError::ArithmeticOverflow {
                    operation: TurnEventOperation::TurnAdvance,
                })?;
        self.turn_sequence = next;
        self.life_lost_this_turn.clear();
        self.end_step_occurrences_this_turn.clear();
        Ok(next)
    }

    fn increment_end_step(&mut self, turn_player: P) -> Result<u32, TurnEventStateError<P, O>> {
        let current = self
            .end_step_occurrences_this_turn
            .get(&turn_player)
            .copied()
            .unwrap_or_default();
        let updated = current
            .checked_add(1)
            .ok_or(TurnEventStateError::ArithmeticOverflow {
                operation: TurnEventOperation::EndStepOccurrence,
            })?;
        self.end_step_occurrences_this_turn
            .insert(turn_player, updated);
        Ok(updated)
    }

    fn validate_active_players(
        &self,
        active_players: impl IntoIterator<Item = P>,
    ) -> Result<BTreeSet<P>, TurnEventStateError<P, O>> {
        let mut active = BTreeSet::new();
        for player in active_players {
            self.require_player(player)?;
            if !active.insert(player) {
                return Err(TurnEventStateError::DuplicateActivePlayer { player });
            }
        }
        Ok(active)
    }

    fn require_player(&self, player: P) -> Result<(), TurnEventStateError<P, O>> {
        if self.known_players.contains(&player) {
            Ok(())
        } else {
            Err(TurnEventStateError::UnknownPlayer { player })
        }
    }

    fn require_object(&self, object: O) -> Result<(), TurnEventStateError<P, O>> {
        if self.known_objects.contains(&object) {
            Ok(())
        } else {
            Err(TurnEventStateError::UnknownObject { object })
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum TurnEventStateError<P, O>
where
    P: Debug,
    O: Debug,
{
    #[error("player {player:?} appears more than once")]
    DuplicatePlayer { player: P },
    #[error("object {object:?} appears more than once")]
    DuplicateObject { object: O },
    #[error("player {player:?} is not registered")]
    UnknownPlayer { player: P },
    #[error("object {object:?} is not registered")]
    UnknownObject { object: O },
    #[error("active-player input repeats {player:?}")]
    DuplicateActivePlayer { player: P },
    #[error("turn player {player:?} is not active")]
    InactiveTurnPlayer { player: P },
    #[error("controller {controller:?} is not active")]
    InactiveController { controller: P },
    #[error("{operation:?} overflowed")]
    ArithmeticOverflow { operation: TurnEventOperation },
}
