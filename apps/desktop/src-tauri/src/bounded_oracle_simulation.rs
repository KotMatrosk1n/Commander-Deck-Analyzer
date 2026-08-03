//! Simulation-facing bridge for the bounded Oracle executor.
//!
//! The parser and consumer deliberately do not know about the trajectory
//! simulator. This module is the narrow integration layer between them. It
//! binds compiled clauses to stable physical object identities, exposes the
//! simulator's resolution and event entry points, and returns a deterministic
//! typed delta after every committed batch.

use std::collections::{BTreeMap, BTreeSet};

use crate::bounded_oracle_consumer::{
    ActionDefinition, ActionWindow, ActivationReductionRecord, AttachmentRecord,
    CastPermissionRecord, ContinuousEffectRecord, DelayedTriggerRecord, ExecutionContext,
    ExecutionError, ExecutionReceipt, ExtraTurnRecord, GameResultRecord, InMemoryOracleState,
    ObjectCharacteristics, ObjectId, OracleStateAdapter, PaymentOrLoseRecord, PhysicalObject,
    PlayerId, PlayerState, ReplacementRecord, RestrictionRecord, ScheduledCopyRecord,
    SkippedStepRecord, SpellReductionRecord, TriggerEvent, clause_has_executable_contract,
    effective_object, execute_action, execute_clause, execute_granted_ability, object_can_attack,
    object_can_be_blocked, object_can_block, object_can_untap_during,
    object_must_attack_each_combat, pay_reduced_spell_mana_cost, trigger_matches,
};
use crate::bounded_oracle_runtime::{
    AttachmentKind, BoundedOracleClause, CardType, ClauseAddress, Color, Condition, Keyword, Step,
    Supertype, Timing, Zone,
};
use crate::keyword_production_bridge::{
    COMBAT_EVASION_PRODUCTION_BRIDGE_VERSION, CombatEvasionProductionBridgeError,
    StaticKeywordObjectBinding, evaluate_combat_evasion_keywords,
    validate_combat_evasion_program_set,
};
use crate::keyword_rules_runtime::{
    CardType as KeywordCardType, KeywordProgram, KeywordReceipt, ManaColor as KeywordManaColor,
    ObjectCharacteristics as KeywordObjectCharacteristics, ObjectId as KeywordObjectId,
    OfficialKeyword, PlayerId as KeywordPlayerId, Zone as KeywordZone,
};
use crate::oracle_clause_backend::{
    DelegatedKeywordClause, LiveBridgeCapability, ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION,
};
use crate::printed_cost_runtime::{
    PRINTED_COST_PAYMENT_BRIDGE_VERSION, PrintedManaCost, PrintedManaPaymentChoices,
    PrintedManaPaymentError, PrintedManaPaymentReceipt, PrintedManaPaymentResources,
    pay_printed_mana_cost, printed_mana_cost_has_exact_payment_contract,
};
use crate::semantics::CompiledCard;

pub const BOUNDED_ORACLE_SIMULATION_BRIDGE_VERSION: &str = "bounded-oracle-simulation-bridge-0.5";
pub(crate) const COMBAT_BLOCK_DECLARATION_PRODUCTION_BRIDGE_VERSION: &str =
    "bounded-combat-block-declaration-bridge/v1";

const COMBAT_BLOCK_LEGALITY_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatBlockLegality,
];

pub fn clause_has_live_bridge_contract(clause: &BoundedOracleClause) -> bool {
    clause_has_executable_contract(clause)
        && matches!(
            clause.timing(),
            Timing::CastingAdditionalCost
                | Timing::SpellResolution
                | Timing::Activated
                | Timing::Triggered(_)
                | Timing::TriggeredModalHeader { .. }
                | Timing::Static
                | Timing::Replacement
                | Timing::ModalHeader { .. }
                | Timing::ModalBranch { .. }
                | Timing::SpecialAction(_)
        )
}

