//! Pending bounded Oracle actions with a real initiation and resolution boundary.
//!
//! This is deliberately narrower than the complete bounded executor. A clause
//! enters this lifecycle only when every effect can retain locked choices,
//! prune illegal targets, and use the source's last known controller without
//! inventing missing game state.

use super::*;

pub const BOUNDED_ORACLE_ACTION_STACK_VERSION: &str = "bounded-oracle-action-stack-0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingActionId(u64);

impl PendingActionId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundOracleOccurrence {
    semantic_digest: String,
    runtime_version: String,
    address: ClauseAddress,
}

impl BoundOracleOccurrence {
    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn runtime_version(&self) -> &str {
        &self.runtime_version
    }

    pub fn address(&self) -> ClauseAddress {
        self.address
    }

    pub fn matches_clause(&self, clause: &BoundedOracleClause) -> bool {
        self.address == clause.address() && self.matches_content(clause)
    }

    pub fn matches_content(&self, clause: &BoundedOracleClause) -> bool {
        self.semantic_digest == clause.semantic_digest()
            && self.runtime_version == clause.runtime_version()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAction {
    identity: PendingActionId,
    occurrence: BoundOracleOccurrence,
    controller: PlayerId,
    source_identity: ObjectId,
    last_known_source: PhysicalObject,
    locked_context: ExecutionContext,
    declared_targets: Vec<Target>,
    effects: Vec<Effect>,
    costs_paid: usize,
}

impl PendingAction {
    pub fn identity(&self) -> PendingActionId {
        self.identity
    }

    pub fn occurrence(&self) -> &BoundOracleOccurrence {
        &self.occurrence
    }

    pub fn controller(&self) -> PlayerId {
        self.controller
    }

    pub fn source_identity(&self) -> ObjectId {
        self.source_identity
    }

    pub fn last_known_source(&self) -> &PhysicalObject {
        &self.last_known_source
    }

    pub fn locked_context(&self) -> &ExecutionContext {
        &self.locked_context
    }

    pub fn declared_targets(&self) -> &[Target] {
        &self.declared_targets
    }

    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    pub fn costs_paid(&self) -> usize {
        self.costs_paid
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundedOracleActionStack {
    pending: BTreeMap<PendingActionId, PendingAction>,
    order: Vec<PendingActionId>,
    seen: BTreeSet<PendingActionId>,
}

impl BoundedOracleActionStack {
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn contains(&self, identity: PendingActionId) -> bool {
        self.pending.contains_key(&identity)
    }

    pub fn get(&self, identity: PendingActionId) -> Option<&PendingAction> {
        self.pending.get(&identity)
    }

    pub fn top(&self) -> Option<&PendingAction> {
        self.order
            .last()
            .and_then(|identity| self.pending.get(identity))
    }

    pub fn pending_ids(&self) -> &[PendingActionId] {
        &self.order
    }

    fn identity_is_available(&self, identity: PendingActionId) -> bool {
        !self.seen.contains(&identity)
    }

    fn insert(&mut self, action: PendingAction) {
        self.seen.insert(action.identity);
        self.order.push(action.identity);
        self.pending.insert(action.identity, action);
    }

    fn consume_identity(&mut self, identity: PendingActionId) {
        self.seen.insert(identity);
    }

    fn remove(&mut self, identity: PendingActionId) -> Option<PendingAction> {
        let action = self.pending.remove(&identity)?;
        self.order.retain(|candidate| *candidate != identity);
        Some(action)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeginPendingActionReceipt {
    pub identity: PendingActionId,
    pub costs_paid: usize,
    pub state_based_losses: Vec<PlayerId>,
    pub pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingActionResolutionStatus {
    Resolved,
    ResolvedWithPartialTargets,
    Countered,
    AllTargetsIllegal,
    ControllerLostBeforeResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingActionResolutionReceipt {
    pub identity: PendingActionId,
    pub status: PendingActionResolutionStatus,
    pub costs_paid: usize,
    pub effects_applied: usize,
    pub legal_targets: BTreeMap<u8, Vec<SelectedTarget>>,
    pub illegal_targets: BTreeMap<u8, Vec<SelectedTarget>>,
    pub state_based_losses: Vec<PlayerId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingActionError {
    DuplicateIdentity(PendingActionId),
    MissingAction(PendingActionId),
    NotTopAction {
        requested: PendingActionId,
        top: PendingActionId,
    },
    UnsupportedLifecycleEnvelope(&'static str),
    Initiation(ExecutionError),
    ResolutionFailed {
        identity: PendingActionId,
        effect_index: usize,
        reason: String,
    },
}

impl fmt::Display for PendingActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateIdentity(identity) => {
                write!(
                    formatter,
                    "pending action {} was already used",
                    identity.value()
                )
            }
            Self::MissingAction(identity) => {
                write!(
                    formatter,
                    "pending action {} does not exist",
                    identity.value()
                )
            }
            Self::NotTopAction { requested, top } => write!(
                formatter,
                "pending action {} cannot resolve before top action {}",
                requested.value(),
                top.value()
            ),
            Self::UnsupportedLifecycleEnvelope(reason) => {
                write!(formatter, "pending lifecycle does not support {reason}")
            }
            Self::Initiation(error) => write!(formatter, "action initiation failed: {error}"),
            Self::ResolutionFailed {
                identity,
                effect_index,
                reason,
            } => write!(
                formatter,
                "pending action {} failed at effect {}: {}",
                identity.value(),
                effect_index,
                reason
            ),
        }
    }
}

impl std::error::Error for PendingActionError {}

impl From<ExecutionError> for PendingActionError {
    fn from(error: ExecutionError) -> Self {
        Self::Initiation(error)
    }
}

pub fn pending_action_clause_has_live_contract(clause: &BoundedOracleClause) -> bool {
    clause.runtime_version() == BOUNDED_ORACLE_RUNTIME_VERSION
        && clause_has_executable_contract(clause)
        && matches!(
            clause.timing(),
            Timing::Activated | Timing::Triggered(_) | Timing::SpellResolution
        )
        && clause.costs().iter().all(pending_cost_is_supported)
        && clause.targets().iter().all(pending_target_is_supported)
        && clause.effects().iter().all(pending_effect_is_supported)
        && clause
            .effects()
            .iter()
            .all(|effect| pending_effect_target_contract_is_supported(effect, clause.targets()))
}

pub fn begin_activation<S: OracleStateAdapter>(
    state: &mut S,
    stack: &mut BoundedOracleActionStack,
    identity: PendingActionId,
    clause: &BoundedOracleClause,
    context: &ExecutionContext,
) -> Result<BeginPendingActionReceipt, PendingActionError> {
    if !stack.identity_is_available(identity) {
        return Err(PendingActionError::DuplicateIdentity(identity));
    }
    if clause.runtime_version() != BOUNDED_ORACLE_RUNTIME_VERSION {
        return Err(PendingActionError::Initiation(
            ExecutionError::RuntimeVersionMismatch {
                expected: BOUNDED_ORACLE_RUNTIME_VERSION,
                actual: clause.runtime_version(),
            },
        ));
    }
    if !pending_action_clause_has_live_contract(clause) {
        return Err(PendingActionError::UnsupportedLifecycleEnvelope(
            "the clause timing, target shape, or effect shape",
        ));
    }
    if context.countered {
        return Err(PendingActionError::UnsupportedLifecycleEnvelope(
            "a pre-countered initiation context",
        ));
    }
    if !timing_matches(state, clause.timing(), context)? {
        return Err(PendingActionError::Initiation(
            ExecutionError::TimingMismatch,
        ));
    }
    if let Some(restriction) = clause.activation_restriction()
        && !activation_restriction_holds(state, restriction, context)?
    {
        return Err(PendingActionError::Initiation(
            ExecutionError::ActivationRestrictionFailed,
        ));
    }
    if stack_restriction_blocks(state, context)? {
        return Err(PendingActionError::Initiation(
            ExecutionError::StackRestriction,
        ));
    }
    validate_targets(state, clause.targets(), context)?;
    for (index, condition) in clause.conditions().iter().enumerate() {
        if !condition_holds(state, condition, context)? {
            return Err(PendingActionError::Initiation(
                ExecutionError::ConditionFailed { index },
            ));
        }
    }
    let mut last_known_source = state
        .object(context.source)
        .ok_or(ExecutionError::MissingObject(context.source))?;
    let mut source_departed = false;

    let checkpoint = state.checkpoint();
    for (index, cost) in clause.costs().iter().enumerate() {
        let source_before_cost = (!source_departed)
            .then(|| state.object(context.source))
            .flatten();
        if let Err(error) = pay_pending_cost(state, cost, context) {
            state.restore(checkpoint);
            return Err(PendingActionError::Initiation(ExecutionError::CostFailed {
                index,
                reason: error.to_string(),
            }));
        }
        if !source_departed {
            match (source_before_cost, state.object(context.source)) {
                (Some(before), Some(after)) if before.zone == after.zone => {
                    last_known_source = after;
                }
                (Some(before), _) => {
                    last_known_source = before;
                    source_departed = true;
                }
                (None, _) => {
                    state.restore(checkpoint);
                    return Err(PendingActionError::Initiation(
                        ExecutionError::MissingObject(context.source),
                    ));
                }
            }
        }
    }

    stack.consume_identity(identity);
    let state_based_losses = run_state_based_actions(state, context.source);
    let controller_lost = player_has_lost(state, context.actor);
    if !controller_lost {
        let mut locked_context = context.clone();
        locked_context.countered = false;
        locked_context.last_known_source = Some(Box::new(last_known_source.clone()));
        stack.insert(PendingAction {
            identity,
            occurrence: BoundOracleOccurrence {
                semantic_digest: clause.semantic_digest().to_owned(),
                runtime_version: clause.runtime_version().to_owned(),
                address: clause.address(),
            },
            controller: context.actor,
            source_identity: context.source,
            last_known_source,
            locked_context,
            declared_targets: clause.targets().to_vec(),
            effects: clause.effects().to_vec(),
            costs_paid: clause.costs().len(),
        });
    }

    Ok(BeginPendingActionReceipt {
        identity,
        costs_paid: clause.costs().len(),
        state_based_losses,
        pending: !controller_lost,
    })
}

pub fn counter_pending_action(
    stack: &mut BoundedOracleActionStack,
    identity: PendingActionId,
) -> Result<PendingActionResolutionReceipt, PendingActionError> {
    let action = stack
        .remove(identity)
        .ok_or(PendingActionError::MissingAction(identity))?;
    Ok(PendingActionResolutionReceipt {
        identity,
        status: PendingActionResolutionStatus::Countered,
        costs_paid: action.costs_paid,
        effects_applied: 0,
        legal_targets: action.locked_context.targets,
        illegal_targets: BTreeMap::new(),
        state_based_losses: Vec::new(),
    })
}

pub fn resolve_pending_action<S: OracleStateAdapter>(
    state: &mut S,
    stack: &mut BoundedOracleActionStack,
    identity: PendingActionId,
) -> Result<PendingActionResolutionReceipt, PendingActionError> {
    let top = stack
        .order
        .last()
        .copied()
        .ok_or(PendingActionError::MissingAction(identity))?;
    if top != identity {
        return Err(PendingActionError::NotTopAction {
            requested: identity,
            top,
        });
    }
    let action = stack
        .remove(identity)
        .ok_or(PendingActionError::MissingAction(identity))?;
    if player_has_lost(state, action.controller) {
        return Ok(PendingActionResolutionReceipt {
            identity,
            status: PendingActionResolutionStatus::ControllerLostBeforeResolution,
            costs_paid: action.costs_paid,
            effects_applied: 0,
            legal_targets: BTreeMap::new(),
            illegal_targets: action.locked_context.targets,
            state_based_losses: Vec::new(),
        });
    }

    let mut context = action.locked_context.clone();
    context.last_known_source = Some(Box::new(action.last_known_source.clone()));
    let resolution_checkpoint = state.checkpoint();
    let (legal_targets, illegal_targets, initial_count, legal_count) =
        match revalidate_targets(state, &action.declared_targets, &context) {
            Ok(targets) => targets,
            Err(error) => {
                state.restore(resolution_checkpoint);
                return Err(error);
            }
        };
    context.targets = legal_targets.clone();
    if initial_count > 0 && legal_count == 0 {
        let state_based_losses = run_state_based_actions(state, action.source_identity);
        return Ok(PendingActionResolutionReceipt {
            identity,
            status: PendingActionResolutionStatus::AllTargetsIllegal,
            costs_paid: action.costs_paid,
            effects_applied: 0,
            legal_targets,
            illegal_targets,
            state_based_losses,
        });
    }

    let mut effects_applied = 0usize;
    for (effect_index, effect) in action.effects.iter().enumerate() {
        if effect_has_only_illegal_targets(effect, &context.targets) {
            continue;
        }
        if let Err(error) = apply_effect(state, effect, &context) {
            state.restore(resolution_checkpoint);
            let _ = run_state_based_actions(state, action.source_identity);
            return Err(PendingActionError::ResolutionFailed {
                identity,
                effect_index,
                reason: error.to_string(),
            });
        }
        effects_applied += 1;
    }
    let state_based_losses = run_state_based_actions(state, action.source_identity);
    let status = if legal_count < initial_count {
        PendingActionResolutionStatus::ResolvedWithPartialTargets
    } else {
        PendingActionResolutionStatus::Resolved
    };
    Ok(PendingActionResolutionReceipt {
        identity,
        status,
        costs_paid: action.costs_paid,
        effects_applied,
        legal_targets,
        illegal_targets,
        state_based_losses,
    })
}

fn pay_pending_cost<S: OracleStateAdapter>(
    state: &mut S,
    cost: &Cost,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let Cost::PayLife(amount) = cost else {
        return pay_cost(state, cost, context);
    };
    let amount = i64::from(evaluate_amount(state, amount, context)?);
    let mut player = state
        .player(context.actor)
        .ok_or(ExecutionError::MissingPlayer(context.actor))?;
    if amount < 0 || player.life < amount {
        return Err(ExecutionError::Adapter(format!(
            "player {} cannot pay {amount} life",
            context.actor
        )));
    }
    player.life -= amount;
    state.put_player(player).map_err(ExecutionError::Adapter)?;
    state.record_mutation(format!("pay_pending_life_cost:{}:{amount}", context.actor));
    Ok(())
}

fn run_state_based_actions<S: OracleStateAdapter>(
    state: &mut S,
    source_identity: ObjectId,
) -> Vec<PlayerId> {
    let already_lost = state
        .game_results()
        .into_iter()
        .filter_map(|record| (record.result == GameResult::Lost).then_some(record.player))
        .collect::<BTreeSet<_>>();
    let mut losses = Vec::new();
    for player in state.player_ids() {
        if already_lost.contains(&player) || state.player(player).is_none_or(|state| state.life > 0)
        {
            continue;
        }
        let order = state.next_order();
        state.register_game_result(GameResultRecord {
            order,
            source_identity,
            player,
            result: GameResult::Lost,
        });
        state.record_mutation(format!("state_based_loss:{player}:{order}"));
        losses.push(player);
    }
    losses
}

fn player_has_lost<S: OracleStateAdapter>(state: &S, player: PlayerId) -> bool {
    state
        .game_results()
        .iter()
        .any(|record| record.player == player && record.result == GameResult::Lost)
}

type RevalidatedTargets = (
    BTreeMap<u8, Vec<SelectedTarget>>,
    BTreeMap<u8, Vec<SelectedTarget>>,
    usize,
    usize,
);

fn revalidate_targets<S: OracleStateAdapter>(
    state: &S,
    specifications: &[Target],
    context: &ExecutionContext,
) -> Result<RevalidatedTargets, PendingActionError> {
    let mut legal_targets = BTreeMap::new();
    let mut illegal_targets = BTreeMap::new();
    let mut initial_count = 0usize;
    let mut legal_count = 0usize;
    for specification in specifications {
        let selected =
            context
                .targets
                .get(&specification.id)
                .ok_or(ExecutionError::MissingTarget {
                    id: specification.id,
                })?;
        initial_count += selected.len();
        let candidates = legal_target_candidates(state, &specification.filter, context)?;
        let mut legal = Vec::new();
        let mut illegal = Vec::new();
        for candidate in selected {
            let player_is_still_present = match candidate {
                SelectedTarget::Player(player) => !player_has_lost(state, *player),
                SelectedTarget::Object(_) => true,
            };
            if player_is_still_present
                && candidates.contains(candidate)
                && !targeting_protection_blocks(state, &[*candidate], context)?
            {
                legal.push(*candidate);
            } else {
                illegal.push(*candidate);
            }
        }
        legal_count += legal.len();
        legal_targets.insert(specification.id, legal);
        illegal_targets.insert(specification.id, illegal);
    }
    Ok((legal_targets, illegal_targets, initial_count, legal_count))
}

fn pending_target_is_supported(target: &Target) -> bool {
    matches!(
        target.amount,
        TargetAmount::Exactly(_) | TargetAmount::UpTo(_)
    ) && matches!(target.relationship, TargetRelationship::Independent)
        && pending_player_ref_is_supported(&target.chooser)
        && match &target.filter {
            TargetFilter::Player => true,
            TargetFilter::Object(filter) | TargetFilter::Spell(filter) => {
                pending_filter_is_supported(filter)
            }
            TargetFilter::Any(_) | TargetFilter::Conditional { .. } => false,
        }
}

fn pending_cost_is_supported(cost: &Cost) -> bool {
    !matches!(cost, Cost::Loyalty(_) | Cost::RemoveCounter { .. })
}

fn pending_filter_is_supported(filter: &ObjectFilter) -> bool {
    filter
        .controller
        .as_ref()
        .is_none_or(pending_player_ref_is_supported)
        && filter
            .owner
            .as_ref()
            .is_none_or(pending_player_ref_is_supported)
        && filter
            .power
            .as_ref()
            .is_none_or(|(_, amount)| pending_amount_is_locked(amount))
        && filter
            .mana_value
            .as_ref()
            .is_none_or(|(_, amount)| pending_amount_is_locked(amount))
}

fn pending_effect_is_supported(effect: &Effect) -> bool {
    match effect {
        Effect::Counter { object }
        | Effect::Destroy { object }
        | Effect::Tap { object }
        | Effect::Untap { object }
        | Effect::Transform { object } => pending_target_object_ref_is_supported(object),
        Effect::CounterToZone { object, .. } => pending_target_object_ref_is_supported(object),
        Effect::MoveZone(move_zone) => {
            move_zone.delayed_until.is_none()
                && pending_target_object_ref_is_supported(&move_zone.object)
        }
        Effect::Draw {
            player,
            amount,
            delayed_until: None,
            ..
        }
        | Effect::GainLife { player, amount }
        | Effect::LoseLife { player, amount } => {
            pending_player_ref_is_supported(player) && pending_amount_is_locked(amount)
        }
        Effect::Damage {
            source,
            recipient,
            amount,
        } => {
            matches!(source, ObjectRef::Source | ObjectRef::ObjectIdentity(_))
                && pending_player_ref_is_supported(recipient)
                && pending_amount_is_locked(amount)
        }
        Effect::PutCounter { object, amount, .. } => {
            pending_target_object_ref_is_supported(object) && pending_amount_is_locked(amount)
        }
        _ => false,
    }
}

fn pending_effect_target_contract_is_supported(effect: &Effect, targets: &[Target]) -> bool {
    match effect {
        Effect::Counter { object }
        | Effect::CounterToZone { object, .. }
        | Effect::Destroy { object }
        | Effect::Tap { object }
        | Effect::Untap { object }
        | Effect::Transform { object }
        | Effect::PutCounter { object, .. } => {
            pending_object_target_contract_is_supported(object, targets, true)
        }
        Effect::MoveZone(move_zone) => {
            pending_object_target_contract_is_supported(&move_zone.object, targets, true)
        }
        Effect::Draw { player, .. }
        | Effect::GainLife { player, .. }
        | Effect::LoseLife { player, .. } => {
            pending_player_target_contract_is_supported(player, targets, true)
        }
        Effect::Damage { recipient, .. } => {
            pending_player_target_contract_is_supported(recipient, targets, true)
        }
        _ => false,
    }
}

fn pending_object_target_contract_is_supported(
    object: &ObjectRef,
    targets: &[Target],
    executor_fans_out: bool,
) -> bool {
    let target_ids = match object {
        ObjectRef::Target(id) => std::slice::from_ref(id),
        ObjectRef::TargetSet(ids) if !ids.is_empty() => ids.as_slice(),
        _ => return false,
    };
    target_ids.iter().all(|id| {
        targets
            .iter()
            .find(|target| target.id == *id)
            .is_some_and(|target| {
                !matches!(target.filter, TargetFilter::Player)
                    && (executor_fans_out || pending_target_maximum(target) <= 1)
            })
    })
}

fn pending_player_target_contract_is_supported(
    player: &PlayerRef,
    targets: &[Target],
    executor_fans_out: bool,
) -> bool {
    let PlayerRef::TargetPlayer(id) = player else {
        return matches!(player, PlayerRef::You | PlayerRef::PlayerIdentity(_));
    };
    targets
        .iter()
        .find(|target| target.id == *id)
        .is_some_and(|target| {
            matches!(target.filter, TargetFilter::Player)
                && (executor_fans_out || pending_target_maximum(target) <= 1)
        })
}

fn pending_target_maximum(target: &Target) -> u16 {
    match target.amount {
        TargetAmount::Exactly(amount) | TargetAmount::UpTo(amount) => amount,
        TargetAmount::All => u16::MAX,
    }
}

fn pending_amount_is_locked(amount: &Amount) -> bool {
    matches!(amount, Amount::Constant(_) | Amount::X)
}

fn pending_player_ref_is_supported(player: &PlayerRef) -> bool {
    matches!(
        player,
        PlayerRef::You | PlayerRef::PlayerIdentity(_) | PlayerRef::TargetPlayer(_)
    )
}

fn pending_target_object_ref_is_supported(object: &ObjectRef) -> bool {
    match object {
        ObjectRef::Target(_) => true,
        ObjectRef::TargetSet(targets) => !targets.is_empty(),
        _ => false,
    }
}

fn effect_has_only_illegal_targets(
    effect: &Effect,
    legal_targets: &BTreeMap<u8, Vec<SelectedTarget>>,
) -> bool {
    let target_ids = effect_target_ids(effect);
    !target_ids.is_empty()
        && target_ids
            .iter()
            .all(|id| legal_targets.get(id).is_none_or(Vec::is_empty))
}

fn effect_target_ids(effect: &Effect) -> BTreeSet<u8> {
    let mut targets = BTreeSet::new();
    match effect {
        Effect::Counter { object }
        | Effect::CounterToZone { object, .. }
        | Effect::Destroy { object }
        | Effect::Tap { object }
        | Effect::Untap { object }
        | Effect::Transform { object }
        | Effect::PutCounter { object, .. } => collect_object_ref_targets(object, &mut targets),
        Effect::MoveZone(move_zone) => {
            collect_object_ref_targets(&move_zone.object, &mut targets);
        }
        Effect::Draw { player, .. }
        | Effect::GainLife { player, .. }
        | Effect::LoseLife { player, .. } => collect_player_ref_targets(player, &mut targets),
        Effect::Damage { recipient, .. } => collect_player_ref_targets(recipient, &mut targets),
        _ => {}
    }
    targets
}

fn collect_object_ref_targets(object: &ObjectRef, targets: &mut BTreeSet<u8>) {
    match object {
        ObjectRef::Target(id) => {
            targets.insert(*id);
        }
        ObjectRef::TargetSet(ids) => targets.extend(ids.iter().copied()),
        _ => {}
    }
}

fn collect_player_ref_targets(player: &PlayerRef, targets: &mut BTreeSet<u8>) {
    if let PlayerRef::TargetPlayer(id) = player {
        targets.insert(*id);
    }
}
