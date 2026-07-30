//! Deterministic execution for the bounded Oracle runtime.
//!
//! This module deliberately owns no simulator policy. [`OracleStateAdapter`]
//! exposes the physical state needed by the executor, while
//! [`InMemoryOracleState`] provides a complete reference implementation.
//! The legacy immediate executor evaluates an action against one checkpoint.
//! The pending action lifecycle separates initiation from resolution so paid
//! costs and last known source information survive stack interaction.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::bounded_oracle_mana::{
    BOUNDED_ORACLE_MANA_EXPRESSION_VERSION, CalculatedValue as TypedCalculatedValue,
    CountController as TypedCountController, CountedCardType as TypedCountedCardType,
    CountedKeyword as TypedCountedKeyword, CountedObjects as TypedCountedObjects,
    CountedSubtype as TypedCountedSubtype, DerivedManaTypes as TypedDerivedManaTypes,
    ManaColor as TypedManaColor, ManaColorDomain as TypedManaColorDomain,
    ManaComposition as TypedManaComposition,
    ManaProductionExpression as TypedManaProductionExpression, ManaQuantity as TypedManaQuantity,
    ManaRetention as TypedManaRetention, ManaSymbol as TypedManaSymbol,
    QuantityCalculation as TypedQuantityCalculation,
    ResourceCostComponent as TypedResourceCostComponent, parse_resource_cost_expression,
};
use crate::bounded_oracle_runtime::{
    ActivationRestriction, AlternativeCost, Amount, AnimateEffect, AttachmentKind,
    BOUNDED_ORACLE_RUNTIME_VERSION, BottomOrder, BounceWithControllerCopyEffect,
    BoundedOracleClause, CardType, CastCopyEffect, CastPermission, CastTiming, ChoiceCount,
    ClauseAddress, Color, Comparison, Condition, CopyDestination, CopyEffect, CopyException, Cost,
    CountExpression, CounterKind, Duration, Effect, ExileCollectionEffect, ExtraTurnEffect,
    GrantedAbility, Keyword, LibraryProcedure, LoyaltyCost, ManaChoice, ManaCost, ManaProduction,
    ObjectEventKind, ObjectFilter, ObjectRef, ObjectSelection, ObjectState, PaymentOrLoseEffect,
    PlayerActionKind, PlayerRef, PowerToughnessChange, PowerToughnessOperation, ReminderSemantics,
    RepeatSchedule, ReplacementEffect, ReplacementEvent, Restriction, SearchDestination,
    SearchLibrary, SearchOrdinal, SelectedZoneMove, SetCharacteristics, SpecialActionTiming, Step,
    Supertype, Target, TargetAmount, TargetFilter, TargetRelationship, Timing, TokenCreation,
    TokenDefinition, TokenSpecification, TopLibraryExile, Trigger, TriggerSubject, TurnPlayer,
    WardCost, Zone, ZoneMove,
};

#[path = "bounded_oracle_action_stack.rs"]
mod bounded_oracle_action_stack;

pub use bounded_oracle_action_stack::{
    BOUNDED_ORACLE_ACTION_STACK_VERSION, BeginPendingActionReceipt, BoundOracleOccurrence,
    BoundedOracleActionStack, PendingAction, PendingActionError, PendingActionId,
    PendingActionResolutionReceipt, PendingActionResolutionStatus, begin_activation,
    counter_pending_action, pending_action_clause_has_live_contract, resolve_pending_action,
};

pub const BOUNDED_ORACLE_CONSUMER_VERSION: &str = "bounded-oracle-consumer-0.9";