pub(crate) fn printed_cost_has_live_bridge_contract(cost: &PrintedManaCost) -> bool {
    printed_mana_cost_has_exact_payment_contract(cost)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrintedCostPaymentBatch {
    pub bridge_version: &'static str,
    pub receipt: PrintedManaPaymentReceipt,
    pub resources_before: PrintedManaPaymentResources,
    pub resources_after: PrintedManaPaymentResources,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrintedCostSimulationError {
    InvalidCostContract,
    Payment(PrintedManaPaymentError),
}

/// Pays one selected printed-cost face through the exact staged-payment
/// runtime. The simulator-facing resource state changes only after the
/// complete cost succeeds.
pub(crate) fn execute_printed_cost_payment(
    cost: &PrintedManaCost,
    face_index: usize,
    choices: &PrintedManaPaymentChoices,
    resources: &mut PrintedManaPaymentResources,
) -> Result<PrintedCostPaymentBatch, PrintedCostSimulationError> {
    if !printed_cost_has_live_bridge_contract(cost) {
        return Err(PrintedCostSimulationError::InvalidCostContract);
    }
    let resources_before = resources.clone();
    let mut staged = resources_before.clone();
    let receipt = pay_printed_mana_cost(cost, face_index, choices, &mut staged)
        .map_err(PrintedCostSimulationError::Payment)?;
    let resources_after = staged.clone();
    *resources = staged;
    Ok(PrintedCostPaymentBatch {
        bridge_version: PRINTED_COST_PAYMENT_BRIDGE_VERSION,
        receipt,
        resources_before,
        resources_after,
    })
}

/// Stable binding between one physical object and its exact compiled program.
///
/// A copy receives its own `ObjectId`; `origin_id` on `PhysicalObject` retains
/// the shared physical-card lineage. Zone changes never replace either value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundObjectProgram {
    pub object_id: ObjectId,
    pub clauses: Vec<BoundedOracleClause>,
}

/// Exact delegated combat clauses bound to one physical object identity.
///
/// Clause addresses stay attached to the object across zone and control
/// changes. The query activates only clauses on the object's current face and
/// still requires the object to be a battlefield creature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCombatEvasionProgram {
    pub object_id: ObjectId,
    pub clauses: Vec<DelegatedKeywordClause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedBoundedProgramOccurrence {
    pub address: ClauseAddress,
    pub semantic_digest: String,
    pub blocker_code: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledCardProgramBinding {
    pub object_id: ObjectId,
    pub bounded_clause_addresses: Vec<ClauseAddress>,
    pub combat_evasion_clause_addresses: Vec<ClauseAddress>,
    pub blocked_bounded_clauses: Vec<BlockedBoundedProgramOccurrence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CombatBlockDeclarationResult {
    pub bridge_version: &'static str,
    pub kernel_bridge_version: &'static str,
    pub attacker: ObjectId,
    pub blocker: ObjectId,
    pub defending_player: PlayerId,
    pub attacker_clause_addresses: Vec<ClauseAddress>,
    pub blocker_clause_addresses: Vec<ClauseAddress>,
    pub attacker_keyword_receipts: Vec<KeywordReceipt>,
    pub blocker_keyword_receipts: Vec<KeywordReceipt>,
    pub legal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CombatBlockDeclarationError {
    MissingPlayer(PlayerId),
    MissingObject(ObjectId),
    ProgramFaceUnavailable {
        object: ObjectId,
        face_index: u16,
    },
    ProgramFaceMustBeCreature {
        object: ObjectId,
        face_index: u16,
    },
    InexactDelegatedProgram {
        object: ObjectId,
        address: ClauseAddress,
    },
    MissingActiveCombatEvasionProgram(ObjectId),
    CharacteristicOutOfRange(ObjectId),
    EffectiveState(ExecutionError),
    KeywordBridge(CombatEvasionProductionBridgeError),
}

impl std::fmt::Display for CombatBlockDeclarationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CombatBlockDeclarationError {}

impl From<CombatEvasionProductionBridgeError> for CombatBlockDeclarationError {
    fn from(error: CombatEvasionProductionBridgeError) -> Self {
        Self::KeywordBridge(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectChangeKind {
    Created,
    Removed,
    Updated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectDelta {
    pub object_id: ObjectId,
    pub origin_id: ObjectId,
    pub copy_of: Option<ObjectId>,
    pub kind: ObjectChangeKind,
    /// Complete states make the delta lossless for simulator adapters. The
    /// fields below are stable convenience projections for hot paths.
    pub before: Option<PhysicalObject>,
    pub after: Option<PhysicalObject>,
    pub zone_before: Option<Zone>,
    pub zone_after: Option<Zone>,
    pub controller_before: Option<PlayerId>,
    pub controller_after: Option<PlayerId>,
    pub token_before: Option<bool>,
    pub token_after: Option<bool>,
    pub tapped_before: Option<bool>,
    pub tapped_after: Option<bool>,
    pub active_face_before: Option<u8>,
    pub active_face_after: Option<u8>,
    pub power_before: Option<i64>,
    pub power_after: Option<i64>,
    pub toughness_before: Option<i64>,
    pub toughness_after: Option<i64>,
    pub counters_before: BTreeMap<String, u32>,
    pub counters_after: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerDelta {
    pub player_id: PlayerId,
    pub before: PlayerState,
    pub after: PlayerState,
    pub life_before: i64,
    pub life_after: i64,
    pub colored_mana_delta: [i64; 6],
    pub unrestricted_mana_delta: i64,
    pub library_before: Vec<ObjectId>,
    pub library_after: Vec<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentDelta {
    pub source: ObjectId,
    pub before: Option<AttachmentRecord>,
    pub after: Option<AttachmentRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationDelta {
    pub continuous: Vec<ContinuousEffectRecord>,
    pub delayed_triggers: Vec<DelayedTriggerRecord>,
    pub replacements: Vec<ReplacementRecord>,
    pub restrictions: Vec<RestrictionRecord>,
    pub activation_reductions: Vec<ActivationReductionRecord>,
    pub spell_reductions: Vec<SpellReductionRecord>,
    pub scheduled_copies: Vec<ScheduledCopyRecord>,
    pub cast_permissions: Vec<CastPermissionRecord>,
    pub extra_turns: Vec<ExtraTurnRecord>,
    pub payment_or_lose: Vec<PaymentOrLoseRecord>,
    pub game_results: Vec<GameResultRecord>,
    pub skipped_steps: Vec<SkippedStepRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationDelta {
    pub players: Vec<PlayerDelta>,
    pub objects: Vec<ObjectDelta>,
    pub attachments: Vec<AttachmentDelta>,
    pub registrations: RegistrationDelta,
    pub mutation_log: Vec<String>,
}

impl SimulationDelta {
    pub fn between(before: &InMemoryOracleState, after: &InMemoryOracleState) -> Self {
        let mut player_ids = before.players.keys().copied().collect::<BTreeSet<_>>();
        player_ids.extend(after.players.keys().copied());
        let mut players = Vec::new();
        for player_id in player_ids {
            let Some(before_player) = before.players.get(&player_id) else {
                continue;
            };
            let Some(after_player) = after.players.get(&player_id) else {
                continue;
            };
            if before_player == after_player {
                continue;
            }
            let mut colored_mana_delta = [0i64; 6];
            for (index, delta) in colored_mana_delta.iter_mut().enumerate() {
                *delta = i64::from(after_player.mana.colored[index])
                    - i64::from(before_player.mana.colored[index]);
            }
            players.push(PlayerDelta {
                player_id,
                before: before_player.clone(),
                after: after_player.clone(),
                life_before: before_player.life,
                life_after: after_player.life,
                colored_mana_delta,
                unrestricted_mana_delta: i64::from(after_player.mana.unrestricted)
                    - i64::from(before_player.mana.unrestricted),
                library_before: before_player.library.clone(),
                library_after: after_player.library.clone(),
            });
        }

        let mut object_ids = before.objects.keys().copied().collect::<BTreeSet<_>>();
        object_ids.extend(after.objects.keys().copied());
        let mut objects = Vec::new();
        for object_id in object_ids {
            let before_object = before.objects.get(&object_id);
            let after_object = after.objects.get(&object_id);
            if before_object == after_object {
                continue;
            }
            let exemplar = after_object
                .or(before_object)
                .expect("union contains object");
            let before_characteristics = before_object.map(PhysicalObject::characteristics);
            let after_characteristics = after_object.map(PhysicalObject::characteristics);
            objects.push(ObjectDelta {
                object_id,
                origin_id: exemplar.origin_id,
                copy_of: exemplar.copy_of,
                kind: match (before_object, after_object) {
                    (None, Some(_)) => ObjectChangeKind::Created,
                    (Some(_), None) => ObjectChangeKind::Removed,
                    (Some(_), Some(_)) => ObjectChangeKind::Updated,
                    (None, None) => unreachable!("union contains object"),
                },
                before: before_object.cloned(),
                after: after_object.cloned(),
                zone_before: before_object.map(|object| object.zone),
                zone_after: after_object.map(|object| object.zone),
                controller_before: before_object.map(|object| object.controller),
                controller_after: after_object.map(|object| object.controller),
                token_before: before_object.map(|object| object.token),
                token_after: after_object.map(|object| object.token),
                tapped_before: before_object.map(|object| object.tapped),
                tapped_after: after_object.map(|object| object.tapped),
                active_face_before: before_object.map(|object| object.active_face),
                active_face_after: after_object.map(|object| object.active_face),
                power_before: before_characteristics.map(|characteristics| characteristics.power),
                power_after: after_characteristics.map(|characteristics| characteristics.power),
                toughness_before: before_characteristics
                    .map(|characteristics| characteristics.toughness),
                toughness_after: after_characteristics
                    .map(|characteristics| characteristics.toughness),
                counters_before: before_object
                    .map(|object| object.counters.clone())
                    .unwrap_or_default(),
                counters_after: after_object
                    .map(|object| object.counters.clone())
                    .unwrap_or_default(),
            });
        }

        let mut attachment_sources = before.attachments.keys().copied().collect::<BTreeSet<_>>();
        attachment_sources.extend(after.attachments.keys().copied());
        let attachments = attachment_sources
            .into_iter()
            .filter_map(|source| {
                let before_attachment = before.attachments.get(&source).copied();
                let after_attachment = after.attachments.get(&source).copied();
                (before_attachment != after_attachment).then_some(AttachmentDelta {
                    source,
                    before: before_attachment,
                    after: after_attachment,
                })
            })
            .collect();

        Self {
            players,
            objects,
            attachments,
            registrations: RegistrationDelta {
                continuous: appended(&before.continuous_effects, &after.continuous_effects),
                delayed_triggers: appended(&before.delayed_triggers, &after.delayed_triggers),
                replacements: appended(&before.replacement_effects, &after.replacement_effects),
                restrictions: appended(&before.restriction_effects, &after.restriction_effects),
                activation_reductions: appended(
                    &before.activation_reductions,
                    &after.activation_reductions,
                ),
                spell_reductions: appended(&before.spell_reductions, &after.spell_reductions),
                scheduled_copies: appended(&before.scheduled_copies, &after.scheduled_copies),
                cast_permissions: appended(&before.cast_permissions, &after.cast_permissions),
                extra_turns: appended(&before.extra_turns, &after.extra_turns),
                payment_or_lose: appended(&before.payment_or_lose, &after.payment_or_lose),
                game_results: appended(&before.game_results, &after.game_results),
                skipped_steps: appended(&before.skipped_steps, &after.skipped_steps),
            },
            mutation_log: appended(&before.mutation_log, &after.mutation_log),
        }
    }

    pub fn object(&self, object_id: ObjectId) -> Option<&ObjectDelta> {
        self.objects
            .iter()
            .find(|delta| delta.object_id == object_id)
    }
}

fn appended<T: Clone>(before: &[T], after: &[T]) -> Vec<T> {
    after.get(before.len()..).unwrap_or_default().to_vec()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationBatch {
    pub receipts: Vec<(ObjectId, ClauseAddress, ExecutionReceipt)>,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantedAbilityBatch {
    pub source: ObjectId,
    pub ability_index: usize,
    pub receipt: ExecutionReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellManaPaymentBatch {
    pub source: ObjectId,
    pub player: PlayerId,
    pub printed_cost: crate::bounded_oracle_runtime::ManaCost,
    pub reduced_cost: crate::bounded_oracle_runtime::ManaCost,
    pub generic_reduction: u32,
    pub delta: SimulationDelta,
}

/// Owns the bounded state and the exact program bound to each physical object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedOracleSimulation {
    state: InMemoryOracleState,
    programs: BTreeMap<ObjectId, Vec<BoundedOracleClause>>,
    combat_evasion_programs: BTreeMap<ObjectId, Vec<DelegatedKeywordClause>>,
}

impl Default for BoundedOracleSimulation {
    fn default() -> Self {
        Self::new(InMemoryOracleState::default())
    }
}

impl BoundedOracleSimulation {
    pub fn new(state: InMemoryOracleState) -> Self {
        Self {
            state,
            programs: BTreeMap::new(),
            combat_evasion_programs: BTreeMap::new(),
        }
    }

    pub fn state(&self) -> &InMemoryOracleState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut InMemoryOracleState {
        &mut self.state
    }

    pub fn into_state(self) -> InMemoryOracleState {
        self.state
    }

    pub fn bind_program(
        &mut self,
        object_id: ObjectId,
        clauses: impl IntoIterator<Item = BoundedOracleClause>,
    ) -> Result<(), ExecutionError> {
        if self.state.object(object_id).is_none() {
            return Err(ExecutionError::MissingObject(object_id));
        }
        let mut clauses = clauses.into_iter().collect::<Vec<_>>();
        clauses.sort_by_key(BoundedOracleClause::address);
        if clauses
            .iter()
            .any(|clause| !clause_has_live_bridge_contract(clause))
        {
            return Err(ExecutionError::Adapter(
                "bounded program contains a clause without a complete live bridge contract"
                    .to_owned(),
            ));
        }
        self.programs.insert(object_id, clauses);
        Ok(())
    }

    pub fn bind_compiled_card(
        &mut self,
        object_id: ObjectId,
        card: &CompiledCard,
    ) -> Result<CompiledCardProgramBinding, ExecutionError> {
        let prior_bounded = self.programs.get(&object_id).cloned();
        let prior_combat = self.combat_evasion_programs.get(&object_id).cloned();
        let mut bounded_clauses = Vec::new();
        let mut blocked_bounded_clauses = Vec::new();
        for clause in &card.effects.bounded_oracle {
            if clause_has_live_bridge_contract(clause) {
                bounded_clauses.push(clause.clone());
            } else {
                blocked_bounded_clauses.push(BlockedBoundedProgramOccurrence {
                    address: clause.address(),
                    semantic_digest: clause.semantic_digest().to_owned(),
                    blocker_code: "bounded-clause-live-bridge-unavailable",
                });
            }
        }
        self.bind_program(object_id, bounded_clauses)?;
        let combat_clauses = card
            .effects
            .delegated_oracle
            .iter()
            .filter(|clause| {
                matches!(
                    clause.keyword_program().keyword(),
                    OfficialKeyword::Fear | OfficialKeyword::Shadow | OfficialKeyword::Landwalk
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        if let Err(error) = self.bind_combat_evasion_programs(object_id, combat_clauses.clone()) {
            match prior_bounded {
                Some(clauses) => {
                    self.programs.insert(object_id, clauses);
                }
                None => {
                    self.programs.remove(&object_id);
                }
            }
            match prior_combat {
                Some(clauses) => {
                    self.combat_evasion_programs.insert(object_id, clauses);
                }
                None => {
                    self.combat_evasion_programs.remove(&object_id);
                }
            }
            return Err(ExecutionError::Adapter(error.to_string()));
        }
        let bounded_clause_addresses = self
            .programs
            .get(&object_id)
            .into_iter()
            .flatten()
            .map(BoundedOracleClause::address)
            .collect();
        let combat_evasion_clause_addresses = combat_clauses
            .iter()
            .map(DelegatedKeywordClause::address)
            .collect();
        Ok(CompiledCardProgramBinding {
            object_id,
            bounded_clause_addresses,
            combat_evasion_clause_addresses,
            blocked_bounded_clauses,
        })
    }

    /// Bind exact occurrence-addressed Fear, Shadow, and landwalk programs to
    /// one real physical object with matching printed creature faces.
    ///
    /// Distinct clause addresses are distinct keyword instances. Reusing one
    /// address is rejected, while multiple same-kind landwalk clauses remain
    /// registered and are redundant only when declaration legality executes.
    pub(crate) fn bind_combat_evasion_programs(
        &mut self,
        object_id: ObjectId,
        clauses: impl IntoIterator<Item = DelegatedKeywordClause>,
    ) -> Result<(), CombatBlockDeclarationError> {
        let object = self
            .state
            .object(object_id)
            .ok_or(CombatBlockDeclarationError::MissingObject(object_id))?;
        let mut clauses = clauses.into_iter().collect::<Vec<_>>();
        if clauses.is_empty() {
            self.combat_evasion_programs.remove(&object_id);
            return Ok(());
        }

        let mut by_face = BTreeMap::<u16, Vec<&KeywordProgram>>::new();
        for clause in &clauses {
            let address = clause.address();
            let program = clause.keyword_program();
            if clause.runtime_version() != ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION
                || clause.required_live_bridge_capabilities() != COMBAT_BLOCK_LEGALITY_CAPABILITIES
                || program.source().face_index != address.face_index
                || program.source().clause_index != address.clause_index
                || !matches!(
                    program.keyword(),
                    OfficialKeyword::Fear | OfficialKeyword::Shadow | OfficialKeyword::Landwalk
                )
            {
                return Err(CombatBlockDeclarationError::InexactDelegatedProgram {
                    object: object_id,
                    address,
                });
            }
            let face = physical_object_face(&object, address.face_index).ok_or(
                CombatBlockDeclarationError::ProgramFaceUnavailable {
                    object: object_id,
                    face_index: address.face_index,
                },
            )?;
            if !face.card_types.contains(&CardType::Creature) {
                return Err(CombatBlockDeclarationError::ProgramFaceMustBeCreature {
                    object: object_id,
                    face_index: address.face_index,
                });
            }
            by_face.entry(address.face_index).or_default().push(program);
        }
        for programs in by_face.values() {
            validate_combat_evasion_program_set(programs)?;
        }

        clauses.sort_by_key(DelegatedKeywordClause::address);
        self.combat_evasion_programs.insert(object_id, clauses);
        Ok(())
    }

    pub fn unbind_program(&mut self, object_id: ObjectId) {
        self.programs.remove(&object_id);
        self.combat_evasion_programs.remove(&object_id);
    }

    pub fn bound_program(&self, object_id: ObjectId) -> Option<BoundObjectProgram> {
        self.programs
            .get(&object_id)
            .cloned()
            .map(|clauses| BoundObjectProgram { object_id, clauses })
    }

    pub(crate) fn bound_combat_evasion_program(
        &self,
        object_id: ObjectId,
    ) -> Option<BoundCombatEvasionProgram> {
        self.combat_evasion_programs
            .get(&object_id)
            .cloned()
            .map(|clauses| BoundCombatEvasionProgram { object_id, clauses })
    }

    pub fn can_block(&self, object: ObjectId) -> Result<bool, ExecutionError> {
        let candidate = self
            .state
            .object(object)
            .ok_or(ExecutionError::MissingObject(object))?;
        object_can_block(
            &self.state,
            object,
            &ExecutionContext::new(candidate.controller, object, ActionWindow::Static),
        )
    }

    pub fn can_attack(&self, object: ObjectId) -> Result<bool, ExecutionError> {
        let candidate = self
            .state
            .object(object)
            .ok_or(ExecutionError::MissingObject(object))?;
        object_can_attack(
            &self.state,
            object,
            &ExecutionContext::new(candidate.controller, object, ActionWindow::Static),
        )
    }

    pub fn must_attack_each_combat(&self, object: ObjectId) -> Result<bool, ExecutionError> {
        let candidate = self
            .state
            .object(object)
            .ok_or(ExecutionError::MissingObject(object))?;
        object_must_attack_each_combat(
            &self.state,
            object,
            &ExecutionContext::new(candidate.controller, object, ActionWindow::Static),
        )
    }

    pub fn can_be_blocked(&self, object: ObjectId) -> Result<bool, ExecutionError> {
        let candidate = self
            .state
            .object(object)
            .ok_or(ExecutionError::MissingObject(object))?;
        object_can_be_blocked(
            &self.state,
            object,
            &ExecutionContext::new(candidate.controller, object, ActionWindow::Static),
        )
    }

    /// Evaluate one real block declaration from the current bounded state.
    ///
    /// The attacker and blocker are current physical objects. Effective
    /// characteristics are resolved before translating the complete
    /// battlefield into the keyword kernel. Exact programs are selected by
    /// physical object identity and current face. No retained keyword metadata
    /// or card name can authorize the result.
    pub(crate) fn evaluate_combat_evasion_block_declaration(
        &self,
        attacker: ObjectId,
        blocker: ObjectId,
        defending_player: PlayerId,
    ) -> Result<CombatBlockDeclarationResult, CombatBlockDeclarationError> {
        if !self.state.players.contains_key(&defending_player) {
            return Err(CombatBlockDeclarationError::MissingPlayer(defending_player));
        }
        let raw_attacker = self
            .state
            .object(attacker)
            .ok_or(CombatBlockDeclarationError::MissingObject(attacker))?;
        let raw_blocker = self
            .state
            .object(blocker)
            .ok_or(CombatBlockDeclarationError::MissingObject(blocker))?;
        let attacker_clauses =
            self.active_combat_evasion_clauses(attacker, raw_attacker.active_face);
        if attacker_clauses.is_empty() {
            return Err(CombatBlockDeclarationError::MissingActiveCombatEvasionProgram(attacker));
        }
        let blocker_clauses = self
            .active_combat_evasion_clauses(blocker, raw_blocker.active_face)
            .into_iter()
            .filter(|clause| clause.keyword_program().keyword() == OfficialKeyword::Shadow)
            .collect::<Vec<_>>();
        let attacker_addresses = attacker_clauses
            .iter()
            .map(|clause| clause.address())
            .collect::<Vec<_>>();
        let blocker_addresses = blocker_clauses
            .iter()
            .map(|clause| clause.address())
            .collect::<Vec<_>>();

        let effective_attacker = self
            .effective_object(attacker)
            .map_err(CombatBlockDeclarationError::EffectiveState)?;
        let effective_blocker = self
            .effective_object(blocker)
            .map_err(CombatBlockDeclarationError::EffectiveState)?;
        let attacker_is_creature = effective_attacker
            .characteristics()
            .card_types
            .contains(&CardType::Creature);
        let blocker_is_creature = effective_blocker
            .characteristics()
            .card_types
            .contains(&CardType::Creature);
        let physical_context_is_legal = attacker != blocker
            && effective_attacker.zone == Zone::Battlefield
            && effective_blocker.zone == Zone::Battlefield
            && attacker_is_creature
            && blocker_is_creature
            && effective_attacker.attacking
            && !effective_blocker.attacking
            && effective_attacker.controller != defending_player
            && effective_blocker.controller == defending_player;
        if !physical_context_is_legal {
            return Ok(CombatBlockDeclarationResult {
                bridge_version: COMBAT_BLOCK_DECLARATION_PRODUCTION_BRIDGE_VERSION,
                kernel_bridge_version: COMBAT_EVASION_PRODUCTION_BRIDGE_VERSION,
                attacker,
                blocker,
                defending_player,
                attacker_clause_addresses: attacker_addresses,
                blocker_clause_addresses: blocker_addresses,
                attacker_keyword_receipts: Vec::new(),
                blocker_keyword_receipts: Vec::new(),
                legal: false,
            });
        }

        let attacker_binding = combat_object_binding(&effective_attacker);
        let blocker_binding = combat_object_binding(&effective_blocker);
        let attacker_printed = combat_object_characteristics(&effective_attacker)?;
        let blocker_printed = combat_object_characteristics(&effective_blocker)?;
        let attacker_programs = attacker_clauses
            .iter()
            .map(|clause| clause.keyword_program())
            .collect::<Vec<_>>();
        let blocker_programs = blocker_clauses
            .iter()
            .map(|clause| clause.keyword_program())
            .collect::<Vec<_>>();

        let mut battlefield = Vec::new();
        for object_id in self.state.object_ids() {
            if object_id == attacker || object_id == blocker {
                continue;
            }
            let effective = self
                .effective_object(object_id)
                .map_err(CombatBlockDeclarationError::EffectiveState)?;
            if effective.zone != Zone::Battlefield {
                continue;
            }
            battlefield.push((
                combat_object_binding(&effective),
                combat_object_characteristics(&effective)?,
            ));
        }

        let evaluation = evaluate_combat_evasion_keywords(
            &attacker_programs,
            attacker_binding,
            attacker_printed,
        )?;
        let kernel_evaluation = evaluation.evaluate_block_by(
            blocker_binding,
            blocker_printed,
            &blocker_programs,
            &battlefield,
        )?;
        let bounded_legal = object_can_block(
            &self.state,
            blocker,
            &ExecutionContext::new(effective_blocker.controller, blocker, ActionWindow::Static),
        )
        .map_err(CombatBlockDeclarationError::EffectiveState)?
            && object_can_be_blocked(
                &self.state,
                attacker,
                &ExecutionContext::new(
                    effective_attacker.controller,
                    attacker,
                    ActionWindow::Static,
                ),
            )
            .map_err(CombatBlockDeclarationError::EffectiveState)?;

        Ok(CombatBlockDeclarationResult {
            bridge_version: COMBAT_BLOCK_DECLARATION_PRODUCTION_BRIDGE_VERSION,
            kernel_bridge_version: evaluation.bridge_version(),
            attacker,
            blocker,
            defending_player,
            attacker_clause_addresses: attacker_addresses,
            blocker_clause_addresses: blocker_addresses,
            attacker_keyword_receipts: evaluation.receipts().to_vec(),
            blocker_keyword_receipts: kernel_evaluation.blocker_receipts().to_vec(),
            legal: bounded_legal && kernel_evaluation.permitted(),
        })
    }

    fn active_combat_evasion_clauses(
        &self,
        object: ObjectId,
        face_index: u8,
    ) -> Vec<&DelegatedKeywordClause> {
        self.combat_evasion_programs
            .get(&object)
            .into_iter()
            .flatten()
            .filter(|clause| clause.address().face_index == u16::from(face_index))
            .collect()
    }

    pub fn attach(
        &mut self,
        source: ObjectId,
        target: ObjectId,
        kind: AttachmentKind,
    ) -> Result<SimulationDelta, ExecutionError> {
        let before = self.state.clone();
        if let Err(error) = self.state.set_attachment(AttachmentRecord {
            source,
            target,
            kind,
        }) {
            self.state = before;
            return Err(ExecutionError::Adapter(error));
        }
        Ok(SimulationDelta::between(&before, &self.state))
    }

    pub fn detach(&mut self, source: ObjectId) -> Result<SimulationDelta, ExecutionError> {
        let before = self.state.clone();
        if let Err(error) = self.state.clear_attachment(source) {
            self.state = before;
            return Err(ExecutionError::Adapter(error));
        }
        Ok(SimulationDelta::between(&before, &self.state))
    }

    pub fn effective_object(&self, object: ObjectId) -> Result<PhysicalObject, ExecutionError> {
        let candidate = self
            .state
            .object(object)
            .ok_or(ExecutionError::MissingObject(object))?;
        effective_object(
            &self.state,
            object,
            &ExecutionContext::new(candidate.controller, object, ActionWindow::Static),
        )
    }

    pub fn can_untap_during(&self, object: ObjectId, step: Step) -> Result<bool, ExecutionError> {
        let candidate = self
            .state
            .object(object)
            .ok_or(ExecutionError::MissingObject(object))?;
        object_can_untap_during(
            &self.state,
            object,
            step,
            &ExecutionContext::new(candidate.controller, object, ActionWindow::Static),
        )
    }

    pub fn pay_spell_mana(
        &mut self,
        source: ObjectId,
        player: PlayerId,
        printed_cost: crate::bounded_oracle_runtime::ManaCost,
        x_value: u32,
    ) -> Result<SpellManaPaymentBatch, ExecutionError> {
        let before = self.state.clone();
        let mut context =
            ExecutionContext::new(player, source, ActionWindow::CastingAdditionalCost);
        context.x_value = x_value;
        let (reduced_cost, generic_reduction) = match pay_reduced_spell_mana_cost(
            &mut self.state,
            source,
            player,
            &printed_cost,
            x_value,
            &context,
        ) {
            Ok(payment) => payment,
            Err(error) => {
                self.state = before;
                return Err(error);
            }
        };
        Ok(SpellManaPaymentBatch {
            source,
            player,
            printed_cost,
            reduced_cost,
            generic_reduction,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    /// Resolve every exact spell-resolution clause as one simulator batch.
    ///
    /// The caller supplies target and choice state per clause. Any failure
    /// restores the state from before the first clause.
    pub fn resolve_spell<F>(
        &mut self,
        source: ObjectId,
        mut context_for: F,
    ) -> Result<SimulationBatch, ExecutionError>
    where
        F: FnMut(&BoundedOracleClause) -> ExecutionContext,
    {
        self.execute_matching(
            source,
            |timing| matches!(timing, Timing::SpellResolution),
            |clause| {
                let mut context = context_for(clause);
                context.source = source;
                context.window = ActionWindow::SpellResolution;
                context
            },
        )
    }

    /// Pays every mandatory printed additional cost for one source spell as
    /// one atomic casting batch. Any failed cost restores the complete state
    /// from before the first additional-cost clause.
    pub fn pay_cast_additional_costs<F>(
        &mut self,
        source: ObjectId,
        mut context_for: F,
    ) -> Result<SimulationBatch, ExecutionError>
    where
        F: FnMut(&BoundedOracleClause) -> ExecutionContext,
    {
        self.execute_matching(
            source,
            |timing| matches!(timing, Timing::CastingAdditionalCost),
            |clause| {
                let mut context = context_for(clause);
                context.source = source;
                context.window = ActionWindow::CastingAdditionalCost;
                context
            },
        )
    }

    /// Register static and replacement programs for a battlefield object.
    pub fn register_static_and_replacements(
        &mut self,
        source: ObjectId,
        actor: PlayerId,
    ) -> Result<SimulationBatch, ExecutionError> {
        let clauses = self
            .programs
            .get(&source)
            .cloned()
            .ok_or(ExecutionError::MissingObject(source))?;
        let before = self.state.clone();
        let mut receipts = Vec::new();
        for clause in clauses {
            let window = match clause.timing() {
                Timing::Static => ActionWindow::Static,
                Timing::Replacement => ActionWindow::Replacement,
                _ => continue,
            };
            let context = ExecutionContext::new(actor, source, window);
            let result = if matches!(clause.timing(), Timing::Replacement) {
                // EventWouldOccur describes which future event the registered
                // replacement observes. It is not a precondition for making
                // the replacement live while its source is present.
                let registration_conditions = clause
                    .conditions()
                    .iter()
                    .filter(|condition| !matches!(condition, Condition::EventWouldOccur(_)))
                    .cloned()
                    .collect::<Vec<_>>();
                execute_action(
                    &mut self.state,
                    ActionDefinition {
                        timing: clause.timing(),
                        conditions: &registration_conditions,
                        costs: clause.costs(),
                        targets: clause.targets(),
                        effects: clause.effects(),
                        activation_restriction: clause.activation_restriction(),
                    },
                    &context,
                )
            } else {
                execute_clause(&mut self.state, &clause, &context)
            };
            match result {
                Ok(receipt) => receipts.push((source, clause.address(), receipt)),
                Err(error) => {
                    self.state = before;
                    return Err(error);
                }
            }
        }
        Ok(SimulationBatch {
            receipts,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    /// Complete a permanent entry, register its persistent programs, then
    /// dispatch the physical `ObjectEntered` event to every bound source.
    pub fn permanent_entered<F>(
        &mut self,
        object: ObjectId,
        actor: PlayerId,
        mut context_for: F,
    ) -> Result<SimulationBatch, ExecutionError>
    where
        F: FnMut(ObjectId, &BoundedOracleClause, &TriggerEvent) -> ExecutionContext,
    {
        let before = self.state.clone();
        let mut receipts = self
            .register_static_and_replacements(object, actor)?
            .receipts;
        let event = TriggerEvent::ObjectEntered { object };
        match self.dispatch_trigger(event, |source, clause, event| {
            context_for(source, clause, event)
        }) {
            Ok(triggered) => receipts.extend(triggered.receipts),
            Err(error) => {
                self.state = before;
                return Err(error);
            }
        }
        Ok(SimulationBatch {
            receipts,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    /// Execute the exact activated clauses printed on one physical source.
    pub fn activate<F>(
        &mut self,
        source: ObjectId,
        mut context_for: F,
    ) -> Result<SimulationBatch, ExecutionError>
    where
        F: FnMut(&BoundedOracleClause) -> ExecutionContext,
    {
        self.execute_matching(
            source,
            |timing| matches!(timing, Timing::Activated),
            |clause| {
                let mut context = context_for(clause);
                context.source = source;
                context.window = ActionWindow::Activated;
                context
            },
        )
    }

    /// Activate one exact ability carried by the source's active
    /// characteristics, including token abilities such as Food and Treasure.
    pub fn activate_granted_ability(
        &mut self,
        source: ObjectId,
        ability_index: usize,
        mut context: ExecutionContext,
    ) -> Result<GrantedAbilityBatch, ExecutionError> {
        let before = self.state.clone();
        context.source = source;
        context.window = ActionWindow::Activated;
        let receipt = execute_granted_ability(&mut self.state, source, ability_index, &context)?;
        Ok(GrantedAbilityBatch {
            source,
            ability_index,
            receipt,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    /// Dispatch an event in stable physical-object and clause-address order.
    pub fn dispatch_trigger<F>(
        &mut self,
        event: TriggerEvent,
        mut context_for: F,
    ) -> Result<SimulationBatch, ExecutionError>
    where
        F: FnMut(ObjectId, &BoundedOracleClause, &TriggerEvent) -> ExecutionContext,
    {
        let before = self.state.clone();
        let mut receipts = Vec::new();
        if let TriggerEvent::BeginningOf {
            step: crate::bounded_oracle_runtime::Step::UntapStep,
            active_player,
            is_next_turn: true,
        } = &event
            && let Some((index, _)) = self
                .state
                .extra_turns
                .iter()
                .enumerate()
                .filter(|(_, record)| record.player == *active_player)
                .min_by_key(|(_, record)| (record.order, record.source_identity))
        {
            let record = self.state.extra_turns.remove(index);
            self.state.record_mutation(format!(
                "consume_extra_turn:{}:{}",
                record.player, record.order
            ));
        }

        let mut delayed = self.state.delayed_triggers.clone();
        delayed.sort_by_key(|record| (record.order, record.source_identity));
        let mut consumed_delayed = BTreeSet::new();
        for record in delayed {
            let actor = self
                .state
                .object(record.source_identity)
                .map(|object| object.controller)
                .or_else(|| event_player(&event))
                .or_else(|| self.state.player_ids().into_iter().next())
                .ok_or(ExecutionError::InvalidAmount(
                    "delayed trigger has no acting player",
                ))?;
            let mut context = ExecutionContext::new(
                actor,
                record.source_identity,
                ActionWindow::Triggered(event.clone()),
            );
            populate_trigger_context(&mut context, &event);
            if !trigger_matches(&self.state, &record.trigger, &event, &context)? {
                continue;
            }
            let timing = Timing::Triggered(Box::new(record.trigger.clone()));
            let receipt = execute_action(
                &mut self.state,
                ActionDefinition {
                    timing: &timing,
                    conditions: &[],
                    costs: &[],
                    targets: &[],
                    effects: &record.effects,
                    activation_restriction: None,
                },
                &context,
            );
            if let Err(error) = receipt {
                self.state = before;
                return Err(error);
            }
            consumed_delayed.insert(record.order);
        }
        self.state
            .delayed_triggers
            .retain(|record| !consumed_delayed.contains(&record.order));

        let programs = self.programs.clone();
        for (source, clauses) in programs {
            for clause in &clauses {
                match clause.timing() {
                    Timing::Triggered(_) => {
                        let mut context = context_for(source, clause, &event);
                        context.source = source;
                        context.window = ActionWindow::Triggered(event.clone());
                        populate_trigger_context(&mut context, &event);
                        match execute_clause(&mut self.state, clause, &context) {
                            Ok(receipt) => receipts.push((source, clause.address(), receipt)),
                            Err(
                                ExecutionError::TimingMismatch
                                | ExecutionError::ConditionFailed { .. },
                            ) => {}
                            Err(error) => {
                                self.state = before;
                                return Err(error);
                            }
                        }
                    }
                    Timing::TriggeredModalHeader { .. } => {
                        let mut header_context = context_for(source, clause, &event);
                        header_context.source = source;
                        header_context.window = ActionWindow::Triggered(event.clone());
                        populate_trigger_context(&mut header_context, &event);
                        let selected_modes = header_context.selected_modes.clone();
                        match execute_clause(&mut self.state, clause, &header_context) {
                            Ok(receipt) => receipts.push((source, clause.address(), receipt)),
                            Err(
                                ExecutionError::TimingMismatch
                                | ExecutionError::ConditionFailed { .. },
                            ) => continue,
                            Err(error) => {
                                self.state = before;
                                return Err(error);
                            }
                        }
                        for branch_index in selected_modes {
                            let Some(branch) = clauses.iter().find(|candidate| {
                                matches!(
                                    candidate.timing(),
                                    Timing::ModalBranch {
                                        header_clause_index: Some(header_clause_index),
                                        branch_index: candidate_branch,
                                    } if *header_clause_index == clause.address().clause_index
                                        && *candidate_branch == branch_index
                                )
                            }) else {
                                self.state = before;
                                return Err(ExecutionError::InvalidAmount(
                                    "selected triggered modal branch is unavailable",
                                ));
                            };
                            let mut branch_context = context_for(source, branch, &event);
                            branch_context.source = source;
                            branch_context.window = ActionWindow::ModalBranch {
                                header_clause_index: Some(clause.address().clause_index),
                                branch_index,
                            };
                            populate_trigger_context(&mut branch_context, &event);
                            match execute_clause(&mut self.state, branch, &branch_context) {
                                Ok(receipt) => {
                                    receipts.push((source, branch.address(), receipt));
                                }
                                Err(error) => {
                                    self.state = before;
                                    return Err(error);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(SimulationBatch {
            receipts,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    fn execute_matching<P, F>(
        &mut self,
        source: ObjectId,
        mut predicate: P,
        mut context_for: F,
    ) -> Result<SimulationBatch, ExecutionError>
    where
        P: FnMut(&Timing) -> bool,
        F: FnMut(&BoundedOracleClause) -> ExecutionContext,
    {
        let clauses = self
            .programs
            .get(&source)
            .cloned()
            .ok_or(ExecutionError::MissingObject(source))?;
        let before = self.state.clone();
        let mut receipts = Vec::new();
        for clause in clauses {
            if !predicate(clause.timing()) {
                continue;
            }
            match execute_clause(&mut self.state, &clause, &context_for(&clause)) {
                Ok(receipt) => receipts.push((source, clause.address(), receipt)),
                Err(error) => {
                    self.state = before;
                    return Err(error);
                }
            }
        }
        Ok(SimulationBatch {
            receipts,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }
}

fn physical_object_face(
    object: &PhysicalObject,
    face_index: u16,
) -> Option<&ObjectCharacteristics> {
    match face_index {
        0 => Some(&object.front),
        1 => object.back.as_ref(),
        _ => None,
    }
}

fn combat_object_binding(object: &PhysicalObject) -> StaticKeywordObjectBinding {
    StaticKeywordObjectBinding::new(
        KeywordObjectId(object.id),
        KeywordPlayerId(u16::from(object.owner)),
        KeywordPlayerId(u16::from(object.controller)),
        combat_zone(object.zone),
        true,
        object.tapped,
    )
}

fn combat_zone(zone: Zone) -> KeywordZone {
    match zone {
        Zone::Library => KeywordZone::Library,
        Zone::Hand => KeywordZone::Hand,
        Zone::Battlefield => KeywordZone::Battlefield,
        Zone::Graveyard => KeywordZone::Graveyard,
        Zone::Exile => KeywordZone::Exile,
        Zone::Stack => KeywordZone::Stack,
        Zone::Command => KeywordZone::Command,
    }
}

fn combat_object_characteristics(
    object: &PhysicalObject,
) -> Result<KeywordObjectCharacteristics, CombatBlockDeclarationError> {
    let characteristics = object.characteristics();
    let power = i32::try_from(characteristics.power)
        .map_err(|_| CombatBlockDeclarationError::CharacteristicOutOfRange(object.id))?;
    let toughness = i32::try_from(characteristics.toughness)
        .map_err(|_| CombatBlockDeclarationError::CharacteristicOutOfRange(object.id))?;
    Ok(KeywordObjectCharacteristics {
        name: characteristics.names.first().cloned(),
        card_types: characteristics
            .card_types
            .iter()
            .filter_map(|card_type| match card_type {
                CardType::Artifact => Some(KeywordCardType::Artifact),
                CardType::Battle => Some(KeywordCardType::Battle),
                CardType::Creature => Some(KeywordCardType::Creature),
                CardType::Enchantment => Some(KeywordCardType::Enchantment),
                CardType::Instant => Some(KeywordCardType::Instant),
                CardType::Land => Some(KeywordCardType::Land),
                CardType::Planeswalker => Some(KeywordCardType::Planeswalker),
                CardType::Sorcery => Some(KeywordCardType::Sorcery),
                CardType::Spell | CardType::Permanent => None,
            })
            .collect(),
        supertypes: characteristics
            .supertypes
            .iter()
            .map(|supertype| match supertype {
                Supertype::Basic => "Basic",
                Supertype::Legendary => "Legendary",
                Supertype::Snow => "Snow",
                Supertype::Nonbasic => "Nonbasic",
            })
            .map(str::to_owned)
            .collect(),
        subtypes: characteristics.subtypes.iter().cloned().collect(),
        colors: characteristics
            .colors
            .iter()
            .map(|color| match color {
                Color::White => KeywordManaColor::White,
                Color::Blue => KeywordManaColor::Blue,
                Color::Black => KeywordManaColor::Black,
                Color::Red => KeywordManaColor::Red,
                Color::Green => KeywordManaColor::Green,
                Color::Colorless => KeywordManaColor::Colorless,
            })
            .collect(),
        mana_value: characteristics.mana_value,
        power: Some(power),
        toughness: Some(toughness),
        oracle_text: None,
    })
}

fn event_player(event: &TriggerEvent) -> Option<PlayerId> {
    match event {
        TriggerEvent::SpellCast { player, .. }
        | TriggerEvent::CardDrawn { player, .. }
        | TriggerEvent::LifeGained { player, .. }
        | TriggerEvent::TokenCreated { player, .. }
        | TriggerEvent::PlayerAction { player, .. }
        | TriggerEvent::CombatDamageToPlayer { player, .. }
        | TriggerEvent::DamageToPlayer { player, .. } => Some(*player),
        TriggerEvent::BecameTarget { controller, .. } => Some(*controller),
        TriggerEvent::BeginningOf { active_player, .. } => Some(*active_player),
        TriggerEvent::ObjectEntered { .. }
        | TriggerEvent::ObjectAttacked { .. }
        | TriggerEvent::ObjectBlocked { .. }
        | TriggerEvent::SchemeSetInMotion { .. }
        | TriggerEvent::ObjectTappedForMana { .. }
        | TriggerEvent::AttachmentTargetEvent { .. }
        | TriggerEvent::CombatDamageToObject { .. }
        | TriggerEvent::DamageToObject { .. }
        | TriggerEvent::ObjectEvent { .. } => None,
    }
}

fn populate_trigger_context(context: &mut ExecutionContext, event: &TriggerEvent) {
    match event {
        TriggerEvent::SpellCast { spell, player, .. } => {
            context.triggering_object = Some(*spell);
            context.that_player = Some(*player);
        }
        TriggerEvent::CardDrawn { player, card, .. } => {
            context.triggering_object = Some(*card);
            context.that_player = Some(*player);
        }
        TriggerEvent::ObjectEntered { object }
        | TriggerEvent::ObjectAttacked { object }
        | TriggerEvent::SchemeSetInMotion { object }
        | TriggerEvent::ObjectTappedForMana { object } => {
            context.triggering_object = Some(*object);
        }
        TriggerEvent::ObjectBlocked { blocked, .. } => {
            context.triggering_object = Some(*blocked);
        }
        TriggerEvent::ObjectEvent { object, .. } => {
            context.triggering_object = Some(*object);
        }
        TriggerEvent::AttachmentTargetEvent { object, .. } => {
            context.triggering_object = Some(*object);
        }
        TriggerEvent::LifeGained { player, .. } => {
            context.that_player = Some(*player);
        }
        TriggerEvent::TokenCreated { player, token } => {
            context.triggering_object = Some(*token);
            context.that_player = Some(*player);
        }
        TriggerEvent::PlayerAction { player, object, .. } => {
            context.triggering_object = *object;
            context.that_player = Some(*player);
        }
        TriggerEvent::CombatDamageToPlayer { source, player, .. }
        | TriggerEvent::DamageToPlayer { source, player, .. } => {
            context.triggering_object = Some(*source);
            context.that_player = Some(*player);
        }
        TriggerEvent::CombatDamageToObject { object, .. }
        | TriggerEvent::DamageToObject { object, .. } => {
            context.triggering_object = Some(*object);
        }
        TriggerEvent::BecameTarget {
            controller, source, ..
        } => {
            context.triggering_object = Some(*source);
            context.that_player = Some(*controller);
        }
        TriggerEvent::BeginningOf { active_player, .. } => {
            context.active_player = *active_player;
            context.that_player = Some(*active_player);
        }
    }
}

/// Exact front-face object construction used by the existing deck simulator.
///
/// Back-face characteristics are supplied by the caller when the card-data
/// layer exposes a transforming face. This function never invents them.
pub fn physical_object_from_compiled_card(
    object_id: ObjectId,
    owner: PlayerId,
    controller: PlayerId,
    zone: Zone,
    card: &CompiledCard,
) -> PhysicalObject {
    let card_types = card_types_from_profile(&card.effects.card_types);
    let colors = card
        .colors
        .iter()
        .filter_map(|color| color_from_symbol(color))
        .collect();
    let keywords = [
        ("Deathtouch", Keyword::Deathtouch),
        ("Defender", Keyword::Defender),
        ("Double strike", Keyword::DoubleStrike),
        ("First strike", Keyword::FirstStrike),
        ("Flying", Keyword::Flying),
        ("Haste", Keyword::Haste),
        ("Hexproof", Keyword::Hexproof),
        ("Indestructible", Keyword::Indestructible),
        ("Lifelink", Keyword::Lifelink),
        ("Menace", Keyword::Menace),
        ("Reach", Keyword::Reach),
        ("Shroud", Keyword::Shroud),
        ("Trample", Keyword::Trample),
        ("Vigilance", Keyword::Vigilance),
    ]
    .into_iter()
    .filter_map(|(name, keyword)| card.effects.has_printed_keyword(name).then_some(keyword))
    .collect();
    PhysicalObject {
        id: object_id,
        origin_id: object_id,
        copy_of: None,
        owner,
        controller,
        zone,
        token: false,
        tapped: false,
        attacking: false,
        blocking: false,
        prepared: false,
        face_down: false,
        active_face: 0,
        class_level: if subtypes_from_type_line(&card.type_line)
            .iter()
            .any(|subtype| subtype.eq_ignore_ascii_case("Class"))
        {
            1
        } else {
            0
        },
        front: ObjectCharacteristics {
            names: vec![card.name.clone()],
            card_types,
            supertypes: supertypes_from_type_line(&card.type_line),
            subtypes: subtypes_from_type_line(&card.type_line),
            colors,
            mana_value: card.mana_value.max(0.0) as u32,
            power: i64::from(card.printed_power.unwrap_or_default()),
            toughness: i64::from(card.printed_toughness.unwrap_or_default()),
            keywords,
            abilities: Vec::new(),
        },
        back: None,
        counters: BTreeMap::new(),
    }
}

fn card_types_from_profile(profile: &crate::effects::CardTypeProfile) -> Vec<CardType> {
    [
        (profile.is_artifact, CardType::Artifact),
        (profile.is_battle, CardType::Battle),
        (profile.is_creature, CardType::Creature),
        (profile.is_enchantment, CardType::Enchantment),
        (profile.is_instant, CardType::Instant),
        (profile.is_land, CardType::Land),
        (profile.is_planeswalker, CardType::Planeswalker),
        (profile.is_sorcery, CardType::Sorcery),
    ]
    .into_iter()
    .filter_map(|(present, card_type)| present.then_some(card_type))
    .collect()
}

fn color_from_symbol(symbol: &str) -> Option<Color> {
    match symbol.trim().to_ascii_uppercase().as_str() {
        "W" | "WHITE" => Some(Color::White),
        "U" | "BLUE" => Some(Color::Blue),
        "B" | "BLACK" => Some(Color::Black),
        "R" | "RED" => Some(Color::Red),
        "G" | "GREEN" => Some(Color::Green),
        "C" | "COLORLESS" => Some(Color::Colorless),
        _ => None,
    }
}

fn supertypes_from_type_line(type_line: &str) -> Vec<crate::bounded_oracle_runtime::Supertype> {
    use crate::bounded_oracle_runtime::Supertype;
    let leading = type_line
        .split_once('\u{2014}')
        .map_or(type_line, |(left, _)| left);
    [
        ("Basic", Supertype::Basic),
        ("Legendary", Supertype::Legendary),
        ("Snow", Supertype::Snow),
    ]
    .into_iter()
    .filter_map(|(name, supertype)| {
        leading
            .split_whitespace()
            .any(|word| word.eq_ignore_ascii_case(name))
            .then_some(supertype)
    })
    .collect()
}

fn subtypes_from_type_line(type_line: &str) -> Vec<String> {
    type_line
        .split_once('\u{2014}')
        .map(|(_, right)| {
            right
                .split_whitespace()
                .map(|subtype| subtype.trim().to_owned())
                .filter(|subtype| !subtype.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Utility for callers building the four-player state used by trajectory
/// simulation.
pub fn insert_player(
    state: &mut InMemoryOracleState,
    id: PlayerId,
    life: i64,
    commander_identity: Vec<Color>,
) {
    state.insert_player(PlayerState {
        id,
        life,
        mana: Default::default(),
        commander_identity,
        library: Vec::new(),
        counters: Default::default(),
        chosen_creature_type: None,
        maximum_hand_size: Some(7),
    });
}
