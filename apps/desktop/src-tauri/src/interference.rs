// The normal-activity feed is intentionally landed as a typed foundation
// before the episode executor consumes it. Keep the staged API visible to
// strict builds in the same way as the staged executable ability program.
#![allow(dead_code)]

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::domain::InteractionProfile;

// Interaction rolls live on a stream that is intentionally independent from
// draws, mana-source reliability, and pilot decisions. Baseline and interfered
// episodes can therefore share the same gameplay seed without conditional
// opponent actions shifting the rest of the episode's random sequence.
const INTERACTION_STREAM_TAG: u64 = 0x4f50_504f_4e45_4e54;
// Normal table activity is present even in an interference-free goldfish. It
// therefore owns a separate stream: adding a draw, spell, or payment decision
// must never shift disruptive-interaction rolls (or vice versa).
const TABLE_ACTIVITY_STREAM_TAG: u64 = 0x5441_424c_455f_4143;
pub(crate) const OPPONENTS_PER_COMMANDER_TABLE: usize = 3;

#[derive(Debug, Clone, Copy)]
pub(crate) struct InteractionParameters {
    pub(crate) engine_disruption_chance: f64,
    pub(crate) attempt_stop_chance: f64,
    pub(crate) wipe_chance: f64,
    pub(crate) mana_pressure: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OpponentEventRolls {
    engine_disruption: f64,
    protection_response: f64,
    commander_hit: f64,
    board_wipe: f64,
    attempt_stop: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct OpponentEventTimeline {
    turns: Vec<OpponentEventRolls>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TableActivityTimeline {
    turns: Vec<TableTurnActivity>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TableTurnActivity {
    opponents: [OpponentTurnActivity; OPPONENTS_PER_COMMANDER_TABLE],
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpponentTurnActivity {
    opponent_index: u8,
    draws: Vec<OpponentDrawActivity>,
    spells: Vec<OpponentSpellActivity>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpponentDrawActivity {
    ordinal: u8,
    payment: TablePaymentDecision,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpponentSpellActivity {
    ordinal: u8,
    noncreature: bool,
    noncreature_ordinal: Option<u8>,
    payment: TablePaymentDecision,
}

/// One stable optional-payment choice attached to a concrete table event.
/// The same opportunity is monotonic by generic amount: an opponent willing
/// and able to pay four is also willing and able to pay one or two.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TablePaymentDecision {
    generic_mana_available: u8,
    willingness: f64,
}

impl OpponentEventTimeline {
    pub(crate) fn for_episode(episode_seed: u64, maximum_turn: u8) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(mix_seed(episode_seed ^ INTERACTION_STREAM_TAG));
        let turns = (0..maximum_turn)
            .map(|_| OpponentEventRolls {
                engine_disruption: rng.random(),
                protection_response: rng.random(),
                commander_hit: rng.random(),
                board_wipe: rng.random(),
                attempt_stop: rng.random(),
            })
            .collect();
        Self { turns }
    }

    pub(crate) fn turn(&self, turn: u8) -> Option<&OpponentEventRolls> {
        turn.checked_sub(1)
            .and_then(|index| self.turns.get(index as usize))
    }
}

impl TableActivityTimeline {
    pub(crate) fn for_episode(episode_seed: u64, maximum_turn: u8) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(mix_seed(episode_seed ^ TABLE_ACTIVITY_STREAM_TAG));
        let turns = (1..=maximum_turn)
            .map(|turn| TableTurnActivity {
                opponents: std::array::from_fn(|opponent_index| {
                    generate_opponent_turn_activity(
                        &mut rng,
                        turn,
                        u8::try_from(opponent_index)
                            .expect("a Commander table opponent index always fits in u8"),
                    )
                }),
            })
            .collect();
        Self { turns }
    }

    pub(crate) fn turn(&self, turn: u8) -> Option<&TableTurnActivity> {
        turn.checked_sub(1)
            .and_then(|index| self.turns.get(index as usize))
    }
}

impl TableTurnActivity {
    pub(crate) fn opponents(&self) -> &[OpponentTurnActivity; OPPONENTS_PER_COMMANDER_TABLE] {
        &self.opponents
    }

    pub(crate) fn total_draws(&self) -> u16 {
        self.opponents
            .iter()
            .map(|opponent| u16::from(opponent.draw_count()))
            .sum()
    }

    pub(crate) fn total_spells(&self) -> u16 {
        self.opponents
            .iter()
            .map(|opponent| u16::from(opponent.spell_count()))
            .sum()
    }

    pub(crate) fn total_noncreature_spells(&self) -> u16 {
        self.opponents
            .iter()
            .map(|opponent| u16::from(opponent.noncreature_spell_count()))
            .sum()
    }
}

impl OpponentTurnActivity {
    pub(crate) fn opponent_index(&self) -> u8 {
        self.opponent_index
    }

    pub(crate) fn draws(&self) -> &[OpponentDrawActivity] {
        &self.draws
    }

    pub(crate) fn spells(&self) -> &[OpponentSpellActivity] {
        &self.spells
    }

    pub(crate) fn draw_count(&self) -> u8 {
        u8::try_from(self.draws.len()).expect("bounded table draw count fits in u8")
    }

    pub(crate) fn spell_count(&self) -> u8 {
        u8::try_from(self.spells.len()).expect("bounded table spell count fits in u8")
    }

    pub(crate) fn noncreature_spell_count(&self) -> u8 {
        u8::try_from(
            self.spells
                .iter()
                .filter(|spell| spell.is_noncreature())
                .count(),
        )
        .expect("bounded noncreature spell count fits in u8")
    }

    pub(crate) fn first_spell(&self) -> Option<&OpponentSpellActivity> {
        self.spells.first()
    }

    pub(crate) fn second_spell(&self) -> Option<&OpponentSpellActivity> {
        self.spells.get(1)
    }

    pub(crate) fn first_noncreature_spell(&self) -> Option<&OpponentSpellActivity> {
        self.spells
            .iter()
            .find(|spell| spell.noncreature_ordinal() == Some(1))
    }
}

impl OpponentDrawActivity {
    pub(crate) fn ordinal(&self) -> u8 {
        self.ordinal
    }

    pub(crate) fn payment(&self) -> TablePaymentDecision {
        self.payment
    }
}

impl OpponentSpellActivity {
    pub(crate) fn ordinal(&self) -> u8 {
        self.ordinal
    }

    pub(crate) fn is_noncreature(&self) -> bool {
        self.noncreature
    }

    pub(crate) fn noncreature_ordinal(&self) -> Option<u8> {
        self.noncreature_ordinal
    }

    pub(crate) fn payment(&self) -> TablePaymentDecision {
        self.payment
    }
}

impl TablePaymentDecision {
    pub(crate) fn pays_generic(&self, amount: u16) -> bool {
        if amount == 0 {
            return true;
        }
        let Ok(amount) = u8::try_from(amount) else {
            return false;
        };
        if amount > self.generic_mana_available {
            return false;
        }
        self.willingness < payment_willingness_threshold(amount)
    }
}

fn generate_opponent_turn_activity(
    rng: &mut ChaCha8Rng,
    turn: u8,
    opponent_index: u8,
) -> OpponentTurnActivity {
    let draw_count = sampled_draw_count(turn, rng.random());
    let spell_count = sampled_spell_count(turn, rng.random());
    let draws = (1..=draw_count)
        .map(|ordinal| OpponentDrawActivity {
            ordinal,
            payment: sample_payment_decision(rng, turn),
        })
        .collect();
    let mut noncreature_count = 0u8;
    let spells = (1..=spell_count)
        .map(|ordinal| {
            let noncreature = rng.random::<f64>() < 0.70;
            let noncreature_ordinal = noncreature.then(|| {
                noncreature_count = noncreature_count.saturating_add(1);
                noncreature_count
            });
            OpponentSpellActivity {
                ordinal,
                noncreature,
                noncreature_ordinal,
                payment: sample_payment_decision(rng, turn),
            }
        })
        .collect();
    OpponentTurnActivity {
        opponent_index,
        draws,
        spells,
    }
}

fn sampled_draw_count(turn: u8, roll: f64) -> u8 {
    let (one_draw_ceiling, two_draw_ceiling) = match turn {
        0 | 1 => (0.90, 0.985),
        2 => (0.84, 0.975),
        _ => (0.76, 0.955),
    };
    if roll < one_draw_ceiling {
        1
    } else if roll < two_draw_ceiling {
        2
    } else {
        3
    }
}

fn sampled_spell_count(turn: u8, roll: f64) -> u8 {
    let ceilings = match turn {
        0 | 1 => [0.15, 0.60, 0.90, 0.98],
        2 => [0.08, 0.40, 0.78, 0.95],
        _ => [0.06, 0.34, 0.74, 0.94],
    };
    if roll < ceilings[0] {
        0
    } else if roll < ceilings[1] {
        1
    } else if roll < ceilings[2] {
        2
    } else if roll < ceilings[3] {
        3
    } else {
        4
    }
}

fn sample_payment_decision(rng: &mut ChaCha8Rng, turn: u8) -> TablePaymentDecision {
    let acceleration = match rng.random::<f64>() {
        roll if roll < 0.12 => 2,
        roll if roll < 0.38 => 1,
        _ => 0,
    };
    TablePaymentDecision {
        generic_mana_available: turn.saturating_add(acceleration).min(8),
        willingness: rng.random(),
    }
}

fn payment_willingness_threshold(amount: u8) -> f64 {
    match amount {
        0 => 1.0,
        1 => 0.46,
        2 => 0.28,
        3 => 0.14,
        _ => 0.06,
    }
}

impl OpponentEventRolls {
    pub(crate) fn disrupts_engine(&self, profile: InteractionProfile) -> bool {
        self.engine_disruption < interaction_parameters(profile).engine_disruption_chance
    }

    pub(crate) fn protection_prevents_engine_disruption(&self) -> bool {
        self.protection_response < 0.55
    }

    pub(crate) fn hits_commander(&self) -> bool {
        self.commander_hit < 0.45
    }

    pub(crate) fn wipes_board(&self, profile: InteractionProfile) -> bool {
        self.board_wipe < interaction_parameters(profile).wipe_chance
    }

    pub(crate) fn stops_attempt(&self, profile: InteractionProfile, protection_count: u8) -> bool {
        self.attempt_stop
            < interaction_parameters(profile).attempt_stop_chance
                * protection_stop_multiplier(protection_count)
    }
}

pub(crate) fn interaction_parameters(profile: InteractionProfile) -> InteractionParameters {
    match profile {
        InteractionProfile::None => InteractionParameters {
            engine_disruption_chance: 0.0,
            attempt_stop_chance: 0.0,
            wipe_chance: 0.0,
            mana_pressure: 0,
        },
        InteractionProfile::Light => InteractionParameters {
            engine_disruption_chance: 0.05,
            attempt_stop_chance: 0.18,
            wipe_chance: 0.025,
            mana_pressure: 0,
        },
        InteractionProfile::Typical => InteractionParameters {
            engine_disruption_chance: 0.10,
            attempt_stop_chance: 0.42,
            wipe_chance: 0.055,
            mana_pressure: 0,
        },
        InteractionProfile::HighPower => InteractionParameters {
            engine_disruption_chance: 0.16,
            attempt_stop_chance: 0.67,
            wipe_chance: 0.075,
            mana_pressure: 1,
        },
    }
}

fn protection_stop_multiplier(protection_count: u8) -> f64 {
    match protection_count {
        0 => 1.0,
        1 => 0.72,
        _ => 0.55,
    }
}

fn mix_seed(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
