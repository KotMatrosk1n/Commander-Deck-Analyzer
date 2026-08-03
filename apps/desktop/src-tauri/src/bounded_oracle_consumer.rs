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
    ActivationRestriction, AlternativeCost, Amount, AnimateEffect, AtomicResourceCost,
    AttachmentKind, BOUNDED_ORACLE_RUNTIME_VERSION, BottomOrder, BounceWithControllerCopyEffect,
    BoundedOracleClause, CardType, CastCopyEffect, CastPermission, CastTiming, ChoiceCount,
    ClauseAddress, CoinFlipResult, Color, CombatStep, Comparison, Condition, CopyDestination,
    CopyEffect, CopyException, Cost, CountExpression, CounterKind, DayNightDesignation, Duration,
    Effect, ExileCollectionEffect, ExtraTurnEffect, GrantedAbility, Keyword, LibraryProcedure,
    LoyaltyCost, ManaChoice, ManaCost, ManaProduction, ObjectEventKind, ObjectFilter, ObjectRef,
    ObjectSelection, ObjectState, PaymentOrLoseEffect, PlayerActionKind, PlayerRef,
    PowerToughnessChange, PowerToughnessOperation, ReminderSemantics, RepeatSchedule,
    ReplacementEffect, ReplacementEvent, Restriction, RulesTextChoiceKind, SearchDestination,
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