pub type ObjectId = u64;
pub type PlayerId = u8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerEvent {
    ObjectEntered {
        object: ObjectId,
    },
    ObjectAttacked {
        object: ObjectId,
    },
    SpellCast {
        player: PlayerId,
        spell: ObjectId,
        occurrence_this_turn: u32,
    },
    CardDrawn {
        player: PlayerId,
        card: ObjectId,
        occurrence_this_turn: u32,
    },
    ObjectEvent {
        object: ObjectId,
        event: ObjectEventKind,
    },
    LifeGained {
        player: PlayerId,
        amount: u32,
    },
    TokenCreated {
        player: PlayerId,
        token: ObjectId,
    },
    PlayerAction {
        player: PlayerId,
        action: PlayerActionKind,
        object: Option<ObjectId>,
    },
    CombatDamageToPlayer {
        source: ObjectId,
        player: PlayerId,
        amount: u32,
    },
    BecameTarget {
        object: ObjectId,
        controller: PlayerId,
        source: ObjectId,
    },
    BeginningOf {
        step: Step,
        active_player: PlayerId,
        is_next_turn: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionWindow {
    CastingAdditionalCost,
    SpellResolution,
    Activated,
    Triggered(TriggerEvent),
    Static,
    Replacement,
    ModalHeader,
    ModalBranch {
        header_clause_index: Option<u16>,
        branch_index: u16,
    },
    SpecialAction(SpecialActionTiming),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SelectedTarget {
    Player(PlayerId),
    Object(ObjectId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementOccurrence {
    pub event: ReplacementEvent,
    pub amount: u32,
    pub affected_player: Option<PlayerId>,
    pub object: Option<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionContext {
    pub actor: PlayerId,
    pub source: ObjectId,
    pub window: ActionWindow,
    pub active_player: PlayerId,
    pub sorcery_timing: bool,
    pub instant_timing: bool,
    pub is_mana_ability: bool,
    pub targets: BTreeMap<u8, Vec<SelectedTarget>>,
    pub triggering_object: Option<ObjectId>,
    pub that_objects: BTreeMap<u8, ObjectId>,
    pub searched_cards: BTreeMap<u8, ObjectId>,
    pub that_player: Option<PlayerId>,
    pub x_value: u32,
    pub chosen_amount: Option<u32>,
    pub chosen_creature_type: Option<String>,
    pub selected_modes: Vec<u16>,
    pub object_choices: BTreeMap<u8, Vec<ObjectId>>,
    pub library_choices: BTreeMap<PlayerId, OrderedLibraryChoice>,
    pub card_was_cast_with_alternative_cost: bool,
    pub first_resolution_of_named_spell: bool,
    pub payment_declined: bool,
    pub optional_effect_declined: bool,
    pub ability_occurrence_this_turn: u32,
    pub gift_promised: bool,
    pub source_was_in_opening_hand: bool,
    pub playing_first: bool,
    pub selected_card_name: Option<String>,
    /// Optional concrete mana composition for a typed production effect.
    /// When absent, the reference consumer chooses the first deterministic
    /// legal composition.
    pub mana_production_choice: Option<Vec<Color>>,
    pub accepted_library_card: Option<ObjectId>,
    pub commander_controlled: bool,
    pub opponents_dealt_combat_damage_this_turn: u32,
    pub devotion_by_color: [u32; 6],
    pub countered: bool,
    pub replacement_event: Option<ReplacementOccurrence>,
    pub replay_seed: u64,
    pub last_known_source: Option<Box<PhysicalObject>>,
}

impl ExecutionContext {
    pub fn new(actor: PlayerId, source: ObjectId, window: ActionWindow) -> Self {
        Self {
            actor,
            source,
            window,
            active_player: actor,
            sorcery_timing: true,
            instant_timing: true,
            is_mana_ability: false,
            targets: BTreeMap::new(),
            triggering_object: None,
            that_objects: BTreeMap::new(),
            searched_cards: BTreeMap::new(),
            that_player: None,
            x_value: 0,
            chosen_amount: None,
            chosen_creature_type: None,
            selected_modes: Vec::new(),
            object_choices: BTreeMap::new(),
            library_choices: BTreeMap::new(),
            card_was_cast_with_alternative_cost: false,
            first_resolution_of_named_spell: false,
            payment_declined: true,
            optional_effect_declined: false,
            ability_occurrence_this_turn: 1,
            gift_promised: false,
            source_was_in_opening_hand: false,
            playing_first: true,
            selected_card_name: None,
            mana_production_choice: None,
            accepted_library_card: None,
            commander_controlled: false,
            opponents_dealt_combat_damage_this_turn: 0,
            devotion_by_color: [0; 6],
            countered: false,
            replacement_event: None,
            replay_seed: 0,
            last_known_source: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderedLibraryChoice {
    pub keep_on_top: Vec<ObjectId>,
    pub move_away: Vec<ObjectId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectCharacteristics {
    pub names: Vec<String>,
    pub card_types: Vec<CardType>,
    pub supertypes: Vec<Supertype>,
    pub subtypes: Vec<String>,
    pub colors: Vec<Color>,
    pub mana_value: u32,
    pub power: i64,
    pub toughness: i64,
    pub keywords: Vec<Keyword>,
    pub abilities: Vec<GrantedAbility>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalObject {
    pub id: ObjectId,
    pub origin_id: ObjectId,
    pub copy_of: Option<ObjectId>,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub token: bool,
    pub tapped: bool,
    pub attacking: bool,
    pub prepared: bool,
    pub face_down: bool,
    pub active_face: u8,
    pub front: ObjectCharacteristics,
    pub back: Option<ObjectCharacteristics>,
    pub counters: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachmentRecord {
    pub source: ObjectId,
    pub target: ObjectId,
    pub kind: AttachmentKind,
}

impl PhysicalObject {
    pub fn characteristics(&self) -> &ObjectCharacteristics {
        if self.active_face == 1 {
            self.back.as_ref().unwrap_or(&self.front)
        } else {
            &self.front
        }
    }

    pub fn characteristics_mut(&mut self) -> &mut ObjectCharacteristics {
        if self.active_face == 1
            && let Some(back) = self.back.as_mut()
        {
            return back;
        }
        &mut self.front
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManaPool {
    pub colored: [u32; 6],
    pub unrestricted: u32,
}

impl ManaPool {
    pub fn total(&self) -> u32 {
        self.colored
            .iter()
            .copied()
            .fold(self.unrestricted, u32::saturating_add)
    }

    pub fn amount(&self, color: Color) -> u32 {
        self.colored[color_index(color)]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub id: PlayerId,
    pub life: i64,
    pub mana: ManaPool,
    pub commander_identity: Vec<Color>,
    pub library: Vec<ObjectId>,
    pub chosen_creature_type: Option<String>,
    pub maximum_hand_size: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousEffectRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub object_identities: Vec<ObjectId>,
    pub effect: Effect,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedTriggerRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub object_identities: Vec<ObjectId>,
    pub trigger: Trigger,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub effect: ReplacementEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictionRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub restriction: Restriction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationReductionRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub mana: ManaCost,
    pub per: CountExpression,
    pub minimum_total: Option<ManaCost>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellReductionRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub object: ObjectRef,
    pub mana: ManaCost,
    pub per: CountExpression,
    pub maximum_reduction: Option<ManaCost>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledCopyRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub copied_object_identity: ObjectId,
    pub timing: Trigger,
    pub repeat: RepeatSchedule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastPermissionRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub object_identities: Vec<ObjectId>,
    pub permission: CastPermission,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraTurnRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub player: PlayerId,
    pub lose_at_end_step: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentOrLoseRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub player: PlayerId,
    pub cost: Cost,
    pub trigger: Trigger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    Won,
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameResultRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub player: PlayerId,
    pub result: GameResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedStepRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub player: PlayerId,
    pub step: Step,
}

pub trait OracleStateAdapter {
    type Checkpoint;

    fn checkpoint(&self) -> Self::Checkpoint;
    fn restore(&mut self, checkpoint: Self::Checkpoint);

    fn player_ids(&self) -> Vec<PlayerId>;
    fn player(&self, id: PlayerId) -> Option<PlayerState>;
    fn put_player(&mut self, player: PlayerState) -> Result<(), String>;

    fn object_ids(&self) -> Vec<ObjectId>;
    fn object(&self, id: ObjectId) -> Option<PhysicalObject>;
    fn put_object(&mut self, object: PhysicalObject) -> Result<(), String>;
    fn insert_physical_object(&mut self, object: PhysicalObject) -> Result<(), String>;
    fn move_object(&mut self, id: ObjectId, zone: Zone) -> Result<(), String>;
    fn allocate_object_id(&mut self) -> ObjectId;
    fn attachment(&self, source: ObjectId) -> Option<AttachmentRecord>;
    fn set_attachment(&mut self, attachment: AttachmentRecord) -> Result<(), String>;
    fn clear_attachment(&mut self, source: ObjectId) -> Result<(), String>;

    fn pay_mana(&mut self, player: PlayerId, cost: &ManaCost, x_value: u32) -> Result<(), String>;
    fn can_pay_mana(&self, player: PlayerId, cost: &ManaCost, x_value: u32) -> bool;
    fn add_mana(&mut self, player: PlayerId, colors: &[Color], amount: u32) -> Result<(), String>;

    fn next_order(&mut self) -> u64;
    fn continuous_effects(&self) -> Vec<ContinuousEffectRecord>;
    fn replacements(&self) -> Vec<ReplacementRecord>;
    fn restrictions(&self) -> Vec<RestrictionRecord>;
    fn register_continuous(&mut self, record: ContinuousEffectRecord);
    fn register_delayed_trigger(&mut self, record: DelayedTriggerRecord);
    fn register_replacement(&mut self, record: ReplacementRecord);
    fn register_restriction(&mut self, record: RestrictionRecord);
    fn register_activation_reduction(&mut self, record: ActivationReductionRecord);
    fn spell_reductions(&self) -> Vec<SpellReductionRecord>;
    fn register_spell_reduction(&mut self, record: SpellReductionRecord);
    fn register_scheduled_copy(&mut self, record: ScheduledCopyRecord);
    fn register_cast_permission(&mut self, record: CastPermissionRecord);
    fn register_extra_turn(&mut self, record: ExtraTurnRecord);
    fn register_payment_or_lose(&mut self, record: PaymentOrLoseRecord);
    fn register_game_result(&mut self, record: GameResultRecord);
    fn game_results(&self) -> Vec<GameResultRecord>;
    fn register_skipped_step(&mut self, record: SkippedStepRecord);
    fn looked_at(&self, player: PlayerId) -> Vec<ObjectId>;
    fn put_looked_at(&mut self, player: PlayerId, objects: Vec<ObjectId>);
    fn loyalty_ability_activated_this_turn(&self, source: ObjectId) -> bool;
    fn record_loyalty_ability_activation(&mut self, source: ObjectId) -> Result<(), String>;
    fn record_mutation(&mut self, description: String);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InMemoryOracleState {
    pub players: BTreeMap<PlayerId, PlayerState>,
    pub objects: BTreeMap<ObjectId, PhysicalObject>,
    pub attachments: BTreeMap<ObjectId, AttachmentRecord>,
    pub continuous_effects: Vec<ContinuousEffectRecord>,
    pub delayed_triggers: Vec<DelayedTriggerRecord>,
    pub replacement_effects: Vec<ReplacementRecord>,
    pub restriction_effects: Vec<RestrictionRecord>,
    pub activation_reductions: Vec<ActivationReductionRecord>,
    pub spell_reductions: Vec<SpellReductionRecord>,
    pub scheduled_copies: Vec<ScheduledCopyRecord>,
    pub cast_permissions: Vec<CastPermissionRecord>,
    pub extra_turns: Vec<ExtraTurnRecord>,
    pub payment_or_lose: Vec<PaymentOrLoseRecord>,
    pub game_results: Vec<GameResultRecord>,
    pub skipped_steps: Vec<SkippedStepRecord>,
    pub looked_at: BTreeMap<PlayerId, Vec<ObjectId>>,
    pub loyalty_activations_this_turn: BTreeSet<ObjectId>,
    pub mutation_log: Vec<String>,
    next_object_id: ObjectId,
    next_order: u64,
}

impl Default for InMemoryOracleState {
    fn default() -> Self {
        Self {
            players: BTreeMap::new(),
            objects: BTreeMap::new(),
            attachments: BTreeMap::new(),
            continuous_effects: Vec::new(),
            delayed_triggers: Vec::new(),
            replacement_effects: Vec::new(),
            restriction_effects: Vec::new(),
            activation_reductions: Vec::new(),
            spell_reductions: Vec::new(),
            scheduled_copies: Vec::new(),
            cast_permissions: Vec::new(),
            extra_turns: Vec::new(),
            payment_or_lose: Vec::new(),
            game_results: Vec::new(),
            skipped_steps: Vec::new(),
            looked_at: BTreeMap::new(),
            loyalty_activations_this_turn: BTreeSet::new(),
            mutation_log: Vec::new(),
            next_object_id: 1,
            next_order: 1,
        }
    }
}

impl InMemoryOracleState {
    pub fn insert_player(&mut self, player: PlayerState) {
        self.players.insert(player.id, player);
    }

    pub fn insert_object(&mut self, object: PhysicalObject) -> Result<(), String> {
        if self.objects.contains_key(&object.id) {
            return Err(format!("object {} already exists", object.id));
        }
        self.next_object_id = self.next_object_id.max(object.id.saturating_add(1));
        if object.zone == Zone::Library {
            let player = self
                .players
                .get_mut(&object.owner)
                .ok_or_else(|| format!("missing owner {}", object.owner))?;
            player.library.push(object.id);
        }
        self.objects.insert(object.id, object);
        Ok(())
    }

    pub fn zone_ids(&self, zone: Zone) -> Vec<ObjectId> {
        self.objects
            .values()
            .filter(|object| object.zone == zone)
            .map(|object| object.id)
            .collect()
    }

    pub fn begin_turn(&mut self) {
        self.loyalty_activations_this_turn.clear();
    }
}

impl OracleStateAdapter for InMemoryOracleState {
    type Checkpoint = Self;

    fn checkpoint(&self) -> Self::Checkpoint {
        self.clone()
    }

    fn restore(&mut self, checkpoint: Self::Checkpoint) {
        *self = checkpoint;
    }

    fn player_ids(&self) -> Vec<PlayerId> {
        self.players.keys().copied().collect()
    }

    fn player(&self, id: PlayerId) -> Option<PlayerState> {
        self.players.get(&id).cloned()
    }

    fn put_player(&mut self, player: PlayerState) -> Result<(), String> {
        if !self.players.contains_key(&player.id) {
            return Err(format!("missing player {}", player.id));
        }
        self.players.insert(player.id, player);
        Ok(())
    }

    fn object_ids(&self) -> Vec<ObjectId> {
        self.objects.keys().copied().collect()
    }

    fn object(&self, id: ObjectId) -> Option<PhysicalObject> {
        self.objects.get(&id).cloned()
    }

    fn put_object(&mut self, object: PhysicalObject) -> Result<(), String> {
        if !self.objects.contains_key(&object.id) {
            return Err(format!("missing object {}", object.id));
        }
        self.objects.insert(object.id, object);
        Ok(())
    }

    fn insert_physical_object(&mut self, object: PhysicalObject) -> Result<(), String> {
        self.insert_object(object)
    }

    fn move_object(&mut self, id: ObjectId, zone: Zone) -> Result<(), String> {
        let mut object = self
            .objects
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("missing object {id}"))?;
        if object.zone == Zone::Library {
            let owner = self
                .players
                .get_mut(&object.owner)
                .ok_or_else(|| format!("missing owner {}", object.owner))?;
            owner.library.retain(|candidate| *candidate != id);
        }
        object.zone = zone;
        if zone != Zone::Battlefield {
            object.attacking = false;
        }
        if zone == Zone::Library {
            let owner = self
                .players
                .get_mut(&object.owner)
                .ok_or_else(|| format!("missing owner {}", object.owner))?;
            owner.library.insert(0, id);
        }
        if zone != Zone::Battlefield {
            let detached_sources = self
                .attachments
                .iter()
                .filter_map(|(source, attachment)| {
                    (*source == id || attachment.target == id).then_some(*source)
                })
                .collect::<Vec<_>>();
            for source in detached_sources {
                self.attachments.remove(&source);
                self.mutation_log.push(format!("detach:{source}"));
            }
        }
        self.objects.insert(id, object);
        self.mutation_log.push(format!("move:{id}:{zone:?}"));
        Ok(())
    }

    fn allocate_object_id(&mut self) -> ObjectId {
        let id = self.next_object_id;
        self.next_object_id = self.next_object_id.saturating_add(1);
        id
    }

    fn attachment(&self, source: ObjectId) -> Option<AttachmentRecord> {
        self.attachments.get(&source).copied()
    }

    fn set_attachment(&mut self, attachment: AttachmentRecord) -> Result<(), String> {
        if attachment.source == attachment.target {
            return Err("an attachment source cannot attach to itself".into());
        }
        let source = self
            .objects
            .get(&attachment.source)
            .ok_or_else(|| format!("missing attachment source {}", attachment.source))?;
        let target = self
            .objects
            .get(&attachment.target)
            .ok_or_else(|| format!("missing attachment target {}", attachment.target))?;
        if source.zone != Zone::Battlefield || target.zone != Zone::Battlefield {
            return Err("attachment source and target must be on the battlefield".into());
        }
        if !target
            .characteristics()
            .card_types
            .contains(&CardType::Creature)
        {
            return Err("attachment target is not a creature".into());
        }
        let source_characteristics = source.characteristics();
        let source_is_legal = match attachment.kind {
            AttachmentKind::Aura => {
                source_characteristics
                    .card_types
                    .contains(&CardType::Enchantment)
                    && source_characteristics
                        .subtypes
                        .iter()
                        .any(|subtype| subtype.eq_ignore_ascii_case("Aura"))
            }
            AttachmentKind::Equipment => {
                source_characteristics
                    .card_types
                    .contains(&CardType::Artifact)
                    && source_characteristics
                        .subtypes
                        .iter()
                        .any(|subtype| subtype.eq_ignore_ascii_case("Equipment"))
            }
        };
        if !source_is_legal {
            return Err(format!(
                "attachment source {} does not match {:?}",
                attachment.source, attachment.kind
            ));
        }
        self.attachments.insert(attachment.source, attachment);
        self.mutation_log.push(format!(
            "attach:{}:{}:{:?}",
            attachment.source, attachment.target, attachment.kind
        ));
        Ok(())
    }

    fn clear_attachment(&mut self, source: ObjectId) -> Result<(), String> {
        if self.attachments.remove(&source).is_none() {
            return Err(format!("attachment source {source} is not attached"));
        }
        self.mutation_log.push(format!("detach:{source}"));
        Ok(())
    }

    fn pay_mana(&mut self, player: PlayerId, cost: &ManaCost, x_value: u32) -> Result<(), String> {
        let mut player_state = self
            .players
            .get(&player)
            .cloned()
            .ok_or_else(|| format!("missing player {player}"))?;
        pay_mana_from_player(&mut player_state, cost, x_value)?;
        self.players.insert(player, player_state);
        self.mutation_log
            .push(format!("pay_mana:{player}:{}", cost.0));
        Ok(())
    }

    fn can_pay_mana(&self, player: PlayerId, cost: &ManaCost, x_value: u32) -> bool {
        let Some(mut player) = self.players.get(&player).cloned() else {
            return false;
        };
        pay_mana_from_player(&mut player, cost, x_value).is_ok()
    }

    fn add_mana(&mut self, player: PlayerId, colors: &[Color], amount: u32) -> Result<(), String> {
        let player_state = self
            .players
            .get_mut(&player)
            .ok_or_else(|| format!("missing player {player}"))?;
        if amount == 0 {
            return Ok(());
        }
        let selected = if colors.len()
            == usize::try_from(amount).map_err(|_| "mana amount overflow".to_owned())?
        {
            colors.to_vec()
        } else if let [color] = colors {
            vec![*color; usize::try_from(amount).map_err(|_| "mana amount overflow".to_owned())?]
        } else {
            return Err("mana production composition does not match its quantity".to_owned());
        };
        let mut next = player_state.mana.colored;
        for color in &selected {
            let index = color_index(*color);
            next[index] = next[index]
                .checked_add(1)
                .ok_or_else(|| "mana pool overflow".to_owned())?;
        }
        player_state.mana.colored = next;
        self.mutation_log
            .push(format!("add_mana:{player}:{selected:?}"));
        Ok(())
    }

    fn next_order(&mut self) -> u64 {
        let order = self.next_order;
        self.next_order = self.next_order.saturating_add(1);
        order
    }

    fn continuous_effects(&self) -> Vec<ContinuousEffectRecord> {
        self.continuous_effects.clone()
    }

    fn replacements(&self) -> Vec<ReplacementRecord> {
        self.replacement_effects.clone()
    }

    fn restrictions(&self) -> Vec<RestrictionRecord> {
        self.restriction_effects.clone()
    }

    fn register_continuous(&mut self, record: ContinuousEffectRecord) {
        self.continuous_effects.push(record);
        self.continuous_effects
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn register_delayed_trigger(&mut self, record: DelayedTriggerRecord) {
        self.delayed_triggers.push(record);
        self.delayed_triggers
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn register_replacement(&mut self, record: ReplacementRecord) {
        self.replacement_effects.push(record);
        self.replacement_effects
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn register_restriction(&mut self, record: RestrictionRecord) {
        self.restriction_effects.push(record);
        self.restriction_effects
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn register_activation_reduction(&mut self, record: ActivationReductionRecord) {
        self.activation_reductions.push(record);
        self.activation_reductions
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn spell_reductions(&self) -> Vec<SpellReductionRecord> {
        self.spell_reductions.clone()
    }

    fn register_spell_reduction(&mut self, record: SpellReductionRecord) {
        self.spell_reductions.push(record);
        self.spell_reductions
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn register_scheduled_copy(&mut self, record: ScheduledCopyRecord) {
        self.scheduled_copies.push(record);
        self.scheduled_copies
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn register_cast_permission(&mut self, record: CastPermissionRecord) {
        self.cast_permissions.push(record);
        self.cast_permissions
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn register_extra_turn(&mut self, record: ExtraTurnRecord) {
        self.extra_turns.push(record);
        self.extra_turns
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn register_payment_or_lose(&mut self, record: PaymentOrLoseRecord) {
        self.payment_or_lose.push(record);
        self.payment_or_lose
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn register_game_result(&mut self, record: GameResultRecord) {
        self.game_results.push(record);
        self.game_results
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn game_results(&self) -> Vec<GameResultRecord> {
        self.game_results.clone()
    }

    fn register_skipped_step(&mut self, record: SkippedStepRecord) {
        self.skipped_steps.push(record);
        self.skipped_steps
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn looked_at(&self, player: PlayerId) -> Vec<ObjectId> {
        self.looked_at.get(&player).cloned().unwrap_or_default()
    }

    fn put_looked_at(&mut self, player: PlayerId, objects: Vec<ObjectId>) {
        self.looked_at.insert(player, objects);
    }

    fn loyalty_ability_activated_this_turn(&self, source: ObjectId) -> bool {
        self.loyalty_activations_this_turn.contains(&source)
    }

    fn record_loyalty_ability_activation(&mut self, source: ObjectId) -> Result<(), String> {
        if !self.loyalty_activations_this_turn.insert(source) {
            return Err(format!(
                "object {source} already activated a loyalty ability this turn"
            ));
        }
        Ok(())
    }

    fn record_mutation(&mut self, description: String) {
        self.mutation_log.push(description);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    Committed,
    Countered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReceipt {
    pub status: ExecutionStatus,
    pub costs_paid: usize,
    pub effects_applied: usize,
    pub selected_targets: BTreeMap<u8, Vec<SelectedTarget>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    RuntimeVersionMismatch {
        expected: &'static str,
        actual: &'static str,
    },
    TimingMismatch,
    ActivationRestrictionFailed,
    StackRestriction,
    ConditionFailed {
        index: usize,
    },
    MissingTarget {
        id: u8,
    },
    IllegalTarget {
        id: u8,
    },
    CostFailed {
        index: usize,
        reason: String,
    },
    EffectFailed {
        index: usize,
        reason: String,
    },
    MissingPlayer(PlayerId),
    MissingObject(ObjectId),
    MissingAttachment {
        source: ObjectId,
    },
    IllegalAttachment {
        source: ObjectId,
        target: ObjectId,
        expected: AttachmentKind,
    },
    MissingGrantedAbility {
        source: ObjectId,
        index: usize,
    },
    InvalidAmount(&'static str),
    ArithmeticOverflow,
    Adapter(String),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeVersionMismatch { expected, actual } => {
                write!(
                    formatter,
                    "runtime version {actual} does not match {expected}"
                )
            }
            Self::TimingMismatch => write!(formatter, "the action window does not match"),
            Self::ActivationRestrictionFailed => write!(formatter, "activation restriction failed"),
            Self::StackRestriction => write!(formatter, "a stack restriction forbids the action"),
            Self::ConditionFailed { index } => write!(formatter, "condition {index} failed"),
            Self::MissingTarget { id } => write!(formatter, "target set {id} is missing"),
            Self::IllegalTarget { id } => write!(formatter, "target set {id} is illegal"),
            Self::CostFailed { index, reason } => {
                write!(formatter, "cost {index} failed: {reason}")
            }
            Self::EffectFailed { index, reason } => {
                write!(formatter, "effect {index} failed: {reason}")
            }
            Self::MissingPlayer(id) => write!(formatter, "player {id} is missing"),
            Self::MissingObject(id) => write!(formatter, "object {id} is missing"),
            Self::MissingAttachment { source } => {
                write!(formatter, "object {source} has no live attachment")
            }
            Self::IllegalAttachment {
                source,
                target,
                expected,
            } => write!(
                formatter,
                "object {source} has an illegal attachment to {target} for {expected:?}"
            ),
            Self::MissingGrantedAbility { source, index } => {
                write!(formatter, "object {source} has no granted ability {index}")
            }
            Self::InvalidAmount(reason) => write!(formatter, "invalid amount: {reason}"),
            Self::ArithmeticOverflow => write!(formatter, "arithmetic overflow"),
            Self::Adapter(reason) => write!(formatter, "state adapter failed: {reason}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

pub struct ActionDefinition<'a> {
    pub timing: &'a Timing,
    pub conditions: &'a [Condition],
    pub costs: &'a [Cost],
    pub targets: &'a [Target],
    pub effects: &'a [Effect],
    pub activation_restriction: Option<&'a ActivationRestriction>,
}

thread_local! {
    static CONTRACT_DECLARED_TARGETS: RefCell<Option<BTreeSet<u8>>> = const { RefCell::new(None) };
}

struct ContractTargetScope {
    previous: Option<BTreeSet<u8>>,
}

impl Drop for ContractTargetScope {
    fn drop(&mut self) {
        CONTRACT_DECLARED_TARGETS.with(|declared| {
            declared.replace(self.previous.take());
        });
    }
}

fn with_contract_declared_targets(targets: &[Target], check: impl FnOnce() -> bool) -> bool {
    let declared_ids = targets.iter().map(|target| target.id).collect();
    let previous = CONTRACT_DECLARED_TARGETS.with(|declared| declared.replace(Some(declared_ids)));
    let _scope = ContractTargetScope { previous };
    check()
}

fn target_id_has_contract(id: u8) -> bool {
    CONTRACT_DECLARED_TARGETS.with(|declared| {
        declared
            .borrow()
            .as_ref()
            .is_none_or(|declared| declared.contains(&id))
    })
}

pub fn clause_has_executable_contract(clause: &BoundedOracleClause) -> bool {
    with_contract_declared_targets(clause.targets(), || {
        !clause.requires_saga_lore_consumer()
            && clause.runtime_version() == BOUNDED_ORACLE_RUNTIME_VERSION
            && timing_has_contract(clause.timing())
            && clause.conditions().iter().all(condition_has_contract)
            && clause.costs().iter().all(cost_has_contract)
            && targets_have_contract(clause.targets())
            && clause.effects().iter().all(effect_has_contract)
            && clause
                .activation_restriction()
                .is_none_or(activation_restriction_has_contract)
            && clause.reminder().is_none_or(reminder_has_contract)
    })
}

fn unbridged_damage_contract_enabled() -> bool {
    #[cfg(not(test))]
    {
        false
    }
}

fn timing_has_contract(timing: &Timing) -> bool {
    match timing {
        Timing::CastingAdditionalCost
        | Timing::SpellResolution
        | Timing::Activated
        | Timing::Static
        | Timing::Replacement
        | Timing::SpecialAction(
            SpecialActionTiming::Pregame
            | SpecialActionTiming::EntersPrepared
            | SpecialActionTiming::TransformBackFaceAnnotation,
        ) => true,
        Timing::Triggered(trigger) => trigger_has_contract(trigger),
        Timing::TriggeredModalHeader { trigger, choices } => {
            trigger_has_contract(trigger) && choice_count_has_contract(choices)
        }
        Timing::ModalHeader { choices } => choice_count_has_contract(choices),
        Timing::ModalBranch { .. } => true,
        Timing::TypedStandaloneProgram => false,
    }
}

fn trigger_has_contract(trigger: &Trigger) -> bool {
    match trigger {
        Trigger::AnyOf(triggers) => {
            !triggers.is_empty() && triggers.iter().all(trigger_has_contract)
        }
        Trigger::OncePerTurn(trigger) => trigger_has_contract(trigger),
        Trigger::SourceEnters
        | Trigger::SourceCast
        | Trigger::SourceAttacks
        | Trigger::SourceCombatDamageToPlayer
        | Trigger::BeginningOfNextEndStep => true,
        Trigger::SagaChapterReached { .. } => false,
        Trigger::ObjectEnters(filter)
        | Trigger::ObjectAttacks(filter)
        | Trigger::CombatDamageToPlayer { source: filter } => filter_has_contract(filter),
        Trigger::Cast { player, spell } => {
            player_ref_has_contract(player) && filter_has_contract(spell)
        }
        Trigger::NthSpellCast {
            player,
            occurrence_this_turn,
        } => player_ref_has_contract(player) && *occurrence_this_turn > 0,
        Trigger::CardDrawn {
            player,
            occurrence_this_turn,
        } => {
            player_ref_has_contract(player)
                && occurrence_this_turn.is_none_or(|occurrence| occurrence > 0)
        }
        Trigger::ObjectEvent { subject, .. } => match subject {
            TriggerSubject::Source => true,
            TriggerSubject::Matching(filter) => filter_has_contract(filter),
        },
        Trigger::LifeGained { player } | Trigger::TokenCreated { player } => {
            player_ref_has_contract(player)
        }
        Trigger::PlayerAction {
            player, subject, ..
        } => {
            player_ref_has_contract(player)
                && subject.as_ref().is_none_or(|subject| match subject {
                    TriggerSubject::Source => true,
                    TriggerSubject::Matching(filter) => filter_has_contract(filter),
                })
        }
        Trigger::BecomesTarget {
            object,
            controller,
            source_kinds: _,
        } => object_ref_has_contract(object) && player_ref_has_contract(controller),
        Trigger::BeginningOf {
            step:
                Step::Upkeep
                | Step::DrawStep
                | Step::FirstMainPhase
                | Step::PostcombatMainPhase
                | Step::EndStep
                | Step::Combat
                | Step::UntapStep,
            player: TurnPlayer::You | TurnPlayer::EachPlayer | TurnPlayer::NextTurn,
        } => true,
    }
}

fn activation_restriction_has_contract(restriction: &ActivationRestriction) -> bool {
    match restriction {
        ActivationRestriction::SorceryTiming
        | ActivationRestriction::InstantTiming
        | ActivationRestriction::YourTurn
        | ActivationRestriction::SourceZone(
            Zone::Library
            | Zone::Hand
            | Zone::Battlefield
            | Zone::Graveyard
            | Zone::Exile
            | Zone::Stack
            | Zone::Command,
        ) => true,
    }
}

fn choice_count_has_contract(count: &ChoiceCount) -> bool {
    match count {
        ChoiceCount::Exactly(amount) => *amount > 0,
        ChoiceCount::UpTo(amount) => *amount > 0,
        ChoiceCount::Between { minimum, maximum } => *minimum > 0 && minimum <= maximum,
    }
}

fn targets_have_contract(targets: &[Target]) -> bool {
    let mut ids = BTreeSet::new();
    targets.iter().all(|target| {
        ids.insert(target.id)
            && player_ref_has_contract(&target.chooser)
            && target_filter_has_contract(&target.filter)
            && match target.amount {
                TargetAmount::Exactly(amount) => amount > 0,
                TargetAmount::UpTo(_) | TargetAmount::All => true,
            }
            && match &target.relationship {
                TargetRelationship::Independent | TargetRelationship::DifferentControllers => true,
                TargetRelationship::OtherThan(object) => object_ref_has_contract(object),
            }
    })
}

fn target_filter_has_contract(filter: &TargetFilter) -> bool {
    match filter {
        TargetFilter::Player => true,
        TargetFilter::Object(filter) | TargetFilter::Spell(filter) => filter_has_contract(filter),
        TargetFilter::Any(filters) => {
            filters.len() >= 2 && filters.iter().all(target_filter_has_contract)
        }
        TargetFilter::Conditional {
            condition,
            if_true,
            if_false,
        } => {
            condition_has_contract(condition)
                && target_filter_has_contract(if_true)
                && target_filter_has_contract(if_false)
        }
    }
}

fn condition_has_contract(condition: &Condition) -> bool {
    match condition {
        Condition::ControlCount {
            player,
            filter,
            amount,
            ..
        } => {
            player_ref_has_contract(player)
                && filter_has_contract(filter)
                && amount_has_contract(amount)
        }
        Condition::ControlAny { player, filters } => {
            player_ref_has_contract(player)
                && !filters.is_empty()
                && filters.iter().all(filter_has_contract)
        }
        Condition::SourceState(
            ObjectState::Tapped
            | ObjectState::Untapped
            | ObjectState::Attacking
            | ObjectState::Prepared
            | ObjectState::FaceDown,
        ) => true,
        Condition::TargetState { target, .. } => object_ref_has_contract(target),
        Condition::PowerComparison { object, amount, .. } => {
            filter_has_contract(object) && amount_has_contract(amount)
        }
        Condition::EventWouldOccur(event) => replacement_event_has_contract(event),
        Condition::PaymentDeclined(cost) | Condition::PaymentAccepted(cost) => {
            cost_has_contract(cost)
        }
        Condition::CardWasCastWithAlternativeCost
        | Condition::NotYourTurn
        | Condition::NotThatPlayersTurn
        | Condition::GiftPromised
        | Condition::SourceInOpeningHand
        | Condition::NotPlayingFirst
        | Condition::SourceWasCounteredByThisEffect
        | Condition::FirstResolutionOfNamedSpell => true,
        Condition::GraveyardCardCount { player, amount, .. }
        | Condition::CardTypesInGraveyard { player, amount, .. } => {
            player_ref_has_contract(player) && amount_has_contract(amount)
        }
        Condition::SourceHasCounter { .. } => true,
        Condition::CommanderControlled { player } => player_ref_has_contract(player),
        Condition::ObjectIsCardType { object, .. } => object_ref_has_contract(object),
        Condition::UnlessPaid { player, cost } => {
            player_ref_has_contract(player) && cost_has_contract(cost)
        }
    }
}

fn cost_has_contract(cost: &Cost) -> bool {
    match cost {
        Cost::Mana(cost) => mana_cost_has_contract(cost),
        Cost::AtomicResource(_) => false,
        Cost::Loyalty(LoyaltyCost::Add(_)) | Cost::Loyalty(LoyaltyCost::Zero) => true,
        Cost::Loyalty(LoyaltyCost::Remove(amount)) => amount_has_contract(amount),
        Cost::Tap(object)
        | Cost::Untap(object)
        | Cost::SacrificeObject(object)
        | Cost::Discard(object)
        | Cost::ExileObject(object)
        | Cost::Unprepare(object) => object_ref_has_contract(object),
        Cost::ExileSourceFromOwnGraveyard => true,
        Cost::DiscardSelection(selection) | Cost::ExileSelection(selection) => {
            selection_has_contract(selection)
        }
        Cost::DiscardHand { player } => player_ref_has_contract(player),
        Cost::RemoveCounter { object, amount, .. } => {
            object_ref_has_contract(object) && amount_has_contract(amount)
        }
        Cost::TapCreaturesWithTotalPower { player, minimum } => {
            player_ref_has_contract(player) && amount_has_contract(minimum)
        }
        Cost::PayLife(amount) => amount_has_contract(amount),
        Cost::Sacrifice { amount, filter } => {
            amount_has_contract(amount) && filter_has_contract(filter)
        }
    }
}

fn amount_has_contract(amount: &Amount) -> bool {
    match amount {
        Amount::Constant(_) | Amount::X | Amount::OneOrMore | Amount::Any => true,
        Amount::Twice(inner) | Amount::UpTo(inner) => amount_has_contract(inner),
        Amount::Product { factor, value } => *factor > 0 && amount_has_contract(value),
        Amount::Count(expression) => count_has_contract(expression),
    }
}

fn count_has_contract(count: &CountExpression) -> bool {
    match count {
        CountExpression::MatchingObjects { player, filter }
        | CountExpression::GreatestPower { player, filter } => {
            player_ref_has_contract(player) && filter_has_contract(filter)
        }
        CountExpression::CountersOn { object, .. } => object_ref_has_contract(object),
        CountExpression::CardsInZone { player, filter, .. } => {
            player_ref_has_contract(player) && filter_has_contract(filter)
        }
        CountExpression::OpponentsDealtCombatDamage { player }
        | CountExpression::Devotion { player, .. } => player_ref_has_contract(player),
        CountExpression::ManaValueOf { object } => object_ref_has_contract(object),
        CountExpression::TriggerEventAmount => true,
        CountExpression::ReplacementEventAmount => true,
    }
}

fn selection_has_contract(selection: &ObjectSelection) -> bool {
    player_ref_has_contract(&selection.chooser)
        && filter_has_contract(&selection.filter)
        && match selection.amount {
            TargetAmount::Exactly(amount) | TargetAmount::UpTo(amount) => amount > 0,
            TargetAmount::All => true,
        }
}

fn replacement_event_has_contract(event: &ReplacementEvent) -> bool {
    match event {
        ReplacementEvent::CreateTokens { player } => player_ref_has_contract(player),
        ReplacementEvent::PutCounters { object, .. } => filter_has_contract(object),
        ReplacementEvent::SourceWouldEnter => true,
    }
}

fn player_ref_has_contract(player: &PlayerRef) -> bool {
    match player {
        PlayerRef::You
        | PlayerRef::PlayerIdentity(_)
        | PlayerRef::Opponent
        | PlayerRef::Any
        | PlayerRef::ThatPlayer => true,
        PlayerRef::TargetPlayer(id) => target_id_has_contract(*id),
        PlayerRef::ControllerOf(object) | PlayerRef::OwnerOf(object) => {
            object_ref_has_contract(object)
        }
    }
}

fn object_ref_has_contract(object: &ObjectRef) -> bool {
    match object {
        ObjectRef::Source
        | ObjectRef::ObjectIdentity(_)
        | ObjectRef::TriggeringObject
        | ObjectRef::ThatObject(_)
        | ObjectRef::SearchedCard(_) => true,
        ObjectRef::AttachmentTarget { .. } => false,
        ObjectRef::Target(id) => target_id_has_contract(*id),
        ObjectRef::TargetSet(targets) => {
            !targets.is_empty() && targets.iter().all(|id| target_id_has_contract(*id))
        }
        ObjectRef::TopCard { player } => player_ref_has_contract(player),
        ObjectRef::EachMatching(filter) => filter_has_contract(filter),
    }
}

fn filter_has_contract(filter: &ObjectFilter) -> bool {
    filter
        .controller
        .as_ref()
        .is_none_or(player_ref_has_contract)
        && filter.owner.as_ref().is_none_or(player_ref_has_contract)
        && filter.names.iter().all(|name| !name.trim().is_empty())
        && filter
            .excluded_colors
            .iter()
            .all(|color| !filter.colors.contains(color))
        && filter
            .power
            .as_ref()
            .is_none_or(|(_, amount)| amount_has_contract(amount))
}

fn mana_cost_has_contract(cost: &ManaCost) -> bool {
    let Ok(expression) = parse_resource_cost_expression(&cost.0) else {
        return false;
    };
    let [TypedResourceCostComponent::Mana(mana)] = expression.components.as_slice() else {
        return false;
    };
    !mana.symbols.is_empty()
        && mana
            .symbols
            .iter()
            .all(|symbol| !matches!(symbol, TypedManaSymbol::Snow))
}

fn typed_mana_production_has_contract(production: &TypedManaProductionExpression) -> bool {
    production.version == BOUNDED_ORACLE_MANA_EXPRESSION_VERSION
        && production.spend_restriction.is_none()
        && production.retention == TypedManaRetention::Normal
        && typed_mana_quantity_has_contract(&production.quantity)
        && typed_mana_composition_has_contract(&production.composition)
}

fn typed_mana_quantity_has_contract(quantity: &TypedManaQuantity) -> bool {
    match quantity {
        TypedManaQuantity::Fixed(_) | TypedManaQuantity::X { defined_as: None } => true,
        TypedManaQuantity::X {
            defined_as: Some(calculation),
        }
        | TypedManaQuantity::Calculated(calculation) => {
            typed_mana_calculation_has_contract(calculation)
        }
    }
}

fn typed_mana_calculation_has_contract(calculation: &TypedQuantityCalculation) -> bool {
    match calculation {
        TypedQuantityCalculation::Value(TypedCalculatedValue::Constant(_))
        | TypedQuantityCalculation::Value(TypedCalculatedValue::SourcePower) => true,
        TypedQuantityCalculation::Value(TypedCalculatedValue::Count(objects)) => {
            typed_mana_count_has_contract(objects)
        }
        TypedQuantityCalculation::Value(
            TypedCalculatedValue::SacrificedCreatureManaValue
            | TypedCalculatedValue::SacrificedPermanentManaValue
            | TypedCalculatedValue::ManaSpentToCastReferencedSpell,
        ) => false,
        TypedQuantityCalculation::Sum(terms) => {
            !terms.is_empty() && terms.iter().all(typed_mana_calculation_has_contract)
        }
    }
}

fn typed_mana_count_has_contract(objects: &TypedCountedObjects) -> bool {
    objects.controller == TypedCountController::You
        && objects.shares_creature_type_with.is_none()
        && matches!(
            (objects.card_type, objects.subtype, objects.keyword),
            (
                Some(TypedCountedCardType::Creature),
                None,
                Some(TypedCountedKeyword::Defender),
            ) | (None, Some(TypedCountedSubtype::Shrine), None)
        )
}

fn typed_mana_composition_has_contract(composition: &TypedManaComposition) -> bool {
    match composition {
        TypedManaComposition::Exact(colors) => !colors.is_empty(),
        TypedManaComposition::OneOf(choices) => {
            !choices.is_empty() && choices.iter().all(|choice| !choice.is_empty())
        }
        TypedManaComposition::AnyOneColor => true,
        TypedManaComposition::AnyCombination(domain)
        | TypedManaComposition::DifferentColors(domain) => {
            !typed_mana_domain_colors(domain).is_empty()
        }
        TypedManaComposition::Derived(TypedDerivedManaTypes::CommanderColorIdentity) => true,
        TypedManaComposition::Derived(
            TypedDerivedManaTypes::ChosenColor
            | TypedDerivedManaTypes::ChosenColors
            | TypedDerivedManaTypes::ExiledCardsColors
            | TypedDerivedManaTypes::SacrificedLandCouldProduce
            | TypedDerivedManaTypes::ControlledLandsCouldProduce,
        ) => false,
    }
}

fn duration_has_contract(duration: &Duration) -> bool {
    match duration {
        Duration::Permanent
        | Duration::ThisTurn
        | Duration::UntilEndOfTurn
        | Duration::WhileSourceOnBattlefield
        | Duration::BeginningOfNextEndStep
        | Duration::BeginningOfNextTurnUpkeep => true,
        Duration::WhileCondition(condition) => condition_has_contract(condition),
    }
}

fn keyword_has_contract(keyword: &Keyword) -> bool {
    if let Keyword::Ward(cost) = keyword {
        return ward_cost_has_contract(cost);
    }
    let official_keyword = match keyword {
        Keyword::Deathtouch => Some(crate::keyword_rules_runtime::OfficialKeyword::Deathtouch),
        Keyword::Defender => Some(crate::keyword_rules_runtime::OfficialKeyword::Defender),
        Keyword::DoubleStrike => Some(crate::keyword_rules_runtime::OfficialKeyword::DoubleStrike),
        Keyword::FirstStrike => Some(crate::keyword_rules_runtime::OfficialKeyword::FirstStrike),
        Keyword::Flying => Some(crate::keyword_rules_runtime::OfficialKeyword::Flying),
        Keyword::Haste => Some(crate::keyword_rules_runtime::OfficialKeyword::Haste),
        Keyword::Hexproof => Some(crate::keyword_rules_runtime::OfficialKeyword::Hexproof),
        Keyword::Indestructible => {
            Some(crate::keyword_rules_runtime::OfficialKeyword::Indestructible)
        }
        Keyword::Lifelink => Some(crate::keyword_rules_runtime::OfficialKeyword::Lifelink),
        Keyword::Menace => Some(crate::keyword_rules_runtime::OfficialKeyword::Menace),
        Keyword::Reach => Some(crate::keyword_rules_runtime::OfficialKeyword::Reach),
        Keyword::Trample => Some(crate::keyword_rules_runtime::OfficialKeyword::Trample),
        Keyword::Vigilance => Some(crate::keyword_rules_runtime::OfficialKeyword::Vigilance),
        Keyword::Ward(_) => None,
    };
    official_keyword.is_some_and(
        crate::keyword_production_bridge::static_keyword_has_complete_production_contract,
    )
}

fn ward_cost_has_contract(cost: &WardCost) -> bool {
    match cost {
        WardCost::Mana(cost) => mana_cost_has_contract(cost),
        WardCost::PayLife(amount) => amount_has_contract(amount),
    }
}

fn token_definition_has_contract(definition: &TokenDefinition) -> bool {
    definition.power.as_ref().is_none_or(amount_has_contract)
        && definition
            .toughness
            .as_ref()
            .is_none_or(amount_has_contract)
        && definition.keywords.iter().all(keyword_has_contract)
        && definition
            .abilities
            .iter()
            .all(granted_ability_has_contract)
}

fn token_specification_has_contract(specification: &TokenSpecification) -> bool {
    match specification {
        TokenSpecification::Defined(definition) => token_definition_has_contract(definition),
        TokenSpecification::CopyOf(object) | TokenSpecification::ManifestedCard(object) => {
            object_ref_has_contract(object)
        }
    }
}

fn token_creation_has_contract(creation: &TokenCreation) -> bool {
    player_ref_has_contract(&creation.player)
        && amount_has_contract(&creation.amount)
        && token_specification_has_contract(&creation.specification)
}

fn granted_ability_has_contract(ability: &GrantedAbility) -> bool {
    ability.costs.iter().all(cost_has_contract) && ability.effects.iter().all(effect_has_contract)
}

fn copy_has_contract(copy: &CopyEffect) -> bool {
    let destination = match &copy.destination {
        CopyDestination::SourceAsItEnters => true,
        CopyDestination::TokenControlledBy(player) => player_ref_has_contract(player),
    };
    destination
        && object_ref_has_contract(&copy.original)
        && filter_has_contract(&copy.filter)
        && copy.exceptions.iter().all(|exception| match exception {
            CopyException::RetainSourceAbilities
            | CopyException::SetName(_)
            | CopyException::AddLegendary
            | CopyException::RemoveLegendary
            | CopyException::AddCardType(_)
            | CopyException::AddSubtype(_)
            | CopyException::AddKeyword(_) => true,
            CopyException::AddCounterIfType { amount, .. } => amount_has_contract(amount),
            CopyException::AddGrantedAbility(ability) => granted_ability_has_contract(ability),
        })
}

fn restriction_has_contract(restriction: &Restriction) -> bool {
    match restriction {
        Restriction::ActivatedAbilitiesCannotBeActivated {
            object: ObjectRef::AttachmentTarget { .. },
            duration: Duration::WhileSourceOnBattlefield,
        }
        | Restriction::MustAttackEachCombatIfAble {
            object: ObjectRef::AttachmentTarget { .. },
            duration: Duration::WhileSourceOnBattlefield,
        }
        | Restriction::CannotAttack {
            object: ObjectRef::AttachmentTarget { .. },
            duration: Duration::WhileSourceOnBattlefield,
        }
        | Restriction::CannotBlock {
            object: ObjectRef::AttachmentTarget { .. },
            duration: Duration::WhileSourceOnBattlefield,
        }
        | Restriction::CannotBeBlocked {
            object: ObjectRef::AttachmentTarget { .. },
            duration: Duration::WhileSourceOnBattlefield,
        } => true,
        Restriction::SpellCannotBeCountered { object }
        | Restriction::ActivatedAbilitiesCannotBeActivated { object, .. }
        | Restriction::DestroyProtection { object } => object_ref_has_contract(object),
        Restriction::MustAttackEachCombatIfAble { object, duration }
        | Restriction::CannotAttack { object, duration } => {
            object_ref_has_contract(object) && duration_has_contract(duration)
        }
        Restriction::DoesNotUntapDuring {
            object: ObjectRef::AttachmentTarget { .. },
            step: Step::UntapStep,
        } => true,
        Restriction::DoesNotUntapDuring { object, .. } => object_ref_has_contract(object),
        Restriction::CannotBlock { object, duration }
        | Restriction::CannotBeBlocked { object, duration } => {
            object_ref_has_contract(object) && duration_has_contract(duration)
        }
        Restriction::MatchingSpellsCannotBeCountered { player, filter } => {
            player_ref_has_contract(player) && filter_has_contract(filter)
        }
        Restriction::CannotCast {
            affected,
            filter,
            duration,
            during_turn_of,
        } => {
            player_ref_has_contract(affected)
                && filter_has_contract(filter)
                && duration_has_contract(duration)
                && during_turn_of.as_ref().is_none_or(player_ref_has_contract)
        }
        Restriction::ManaSpendRestriction { source, filter, .. } => {
            object_ref_has_contract(source) && filter_has_contract(filter)
        }
        Restriction::ManaDoesNotEmpty { player, duration } => {
            player_ref_has_contract(player) && duration_has_contract(duration)
        }
        Restriction::AbilityUseLimit {
            object,
            uses_per_turn,
            ..
        } => object_ref_has_contract(object) && *uses_per_turn > 0,
        Restriction::CannotCastNonManaSpellsWhileOnStack { affected }
        | Restriction::CannotActivateNonManaAbilitiesWhileOnStack { affected } => {
            player_ref_has_contract(affected)
        }
        Restriction::MaximumHandSize { player, .. }
        | Restriction::LegendRuleDoesNotApply { player } => player_ref_has_contract(player),
        Restriction::UntapLimit { player, filter, .. } => {
            player_ref_has_contract(player) && filter_has_contract(filter)
        }
        Restriction::TargetingProtection {
            object,
            forbidden_controller,
        } => object_ref_has_contract(object) && player_ref_has_contract(forbidden_controller),
        Restriction::EnchantRestriction { filter } => filter_has_contract(filter),
        Restriction::AlternativeCastPermission(permission) => {
            object_ref_has_contract(&permission.object)
                && match &permission.cost {
                    AlternativeCost::WithoutPayingManaCost => true,
                    AlternativeCost::Mana(cost) => mana_cost_has_contract(cost),
                    AlternativeCost::PrintedManaCost => true,
                    AlternativeCost::Costs(costs) | AlternativeCost::PrintedManaCostPlus(costs) => {
                        !costs.is_empty() && costs.iter().all(cost_has_contract)
                    }
                }
                && trigger_has_contract(&permission.timing)
                && permission
                    .condition
                    .as_ref()
                    .is_none_or(condition_has_contract)
        }
        Restriction::PartnerCommanderPairing
        | Restriction::PreparedCastPermission
        | Restriction::SpellCommanderEligibility { .. } => true,
    }
}

fn cast_permission_has_contract(permission: &CastPermission) -> bool {
    player_ref_has_contract(&permission.affected)
        && permission
            .objects
            .as_ref()
            .is_none_or(object_ref_has_contract)
        && filter_has_contract(&permission.filter)
        && duration_has_contract(&permission.duration)
        && permission
            .alternative_cost
            .as_ref()
            .is_none_or(|cost| match cost {
                AlternativeCost::WithoutPayingManaCost | AlternativeCost::PrintedManaCost => true,
                AlternativeCost::Mana(cost) => mana_cost_has_contract(cost),
                AlternativeCost::Costs(costs) | AlternativeCost::PrintedManaCostPlus(costs) => {
                    !costs.is_empty() && costs.iter().all(cost_has_contract)
                }
            })
        && permission.additional_costs.iter().all(cost_has_contract)
        && matches!(
            permission.timing,
            CastTiming::Normal | CastTiming::AsThoughFlash
        )
}

fn replacement_has_contract(replacement: &ReplacementEffect) -> bool {
    match replacement {
        ReplacementEffect::MultiplyEvent { event, multiplier } => {
            *multiplier > 0 && replacement_event_has_contract(event)
        }
        ReplacementEffect::EntersTapped(replacement) => {
            object_ref_has_contract(&replacement.object)
                && replacement
                    .unless
                    .as_ref()
                    .is_none_or(condition_has_contract)
                && replacement
                    .optional_cost
                    .as_ref()
                    .is_none_or(cost_has_contract)
                && replacement
                    .optional_reveal
                    .as_ref()
                    .is_none_or(filter_has_contract)
        }
        ReplacementEffect::EnterAsCopy(copy) => copy_has_contract(copy),
        ReplacementEffect::ConditionalTokenSubstitution {
            condition,
            ordinary,
            replacement,
        } => {
            condition_has_contract(condition)
                && token_creation_has_contract(ordinary)
                && token_creation_has_contract(replacement)
        }
    }
}

fn effect_has_contract(effect: &Effect) -> bool {
    match effect {
        Effect::Optional(effects) => !effects.is_empty() && effects.iter().all(effect_has_contract),
        Effect::PayCost(cost) => cost_has_contract(cost),
        Effect::AddMana(production) => {
            player_ref_has_contract(&production.player)
                && production.typed.as_ref().map_or_else(
                    || {
                        !production.choices.is_empty()
                            && production
                                .choices
                                .iter()
                                .all(|choice| !choice.symbols.is_empty())
                            && amount_has_contract(&production.amount)
                            && production
                                .scales_with
                                .as_ref()
                                .is_none_or(count_has_contract)
                    },
                    typed_mana_production_has_contract,
                )
        }
        Effect::Counter { object }
        | Effect::CounterToZone { object, .. }
        | Effect::Destroy { object }
        | Effect::Tap { object }
        | Effect::Untap { object }
        | Effect::Transform { object }
        | Effect::ExileSpellAfterResolution { object }
        | Effect::ChooseNewTargets { object } => object_ref_has_contract(object),
        Effect::CopyStackObject { object, .. } => object_ref_has_contract(object),
        Effect::ChangeControl { object, controller } => {
            object_ref_has_contract(object) && player_ref_has_contract(controller)
        }
        Effect::SkipStep { player, .. }
        | Effect::WinGame { player }
        | Effect::LoseGame { player } => player_ref_has_contract(player),
        Effect::TakeExtraTurn(effect) => player_ref_has_contract(&effect.player),
        Effect::SchedulePaymentOrLose(effect) => {
            player_ref_has_contract(&effect.player)
                && cost_has_contract(&effect.cost)
                && trigger_has_contract(&effect.trigger)
        }
        Effect::MoveZone(move_zone) => {
            object_ref_has_contract(&move_zone.object)
                && move_zone
                    .delayed_until
                    .as_ref()
                    .is_none_or(trigger_has_contract)
        }
        Effect::MoveSelected(move_zone) => selection_has_contract(&move_zone.selection),
        Effect::SetSelectedTapped { selection, .. } => selection_has_contract(selection),
        Effect::SearchLibrary(search) => {
            player_ref_has_contract(&search.player)
                && player_ref_has_contract(&search.chooser)
                && amount_has_contract(&search.amount)
                && filter_has_contract(&search.predicate)
                && !search.destinations.is_empty()
        }
        Effect::ExileTop(exile) => {
            player_ref_has_contract(&exile.player)
                && amount_has_contract(&exile.amount)
                && exile
                    .cast_permission
                    .as_ref()
                    .is_none_or(cast_permission_has_contract)
                && exile
                    .delayed_destination
                    .as_ref()
                    .is_none_or(|(_, trigger)| trigger_has_contract(trigger))
        }
        Effect::ExileCollection(exile) => {
            object_ref_has_contract(&exile.objects)
                && exile
                    .cast_permission
                    .as_ref()
                    .is_none_or(cast_permission_has_contract)
                && exile
                    .delayed_destination
                    .as_ref()
                    .is_none_or(|(_, trigger)| trigger_has_contract(trigger))
        }
        Effect::BounceWithControllerCopy(effect) => {
            object_ref_has_contract(&effect.object)
                && selection_has_contract(&effect.sacrifice)
                && object_ref_has_contract(&effect.copy_source)
        }
        Effect::GrantCastPermission(permission) => cast_permission_has_contract(permission),
        Effect::LibraryProcedure(procedure) => match procedure {
            LibraryProcedure::DiscardHandsAndDraw { player, amount } => {
                player_ref_has_contract(player) && amount_has_contract(amount)
            }
            LibraryProcedure::RevealTopToHandLoseManaValue { player, repeat } => {
                player_ref_has_contract(player) && amount_has_contract(repeat)
            }
            LibraryProcedure::ExileUntilNamedCard { player, .. }
            | LibraryProcedure::ExileUntilAcceptedOrDuplicate { player }
            | LibraryProcedure::DevotionLookAndWin { player, .. } => {
                player_ref_has_contract(player)
            }
        },
        Effect::ShuffleLibrary { player } | Effect::ChooseCreatureType { player } => {
            player_ref_has_contract(player)
        }
        Effect::PutRestOnLibraryBottom { player, order } => {
            player_ref_has_contract(player) && matches!(order, BottomOrder::AnyOrder)
        }
        Effect::CreateToken(creation) => token_creation_has_contract(creation),
        Effect::CreateTokenWithDelayedMove {
            creation, trigger, ..
        } => token_creation_has_contract(creation) && trigger_has_contract(trigger),
        Effect::Draw {
            player,
            amount,
            delayed_until,
            ..
        } => {
            player_ref_has_contract(player)
                && amount_has_contract(amount)
                && delayed_until.as_ref().is_none_or(trigger_has_contract)
        }
        Effect::Discard(selection) => selection_has_contract(selection),
        Effect::GainLife { player, amount }
        | Effect::LoseLife { player, amount }
        | Effect::PayLife { player, amount }
        | Effect::Scry { player, amount }
        | Effect::Surveil { player, amount }
        | Effect::Mill { player, amount }
        | Effect::LookAtTop { player, amount } => {
            player_ref_has_contract(player) && amount_has_contract(amount)
        }
        Effect::Damage {
            source,
            recipient,
            amount,
        } => {
            unbridged_damage_contract_enabled()
                && object_ref_has_contract(source)
                && player_ref_has_contract(recipient)
                && amount_has_contract(amount)
        }
        Effect::PreventDamage {
            amount, duration, ..
        } => {
            unbridged_damage_contract_enabled()
                && amount_has_contract(amount)
                && duration_has_contract(duration)
        }
        Effect::Manifest { player, card } => {
            player_ref_has_contract(player) && object_ref_has_contract(card)
        }
        Effect::PutCounter { object, amount, .. } => {
            object_ref_has_contract(object) && amount_has_contract(amount)
        }
        Effect::ModifyPowerToughness(PowerToughnessChange {
            objects: ObjectRef::AttachmentTarget { .. },
            operation:
                PowerToughnessOperation::Add
                | PowerToughnessOperation::Subtract
                | PowerToughnessOperation::AddPowerSubtractToughness
                | PowerToughnessOperation::SubtractPowerAddToughness,
            power: Amount::Constant(_),
            toughness: Amount::Constant(_),
            duration: Duration::WhileSourceOnBattlefield,
        }) => true,
        Effect::ModifyPowerToughness(change) => {
            object_ref_has_contract(&change.objects)
                && amount_has_contract(&change.power)
                && amount_has_contract(&change.toughness)
                && duration_has_contract(&change.duration)
        }
        Effect::GrantKeyword {
            objects,
            keywords,
            duration,
        } => {
            object_ref_has_contract(objects)
                && !keywords.is_empty()
                && keywords.iter().all(keyword_has_contract)
                && duration_has_contract(duration)
        }
        Effect::GrantAbility {
            objects,
            ability,
            duration,
        } => {
            object_ref_has_contract(objects)
                && granted_ability_has_contract(ability)
                && duration_has_contract(duration)
        }
        Effect::LoseAllAbilities { object, duration } => {
            object_ref_has_contract(object) && duration_has_contract(duration)
        }
        Effect::SetCharacteristics(change) => {
            object_ref_has_contract(&change.object)
                && change.base_power.as_ref().is_none_or(amount_has_contract)
                && change
                    .base_toughness
                    .as_ref()
                    .is_none_or(amount_has_contract)
                && duration_has_contract(&change.duration)
        }
        Effect::Restriction(restriction) => restriction_has_contract(restriction),
        Effect::Replacement(replacement) => replacement_has_contract(replacement),
        Effect::Copy(copy) => copy_has_contract(copy),
        Effect::ResolveWard {
            payer,
            source,
            cost,
        } => {
            player_ref_has_contract(payer)
                && object_ref_has_contract(source)
                && ward_cost_has_contract(cost)
        }
        Effect::Animate(animation) => {
            object_ref_has_contract(&animation.object)
                && amount_has_contract(&animation.power)
                && amount_has_contract(&animation.toughness)
                && animation.keywords.iter().all(keyword_has_contract)
                && duration_has_contract(&animation.duration)
        }
        Effect::SelectFromLookedAt {
            player,
            amount,
            predicate,
            ..
        } => {
            player_ref_has_contract(player)
                && amount_has_contract(amount)
                && filter_has_contract(predicate)
        }
        Effect::CastCopy(copy) => {
            object_ref_has_contract(&copy.source)
                && trigger_has_contract(&copy.timing)
                && matches!(
                    copy.repeat,
                    RepeatSchedule::Once | RepeatSchedule::EachFirstMainPhase
                )
        }
        Effect::ReduceActivationCost {
            mana,
            per,
            minimum_total,
        } => {
            mana_cost_has_contract(mana)
                && count_has_contract(per)
                && minimum_total.as_ref().is_none_or(mana_cost_has_contract)
        }
        Effect::ReduceSpellCost {
            object,
            mana,
            per,
            maximum_reduction,
        } => {
            object_ref_has_contract(object)
                && mana_cost_has_contract(mana)
                && count_has_contract(per)
                && maximum_reduction
                    .as_ref()
                    .is_none_or(mana_cost_has_contract)
        }
        Effect::ChooseMode { count } => choice_count_has_contract(count),
        Effect::StandaloneRuleProgram(_) => false,
        Effect::Conditional {
            condition,
            if_true,
            if_false,
        } => {
            condition_has_contract(condition)
                && if_true.iter().all(effect_has_contract)
                && if_false.iter().all(effect_has_contract)
        }
    }
}

fn reminder_has_contract(reminder: &ReminderSemantics) -> bool {
    match reminder {
        ReminderSemantics::Composite(reminders) => {
            !reminders.is_empty() && reminders.iter().all(reminder_has_contract)
        }
        ReminderSemantics::SpecialResourceExplanation(_)
        | ReminderSemantics::ManaNotationExplanation(_)
        | ReminderSemantics::StandaloneAnnotation(_) => false,
        ReminderSemantics::KeywordExplanation(keyword) => keyword_has_contract(keyword),
        ReminderSemantics::KeywordExplanations(keywords) => {
            !keywords.is_empty() && keywords.iter().all(keyword_has_contract)
        }
        ReminderSemantics::TreasureDefinition(definition)
        | ReminderSemantics::FoodDefinition(definition)
        | ReminderSemantics::ClueDefinition(definition)
        | ReminderSemantics::BloodDefinition(definition)
        | ReminderSemantics::GoldDefinition(definition) => {
            token_definition_has_contract(definition)
        }
        ReminderSemantics::TrampleExplanation
        | ReminderSemantics::HexproofExplanation
        | ReminderSemantics::IndestructibleExplanation
        | ReminderSemantics::ProwessProcedure
        | ReminderSemantics::ManifestProcedure
        | ReminderSemantics::SplitSecondProcedure
        | ReminderSemantics::PartnerProcedure
        | ReminderSemantics::PreparedProcedure
        | ReminderSemantics::SpellCommanderProcedure
        | ReminderSemantics::ParadigmProcedure
        | ReminderSemantics::FlashProcedure
        | ReminderSemantics::UntapSymbolProcedure
        | ReminderSemantics::TransformOrigin { .. }
        | ReminderSemantics::CharacteristicLossExplanation => true,
        ReminderSemantics::SurveilProcedure { amount }
        | ReminderSemantics::ScryProcedure { amount } => amount_has_contract(amount),
        ReminderSemantics::MillProcedure { player, amount, .. } => {
            player_ref_has_contract(player) && amount_has_contract(amount)
        }
        ReminderSemantics::CrewProcedure { required_power } => amount_has_contract(required_power),
        ReminderSemantics::CyclingProcedure { cost } => mana_cost_has_contract(cost),
        ReminderSemantics::TypecyclingProcedure { cost, filter, .. } => {
            mana_cost_has_contract(cost) && filter_has_contract(filter)
        }
        ReminderSemantics::EvokeProcedure { cost } => mana_cost_has_contract(cost),
        ReminderSemantics::DevotionProcedure { .. } => true,
        ReminderSemantics::FlashbackProcedure | ReminderSemantics::EscapeProcedure => true,
        ReminderSemantics::DashProcedure { cost } => mana_cost_has_contract(cost),
        ReminderSemantics::GiftProcedure { token, .. } => token_definition_has_contract(token),
        ReminderSemantics::MobilizeProcedure { amount, token } => {
            amount_has_contract(amount) && token_definition_has_contract(token)
        }
    }
}

pub fn execute_clause<S: OracleStateAdapter>(
    state: &mut S,
    clause: &BoundedOracleClause,
    context: &ExecutionContext,
) -> Result<ExecutionReceipt, ExecutionError> {
    if clause.runtime_version() != BOUNDED_ORACLE_RUNTIME_VERSION {
        return Err(ExecutionError::RuntimeVersionMismatch {
            expected: BOUNDED_ORACLE_RUNTIME_VERSION,
            actual: clause.runtime_version(),
        });
    }
    if !clause_has_executable_contract(clause) {
        return Err(ExecutionError::Adapter(
            "compiled clause has no complete bounded execution contract".to_owned(),
        ));
    }
    execute_action(
        state,
        ActionDefinition {
            timing: clause.timing(),
            conditions: clause.conditions(),
            costs: clause.costs(),
            targets: clause.targets(),
            effects: clause.effects(),
            activation_restriction: clause.activation_restriction(),
        },
        context,
    )
}

pub fn execute_granted_ability<S: OracleStateAdapter>(
    state: &mut S,
    source: ObjectId,
    ability_index: usize,
    context: &ExecutionContext,
) -> Result<ExecutionReceipt, ExecutionError> {
    let object = effective_object(state, source, context)?;
    if object.zone != Zone::Battlefield || object.controller != context.actor {
        return Err(ExecutionError::ActivationRestrictionFailed);
    }
    let ability = object
        .characteristics()
        .abilities
        .get(ability_index)
        .cloned()
        .ok_or(ExecutionError::MissingGrantedAbility {
            source,
            index: ability_index,
        })?;
    if !granted_ability_has_contract(&ability) {
        return Err(ExecutionError::Adapter(
            "granted ability has no complete bounded contract".to_owned(),
        ));
    }
    let timing = Timing::Activated;
    let mut activation_context = context.clone();
    activation_context.source = source;
    activation_context.window = ActionWindow::Activated;
    execute_action(
        state,
        ActionDefinition {
            timing: &timing,
            conditions: &[],
            costs: &ability.costs,
            targets: &[],
            effects: &ability.effects,
            activation_restriction: None,
        },
        &activation_context,
    )
}

pub fn execute_action<S: OracleStateAdapter>(
    state: &mut S,
    action: ActionDefinition<'_>,
    context: &ExecutionContext,
) -> Result<ExecutionReceipt, ExecutionError> {
    if !timing_matches(state, action.timing, context)? {
        return Err(ExecutionError::TimingMismatch);
    }
    if let Some(restriction) = action.activation_restriction
        && !activation_restriction_holds(state, restriction, context)?
    {
        return Err(ExecutionError::ActivationRestrictionFailed);
    }
    if stack_restriction_blocks(state, context)? {
        return Err(ExecutionError::StackRestriction);
    }
    validate_targets(state, action.targets, context)?;
    for (index, condition) in action.conditions.iter().enumerate() {
        if !condition_holds(state, condition, context)? {
            return Err(ExecutionError::ConditionFailed { index });
        }
    }

    let checkpoint = state.checkpoint();
    for (index, cost) in action.costs.iter().enumerate() {
        if let Err(error) = pay_cost(state, cost, context) {
            state.restore(checkpoint);
            return Err(ExecutionError::CostFailed {
                index,
                reason: error.to_string(),
            });
        }
    }

    if context.countered {
        state.record_mutation(format!("countered:{}", context.source));
        return Ok(ExecutionReceipt {
            status: ExecutionStatus::Countered,
            costs_paid: action.costs.len(),
            effects_applied: 0,
            selected_targets: context.targets.clone(),
        });
    }

    for (index, effect) in action.effects.iter().enumerate() {
        if let Err(error) = apply_effect(state, effect, context) {
            state.restore(checkpoint);
            return Err(ExecutionError::EffectFailed {
                index,
                reason: error.to_string(),
            });
        }
    }

    Ok(ExecutionReceipt {
        status: ExecutionStatus::Committed,
        costs_paid: action.costs.len(),
        effects_applied: action.effects.len(),
        selected_targets: context.targets.clone(),
    })
}

fn timing_matches<S: OracleStateAdapter>(
    state: &S,
    timing: &Timing,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    Ok(match (timing, &context.window) {
        (Timing::CastingAdditionalCost, ActionWindow::CastingAdditionalCost)
        | (Timing::SpellResolution, ActionWindow::SpellResolution)
        | (Timing::Activated, ActionWindow::Activated)
        | (Timing::Static, ActionWindow::Static)
        | (Timing::Replacement, ActionWindow::Replacement) => true,
        (Timing::Triggered(trigger), ActionWindow::Triggered(event)) => {
            trigger_matches(state, trigger, event, context)?
        }
        (Timing::TriggeredModalHeader { trigger, choices }, ActionWindow::Triggered(event)) => {
            trigger_matches(state, trigger, event, context)?
                && choice_count_matches(choices, &context.selected_modes)
        }
        (Timing::ModalHeader { choices }, ActionWindow::ModalHeader) => {
            choice_count_matches(choices, &context.selected_modes)
        }
        (
            Timing::ModalBranch {
                header_clause_index,
                branch_index,
            },
            ActionWindow::ModalBranch {
                header_clause_index: actual_header,
                branch_index: actual_branch,
            },
        ) => header_clause_index == actual_header && branch_index == actual_branch,
        (Timing::SpecialAction(expected), ActionWindow::SpecialAction(actual)) => {
            expected == actual
        }
        _ => false,
    })
}

fn choice_count_matches(count: &ChoiceCount, selected: &[u16]) -> bool {
    let unique = selected.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != selected.len() {
        return false;
    }
    match count {
        ChoiceCount::Exactly(amount) => selected.len() == usize::from(*amount),
        ChoiceCount::UpTo(amount) => selected.len() <= usize::from(*amount),
        ChoiceCount::Between { minimum, maximum } => {
            selected.len() >= usize::from(*minimum) && selected.len() <= usize::from(*maximum)
        }
    }
}

pub(crate) fn trigger_matches<S: OracleStateAdapter>(
    state: &S,
    trigger: &Trigger,
    event: &TriggerEvent,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    Ok(match trigger {
        Trigger::AnyOf(triggers) => {
            let mut matched = false;
            for candidate in triggers {
                if trigger_matches(state, candidate, event, context)? {
                    matched = true;
                    break;
                }
            }
            matched
        }
        Trigger::OncePerTurn(trigger) => {
            context.ability_occurrence_this_turn == 1
                && trigger_matches(state, trigger, event, context)?
        }
        Trigger::SourceEnters => {
            matches!(event, TriggerEvent::ObjectEntered { object } if *object == context.source)
        }
        Trigger::SourceCast => matches!(
            event,
            TriggerEvent::SpellCast { spell, .. } if *spell == context.source
        ),
        Trigger::ObjectEnters(filter) => match event {
            TriggerEvent::ObjectEntered { object } => {
                object_matches_filter(state, *object, filter, context)?
            }
            _ => false,
        },
        Trigger::ObjectAttacks(filter) => match event {
            TriggerEvent::ObjectAttacked { object } => {
                object_matches_filter(state, *object, filter, context)?
            }
            _ => false,
        },
        Trigger::SourceAttacks => {
            matches!(event, TriggerEvent::ObjectAttacked { object } if *object == context.source)
        }
        Trigger::Cast { player, spell } => match event {
            TriggerEvent::SpellCast {
                player: actual_player,
                spell: actual_spell,
                ..
            } => {
                resolve_players(state, player, context)?.contains(actual_player)
                    && object_matches_filter(state, *actual_spell, spell, context)?
            }
            _ => false,
        },
        Trigger::NthSpellCast {
            player,
            occurrence_this_turn,
        } => match event {
            TriggerEvent::SpellCast {
                player: actual_player,
                occurrence_this_turn: actual_occurrence,
                ..
            } => {
                actual_occurrence == occurrence_this_turn
                    && resolve_players(state, player, context)?.contains(actual_player)
            }
            _ => false,
        },
        Trigger::CardDrawn {
            player,
            occurrence_this_turn,
        } => match event {
            TriggerEvent::CardDrawn {
                player: actual_player,
                occurrence_this_turn: actual_occurrence,
                ..
            } => {
                occurrence_this_turn.is_none_or(|expected| expected == *actual_occurrence)
                    && resolve_players(state, player, context)?.contains(actual_player)
            }
            _ => false,
        },
        Trigger::ObjectEvent {
            subject,
            event: expected,
        } => match event {
            TriggerEvent::ObjectEvent {
                object,
                event: actual,
            } if expected == actual => match subject {
                TriggerSubject::Source => *object == context.source,
                TriggerSubject::Matching(filter) => {
                    object_matches_filter(state, *object, filter, context)?
                }
            },
            _ => false,
        },
        Trigger::LifeGained { player } => match event {
            TriggerEvent::LifeGained { player: actual, .. } => {
                resolve_players(state, player, context)?.contains(actual)
            }
            _ => false,
        },
        Trigger::TokenCreated { player } => match event {
            TriggerEvent::TokenCreated { player: actual, .. } => {
                resolve_players(state, player, context)?.contains(actual)
            }
            _ => false,
        },
        Trigger::PlayerAction {
            player,
            action,
            subject,
        } => match event {
            TriggerEvent::PlayerAction {
                player: actual_player,
                action: actual_action,
                object,
            } if action == actual_action
                && resolve_players(state, player, context)?.contains(actual_player) =>
            {
                match subject {
                    None => true,
                    Some(TriggerSubject::Source) => object.is_some_and(|id| id == context.source),
                    Some(TriggerSubject::Matching(filter)) => match object {
                        Some(id) => object_matches_filter(state, *id, filter, context)?,
                        None => false,
                    },
                }
            }
            _ => false,
        },
        Trigger::CombatDamageToPlayer { source } => match event {
            TriggerEvent::CombatDamageToPlayer {
                source: actual_source,
                ..
            } => object_matches_filter(state, *actual_source, source, context)?,
            _ => false,
        },
        Trigger::SourceCombatDamageToPlayer => matches!(
            event,
            TriggerEvent::CombatDamageToPlayer { source, .. } if *source == context.source
        ),
        Trigger::BecomesTarget {
            object,
            controller,
            source_kinds,
        } => match event {
            TriggerEvent::BecameTarget {
                object: actual_object,
                controller: actual_controller,
                source,
            } => {
                let objects = resolve_objects(state, object, context)?;
                let controllers = resolve_players(state, controller, context)?;
                let source_object = state
                    .object(*source)
                    .ok_or(ExecutionError::MissingObject(*source))?;
                objects.contains(actual_object)
                    && controllers.contains(actual_controller)
                    && (source_kinds.is_empty()
                        || source_kinds
                            .iter()
                            .any(|kind| object_has_type(&source_object, *kind)))
            }
            _ => false,
        },
        Trigger::BeginningOf { step, player } => match event {
            TriggerEvent::BeginningOf {
                step: actual_step,
                active_player,
                is_next_turn,
            } => {
                step == actual_step
                    && match player {
                        TurnPlayer::You => source_controller(state, context)? == *active_player,
                        TurnPlayer::EachPlayer => true,
                        TurnPlayer::NextTurn => *is_next_turn,
                    }
            }
            _ => false,
        },
        Trigger::BeginningOfNextEndStep => matches!(
            event,
            TriggerEvent::BeginningOf {
                step: Step::EndStep,
                is_next_turn: true,
                ..
            }
        ),
        Trigger::SagaChapterReached { .. } => false,
    })
}

fn activation_restriction_holds<S: OracleStateAdapter>(
    state: &S,
    restriction: &ActivationRestriction,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    Ok(match restriction {
        ActivationRestriction::SorceryTiming => context.sorcery_timing,
        ActivationRestriction::InstantTiming => context.instant_timing,
        ActivationRestriction::YourTurn => {
            source_controller(state, context)? == context.active_player
        }
        ActivationRestriction::SourceZone(zone) => {
            state
                .object(context.source)
                .ok_or(ExecutionError::MissingObject(context.source))?
                .zone
                == *zone
        }
    })
}

fn stack_restriction_blocks<S: OracleStateAdapter>(
    state: &S,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    let restrictions = sorted_restrictions(state);
    for record in restrictions {
        let mut local = context.clone();
        local.source = record.source_identity;
        match &record.restriction {
            Restriction::CannotCastNonManaSpellsWhileOnStack { affected }
                if matches!(context.window, ActionWindow::SpellResolution)
                    && !context.is_mana_ability
                    && resolve_players(state, affected, &local)?.contains(&context.actor) =>
            {
                return Ok(true);
            }
            Restriction::CannotActivateNonManaAbilitiesWhileOnStack { affected }
                if matches!(context.window, ActionWindow::Activated)
                    && !context.is_mana_ability
                    && resolve_players(state, affected, &local)?.contains(&context.actor) =>
            {
                return Ok(true);
            }
            Restriction::ActivatedAbilitiesCannotBeActivated { object, duration }
                if matches!(context.window, ActionWindow::Activated)
                    && restriction_duration_is_active(
                        state,
                        record.source_identity,
                        duration,
                        &local,
                    )?
                    && resolve_objects(state, object, &local)?.contains(&context.source) =>
            {
                return Ok(true);
            }
            Restriction::CannotCast {
                affected,
                filter,
                during_turn_of,
                ..
            } if matches!(
                context.window,
                ActionWindow::CastingAdditionalCost | ActionWindow::SpellResolution
            ) && resolve_players(state, affected, &local)?.contains(&context.actor)
                && during_turn_of.as_ref().is_none_or(|turn_player| {
                    resolve_players(state, turn_player, &local)
                        .unwrap_or_default()
                        .contains(&context.active_player)
                })
                && object_matches_filter(state, context.source, filter, &local)? =>
            {
                return Ok(true);
            }
            _ => {}
        }
    }
    Ok(false)
}

fn sorted_restrictions<S: OracleStateAdapter>(state: &S) -> Vec<RestrictionRecord> {
    let mut restrictions = state.restrictions();
    restrictions.sort_by_key(|item| (item.order, item.source_identity));
    restrictions
}

fn restriction_duration_is_active<S: OracleStateAdapter>(
    state: &S,
    source_identity: ObjectId,
    duration: &Duration,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    Ok(match duration {
        Duration::Permanent | Duration::ThisTurn | Duration::UntilEndOfTurn => true,
        Duration::WhileSourceOnBattlefield => state
            .object(source_identity)
            .is_some_and(|source| source.zone == Zone::Battlefield),
        Duration::WhileCondition(condition) => condition_holds(state, condition, context)?,
        Duration::BeginningOfNextEndStep | Duration::BeginningOfNextTurnUpkeep => true,
    })
}

pub fn object_can_attack<S: OracleStateAdapter>(
    state: &S,
    object: ObjectId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    let candidate = state
        .object(object)
        .ok_or(ExecutionError::MissingObject(object))?;
    if candidate.zone != Zone::Battlefield {
        return Ok(false);
    }
    for record in sorted_restrictions(state) {
        let Restriction::CannotAttack {
            object: restricted,
            duration,
        } = &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        let active =
            restriction_duration_is_active(state, record.source_identity, duration, &local)?;
        if active && resolve_objects(state, restricted, &local)?.contains(&object) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn object_must_attack_each_combat<S: OracleStateAdapter>(
    state: &S,
    object: ObjectId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    let candidate = state
        .object(object)
        .ok_or(ExecutionError::MissingObject(object))?;
    if candidate.zone != Zone::Battlefield {
        return Ok(false);
    }
    for record in sorted_restrictions(state) {
        let Restriction::MustAttackEachCombatIfAble {
            object: required,
            duration,
        } = &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        let active =
            restriction_duration_is_active(state, record.source_identity, duration, &local)?;
        if active && resolve_objects(state, required, &local)?.contains(&object) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn object_can_block<S: OracleStateAdapter>(
    state: &S,
    object: ObjectId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    let candidate = state
        .object(object)
        .ok_or(ExecutionError::MissingObject(object))?;
    if candidate.zone != Zone::Battlefield {
        return Ok(false);
    }
    for record in sorted_restrictions(state) {
        let Restriction::CannotBlock {
            object: restricted,
            duration,
        } = &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        let active =
            restriction_duration_is_active(state, record.source_identity, duration, &local)?;
        if active && resolve_objects(state, restricted, &local)?.contains(&object) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn object_can_be_blocked<S: OracleStateAdapter>(
    state: &S,
    object: ObjectId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    let candidate = state
        .object(object)
        .ok_or(ExecutionError::MissingObject(object))?;
    if candidate.zone != Zone::Battlefield {
        return Ok(false);
    }
    for record in sorted_restrictions(state) {
        let Restriction::CannotBeBlocked {
            object: restricted,
            duration,
        } = &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        let active =
            restriction_duration_is_active(state, record.source_identity, duration, &local)?;
        if active && resolve_objects(state, restricted, &local)?.contains(&object) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn object_can_untap_during<S: OracleStateAdapter>(
    state: &S,
    object: ObjectId,
    step: Step,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    let candidate = state
        .object(object)
        .ok_or(ExecutionError::MissingObject(object))?;
    if candidate.zone != Zone::Battlefield {
        return Ok(false);
    }
    for record in sorted_restrictions(state) {
        let Restriction::DoesNotUntapDuring {
            object: restricted,
            step: restricted_step,
        } = &record.restriction
        else {
            continue;
        };
        if *restricted_step != step {
            continue;
        }
        let Some(source) = state.object(record.source_identity) else {
            continue;
        };
        if source.zone != Zone::Battlefield {
            continue;
        }
        let mut local = context.clone();
        local.source = record.source_identity;
        if resolve_objects(state, restricted, &local)?.contains(&object) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn effective_object<S: OracleStateAdapter>(
    state: &S,
    object: ObjectId,
    context: &ExecutionContext,
) -> Result<PhysicalObject, ExecutionError> {
    let mut effective = state
        .object(object)
        .ok_or(ExecutionError::MissingObject(object))?;
    let mut continuous = state.continuous_effects();
    continuous.sort_by_key(|record| (record.order, record.source_identity));
    effective.controller = effective_attachment_controllers(state, &continuous, context)?
        .get(&object)
        .copied()
        .ok_or(ExecutionError::MissingObject(object))?;

    for record in &continuous {
        let mut local = context.clone();
        local.source = record.source_identity;
        if !restriction_duration_is_active(state, record.source_identity, &record.duration, &local)?
        {
            continue;
        }
        match &record.effect {
            Effect::SetCharacteristics(change)
                if matches!(change.object, ObjectRef::AttachmentTarget { .. })
                    && resolve_objects(state, &change.object, &local)?.contains(&object) =>
            {
                apply_attachment_non_power_characteristics(effective.characteristics_mut(), change);
            }
            _ => {}
        }
    }

    for record in &continuous {
        let mut local = context.clone();
        local.source = record.source_identity;
        if !restriction_duration_is_active(state, record.source_identity, &record.duration, &local)?
        {
            continue;
        }
        match &record.effect {
            Effect::LoseAllAbilities {
                object: attached, ..
            } if matches!(attached, ObjectRef::AttachmentTarget { .. })
                && resolve_objects(state, attached, &local)?.contains(&object) =>
            {
                effective.characteristics_mut().abilities.clear();
                effective.characteristics_mut().keywords.clear();
            }
            Effect::GrantKeyword {
                objects, keywords, ..
            } if matches!(objects, ObjectRef::AttachmentTarget { .. })
                && resolve_objects(state, objects, &local)?.contains(&object) =>
            {
                for keyword in keywords {
                    if !effective.characteristics().keywords.contains(keyword) {
                        effective
                            .characteristics_mut()
                            .keywords
                            .push(keyword.clone());
                    }
                }
            }
            Effect::GrantAbility {
                objects, ability, ..
            } if matches!(objects, ObjectRef::AttachmentTarget { .. })
                && resolve_objects(state, objects, &local)?.contains(&object) =>
            {
                effective
                    .characteristics_mut()
                    .abilities
                    .push(ability.clone());
            }
            _ => {}
        }
    }

    for record in &continuous {
        let mut local = context.clone();
        local.source = record.source_identity;
        if !restriction_duration_is_active(state, record.source_identity, &record.duration, &local)?
        {
            continue;
        }
        match &record.effect {
            Effect::SetCharacteristics(change)
                if matches!(change.object, ObjectRef::AttachmentTarget { .. })
                    && resolve_objects(state, &change.object, &local)?.contains(&object) =>
            {
                if let Some(power) = &change.base_power {
                    effective.characteristics_mut().power = exact_attachment_amount(power)?;
                }
                if let Some(toughness) = &change.base_toughness {
                    effective.characteristics_mut().toughness = exact_attachment_amount(toughness)?;
                }
            }
            Effect::ModifyPowerToughness(change)
                if matches!(change.objects, ObjectRef::AttachmentTarget { .. })
                    && change.operation == PowerToughnessOperation::SetBase
                    && resolve_objects(state, &change.objects, &local)?.contains(&object) =>
            {
                let power = exact_attachment_amount(&change.power)?;
                let toughness = exact_attachment_amount(&change.toughness)?;
                apply_power_toughness_operation(
                    effective.characteristics_mut(),
                    PowerToughnessOperation::SetBase,
                    power,
                    toughness,
                )?;
            }
            _ => {}
        }
    }

    for record in &continuous {
        let mut local = context.clone();
        local.source = record.source_identity;
        if !restriction_duration_is_active(state, record.source_identity, &record.duration, &local)?
        {
            continue;
        }
        let Effect::ModifyPowerToughness(change) = &record.effect else {
            continue;
        };
        if !matches!(change.objects, ObjectRef::AttachmentTarget { .. })
            || change.operation == PowerToughnessOperation::SetBase
            || !resolve_objects(state, &change.objects, &local)?.contains(&object)
        {
            continue;
        }
        let power = exact_attachment_amount(&change.power)?;
        let toughness = exact_attachment_amount(&change.toughness)?;
        apply_power_toughness_operation(
            effective.characteristics_mut(),
            change.operation.clone(),
            power,
            toughness,
        )?;
    }
    Ok(effective)
}

fn effective_attachment_controllers<S: OracleStateAdapter>(
    state: &S,
    continuous: &[ContinuousEffectRecord],
    context: &ExecutionContext,
) -> Result<BTreeMap<ObjectId, PlayerId>, ExecutionError> {
    let mut controllers = BTreeMap::new();
    for id in state.object_ids() {
        let candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
        controllers.insert(id, candidate.controller);
    }

    let mut controls = Vec::<(ObjectId, ObjectId)>::new();
    for record in continuous {
        let Effect::ChangeControl {
            object: attached,
            controller: PlayerRef::You,
        } = &record.effect
        else {
            continue;
        };
        if !matches!(attached, ObjectRef::AttachmentTarget { .. }) {
            continue;
        }
        let mut local = context.clone();
        local.source = record.source_identity;
        if !restriction_duration_is_active(state, record.source_identity, &record.duration, &local)?
        {
            continue;
        }
        let target = resolve_objects(state, attached, &local)?
            .into_iter()
            .next()
            .ok_or(ExecutionError::InvalidAmount(
                "attachment control effect has no physical object",
            ))?;
        controls.push((record.source_identity, target));
    }

    let mut applied = vec![false; controls.len()];
    for _ in 0..controls.len() {
        let Some(index) = controls
            .iter()
            .enumerate()
            .find_map(|(index, (source, _))| {
                if applied[index] {
                    return None;
                }
                let pending_dependency = controls.iter().enumerate().any(|(other, (_, target))| {
                    !applied[other] && other != index && target == source
                });
                (!pending_dependency).then_some(index)
            })
        else {
            return Err(ExecutionError::Adapter(
                "cyclic attachment control dependency".to_owned(),
            ));
        };
        let (source, target) = controls[index];
        let controller = controllers
            .get(&source)
            .copied()
            .ok_or(ExecutionError::MissingObject(source))?;
        controllers.insert(target, controller);
        applied[index] = true;
    }
    Ok(controllers)
}

fn apply_attachment_non_power_characteristics(
    characteristics: &mut ObjectCharacteristics,
    change: &SetCharacteristics,
) {
    if let Some(colors) = &change.colors {
        merge_or_replace(
            &mut characteristics.colors,
            colors,
            change.retain_other_colors,
        );
    }
    if let Some(card_types) = &change.card_types {
        merge_or_replace(
            &mut characteristics.card_types,
            card_types,
            change.retain_other_card_types,
        );
    }
    if let Some(subtypes) = &change.subtypes {
        merge_or_replace(
            &mut characteristics.subtypes,
            subtypes,
            change.retain_other_subtypes,
        );
    }
    if let Some(name) = &change.name {
        if !change.retain_other_names {
            characteristics.names.clear();
        }
        if !characteristics.names.contains(name) {
            characteristics.names.push(name.clone());
        }
    }
}

fn exact_attachment_amount(amount: &Amount) -> Result<i64, ExecutionError> {
    match amount {
        Amount::Constant(value) => Ok(i64::from(*value)),
        _ => Err(ExecutionError::InvalidAmount(
            "attachment continuous effect requires an exact constant amount",
        )),
    }
}

pub fn reduced_spell_mana_cost<S: OracleStateAdapter>(
    state: &S,
    spell: ObjectId,
    printed_cost: &ManaCost,
    x_value: u32,
    context: &ExecutionContext,
) -> Result<(ManaCost, u32), ExecutionError> {
    let symbols = mana_symbols(&printed_cost.0).map_err(ExecutionError::Adapter)?;
    let mut generic_total = 0u32;
    let mut retained = Vec::new();
    for symbol in symbols {
        if symbol == "X" {
            generic_total = generic_total
                .checked_add(x_value)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        } else if let Ok(amount) = symbol.parse::<u32>() {
            generic_total = generic_total
                .checked_add(amount)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        } else {
            if symbol.contains('/') {
                return Err(ExecutionError::InvalidAmount(
                    "spell cost reduction requires explicit hybrid payment choices",
                ));
            }
            retained.push(symbol);
        }
    }

    let mut records = state.spell_reductions();
    records.sort_by_key(|record| (record.order, record.source_identity));
    let mut requested_reduction = 0u32;
    for record in records {
        let mut local = context.clone();
        local.source = record.source_identity;
        if !resolve_objects(state, &record.object, &local)?.contains(&spell) {
            continue;
        }
        let per_object = generic_only_mana_amount(&record.mana)?;
        let count = evaluate_count(state, &record.per, &local)?;
        let mut reduction = per_object
            .checked_mul(count)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
        if let Some(maximum) = &record.maximum_reduction {
            reduction = reduction.min(generic_only_mana_amount(maximum)?);
        }
        requested_reduction = requested_reduction
            .checked_add(reduction)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
    }
    let applied_reduction = requested_reduction.min(generic_total);
    let remaining_generic = generic_total - applied_reduction;
    let mut reduced = String::new();
    if remaining_generic > 0 || retained.is_empty() {
        reduced.push_str(&format!("{{{remaining_generic}}}"));
    }
    for symbol in retained {
        reduced.push('{');
        reduced.push_str(&symbol);
        reduced.push('}');
    }
    Ok((ManaCost(reduced), applied_reduction))
}

pub fn pay_reduced_spell_mana_cost<S: OracleStateAdapter>(
    state: &mut S,
    spell: ObjectId,
    player: PlayerId,
    printed_cost: &ManaCost,
    x_value: u32,
    context: &ExecutionContext,
) -> Result<(ManaCost, u32), ExecutionError> {
    let checkpoint = state.checkpoint();
    let (reduced_cost, applied_reduction) =
        reduced_spell_mana_cost(state, spell, printed_cost, x_value, context)?;
    if let Err(error) = state.pay_mana(player, &reduced_cost, 0) {
        state.restore(checkpoint);
        return Err(ExecutionError::Adapter(error));
    }
    state.record_mutation(format!(
        "pay_reduced_spell_mana:{spell}:{player}:{}:{applied_reduction}",
        reduced_cost.0
    ));
    Ok((reduced_cost, applied_reduction))
}

fn generic_only_mana_amount(cost: &ManaCost) -> Result<u32, ExecutionError> {
    let mut total = 0u32;
    for symbol in mana_symbols(&cost.0).map_err(ExecutionError::Adapter)? {
        let amount = symbol.parse::<u32>().map_err(|_| {
            ExecutionError::InvalidAmount("spell cost reduction must be generic mana")
        })?;
        total = total
            .checked_add(amount)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
    }
    Ok(total)
}

fn validate_targets<S: OracleStateAdapter>(
    state: &S,
    specifications: &[Target],
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let expected_ids = specifications
        .iter()
        .map(|target| target.id)
        .collect::<BTreeSet<_>>();
    if context
        .targets
        .keys()
        .any(|target_id| !expected_ids.contains(target_id))
    {
        return Err(ExecutionError::IllegalTarget {
            id: context
                .targets
                .keys()
                .find(|target_id| !expected_ids.contains(target_id))
                .copied()
                .unwrap_or_default(),
        });
    }

    for target in specifications {
        let selected = context
            .targets
            .get(&target.id)
            .ok_or(ExecutionError::MissingTarget { id: target.id })?;
        if selected.iter().copied().collect::<BTreeSet<_>>().len() != selected.len() {
            return Err(ExecutionError::IllegalTarget { id: target.id });
        }
        let _choosers = resolve_players(state, &target.chooser, context)?;
        let legal = legal_target_candidates(state, &target.filter, context)?;
        if !selected.iter().all(|candidate| legal.contains(candidate)) {
            return Err(ExecutionError::IllegalTarget { id: target.id });
        }
        match target.amount {
            TargetAmount::Exactly(amount) if selected.len() != usize::from(amount) => {
                return Err(ExecutionError::IllegalTarget { id: target.id });
            }
            TargetAmount::UpTo(amount) if selected.len() > usize::from(amount) => {
                return Err(ExecutionError::IllegalTarget { id: target.id });
            }
            TargetAmount::All => {
                let legal_set = legal.iter().copied().collect::<BTreeSet<_>>();
                let selected_set = selected.iter().copied().collect::<BTreeSet<_>>();
                if legal_set != selected_set {
                    return Err(ExecutionError::IllegalTarget { id: target.id });
                }
            }
            TargetAmount::Exactly(_) | TargetAmount::UpTo(_) => {}
        }
        match &target.relationship {
            TargetRelationship::Independent => {}
            TargetRelationship::DifferentControllers => {
                let mut controllers = BTreeSet::new();
                for value in selected {
                    let SelectedTarget::Object(id) = value else {
                        return Err(ExecutionError::IllegalTarget { id: target.id });
                    };
                    let controller = state
                        .object(*id)
                        .ok_or(ExecutionError::MissingObject(*id))?
                        .controller;
                    if !controllers.insert(controller) {
                        return Err(ExecutionError::IllegalTarget { id: target.id });
                    }
                }
            }
            TargetRelationship::OtherThan(other) => {
                let excluded = resolve_objects(state, other, context)?;
                if selected.iter().any(
                    |value| matches!(value, SelectedTarget::Object(id) if excluded.contains(id)),
                ) {
                    return Err(ExecutionError::IllegalTarget { id: target.id });
                }
            }
        }
        if targeting_protection_blocks(state, selected, context)? {
            return Err(ExecutionError::IllegalTarget { id: target.id });
        }
    }
    Ok(())
}

fn legal_target_candidates<S: OracleStateAdapter>(
    state: &S,
    filter: &TargetFilter,
    context: &ExecutionContext,
) -> Result<Vec<SelectedTarget>, ExecutionError> {
    let mut candidates = match filter {
        TargetFilter::Player => state
            .player_ids()
            .into_iter()
            .map(SelectedTarget::Player)
            .collect::<Vec<_>>(),
        TargetFilter::Object(filter) => {
            let mut object_filter = filter.clone();
            if object_filter.zones.is_empty() {
                object_filter.zones.push(Zone::Battlefield);
            }
            matching_objects(state, &object_filter, context)?
                .into_iter()
                .map(SelectedTarget::Object)
                .collect()
        }
        TargetFilter::Spell(filter) => {
            let mut spell_filter = filter.clone();
            if spell_filter.zones.is_empty() {
                spell_filter.zones.push(Zone::Stack);
            }
            matching_objects(state, &spell_filter, context)?
                .into_iter()
                .map(SelectedTarget::Object)
                .collect()
        }
        TargetFilter::Any(filters) => {
            let mut candidates = Vec::new();
            for filter in filters {
                candidates.extend(legal_target_candidates(state, filter, context)?);
            }
            candidates
        }
        TargetFilter::Conditional {
            condition,
            if_true,
            if_false,
        } => {
            let filter = if condition_holds(state, condition, context)? {
                if_true
            } else {
                if_false
            };
            legal_target_candidates(state, filter, context)?
        }
    };
    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

fn targeting_protection_blocks<S: OracleStateAdapter>(
    state: &S,
    selected: &[SelectedTarget],
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    for record in sorted_restrictions(state) {
        let Restriction::TargetingProtection {
            object,
            forbidden_controller,
        } = &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        let protected = resolve_objects(state, object, &local)?;
        let forbidden = resolve_players(state, forbidden_controller, &local)?;
        if forbidden.contains(&context.actor)
            && selected.iter().any(
                |target| matches!(target, SelectedTarget::Object(id) if protected.contains(id)),
            )
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn condition_holds<S: OracleStateAdapter>(
    state: &S,
    condition: &Condition,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    Ok(match condition {
        Condition::ControlCount {
            player,
            filter,
            comparison,
            amount,
        } => {
            let players = resolve_players(state, player, context)?;
            let count = matching_objects(state, filter, context)?
                .into_iter()
                .filter(|id| {
                    state
                        .object(*id)
                        .is_some_and(|object| players.contains(&object.controller))
                })
                .count() as u32;
            let expected = evaluate_amount(state, amount, context)?;
            match comparison {
                Comparison::AtLeast => count >= expected,
                Comparison::AtMost => count <= expected,
                Comparison::Exactly => count == expected,
                Comparison::Greatest => {
                    let greatest = state
                        .player_ids()
                        .into_iter()
                        .map(|candidate| {
                            matching_objects(state, filter, context)
                                .unwrap_or_default()
                                .into_iter()
                                .filter(|id| {
                                    state
                                        .object(*id)
                                        .is_some_and(|object| object.controller == candidate)
                                })
                                .count() as u32
                        })
                        .max()
                        .unwrap_or(0);
                    count == greatest && count >= expected
                }
            }
        }
        Condition::ControlAny { player, filters } => {
            let players = resolve_players(state, player, context)?;
            let mut found = false;
            for filter in filters {
                if matching_objects(state, filter, context)?
                    .into_iter()
                    .any(|id| {
                        state
                            .object(id)
                            .is_some_and(|object| players.contains(&object.controller))
                    })
                {
                    found = true;
                    break;
                }
            }
            found
        }
        Condition::SourceState(required) => object_has_state(
            &state
                .object(context.source)
                .ok_or(ExecutionError::MissingObject(context.source))?,
            required,
        ),
        Condition::TargetState {
            target,
            state: required,
        } => {
            let objects = resolve_objects(state, target, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state
                        .object(*id)
                        .is_some_and(|object| object_has_state(&object, required))
                })
        }
        Condition::PowerComparison {
            object,
            comparison,
            amount,
        } => {
            let objects = matching_objects(state, object, context)?;
            let expected = i64::from(evaluate_amount(state, amount, context)?);
            let powers = objects
                .iter()
                .filter_map(|id| state.object(*id))
                .map(|object| object.characteristics().power)
                .collect::<Vec<_>>();
            match comparison {
                Comparison::AtLeast => powers.iter().any(|power| *power >= expected),
                Comparison::AtMost => powers.iter().any(|power| *power <= expected),
                Comparison::Exactly => powers.contains(&expected),
                Comparison::Greatest => {
                    let greatest = state
                        .object_ids()
                        .into_iter()
                        .filter_map(|id| state.object(id))
                        .map(|candidate| candidate.characteristics().power)
                        .max()
                        .unwrap_or(i64::MIN);
                    powers
                        .into_iter()
                        .any(|power| power == greatest && power >= expected)
                }
            }
        }
        Condition::EventWouldOccur(event) => context
            .replacement_event
            .as_ref()
            .is_some_and(|actual| replacement_event_matches(state, event, actual, context)),
        Condition::PaymentDeclined(cost) => {
            context.payment_declined || !cost_is_payable(state, cost, context)?
        }
        Condition::PaymentAccepted(cost) => {
            !context.payment_declined && cost_is_payable(state, cost, context)?
        }
        Condition::CardWasCastWithAlternativeCost => context.card_was_cast_with_alternative_cost,
        Condition::NotYourTurn => source_controller(state, context)? != context.active_player,
        Condition::NotThatPlayersTurn => context
            .that_player
            .is_some_and(|player| player != context.active_player),
        Condition::GraveyardCardCount {
            player,
            comparison,
            amount,
        } => {
            let players = resolve_players(state, player, context)?;
            let count = state
                .object_ids()
                .into_iter()
                .filter_map(|id| state.object(id))
                .filter(|object| object.zone == Zone::Graveyard && players.contains(&object.owner))
                .count() as u32;
            compare_u32(count, *comparison, evaluate_amount(state, amount, context)?)
        }
        Condition::CardTypesInGraveyard {
            player,
            comparison,
            amount,
        } => {
            let players = resolve_players(state, player, context)?;
            let mut types = Vec::new();
            for object in state
                .object_ids()
                .into_iter()
                .filter_map(|id| state.object(id))
                .filter(|object| object.zone == Zone::Graveyard && players.contains(&object.owner))
            {
                for card_type in &object.characteristics().card_types {
                    if !types.contains(card_type) {
                        types.push(*card_type);
                    }
                }
            }
            compare_u32(
                types.len() as u32,
                *comparison,
                evaluate_amount(state, amount, context)?,
            )
        }
        Condition::SourceHasCounter { counter } => {
            state
                .object(context.source)
                .ok_or(ExecutionError::MissingObject(context.source))?
                .counters
                .get(&counter_key(counter))
                .copied()
                .unwrap_or_default()
                > 0
        }
        Condition::CommanderControlled { .. } => context.commander_controlled,
        Condition::GiftPromised => context.gift_promised,
        Condition::SourceInOpeningHand => {
            context.source_was_in_opening_hand
                && state
                    .object(context.source)
                    .is_some_and(|object| object.zone == Zone::Hand)
        }
        Condition::NotPlayingFirst => !context.playing_first,
        Condition::SourceWasCounteredByThisEffect => context.countered,
        Condition::ObjectIsCardType { object, card_type } => {
            let objects = resolve_objects(state, object, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state
                        .object(*id)
                        .is_some_and(|candidate| object_has_type(&candidate, *card_type))
                })
        }
        Condition::FirstResolutionOfNamedSpell => context.first_resolution_of_named_spell,
        Condition::UnlessPaid { player, cost } => {
            let players = resolve_players(state, player, context)?;
            context.payment_declined
                || players.iter().all(|player| {
                    !cost_is_payable_by(state, cost, *player, context).unwrap_or(false)
                })
        }
    })
}

fn compare_u32(actual: u32, comparison: Comparison, expected: u32) -> bool {
    match comparison {
        Comparison::AtLeast => actual >= expected,
        Comparison::AtMost => actual <= expected,
        Comparison::Exactly => actual == expected,
        Comparison::Greatest => actual >= expected,
    }
}

fn replacement_event_matches<S: OracleStateAdapter>(
    state: &S,
    expected: &ReplacementEvent,
    actual: &ReplacementOccurrence,
    context: &ExecutionContext,
) -> bool {
    match (expected, &actual.event) {
        (ReplacementEvent::CreateTokens { player }, ReplacementEvent::CreateTokens { .. }) => {
            actual.affected_player.is_some_and(|actual_player| {
                resolve_players(state, player, context)
                    .unwrap_or_default()
                    .contains(&actual_player)
            })
        }
        (
            ReplacementEvent::PutCounters { counter, object },
            ReplacementEvent::PutCounters {
                counter: actual_counter,
                ..
            },
        ) => {
            counter == actual_counter
                && actual.object.is_some_and(|actual_object| {
                    object_matches_filter(state, actual_object, object, context).unwrap_or(false)
                })
        }
        (ReplacementEvent::SourceWouldEnter, ReplacementEvent::SourceWouldEnter) => {
            actual.object == Some(context.source)
        }
        _ => false,
    }
}

fn object_has_state(object: &PhysicalObject, state: &ObjectState) -> bool {
    match state {
        ObjectState::Tapped => object.tapped,
        ObjectState::Untapped => !object.tapped,
        ObjectState::Attacking => object.attacking,
        ObjectState::Prepared => object.prepared,
        ObjectState::FaceDown => object.face_down,
    }
}

fn evaluate_amount<S: OracleStateAdapter>(
    state: &S,
    amount: &Amount,
    context: &ExecutionContext,
) -> Result<u32, ExecutionError> {
    match amount {
        Amount::Constant(value) => Ok(*value),
        Amount::X => Ok(context.x_value),
        Amount::OneOrMore => match context.chosen_amount {
            Some(value) if value >= 1 => Ok(value),
            _ => Err(ExecutionError::InvalidAmount(
                "one or more requires a positive choice",
            )),
        },
        Amount::Any => context.chosen_amount.ok_or(ExecutionError::InvalidAmount(
            "any requires an explicit choice",
        )),
        Amount::Twice(inner) => evaluate_amount(state, inner, context)?
            .checked_mul(2)
            .ok_or(ExecutionError::ArithmeticOverflow),
        Amount::Product { factor, value } => evaluate_amount(state, value, context)?
            .checked_mul(*factor)
            .ok_or(ExecutionError::ArithmeticOverflow),
        Amount::Count(expression) => evaluate_count(state, expression, context),
        Amount::UpTo(inner) => {
            let maximum = evaluate_amount(state, inner, context)?;
            Ok(context.chosen_amount.unwrap_or(maximum).min(maximum))
        }
    }
}

fn evaluate_count<S: OracleStateAdapter>(
    state: &S,
    expression: &CountExpression,
    context: &ExecutionContext,
) -> Result<u32, ExecutionError> {
    match expression {
        CountExpression::MatchingObjects { player, filter } => {
            let players = resolve_players(state, player, context)?;
            Ok(matching_objects(state, filter, context)?
                .into_iter()
                .filter(|id| {
                    state
                        .object(*id)
                        .is_some_and(|object| players.contains(&object.controller))
                })
                .count() as u32)
        }
        CountExpression::GreatestPower { player, filter } => {
            let players = resolve_players(state, player, context)?;
            let greatest = matching_objects(state, filter, context)?
                .into_iter()
                .filter_map(|id| state.object(id))
                .filter(|object| players.contains(&object.controller))
                .map(|object| object.characteristics().power.max(0) as u32)
                .max()
                .unwrap_or(0);
            Ok(greatest)
        }
        CountExpression::CountersOn { object, counter } => {
            let objects = resolve_objects(state, object, context)?;
            if objects.len() != 1 {
                return Err(ExecutionError::InvalidAmount(
                    "counter count requires exactly one object",
                ));
            }
            Ok(state
                .object(objects[0])
                .ok_or(ExecutionError::MissingObject(objects[0]))?
                .counters
                .get(&counter_key(counter))
                .copied()
                .unwrap_or_default())
        }
        CountExpression::CardsInZone {
            player,
            zone,
            filter,
        } => {
            let players = resolve_players(state, player, context)?;
            Ok(matching_objects(state, filter, context)?
                .into_iter()
                .filter(|id| {
                    state.object(*id).is_some_and(|object| {
                        object.zone == *zone && players.contains(&object.owner)
                    })
                })
                .count() as u32)
        }
        CountExpression::OpponentsDealtCombatDamage { .. } => {
            Ok(context.opponents_dealt_combat_damage_this_turn)
        }
        CountExpression::Devotion { color, .. } => {
            Ok(context.devotion_by_color[color_index(*color)])
        }
        CountExpression::ManaValueOf { object } => {
            let objects = resolve_objects(state, object, context)?;
            if objects.len() != 1 {
                return Err(ExecutionError::InvalidAmount(
                    "mana value count requires exactly one object",
                ));
            }
            Ok(state
                .object(objects[0])
                .ok_or(ExecutionError::MissingObject(objects[0]))?
                .characteristics()
                .mana_value)
        }
        CountExpression::TriggerEventAmount => match &context.window {
            ActionWindow::Triggered(TriggerEvent::CombatDamageToPlayer { amount, .. })
            | ActionWindow::Triggered(TriggerEvent::LifeGained { amount, .. }) => Ok(*amount),
            _ => Err(ExecutionError::InvalidAmount(
                "trigger event amount is unavailable",
            )),
        },
        CountExpression::ReplacementEventAmount => context
            .replacement_event
            .as_ref()
            .map(|event| event.amount)
            .ok_or(ExecutionError::InvalidAmount(
                "replacement event amount is unavailable",
            )),
    }
}

fn loyalty_cost_is_payable<S: OracleStateAdapter>(
    state: &S,
    cost: &LoyaltyCost,
    player: PlayerId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    let source = effective_object(state, context.source, context)?;
    if !matches!(context.window, ActionWindow::Activated)
        || !context.sorcery_timing
        || context.active_player != player
        || context.actor != player
        || source.zone != Zone::Battlefield
        || source.controller != player
        || !object_has_type(&source, CardType::Planeswalker)
        || state.loyalty_ability_activated_this_turn(context.source)
    {
        return Ok(false);
    }
    let current = source
        .counters
        .get(&counter_key(&CounterKind::Loyalty))
        .copied()
        .unwrap_or_default();
    match cost {
        LoyaltyCost::Add(amount) => Ok(current.checked_add(*amount).is_some()),
        LoyaltyCost::Remove(amount) => Ok(current >= evaluate_amount(state, amount, context)?),
        LoyaltyCost::Zero => Ok(true),
    }
}

fn pay_loyalty_cost<S: OracleStateAdapter>(
    state: &mut S,
    cost: &LoyaltyCost,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    if !loyalty_cost_is_payable(state, cost, context.actor, context)? {
        return Err(ExecutionError::Adapter(
            "loyalty ability cannot be activated or its cost cannot be paid".to_owned(),
        ));
    }
    let mut source = state
        .object(context.source)
        .ok_or(ExecutionError::MissingObject(context.source))?;
    let key = counter_key(&CounterKind::Loyalty);
    let current = source.counters.get(&key).copied().unwrap_or_default();
    let next = match cost {
        LoyaltyCost::Add(amount) => current
            .checked_add(*amount)
            .ok_or(ExecutionError::ArithmeticOverflow)?,
        LoyaltyCost::Remove(amount) => current
            .checked_sub(evaluate_amount(state, amount, context)?)
            .ok_or_else(|| {
                ExecutionError::Adapter(
                    "the source does not have enough loyalty counters".to_owned(),
                )
            })?,
        LoyaltyCost::Zero => current,
    };
    if !matches!(cost, LoyaltyCost::Zero) {
        if next == 0 {
            source.counters.remove(&key);
        } else {
            source.counters.insert(key, next);
        }
        state.put_object(source).map_err(ExecutionError::Adapter)?;
    }
    state
        .record_loyalty_ability_activation(context.source)
        .map_err(ExecutionError::Adapter)?;
    state.record_mutation(format!("pay_loyalty_cost:{}:{cost:?}", context.source));
    Ok(())
}

fn pay_cost<S: OracleStateAdapter>(
    state: &mut S,
    cost: &Cost,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    match cost {
        Cost::Mana(mana) => state
            .pay_mana(context.actor, mana, context.x_value)
            .map_err(ExecutionError::Adapter),
        Cost::AtomicResource(_) => Err(ExecutionError::InvalidAmount(
            "special resource costs require a production payment adapter",
        )),
        Cost::Loyalty(cost) => pay_loyalty_cost(state, cost, context),
        Cost::Tap(object) => {
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount("tap cost has no object"));
            }
            for id in objects {
                let mut object = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if object.zone != Zone::Battlefield || object.tapped {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot be tapped"
                    )));
                }
                object.tapped = true;
                state.put_object(object).map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("tap_cost:{id}"));
            }
            Ok(())
        }
        Cost::Untap(object) => {
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount("untap cost has no object"));
            }
            for id in objects {
                let mut object = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if object.zone != Zone::Battlefield || !object.tapped {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot be untapped"
                    )));
                }
                object.tapped = false;
                state.put_object(object).map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("untap_cost:{id}"));
            }
            Ok(())
        }
        Cost::TapCreaturesWithTotalPower { player, minimum } => {
            let eligible_players = resolve_players(state, player, context)?;
            let minimum = i64::from(evaluate_amount(state, minimum, context)?);
            let mut candidates = state
                .object_ids()
                .into_iter()
                .filter_map(|id| state.object(id))
                .filter(|object| {
                    object.zone == Zone::Battlefield
                        && eligible_players.contains(&object.controller)
                        && object_has_type(object, CardType::Creature)
                        && !object.tapped
                })
                .collect::<Vec<_>>();
            candidates.sort_by_key(|object| object.id);
            let mut selected = Vec::new();
            let mut total = 0_i64;
            for object in candidates {
                selected.push(object.id);
                total = total
                    .checked_add(object.characteristics().power.max(0))
                    .ok_or(ExecutionError::ArithmeticOverflow)?;
                if total >= minimum {
                    break;
                }
            }
            if total < minimum {
                return Err(ExecutionError::Adapter(
                    "untapped creatures do not have enough total power".to_owned(),
                ));
            }
            for id in selected {
                let mut object = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                object.tapped = true;
                state.put_object(object).map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("tap_power_cost:{id}"));
            }
            Ok(())
        }
        Cost::PayLife(amount) => {
            let amount = i64::from(evaluate_amount(state, amount, context)?);
            let mut player = state
                .player(context.actor)
                .ok_or(ExecutionError::MissingPlayer(context.actor))?;
            if amount < 0 || player.life <= amount {
                return Err(ExecutionError::Adapter(format!(
                    "player {} cannot pay {amount} life",
                    context.actor
                )));
            }
            player.life -= amount;
            state.put_player(player).map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("pay_life_cost:{}:{amount}", context.actor));
            Ok(())
        }
        Cost::Sacrifice { amount, filter } => {
            let amount = evaluate_amount(state, amount, context)? as usize;
            let controller = source_controller(state, context)?;
            let mut objects = matching_objects(state, filter, context)?
                .into_iter()
                .filter(|id| {
                    state.object(*id).is_some_and(|object| {
                        object.zone == Zone::Battlefield && object.controller == controller
                    })
                })
                .collect::<Vec<_>>();
            objects.sort_unstable();
            if objects.len() < amount {
                return Err(ExecutionError::Adapter(
                    "not enough legal permanents to sacrifice".to_owned(),
                ));
            }
            for id in objects.into_iter().take(amount) {
                state
                    .move_object(id, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(())
        }
        Cost::SacrificeObject(object) => {
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "sacrifice cost has no object",
                ));
            }
            for id in objects {
                let candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Battlefield
                    || candidate.controller != source_controller(state, context)?
                {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot be sacrificed"
                    )));
                }
                state
                    .move_object(id, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(())
        }
        Cost::Discard(object) => {
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount("discard cost has no object"));
            }
            for id in objects {
                let candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Hand || candidate.owner != context.actor {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot be discarded"
                    )));
                }
                state
                    .move_object(id, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(())
        }
        Cost::DiscardSelection(selection) => {
            let objects = resolve_object_selection(state, selection, context)?;
            for id in objects {
                let candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Hand || candidate.owner != context.actor {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot be discarded"
                    )));
                }
                state
                    .move_object(id, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(())
        }
        Cost::DiscardHand { player } => {
            let players = resolve_players(state, player, context)?;
            for player in players {
                let objects = state
                    .object_ids()
                    .into_iter()
                    .filter_map(|id| state.object(id))
                    .filter(|object| object.zone == Zone::Hand && object.owner == player)
                    .map(|object| object.id)
                    .collect::<Vec<_>>();
                for id in objects {
                    state
                        .move_object(id, Zone::Graveyard)
                        .map_err(ExecutionError::Adapter)?;
                }
            }
            Ok(())
        }
        Cost::ExileObject(object) => {
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount("exile cost has no object"));
            }
            for id in objects {
                state
                    .move_object(id, Zone::Exile)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(())
        }
        Cost::ExileSourceFromOwnGraveyard => {
            let source = state
                .object(context.source)
                .ok_or(ExecutionError::MissingObject(context.source))?;
            if source.zone != Zone::Graveyard || source.owner != context.actor {
                return Err(ExecutionError::Adapter(format!(
                    "source {} is not in its activating owner's graveyard",
                    context.source
                )));
            }
            state
                .move_object(context.source, Zone::Exile)
                .map_err(ExecutionError::Adapter)
        }
        Cost::ExileSelection(selection) => {
            let objects = resolve_object_selection(state, selection, context)?;
            for id in objects {
                state
                    .move_object(id, Zone::Exile)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(())
        }
        Cost::RemoveCounter {
            object,
            counter,
            amount,
        } => {
            let amount = evaluate_amount(state, amount, context)?;
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "counter removal cost has no object",
                ));
            }
            for id in objects {
                let mut candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if matches!(object, ObjectRef::Source)
                    && matches!(context.window, ActionWindow::Activated)
                    && (candidate.zone != Zone::Battlefield
                        || candidate.controller != context.actor)
                {
                    return Err(ExecutionError::Adapter(format!(
                        "source {id} is not a battlefield permanent controlled by the activating player"
                    )));
                }
                let key = counter_key(counter);
                let current = candidate.counters.get(&key).copied().unwrap_or_default();
                let next = current.checked_sub(amount).ok_or_else(|| {
                    ExecutionError::Adapter(format!("object {id} lacks required counters"))
                })?;
                if next == 0 {
                    candidate.counters.remove(&key);
                } else {
                    candidate.counters.insert(key, next);
                }
                state
                    .put_object(candidate)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(())
        }
        Cost::Unprepare(object) => {
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "unprepare cost has no object",
                ));
            }
            for id in objects {
                let mut candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if !candidate.prepared {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} is not prepared"
                    )));
                }
                candidate.prepared = false;
                state
                    .put_object(candidate)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("unprepare:{id}"));
            }
            Ok(())
        }
    }
}

fn cost_is_payable<S: OracleStateAdapter>(
    state: &S,
    cost: &Cost,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    cost_is_payable_by(state, cost, context.actor, context)
}

fn cost_is_payable_by<S: OracleStateAdapter>(
    state: &S,
    cost: &Cost,
    player: PlayerId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    Ok(match cost {
        Cost::Mana(mana) => state.can_pay_mana(player, mana, context.x_value),
        Cost::AtomicResource(_) => false,
        Cost::Loyalty(cost) => loyalty_cost_is_payable(state, cost, player, context)?,
        Cost::Tap(object) => {
            let objects = resolve_objects(state, object, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|candidate| {
                        candidate.zone == Zone::Battlefield && !candidate.tapped
                    })
                })
        }
        Cost::Untap(object) => {
            let objects = resolve_objects(state, object, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|candidate| {
                        candidate.zone == Zone::Battlefield && candidate.tapped
                    })
                })
        }
        Cost::TapCreaturesWithTotalPower {
            player: affected,
            minimum,
        } => {
            let players = resolve_players(state, affected, context)?;
            let minimum = i64::from(evaluate_amount(state, minimum, context)?);
            state
                .object_ids()
                .into_iter()
                .filter_map(|id| state.object(id))
                .filter(|object| {
                    object.zone == Zone::Battlefield
                        && players.contains(&object.controller)
                        && object_has_type(object, CardType::Creature)
                        && !object.tapped
                })
                .map(|object| object.characteristics().power.max(0))
                .sum::<i64>()
                >= minimum
        }
        Cost::PayLife(amount) => {
            let amount = i64::from(evaluate_amount(state, amount, context)?);
            state
                .player(player)
                .is_some_and(|state| state.life > amount)
        }
        Cost::Sacrifice { amount, filter } => {
            let amount = evaluate_amount(state, amount, context)? as usize;
            matching_objects(state, filter, context)?
                .into_iter()
                .filter(|id| {
                    state.object(*id).is_some_and(|candidate| {
                        candidate.zone == Zone::Battlefield && candidate.controller == player
                    })
                })
                .count()
                >= amount
        }
        Cost::SacrificeObject(object) => {
            let objects = resolve_objects(state, object, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|candidate| {
                        candidate.zone == Zone::Battlefield && candidate.controller == player
                    })
                })
        }
        Cost::Discard(object) => {
            let objects = resolve_objects(state, object, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|candidate| {
                        candidate.zone == Zone::Hand && candidate.owner == player
                    })
                })
        }
        Cost::DiscardSelection(selection) => {
            let objects = resolve_object_selection(state, selection, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|candidate| {
                        candidate.zone == Zone::Hand && candidate.owner == player
                    })
                })
        }
        Cost::DiscardHand { player: affected } => {
            resolve_players(state, affected, context)?.contains(&player)
        }
        Cost::ExileObject(object) => {
            let objects = resolve_objects(state, object, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state
                        .object(*id)
                        .is_some_and(|candidate| candidate.owner == player)
                })
        }
        Cost::ExileSourceFromOwnGraveyard => state
            .object(context.source)
            .is_some_and(|source| source.zone == Zone::Graveyard && source.owner == player),
        Cost::ExileSelection(selection) => {
            let objects = resolve_object_selection(state, selection, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state
                        .object(*id)
                        .is_some_and(|candidate| candidate.owner == player)
                })
        }
        Cost::RemoveCounter {
            object,
            counter,
            amount,
        } => {
            let required = evaluate_amount(state, amount, context)?;
            let objects = resolve_objects(state, object, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state
                        .object(*id)
                        .filter(|candidate| {
                            !matches!(object, ObjectRef::Source)
                                || !matches!(context.window, ActionWindow::Activated)
                                || (candidate.zone == Zone::Battlefield
                                    && candidate.controller == player)
                        })
                        .and_then(|candidate| {
                            candidate.counters.get(&counter_key(counter)).copied()
                        })
                        .unwrap_or_default()
                        >= required
                })
        }
        Cost::Unprepare(object) => {
            let objects = resolve_objects(state, object, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state
                        .object(*id)
                        .is_some_and(|candidate| candidate.prepared)
                })
        }
    })
}

