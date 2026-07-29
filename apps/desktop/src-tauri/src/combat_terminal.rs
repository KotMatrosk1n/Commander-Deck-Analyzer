//! Rules-backed Commander combat and terminal-state evaluation.
//!
//! This module deliberately separates a *presented* attack (the declared
//! attackers would eliminate every remaining opponent if their assigned
//! damage connects) from a *resolved* table win (combat damage was explicitly
//! supplied and state-based actions eliminated every opponent).
//!
//! Rules basis:
//! - CR 104.2a: a player wins once all opponents have left the game.
//! - CR 104.3b: a player with 0 or less life loses as a state-based action.
//! - CR 104.3c: a player who was required to draw more cards than remained in
//!   their library loses the game.
//! - CR 104.3j: a Commander player dealt 21 or more combat damage by the same
//!   commander loses as a state-based action.
//! - CR 704.5b: the empty-library draw loss is applied at the next
//!   state-based-action checkpoint.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const OPPONENT_COUNT: usize = 3;
pub const COMMANDER_STARTING_LIFE: i64 = 40;
pub const COMMANDER_DAMAGE_LOSS_THRESHOLD: u32 = 21;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpponentId(u8);

impl OpponentId {
    pub const FIRST: Self = Self(0);
    pub const SECOND: Self = Self(1);
    pub const THIRD: Self = Self(2);
    pub const ALL: [Self; OPPONENT_COUNT] = [Self::FIRST, Self::SECOND, Self::THIRD];