pub const BOUNDED_ORACLE_CONSUMER_VERSION: &str = "bounded-oracle-consumer-0.16";

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
    ObjectBlocked {
        blocker: ObjectId,
        blocked: ObjectId,
    },
    SchemeSetInMotion {
        object: ObjectId,
    },
    ObjectTappedForMana {
        object: ObjectId,
    },
    AttachmentTargetEvent {
        attachment_source: ObjectId,
        object: ObjectId,
        kind: AttachmentKind,
        event: ObjectEventKind,
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
    DamageToPlayer {
        source: ObjectId,
        player: PlayerId,
        amount: u32,
        combat: bool,
    },
    CombatDamageToObject {
        source: ObjectId,
        object: ObjectId,
        amount: u32,
    },
    DamageToObject {
        source: ObjectId,
        object: ObjectId,
        amount: u32,
        combat: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryEndChoice {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryEndSelection {
    pub chooser: PlayerId,
    pub choice: LibraryEndChoice,
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
    pub defending_player: Option<PlayerId>,
    pub current_step: Option<Step>,
    pub combat_step: Option<CombatStep>,
    pub attackers_declared: bool,
    pub blockers_declared: bool,
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
    pub chosen_basic_land_type: Option<String>,
    pub chosen_color: Option<Color>,
    pub selected_modes: Vec<u16>,
    pub previously_selected_modes: BTreeSet<u16>,
    pub selected_mode_chooser: Option<PlayerId>,
    pub object_choices: BTreeMap<u8, Vec<ObjectId>>,
    pub player_choices: BTreeMap<u8, Vec<PlayerId>>,
    pub per_player_object_choices: BTreeMap<PlayerId, Vec<ObjectId>>,
    pub library_end_choices: BTreeMap<u8, LibraryEndSelection>,
    pub library_choices: BTreeMap<PlayerId, OrderedLibraryChoice>,
    pub card_was_cast_with_alternative_cost: bool,
    pub card_was_cast_using_escape: bool,
    pub card_was_kicked: bool,
    pub card_was_cast_using_teamwork: bool,
    pub you_attacked_this_turn: bool,
    pub opponent_lost_life_this_turn: bool,
    pub first_resolution_of_named_spell: bool,
    pub payment_declined: bool,
    pub optional_effect_declined: bool,
    pub ability_occurrence_this_turn: u32,
    pub gift_promised: bool,
    pub source_was_in_opening_hand: bool,
    pub playing_first: bool,
    pub spells_cast_by_actor_this_turn: u32,
    pub selected_card_name: Option<String>,
    pub selected_card_name_is_nonland: Option<bool>,
    pub selected_rules_text: Option<String>,
    pub cast_from_zone: Option<Zone>,
    pub source_attacking_alone: bool,
    pub creatures_attacked_this_turn: u32,
    pub attack_tax_generic_paid: BTreeMap<ObjectId, u32>,
    pub mana_spent_to_cast_triggering_spell: u32,
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
            defending_player: None,
            current_step: None,
            combat_step: None,
            attackers_declared: false,
            blockers_declared: false,
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
            chosen_basic_land_type: None,
            chosen_color: None,
            selected_modes: Vec::new(),
            previously_selected_modes: BTreeSet::new(),
            selected_mode_chooser: None,
            object_choices: BTreeMap::new(),
            player_choices: BTreeMap::new(),
            per_player_object_choices: BTreeMap::new(),
            library_end_choices: BTreeMap::new(),
            library_choices: BTreeMap::new(),
            card_was_cast_with_alternative_cost: false,
            card_was_cast_using_escape: false,
            card_was_kicked: false,
            card_was_cast_using_teamwork: false,
            you_attacked_this_turn: false,
            opponent_lost_life_this_turn: false,
            first_resolution_of_named_spell: false,
            payment_declined: true,
            optional_effect_declined: false,
            ability_occurrence_this_turn: 1,
            gift_promised: false,
            source_was_in_opening_hand: false,
            playing_first: true,
            spells_cast_by_actor_this_turn: 0,
            selected_card_name: None,
            selected_card_name_is_nonland: None,
            selected_rules_text: None,
            cast_from_zone: None,
            source_attacking_alone: false,
            creatures_attacked_this_turn: 0,
            attack_tax_generic_paid: BTreeMap::new(),
            mana_spent_to_cast_triggering_spell: 0,
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
    pub blocking: bool,
    pub prepared: bool,
    pub face_down: bool,
    pub active_face: u8,
    /// Current level of a Class permanent. Non-Class objects use zero.
    pub class_level: u8,
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
    pub counters: BTreeMap<String, u32>,
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
    pub condition: Option<Condition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellIncreaseRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub object: ObjectRef,
    pub mana: ManaCost,
    pub per: CountExpression,
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
pub struct RevealHandRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub player: PlayerId,
    pub cards: Vec<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandInspectionRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub viewer: PlayerId,
    pub player: PlayerId,
    pub cards: Vec<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealedCardRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub player: PlayerId,
    pub card: ObjectId,
    pub as_additional_cost: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedStepRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub player: PlayerId,
    pub step: Step,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextUntapPreventionRecord {
    pub order: u64,
    pub source_identity: ObjectId,
    pub object_identities: Vec<ObjectId>,
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
    fn spell_increases(&self) -> Vec<SpellIncreaseRecord>;
    fn register_spell_increase(&mut self, record: SpellIncreaseRecord);
    fn register_scheduled_copy(&mut self, record: ScheduledCopyRecord);
    fn register_cast_permission(&mut self, record: CastPermissionRecord);
    fn register_extra_turn(&mut self, record: ExtraTurnRecord);
    fn register_payment_or_lose(&mut self, record: PaymentOrLoseRecord);
    fn register_game_result(&mut self, record: GameResultRecord);
    fn game_results(&self) -> Vec<GameResultRecord>;
    fn register_revealed_hand(&mut self, record: RevealHandRecord);
    fn register_hand_inspection(&mut self, record: HandInspectionRecord);
    fn register_revealed_card(&mut self, record: RevealedCardRecord);
    fn register_skipped_step(&mut self, record: SkippedStepRecord);
    fn next_untap_preventions(&self) -> Vec<NextUntapPreventionRecord>;
    fn register_next_untap_prevention(&mut self, record: NextUntapPreventionRecord);
    fn consume_next_untap_prevention(&mut self, order: u64);
    fn looked_at(&self, player: PlayerId) -> Vec<ObjectId>;
    fn put_looked_at(&mut self, player: PlayerId, objects: Vec<ObjectId>);
    fn loyalty_ability_activated_this_turn(&self, source: ObjectId) -> bool;
    fn record_loyalty_ability_activation(&mut self, source: ObjectId) -> Result<(), String>;
    fn chosen_card_name(&self, source: ObjectId) -> Option<String>;
    fn set_chosen_card_name(&mut self, source: ObjectId, name: String) -> Result<(), String>;
    fn chosen_color(&self, source: ObjectId) -> Option<Color>;
    fn set_chosen_color(&mut self, source: ObjectId, color: Color) -> Result<(), String>;
    fn die_roll(&self, source: ObjectId) -> Option<(u16, u16)>;
    fn set_die_roll(&mut self, source: ObjectId, sides: u16, result: u16) -> Result<(), String>;
    fn coin_flip(&self, source: ObjectId) -> Option<CoinFlipResult>;
    fn set_coin_flip(&mut self, source: ObjectId, result: CoinFlipResult) -> Result<(), String>;
    fn intensity(&self, source: ObjectId) -> Option<u16>;
    fn initialize_intensity(&mut self, source: ObjectId, amount: u16) -> Result<bool, String>;
    fn chosen_option(&self, source: ObjectId) -> Option<String>;
    fn set_chosen_option(&mut self, source: ObjectId, option: String) -> Result<(), String>;
    fn chosen_rules_text(&self, source: ObjectId) -> Option<(RulesTextChoiceKind, String)>;
    fn set_chosen_rules_text(
        &mut self,
        source: ObjectId,
        kind: RulesTextChoiceKind,
        value: String,
    ) -> Result<(), String>;
    fn day_night_designation(&self) -> Option<DayNightDesignation>;
    fn establish_day_if_unset(&mut self) -> bool;
    fn exhaust_ability_was_activated(&self, source: ObjectId, ability_key: &str) -> bool;
    fn mark_exhaust_ability_activated(
        &mut self,
        source: ObjectId,
        ability_key: String,
    ) -> Result<(), String>;
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
    pub spell_increases: Vec<SpellIncreaseRecord>,
    pub scheduled_copies: Vec<ScheduledCopyRecord>,
    pub cast_permissions: Vec<CastPermissionRecord>,
    pub extra_turns: Vec<ExtraTurnRecord>,
    pub payment_or_lose: Vec<PaymentOrLoseRecord>,
    pub game_results: Vec<GameResultRecord>,
    pub revealed_hands: Vec<RevealHandRecord>,
    pub hand_inspections: Vec<HandInspectionRecord>,
    pub revealed_cards: Vec<RevealedCardRecord>,
    pub skipped_steps: Vec<SkippedStepRecord>,
    pub next_untap_preventions: Vec<NextUntapPreventionRecord>,
    pub looked_at: BTreeMap<PlayerId, Vec<ObjectId>>,
    pub loyalty_activations_this_turn: BTreeSet<ObjectId>,
    pub chosen_card_names: BTreeMap<ObjectId, String>,
    pub chosen_colors: BTreeMap<ObjectId, Color>,
    pub die_rolls: BTreeMap<ObjectId, (u16, u16)>,
    pub coin_flips: BTreeMap<ObjectId, CoinFlipResult>,
    pub intensities: BTreeMap<ObjectId, u16>,
    pub chosen_options: BTreeMap<ObjectId, String>,
    pub chosen_rules_text: BTreeMap<ObjectId, (RulesTextChoiceKind, String)>,
    pub day_night_designation: Option<DayNightDesignation>,
    pub exhaust_activations: BTreeSet<(ObjectId, String)>,
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
            spell_increases: Vec::new(),
            scheduled_copies: Vec::new(),
            cast_permissions: Vec::new(),
            extra_turns: Vec::new(),
            payment_or_lose: Vec::new(),
            game_results: Vec::new(),
            revealed_hands: Vec::new(),
            hand_inspections: Vec::new(),
            revealed_cards: Vec::new(),
            skipped_steps: Vec::new(),
            next_untap_preventions: Vec::new(),
            looked_at: BTreeMap::new(),
            loyalty_activations_this_turn: BTreeSet::new(),
            chosen_card_names: BTreeMap::new(),
            chosen_colors: BTreeMap::new(),
            die_rolls: BTreeMap::new(),
            coin_flips: BTreeMap::new(),
            intensities: BTreeMap::new(),
            chosen_options: BTreeMap::new(),
            chosen_rules_text: BTreeMap::new(),
            day_night_designation: None,
            exhaust_activations: BTreeSet::new(),
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
        self.continuous_effects.retain(|record| {
            !matches!(
                record.duration,
                Duration::ThisTurn | Duration::UntilEndOfTurn
            )
        });
        self.restriction_effects.retain(|record| {
            !matches!(
                record.restriction,
                Restriction::AdditionalLandPlays {
                    duration: Duration::ThisTurn | Duration::UntilEndOfTurn,
                    ..
                }
            )
        });
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
        let previous_zone = object.zone;
        if object.zone == Zone::Library {
            let owner = self
                .players
                .get_mut(&object.owner)
                .ok_or_else(|| format!("missing owner {}", object.owner))?;
            owner.library.retain(|candidate| *candidate != id);
        }
        object.zone = zone;
        let is_class = object
            .characteristics()
            .subtypes
            .iter()
            .any(|subtype| subtype.eq_ignore_ascii_case("Class"));
        if is_class && previous_zone != Zone::Battlefield && zone == Zone::Battlefield {
            object.class_level = 1;
        } else if is_class && previous_zone == Zone::Battlefield && zone != Zone::Battlefield {
            object.class_level = 0;
        }
        if previous_zone == Zone::Battlefield && zone != Zone::Battlefield {
            self.exhaust_activations.retain(|(source, _)| *source != id);
            object.counters.clear();
            object.prepared = false;
            object.active_face = 0;
            object.tapped = false;
            object.face_down = false;
        }
        if zone != Zone::Battlefield {
            object.attacking = false;
            object.blocking = false;
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
        let target_types = &target.characteristics().card_types;
        let target_is_legal = match attachment.kind {
            // The specific Enchant filter is validated by the attachment
            // production runtime. This adapter consumes an already-legal
            // battlefield attachment, so it preserves the full permanent
            // domain instead of incorrectly narrowing every Aura to creatures.
            AttachmentKind::Aura => target_types.iter().any(|card_type| {
                matches!(
                    card_type,
                    CardType::Artifact
                        | CardType::Battle
                        | CardType::Creature
                        | CardType::Enchantment
                        | CardType::Land
                        | CardType::Planeswalker
                        | CardType::Permanent
                )
            }),
            AttachmentKind::Equipment => target_types.contains(&CardType::Creature),
        };
        if !target_is_legal {
            return Err(format!(
                "attachment target {} is not a legal battlefield target for {:?}",
                attachment.target, attachment.kind
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

    fn spell_increases(&self) -> Vec<SpellIncreaseRecord> {
        self.spell_increases.clone()
    }

    fn register_spell_increase(&mut self, record: SpellIncreaseRecord) {
        self.spell_increases.push(record);
        self.spell_increases
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

    fn register_revealed_hand(&mut self, record: RevealHandRecord) {
        self.revealed_hands.push(record);
        self.revealed_hands
            .sort_by_key(|item| (item.order, item.source_identity, item.player));
    }

    fn register_hand_inspection(&mut self, record: HandInspectionRecord) {
        self.hand_inspections.push(record);
        self.hand_inspections
            .sort_by_key(|item| (item.order, item.source_identity, item.viewer, item.player));
    }

    fn register_revealed_card(&mut self, record: RevealedCardRecord) {
        self.revealed_cards.push(record);
        self.revealed_cards
            .sort_by_key(|item| (item.order, item.source_identity, item.player, item.card));
    }

    fn register_skipped_step(&mut self, record: SkippedStepRecord) {
        self.skipped_steps.push(record);
        self.skipped_steps
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn next_untap_preventions(&self) -> Vec<NextUntapPreventionRecord> {
        self.next_untap_preventions.clone()
    }

    fn register_next_untap_prevention(&mut self, record: NextUntapPreventionRecord) {
        self.next_untap_preventions.push(record);
        self.next_untap_preventions
            .sort_by_key(|item| (item.order, item.source_identity));
    }

    fn consume_next_untap_prevention(&mut self, order: u64) {
        self.next_untap_preventions
            .retain(|record| record.order != order);
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

    fn chosen_card_name(&self, source: ObjectId) -> Option<String> {
        self.chosen_card_names.get(&source).cloned()
    }

    fn set_chosen_card_name(&mut self, source: ObjectId, name: String) -> Result<(), String> {
        if !self.objects.contains_key(&source) {
            return Err(format!("missing choice source {source}"));
        }
        self.chosen_card_names.insert(source, name);
        Ok(())
    }

    fn chosen_color(&self, source: ObjectId) -> Option<Color> {
        self.chosen_colors.get(&source).copied()
    }

    fn set_chosen_color(&mut self, source: ObjectId, color: Color) -> Result<(), String> {
        if !self.objects.contains_key(&source) {
            return Err(format!("missing choice source {source}"));
        }
        self.chosen_colors.insert(source, color);
        Ok(())
    }

    fn die_roll(&self, source: ObjectId) -> Option<(u16, u16)> {
        self.die_rolls.get(&source).copied()
    }

    fn set_die_roll(&mut self, source: ObjectId, sides: u16, result: u16) -> Result<(), String> {
        if !self.objects.contains_key(&source) {
            return Err(format!("missing die-roll source {source}"));
        }
        if sides < 2 || result == 0 || result > sides {
            return Err(format!("invalid d{sides} result {result}"));
        }
        self.die_rolls.insert(source, (sides, result));
        Ok(())
    }

    fn coin_flip(&self, source: ObjectId) -> Option<CoinFlipResult> {
        self.coin_flips.get(&source).copied()
    }

    fn set_coin_flip(&mut self, source: ObjectId, result: CoinFlipResult) -> Result<(), String> {
        if !self.objects.contains_key(&source) {
            return Err(format!("missing coin-flip source {source}"));
        }
        self.coin_flips.insert(source, result);
        Ok(())
    }

    fn intensity(&self, source: ObjectId) -> Option<u16> {
        self.intensities.get(&source).copied()
    }

    fn initialize_intensity(&mut self, source: ObjectId, amount: u16) -> Result<bool, String> {
        if !self.objects.contains_key(&source) {
            return Err(format!("missing intensity source {source}"));
        }
        if self.intensities.contains_key(&source) {
            return Ok(false);
        }
        self.intensities.insert(source, amount);
        Ok(true)
    }

    fn chosen_option(&self, source: ObjectId) -> Option<String> {
        self.chosen_options.get(&source).cloned()
    }

    fn set_chosen_option(&mut self, source: ObjectId, option: String) -> Result<(), String> {
        if !self.objects.contains_key(&source) {
            return Err(format!("missing named-choice source {source}"));
        }
        if option.trim().is_empty() {
            return Err("named choice cannot be empty".to_owned());
        }
        self.chosen_options.insert(source, option);
        Ok(())
    }

    fn chosen_rules_text(&self, source: ObjectId) -> Option<(RulesTextChoiceKind, String)> {
        self.chosen_rules_text.get(&source).cloned()
    }

    fn set_chosen_rules_text(
        &mut self,
        source: ObjectId,
        kind: RulesTextChoiceKind,
        value: String,
    ) -> Result<(), String> {
        if !self.objects.contains_key(&source) {
            return Err(format!("missing rules-text choice source {source}"));
        }
        let value = value.trim();
        if value.is_empty() {
            return Err("rules-text choice cannot be empty".to_owned());
        }
        self.chosen_rules_text
            .insert(source, (kind, value.to_owned()));
        Ok(())
    }

    fn day_night_designation(&self) -> Option<DayNightDesignation> {
        self.day_night_designation
    }

    fn establish_day_if_unset(&mut self) -> bool {
        if self.day_night_designation.is_some() {
            return false;
        }
        self.day_night_designation = Some(DayNightDesignation::Day);
        true
    }

    fn exhaust_ability_was_activated(&self, source: ObjectId, ability_key: &str) -> bool {
        self.exhaust_activations
            .contains(&(source, ability_key.to_owned()))
    }

    fn mark_exhaust_ability_activated(
        &mut self,
        source: ObjectId,
        ability_key: String,
    ) -> Result<(), String> {
        if !self.exhaust_activations.insert((source, ability_key)) {
            return Err("exhaust ability was already activated".to_owned());
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
    false
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
        | Trigger::SchemeSetInMotion
        | Trigger::SourceAttacks
        | Trigger::SourceCombatDamageToPlayer
        | Trigger::SourceDamageToPlayer
        | Trigger::AttachmentTargetCombatDamageToPlayer { .. }
        | Trigger::BeginningOfNextEndStep => true,
        Trigger::SourceBlocks { object } => filter_has_contract(object),
        Trigger::SagaChapterReached { .. } => false,
        Trigger::ObjectEnters(filter)
        | Trigger::ObjectAttacks(filter)
        | Trigger::CombatDamageToPlayer { source: filter }
        | Trigger::SourceCombatDamageToObject { object: filter } => filter_has_contract(filter),
        Trigger::ObjectTappedForMana {
            object: ObjectRef::AttachmentTarget { .. },
        } => true,
        Trigger::ObjectTappedForMana { object } => object_ref_has_contract(object),
        Trigger::AttachmentTargetEvent { .. } => true,
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
        Trigger::DamageToPlayer { source, player } => {
            filter_has_contract(source) && player_ref_has_contract(player)
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
        ActivationRestriction::All(restrictions) => {
            !restrictions.is_empty() && restrictions.iter().all(activation_restriction_has_contract)
        }
        ActivationRestriction::SorceryTiming
        | ActivationRestriction::InstantTiming
        | ActivationRestriction::YourTurn
        | ActivationRestriction::DuringYourUpkeep
        | ActivationRestriction::DuringYourTurnBeforeAttackersDeclared
        | ActivationRestriction::OnceEachTurn
        | ActivationRestriction::TimesEachTurn(1..=u16::MAX)
        | ActivationRestriction::Exhaust { .. }
        | ActivationRestriction::AnyPlayerMayActivate
        | ActivationRestriction::SourceZone(
            Zone::Library
            | Zone::Hand
            | Zone::Battlefield
            | Zone::Graveyard
            | Zone::Exile
            | Zone::Stack
            | Zone::Command,
        ) => true,
        ActivationRestriction::TimesEachTurn(0) => false,
    }
}

fn choice_count_has_contract(count: &ChoiceCount) -> bool {
    match count {
        ChoiceCount::Exactly(amount) => *amount > 0,
        ChoiceCount::ExactlyWithRepeats(amount) => *amount > 0,
        ChoiceCount::UpTo(amount) => *amount > 0,
        ChoiceCount::Between { minimum, maximum } => *minimum > 0 && minimum <= maximum,
        ChoiceCount::OneOrMore | ChoiceCount::OneOrBothIfTeamwork => true,
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
                TargetAmount::UpTo(_) | TargetAmount::AnyNumber | TargetAmount::All => true,
            }
            && match &target.relationship {
                TargetRelationship::Independent
                | TargetRelationship::DifferentControllers
                | TargetRelationship::ShareCreatureType => true,
                TargetRelationship::OtherThan(object) => object_ref_has_contract(object),
            }
    })
}

fn target_filter_has_contract(filter: &TargetFilter) -> bool {
    match filter {
        TargetFilter::Player | TargetFilter::Opponent => true,
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
            | ObjectState::Blocking
            | ObjectState::Prepared
            | ObjectState::FaceDown,
        ) => true,
        Condition::TargetState { target, .. } => object_ref_has_contract(target),
        Condition::SpellTargetsState { .. } => true,
        Condition::PowerComparison { object, amount, .. } => {
            filter_has_contract(object) && amount_has_contract(amount)
        }
        Condition::EventWouldOccur(event) => replacement_event_has_contract(event),
        Condition::PaymentDeclined(cost) | Condition::PaymentAccepted(cost) => {
            cost_has_contract(cost)
        }
        Condition::CardWasCastWithAlternativeCost
        | Condition::CardWasCastUsingEscape
        | Condition::CardWasKicked
        | Condition::CardWasCastUsingTeamwork
        | Condition::YouAttackedThisTurn
        | Condition::OpponentLostLifeThisTurn
        | Condition::NotYourTurn
        | Condition::NotThatPlayersTurn
        | Condition::GiftPromised
        | Condition::SourceInOpeningHand
        | Condition::NotPlayingFirst
        | Condition::ModeSelected(_)
        | Condition::AnotherSpellCastThisTurn
        | Condition::SourceAttackingAlone
        | Condition::SpellCastFromNonHand
        | Condition::ManaSpentGreaterThanSourcePowerOrToughness
        | Condition::CastOnlyDuringCombat
        | Condition::CastOnlyDuringCombatBeforeBlockers
        | Condition::CastOnlyDuringDeclareBlockers
        | Condition::CastOnlyDuringCombatAfterBlockers
        | Condition::SourceWasCounteredByThisEffect
        | Condition::FirstResolutionOfNamedSpell => true,
        Condition::SpellsCastByActorThisTurn { amount, .. } => *amount > 0,
        Condition::GraveyardCardCount { player, amount, .. }
        | Condition::HandCardCount { player, amount, .. }
        | Condition::CardTypesInGraveyard { player, amount, .. } => {
            player_ref_has_contract(player) && amount_has_contract(amount)
        }
        Condition::SourceHasCounter { .. } => true,
        Condition::SourceCounterCount {
            comparison, amount, ..
        } => *amount > 0 || (*comparison == Comparison::Exactly && *amount == 0),
        Condition::ObjectCounterCount {
            object,
            comparison,
            amount,
            ..
        } => {
            (matches!(object, ObjectRef::AttachmentTarget { .. })
                || object_ref_has_contract(object))
                && (*amount > 0 || (*comparison == Comparison::Exactly && *amount == 0))
        }
        Condition::CommanderControlled { player } => player_ref_has_contract(player),
        Condition::ObjectIsCardType { object, .. } => object_ref_has_contract(object),
        Condition::UnlessPaid { player, cost } => {
            player_ref_has_contract(player) && cost_has_contract(cost)
        }
    }
}

fn cost_has_contract(cost: &Cost) -> bool {
    match cost {
        Cost::Optional(cost) => cost_has_contract(cost),
        Cost::Mana(cost) => mana_cost_has_contract(cost),
        Cost::AtomicResource(cost) => atomic_energy_cost_amount(cost).is_some(),
        Cost::Loyalty(LoyaltyCost::Add(_)) | Cost::Loyalty(LoyaltyCost::Zero) => true,
        Cost::Loyalty(LoyaltyCost::Remove(amount)) => amount_has_contract(amount),
        Cost::Tap(object)
        | Cost::Untap(object)
        | Cost::SacrificeObject(object)
        | Cost::Discard(object)
        | Cost::ExileObject(object)
        | Cost::Unprepare(object) => object_ref_has_contract(object),
        Cost::ExileSourceFromBattlefield | Cost::ExileSourceFromOwnGraveyard => true,
        Cost::TapSelection(selection)
        | Cost::SacrificeSelection(selection)
        | Cost::DiscardSelection(selection)
        | Cost::ExileSelection(selection) => selection_has_contract(selection),
        Cost::ExileSelectionWithTotalManaValue { selection, minimum } => {
            selection_has_contract(selection) && amount_has_contract(minimum)
        }
        Cost::DiscardHand { player } => player_ref_has_contract(player),
        Cost::DiscardRandom { player } => player_ref_has_contract(player),
        Cost::ReturnSelectionToHand(selection) => selection_has_contract(selection),
        Cost::RevealSelection { selection, .. } => selection_has_contract(selection),
        Cost::BeholdSelection {
            battlefield, hand, ..
        } => filter_has_contract(battlefield) && filter_has_contract(hand),
        Cost::Waterbend { selection, amount } => {
            selection_has_contract(selection) && amount_has_contract(amount)
        }
        Cost::PutCounter { object, amount, .. } => {
            object_ref_has_contract(object) && amount_has_contract(amount)
        }
        Cost::PutCounterSelection {
            selection, amount, ..
        } => selection_has_contract(selection) && amount_has_contract(amount),
        Cost::RemoveCounter { object, amount, .. } => {
            object_ref_has_contract(object) && amount_has_contract(amount)
        }
        Cost::TapCreaturesWithTotalPower { player, minimum } => {
            player_ref_has_contract(player) && amount_has_contract(minimum)
        }
        Cost::TapCreatureSelectionWithTotalPower { selection, minimum } => {
            selection_has_contract(selection) && amount_has_contract(minimum)
        }
        Cost::PayLife(amount) => amount_has_contract(amount),
        Cost::Sacrifice { amount, filter } => {
            amount_has_contract(amount) && filter_has_contract(filter)
        }
    }
}

fn atomic_energy_cost_amount(cost: &AtomicResourceCost) -> Option<u32> {
    let mut total = 0u32;
    for component in &cost.expression().components {
        let TypedResourceCostComponent::Energy(amount) = component else {
            return None;
        };
        total = total.checked_add(*amount)?;
    }
    (total > 0).then_some(total)
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
        CountExpression::Constant(_) => true,
        CountExpression::OpponentCount { player }
        | CountExpression::PartySize { player }
        | CountExpression::DistinctBasicLandTypes { player }
        | CountExpression::CreaturesAttackedThisTurn { player } => player_ref_has_contract(player),
        CountExpression::PowerOf { object } | CountExpression::ToughnessOf { object } => {
            object_ref_has_contract(object)
        }
        CountExpression::HalfLifeTotal { player, .. } => player_ref_has_contract(player),
        CountExpression::SelectedObjectsTotalPower { .. }
        | CountExpression::SelectedObjectsTotalToughness { .. } => true,
        CountExpression::MatchingObjects { player, filter }
        | CountExpression::GreatestPower { player, filter } => {
            player_ref_has_contract(player) && filter_has_contract(filter)
        }
        CountExpression::CountersOn { object, .. }
        | CountExpression::AttachmentsOn { object, .. } => object_ref_has_contract(object),
        CountExpression::CardsInZone { player, filter, .. } => {
            player_ref_has_contract(player) && filter_has_contract(filter)
        }
        CountExpression::OpponentsDealtCombatDamage { player }
        | CountExpression::Devotion { player, .. } => player_ref_has_contract(player),
        CountExpression::ManaValueOf { object } => object_ref_has_contract(object),
        CountExpression::LifeLostThisWay {
            players,
            amount_each,
        } => player_ref_has_contract(players) && amount_has_contract(amount_each),
        CountExpression::TriggerEventAmount => true,
        CountExpression::ReplacementEventAmount => true,
    }
}

fn selection_has_contract(selection: &ObjectSelection) -> bool {
    player_ref_has_contract(&selection.chooser)
        && filter_has_contract(&selection.filter)
        && match selection.amount {
            TargetAmount::Exactly(amount) | TargetAmount::UpTo(amount) => amount > 0,
            TargetAmount::AnyNumber | TargetAmount::All => true,
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
        | PlayerRef::OtherPlayer
        | PlayerRef::Any
        | PlayerRef::ThatPlayer
        | PlayerRef::DefendingPlayer => true,
        PlayerRef::TargetPlayer(id) => target_id_has_contract(*id),
        PlayerRef::ControllerOf(object) | PlayerRef::OwnerOf(object) => {
            matches!(object.as_ref(), ObjectRef::AttachmentTarget { .. })
                || object_ref_has_contract(object)
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
        && filter
            .toughness
            .as_ref()
            .is_none_or(|(_, amount)| amount_has_contract(amount))
        && filter
            .minimum_counter
            .as_ref()
            .is_none_or(|(_, amount)| *amount > 0)
        && filter.keywords.iter().all(keyword_filter_has_contract)
        && filter
            .excluded_keywords
            .iter()
            .all(keyword_filter_has_contract)
}

fn keyword_filter_has_contract(keyword: &Keyword) -> bool {
    match keyword {
        Keyword::Ward(cost) => ward_cost_has_contract(cost),
        _ => true,
    }
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
        TypedManaComposition::Alternatives(alternatives) => {
            !alternatives.is_empty() && alternatives.iter().all(typed_mana_composition_has_contract)
        }
        TypedManaComposition::AnyOneColor => true,
        TypedManaComposition::AnyCombination(domain)
        | TypedManaComposition::DifferentColors(domain) => {
            !typed_mana_domain_colors(domain).is_empty()
        }
        TypedManaComposition::Derived(
            TypedDerivedManaTypes::CommanderColorIdentity | TypedDerivedManaTypes::ChosenColor,
        ) => true,
        TypedManaComposition::Derived(
            TypedDerivedManaTypes::ChosenColors
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
        | Duration::UntilEndOfNextTurn
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
        Keyword::Shroud => Some(crate::keyword_rules_runtime::OfficialKeyword::Shroud),
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
        | Restriction::SpellCannotBeCopied { object }
        | Restriction::ActivatedAbilitiesCannotBeActivated { object, .. }
        | Restriction::DestroyProtection { object } => object_ref_has_contract(object),
        Restriction::ActivatedAbilitiesOfMatchingSourcesCannotBeActivated {
            objects,
            duration,
            ..
        } => filter_has_contract(objects) && duration_has_contract(duration),
        Restriction::MinimumX { object, minimum } => {
            object_ref_has_contract(object) && *minimum > 0
        }
        Restriction::MustAttackEachCombatIfAble { object, duration }
        | Restriction::CannotAttack { object, duration } => {
            object_ref_has_contract(object) && duration_has_contract(duration)
        }
        Restriction::AttackCost {
            attackers,
            attacked_player,
            mana_per_attacker,
            duration,
        } => {
            filter_has_contract(attackers)
                && player_ref_has_contract(attacked_player)
                && mana_cost_has_contract(mana_per_attacker)
                && duration_has_contract(duration)
        }
        Restriction::EntersUntapped { objects, duration } => {
            filter_has_contract(objects) && duration_has_contract(duration)
        }
        Restriction::DoesNotUntapDuring {
            object: ObjectRef::AttachmentTarget { .. },
            step: Step::UntapStep,
        } => true,
        Restriction::DoesNotUntapDuring { object, .. } => object_ref_has_contract(object),
        Restriction::DoesNotUntapDuringIf {
            object, condition, ..
        } => object_ref_has_contract(object) && condition_has_contract(condition),
        Restriction::CannotBlock { object, duration }
        | Restriction::CannotBeBlocked { object, duration } => {
            object_ref_has_contract(object) && duration_has_contract(duration)
        }
        Restriction::CannotBeBlockedWhen {
            object,
            condition,
            duration,
        } => {
            object_ref_has_contract(object)
                && condition_has_contract(condition)
                && duration_has_contract(duration)
        }
        Restriction::BlockerMustMatch {
            attacker: ObjectRef::AttachmentTarget { .. },
            blocker_filter,
            duration: Duration::WhileSourceOnBattlefield,
        } => target_filter_has_contract(blocker_filter),
        Restriction::BlockerMustMatch {
            attacker,
            blocker_filter,
            duration,
        } => {
            object_ref_has_contract(attacker)
                && target_filter_has_contract(blocker_filter)
                && duration_has_contract(duration)
        }
        Restriction::CannotBlockMatching {
            blocker,
            attacker_filter,
            duration,
        } => {
            object_ref_has_contract(blocker)
                && filter_has_contract(attacker_filter)
                && duration_has_contract(duration)
        }
        Restriction::CannotBlockObject {
            blocker,
            attacker,
            duration,
        } => {
            object_ref_has_contract(blocker)
                && object_ref_has_contract(attacker)
                && duration_has_contract(duration)
        }
        Restriction::MustBlockIfAble {
            blockers,
            attacker,
            duration,
        } => {
            object_ref_has_contract(blockers)
                && attacker.as_ref().is_none_or(object_ref_has_contract)
                && duration_has_contract(duration)
        }
        Restriction::AssignCombatDamageUsingToughness {
            objects, duration, ..
        } => object_ref_has_contract(objects) && duration_has_contract(duration),
        Restriction::AssignCombatDamageAsThoughUnblocked { objects, duration } => {
            object_ref_has_contract(objects) && duration_has_contract(duration)
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
        Restriction::PlayerCannotLoseGame { players, duration }
        | Restriction::PlayerCannotWinGame { players, duration }
        | Restriction::NonpositiveLifeDoesNotCauseLoss { players, duration } => {
            player_ref_has_contract(players) && duration_has_contract(duration)
        }
        Restriction::HandsRevealed { players, duration } => {
            player_ref_has_contract(players) && duration_has_contract(duration)
        }
        Restriction::SorcerySpeedCastingOnly { players, duration } => {
            player_ref_has_contract(players) && duration_has_contract(duration)
        }
        Restriction::AttackLimit {
            player,
            amount,
            duration,
        }
        | Restriction::AdditionalLandPlays {
            player,
            amount,
            duration,
        } => player_ref_has_contract(player) && *amount > 0 && duration_has_contract(duration),
        Restriction::AdditionalBlockCapacity {
            objects,
            amount,
            duration,
        } => object_ref_has_contract(objects) && *amount > 0 && duration_has_contract(duration),
        Restriction::IgnoreSummoningSicknessForActivatedAbilities { objects, duration } => {
            object_ref_has_contract(objects) && duration_has_contract(duration)
        }
        Restriction::UntapLimit { player, filter, .. } => {
            player_ref_has_contract(player) && filter_has_contract(filter)
        }
        Restriction::TargetingProtection {
            object,
            forbidden_controller,
            duration,
        } => {
            object_ref_has_contract(object)
                && player_ref_has_contract(forbidden_controller)
                && duration_has_contract(duration)
        }
        Restriction::ObjectsCannotBeTargeted { objects, duration } => {
            filter_has_contract(objects) && duration_has_contract(duration)
        }
        Restriction::PlayersCannotBeTargeted { players, duration } => {
            player_ref_has_contract(players) && duration_has_contract(duration)
        }
        Restriction::EnteringCreaturesDoNotCauseAbilitiesToTrigger { duration } => {
            duration_has_contract(duration)
        }
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
        ReplacementEffect::IncreaseEvent { event, addend } => {
            *addend > 0 && replacement_event_has_contract(event)
        }
        ReplacementEffect::EntersTapped(replacement) => {
            object_ref_has_contract(&replacement.object)
                && replacement.when.as_ref().is_none_or(condition_has_contract)
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
        Effect::AddMana(ManaProduction {
            player: PlayerRef::ControllerOf(object),
            choices,
            amount,
            commander_identity_only: false,
            scales_with: None,
            typed: None,
        }) if matches!(&**object, ObjectRef::AttachmentTarget { .. }) => {
            !choices.is_empty()
                && choices.iter().all(|choice| !choice.symbols.is_empty())
                && amount_has_contract(amount)
        }
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
        Effect::SetClassLevel { level } => matches!(level, 2 | 3),
        Effect::Tap {
            object: ObjectRef::AttachmentTarget { .. },
        }
        | Effect::Destroy {
            object: ObjectRef::AttachmentTarget { .. },
        } => true,
        Effect::Counter { object }
        | Effect::CounterToZone { object, .. }
        | Effect::Destroy { object }
        | Effect::DestroyWithoutRegeneration { object }
        | Effect::Tap { object }
        | Effect::Untap { object }
        | Effect::RemoveFromCombat { object }
        | Effect::Exert { object }
        | Effect::PreventNextUntap { object }
        | Effect::Transform { object }
        | Effect::Prepare { object }
        | Effect::ExileSpellAfterResolution { object }
        | Effect::ChooseNewTargets { object } => object_ref_has_contract(object),
        Effect::MoveToLibraryBottom { object }
        | Effect::MoveToChosenLibraryEnd { object, .. }
        | Effect::PutOnLibraryTopInOrder { objects: object } => object_ref_has_contract(object),
        Effect::PutInLibraryAtPosition {
            object,
            position_from_top,
        } => object_ref_has_contract(object) && *position_from_top > 0,
        Effect::CopyStackObject { object, .. } => object_ref_has_contract(object),
        Effect::ChangeControl { object, controller } => {
            object_ref_has_contract(object) && player_ref_has_contract(controller)
        }
        Effect::ChangeControlUntil {
            object,
            controller: PlayerRef::You,
            duration,
        } => object_ref_has_contract(object) && duration_has_contract(duration),
        Effect::ChangeControlUntil { .. } => false,
        Effect::SkipStep { player, .. }
        | Effect::WinGame { player }
        | Effect::LoseGame { player } => player_ref_has_contract(player),
        Effect::TakeExtraTurn(effect) => player_ref_has_contract(&effect.player),
        Effect::SchedulePaymentOrLose(effect) => {
            player_ref_has_contract(&effect.player)
                && cost_has_contract(&effect.cost)
                && trigger_has_contract(&effect.trigger)
        }
        Effect::MoveZone(ZoneMove {
            object: ObjectRef::AttachmentTarget { .. },
            from: Some(Zone::Battlefield),
            to,
            delayed_until: None,
            ..
        }) => matches!(
            to,
            Zone::Hand | Zone::Graveyard | Zone::Exile | Zone::Library
        ),
        Effect::MoveZone(move_zone) => {
            object_ref_has_contract(&move_zone.object)
                && move_zone
                    .delayed_until
                    .as_ref()
                    .is_none_or(trigger_has_contract)
        }
        Effect::MoveZoneUnderControl {
            object,
            controller,
            from,
            to,
            delayed_until,
            ..
        } => {
            object_ref_has_contract(object)
                && player_ref_has_contract(controller)
                && from != to
                && delayed_until.as_ref().is_none_or(trigger_has_contract)
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
            LibraryProcedure::ShuffleGraveyardIntoLibrary { player } => {
                player_ref_has_contract(player)
            }
            LibraryProcedure::ShuffleHandIntoLibraryAndDrawSame { player } => {
                player_ref_has_contract(player)
            }
            LibraryProcedure::ShuffleHandAndGraveyardIntoLibraryAndDraw { player, amount } => {
                player_ref_has_contract(player) && amount_has_contract(amount)
            }
            LibraryProcedure::DiscardHandsAndDraw { player, amount } => {
                player_ref_has_contract(player) && amount_has_contract(amount)
            }
            LibraryProcedure::DiscardHandsAndDrawDiscarded { player, adjustment } => {
                player_ref_has_contract(player) && (-1..=1).contains(adjustment)
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
            player_ref_has_contract(player)
                && matches!(
                    order,
                    BottomOrder::AnyOrder | BottomOrder::ReplaySeededRandom
                )
        }
        Effect::PutRestOfLookedAt {
            player,
            destination,
        } => {
            player_ref_has_contract(player)
                && matches!(destination, Zone::Hand | Zone::Graveyard | Zone::Exile)
        }
        Effect::ReorderLookedAtOnLibraryTop { player } => player_ref_has_contract(player),
        Effect::CreateToken(creation) => token_creation_has_contract(creation),
        Effect::CreateTokenAttached {
            creation, target, ..
        } => token_creation_has_contract(creation) && object_ref_has_contract(target),
        Effect::CreateTokenAndAttachSource {
            creation,
            attachment,
            ..
        } => token_creation_has_contract(creation) && object_ref_has_contract(attachment),
        Effect::Attach {
            attachment, target, ..
        } => object_ref_has_contract(attachment) && object_ref_has_contract(target),
        Effect::ResolveTargetChoice { object } => object_ref_has_contract(object),
        Effect::ResolvePlayerTargetChoice { player } => player_ref_has_contract(player),
        Effect::ChoosePlayer { chooser, eligible } => {
            player_ref_has_contract(chooser) && player_ref_has_contract(eligible)
        }
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
        Effect::DrawRevealDiscardIfNonland { player } => player_ref_has_contract(player),
        Effect::DrawThenDiscardUnless {
            player,
            draw,
            discard,
            alternative,
            ..
        } => {
            player_ref_has_contract(player)
                && *draw > 0
                && *discard > 0
                && filter_has_contract(alternative)
        }
        Effect::ChooseCardName { .. } | Effect::ChooseColor => true,
        Effect::RollDie { sides } => *sides >= 2,
        Effect::FlipCoin => true,
        Effect::Proliferate { .. } => true,
        Effect::InitializeIntensity { .. } => true,
        Effect::ChooseNamedOption { options } => {
            !options.is_empty()
                && options.iter().all(|option| !option.trim().is_empty())
                && options.iter().collect::<BTreeSet<_>>().len() == options.len()
        }
        Effect::ChooseRulesText { .. } => true,
        Effect::EstablishDayIfUnset => true,
        Effect::EachPlayerSacrifices { filter, amount } => {
            filter_has_contract(filter) && *amount > 0
        }
        Effect::PlayersSacrifice {
            players,
            filter,
            amount,
        } => player_ref_has_contract(players) && filter_has_contract(filter) && *amount > 0,
        Effect::RevealHand { player } => player_ref_has_contract(player),
        Effect::LookAtHand { viewer, player } => {
            player_ref_has_contract(viewer) && player_ref_has_contract(player)
        }
        Effect::Discard(selection) => selection_has_contract(selection),
        Effect::Connive { object, discard } => {
            object_ref_has_contract(object) && selection_has_contract(discard)
        }
        Effect::GainLife { player, amount }
        | Effect::LoseLife { player, amount }
        | Effect::PayLife { player, amount }
        | Effect::Scry { player, amount }
        | Effect::Surveil { player, amount }
        | Effect::Mill { player, amount }
        | Effect::LookAtTop { player, amount }
        | Effect::RevealTop { player, amount } => {
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
        Effect::PutPlayerCounter {
            player,
            counter,
            amount,
        } => {
            player_ref_has_contract(player)
                && !counter.trim().is_empty()
                && amount_has_contract(amount)
        }
        Effect::PutCounter { object, amount, .. } => {
            (matches!(object, ObjectRef::AttachmentTarget { .. })
                || object_ref_has_contract(object))
                && amount_has_contract(amount)
        }
        Effect::RemoveCounter { object, amount, .. } => {
            object_ref_has_contract(object) && amount_has_contract(amount)
        }
        Effect::MoveAllCounters { from, to } => {
            object_ref_has_contract(from) && object_ref_has_contract(to)
        }
        Effect::ModifyPowerToughness(PowerToughnessChange {
            objects: ObjectRef::AttachmentTarget { .. },
            operation:
                PowerToughnessOperation::Add
                | PowerToughnessOperation::Subtract
                | PowerToughnessOperation::AddPowerSubtractToughness
                | PowerToughnessOperation::SubtractPowerAddToughness
                | PowerToughnessOperation::Switch,
            power,
            toughness,
            duration,
        }) => {
            amount_has_contract(power)
                && amount_has_contract(toughness)
                && duration_has_contract(duration)
        }
        Effect::ModifyPowerToughness(change) => {
            object_ref_has_contract(&change.objects)
                && amount_has_contract(&change.power)
                && amount_has_contract(&change.toughness)
                && duration_has_contract(&change.duration)
        }
        Effect::GrantKeyword {
            objects: ObjectRef::AttachmentTarget { .. },
            keywords,
            duration: Duration::WhileSourceOnBattlefield,
        } => !keywords.is_empty() && keywords.iter().all(keyword_has_contract),
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
            objects: ObjectRef::AttachmentTarget { .. },
            ability,
            duration: Duration::WhileSourceOnBattlefield,
        } => granted_ability_has_contract(ability),
        Effect::GrantAbility {
            objects,
            ability,
            duration,
        } => {
            object_ref_has_contract(objects)
                && granted_ability_has_contract(ability)
                && duration_has_contract(duration)
        }
        Effect::LoseAllAbilities {
            object: ObjectRef::AttachmentTarget { .. },
            duration: Duration::WhileSourceOnBattlefield,
        } => true,
        Effect::LoseAllAbilities { object, duration } => {
            object_ref_has_contract(object) && duration_has_contract(duration)
        }
        Effect::SetCharacteristics(SetCharacteristics {
            object: ObjectRef::AttachmentTarget { .. },
            duration: Duration::WhileSourceOnBattlefield,
            ..
        }) => true,
        Effect::SetCharacteristics(change) => {
            object_ref_has_contract(&change.object)
                && change.base_power.as_ref().is_none_or(amount_has_contract)
                && change
                    .base_toughness
                    .as_ref()
                    .is_none_or(amount_has_contract)
                && duration_has_contract(&change.duration)
        }
        Effect::SetCreatureTypeToChoice { object, duration } => {
            object_ref_has_contract(object) && duration_has_contract(duration)
        }
        Effect::SetColorToChoice { object, duration } => {
            object_ref_has_contract(object) && duration_has_contract(duration)
        }
        Effect::SetBasicLandTypeToChoice { object, duration } => {
            object_ref_has_contract(object) && duration_has_contract(duration)
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
        Effect::ReduceSpellCostWhen {
            object,
            mana,
            condition,
        } => {
            object_ref_has_contract(object)
                && mana_cost_has_contract(mana)
                && condition_has_contract(condition)
        }
        Effect::IncreaseSpellCost { object, mana, per } => {
            object_ref_has_contract(object)
                && mana_cost_has_contract(mana)
                && count_has_contract(per)
        }
        Effect::ChooseMode { count } => choice_count_has_contract(count),
        Effect::ChooseModeBy { chooser, count } => {
            player_ref_has_contract(chooser) && choice_count_has_contract(count)
        }
        Effect::ChooseModeNotPreviouslyChosen { count } => choice_count_has_contract(count),
        Effect::ChooseModeFrom {
            count,
            option_count,
        } => choice_count_has_contract(count) && *option_count > 0,
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
        | ReminderSemantics::GoldDefinition(definition)
        | ReminderSemantics::LanderDefinition(definition) => {
            token_definition_has_contract(definition)
        }
        ReminderSemantics::TrampleExplanation
        | ReminderSemantics::HexproofExplanation
        | ReminderSemantics::PlayerShroudProcedure
        | ReminderSemantics::IndestructibleExplanation
        | ReminderSemantics::ProwessProcedure
        | ReminderSemantics::ManifestProcedure
        | ReminderSemantics::SplitSecondProcedure
        | ReminderSemantics::PartnerProcedure
        | ReminderSemantics::PreparedProcedure
        | ReminderSemantics::SpellCommanderProcedure
        | ReminderSemantics::ParadigmProcedure
        | ReminderSemantics::GiftCardProcedure
        | ReminderSemantics::ConniveProcedure
        | ReminderSemantics::UndauntedProcedure
        | ReminderSemantics::PartyComposition
        | ReminderSemantics::FlashProcedure
        | ReminderSemantics::UntapSymbolProcedure
        | ReminderSemantics::StunCounterProcedure
        | ReminderSemantics::ExertProcedure
        | ReminderSemantics::BoastProcedure
        | ReminderSemantics::ExhaustProcedure
        | ReminderSemantics::HistoricDefinition
        | ReminderSemantics::ProliferateProcedure
        | ReminderSemantics::AdventureProcedure
        | ReminderSemantics::OmenProcedure
        | ReminderSemantics::TransformOrigin { .. }
        | ReminderSemantics::CharacteristicLossExplanation => true,
        ReminderSemantics::ZoneQualification { object, .. } => object_ref_has_contract(object),
        ReminderSemantics::TeamworkProcedure { minimum_power } => {
            amount_has_contract(minimum_power)
        }
        ReminderSemantics::KickerSacrificeProcedure { amount, filter } => {
            amount_has_contract(amount) && filter_has_contract(filter)
        }
        ReminderSemantics::CollectEvidenceProcedure { minimum }
        | ReminderSemantics::BlightProcedure { amount: minimum }
        | ReminderSemantics::EnergyCounterExplanation { amount: minimum } => {
            amount_has_contract(minimum)
        }
        ReminderSemantics::BeholdProcedure { subtype } => !subtype.trim().is_empty(),
        ReminderSemantics::WaterbendProcedure { amount } => amount_has_contract(amount),
        ReminderSemantics::IncrementProcedure => true,
        ReminderSemantics::FearProcedure => true,
        ReminderSemantics::SurveilProcedure { amount }
        | ReminderSemantics::ScryProcedure { amount }
        | ReminderSemantics::HideawayProcedure { amount }
        | ReminderSemantics::AfterlifeProcedure { amount } => amount_has_contract(amount),
        ReminderSemantics::MillProcedure { player, amount, .. } => {
            player_ref_has_contract(player) && amount_has_contract(amount)
        }
        ReminderSemantics::CrewProcedure { required_power } => amount_has_contract(required_power),
        ReminderSemantics::StationProcedure { creature_threshold } => {
            creature_threshold.is_none_or(|threshold| threshold > 0)
        }
        ReminderSemantics::CyclingProcedure { cost } => mana_cost_has_contract(cost),
        ReminderSemantics::TypecyclingProcedure { cost, filter, .. } => {
            mana_cost_has_contract(cost) && filter_has_contract(filter)
        }
        ReminderSemantics::EvokeProcedure { cost } => mana_cost_has_contract(cost),
        ReminderSemantics::DevotionProcedure { .. } => true,
        ReminderSemantics::FlashbackProcedure | ReminderSemantics::EscapeProcedure => true,
        ReminderSemantics::DashProcedure { cost } => mana_cost_has_contract(cost),
        ReminderSemantics::OutlastProcedure { cost } => mana_cost_has_contract(cost),
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
    let exhaust_key = matches!(
        clause.activation_restriction(),
        Some(ActivationRestriction::Exhaust { .. })
    )
    .then(|| clause.semantic_digest().to_owned());
    if exhaust_key
        .as_deref()
        .is_some_and(|key| state.exhaust_ability_was_activated(context.source, key))
    {
        return Err(ExecutionError::ActivationRestrictionFailed);
    }
    let checkpoint = exhaust_key.as_ref().map(|_| state.checkpoint());
    let receipt = execute_action(
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
    )?;
    if let Some(key) = exhaust_key {
        if let Err(error) = state.mark_exhaust_ability_activated(context.source, key) {
            state.restore(checkpoint.expect("Exhaust execution retained a checkpoint"));
            return Err(ExecutionError::Adapter(error));
        }
        state.record_mutation(format!("exhaust_activated:{}", context.source));
    }
    Ok(receipt)
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
    if matches!(action.timing, Timing::Activated)
        && !matches!(
            action.activation_restriction,
            Some(ActivationRestriction::AnyPlayerMayActivate)
        )
        && source_controller(state, context)? != context.actor
    {
        return Err(ExecutionError::ActivationRestrictionFailed);
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
                && !entering_creature_trigger_is_suppressed(state, event, context)?
        }
        (Timing::TriggeredModalHeader { trigger, choices }, ActionWindow::Triggered(event)) => {
            trigger_matches(state, trigger, event, context)?
                && !entering_creature_trigger_is_suppressed(state, event, context)?
                && choice_count_matches(choices, &context.selected_modes, context)
        }
        (Timing::ModalHeader { choices }, ActionWindow::ModalHeader) => {
            choice_count_matches(choices, &context.selected_modes, context)
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

fn entering_creature_trigger_is_suppressed<S: OracleStateAdapter>(
    state: &S,
    event: &TriggerEvent,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    let TriggerEvent::ObjectEntered { object } = event else {
        return Ok(false);
    };
    let entering = effective_object(state, *object, context)?;
    if entering.zone != Zone::Battlefield
        || !entering
            .characteristics()
            .card_types
            .contains(&CardType::Creature)
    {
        return Ok(false);
    }
    for record in sorted_restrictions(state) {
        let Restriction::EnteringCreaturesDoNotCauseAbilitiesToTrigger { duration } =
            &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if restriction_duration_is_active(state, record.source_identity, duration, &local)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn choice_count_matches(count: &ChoiceCount, selected: &[u16], context: &ExecutionContext) -> bool {
    let unique = selected.iter().copied().collect::<BTreeSet<_>>();
    if !matches!(count, ChoiceCount::ExactlyWithRepeats(_)) && unique.len() != selected.len() {
        return false;
    }
    match count {
        ChoiceCount::Exactly(amount) => selected.len() == usize::from(*amount),
        ChoiceCount::ExactlyWithRepeats(amount) => selected.len() == usize::from(*amount),
        ChoiceCount::UpTo(amount) => selected.len() <= usize::from(*amount),
        ChoiceCount::Between { minimum, maximum } => {
            selected.len() >= usize::from(*minimum) && selected.len() <= usize::from(*maximum)
        }
        ChoiceCount::OneOrMore => !selected.is_empty(),
        ChoiceCount::OneOrBothIfTeamwork => {
            selected.len()
                == if context.card_was_cast_using_teamwork {
                    2
                } else {
                    1
                }
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
        Trigger::SchemeSetInMotion => matches!(
            event,
            TriggerEvent::SchemeSetInMotion { object } if *object == context.source
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
        Trigger::ObjectTappedForMana { object } => match event {
            TriggerEvent::ObjectTappedForMana {
                object: actual_object,
            } => resolve_objects(state, object, context)?.contains(actual_object),
            _ => false,
        },
        Trigger::AttachmentTargetEvent {
            kind,
            event: expected,
        } => matches!(
            event,
            TriggerEvent::AttachmentTargetEvent {
                attachment_source,
                kind: actual_kind,
                event: actual,
                ..
            } if *attachment_source == context.source
                && actual == expected
                && actual_kind == kind
        ),
        Trigger::SourceAttacks => {
            matches!(event, TriggerEvent::ObjectAttacked { object } if *object == context.source)
        }
        Trigger::SourceBlocks { object } => match event {
            TriggerEvent::ObjectBlocked { blocker, blocked } => {
                *blocker == context.source
                    && object_matches_filter(state, *blocked, object, context)?
            }
            _ => false,
        },
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
            event: ObjectEventKind::DealtDamage,
        } => match event {
            TriggerEvent::DamageToObject { object, .. }
            | TriggerEvent::CombatDamageToObject { object, .. } => match subject {
                TriggerSubject::Source => *object == context.source,
                TriggerSubject::Matching(filter) => {
                    object_matches_filter(state, *object, filter, context)?
                }
            },
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
        Trigger::AttachmentTargetCombatDamageToPlayer { kind } => match event {
            TriggerEvent::CombatDamageToPlayer { source, .. } => {
                resolve_attachment_target(state, context.source, *kind)? == *source
            }
            _ => false,
        },
        Trigger::DamageToPlayer { source, player } => match event {
            TriggerEvent::DamageToPlayer {
                source: actual_source,
                player: actual_player,
                ..
            }
            | TriggerEvent::CombatDamageToPlayer {
                source: actual_source,
                player: actual_player,
                ..
            } => {
                resolve_players(state, player, context)?.contains(actual_player)
                    && object_matches_filter(state, *actual_source, source, context)?
            }
            _ => false,
        },
        Trigger::SourceCombatDamageToPlayer => matches!(
            event,
            TriggerEvent::CombatDamageToPlayer { source, .. } if *source == context.source
        ),
        Trigger::SourceDamageToPlayer => matches!(
            event,
            TriggerEvent::CombatDamageToPlayer { source, .. }
                | TriggerEvent::DamageToPlayer { source, .. }
                if *source == context.source
        ),
        Trigger::SourceCombatDamageToObject { object } => match event {
            TriggerEvent::CombatDamageToObject {
                source,
                object: actual_object,
                ..
            }
            | TriggerEvent::DamageToObject {
                source,
                object: actual_object,
                combat: true,
                ..
            } => {
                *source == context.source
                    && object_matches_filter(state, *actual_object, object, context)?
            }
            _ => false,
        },
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
        ActivationRestriction::All(restrictions) => {
            for restriction in restrictions {
                if !activation_restriction_holds(state, restriction, context)? {
                    return Ok(false);
                }
            }
            !restrictions.is_empty()
        }
        ActivationRestriction::SorceryTiming => context.sorcery_timing,
        ActivationRestriction::InstantTiming => context.instant_timing,
        ActivationRestriction::YourTurn => {
            source_controller(state, context)? == context.active_player
        }
        ActivationRestriction::DuringYourUpkeep => {
            source_controller(state, context)? == context.active_player
                && context.current_step == Some(Step::Upkeep)
        }
        ActivationRestriction::DuringYourTurnBeforeAttackersDeclared => {
            source_controller(state, context)? == context.active_player
                && !context.attackers_declared
        }
        ActivationRestriction::OnceEachTurn => context.ability_occurrence_this_turn == 1,
        ActivationRestriction::TimesEachTurn(maximum) => {
            context.ability_occurrence_this_turn >= 1
                && context.ability_occurrence_this_turn <= u32::from(*maximum)
        }
        ActivationRestriction::Exhaust {
            sorcery_timing_only,
        } => !sorcery_timing_only || context.sorcery_timing,
        ActivationRestriction::AnyPlayerMayActivate => true,
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
            Restriction::ActivatedAbilitiesOfMatchingSourcesCannotBeActivated {
                objects,
                except_mana_abilities,
                duration,
            } if matches!(context.window, ActionWindow::Activated)
                && (!except_mana_abilities || !context.is_mana_ability)
                && restriction_duration_is_active(
                    state,
                    record.source_identity,
                    duration,
                    &local,
                )?
                && object_matches_filter(state, context.source, objects, &local)? =>
            {
                return Ok(true);
            }
            Restriction::CannotCast {
                affected,
                filter,
                duration,
                during_turn_of,
            } if matches!(
                context.window,
                ActionWindow::CastingAdditionalCost | ActionWindow::SpellResolution
            ) && resolve_players(state, affected, &local)?.contains(&context.actor)
                && restriction_duration_is_active(
                    state,
                    record.source_identity,
                    duration,
                    &local,
                )?
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
        Duration::Permanent
        | Duration::ThisTurn
        | Duration::UntilEndOfTurn
        | Duration::UntilEndOfNextTurn => true,
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
    let restrictions = sorted_restrictions(state);
    for record in &restrictions {
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
    let mut required_generic = 0u32;
    for record in &restrictions {
        let Restriction::AttackCost {
            attackers,
            attacked_player,
            mana_per_attacker,
            duration,
        } = &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if !restriction_duration_is_active(state, record.source_identity, duration, &local)?
            || !object_matches_filter(state, object, attackers, &local)?
        {
            continue;
        }
        let defender = context
            .defending_player
            .ok_or(ExecutionError::InvalidAmount(
                "defending player is required to evaluate attack costs",
            ))?;
        if !resolve_players(state, attacked_player, &local)?.contains(&defender) {
            continue;
        }
        required_generic = required_generic
            .checked_add(generic_only_mana_amount(mana_per_attacker)?)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
    }
    if context
        .attack_tax_generic_paid
        .get(&object)
        .copied()
        .unwrap_or_default()
        < required_generic
    {
        return Ok(false);
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

pub fn object_can_block_attacker<S: OracleStateAdapter>(
    state: &S,
    blocker: ObjectId,
    attacker: ObjectId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    if !object_can_block(state, blocker, context)? {
        return Ok(false);
    }
    if !object_can_be_blocked(state, attacker, context)? {
        return Ok(false);
    }
    let blocker_object = state
        .object(blocker)
        .ok_or(ExecutionError::MissingObject(blocker))?;
    let attacker_object = state
        .object(attacker)
        .ok_or(ExecutionError::MissingObject(attacker))?;
    if blocker_object.zone != Zone::Battlefield || attacker_object.zone != Zone::Battlefield {
        return Ok(false);
    }
    for record in sorted_restrictions(state) {
        let mut local = context.clone();
        local.source = record.source_identity;
        match &record.restriction {
            Restriction::BlockerMustMatch {
                attacker: restricted_attacker,
                blocker_filter,
                duration,
            } => {
                if restriction_duration_is_active(state, record.source_identity, duration, &local)?
                    && resolve_objects(state, restricted_attacker, &local)?.contains(&attacker)
                    && !legal_target_candidates(state, blocker_filter, &local)?
                        .contains(&SelectedTarget::Object(blocker))
                {
                    return Ok(false);
                }
            }
            Restriction::CannotBlockMatching {
                blocker: restricted_blocker,
                attacker_filter,
                duration,
            } => {
                if restriction_duration_is_active(state, record.source_identity, duration, &local)?
                    && resolve_objects(state, restricted_blocker, &local)?.contains(&blocker)
                    && object_matches_filter(state, attacker, attacker_filter, &local)?
                {
                    return Ok(false);
                }
            }
            Restriction::CannotBlockObject {
                blocker: restricted_blocker,
                attacker: restricted_attacker,
                duration,
            } if restriction_duration_is_active(
                state,
                record.source_identity,
                duration,
                &local,
            )? && resolve_objects(state, restricted_blocker, &local)?.contains(&blocker)
                && resolve_objects(state, restricted_attacker, &local)?.contains(&attacker) =>
            {
                return Ok(false);
            }
            _ => {}
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
        let mut local = context.clone();
        local.source = record.source_identity;
        match &record.restriction {
            Restriction::CannotBeBlocked {
                object: restricted,
                duration,
            } => {
                let active = restriction_duration_is_active(
                    state,
                    record.source_identity,
                    duration,
                    &local,
                )?;
                if active && resolve_objects(state, restricted, &local)?.contains(&object) {
                    return Ok(false);
                }
            }
            Restriction::CannotBeBlockedWhen {
                object: restricted,
                condition,
                duration,
            } => {
                let active = restriction_duration_is_active(
                    state,
                    record.source_identity,
                    duration,
                    &local,
                )?;
                if active
                    && condition_holds(state, condition, &local)?
                    && resolve_objects(state, restricted, &local)?.contains(&object)
                {
                    return Ok(false);
                }
            }
            _ => {}
        }
    }
    Ok(true)
}

pub fn player_hand_is_revealed<S: OracleStateAdapter>(
    state: &S,
    player: PlayerId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    if state.player(player).is_none() {
        return Err(ExecutionError::MissingPlayer(player));
    }
    for record in sorted_restrictions(state) {
        let Restriction::HandsRevealed { players, duration } = &record.restriction else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if restriction_duration_is_active(state, record.source_identity, duration, &local)?
            && resolve_players(state, players, &local)?.contains(&player)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn player_has_game_rule_restriction<S: OracleStateAdapter>(
    state: &S,
    player: PlayerId,
    context: &ExecutionContext,
    matches_rule: impl Fn(&Restriction) -> Option<(&PlayerRef, &Duration)>,
) -> Result<bool, ExecutionError> {
    if state.player(player).is_none() {
        return Err(ExecutionError::MissingPlayer(player));
    }
    for record in sorted_restrictions(state) {
        let Some((players, duration)) = matches_rule(&record.restriction) else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if restriction_duration_is_active(state, record.source_identity, duration, &local)?
            && resolve_players(state, players, &local)?.contains(&player)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn player_can_lose_game<S: OracleStateAdapter>(
    state: &S,
    player: PlayerId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    Ok(!player_has_game_rule_restriction(
        state,
        player,
        context,
        |restriction| match restriction {
            Restriction::PlayerCannotLoseGame { players, duration } => Some((players, duration)),
            _ => None,
        },
    )?)
}

pub fn player_can_win_game<S: OracleStateAdapter>(
    state: &S,
    player: PlayerId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    Ok(!player_has_game_rule_restriction(
        state,
        player,
        context,
        |restriction| match restriction {
            Restriction::PlayerCannotWinGame { players, duration } => Some((players, duration)),
            _ => None,
        },
    )?)
}

pub fn nonpositive_life_causes_player_to_lose<S: OracleStateAdapter>(
    state: &S,
    player: PlayerId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    Ok(!player_has_game_rule_restriction(
        state,
        player,
        context,
        |restriction| match restriction {
            Restriction::NonpositiveLifeDoesNotCauseLoss { players, duration } => {
                Some((players, duration))
            }
            _ => None,
        },
    )?)
}

pub fn player_can_cast_at_current_timing<S: OracleStateAdapter>(
    state: &S,
    player: PlayerId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    if state.player(player).is_none() {
        return Err(ExecutionError::MissingPlayer(player));
    }
    for record in sorted_restrictions(state) {
        let Restriction::SorcerySpeedCastingOnly { players, duration } = &record.restriction else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if restriction_duration_is_active(state, record.source_identity, duration, &local)?
            && resolve_players(state, players, &local)?.contains(&player)
            && !context.sorcery_timing
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn object_ignores_summoning_sickness_for_activated_abilities<S: OracleStateAdapter>(
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
        let Restriction::IgnoreSummoningSicknessForActivatedAbilities { objects, duration } =
            &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if restriction_duration_is_active(state, record.source_identity, duration, &local)?
            && resolve_objects(state, objects, &local)?.contains(&object)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn maximum_creature_attackers<S: OracleStateAdapter>(
    state: &S,
    player: PlayerId,
    context: &ExecutionContext,
) -> Result<Option<u16>, ExecutionError> {
    if state.player(player).is_none() {
        return Err(ExecutionError::MissingPlayer(player));
    }
    let mut limit = None;
    for record in sorted_restrictions(state) {
        let Restriction::AttackLimit {
            player: affected,
            amount,
            duration,
        } = &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if restriction_duration_is_active(state, record.source_identity, duration, &local)?
            && resolve_players(state, affected, &local)?.contains(&player)
        {
            limit = Some(limit.map_or(*amount, |current: u16| current.min(*amount)));
        }
    }
    Ok(limit)
}

pub fn creature_block_capacity<S: OracleStateAdapter>(
    state: &S,
    object: ObjectId,
    context: &ExecutionContext,
) -> Result<u16, ExecutionError> {
    let candidate = state
        .object(object)
        .ok_or(ExecutionError::MissingObject(object))?;
    if candidate.zone != Zone::Battlefield || !object_has_type(&candidate, CardType::Creature) {
        return Ok(0);
    }
    let mut capacity = 1u16;
    for record in sorted_restrictions(state) {
        let Restriction::AdditionalBlockCapacity {
            objects,
            amount,
            duration,
        } = &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if restriction_duration_is_active(state, record.source_identity, duration, &local)?
            && resolve_objects(state, objects, &local)?.contains(&object)
        {
            capacity = capacity
                .checked_add(*amount)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        }
    }
    Ok(capacity)
}

pub fn spell_can_be_copied<S: OracleStateAdapter>(
    state: &S,
    spell: ObjectId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    for record in sorted_restrictions(state) {
        let Restriction::SpellCannotBeCopied { object } = &record.restriction else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if resolve_objects(state, object, &local)?.contains(&spell) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn spell_x_value_is_legal<S: OracleStateAdapter>(
    state: &S,
    spell: ObjectId,
    x_value: u32,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    for record in sorted_restrictions(state) {
        let Restriction::MinimumX { object, minimum } = &record.restriction else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if resolve_objects(state, object, &local)?.contains(&spell) && x_value < *minimum {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn land_play_limit<S: OracleStateAdapter>(
    state: &S,
    player: PlayerId,
    context: &ExecutionContext,
) -> Result<u16, ExecutionError> {
    if state.player(player).is_none() {
        return Err(ExecutionError::MissingPlayer(player));
    }
    let mut limit = 1u16;
    for record in sorted_restrictions(state) {
        let Restriction::AdditionalLandPlays {
            player: affected,
            amount,
            duration,
        } = &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if restriction_duration_is_active(state, record.source_identity, duration, &local)?
            && resolve_players(state, affected, &local)?.contains(&player)
        {
            limit = limit
                .checked_add(*amount)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
        }
    }
    Ok(limit)
}

/// Reports whether a continuous requirement makes `blocker` block the given
/// `attacker` if the normal combat legality rules allow that block.
pub fn object_must_block_attacker_if_able<S: OracleStateAdapter>(
    state: &S,
    blocker: ObjectId,
    attacker: ObjectId,
    context: &ExecutionContext,
) -> Result<bool, ExecutionError> {
    let blocker_object = state
        .object(blocker)
        .ok_or(ExecutionError::MissingObject(blocker))?;
    let attacker_object = state
        .object(attacker)
        .ok_or(ExecutionError::MissingObject(attacker))?;
    if blocker_object.zone != Zone::Battlefield || attacker_object.zone != Zone::Battlefield {
        return Ok(false);
    }
    for record in sorted_restrictions(state) {
        let Restriction::MustBlockIfAble {
            blockers,
            attacker: required_attacker,
            duration,
        } = &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if !restriction_duration_is_active(state, record.source_identity, duration, &local)?
            || !resolve_objects(state, blockers, &local)?.contains(&blocker)
        {
            continue;
        }
        if required_attacker.as_ref().is_none_or(|required| {
            resolve_objects(state, required, &local)
                .unwrap_or_default()
                .contains(&attacker)
        }) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Returns the current characteristic used to assign this creature's combat
/// damage after applying typed power-versus-toughness assignment rules.
pub fn combat_damage_assignment_value<S: OracleStateAdapter>(
    state: &S,
    object: ObjectId,
    context: &ExecutionContext,
) -> Result<i64, ExecutionError> {
    let candidate = state
        .object(object)
        .ok_or(ExecutionError::MissingObject(object))?;
    if candidate.zone != Zone::Battlefield {
        return Err(ExecutionError::InvalidAmount(
            "combat-damage source is not on the battlefield",
        ));
    }
    let characteristics = candidate.characteristics();
    for record in sorted_restrictions(state) {
        let Restriction::AssignCombatDamageUsingToughness {
            objects,
            only_if_toughness_greater,
            duration,
        } = &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if restriction_duration_is_active(state, record.source_identity, duration, &local)?
            && resolve_objects(state, objects, &local)?.contains(&object)
            && (!only_if_toughness_greater || characteristics.toughness > characteristics.power)
        {
            return Ok(characteristics.toughness);
        }
    }
    Ok(characteristics.power)
}

pub fn object_may_assign_combat_damage_as_though_unblocked<S: OracleStateAdapter>(
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
        let Restriction::AssignCombatDamageAsThoughUnblocked { objects, duration } =
            &record.restriction
        else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if restriction_duration_is_active(state, record.source_identity, duration, &local)?
            && resolve_objects(state, objects, &local)?.contains(&object)
        {
            return Ok(true);
        }
    }
    Ok(false)
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
    if candidate.counters.get("stun").copied().unwrap_or(0) > 0 {
        return Ok(false);
    }
    if step == Step::UntapStep
        && state
            .next_untap_preventions()
            .iter()
            .any(|record| record.object_identities.contains(&object))
    {
        return Ok(false);
    }
    for record in sorted_restrictions(state) {
        let (restricted, restricted_step, condition) = match &record.restriction {
            Restriction::DoesNotUntapDuring { object, step } => (object, step, None),
            Restriction::DoesNotUntapDuringIf {
                object,
                step,
                condition,
            } => (object, step, Some(condition)),
            _ => continue,
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
        if let Some(condition) = condition
            && !condition_holds(state, condition, &local)?
        {
            continue;
        }
        if resolve_objects(state, restricted, &local)?.contains(&object) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn untap_object_during<S: OracleStateAdapter>(
    state: &mut S,
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
        let (restricted, restricted_step, condition) = match &record.restriction {
            Restriction::DoesNotUntapDuring { object, step } => (object, step, None),
            Restriction::DoesNotUntapDuringIf {
                object,
                step,
                condition,
            } => (object, step, Some(condition)),
            _ => continue,
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
        if let Some(condition) = condition
            && !condition_holds(state, condition, &local)?
        {
            continue;
        }
        if resolve_objects(state, restricted, &local)?.contains(&object) {
            state.record_mutation(format!("prevent_untap_static:{object}:{step:?}"));
            return Ok(false);
        }
    }

    if step == Step::UntapStep {
        let mut records = state.next_untap_preventions();
        records.sort_by_key(|record| (record.order, record.source_identity));
        if let Some(record) = records
            .into_iter()
            .find(|record| record.object_identities.contains(&object))
        {
            state.consume_next_untap_prevention(record.order);
            state.record_mutation(format!("prevent_next_untap:{object}:{}", record.order));
            return Ok(false);
        }
    }

    attempt_direct_untap(state, object)
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
                if resolve_objects(state, &change.object, &local)?.contains(&object) =>
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
            } if resolve_objects(state, attached, &local)?.contains(&object) => {
                effective.characteristics_mut().abilities.clear();
                effective.characteristics_mut().keywords.clear();
            }
            Effect::GrantKeyword {
                objects, keywords, ..
            } if resolve_objects(state, objects, &local)?.contains(&object) => {
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
            } if resolve_objects(state, objects, &local)?.contains(&object) => {
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
                if resolve_objects(state, &change.object, &local)?.contains(&object) =>
            {
                if let Some(power) = &change.base_power {
                    effective.characteristics_mut().power =
                        i64::from(evaluate_amount(state, power, &local)?);
                }
                if let Some(toughness) = &change.base_toughness {
                    effective.characteristics_mut().toughness =
                        i64::from(evaluate_amount(state, toughness, &local)?);
                }
            }
            Effect::ModifyPowerToughness(change)
                if matches!(
                    change.operation,
                    PowerToughnessOperation::SetBase
                        | PowerToughnessOperation::SetPower
                        | PowerToughnessOperation::SetToughness
                ) && resolve_objects(state, &change.objects, &local)?.contains(&object) =>
            {
                let power = i64::from(evaluate_amount(state, &change.power, &local)?);
                let toughness = i64::from(evaluate_amount(state, &change.toughness, &local)?);
                apply_power_toughness_operation(
                    effective.characteristics_mut(),
                    change.operation.clone(),
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
        if matches!(
            change.operation,
            PowerToughnessOperation::SetBase
                | PowerToughnessOperation::SetPower
                | PowerToughnessOperation::SetToughness
        ) || !resolve_objects(state, &change.objects, &local)?.contains(&object)
        {
            continue;
        }
        let power = i64::from(evaluate_amount(state, &change.power, &local)?);
        let toughness = i64::from(evaluate_amount(state, &change.toughness, &local)?);
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
        let controlled = match &record.effect {
            Effect::ChangeControl {
                object,
                controller: PlayerRef::You,
            } if matches!(object, ObjectRef::AttachmentTarget { .. }) => object,
            Effect::ChangeControlUntil {
                object,
                controller: PlayerRef::You,
                ..
            } => object,
            _ => continue,
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if !restriction_duration_is_active(state, record.source_identity, &record.duration, &local)?
        {
            continue;
        }
        let target = resolve_objects(state, controlled, &local)?
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

    let mut increases = state.spell_increases();
    increases.sort_by_key(|record| (record.order, record.source_identity));
    let mut requested_increase = 0u32;
    for record in increases {
        let Some(source) = state.object(record.source_identity) else {
            continue;
        };
        if source.zone != Zone::Battlefield {
            continue;
        }
        let mut local = context.clone();
        local.source = record.source_identity;
        if !resolve_objects(state, &record.object, &local)?.contains(&spell) {
            continue;
        }
        let per_object = generic_only_mana_amount(&record.mana)?;
        let increase = per_object
            .checked_mul(evaluate_count(state, &record.per, &local)?)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
        requested_increase = requested_increase
            .checked_add(increase)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
    }
    generic_total = generic_total
        .checked_add(requested_increase)
        .ok_or(ExecutionError::ArithmeticOverflow)?;

    let mut records = state.spell_reductions();
    records.sort_by_key(|record| (record.order, record.source_identity));
    let mut requested_reduction = 0u32;
    for record in records {
        let mut local = context.clone();
        local.source = record.source_identity;
        if !matches!(record.object, ObjectRef::Source)
            && !state
                .object(record.source_identity)
                .is_some_and(|source| source.zone == Zone::Battlefield)
        {
            continue;
        }
        if let Some(condition) = &record.condition
            && !condition_holds(state, condition, &local)?
        {
            continue;
        }
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
            TargetAmount::Exactly(_) | TargetAmount::UpTo(_) | TargetAmount::AnyNumber => {}
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
            TargetRelationship::ShareCreatureType => {
                let mut shared_types: Option<BTreeSet<String>> = None;
                for value in selected {
                    let SelectedTarget::Object(id) = value else {
                        return Err(ExecutionError::IllegalTarget { id: target.id });
                    };
                    let creature_types = state
                        .object(*id)
                        .ok_or(ExecutionError::MissingObject(*id))?
                        .characteristics()
                        .subtypes
                        .iter()
                        .map(|subtype| subtype.to_ascii_lowercase())
                        .collect::<BTreeSet<_>>();
                    shared_types = Some(match shared_types {
                        None => creature_types,
                        Some(shared) => shared
                            .intersection(&creature_types)
                            .cloned()
                            .collect::<BTreeSet<_>>(),
                    });
                }
                if shared_types.is_none_or(|types| types.is_empty()) {
                    return Err(ExecutionError::IllegalTarget { id: target.id });
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
        TargetFilter::Opponent => {
            let controller = source_controller(state, context)?;
            state
                .player_ids()
                .into_iter()
                .filter(|player| *player != controller)
                .map(SelectedTarget::Player)
                .collect::<Vec<_>>()
        }
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
        let mut local = context.clone();
        local.source = record.source_identity;
        match &record.restriction {
            Restriction::TargetingProtection {
                object,
                forbidden_controller,
                duration,
            } => {
                if !restriction_duration_is_active(
                    state,
                    record.source_identity,
                    duration,
                    &local,
                )? {
                    continue;
                }
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
            Restriction::ObjectsCannotBeTargeted { objects, duration } => {
                if restriction_duration_is_active(
                    state,
                    record.source_identity,
                    duration,
                    &local,
                )? && selected.iter().any(|target| {
                    matches!(target, SelectedTarget::Object(id) if object_matches_filter(state, *id, objects, &local).unwrap_or(false))
                }) {
                    return Ok(true);
                }
            }
            Restriction::PlayersCannotBeTargeted { players, duration }
                if restriction_duration_is_active(
                    state,
                    record.source_identity,
                    duration,
                    &local,
                )? => {
                    let protected = resolve_players(state, players, &local)?;
                    if selected.iter().any(
                        |target| matches!(target, SelectedTarget::Player(id) if protected.contains(id)),
                    ) {
                        return Ok(true);
                    }
                }
            _ => {}
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
        Condition::SpellTargetsState {
            card_type,
            state: required,
        } => context
            .targets
            .values()
            .flatten()
            .any(|target| match target {
                SelectedTarget::Object(id) => state.object(*id).is_some_and(|object| {
                    object_has_type(&object, *card_type) && object_has_state(&object, required)
                }),
                SelectedTarget::Player(_) => false,
            }),
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
        Condition::CardWasCastUsingEscape => context.card_was_cast_using_escape,
        Condition::CardWasKicked => context.card_was_kicked,
        Condition::CardWasCastUsingTeamwork => context.card_was_cast_using_teamwork,
        Condition::YouAttackedThisTurn => context.you_attacked_this_turn,
        Condition::OpponentLostLifeThisTurn => context.opponent_lost_life_this_turn,
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
        Condition::HandCardCount {
            player,
            comparison,
            amount,
        } => {
            let players = resolve_players(state, player, context)?;
            let count = state
                .object_ids()
                .into_iter()
                .filter_map(|id| state.object(id))
                .filter(|object| object.zone == Zone::Hand && players.contains(&object.owner))
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
        Condition::SourceCounterCount {
            counter,
            comparison,
            amount,
        } => {
            let count = state
                .object(context.source)
                .ok_or(ExecutionError::MissingObject(context.source))?
                .counters
                .get(&counter_key(counter))
                .copied()
                .unwrap_or_default();
            compare_u32(count, *comparison, *amount)
        }
        Condition::ObjectCounterCount {
            object,
            counter,
            comparison,
            amount,
        } => resolve_objects(state, object, context)?
            .into_iter()
            .try_fold(false, |matched, object| {
                if matched {
                    return Ok(true);
                }
                let count = state
                    .object(object)
                    .ok_or(ExecutionError::MissingObject(object))?
                    .counters
                    .get(&counter_key(counter))
                    .copied()
                    .unwrap_or_default();
                Ok(compare_u32(count, *comparison, *amount))
            })?,
        Condition::CommanderControlled { .. } => context.commander_controlled,
        Condition::GiftPromised => context.gift_promised,
        Condition::SourceInOpeningHand => {
            context.source_was_in_opening_hand
                && state
                    .object(context.source)
                    .is_some_and(|object| object.zone == Zone::Hand)
        }
        Condition::NotPlayingFirst => !context.playing_first,
        Condition::ModeSelected(mode) => context.selected_modes.contains(mode),
        Condition::AnotherSpellCastThisTurn => context.spells_cast_by_actor_this_turn > 0,
        Condition::SpellsCastByActorThisTurn { comparison, amount } => {
            compare_u32(context.spells_cast_by_actor_this_turn, *comparison, *amount)
        }
        Condition::SourceAttackingAlone => context.source_attacking_alone,
        Condition::SpellCastFromNonHand => context
            .cast_from_zone
            .is_some_and(|zone| zone != Zone::Hand),
        Condition::ManaSpentGreaterThanSourcePowerOrToughness => {
            let source = state
                .object(context.source)
                .ok_or(ExecutionError::MissingObject(context.source))?;
            let spent = i64::from(context.mana_spent_to_cast_triggering_spell);
            spent > source.characteristics().power || spent > source.characteristics().toughness
        }
        Condition::CastOnlyDuringCombat => context.combat_step.is_some(),
        Condition::CastOnlyDuringCombatBeforeBlockers => matches!(
            context.combat_step,
            Some(CombatStep::Beginning | CombatStep::DeclareAttackers)
        ),
        Condition::CastOnlyDuringDeclareBlockers => {
            context.combat_step == Some(CombatStep::DeclareBlockers)
        }
        Condition::CastOnlyDuringCombatAfterBlockers => {
            context.blockers_declared
                && matches!(
                    context.combat_step,
                    Some(CombatStep::DeclareBlockers | CombatStep::CombatDamage | CombatStep::End)
                )
        }
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
        ObjectState::Blocking => object.blocking,
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
        CountExpression::Constant(value) => Ok(*value),
        CountExpression::OpponentCount { player } => {
            let referenced = resolve_players(state, player, context)?;
            let opponents = state
                .player_ids()
                .into_iter()
                .filter(|candidate| !referenced.contains(candidate))
                .count();
            u32::try_from(opponents).map_err(|_| ExecutionError::ArithmeticOverflow)
        }
        CountExpression::PartySize { player } => {
            let players = resolve_players(state, player, context)?;
            let mut candidates = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
            const PARTY_ROLES: [&str; 4] = ["Cleric", "Rogue", "Warrior", "Wizard"];
            for id in state.object_ids() {
                let Some(object) = state.object(id) else {
                    continue;
                };
                if object.zone != Zone::Battlefield
                    || !players.contains(&object.controller)
                    || !object_has_type(&object, CardType::Creature)
                {
                    continue;
                }
                for (role_index, role) in PARTY_ROLES.iter().enumerate() {
                    if object
                        .characteristics()
                        .subtypes
                        .iter()
                        .any(|subtype| subtype.eq_ignore_ascii_case(role))
                    {
                        candidates[role_index].push(id);
                    }
                }
            }
            fn maximum_distinct_roles(
                candidates: &[Vec<ObjectId>; 4],
                role: usize,
                used: &mut BTreeSet<ObjectId>,
            ) -> u32 {
                if role == candidates.len() {
                    return 0;
                }
                let mut best = maximum_distinct_roles(candidates, role + 1, used);
                for candidate in &candidates[role] {
                    if used.insert(*candidate) {
                        best = best.max(1 + maximum_distinct_roles(candidates, role + 1, used));
                        used.remove(candidate);
                    }
                }
                best
            }
            Ok(maximum_distinct_roles(&candidates, 0, &mut BTreeSet::new()))
        }
        CountExpression::DistinctBasicLandTypes { player } => {
            const BASIC_LAND_TYPES: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];
            let players = resolve_players(state, player, context)?;
            let mut present = BTreeSet::new();
            for id in state.object_ids() {
                let Some(object) = state.object(id) else {
                    continue;
                };
                if object.zone != Zone::Battlefield
                    || !players.contains(&object.controller)
                    || !object_has_type(&object, CardType::Land)
                {
                    continue;
                }
                for (index, basic_type) in BASIC_LAND_TYPES.iter().enumerate() {
                    if object
                        .characteristics()
                        .subtypes
                        .iter()
                        .any(|subtype| subtype.eq_ignore_ascii_case(basic_type))
                    {
                        present.insert(index);
                    }
                }
            }
            u32::try_from(present.len()).map_err(|_| ExecutionError::ArithmeticOverflow)
        }
        CountExpression::CreaturesAttackedThisTurn { .. } => {
            Ok(context.creatures_attacked_this_turn)
        }
        CountExpression::PowerOf { object } => {
            let objects = resolve_objects(state, object, context)?;
            if objects.len() != 1 {
                return Err(ExecutionError::InvalidAmount(
                    "power count requires exactly one object",
                ));
            }
            let id = objects[0];
            let power = state
                .object(id)
                .or_else(|| {
                    context
                        .last_known_source
                        .as_deref()
                        .filter(|source| source.id == id)
                        .cloned()
                })
                .ok_or(ExecutionError::MissingObject(id))?
                .characteristics()
                .power;
            u32::try_from(power.max(0)).map_err(|_| ExecutionError::ArithmeticOverflow)
        }
        CountExpression::ToughnessOf { object } => {
            let objects = resolve_objects(state, object, context)?;
            if objects.len() != 1 {
                return Err(ExecutionError::InvalidAmount(
                    "toughness count requires exactly one object",
                ));
            }
            let id = objects[0];
            let toughness = state
                .object(id)
                .or_else(|| {
                    context
                        .last_known_source
                        .as_deref()
                        .filter(|source| source.id == id)
                        .cloned()
                })
                .ok_or(ExecutionError::MissingObject(id))?
                .characteristics()
                .toughness;
            u32::try_from(toughness.max(0)).map_err(|_| ExecutionError::ArithmeticOverflow)
        }
        CountExpression::HalfLifeTotal { player, round_up } => {
            let players = resolve_players(state, player, context)?;
            let [player] = players.as_slice() else {
                return Err(ExecutionError::InvalidAmount(
                    "half-life count requires exactly one player",
                ));
            };
            let life = state
                .player(*player)
                .ok_or(ExecutionError::MissingPlayer(*player))?
                .life
                .max(0);
            let half = if *round_up {
                life.checked_add(1)
                    .ok_or(ExecutionError::ArithmeticOverflow)?
                    / 2
            } else {
                life / 2
            };
            u32::try_from(half).map_err(|_| ExecutionError::ArithmeticOverflow)
        }
        CountExpression::SelectedObjectsTotalPower { selection_id } => {
            let selected =
                context
                    .object_choices
                    .get(selection_id)
                    .ok_or(ExecutionError::InvalidAmount(
                        "selected-object power evidence is unavailable",
                    ))?;
            if selected.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "selected-object power requires at least one object",
                ));
            }
            let mut total = 0u32;
            for id in selected {
                let power = effective_object(state, *id, context)?
                    .characteristics()
                    .power;
                total = total
                    .checked_add(
                        u32::try_from(power.max(0))
                            .map_err(|_| ExecutionError::ArithmeticOverflow)?,
                    )
                    .ok_or(ExecutionError::ArithmeticOverflow)?;
            }
            Ok(total)
        }
        CountExpression::SelectedObjectsTotalToughness { selection_id } => {
            let selected =
                context
                    .object_choices
                    .get(selection_id)
                    .ok_or(ExecutionError::InvalidAmount(
                        "selected-object toughness evidence is unavailable",
                    ))?;
            if selected.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "selected-object toughness requires at least one object",
                ));
            }
            let mut total = 0u32;
            for id in selected {
                let toughness = effective_object(state, *id, context)?
                    .characteristics()
                    .toughness;
                total = total
                    .checked_add(
                        u32::try_from(toughness.max(0))
                            .map_err(|_| ExecutionError::ArithmeticOverflow)?,
                    )
                    .ok_or(ExecutionError::ArithmeticOverflow)?;
            }
            Ok(total)
        }
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
        CountExpression::AttachmentsOn { object, kind } => {
            let objects = resolve_objects(state, object, context)?;
            if objects.len() != 1 {
                return Err(ExecutionError::InvalidAmount(
                    "attachment count requires exactly one object",
                ));
            }
            Ok(state
                .object_ids()
                .into_iter()
                .filter_map(|source| state.attachment(source))
                .filter(|attachment| attachment.target == objects[0] && attachment.kind == *kind)
                .count() as u32)
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
        CountExpression::LifeLostThisWay {
            players,
            amount_each,
        } => {
            let player_count = u32::try_from(resolve_players(state, players, context)?.len())
                .map_err(|_| ExecutionError::ArithmeticOverflow)?;
            evaluate_amount(state, amount_each, context)?
                .checked_mul(player_count)
                .ok_or(ExecutionError::ArithmeticOverflow)
        }
        CountExpression::TriggerEventAmount => match &context.window {
            ActionWindow::Triggered(TriggerEvent::CombatDamageToPlayer { amount, .. })
            | ActionWindow::Triggered(TriggerEvent::DamageToPlayer { amount, .. })
            | ActionWindow::Triggered(TriggerEvent::CombatDamageToObject { amount, .. })
            | ActionWindow::Triggered(TriggerEvent::DamageToObject { amount, .. })
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
        Cost::Optional(cost) => {
            if context.payment_declined {
                Ok(())
            } else {
                pay_cost(state, cost, context)
            }
        }
        Cost::Mana(mana) => state
            .pay_mana(context.actor, mana, context.x_value)
            .map_err(ExecutionError::Adapter),
        Cost::AtomicResource(cost) => {
            let amount = atomic_energy_cost_amount(cost).ok_or(ExecutionError::InvalidAmount(
                "special resource cost has no executable production payment adapter",
            ))?;
            let mut player = state
                .player(context.actor)
                .ok_or(ExecutionError::MissingPlayer(context.actor))?;
            let current = player.counters.get("energy").copied().unwrap_or_default();
            let remaining = current
                .checked_sub(amount)
                .ok_or(ExecutionError::InvalidAmount(
                    "player lacks the required energy counters",
                ))?;
            if remaining == 0 {
                player.counters.remove("energy");
            } else {
                player.counters.insert("energy".to_owned(), remaining);
            }
            state.put_player(player).map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("pay_energy:{}:{amount}", context.actor));
            Ok(())
        }
        Cost::SacrificeSelection(selection) => {
            let objects = resolve_object_selection(state, selection, context)?;
            let controller = source_controller(state, context)?;
            for id in &objects {
                let candidate = state
                    .object(*id)
                    .ok_or(ExecutionError::MissingObject(*id))?;
                if candidate.zone != Zone::Battlefield
                    || candidate.controller != controller
                    || !object_matches_filter(state, *id, &selection.filter, context)?
                {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot be sacrificed for the selected cost"
                    )));
                }
            }
            for id in objects {
                state
                    .move_object(id, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("sacrifice_selected_cost:{id}"));
            }
            Ok(())
        }
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
        Cost::TapSelection(selection) => {
            let objects = resolve_object_selection(state, selection, context)?;
            for id in objects {
                let mut candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Battlefield
                    || candidate.controller != context.actor
                    || candidate.tapped
                {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot be tapped for this cost"
                    )));
                }
                candidate.tapped = true;
                state
                    .put_object(candidate)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("tap_selection_cost:{id}"));
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
        Cost::TapCreatureSelectionWithTotalPower { selection, minimum } => {
            let objects = resolve_object_selection(state, selection, context)?;
            let minimum = i64::from(evaluate_amount(state, minimum, context)?);
            let mut total = 0_i64;
            for id in &objects {
                let object = state
                    .object(*id)
                    .ok_or(ExecutionError::MissingObject(*id))?;
                if object.zone != Zone::Battlefield
                    || object.controller != context.actor
                    || object.tapped
                    || !object_has_type(&object, CardType::Creature)
                {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot be tapped for this total-power cost"
                    )));
                }
                total = total
                    .checked_add(object.characteristics().power.max(0))
                    .ok_or(ExecutionError::ArithmeticOverflow)?;
            }
            if total < minimum {
                return Err(ExecutionError::Adapter(
                    "selected creatures do not have enough total power".to_owned(),
                ));
            }
            for id in objects {
                let mut object = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                object.tapped = true;
                state.put_object(object).map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("tap_selected_power_cost:{id}"));
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
        Cost::DiscardRandom { player } => {
            let players = resolve_players(state, player, context)?;
            if players.len() != 1 || players[0] != context.actor {
                return Err(ExecutionError::InvalidAmount(
                    "random discard cost requires the casting player",
                ));
            }
            let player = players[0];
            let mut cards = state
                .object_ids()
                .into_iter()
                .filter(|id| {
                    state
                        .object(*id)
                        .is_some_and(|card| card.zone == Zone::Hand && card.owner == player)
                })
                .collect::<Vec<_>>();
            cards.sort_unstable();
            if cards.is_empty() {
                return Err(ExecutionError::Adapter(
                    "random discard cost requires a card in hand".to_owned(),
                ));
            }
            let index = (context.replay_seed % cards.len() as u64) as usize;
            let card = cards[index];
            state
                .move_object(card, Zone::Graveyard)
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("discard_random:{player}:{card}:{index}"));
            Ok(())
        }
        Cost::ReturnSelectionToHand(selection) => {
            let objects = resolve_object_selection(state, selection, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "return cost requires a permanent",
                ));
            }
            for id in objects {
                let candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Battlefield || candidate.controller != context.actor {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot be returned as a cost"
                    )));
                }
                state
                    .move_object(id, Zone::Hand)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("return_cost:{id}"));
            }
            Ok(())
        }
        Cost::RevealSelection {
            selection,
            optional,
        } => {
            if *optional && context.payment_declined {
                return Ok(());
            }
            let objects = resolve_object_selection(state, selection, context)?;
            let [card] = objects.as_slice() else {
                return Err(ExecutionError::InvalidAmount(
                    "reveal cost requires exactly one card",
                ));
            };
            let candidate = state
                .object(*card)
                .ok_or(ExecutionError::MissingObject(*card))?;
            if candidate.zone != Zone::Hand || candidate.owner != context.actor {
                return Err(ExecutionError::Adapter(format!(
                    "object {card} cannot be revealed as an additional cost"
                )));
            }
            let order = state.next_order();
            state.register_revealed_card(RevealedCardRecord {
                order,
                source_identity: context.source,
                player: context.actor,
                card: *card,
                as_additional_cost: true,
            });
            state.record_mutation(format!(
                "reveal_additional_cost:{}:{card}:{order}",
                context.actor
            ));
            Ok(())
        }
        Cost::BeholdSelection {
            choice_id,
            battlefield,
            hand,
        } => {
            let selected = context
                .object_choices
                .get(choice_id)
                .cloned()
                .unwrap_or_default();
            let [object] = selected.as_slice() else {
                return Err(ExecutionError::InvalidAmount(
                    "behold cost requires exactly one object",
                ));
            };
            let on_battlefield = object_matches_filter(state, *object, battlefield, context)?;
            let in_hand = object_matches_filter(state, *object, hand, context)?;
            if on_battlefield == in_hand {
                return Err(ExecutionError::InvalidAmount(
                    "behold choice must satisfy exactly one legal branch",
                ));
            }
            if in_hand {
                let order = state.next_order();
                state.register_revealed_card(RevealedCardRecord {
                    order,
                    source_identity: context.source,
                    player: context.actor,
                    card: *object,
                    as_additional_cost: true,
                });
                state.record_mutation(format!("behold_reveal:{}:{object}:{order}", context.actor));
            } else {
                state.record_mutation(format!("behold_controlled:{}:{object}", context.actor));
            }
            Ok(())
        }
        Cost::Waterbend { selection, amount } => {
            let objects = resolve_object_selection(state, selection, context)?;
            let required = evaluate_amount(state, amount, context)?;
            let contributed = u32::try_from(objects.len())
                .map_err(|_| ExecutionError::InvalidAmount("waterbend selection is too large"))?;
            if contributed > required {
                return Err(ExecutionError::InvalidAmount(
                    "waterbend selection contributes more than the chosen X",
                ));
            }
            for id in &objects {
                let mut object = state
                    .object(*id)
                    .ok_or(ExecutionError::MissingObject(*id))?;
                if object.zone != Zone::Battlefield
                    || object.controller != context.actor
                    || object.tapped
                    || !(object_has_type(&object, CardType::Artifact)
                        || object_has_type(&object, CardType::Creature))
                {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot contribute to waterbend"
                    )));
                }
                object.tapped = true;
                state.put_object(object).map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("waterbend_tap:{id}"));
            }
            let remaining = required - contributed;
            if remaining > 0 {
                state
                    .pay_mana(context.actor, &ManaCost(format!("{{{remaining}}}")), 0)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("waterbend_mana:{}:{remaining}", context.actor));
            }
            Ok(())
        }
        Cost::PutCounter {
            object,
            counter,
            amount,
        } => {
            let requested = evaluate_amount(state, amount, context)?;
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "put-counter cost has no object",
                ));
            }
            for id in objects {
                let mut candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Battlefield || candidate.controller != context.actor {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot receive a counter as this activation cost"
                    )));
                }
                let resolved = replace_counter_event(state, id, counter, requested, context)?;
                if resolved != requested {
                    return Err(ExecutionError::Adapter(
                        "a counter replacement prevented the exact activation cost".to_owned(),
                    ));
                }
                let key = counter_key(counter);
                let current = candidate.counters.get(&key).copied().unwrap_or_default();
                candidate.counters.insert(
                    key.clone(),
                    current
                        .checked_add(resolved)
                        .ok_or(ExecutionError::ArithmeticOverflow)?,
                );
                apply_counter_stat_delta(&mut candidate, counter, i64::from(resolved))?;
                state
                    .put_object(candidate)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("put_counter_cost:{id}:{key}:{resolved}"));
            }
            Ok(())
        }
        Cost::PutCounterSelection {
            selection,
            counter,
            amount,
        } => {
            let requested = evaluate_amount(state, amount, context)?;
            let objects = resolve_object_selection(state, selection, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "put-counter selection cost has no object",
                ));
            }
            for id in objects {
                let mut candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Battlefield || candidate.controller != context.actor {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot receive a counter as this additional cost"
                    )));
                }
                let resolved = replace_counter_event(state, id, counter, requested, context)?;
                if resolved != requested {
                    return Err(ExecutionError::Adapter(
                        "a counter replacement prevented the exact additional cost".to_owned(),
                    ));
                }
                let key = counter_key(counter);
                let current = candidate.counters.get(&key).copied().unwrap_or_default();
                candidate.counters.insert(
                    key.clone(),
                    current
                        .checked_add(resolved)
                        .ok_or(ExecutionError::ArithmeticOverflow)?,
                );
                apply_counter_stat_delta(&mut candidate, counter, i64::from(resolved))?;
                state
                    .put_object(candidate)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("put_counter_selection_cost:{id}:{key}:{resolved}"));
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
        Cost::ExileSourceFromBattlefield => {
            let source = state
                .object(context.source)
                .ok_or(ExecutionError::MissingObject(context.source))?;
            if source.zone != Zone::Battlefield || source.controller != context.actor {
                return Err(ExecutionError::Adapter(format!(
                    "source {} is not a battlefield permanent controlled by the activating player",
                    context.source
                )));
            }
            state
                .move_object(context.source, Zone::Exile)
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("exile_source_cost:{}", context.source));
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
        Cost::ExileSelectionWithTotalManaValue { selection, minimum } => {
            let objects = resolve_object_selection(state, selection, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "mana-value exile cost requires at least one card",
                ));
            }
            let required = evaluate_amount(state, minimum, context)?;
            let total = objects.iter().try_fold(0u32, |total, id| {
                let card = state
                    .object(*id)
                    .ok_or(ExecutionError::MissingObject(*id))?;
                total
                    .checked_add(card.characteristics().mana_value)
                    .ok_or(ExecutionError::ArithmeticOverflow)
            })?;
            if total < required {
                return Err(ExecutionError::InvalidAmount(
                    "exiled cards do not have enough total mana value",
                ));
            }
            for id in objects {
                state
                    .move_object(id, Zone::Exile)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("exile_mana_value_cost:{id}"));
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
        Cost::Optional(cost) => {
            context.payment_declined || cost_is_payable_by(state, cost, player, context)?
        }
        Cost::Mana(mana) => state.can_pay_mana(player, mana, context.x_value),
        Cost::AtomicResource(cost) => atomic_energy_cost_amount(cost).is_some_and(|amount| {
            state
                .player(player)
                .and_then(|player| player.counters.get("energy").copied())
                .unwrap_or_default()
                >= amount
        }),
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
        Cost::PutCounterSelection {
            selection,
            counter,
            amount,
        } => {
            let requested = evaluate_amount(state, amount, context)?;
            let objects = resolve_object_selection(state, selection, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|candidate| {
                        candidate.zone == Zone::Battlefield && candidate.controller == player
                    }) && replace_counter_event(state, *id, counter, requested, context)
                        .is_ok_and(|resolved| resolved == requested)
                })
        }
        Cost::TapSelection(selection) => {
            let objects = resolve_object_selection(state, selection, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|candidate| {
                        candidate.zone == Zone::Battlefield
                            && candidate.controller == player
                            && !candidate.tapped
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
        Cost::TapCreatureSelectionWithTotalPower { selection, minimum } => {
            let objects = resolve_object_selection(state, selection, context)?;
            let minimum = i64::from(evaluate_amount(state, minimum, context)?);
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|object| {
                        object.zone == Zone::Battlefield
                            && object.controller == player
                            && !object.tapped
                            && object_has_type(&object, CardType::Creature)
                    })
                })
                && objects
                    .into_iter()
                    .filter_map(|id| state.object(id))
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
        Cost::SacrificeSelection(selection) => {
            let objects = resolve_object_selection(state, selection, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|candidate| {
                        candidate.zone == Zone::Battlefield && candidate.controller == player
                    })
                })
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
        Cost::DiscardRandom { player: affected } => {
            resolve_players(state, affected, context)?.contains(&player)
                && state.object_ids().into_iter().any(|id| {
                    state
                        .object(id)
                        .is_some_and(|card| card.zone == Zone::Hand && card.owner == player)
                })
        }
        Cost::ReturnSelectionToHand(selection) => {
            let objects = resolve_object_selection(state, selection, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|candidate| {
                        candidate.zone == Zone::Battlefield && candidate.controller == player
                    })
                })
        }
        Cost::RevealSelection {
            selection,
            optional,
        } => {
            if *optional {
                true
            } else {
                let objects = resolve_object_selection(state, selection, context)?;
                objects.len() == 1
                    && state.object(objects[0]).is_some_and(|candidate| {
                        candidate.zone == Zone::Hand && candidate.owner == player
                    })
            }
        }
        Cost::BeholdSelection {
            choice_id,
            battlefield,
            hand,
        } => {
            let selected = context
                .object_choices
                .get(choice_id)
                .cloned()
                .unwrap_or_default();
            matches!(selected.as_slice(), [object]
                if object_matches_filter(state, *object, battlefield, context)?
                    ^ object_matches_filter(state, *object, hand, context)?)
        }
        Cost::Waterbend { selection, amount } => {
            let objects = resolve_object_selection(state, selection, context)?;
            let required = evaluate_amount(state, amount, context)?;
            let contributed = u32::try_from(objects.len()).unwrap_or(u32::MAX);
            contributed <= required
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|object| {
                        object.zone == Zone::Battlefield
                            && object.controller == player
                            && !object.tapped
                            && (object_has_type(&object, CardType::Artifact)
                                || object_has_type(&object, CardType::Creature))
                    })
                })
                && (contributed == required
                    || state.can_pay_mana(
                        player,
                        &ManaCost(format!("{{{}}}", required - contributed)),
                        0,
                    ))
        }
        Cost::PutCounter {
            object,
            counter,
            amount,
        } => {
            let requested = evaluate_amount(state, amount, context)?;
            let objects = resolve_objects(state, object, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state.object(*id).is_some_and(|candidate| {
                        candidate.zone == Zone::Battlefield && candidate.controller == player
                    }) && replace_counter_event(state, *id, counter, requested, context)
                        .is_ok_and(|resolved| resolved == requested)
                })
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
        Cost::ExileSourceFromBattlefield => state
            .object(context.source)
            .is_some_and(|source| source.zone == Zone::Battlefield && source.controller == player),
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
        Cost::ExileSelectionWithTotalManaValue { selection, minimum } => {
            let objects = resolve_object_selection(state, selection, context)?;
            let required = evaluate_amount(state, minimum, context)?;
            !objects.is_empty()
                && objects.iter().all(|id| {
                    state
                        .object(*id)
                        .is_some_and(|candidate| candidate.owner == player)
                })
                && objects
                    .iter()
                    .filter_map(|id| state.object(*id))
                    .try_fold(0u32, |total, card| {
                        total.checked_add(card.characteristics().mana_value)
                    })
                    .is_some_and(|total| total >= required)
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
        PlayerRef::OtherPlayer => {
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
                .or(match &context.window {
                    ActionWindow::Triggered(
                        TriggerEvent::SpellCast { player, .. }
                        | TriggerEvent::CardDrawn { player, .. }
                        | TriggerEvent::LifeGained { player, .. }
                        | TriggerEvent::TokenCreated { player, .. }
                        | TriggerEvent::PlayerAction { player, .. }
                        | TriggerEvent::CombatDamageToPlayer { player, .. }
                        | TriggerEvent::DamageToPlayer { player, .. },
                    ) => Some(*player),
                    ActionWindow::Triggered(TriggerEvent::BecameTarget { controller, .. }) => {
                        Some(*controller)
                    }
                    ActionWindow::Triggered(TriggerEvent::BeginningOf {
                        active_player, ..
                    }) => Some(*active_player),
                    _ => None,
                })
                .ok_or(ExecutionError::InvalidAmount("that player is unavailable"))?,
        ],
        PlayerRef::DefendingPlayer => {
            vec![
                context
                    .defending_player
                    .ok_or(ExecutionError::InvalidAmount(
                        "defending player is unavailable",
                    ))?,
            ]
        }
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
    let target_types = &target.characteristics().card_types;
    let target_kind_is_legal = match expected {
        AttachmentKind::Aura => target_types.iter().any(|card_type| {
            matches!(
                card_type,
                CardType::Artifact
                    | CardType::Battle
                    | CardType::Creature
                    | CardType::Enchantment
                    | CardType::Land
                    | CardType::Planeswalker
                    | CardType::Permanent
            )
        }),
        AttachmentKind::Equipment => target_types.contains(&CardType::Creature),
    };
    if attachment.source != source_id
        || attachment.kind != expected
        || source.zone != Zone::Battlefield
        || target.zone != Zone::Battlefield
        || !target_kind_is_legal
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
    if filter.chosen_name_of_source {
        let chosen = state
            .chosen_card_name(context.source)
            .filter(|name| !name.trim().is_empty())
            .ok_or(ExecutionError::InvalidAmount(
                "chosen card name is unavailable",
            ))?;
        if !object
            .characteristics()
            .names
            .iter()
            .any(|actual| actual.eq_ignore_ascii_case(chosen.trim()))
        {
            return Ok(false);
        }
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
    if filter.historic
        && !object_has_type(&object, CardType::Artifact)
        && !characteristics.supertypes.contains(&Supertype::Legendary)
        && !characteristics
            .subtypes
            .iter()
            .any(|subtype| subtype.eq_ignore_ascii_case("Saga"))
    {
        return Ok(false);
    }
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
    if filter.excluded_subtypes.iter().any(|subtype| {
        characteristics
            .subtypes
            .iter()
            .any(|actual| actual.eq_ignore_ascii_case(subtype))
    }) {
        return Ok(false);
    }
    if !filter
        .keywords
        .iter()
        .all(|keyword| characteristics.keywords.contains(keyword))
        || filter
            .excluded_keywords
            .iter()
            .any(|keyword| characteristics.keywords.contains(keyword))
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
    if filter
        .blocking
        .is_some_and(|blocking| blocking != object.blocking)
    {
        return Ok(false);
    }
    if let Some(kind) = filter.attached_by
        && !state
            .object_ids()
            .into_iter()
            .filter_map(|source| state.attachment(source))
            .any(|attachment| attachment.target == id && attachment.kind == kind)
    {
        return Ok(false);
    }
    if filter.other_than_source && object.id == context.source {
        return Ok(false);
    }
    if filter.targets_source
        && !context
            .targets
            .values()
            .flatten()
            .any(|target| matches!(target, SelectedTarget::Object(id) if *id == context.source))
    {
        return Ok(false);
    }
    if let Some((counter, minimum)) = &filter.minimum_counter
        && object
            .counters
            .get(&counter_key(counter))
            .copied()
            .unwrap_or_default()
            < *minimum
    {
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
    if let Some((comparison, amount)) = &filter.toughness {
        let expected = i64::from(evaluate_amount(state, amount, context)?);
        match comparison {
            Comparison::AtLeast if characteristics.toughness < expected => return Ok(false),
            Comparison::AtMost if characteristics.toughness > expected => return Ok(false),
            Comparison::Exactly if characteristics.toughness != expected => return Ok(false),
            Comparison::Greatest => {
                let mut comparison_filter = filter.clone();
                comparison_filter.toughness = None;
                let greatest = state
                    .object_ids()
                    .into_iter()
                    .filter_map(|candidate_id| {
                        if candidate_id == id {
                            return Some(characteristics.toughness);
                        }
                        object_matches_filter(state, candidate_id, &comparison_filter, context)
                            .ok()
                            .filter(|matched| *matched)
                            .and_then(|_| state.object(candidate_id))
                            .map(|candidate| candidate.characteristics().toughness)
                    })
                    .max()
                    .unwrap_or(characteristics.toughness);
                if characteristics.toughness != greatest || characteristics.toughness < expected {
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
        Effect::SetClassLevel { level } => {
            let mut source = state
                .object(context.source)
                .ok_or(ExecutionError::MissingObject(context.source))?;
            let is_class = source
                .characteristics()
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case("Class"));
            if !is_class
                || source.zone != Zone::Battlefield
                || source.controller != context.actor
                || source.class_level.saturating_add(1) != *level
            {
                return Err(ExecutionError::Adapter(format!(
                    "class level {} cannot advance to {level}",
                    source.class_level
                )));
            }
            source.class_level = *level;
            state.put_object(source).map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("class_level:{}:{level}", context.source));
            Ok(())
        }
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
                if candidate.zone != Zone::Stack || spell_cannot_be_countered(state, id, context)? {
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
        Effect::DestroyWithoutRegeneration { object } => {
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
                state.record_mutation(format!("destroy_no_regeneration:{id}"));
            }
            Ok(())
        }
        Effect::MoveZone(zone_move) => apply_zone_move(state, zone_move, context),
        Effect::MoveZoneUnderControl {
            object,
            from,
            to,
            controller,
            tapped,
            face_down,
            delayed_until,
        } => {
            let objects = resolve_objects(state, object, context)?
                .into_iter()
                .filter(|id| {
                    state
                        .object(*id)
                        .is_some_and(|candidate| candidate.zone == *from)
                })
                .collect::<Vec<_>>();
            if let Some(trigger) = delayed_until {
                let order = state.next_order();
                let effects = objects
                    .iter()
                    .map(|object_id| {
                        let identity = ObjectRef::ObjectIdentity(*object_id);
                        let controller = match controller {
                            PlayerRef::OwnerOf(_) => PlayerRef::OwnerOf(Box::new(identity.clone())),
                            PlayerRef::ControllerOf(_) => {
                                PlayerRef::ControllerOf(Box::new(identity.clone()))
                            }
                            controller => controller.clone(),
                        };
                        Effect::MoveZoneUnderControl {
                            object: identity,
                            from: *from,
                            to: *to,
                            controller,
                            tapped: *tapped,
                            face_down: *face_down,
                            delayed_until: None,
                        }
                    })
                    .collect();
                state.register_delayed_trigger(DelayedTriggerRecord {
                    order,
                    source_identity: context.source,
                    object_identities: objects,
                    trigger: trigger.clone(),
                    effects,
                });
                state.record_mutation(format!("delay_controlled_move:{}:{order}", context.source));
                return Ok(());
            }
            let controllers = resolve_players(state, controller, context)?;
            let [controller] = controllers.as_slice() else {
                return Err(ExecutionError::InvalidAmount(
                    "controlled zone move requires exactly one controller",
                ));
            };
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "controlled zone move has no physical object",
                ));
            }
            for id in objects {
                let candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != *from {
                    continue;
                }
                state
                    .move_object(id, *to)
                    .map_err(ExecutionError::Adapter)?;
                let mut candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                candidate.controller = *controller;
                candidate.tapped = *tapped;
                candidate.face_down = *face_down;
                state
                    .put_object(candidate)
                    .map_err(ExecutionError::Adapter)?;
                if *to == Zone::Battlefield {
                    apply_enters_replacements(state, id, context)?;
                }
                state.record_mutation(format!("move_under_control:{id}:{to:?}:{controller}"));
            }
            Ok(())
        }
        Effect::MoveToLibraryBottom { object } => {
            apply_move_to_library_bottom(state, object, context)
        }
        Effect::MoveToChosenLibraryEnd { object, choice_id } => {
            let objects = resolve_objects(state, object, context)?;
            let [id] = objects.as_slice() else {
                return Err(ExecutionError::InvalidAmount(
                    "top-or-bottom library move requires exactly one object",
                ));
            };
            let selection = context.library_end_choices.get(choice_id).copied().ok_or(
                ExecutionError::InvalidAmount("top-or-bottom library choice is missing"),
            )?;
            let owner = state
                .object(*id)
                .ok_or(ExecutionError::MissingObject(*id))?
                .owner;
            if selection.chooser != owner {
                return Err(ExecutionError::Adapter(format!(
                    "player {} cannot choose the library end for object {id} owned by {owner}",
                    selection.chooser
                )));
            }
            state
                .move_object(*id, Zone::Library)
                .map_err(ExecutionError::Adapter)?;
            if selection.choice == LibraryEndChoice::Bottom {
                let mut player = state
                    .player(owner)
                    .ok_or(ExecutionError::MissingPlayer(owner))?;
                player.library.retain(|candidate| candidate != id);
                player.library.push(*id);
                state.put_player(player).map_err(ExecutionError::Adapter)?;
            }
            state.record_mutation(format!(
                "library_end_choice:{owner}:{id}:{choice:?}:{choice_id}",
                choice = selection.choice
            ));
            Ok(())
        }
        Effect::PutOnLibraryTopInOrder { objects } => {
            let objects = resolve_objects(state, objects, context)?;
            for object in objects.iter().rev() {
                state
                    .move_object(*object, Zone::Library)
                    .map_err(ExecutionError::Adapter)?;
            }
            state.record_mutation(format!("put_on_library_top:{objects:?}"));
            Ok(())
        }
        Effect::PutInLibraryAtPosition {
            object,
            position_from_top,
        } => {
            let objects = resolve_objects(state, object, context)?;
            let [id] = objects.as_slice() else {
                return Err(ExecutionError::InvalidAmount(
                    "fixed-position library move requires exactly one object",
                ));
            };
            let owner = state
                .object(*id)
                .ok_or(ExecutionError::MissingObject(*id))?
                .owner;
            state
                .move_object(*id, Zone::Library)
                .map_err(ExecutionError::Adapter)?;
            let mut player = state
                .player(owner)
                .ok_or(ExecutionError::MissingPlayer(owner))?;
            player.library.retain(|candidate| candidate != id);
            let index = usize::from(position_from_top.saturating_sub(1)).min(player.library.len());
            player.library.insert(index, *id);
            state.put_player(player).map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!(
                "put_in_library_position:{owner}:{id}:{position_from_top}"
            ));
            Ok(())
        }
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
        Effect::CreateTokenAttached {
            creation,
            target,
            kind,
        } => {
            let created = apply_create_token(state, creation, context)?;
            let targets = resolve_objects(state, target, context)?;
            let ([source], [target]) = (created.as_slice(), targets.as_slice()) else {
                return Err(ExecutionError::InvalidAmount(
                    "attached token creation requires one token and one target",
                ));
            };
            state
                .set_attachment(AttachmentRecord {
                    source: *source,
                    target: *target,
                    kind: *kind,
                })
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("attach_created:{source}:{target}:{kind:?}"));
            Ok(())
        }
        Effect::CreateTokenAndAttachSource {
            creation,
            attachment,
            kind,
        } => {
            let created = apply_create_token(state, creation, context)?;
            let attachments = resolve_objects(state, attachment, context)?;
            let ([target], [source]) = (created.as_slice(), attachments.as_slice()) else {
                return Err(ExecutionError::InvalidAmount(
                    "token-and-source attachment requires one token and one attachment",
                ));
            };
            state
                .set_attachment(AttachmentRecord {
                    source: *source,
                    target: *target,
                    kind: *kind,
                })
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!(
                "attach_source_to_created:{source}:{target}:{kind:?}"
            ));
            Ok(())
        }
        Effect::Attach {
            attachment,
            target,
            kind,
        } => {
            let attachments = resolve_objects(state, attachment, context)?;
            let targets = resolve_objects(state, target, context)?;
            let ([source], [target]) = (attachments.as_slice(), targets.as_slice()) else {
                return Err(ExecutionError::InvalidAmount(
                    "attach requires one attachment and one target",
                ));
            };
            state
                .set_attachment(AttachmentRecord {
                    source: *source,
                    target: *target,
                    kind: *kind,
                })
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("attach:{source}:{target}:{kind:?}"));
            Ok(())
        }
        Effect::ResolveTargetChoice { object } => {
            let chosen = resolve_objects(state, object, context)?;
            let [object] = chosen.as_slice() else {
                return Err(ExecutionError::InvalidAmount(
                    "target-choice instruction requires exactly one object",
                ));
            };
            state.record_mutation(format!("resolve_target_choice:{object}"));
            Ok(())
        }
        Effect::ResolvePlayerTargetChoice { player } => {
            let chosen = resolve_players(state, player, context)?;
            let [player] = chosen.as_slice() else {
                return Err(ExecutionError::InvalidAmount(
                    "player-target-choice instruction requires exactly one player",
                ));
            };
            state.record_mutation(format!("resolve_player_target_choice:{player}"));
            Ok(())
        }
        Effect::ChoosePlayer { chooser, eligible } => {
            let selected = context
                .that_player
                .ok_or(ExecutionError::InvalidAmount("chosen player is missing"))?;
            if !resolve_players(state, chooser, context)?.contains(&context.actor)
                || !resolve_players(state, eligible, context)?.contains(&selected)
            {
                return Err(ExecutionError::InvalidAmount(
                    "chosen player or chooser is illegal",
                ));
            }
            state.record_mutation(format!("choose_player:{}:{selected}", context.actor));
            Ok(())
        }
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
        Effect::DrawRevealDiscardIfNonland { player } => {
            let players = resolve_players(state, player, context)?;
            if players.len() != 1 {
                return Err(ExecutionError::InvalidAmount(
                    "draw-reveal-discard requires exactly one player",
                ));
            }
            let player = players[0];
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
            let order = state.next_order();
            state.register_revealed_card(RevealedCardRecord {
                order,
                source_identity: context.source,
                player,
                card,
                as_additional_cost: false,
            });
            state.record_mutation(format!("reveal_card:{player}:{card}:{order}"));
            let drawn = state
                .object(card)
                .ok_or(ExecutionError::MissingObject(card))?;
            if !drawn.characteristics().card_types.contains(&CardType::Land) {
                state
                    .move_object(card, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("discard:{player}:{card}"));
            }
            Ok(())
        }
        Effect::DrawThenDiscardUnless {
            player,
            draw,
            discard,
            alternative,
            choice_id,
        } => {
            let players = resolve_players(state, player, context)?;
            if players.len() != 1 {
                return Err(ExecutionError::InvalidAmount(
                    "draw-then-discard requires exactly one player",
                ));
            }
            let player = players[0];
            draw_cards(state, player, u32::from(*draw))?;
            let selected = context
                .object_choices
                .get(choice_id)
                .cloned()
                .unwrap_or_default();
            if selected.iter().copied().collect::<BTreeSet<_>>().len() != selected.len() {
                return Err(ExecutionError::InvalidAmount(
                    "discard choice contains duplicate cards",
                ));
            }
            let all_in_hand = selected.iter().all(|object| {
                state
                    .object(*object)
                    .is_some_and(|card| card.owner == player && card.zone == Zone::Hand)
            });
            let alternative_selected = selected.len() == 1
                && object_matches_filter(state, selected[0], alternative, context)?;
            if !all_in_hand || (!alternative_selected && selected.len() != usize::from(*discard)) {
                return Err(ExecutionError::InvalidAmount(
                    "draw-then-discard choice does not satisfy either legal payment",
                ));
            }
            for object in selected {
                state
                    .move_object(object, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("discard:{player}:{object}"));
            }
            Ok(())
        }
        Effect::ChooseCardName { nonland } => {
            let chosen = context
                .selected_card_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .ok_or(ExecutionError::InvalidAmount(
                    "selected card name is unavailable",
                ))?;
            if *nonland && context.selected_card_name_is_nonland != Some(true) {
                return Err(ExecutionError::InvalidAmount(
                    "selected card name is not proven to be nonland",
                ));
            }
            state
                .set_chosen_card_name(context.source, chosen.to_owned())
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("choose_card_name:{}:{chosen}", context.source));
            Ok(())
        }
        Effect::ChooseColor => {
            let chosen = context
                .chosen_color
                .ok_or(ExecutionError::InvalidAmount("chosen color is unavailable"))?;
            state
                .set_chosen_color(context.source, chosen)
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("choose_color:{}:{chosen:?}", context.source));
            Ok(())
        }
        Effect::RollDie { sides } => {
            if *sides < 2 {
                return Err(ExecutionError::InvalidAmount(
                    "a die must have at least two sides",
                ));
            }
            let result =
                u16::try_from(mix64(context.replay_seed ^ context.source) % u64::from(*sides) + 1)
                    .map_err(|_| ExecutionError::ArithmeticOverflow)?;
            state
                .set_die_roll(context.source, *sides, result)
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("roll_die:{}:d{sides}:{result}", context.source));
            Ok(())
        }
        Effect::FlipCoin => {
            let result = if mix64(context.replay_seed ^ context.source) & 1 == 0 {
                CoinFlipResult::Won
            } else {
                CoinFlipResult::Lost
            };
            state
                .set_coin_flip(context.source, result)
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("flip_coin:{}:{result:?}", context.source));
            Ok(())
        }
        Effect::Proliferate { choice_id } => {
            let objects = context
                .object_choices
                .get(choice_id)
                .cloned()
                .unwrap_or_default();
            let players = context
                .player_choices
                .get(choice_id)
                .cloned()
                .unwrap_or_default();
            if objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
                || players.iter().collect::<BTreeSet<_>>().len() != players.len()
            {
                return Err(ExecutionError::InvalidAmount(
                    "proliferate choices contain duplicates",
                ));
            }

            let mut updated_objects = Vec::with_capacity(objects.len());
            for object_id in objects {
                let mut object = state
                    .object(object_id)
                    .ok_or(ExecutionError::MissingObject(object_id))?;
                let is_permanent = object.characteristics().card_types.iter().any(|card_type| {
                    matches!(
                        card_type,
                        CardType::Artifact
                            | CardType::Battle
                            | CardType::Creature
                            | CardType::Enchantment
                            | CardType::Land
                            | CardType::Planeswalker
                            | CardType::Permanent
                    )
                });
                if object.zone != Zone::Battlefield || !is_permanent || object.counters.is_empty() {
                    return Err(ExecutionError::InvalidAmount(
                        "proliferate object must be a permanent with a counter",
                    ));
                }
                for amount in object.counters.values_mut() {
                    *amount = amount
                        .checked_add(1)
                        .ok_or(ExecutionError::ArithmeticOverflow)?;
                }
                updated_objects.push(object);
            }

            let mut updated_players = Vec::with_capacity(players.len());
            for player_id in players {
                let mut player = state
                    .player(player_id)
                    .ok_or(ExecutionError::MissingPlayer(player_id))?;
                if player.counters.is_empty() {
                    return Err(ExecutionError::InvalidAmount(
                        "proliferate player must have a counter",
                    ));
                }
                for amount in player.counters.values_mut() {
                    *amount = amount
                        .checked_add(1)
                        .ok_or(ExecutionError::ArithmeticOverflow)?;
                }
                updated_players.push(player);
            }

            for object in updated_objects {
                let object_id = object.id;
                state.put_object(object).map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("proliferate_object:{object_id}"));
            }
            for player in updated_players {
                let player_id = player.id;
                state.put_player(player).map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("proliferate_player:{player_id}"));
            }
            Ok(())
        }
        Effect::InitializeIntensity { amount } => {
            if state
                .initialize_intensity(context.source, *amount)
                .map_err(ExecutionError::Adapter)?
            {
                state.record_mutation(format!("initialize_intensity:{}:{amount}", context.source));
            }
            Ok(())
        }
        Effect::ChooseNamedOption { options } => {
            let [selected] = context.selected_modes.as_slice() else {
                return Err(ExecutionError::InvalidAmount(
                    "named entry choice requires exactly one selected option",
                ));
            };
            let option = options
                .get(usize::from(*selected))
                .ok_or(ExecutionError::InvalidAmount(
                    "named entry choice is outside the printed options",
                ))?
                .clone();
            state
                .set_chosen_option(context.source, option.clone())
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("choose_named_option:{}:{option}", context.source));
            Ok(())
        }
        Effect::ChooseRulesText { kind } => {
            let value = context
                .selected_rules_text
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or(ExecutionError::InvalidAmount(
                    "selected rules-text value is unavailable",
                ))?;
            state
                .set_chosen_rules_text(context.source, *kind, value.to_owned())
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!(
                "choose_rules_text:{}:{kind:?}:{value}",
                context.source
            ));
            Ok(())
        }
        Effect::EstablishDayIfUnset => {
            if state.establish_day_if_unset() {
                state.record_mutation("establish_day".to_owned());
            }
            Ok(())
        }
        Effect::EachPlayerSacrifices { filter, amount } => {
            let mut sacrifices = Vec::new();
            for player in state.player_ids() {
                let mut local_filter = filter.clone();
                local_filter.controller = Some(PlayerRef::PlayerIdentity(player));
                let legal = matching_objects(state, &local_filter, context)?;
                let required = usize::from(*amount).min(legal.len());
                let selected = context
                    .per_player_object_choices
                    .get(&player)
                    .cloned()
                    .unwrap_or_default();
                if selected.len() != required
                    || selected.iter().copied().collect::<BTreeSet<_>>().len() != selected.len()
                    || selected.iter().any(|object| !legal.contains(object))
                {
                    return Err(ExecutionError::InvalidAmount(
                        "each-player sacrifice choice is illegal",
                    ));
                }
                sacrifices.extend(selected);
            }
            for object in sacrifices {
                state
                    .move_object(object, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("each_player_sacrifice:{object}"));
            }
            Ok(())
        }
        Effect::PlayersSacrifice {
            players,
            filter,
            amount,
        } => {
            let mut sacrifices = Vec::new();
            for player in resolve_players(state, players, context)? {
                let mut local_filter = filter.clone();
                local_filter.controller = Some(PlayerRef::PlayerIdentity(player));
                let legal = matching_objects(state, &local_filter, context)?;
                let required = usize::from(*amount).min(legal.len());
                let selected = context
                    .per_player_object_choices
                    .get(&player)
                    .cloned()
                    .unwrap_or_default();
                if selected.len() != required
                    || selected.iter().copied().collect::<BTreeSet<_>>().len() != selected.len()
                    || selected.iter().any(|object| !legal.contains(object))
                {
                    return Err(ExecutionError::InvalidAmount(
                        "scoped-player sacrifice choice is illegal",
                    ));
                }
                sacrifices.extend(selected);
            }
            for object in sacrifices {
                state
                    .move_object(object, Zone::Graveyard)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("scoped_player_sacrifice:{object}"));
            }
            Ok(())
        }
        Effect::RevealHand { player } => {
            for player in resolve_players(state, player, context)? {
                let mut cards = state
                    .object_ids()
                    .into_iter()
                    .filter(|id| {
                        state.object(*id).is_some_and(|object| {
                            object.owner == player && object.zone == Zone::Hand
                        })
                    })
                    .collect::<Vec<_>>();
                cards.sort_unstable();
                let order = state.next_order();
                state.register_revealed_hand(RevealHandRecord {
                    order,
                    source_identity: context.source,
                    player,
                    cards,
                });
                state.record_mutation(format!("reveal_hand:{player}:{order}"));
            }
            Ok(())
        }
        Effect::LookAtHand { viewer, player } => {
            let viewers = resolve_players(state, viewer, context)?;
            let players = resolve_players(state, player, context)?;
            if viewers.len() != 1 || players.len() != 1 {
                return Err(ExecutionError::InvalidAmount(
                    "private hand inspection requires exactly one viewer and one player",
                ));
            }
            let viewer = viewers[0];
            let player = players[0];
            let mut cards = state
                .object_ids()
                .into_iter()
                .filter(|id| {
                    state
                        .object(*id)
                        .is_some_and(|object| object.owner == player && object.zone == Zone::Hand)
                })
                .collect::<Vec<_>>();
            cards.sort_unstable();
            let order = state.next_order();
            state.register_hand_inspection(HandInspectionRecord {
                order,
                source_identity: context.source,
                viewer,
                player,
                cards,
            });
            state.record_mutation(format!(
                "look_at_hand:{viewer}:{player}:{}:{order}",
                context.source
            ));
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
        Effect::Connive { object, discard } => apply_connive(state, object, discard, context),
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
        Effect::RemoveFromCombat { object } => {
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "remove-from-combat has no physical object",
                ));
            }
            for id in objects {
                let mut candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                candidate.attacking = false;
                candidate.blocking = false;
                state
                    .put_object(candidate)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("remove_from_combat:{id}"));
            }
            Ok(())
        }
        Effect::Exert { object } => {
            let object_identities = resolve_objects(state, object, context)?;
            let [exerted] = object_identities.as_slice() else {
                return Err(ExecutionError::InvalidAmount(
                    "exert requires exactly one physical object",
                ));
            };
            let order = state.next_order();
            state.register_next_untap_prevention(NextUntapPreventionRecord {
                order,
                source_identity: context.source,
                object_identities: object_identities.clone(),
            });
            state.record_mutation(format!("exert:{}:{exerted}:{order}", context.actor));
            Ok(())
        }
        Effect::PreventNextUntap { object } => {
            let object_identities = resolve_objects(state, object, context)?;
            if object_identities.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "next-untap prevention has no physical object",
                ));
            }
            let order = state.next_order();
            state.register_next_untap_prevention(NextUntapPreventionRecord {
                order,
                source_identity: context.source,
                object_identities,
            });
            state.record_mutation(format!("register_next_untap:{}:{order}", context.source));
            Ok(())
        }
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
        Effect::PutPlayerCounter {
            player,
            counter,
            amount,
        } => {
            let amount = evaluate_amount(state, amount, context)?;
            for player_id in resolve_players(state, player, context)? {
                let mut player_state = state
                    .player(player_id)
                    .ok_or(ExecutionError::MissingPlayer(player_id))?;
                let key = counter.to_ascii_lowercase();
                let current = player_state.counters.get(&key).copied().unwrap_or(0);
                player_state.counters.insert(
                    key.clone(),
                    current
                        .checked_add(amount)
                        .ok_or(ExecutionError::ArithmeticOverflow)?,
                );
                state
                    .put_player(player_state)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("player_counter:{player_id}:{key}:{amount}"));
            }
            Ok(())
        }
        Effect::PutCounter {
            object,
            counter,
            amount,
        } => apply_put_counter(state, object, counter, amount, context),
        Effect::RemoveCounter {
            object,
            counter,
            amount,
        } => apply_remove_counter(state, object, counter, amount, context),
        Effect::MoveAllCounters { from, to } => {
            let sources = resolve_objects(state, from, context)?;
            let destinations = resolve_objects(state, to, context)?;
            let ([source], [destination]) = (sources.as_slice(), destinations.as_slice()) else {
                return Err(ExecutionError::InvalidAmount(
                    "counter transfer requires one source and one destination",
                ));
            };
            let mut source_object = state
                .object(*source)
                .ok_or(ExecutionError::MissingObject(*source))?;
            let mut destination_object = state
                .object(*destination)
                .ok_or(ExecutionError::MissingObject(*destination))?;
            for (counter, amount) in &source_object.counters {
                let current = destination_object
                    .counters
                    .get(counter)
                    .copied()
                    .unwrap_or(0);
                destination_object.counters.insert(
                    counter.clone(),
                    current
                        .checked_add(*amount)
                        .ok_or(ExecutionError::ArithmeticOverflow)?,
                );
            }
            source_object.counters.clear();
            state
                .put_object(source_object)
                .map_err(ExecutionError::Adapter)?;
            state
                .put_object(destination_object)
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!("move_all_counters:{source}:{destination}"));
            Ok(())
        }
        Effect::ModifyPowerToughness(change) => {
            apply_power_toughness_change(state, change, context)
        }
        Effect::GrantKeyword {
            objects,
            keywords,
            duration,
        } => {
            let ids = resolve_objects(state, objects, context)?;
            if *duration != Duration::Permanent {
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
            if *duration != Duration::Permanent {
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
            if *duration != Duration::Permanent {
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
        Effect::SetCreatureTypeToChoice { object, duration } => {
            let chosen = context
                .chosen_creature_type
                .as_ref()
                .filter(|chosen| !chosen.trim().is_empty())
                .ok_or(ExecutionError::InvalidAmount(
                    "chosen creature type is unavailable",
                ))?
                .trim()
                .to_owned();
            apply_set_characteristics(
                state,
                &SetCharacteristics {
                    object: object.clone(),
                    colors: None,
                    card_types: None,
                    subtypes: Some(vec![chosen]),
                    name: None,
                    base_power: None,
                    base_toughness: None,
                    retain_other_card_types: true,
                    retain_other_subtypes: false,
                    retain_other_colors: true,
                    retain_other_names: true,
                    duration: duration.clone(),
                },
                context,
            )
        }
        Effect::SetColorToChoice { object, duration } => {
            let chosen = context
                .chosen_color
                .ok_or(ExecutionError::InvalidAmount("chosen color is unavailable"))?;
            apply_set_characteristics(
                state,
                &SetCharacteristics {
                    object: object.clone(),
                    colors: Some(vec![chosen]),
                    card_types: None,
                    subtypes: None,
                    name: None,
                    base_power: None,
                    base_toughness: None,
                    retain_other_card_types: true,
                    retain_other_subtypes: true,
                    retain_other_colors: false,
                    retain_other_names: true,
                    duration: duration.clone(),
                },
                context,
            )
        }
        Effect::SetBasicLandTypeToChoice { object, duration } => {
            let (subtype, color) = match context
                .chosen_basic_land_type
                .as_deref()
                .map(str::trim)
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("plains") => ("Plains", Color::White),
                Some("island") => ("Island", Color::Blue),
                Some("swamp") => ("Swamp", Color::Black),
                Some("mountain") => ("Mountain", Color::Red),
                Some("forest") => ("Forest", Color::Green),
                _ => {
                    return Err(ExecutionError::InvalidAmount(
                        "chosen basic land type is unavailable or illegal",
                    ));
                }
            };
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "basic land type effect has no physical object",
                ));
            }
            for object_id in objects {
                let object_ref = ObjectRef::ObjectIdentity(object_id);
                let mana_ability = GrantedAbility {
                    costs: vec![Cost::Tap(ObjectRef::Source)],
                    effects: vec![Effect::AddMana(ManaProduction {
                        player: PlayerRef::ControllerOf(Box::new(ObjectRef::Source)),
                        choices: vec![ManaChoice {
                            symbols: vec![color],
                        }],
                        amount: Amount::Constant(1),
                        commander_identity_only: false,
                        scales_with: None,
                        typed: None,
                    })],
                };
                for derived in [
                    Effect::LoseAllAbilities {
                        object: object_ref.clone(),
                        duration: duration.clone(),
                    },
                    Effect::SetCharacteristics(SetCharacteristics {
                        object: object_ref.clone(),
                        colors: None,
                        card_types: None,
                        subtypes: Some(vec![subtype.to_owned()]),
                        name: None,
                        base_power: None,
                        base_toughness: None,
                        retain_other_card_types: true,
                        retain_other_subtypes: false,
                        retain_other_colors: true,
                        retain_other_names: true,
                        duration: duration.clone(),
                    }),
                    Effect::GrantAbility {
                        objects: object_ref,
                        ability: mana_ability,
                        duration: duration.clone(),
                    },
                ] {
                    apply_effect(state, &derived, context)?;
                }
            }
            Ok(())
        }
        Effect::Restriction(restriction) => {
            let attachment_object = match restriction {
                Restriction::DoesNotUntapDuring { object, .. }
                | Restriction::DoesNotUntapDuringIf { object, .. }
                | Restriction::ActivatedAbilitiesCannotBeActivated { object, .. }
                | Restriction::MustAttackEachCombatIfAble { object, .. }
                | Restriction::CannotAttack { object, .. }
                | Restriction::CannotBlock { object, .. }
                | Restriction::CannotBeBlocked { object, .. }
                    if matches!(object, ObjectRef::AttachmentTarget { .. }) =>
                {
                    Some(object)
                }
                Restriction::BlockerMustMatch { attacker, .. }
                    if matches!(attacker, ObjectRef::AttachmentTarget { .. }) =>
                {
                    Some(attacker)
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
        Effect::Prepare { object } => {
            for id in resolve_objects(state, object, context)? {
                let mut candidate = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
                if candidate.zone != Zone::Battlefield {
                    return Err(ExecutionError::Adapter(format!(
                        "object {id} cannot become prepared outside the battlefield"
                    )));
                }
                candidate.prepared = true;
                state
                    .put_object(candidate)
                    .map_err(ExecutionError::Adapter)?;
                state.record_mutation(format!("prepare:{id}"));
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
        Effect::RevealTop { player, amount } => {
            let amount = evaluate_amount(state, amount, context)? as usize;
            for player in resolve_players(state, player, context)? {
                let cards = state
                    .player(player)
                    .ok_or(ExecutionError::MissingPlayer(player))?
                    .library
                    .into_iter()
                    .take(amount)
                    .collect::<Vec<_>>();
                for card in &cards {
                    let order = state.next_order();
                    state.register_revealed_card(RevealedCardRecord {
                        order,
                        source_identity: context.source,
                        player,
                        card: *card,
                        as_additional_cost: false,
                    });
                    state.record_mutation(format!("reveal_card:{player}:{card}:{order}"));
                }
                set_looked_at(state, player, cards)?;
            }
            Ok(())
        }
        Effect::SelectFromLookedAt {
            player,
            amount,
            predicate,
            reveal,
            face_down,
            tapped,
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
                    *face_down,
                    *tapped,
                    *destination,
                    context,
                )?;
            }
            Ok(())
        }
        Effect::PutRestOnLibraryBottom { player, order } => {
            for player in resolve_players(state, player, context)? {
                put_rest_on_bottom(state, player, *order, context)?;
            }
            Ok(())
        }
        Effect::PutRestOfLookedAt {
            player,
            destination,
        } => {
            for player in resolve_players(state, player, context)? {
                let cards = state.looked_at(player);
                for card in &cards {
                    state
                        .move_object(*card, *destination)
                        .map_err(ExecutionError::Adapter)?;
                }
                state.put_looked_at(player, Vec::new());
                state.record_mutation(format!(
                    "put_rest_looked_at:{player}:{destination:?}:{}",
                    cards.len()
                ));
            }
            Ok(())
        }
        Effect::ReorderLookedAtOnLibraryTop { player } => {
            for player in resolve_players(state, player, context)? {
                reorder_looked_at_on_top(state, player, context)?;
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
        Effect::ChangeControlUntil {
            object,
            controller,
            duration,
        } => {
            let objects = resolve_objects(state, object, context)?;
            if objects.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "temporary control effect has no physical object",
                ));
            }
            for object in objects {
                register_continuous_effect(
                    state,
                    context,
                    vec![object],
                    Effect::ChangeControlUntil {
                        object: ObjectRef::ObjectIdentity(object),
                        controller: controller.clone(),
                        duration: duration.clone(),
                    },
                    duration.clone(),
                );
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
                condition: None,
            });
            state.record_mutation(format!("spell_reduction:{}:{order}", context.source));
            Ok(())
        }
        Effect::ReduceSpellCostWhen {
            object,
            mana,
            condition,
        } => {
            let order = state.next_order();
            state.register_spell_reduction(SpellReductionRecord {
                order,
                source_identity: context.source,
                object: object.clone(),
                mana: mana.clone(),
                per: CountExpression::Constant(1),
                maximum_reduction: None,
                condition: Some(condition.clone()),
            });
            state.record_mutation(format!(
                "conditional_spell_reduction:{}:{order}",
                context.source
            ));
            Ok(())
        }
        Effect::IncreaseSpellCost { object, mana, per } => {
            let order = state.next_order();
            state.register_spell_increase(SpellIncreaseRecord {
                order,
                source_identity: context.source,
                object: object.clone(),
                mana: mana.clone(),
                per: per.clone(),
            });
            state.record_mutation(format!("spell_increase:{}:{order}", context.source));
            Ok(())
        }
        Effect::ChooseMode { count } => {
            if !choice_count_matches(count, &context.selected_modes, context) {
                return Err(ExecutionError::InvalidAmount("mode choice is illegal"));
            }
            state.record_mutation(format!(
                "choose_mode:{}:{:?}",
                context.source, context.selected_modes
            ));
            Ok(())
        }
        Effect::ChooseModeBy { chooser, count } => {
            if !choice_count_matches(count, &context.selected_modes, context) {
                return Err(ExecutionError::InvalidAmount("mode choice is illegal"));
            }
            let selected_chooser = context
                .selected_mode_chooser
                .ok_or(ExecutionError::InvalidAmount("mode chooser is missing"))?;
            if !resolve_players(state, chooser, context)?.contains(&selected_chooser) {
                return Err(ExecutionError::InvalidAmount("mode chooser is illegal"));
            }
            state.record_mutation(format!(
                "choose_mode_by:{}:{selected_chooser}:{:?}",
                context.source, context.selected_modes
            ));
            Ok(())
        }
        Effect::ChooseModeNotPreviouslyChosen { count } => {
            if !choice_count_matches(count, &context.selected_modes, context)
                || context
                    .selected_modes
                    .iter()
                    .any(|mode| context.previously_selected_modes.contains(mode))
            {
                return Err(ExecutionError::InvalidAmount(
                    "mode was already chosen or the choice count is illegal",
                ));
            }
            state.record_mutation(format!(
                "choose_new_mode:{}:{:?}",
                context.source, context.selected_modes
            ));
            Ok(())
        }
        Effect::ChooseModeFrom {
            count,
            option_count,
        } => {
            if !choice_count_matches(count, &context.selected_modes, context)
                || context
                    .selected_modes
                    .iter()
                    .any(|mode| *mode >= *option_count)
            {
                return Err(ExecutionError::InvalidAmount("mode choice is illegal"));
            }
            state.record_mutation(format!(
                "choose_mode_from:{}:{option_count}:{:?}",
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
        LibraryProcedure::ShuffleGraveyardIntoLibrary { player } => {
            for player in resolve_players(state, player, context)? {
                let cards = state
                    .object_ids()
                    .into_iter()
                    .filter(|id| {
                        state.object(*id).is_some_and(|object| {
                            object.zone == Zone::Graveyard && object.owner == player
                        })
                    })
                    .collect::<Vec<_>>();
                for card in cards {
                    state
                        .move_object(card, Zone::Library)
                        .map_err(ExecutionError::Adapter)?;
                }
                deterministic_shuffle(state, player, context.replay_seed)?;
            }
            Ok(())
        }
        LibraryProcedure::ShuffleHandIntoLibraryAndDrawSame { player } => {
            for player in resolve_players(state, player, context)? {
                let hand = state
                    .object_ids()
                    .into_iter()
                    .filter_map(|id| state.object(id))
                    .filter(|object| object.zone == Zone::Hand && object.owner == player)
                    .map(|object| object.id)
                    .collect::<Vec<_>>();
                let draw = u32::try_from(hand.len())
                    .map_err(|_| ExecutionError::InvalidAmount("hand size overflow"))?;
                for card in hand {
                    state
                        .move_object(card, Zone::Library)
                        .map_err(ExecutionError::Adapter)?;
                }
                deterministic_shuffle(state, player, context.replay_seed)?;
                draw_cards(state, player, draw)?;
            }
            Ok(())
        }
        LibraryProcedure::ShuffleHandAndGraveyardIntoLibraryAndDraw { player, amount } => {
            let draw = evaluate_amount(state, amount, context)?;
            for player in resolve_players(state, player, context)? {
                let cards = state
                    .object_ids()
                    .into_iter()
                    .filter_map(|id| state.object(id))
                    .filter(|object| {
                        object.owner == player
                            && matches!(object.zone, Zone::Hand | Zone::Graveyard)
                    })
                    .map(|object| object.id)
                    .collect::<Vec<_>>();
                for card in cards {
                    state
                        .move_object(card, Zone::Library)
                        .map_err(ExecutionError::Adapter)?;
                }
                deterministic_shuffle(state, player, context.replay_seed)?;
                draw_cards(state, player, draw)?;
            }
            Ok(())
        }
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
        LibraryProcedure::DiscardHandsAndDrawDiscarded { player, adjustment } => {
            for player in resolve_players(state, player, context)? {
                let hand = state
                    .object_ids()
                    .into_iter()
                    .filter_map(|id| state.object(id))
                    .filter(|object| object.zone == Zone::Hand && object.owner == player)
                    .map(|object| object.id)
                    .collect::<Vec<_>>();
                let draw = i64::try_from(hand.len())
                    .map_err(|_| ExecutionError::InvalidAmount("hand size overflow"))?
                    .saturating_add(i64::from(*adjustment))
                    .max(0);
                let draw = u32::try_from(draw)
                    .map_err(|_| ExecutionError::InvalidAmount("draw count overflow"))?;
                for card in hand {
                    state
                        .move_object(card, Zone::Graveyard)
                        .map_err(ExecutionError::Adapter)?;
                }
                draw_cards(state, player, draw)?;
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
                loop {
                    let Some(card) = state
                        .player(player)
                        .ok_or(ExecutionError::MissingPlayer(player))?
                        .library
                        .first()
                        .copied()
                    else {
                        break;
                    };
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
                loop {
                    let Some(card) = state
                        .player(player)
                        .ok_or(ExecutionError::MissingPlayer(player))?
                        .library
                        .first()
                        .copied()
                    else {
                        break;
                    };
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
                state.chosen_color(context.source),
                context.mana_production_choice.as_deref(),
            )?
        } else {
            if let Some(requested) = context.mana_production_choice.as_deref() {
                production
                    .choices
                    .iter()
                    .find(|choice| {
                        choice.symbols == requested
                            && (!production.commander_identity_only
                                || choice
                                    .symbols
                                    .iter()
                                    .all(|color| player_state.commander_identity.contains(color)))
                    })
                    .map(|choice| choice.symbols.clone())
                    .ok_or(ExecutionError::InvalidAmount(
                        "requested mana production choice is not legal",
                    ))?
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
            }
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
    chosen_color: Option<Color>,
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
        TypedManaComposition::Alternatives(alternatives) => {
            if alternatives.is_empty() {
                return Err(ExecutionError::InvalidAmount(
                    "typed mana production has no printed alternative",
                ));
            }
            if let Some(requested) = requested {
                return alternatives
                    .iter()
                    .find_map(|alternative| {
                        choose_typed_mana_composition(
                            alternative,
                            amount as u32,
                            commander_identity,
                            chosen_color,
                            Some(requested),
                        )
                        .ok()
                    })
                    .ok_or(ExecutionError::InvalidAmount(
                        "the requested mana choice is not one of the printed alternatives",
                    ));
            }
            alternatives
                .iter()
                .find_map(|alternative| {
                    choose_typed_mana_composition(
                        alternative,
                        amount as u32,
                        commander_identity,
                        chosen_color,
                        None,
                    )
                    .ok()
                })
                .ok_or(ExecutionError::InvalidAmount(
                    "no printed mana alternative is currently available",
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
        TypedManaComposition::Derived(TypedDerivedManaTypes::ChosenColor) => {
            let chosen = chosen_color.ok_or(ExecutionError::InvalidAmount(
                "chosen color is unavailable for mana production",
            ))?;
            choose_repeated_mana_color(&[chosen], amount, requested)
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

fn apply_move_to_library_bottom<S: OracleStateAdapter>(
    state: &mut S,
    object: &ObjectRef,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let objects = resolve_objects(state, object, context)?;
    if objects.is_empty() {
        return Err(ExecutionError::InvalidAmount(
            "library-bottom move has no physical object",
        ));
    }
    for id in objects {
        let owner = state
            .object(id)
            .ok_or(ExecutionError::MissingObject(id))?
            .owner;
        state
            .move_object(id, Zone::Library)
            .map_err(ExecutionError::Adapter)?;
        let mut player = state
            .player(owner)
            .ok_or(ExecutionError::MissingPlayer(owner))?;
        player.library.retain(|candidate| *candidate != id);
        player.library.push(id);
        state.put_player(player).map_err(ExecutionError::Adapter)?;
        state.record_mutation(format!("library_bottom:{owner}:{id}"));
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
        TargetAmount::AnyNumber => true,
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
        if !tapped {
            attempt_direct_untap(state, object)?;
            continue;
        }
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
        if !state
            .object(record.source_identity)
            .is_some_and(|source| source.zone == Zone::Battlefield)
        {
            continue;
        }
        let mut local = context.clone();
        local.source = record.source_identity;
        match &record.effect {
            ReplacementEffect::EntersTapped(replacement)
                if resolve_objects(state, &replacement.object, &local)?
                    .contains(&entering_object) =>
            {
                let applies = match &replacement.when {
                    Some(condition) => condition_holds(state, condition, &local)?,
                    None => true,
                };
                let exempt = match &replacement.unless {
                    Some(condition) => condition_holds(state, condition, &local)?,
                    None => false,
                };
                if applies && !exempt {
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
            | ReplacementEffect::IncreaseEvent { .. }
            | ReplacementEffect::ConditionalTokenSubstitution { .. }
            | ReplacementEffect::EntersTapped(_)
            | ReplacementEffect::EnterAsCopy(_) => {}
        }
    }
    for record in sorted_restrictions(state) {
        let Restriction::EntersUntapped { objects, duration } = &record.restriction else {
            continue;
        };
        let mut local = context.clone();
        local.source = record.source_identity;
        if restriction_duration_is_active(state, record.source_identity, duration, &local)?
            && object_matches_filter(state, entering_object, objects, &local)?
        {
            let mut candidate = state
                .object(entering_object)
                .ok_or(ExecutionError::MissingObject(entering_object))?;
            candidate.tapped = false;
            state
                .put_object(candidate)
                .map_err(ExecutionError::Adapter)?;
            state.record_mutation(format!(
                "restriction:{}:enters_untapped:{entering_object}",
                record.order
            ));
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
        let events = match &creation.specification {
            TokenSpecification::CopyOf(original) => {
                let originals = resolve_objects(state, original, context)?;
                if originals.is_empty() {
                    return Err(ExecutionError::InvalidAmount("copy token has no original"));
                }
                originals
                    .into_iter()
                    .map(|original| TokenSpecification::CopyOf(ObjectRef::ObjectIdentity(original)))
                    .collect::<Vec<_>>()
            }
            specification => vec![specification.clone()],
        };
        for event in events {
            let (amount, specification) =
                replace_token_event(state, player, base_amount, &event, context)?;
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
            ReplacementEffect::IncreaseEvent { .. } => {}
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
        blocking: false,
        prepared: false,
        face_down: false,
        active_face: 0,
        class_level: if definition
            .subtypes
            .iter()
            .any(|subtype| subtype.eq_ignore_ascii_case("Class"))
        {
            1
        } else {
            0
        },
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
    object.blocking = false;
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

fn apply_connive<S: OracleStateAdapter>(
    state: &mut S,
    object: &ObjectRef,
    discard: &ObjectSelection,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let players = resolve_players(state, &discard.chooser, context)?;
    let [player] = players.as_slice() else {
        return Err(ExecutionError::Adapter(
            "connive requires exactly one object's controller".to_owned(),
        ));
    };
    draw_cards(state, *player, 1)?;

    let selected = resolve_object_selection(state, discard, context)?;
    let [discarded] = selected.as_slice() else {
        return Err(ExecutionError::InvalidAmount(
            "connive must discard exactly one card",
        ));
    };
    let discarded_card = state
        .object(*discarded)
        .ok_or(ExecutionError::MissingObject(*discarded))?;
    let discarded_nonland = !discarded_card
        .characteristics()
        .card_types
        .contains(&CardType::Land);
    state
        .move_object(*discarded, Zone::Graveyard)
        .map_err(ExecutionError::Adapter)?;
    state.record_mutation(format!("connive_discard:{player}:{discarded}"));

    if discarded_nonland {
        apply_put_counter(
            state,
            object,
            &CounterKind::PlusOnePlusOne,
            &Amount::Constant(1),
            context,
        )?;
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
        if !tapped {
            attempt_direct_untap(state, id)?;
            continue;
        }
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

fn attempt_direct_untap<S: OracleStateAdapter>(
    state: &mut S,
    id: ObjectId,
) -> Result<bool, ExecutionError> {
    let mut object = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
    if object.zone != Zone::Battlefield {
        return Err(ExecutionError::Adapter(format!(
            "object {id} is not on the battlefield"
        )));
    }
    let stun_counters = object.counters.get("stun").copied().unwrap_or(0);
    if stun_counters > 0 {
        if stun_counters == 1 {
            object.counters.remove("stun");
        } else {
            object.counters.insert("stun".to_owned(), stun_counters - 1);
        }
        state.put_object(object).map_err(ExecutionError::Adapter)?;
        state.record_mutation(format!("consume_stun_counter:{id}"));
        return Ok(false);
    }
    object.tapped = false;
    state.put_object(object).map_err(ExecutionError::Adapter)?;
    state.record_mutation(format!("set_tapped:{id}:false"));
    Ok(true)
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
        apply_counter_stat_delta(&mut object, counter, i64::from(resolved))?;
        state.put_object(object).map_err(ExecutionError::Adapter)?;
        state.record_mutation(format!("counter:{id}:{key}:{resolved}"));
    }
    Ok(())
}

fn apply_remove_counter<S: OracleStateAdapter>(
    state: &mut S,
    reference: &ObjectRef,
    counter: &CounterKind,
    amount: &Amount,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let requested = evaluate_amount(state, amount, context)?;
    let objects = resolve_objects(state, reference, context)?;
    if objects.is_empty() {
        return Err(ExecutionError::InvalidAmount(
            "counter removal has no object",
        ));
    }
    for id in objects {
        let mut object = state.object(id).ok_or(ExecutionError::MissingObject(id))?;
        let key = counter_key(counter);
        let current = object.counters.get(&key).copied().unwrap_or(0);
        let removed = current.min(requested);
        let remaining = current - removed;
        if remaining == 0 {
            object.counters.remove(&key);
        } else {
            object.counters.insert(key.clone(), remaining);
        }
        apply_counter_stat_delta(&mut object, counter, -i64::from(removed))?;
        state.put_object(object).map_err(ExecutionError::Adapter)?;
        state.record_mutation(format!("remove_counter:{id}:{key}:{removed}"));
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
        match &record.effect {
            ReplacementEffect::MultiplyEvent { event, multiplier }
                if replacement_event_matches(state, event, &occurrence, &local) =>
            {
                amount = amount
                    .checked_mul(u32::from(*multiplier))
                    .ok_or(ExecutionError::ArithmeticOverflow)?;
            }
            ReplacementEffect::IncreaseEvent { event, addend }
                if replacement_event_matches(state, event, &occurrence, &local) =>
            {
                amount = amount
                    .checked_add(u32::from(*addend))
                    .ok_or(ExecutionError::ArithmeticOverflow)?;
            }
            _ => {}
        }
    }
    Ok(amount)
}

fn apply_counter_stat_delta(
    object: &mut PhysicalObject,
    counter: &CounterKind,
    counter_delta: i64,
) -> Result<(), ExecutionError> {
    let stat_delta = match counter {
        CounterKind::PlusOnePlusOne => counter_delta,
        CounterKind::MinusOneMinusOne => -counter_delta,
        CounterKind::Loyalty | CounterKind::Indestructible | CounterKind::Named(_) => return Ok(()),
    };
    object.characteristics_mut().power = object
        .characteristics()
        .power
        .checked_add(stat_delta)
        .ok_or(ExecutionError::ArithmeticOverflow)?;
    object.characteristics_mut().toughness = object
        .characteristics()
        .toughness
        .checked_add(stat_delta)
        .ok_or(ExecutionError::ArithmeticOverflow)?;
    Ok(())
}

fn counter_key(counter: &CounterKind) -> String {
    match counter {
        CounterKind::PlusOnePlusOne => "+1/+1".to_owned(),
        CounterKind::MinusOneMinusOne => "-1/-1".to_owned(),
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
    let objects = resolve_objects(state, &change.objects, context)?;
    if change.duration != Duration::Permanent {
        let registered = if matches!(
            change.duration,
            Duration::ThisTurn
                | Duration::UntilEndOfTurn
                | Duration::BeginningOfNextEndStep
                | Duration::BeginningOfNextTurnUpkeep
        ) {
            let mut frozen = change.clone();
            frozen.power = Amount::Constant(evaluate_amount(state, &change.power, context)?);
            frozen.toughness =
                Amount::Constant(evaluate_amount(state, &change.toughness, context)?);
            frozen
        } else {
            change.clone()
        };
        register_continuous_effect(
            state,
            context,
            objects,
            Effect::ModifyPowerToughness(registered),
            change.duration.clone(),
        );
        return Ok(());
    }
    let power = i64::from(evaluate_amount(state, &change.power, context)?);
    let toughness = i64::from(evaluate_amount(state, &change.toughness, context)?);
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
        PowerToughnessOperation::SetPower => characteristics.power = power,
        PowerToughnessOperation::SetToughness => characteristics.toughness = toughness,
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
        PowerToughnessOperation::Switch => {
            std::mem::swap(&mut characteristics.power, &mut characteristics.toughness);
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
    if change.duration != Duration::Permanent {
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
    face_down: bool,
    tapped: bool,
    destination: Zone,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let looked = state.looked_at(player);
    let selected = if amount == 1
        && let Some(chosen) = context.accepted_library_card
    {
        if !looked.contains(&chosen)
            || !object_matches_filter(state, chosen, predicate, context).unwrap_or(false)
        {
            return Err(ExecutionError::Adapter(
                "chosen looked-at card must be present and match the predicate".to_owned(),
            ));
        }
        vec![chosen]
    } else {
        looked
            .iter()
            .copied()
            .filter(|id| object_matches_filter(state, *id, predicate, context).unwrap_or(false))
            .take(amount)
            .collect::<Vec<_>>()
    };
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
        if face_down {
            let mut object = state
                .object(*id)
                .ok_or(ExecutionError::MissingObject(*id))?;
            object.face_down = true;
            state.put_object(object).map_err(ExecutionError::Adapter)?;
        }
        if tapped {
            let mut object = state
                .object(*id)
                .ok_or(ExecutionError::MissingObject(*id))?;
            object.tapped = true;
            state.put_object(object).map_err(ExecutionError::Adapter)?;
        }
        state.record_mutation(format!(
            "select_looked:{player}:{id}:{destination:?}:{reveal}:{face_down}:{tapped}"
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
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let mut rest = state.looked_at(player);
    match order {
        BottomOrder::AnyOrder => rest.sort_unstable(),
        BottomOrder::ReplaySeededRandom => {
            rest.sort_by_key(|id| mix64(context.replay_seed ^ *id ^ u64::from(player)));
        }
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

fn reorder_looked_at_on_top<S: OracleStateAdapter>(
    state: &mut S,
    player: PlayerId,
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    let looked = state.looked_at(player);
    let ordered = if let Some(choice) = context.library_choices.get(&player) {
        if !choice.move_away.is_empty() {
            return Err(ExecutionError::Adapter(
                "top-card reordering cannot move an examined card away".to_owned(),
            ));
        }
        let chosen = choice.keep_on_top.clone();
        let chosen_set = chosen.iter().copied().collect::<BTreeSet<_>>();
        let looked_set = looked.iter().copied().collect::<BTreeSet<_>>();
        if chosen.len() != looked.len()
            || chosen_set.len() != chosen.len()
            || chosen_set != looked_set
        {
            return Err(ExecutionError::Adapter(
                "top-card order must contain every examined card exactly once".to_owned(),
            ));
        }
        chosen
    } else {
        looked.clone()
    };

    let mut player_state = state
        .player(player)
        .ok_or(ExecutionError::MissingPlayer(player))?;
    let prefix = player_state
        .library
        .iter()
        .take(looked.len())
        .copied()
        .collect::<BTreeSet<_>>();
    if prefix != looked.iter().copied().collect::<BTreeSet<_>>() {
        return Err(ExecutionError::Adapter(
            "examined cards are no longer the top cards of the library".to_owned(),
        ));
    }
    let mut library = ordered;
    library.extend(player_state.library.iter().skip(looked.len()).copied());
    player_state.library = library;
    state
        .put_player(player_state)
        .map_err(ExecutionError::Adapter)?;
    state.put_looked_at(player, Vec::new());
    state.record_mutation(format!("reorder_top:{player}:{}", looked.len()));
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