fn source_controller<S: OracleStateAdapter>(
    state: &S,
    context: &ExecutionContext,
) -> Result<PlayerId, ExecutionError> {
    if let Some(source) = context
        .last_known_source
        .as_deref()
        .filter(|source| source.id == context.source)
    {
        return Ok(source.controller);
    }
    state
        .object(context.source)
        .map(|object| object.controller)
        .ok_or(ExecutionError::MissingObject(context.source))
}

fn resolve_players<S: OracleStateAdapter>(
    state: &S,
    reference: &PlayerRef,
    context: &ExecutionContext,
) -> Result<Vec<PlayerId>, ExecutionError> {
    let mut players = match reference {
        PlayerRef::You => vec![source_controller(state, context)?],
        PlayerRef::PlayerIdentity(player) => vec![*player],
        PlayerRef::Opponent => {
            let controller = source_controller(state, context)?;
            state
                .player_ids()
                .into_iter()
                .filter(|player| *player != controller)
                .collect()
        }
        PlayerRef::Any => state.player_ids(),
        PlayerRef::TargetPlayer(target_id) => context
            .targets
            .get(target_id)
            .ok_or(ExecutionError::MissingTarget { id: *target_id })?
            .iter()
            .map(|target| match target {
                SelectedTarget::Player(player) => Ok(*player),
                SelectedTarget::Object(_) => Err(ExecutionError::IllegalTarget { id: *target_id }),
            })
            .collect::<Result<Vec<_>, _>>()?,
        PlayerRef::ControllerOf(object) => resolve_objects(state, object, context)?
            .into_iter()
            .map(|id| {
                state
                    .object(id)
                    .map(|object| object.controller)
                    .ok_or(ExecutionError::MissingObject(id))
            })
            .collect::<Result<Vec<_>, _>>()?,
        PlayerRef::OwnerOf(object) => resolve_objects(state, object, context)?
            .into_iter()
            .map(|id| {
                state
                    .object(id)
                    .map(|object| object.owner)
                    .ok_or(ExecutionError::MissingObject(id))
            })
            .collect::<Result<Vec<_>, _>>()?,
        PlayerRef::ThatPlayer => vec![
            context
                .that_player
                .ok_or(ExecutionError::InvalidAmount("that player is unavailable"))?,
        ],
    };
    players.sort_unstable();
    players.dedup();
    for player in &players {
        if state.player(*player).is_none() {
            return Err(ExecutionError::MissingPlayer(*player));
        }
    }
    Ok(players)
}

