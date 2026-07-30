use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::bounded_oracle_consumer::{
    ActionWindow, ExecutionContext, ExecutionError, InMemoryOracleState, ObjectId,
    OracleStateAdapter,
};
use crate::bounded_oracle_runtime::{
    Amount, BoundedOracleClause, Color, Effect, PlayerRef, Timing, Zone,
};
use crate::bounded_oracle_simulation::{
    BoundedOracleSimulation, clause_has_live_bridge_contract, insert_player,
    physical_object_from_compiled_card,
};
use crate::semantics::CompiledDeck;

const TRAJECTORY_PLAYER_ID: u8 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrajectoryCardRules {
    bounded_oracle: Vec<BoundedOracleClause>,
    context_free_draw_spell: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TrajectoryRulesCatalog {
    cards: Vec<TrajectoryCardRules>,
}

impl TrajectoryRulesCatalog {
    pub(super) fn compile(deck: &CompiledDeck) -> Self {
        Self {
            cards: deck
                .cards
                .iter()
                .map(|card| {
                    let spell_resolution_clauses = card
                        .effects
                        .bounded_oracle
                        .iter()
                        .filter(|clause| matches!(clause.timing(), Timing::SpellResolution))
                        .collect::<Vec<_>>();
                    let context_free_draw_spell = (card.effects.card_types.is_instant
                        || card.effects.card_types.is_sorcery)
                        && !spell_resolution_clauses.is_empty()
                        && spell_resolution_clauses
                            .iter()
                            .all(|clause| clause_is_context_free_draw(clause));
                    TrajectoryCardRules {
                        bounded_oracle: card.effects.bounded_oracle.clone(),
                        context_free_draw_spell,
                    }
                })
                .collect(),
        }
    }

    fn card(&self, card_index: usize) -> Result<&TrajectoryCardRules, TrajectoryRulesBridgeError> {
        self.cards
            .get(card_index)
            .ok_or(TrajectoryRulesBridgeError::InvalidCardIndex(card_index))
    }
}

fn clause_is_context_free_draw(clause: &BoundedOracleClause) -> bool {
    clause_has_live_bridge_contract(clause)
        && matches!(clause.timing(), Timing::SpellResolution)
        && clause.conditions().is_empty()
        && clause.costs().is_empty()
        && clause.targets().is_empty()
        && clause.activation_restriction().is_none()
        && !clause.effects().is_empty()
        && clause.effects().iter().all(|effect| {
            matches!(
                effect,
                Effect::Draw {
                    player: PlayerRef::You,
                    amount: Amount::Constant(amount),
                    optional: false,
                    delayed_until: None,
                } if *amount > 0
            )
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PhysicalCardRef {
    pub(super) object_id: ObjectId,
    pub(super) card_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrajectorySpellStatus {
    NotApplicable,
    Resolved,
    Countered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TrajectorySpellResolution {
    pub(super) status: TrajectorySpellStatus,
    pub(super) source: Option<PhysicalCardRef>,
    pub(super) drawn_cards: Vec<PhysicalCardRef>,
}

impl TrajectorySpellResolution {
    fn not_applicable() -> Self {
        Self {
            status: TrajectorySpellStatus::NotApplicable,
            source: None,
            drawn_cards: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TrajectoryRulesBridgeError {
    InvalidCardIndex(usize),
    InvalidLibraryPosition {
        position: usize,
        library_len: usize,
    },
    MissingPlayer,
    MissingPhysicalCard {
        card_index: usize,
        zone: Zone,
    },
    MissingObject(ObjectId),
    UnexpectedObjectZone {
        object_id: ObjectId,
        expected: Zone,
        actual: Zone,
    },
    LibraryOrderMismatch {
        position: usize,
        expected_card_index: usize,
        observed_card_index: usize,
    },
    PhysicalLibraryOrderMismatch {
        position: usize,
        expected_object_id: ObjectId,
        observed_object_id: ObjectId,
    },
    NonPrefixLibraryMutation,
    Adapter(String),
    Execution(ExecutionError),
}

impl From<ExecutionError> for TrajectoryRulesBridgeError {
    fn from(error: ExecutionError) -> Self {
        Self::Execution(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TrajectoryRulesBridge {
    catalog: Arc<TrajectoryRulesCatalog>,
    simulation: BoundedOracleSimulation,
    objects_by_card: HashMap<usize, Vec<ObjectId>>,
    cards_by_object: BTreeMap<ObjectId, usize>,
    physical_library_order: Vec<ObjectId>,
    observed_draw_position: usize,
}

impl TrajectoryRulesBridge {
    pub(super) fn new(
        catalog: Arc<TrajectoryRulesCatalog>,
        deck: &CompiledDeck,
        episode_hand: &[usize],
        episode_library_order: &[usize],
        starting_life: f32,
    ) -> Result<Self, TrajectoryRulesBridgeError> {
        let mut state = InMemoryOracleState::default();
        insert_player(
            &mut state,
            TRAJECTORY_PLAYER_ID,
            starting_life.round() as i64,
            commander_color_identity(deck),
        );

        let mut objects_by_card = HashMap::<usize, Vec<ObjectId>>::new();
        let mut cards_by_object = BTreeMap::<ObjectId, usize>::new();
        let mut physical_library_order = Vec::with_capacity(episode_library_order.len());
        let mut next_object_id = 1u64;

        for (zone, cards) in [
            (Zone::Hand, episode_hand),
            (Zone::Library, episode_library_order),
        ] {
            for &card_index in cards {
                let card = deck
                    .cards
                    .get(card_index)
                    .ok_or(TrajectoryRulesBridgeError::InvalidCardIndex(card_index))?;
                let object_id = next_object_id;
                next_object_id = next_object_id.saturating_add(1);
                state
                    .insert_object(physical_object_from_compiled_card(
                        object_id,
                        TRAJECTORY_PLAYER_ID,
                        TRAJECTORY_PLAYER_ID,
                        zone,
                        card,
                    ))
                    .map_err(TrajectoryRulesBridgeError::Adapter)?;
                objects_by_card
                    .entry(card_index)
                    .or_default()
                    .push(object_id);
                cards_by_object.insert(object_id, card_index);
                if zone == Zone::Library {
                    physical_library_order.push(object_id);
                }
            }
        }

        Ok(Self {
            catalog,
            simulation: BoundedOracleSimulation::new(state),
            objects_by_card,
            cards_by_object,
            physical_library_order,
            observed_draw_position: 0,
        })
    }

    pub(super) fn resolve_context_free_draw_spell(
        &mut self,
        card_index: usize,
        episode_hand: &mut Vec<usize>,
        episode_library_order: &[usize],
        episode_position: &mut usize,
    ) -> Result<TrajectorySpellResolution, TrajectoryRulesBridgeError> {
        if !self.catalog.card(card_index)?.context_free_draw_spell {
            return Ok(TrajectorySpellResolution::not_applicable());
        }

        let checkpoint = self.clone();
        let result = self.resolve_context_free_draw_spell_inner(
            card_index,
            episode_hand,
            episode_library_order,
            episode_position,
        );
        if result.is_err() {
            *self = checkpoint;
        }
        result
    }

    pub(super) fn counter_spell(
        &mut self,
        card_index: usize,
        episode_library_order: &[usize],
        episode_position: usize,
    ) -> Result<TrajectorySpellResolution, TrajectoryRulesBridgeError> {
        if !self.catalog.card(card_index)?.context_free_draw_spell {
            return Ok(TrajectorySpellResolution::not_applicable());
        }
        let checkpoint = self.clone();
        let result = (|| {
            self.observe_episode_draws_inner(episode_library_order, episode_position)?;
            let source = self.hand_object_for_card(card_index)?;
            self.move_object(source.object_id, Zone::Hand, Zone::Stack)?;
            self.move_object(source.object_id, Zone::Stack, Zone::Graveyard)?;
            Ok(TrajectorySpellResolution {
                status: TrajectorySpellStatus::Countered,
                source: Some(source),
                drawn_cards: Vec::new(),
            })
        })();
        if result.is_err() {
            *self = checkpoint;
        }
        result
    }

    fn resolve_context_free_draw_spell_inner(
        &mut self,
        card_index: usize,
        episode_hand: &mut Vec<usize>,
        episode_library_order: &[usize],
        episode_position: &mut usize,
    ) -> Result<TrajectorySpellResolution, TrajectoryRulesBridgeError> {
        self.observe_episode_draws_inner(episode_library_order, *episode_position)?;
        let source = self.hand_object_for_card(card_index)?;
        self.move_object(source.object_id, Zone::Hand, Zone::Stack)?;
        let clauses = self.catalog.card(card_index)?.bounded_oracle.clone();
        self.simulation.bind_program(source.object_id, clauses)?;

        let library_before = self.player_library()?.to_vec();
        self.simulation.resolve_spell(source.object_id, |_| {
            ExecutionContext::new(
                TRAJECTORY_PLAYER_ID,
                source.object_id,
                ActionWindow::SpellResolution,
            )
        })?;
        let library_after = self.player_library()?.to_vec();
        if library_before.len() < library_after.len()
            || !library_before.ends_with(library_after.as_slice())
        {
            return Err(TrajectoryRulesBridgeError::NonPrefixLibraryMutation);
        }

        let draw_count = library_before.len() - library_after.len();
        let drawn_object_ids = &library_before[..draw_count];
        let mut staged_hand = episode_hand.clone();
        let mut staged_position = *episode_position;
        let mut drawn_cards = Vec::with_capacity(draw_count);
        for &object_id in drawn_object_ids {
            let expected_object_id = self
                .physical_library_order
                .get(staged_position)
                .copied()
                .ok_or(TrajectoryRulesBridgeError::InvalidLibraryPosition {
                    position: staged_position,
                    library_len: self.physical_library_order.len(),
                })?;
            if object_id != expected_object_id {
                return Err(TrajectoryRulesBridgeError::PhysicalLibraryOrderMismatch {
                    position: staged_position,
                    expected_object_id,
                    observed_object_id: object_id,
                });
            }
            let card_index = self.card_for_object(object_id)?;
            let observed_card_index = episode_library_order.get(staged_position).copied().ok_or(
                TrajectoryRulesBridgeError::InvalidLibraryPosition {
                    position: staged_position,
                    library_len: episode_library_order.len(),
                },
            )?;
            if card_index != observed_card_index {
                return Err(TrajectoryRulesBridgeError::LibraryOrderMismatch {
                    position: staged_position,
                    expected_card_index: card_index,
                    observed_card_index,
                });
            }
            staged_hand.push(card_index);
            staged_position = staged_position.saturating_add(1);
            drawn_cards.push(PhysicalCardRef {
                object_id,
                card_index,
            });
        }

        self.move_object(source.object_id, Zone::Stack, Zone::Graveyard)?;
        self.observed_draw_position = staged_position;
        *episode_hand = staged_hand;
        *episode_position = staged_position;
        Ok(TrajectorySpellResolution {
            status: TrajectorySpellStatus::Resolved,
            source: Some(source),
            drawn_cards,
        })
    }

    fn observe_episode_draws_inner(
        &mut self,
        episode_library_order: &[usize],
        episode_position: usize,
    ) -> Result<(), TrajectoryRulesBridgeError> {
        if episode_position > episode_library_order.len()
            || episode_position > self.physical_library_order.len()
        {
            return Err(TrajectoryRulesBridgeError::InvalidLibraryPosition {
                position: episode_position,
                library_len: episode_library_order
                    .len()
                    .min(self.physical_library_order.len()),
            });
        }
        while self.observed_draw_position < episode_position {
            let position = self.observed_draw_position;
            let object_id = self.physical_library_order[position];
            let expected_card_index = self.card_for_object(object_id)?;
            let observed_card_index = episode_library_order[position];
            if expected_card_index != observed_card_index {
                return Err(TrajectoryRulesBridgeError::LibraryOrderMismatch {
                    position,
                    expected_card_index,
                    observed_card_index,
                });
            }
            let observed_object_id = self.player_library()?.first().copied().ok_or(
                TrajectoryRulesBridgeError::InvalidLibraryPosition {
                    position,
                    library_len: 0,
                },
            )?;
            if object_id != observed_object_id {
                return Err(TrajectoryRulesBridgeError::PhysicalLibraryOrderMismatch {
                    position,
                    expected_object_id: object_id,
                    observed_object_id,
                });
            }
            self.move_object(object_id, Zone::Library, Zone::Hand)?;
            self.observed_draw_position = self.observed_draw_position.saturating_add(1);
        }
        Ok(())
    }

    fn hand_object_for_card(
        &self,
        card_index: usize,
    ) -> Result<PhysicalCardRef, TrajectoryRulesBridgeError> {
        let object_id = self
            .objects_by_card
            .get(&card_index)
            .into_iter()
            .flatten()
            .copied()
            .find(|object_id| {
                self.simulation
                    .state()
                    .objects
                    .get(object_id)
                    .is_some_and(|object| object.zone == Zone::Hand)
            })
            .ok_or(TrajectoryRulesBridgeError::MissingPhysicalCard {
                card_index,
                zone: Zone::Hand,
            })?;
        Ok(PhysicalCardRef {
            object_id,
            card_index,
        })
    }

    fn move_object(
        &mut self,
        object_id: ObjectId,
        expected: Zone,
        destination: Zone,
    ) -> Result<(), TrajectoryRulesBridgeError> {
        let object = self
            .simulation
            .state()
            .objects
            .get(&object_id)
            .ok_or(TrajectoryRulesBridgeError::MissingObject(object_id))?;
        if object.zone != expected {
            return Err(TrajectoryRulesBridgeError::UnexpectedObjectZone {
                object_id,
                expected,
                actual: object.zone,
            });
        }
        self.simulation
            .state_mut()
            .move_object(object_id, destination)
            .map_err(TrajectoryRulesBridgeError::Adapter)
    }

    fn player_library(&self) -> Result<&[ObjectId], TrajectoryRulesBridgeError> {
        self.simulation
            .state()
            .players
            .get(&TRAJECTORY_PLAYER_ID)
            .map(|player| player.library.as_slice())
            .ok_or(TrajectoryRulesBridgeError::MissingPlayer)
    }

    fn card_for_object(&self, object_id: ObjectId) -> Result<usize, TrajectoryRulesBridgeError> {
        self.cards_by_object
            .get(&object_id)
            .copied()
            .ok_or(TrajectoryRulesBridgeError::MissingObject(object_id))
    }
}

fn commander_color_identity(deck: &CompiledDeck) -> Vec<Color> {
    let mut identity = Vec::new();
    for card_index in &deck.commanders {
        let Some(card) = deck.cards.get(*card_index) else {
            continue;
        };
        for color in &card.colors {
            let color = match color.trim().to_ascii_uppercase().as_str() {
                "W" | "WHITE" => Some(Color::White),
                "U" | "BLUE" => Some(Color::Blue),
                "B" | "BLACK" => Some(Color::Black),
                "R" | "RED" => Some(Color::Red),
                "G" | "GREEN" => Some(Color::Green),
                "C" | "COLORLESS" => Some(Color::Colorless),
                _ => None,
            };
            if let Some(color) = color
                && !identity.contains(&color)
            {
                identity.push(color);
            }
        }
    }
    identity
}