    pub fn new(index: usize) -> Result<Self, CombatTerminalError> {
        if index < OPPONENT_COUNT {
            Ok(Self(index as u8))
        } else {
            Err(CombatTerminalError::InvalidOpponentIndex { index })
        }
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl TryFrom<usize> for OpponentId {
    type Error = CombatTerminalError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalRule {
    Cr104_2aAllOpponentsLeft,
    Cr104_3bLifeTotal,
    Cr104_3cRequiredDrawExceedsLibrary,
    Cr104_3jCommanderDamage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LossRules {
    pub life_total: bool,
    #[serde(default)]
    pub required_draw_exceeds_library: bool,
    pub commander_damage: bool,
}

impl LossRules {
    pub const fn primary_rule(self) -> TerminalRule {
        if self.life_total {
            TerminalRule::Cr104_3bLifeTotal
        } else if self.required_draw_exceeds_library {
            TerminalRule::Cr104_3cRequiredDrawExceedsLibrary
        } else {
            TerminalRule::Cr104_3jCommanderDamage
        }
    }

    pub fn rules(self) -> impl Iterator<Item = TerminalRule> {
        [
            self.life_total.then_some(TerminalRule::Cr104_3bLifeTotal),
            self.required_draw_exceeds_library
                .then_some(TerminalRule::Cr104_3cRequiredDrawExceedsLibrary),
            self.commander_damage
                .then_some(TerminalRule::Cr104_3jCommanderDamage),
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum OpponentTerminalStatus {
    Active,
    AlreadyLeft,
    LosesAsStateBasedAction { rules: LossRules },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "endpoint")]
pub enum TerminalEndpoint {
    InProgress { opponents_remaining: u8 },
    TableWin { rule: TerminalRule },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalEvaluation {
    pub opponents: [OpponentTerminalStatus; OPPONENT_COUNT],
    pub endpoint: TerminalEndpoint,
}

impl TerminalEvaluation {
    pub const fn is_table_win(&self) -> bool {
        matches!(self.endpoint, TerminalEndpoint::TableWin { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpponentState {
    life_total: i64,
    commander_combat_damage: u32,
    #[serde(default)]
    attempted_required_draw_from_empty_library: bool,
    has_left_game: bool,
}

impl Default for OpponentState {
    fn default() -> Self {
        Self {
            life_total: COMMANDER_STARTING_LIFE,
            commander_combat_damage: 0,
            attempted_required_draw_from_empty_library: false,
            has_left_game: false,
        }
    }
}

impl OpponentState {
    pub const fn life_total(&self) -> i64 {
        self.life_total
    }

    pub const fn commander_combat_damage(&self) -> u32 {
        self.commander_combat_damage
    }

    pub const fn attempted_required_draw_from_empty_library(&self) -> bool {
        self.attempted_required_draw_from_empty_library
    }

    pub const fn has_left_game(&self) -> bool {
        self.has_left_game
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommanderCombatState {
    opponents: [OpponentState; OPPONENT_COUNT],
}

impl Default for CommanderCombatState {
    fn default() -> Self {
        Self::new()
    }
}

impl CommanderCombatState {
    /// Create the analyzer's fixed four-player Commander table: the analyzed
    /// player plus exactly three opponents, each at 40 life.
    pub fn new() -> Self {
        Self {
            opponents: std::array::from_fn(|_| OpponentState::default()),
        }
    }

    pub fn opponent(&self, opponent: OpponentId) -> &OpponentState {
        &self.opponents[opponent.index()]
    }

    /// Report whether an opponent can still take game actions. This is a
    /// read-only projection so other bounded simulator layers can suppress
    /// activity from opponents that have already lost or left the game.
    pub(crate) fn is_opponent_active(&self, opponent: OpponentId) -> bool {
        opponent_is_active(self.opponent(opponent))
    }

    /// Count opponents that can still take game actions.
    pub(crate) fn active_opponent_count(&self) -> usize {
        OpponentId::ALL
            .into_iter()
            .filter(|opponent| self.is_opponent_active(*opponent))
            .count()
    }

    pub fn set_life_total(&mut self, opponent: OpponentId, life_total: i64) {
        self.opponents[opponent.index()].life_total = life_total;
    }

    /// Record combat damage from the one commander identity tracked by this
    /// state. Partner commanders require distinct tracked states because CR
    /// 104.3j counts damage from each commander separately.
    pub fn record_commander_combat_damage(&mut self, opponent: OpponentId, amount: u32) {
        let state = &mut self.opponents[opponent.index()];
        state.commander_combat_damage = state.commander_combat_damage.saturating_add(amount);
    }

    /// Mark an opponent as having left for a reason outside the combat model,
    /// such as an explicit lose-the-game effect or concession.
    pub fn mark_opponent_left(&mut self, opponent: OpponentId) {
        self.opponents[opponent.index()].has_left_game = true;
    }

    /// Record CR 104.3c after a modeled draw instruction could not be
    /// completed, then immediately apply the next CR 704.5b checkpoint.
    ///
    /// Emptying a library alone must not call this method: the loss exists
    /// only after a later required draw is attempted.
    pub(crate) fn record_required_draw_from_empty_library(
        &mut self,
        opponent: OpponentId,
    ) -> Result<TerminalEvaluation, CombatTerminalError> {
        if !self.is_opponent_active(opponent) {
            return Err(CombatTerminalError::OpponentAlreadyLeft { opponent });
        }
        let mut staged = self.clone();
        staged.opponents[opponent.index()].attempted_required_draw_from_empty_library = true;
        let terminal = staged.apply_state_based_actions();
        *self = staged;
        Ok(terminal)
    }

    /// Evaluate the next state-based-action checkpoint without mutating state.
    pub fn evaluate_terminal(&self) -> TerminalEvaluation {
        let opponents = std::array::from_fn(|index| {
            let opponent = &self.opponents[index];
            if opponent.has_left_game {
                OpponentTerminalStatus::AlreadyLeft
            } else {
                let rules = LossRules {
                    life_total: opponent.life_total <= 0,
                    required_draw_exceeds_library: opponent
                        .attempted_required_draw_from_empty_library,
                    commander_damage: opponent.commander_combat_damage
                        >= COMMANDER_DAMAGE_LOSS_THRESHOLD,
                };
                if rules.life_total || rules.required_draw_exceeds_library || rules.commander_damage
                {
                    OpponentTerminalStatus::LosesAsStateBasedAction { rules }
                } else {
                    OpponentTerminalStatus::Active
                }
            }
        });
        let opponents_remaining = opponents
            .iter()
            .filter(|status| matches!(status, OpponentTerminalStatus::Active))
            .count() as u8;
        let endpoint = if opponents_remaining == 0 {
            TerminalEndpoint::TableWin {
                rule: TerminalRule::Cr104_2aAllOpponentsLeft,
            }
        } else {
            TerminalEndpoint::InProgress {
                opponents_remaining,
            }
        };
        TerminalEvaluation {
            opponents,
            endpoint,
        }
    }

    /// Apply state-based losses and then evaluate CR 104.2a.
    pub fn apply_state_based_actions(&mut self) -> TerminalEvaluation {
        let evaluation = self.evaluate_terminal();
        for (opponent, status) in self.opponents.iter_mut().zip(&evaluation.opponents) {
            if matches!(
                status,
                OpponentTerminalStatus::LosesAsStateBasedAction { .. }
            ) {
                opponent.has_left_game = true;
            }
        }
        evaluation
    }

    /// Resolve only the combat damage explicitly supplied by the caller.
    /// Omitted attackers dealt no player damage; a presented attack alone can
    /// never produce a resolved table win.
    pub fn resolve_presented_attack(
        &mut self,
        attack: &PresentedAttack,
        connected_damage: &[ConnectedCombatDamage],
    ) -> Result<CombatResolution, CombatTerminalError> {
        if self.evaluate_terminal().is_table_win() {
            return Err(CombatTerminalError::GameAlreadyEnded);
        }

        let assignments = attack
            .assignments
            .iter()
            .map(|assignment| (assignment.attacker_id, assignment))
            .collect::<BTreeMap<_, _>>();
        let mut staged = self.clone();
        let mut seen = BTreeSet::new();
        for damage in connected_damage {
            if !seen.insert(damage.attacker_id) {
                return Err(CombatTerminalError::DuplicateResolvedAttacker {
                    attacker_id: damage.attacker_id,
                });
            }
            let assignment = assignments.get(&damage.attacker_id).ok_or(
                CombatTerminalError::UnknownResolvedAttacker {
                    attacker_id: damage.attacker_id,
                },
            )?;
            if damage.combat_damage > assignment.assigned_combat_damage {
                return Err(CombatTerminalError::DamageExceedsPresentation {
                    attacker_id: damage.attacker_id,
                    presented: assignment.assigned_combat_damage,
                    resolved: damage.combat_damage,
                });
            }
            let opponent = &mut staged.opponents[assignment.opponent.index()];
            if opponent.has_left_game {
                return Err(CombatTerminalError::OpponentAlreadyLeft {
                    opponent: assignment.opponent,
                });
            }
            opponent.life_total = opponent
                .life_total
                .saturating_sub(i64::from(damage.combat_damage));
            if assignment.is_tracked_commander {
                opponent.commander_combat_damage = opponent
                    .commander_combat_damage
                    .saturating_add(damage.combat_damage);
            }
        }

        let terminal = staged.apply_state_based_actions();
        *self = staged;
        Ok(CombatResolution {
            resolved_table_win: terminal.is_table_win(),
            terminal,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatAttacker {
    pub attacker_id: u32,
    /// Player combat damage this attacker is currently projected to deal if
    /// it connects. This is damage, not a creature-count proxy.
    pub projected_combat_damage: u32,
    /// True only for the physical commander card tracked by this state.
    pub is_tracked_commander: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackAssignment {
    pub attacker_id: u32,
    pub opponent: OpponentId,
    pub assigned_combat_damage: u32,
    pub is_tracked_commander: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentedAttack {
    assignments: Vec<AttackAssignment>,
    projected_terminal: TerminalEvaluation,
    presents_table_lethal: bool,
}

impl PresentedAttack {
    pub fn assignments(&self) -> &[AttackAssignment] {
        &self.assignments
    }

    pub const fn projected_terminal(&self) -> &TerminalEvaluation {
        &self.projected_terminal
    }

    /// True means the declared assignment would be table-lethal if every
    /// assigned damage packet connects. It is not a resolved table win.
    pub const fn presents_table_lethal(&self) -> bool {
        self.presents_table_lethal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedCombatDamage {
    pub attacker_id: u32,
    pub combat_damage: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombatResolution {
    pub terminal: TerminalEvaluation,
    pub resolved_table_win: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CombatTerminalError {
    #[error("opponent index {index} is outside the fixed three-opponent table")]
    InvalidOpponentIndex { index: usize },
    #[error("attacker id {attacker_id} appears more than once")]
    DuplicateAttacker { attacker_id: u32 },
    #[error("attacker id {attacker_id} has zero projected combat damage")]
    ZeroDamageAttacker { attacker_id: u32 },
    #[error("only one physical tracked commander can be declared as an attacker")]
    MultipleTrackedCommanderAttackers,
    #[error("the game has already reached a table-win terminal state")]
    GameAlreadyEnded,
    #[error("resolved attacker id {attacker_id} was not in the presented attack")]
    UnknownResolvedAttacker { attacker_id: u32 },
    #[error("resolved attacker id {attacker_id} appears more than once")]
    DuplicateResolvedAttacker { attacker_id: u32 },
    #[error(
        "attacker id {attacker_id} resolved {resolved} damage, above its presented {presented}"
    )]
    DamageExceedsPresentation {
        attacker_id: u32,
        presented: u32,
        resolved: u32,
    },
    #[error("opponent {opponent:?} had already left the game")]
    OpponentAlreadyLeft { opponent: OpponentId },
}

/// Deterministically assign attack-capable damage across the three opponents.
///
/// The allocator first searches for an exact all-opponents-lethal assignment.
/// It may hold attackers back, and it reasons from combat damage plus current
/// life or commander-damage state, not from a creature-count threshold. When no
/// table-lethal assignment exists, it focuses damage on the nearest legal
/// elimination in stable opponent/attacker order.
pub fn allocate_attack(
    state: &CommanderCombatState,
    attackers: &[CombatAttacker],
) -> Result<PresentedAttack, CombatTerminalError> {
    if state.evaluate_terminal().is_table_win() {
        return Err(CombatTerminalError::GameAlreadyEnded);
    }
    validate_attackers(attackers)?;

    let mut attackers = attackers.to_vec();
    attackers.sort_by_key(|attacker| attacker.attacker_id);
    let commander = attackers
        .iter()
        .copied()
        .find(|attacker| attacker.is_tracked_commander);
    let regular = attackers
        .iter()
        .copied()
        .filter(|attacker| !attacker.is_tracked_commander)
        .collect::<Vec<_>>();

    if let Some(assignments) = exact_table_lethal_assignment(state, commander, &regular) {
        return Ok(build_presented_attack(state, assignments));
    }

    let mut projected = state.clone();
    let mut assignments = Vec::new();
    for attacker in attackers {
        let target = nearest_live_target(&projected, attacker.is_tracked_commander);
        let Some(opponent) = target else {
            break;
        };
        apply_projected_damage(
            &mut projected,
            opponent,
            attacker.projected_combat_damage,
            attacker.is_tracked_commander,
        );
        projected.apply_state_based_actions();
        assignments.push(AttackAssignment {
            attacker_id: attacker.attacker_id,
            opponent,
            assigned_combat_damage: attacker.projected_combat_damage,
            is_tracked_commander: attacker.is_tracked_commander,
        });
    }
    Ok(build_presented_attack(state, assignments))
}

fn validate_attackers(attackers: &[CombatAttacker]) -> Result<(), CombatTerminalError> {
    let mut ids = BTreeSet::new();
    let mut commander_count = 0usize;
    for attacker in attackers {
        if !ids.insert(attacker.attacker_id) {
            return Err(CombatTerminalError::DuplicateAttacker {
                attacker_id: attacker.attacker_id,
            });
        }
        if attacker.projected_combat_damage == 0 {
            return Err(CombatTerminalError::ZeroDamageAttacker {
                attacker_id: attacker.attacker_id,
            });
        }
        commander_count += usize::from(attacker.is_tracked_commander);
    }
    if commander_count > 1 {
        return Err(CombatTerminalError::MultipleTrackedCommanderAttackers);
    }
    Ok(())
}

fn exact_table_lethal_assignment(
    state: &CommanderCombatState,
    commander: Option<CombatAttacker>,
    regular: &[CombatAttacker],
) -> Option<Vec<AttackAssignment>> {
    let active = OpponentId::ALL
        .into_iter()
        .filter(|opponent| opponent_is_active(state.opponent(*opponent)))
        .collect::<Vec<_>>();
    let commander_targets = commander
        .map(|_| {
            let mut targets = active.iter().copied().map(Some).collect::<Vec<_>>();
            targets.push(None);
            targets
        })
        .unwrap_or_else(|| vec![None]);

    for commander_target in commander_targets {
        let mut requirements = std::array::from_fn(|index| {
            let opponent = &state.opponents[index];
            if opponent.has_left_game {
                0
            } else {
                u32::try_from(opponent.life_total.max(0)).unwrap_or(u32::MAX)
            }
        });
        let mut assignments = Vec::new();

        if let (Some(commander), Some(target)) = (commander, commander_target) {
            let target_state = state.opponent(target);
            let commander_lethal = target_state
                .commander_combat_damage
                .saturating_add(commander.projected_combat_damage)
                >= COMMANDER_DAMAGE_LOSS_THRESHOLD;
            requirements[target.index()] = if commander_lethal {
                0
            } else {
                requirements[target.index()].saturating_sub(commander.projected_combat_damage)
            };
            assignments.push(AttackAssignment {
                attacker_id: commander.attacker_id,
                opponent: target,
                assigned_combat_damage: commander.projected_combat_damage,
                is_tracked_commander: true,
            });
        }

        let total_required = requirements
            .iter()
            .map(|value| u64::from(*value))
            .sum::<u64>();
        let total_available = regular
            .iter()
            .map(|attacker| u64::from(attacker.projected_combat_damage))
            .sum::<u64>();
        if total_available < total_required {
            continue;
        }

        let target = requirements;
        let mut paths = BTreeMap::from([([0u32; OPPONENT_COUNT], Vec::<u8>::new())]);
        for attacker in regular {
            let mut next = BTreeMap::new();
            for (damage, choices) in paths {
                let mut held = choices.clone();
                held.push(0);
                next.entry(damage).or_insert(held);

                for opponent in OpponentId::ALL {
                    if target[opponent.index()] == 0 {
                        continue;
                    }
                    let mut updated = damage;
                    updated[opponent.index()] = updated[opponent.index()]
                        .saturating_add(attacker.projected_combat_damage)
                        .min(target[opponent.index()]);
                    if updated == damage {
                        continue;
                    }
                    let mut assigned = choices.clone();
                    assigned.push(opponent.0 + 1);
                    next.entry(updated).or_insert(assigned);
                }
            }
            paths = next;
            if paths.contains_key(&target) {
                break;
            }
        }

        let Some(choices) = paths.get(&target) else {
            continue;
        };
        assignments.extend(
            regular
                .iter()
                .zip(choices)
                .filter_map(|(attacker, choice)| {
                    if *choice == 0 {
                        return None;
                    }
                    Some(AttackAssignment {
                        attacker_id: attacker.attacker_id,
                        opponent: OpponentId(*choice - 1),
                        assigned_combat_damage: attacker.projected_combat_damage,
                        is_tracked_commander: false,
                    })
                }),
        );
        assignments.sort_by_key(|assignment| assignment.attacker_id);
        return Some(assignments);
    }
    None
}

fn nearest_live_target(state: &CommanderCombatState, commander_damage: bool) -> Option<OpponentId> {
    OpponentId::ALL
        .into_iter()
        .filter(|opponent| opponent_is_active(state.opponent(*opponent)))
        .min_by_key(|opponent| {
            let state = state.opponent(*opponent);
            let life_deficit = u64::try_from(state.life_total.max(0)).unwrap_or(u64::MAX);
            let effective_deficit = if commander_damage {
                life_deficit.min(u64::from(
                    COMMANDER_DAMAGE_LOSS_THRESHOLD.saturating_sub(state.commander_combat_damage),
                ))
            } else {
                life_deficit
            };
            (effective_deficit, opponent.index())
        })
}

fn opponent_is_active(opponent: &OpponentState) -> bool {
    !opponent.has_left_game
        && opponent.life_total > 0
        && !opponent.attempted_required_draw_from_empty_library
        && opponent.commander_combat_damage < COMMANDER_DAMAGE_LOSS_THRESHOLD
}

fn build_presented_attack(
    state: &CommanderCombatState,
    assignments: Vec<AttackAssignment>,
) -> PresentedAttack {
    let mut projected = state.clone();
    for assignment in &assignments {
        apply_projected_damage(
            &mut projected,
            assignment.opponent,
            assignment.assigned_combat_damage,
            assignment.is_tracked_commander,
        );
    }
    let projected_terminal = projected.evaluate_terminal();
    PresentedAttack {
        presents_table_lethal: projected_terminal.is_table_win(),
        assignments,
        projected_terminal,
    }
}

fn apply_projected_damage(
    state: &mut CommanderCombatState,
    opponent: OpponentId,
    damage: u32,
    commander_damage: bool,
) {
    let opponent = &mut state.opponents[opponent.index()];
    opponent.life_total = opponent.life_total.saturating_sub(i64::from(damage));
    if commander_damage {
        opponent.commander_combat_damage = opponent.commander_combat_damage.saturating_add(damage);
    }
}