fn resolve_objects<S: OracleStateAdapter>(
    state: &S,
    reference: &ObjectRef,
    context: &ExecutionContext,
) -> Result<Vec<ObjectId>, ExecutionError> {
    let mut objects = match reference {
        ObjectRef::Source => vec![context.source],
        ObjectRef::ObjectIdentity(object) => vec![*object],
        ObjectRef::AttachmentTarget { kind } => {
            vec![resolve_attachment_target(state, context.source, *kind)?]
        }
        ObjectRef::Target(target_id) => context
            .targets
            .get(target_id)
            .ok_or(ExecutionError::MissingTarget { id: *target_id })?
            .iter()
            .map(|target| match target {
                SelectedTarget::Object(object) => Ok(*object),
                SelectedTarget::Player(_) => Err(ExecutionError::IllegalTarget { id: *target_id }),
            })
            .collect::<Result<Vec<_>, _>>()?,
        ObjectRef::TargetSet(target_ids) => {
            let mut resolved = Vec::new();
            for target_id in target_ids {
                let values = context
                    .targets
                    .get(target_id)
                    .ok_or(ExecutionError::MissingTarget { id: *target_id })?;
                for target in values {
                    match target {
                        SelectedTarget::Object(object) => resolved.push(*object),
                        SelectedTarget::Player(_) => {
                            return Err(ExecutionError::IllegalTarget { id: *target_id });
                        }
                    }
                }
            }
            resolved
        }
        ObjectRef::TriggeringObject => {
            vec![
                context
                    .triggering_object
                    .ok_or(ExecutionError::InvalidAmount(
                        "triggering object is unavailable",
                    ))?,
            ]
        }
        ObjectRef::ThatObject(index) => vec![
            *context
                .that_objects
                .get(index)
                .ok_or(ExecutionError::InvalidAmount("that object is unavailable"))?,
        ],
        ObjectRef::SearchedCard(index) => {
            vec![
                *context
                    .searched_cards
                    .get(index)
                    .ok_or(ExecutionError::InvalidAmount(
                        "searched card is unavailable",
                    ))?,
            ]
        }
        ObjectRef::TopCard { player } => {
            let mut resolved = Vec::new();
            for player in resolve_players(state, player, context)? {
                if let Some(card) = state
                    .player(player)
                    .and_then(|player_state| player_state.library.first().copied())
                {
                    resolved.push(card);
                }
            }
            resolved
        }
        ObjectRef::EachMatching(filter) => matching_objects(state, filter, context)?,
    };
    objects.sort_unstable();
    objects.dedup();
    for object in &objects {
        let is_last_known_source = context
            .last_known_source
            .as_deref()
            .is_some_and(|source| source.id == *object && *object == context.source);
        if state.object(*object).is_none() && !is_last_known_source {
            return Err(ExecutionError::MissingObject(*object));
        }
    }
    Ok(objects)
}

