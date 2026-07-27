//! Fail-closed execution primitives for a future rules-backed simulator.
//!
//! This module deliberately has no dependency on the existing role-based
//! trajectory engine. It provides object-level zones, transactional costs,
//! typed events, and atomic actions. It does not parse Oracle text and it does
//! not claim that every Magic card is supported. A compiler may hand this
//! kernel only an [`ActionProgram::Supported`] program after every relevant
//! timing, cost, condition, target, choice, and effect has been represented.
//! [`ActionProgram::Unsupported`] is rejected before any state is touched.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const STRICT_ENGINE_VERSION: &str = "strict-kernel-0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaUnitId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Command,
    Stack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZonePlacement {
    Default,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardType {
    Artifact,
    Battle,
    Creature,
    Enchantment,
    Instant,
    Kindred,
    Land,
    Planeswalker,
    Sorcery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameObject {
    pub id: ObjectId,
    /// Physical-card identity remains stable across zone changes. `incarnation`
    /// increments so later target/last-known-information work can distinguish
    /// the new rules object created by a zone change.
    pub incarnation: u32,
    pub card_key: String,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub card_types: BTreeSet<CardType>,
    pub tapped: bool,
    pub summoning_sick: bool,
    pub is_commander: bool,
    pub is_token: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub id: PlayerId,
    pub life: i32,
    pub poison: u8,
    /// The front of the deque is the top of the library.
    pub library: VecDeque<ObjectId>,
    pub hand: Vec<ObjectId>,
    pub graveyard: Vec<ObjectId>,
    pub exile: Vec<ObjectId>,
    pub command: Vec<ObjectId>,
}

impl PlayerState {
    fn new(id: PlayerId, life: i32) -> Self {
        Self {
            id,
            life,
            poison: 0,
            library: VecDeque::new(),
            hand: Vec::new(),
            graveyard: Vec::new(),
            exile: Vec::new(),
            command: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommanderCastState {
    pub owner: PlayerId,
    pub casts_from_command: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManaUnit {
    pub id: ManaUnitId,
    pub color: ManaColor,
    /// A snow mana symbol may be paid only by a unit whose source had the snow
    /// supertype when the mana was produced.
    pub from_snow_source: bool,
    pub source: Option<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaSymbol {
    Generic(u16),
    Colored(ManaColor),
    Colorless,
    Hybrid(ManaColor, ManaColor),
    VariableX,
    Phyrexian(ManaColor),
    Snow,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManaCost {
    pub symbols: Vec<ManaSymbol>,
}

impl ManaCost {
    pub fn generic(amount: u16) -> Self {
        Self {
            symbols: vec![ManaSymbol::Generic(amount)],
        }
    }

    pub fn with_added_generic(&self, amount: u16) -> Self {
        let mut result = self.clone();
        if amount > 0 {
            result.symbols.push(ManaSymbol::Generic(amount));
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhyrexianPayment {
    Mana,
    Life,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManaPaymentChoices {
    /// Chosen once for the action. Each `{X}` in the cost contributes this much
    /// generic mana.
    pub x_value: u16,
    /// One explicit choice for each Phyrexian symbol in source order.
    pub phyrexian: Vec<PhyrexianPayment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostPart {
    Mana {
        cost: ManaCost,
        choices: ManaPaymentChoices,
    },
    Tap(ObjectId),
    Sacrifice(ObjectId),
    Discard(ObjectId),
    Exile {
        object: ObjectId,
        from: Zone,
    },
    PayLife(u16),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompositeCost {
    pub parts: Vec<CostPart>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerRelation {
    Any,
    Actor,
    Opponent,
    Exact(PlayerId),
}

impl PlayerRelation {
    fn matches(self, actor: PlayerId, candidate: PlayerId) -> bool {
        match self {
            Self::Any => true,
            Self::Actor => actor == candidate,
            Self::Opponent => actor != candidate,
            Self::Exact(expected) => expected == candidate,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSpec {
    pub minimum: usize,
    pub maximum: usize,
    pub allowed_zones: BTreeSet<Zone>,
    pub owner: PlayerRelation,
    pub controller: PlayerRelation,
    pub required_type: Option<CardType>,
}

impl TargetSpec {
    pub fn exactly_one(zone: Zone) -> Self {
        Self {
            minimum: 1,
            maximum: 1,
            allowed_zones: BTreeSet::from([zone]),
            owner: PlayerRelation::Any,
            controller: PlayerRelation::Any,
            required_type: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneChangeReason {
    Cast,
    Draw,
    Discard,
    CostSacrifice,
    CostExile,
    Effect,
    Resolution,
    CommanderReturn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtomicEffect {
    MoveObject {
        object: ObjectId,
        expected_from: Zone,
        to: Zone,
        placement: ZonePlacement,
        reason: ZoneChangeReason,
    },
    MoveTarget {
        target_index: usize,
        expected_from: Zone,
        to: Zone,
        placement: ZonePlacement,
        reason: ZoneChangeReason,
    },
    Draw {
        player: PlayerId,
        count: u16,
    },
    Discard {
        player: PlayerId,
        object: ObjectId,
    },
    AddMana {
        player: PlayerId,
        color: ManaColor,
        from_snow_source: bool,
        source: Option<ObjectId>,
    },
    RecordCommanderCast {
        commander: ObjectId,
    },
    NoOp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedReason {
    pub code: String,
    pub detail: String,
}

impl UnsupportedReason {
    pub fn new(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionProgram {
    Supported(Vec<AtomicEffect>),
    Unsupported(UnsupportedReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicAction {
    pub actor: PlayerId,
    pub cost: CompositeCost,
    pub targets: Vec<ObjectId>,
    pub target_spec: Option<TargetSpec>,
    pub program: ActionProgram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnStep {
    Untap,
    Upkeep,
    Draw,
    PrecombatMain,
    BeginningOfCombat,
    DeclareAttackers,
    DeclareBlockers,
    CombatDamage,
    EndOfCombat,
    PostcombatMain,
    EndStep,
    Cleanup,
}

impl TurnStep {
    fn next(self) -> Option<Self> {
        match self {
            Self::Untap => Some(Self::Upkeep),
            Self::Upkeep => Some(Self::Draw),
            Self::Draw => Some(Self::PrecombatMain),
            Self::PrecombatMain => Some(Self::BeginningOfCombat),
            Self::BeginningOfCombat => Some(Self::DeclareAttackers),
            Self::DeclareAttackers => Some(Self::DeclareBlockers),
            Self::DeclareBlockers => Some(Self::CombatDamage),
            Self::CombatDamage => Some(Self::EndOfCombat),
            Self::EndOfCombat => Some(Self::PostcombatMain),
            Self::PostcombatMain => Some(Self::EndStep),
            Self::EndStep => Some(Self::Cleanup),
            Self::Cleanup => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnState {
    pub turn_number: u32,
    pub active_player: PlayerId,
    pub step: TurnStep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifePaymentReason {
    Cost,
    PhyrexianMana,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    ActionStarted {
        action: ActionId,
        actor: PlayerId,
    },
    ActionCompleted {
        action: ActionId,
    },
    ManaAdded {
        player: PlayerId,
        unit: ManaUnit,
    },
    ManaPaid {
        player: PlayerId,
        units: Vec<ManaUnitId>,
        x_value: u16,
    },
    ManaEmptied {
        player: PlayerId,
        units: Vec<ManaUnitId>,
    },
    LifePaid {
        player: PlayerId,
        amount: u16,
        reason: LifePaymentReason,
    },
    ObjectTapped {
        object: ObjectId,
    },
    ZoneChanged {
        object: ObjectId,
        from: Zone,
        to: Zone,
        new_incarnation: u32,
        reason: ZoneChangeReason,
    },
    CardDrawn {
        player: PlayerId,
        object: ObjectId,
    },
    CardDiscarded {
        player: PlayerId,
        object: ObjectId,
    },
    CommanderCastRecorded {
        commander: ObjectId,
        casts_from_command: u16,
        next_tax: u16,
    },
    StepAdvanced {
        from: TurnState,
        to: TurnState,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameEvent {
    pub sequence: u64,
    pub kind: EventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionReceipt {
    pub action: ActionId,
    pub first_event_sequence: u64,
    pub last_event_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostFailure {
    InsufficientMana,
    InvalidPhyrexianChoices,
    UnexpectedXChoice,
    InsufficientLife,
    ObjectNotControlled(ObjectId),
    ObjectNotInZone {
        object: ObjectId,
        expected: Zone,
        actual: Zone,
    },
    ObjectAlreadyTapped(ObjectId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    UnknownPlayer(PlayerId),
    UnknownObject(ObjectId),
    InvalidAction(String),
    IllegalTarget { object: ObjectId, reason: String },
    Cost(CostFailure),
    InvariantViolation(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionResult {
    Applied(ActionReceipt),
    Rejected(ExecutionError),
    Unsupported(UnsupportedReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    pub players: BTreeMap<PlayerId, PlayerState>,
    pub objects: BTreeMap<ObjectId, GameObject>,
    pub battlefield: Vec<ObjectId>,
    pub stack: Vec<ObjectId>,
    pub mana_pools: BTreeMap<PlayerId, Vec<ManaUnit>>,
    pub commander_casts: BTreeMap<ObjectId, CommanderCastState>,
    pub turn_order: Vec<PlayerId>,
    pub turn: Option<TurnState>,
    pub events: Vec<GameEvent>,
    next_player_id: u16,
    next_object_id: u64,
    next_mana_unit_id: u64,
    next_action_id: u64,
    next_event_sequence: u64,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            players: BTreeMap::new(),
            objects: BTreeMap::new(),
            battlefield: Vec::new(),
            stack: Vec::new(),
            mana_pools: BTreeMap::new(),
            commander_casts: BTreeMap::new(),
            turn_order: Vec::new(),
            turn: None,
            events: Vec::new(),
            next_player_id: 1,
            next_object_id: 1,
            next_mana_unit_id: 1,
            next_action_id: 1,
            next_event_sequence: 1,
        }
    }
}

impl GameState {
    pub fn add_player(&mut self, life: i32) -> PlayerId {
        let id = PlayerId(self.next_player_id);
        self.next_player_id = self.next_player_id.saturating_add(1);
        self.players.insert(id, PlayerState::new(id, life));
        self.mana_pools.insert(id, Vec::new());
        self.turn_order.push(id);
        if self.turn.is_none() {
            self.turn = Some(TurnState {
                turn_number: 1,
                active_player: id,
                step: TurnStep::Untap,
            });
        }
        id
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_object(
        &mut self,
        owner: PlayerId,
        controller: PlayerId,
        card_key: impl Into<String>,
        card_types: impl IntoIterator<Item = CardType>,
        zone: Zone,
        placement: ZonePlacement,
        is_commander: bool,
        is_token: bool,
    ) -> Result<ObjectId, ExecutionError> {
        self.require_player(owner)?;
        self.require_player(controller)?;
        let id = ObjectId(self.next_object_id);
        self.next_object_id = self.next_object_id.saturating_add(1);
        let object = GameObject {
            id,
            incarnation: 0,
            card_key: card_key.into(),
            owner,
            controller,
            zone,
            card_types: card_types.into_iter().collect(),
            tapped: false,
            summoning_sick: false,
            is_commander,
            is_token,
        };
        self.objects.insert(id, object);
        self.attach_to_zone(id, zone, placement)?;
        if is_commander {
            self.commander_casts.insert(
                id,
                CommanderCastState {
                    owner,
                    casts_from_command: 0,
                },
            );
        }
        self.validate_invariants()?;
        Ok(id)
    }

    pub fn object(&self, id: ObjectId) -> Result<&GameObject, ExecutionError> {
        self.objects
            .get(&id)
            .ok_or(ExecutionError::UnknownObject(id))
    }

    pub fn commander_tax(&self, commander: ObjectId) -> Result<u16, ExecutionError> {
        let state = self.commander_casts.get(&commander).ok_or_else(|| {
            ExecutionError::InvalidAction(format!(
                "Object {} is not registered as a commander.",
                commander.0
            ))
        })?;
        Ok(state.casts_from_command.saturating_mul(2))
    }

    pub fn execute_commander_cast(
        &mut self,
        actor: PlayerId,
        commander: ObjectId,
        printed_cost: ManaCost,
        choices: ManaPaymentChoices,
    ) -> ExecutionResult {
        let tax = match self.commander_tax(commander) {
            Ok(tax) => tax,
            Err(error) => return ExecutionResult::Rejected(error),
        };
        let action = AtomicAction {
            actor,
            cost: CompositeCost {
                parts: vec![CostPart::Mana {
                    cost: printed_cost.with_added_generic(tax),
                    choices,
                }],
            },
            targets: Vec::new(),
            target_spec: None,
            program: ActionProgram::Supported(vec![
                AtomicEffect::MoveObject {
                    object: commander,
                    expected_from: Zone::Command,
                    to: Zone::Stack,
                    placement: ZonePlacement::Default,
                    reason: ZoneChangeReason::Cast,
                },
                AtomicEffect::RecordCommanderCast { commander },
            ]),
        };
        self.execute_action(action)
    }

    pub fn execute_action(&mut self, action: AtomicAction) -> ExecutionResult {
        if let ActionProgram::Unsupported(reason) = &action.program {
            return ExecutionResult::Unsupported(reason.clone());
        }

        let checkpoint = self.clone();
        match self.execute_supported_action(action) {
            Ok(receipt) => ExecutionResult::Applied(receipt),
            Err(error) => {
                *self = checkpoint;
                ExecutionResult::Rejected(error)
            }
        }
    }

    fn execute_supported_action(
        &mut self,
        action: AtomicAction,
    ) -> Result<ActionReceipt, ExecutionError> {
        self.require_player(action.actor)?;
        self.validate_targets(action.actor, &action.targets, action.target_spec.as_ref())?;

        let action_id = ActionId(self.next_action_id);
        self.next_action_id = self.next_action_id.saturating_add(1);
        let first_event_sequence = self.next_event_sequence;
        self.push_event(EventKind::ActionStarted {
            action: action_id,
            actor: action.actor,
        });

        for cost in &action.cost.parts {
            self.pay_cost_part(action.actor, cost)?;
        }

        let effects = match action.program {
            ActionProgram::Supported(effects) => effects,
            ActionProgram::Unsupported(_) => {
                return Err(ExecutionError::InvalidAction(
                    "Unsupported program reached the supported executor.".into(),
                ));
            }
        };
        for effect in effects {
            self.apply_effect(action.actor, &action.targets, effect)?;
        }
        self.validate_invariants()?;
        self.push_event(EventKind::ActionCompleted { action: action_id });
        Ok(ActionReceipt {
            action: action_id,
            first_event_sequence,
            last_event_sequence: self.next_event_sequence.saturating_sub(1),
        })
    }

    fn validate_targets(
        &self,
        actor: PlayerId,
        targets: &[ObjectId],
        spec: Option<&TargetSpec>,
    ) -> Result<(), ExecutionError> {
        let Some(spec) = spec else {
            if targets.is_empty() {
                return Ok(());
            }
            return Err(ExecutionError::InvalidAction(
                "Targets were supplied without a target specification.".into(),
            ));
        };
        if spec.minimum > spec.maximum {
            return Err(ExecutionError::InvalidAction(
                "Target minimum exceeds target maximum.".into(),
            ));
        }
        if !(spec.minimum..=spec.maximum).contains(&targets.len()) {
            return Err(ExecutionError::InvalidAction(format!(
                "Expected {}..={} targets, received {}.",
                spec.minimum,
                spec.maximum,
                targets.len()
            )));
        }
        let unique = targets.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != targets.len() {
            return Err(ExecutionError::InvalidAction(
                "The same object cannot satisfy multiple target slots.".into(),
            ));
        }
        for target in targets {
            let object = self.object(*target)?;
            if !spec.allowed_zones.contains(&object.zone) {
                return Err(ExecutionError::IllegalTarget {
                    object: *target,
                    reason: format!("Object is in {:?}.", object.zone),
                });
            }
            if !spec.owner.matches(actor, object.owner) {
                return Err(ExecutionError::IllegalTarget {
                    object: *target,
                    reason: "Object owner does not satisfy the target predicate.".into(),
                });
            }
            if !spec.controller.matches(actor, object.controller) {
                return Err(ExecutionError::IllegalTarget {
                    object: *target,
                    reason: "Object controller does not satisfy the target predicate.".into(),
                });
            }
            if spec
                .required_type
                .is_some_and(|required| !object.card_types.contains(&required))
            {
                return Err(ExecutionError::IllegalTarget {
                    object: *target,
                    reason: "Object does not have the required card type.".into(),
                });
            }
        }
        Ok(())
    }

    fn pay_cost_part(&mut self, actor: PlayerId, part: &CostPart) -> Result<(), ExecutionError> {
        match part {
            CostPart::Mana { cost, choices } => self.pay_mana_cost(actor, cost, choices),
            CostPart::Tap(object) => {
                let candidate = self.object(*object)?;
                if candidate.controller != actor {
                    return Err(ExecutionError::Cost(CostFailure::ObjectNotControlled(
                        *object,
                    )));
                }
                if candidate.zone != Zone::Battlefield {
                    return Err(ExecutionError::Cost(CostFailure::ObjectNotInZone {
                        object: *object,
                        expected: Zone::Battlefield,
                        actual: candidate.zone,
                    }));
                }
                if candidate.tapped {
                    return Err(ExecutionError::Cost(CostFailure::ObjectAlreadyTapped(
                        *object,
                    )));
                }
                self.objects
                    .get_mut(object)
                    .expect("validated object exists")
                    .tapped = true;
                self.push_event(EventKind::ObjectTapped { object: *object });
                Ok(())
            }
            CostPart::Sacrifice(object) => {
                let candidate = self.object(*object)?;
                if candidate.controller != actor {
                    return Err(ExecutionError::Cost(CostFailure::ObjectNotControlled(
                        *object,
                    )));
                }
                if candidate.zone != Zone::Battlefield {
                    return Err(ExecutionError::Cost(CostFailure::ObjectNotInZone {
                        object: *object,
                        expected: Zone::Battlefield,
                        actual: candidate.zone,
                    }));
                }
                self.move_object_internal(
                    *object,
                    Zone::Graveyard,
                    ZonePlacement::Default,
                    ZoneChangeReason::CostSacrifice,
                )
            }
            CostPart::Discard(object) => {
                let candidate = self.object(*object)?;
                if candidate.owner != actor {
                    return Err(ExecutionError::Cost(CostFailure::ObjectNotControlled(
                        *object,
                    )));
                }
                if candidate.zone != Zone::Hand {
                    return Err(ExecutionError::Cost(CostFailure::ObjectNotInZone {
                        object: *object,
                        expected: Zone::Hand,
                        actual: candidate.zone,
                    }));
                }
                self.discard_internal(actor, *object)
            }
            CostPart::Exile { object, from } => {
                let candidate = self.object(*object)?;
                let controls_cost_object = match *from {
                    Zone::Battlefield | Zone::Stack => candidate.controller == actor,
                    _ => candidate.owner == actor,
                };
                if !controls_cost_object {
                    return Err(ExecutionError::Cost(CostFailure::ObjectNotControlled(
                        *object,
                    )));
                }
                if candidate.zone != *from {
                    return Err(ExecutionError::Cost(CostFailure::ObjectNotInZone {
                        object: *object,
                        expected: *from,
                        actual: candidate.zone,
                    }));
                }
                self.move_object_internal(
                    *object,
                    Zone::Exile,
                    ZonePlacement::Default,
                    ZoneChangeReason::CostExile,
                )
            }
            CostPart::PayLife(amount) => self.pay_life(actor, *amount, LifePaymentReason::Cost),
        }
    }

    fn pay_mana_cost(
        &mut self,
        actor: PlayerId,
        cost: &ManaCost,
        choices: &ManaPaymentChoices,
    ) -> Result<(), ExecutionError> {
        self.require_player(actor)?;
        let phyrexian_count = cost
            .symbols
            .iter()
            .filter(|symbol| matches!(symbol, ManaSymbol::Phyrexian(_)))
            .count();
        if choices.phyrexian.len() != phyrexian_count {
            return Err(ExecutionError::Cost(CostFailure::InvalidPhyrexianChoices));
        }
        let x_count = cost
            .symbols
            .iter()
            .filter(|symbol| matches!(symbol, ManaSymbol::VariableX))
            .count();
        if x_count == 0 && choices.x_value != 0 {
            return Err(ExecutionError::Cost(CostFailure::UnexpectedXChoice));
        }

        let mut generic_due = 0u16;
        let mut requirements = Vec::new();
        let mut phyrexian_life = 0u16;
        let mut phyrexian_index = 0usize;
        for symbol in &cost.symbols {
            match *symbol {
                ManaSymbol::Generic(amount) => {
                    generic_due = generic_due.checked_add(amount).ok_or_else(|| {
                        ExecutionError::InvalidAction("Generic mana cost overflowed.".into())
                    })?;
                }
                ManaSymbol::Colored(color) => {
                    requirements.push(ManaRequirement::Color(color));
                }
                ManaSymbol::Colorless => requirements.push(ManaRequirement::Colorless),
                ManaSymbol::Hybrid(first, second) => {
                    requirements.push(ManaRequirement::Hybrid(first, second));
                }
                ManaSymbol::VariableX => {
                    generic_due = generic_due.checked_add(choices.x_value).ok_or_else(|| {
                        ExecutionError::InvalidAction("Variable mana cost overflowed.".into())
                    })?;
                }
                ManaSymbol::Phyrexian(color) => {
                    match choices.phyrexian[phyrexian_index] {
                        PhyrexianPayment::Mana => {
                            requirements.push(ManaRequirement::Color(color));
                        }
                        PhyrexianPayment::Life => {
                            phyrexian_life = phyrexian_life.checked_add(2).ok_or_else(|| {
                                ExecutionError::InvalidAction(
                                    "Phyrexian life payment overflowed.".into(),
                                )
                            })?;
                        }
                    }
                    phyrexian_index += 1;
                }
                ManaSymbol::Snow => requirements.push(ManaRequirement::Snow),
            }
        }

        let pool = self
            .mana_pools
            .get(&actor)
            .ok_or(ExecutionError::UnknownPlayer(actor))?;
        let mut used = vec![false; pool.len()];
        let Some(mut selected_indices) =
            select_exact_mana(pool, &requirements, 0, &mut used, generic_due as usize)
        else {
            return Err(ExecutionError::Cost(CostFailure::InsufficientMana));
        };
        let mut generic_indices = pool
            .iter()
            .enumerate()
            .filter(|(index, _)| !selected_indices.contains(index))
            .map(|(index, _)| index)
            .take(generic_due as usize)
            .collect::<Vec<_>>();
        if generic_indices.len() != generic_due as usize {
            return Err(ExecutionError::Cost(CostFailure::InsufficientMana));
        }
        selected_indices.append(&mut generic_indices);
        selected_indices.sort_unstable();
        selected_indices.dedup();

        if phyrexian_life > 0 {
            let life = self
                .players
                .get(&actor)
                .ok_or(ExecutionError::UnknownPlayer(actor))?
                .life;
            if life < i32::from(phyrexian_life) {
                return Err(ExecutionError::Cost(CostFailure::InsufficientLife));
            }
        }

        let selected_ids = selected_indices
            .iter()
            .map(|index| pool[*index].id)
            .collect::<Vec<_>>();
        let selected_id_set = selected_ids.iter().copied().collect::<BTreeSet<_>>();
        self.mana_pools
            .get_mut(&actor)
            .expect("validated player has a mana pool")
            .retain(|unit| !selected_id_set.contains(&unit.id));
        self.push_event(EventKind::ManaPaid {
            player: actor,
            units: selected_ids,
            x_value: choices.x_value,
        });
        if phyrexian_life > 0 {
            self.pay_life(actor, phyrexian_life, LifePaymentReason::PhyrexianMana)?;
        }
        Ok(())
    }

    fn pay_life(
        &mut self,
        player: PlayerId,
        amount: u16,
        reason: LifePaymentReason,
    ) -> Result<(), ExecutionError> {
        let state = self
            .players
            .get_mut(&player)
            .ok_or(ExecutionError::UnknownPlayer(player))?;
        if state.life < i32::from(amount) {
            return Err(ExecutionError::Cost(CostFailure::InsufficientLife));
        }
        state.life -= i32::from(amount);
        self.push_event(EventKind::LifePaid {
            player,
            amount,
            reason,
        });
        Ok(())
    }

    fn apply_effect(
        &mut self,
        actor: PlayerId,
        targets: &[ObjectId],
        effect: AtomicEffect,
    ) -> Result<(), ExecutionError> {
        match effect {
            AtomicEffect::MoveObject {
                object,
                expected_from,
                to,
                placement,
                reason,
            } => {
                self.require_zone(object, expected_from)?;
                self.move_object_internal(object, to, placement, reason)
            }
            AtomicEffect::MoveTarget {
                target_index,
                expected_from,
                to,
                placement,
                reason,
            } => {
                let object = targets.get(target_index).copied().ok_or_else(|| {
                    ExecutionError::InvalidAction(format!(
                        "Target index {target_index} is out of bounds."
                    ))
                })?;
                self.require_zone(object, expected_from)?;
                self.move_object_internal(object, to, placement, reason)
            }
            AtomicEffect::Draw { player, count } => {
                for _ in 0..count {
                    self.draw_one(player)?;
                }
                Ok(())
            }
            AtomicEffect::Discard { player, object } => self.discard_internal(player, object),
            AtomicEffect::AddMana {
                player,
                color,
                from_snow_source,
                source,
            } => {
                self.require_player(player)?;
                if let Some(source) = source {
                    self.object(source)?;
                }
                let unit = ManaUnit {
                    id: ManaUnitId(self.next_mana_unit_id),
                    color,
                    from_snow_source,
                    source,
                };
                self.next_mana_unit_id = self.next_mana_unit_id.saturating_add(1);
                self.mana_pools
                    .get_mut(&player)
                    .expect("validated player has a mana pool")
                    .push(unit);
                self.push_event(EventKind::ManaAdded { player, unit });
                Ok(())
            }
            AtomicEffect::RecordCommanderCast { commander } => {
                let object = self.object(commander)?;
                if !object.is_commander || object.controller != actor || object.zone != Zone::Stack
                {
                    return Err(ExecutionError::InvalidAction(
                        "Commander cast recording requires the actor's commander on the stack."
                            .into(),
                    ));
                }
                let state = self.commander_casts.get_mut(&commander).ok_or_else(|| {
                    ExecutionError::InvalidAction(
                        "Commander has no independent cast-state record.".into(),
                    )
                })?;
                state.casts_from_command = state.casts_from_command.saturating_add(1);
                let casts_from_command = state.casts_from_command;
                self.push_event(EventKind::CommanderCastRecorded {
                    commander,
                    casts_from_command,
                    next_tax: casts_from_command.saturating_mul(2),
                });
                Ok(())
            }
            AtomicEffect::NoOp => Ok(()),
        }
    }

    fn draw_one(&mut self, player: PlayerId) -> Result<(), ExecutionError> {
        let object = self
            .players
            .get(&player)
            .ok_or(ExecutionError::UnknownPlayer(player))?
            .library
            .front()
            .copied()
            .ok_or_else(|| {
                ExecutionError::InvalidAction(
                    "Drawing from an empty library needs a typed loss/state-based-action handler."
                        .into(),
                )
            })?;
        self.move_object_internal(
            object,
            Zone::Hand,
            ZonePlacement::Default,
            ZoneChangeReason::Draw,
        )?;
        self.push_event(EventKind::CardDrawn { player, object });
        Ok(())
    }

    fn discard_internal(
        &mut self,
        player: PlayerId,
        object: ObjectId,
    ) -> Result<(), ExecutionError> {
        let candidate = self.object(object)?;
        if candidate.owner != player {
            return Err(ExecutionError::Cost(CostFailure::ObjectNotControlled(
                object,
            )));
        }
        self.require_zone(object, Zone::Hand)?;
        self.move_object_internal(
            object,
            Zone::Graveyard,
            ZonePlacement::Default,
            ZoneChangeReason::Discard,
        )?;
        self.push_event(EventKind::CardDiscarded { player, object });
        Ok(())
    }

    fn move_object_internal(
        &mut self,
        object: ObjectId,
        to: Zone,
        placement: ZonePlacement,
        reason: ZoneChangeReason,
    ) -> Result<(), ExecutionError> {
        let from = self.object(object)?.zone;
        if from == to {
            return Err(ExecutionError::InvalidAction(format!(
                "Object {} is already in {:?}.",
                object.0, to
            )));
        }
        self.detach_from_zone(object, from)?;
        {
            let candidate = self
                .objects
                .get_mut(&object)
                .expect("validated object exists");
            candidate.zone = to;
            candidate.incarnation = candidate.incarnation.saturating_add(1);
            candidate.tapped = false;
            candidate.summoning_sick = false;
            if matches!(
                to,
                Zone::Library | Zone::Hand | Zone::Graveyard | Zone::Exile | Zone::Command
            ) {
                candidate.controller = candidate.owner;
            }
        }
        self.attach_to_zone(object, to, placement)?;
        let new_incarnation = self.object(object)?.incarnation;
        self.push_event(EventKind::ZoneChanged {
            object,
            from,
            to,
            new_incarnation,
            reason,
        });
        Ok(())
    }

    fn detach_from_zone(&mut self, object: ObjectId, zone: Zone) -> Result<(), ExecutionError> {
        let owner = self.object(object)?.owner;
        let removed = match zone {
            Zone::Library => remove_from_deque(
                &mut self
                    .players
                    .get_mut(&owner)
                    .ok_or(ExecutionError::UnknownPlayer(owner))?
                    .library,
                object,
            ),
            Zone::Hand => remove_from_vec(
                &mut self
                    .players
                    .get_mut(&owner)
                    .ok_or(ExecutionError::UnknownPlayer(owner))?
                    .hand,
                object,
            ),
            Zone::Battlefield => remove_from_vec(&mut self.battlefield, object),
            Zone::Graveyard => remove_from_vec(
                &mut self
                    .players
                    .get_mut(&owner)
                    .ok_or(ExecutionError::UnknownPlayer(owner))?
                    .graveyard,
                object,
            ),
            Zone::Exile => remove_from_vec(
                &mut self
                    .players
                    .get_mut(&owner)
                    .ok_or(ExecutionError::UnknownPlayer(owner))?
                    .exile,
                object,
            ),
            Zone::Command => remove_from_vec(
                &mut self
                    .players
                    .get_mut(&owner)
                    .ok_or(ExecutionError::UnknownPlayer(owner))?
                    .command,
                object,
            ),
            Zone::Stack => remove_from_vec(&mut self.stack, object),
        };
        if removed {
            Ok(())
        } else {
            Err(ExecutionError::InvariantViolation(format!(
                "Object {} says it is in {:?}, but the zone did not contain it.",
                object.0, zone
            )))
        }
    }

    fn attach_to_zone(
        &mut self,
        object: ObjectId,
        zone: Zone,
        placement: ZonePlacement,
    ) -> Result<(), ExecutionError> {
        let owner = self.object(object)?.owner;
        match zone {
            Zone::Library => {
                let library = &mut self
                    .players
                    .get_mut(&owner)
                    .ok_or(ExecutionError::UnknownPlayer(owner))?
                    .library;
                match placement {
                    ZonePlacement::Top => library.push_front(object),
                    ZonePlacement::Default | ZonePlacement::Bottom => library.push_back(object),
                }
            }
            Zone::Hand => self
                .players
                .get_mut(&owner)
                .ok_or(ExecutionError::UnknownPlayer(owner))?
                .hand
                .push(object),
            Zone::Battlefield => self.battlefield.push(object),
            Zone::Graveyard => self
                .players
                .get_mut(&owner)
                .ok_or(ExecutionError::UnknownPlayer(owner))?
                .graveyard
                .push(object),
            Zone::Exile => self
                .players
                .get_mut(&owner)
                .ok_or(ExecutionError::UnknownPlayer(owner))?
                .exile
                .push(object),
            Zone::Command => self
                .players
                .get_mut(&owner)
                .ok_or(ExecutionError::UnknownPlayer(owner))?
                .command
                .push(object),
            Zone::Stack => self.stack.push(object),
        }
        Ok(())
    }

    fn require_zone(&self, object: ObjectId, expected: Zone) -> Result<(), ExecutionError> {
        let actual = self.object(object)?.zone;
        if actual == expected {
            Ok(())
        } else {
            Err(ExecutionError::Cost(CostFailure::ObjectNotInZone {
                object,
                expected,
                actual,
            }))
        }
    }

    fn require_player(&self, player: PlayerId) -> Result<(), ExecutionError> {
        if self.players.contains_key(&player) {
            Ok(())
        } else {
            Err(ExecutionError::UnknownPlayer(player))
        }
    }

    pub fn advance_step(&mut self) -> Result<TurnState, ExecutionError> {
        let from = self
            .turn
            .ok_or_else(|| ExecutionError::InvalidAction("No active turn exists.".into()))?;
        self.empty_mana_pools();
        let to = if let Some(step) = from.step.next() {
            TurnState { step, ..from }
        } else {
            let current_position = self
                .turn_order
                .iter()
                .position(|player| *player == from.active_player)
                .ok_or_else(|| {
                    ExecutionError::InvariantViolation(
                        "The active player is missing from turn order.".into(),
                    )
                })?;
            let next_position = (current_position + 1) % self.turn_order.len().max(1);
            let active_player = *self.turn_order.get(next_position).ok_or_else(|| {
                ExecutionError::InvalidAction("Turn order contains no players.".into())
            })?;
            TurnState {
                turn_number: from.turn_number.saturating_add(1),
                active_player,
                step: TurnStep::Untap,
            }
        };
        self.turn = Some(to);
        self.push_event(EventKind::StepAdvanced { from, to });
        Ok(to)
    }

    fn empty_mana_pools(&mut self) {
        let players = self.mana_pools.keys().copied().collect::<Vec<_>>();
        for player in players {
            let units = self
                .mana_pools
                .get_mut(&player)
                .expect("key collected from the map")
                .drain(..)
                .map(|unit| unit.id)
                .collect::<Vec<_>>();
            if !units.is_empty() {
                self.push_event(EventKind::ManaEmptied { player, units });
            }
        }
    }

    pub fn validate_invariants(&self) -> Result<(), ExecutionError> {
        let mut appearances = BTreeMap::<ObjectId, usize>::new();
        for player in self.players.values() {
            for object in player
                .library
                .iter()
                .chain(player.hand.iter())
                .chain(player.graveyard.iter())
                .chain(player.exile.iter())
                .chain(player.command.iter())
            {
                *appearances.entry(*object).or_default() += 1;
            }
        }
        for object in self.battlefield.iter().chain(self.stack.iter()) {
            *appearances.entry(*object).or_default() += 1;
        }
        for (id, object) in &self.objects {
            let count = appearances.get(id).copied().unwrap_or(0);
            if count != 1 {
                return Err(ExecutionError::InvariantViolation(format!(
                    "Object {} appears in {count} zone containers.",
                    id.0
                )));
            }
            if !self.zone_contains(*id, object.zone)? {
                return Err(ExecutionError::InvariantViolation(format!(
                    "Object {} zone field disagrees with its container.",
                    id.0
                )));
            }
        }
        if appearances
            .keys()
            .any(|object| !self.objects.contains_key(object))
        {
            return Err(ExecutionError::InvariantViolation(
                "A zone contains an unknown object.".into(),
            ));
        }
        for (commander, cast_state) in &self.commander_casts {
            let object = self.object(*commander)?;
            if !object.is_commander || object.owner != cast_state.owner {
                return Err(ExecutionError::InvariantViolation(format!(
                    "Commander ledger disagrees with object {}.",
                    commander.0
                )));
            }
        }
        Ok(())
    }

    fn zone_contains(&self, object: ObjectId, zone: Zone) -> Result<bool, ExecutionError> {
        let owner = self.object(object)?.owner;
        let contains = match zone {
            Zone::Library => self
                .players
                .get(&owner)
                .ok_or(ExecutionError::UnknownPlayer(owner))?
                .library
                .contains(&object),
            Zone::Hand => self
                .players
                .get(&owner)
                .ok_or(ExecutionError::UnknownPlayer(owner))?
                .hand
                .contains(&object),
            Zone::Battlefield => self.battlefield.contains(&object),
            Zone::Graveyard => self
                .players
                .get(&owner)
                .ok_or(ExecutionError::UnknownPlayer(owner))?
                .graveyard
                .contains(&object),
            Zone::Exile => self
                .players
                .get(&owner)
                .ok_or(ExecutionError::UnknownPlayer(owner))?
                .exile
                .contains(&object),
            Zone::Command => self
                .players
                .get(&owner)
                .ok_or(ExecutionError::UnknownPlayer(owner))?
                .command
                .contains(&object),
            Zone::Stack => self.stack.contains(&object),
        };
        Ok(contains)
    }

    fn push_event(&mut self, kind: EventKind) {
        self.events.push(GameEvent {
            sequence: self.next_event_sequence,
            kind,
        });
        self.next_event_sequence = self.next_event_sequence.saturating_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManaRequirement {
    Color(ManaColor),
    Colorless,
    Hybrid(ManaColor, ManaColor),
    Snow,
}

impl ManaRequirement {
    fn accepts(self, unit: ManaUnit) -> bool {
        match self {
            Self::Color(color) => unit.color == color,
            Self::Colorless => unit.color == ManaColor::Colorless,
            Self::Hybrid(first, second) => unit.color == first || unit.color == second,
            Self::Snow => unit.from_snow_source,
        }
    }
}

fn select_exact_mana(
    pool: &[ManaUnit],
    requirements: &[ManaRequirement],
    position: usize,
    used: &mut [bool],
    generic_due: usize,
) -> Option<Vec<usize>> {
    if position == requirements.len() {
        let unused = used.iter().filter(|used| !**used).count();
        return (unused >= generic_due).then(Vec::new);
    }
    for (index, unit) in pool.iter().copied().enumerate() {
        if used[index] || !requirements[position].accepts(unit) {
            continue;
        }
        used[index] = true;
        if let Some(mut selected) =
            select_exact_mana(pool, requirements, position + 1, used, generic_due)
        {
            selected.push(index);
            return Some(selected);
        }
        used[index] = false;
    }
    None
}

fn remove_from_vec(values: &mut Vec<ObjectId>, object: ObjectId) -> bool {
    let Some(position) = values.iter().position(|candidate| *candidate == object) else {
        return false;
    };
    values.remove(position);
    true
}

fn remove_from_deque(values: &mut VecDeque<ObjectId>, object: ObjectId) -> bool {
    let Some(position) = values.iter().position(|candidate| *candidate == object) else {
        return false;
    };
    values.remove(position);
    true
}