fn resolve_attachment_target<S: OracleStateAdapter>(
    state: &S,
    source_id: ObjectId,
    expected: AttachmentKind,
) -> Result<ObjectId, ExecutionError> {
    let attachment = state
        .attachment(source_id)
        .ok_or(ExecutionError::MissingAttachment { source: source_id })?;
    let source = state
        .object(source_id)
        .ok_or(ExecutionError::MissingObject(source_id))?;
    let target = state
        .object(attachment.target)
        .ok_or(ExecutionError::MissingObject(attachment.target))?;
    let source_characteristics = source.characteristics();
    let source_kind_is_legal = match expected {
        AttachmentKind::Aura => {
            source_characteristics
                .card_types
                .contains(&CardType::Enchantment)
                && source_characteristics
                    .subtypes
                    .iter()
                    .any(|subtype| subtype.eq_ignore_ascii_case("Aura"))
        }
        AttachmentKind::Equipment => {
            source_characteristics
                .card_types
                .contains(&CardType::Artifact)
                && source_characteristics
                    .subtypes
                    .iter()
                    .any(|subtype| subtype.eq_ignore_ascii_case("Equipment"))
        }
    };
    if attachment.source != source_id
        || attachment.kind != expected
        || source.zone != Zone::Battlefield
        || target.zone != Zone::Battlefield
        || !target
            .characteristics()
            .card_types
            .contains(&CardType::Creature)
        || !source_kind_is_legal
    {
        return Err(ExecutionError::IllegalAttachment {
            source: source_id,
            target: attachment.target,
            expected,
        });
    }
    Ok(attachment.target)
}

fn matching_objects<S: OracleStateAdapter>(
    state: &S,
    filter: &ObjectFilter,
    context: &ExecutionContext,
) -> Result<Vec<ObjectId>, ExecutionError> {
    let mut objects = Vec::new();
    for id in state.object_ids() {
        if object_matches_filter(state, id, filter, context)? {
            objects.push(id);
        }
    }
    objects.sort_unstable();
    Ok(objects)
}

fn object_matches_filter<S: OracleStateAdapter>(
    state: &S,
    id: ObjectId,
    filter: &ObjectFilter,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    let Some(object) = state.object(id) else {
        return Ok(false);
    };
    if !filter.zones.is_empty() && !filter.zones.contains(&object.zone) {
        return Ok(false);
    }
    if let Some(controller) = &filter.controller
        && !resolve_players(state, controller, context)?.contains(&object.controller)
    {
        return Ok(false);
    }
    if let Some(owner) = &filter.owner
        && !resolve_players(state, owner, context)?.contains(&object.owner)
    {
        return Ok(false);
    }
    if !filter.names.is_empty()
        && !filter.names.iter().any(|name| {
            object
                .characteristics()
                .names
                .iter()
                .any(|actual| actual.eq_ignore_ascii_case(name))
        })
    {
        return Ok(false);
    }
    let type_matches = if filter.card_type_match_any {
        filter.card_types.is_empty()
            || filter
                .card_types
                .iter()
                .any(|card_type| object_has_type(&object, *card_type))
    } else {
        filter
            .card_types
            .iter()
            .all(|card_type| object_has_type(&object, *card_type))
    };
    if !type_matches {
        return Ok(false);
    }
    if filter
        .excluded_card_types
        .iter()
        .any(|card_type| object_has_type(&object, *card_type))
    {
        return Ok(false);
    }
    let characteristics = object.characteristics();
    if !filter
        .supertypes
        .iter()
        .all(|supertype| characteristics.supertypes.contains(supertype))
    {
        return Ok(false);
    }
    if (!filter.subtype_match_any
        && !filter.subtypes.iter().all(|subtype| {
            characteristics
                .subtypes
                .iter()
                .any(|actual| actual.eq_ignore_ascii_case(subtype))
        }))
        || (filter.subtype_match_any
            && !filter.subtypes.is_empty()
            && !filter.subtypes.iter().any(|subtype| {
                characteristics
                    .subtypes
                    .iter()
                    .any(|actual| actual.eq_ignore_ascii_case(subtype))
            }))
    {
        return Ok(false);
    }
    let color_matches = if filter.color_match_any {
        filter.colors.is_empty()
            || filter
                .colors
                .iter()
                .any(|color| characteristics.colors.contains(color))
    } else {
        filter
            .colors
            .iter()
            .all(|color| characteristics.colors.contains(color))
    };
    if !color_matches {
        return Ok(false);
    }
    if filter
        .excluded_colors
        .iter()
        .any(|color| characteristics.colors.contains(color))
    {
        return Ok(false);
    }
    if filter.token.is_some_and(|token| token != object.token) {
        return Ok(false);
    }
    if filter.tapped.is_some_and(|tapped| tapped != object.tapped) {
        return Ok(false);
    }
    if filter
        .attacking
        .is_some_and(|attacking| attacking != object.attacking)
    {
        return Ok(false);
    }
    if filter.other_than_source && object.id == context.source {
        return Ok(false);
    }
    if filter.chosen_creature_type {
        let controller = source_controller(state, context)?;
        let chosen = state
            .player(controller)
            .and_then(|player| player.chosen_creature_type);
        if !chosen.is_some_and(|chosen| {
            characteristics
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case(&chosen))
        }) {
            return Ok(false);
        }
    }
    if let Some((comparison, amount)) = &filter.power {
        let expected = i64::from(evaluate_amount(state, amount, context)?);
        match comparison {
            Comparison::AtLeast if characteristics.power < expected => return Ok(false),
            Comparison::AtMost if characteristics.power > expected => return Ok(false),
            Comparison::Exactly if characteristics.power != expected => return Ok(false),
            Comparison::Greatest => {
                let mut comparison_filter = filter.clone();
                comparison_filter.power = None;
                let greatest = state
                    .object_ids()
                    .into_iter()
                    .filter_map(|candidate_id| {
                        if candidate_id == id {
                            return Some(characteristics.power);
                        }
                        object_matches_filter(state, candidate_id, &comparison_filter, context)
                            .ok()
                            .filter(|matched| *matched)
                            .and_then(|_| state.object(candidate_id))
                            .map(|candidate| candidate.characteristics().power)
                    })
                    .max()
                    .unwrap_or(characteristics.power);
                if characteristics.power != greatest || characteristics.power < expected {
                    return Ok(false);
                }
            }
            Comparison::AtLeast | Comparison::AtMost | Comparison::Exactly => {}
        }
    }
    if let Some((comparison, amount)) = &filter.mana_value {
        let expected = evaluate_amount(state, amount, context)?;
        let actual = characteristics.mana_value;
        match comparison {
            Comparison::AtLeast if actual < expected => return Ok(false),
            Comparison::AtMost if actual > expected => return Ok(false),
            Comparison::Exactly if actual != expected => return Ok(false),
            Comparison::Greatest => {
                let mut comparison_filter = filter.clone();
                comparison_filter.mana_value = None;
                let greatest = state
                    .object_ids()
                    .into_iter()
                    .filter_map(|candidate_id| {
                        object_matches_filter(state, candidate_id, &comparison_filter, context)
                            .ok()
                            .filter(|matched| *matched)
                            .and_then(|_| state.object(candidate_id))
                            .map(|candidate| candidate.characteristics().mana_value)
                    })
                    .max()
                    .unwrap_or(actual);
                if actual != greatest || actual < expected {
                    return Ok(false);
                }
            }
            Comparison::AtLeast | Comparison::AtMost | Comparison::Exactly => {}
        }
    }
    Ok(true)
}

fn object_has_type(object: &PhysicalObject, card_type: CardType) -> bool {
    let characteristics = object.characteristics();
    match card_type {
        CardType::Permanent => {
            object.zone == Zone::Battlefield
                && characteristics.card_types.iter().any(|candidate| {
                    matches!(
                        candidate,
                        CardType::Artifact
                            | CardType::Battle
                            | CardType::Creature
                            | CardType::Enchantment
                            | CardType::Land
                            | CardType::Planeswalker
                    )
                })
        }
        CardType::Spell => object.zone == Zone::Stack,
        _ => characteristics.card_types.contains(&card_type),
    }
}

fn color_index(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Blue => 1,
        Color::Black => 2,
        Color::Red => 3,
        Color::Green => 4,
        Color::Colorless => 5,
    }
}

fn pay_mana_from_player(
    player: &mut PlayerState,
    cost: &ManaCost,
    x_value: u32,
) -> Result<(), String> {
    let expression = parse_resource_cost_expression(&cost.0).map_err(|error| error.to_string())?;
    let [TypedResourceCostComponent::Mana(mana)] = expression.components.as_slice() else {
        return Err("mana payment contains a nonmana resource component".to_owned());
    };
    let mut generic = 0u32;
    for symbol in &mana.symbols {
        match symbol {
            TypedManaSymbol::Generic(amount) => {
                generic = generic
                    .checked_add(*amount)
                    .ok_or_else(|| "mana amount overflow".to_owned())?;
            }
            TypedManaSymbol::VariableX => {
                generic = generic
                    .checked_add(x_value)
                    .ok_or_else(|| "mana amount overflow".to_owned())?;
            }
            TypedManaSymbol::Color(color) => {
                spend_colored_mana(player, runtime_typed_mana_color(*color))?;
            }
            TypedManaSymbol::Hybrid(left, right) => {
                let left = runtime_typed_mana_color(*left);
                let right = runtime_typed_mana_color(*right);
                if !try_spend_colored_mana(player, left) && !try_spend_colored_mana(player, right) {
                    return Err("cannot pay hybrid mana symbol".to_owned());
                }
            }
            TypedManaSymbol::GenericHybrid {
                generic: alternative,
                color,
            } => {
                let color = runtime_typed_mana_color(*color);
                if !try_spend_colored_mana(player, color) {
                    spend_generic(&mut player.mana, *alternative)?;
                }
            }
            TypedManaSymbol::Phyrexian(color) => {
                let color = runtime_typed_mana_color(*color);
                if !try_spend_colored_mana(player, color) {
                    if player.life < 2 {
                        return Err("cannot pay Phyrexian mana with mana or 2 life".to_owned());
                    }
                    player.life -= 2;
                }
            }
            TypedManaSymbol::Snow => {
                return Err("snow payment requires tracked snow-source provenance".to_owned());
            }
        }
    }
    spend_generic(&mut player.mana, generic)
}

fn spend_colored_mana(player: &mut PlayerState, color: Color) -> Result<(), String> {
    if try_spend_colored_mana(player, color) {
        Ok(())
    } else {
        Err(format!("cannot pay {color:?} mana"))
    }
}

fn try_spend_colored_mana(player: &mut PlayerState, color: Color) -> bool {
    let index = color_index(color);
    if player.mana.colored[index] == 0 {
        return false;
    }
    player.mana.colored[index] -= 1;
    true
}

fn mana_symbols(cost: &str) -> Result<Vec<String>, String> {
    let bytes = cost.as_bytes();
    let mut symbols = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if bytes[index] != b'{' {
            return Err(format!("invalid mana cost {cost}"));
        }
        let Some(relative_end) = cost[index + 1..].find('}') else {
            return Err(format!("invalid mana cost {cost}"));
        };
        let end = index + 1 + relative_end;
        let symbol = cost[index + 1..end].trim().to_ascii_uppercase();
        if symbol.is_empty() {
            return Err(format!("invalid mana cost {cost}"));
        }
        symbols.push(symbol);
        index = end + 1;
    }
    Ok(symbols)
}

fn spend_generic(pool: &mut ManaPool, mut amount: u32) -> Result<(), String> {
    let from_unrestricted = pool.unrestricted.min(amount);
    pool.unrestricted -= from_unrestricted;
    amount -= from_unrestricted;
    for index in [5_usize, 0, 1, 2, 3, 4] {
        let spend = pool.colored[index].min(amount);
        pool.colored[index] -= spend;
        amount -= spend;
    }
    if amount == 0 {
        Ok(())
    } else {
        Err("not enough mana".to_owned())
    }
}

fn apply_effect<S: OracleStateAdapter>(
    state: &mut S,
    effect: &Effect,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    match effect {
        Effect::Optional(effects) => {
            if context.optional_effect_declined {
                return Ok(());
            }
            for effect in effects {
                apply_effect(state, effect, context)?;
            }
            Ok(())
        }
        Effect::PayCost(cost) => pay_cost(state, cost, context),
        Effect::AddMana(production) => apply_add_mana(state, production, context),
        Effect::Counter { object } => {
            for id in resolve_objects(state, object, context)? {
                let candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Stack || spell_cannot_be_countered(state, id, context)? {
                    continue;
                }
                state
                    .move_object(id, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("counter:{id}"));
            }
            Ok(())
        }
        Effect::CounterToZone { object, zone } => {
            for id in resolve_objects(state, object, context)? {
                let candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Stack {
                    continue;
                }
                state
                    .move_object(id, *zone)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("counter_to:{id}:{zone:?}"));
            }
            Ok(())
        }
        Effect::Destroy { object } => {
            for id in resolve_objects(state, object, context)? {
                let candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Battlefield
                    || object_is_indestructible(state, &candidate, context)?
                {
                    continue;
                }
                state
                    .move_object(id, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("destroy:{id}"));
            }
            Ok(())
        }
        Effect::MoveZone(zone_move) => apply_zone_move(state, zone_move, context),
        Effect::MoveSelected(zone_move) => apply_selected_zone_move(state, zone_move, context),
        Effect::SetSelectedTapped { selection, tapped } => {
            apply_selected_tapped_state(state, selection, *tapped, context)
        }
        Effect::SearchLibrary(search) => apply_search(state, search, context),
        Effect::ExileTop(exile) => apply_exile_top(state, exile, context),
        Effect::ExileCollection(exile) => apply_exile_collection(state, exile, context),
        Effect::BounceWithControllerCopy(effect) => {
            apply_bounce_with_controller_copy(state, effect, context)
        }
        Effect::GrantCastPermission(permission) => {
            let object_identities = permission
                .objects
                .as_ref()
                .map(|objects| resolve_objects(state, objects, context))
                .transpose()?
                .unwrap_or_default();
            let order = state.next_order();
            state.register_cast_permission(CastPermissionRecord {
                order,
                source_identity: context.source,
                object_identities,
                permission: permission.clone(),
            });
            state.record_mutation(format!("cast_permission:{}:{order}", context.source));
            Ok(())
        }
        Effect::LibraryProcedure(procedure) => apply_library_procedure(state, procedure, context),
        Effect::ShuffleLibrary { player } => {
            for player in resolve_players(state, player, context)? {
                deterministic_shuffle(state, player, context.replay_seed)?;
            }
            Ok(())
        }
        Effect::CreateToken(creation) => apply_create_token(state, creation, context).map(|_| ()),
        Effect::CreateTokenWithDelayedMove {
            creation,
            destination,
            trigger,
        } => {
            let created = apply_create_token(state, creation, context)?;
            let order = state.next_order();
            let effects = created
                .iter()
                .map(|object| {
                    Effect::MoveZone(ZoneMove {
                        object: ObjectRef::ObjectIdentity(*object),
                        from: Some(Zone::Battlefield),
                        to: *destination,
                        tapped: false,
                        face_down: false,
                        delayed_until: None,
                    })
                })
                .collect();
            state.register_delayed_trigger(DelayedTriggerRecord {
                order,
                source_identity: context.source,
                object_identities: created,
                trigger: trigger.clone(),
                effects,
            });
            state.record_mutation(format!(
                "delay_created_move:{}:{destination:?}:{order}",
                context.source
            ));
            Ok(())
        }
        Effect::Draw {
            player,
            amount,
            optional,
            delayed_until,
        } => {
            if *optional && context.optional_effect_declined {
                return Ok(());
            }
            if let Some(trigger) = delayed_until {
                let order = state.next_order();
                state.register_delayed_trigger(DelayedTriggerRecord {
                    order,
                    source_identity: context.source,
                    object_identities: vec![context.source],
                    trigger: trigger.clone(),
                    effects: vec![Effect::Draw {
                        player: player.clone(),
                        amount: amount.clone(),
                        optional: *optional,
                        delayed_until: None,
                    }],
                });
                state.record_mutation(format!("delay_draw:{}:{order}", context.source));
                return Ok(());
            }
            let amount = evaluate_amount(state, amount, context)?;
            for player in resolve_players(state, player, context)? {
                draw_cards(state, player, amount)?;
            }
            Ok(())
        }
        Effect::Discard(selection) => {
            let objects = resolve_object_selection(state, selection, context)?;
            for object in objects {
                state
                    .move_object(object, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("discard:{}:{object}", context.actor));
            }
            Ok(())
        }
        Effect::GainLife { player, amount } => {
            let amount = i64::from(evaluate_amount(state, amount, context)?);
            for player in resolve_players(state, player, context)? {
                change_life(state, player, amount)?;
            }
            Ok(())
        }
        Effect::LoseLife { player, amount } => {
            let amount = i64::from(evaluate_amount(state, amount, context)?);
            for player in resolve_players(state, player, context)? {
                change_life(state, player, -amount)?;
            }
            Ok(())
        }
        Effect::PayLife { player, amount } => {
            let amount = i64::from(evaluate_amount(state, amount, context)?);
            for player in resolve_players(state, player, context)? {
                let current = state
                    .player(player)
                    .ok_or(ExecutionError::MissingPlayer(player))?;
                if current.life <= amount {
                    return Err(ExecutionError::Adapter(format!(
                        "player {player} cannot pay {amount} life"
                    )));
                }
                change_life(state, player, -amount)?;
            }
            Ok(())
        }
        Effect::Damage {
            source,
            recipient,
            amount,
        } => {
            let sources = resolve_objects(state, source, context)?;
            if sources.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "damage source is unavailable",
                ));
            }
            let amount = i64::from(evaluate_amount(state, amount, context)?);
            for player in resolve_players(state, recipient, context)? {
                change_life(state, player, -amount)?;
                state.record_mutation(format!("damage:{}:{player}:{amount}", sources[0]));
            }
            Ok(())
        }
        Effect::PreventDamage {
            combat_only,
            amount,
            duration,
        } => {
            let resolved_amount = evaluate_amount(state, amount, context)?;
            register_continuous_effect(
                state,
                context,
                Vec::new(),
                effect.clone(),
                duration.clone(),
            );
            state.record_mutation(format!(
                "prevent_damage:{}:{combat_only}:{resolved_amount}",
                context.source
            ));
            Ok(())
        }
        Effect::Tap { object } => set_tapped(state, object, context, true),
        Effect::Untap { object } => set_tapped(state, object, context, false),
        Effect::Scry { player, amount } => {
            let amount = evaluate_amount(state, amount, context)?;
            for player in resolve_players(state, player, context)? {
                scry(state, player, amount, context)?;
            }
            Ok(())
        }
        Effect::Surveil { player, amount } => {
            let amount = evaluate_amount(state, amount, context)?;
            for player in resolve_players(state, player, context)? {
                surveil(state, player, amount, context)?;
            }
            Ok(())
        }
        Effect::Mill { player, amount } => {
            let amount = evaluate_amount(state, amount, context)?;
            for player in resolve_players(state, player, context)? {
                mill(state, player, amount)?;
            }
            Ok(())
        }
        Effect::Manifest { player, card } => {
            let players = resolve_players(state, player, context)?;
            let cards = resolve_objects(state, card, context)?;
            if players.is_empty() || cards.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "manifest requires a player and a card",
                ));
            }
            for (index, id) in cards.into_iter().enumerate() {
                let controller = players[index.min(players.len() - 1)];
                state
                    .move_object(id, Zone::Battlefield)
                    .map_err(ExecutionError::Adapter)?;
                let mut object = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                object.controller = controller;
                object.face_down = true;
                object.front.card_types = vec![CardType::Creature];
                object.front.power = 2;
                object.front.toughness = 2;
                state.put_object(object).map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("manifest:{id}:{controller}"));
            }
            Ok(())
        }
        Effect::PutCounter {
            object,
            counter,
            amount,
        } => apply_put_counter(state, object, counter, amount, context),
        Effect::ModifyPowerToughness(change) => {
            apply_power_toughness_change(state, change, context)
        }
        Effect::GrantKeyword {
            objects,
            keywords,
            duration,
        } => {
            let ids = resolve_objects(state, objects, context)?;
            if matches!(objects, ObjectRef::AttachmentTarget { .. }) {
                register_continuous_if_needed(
                    state,
                    context,
                    ids,
                    effect.clone(),
                    duration.clone(),
                );
                return Ok(());
            }
            for id in &ids {
                let mut object = state
                    .object(*id)
                    .ok_or(ExecutionError::MissingObject(*id))?;
                for keyword in keywords {
                    if !object.characteristics().keywords.contains(keyword) {
                        object.characteristics_mut().keywords.push(keyword.clone());
                    }
                }
                state.put_object(object).map_err(ExecutionError::Adapter)?;
            }
            register_continuous_if_needed(state, context, ids, effect.clone(), duration.clone());
            Ok(())
        }
        Effect::GrantAbility {
            objects,
            ability,
            duration,
        } => {
            let ids = resolve_objects(state, objects, context)?;
            if matches!(objects, ObjectRef::AttachmentTarget { .. }) {
                register_continuous_if_needed(
                    state,
                    context,
                    ids,
                    effect.clone(),
                    duration.clone(),
                );
                return Ok(());
            }
            for id in &ids {
                let mut object = state
                    .object(*id)
                    .ok_or(ExecutionError::MissingObject(*id))?;
                object.characteristics_mut().abilities.push(ability.clone());
                state.put_object(object).map_err(ExecutionError::Adapter)?;
            }
            register_continuous_if_needed(state, context, ids, effect.clone(), duration.clone());
            Ok(())
        }
        Effect::LoseAllAbilities { object, duration } => {
            let ids = resolve_objects(state, object, context)?;
            if matches!(object, ObjectRef::AttachmentTarget { .. }) {
                register_continuous_if_needed(
                    state,
                    context,
                    ids,
                    effect.clone(),
                    duration.clone(),
                );
                return Ok(());
            }
            for id in &ids {
                let mut object = state
                    .object(*id)
                    .ok_or(ExecutionError::MissingObject(*id))?;
                object.characteristics_mut().abilities.clear();
                object.characteristics_mut().keywords.clear();
                state.put_object(object).map_err(ExecutionError::Adapter)?;
            }
            register_continuous_if_needed(state, context, ids, effect.clone(), duration.clone());
            Ok(())
        }
        Effect::SetCharacteristics(characteristics) => {
            apply_set_characteristics(state, characteristics, context)
        }
        Effect::Restriction(restriction) => {
            let attachment_object = match restriction {
                Restriction::DoesNotUntapDuring { object, .. }
                | Restriction::ActivatedAbilitiesCannotBeActivated { object, .. }
                | Restriction::MustAttackEachCombatIfAble { object, .. }
                | Restriction::CannotAttack { object, .. }
                | Restriction::CannotBlock { object, .. }
                | Restriction::CannotBeBlocked { object, .. }
                    if matches!(object, ObjectRef::AttachmentTarget { .. }) =>
                {
                    Some(object)
                }
                _ => None,
            };
            if let Some(object) = attachment_object {
                let resolved = resolve_objects(state, object, context)?;
                if resolved.is_empty() {
                    return Err(ExecutionError::InvalidAmount(
                        "attachment restriction has no physical object",
                    ));
                }
            }
            let order = state.next_order();
            state.register_restriction(RestrictionRecord {
                order,
                source_identity: context.source,
                restriction: restriction.clone(),
            });
            state.record_mutation(format!("restriction:{}:{order}", context.source));
            Ok(())
        }
        Effect::Replacement(replacement) => {
            let order = state.next_order();
            state.register_replacement(ReplacementRecord {
                order,
                source_identity: context.source,
                effect: replacement.as_ref().clone(),
            });
            state.record_mutation(format!("replacement:{}:{order}", context.source));
            Ok(())
        }
        Effect::Copy(copy) => apply_copy(state, copy, context).map(|_| ()),
        Effect::Transform { object } => {
            for id in resolve_objects(state, object, context)? {
                let mut candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.back.is_none() {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} has no other face"
                    )));
                }
                candidate.active_face = if candidate.active_face == 0 { 1 } else { 0 };
                state
                    .put_object(candidate)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("transform:{id}"));
            }
            Ok(())
        }
        Effect::ResolveWard {
            payer,
            source,
            cost,
        } => resolve_ward(state, payer, source, cost, context),
        Effect::Animate(animation) => apply_animation(state, animation, context),
        Effect::ChooseCreatureType { player } => {
            let chosen = context
                .chosen_creature_type
                .as_ref()
                .map(|choice| choice.trim())
                .filter(|choice| !choice.is_empty())
                .ok_or(ExecutionError::InvalidAmount(
                    "creature type choice is unavailable",
                ))?
                .to_owned();
            for player in resolve_players(state, player, context)? {
                let mut player_state = state
                    .player(player)
                    .ok_or(ExecutionError::MissingPlayer(player))?;
                player_state.chosen_creature_type = Some(chosen.clone());
                state
                    .put_player(player_state)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("choose_type:{player}:{chosen}"));
            }
            Ok(())
        }
        Effect::LookAtTop { player, amount } => {
            let amount = evaluate_amount(state, amount, context)? as usize;
            for player in resolve_players(state, player, context)? {
                let cards = state
                    .player(player)
                    .ok_or(ExecutionError::MissingPlayer(player))?
                    .library
                    .into_iter()
                    .take(amount)
                    .collect::<Vec<_>>();
                state.record_mutation(format!("look_top:{player}:{}", cards.len()));
                set_looked_at(state, player, cards)?;
            }
            Ok(())
        }
        Effect::SelectFromLookedAt {
            player,
            amount,
            predicate,
            reveal,
            destination,
        } => {
            let amount = evaluate_amount(state, amount, context)? as usize;
            for player in resolve_players(state, player, context)? {
                select_from_looked_at(
                    state,
                    player,
                    amount,
                    predicate,
                    *reveal,
                    *destination,
                    context,
                )?;
            }
            Ok(())
        }
        Effect::PutRestOnLibraryBottom { player, order } => {
            for player in resolve_players(state, player, context)? {
                put_rest_on_bottom(state, player, *order)?;
            }
            Ok(())
        }
        Effect::ExileSpellAfterResolution { object } => {
            for id in resolve_objects(state, object, context)? {
                let candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Stack && candidate.zone != Zone::Graveyard {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} is not resolving"
                    )));
                }
                state
                    .move_object(id, Zone::Exile)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("exile_after_resolution:{id}"));
            }
            Ok(())
        }
        Effect::CopyStackObject {
            object,
            may_choose_new_targets,
        } => {
            let originals = resolve_objects(state, object, context)?;
            let original = *originals
                .first()
                .ok_or(ExecutionError::InvalidAmount("copy has no stack object"))?;
            let mut copy = state
                .object(original)
                .ok_or(ExecutionError::MissingObject(original))?;
            if copy.zone != Zone::Stack {
                return Err(ExecutionError::Adapter(format!(
                    "object {original} is not on the stack"
                )));
            }
            let id = state.allocate_object_id();
            copy.id = id;
            copy.origin_id = id;
            copy.copy_of = Some(original);
            copy.token = false;
            state
                .insert_physical_object(copy)
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!(
                "copy_stack:{original}:{id}:{may_choose_new_targets}"
            ));
            Ok(())
        }
        Effect::ChooseNewTargets { object } => {
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "retarget effect has no spell or ability",
                ));
            }
            state.record_mutation(format!("choose_new_targets:{objects:?}"));
            Ok(())
        }
        Effect::ChangeControl { object, controller } => {
            if matches!(object, ObjectRef::AttachmentTarget { .. }) {
                let ids = resolve_objects(state, object, context)?;
                if ids.is_empty() {
                    return Err(ExecutionError::InvalidAmount(
                        "attachment control effect has no physical object",
                    ));
                }
                register_continuous_effect(
                    state,
                    context,
                    ids,
                    effect.clone(),
                    Duration::WhileSourceOnBattlefield,
                );
                return Ok(());
            }
            let controllers = resolve_players(state, controller, context)?;
            let controller = *controllers
                .first()
                .ok_or(ExecutionError::InvalidAmount("controller is unavailable"))?;
            for id in resolve_objects(state, object, context)? {
                let mut candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                candidate.controller = controller;
                state
                    .put_object(candidate)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("control:{id}:{controller}"));
            }
            Ok(())
        }
        Effect::SkipStep { player, step } => {
            for player in resolve_players(state, player, context)? {
                let order = state.next_order();
                state.register_skipped_step(SkippedStepRecord {
                    order,
                    source_identity: context.source,
                    player,
                    step: step.clone(),
                });
                state.record_mutation(format!("skip_step:{player}:{step:?}:{order}"));
            }
            Ok(())
        }
        Effect::WinGame { player } => register_game_result(state, player, GameResult::Won, context),
        Effect::LoseGame { player } => {
            register_game_result(state, player, GameResult::Lost, context)
        }
        Effect::TakeExtraTurn(effect) => apply_extra_turn(state, effect, context),
        Effect::SchedulePaymentOrLose(effect) => apply_payment_or_lose(state, effect, context),
        Effect::CastCopy(copy) => apply_cast_copy(state, copy, context),
        Effect::ReduceActivationCost {
            mana,
            per,
            minimum_total,
        } => {
            let order = state.next_order();
            state.register_activation_reduction(ActivationReductionRecord {
                order,
                source_identity: context.source,
                mana: mana.clone(),
                per: per.clone(),
                minimum_total: minimum_total.clone(),
            });
            state.record_mutation(format!("activation_reduction:{}:{order}", context.source));
            Ok(())
        }
        Effect::ReduceSpellCost {
            object,
            mana,
            per,
            maximum_reduction,
        } => {
            let order = state.next_order();
            state.register_spell_reduction(SpellReductionRecord {
                order,
                source_identity: context.source,
                object: object.clone(),
                mana: mana.clone(),
                per: per.clone(),
                maximum_reduction: maximum_reduction.clone(),
            });
            state.record_mutation(format!("spell_reduction:{}:{order}", context.source));
            Ok(())
        }
        Effect::ChooseMode { count } => {
            if !choice_count_matches(count, &context.selected_modes) {
                return Err(ExecutionError::InvalidAmount("mode choice is illegal"));
            }
            state.record_mutation(format!(
                "choose_mode:{}:{:?}",
                context.source, context.selected_modes
            ));
            Ok(())
        }
        Effect::StandaloneRuleProgram(_) => Err(ExecutionError::InvalidAmount(
            "standalone rule program requires its dedicated state adapter",
        )),
        Effect::Conditional {
            condition,
            if_true,
            if_false,
        } => {
            let branch = if condition_holds(state, condition, context)? {
                if_true
            } else {
                if_false
            };
            for nested in branch {
                apply_effect(state, nested, context)?;
            }
            Ok(())
        }
    }
}

fn apply_exile_top<S: OracleStateAdapter>(
    state: &mut S,
    exile: &TopLibraryExile,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let amount = evaluate_amount(state, &exile.amount, context)? as usize;
    for player in resolve_players(state, &exile.player, context)? {
        let cards = state
            .player(player)
            .ok_or(ExecutionError::MissingPlayer(player))?
            .library
            .into_iter()
            .take(amount)
            .collect::<Vec<_>>();
        for id in &cards {
            state
                .move_object(*id, Zone::Exile)
                .map_err(ExecutionError::Adapter)?;
            if exile.face_down {
                let mut card = state
                    .object(*id)
                    .ok_or(ExecutionError::MissingObject(*id))?;
                card.face_down = true;
                state.put_object(card).map_err(ExecutionError::Adapter)?;
            }
        }
        if let Some(permission) = &exile.cast_permission {
            let order = state.next_order();
            state.register_cast_permission(CastPermissionRecord {
                order,
                source_identity: context.source,
                object_identities: cards.clone(),
                permission: permission.clone(),
            });
        }
        if let Some((destination, trigger)) = &exile.delayed_destination {
            let order = state.next_order();
            let effects = cards
                .iter()
                .map(|object| {
                    Effect::MoveZone(ZoneMove {
                        object: ObjectRef::ObjectIdentity(*object),
                        from: Some(Zone::Exile),
                        to: *destination,
                        tapped: false,
                        face_down: false,
                        delayed_until: None,
                    })
                })
                .collect();
            state.register_delayed_trigger(DelayedTriggerRecord {
                order,
                source_identity: context.source,
                object_identities: cards.clone(),
                trigger: trigger.clone(),
                effects,
            });
            state.record_mutation(format!(
                "delay_exiled_move:{}:{destination:?}:{order}",
                context.source
            ));
        }
        state.record_mutation(format!("exile_top:{player}:{}", cards.len()));
    }
    Ok(())
}

fn apply_exile_collection<S: OracleStateAdapter>(
    state: &mut S,
    exile: &ExileCollectionEffect,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let cards = resolve_objects(state, &exile.objects, context)?
        .into_iter()
        .filter(|object| {
            state
                .object(*object)
                .is_some_and(|card| card.zone == exile.from)
        })
        .collect::<Vec<_>>();
    for card in &cards {
        state
            .move_object(*card, Zone::Exile)
            .map_err(ExecutionError::Adapter)?;
    }
    if let Some(permission) = &exile.cast_permission {
        let order = state.next_order();
        state.register_cast_permission(CastPermissionRecord {
            order,
            source_identity: context.source,
            object_identities: cards.clone(),
            permission: permission.clone(),
        });
        state.record_mutation(format!(
            "collection_cast_permission:{}:{order}",
            context.source
        ));
    }
    if let Some((destination, trigger)) = &exile.delayed_destination {
        let order = state.next_order();
        let effects = cards
            .iter()
            .map(|card| {
                Effect::MoveZone(ZoneMove {
                    object: ObjectRef::ObjectIdentity(*card),
                    from: Some(Zone::Exile),
                    to: *destination,
                    tapped: false,
                    face_down: false,
                    delayed_until: None,
                })
            })
            .collect();
        state.register_delayed_trigger(DelayedTriggerRecord {
            order,
            source_identity: context.source,
            object_identities: cards.clone(),
            trigger: trigger.clone(),
            effects,
        });
        state.record_mutation(format!(
            "collection_delayed_move:{}:{destination:?}:{order}",
            context.source
        ));
    }
    state.record_mutation(format!(
        "exile_collection:{}:{}",
        context.source,
        cards.len()
    ));
    Ok(())
}

fn apply_bounce_with_controller_copy<S: OracleStateAdapter>(
    state: &mut S,
    effect: &BounceWithControllerCopyEffect,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let objects = resolve_objects(state, &effect.object, context)?;
    if objects.len() != 1 {
        return Err(ExecutionError::InvalidAmount(
            "bounce and copy procedure requires one permanent",
        ));
    }
    let object = objects[0];
    let candidate = state
        .object(object)
        .ok_or(ExecutionError::MissingObject(object))?;
    if candidate.zone != Zone::Battlefield {
        return Ok(());
    }
    state
        .move_object(object, Zone::Hand)
        .map_err(ExecutionError::Adapter)?;
    state.record_mutation(format!("bounce:{object}"));
    if context.optional_effect_declined {
        return Ok(());
    }
    for land in resolve_object_selection(state, &effect.sacrifice, context)? {
        state
            .move_object(land, Zone::Graveyard)
            .map_err(ExecutionError::Adapter)?;
        state.record_mutation(format!("sacrifice_for_copy:{land}"));
    }
    let originals = resolve_objects(state, &effect.copy_source, context)?;
    let original = *originals
        .first()
        .ok_or(ExecutionError::InvalidAmount("copy has no stack object"))?;
    let mut copy = state
        .object(original)
        .ok_or(ExecutionError::MissingObject(original))?;
    if copy.zone != Zone::Stack {
        return Err(ExecutionError::Adapter(format!(
            "object {original} is not on the stack"
        )));
    }
    let id = state.allocate_object_id();
    copy.id = id;
    copy.origin_id = id;
    copy.copy_of = Some(original);
    copy.token = false;
    state
        .insert_physical_object(copy)
        .map_err(ExecutionError::Adapter)?;
    state.record_mutation(format!(
        "copy_stack:{original}:{id}:{}",
        effect.may_choose_new_targets
    ));
    Ok(())
}

fn apply_library_procedure<S: OracleStateAdapter>(
    state: &mut S,
    procedure: &LibraryProcedure,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    match procedure {
        LibraryProcedure::DiscardHandsAndDraw { player, amount } => {
            let amount = evaluate_amount(state, amount, context)?;
            for player in resolve_players(state, player, context)? {
                let hand = state
                    .object_ids()
                    .into_iter()
                    .filter_map(|id| state.object(id))
                    .filter(|object| object.zone == Zone::Hand && object.owner == player)
                    .map(|object| object.id)
                    .collect::<Vec<_>>();
                for card in hand {
                    state
                        .move_object(card, Zone::Graveyard)
                        .map_err(ExecutionError::Adapter)?;
                }
                draw_cards(state, player, amount)?;
            }
            Ok(())
        }
        LibraryProcedure::RevealTopToHandLoseManaValue { player, repeat } => {
            let amount = evaluate_amount(state, repeat, context)?;
            for player in resolve_players(state, player, context)? {
                for _ in 0..amount {
                    let Some(card) = state
                        .player(player)
                        .ok_or(ExecutionError::MissingPlayer(player))?
                        .library
                        .first()
                        .copied()
                    else {
                        break;
                    };
                    let mana_value = state
                        .object(card)
                        .ok_or(ExecutionError::MissingObject(card))?
                        .characteristics()
                        .mana_value;
                    state
                        .move_object(card, Zone::Hand)
                        .map_err(ExecutionError::Adapter)?;
                    change_life(state, player, -i64::from(mana_value))?;
                }
            }
            Ok(())
        }
        LibraryProcedure::ExileUntilNamedCard {
            player,
            initial_exile,
        } => {
            let selected_name = context
                .selected_card_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .ok_or(ExecutionError::InvalidAmount(
                    "a selected card name is required",
                ))?;
            for player in resolve_players(state, player, context)? {
                let initial = state
                    .player(player)
                    .ok_or(ExecutionError::MissingPlayer(player))?
                    .library
                    .into_iter()
                    .take(*initial_exile as usize)
                    .collect::<Vec<_>>();
                for card in initial {
                    state
                        .move_object(card, Zone::Exile)
                        .map_err(ExecutionError::Adapter)?;
                }
                while let Some(card) = state
                    .player(player)
                    .ok_or(ExecutionError::MissingPlayer(player))?
                    .library
                    .first()
                    .copied()
                {
                    let named = state
                        .object(card)
                        .ok_or(ExecutionError::MissingObject(card))?
                        .characteristics()
                        .names
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(selected_name));
                    state
                        .move_object(card, if named { Zone::Hand } else { Zone::Exile })
                        .map_err(ExecutionError::Adapter)?;
                    if named {
                        break;
                    }
                }
            }
            Ok(())
        }
        LibraryProcedure::ExileUntilAcceptedOrDuplicate { player } => {
            for player in resolve_players(state, player, context)? {
                let mut seen = BTreeSet::new();
                while let Some(card) = state
                    .player(player)
                    .ok_or(ExecutionError::MissingPlayer(player))?
                    .library
                    .first()
                    .copied()
                {
                    let name = state
                        .object(card)
                        .ok_or(ExecutionError::MissingObject(card))?
                        .characteristics()
                        .names
                        .first()
                        .cloned()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    if !seen.insert(name) {
                        state
                            .move_object(card, Zone::Exile)
                            .map_err(ExecutionError::Adapter)?;
                        break;
                    }
                    if context.accepted_library_card == Some(card) {
                        state
                            .move_object(card, Zone::Hand)
                            .map_err(ExecutionError::Adapter)?;
                        break;
                    }
                    state
                        .move_object(card, Zone::Exile)
                        .map_err(ExecutionError::Adapter)?;
                }
            }
            Ok(())
        }
        LibraryProcedure::DevotionLookAndWin { player, color } => {
            for player in resolve_players(state, player, context)? {
                let devotion = context.devotion_by_color[color_index(*color)] as usize;
                let library = state
                    .player(player)
                    .ok_or(ExecutionError::MissingPlayer(player))?
                    .library;
                let looked = library.iter().copied().take(devotion).collect::<Vec<_>>();
                let keep = context
                    .library_choices
                    .get(&player)
                    .and_then(|choice| choice.keep_on_top.first())
                    .copied()
                    .filter(|card| looked.contains(card));
                let mut player_state = state
                    .player(player)
                    .ok_or(ExecutionError::MissingPlayer(player))?;
                player_state.library.retain(|card| !looked.contains(card));
                let mut bottom = looked
                    .into_iter()
                    .filter(|card| Some(*card) != keep)
                    .collect::<Vec<_>>();
                bottom.sort_by_key(|card| mix64(context.replay_seed ^ *card));
                if let Some(card) = keep {
                    player_state.library.insert(0, card);
                }
                player_state.library.extend(bottom);
                let wins = devotion >= player_state.library.len();
                state
                    .put_player(player_state)
                    .map_err(ExecutionError::Adapter)?;
                if wins {
                    let order = state.next_order();
                    state.register_game_result(GameResultRecord {
                        order,
                        source_identity: context.source,
                        player,
                        result: GameResult::Won,
                    });
                }
            }
            Ok(())
        }
    }
}

fn register_game_result<S: OracleStateAdapter>(
    state: &mut S,
    player: &PlayerRef,
    result: GameResult,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    for player in resolve_players(state, player, context)? {
        let order = state.next_order();
        state.register_game_result(GameResultRecord {
            order,
            source_identity: context.source,
            player,
            result,
        });
        state.record_mutation(format!("game_result:{player}:{result:?}:{order}"));
    }
    Ok(())
}

fn apply_extra_turn<S: OracleStateAdapter>(
    state: &mut S,
    effect: &ExtraTurnEffect,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    for player in resolve_players(state, &effect.player, context)? {
        let order = state.next_order();
        state.register_extra_turn(ExtraTurnRecord {
            order,
            source_identity: context.source,
            player,
            lose_at_end_step: effect.lose_at_end_step,
        });
        if effect.lose_at_end_step {
            let delayed_order = state.next_order();
            state.register_delayed_trigger(DelayedTriggerRecord {
                order: delayed_order,
                source_identity: context.source,
                object_identities: Vec::new(),
                trigger: Trigger::BeginningOfNextEndStep,
                effects: vec![Effect::LoseGame {
                    player: PlayerRef::PlayerIdentity(player),
                }],
            });
            state.record_mutation(format!("extra_turn_loss:{player}:{delayed_order}"));
        }
        state.record_mutation(format!(
            "extra_turn:{player}:{}:{order}",
            effect.lose_at_end_step
        ));
    }
    Ok(())
}

fn apply_payment_or_lose<S: OracleStateAdapter>(
    state: &mut S,
    effect: &PaymentOrLoseEffect,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    for player in resolve_players(state, &effect.player, context)? {
        let order = state.next_order();
        state.register_payment_or_lose(PaymentOrLoseRecord {
            order,
            source_identity: context.source,
            player,
            cost: effect.cost.clone(),
            trigger: effect.trigger.clone(),
        });
        state.record_mutation(format!("payment_or_lose:{player}:{order}"));
    }
    Ok(())
}

fn apply_add_mana<S: OracleStateAdapter>(
    state: &mut S,
    production: &ManaProduction,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let amount = if let Some(typed) = &production.typed {
        if !typed_mana_production_has_contract(typed) {
            return Err(ExecutionError::InvalidAmount(
                "typed mana production has no executable contract",
            ));
        }
        evaluate_typed_mana_quantity(state, &typed.quantity, context)?
    } else {
        let base = evaluate_amount(state, &production.amount, context)?;
        let scale = match &production.scales_with {
            Some(expression) => evaluate_count(state, expression, context)?,
            None => 1,
        };
        base.checked_mul(scale)
            .ok_or(ExecutionError::ArithmeticOverflow)?
    };
    if amount == 0 {
        return Ok(());
    }
    for player in resolve_players(state, &production.player, context)? {
        let player_state = state
            .player(player)
            .ok_or(ExecutionError::MissingPlayer(player))?;
        let selected = if let Some(typed) = &production.typed {
            choose_typed_mana_composition(
                &typed.composition,
                amount,
                &player_state.commander_identity,
                context.mana_production_choice.as_deref(),
            )?
        } else {
            choose_mana_choice(
                &production.choices,
                production.commander_identity_only,
                &player_state.commander_identity,
            )
            .map(|choice| choice.symbols.clone())
            .ok_or(ExecutionError::InvalidAmount(
                "mana production has no legal choice",
            ))?
        };
        state
            .add_mana(player, &selected, amount)
            .map_err(ExecutionError::Adapter)?;
    }
    Ok(())
}

fn evaluate_typed_mana_quantity<S: OracleStateAdapter>(
    state: &S,
    quantity: &TypedManaQuantity,
    context: &ExecutionContext,
) -> Result<u32, ExecutionError> {
    match quantity {
        TypedManaQuantity::Fixed(amount) => Ok(*amount),
        TypedManaQuantity::X { defined_as: None } => Ok(context.x_value),
        TypedManaQuantity::X {
            defined_as: Some(calculation),
        }
        | TypedManaQuantity::Calculated(calculation) => {
            evaluate_typed_mana_calculation(state, calculation, context)
        }
    }
}

fn evaluate_typed_mana_calculation<S: OracleStateAdapter>(
    state: &S,
    calculation: &TypedQuantityCalculation,
    context: &ExecutionContext,
) -> Result<u32, ExecutionError> {
    match calculation {
        TypedQuantityCalculation::Value(TypedCalculatedValue::Constant(amount)) => Ok(*amount),
        TypedQuantityCalculation::Value(TypedCalculatedValue::SourcePower) => {
            let source = state
                .object(context.source)
                .ok_or(ExecutionError::MissingObject(context.source))?;
            u32::try_from(source.characteristics().power.max(0))
                .map_err(|_| ExecutionError::ArithmeticOverflow)
        }
        TypedQuantityCalculation::Value(TypedCalculatedValue::Count(objects)) => {
            evaluate_typed_mana_count(state, objects, context)
        }
        TypedQuantityCalculation::Sum(terms) => {
            let mut total = 0u32;
            for term in terms {
                total = total
                    .checked_add(evaluate_typed_mana_calculation(state, term, context)?)
                    .ok_or(ExecutionError::ArithmeticOverflow)?;
            }
            Ok(total)
        }
        TypedQuantityCalculation::Value(
            TypedCalculatedValue::SacrificedCreatureManaValue
            | TypedCalculatedValue::SacrificedPermanentManaValue
            | TypedCalculatedValue::ManaSpentToCastReferencedSpell,
        ) => Err(ExecutionError::InvalidAmount(
            "typed mana calculation requires unavailable event state",
        )),
    }
}

fn evaluate_typed_mana_count<S: OracleStateAdapter>(
    state: &S,
    objects: &TypedCountedObjects,
    context: &ExecutionContext,
) -> Result<u32, ExecutionError> {
    if !typed_mana_count_has_contract(objects) {
        return Err(ExecutionError::InvalidAmount(
            "typed mana count has no executable contract",
        ));
    }
    let count = state
        .object_ids()
        .into_iter()
        .filter_map(|id| state.object(id))
        .filter(|object| object.zone == Zone::Battlefield && object.controller == context.actor)
        .filter(
            |object| match (objects.card_type, objects.subtype, objects.keyword) {
                (
                    Some(TypedCountedCardType::Creature),
                    None,
                    Some(TypedCountedKeyword::Defender),
                ) => {
                    object_has_type(object, CardType::Creature)
                        && object
                            .characteristics()
                            .keywords
                            .contains(&Keyword::Defender)
                }
                (None, Some(TypedCountedSubtype::Shrine), None) => object
                    .characteristics()
                    .subtypes
                    .iter()
                    .any(|subtype| subtype.eq_ignore_ascii_case("Shrine")),
                _ => false,
            },
        )
        .count();
    u32::try_from(count).map_err(|_| ExecutionError::ArithmeticOverflow)
}

fn choose_typed_mana_composition(
    composition: &TypedManaComposition,
    amount: u32,
    commander_identity: &[Color],
    requested: Option<&[Color]>,
) -> Result<Vec<Color>, ExecutionError> {
    let amount = usize::try_from(amount).map_err(|_| ExecutionError::ArithmeticOverflow)?;
    match composition {
        TypedManaComposition::Exact(colors) => {
            let exact = expand_exact_mana_colors(colors, amount)?;
            if requested.is_some_and(|requested| requested != exact.as_slice()) {
                return Err(ExecutionError::InvalidAmount(
                    "the requested mana choice does not match the exact composition",
                ));
            }
            Ok(exact)
        }
        TypedManaComposition::OneOf(choices) => {
            let expanded = choices
                .iter()
                .map(|choice| expand_exact_mana_colors(choice, amount))
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(requested) = requested {
                return expanded
                    .into_iter()
                    .find(|choice| choice.as_slice() == requested)
                    .ok_or(ExecutionError::InvalidAmount(
                        "the requested mana choice is not one of the printed choices",
                    ));
            }
            expanded
                .into_iter()
                .next()
                .ok_or(ExecutionError::InvalidAmount(
                    "typed mana production has no printed choice",
                ))
        }
        TypedManaComposition::AnyOneColor => choose_repeated_mana_color(
            &[
                Color::White,
                Color::Blue,
                Color::Black,
                Color::Red,
                Color::Green,
            ],
            amount,
            requested,
        ),
        TypedManaComposition::AnyCombination(domain) => {
            let allowed = typed_mana_domain_colors(domain);
            choose_mana_combination(&allowed, amount, requested, false)
        }
        TypedManaComposition::DifferentColors(domain) => {
            let allowed = typed_mana_domain_colors(domain);
            choose_mana_combination(&allowed, amount, requested, true)
        }
        TypedManaComposition::Derived(TypedDerivedManaTypes::CommanderColorIdentity) => {
            let mut allowed = Vec::new();
            for color in commander_identity {
                if !allowed.contains(color) {
                    allowed.push(*color);
                }
            }
            choose_repeated_mana_color(&allowed, amount, requested)
        }
        TypedManaComposition::Derived(_) => Err(ExecutionError::InvalidAmount(
            "typed mana composition requires unavailable derived mana types",
        )),
    }
}

fn expand_exact_mana_colors(
    colors: &[TypedManaColor],
    amount: usize,
) -> Result<Vec<Color>, ExecutionError> {
    let colors = colors
        .iter()
        .copied()
        .map(runtime_typed_mana_color)
        .collect::<Vec<_>>();
    if colors.len() == amount {
        return Ok(colors);
    }
    if colors.len() == 1 {
        return Ok(vec![colors[0]; amount]);
    }
    Err(ExecutionError::InvalidAmount(
        "typed mana quantity does not match its exact composition",
    ))
}

fn choose_repeated_mana_color(
    allowed: &[Color],
    amount: usize,
    requested: Option<&[Color]>,
) -> Result<Vec<Color>, ExecutionError> {
    let color = match requested {
        Some([color]) if allowed.contains(color) => *color,
        Some(requested)
            if requested.len() == amount
                && requested
                    .first()
                    .is_some_and(|first| requested.iter().all(|color| color == first))
                && requested.iter().all(|color| allowed.contains(color)) =>
        {
            requested[0]
        }
        Some(_) => {
            return Err(ExecutionError::InvalidAmount(
                "the requested mana choice is not one legal color",
            ));
        }
        None => *allowed.first().ok_or(ExecutionError::InvalidAmount(
            "typed mana production has no legal color",
        ))?,
    };
    Ok(vec![color; amount])
}

fn choose_mana_combination(
    allowed: &[Color],
    amount: usize,
    requested: Option<&[Color]>,
    require_distinct: bool,
) -> Result<Vec<Color>, ExecutionError> {
    if let Some(requested) = requested {
        if requested.len() != amount
            || requested.iter().any(|color| !allowed.contains(color))
            || (require_distinct && has_duplicate_colors(requested))
        {
            return Err(ExecutionError::InvalidAmount(
                "the requested mana distribution is not legal for the printed composition",
            ));
        }
        return Ok(requested.to_vec());
    }
    if require_distinct {
        if amount > allowed.len() {
            return Err(ExecutionError::InvalidAmount(
                "the printed quantity exceeds the available different mana types",
            ));
        }
        return Ok(allowed.iter().copied().take(amount).collect());
    }
    let color = *allowed.first().ok_or(ExecutionError::InvalidAmount(
        "typed mana production has no legal mana type",
    ))?;
    Ok(vec![color; amount])
}

fn has_duplicate_colors(colors: &[Color]) -> bool {
    colors
        .iter()
        .enumerate()
        .any(|(index, color)| colors[..index].contains(color))
}

fn typed_mana_domain_colors(domain: &TypedManaColorDomain) -> Vec<Color> {
    match domain {
        TypedManaColorDomain::Colors => vec![
            Color::White,
            Color::Blue,
            Color::Black,
            Color::Red,
            Color::Green,
        ],
        TypedManaColorDomain::ManaTypes => vec![
            Color::White,
            Color::Blue,
            Color::Black,
            Color::Red,
            Color::Green,
            Color::Colorless,
        ],
        TypedManaColorDomain::Explicit(colors) => colors
            .iter()
            .copied()
            .map(runtime_typed_mana_color)
            .collect(),
    }
}

fn runtime_typed_mana_color(color: TypedManaColor) -> Color {
    match color {
        TypedManaColor::White => Color::White,
        TypedManaColor::Blue => Color::Blue,
        TypedManaColor::Black => Color::Black,
        TypedManaColor::Red => Color::Red,
        TypedManaColor::Green => Color::Green,
        TypedManaColor::Colorless => Color::Colorless,
    }
}

fn choose_mana_choice<'a>(
    choices: &'a [ManaChoice],
    commander_identity_only: bool,
    identity: &[Color],
) -> Option<&'a ManaChoice> {
    choices.iter().find(|choice| {
        !choice.symbols.is_empty()
            && (!commander_identity_only
                || choice
                    .symbols
                    .iter()
                    .all(|color| identity.contains(color) || *color == Color::Colorless))
    })
}

fn spell_cannot_be_countered<S: OracleStateAdapter>(
    state: &S,
    object: ObjectId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    for record in sorted_restrictions(state) {
        let mut local = context.clone();
        local.source = record.source_identity;
        match &record.restriction {
            Restriction::SpellCannotBeCountered { object: protected }
                if resolve_objects(state, protected, &local)?.contains(&object) =>
            {
                return Ok(true);
            }
            Restriction::MatchingSpellsCannotBeCountered { player, filter } => {
                let controllers = resolve_players(state, player, &local)?;
                if state
                    .object(object)
                    .is_some_and(|spell| controllers.contains(&spell.controller))
                    && object_matches_filter(state, object, filter, &local)?
                {
                    return Ok(true);
                }
            }
            _ => {}
        }
    }
    Ok(false)
}

fn object_is_indestructible<S: OracleStateAdapter>(
    state: &S,
    object: &PhysicalObject,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    if object
        .characteristics()
        .keywords
        .contains(&Keyword::Indestructible)
        || object
            .counters
            .get(&counter_key(&CounterKind::Indestructible))
            .copied()
            .unwrap_or(0)
            > 0
    {
        return Ok(true);
    }
    for record in sorted_restrictions(state) {
        let Restriction::DestroyProtection { object: protected } = &record.restriction else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if resolve_objects(state, protected, &local)?.contains(&object.id) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn apply_zone_move<S: OracleStateAdapter>(
    state: &mut S,
    zone_move: &ZoneMove,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let mut objects = resolve_objects(state, &zone_move.object, context)?;
    if objects.is_empty() {
        return Err(ExecutionError::InvalidAmount(
            "zone move has no physical object",
        ));
    }
    objects.retain(|id| {
        state
            .object(*id)
            .is_some_and(|candidate| zone_move.from.is_none_or(|from| candidate.zone == from))
    });
    if let Some(trigger) = &zone_move.delayed_until {
        let order = state.next_order();
        let effects = objects
            .iter()
            .map(|object| {
                Effect::MoveZone(ZoneMove {
                    object: ObjectRef::ObjectIdentity(*object),
                    delayed_until: None,
                    ..zone_move.clone()
                })
            })
            .collect();
        state.register_delayed_trigger(DelayedTriggerRecord {
            order,
            source_identity: context.source,
            object_identities: objects,
            trigger: trigger.clone(),
            effects,
        });
        state.record_mutation(format!("delay_move:{}:{order}", context.source));
        return Ok(());
    }
    for id in &objects {
        state
            .move_object(*id, zone_move.to)
            .map_err(ExecutionError::Adapter)?;
        let mut candidate = state
            .object(*id)
            .ok_or(ExecutionError::MissingObject(*id))?;
        candidate.tapped = zone_move.tapped;
        candidate.face_down = zone_move.face_down;
        state
            .put_object(candidate)
            .map_err(ExecutionError::Adapter)?;
        if zone_move.to == Zone::Battlefield {
            apply_enters_replacements(state, *id, context)?;
        }
    }
    Ok(())
}

fn resolve_object_selection<S: OracleStateAdapter>(
    state: &S,
    selection: &ObjectSelection,
    context: &ExecutionContext,
) -> Result<Vec<ObjectId>, ExecutionError> {
    let choosers = resolve_players(state, &selection.chooser, context)?;
    if choosers.len() != 1 {
        return Err(ExecutionError::InvalidAmount(
            "object selection requires exactly one chooser",
        ));
    }
    let selected = context
        .object_choices
        .get(&selection.id)
        .cloned()
        .unwrap_or_default();
    let unique = selected.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != selected.len() {
        return Err(ExecutionError::InvalidAmount(
            "object selection contains duplicate objects",
        ));
    }
    let valid_count = match selection.amount {
        TargetAmount::Exactly(amount) => selected.len() == usize::from(amount),
        TargetAmount::UpTo(amount) => selected.len() <= usize::from(amount),
        TargetAmount::All => {
            let matching = matching_objects(state, &selection.filter, context)?
                .into_iter()
                .collect::<BTreeSet<_>>();
            unique == matching
        }
    };
    if !valid_count {
        return Err(ExecutionError::InvalidAmount(
            "object selection has an illegal count",
        ));
    }
    for object in &selected {
        if !object_matches_filter(state, *object, &selection.filter, context)? {
            return Err(ExecutionError::Adapter(format!(
                "selected object {object} does not satisfy its selection filter"
            )));
        }
    }
    Ok(selected)
}

fn apply_selected_zone_move<S: OracleStateAdapter>(
    state: &mut S,
    zone_move: &SelectedZoneMove,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let objects = resolve_object_selection(state, &zone_move.selection, context)?;
    for object in objects {
        state
            .move_object(object, zone_move.to)
            .map_err(ExecutionError::Adapter)?;
        let mut moved = state
            .object(object)
            .ok_or(ExecutionError::MissingObject(object))?;
        moved.tapped = zone_move.tapped;
        moved.face_down = zone_move.face_down;
        state.put_object(moved).map_err(ExecutionError::Adapter)?;
        if zone_move.to == Zone::Battlefield {
            apply_enters_replacements(state, object, context)?;
        }
    }
    Ok(())
}

fn apply_selected_tapped_state<S: OracleStateAdapter>(
    state: &mut S,
    selection: &ObjectSelection,
    tapped: bool,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    for object in resolve_object_selection(state, selection, context)? {
        let mut selected = state
            .object(object)
            .ok_or(ExecutionError::MissingObject(object))?;
        selected.tapped = tapped;
        state
            .put_object(selected)
            .map_err(ExecutionError::Adapter)?;
        state.record_mutation(format!("selected_tapped:{object}:{tapped}"));
    }
    Ok(())
}

fn apply_enters_replacements<S: OracleStateAdapter>(
    state: &mut S,
    entering_object: ObjectId,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let mut replacements = state.replacements();
    replacements.sort_by_key(|record| (record.order, record.source_identity));
    for record in replacements {
        let mut local = context.clone();
        local.source = record.source_identity;
        match &record.effect {
            ReplacementEffect::EntersTapped(replacement)
                if resolve_objects(state, &replacement.object, &local)?
                    .contains(&entering_object) =>
            {
                let exempt = match &replacement.unless {
                    Some(condition) => condition_holds(state, condition, &local)?,
                    None => false,
                };
                if !exempt {
                    let mut candidate = state
                        .object(entering_object)
                        .ok_or(ExecutionError::MissingObject(entering_object))?;
                    candidate.tapped = true;
                    state
                        .put_object(candidate)
                        .map_err(ExecutionError::Adapter)?;
                    state.record_mutation(format!(
                        "replacement:{}:enters_tapped:{entering_object}",
                        record.order
                    ));
                }
            }
            ReplacementEffect::EnterAsCopy(copy)
                if copy.destination == CopyDestination::SourceAsItEnters
                    && record.source_identity == entering_object =>
            {
                copy_onto_existing(state, entering_object, copy, &local)?;
                state.record_mutation(format!(
                    "replacement:{}:enter_copy:{entering_object}",
                    record.order
                ));
            }
            ReplacementEffect::MultiplyEvent { .. }
            | ReplacementEffect::ConditionalTokenSubstitution { .. }
            | ReplacementEffect::EntersTapped(_)
            | ReplacementEffect::EnterAsCopy(_) => {}
        }
    }
    Ok(())
}

fn apply_search<S: OracleStateAdapter>(
    state: &mut S,
    search: &SearchLibrary,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    if search.optional && context.optional_effect_declined {
        return Ok(());
    }
    let amount = evaluate_amount(state, &search.amount, context)? as usize;
    let players = resolve_players(state, &search.player, context)?;
    let _choosers = resolve_players(state, &search.chooser, context)?;
    for player in players {
        let player_state = state
            .player(player)
            .ok_or(ExecutionError::MissingPlayer(player))?;
        let requested = context.searched_cards.values().copied().collect::<Vec<_>>();
        if requested.len() > amount {
            return Err(ExecutionError::Adapter(format!(
                "library search selected {} cards but permits at most {amount}",
                requested.len()
            )));
        }
        if requested.iter().copied().collect::<BTreeSet<_>>().len() != requested.len() {
            return Err(ExecutionError::Adapter(
                "library search selected the same physical card more than once".to_owned(),
            ));
        }
        if let Some(id) = requested
            .iter()
            .find(|id| !player_state.library.contains(id))
        {
            return Err(ExecutionError::Adapter(format!(
                "searched card {id} is not in player {player}'s library"
            )));
        }
        let mut selected = requested;
        if selected.is_empty() && amount > 0 {
            for id in &player_state.library {
                if object_matches_filter(state, *id, &search.predicate, context)? {
                    selected.push(*id);
                    if selected.len() == amount {
                        break;
                    }
                }
            }
        }
        for id in &selected {
            if !object_matches_filter(state, *id, &search.predicate, context)? {
                return Err(ExecutionError::Adapter(format!(
                    "searched card {id} does not satisfy the search predicate"
                )));
            }
        }
        if selected.len() < amount
            && !search.allow_fail_to_find
            && player_state.library.len() >= amount
        {
            return Err(ExecutionError::Adapter(format!(
                "library search found {} of {amount} required cards",
                selected.len()
            )));
        }
        if search.shuffle_before_destination {
            deterministic_shuffle(state, player, context.replay_seed)?;
        }
        for (index, id) in selected.iter().enumerate() {
            let destination = search_destination_for(index, &search.destinations).ok_or(
                ExecutionError::InvalidAmount("search result has no destination"),
            )?;
            state
                .move_object(*id, destination.zone)
                .map_err(ExecutionError::Adapter)?;
            let mut object = state
                .object(*id)
                .ok_or(ExecutionError::MissingObject(*id))?;
            object.tapped = destination.tapped;
            state.put_object(object).map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!(
                "search:{player}:{id}:{:?}:{}",
                destination.zone, search.reveal
            ));
        }
        if search.shuffle_after {
            deterministic_shuffle(state, player, context.replay_seed)?;
        }
    }
    Ok(())
}

fn search_destination_for(
    index: usize,
    destinations: &[SearchDestination],
) -> Option<&SearchDestination> {
    destinations
        .iter()
        .find(|destination| {
            matches!(
                (index, &destination.selected_ordinal),
                (_, SearchOrdinal::Each) | (0, SearchOrdinal::First) | (1.., SearchOrdinal::Other)
            )
        })
        .or_else(|| destinations.get(index))
}

fn deterministic_shuffle<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    seed: u64,
) -> Result<(), ExecutionError> {
    let mut player_state = state
        .player(player)
        .ok_or(ExecutionError::MissingPlayer(player))?;
    let mut keyed = player_state
        .library
        .iter()
        .copied()
        .map(|id| (mix64(seed ^ id ^ u64::from(player)), id))
        .collect::<Vec<_>>();
    keyed.sort_unstable();
    player_state.library = keyed.into_iter().map(|(_, id)| id).collect();
    state
        .put_player(player_state)
        .map_err(ExecutionError::Adapter)?;
    state.record_mutation(format!("shuffle:{player}:{seed}"));
    Ok(())
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn apply_create_token<S: OracleStateAdapter>(
    state: &mut S,
    creation: &TokenCreation,
    context: &ExecutionContext,
) -> Result<Vec<ObjectId>, ExecutionError> {
    let players = resolve_players(state, &creation.player, context)?;
    let base_amount = evaluate_amount(state, &creation.amount, context)?;
    let mut created = Vec::new();
    for player in players {
        let (amount, specification) =
            replace_token_event(state, player, base_amount, &creation.specification, context)?;
        for _ in 0..amount {
            let id = match &specification {
                TokenSpecification::Defined(definition) => {
                    insert_defined_token(state, player, definition, context)?
                }
                TokenSpecification::CopyOf(original) => {
                    let originals = resolve_objects(state, original, context)?;
                    let original = *originals
                        .first()
                        .ok_or(ExecutionError::InvalidAmount("copy token has no original"))?;
                    insert_copy_token(state, player, original)?
                }
                TokenSpecification::ManifestedCard(card) => {
                    let cards = resolve_objects(state, card, context)?;
                    let original = *cards
                        .first()
                        .ok_or(ExecutionError::InvalidAmount("manifest token has no card"))?;
                    let mut token = state
                        .object(original)
                        .ok_or(ExecutionError::MissingObject(original))?;
                    let id = state.allocate_object_id();
                    token.id = id;
                    token.origin_id = id;
                    token.copy_of = Some(original);
                    token.owner = player;
                    token.controller = player;
                    token.zone = Zone::Battlefield;
                    token.token = true;
                    token.face_down = true;
                    token.front.card_types = vec![CardType::Creature];
                    token.front.power = 2;
                    token.front.toughness = 2;
                    token.back = None;
                    state
                        .insert_physical_object(token)
                        .map_err(ExecutionError::Adapter)?;
                    id
                }
            };
            if creation.tapped || creation.attacking {
                let mut token = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                token.tapped = creation.tapped;
                token.attacking = creation.attacking;
                state.put_object(token).map_err(ExecutionError::Adapter)?;
            }
            apply_enters_replacements(state, id, context)?;
            created.push(id);
            state.record_mutation(format!("token:{player}:{id}"));
        }
    }
    Ok(created)
}

fn replace_token_event<S: OracleStateAdapter>(
    state: &S,
    player: PlayerId,
    mut amount: u32,
    specification: &TokenSpecification,
    context: &ExecutionContext,
) -> Result<(u32, TokenSpecification), ExecutionError> {
    let mut specification = specification.clone();
    let mut replacements = state.replacements();
    replacements.sort_by_key(|record| (record.order, record.source_identity));
    for record in replacements {
        let mut local = context.clone();
        local.source = record.source_identity;
        match &record.effect {
            ReplacementEffect::MultiplyEvent { event, multiplier } => {
                let occurrence = ReplacementOccurrence {
                    event: ReplacementEvent::CreateTokens {
                        player: PlayerRef::You,
                    },
                    amount,
                    affected_player: Some(player),
                    object: None,
                };
                if replacement_event_matches(state, event, &occurrence, &local) {
                    amount = amount
                        .checked_mul(u32::from(*multiplier))
                        .ok_or(ExecutionError::ArithmeticOverflow)?;
                }
            }
            ReplacementEffect::ConditionalTokenSubstitution {
                condition,
                ordinary,
                replacement,
            } if condition_holds(state, condition, &local)?
                && ordinary.specification == specification
                && resolve_players(state, &ordinary.player, &local)?.contains(&player) =>
            {
                amount = evaluate_amount(state, &replacement.amount, &local)?;
                specification = replacement.specification.clone();
            }
            ReplacementEffect::EntersTapped(_)
            | ReplacementEffect::EnterAsCopy(_)
            | ReplacementEffect::ConditionalTokenSubstitution { .. } => {}
        }
    }
    Ok((amount, specification))
}

fn insert_defined_token<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    definition: &TokenDefinition,
    context: &ExecutionContext,
) -> Result<ObjectId, ExecutionError> {
    let id = state.allocate_object_id();
    let power = definition
        .power
        .as_ref()
        .map(|amount| evaluate_amount(state, amount, context).map(i64::from))
        .transpose()?
        .unwrap_or(0);
    let toughness = definition
        .toughness
        .as_ref()
        .map(|amount| evaluate_amount(state, amount, context).map(i64::from))
        .transpose()?
        .unwrap_or(0);
    let object = PhysicalObject {
        id,
        origin_id: id,
        copy_of: None,
        owner: player,
        controller: player,
        zone: Zone::Battlefield,
        token: true,
        tapped: false,
        attacking: false,
        prepared: false,
        face_down: false,
        active_face: 0,
        front: ObjectCharacteristics {
            names: definition.name.iter().cloned().collect(),
            card_types: definition.card_types.clone(),
            supertypes: Vec::new(),
            subtypes: definition.subtypes.clone(),
            colors: definition.colors.clone(),
            mana_value: 0,
            power,
            toughness,
            keywords: definition.keywords.clone(),
            abilities: definition.abilities.clone(),
        },
        back: None,
        counters: BTreeMap::new(),
    };
    state
        .insert_physical_object(object)
        .map_err(ExecutionError::Adapter)?;
    Ok(id)
}

fn insert_copy_token<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    original: ObjectId,
) -> Result<ObjectId, ExecutionError> {
    let mut object = state
        .object(original)
        .ok_or(ExecutionError::MissingObject(original))?;
    let id = state.allocate_object_id();
    object.id = id;
    object.origin_id = id;
    object.copy_of = Some(original);
    object.owner = player;
    object.controller = player;
    object.zone = Zone::Battlefield;
    object.token = true;
    object.tapped = false;
    object.attacking = false;
    object.counters.clear();
    state
        .insert_physical_object(object)
        .map_err(ExecutionError::Adapter)?;
    Ok(id)
}

fn draw_cards<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    amount: u32,
) -> Result<(), ExecutionError> {
    for _ in 0..amount {
        let card = state
            .player(player)
            .ok_or(ExecutionError::MissingPlayer(player))?
            .library
            .first()
            .copied()
            .ok_or(ExecutionError::Adapter(format!(
                "player {player} cannot draw from an empty library"
            )))?;
        state
            .move_object(card, Zone::Hand)
            .map_err(ExecutionError::Adapter)?;
        state.record_mutation(format!("draw:{player}:{card}"));
    }
    Ok(())
}

fn change_life<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    change: i64,
) -> Result<(), ExecutionError> {
    let mut player_state = state
        .player(player)
        .ok_or(ExecutionError::MissingPlayer(player))?;
    player_state.life = player_state
        .life
        .checked_add(change)
        .ok_or(ExecutionError::ArithmeticOverflow)?;
    state
        .put_player(player_state)
        .map_err(ExecutionError::Adapter)?;
    state.record_mutation(format!("life:{player}:{change}"));
    Ok(())
}

fn register_continuous_effect<S: OracleStateAdapter>(
    state: &mut S,
    context: &ExecutionContext,
    object_identities: Vec<ObjectId>,
    effect: Effect,
    duration: Duration,
) {
    let order = state.next_order();
    state.register_continuous(ContinuousEffectRecord {
        order,
        source_identity: context.source,
        object_identities,
        effect,
        duration,
    });
}

fn register_continuous_if_needed<S: OracleStateAdapter>(
    state: &mut S,
    context: &ExecutionContext,
    object_identities: Vec<ObjectId>,
    effect: Effect,
    duration: Duration,
) {
    if duration != Duration::Permanent {
        register_continuous_effect(state, context, object_identities, effect, duration);
    }
}

fn set_tapped<S: OracleStateAdapter>(
    state: &mut S,
    reference: &ObjectRef,
    context: &ExecutionContext,
    tapped: bool,
) -> Result<(), ExecutionError> {
    let objects = resolve_objects(state, reference, context)?;
    if objects.is_empty() {
        return Err(ExecutionError::InvalidAmount(
            "tap effect has no physical object",
        ));
    }
    for id in objects {
        let mut object = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
        if object.zone != Zone::Battlefield {
            return Err(ExecutionError::Adapter(format!(
                "object {id} is not on the battlefield"
            )));
        }
        object.tapped = tapped;
        state.put_object(object).map_err(ExecutionError::Adapter)?;
        state.record_mutation(format!("set_tapped:{id}:{tapped}"));
    }
    Ok(())
}

fn scry<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    amount: u32,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let player_state = state
        .player(player)
        .ok_or(ExecutionError::MissingPlayer(player))?;
    let count = usize::try_from(amount)
        .unwrap_or(usize::MAX)
        .min(player_state.library.len());
    let examined = player_state.library[..count].to_vec();
    let (keep_on_top, move_away) = ordered_library_partition(context, player, &examined)?;
    let mut library = keep_on_top;
    library.extend_from_slice(&player_state.library[count..]);
    library.extend(move_away);
    let mut player_state = player_state;
    player_state.library = library;
    state
        .put_player(player_state)
        .map_err(ExecutionError::Adapter)?;
    state.record_mutation(format!("scry:{player}:{count}"));
    Ok(())
}

fn surveil<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    amount: u32,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let library = state
        .player(player)
        .ok_or(ExecutionError::MissingPlayer(player))?
        .library;
    let count = usize::try_from(amount)
        .unwrap_or(usize::MAX)
        .min(library.len());
    let examined = library[..count].to_vec();
    let (keep_on_top, move_away) = ordered_library_partition(context, player, &examined)?;
    for card in &move_away {
        state
            .move_object(*card, Zone::Graveyard)
            .map_err(ExecutionError::Adapter)?;
    }
    let mut player_state = state
        .player(player)
        .ok_or(ExecutionError::MissingPlayer(player))?;
    let mut ordered_library = keep_on_top;
    ordered_library.extend(library[count..].iter().copied());
    player_state.library = ordered_library;
    state
        .put_player(player_state)
        .map_err(ExecutionError::Adapter)?;
    state.record_mutation(format!("surveil:{player}:{}", move_away.len()));
    Ok(())
}

fn mill<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    amount: u32,
) -> Result<(), ExecutionError> {
    let cards = state
        .player(player)
        .ok_or(ExecutionError::MissingPlayer(player))?
        .library
        .into_iter()
        .take(usize::try_from(amount).unwrap_or(usize::MAX))
        .collect::<Vec<_>>();
    for card in &cards {
        state
            .move_object(*card, Zone::Graveyard)
            .map_err(ExecutionError::Adapter)?;
    }
    state.record_mutation(format!("mill:{player}:{}", cards.len()));
    Ok(())
}

fn ordered_library_partition(
    context: &ExecutionContext,
    player: PlayerId,
    examined: &[ObjectId],
) -> Result<(Vec<ObjectId>, Vec<ObjectId>), ExecutionError> {
    let Some(choice) = context.library_choices.get(&player) else {
        return Ok((examined.to_vec(), Vec::new()));
    };
    let combined = choice
        .keep_on_top
        .iter()
        .chain(&choice.move_away)
        .copied()
        .collect::<Vec<_>>();
    let unique = combined.iter().copied().collect::<BTreeSet<_>>();
    let expected = examined.iter().copied().collect::<BTreeSet<_>>();
    if combined.len() != examined.len() || unique.len() != combined.len() || unique != expected {
        return Err(ExecutionError::Adapter(
            "library choice must partition exactly the examined cards".to_owned(),
        ));
    }
    Ok((choice.keep_on_top.clone(), choice.move_away.clone()))
}

fn resolve_ward<S: OracleStateAdapter>(
    state: &mut S,
    payer: &PlayerRef,
    source: &ObjectRef,
    cost: &WardCost,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let payers = resolve_players(state, payer, context)?;
    let [payer] = payers.as_slice() else {
        return Err(ExecutionError::InvalidAmount(
            "ward requires exactly one targeting player",
        ));
    };
    let typed_cost = match cost {
        WardCost::Mana(cost) => Cost::Mana(cost.clone()),
        WardCost::PayLife(amount) => Cost::PayLife(amount.clone()),
    };
    if context.payment_declined || !cost_is_payable_by(state, &typed_cost, *payer, context)? {
        return apply_effect(
            state,
            &Effect::Counter {
                object: source.clone(),
            },
            context,
        );
    }
    let mut payment_context = context.clone();
    payment_context.actor = *payer;
    pay_cost(state, &typed_cost, &payment_context)
}

fn apply_put_counter<S: OracleStateAdapter>(
    state: &mut S,
    reference: &ObjectRef,
    counter: &CounterKind,
    amount: &Amount,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let base = evaluate_amount(state, amount, context)?;
    let objects = resolve_objects(state, reference, context)?;
    if objects.is_empty() {
        return Err(ExecutionError::InvalidAmount(
            "counter effect has no object",
        ));
    }
    for id in objects {
        let resolved = replace_counter_event(state, id, counter, base, context)?;
        let mut object = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
        let key = counter_key(counter);
        let current = object.counters.get(&key).copied().unwrap_or(0);
        object.counters.insert(
            key.clone(),
            current
                .checked_add(resolved)
                .ok_or(ExecutionError::ArithmeticOverflow)?,
        );
        if *counter == CounterKind::PlusOnePlusOne {
            object.characteristics_mut().power = object
                .characteristics()
                .power
                .checked_add(i64::from(resolved))
                .ok_or(ExecutionError::ArithmeticOverflow)?;
            object.characteristics_mut().toughness = object
                .characteristics()
                .toughness
                .checked_add(i64::from(resolved))
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        }
        state.put_object(object).map_err(ExecutionError::Adapter)?;
        state.record_mutation(format!("counter:{id}:{key}:{resolved}"));
    }
    Ok(())
}

fn replace_counter_event<S: OracleStateAdapter>(
    state: &S,
    object: ObjectId,
    counter: &CounterKind,
    mut amount: u32,
    context: &ExecutionContext,
) -> Result<u32, ExecutionError> {
    let mut replacements = state.replacements();
    replacements.sort_by_key(|record| (record.order, record.source_identity));
    for record in replacements {
        let ReplacementEffect::MultiplyEvent { event, multiplier } = &record.effect else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        let occurrence = ReplacementOccurrence {
            event: ReplacementEvent::PutCounters {
                counter: counter.clone(),
                object: Box::new(ObjectFilter::default()),
            },
            amount,
            affected_player: state.object(object).map(|candidate| candidate.controller),
            object: Some(object),
        };
        if replacement_event_matches(state, event, &occurrence, &local) {
            amount = amount
                .checked_mul(u32::from(*multiplier))
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        }
    }
    Ok(amount)
}

fn counter_key(counter: &CounterKind) -> String {
    match counter {
        CounterKind::PlusOnePlusOne => "+1/+1".to_owned(),
        CounterKind::Loyalty => "loyalty".to_owned(),
        CounterKind::Indestructible => "indestructible".to_owned(),
        CounterKind::Named(name) => name.to_ascii_lowercase(),
    }
}

fn apply_power_toughness_change<S: OracleStateAdapter>(
    state: &mut S,
    change: &PowerToughnessChange,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let power = i64::from(evaluate_amount(state, &change.power, context)?);
    let toughness = i64::from(evaluate_amount(state, &change.toughness, context)?);
    let objects = resolve_objects(state, &change.objects, context)?;
    if matches!(change.objects, ObjectRef::AttachmentTarget { .. }) {
        register_continuous_if_needed(
            state,
            context,
            objects,
            Effect::ModifyPowerToughness(change.clone()),
            change.duration.clone(),
        );
        return Ok(());
    }
    for id in &objects {
        let mut object = state
            .object(*id)
            .ok_or(ExecutionError::MissingObject(*id))?;
        apply_power_toughness_operation(
            object.characteristics_mut(),
            change.operation.clone(),
            power,
            toughness,
        )?;
        state.put_object(object).map_err(ExecutionError::Adapter)?;
    }
    register_continuous_if_needed(
        state,
        context,
        objects,
        Effect::ModifyPowerToughness(change.clone()),
        change.duration.clone(),
    );
    Ok(())
}

fn apply_power_toughness_operation(
    characteristics: &mut ObjectCharacteristics,
    operation: PowerToughnessOperation,
    power: i64,
    toughness: i64,
) -> Result<(), ExecutionError> {
    match operation {
        PowerToughnessOperation::Add => {
            characteristics.power = characteristics
                .power
                .checked_add(power)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
            characteristics.toughness = characteristics
                .toughness
                .checked_add(toughness)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        }
        PowerToughnessOperation::Subtract => {
            characteristics.power = characteristics
                .power
                .checked_sub(power)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
            characteristics.toughness = characteristics
                .toughness
                .checked_sub(toughness)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        }
        PowerToughnessOperation::AddPowerSubtractToughness => {
            characteristics.power = characteristics
                .power
                .checked_add(power)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
            characteristics.toughness = characteristics
                .toughness
                .checked_sub(toughness)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        }
        PowerToughnessOperation::SubtractPowerAddToughness => {
            characteristics.power = characteristics
                .power
                .checked_sub(power)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
            characteristics.toughness = characteristics
                .toughness
                .checked_add(toughness)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        }
        PowerToughnessOperation::SetBase => {
            characteristics.power = power;
            characteristics.toughness = toughness;
        }
        PowerToughnessOperation::Double => {
            characteristics.power = characteristics
                .power
                .checked_mul(2)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
            characteristics.toughness = characteristics
                .toughness
                .checked_mul(2)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        }
    }
    Ok(())
}

fn apply_set_characteristics<S: OracleStateAdapter>(
    state: &mut S,
    change: &SetCharacteristics,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let objects = resolve_objects(state, &change.object, context)?;
    if matches!(change.object, ObjectRef::AttachmentTarget { .. }) {
        register_continuous_if_needed(
            state,
            context,
            objects,
            Effect::SetCharacteristics(change.clone()),
            change.duration.clone(),
        );
        return Ok(());
    }
    for id in &objects {
        let mut object = state
            .object(*id)
            .ok_or(ExecutionError::MissingObject(*id))?;
        let characteristics = object.characteristics_mut();
        if let Some(colors) = &change.colors {
            merge_or_replace(
                &mut characteristics.colors,
                colors,
                change.retain_other_colors,
            );
        }
        if let Some(card_types) = &change.card_types {
            merge_or_replace(
                &mut characteristics.card_types,
                card_types,
                change.retain_other_card_types,
            );
        }
        if let Some(subtypes) = &change.subtypes {
            merge_or_replace(
                &mut characteristics.subtypes,
                subtypes,
                change.retain_other_subtypes,
            );
        }
        if let Some(name) = &change.name {
            if !change.retain_other_names {
                characteristics.names.clear();
            }
            if !characteristics.names.contains(name) {
                characteristics.names.push(name.clone());
            }
        }
        if let Some(power) = &change.base_power {
            characteristics.power = i64::from(evaluate_amount(state, power, context)?);
        }
        if let Some(toughness) = &change.base_toughness {
            characteristics.toughness = i64::from(evaluate_amount(state, toughness, context)?);
        }
        state.put_object(object).map_err(ExecutionError::Adapter)?;
    }
    register_continuous_if_needed(
        state,
        context,
        objects,
        Effect::SetCharacteristics(change.clone()),
        change.duration.clone(),
    );
    Ok(())
}

fn merge_or_replace<T: Clone + PartialEq>(current: &mut Vec<T>, next: &[T], retain: bool) {
    if !retain {
        current.clear();
    }
    for value in next {
        if !current.contains(value) {
            current.push(value.clone());
        }
    }
}

fn apply_copy<S: OracleStateAdapter>(
    state: &mut S,
    copy: &CopyEffect,
    context: &ExecutionContext,
) -> Result<Vec<ObjectId>, ExecutionError> {
    if copy.optional && context.optional_effect_declined {
        return Ok(Vec::new());
    }
    let originals = resolve_objects(state, &copy.original, context)?
        .into_iter()
        .filter(|id| object_matches_filter(state, *id, &copy.filter, context).unwrap_or(false))
        .collect::<Vec<_>>();
    let original = *originals
        .first()
        .ok_or(ExecutionError::InvalidAmount("copy has no legal original"))?;
    match &copy.destination {
        CopyDestination::SourceAsItEnters => {
            copy_onto_existing(state, context.source, copy, context)?;
            Ok(vec![context.source])
        }
        CopyDestination::TokenControlledBy(player) => {
            let mut created = Vec::new();
            for player in resolve_players(state, player, context)? {
                let id = insert_copy_token(state, player, original)?;
                apply_copy_exceptions(state, id, &copy.exceptions, context)?;
                created.push(id);
            }
            Ok(created)
        }
    }
}

fn copy_onto_existing<S: OracleStateAdapter>(
    state: &mut S,
    destination: ObjectId,
    copy: &CopyEffect,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    if copy.optional && context.optional_effect_declined {
        return Ok(());
    }
    let originals = resolve_objects(state, &copy.original, context)?
        .into_iter()
        .filter(|id| object_matches_filter(state, *id, &copy.filter, context).unwrap_or(false))
        .collect::<Vec<_>>();
    let original = *originals
        .first()
        .ok_or(ExecutionError::InvalidAmount("copy has no legal original"))?;
    let source = state
        .object(original)
        .ok_or(ExecutionError::MissingObject(original))?;
    let mut target = state
        .object(destination)
        .ok_or(ExecutionError::MissingObject(destination))?;
    let retained_abilities = target.characteristics().abilities.clone();
    target.front = source.front;
    target.back = source.back;
    target.active_face = source.active_face;
    target.copy_of = Some(original);
    target.counters.clear();
    if copy
        .exceptions
        .contains(&CopyException::RetainSourceAbilities)
    {
        target
            .characteristics_mut()
            .abilities
            .extend(retained_abilities);
    }
    state.put_object(target).map_err(ExecutionError::Adapter)?;
    apply_copy_exceptions(state, destination, &copy.exceptions, context)
}

fn apply_copy_exceptions<S: OracleStateAdapter>(
    state: &mut S,
    object_id: ObjectId,
    exceptions: &[CopyException],
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    for exception in exceptions {
        let mut object = state
            .object(object_id)
            .ok_or(ExecutionError::MissingObject(object_id))?;
        match exception {
            CopyException::RetainSourceAbilities => {}
            CopyException::SetName(name) => {
                object.characteristics_mut().names = vec![name.clone()];
            }
            CopyException::AddLegendary => {
                if !object
                    .characteristics()
                    .supertypes
                    .contains(&Supertype::Legendary)
                {
                    object
                        .characteristics_mut()
                        .supertypes
                        .push(Supertype::Legendary);
                }
            }
            CopyException::RemoveLegendary => object
                .characteristics_mut()
                .supertypes
                .retain(|kind| *kind != Supertype::Legendary),
            CopyException::AddCardType(card_type) => {
                if !object.characteristics().card_types.contains(card_type) {
                    object.characteristics_mut().card_types.push(*card_type);
                }
            }
            CopyException::AddSubtype(subtype) => {
                if !object
                    .characteristics()
                    .subtypes
                    .iter()
                    .any(|actual| actual.eq_ignore_ascii_case(subtype))
                {
                    object.characteristics_mut().subtypes.push(subtype.clone());
                }
            }
            CopyException::AddKeyword(keyword) => {
                if !object.characteristics().keywords.contains(keyword) {
                    object.characteristics_mut().keywords.push(keyword.clone());
                }
            }
            CopyException::AddCounterIfType {
                card_type,
                counter,
                amount,
            } => {
                if object_has_type(&object, *card_type) {
                    state.put_object(object).map_err(ExecutionError::Adapter)?;
                    apply_put_counter(
                        state,
                        &ObjectRef::ThatObject(255),
                        counter,
                        amount,
                        &ExecutionContext {
                            that_objects: BTreeMap::from([(255, object_id)]),
                            ..context.clone()
                        },
                    )?;
                    continue;
                }
            }
            CopyException::AddGrantedAbility(ability) => {
                object.characteristics_mut().abilities.push(ability.clone());
            }
        }
        state.put_object(object).map_err(ExecutionError::Adapter)?;
    }
    Ok(())
}

fn apply_animation<S: OracleStateAdapter>(
    state: &mut S,
    animation: &AnimateEffect,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let power = (!animation.retain_printed_power_toughness)
        .then(|| evaluate_amount(state, &animation.power, context))
        .transpose()?
        .map(i64::from);
    let toughness = (!animation.retain_printed_power_toughness)
        .then(|| evaluate_amount(state, &animation.toughness, context))
        .transpose()?
        .map(i64::from);
    let objects = resolve_objects(state, &animation.object, context)?;
    for id in &objects {
        let mut object = state
            .object(*id)
            .ok_or(ExecutionError::MissingObject(*id))?;
        let characteristics = object.characteristics_mut();
        if !animation.retain_land {
            characteristics
                .card_types
                .retain(|kind| *kind != CardType::Land);
        }
        if !characteristics.card_types.contains(&CardType::Creature) {
            characteristics.card_types.push(CardType::Creature);
        }
        if let Some(power) = power {
            characteristics.power = power;
        }
        if let Some(toughness) = toughness {
            characteristics.toughness = toughness;
        }
        merge_or_replace(&mut characteristics.colors, &animation.colors, true);
        merge_or_replace(&mut characteristics.subtypes, &animation.subtypes, true);
        merge_or_replace(&mut characteristics.keywords, &animation.keywords, true);
        state.put_object(object).map_err(ExecutionError::Adapter)?;
    }
    register_continuous_if_needed(
        state,
        context,
        objects,
        Effect::Animate(animation.clone()),
        animation.duration.clone(),
    );
    Ok(())
}

fn set_looked_at<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    cards: Vec<ObjectId>,
) -> Result<(), ExecutionError> {
    if cards.iter().any(|id| {
        !state
            .object(*id)
            .is_some_and(|object| object.zone == Zone::Library && object.owner == player)
    }) {
        return Err(ExecutionError::Adapter(
            "looked at cards are not in the player's library".to_owned(),
        ));
    }
    state.put_looked_at(player, cards);
    Ok(())
}

fn select_from_looked_at<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    amount: usize,
    predicate: &ObjectFilter,
    reveal: bool,
    destination: Zone,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let looked = state.looked_at(player);
    let selected = looked
        .iter()
        .copied()
        .filter(|id| object_matches_filter(state, *id, predicate, context).unwrap_or(false))
        .take(amount)
        .collect::<Vec<_>>();
    if selected.len() < amount {
        return Err(ExecutionError::Adapter(format!(
            "looked at selection found {} of {amount} cards",
            selected.len()
        )));
    }
    for id in &selected {
        state
            .move_object(*id, destination)
            .map_err(ExecutionError::Adapter)?;
        state.record_mutation(format!(
            "select_looked:{player}:{id}:{destination:?}:{reveal}"
        ));
    }
    state.put_looked_at(
        player,
        looked
            .into_iter()
            .filter(|id| !selected.contains(id))
            .collect(),
    );
    Ok(())
}

fn put_rest_on_bottom<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    order: BottomOrder,
) -> Result<(), ExecutionError> {
    let mut rest = state.looked_at(player);
    match order {
        BottomOrder::AnyOrder => rest.sort_unstable(),
    }
    let mut player_state = state
        .player(player)
        .ok_or(ExecutionError::MissingPlayer(player))?;
    player_state.library.retain(|id| !rest.contains(id));
    player_state.library.extend(rest.iter().copied());
    state
        .put_player(player_state)
        .map_err(ExecutionError::Adapter)?;
    state.put_looked_at(player, Vec::new());
    state.record_mutation(format!("bottom:{player}:{}", rest.len()));
    Ok(())
}

fn apply_cast_copy<S: OracleStateAdapter>(
    state: &mut S,
    copy: &CastCopyEffect,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let sources = resolve_objects(state, &copy.source, context)?;
    let source = *sources
        .first()
        .ok_or(ExecutionError::InvalidAmount("cast copy has no source"))?;
    let original = state
        .object(source)
        .ok_or(ExecutionError::MissingObject(source))?;
    if original.zone != copy.from {
        return Err(ExecutionError::Adapter(format!(
            "copy source {source} is not in {:?}",
            copy.from
        )));
    }
    let mut stack_copy = original;
    let id = state.allocate_object_id();
    stack_copy.id = id;
    stack_copy.origin_id = id;
    stack_copy.copy_of = Some(source);
    stack_copy.zone = Zone::Stack;
    stack_copy.token = false;
    stack_copy.counters.clear();
    state
        .insert_physical_object(stack_copy)
        .map_err(ExecutionError::Adapter)?;
    state.record_mutation(format!(
        "cast_copy:{source}:{id}:{}",
        copy.without_paying_mana_cost
    ));
    if copy.repeat != RepeatSchedule::Once {
        let order = state.next_order();
        state.register_scheduled_copy(ScheduledCopyRecord {
            order,
            source_identity: context.source,
            copied_object_identity: source,
            timing: copy.timing.clone(),
            repeat: copy.repeat.clone(),
        });
    }
    Ok(())
}
