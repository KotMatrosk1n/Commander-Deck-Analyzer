//! A bounded, fail-closed Oracle-clause compiler.
//!
//! The compiler in this module is intentionally card-name independent. Card
//! names are accepted only as source identity context so printed self
//! references can be normalized to [`ObjectRef::Source`]. A clause is claimed
//! only when one grammar path consumes the complete normalized paragraph,
//! including any reminder text.
//!
//! This module contains no simulator state and performs no mutation itself.
//! Its output is a deterministic, typed contract for a bounded live consumer.

use std::fmt;

use sha2::{Digest, Sha256};

use crate::ability_clause_bridge::{
    ABILITY_CLAUSE_BRIDGE_COMPILER_VERSION, ABILITY_CLAUSE_BRIDGE_RUNTIME_VERSION,
    AbilityClauseBridgeProgram,
};
use crate::alternate_zone_cast_keyword_runtime::{
    ALTERNATE_ZONE_CAST_COMPILER_VERSION, ALTERNATE_ZONE_CAST_RULES_CONTEXT_VERSION,
    ALTERNATE_ZONE_CAST_RUNTIME_VERSION, AlternateZoneKeywordKind, AlternateZoneKeywordProgram,
    compile_alternate_zone_cast_keyword_program,
};
use crate::attachment_entry_runtime::{
    ATTACHMENT_ENTRY_COMPILER_VERSION, ATTACHMENT_ENTRY_RUNTIME_VERSION, AttachmentEntryProgram,
    compile_attachment_entry_program,
};
use crate::attachment_filter_runtime::{
    ATTACHMENT_FILTER_COMPILER_VERSION, ATTACHMENT_FILTER_RULES_CONTEXT_VERSION,
    ATTACHMENT_FILTER_RUNTIME_VERSION, AttachmentFilterCompilerInput, AttachmentFilterProgram,
    AttachmentFilterProgramKind, compile_attachment_filter_program,
};
use crate::bounded_oracle_mana::{
    BOUNDED_ORACLE_MANA_EXPRESSION_VERSION, DerivedManaTypes as TypedDerivedManaTypes,
    ManaColor as TypedManaColor, ManaColorDomain as TypedManaColorDomain,
    ManaComposition as TypedManaComposition, ManaExpressionError,
    ManaProductionExpression as TypedManaProductionExpression, ManaQuantity as TypedManaQuantity,
    ManaRetention as TypedManaRetention, ManaSymbol as TypedManaSymbol,
    ResourceCostComponent as TypedResourceCostComponent, ResourceCostExpression,
    parse_mana_production_expression, parse_mana_retention_clause,
    parse_mana_spend_restriction_clause, parse_resource_cost_expression,
};

use crate::cast_choice_keyword_runtime::{
    CAST_CHOICE_KEYWORD_COMPILER_VERSION, CAST_CHOICE_KEYWORD_RULES_CONTEXT_VERSION,
    CAST_CHOICE_KEYWORD_RUNTIME_VERSION, CastChoiceKeywordProgram, CastChoiceSourceContext,
    compile_cast_choice_keyword_program,
};
use crate::cast_modifier_keyword_runtime::{
    CAST_MODIFIER_KEYWORD_COMPILER_VERSION, CAST_MODIFIER_KEYWORD_RULES_CONTEXT_VERSION,
    CAST_MODIFIER_KEYWORD_RUNTIME_VERSION, CastModifierKeywordKind, CastModifierKeywordProgram,
    compile_cast_modifier_keyword_program,
};
use crate::combat_restriction_runtime::{
    COMBAT_RESTRICTION_COMPILER_VERSION, COMBAT_RESTRICTION_RUNTIME_VERSION,
    CombatRestrictionProgram, compile_combat_restriction_program,
};
use crate::combat_special_keyword_runtime::{
    COMBAT_SPECIAL_KEYWORD_COMPILER_VERSION, COMBAT_SPECIAL_KEYWORD_RULES_CONTEXT_VERSION,
    COMBAT_SPECIAL_KEYWORD_RUNTIME_VERSION, CombatSpecialKeywordKind, CombatSpecialKeywordProgram,
    compile_combat_special_keyword_program,
};
use crate::combat_trigger_keyword_runtime::{
    COMBAT_TRIGGER_KEYWORD_COMPILER_VERSION, COMBAT_TRIGGER_KEYWORD_RULES_CONTEXT_VERSION,
    COMBAT_TRIGGER_KEYWORD_RUNTIME_VERSION, CombatTriggerKeywordKind, CombatTriggerKeywordProgram,
    compile_combat_trigger_keyword_program, reviewed_combat_trigger_normalized_source,
};
use crate::common_action_procedure_runtime::{
    COMMON_ACTION_PROCEDURE_COMPILER_VERSION, COMMON_ACTION_PROCEDURE_RULES_CONTEXT_VERSION,
    COMMON_ACTION_PROCEDURE_RUNTIME_VERSION, CommonActionProgram, compile_common_action_program,
    reviewed_common_action_normalized_source,
};
use crate::creature_counter_keyword_runtime::{
    CREATURE_COUNTER_COMPILER_VERSION, CREATURE_COUNTER_RULES_CONTEXT_VERSION,
    CREATURE_COUNTER_RUNTIME_VERSION, CreatureCounterKeywordProgram,
    compile_creature_counter_keyword_program,
};
use crate::damage_clause_compiler::{
    CompiledDamageClause, DAMAGE_CLAUSE_COMPILER_VERSION, DamageClauseInput, compile_damage_clause,
};
use crate::damage_transaction_runtime::DAMAGE_TRANSACTION_RUNTIME_VERSION;
use crate::delayed_counter_keyword_runtime::{
    DELAYED_COUNTER_KEYWORD_COMPILER_VERSION, DELAYED_COUNTER_KEYWORD_RUNTIME_VERSION,
    DELAYED_COUNTER_RULES_CONTEXT_VERSION, DelayedCounterKeywordProgram,
    compile_delayed_counter_keyword_program,
};
use crate::entry_choice_keyword_runtime::{
    ENTRY_CHOICE_KEYWORD_COMPILER_VERSION, ENTRY_CHOICE_KEYWORD_RULES_CONTEXT_VERSION,
    ENTRY_CHOICE_KEYWORD_RUNTIME_VERSION, EntryChoiceKeywordProgram,
    compile_entry_choice_keyword_program,
};
use crate::extended_cast_zone_keyword_runtime::{
    EXTENDED_CAST_ZONE_COMPILER_VERSION, EXTENDED_CAST_ZONE_RULES_CONTEXT_VERSION,
    EXTENDED_CAST_ZONE_RUNTIME_VERSION, ExtendedCastZoneProgram,
    SnapshotCandidateClass as ExtendedCastZoneSnapshotCandidateClass,
    classify_extended_cast_zone_snapshot_candidate, compile_extended_cast_zone_keyword_program,
};
use crate::face_down_merge_keyword_runtime::{
    FACE_DOWN_MERGE_KEYWORD_COMPILER_VERSION, FACE_DOWN_MERGE_KEYWORD_RULES_CONTEXT_VERSION,
    FACE_DOWN_MERGE_KEYWORD_RUNTIME_VERSION, FaceDownMergeKeywordProgram,
    SnapshotCandidateClass as FaceDownMergeSnapshotCandidateClass,
    classify_snapshot_candidate as classify_face_down_merge_snapshot_candidate,
    compile_face_down_merge_keyword_program,
};
use crate::graveyard_hand_library_keyword_runtime::{
    SourceSemanticContext as ZoneKeywordSourceSemanticContext, ZONE_KEYWORD_COMPILER_VERSION,
    ZONE_KEYWORD_RULES_CONTEXT_VERSION, ZONE_KEYWORD_RUNTIME_VERSION, ZoneKeywordProgram,
    compile_zone_keyword_program,
};
use crate::graveyard_transform_keyword_runtime::{
    GRAVEYARD_TRANSFORM_KEYWORD_COMPILER_VERSION,
    GRAVEYARD_TRANSFORM_KEYWORD_RULES_CONTEXT_VERSION, GRAVEYARD_TRANSFORM_KEYWORD_RUNTIME_VERSION,
    GraveyardTransformKeywordKind, GraveyardTransformKeywordProgram,
    SourceSemanticContext as GraveyardTransformSourceSemanticContext,
    compile_graveyard_transform_keyword_program,
};
use crate::keyword_rules_runtime::{KeywordProgram, KeywordProgramInput, compile_keyword_program};
use crate::level_progression_runtime::{
    LEVEL_PROGRESSION_COMPILER_VERSION, LEVEL_PROGRESSION_RULES_CONTEXT_VERSION,
    LEVEL_PROGRESSION_RUNTIME_VERSION, LevelProgressionProgram,
};
use crate::library_access_runtime::{
    LIBRARY_ACCESS_COMPILER_VERSION, LIBRARY_ACCESS_RUNTIME_VERSION, LibraryAccessProgram,
    compile_library_access_program,
};
use crate::linked_cast_cost_keyword_runtime::{
    LINKED_CAST_COST_COMPILER_VERSION, LINKED_CAST_COST_RULES_CONTEXT_VERSION,
    LINKED_CAST_COST_RUNTIME_VERSION, LinkedCastCostProgram,
    compile_linked_cast_cost_keyword_program,
};
use crate::object_state_clause_runtime::{
    OBJECT_STATE_CLAUSE_COMPILER_VERSION, OBJECT_STATE_CLAUSE_RUNTIME_VERSION,
    OBJECT_STATE_RULES_CONTEXT_VERSION, ObjectStateClauseKind, ObjectStateClauseProgram,
    compile_object_state_clause_program,
};
use crate::old_transform_runtime::{
    OLD_TRANSFORM_COMPILER_VERSION, OLD_TRANSFORM_RUNTIME_VERSION, OldTransformProgram,
    compile_old_transform_program,
};
use crate::oracle_ability_envelope_runtime::{
    AbilityEnvelopeCompileInput, ORACLE_ABILITY_ENVELOPE_COMPILER_VERSION,
    ORACLE_ABILITY_ENVELOPE_RULES_CONTEXT_VERSION, ORACLE_ABILITY_ENVELOPE_RUNTIME_VERSION,
    OracleAbilityEnvelopeProgram, compile_oracle_ability_envelope,
};
use crate::oracle_action_algebra_runtime::{
    ORACLE_ACTION_ALGEBRA_COMPILER_VERSION, ORACLE_ACTION_ALGEBRA_RULES_CONTEXT_VERSION,
    ORACLE_ACTION_ALGEBRA_RUNTIME_VERSION, OracleActionCompileInput, OracleActionProgram,
    compile_oracle_action_program,
};
use crate::oracle_cast_zone_envelope_runtime::{
    CastZoneEnvelopeProgram, ORACLE_CAST_ZONE_ENVELOPE_COMPILER_VERSION,
    ORACLE_CAST_ZONE_ENVELOPE_RULES_CONTEXT_VERSION, ORACLE_CAST_ZONE_ENVELOPE_RUNTIME_VERSION,
    compile_cast_zone_envelope_program,
};
use crate::oracle_clause_composition::{
    ORACLE_CLAUSE_COMPOSITION_COMPILER_VERSION, ORACLE_CLAUSE_COMPOSITION_RULES_CONTEXT_VERSION,
    ORACLE_CLAUSE_COMPOSITION_RUNTIME_VERSION, SemanticCapability, SourceSpan,
    TypedOracleComposition,
};
use crate::oracle_clause_syntax::{
    OracleClauseSyntaxError, ValidatedOracleClauseLine, validate_oracle_clause_line,
};
use crate::oracle_face_program_assembler::{
    AssembledModalProgram, ORACLE_FACE_MODAL_RULES_CONTEXT_VERSION,
    ORACLE_FACE_PROGRAM_ASSEMBLER_COMPILER_VERSION, ORACLE_FACE_PROGRAM_ASSEMBLER_RUNTIME_VERSION,
};
use crate::oracle_static_replacement_runtime::{
    ORACLE_STATIC_REPLACEMENT_COMPILER_VERSION, ORACLE_STATIC_REPLACEMENT_RULES_CONTEXT_VERSION,
    ORACLE_STATIC_REPLACEMENT_RUNTIME_VERSION, OracleStaticReplacementCompileInput,
    OracleStaticReplacementProgram, OracleStaticReplacementProgramKind,
    compile_oracle_static_replacement_program,
};
use crate::pregame_clause_runtime::{
    PREGAME_CLAUSE_COMPILER_VERSION, PREGAME_CLAUSE_RUNTIME_VERSION, PREGAME_RULES_CONTEXT_VERSION,
    PregameClauseKind, PregameClauseProgram, compile_pregame_clause_program,
};
use crate::regeneration_action_runtime::{
    REGENERATION_ACTION_COMPILER_VERSION, REGENERATION_ACTION_RULES_CONTEXT_VERSION,
    REGENERATION_ACTION_RUNTIME_VERSION, RegenerationActionProgram,
    compile_regeneration_action_program,
};
use crate::residual_cost_keyword_runtime::{
    RESIDUAL_COST_KEYWORD_COMPILER_VERSION, RESIDUAL_COST_KEYWORD_RULES_CONTEXT_VERSION,
    RESIDUAL_COST_KEYWORD_RUNTIME_VERSION, ResidualCostKeywordKind, ResidualCostKeywordProgram,
    compile_residual_cost_keyword_program,
};
use crate::saga_transform_runtime::{
    SAGA_TRANSFORM_COMPILER_VERSION, SAGA_TRANSFORM_RUNTIME_VERSION, SagaTransformFaceRole,
    SagaTransformLayoutKind, SagaTransformProgram, SagaTransformSourceContext,
    compile_saga_transform_program,
};
use crate::special_resource_runtime::{
    SPECIAL_RESOURCE_RUNTIME_VERSION, SpecialCostSymbol, SpecialResourceCost,
};
use crate::standalone_oracle_annotation::{
    STANDALONE_ORACLE_ANNOTATION_COMPILER_VERSION, STANDALONE_ORACLE_ANNOTATION_RUNTIME_VERSION,
    StandaloneOracleAnnotation, StandaloneOracleAnnotationKind,
    compile_standalone_oracle_annotation,
};
use crate::static_special_keyword_runtime::{
    STATIC_SPECIAL_KEYWORD_COMPILER_VERSION, STATIC_SPECIAL_KEYWORD_RUNTIME_VERSION,
    StaticSpecialKeywordProgram, StaticSpecialSourceContext,
    compile_static_special_keyword_program, reviewed_static_special_normalized_source,
};
use crate::targeting_protection_runtime::{
    ProtectionRecipient, TARGETING_PROTECTION_COMPILER_VERSION,
    TARGETING_PROTECTION_RULES_CONTEXT_VERSION, TARGETING_PROTECTION_RUNTIME_VERSION,
    TargetingProtectionKind, TargetingProtectionProgram, compile_targeting_protection_program,
};

pub const BOUNDED_ORACLE_COMPILER_VERSION: &str = "bounded-oracle-compiler-0.53";
pub const BOUNDED_ORACLE_RUNTIME_VERSION: &str = "bounded-oracle-runtime-0.29";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClauseAddress {
    pub face_index: u16,
    pub clause_index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleClauseInput<'a> {
    pub face_index: u16,
    pub clause_index: u16,
    pub source_name: &'a str,
    pub source_type_line: &'a str,
    pub oracle_clause: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedOracleCardContext<'a> {
    pub layout: &'a str,
    pub face_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleFaceInput<'a> {
    pub face_index: u16,
    pub source_name: &'a str,
    pub source_type_line: &'a str,
    pub oracle_clauses: &'a [&'a str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedOracleFace {
    pub runtime_version: &'static str,
    pub face_index: u16,
    pub clauses: Vec<BoundedOracleClause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedOracleClause {
    runtime_version: &'static str,
    semantic_digest: String,
    address: ClauseAddress,
    source_clause: String,
    normalized_clause: String,
    ability_word: Option<String>,
    timing: Timing,
    conditions: Vec<Condition>,
    costs: Vec<Cost>,
    targets: Vec<Target>,
    effects: Vec<Effect>,
    activation_restriction: Option<ActivationRestriction>,
    reminder: Option<ReminderSemantics>,
    saga_lore_procedure: Option<SagaLoreProcedure>,
}

impl BoundedOracleClause {
    pub fn runtime_version(&self) -> &'static str {
        self.runtime_version
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn address(&self) -> ClauseAddress {
        self.address
    }

    /// Exact trimmed Oracle text used to compile this clause. Occurrence
    /// coordinates and card-name metadata are deliberately kept separate.
    pub fn source_clause(&self) -> &str {
        &self.source_clause
    }

    pub fn normalized_clause(&self) -> &str {
        &self.normalized_clause
    }

    pub fn ability_word(&self) -> Option<&str> {
        self.ability_word.as_deref()
    }

    pub fn timing(&self) -> &Timing {
        &self.timing
    }

    pub fn conditions(&self) -> &[Condition] {
        &self.conditions
    }

    pub fn costs(&self) -> &[Cost] {
        &self.costs
    }

    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    pub fn activation_restriction(&self) -> Option<&ActivationRestriction> {
        self.activation_restriction.as_ref()
    }

    pub fn reminder(&self) -> Option<&ReminderSemantics> {
        self.reminder.as_ref()
    }

    pub fn saga_lore_procedure(&self) -> Option<&SagaLoreProcedure> {
        self.saga_lore_procedure.as_ref()
    }

    /// A compiled Saga rules procedure is intentionally not an executable
    /// clause until a dedicated lifecycle consumer can observe lore placement,
    /// the source chapter abilities on the stack, and state-based actions.
    pub const fn requires_saga_lore_consumer(&self) -> bool {
        self.saga_lore_procedure.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaLoreProcedure {
    pub object: ObjectRef,
    pub lore_counter: CounterKind,
    pub enters_with: Amount,
    pub after_controller_draw_step: Amount,
    pub final_chapter: SagaFinalChapter,
    pub state_based_sacrifice: SagaStateBasedSacrifice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SagaFinalChapter {
    /// The reminder printed a chapter number, but a direct clause compilation
    /// has not proved that it is the highest chapter on the source face.
    PrintedUnvalidated(u16),
    /// The compact reminder delegates the threshold to the highest chapter
    /// printed on the source face.
    HighestPrintedChapterOnFace,
    /// Complete face compilation found and validated the highest printed
    /// chapter.
    BoundHighestPrintedChapter(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaStateBasedSacrifice {
    pub sacrifice_source: bool,
    pub lore_at_least_final_chapter: bool,
    pub no_source_chapter_ability_on_stack: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    EmptyClause {
        address: ClauseAddress,
    },
    InvalidMana {
        address: ClauseAddress,
        text: String,
    },
    UnsupportedSyntax {
        address: ClauseAddress,
        normalized_clause: String,
    },
    UnsupportedReminder {
        address: ClauseAddress,
        reminder: String,
    },
    MalformedModalGroup {
        address: ClauseAddress,
        detail: &'static str,
    },
    MalformedSyntax {
        address: ClauseAddress,
        error: OracleClauseSyntaxError,
    },
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyClause { address } => write!(
                formatter,
                "face {} clause {} is empty",
                address.face_index, address.clause_index
            ),
            Self::InvalidMana { address, text } => write!(
                formatter,
                "face {} clause {} contains invalid mana syntax `{text}`",
                address.face_index, address.clause_index
            ),
            Self::UnsupportedSyntax {
                address,
                normalized_clause,
            } => write!(
                formatter,
                "face {} clause {} is not completely parsed: {normalized_clause}",
                address.face_index, address.clause_index
            ),
            Self::UnsupportedReminder { address, reminder } => write!(
                formatter,
                "face {} clause {} has unsupported reminder text: {reminder}",
                address.face_index, address.clause_index
            ),
            Self::MalformedModalGroup { address, detail } => write!(
                formatter,
                "face {} clause {} has a malformed modal group: {detail}",
                address.face_index, address.clause_index
            ),
            Self::MalformedSyntax { address, error } => write!(
                formatter,
                "face {} clause {} has malformed Oracle syntax: {error}",
                address.face_index, address.clause_index
            ),
        }
    }
}

impl std::error::Error for CompileError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Timing {
    /// A mandatory printed additional cost paid while casting the source
    /// spell, before targets and the total cost are finalized.
    CastingAdditionalCost,
    SpellResolution,
    Activated,
    Triggered(Box<Trigger>),
    TriggeredModalHeader {
        trigger: Box<Trigger>,
        choices: ChoiceCount,
    },
    Static,
    Replacement,
    ModalHeader {
        choices: ChoiceCount,
    },
    ModalBranch {
        header_clause_index: Option<u16>,
        branch_index: u16,
    },
    /// Exact timing remains inside a typed standalone rules program. The
    /// generic bounded consumer must not infer a live window from this marker.
    TypedStandaloneProgram,
    SpecialAction(SpecialActionTiming),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    AnyOf(Vec<Trigger>),
    OncePerTurn(Box<Trigger>),
    SourceEnters,
    SourceCast,
    SchemeSetInMotion,
    ObjectEnters(ObjectFilter),
    ObjectAttacks(ObjectFilter),
    ObjectTappedForMana {
        object: ObjectRef,
    },
    AttachmentTargetEvent {
        kind: AttachmentKind,
        event: ObjectEventKind,
    },
    SourceAttacks,
    SourceBlocks {
        object: ObjectFilter,
    },
    ObjectEvent {
        subject: TriggerSubject,
        event: ObjectEventKind,
    },
    LifeGained {
        player: PlayerRef,
    },
    TokenCreated {
        player: PlayerRef,
    },
    PlayerAction {
        player: PlayerRef,
        action: PlayerActionKind,
        subject: Option<TriggerSubject>,
    },
    Cast {
        player: PlayerRef,
        spell: ObjectFilter,
    },
    NthSpellCast {
        player: PlayerRef,
        occurrence_this_turn: u32,
    },
    CardDrawn {
        player: PlayerRef,
        occurrence_this_turn: Option<u32>,
    },
    CombatDamageToPlayer {
        source: ObjectFilter,
    },
    AttachmentTargetCombatDamageToPlayer {
        kind: AttachmentKind,
    },
    DamageToPlayer {
        source: ObjectFilter,
        player: PlayerRef,
    },
    SourceCombatDamageToPlayer,
    SourceDamageToPlayer,
    SourceCombatDamageToObject {
        object: ObjectFilter,
    },
    BecomesTarget {
        object: ObjectRef,
        controller: PlayerRef,
        source_kinds: Vec<CardType>,
    },
    BeginningOf {
        step: Step,
        player: TurnPlayer,
    },
    BeginningOfNextEndStep,
    SagaChapterReached {
        chapter: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerSubject {
    Source,
    Matching(ObjectFilter),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectEventKind {
    Dies,
    DealtDamage,
    Attacks,
    LeavesBattlefield,
    PutIntoGraveyardFromBattlefield,
    PutIntoGraveyardFromAnywhere,
    BecomesTapped,
    BecomesBlocked,
    Blocks,
    TurnedFaceUp,
    Mutates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerActionKind {
    Attack,
    Cycle,
    Discard,
    Exert,
    Sacrifice,
    Scry,
    Surveil,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Upkeep,
    DrawStep,
    FirstMainPhase,
    PostcombatMainPhase,
    EndStep,
    Combat,
    UntapStep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatStep {
    Beginning,
    DeclareAttackers,
    DeclareBlockers,
    CombatDamage,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnPlayer {
    You,
    EachPlayer,
    NextTurn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialActionTiming {
    Pregame,
    EntersPrepared,
    TransformBackFaceAnnotation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationRestriction {
    All(Vec<ActivationRestriction>),
    SorceryTiming,
    InstantTiming,
    YourTurn,
    DuringYourUpkeep,
    DuringYourTurnBeforeAttackersDeclared,
    OnceEachTurn,
    TimesEachTurn(u16),
    Exhaust { sorcery_timing_only: bool },
    AnyPlayerMayActivate,
    SourceZone(Zone),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cost {
    Optional(Box<Cost>),
    Mana(ManaCost),
    /// One exact resource expression containing energy or snow. The complete
    /// expression stays atomic so an eventual production adapter cannot pay
    /// the ordinary and special parts independently.
    AtomicResource(AtomicResourceCost),
    Loyalty(LoyaltyCost),
    Tap(ObjectRef),
    TapSelection(ObjectSelection),
    Untap(ObjectRef),
    TapCreaturesWithTotalPower {
        player: PlayerRef,
        minimum: Amount,
    },
    TapCreatureSelectionWithTotalPower {
        selection: ObjectSelection,
        minimum: Amount,
    },
    PayLife(Amount),
    Sacrifice {
        amount: Amount,
        filter: ObjectFilter,
    },
    SacrificeSelection(ObjectSelection),
    SacrificeObject(ObjectRef),
    Discard(ObjectRef),
    DiscardSelection(ObjectSelection),
    DiscardRandom {
        player: PlayerRef,
    },
    ReturnSelectionToHand(ObjectSelection),
    DiscardHand {
        player: PlayerRef,
    },
    RevealSelection {
        selection: ObjectSelection,
        optional: bool,
    },
    BeholdSelection {
        choice_id: u8,
        battlefield: ObjectFilter,
        hand: ObjectFilter,
    },
    Waterbend {
        selection: ObjectSelection,
        amount: Amount,
    },
    PutCounter {
        object: ObjectRef,
        counter: CounterKind,
        amount: Amount,
    },
    PutCounterSelection {
        selection: ObjectSelection,
        counter: CounterKind,
        amount: Amount,
    },
    ExileObject(ObjectRef),
    ExileSourceFromBattlefield,
    ExileSourceFromOwnGraveyard,
    ExileSelection(ObjectSelection),
    ExileSelectionWithTotalManaValue {
        selection: ObjectSelection,
        minimum: Amount,
    },
    RemoveCounter {
        object: ObjectRef,
        counter: CounterKind,
        amount: Amount,
    },
    Unprepare(ObjectRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicResourceCost {
    expression: ResourceCostExpression,
    special: SpecialResourceCost,
}

impl AtomicResourceCost {
    pub fn expression(&self) -> &ResourceCostExpression {
        &self.expression
    }

    pub fn special(&self) -> &SpecialResourceCost {
        &self.special
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoyaltyCost {
    Add(u32),
    Remove(Amount),
    Zero,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaCost(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    ControlCount {
        player: PlayerRef,
        filter: ObjectFilter,
        comparison: Comparison,
        amount: Amount,
    },
    ControlAny {
        player: PlayerRef,
        filters: Vec<ObjectFilter>,
    },
    SourceState(ObjectState),
    TargetState {
        target: ObjectRef,
        state: ObjectState,
    },
    SpellTargetsState {
        card_type: CardType,
        state: ObjectState,
    },
    PowerComparison {
        object: ObjectFilter,
        comparison: Comparison,
        amount: Amount,
    },
    EventWouldOccur(ReplacementEvent),
    PaymentDeclined(Cost),
    PaymentAccepted(Cost),
    CardWasCastWithAlternativeCost,
    CardWasCastUsingEscape,
    CardWasKicked,
    CardWasCastUsingTeamwork,
    YouAttackedThisTurn,
    OpponentLostLifeThisTurn,
    NotYourTurn,
    NotThatPlayersTurn,
    GraveyardCardCount {
        player: PlayerRef,
        comparison: Comparison,
        amount: Amount,
    },
    HandCardCount {
        player: PlayerRef,
        comparison: Comparison,
        amount: Amount,
    },
    CardTypesInGraveyard {
        player: PlayerRef,
        comparison: Comparison,
        amount: Amount,
    },
    SourceHasCounter {
        counter: CounterKind,
    },
    SourceCounterCount {
        counter: CounterKind,
        comparison: Comparison,
        amount: u32,
    },
    ObjectCounterCount {
        object: ObjectRef,
        counter: CounterKind,
        comparison: Comparison,
        amount: u32,
    },
    CommanderControlled {
        player: PlayerRef,
    },
    GiftPromised,
    SourceInOpeningHand,
    NotPlayingFirst,
    ModeSelected(u16),
    AnotherSpellCastThisTurn,
    SpellsCastByActorThisTurn {
        comparison: Comparison,
        amount: u32,
    },
    SourceAttackingAlone,
    SpellCastFromNonHand,
    ManaSpentGreaterThanSourcePowerOrToughness,
    CastOnlyDuringCombat,
    CastOnlyDuringCombatBeforeBlockers,
    CastOnlyDuringDeclareBlockers,
    CastOnlyDuringCombatAfterBlockers,
    SourceWasCounteredByThisEffect,
    ObjectIsCardType {
        object: ObjectRef,
        card_type: CardType,
    },
    FirstResolutionOfNamedSpell,
    UnlessPaid {
        player: PlayerRef,
        cost: Cost,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    AtLeast,
    AtMost,
    Exactly,
    Greatest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectState {
    Tapped,
    Untapped,
    Attacking,
    Blocking,
    Prepared,
    FaceDown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementEvent {
    CreateTokens {
        player: PlayerRef,
    },
    PutCounters {
        counter: CounterKind,
        object: Box<ObjectFilter>,
    },
    SourceWouldEnter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerRef {
    You,
    PlayerIdentity(u8),
    Opponent,
    OtherPlayer,
    Any,
    TargetPlayer(u8),
    ControllerOf(Box<ObjectRef>),
    OwnerOf(Box<ObjectRef>),
    ThatPlayer,
    DefendingPlayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttachmentKind {
    Aura,
    Equipment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectRef {
    Source,
    ObjectIdentity(u64),
    AttachmentTarget { kind: AttachmentKind },
    Target(u8),
    TargetSet(Vec<u8>),
    TriggeringObject,
    ThatObject(u8),
    SearchedCard(u8),
    TopCard { player: Box<PlayerRef> },
    EachMatching(ObjectFilter),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub id: u8,
    pub chooser: PlayerRef,
    pub filter: TargetFilter,
    pub amount: TargetAmount,
    pub relationship: TargetRelationship,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetFilter {
    Player,
    Opponent,
    Object(ObjectFilter),
    Spell(ObjectFilter),
    Any(Vec<TargetFilter>),
    Conditional {
        condition: Condition,
        if_true: Box<TargetFilter>,
        if_false: Box<TargetFilter>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetAmount {
    Exactly(u16),
    UpTo(u16),
    AnyNumber,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetRelationship {
    Independent,
    DifferentControllers,
    ShareCreatureType,
    OtherThan(ObjectRef),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectFilter {
    pub zones: Vec<Zone>,
    pub controller: Option<PlayerRef>,
    pub owner: Option<PlayerRef>,
    pub names: Vec<String>,
    pub chosen_name_of_source: bool,
    pub card_types: Vec<CardType>,
    pub card_type_match_any: bool,
    pub excluded_card_types: Vec<CardType>,
    pub supertypes: Vec<Supertype>,
    pub subtypes: Vec<String>,
    pub excluded_subtypes: Vec<String>,
    pub subtype_match_any: bool,
    pub colors: Vec<Color>,
    pub excluded_colors: Vec<Color>,
    pub color_match_any: bool,
    pub keywords: Vec<Keyword>,
    pub excluded_keywords: Vec<Keyword>,
    pub token: Option<bool>,
    pub tapped: Option<bool>,
    pub attacking: Option<bool>,
    pub blocking: Option<bool>,
    pub attached_by: Option<AttachmentKind>,
    pub other_than_source: bool,
    pub targets_source: bool,
    pub chosen_creature_type: bool,
    pub power: Option<(Comparison, Box<Amount>)>,
    pub toughness: Option<(Comparison, Box<Amount>)>,
    pub mana_value: Option<(Comparison, Box<Amount>)>,
    pub minimum_counter: Option<(CounterKind, u32)>,
    /// A historic object is an artifact, a legendary object, or a Saga.
    /// This is a cross-category union and therefore cannot be represented by
    /// the ordinary conjunctive card-type/supertype/subtype fields.
    pub historic: bool,
}

impl ObjectFilter {
    fn in_zone(zone: Zone) -> Self {
        Self {
            zones: vec![zone],
            ..Self::default()
        }
    }

    fn with_type(card_type: CardType) -> Self {
        Self {
            card_types: vec![card_type],
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Stack,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardType {
    Artifact,
    Battle,
    Creature,
    Enchantment,
    Instant,
    Land,
    Planeswalker,
    Sorcery,
    Spell,
    Permanent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Supertype {
    Basic,
    Legendary,
    Snow,
    Nonbasic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Amount {
    Constant(u32),
    X,
    OneOrMore,
    Any,
    Twice(Box<Amount>),
    Product { factor: u32, value: Box<Amount> },
    Count(Box<CountExpression>),
    UpTo(Box<Amount>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountExpression {
    Constant(u32),
    OpponentCount {
        player: PlayerRef,
    },
    PartySize {
        player: PlayerRef,
    },
    DistinctBasicLandTypes {
        player: PlayerRef,
    },
    CreaturesAttackedThisTurn {
        player: PlayerRef,
    },
    PowerOf {
        object: ObjectRef,
    },
    ToughnessOf {
        object: ObjectRef,
    },
    HalfLifeTotal {
        player: PlayerRef,
        round_up: bool,
    },
    SelectedObjectsTotalPower {
        selection_id: u8,
    },
    SelectedObjectsTotalToughness {
        selection_id: u8,
    },
    MatchingObjects {
        player: PlayerRef,
        filter: ObjectFilter,
    },
    GreatestPower {
        player: PlayerRef,
        filter: ObjectFilter,
    },
    CountersOn {
        object: ObjectRef,
        counter: CounterKind,
    },
    AttachmentsOn {
        object: ObjectRef,
        kind: AttachmentKind,
    },
    CardsInZone {
        player: PlayerRef,
        zone: Zone,
        filter: ObjectFilter,
    },
    OpponentsDealtCombatDamage {
        player: PlayerRef,
    },
    Devotion {
        player: PlayerRef,
        color: Color,
    },
    ManaValueOf {
        object: ObjectRef,
    },
    LifeLostThisWay {
        players: PlayerRef,
        amount_each: Box<Amount>,
    },
    TriggerEventAmount,
    ReplacementEventAmount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoiceCount {
    Exactly(u16),
    ExactlyWithRepeats(u16),
    UpTo(u16),
    Between { minimum: u16, maximum: u16 },
    OneOrMore,
    OneOrBothIfTeamwork,
}

/// A complete rules program whose standalone runtime is accurate but whose
/// production state adapter has not yet been connected. These programs remain
/// explicit nonlive coverage until that adapter exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandaloneRuleProgram {
    AbilityClause(Box<AbilityClauseBridgeProgram>),
    OracleAbilityEnvelope(Box<OracleAbilityEnvelopeProgram>),
    OracleStaticReplacement(Box<OracleStaticReplacementProgram>),
    OracleCastZoneEnvelope(Box<CastZoneEnvelopeProgram>),
    AlternateZoneCastKeyword(Box<AlternateZoneKeywordProgram>),
    CastModifierKeyword(Box<CastModifierKeywordProgram>),
    AttachmentFilter(Box<AttachmentFilterProgram>),
    DelayedCounterKeyword(Box<DelayedCounterKeywordProgram>),
    LibraryAccess(Box<LibraryAccessProgram>),
    OldTransform(Box<OldTransformProgram>),
    ObjectState(Box<ObjectStateClauseProgram>),
    Pregame(Box<PregameClauseProgram>),
    CombatRestriction(Box<CombatRestrictionProgram>),
    AttachmentEntry(Box<AttachmentEntryProgram>),
    SagaTransform(Box<SagaTransformProgram>),
    DamageClause(Box<CompiledDamageClause>),
    TargetingProtection(Box<TargetingProtectionProgram>),
    EntryChoiceKeyword(Box<EntryChoiceKeywordProgram>),
    FaceDownMergeKeyword(Box<FaceDownMergeKeywordProgram>),
    CombatSpecialKeyword(Box<CombatSpecialKeywordProgram>),
    GraveyardTransformKeyword(Box<GraveyardTransformKeywordProgram>),
    LevelProgression(Box<LevelProgressionProgram>),
    ExtendedCastZoneKeyword(Box<ExtendedCastZoneProgram>),
    ResidualCostKeyword(Box<ResidualCostKeywordProgram>),
    LinkedCastCostKeyword(Box<LinkedCastCostProgram>),
    CombatTriggerKeyword(Box<CombatTriggerKeywordProgram>),
    CreatureCounterKeyword(Box<CreatureCounterKeywordProgram>),
    GraveyardHandLibraryKeyword(Box<ZoneKeywordProgram>),
    CastChoiceKeyword(Box<CastChoiceKeywordProgram>),
    StaticSpecialKeyword(Box<StaticSpecialKeywordProgram>),
    CommonActionProcedure(Box<CommonActionProgram>),
    RegenerationAction(Box<RegenerationActionProgram>),
    OracleAction(Box<OracleActionProgram>),
    OracleFaceModalLine(Box<OracleFaceModalLineProgram>),
    OracleComposition(Box<OracleCompositionProgram>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleFaceModalLineRole {
    Header,
    Branch { branch_index: u16 },
}

impl OracleFaceModalLineRole {
    fn stable_id(self) -> String {
        match self {
            Self::Header => "header/v1".to_owned(),
            Self::Branch { branch_index } => format!("branch/v1/{branch_index}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleFaceModalLineProgram {
    exact_source: String,
    semantic_digest: String,
    role: OracleFaceModalLineRole,
    group: AssembledModalProgram<OracleCompositionChildProgram>,
}

impl OracleFaceModalLineProgram {
    pub(crate) fn compile(
        exact_source: &str,
        role: OracleFaceModalLineRole,
        group: AssembledModalProgram<OracleCompositionChildProgram>,
    ) -> Option<Self> {
        if exact_source.trim() != exact_source
            || exact_source.is_empty()
            || group.production_adapter_connected()
        {
            return None;
        }
        let expected_source = match role {
            OracleFaceModalLineRole::Header => modal_group_relative_slice(
                &group,
                group.header().source_span.start,
                group.header().source_span.end,
            )?,
            OracleFaceModalLineRole::Branch { branch_index } => {
                let branch = group.branches().get(usize::from(branch_index))?;
                modal_group_relative_slice(&group, branch.marker_span.start, branch.body_span.end)?
            }
        };
        if expected_source != exact_source {
            return None;
        }
        let role_id = role.stable_id();
        let semantic_digest = bounded_clause_semantic_digest_with_versions_and_context(
            ORACLE_FACE_PROGRAM_ASSEMBLER_COMPILER_VERSION,
            ORACLE_FACE_PROGRAM_ASSEMBLER_RUNTIME_VERSION,
            exact_source,
            exact_source,
            &[
                ORACLE_FACE_MODAL_RULES_CONTEXT_VERSION,
                group.semantic_digest(),
                &role_id,
            ],
        );
        Some(Self {
            exact_source: exact_source.to_owned(),
            semantic_digest,
            role,
            group,
        })
    }

    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub const fn role(&self) -> OracleFaceModalLineRole {
        self.role
    }

    pub fn group(&self) -> &AssembledModalProgram<OracleCompositionChildProgram> {
        &self.group
    }

    pub const fn production_adapter_connected(&self) -> bool {
        false
    }
}

fn modal_group_relative_slice(
    group: &AssembledModalProgram<OracleCompositionChildProgram>,
    absolute_start: usize,
    absolute_end: usize,
) -> Option<&str> {
    let group_start = group.source_span().start;
    let start = absolute_start.checked_sub(group_start)?;
    let end = absolute_end.checked_sub(group_start)?;
    group.exact_source().get(start..end)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleCompositionProgram {
    typed: TypedOracleComposition,
    children: Vec<OracleCompositionChildBinding>,
}

impl OracleCompositionProgram {
    pub fn exact_source(&self) -> &str {
        self.typed.exact_oracle()
    }

    pub fn semantic_digest(&self) -> &str {
        self.typed.semantic_digest()
    }

    pub fn typed(&self) -> &TypedOracleComposition {
        &self.typed
    }

    pub fn children(&self) -> &[OracleCompositionChildBinding] {
        &self.children
    }

    pub const fn production_adapter_connected(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleCompositionChildBinding {
    span: SourceSpan,
    capabilities: Vec<SemanticCapability>,
    program: OracleCompositionChildProgram,
}

impl OracleCompositionChildBinding {
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub fn capabilities(&self) -> &[SemanticCapability] {
        &self.capabilities
    }

    pub fn program(&self) -> &OracleCompositionChildProgram {
        &self.program
    }

    pub(crate) fn bounded(
        span: SourceSpan,
        capabilities: Vec<SemanticCapability>,
        program: BoundedOracleClause,
    ) -> Self {
        Self {
            span,
            capabilities,
            program: OracleCompositionChildProgram::Bounded(Box::new(program)),
        }
    }

    pub(crate) fn delegated_keyword(
        span: SourceSpan,
        capabilities: Vec<SemanticCapability>,
        exact_source: String,
        semantic_digest: String,
        normalized_clause: String,
        keyword_program: KeywordProgram,
    ) -> Self {
        Self {
            span,
            capabilities,
            program: OracleCompositionChildProgram::DelegatedKeyword(Box::new(
                OracleCompositionDelegatedKeywordChild {
                    exact_source,
                    semantic_digest,
                    normalized_clause,
                    keyword_program,
                },
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleCompositionChildProgram {
    Bounded(Box<BoundedOracleClause>),
    DelegatedKeyword(Box<OracleCompositionDelegatedKeywordChild>),
}

impl OracleCompositionChildProgram {
    pub(crate) fn bounded(program: BoundedOracleClause) -> Self {
        Self::Bounded(Box::new(program))
    }

    pub(crate) fn delegated_keyword(
        exact_source: String,
        semantic_digest: String,
        normalized_clause: String,
        keyword_program: KeywordProgram,
    ) -> Self {
        Self::DelegatedKeyword(Box::new(OracleCompositionDelegatedKeywordChild {
            exact_source,
            semantic_digest,
            normalized_clause,
            keyword_program,
        }))
    }

    pub fn exact_source(&self) -> &str {
        match self {
            Self::Bounded(program) => program.source_clause(),
            Self::DelegatedKeyword(program) => program.exact_source(),
        }
    }

    pub fn semantic_digest(&self) -> &str {
        match self {
            Self::Bounded(program) => program.semantic_digest(),
            Self::DelegatedKeyword(program) => program.semantic_digest(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleCompositionDelegatedKeywordChild {
    exact_source: String,
    semantic_digest: String,
    normalized_clause: String,
    keyword_program: KeywordProgram,
}

impl OracleCompositionDelegatedKeywordChild {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn normalized_clause(&self) -> &str {
        &self.normalized_clause
    }

    pub fn keyword_program(&self) -> &KeywordProgram {
        &self.keyword_program
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulesTextChoiceKind {
    Word,
    Artist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayNightDesignation {
    Day,
    Night,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinFlipResult {
    Won,
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// One optional instruction together with every dependent "if you do"
    /// instruction that follows it.
    Optional(Vec<Effect>),
    PayCost(Cost),
    AddMana(ManaProduction),
    SetClassLevel {
        level: u8,
    },
    Counter {
        object: ObjectRef,
    },
    CounterToZone {
        object: ObjectRef,
        zone: Zone,
    },
    Destroy {
        object: ObjectRef,
    },
    DestroyWithoutRegeneration {
        object: ObjectRef,
    },
    MoveZone(ZoneMove),
    MoveZoneUnderControl {
        object: ObjectRef,
        from: Zone,
        to: Zone,
        controller: PlayerRef,
        tapped: bool,
        face_down: bool,
        delayed_until: Option<Trigger>,
    },
    MoveToLibraryBottom {
        object: ObjectRef,
    },
    MoveToChosenLibraryEnd {
        object: ObjectRef,
        choice_id: u8,
    },
    PutOnLibraryTopInOrder {
        objects: ObjectRef,
    },
    PutInLibraryAtPosition {
        object: ObjectRef,
        position_from_top: u16,
    },
    MoveSelected(SelectedZoneMove),
    SetSelectedTapped {
        selection: ObjectSelection,
        tapped: bool,
    },
    SearchLibrary(SearchLibrary),
    ShuffleLibrary {
        player: PlayerRef,
    },
    ExileTop(TopLibraryExile),
    ExileCollection(ExileCollectionEffect),
    BounceWithControllerCopy(BounceWithControllerCopyEffect),
    GrantCastPermission(CastPermission),
    LibraryProcedure(LibraryProcedure),
    CreateToken(TokenCreation),
    CreateTokenAttached {
        creation: TokenCreation,
        target: ObjectRef,
        kind: AttachmentKind,
    },
    CreateTokenAndAttachSource {
        creation: TokenCreation,
        attachment: ObjectRef,
        kind: AttachmentKind,
    },
    Attach {
        attachment: ObjectRef,
        target: ObjectRef,
        kind: AttachmentKind,
    },
    ResolveTargetChoice {
        object: ObjectRef,
    },
    ResolvePlayerTargetChoice {
        player: PlayerRef,
    },
    ChoosePlayer {
        chooser: PlayerRef,
        eligible: PlayerRef,
    },
    CreateTokenWithDelayedMove {
        creation: TokenCreation,
        destination: Zone,
        trigger: Trigger,
    },
    Draw {
        player: PlayerRef,
        amount: Amount,
        optional: bool,
        delayed_until: Option<Trigger>,
    },
    DrawRevealDiscardIfNonland {
        player: PlayerRef,
    },
    DrawThenDiscardUnless {
        player: PlayerRef,
        draw: u16,
        discard: u16,
        alternative: ObjectFilter,
        choice_id: u8,
    },
    ChooseCardName {
        nonland: bool,
    },
    ChooseColor,
    RollDie {
        sides: u16,
    },
    FlipCoin,
    Proliferate {
        choice_id: u8,
    },
    InitializeIntensity {
        amount: u16,
    },
    ChooseNamedOption {
        options: Vec<String>,
    },
    ChooseRulesText {
        kind: RulesTextChoiceKind,
    },
    EstablishDayIfUnset,
    EachPlayerSacrifices {
        filter: ObjectFilter,
        amount: u16,
    },
    PlayersSacrifice {
        players: PlayerRef,
        filter: ObjectFilter,
        amount: u16,
    },
    RevealHand {
        player: PlayerRef,
    },
    LookAtHand {
        viewer: PlayerRef,
        player: PlayerRef,
    },
    Discard(ObjectSelection),
    Connive {
        object: ObjectRef,
        discard: ObjectSelection,
    },
    GainLife {
        player: PlayerRef,
        amount: Amount,
    },
    LoseLife {
        player: PlayerRef,
        amount: Amount,
    },
    PayLife {
        player: PlayerRef,
        amount: Amount,
    },
    Damage {
        source: ObjectRef,
        recipient: PlayerRef,
        amount: Amount,
    },
    PreventDamage {
        combat_only: bool,
        amount: Amount,
        duration: Duration,
    },
    Tap {
        object: ObjectRef,
    },
    Untap {
        object: ObjectRef,
    },
    RemoveFromCombat {
        object: ObjectRef,
    },
    Exert {
        object: ObjectRef,
    },
    PreventNextUntap {
        object: ObjectRef,
    },
    Scry {
        player: PlayerRef,
        amount: Amount,
    },
    Surveil {
        player: PlayerRef,
        amount: Amount,
    },
    Mill {
        player: PlayerRef,
        amount: Amount,
    },
    Manifest {
        player: PlayerRef,
        card: ObjectRef,
    },
    PutPlayerCounter {
        player: PlayerRef,
        counter: String,
        amount: Amount,
    },
    PutCounter {
        object: ObjectRef,
        counter: CounterKind,
        amount: Amount,
    },
    RemoveCounter {
        object: ObjectRef,
        counter: CounterKind,
        amount: Amount,
    },
    MoveAllCounters {
        from: ObjectRef,
        to: ObjectRef,
    },
    ModifyPowerToughness(PowerToughnessChange),
    GrantKeyword {
        objects: ObjectRef,
        keywords: Vec<Keyword>,
        duration: Duration,
    },
    GrantAbility {
        objects: ObjectRef,
        ability: GrantedAbility,
        duration: Duration,
    },
    LoseAllAbilities {
        object: ObjectRef,
        duration: Duration,
    },
    SetCharacteristics(SetCharacteristics),
    SetCreatureTypeToChoice {
        object: ObjectRef,
        duration: Duration,
    },
    SetColorToChoice {
        object: ObjectRef,
        duration: Duration,
    },
    SetBasicLandTypeToChoice {
        object: ObjectRef,
        duration: Duration,
    },
    Restriction(Restriction),
    Replacement(Box<ReplacementEffect>),
    Copy(CopyEffect),
    Transform {
        object: ObjectRef,
    },
    Prepare {
        object: ObjectRef,
    },
    ResolveWard {
        payer: PlayerRef,
        source: ObjectRef,
        cost: Box<WardCost>,
    },
    Animate(AnimateEffect),
    ChooseCreatureType {
        player: PlayerRef,
    },
    LookAtTop {
        player: PlayerRef,
        amount: Amount,
    },
    RevealTop {
        player: PlayerRef,
        amount: Amount,
    },
    SelectFromLookedAt {
        player: PlayerRef,
        amount: Amount,
        predicate: ObjectFilter,
        reveal: bool,
        face_down: bool,
        tapped: bool,
        destination: Zone,
    },
    PutRestOnLibraryBottom {
        player: PlayerRef,
        order: BottomOrder,
    },
    PutRestOfLookedAt {
        player: PlayerRef,
        destination: Zone,
    },
    ReorderLookedAtOnLibraryTop {
        player: PlayerRef,
    },
    ExileSpellAfterResolution {
        object: ObjectRef,
    },
    CopyStackObject {
        object: ObjectRef,
        may_choose_new_targets: bool,
    },
    ChooseNewTargets {
        object: ObjectRef,
    },
    ChangeControl {
        object: ObjectRef,
        controller: PlayerRef,
    },
    ChangeControlUntil {
        object: ObjectRef,
        controller: PlayerRef,
        duration: Duration,
    },
    SkipStep {
        player: PlayerRef,
        step: Step,
    },
    WinGame {
        player: PlayerRef,
    },
    LoseGame {
        player: PlayerRef,
    },
    TakeExtraTurn(ExtraTurnEffect),
    SchedulePaymentOrLose(PaymentOrLoseEffect),
    CastCopy(CastCopyEffect),
    ReduceActivationCost {
        mana: ManaCost,
        per: CountExpression,
        minimum_total: Option<ManaCost>,
    },
    ReduceSpellCost {
        object: ObjectRef,
        mana: ManaCost,
        per: CountExpression,
        maximum_reduction: Option<ManaCost>,
    },
    ReduceSpellCostWhen {
        object: ObjectRef,
        mana: ManaCost,
        condition: Condition,
    },
    IncreaseSpellCost {
        object: ObjectRef,
        mana: ManaCost,
        per: CountExpression,
    },
    ChooseMode {
        count: ChoiceCount,
    },
    ChooseModeBy {
        chooser: PlayerRef,
        count: ChoiceCount,
    },
    ChooseModeNotPreviouslyChosen {
        count: ChoiceCount,
    },
    ChooseModeFrom {
        count: ChoiceCount,
        option_count: u16,
    },
    StandaloneRuleProgram(StandaloneRuleProgram),
    Conditional {
        condition: Condition,
        if_true: Vec<Effect>,
        if_false: Vec<Effect>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaProduction {
    pub player: PlayerRef,
    pub choices: Vec<ManaChoice>,
    pub amount: Amount,
    pub commander_identity_only: bool,
    pub scales_with: Option<CountExpression>,
    /// Complete source semantics for mana expressions accepted through the
    /// typed mana parser. Legacy constructors leave this empty and retain the
    /// bounded representation above.
    pub typed: Option<TypedManaProductionExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaChoice {
    pub symbols: Vec<Color>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneMove {
    pub object: ObjectRef,
    pub from: Option<Zone>,
    pub to: Zone,
    pub tapped: bool,
    pub face_down: bool,
    pub delayed_until: Option<Trigger>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectSelection {
    pub id: u8,
    pub chooser: PlayerRef,
    pub filter: ObjectFilter,
    pub amount: TargetAmount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedZoneMove {
    pub selection: ObjectSelection,
    pub to: Zone,
    pub tapped: bool,
    pub face_down: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchLibrary {
    pub player: PlayerRef,
    pub chooser: PlayerRef,
    pub optional: bool,
    pub allow_fail_to_find: bool,
    pub amount: Amount,
    pub predicate: ObjectFilter,
    pub reveal: bool,
    pub destinations: Vec<SearchDestination>,
    pub shuffle_before_destination: bool,
    pub shuffle_after: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLibraryExile {
    pub player: PlayerRef,
    pub amount: Amount,
    pub face_down: bool,
    pub cast_permission: Option<CastPermission>,
    pub delayed_destination: Option<(Zone, Trigger)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExileCollectionEffect {
    pub objects: ObjectRef,
    pub from: Zone,
    pub cast_permission: Option<CastPermission>,
    pub delayed_destination: Option<(Zone, Trigger)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BounceWithControllerCopyEffect {
    pub object: ObjectRef,
    pub sacrifice: ObjectSelection,
    pub copy_source: ObjectRef,
    pub may_choose_new_targets: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastPermission {
    pub affected: PlayerRef,
    pub objects: Option<ObjectRef>,
    pub filter: ObjectFilter,
    pub from: Zone,
    pub timing: CastTiming,
    pub duration: Duration,
    pub alternative_cost: Option<AlternativeCost>,
    pub additional_costs: Vec<Cost>,
    pub mana_as_any_type: bool,
    pub exile_after_resolution: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastTiming {
    Normal,
    AsThoughFlash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryProcedure {
    ShuffleGraveyardIntoLibrary {
        player: PlayerRef,
    },
    ShuffleHandIntoLibraryAndDrawSame {
        player: PlayerRef,
    },
    ShuffleHandAndGraveyardIntoLibraryAndDraw {
        player: PlayerRef,
        amount: Amount,
    },
    DiscardHandsAndDraw {
        player: PlayerRef,
        amount: Amount,
    },
    DiscardHandsAndDrawDiscarded {
        player: PlayerRef,
        adjustment: i8,
    },
    RevealTopToHandLoseManaValue {
        player: PlayerRef,
        repeat: Amount,
    },
    ExileUntilNamedCard {
        player: PlayerRef,
        initial_exile: u32,
    },
    ExileUntilAcceptedOrDuplicate {
        player: PlayerRef,
    },
    DevotionLookAndWin {
        player: PlayerRef,
        color: Color,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraTurnEffect {
    pub player: PlayerRef,
    pub lose_at_end_step: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentOrLoseEffect {
    pub player: PlayerRef,
    pub cost: Cost,
    pub trigger: Trigger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDestination {
    pub selected_ordinal: SearchOrdinal,
    pub zone: Zone,
    pub tapped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchOrdinal {
    Each,
    First,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenCreation {
    pub player: PlayerRef,
    pub amount: Amount,
    pub specification: TokenSpecification,
    pub tapped: bool,
    pub attacking: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSpecification {
    Defined(Box<TokenDefinition>),
    CopyOf(ObjectRef),
    ManifestedCard(ObjectRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenDefinition {
    pub name: Option<String>,
    pub power: Option<Amount>,
    pub toughness: Option<Amount>,
    pub colors: Vec<Color>,
    pub card_types: Vec<CardType>,
    pub subtypes: Vec<String>,
    pub keywords: Vec<Keyword>,
    pub abilities: Vec<GrantedAbility>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantedAbility {
    pub costs: Vec<Cost>,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CounterKind {
    PlusOnePlusOne,
    MinusOneMinusOne,
    Loyalty,
    Indestructible,
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerToughnessChange {
    pub objects: ObjectRef,
    pub operation: PowerToughnessOperation,
    pub power: Amount,
    pub toughness: Amount,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowerToughnessOperation {
    Add,
    Subtract,
    AddPowerSubtractToughness,
    SubtractPowerAddToughness,
    SetBase,
    SetPower,
    SetToughness,
    Double,
    Switch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCharacteristics {
    pub object: ObjectRef,
    pub colors: Option<Vec<Color>>,
    pub card_types: Option<Vec<CardType>>,
    pub subtypes: Option<Vec<String>>,
    pub name: Option<String>,
    pub base_power: Option<Amount>,
    pub base_toughness: Option<Amount>,
    pub retain_other_card_types: bool,
    pub retain_other_subtypes: bool,
    pub retain_other_colors: bool,
    pub retain_other_names: bool,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Duration {
    Permanent,
    ThisTurn,
    UntilEndOfTurn,
    UntilEndOfNextTurn,
    WhileSourceOnBattlefield,
    WhileCondition(Box<Condition>),
    BeginningOfNextEndStep,
    BeginningOfNextTurnUpkeep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keyword {
    Deathtouch,
    Defender,
    DoubleStrike,
    FirstStrike,
    Flying,
    Haste,
    Hexproof,
    Indestructible,
    Lifelink,
    Menace,
    Reach,
    Shroud,
    Trample,
    Vigilance,
    Ward(Box<WardCost>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WardCost {
    Mana(ManaCost),
    PayLife(Amount),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restriction {
    SpellCannotBeCountered {
        object: ObjectRef,
    },
    SpellCannotBeCopied {
        object: ObjectRef,
    },
    MinimumX {
        object: ObjectRef,
        minimum: u32,
    },
    MatchingSpellsCannotBeCountered {
        player: PlayerRef,
        filter: ObjectFilter,
    },
    CannotCast {
        affected: PlayerRef,
        filter: ObjectFilter,
        duration: Duration,
        during_turn_of: Option<PlayerRef>,
    },
    DoesNotUntapDuring {
        object: ObjectRef,
        step: Step,
    },
    DoesNotUntapDuringIf {
        object: ObjectRef,
        step: Step,
        condition: Condition,
    },
    ManaSpendRestriction {
        source: ObjectRef,
        filter: ObjectFilter,
        makes_spell_uncounterable: bool,
    },
    ManaDoesNotEmpty {
        player: PlayerRef,
        duration: Duration,
    },
    AbilityUseLimit {
        object: ObjectRef,
        label: String,
        uses_per_turn: u16,
    },
    CannotCastNonManaSpellsWhileOnStack {
        affected: PlayerRef,
    },
    CannotActivateNonManaAbilitiesWhileOnStack {
        affected: PlayerRef,
    },
    ActivatedAbilitiesCannotBeActivated {
        object: ObjectRef,
        duration: Duration,
    },
    ActivatedAbilitiesOfMatchingSourcesCannotBeActivated {
        objects: ObjectFilter,
        except_mana_abilities: bool,
        duration: Duration,
    },
    MaximumHandSize {
        player: PlayerRef,
        maximum: Option<u32>,
    },
    PlayerCannotLoseGame {
        players: PlayerRef,
        duration: Duration,
    },
    PlayerCannotWinGame {
        players: PlayerRef,
        duration: Duration,
    },
    NonpositiveLifeDoesNotCauseLoss {
        players: PlayerRef,
        duration: Duration,
    },
    HandsRevealed {
        players: PlayerRef,
        duration: Duration,
    },
    SorcerySpeedCastingOnly {
        players: PlayerRef,
        duration: Duration,
    },
    AttackLimit {
        player: PlayerRef,
        amount: u16,
        duration: Duration,
    },
    AdditionalBlockCapacity {
        objects: ObjectRef,
        amount: u16,
        duration: Duration,
    },
    AdditionalLandPlays {
        player: PlayerRef,
        amount: u16,
        duration: Duration,
    },
    IgnoreSummoningSicknessForActivatedAbilities {
        objects: ObjectRef,
        duration: Duration,
    },
    LegendRuleDoesNotApply {
        player: PlayerRef,
    },
    MustAttackEachCombatIfAble {
        object: ObjectRef,
        duration: Duration,
    },
    CannotAttack {
        object: ObjectRef,
        duration: Duration,
    },
    AttackCost {
        attackers: ObjectFilter,
        attacked_player: PlayerRef,
        mana_per_attacker: ManaCost,
        duration: Duration,
    },
    EntersUntapped {
        objects: ObjectFilter,
        duration: Duration,
    },
    EnteringCreaturesDoNotCauseAbilitiesToTrigger {
        duration: Duration,
    },
    CannotBlock {
        object: ObjectRef,
        duration: Duration,
    },
    CannotBeBlocked {
        object: ObjectRef,
        duration: Duration,
    },
    CannotBeBlockedWhen {
        object: ObjectRef,
        condition: Condition,
        duration: Duration,
    },
    BlockerMustMatch {
        attacker: ObjectRef,
        blocker_filter: TargetFilter,
        duration: Duration,
    },
    CannotBlockMatching {
        blocker: ObjectRef,
        attacker_filter: ObjectFilter,
        duration: Duration,
    },
    CannotBlockObject {
        blocker: ObjectRef,
        attacker: ObjectRef,
        duration: Duration,
    },
    MustBlockIfAble {
        blockers: ObjectRef,
        attacker: Option<ObjectRef>,
        duration: Duration,
    },
    AssignCombatDamageUsingToughness {
        objects: ObjectRef,
        only_if_toughness_greater: bool,
        duration: Duration,
    },
    AssignCombatDamageAsThoughUnblocked {
        objects: ObjectRef,
        duration: Duration,
    },
    UntapLimit {
        player: PlayerRef,
        filter: ObjectFilter,
        amount: u16,
        step: Step,
    },
    TargetingProtection {
        object: ObjectRef,
        forbidden_controller: PlayerRef,
        duration: Duration,
    },
    ObjectsCannotBeTargeted {
        objects: ObjectFilter,
        duration: Duration,
    },
    PlayersCannotBeTargeted {
        players: PlayerRef,
        duration: Duration,
    },
    DestroyProtection {
        object: ObjectRef,
    },
    EnchantRestriction {
        filter: ObjectFilter,
    },
    AlternativeCastPermission(Box<AlternativeCastPermission>),
    PartnerCommanderPairing,
    PreparedCastPermission,
    SpellCommanderEligibility {
        limited_partner: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternativeCastPermission {
    pub object: ObjectRef,
    pub from: Zone,
    pub cost: AlternativeCost,
    pub timing: Trigger,
    pub condition: Option<Condition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlternativeCost {
    WithoutPayingManaCost,
    Mana(ManaCost),
    PrintedManaCost,
    Costs(Vec<Cost>),
    PrintedManaCostPlus(Vec<Cost>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntersTappedReplacement {
    pub object: ObjectRef,
    pub when: Option<Condition>,
    pub unless: Option<Condition>,
    pub optional_cost: Option<Cost>,
    pub optional_reveal: Option<ObjectFilter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementEffect {
    MultiplyEvent {
        event: ReplacementEvent,
        multiplier: u16,
    },
    IncreaseEvent {
        event: ReplacementEvent,
        addend: u16,
    },
    EntersTapped(Box<EntersTappedReplacement>),
    EnterAsCopy(CopyEffect),
    ConditionalTokenSubstitution {
        condition: Condition,
        ordinary: TokenCreation,
        replacement: Box<TokenCreation>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyEffect {
    pub destination: CopyDestination,
    pub original: ObjectRef,
    pub filter: ObjectFilter,
    pub exceptions: Vec<CopyException>,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyDestination {
    SourceAsItEnters,
    TokenControlledBy(PlayerRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyException {
    RetainSourceAbilities,
    SetName(String),
    AddLegendary,
    RemoveLegendary,
    AddCardType(CardType),
    AddSubtype(String),
    AddKeyword(Keyword),
    AddCounterIfType {
        card_type: CardType,
        counter: CounterKind,
        amount: Box<Amount>,
    },
    AddGrantedAbility(GrantedAbility),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimateEffect {
    pub object: ObjectRef,
    pub power: Amount,
    pub toughness: Amount,
    pub retain_printed_power_toughness: bool,
    pub colors: Vec<Color>,
    pub subtypes: Vec<String>,
    pub keywords: Vec<Keyword>,
    pub retain_land: bool,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastCopyEffect {
    pub source: ObjectRef,
    pub from: Zone,
    pub without_paying_mana_cost: bool,
    pub timing: Trigger,
    pub repeat: RepeatSchedule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepeatSchedule {
    Once,
    EachFirstMainPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomOrder {
    AnyOrder,
    ReplaySeededRandom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReminderSemantics {
    Composite(Vec<ReminderSemantics>),
    SpecialResourceExplanation(SpecialResourceCost),
    ManaNotationExplanation(ManaNotationExplanation),
    StandaloneAnnotation(Box<StandaloneOracleAnnotation>),
    KeywordExplanation(Keyword),
    KeywordExplanations(Vec<Keyword>),
    TreasureDefinition(Box<TokenDefinition>),
    FoodDefinition(Box<TokenDefinition>),
    ClueDefinition(Box<TokenDefinition>),
    BloodDefinition(Box<TokenDefinition>),
    GoldDefinition(Box<TokenDefinition>),
    LanderDefinition(Box<TokenDefinition>),
    CyclingProcedure {
        cost: ManaCost,
    },
    TypecyclingProcedure {
        type_name: String,
        cost: ManaCost,
        filter: Box<ObjectFilter>,
    },
    ProwessProcedure,
    TrampleExplanation,
    HexproofExplanation,
    PlayerShroudProcedure,
    IndestructibleExplanation,
    HideawayProcedure {
        amount: Amount,
    },
    SurveilProcedure {
        amount: Amount,
    },
    ScryProcedure {
        amount: Amount,
    },
    MillProcedure {
        player: PlayerRef,
        amount: Amount,
        optional: bool,
    },
    ManifestProcedure,
    CrewProcedure {
        required_power: Amount,
    },
    StationProcedure {
        creature_threshold: Option<u32>,
    },
    ExhaustProcedure,
    HistoricDefinition,
    EvokeProcedure {
        cost: ManaCost,
    },
    SplitSecondProcedure,
    PartnerProcedure,
    PreparedProcedure,
    SpellCommanderProcedure,
    ParadigmProcedure,
    UntapSymbolProcedure,
    StunCounterProcedure,
    ExertProcedure,
    BoastProcedure,
    EnergyCounterExplanation {
        amount: Amount,
    },
    ProliferateProcedure,
    AdventureProcedure,
    OmenProcedure,
    DevotionProcedure {
        color: Color,
    },
    FlashProcedure,
    AfterlifeProcedure {
        amount: Amount,
    },
    FlashbackProcedure,
    EscapeProcedure,
    DashProcedure {
        cost: ManaCost,
    },
    OutlastProcedure {
        cost: ManaCost,
    },
    GiftProcedure {
        token: Box<TokenDefinition>,
        tapped: bool,
    },
    GiftCardProcedure,
    ConniveProcedure,
    MobilizeProcedure {
        amount: Amount,
        token: Box<TokenDefinition>,
    },
    UndauntedProcedure,
    PartyComposition,
    TeamworkProcedure {
        minimum_power: Amount,
    },
    KickerSacrificeProcedure {
        amount: Amount,
        filter: Box<ObjectFilter>,
    },
    CollectEvidenceProcedure {
        minimum: Amount,
    },
    BlightProcedure {
        amount: Amount,
    },
    BeholdProcedure {
        subtype: String,
    },
    WaterbendProcedure {
        amount: Amount,
    },
    IncrementProcedure,
    FearProcedure,
    TransformOrigin {
        front_face_name: String,
    },
    CharacteristicLossExplanation,
    ZoneQualification {
        object: ObjectRef,
        zone: Zone,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaNotationExplanation {
    RepresentsManaType {
        symbol: TypedManaSymbol,
        mana_type: TypedManaColor,
    },
    PaymentAlternatives {
        symbol: TypedManaSymbol,
        alternatives: Vec<ManaNotationPaymentAlternative>,
    },
}

impl ManaNotationExplanation {
    fn symbol(&self) -> &TypedManaSymbol {
        match self {
            Self::RepresentsManaType { symbol, .. } | Self::PaymentAlternatives { symbol, .. } => {
                symbol
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaNotationPaymentAlternative {
    Mana(TypedManaSymbol),
    Life(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PredefinedArtifactTokenKind {
    Treasure,
    Food,
    Clue,
    Blood,
    Gold,
    Lander,
}

impl PredefinedArtifactTokenKind {
    const fn requires_fixed_amount(self) -> bool {
        matches!(self, Self::Clue | Self::Blood | Self::Gold)
    }

    fn definition(self) -> TokenDefinition {
        match self {
            Self::Treasure => treasure_definition(),
            Self::Food => food_definition(),
            Self::Clue => clue_definition(),
            Self::Blood => blood_definition(),
            Self::Gold => gold_definition(),
            Self::Lander => lander_definition(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenGrammaticalNumber {
    Singular,
    Plural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedPredefinedTokenCreation {
    kind: PredefinedArtifactTokenKind,
    number: TokenGrammaticalNumber,
}

#[derive(Debug, Clone)]
struct ParsedClause {
    ability_word: Option<String>,
    timing: Timing,
    conditions: Vec<Condition>,
    costs: Vec<Cost>,
    targets: Vec<Target>,
    effects: Vec<Effect>,
    activation_restriction: Option<ActivationRestriction>,
    reminder: Option<ReminderSemantics>,
    saga_lore_procedure: Option<SagaLoreProcedure>,
    predefined_token_creations: Vec<ParsedPredefinedTokenCreation>,
}

impl ParsedClause {
    fn new(timing: Timing) -> Self {
        Self {
            ability_word: None,
            timing,
            conditions: Vec::new(),
            costs: Vec::new(),
            targets: Vec::new(),
            effects: Vec::new(),
            activation_restriction: None,
            reminder: None,
            saga_lore_procedure: None,
            predefined_token_creations: Vec::new(),
        }
    }
}

pub fn compile_bounded_oracle_face(
    input: OracleFaceInput<'_>,
) -> Result<BoundedOracleFace, CompileError> {
    let mut clauses = input
        .oracle_clauses
        .iter()
        .enumerate()
        .map(|(clause_index, oracle_clause)| {
            compile_bounded_oracle_clause(OracleClauseInput {
                face_index: input.face_index,
                clause_index: clause_index as u16,
                source_name: input.source_name,
                source_type_line: input.source_type_line,
                oracle_clause,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut open_modal_header = None::<(usize, u16)>;
    let mut branch_count = 0u16;
    for index in 0..clauses.len() {
        match clauses[index].timing {
            Timing::ModalHeader { .. } | Timing::TriggeredModalHeader { .. } => {
                if open_modal_header.is_some() {
                    return Err(CompileError::MalformedModalGroup {
                        address: clauses[index].address,
                        detail: "a modal header began before the previous group received a branch",
                    });
                }
                open_modal_header = Some((index, clauses[index].address.clause_index));
                branch_count = 0;
            }
            Timing::ModalBranch { .. } => {
                let Some((header_index, header_clause_index)) = open_modal_header else {
                    return Err(CompileError::MalformedModalGroup {
                        address: clauses[index].address,
                        detail: "a branch has no immediately preceding modal header",
                    });
                };
                let branch_index = branch_count;
                branch_count = branch_count.saturating_add(1);
                clauses[index].timing = Timing::ModalBranch {
                    header_clause_index: Some(header_clause_index),
                    branch_index,
                };
                let header_digest = clauses[header_index].semantic_digest.clone();
                let branch_ordinal = branch_index.to_string();
                clauses[index].semantic_digest =
                    bounded_clause_semantic_digest_with_program_context(
                        &clauses[index].source_clause,
                        &clauses[index].normalized_clause,
                        &clauses[index].costs,
                        &clauses[index].conditions,
                        &clauses[index].effects,
                        clauses[index].reminder.as_ref(),
                        &["modal-branch", &header_digest, &branch_ordinal],
                    );
            }
            _ => {
                if let Some((header_index, _)) = open_modal_header.take()
                    && branch_count == 0
                {
                    return Err(CompileError::MalformedModalGroup {
                        address: clauses[header_index].address,
                        detail: "a modal header has no branches",
                    });
                }
            }
        }
    }
    if let Some((header_index, _)) = open_modal_header
        && branch_count == 0
    {
        return Err(CompileError::MalformedModalGroup {
            address: clauses[header_index].address,
            detail: "a modal header has no branches",
        });
    }

    bind_saga_lore_face_context(&input, &mut clauses)?;

    Ok(BoundedOracleFace {
        runtime_version: BOUNDED_ORACLE_RUNTIME_VERSION,
        face_index: input.face_index,
        clauses,
    })
}

fn bind_saga_lore_face_context(
    input: &OracleFaceInput<'_>,
    clauses: &mut [BoundedOracleClause],
) -> Result<(), CompileError> {
    let Some(first_saga_index) = clauses
        .iter()
        .position(BoundedOracleClause::requires_saga_lore_consumer)
    else {
        return Ok(());
    };
    let chapter_context = input
        .oracle_clauses
        .iter()
        .filter_map(|clause| {
            parse_saga_chapter_header(clause).map(|chapters| {
                (
                    normalize_oracle_clause(clause, input.source_name, input.source_type_line),
                    chapters,
                )
            })
        })
        .collect::<Vec<_>>();
    let highest_printed_chapter = chapter_context
        .iter()
        .flat_map(|(_, chapters)| chapters.iter().copied())
        .max()
        .ok_or_else(|| CompileError::UnsupportedSyntax {
            address: clauses[first_saga_index].address,
            normalized_clause: clauses[first_saga_index].normalized_clause.clone(),
        })?;

    let mut digest_context = Vec::with_capacity(chapter_context.len() + 1);
    digest_context.push("saga-face-context/v1".to_owned());
    digest_context.extend(
        chapter_context
            .iter()
            .map(|(normalized_clause, _)| normalized_clause.clone()),
    );
    let digest_context = digest_context
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();

    for clause in clauses
        .iter_mut()
        .filter(|clause| clause.requires_saga_lore_consumer())
    {
        let procedure = clause
            .saga_lore_procedure
            .as_mut()
            .expect("Saga consumer requirement is backed by a typed procedure");
        if let SagaFinalChapter::PrintedUnvalidated(printed) = procedure.final_chapter
            && printed != highest_printed_chapter
        {
            return Err(CompileError::UnsupportedSyntax {
                address: clause.address,
                normalized_clause: clause.normalized_clause.clone(),
            });
        }
        procedure.final_chapter =
            SagaFinalChapter::BoundHighestPrintedChapter(highest_printed_chapter);
        clause.semantic_digest = bounded_clause_semantic_digest_with_program_context(
            &clause.source_clause,
            &clause.normalized_clause,
            &clause.costs,
            &clause.conditions,
            &clause.effects,
            clause.reminder.as_ref(),
            &digest_context,
        );
    }
    Ok(())
}

pub fn compile_bounded_oracle_clause(
    input: OracleClauseInput<'_>,
) -> Result<BoundedOracleClause, CompileError> {
    let address = ClauseAddress {
        face_index: input.face_index,
        clause_index: input.clause_index,
    };
    let source_clause = input.oracle_clause.trim().to_owned();
    if source_clause.is_empty() {
        return Err(CompileError::EmptyClause { address });
    }
    let validated = validate_oracle_clause_line(input.oracle_clause)
        .map_err(|error| CompileError::MalformedSyntax { address, error })?;
    compile_bounded_oracle_clause_after_syntax_validation(input, validated)
}

pub fn compile_bounded_oracle_clause_with_context(
    input: OracleClauseInput<'_>,
    card_context: BoundedOracleCardContext<'_>,
) -> Result<BoundedOracleClause, CompileError> {
    let address = ClauseAddress {
        face_index: input.face_index,
        clause_index: input.clause_index,
    };
    if input.oracle_clause.trim().is_empty() {
        return Err(CompileError::EmptyClause { address });
    }
    let validated = validate_oracle_clause_line(input.oracle_clause)
        .map_err(|error| CompileError::MalformedSyntax { address, error })?;
    compile_bounded_oracle_clause_after_syntax_validation_with_context(
        input,
        validated,
        card_context,
    )
}

/// Compile only the native bounded grammar and annotation programs, without
/// consulting any standalone fallback compiler. Standalone compilers use this
/// gate to prove that ownership does not recurse back into themselves.
pub(crate) fn compile_bounded_oracle_clause_core(
    input: OracleClauseInput<'_>,
) -> Result<BoundedOracleClause, CompileError> {
    let address = ClauseAddress {
        face_index: input.face_index,
        clause_index: input.clause_index,
    };
    if input.oracle_clause.trim().is_empty() {
        return Err(CompileError::EmptyClause { address });
    }
    let validated = validate_oracle_clause_line(input.oracle_clause)
        .map_err(|error| CompileError::MalformedSyntax { address, error })?;
    compile_bounded_oracle_clause_with_standalone_fallbacks(input, validated, false, None)
}

/// Compile a clause whose exact source line has already passed the shared
/// canonical syntax gate. The unforgeable validation token is the source of
/// truth, so an internal caller cannot parse a different unchecked line.
pub(crate) fn compile_bounded_oracle_clause_after_syntax_validation(
    input: OracleClauseInput<'_>,
    validated: ValidatedOracleClauseLine<'_>,
) -> Result<BoundedOracleClause, CompileError> {
    compile_bounded_oracle_clause_with_standalone_fallbacks(input, validated, true, None)
}

pub(crate) fn compile_bounded_oracle_clause_after_syntax_validation_with_context(
    input: OracleClauseInput<'_>,
    validated: ValidatedOracleClauseLine<'_>,
    card_context: BoundedOracleCardContext<'_>,
) -> Result<BoundedOracleClause, CompileError> {
    compile_bounded_oracle_clause_with_standalone_fallbacks(
        input,
        validated,
        true,
        Some(card_context),
    )
}

pub(crate) fn retain_ability_clause_bridge_program(
    input: OracleClauseInput<'_>,
    program: AbilityClauseBridgeProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let address = ClauseAddress {
        face_index: input.face_index,
        clause_index: input.clause_index,
    };
    let source_clause = input.oracle_clause.trim();
    if source_clause.is_empty() {
        return Err(CompileError::EmptyClause { address });
    }
    if source_clause != input.oracle_clause || source_clause != program.exact_source() {
        return Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: normalize_oracle_clause(
                source_clause,
                input.source_name,
                input.source_type_line,
            ),
        });
    }

    let normalized_clause = program.normalized_source().to_owned();
    let effects = vec![Effect::StandaloneRuleProgram(
        StandaloneRuleProgram::AbilityClause(Box::new(program)),
    )];
    let semantic_digest = bounded_clause_semantic_digest_with_program_context(
        source_clause,
        &normalized_clause,
        &[],
        &[],
        &effects,
        None,
        &[],
    );
    Ok(BoundedOracleClause {
        runtime_version: BOUNDED_ORACLE_RUNTIME_VERSION,
        semantic_digest,
        address,
        source_clause: source_clause.to_owned(),
        normalized_clause,
        ability_word: None,
        timing: Timing::TypedStandaloneProgram,
        conditions: Vec::new(),
        costs: Vec::new(),
        targets: Vec::new(),
        effects,
        activation_restriction: None,
        reminder: None,
        saga_lore_procedure: None,
    })
}

pub(crate) fn retain_alternate_zone_cast_keyword_program(
    input: OracleClauseInput<'_>,
    program: AlternateZoneKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let timing = match program.kind() {
        AlternateZoneKeywordKind::Unearth(_) => Timing::Activated,
        AlternateZoneKeywordKind::Suspend(_)
        | AlternateZoneKeywordKind::Madness(_)
        | AlternateZoneKeywordKind::Escape(_)
        | AlternateZoneKeywordKind::ResidualFlashback(_) => Timing::TypedStandaloneProgram,
    };
    if program.production_adapter_connected()
        || compile_alternate_zone_cast_keyword_program(input.oracle_clause, input.source_type_line)
            .as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let normalized_clause = exact_source.clone();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_clause,
        timing,
        StandaloneRuleProgram::AlternateZoneCastKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_cast_modifier_keyword_program(
    input: OracleClauseInput<'_>,
    program: CastModifierKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let timing = match program.kind() {
        CastModifierKeywordKind::Buyback { .. }
        | CastModifierKeywordKind::Entwine { .. }
        | CastModifierKeywordKind::SpliceOntoArcane { .. } => Timing::CastingAdditionalCost,
        CastModifierKeywordKind::Overload { .. }
        | CastModifierKeywordKind::Replicate { .. }
        | CastModifierKeywordKind::Storm => Timing::TypedStandaloneProgram,
    };
    if program.production_adapter_connected()
        || compile_cast_modifier_keyword_program(input.oracle_clause).as_ref() != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let normalized_clause = program.normalized_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_clause,
        timing,
        StandaloneRuleProgram::CastModifierKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_attachment_filter_program(
    input: OracleClauseInput<'_>,
    source_layout: &str,
    program: AttachmentFilterProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let timing = match program.kind() {
        AttachmentFilterProgramKind::Enchant { .. } => Timing::Static,
        AttachmentFilterProgramKind::Equip { .. } => Timing::Activated,
        AttachmentFilterProgramKind::Bestow { .. } => Timing::TypedStandaloneProgram,
    };
    if program.production_adapter_connected()
        || compile_attachment_filter_program(AttachmentFilterCompilerInput {
            exact_oracle_clause: input.oracle_clause,
            source_type_line: input.source_type_line,
            source_layout,
        })
        .as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.identity().exact_oracle_clause().to_owned();
    let normalized_clause = program.identity().normalized_oracle_clause().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_clause,
        timing,
        StandaloneRuleProgram::AttachmentFilter(Box::new(program)),
    )
}

pub(crate) fn retain_delayed_counter_keyword_program(
    input: OracleClauseInput<'_>,
    program: DelayedCounterKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || compile_delayed_counter_keyword_program(input.oracle_clause, program.normalized_source())
            .as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let normalized_clause = program.normalized_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_clause,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::DelayedCounterKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_face_down_merge_keyword_program(
    input: OracleClauseInput<'_>,
    source_layout: &str,
    program: FaceDownMergeKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || classify_face_down_merge_snapshot_candidate(
            input.oracle_clause,
            input.source_type_line,
            source_layout,
        ) != Some(FaceDownMergeSnapshotCandidateClass::SupportedResidual)
        || compile_face_down_merge_keyword_program(
            input.oracle_clause,
            input.source_type_line,
            source_layout,
        )
        .as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let normalized_clause = program.normalized_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_clause,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::FaceDownMergeKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_combat_special_keyword_program(
    input: OracleClauseInput<'_>,
    program: CombatSpecialKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || compile_combat_special_keyword_program(input.oracle_clause, program.normalized_source())
            .as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let normalized_clause = program.normalized_source().to_owned();
    let timing = match program.kind() {
        CombatSpecialKeywordKind::Ninjutsu { .. }
        | CombatSpecialKeywordKind::Encore { .. }
        | CombatSpecialKeywordKind::Saddle { .. } => Timing::Activated,
    };
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_clause,
        timing,
        StandaloneRuleProgram::CombatSpecialKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_graveyard_transform_keyword_program(
    input: OracleClauseInput<'_>,
    source_context: &GraveyardTransformSourceSemanticContext,
    program: GraveyardTransformKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || program.source_context() != source_context
        || compile_graveyard_transform_keyword_program(input.oracle_clause, source_context).as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let timing = match program.kind() {
        GraveyardTransformKeywordKind::Disturb(_) => Timing::TypedStandaloneProgram,
        GraveyardTransformKeywordKind::Soulshift(_) => {
            Timing::Triggered(Box::new(Trigger::ObjectEvent {
                subject: TriggerSubject::Source,
                event: ObjectEventKind::Dies,
            }))
        }
        GraveyardTransformKeywordKind::Craft(_) => Timing::Activated,
    };
    retain_residual_standalone_program(
        input,
        &exact_source,
        &exact_source,
        timing,
        StandaloneRuleProgram::GraveyardTransformKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_level_progression_program(
    input: OracleClauseInput<'_>,
    source_layout: &str,
    face_count: usize,
    program: LevelProgressionProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let exact_source = input.oracle_clause.to_owned();
    let line_belongs_to_face = program
        .exact_oracle_text()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && *line != "//")
        .any(|line| line == exact_source);
    if program.has_exact_contract()
        && !crate::level_progression_runtime::level_progression_production_adapter_connected()
        && source_layout == program.exact_layout()
        && face_count == 1
        && input.source_type_line == program.exact_type_line()
        && line_belongs_to_face
    {
        return retain_residual_standalone_program(
            input,
            &exact_source,
            &exact_source,
            Timing::TypedStandaloneProgram,
            StandaloneRuleProgram::LevelProgression(Box::new(program)),
        );
    }
    Err(residual_program_rejected(&input))
}

pub(crate) fn retain_extended_cast_zone_keyword_program(
    input: OracleClauseInput<'_>,
    program: ExtendedCastZoneProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || classify_extended_cast_zone_snapshot_candidate(input.oracle_clause)
            != Some(ExtendedCastZoneSnapshotCandidateClass::SupportedFamily)
        || compile_extended_cast_zone_keyword_program(input.oracle_clause).as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &exact_source,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::ExtendedCastZoneKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_residual_cost_keyword_program(
    input: OracleClauseInput<'_>,
    program: ResidualCostKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let normalized_clause = normalize_oracle_clause(
        input.oracle_clause,
        input.source_name,
        input.source_type_line,
    );
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || program.normalized_source() != normalized_clause
        || compile_residual_cost_keyword_program(input.oracle_clause, &normalized_clause).as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let timing = match program.kind() {
        ResidualCostKeywordKind::Affinity(_) => Timing::Static,
        ResidualCostKeywordKind::Ward(_) => Timing::Triggered(Box::new(Trigger::BecomesTarget {
            object: ObjectRef::Source,
            controller: PlayerRef::Opponent,
            source_kinds: Vec::new(),
        })),
    };
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_clause,
        timing,
        StandaloneRuleProgram::ResidualCostKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_linked_cast_cost_keyword_program(
    input: OracleClauseInput<'_>,
    program: LinkedCastCostProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || compile_linked_cast_cost_keyword_program(input.oracle_clause, input.source_type_line)
            .as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &exact_source,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::LinkedCastCostKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_combat_trigger_keyword_program(
    input: OracleClauseInput<'_>,
    program: CombatTriggerKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let normalized_clause = reviewed_combat_trigger_normalized_source(input.oracle_clause);
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || program.normalized_source() != normalized_clause
        || compile_combat_trigger_keyword_program(input.oracle_clause, &normalized_clause).as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let timing = match program.kind() {
        CombatTriggerKeywordKind::Afflict { .. } | CombatTriggerKeywordKind::Rampage { .. } => {
            Timing::Triggered(Box::new(Trigger::ObjectEvent {
                subject: TriggerSubject::Source,
                event: ObjectEventKind::BecomesBlocked,
            }))
        }
        CombatTriggerKeywordKind::Annihilator { .. }
        | CombatTriggerKeywordKind::BattleCry
        | CombatTriggerKeywordKind::Dethrone
        | CombatTriggerKeywordKind::Melee
        | CombatTriggerKeywordKind::Provoke => Timing::Triggered(Box::new(Trigger::SourceAttacks)),
        CombatTriggerKeywordKind::Ingest => {
            Timing::Triggered(Box::new(Trigger::SourceCombatDamageToPlayer))
        }
        CombatTriggerKeywordKind::Skulk => Timing::Static,
    };
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_clause,
        timing,
        StandaloneRuleProgram::CombatTriggerKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_creature_counter_keyword_program(
    input: OracleClauseInput<'_>,
    program: CreatureCounterKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || compile_creature_counter_keyword_program(input.oracle_clause, input.source_type_line)
            .as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &exact_source,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::CreatureCounterKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_graveyard_hand_library_keyword_program(
    input: OracleClauseInput<'_>,
    source_mana_value: Option<u32>,
    program: ZoneKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let source_context = ZoneKeywordSourceSemanticContext {
        type_line: input.source_type_line,
        mana_value: source_mana_value,
    };
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || compile_zone_keyword_program(input.oracle_clause, source_context).as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &exact_source,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::GraveyardHandLibraryKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_cast_choice_keyword_program(
    input: OracleClauseInput<'_>,
    source_context: &CastChoiceSourceContext,
    program: CastChoiceKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || source_context.source_type_line != input.source_type_line
        || compile_cast_choice_keyword_program(input.oracle_clause, source_context).as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &exact_source,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::CastChoiceKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_static_special_keyword_program(
    input: OracleClauseInput<'_>,
    program: StaticSpecialKeywordProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let source_context = StaticSpecialSourceContext::from_type_line(input.source_type_line);
    let normalized_source = reviewed_static_special_normalized_source(input.oracle_clause);
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || compile_static_special_keyword_program(
            input.oracle_clause,
            &normalized_source,
            source_context,
        )
        .as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &exact_source,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::StaticSpecialKeyword(Box::new(program)),
    )
}

pub(crate) fn retain_common_action_procedure_program(
    input: OracleClauseInput<'_>,
    program: CommonActionProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let normalized_source = reviewed_common_action_normalized_source(input.oracle_clause);
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || compile_common_action_program(input.oracle_clause, &normalized_source).as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &exact_source,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::CommonActionProcedure(Box::new(program)),
    )
}

pub(crate) fn retain_regeneration_action_program(
    input: OracleClauseInput<'_>,
    program: RegenerationActionProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let normalized_source = collapse_whitespace(input.oracle_clause);
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || compile_regeneration_action_program(input.oracle_clause, &normalized_source).as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &exact_source,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::RegenerationAction(Box::new(program)),
    )
}

pub(crate) fn retain_oracle_action_program(
    input: OracleClauseInput<'_>,
    program: OracleActionProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || compile_oracle_action_program(OracleActionCompileInput {
            exact_source: input.oracle_clause,
            normalized_source: program.normalized_source(),
            semantic_context: program.semantic_context(),
        })
        .as_ref()
            != Ok(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let normalized_source = program.normalized_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_source,
        Timing::SpellResolution,
        StandaloneRuleProgram::OracleAction(Box::new(program)),
    )
}

pub(crate) fn retain_oracle_ability_envelope_program(
    input: OracleClauseInput<'_>,
    program: OracleAbilityEnvelopeProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || compile_oracle_ability_envelope(AbilityEnvelopeCompileInput {
            exact_source: input.oracle_clause,
            normalized_source: program.normalized_source(),
        })
        .as_ref()
            != Ok(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let normalized_source = program.normalized_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_source,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::OracleAbilityEnvelope(Box::new(program)),
    )
}

pub(crate) fn retain_oracle_static_replacement_program(
    input: OracleClauseInput<'_>,
    program: OracleStaticReplacementProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || compile_oracle_static_replacement_program(OracleStaticReplacementCompileInput {
            exact_source: input.oracle_clause,
            normalized_source: program.normalized_source(),
            semantic_context: program.semantic_context(),
        })
        .as_ref()
            != Ok(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let normalized_source = program.normalized_source().to_owned();
    let timing = match program.kind() {
        OracleStaticReplacementProgramKind::Static(_) => Timing::Static,
        OracleStaticReplacementProgramKind::Replacement(_) => Timing::Replacement,
    };
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_source,
        timing,
        StandaloneRuleProgram::OracleStaticReplacement(Box::new(program)),
    )
}

pub(crate) fn retain_oracle_cast_zone_envelope_program(
    input: OracleClauseInput<'_>,
    program: CastZoneEnvelopeProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || compile_cast_zone_envelope_program(
            input.oracle_clause,
            program.normalized_source(),
            program.context().clone(),
        )
        .as_ref()
            != Ok(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    let normalized_source = program.normalized_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &normalized_source,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::OracleCastZoneEnvelope(Box::new(program)),
    )
}

pub(crate) fn retain_oracle_face_modal_line_program(
    input: OracleClauseInput<'_>,
    program: OracleFaceModalLineProgram,
) -> Result<BoundedOracleClause, CompileError> {
    if program.production_adapter_connected()
        || program.exact_source() != input.oracle_clause
        || OracleFaceModalLineProgram::compile(
            program.exact_source(),
            program.role(),
            program.group().clone(),
        )
        .as_ref()
            != Some(&program)
    {
        return Err(residual_program_rejected(&input));
    }
    let exact_source = program.exact_source().to_owned();
    retain_residual_standalone_program(
        input,
        &exact_source,
        &exact_source,
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::OracleFaceModalLine(Box::new(program)),
    )
}

fn residual_program_rejected(input: &OracleClauseInput<'_>) -> CompileError {
    CompileError::UnsupportedSyntax {
        address: ClauseAddress {
            face_index: input.face_index,
            clause_index: input.clause_index,
        },
        normalized_clause: normalize_oracle_clause(
            input.oracle_clause,
            input.source_name,
            input.source_type_line,
        ),
    }
}

fn retain_residual_standalone_program(
    input: OracleClauseInput<'_>,
    exact_source: &str,
    normalized_clause: &str,
    timing: Timing,
    program: StandaloneRuleProgram,
) -> Result<BoundedOracleClause, CompileError> {
    let address = ClauseAddress {
        face_index: input.face_index,
        clause_index: input.clause_index,
    };
    let source_clause = input.oracle_clause.trim();
    if source_clause.is_empty() {
        return Err(CompileError::EmptyClause { address });
    }
    if source_clause != input.oracle_clause || source_clause != exact_source {
        return Err(residual_program_rejected(&input));
    }
    let effects = vec![Effect::StandaloneRuleProgram(program)];
    let semantic_digest = bounded_clause_semantic_digest_with_program_context(
        source_clause,
        normalized_clause,
        &[],
        &[],
        &effects,
        None,
        &[],
    );
    Ok(BoundedOracleClause {
        runtime_version: BOUNDED_ORACLE_RUNTIME_VERSION,
        semantic_digest,
        address,
        source_clause: source_clause.to_owned(),
        normalized_clause: normalized_clause.to_owned(),
        ability_word: None,
        timing,
        conditions: Vec::new(),
        costs: Vec::new(),
        targets: Vec::new(),
        effects,
        activation_restriction: None,
        reminder: None,
        saga_lore_procedure: None,
    })
}

pub(crate) fn retain_oracle_clause_composition_program(
    input: OracleClauseInput<'_>,
    typed: TypedOracleComposition,
    mut children: Vec<OracleCompositionChildBinding>,
) -> Result<BoundedOracleClause, CompileError> {
    let address = ClauseAddress {
        face_index: input.face_index,
        clause_index: input.clause_index,
    };
    let source_clause = input.oracle_clause.trim();
    if source_clause.is_empty() {
        return Err(CompileError::EmptyClause { address });
    }
    let normalized_clause =
        normalize_oracle_clause(source_clause, input.source_name, input.source_type_line);
    let invalid = || CompileError::UnsupportedSyntax {
        address,
        normalized_clause: normalized_clause.clone(),
    };
    if source_clause != input.oracle_clause
        || source_clause != typed.exact_oracle()
        || typed.production_adapter_connected()
        || typed.children().len() != children.len()
        || typed.requirements().len() != typed.requirement_child_indices().len()
    {
        return Err(invalid());
    }

    for child in &mut children {
        child.capabilities.sort();
        child.capabilities.dedup();
        let Some(expected_source) = child.span.slice(source_clause) else {
            return Err(invalid());
        };
        if expected_source != child.program.exact_source()
            || !child
                .program
                .semantic_digest()
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || child.program.semantic_digest().len() != 64
            || oracle_composition_child_contains_composition(&child.program)
        {
            return Err(invalid());
        }
    }

    let mut ordered = Vec::with_capacity(children.len());
    for expected in typed.children() {
        let matches = children
            .iter()
            .enumerate()
            .filter_map(|(index, child)| {
                (child.span == expected.span()
                    && child.program.exact_source() == expected.exact_source()
                    && child.program.semantic_digest() == expected.semantic_digest()
                    && child.capabilities == expected.capabilities())
                .then_some(index)
            })
            .collect::<Vec<_>>();
        let [index] = matches.as_slice() else {
            return Err(invalid());
        };
        ordered.push(children.remove(*index));
    }
    if !children.is_empty() {
        return Err(invalid());
    }

    let effects = vec![Effect::StandaloneRuleProgram(
        StandaloneRuleProgram::OracleComposition(Box::new(OracleCompositionProgram {
            typed,
            children: ordered,
        })),
    )];
    let semantic_digest = bounded_clause_semantic_digest_with_program_context(
        source_clause,
        &normalized_clause,
        &[],
        &[],
        &effects,
        None,
        &[],
    );
    Ok(BoundedOracleClause {
        runtime_version: BOUNDED_ORACLE_RUNTIME_VERSION,
        semantic_digest,
        address,
        source_clause: source_clause.to_owned(),
        normalized_clause,
        ability_word: None,
        timing: Timing::TypedStandaloneProgram,
        conditions: Vec::new(),
        costs: Vec::new(),
        targets: Vec::new(),
        effects,
        activation_restriction: None,
        reminder: None,
        saga_lore_procedure: None,
    })
}

fn oracle_composition_child_contains_composition(program: &OracleCompositionChildProgram) -> bool {
    match program {
        OracleCompositionChildProgram::Bounded(program) => {
            effects_contain_oracle_composition(program.effects())
        }
        OracleCompositionChildProgram::DelegatedKeyword(_) => false,
    }
}

fn effects_contain_oracle_composition(effects: &[Effect]) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::StandaloneRuleProgram(StandaloneRuleProgram::OracleComposition(_)) => true,
        Effect::Optional(nested) => effects_contain_oracle_composition(nested),
        Effect::Conditional {
            if_true, if_false, ..
        } => {
            effects_contain_oracle_composition(if_true)
                || effects_contain_oracle_composition(if_false)
        }
        Effect::GrantAbility { ability, .. } => {
            effects_contain_oracle_composition(&ability.effects)
        }
        _ => false,
    })
}

fn compile_bounded_oracle_clause_with_standalone_fallbacks(
    input: OracleClauseInput<'_>,
    validated: ValidatedOracleClauseLine<'_>,
    allow_standalone_fallbacks: bool,
    card_context: Option<BoundedOracleCardContext<'_>>,
) -> Result<BoundedOracleClause, CompileError> {
    let address = ClauseAddress {
        face_index: input.face_index,
        clause_index: input.clause_index,
    };
    let source_clause = validated.line().to_owned();
    let standalone_annotation = compile_standalone_oracle_annotation(validated.line());
    let normalized_clause =
        normalize_oracle_clause(validated.line(), input.source_name, input.source_type_line);
    let (without_reminder, reminder_text) = split_trailing_reminder(&normalized_clause);
    let (ability_word, body) = split_ability_word(without_reminder);
    let (mut parsed, mut standalone_owned) =
        match parse_complete_clause(address, body, input.source_type_line) {
            Ok(parsed) => (parsed, false),
            Err(_)
                if standalone_annotation.as_ref().is_some_and(|annotation| {
                    !matches!(
                        annotation.kind(),
                        StandaloneOracleAnnotationKind::SagaLifecycle(_)
                    ) || type_line_has_word(input.source_type_line, "saga")
                }) =>
            {
                let mut parsed = ParsedClause::new(Timing::Static);
                parsed.reminder = Some(ReminderSemantics::StandaloneAnnotation(Box::new(
                    standalone_annotation.expect("guard proves annotation exists"),
                )));
                (parsed, false)
            }
            Err(error) => {
                let standalone = allow_standalone_fallbacks
                    .then(|| {
                        compile_standalone_parsed_clause(
                            input.clone(),
                            validated.line(),
                            &normalized_clause,
                            card_context,
                        )
                    })
                    .flatten();
                let Some(parsed) = standalone else {
                    return Err(error);
                };
                let standalone_owned = matches!(
                    parsed.effects.as_slice(),
                    [Effect::StandaloneRuleProgram(
                        StandaloneRuleProgram::EntryChoiceKeyword(_)
                    )]
                );
                (parsed, standalone_owned)
            }
        };
    if !standalone_owned {
        parsed.ability_word = ability_word;
        if let Some(reminder_text) = reminder_text {
            if canonical_reminder_text(reminder_text)
                == "activate only if this object attacked this turn and only once each turn."
                && parsed
                    .ability_word
                    .as_deref()
                    .is_some_and(|word| word.eq_ignore_ascii_case("boast"))
            {
                if parsed.activation_restriction.is_some() {
                    return Err(CompileError::UnsupportedReminder {
                        address,
                        reminder: reminder_text.to_owned(),
                    });
                }
                parsed.conditions.push(Condition::YouAttackedThisTurn);
                parsed.activation_restriction = Some(ActivationRestriction::OnceEachTurn);
            }
            if canonical_reminder_text(reminder_text) == "activate each exhaust ability only once."
                && parsed
                    .ability_word
                    .as_deref()
                    .is_some_and(|word| word.eq_ignore_ascii_case("exhaust"))
            {
                parsed.activation_restriction = Some(match parsed.activation_restriction {
                    None => ActivationRestriction::Exhaust {
                        sorcery_timing_only: false,
                    },
                    Some(ActivationRestriction::SorceryTiming) => ActivationRestriction::Exhaust {
                        sorcery_timing_only: true,
                    },
                    Some(_) => {
                        return Err(CompileError::UnsupportedReminder {
                            address,
                            reminder: reminder_text.to_owned(),
                        });
                    }
                });
            }
            let omen_reminder = canonical_reminder_text(reminder_text)
                == "then shuffle this object into its owner's library."
                && type_line_has_word(input.source_type_line, "omen");
            if omen_reminder {
                let object = ObjectRef::Source;
                parsed.effects.extend([
                    Effect::MoveZone(ZoneMove {
                        object: object.clone(),
                        from: Some(Zone::Stack),
                        to: Zone::Library,
                        tapped: false,
                        face_down: false,
                        delayed_until: None,
                    }),
                    Effect::ShuffleLibrary {
                        player: PlayerRef::OwnerOf(Box::new(object)),
                    },
                ]);
                parsed.reminder = Some(ReminderSemantics::OmenProcedure);
            } else {
                match parse_reminder(address, body, reminder_text, &parsed) {
                    Ok(reminder) => {
                        if reminder == ReminderSemantics::AdventureProcedure {
                            parsed.effects.push(Effect::ExileSpellAfterResolution {
                                object: ObjectRef::Source,
                            });
                        }
                        parsed.reminder = Some(reminder);
                    }
                    Err(error) => {
                        let standalone = allow_standalone_fallbacks
                            .then(|| {
                                compile_entry_choice_keyword_parsed_clause(
                                    validated.line(),
                                    &normalized_clause,
                                )
                            })
                            .flatten();
                        let Some(standalone) = standalone else {
                            return Err(error);
                        };
                        parsed = standalone;
                        standalone_owned = true;
                    }
                }
            }
        }
    }
    debug_assert!(
        !standalone_owned || parsed.reminder.is_none(),
        "standalone programs own the complete Oracle line"
    );
    if parsed_has_disallowed_predefined_token_context(&parsed, body) {
        return Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause,
        });
    }
    if parsed_has_unbound_selected_toughness(&parsed) {
        return Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause,
        });
    }

    let semantic_digest = bounded_clause_semantic_digest_with_program_context(
        &source_clause,
        &normalized_clause,
        &parsed.costs,
        &parsed.conditions,
        &parsed.effects,
        parsed.reminder.as_ref(),
        &[],
    );
    Ok(BoundedOracleClause {
        runtime_version: BOUNDED_ORACLE_RUNTIME_VERSION,
        semantic_digest,
        address,
        source_clause,
        normalized_clause,
        ability_word: parsed.ability_word,
        timing: parsed.timing,
        conditions: parsed.conditions,
        costs: parsed.costs,
        targets: parsed.targets,
        effects: parsed.effects,
        activation_restriction: parsed.activation_restriction,
        reminder: parsed.reminder,
        saga_lore_procedure: parsed.saga_lore_procedure,
    })
}

fn compile_standalone_parsed_clause(
    input: OracleClauseInput<'_>,
    exact_clause: &str,
    normalized_clause: &str,
    card_context: Option<BoundedOracleCardContext<'_>>,
) -> Option<ParsedClause> {
    let (timing, program) = compile_standalone_rule_program(
        input.clone(),
        exact_clause,
        normalized_clause,
        card_context,
    )?;
    let mut parsed = ParsedClause::new(timing);
    if let StandaloneRuleProgram::DamageClause(damage) = &program {
        parsed.costs = damage.costs().to_vec();
        parsed.conditions = damage.conditions().to_vec();
        parsed.activation_restriction = damage.activation_restriction().cloned();
    }
    parsed.effects.push(Effect::StandaloneRuleProgram(program));
    Some(parsed)
}

fn compile_entry_choice_keyword_parsed_clause(
    exact_clause: &str,
    normalized_clause: &str,
) -> Option<ParsedClause> {
    let program = compile_entry_choice_keyword_program(exact_clause, normalized_clause)?;
    let mut parsed = ParsedClause::new(Timing::TypedStandaloneProgram);
    parsed.effects.push(Effect::StandaloneRuleProgram(
        StandaloneRuleProgram::EntryChoiceKeyword(Box::new(program)),
    ));
    Some(parsed)
}

fn compile_standalone_rule_program(
    input: OracleClauseInput<'_>,
    exact_clause: &str,
    normalized_clause: &str,
    card_context: Option<BoundedOracleCardContext<'_>>,
) -> Option<(Timing, StandaloneRuleProgram)> {
    compile_library_access_program(exact_clause)
        .map(|program| {
            (
                Timing::Static,
                StandaloneRuleProgram::LibraryAccess(Box::new(program)),
            )
        })
        .or_else(|| {
            compile_old_transform_program(exact_clause, normalized_clause).map(|program| {
                (
                    Timing::Triggered(Box::new(Trigger::BeginningOf {
                        player: TurnPlayer::EachPlayer,
                        step: Step::Upkeep,
                    })),
                    StandaloneRuleProgram::OldTransform(Box::new(program)),
                )
            })
        })
        .or_else(|| {
            compile_object_state_clause_program(exact_clause, normalized_clause).map(|program| {
                let timing = match program.kind() {
                    ObjectStateClauseKind::OptionalUntapDuringYourUntapStep => {
                        Timing::TypedStandaloneProgram
                    }
                    ObjectStateClauseKind::SelfGraveyardMoveBecomesExile
                    | ObjectStateClauseKind::EntersBattlefieldTapped => Timing::Replacement,
                };
                (
                    timing,
                    StandaloneRuleProgram::ObjectState(Box::new(program)),
                )
            })
        })
        .or_else(|| {
            compile_pregame_clause_program(exact_clause, normalized_clause).map(|program| {
                let timing = match program.kind() {
                    PregameClauseKind::OpeningHand(_) => {
                        Timing::SpecialAction(SpecialActionTiming::Pregame)
                    }
                    PregameClauseKind::ExplicitSelfCommanderPermission
                    | PregameClauseKind::RemoveFromDeckWithoutAnte
                    | PregameClauseKind::DeckCopyLimit(_) => Timing::TypedStandaloneProgram,
                };
                (timing, StandaloneRuleProgram::Pregame(Box::new(program)))
            })
        })
        .or_else(|| {
            compile_combat_restriction_program(exact_clause).map(|program| {
                (
                    Timing::Static,
                    StandaloneRuleProgram::CombatRestriction(Box::new(program)),
                )
            })
        })
        .or_else(|| {
            compile_attachment_entry_program(exact_clause, input.source_type_line).map(|program| {
                (
                    Timing::Triggered(Box::new(Trigger::SourceEnters)),
                    StandaloneRuleProgram::AttachmentEntry(Box::new(program)),
                )
            })
        })
        .or_else(|| {
            compile_saga_transform_standalone_program(
                &input,
                exact_clause,
                normalized_clause,
                card_context?,
            )
        })
        .or_else(|| {
            compile_damage_clause(DamageClauseInput {
                source_name: input.source_name,
                source_type_line: input.source_type_line,
                oracle_clause: exact_clause,
            })
            .ok()
            .map(|program| {
                (
                    program.timing().clone(),
                    StandaloneRuleProgram::DamageClause(Box::new(program)),
                )
            })
        })
        .or_else(|| compile_targeting_protection_standalone_program(&input, exact_clause))
        .or_else(|| {
            compile_entry_choice_keyword_program(exact_clause, normalized_clause).map(|program| {
                (
                    Timing::TypedStandaloneProgram,
                    StandaloneRuleProgram::EntryChoiceKeyword(Box::new(program)),
                )
            })
        })
}

fn compile_targeting_protection_standalone_program(
    input: &OracleClauseInput<'_>,
    exact_clause: &str,
) -> Option<(Timing, StandaloneRuleProgram)> {
    let program = compile_targeting_protection_program(exact_clause)?;
    let existing_keyword = match (program.recipient(), program.kind()) {
        (ProtectionRecipient::SourceObject, TargetingProtectionKind::Shroud) => Some("Shroud"),
        (ProtectionRecipient::SourceObject, TargetingProtectionKind::Hexproof { .. }) => {
            Some("Hexproof")
        }
        (ProtectionRecipient::SourceObject, TargetingProtectionKind::Protection { .. }) => {
            Some("Protection")
        }
        _ => None,
    };
    if existing_keyword.is_some_and(|printed_keyword| {
        compile_keyword_program(KeywordProgramInput {
            face_index: input.face_index,
            clause_index: input.clause_index,
            printed_keyword,
            oracle_fragment: Some(exact_clause),
        })
        .is_ok()
    }) {
        return None;
    }
    Some((
        Timing::TypedStandaloneProgram,
        StandaloneRuleProgram::TargetingProtection(Box::new(program)),
    ))
}

fn compile_saga_transform_standalone_program(
    input: &OracleClauseInput<'_>,
    exact_clause: &str,
    normalized_clause: &str,
    card_context: BoundedOracleCardContext<'_>,
) -> Option<(Timing, StandaloneRuleProgram)> {
    if !card_context.layout.eq_ignore_ascii_case("transform")
        || card_context.face_count != 2
        || input.face_index != 0
        || !type_line_has_word(input.source_type_line, "enchantment")
        || !type_line_has_word(input.source_type_line, "saga")
    {
        return None;
    }
    let source_context = SagaTransformSourceContext {
        layout: SagaTransformLayoutKind::TransformingDoubleFaced,
        face_count: 2,
        source_face: SagaTransformFaceRole::Front,
        source_face_is_enchantment: true,
        source_face_is_saga: true,
        source_face_is_permanent: true,
        transformed_face: SagaTransformFaceRole::Back,
        transformed_face_is_permanent: true,
        transformed_face_is_instant_or_sorcery: false,
    };
    compile_saga_transform_program(exact_clause, normalized_clause, source_context).map(|program| {
        (
            Timing::Triggered(Box::new(Trigger::SagaChapterReached { chapter: 3 })),
            StandaloneRuleProgram::SagaTransform(Box::new(program)),
        )
    })
}

fn type_line_has_word(type_line: &str, expected: &str) -> bool {
    type_line
        .split(|character: char| !character.is_alphanumeric())
        .any(|word| word.eq_ignore_ascii_case(expected))
}

fn bounded_clause_semantic_digest_with_program_context(
    source_clause: &str,
    normalized_clause: &str,
    costs: &[Cost],
    conditions: &[Condition],
    effects: &[Effect],
    reminder: Option<&ReminderSemantics>,
    group_context: &[&str],
) -> String {
    let mut context = bounded_program_semantic_context(costs, conditions, effects);
    context.extend(bounded_reminder_semantic_context(reminder));
    context.extend(
        group_context
            .iter()
            .map(|component| (*component).to_owned()),
    );
    let context = context.iter().map(String::as_str).collect::<Vec<_>>();
    bounded_clause_semantic_digest_with_context(source_clause, normalized_clause, &context)
}

fn bounded_reminder_semantic_context(reminder: Option<&ReminderSemantics>) -> Vec<String> {
    if let Some(annotation) = standalone_annotation(reminder) {
        return vec![
            "standalone-oracle-annotation/v1".to_owned(),
            STANDALONE_ORACLE_ANNOTATION_COMPILER_VERSION.to_owned(),
            STANDALONE_ORACLE_ANNOTATION_RUNTIME_VERSION.to_owned(),
            annotation.semantic_digest().to_owned(),
        ];
    }
    let mut explanations = Vec::new();
    collect_mana_notation_explanations(reminder, &mut explanations);
    if explanations.is_empty() {
        return Vec::new();
    }
    let mut context = vec!["bounded-oracle-mana-notation-reminder/v1".to_owned()];
    context.extend(explanations);
    context
}

fn standalone_annotation(
    reminder: Option<&ReminderSemantics>,
) -> Option<&StandaloneOracleAnnotation> {
    match reminder {
        Some(ReminderSemantics::StandaloneAnnotation(annotation)) => Some(annotation),
        Some(ReminderSemantics::Composite(reminders)) => reminders
            .iter()
            .find_map(|reminder| standalone_annotation(Some(reminder))),
        _ => None,
    }
}

fn collect_mana_notation_explanations(
    reminder: Option<&ReminderSemantics>,
    explanations: &mut Vec<String>,
) {
    match reminder {
        Some(ReminderSemantics::Composite(reminders)) => {
            for reminder in reminders {
                collect_mana_notation_explanations(Some(reminder), explanations);
            }
        }
        Some(ReminderSemantics::ManaNotationExplanation(explanation)) => {
            explanations.push(canonical_mana_notation_explanation(explanation));
        }
        _ => {}
    }
}

fn canonical_mana_notation_explanation(explanation: &ManaNotationExplanation) -> String {
    match explanation {
        ManaNotationExplanation::RepresentsManaType { symbol, mana_type } => format!(
            "represents|{}|{}",
            canonical_typed_mana_symbol(symbol),
            typed_mana_color_symbol(*mana_type)
        ),
        ManaNotationExplanation::PaymentAlternatives {
            symbol,
            alternatives,
        } => {
            let alternatives = alternatives
                .iter()
                .map(|alternative| match alternative {
                    ManaNotationPaymentAlternative::Mana(symbol) => {
                        format!("mana:{}", canonical_typed_mana_symbol(symbol))
                    }
                    ManaNotationPaymentAlternative::Life(amount) => format!("life:{amount}"),
                })
                .collect::<Vec<_>>()
                .join("|");
            format!(
                "payment|{}|{alternatives}",
                canonical_typed_mana_symbol(symbol)
            )
        }
    }
}

fn canonical_typed_mana_symbol(symbol: &TypedManaSymbol) -> String {
    match symbol {
        TypedManaSymbol::Generic(amount) => amount.to_string(),
        TypedManaSymbol::Color(color) => typed_mana_color_symbol(*color).to_owned(),
        TypedManaSymbol::VariableX => "X".to_owned(),
        TypedManaSymbol::Snow => "S".to_owned(),
        TypedManaSymbol::Hybrid(first, second) => format!(
            "{}/{}",
            typed_mana_color_symbol(*first),
            typed_mana_color_symbol(*second)
        ),
        TypedManaSymbol::GenericHybrid { generic, color } => {
            format!("{generic}/{}", typed_mana_color_symbol(*color))
        }
        TypedManaSymbol::Phyrexian(color) => {
            format!("{}/P", typed_mana_color_symbol(*color))
        }
    }
}

fn bounded_clause_semantic_digest_with_context(
    source_clause: &str,
    normalized_clause: &str,
    group_context: &[&str],
) -> String {
    bounded_clause_semantic_digest_with_versions_and_context(
        BOUNDED_ORACLE_COMPILER_VERSION,
        BOUNDED_ORACLE_RUNTIME_VERSION,
        source_clause,
        normalized_clause,
        group_context,
    )
}

fn bounded_clause_semantic_digest_with_versions_and_context(
    compiler_version: &str,
    runtime_version: &str,
    source_clause: &str,
    normalized_clause: &str,
    group_context: &[&str],
) -> String {
    let mut hasher = Sha256::new();
    for component in [
        b"bounded-oracle-semantic-content/v2".as_slice(),
        compiler_version.as_bytes(),
        runtime_version.as_bytes(),
        source_clause.as_bytes(),
        normalized_clause.as_bytes(),
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component);
    }
    hasher.update((group_context.len() as u64).to_le_bytes());
    for component in group_context {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn bounded_program_semantic_context(
    costs: &[Cost],
    conditions: &[Condition],
    effects: &[Effect],
) -> Vec<String> {
    let mut definitions = Vec::new();
    collect_predefined_token_definitions(effects, &mut definitions);
    let mut definitions = definitions
        .into_iter()
        .map(|definition| {
            canonical_predefined_token_context(definition).expect(
                "parser-owned predefined artifact tokens have a digestable canonical contract",
            )
        })
        .collect::<Vec<_>>();
    definitions.sort();
    definitions.dedup();
    let mut context = Vec::new();
    if !definitions.is_empty() {
        context.push("bounded-oracle-predefined-token-program/v1".to_owned());
        context.extend(definitions);
    }
    if program_depends_on_special_resource_runtime(costs, conditions, effects) {
        context.push("bounded-oracle-special-resource-program/v1".to_owned());
        context.push(SPECIAL_RESOURCE_RUNTIME_VERSION.to_owned());
    }
    let mut standalone_programs = Vec::new();
    collect_standalone_rule_program_context(effects, &mut standalone_programs);
    standalone_programs.sort();
    standalone_programs.dedup();
    if !standalone_programs.is_empty() {
        context.push("bounded-oracle-standalone-rule-program/v1".to_owned());
        context.extend(standalone_programs);
    }
    context
}

fn collect_standalone_rule_program_context(effects: &[Effect], context: &mut Vec<String>) {
    for effect in effects {
        match effect {
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::AbilityClause(program)) => {
                context.push(format!(
                    "ability-clause/v1/{}/{}/{}",
                    ABILITY_CLAUSE_BRIDGE_COMPILER_VERSION,
                    ABILITY_CLAUSE_BRIDGE_RUNTIME_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::OracleAbilityEnvelope(
                program,
            )) => {
                context.push(format!(
                    "oracle-ability-envelope/v1/{}/{}/{}/{}",
                    ORACLE_ABILITY_ENVELOPE_COMPILER_VERSION,
                    ORACLE_ABILITY_ENVELOPE_RUNTIME_VERSION,
                    ORACLE_ABILITY_ENVELOPE_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::OracleStaticReplacement(
                program,
            )) => {
                context.push(format!(
                    "oracle-static-replacement/v1/{}/{}/{}/{}",
                    ORACLE_STATIC_REPLACEMENT_COMPILER_VERSION,
                    ORACLE_STATIC_REPLACEMENT_RUNTIME_VERSION,
                    ORACLE_STATIC_REPLACEMENT_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::OracleCastZoneEnvelope(
                program,
            )) => {
                context.push(format!(
                    "oracle-cast-zone-envelope/v1/{}/{}/{}/{}",
                    ORACLE_CAST_ZONE_ENVELOPE_COMPILER_VERSION,
                    ORACLE_CAST_ZONE_ENVELOPE_RUNTIME_VERSION,
                    ORACLE_CAST_ZONE_ENVELOPE_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::AlternateZoneCastKeyword(
                program,
            )) => {
                context.push(format!(
                    "alternate-zone-cast-keyword/v1/{}/{}/{}/{}",
                    ALTERNATE_ZONE_CAST_COMPILER_VERSION,
                    ALTERNATE_ZONE_CAST_RUNTIME_VERSION,
                    ALTERNATE_ZONE_CAST_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::CastModifierKeyword(program)) => {
                context.push(format!(
                    "cast-modifier-keyword/v1/{}/{}/{}/{}",
                    CAST_MODIFIER_KEYWORD_COMPILER_VERSION,
                    CAST_MODIFIER_KEYWORD_RUNTIME_VERSION,
                    CAST_MODIFIER_KEYWORD_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::AttachmentFilter(program)) => {
                context.push(format!(
                    "attachment-filter/v1/{}/{}/{}/{}",
                    ATTACHMENT_FILTER_COMPILER_VERSION,
                    ATTACHMENT_FILTER_RUNTIME_VERSION,
                    ATTACHMENT_FILTER_RULES_CONTEXT_VERSION,
                    program.identity().semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::DelayedCounterKeyword(
                program,
            )) => {
                context.push(format!(
                    "delayed-counter-keyword/v1/{}/{}/{}/{}",
                    DELAYED_COUNTER_KEYWORD_COMPILER_VERSION,
                    DELAYED_COUNTER_KEYWORD_RUNTIME_VERSION,
                    DELAYED_COUNTER_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::LibraryAccess(program)) => {
                context.push(format!(
                    "library-access/v1/{}/{}/{}",
                    LIBRARY_ACCESS_COMPILER_VERSION,
                    LIBRARY_ACCESS_RUNTIME_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::OldTransform(program)) => {
                context.push(format!(
                    "old-transform/v1/{}/{}/{}",
                    OLD_TRANSFORM_COMPILER_VERSION,
                    OLD_TRANSFORM_RUNTIME_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::ObjectState(program)) => {
                context.push(format!(
                    "object-state/v1/{}/{}/{}/{}",
                    OBJECT_STATE_CLAUSE_COMPILER_VERSION,
                    OBJECT_STATE_CLAUSE_RUNTIME_VERSION,
                    OBJECT_STATE_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::Pregame(program)) => {
                context.push(format!(
                    "pregame/v1/{}/{}/{}/{}",
                    PREGAME_CLAUSE_COMPILER_VERSION,
                    PREGAME_CLAUSE_RUNTIME_VERSION,
                    PREGAME_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::CombatRestriction(program)) => {
                context.push(format!(
                    "combat-restriction/v1/{}/{}/{}",
                    COMBAT_RESTRICTION_COMPILER_VERSION,
                    COMBAT_RESTRICTION_RUNTIME_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::AttachmentEntry(program)) => {
                context.push(format!(
                    "attachment-entry/v1/{}/{}/{}",
                    ATTACHMENT_ENTRY_COMPILER_VERSION,
                    ATTACHMENT_ENTRY_RUNTIME_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::SagaTransform(program)) => {
                context.push(format!(
                    "saga-transform/v1/{}/{}/{}",
                    SAGA_TRANSFORM_COMPILER_VERSION,
                    SAGA_TRANSFORM_RUNTIME_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::DamageClause(program)) => {
                context.push(format!(
                    "damage-clause/v1/{}/{}/{}",
                    DAMAGE_CLAUSE_COMPILER_VERSION,
                    DAMAGE_TRANSACTION_RUNTIME_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::TargetingProtection(program)) => {
                context.push(format!(
                    "targeting-protection/v1/{}/{}/{}/{}",
                    TARGETING_PROTECTION_COMPILER_VERSION,
                    TARGETING_PROTECTION_RUNTIME_VERSION,
                    TARGETING_PROTECTION_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::EntryChoiceKeyword(program)) => {
                context.push(format!(
                    "entry-choice-keyword/v1/{}/{}/{}/{}",
                    ENTRY_CHOICE_KEYWORD_COMPILER_VERSION,
                    ENTRY_CHOICE_KEYWORD_RUNTIME_VERSION,
                    ENTRY_CHOICE_KEYWORD_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::FaceDownMergeKeyword(program)) => {
                context.push(format!(
                    "face-down-merge-keyword/v1/{}/{}/{}/{}",
                    FACE_DOWN_MERGE_KEYWORD_COMPILER_VERSION,
                    FACE_DOWN_MERGE_KEYWORD_RUNTIME_VERSION,
                    FACE_DOWN_MERGE_KEYWORD_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::CombatSpecialKeyword(program)) => {
                context.push(format!(
                    "combat-special-keyword/v1/{}/{}/{}/{}",
                    COMBAT_SPECIAL_KEYWORD_COMPILER_VERSION,
                    COMBAT_SPECIAL_KEYWORD_RUNTIME_VERSION,
                    COMBAT_SPECIAL_KEYWORD_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::GraveyardTransformKeyword(
                program,
            )) => {
                context.push(format!(
                    "graveyard-transform-keyword/v1/{}/{}/{}/{}",
                    GRAVEYARD_TRANSFORM_KEYWORD_COMPILER_VERSION,
                    GRAVEYARD_TRANSFORM_KEYWORD_RUNTIME_VERSION,
                    GRAVEYARD_TRANSFORM_KEYWORD_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::LevelProgression(program)) => {
                context.push(format!(
                    "level-progression/v1/{}/{}/{}/{}",
                    LEVEL_PROGRESSION_COMPILER_VERSION,
                    LEVEL_PROGRESSION_RUNTIME_VERSION,
                    LEVEL_PROGRESSION_RULES_CONTEXT_VERSION,
                    program.semantic_sha256()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::ExtendedCastZoneKeyword(
                program,
            )) => {
                context.push(format!(
                    "extended-cast-zone-keyword/v1/{}/{}/{}/{}",
                    EXTENDED_CAST_ZONE_COMPILER_VERSION,
                    EXTENDED_CAST_ZONE_RUNTIME_VERSION,
                    EXTENDED_CAST_ZONE_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::ResidualCostKeyword(program)) => {
                context.push(format!(
                    "residual-cost-keyword/v1/{}/{}/{}/{}",
                    RESIDUAL_COST_KEYWORD_COMPILER_VERSION,
                    RESIDUAL_COST_KEYWORD_RUNTIME_VERSION,
                    RESIDUAL_COST_KEYWORD_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::LinkedCastCostKeyword(
                program,
            )) => {
                context.push(format!(
                    "linked-cast-cost-keyword/v1/{}/{}/{}/{}",
                    LINKED_CAST_COST_COMPILER_VERSION,
                    LINKED_CAST_COST_RUNTIME_VERSION,
                    LINKED_CAST_COST_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::CombatTriggerKeyword(program)) => {
                context.push(format!(
                    "combat-trigger-keyword/v1/{}/{}/{}/{}",
                    COMBAT_TRIGGER_KEYWORD_COMPILER_VERSION,
                    COMBAT_TRIGGER_KEYWORD_RUNTIME_VERSION,
                    COMBAT_TRIGGER_KEYWORD_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::CreatureCounterKeyword(
                program,
            )) => {
                context.push(format!(
                    "creature-counter-keyword/v1/{}/{}/{}/{}",
                    CREATURE_COUNTER_COMPILER_VERSION,
                    CREATURE_COUNTER_RUNTIME_VERSION,
                    CREATURE_COUNTER_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::GraveyardHandLibraryKeyword(
                program,
            )) => {
                context.push(format!(
                    "graveyard-hand-library-keyword/v1/{}/{}/{}/{}",
                    ZONE_KEYWORD_COMPILER_VERSION,
                    ZONE_KEYWORD_RUNTIME_VERSION,
                    ZONE_KEYWORD_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::CastChoiceKeyword(program)) => {
                context.push(format!(
                    "cast-choice-keyword/v1/{}/{}/{}/{}",
                    CAST_CHOICE_KEYWORD_COMPILER_VERSION,
                    CAST_CHOICE_KEYWORD_RUNTIME_VERSION,
                    CAST_CHOICE_KEYWORD_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::StaticSpecialKeyword(program)) => {
                context.push(format!(
                    "static-special-keyword/v1/{}/{}/{}",
                    STATIC_SPECIAL_KEYWORD_COMPILER_VERSION,
                    STATIC_SPECIAL_KEYWORD_RUNTIME_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::CommonActionProcedure(
                program,
            )) => {
                context.push(format!(
                    "common-action-procedure/v1/{}/{}/{}/{}",
                    COMMON_ACTION_PROCEDURE_COMPILER_VERSION,
                    COMMON_ACTION_PROCEDURE_RUNTIME_VERSION,
                    COMMON_ACTION_PROCEDURE_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::RegenerationAction(program)) => {
                context.push(format!(
                    "regeneration-action/v1/{}/{}/{}/{}",
                    REGENERATION_ACTION_COMPILER_VERSION,
                    REGENERATION_ACTION_RUNTIME_VERSION,
                    REGENERATION_ACTION_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::OracleAction(program)) => {
                context.push(format!(
                    "oracle-action/v1/{}/{}/{}/{}",
                    ORACLE_ACTION_ALGEBRA_COMPILER_VERSION,
                    ORACLE_ACTION_ALGEBRA_RUNTIME_VERSION,
                    ORACLE_ACTION_ALGEBRA_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::OracleFaceModalLine(program)) => {
                context.push(format!(
                    "oracle-face-modal-line/v1/{}/{}/{}/{}",
                    ORACLE_FACE_PROGRAM_ASSEMBLER_COMPILER_VERSION,
                    ORACLE_FACE_PROGRAM_ASSEMBLER_RUNTIME_VERSION,
                    ORACLE_FACE_MODAL_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::StandaloneRuleProgram(StandaloneRuleProgram::OracleComposition(program)) => {
                context.push(format!(
                    "oracle-composition/v1/{}/{}/{}/{}",
                    ORACLE_CLAUSE_COMPOSITION_COMPILER_VERSION,
                    ORACLE_CLAUSE_COMPOSITION_RUNTIME_VERSION,
                    ORACLE_CLAUSE_COMPOSITION_RULES_CONTEXT_VERSION,
                    program.semantic_digest()
                ));
            }
            Effect::Optional(nested) => {
                collect_standalone_rule_program_context(nested, context);
            }
            Effect::Conditional {
                if_true, if_false, ..
            } => {
                collect_standalone_rule_program_context(if_true, context);
                collect_standalone_rule_program_context(if_false, context);
            }
            Effect::GrantAbility { ability, .. } => {
                collect_standalone_rule_program_context(&ability.effects, context);
            }
            _ => {}
        }
    }
}

fn program_depends_on_special_resource_runtime(
    costs: &[Cost],
    conditions: &[Condition],
    effects: &[Effect],
) -> bool {
    costs.iter().any(cost_depends_on_special_resource_runtime)
        || conditions
            .iter()
            .any(condition_depends_on_special_resource_runtime)
        || effects
            .iter()
            .any(effect_depends_on_special_resource_runtime)
}

fn cost_depends_on_special_resource_runtime(cost: &Cost) -> bool {
    matches!(cost, Cost::AtomicResource(_))
}

fn condition_depends_on_special_resource_runtime(condition: &Condition) -> bool {
    match condition {
        Condition::PaymentDeclined(cost) | Condition::PaymentAccepted(cost) => {
            cost_depends_on_special_resource_runtime(cost)
        }
        Condition::UnlessPaid { cost, .. } => cost_depends_on_special_resource_runtime(cost),
        _ => false,
    }
}

fn effect_depends_on_special_resource_runtime(effect: &Effect) -> bool {
    match effect {
        Effect::PayCost(cost) => cost_depends_on_special_resource_runtime(cost),
        Effect::Optional(effects) => effects
            .iter()
            .any(effect_depends_on_special_resource_runtime),
        Effect::Conditional {
            condition,
            if_true,
            if_false,
        } => {
            condition_depends_on_special_resource_runtime(condition)
                || if_true
                    .iter()
                    .any(effect_depends_on_special_resource_runtime)
                || if_false
                    .iter()
                    .any(effect_depends_on_special_resource_runtime)
        }
        Effect::GrantAbility { ability, .. } => {
            ability
                .costs
                .iter()
                .any(cost_depends_on_special_resource_runtime)
                || ability
                    .effects
                    .iter()
                    .any(effect_depends_on_special_resource_runtime)
        }
        Effect::SchedulePaymentOrLose(payment) => {
            cost_depends_on_special_resource_runtime(&payment.cost)
        }
        _ => false,
    }
}

fn parsed_contains_special_resource_symbol(
    parsed: &ParsedClause,
    symbol: SpecialCostSymbol,
) -> bool {
    parsed
        .costs
        .iter()
        .any(|cost| cost_contains_special_resource_symbol(cost, symbol))
        || parsed
            .conditions
            .iter()
            .any(|condition| condition_contains_special_resource_symbol(condition, symbol))
        || parsed
            .effects
            .iter()
            .any(|effect| effect_contains_special_resource_symbol(effect, symbol))
}

fn cost_contains_special_resource_symbol(cost: &Cost, symbol: SpecialCostSymbol) -> bool {
    matches!(
        cost,
        Cost::AtomicResource(cost) if atomic_resource_cost_contains(cost, symbol)
    )
}

fn condition_contains_special_resource_symbol(
    condition: &Condition,
    symbol: SpecialCostSymbol,
) -> bool {
    match condition {
        Condition::PaymentDeclined(cost) | Condition::PaymentAccepted(cost) => {
            cost_contains_special_resource_symbol(cost, symbol)
        }
        Condition::UnlessPaid { cost, .. } => cost_contains_special_resource_symbol(cost, symbol),
        _ => false,
    }
}

fn effect_contains_special_resource_symbol(effect: &Effect, symbol: SpecialCostSymbol) -> bool {
    match effect {
        Effect::PayCost(cost) => cost_contains_special_resource_symbol(cost, symbol),
        Effect::Optional(effects) => effects
            .iter()
            .any(|effect| effect_contains_special_resource_symbol(effect, symbol)),
        Effect::Conditional {
            condition,
            if_true,
            if_false,
        } => {
            condition_contains_special_resource_symbol(condition, symbol)
                || if_true
                    .iter()
                    .any(|effect| effect_contains_special_resource_symbol(effect, symbol))
                || if_false
                    .iter()
                    .any(|effect| effect_contains_special_resource_symbol(effect, symbol))
        }
        Effect::GrantAbility { ability, .. } => {
            ability
                .costs
                .iter()
                .any(|cost| cost_contains_special_resource_symbol(cost, symbol))
                || ability
                    .effects
                    .iter()
                    .any(|effect| effect_contains_special_resource_symbol(effect, symbol))
        }
        Effect::SchedulePaymentOrLose(payment) => {
            cost_contains_special_resource_symbol(&payment.cost, symbol)
        }
        _ => false,
    }
}

fn collect_predefined_token_definitions<'a>(
    effects: &'a [Effect],
    definitions: &mut Vec<&'a TokenDefinition>,
) {
    for effect in effects {
        match effect {
            Effect::CreateToken(creation)
            | Effect::CreateTokenAttached { creation, .. }
            | Effect::CreateTokenAndAttachSource { creation, .. }
            | Effect::CreateTokenWithDelayedMove { creation, .. } => {
                collect_predefined_token_definition(creation, definitions);
            }
            Effect::Optional(effects) => {
                collect_predefined_token_definitions(effects, definitions);
            }
            Effect::Conditional {
                if_true, if_false, ..
            } => {
                collect_predefined_token_definitions(if_true, definitions);
                collect_predefined_token_definitions(if_false, definitions);
            }
            Effect::GrantAbility { ability, .. } => {
                collect_predefined_token_definitions(&ability.effects, definitions);
            }
            Effect::Copy(copy) => {
                collect_copy_predefined_token_definitions(copy, definitions);
            }
            Effect::Replacement(replacement) => match replacement.as_ref() {
                ReplacementEffect::ConditionalTokenSubstitution {
                    ordinary,
                    replacement,
                    ..
                } => {
                    collect_predefined_token_definition(ordinary, definitions);
                    collect_predefined_token_definition(replacement, definitions);
                }
                ReplacementEffect::EnterAsCopy(copy) => {
                    collect_copy_predefined_token_definitions(copy, definitions);
                }
                ReplacementEffect::MultiplyEvent { .. }
                | ReplacementEffect::IncreaseEvent { .. }
                | ReplacementEffect::EntersTapped(_) => {}
            },
            _ => {}
        }
    }
}

fn collect_copy_predefined_token_definitions<'a>(
    copy: &'a CopyEffect,
    definitions: &mut Vec<&'a TokenDefinition>,
) {
    for exception in &copy.exceptions {
        if let CopyException::AddGrantedAbility(ability) = exception {
            collect_predefined_token_definitions(&ability.effects, definitions);
        }
    }
}

fn collect_predefined_token_definition<'a>(
    creation: &'a TokenCreation,
    definitions: &mut Vec<&'a TokenDefinition>,
) {
    if let TokenSpecification::Defined(definition) = &creation.specification
        && predefined_artifact_token_kind(definition).is_some()
    {
        definitions.push(definition);
    }
}

struct CanonicalContextBuilder {
    encoded: String,
}

impl CanonicalContextBuilder {
    fn new(schema: &str) -> Self {
        let mut builder = Self {
            encoded: String::new(),
        };
        builder.push(schema);
        builder
    }

    fn push(&mut self, component: &str) {
        self.encoded.push_str(&component.len().to_string());
        self.encoded.push(':');
        self.encoded.push_str(component);
    }

    fn finish(self) -> String {
        self.encoded
    }
}

fn canonical_predefined_token_context(definition: &TokenDefinition) -> Option<String> {
    let expected_kind = match definition.name.as_deref()? {
        "Treasure" => PredefinedArtifactTokenKind::Treasure,
        "Food" => PredefinedArtifactTokenKind::Food,
        "Clue" => PredefinedArtifactTokenKind::Clue,
        "Blood" => PredefinedArtifactTokenKind::Blood,
        "Gold" => PredefinedArtifactTokenKind::Gold,
        "Lander" => PredefinedArtifactTokenKind::Lander,
        _ => return None,
    };
    let mut context = CanonicalContextBuilder::new("canonical-predefined-token-definition/v1");
    context.push(match expected_kind {
        PredefinedArtifactTokenKind::Treasure => "treasure",
        PredefinedArtifactTokenKind::Food => "food",
        PredefinedArtifactTokenKind::Clue => "clue",
        PredefinedArtifactTokenKind::Blood => "blood",
        PredefinedArtifactTokenKind::Gold => "gold",
        PredefinedArtifactTokenKind::Lander => "lander",
    });
    encode_optional_contract_amount(&mut context, definition.power.as_ref())?;
    encode_optional_contract_amount(&mut context, definition.toughness.as_ref())?;
    context.push(&definition.colors.len().to_string());
    for color in &definition.colors {
        context.push(contract_color_tag(*color));
    }
    context.push(&definition.card_types.len().to_string());
    for card_type in &definition.card_types {
        context.push(contract_card_type_tag(*card_type));
    }
    context.push(&definition.subtypes.len().to_string());
    for subtype in &definition.subtypes {
        context.push(subtype);
    }
    if !definition.keywords.is_empty() {
        return None;
    }
    context.push("keywords:0");
    context.push(&definition.abilities.len().to_string());
    for ability in &definition.abilities {
        context.push("granted-ability");
        context.push(&ability.costs.len().to_string());
        for cost in &ability.costs {
            encode_token_contract_cost(&mut context, cost)?;
        }
        context.push(&ability.effects.len().to_string());
        for effect in &ability.effects {
            encode_token_contract_effect(&mut context, effect)?;
        }
    }
    Some(context.finish())
}

fn encode_optional_contract_amount(
    context: &mut CanonicalContextBuilder,
    amount: Option<&Amount>,
) -> Option<()> {
    match amount {
        None => context.push("none"),
        Some(amount) => {
            context.push("some");
            encode_contract_amount(context, amount)?;
        }
    }
    Some(())
}

fn encode_contract_amount(context: &mut CanonicalContextBuilder, amount: &Amount) -> Option<()> {
    match amount {
        Amount::Constant(value) => {
            context.push("constant");
            context.push(&value.to_string());
        }
        Amount::X => context.push("x"),
        Amount::OneOrMore => context.push("one-or-more"),
        Amount::Any => context.push("any"),
        Amount::Twice(amount) => {
            context.push("twice");
            encode_contract_amount(context, amount)?;
        }
        Amount::Product { factor, value } => {
            context.push("product");
            context.push(&factor.to_string());
            encode_contract_amount(context, value)?;
        }
        Amount::UpTo(amount) => {
            context.push("up-to");
            encode_contract_amount(context, amount)?;
        }
        Amount::Count(_) => return None,
    }
    Some(())
}

fn encode_token_contract_cost(context: &mut CanonicalContextBuilder, cost: &Cost) -> Option<()> {
    match cost {
        Cost::Mana(ManaCost(cost)) => {
            context.push("mana");
            context.push(cost);
        }
        Cost::Tap(ObjectRef::Source) => context.push("tap-source"),
        Cost::SacrificeObject(ObjectRef::Source) => context.push("sacrifice-source"),
        Cost::DiscardSelection(selection) => {
            let expected_filter = ObjectFilter {
                zones: vec![Zone::Hand],
                owner: Some(PlayerRef::You),
                ..ObjectFilter::default()
            };
            if selection.chooser != PlayerRef::You || selection.filter != expected_filter {
                return None;
            }
            context.push("discard-selection");
            context.push(&selection.id.to_string());
            encode_contract_target_amount(context, &selection.amount);
        }
        _ => return None,
    }
    Some(())
}

fn encode_token_contract_effect(
    context: &mut CanonicalContextBuilder,
    effect: &Effect,
) -> Option<()> {
    match effect {
        Effect::Draw {
            player: PlayerRef::You,
            amount,
            optional,
            delayed_until: None,
        } => {
            context.push("draw-you");
            encode_contract_amount(context, amount)?;
            context.push(if *optional { "optional" } else { "required" });
        }
        Effect::GainLife {
            player: PlayerRef::You,
            amount,
        } => {
            context.push("gain-life-you");
            encode_contract_amount(context, amount)?;
        }
        Effect::AddMana(production)
            if production.player == PlayerRef::You
                && production.scales_with.is_none()
                && production.typed.is_none() =>
        {
            context.push("add-mana-you");
            context.push(&production.choices.len().to_string());
            for choice in &production.choices {
                context.push(&choice.symbols.len().to_string());
                for color in &choice.symbols {
                    context.push(contract_color_tag(*color));
                }
            }
            encode_contract_amount(context, &production.amount)?;
            context.push(if production.commander_identity_only {
                "commander-identity-only"
            } else {
                "unrestricted-colors"
            });
        }
        Effect::SearchLibrary(search) => {
            let expected = SearchLibrary {
                player: PlayerRef::You,
                chooser: PlayerRef::You,
                optional: false,
                allow_fail_to_find: true,
                amount: Amount::Constant(1),
                predicate: ObjectFilter {
                    zones: vec![Zone::Library],
                    card_types: vec![CardType::Land],
                    supertypes: vec![Supertype::Basic],
                    ..ObjectFilter::default()
                },
                reveal: false,
                destinations: vec![SearchDestination {
                    selected_ordinal: SearchOrdinal::Each,
                    zone: Zone::Battlefield,
                    tapped: true,
                }],
                shuffle_before_destination: false,
                shuffle_after: true,
            };
            if search != &expected {
                return None;
            }
            context.push("search-basic-land-to-battlefield-tapped-then-shuffle");
        }
        _ => return None,
    }
    Some(())
}

fn encode_contract_target_amount(context: &mut CanonicalContextBuilder, amount: &TargetAmount) {
    match amount {
        TargetAmount::Exactly(value) => {
            context.push("exactly");
            context.push(&value.to_string());
        }
        TargetAmount::UpTo(value) => {
            context.push("up-to");
            context.push(&value.to_string());
        }
        TargetAmount::AnyNumber => context.push("any-number"),
        TargetAmount::All => context.push("all"),
    }
}

const fn contract_color_tag(color: Color) -> &'static str {
    match color {
        Color::White => "white",
        Color::Blue => "blue",
        Color::Black => "black",
        Color::Red => "red",
        Color::Green => "green",
        Color::Colorless => "colorless",
    }
}

const fn contract_card_type_tag(card_type: CardType) -> &'static str {
    match card_type {
        CardType::Artifact => "artifact",
        CardType::Battle => "battle",
        CardType::Creature => "creature",
        CardType::Enchantment => "enchantment",
        CardType::Instant => "instant",
        CardType::Land => "land",
        CardType::Planeswalker => "planeswalker",
        CardType::Sorcery => "sorcery",
        CardType::Spell => "spell",
        CardType::Permanent => "permanent",
    }
}

pub fn normalize_oracle_clause(clause: &str, source_name: &str, source_type_line: &str) -> String {
    let mut normalized = collapse_whitespace(&clause.trim().replace('\u{2019}', "'"));
    let lower = normalized.to_ascii_lowercase();
    if lower.starts_with("(transforms from ")
        && lower.ends_with(".)")
        && !normalized["(Transforms from ".len()..normalized.len() - 2]
            .trim()
            .is_empty()
    {
        return normalized;
    }
    let aliases = source_aliases(source_name);
    for alias in aliases {
        normalized = replace_source_reference(&normalized, &alias);
    }
    if let Some(stem) = contextual_source_stem(source_name) {
        normalized = replace_ascii_case_insensitive(
            &normalized,
            &format!("have {stem} enter"),
            "have this object enter",
        );
        normalized = replace_ascii_case_insensitive(
            &normalized,
            &format!("{stem}'s other abilities"),
            "this object's other abilities",
        );
    }

    let source_types = words(source_type_line);
    for printed_type in [
        "artifact",
        "battle",
        "creature",
        "enchantment",
        "land",
        "planeswalker",
        "vehicle",
        "aura",
    ] {
        if source_types.iter().any(|word| word == printed_type) {
            normalized = replace_ascii_case_insensitive(
                &normalized,
                &format!("this {printed_type}"),
                "this object",
            );
        }
    }
    normalized = replace_ascii_case_insensitive(&normalized, "this permanent", "this object");
    normalized = replace_ascii_case_insensitive(&normalized, "this card", "this object");
    normalized = replace_ascii_case_insensitive(&normalized, "this spell", "this object spell");
    collapse_whitespace(&normalized)
}

type ClauseParser = fn(ClauseAddress, &str, &str) -> Option<Result<ParsedClause, CompileError>>;

fn parse_complete_clause(
    address: ClauseAddress,
    body: &str,
    source_type_line: &str,
) -> Result<ParsedClause, CompileError> {
    let trimmed = body.trim();
    let lower = trimmed.to_ascii_lowercase();

    let parsers: [ClauseParser; 16] = [
        parse_modal_clause,
        parse_transform_annotation_clause,
        parse_saga_lore_clause,
        parse_parenthetical_activated_clause,
        parse_additional_cast_cost_clause,
        parse_common_oracle_family_clause,
        parse_keyword_clause,
        parse_prepared_clause,
        parse_entry_copy_clause,
        parse_replacement_clause,
        parse_activated_clause,
        parse_exert_attack_clause,
        parse_triggered_clause,
        parse_static_clause,
        parse_duration_leading_clause,
        parse_resolution_clause,
    ];
    for parser in parsers {
        if let Some(result) = parser(address, trimmed, source_type_line) {
            return result;
        }
    }

    Err(CompileError::UnsupportedSyntax {
        address,
        normalized_clause: lower,
    })
}

fn source_aliases(source_name: &str) -> Vec<String> {
    let source_name = collapse_whitespace(source_name.trim());
    if source_name.is_empty() {
        return Vec::new();
    }
    let mut aliases = vec![source_name.clone()];
    if let Some(rebalanced_name) = source_name.strip_prefix("A-") {
        aliases.push(rebalanced_name.trim().to_string());
    }
    if let Some((front, _)) = source_name.split_once(" // ") {
        aliases.push(front.trim().to_string());
    }
    if let Some((short, _)) = source_name.split_once(',') {
        let short = short.trim();
        if !short.is_empty() {
            aliases.push(short.to_string());
        }
    }
    aliases.sort_by_key(|alias| std::cmp::Reverse(alias.len()));
    aliases.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    aliases
}

fn contextual_source_stem(source_name: &str) -> Option<&str> {
    let mut words = source_name.split_whitespace();
    let first = words
        .next()?
        .trim_matches(|character: char| !character.is_alphanumeric());
    words.next()?;
    (!matches!(first.to_ascii_lowercase().as_str(), "a" | "an" | "the")).then_some(first)
}

fn replace_source_reference(source: &str, alias: &str) -> String {
    if alias.is_empty() {
        return source.to_string();
    }
    let mut cursor = 0usize;
    let mut output = String::with_capacity(source.len());
    while let Some(relative) = source[cursor..].find(alias) {
        let start = cursor + relative;
        let end = start + alias.len();
        let left_boundary = start == 0
            || !source[..start]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_alphanumeric());
        let right_boundary = end == source.len()
            || !source[end..]
                .chars()
                .next()
                .is_some_and(|character| character.is_alphanumeric());
        let preceding = source[..start].trim_end().to_ascii_lowercase();
        let named_possessive = preceding.ends_with("named") && source[end..].starts_with("'s ");
        let assigned_name = preceding.ends_with("its name is");
        if left_boundary && right_boundary && !named_possessive && !assigned_name {
            output.push_str(&source[cursor..start]);
            output.push_str("this object");
            cursor = end;
        } else {
            output.push_str(&source[cursor..end]);
            cursor = end;
        }
    }
    output.push_str(&source[cursor..]);
    output
}

fn replace_ascii_case_insensitive(source: &str, needle: &str, replacement: &str) -> String {
    let lower = source.to_ascii_lowercase();
    let needle_lower = needle.to_ascii_lowercase();
    let mut cursor = 0usize;
    let mut output = String::with_capacity(source.len());
    while let Some(relative) = lower[cursor..].find(&needle_lower) {
        let start = cursor + relative;
        let end = start + needle.len();
        output.push_str(&source[cursor..start]);
        output.push_str(replacement);
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    output
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn words(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn split_ability_word(clause: &str) -> (Option<String>, &str) {
    let Some((label, rules)) = clause.split_once(" \u{2014} ") else {
        return (None, clause);
    };
    let label_is_word = !label.is_empty()
        && label.chars().all(|character| {
            character.is_alphabetic() || character.is_whitespace() || character == '\''
        });
    if label_is_word && !rules.trim().is_empty() {
        (Some(label.to_string()), rules)
    } else {
        (None, clause)
    }
}

fn starts_trigger(lower: &str) -> bool {
    lower.starts_with("when ")
        || lower.starts_with("whenever ")
        || lower.starts_with("at the beginning of ")
}

fn split_trailing_reminder(clause: &str) -> (&str, Option<&str>) {
    if clause.starts_with('(') && matching_close_paren(clause, 0) == Some(clause.len() - 1) {
        return (clause, None);
    }
    let bytes = clause.as_bytes();
    let mut depth = 0u16;
    let mut quoted = false;
    let mut candidate = None;
    for (index, character) in clause.char_indices() {
        match character {
            '"' => quoted = !quoted,
            '(' if !quoted => {
                if depth == 0 && index > 0 && bytes[index - 1].is_ascii_whitespace() {
                    candidate = Some(index);
                }
                depth = depth.saturating_add(1);
            }
            ')' if !quoted => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    if depth == 0
        && clause.ends_with(')')
        && let Some(start) = candidate
    {
        return (
            clause[..start].trim_end(),
            Some(clause[start + 1..clause.len() - 1].trim()),
        );
    }
    (clause, None)
}

fn matching_close_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0u16;
    let mut quoted = false;
    for (index, character) in text.char_indices().filter(|(index, _)| *index >= open) {
        match character {
            '"' => quoted = !quoted,
            '(' if !quoted => depth = depth.saturating_add(1),
            ')' if !quoted => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_top_level(text: &str, needle: char) -> Option<usize> {
    let mut paren_depth = 0u16;
    let mut quoted = false;
    for (index, character) in text.char_indices() {
        match character {
            '"' => quoted = !quoted,
            '(' if !quoted => paren_depth = paren_depth.saturating_add(1),
            ')' if !quoted => paren_depth = paren_depth.saturating_sub(1),
            _ if character == needle && !quoted && paren_depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

fn find_top_level_phrases(text: &str, needle: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut paren_depth = 0u16;
    let mut quoted = false;
    for (index, character) in text.char_indices() {
        match character {
            '"' => quoted = !quoted,
            '(' if !quoted => paren_depth = paren_depth.saturating_add(1),
            ')' if !quoted => paren_depth = paren_depth.saturating_sub(1),
            _ if !quoted && paren_depth == 0 && text[index..].starts_with(needle) => {
                positions.push(index);
            }
            _ => {}
        }
    }
    positions
}

fn split_top_level_sentences(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0u16;
    let mut quoted = false;
    for (index, character) in text.char_indices() {
        match character {
            '"' => quoted = !quoted,
            '(' if !quoted => paren_depth = paren_depth.saturating_add(1),
            ')' if !quoted => paren_depth = paren_depth.saturating_sub(1),
            '.' if !quoted && paren_depth == 0 => {
                let end = index + 1;
                let statement = text[start..end].trim();
                if !statement.is_empty() {
                    result.push(statement);
                }
                start = end;
            }
            _ => {}
        }
    }
    let remainder = text[start..].trim();
    if !remainder.is_empty() {
        result.push(remainder);
    }
    result
}

fn parse_reminder(
    address: ClauseAddress,
    body: &str,
    reminder: &str,
    parsed: &ParsedClause,
) -> Result<ReminderSemantics, CompileError> {
    let lower = reminder.to_ascii_lowercase();
    let body_lower = body.to_ascii_lowercase();
    let canonical = canonical_reminder_text(reminder);

    const HISTORIC_DEFINITION: &str = "artifacts, legendaries, and sagas are historic.";
    if canonical == HISTORIC_DEFINITION && parsed_has_historic_filter(parsed) {
        return Ok(ReminderSemantics::HistoricDefinition);
    }
    if let Some(remaining) = canonical
        .strip_prefix(HISTORIC_DEFINITION)
        .map(str::trim)
        .filter(|remaining| !remaining.is_empty())
        && parsed_has_historic_filter(parsed)
    {
        let remaining = parse_reminder(address, body, remaining, parsed)?;
        return Ok(ReminderSemantics::Composite(vec![
            ReminderSemantics::HistoricDefinition,
            remaining,
        ]));
    }

    let has_exert = parsed.effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::Optional(effects)
                if matches!(effects.first(), Some(Effect::Exert { object: ObjectRef::Source }))
        )
    });
    if has_exert {
        for exert_explanation in [
            "an exerted creature won't untap during your next untap step.",
            "it won't untap during your next untap step.",
        ] {
            if canonical == exert_explanation {
                return Ok(ReminderSemantics::ExertProcedure);
            }
            if let Some(remaining) = canonical
                .strip_prefix(exert_explanation)
                .map(str::trim)
                .filter(|remaining| !remaining.is_empty())
            {
                let remaining = parse_reminder(address, body, remaining, parsed)?;
                return Ok(ReminderSemantics::Composite(vec![
                    ReminderSemantics::ExertProcedure,
                    remaining,
                ]));
            }
        }
    }

    if canonical == "activate only if this object attacked this turn and only once each turn."
        && parsed
            .ability_word
            .as_deref()
            .is_some_and(|word| word.eq_ignore_ascii_case("boast"))
        && parsed.conditions.contains(&Condition::YouAttackedThisTurn)
        && parsed.activation_restriction == Some(ActivationRestriction::OnceEachTurn)
    {
        return Ok(ReminderSemantics::BoastProcedure);
    }
    if canonical == "activate each exhaust ability only once."
        && parsed
            .ability_word
            .as_deref()
            .is_some_and(|word| word.eq_ignore_ascii_case("exhaust"))
        && matches!(
            parsed.activation_restriction,
            Some(ActivationRestriction::Exhaust { .. })
        )
    {
        return Ok(ReminderSemantics::ExhaustProcedure);
    }

    let energy_amount = canonical
        .strip_suffix(" energy counter.")
        .or_else(|| canonical.strip_suffix(" energy counters."))
        .and_then(parse_english_amount);
    if let Some(amount) = energy_amount
        && parsed.effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::PutPlayerCounter {
                    player: PlayerRef::You,
                    counter,
                    amount: effect_amount,
                } if counter == "energy" && effect_amount == &amount
            )
        })
    {
        return Ok(ReminderSemantics::EnergyCounterExplanation { amount });
    }

    if canonical == "return it only if it's on the battlefield."
        && matches!(
            parsed.effects.as_slice(),
            [Effect::MoveZone(ZoneMove {
                object: ObjectRef::Source,
                from: Some(Zone::Battlefield),
                to: Zone::Hand,
                ..
            })]
        )
    {
        return Ok(ReminderSemantics::ZoneQualification {
            object: ObjectRef::Source,
            zone: Zone::Battlefield,
        });
    }

    if let Some(amount_text) = body_lower
        .trim()
        .trim_end_matches('.')
        .strip_prefix("hideaway ")
        && let Some(amount) = parse_english_amount(amount_text)
        && let Amount::Constant(value) = amount
        && let Some(word) = english_constant_text(value)
    {
        let procedure = format!(
            "look at the top {word} cards of your library, exile one face down, then put the rest on the bottom in a random order."
        );
        let exact = ["object", "land", "enchantment", "permanent"]
            .into_iter()
            .any(|kind| canonical == format!("when this {kind} enters, {procedure}"));
        if exact
            && matches!(
                parsed.effects.as_slice(),
                [
                    Effect::LookAtTop {
                        player: PlayerRef::You,
                        amount: Amount::Constant(looked),
                    },
                    Effect::SelectFromLookedAt {
                        player: PlayerRef::You,
                        amount: Amount::Constant(1),
                        reveal: false,
                        face_down: true,
                        destination: Zone::Exile,
                        ..
                    },
                    Effect::PutRestOnLibraryBottom {
                        player: PlayerRef::You,
                        order: BottomOrder::ReplaySeededRandom,
                    },
                ] if *looked == value
            )
        {
            return Ok(ReminderSemantics::HideawayProcedure {
                amount: Amount::Constant(value),
            });
        }
    }

    if body_lower.trim() == "you have shroud."
        && canonical == "you can't be the target of spells or abilities."
        && matches!(
            parsed.effects.as_slice(),
            [Effect::Restriction(Restriction::PlayersCannotBeTargeted {
                players: PlayerRef::You,
                duration: Duration::WhileSourceOnBattlefield,
            })]
        )
    {
        return Ok(ReminderSemantics::PlayerShroudProcedure);
    }

    if matches!(
        body_lower.trim(),
        "this object has hexproof as long as it's untapped."
            | "this creature has hexproof as long as it's untapped."
    ) && matches!(
        canonical.as_str(),
        "it can't be the target of spells or abilities your opponents control."
            | "this object can't be the target of spells or abilities your opponents control."
    ) && matches!(
        parsed.effects.as_slice(),
        [Effect::Restriction(Restriction::TargetingProtection {
            object: ObjectRef::Source,
            forbidden_controller: PlayerRef::Opponent,
            duration: Duration::WhileCondition(condition),
        })] if **condition == Condition::SourceState(ObjectState::Untapped)
    ) {
        return Ok(ReminderSemantics::HexproofExplanation);
    }

    if let Some(snow_reminder) =
        parse_exact_snow_resource_reminder(address, body, &canonical, parsed)?
    {
        return Ok(snow_reminder);
    }
    if let Some(mana_notation) =
        parse_exact_mana_notation_reminder(address, body, &canonical, parsed)?
    {
        return Ok(mana_notation);
    }

    if canonical == "exile cards with total mana value 6 or greater from your graveyard."
        && body_lower.contains("you may collect evidence 6")
        && matches!(
            parsed.costs.as_slice(),
            [Cost::Optional(cost)] if matches!(
                cost.as_ref(),
                Cost::ExileSelectionWithTotalManaValue {
                    minimum: Amount::Constant(6),
                    ..
                }
            )
        )
    {
        return Ok(ReminderSemantics::CollectEvidenceProcedure {
            minimum: Amount::Constant(6),
        });
    }
    if canonical == "you may put a -1/-1 counter on a creature you control."
        && body_lower.contains("you may blight 1")
        && matches!(
            parsed.costs.as_slice(),
            [Cost::Optional(cost)] if matches!(
                cost.as_ref(),
                Cost::PutCounterSelection {
                    counter: CounterKind::MinusOneMinusOne,
                    amount: Amount::Constant(1),
                    ..
                }
            )
        )
    {
        return Ok(ReminderSemantics::BlightProcedure {
            amount: Amount::Constant(1),
        });
    }
    if canonical == "you may choose a dragon you control or reveal a dragon card from your hand."
        && body_lower.contains("you may behold a dragon")
        && matches!(
            parsed.costs.as_slice(),
            [Cost::Optional(cost)] if matches!(cost.as_ref(), Cost::BeholdSelection { .. })
        )
    {
        return Ok(ReminderSemantics::BeholdProcedure {
            subtype: "Dragon".to_owned(),
        });
    }
    if canonical
        == "while paying a waterbend cost, you can tap your artifacts and creatures to help. each one pays for {1}."
        && body_lower.contains("waterbend {x}")
        && matches!(
            parsed.costs.as_slice(),
            [Cost::Waterbend {
                amount: Amount::X,
                ..
            }]
        )
    {
        return Ok(ReminderSemantics::WaterbendProcedure { amount: Amount::X });
    }

    let kicker = match canonical.as_str() {
        "you may sacrifice an artifact or creature in addition to any other costs as you cast this object spell." =>
        {
            let mut filter = ObjectFilter::in_zone(Zone::Battlefield);
            filter.controller = Some(PlayerRef::You);
            filter.card_types = vec![CardType::Artifact, CardType::Creature];
            filter.card_type_match_any = true;
            Some((filter, 1))
        }
        "you may sacrifice a land in addition to any other costs as you cast this object spell." => {
            let mut filter = ObjectFilter::with_type(CardType::Land);
            filter.zones = vec![Zone::Battlefield];
            filter.controller = Some(PlayerRef::You);
            Some((filter, 1))
        }
        "you may sacrifice a creature in addition to any other costs as you cast this object spell." =>
        {
            let mut filter = ObjectFilter::with_type(CardType::Creature);
            filter.zones = vec![Zone::Battlefield];
            filter.controller = Some(PlayerRef::You);
            Some((filter, 1))
        }
        "you may sacrifice two lands in addition to any other costs as you cast this object spell." =>
        {
            let mut filter = ObjectFilter::with_type(CardType::Land);
            filter.zones = vec![Zone::Battlefield];
            filter.controller = Some(PlayerRef::You);
            Some((filter, 2))
        }
        _ => None,
    };
    if let Some((filter, amount)) = kicker
        && body_lower
            .trim_start()
            .starts_with("kicker\u{2014}sacrifice ")
        && matches!(
            parsed.costs.as_slice(),
            [Cost::Optional(cost)]
                if matches!(cost.as_ref(), Cost::Sacrifice { amount: Amount::Constant(parsed_amount), filter: parsed_filter } if *parsed_amount == amount && parsed_filter == &filter)
        )
    {
        return Ok(ReminderSemantics::KickerSacrificeProcedure {
            amount: Amount::Constant(amount),
            filter: Box::new(filter),
        });
    }

    if body_lower.trim().trim_end_matches('.') == "increment"
        && matches!(
            canonical.as_str(),
            "whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature."
                | "whenever you cast a spell, if the amount of mana you spent is greater than this object's power or toughness, put a +1/+1 counter on this object."
        )
        && parsed
            .conditions
            .contains(&Condition::ManaSpentGreaterThanSourcePowerOrToughness)
    {
        return Ok(ReminderSemantics::IncrementProcedure);
    }

    if canonical == "it can't be blocked except by artifact creatures and/or black creatures."
        && parsed.effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::Restriction(Restriction::BlockerMustMatch { .. })
            )
        })
    {
        return Ok(ReminderSemantics::FearProcedure);
    }

    if body_lower.trim().trim_end_matches('.') == "flash"
        && canonical == "you may cast this object spell any time you could cast an instant."
    {
        return Ok(ReminderSemantics::FlashProcedure);
    }
    if body_lower.starts_with("you may cast this object spell as though it had flash if you pay ")
        && canonical == "you may cast it any time you could cast an instant."
    {
        return Ok(ReminderSemantics::FlashProcedure);
    }
    if let Some(amount_text) = body_lower
        .trim()
        .trim_end_matches('.')
        .strip_prefix("afterlife ")
        && let Some(amount) = parse_english_amount(amount_text)
    {
        let reminder_matches = match amount {
            Amount::Constant(1) => matches!(
                canonical.as_str(),
                "when this object dies, create a 1/1 white and black spirit creature token with flying."
                    | "when this object is put into a graveyard from the battlefield, create a 1/1 white and black spirit creature token with flying."
            ),
            Amount::Constant(2) => {
                canonical
                    == "when this object dies, create two 1/1 white and black spirit creature tokens with flying."
            }
            Amount::Constant(3) => {
                canonical
                    == "when this object dies, create three 1/1 white and black spirit creature tokens with flying."
            }
            _ => false,
        };
        let parsed_amount_matches = matches!(
            parsed.effects.as_slice(),
            [Effect::CreateToken(TokenCreation {
                amount: parsed_amount,
                specification: TokenSpecification::Defined(definition),
                ..
            })] if parsed_amount == &amount
                && definition.colors == [Color::White, Color::Black]
                && definition.subtypes == ["Spirit".to_owned()]
                && definition.keywords == [Keyword::Flying]
        );
        if reminder_matches && parsed_amount_matches {
            return Ok(ReminderSemantics::AfterlifeProcedure { amount });
        }
    }
    if let Some(keywords) = parse_exact_keyword_explanations(address, &canonical)?
        && parsed_has_keyword_carrier(parsed, &keywords)
    {
        return Ok(if keywords.len() == 1 {
            ReminderSemantics::KeywordExplanation(
                keywords
                    .into_iter()
                    .next()
                    .expect("the exact keyword grammar is nonempty"),
            )
        } else {
            ReminderSemantics::KeywordExplanations(keywords)
        });
    }
    if let Some(keyword) = explained_keyword(body)
        && keyword_reminder_matches(&keyword, &canonical)
        && parsed_has_keyword_carrier(parsed, std::slice::from_ref(&keyword))
    {
        return Ok(ReminderSemantics::KeywordExplanation(keyword));
    }

    if body_lower.trim().trim_end_matches('.') == "undaunted"
        && canonical == "this object spell costs {1} less to cast for each opponent."
        && matches!(
            parsed.effects.as_slice(),
            [Effect::ReduceSpellCost {
                object: ObjectRef::Source,
                mana: ManaCost(mana),
                per: CountExpression::OpponentCount {
                    player: PlayerRef::You,
                },
                maximum_reduction: None,
            }] if mana == "{1}"
        )
    {
        return Ok(ReminderSemantics::UndauntedProcedure);
    }

    if matches!(
        body_lower.trim().trim_end_matches('.'),
        "this object spell costs {1} less to cast for each creature in your party"
            | "this object costs {1} less to cast for each creature in your party"
    ) && canonical
        == "your party consists of up to one each of cleric, rogue, warrior, and wizard."
        && matches!(
            parsed.effects.as_slice(),
            [Effect::ReduceSpellCost {
                object: ObjectRef::Source,
                mana: ManaCost(mana),
                per: CountExpression::PartySize {
                    player: PlayerRef::You,
                },
                maximum_reduction: None,
            }] if mana == "{1}"
        )
    {
        return Ok(ReminderSemantics::PartyComposition);
    }

    if let Some(minimum_text) = body_lower
        .trim()
        .trim_end_matches('.')
        .strip_prefix("teamwork ")
        && let Some(minimum_power) = parse_english_amount(minimum_text)
        && canonical
            == format!(
                "as an additional cost to cast this object spell, you may tap any number of creatures you control with total power {minimum_text} or more."
            )
        && matches!(
            parsed.costs.as_slice(),
            [Cost::Optional(cost)] if matches!(
                &**cost,
                Cost::TapCreatureSelectionWithTotalPower {
                    minimum,
                    ..
                } if minimum == &minimum_power
            )
        )
    {
        return Ok(ReminderSemantics::TeamworkProcedure { minimum_power });
    }

    if body_lower.starts_with("dash ")
        && lower.contains("you may cast this object spell for its dash cost")
        && lower.contains("haste")
        && lower.contains("battlefield")
        && lower.contains("owner's hand")
        && lower.contains("beginning of the next end step")
    {
        let cost_text = body
            .trim()
            .strip_prefix("Dash ")
            .or_else(|| body.trim().strip_prefix("dash "))
            .ok_or_else(|| CompileError::UnsupportedReminder {
                address,
                reminder: reminder.to_owned(),
            })?;
        return Ok(ReminderSemantics::DashProcedure {
            cost: parse_mana_cost(address, cost_text)?,
        });
    }
    if let Some(body_cost) = body_lower.trim().strip_prefix("outlast ")
        && let Some(reminder_cost) = lower
            .strip_suffix(", {t}: put a +1/+1 counter on this object. outlast only as a sorcery.")
    {
        let body_cost = parse_mana_cost(address, body_cost)?;
        let reminder_cost = parse_mana_cost(address, reminder_cost)?;
        if body_cost == reminder_cost {
            return Ok(ReminderSemantics::OutlastProcedure { cost: body_cost });
        }
    }
    if body_lower.starts_with("gift a tapped fish")
        && lower.contains("they create a tapped 1/1 blue fish creature token")
    {
        let token = TokenDefinition {
            name: Some("Fish".to_owned()),
            power: Some(Amount::Constant(1)),
            toughness: Some(Amount::Constant(1)),
            colors: vec![Color::Blue],
            card_types: vec![CardType::Creature],
            subtypes: vec!["Fish".to_owned()],
            keywords: Vec::new(),
            abilities: Vec::new(),
        };
        return Ok(ReminderSemantics::GiftProcedure {
            token: Box::new(token),
            tapped: true,
        });
    }
    if body_lower == "gift a card"
        && lower
            == "you may promise an opponent a gift as you cast this object spell. if you do, they draw a card before its other effects."
    {
        return Ok(ReminderSemantics::GiftCardProcedure);
    }
    if (body_lower.ends_with(", it connives.") || body_lower == "it connives.")
        && lower
            == "draw a card, then discard a card. if you discarded a nonland card, put a +1/+1 counter on this object."
    {
        return Ok(ReminderSemantics::ConniveProcedure);
    }
    if let Some(amount_text) = body_lower.strip_prefix("mobilize ")
        && let Some(amount) = parse_english_amount(amount_text)
        && mobilize_reminder_matches(&amount, &lower)
    {
        let token = TokenDefinition {
            name: Some("Warrior".to_owned()),
            power: Some(Amount::Constant(1)),
            toughness: Some(Amount::Constant(1)),
            colors: vec![Color::Red],
            card_types: vec![CardType::Creature],
            subtypes: vec!["Warrior".to_owned()],
            keywords: Vec::new(),
            abilities: Vec::new(),
        };
        return Ok(ReminderSemantics::MobilizeProcedure {
            amount,
            token: Box::new(token),
        });
    }
    if (lower.contains("you may cast that card from your graveyard for its flashback cost")
        || lower.contains("you may cast this object from your graveyard for its flashback cost"))
        && lower.contains("then exile it")
    {
        return Ok(ReminderSemantics::FlashbackProcedure);
    }
    for (color, symbol, color_name) in [
        (Color::White, "w", "white"),
        (Color::Blue, "u", "blue"),
        (Color::Black, "b", "black"),
        (Color::Red, "r", "red"),
        (Color::Green, "g", "green"),
    ] {
        let expected = format!(
            "each {{{symbol}}} in the mana costs of permanents you control counts toward your devotion to {color_name}."
        );
        if lower == expected && parsed_has_devotion_amount(parsed, color) {
            return Ok(ReminderSemantics::DevotionProcedure { color });
        }
    }
    if lower.contains("you may cast cards from your graveyard for their escape cost") {
        return Ok(ReminderSemantics::EscapeProcedure);
    }

    if let Some(cost) = parse_ward_cost(address, body) {
        let cost = cost?;
        let expected = match &cost {
            WardCost::Mana(cost) => format!(
                "whenever this object becomes the target of a spell or ability an opponent controls, counter it unless that player pays {}.",
                cost.0.to_ascii_lowercase()
            ),
            WardCost::PayLife(amount) => format!(
                "whenever this object becomes the target of a spell or ability an opponent controls, counter it unless that player pays {} life.",
                amount_text(amount).ok_or_else(|| CompileError::UnsupportedReminder {
                    address,
                    reminder: reminder.to_string(),
                })?
            ),
        };
        if lower == expected {
            return Ok(ReminderSemantics::KeywordExplanation(Keyword::Ward(
                Box::new(cost),
            )));
        }
    }
    if let Some(specification) = parse_cycling_specification(address, body) {
        let specification = specification?;
        let expected = match &specification.kind {
            CyclingKind::Draw => format!(
                "{}, discard this object: draw a card.",
                specification.cost.0.to_ascii_lowercase()
            ),
            CyclingKind::Type { type_name, .. } => format!(
                "{}, discard this object: search your library for {} {} card, reveal it, put it into your hand, then shuffle.",
                specification.cost.0.to_ascii_lowercase(),
                indefinite_article(type_name),
                type_name.to_ascii_lowercase()
            ),
        };
        if lower == expected {
            return Ok(match specification.kind {
                CyclingKind::Draw => ReminderSemantics::CyclingProcedure {
                    cost: specification.cost,
                },
                CyclingKind::Type { type_name, filter } => {
                    ReminderSemantics::TypecyclingProcedure {
                        type_name,
                        cost: specification.cost,
                        filter: Box::new(filter),
                    }
                }
            });
        }
    }
    if body_lower.trim_end_matches('.') == "prowess"
        && lower
            == "whenever you cast a noncreature spell, this object gets +1/+1 until end of turn."
    {
        return Ok(ReminderSemantics::ProwessProcedure);
    }
    if canonical
        == "choose any number of permanents and/or players, then give each another counter of each kind already there."
        && parsed
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::Proliferate { .. }))
    {
        return Ok(ReminderSemantics::ProliferateProcedure);
    }
    if matches!(
        canonical.as_str(),
        "then exile this object. you may cast the creature later from exile."
            | "then exile this object. you may cast the artifact later from exile."
    ) {
        return Ok(ReminderSemantics::AdventureProcedure);
    }
    if canonical
        .strip_prefix("to investigate, ")
        .unwrap_or(&canonical)
        .strip_prefix("create a clue token. ")
        .and_then(parse_predefined_token_reminder)
        == Some((
            PredefinedArtifactTokenKind::Clue,
            TokenGrammaticalNumber::Singular,
        ))
        && parsed_predefined_token_number(parsed, PredefinedArtifactTokenKind::Clue)
            == Some(TokenGrammaticalNumber::Singular)
    {
        return Ok(ReminderSemantics::ClueDefinition(Box::new(
            clue_definition(),
        )));
    }
    if let Some((kind, number)) = parse_predefined_token_reminder(&canonical)
        && parsed_predefined_token_number(parsed, kind) == Some(number)
    {
        return Ok(match kind {
            PredefinedArtifactTokenKind::Treasure => {
                ReminderSemantics::TreasureDefinition(Box::new(treasure_definition()))
            }
            PredefinedArtifactTokenKind::Food => {
                ReminderSemantics::FoodDefinition(Box::new(food_definition()))
            }
            PredefinedArtifactTokenKind::Clue => {
                ReminderSemantics::ClueDefinition(Box::new(clue_definition()))
            }
            PredefinedArtifactTokenKind::Blood => {
                ReminderSemantics::BloodDefinition(Box::new(blood_definition()))
            }
            PredefinedArtifactTokenKind::Gold => {
                ReminderSemantics::GoldDefinition(Box::new(gold_definition()))
            }
            PredefinedArtifactTokenKind::Lander => {
                ReminderSemantics::LanderDefinition(Box::new(lander_definition()))
            }
        });
    }
    if let Some(amount) =
        parsed_library_action_amount(&parsed.effects, LibraryReminderAction::Surveil, &canonical)
    {
        return Ok(ReminderSemantics::SurveilProcedure { amount });
    }
    if let Some(amount) =
        parsed_library_action_amount(&parsed.effects, LibraryReminderAction::Scry, &canonical)
    {
        return Ok(ReminderSemantics::ScryProcedure { amount });
    }
    if let Some((player, amount, optional)) =
        parsed_mill_reminder_semantics(&parsed.effects, &canonical)
    {
        return Ok(ReminderSemantics::MillProcedure {
            player,
            amount,
            optional,
        });
    }
    if lower
        == "that player puts the top card of their library onto the battlefield face down as a 2/2 creature. if it's a creature card, it can be turned face up any time for its mana cost."
    {
        return Ok(ReminderSemantics::ManifestProcedure);
    }
    if body_lower.starts_with("crew ") {
        let amount_text = body_lower.strip_prefix("crew ").unwrap_or_default().trim();
        let amount =
            parse_english_amount(amount_text).ok_or_else(|| CompileError::UnsupportedReminder {
                address,
                reminder: reminder.to_string(),
            })?;
        let expected = format!(
            "tap any number of creatures you control with total power {amount_text} or more: this object becomes an artifact creature until end of turn."
        );
        if lower == expected {
            return Ok(ReminderSemantics::CrewProcedure {
                required_power: amount,
            });
        }
    }
    if body_lower.trim() == "station"
        && matches!(
            parsed.costs.as_slice(),
            [Cost::TapSelection(ObjectSelection {
                id: 0,
                amount: TargetAmount::Exactly(1),
                ..
            })]
        )
        && matches!(
            parsed.effects.as_slice(),
            [Effect::PutCounter {
                object: ObjectRef::Source,
                counter: CounterKind::Named(counter),
                amount: Amount::Count(count),
            }] if counter == "charge"
                && matches!(
                    count.as_ref(),
                    CountExpression::SelectedObjectsTotalPower { selection_id: 0 }
                )
        )
        && parsed.activation_restriction == Some(ActivationRestriction::SorceryTiming)
    {
        let spacecraft_prefix = "tap another creature you control: put charge counters equal to its power on this spacecraft. station only as a sorcery.";
        let planet_prefix = "tap another creature you control: put charge counters equal to its power on this planet. station only as a sorcery.";
        let remainder = canonical
            .strip_prefix(spacecraft_prefix)
            .or_else(|| canonical.strip_prefix(planet_prefix));
        if let Some(remainder) = remainder {
            let remainder = remainder.trim();
            let creature_threshold = if remainder.is_empty() {
                None
            } else {
                let threshold = remainder
                    .strip_prefix("it's an artifact creature at ")
                    .and_then(|text| text.strip_suffix("+."))
                    .and_then(|text| text.parse::<u32>().ok());
                let Some(threshold) = threshold.filter(|threshold| *threshold > 0) else {
                    return Err(CompileError::UnsupportedReminder {
                        address,
                        reminder: reminder.to_owned(),
                    });
                };
                Some(threshold)
            };
            return Ok(ReminderSemantics::StationProcedure { creature_threshold });
        }
    }
    if body_lower.starts_with("evoke ")
        && lower
            == "you may cast this object spell for its evoke cost. if you do, it's sacrificed when it enters."
    {
        let cost = parse_mana_cost(
            address,
            body.split_once(' ')
                .map(|(_, value)| value.trim())
                .unwrap_or_default(),
        )?;
        return Ok(ReminderSemantics::EvokeProcedure { cost });
    }
    if body_lower == "split second"
        && lower
            == "as long as this object spell is on the stack, players can't cast spells or activate abilities that aren't mana abilities."
    {
        return Ok(ReminderSemantics::SplitSecondProcedure);
    }
    if (body_lower == "partner" && lower == "you can have two commanders if both have partner.")
        || (body_lower == "friends forever"
            && lower == "you can have two commanders if both have friends forever.")
        || (body_lower == "ready to run"
            && lower == "you can have two commanders if both have ready to run.")
        || (body_lower.starts_with("partner\u{2014}")
            && !body_lower
                .trim_start_matches("partner\u{2014}")
                .trim()
                .is_empty()
            && lower == "you can have two commanders if both have this ability.")
    {
        return Ok(ReminderSemantics::PartnerProcedure);
    }
    if body_lower == "spell commander"
        && lower
            == "this object can be your commander. in limited, it can partner like other monocolored legends."
    {
        return Ok(ReminderSemantics::SpellCommanderProcedure);
    }
    if body_lower == "this object enters prepared."
        && lower == "while it's prepared, you may cast a copy of its spell. doing so unprepares it."
    {
        return Ok(ReminderSemantics::PreparedProcedure);
    }
    if body_lower == "paradigm"
        && lower
            == "then exile this object spell. after you first resolve a spell with this name, you may cast a copy of it from exile without paying its mana cost at the beginning of each of your first main phases."
    {
        return Ok(ReminderSemantics::ParadigmProcedure);
    }
    if lower == "{q} is the untap symbol." {
        return Ok(ReminderSemantics::UntapSymbolProcedure);
    }
    if matches!(
        canonical.as_str(),
        "if a permanent with a stun counter would become untapped, remove one from it instead."
            | "if it would become untapped, remove a stun counter from it instead."
    ) && parsed.effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::PutCounter {
                counter: CounterKind::Named(name),
                ..
            } if name.eq_ignore_ascii_case("stun")
        )
    }) {
        return Ok(ReminderSemantics::StunCounterProcedure);
    }
    if lower.starts_with("transforms from ") && lower.ends_with('.') {
        return Ok(ReminderSemantics::TransformOrigin {
            front_face_name: reminder["Transforms from ".len()..reminder.len() - 1].to_string(),
        });
    }
    if lower.starts_with("it loses all other ")
        && (lower.ends_with("card types and creature types.")
            || lower.ends_with("colors, card types, creature types, and names."))
    {
        return Ok(ReminderSemantics::CharacteristicLossExplanation);
    }

    Err(CompileError::UnsupportedReminder {
        address,
        reminder: reminder.to_string(),
    })
}

fn parsed_has_historic_filter(parsed: &ParsedClause) -> bool {
    timing_has_historic_filter(&parsed.timing)
        || parsed.conditions.iter().any(condition_has_historic_filter)
        || parsed.costs.iter().any(cost_has_historic_filter)
        || parsed
            .targets
            .iter()
            .any(|target| target_filter_has_historic_filter(&target.filter))
        || parsed.effects.iter().any(effect_has_historic_filter)
}

fn parsed_has_devotion_amount(parsed: &ParsedClause, color: Color) -> bool {
    parsed
        .effects
        .iter()
        .any(|effect| effect_has_devotion_amount(effect, color))
}

fn parsed_has_unbound_selected_toughness(parsed: &ParsedClause) -> bool {
    parsed
        .effects
        .iter()
        .any(|effect| effect_has_unbound_selected_toughness(effect, &parsed.costs))
}

fn costs_bind_selected_toughness(costs: &[Cost], selection_id: u8) -> bool {
    costs.iter().any(
        |cost| matches!(cost, Cost::SacrificeSelection(selection) if selection.id == selection_id),
    )
}

fn amount_has_unbound_selected_toughness(amount: &Amount, costs: &[Cost]) -> bool {
    match amount {
        Amount::Count(count) => match count.as_ref() {
            CountExpression::SelectedObjectsTotalToughness { selection_id } => {
                !costs_bind_selected_toughness(costs, *selection_id)
            }
            CountExpression::LifeLostThisWay { amount_each, .. } => {
                amount_has_unbound_selected_toughness(amount_each, costs)
            }
            _ => false,
        },
        Amount::Twice(amount) | Amount::Product { value: amount, .. } | Amount::UpTo(amount) => {
            amount_has_unbound_selected_toughness(amount, costs)
        }
        _ => false,
    }
}

fn effect_has_unbound_selected_toughness(effect: &Effect, costs: &[Cost]) -> bool {
    match effect {
        Effect::Optional(effects) => effects
            .iter()
            .any(|effect| effect_has_unbound_selected_toughness(effect, costs)),
        Effect::Draw { amount, .. }
        | Effect::GainLife { amount, .. }
        | Effect::LoseLife { amount, .. }
        | Effect::PayLife { amount, .. }
        | Effect::Damage { amount, .. }
        | Effect::Scry { amount, .. }
        | Effect::Surveil { amount, .. }
        | Effect::Mill { amount, .. }
        | Effect::PutPlayerCounter { amount, .. }
        | Effect::PutCounter { amount, .. }
        | Effect::RemoveCounter { amount, .. } => {
            amount_has_unbound_selected_toughness(amount, costs)
        }
        Effect::ModifyPowerToughness(change) => {
            amount_has_unbound_selected_toughness(&change.power, costs)
                || amount_has_unbound_selected_toughness(&change.toughness, costs)
        }
        Effect::SetCharacteristics(characteristics) => {
            characteristics
                .base_power
                .as_ref()
                .is_some_and(|amount| amount_has_unbound_selected_toughness(amount, costs))
                || characteristics
                    .base_toughness
                    .as_ref()
                    .is_some_and(|amount| amount_has_unbound_selected_toughness(amount, costs))
        }
        Effect::CreateToken(creation)
        | Effect::CreateTokenAttached { creation, .. }
        | Effect::CreateTokenAndAttachSource { creation, .. }
        | Effect::CreateTokenWithDelayedMove { creation, .. } => {
            amount_has_unbound_selected_toughness(&creation.amount, costs)
        }
        Effect::Conditional {
            if_true, if_false, ..
        } => {
            if_true
                .iter()
                .any(|effect| effect_has_unbound_selected_toughness(effect, costs))
                || if_false
                    .iter()
                    .any(|effect| effect_has_unbound_selected_toughness(effect, costs))
        }
        Effect::GrantAbility { ability, .. } => ability
            .effects
            .iter()
            .any(|effect| effect_has_unbound_selected_toughness(effect, &ability.costs)),
        _ => false,
    }
}

fn amount_has_devotion(amount: &Amount, color: Color) -> bool {
    match amount {
        Amount::Count(count) => match count.as_ref() {
            CountExpression::Devotion {
                player: PlayerRef::You,
                color: actual,
            } => *actual == color,
            CountExpression::LifeLostThisWay { amount_each, .. } => {
                amount_has_devotion(amount_each, color)
            }
            _ => false,
        },
        Amount::Twice(amount) | Amount::Product { value: amount, .. } | Amount::UpTo(amount) => {
            amount_has_devotion(amount, color)
        }
        _ => false,
    }
}

fn effect_has_devotion_amount(effect: &Effect, color: Color) -> bool {
    match effect {
        Effect::Optional(effects) => effects
            .iter()
            .any(|effect| effect_has_devotion_amount(effect, color)),
        Effect::Draw { amount, .. }
        | Effect::GainLife { amount, .. }
        | Effect::LoseLife { amount, .. }
        | Effect::PayLife { amount, .. }
        | Effect::Damage { amount, .. }
        | Effect::Scry { amount, .. }
        | Effect::Surveil { amount, .. }
        | Effect::Mill { amount, .. }
        | Effect::PutPlayerCounter { amount, .. }
        | Effect::PutCounter { amount, .. }
        | Effect::RemoveCounter { amount, .. } => amount_has_devotion(amount, color),
        Effect::ModifyPowerToughness(change) => {
            amount_has_devotion(&change.power, color)
                || amount_has_devotion(&change.toughness, color)
        }
        Effect::SetCharacteristics(characteristics) => {
            characteristics
                .base_power
                .as_ref()
                .is_some_and(|amount| amount_has_devotion(amount, color))
                || characteristics
                    .base_toughness
                    .as_ref()
                    .is_some_and(|amount| amount_has_devotion(amount, color))
        }
        Effect::CreateToken(creation)
        | Effect::CreateTokenAttached { creation, .. }
        | Effect::CreateTokenAndAttachSource { creation, .. }
        | Effect::CreateTokenWithDelayedMove { creation, .. } => {
            amount_has_devotion(&creation.amount, color)
        }
        Effect::Conditional {
            if_true, if_false, ..
        } => {
            if_true
                .iter()
                .any(|effect| effect_has_devotion_amount(effect, color))
                || if_false
                    .iter()
                    .any(|effect| effect_has_devotion_amount(effect, color))
        }
        Effect::GrantAbility { ability, .. } => ability
            .effects
            .iter()
            .any(|effect| effect_has_devotion_amount(effect, color)),
        Effect::LibraryProcedure(LibraryProcedure::DevotionLookAndWin {
            color: procedure_color,
            ..
        }) => *procedure_color == color,
        _ => false,
    }
}

fn timing_has_historic_filter(timing: &Timing) -> bool {
    match timing {
        Timing::Triggered(trigger) | Timing::TriggeredModalHeader { trigger, .. } => {
            trigger_has_historic_filter(trigger)
        }
        _ => false,
    }
}

fn trigger_has_historic_filter(trigger: &Trigger) -> bool {
    match trigger {
        Trigger::AnyOf(triggers) => triggers.iter().any(trigger_has_historic_filter),
        Trigger::OncePerTurn(trigger) => trigger_has_historic_filter(trigger),
        Trigger::ObjectEnters(filter)
        | Trigger::ObjectAttacks(filter)
        | Trigger::SourceBlocks { object: filter }
        | Trigger::CombatDamageToPlayer { source: filter }
        | Trigger::DamageToPlayer { source: filter, .. }
        | Trigger::SourceCombatDamageToObject { object: filter }
        | Trigger::Cast { spell: filter, .. } => filter.historic,
        Trigger::ObjectEvent { subject, .. }
        | Trigger::PlayerAction {
            subject: Some(subject),
            ..
        } => trigger_subject_has_historic_filter(subject),
        _ => false,
    }
}

fn trigger_subject_has_historic_filter(subject: &TriggerSubject) -> bool {
    matches!(subject, TriggerSubject::Matching(filter) if filter.historic)
}

fn target_filter_has_historic_filter(filter: &TargetFilter) -> bool {
    match filter {
        TargetFilter::Object(filter) | TargetFilter::Spell(filter) => filter.historic,
        TargetFilter::Any(filters) => filters.iter().any(target_filter_has_historic_filter),
        TargetFilter::Conditional {
            condition,
            if_true,
            if_false,
        } => {
            condition_has_historic_filter(condition)
                || target_filter_has_historic_filter(if_true)
                || target_filter_has_historic_filter(if_false)
        }
        TargetFilter::Player | TargetFilter::Opponent => false,
    }
}

fn condition_has_historic_filter(condition: &Condition) -> bool {
    match condition {
        Condition::ControlCount { filter, .. }
        | Condition::PowerComparison { object: filter, .. } => filter.historic,
        Condition::ControlAny { filters, .. } => filters.iter().any(|filter| filter.historic),
        Condition::PaymentDeclined(cost)
        | Condition::PaymentAccepted(cost)
        | Condition::UnlessPaid { cost, .. } => cost_has_historic_filter(cost),
        Condition::EventWouldOccur(ReplacementEvent::PutCounters { object, .. }) => object.historic,
        _ => false,
    }
}

fn selection_has_historic_filter(selection: &ObjectSelection) -> bool {
    selection.filter.historic
}

fn cost_has_historic_filter(cost: &Cost) -> bool {
    match cost {
        Cost::Optional(cost) => cost_has_historic_filter(cost),
        Cost::TapSelection(selection)
        | Cost::SacrificeSelection(selection)
        | Cost::DiscardSelection(selection)
        | Cost::ReturnSelectionToHand(selection)
        | Cost::ExileSelection(selection) => selection_has_historic_filter(selection),
        Cost::TapCreatureSelectionWithTotalPower { selection, .. }
        | Cost::RevealSelection { selection, .. }
        | Cost::Waterbend { selection, .. }
        | Cost::PutCounterSelection { selection, .. }
        | Cost::ExileSelectionWithTotalManaValue { selection, .. } => {
            selection_has_historic_filter(selection)
        }
        Cost::Sacrifice { filter, .. } => filter.historic,
        Cost::BeholdSelection {
            battlefield, hand, ..
        } => battlefield.historic || hand.historic,
        _ => false,
    }
}

fn object_ref_has_historic_filter(object: &ObjectRef) -> bool {
    matches!(object, ObjectRef::EachMatching(filter) if filter.historic)
}

fn effect_has_historic_filter(effect: &Effect) -> bool {
    match effect {
        Effect::Optional(effects) => effects.iter().any(effect_has_historic_filter),
        Effect::PayCost(cost) => cost_has_historic_filter(cost),
        Effect::MoveZone(move_effect) => object_ref_has_historic_filter(&move_effect.object),
        Effect::MoveZoneUnderControl { object, .. }
        | Effect::MoveToLibraryBottom { object }
        | Effect::PutOnLibraryTopInOrder { objects: object }
        | Effect::Destroy { object }
        | Effect::DestroyWithoutRegeneration { object }
        | Effect::ExileSpellAfterResolution { object }
        | Effect::Tap { object }
        | Effect::Untap { object } => object_ref_has_historic_filter(object),
        Effect::MoveSelected(move_effect) => selection_has_historic_filter(&move_effect.selection),
        Effect::SetSelectedTapped { selection, .. } | Effect::Discard(selection) => {
            selection_has_historic_filter(selection)
        }
        Effect::SearchLibrary(search) => search.predicate.historic,
        Effect::GrantCastPermission(permission) => permission.filter.historic,
        Effect::DrawThenDiscardUnless { alternative, .. }
        | Effect::SelectFromLookedAt {
            predicate: alternative,
            ..
        }
        | Effect::EachPlayerSacrifices {
            filter: alternative,
            ..
        }
        | Effect::PlayersSacrifice {
            filter: alternative,
            ..
        } => alternative.historic,
        Effect::Conditional {
            condition,
            if_true,
            if_false,
        } => {
            condition_has_historic_filter(condition)
                || if_true.iter().any(effect_has_historic_filter)
                || if_false.iter().any(effect_has_historic_filter)
        }
        Effect::GrantAbility { ability, .. } => {
            ability.costs.iter().any(cost_has_historic_filter)
                || ability.effects.iter().any(effect_has_historic_filter)
        }
        _ => false,
    }
}

fn mobilize_reminder_matches(amount: &Amount, reminder: &str) -> bool {
    match amount {
        Amount::Constant(1) => {
            reminder
                == "whenever this object attacks, create a tapped and attacking 1/1 red warrior creature token. sacrifice it at the beginning of the next end step."
        }
        Amount::Constant(value) => {
            let Some(amount_word) = english_constant_text(*value) else {
                return false;
            };
            reminder
                == format!(
                    "whenever this object attacks, create {amount_word} tapped and attacking 1/1 red warrior creature tokens. sacrifice them at the beginning of the next end step."
                )
        }
        _ => false,
    }
}

fn indefinite_article(noun: &str) -> &'static str {
    if noun.trim_start().chars().next().is_some_and(|character| {
        matches!(character.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u')
    }) {
        "an"
    } else {
        "a"
    }
}

fn parse_exact_snow_resource_reminder(
    address: ClauseAddress,
    body: &str,
    reminder: &str,
    parsed: &ParsedClause,
) -> Result<Option<ReminderSemantics>, CompileError> {
    const SNOW_SENTENCE: &str = "{s} can be paid with one mana from a snow source.";
    let (other, snow_first) = if reminder == SNOW_SENTENCE {
        (None, true)
    } else if let Some(other) = reminder.strip_prefix(&format!("{SNOW_SENTENCE} ")) {
        (Some(other.trim()), true)
    } else if let Some(other) = reminder.strip_suffix(&format!(" {SNOW_SENTENCE}")) {
        (Some(other.trim()), false)
    } else {
        return Ok(None);
    };
    if !parsed_contains_special_resource_symbol(parsed, SpecialCostSymbol::Snow) {
        return Err(CompileError::UnsupportedReminder {
            address,
            reminder: reminder.to_owned(),
        });
    }
    let snow_cost =
        SpecialResourceCost::from_compiled_symbols("{S}", vec![SpecialCostSymbol::Snow]).map_err(
            |_| CompileError::UnsupportedReminder {
                address,
                reminder: reminder.to_owned(),
            },
        )?;
    let snow = ReminderSemantics::SpecialResourceExplanation(snow_cost);
    let Some(other) = other.filter(|other| !other.is_empty()) else {
        return Ok(Some(snow));
    };
    let other = parse_reminder(address, body, other, parsed)?;
    let reminders = if snow_first {
        vec![snow, other]
    } else {
        vec![other, snow]
    };
    Ok(Some(ReminderSemantics::Composite(reminders)))
}

fn parse_exact_mana_notation_reminder(
    address: ClauseAddress,
    body: &str,
    reminder: &str,
    parsed: &ParsedClause,
) -> Result<Option<ReminderSemantics>, CompileError> {
    let sentences = split_top_level_sentences(reminder);
    let mut exact = sentences
        .iter()
        .enumerate()
        .filter_map(|(index, sentence)| {
            parse_exact_mana_notation_sentence(sentence).map(|explanation| (index, explanation))
        })
        .collect::<Vec<_>>();
    if exact.is_empty() {
        return if looks_like_mana_notation_reminder(reminder) {
            Err(CompileError::UnsupportedReminder {
                address,
                reminder: reminder.to_owned(),
            })
        } else {
            Ok(None)
        };
    }
    if exact.len() != 1 {
        return Err(CompileError::UnsupportedReminder {
            address,
            reminder: reminder.to_owned(),
        });
    }
    let (notation_index, explanation) = exact
        .pop()
        .expect("one exact mana notation sentence was established");
    if !parsed_contains_mana_notation_symbol(parsed, explanation.symbol()) {
        return Err(CompileError::UnsupportedReminder {
            address,
            reminder: reminder.to_owned(),
        });
    }

    let notation = ReminderSemantics::ManaNotationExplanation(explanation);
    if sentences.len() == 1 {
        return Ok(Some(notation));
    }
    let other_text = sentences
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != notation_index)
        .map(|(_, sentence)| *sentence)
        .collect::<Vec<_>>()
        .join(" ");
    if other_text.is_empty() {
        return Err(CompileError::UnsupportedReminder {
            address,
            reminder: reminder.to_owned(),
        });
    }
    let other = parse_reminder(address, body, &other_text, parsed)?;
    let reminders = if notation_index == 0 {
        vec![notation, other]
    } else if notation_index + 1 == sentences.len() {
        vec![other, notation]
    } else {
        return Err(CompileError::UnsupportedReminder {
            address,
            reminder: reminder.to_owned(),
        });
    };
    Ok(Some(ReminderSemantics::Composite(reminders)))
}

fn parse_exact_mana_notation_sentence(sentence: &str) -> Option<ManaNotationExplanation> {
    if sentence == "{c} represents colorless mana." {
        return Some(ManaNotationExplanation::RepresentsManaType {
            symbol: TypedManaSymbol::Color(TypedManaColor::Colorless),
            mana_type: TypedManaColor::Colorless,
        });
    }

    const COLORS: [(&str, TypedManaColor); 5] = [
        ("w", TypedManaColor::White),
        ("u", TypedManaColor::Blue),
        ("b", TypedManaColor::Black),
        ("r", TypedManaColor::Red),
        ("g", TypedManaColor::Green),
    ];
    for (code, color) in COLORS {
        if sentence == format!("{{{code}/p}} can be paid with either {{{code}}} or 2 life.") {
            return Some(ManaNotationExplanation::PaymentAlternatives {
                symbol: TypedManaSymbol::Phyrexian(color),
                alternatives: vec![
                    ManaNotationPaymentAlternative::Mana(TypedManaSymbol::Color(color)),
                    ManaNotationPaymentAlternative::Life(2),
                ],
            });
        }
    }
    for (first_code, first) in COLORS {
        for (second_code, second) in COLORS {
            if first == second {
                continue;
            }
            if sentence
                == format!(
                    "{{{first_code}/{second_code}}} can be paid with either {{{first_code}}} or {{{second_code}}}."
                )
            {
                return Some(ManaNotationExplanation::PaymentAlternatives {
                    symbol: TypedManaSymbol::Hybrid(first, second),
                    alternatives: vec![
                        ManaNotationPaymentAlternative::Mana(TypedManaSymbol::Color(first)),
                        ManaNotationPaymentAlternative::Mana(TypedManaSymbol::Color(second)),
                    ],
                });
            }
        }
    }
    None
}

fn looks_like_mana_notation_reminder(reminder: &str) -> bool {
    reminder.contains("represents colorless mana")
        || (reminder.contains("can be paid")
            && (reminder.contains("/p}")
                || [
                    "{w/u}", "{w/b}", "{w/r}", "{w/g}", "{u/w}", "{u/b}", "{u/r}", "{u/g}",
                    "{b/w}", "{b/u}", "{b/r}", "{b/g}", "{r/w}", "{r/u}", "{r/b}", "{r/g}",
                    "{g/w}", "{g/u}", "{g/b}", "{g/r}",
                ]
                .iter()
                .any(|symbol| reminder.contains(symbol))))
}

fn parsed_contains_mana_notation_symbol(parsed: &ParsedClause, symbol: &TypedManaSymbol) -> bool {
    parsed
        .costs
        .iter()
        .any(|cost| cost_contains_mana_notation_symbol(cost, symbol))
        || parsed
            .conditions
            .iter()
            .any(|condition| condition_contains_mana_notation_symbol(condition, symbol))
        || parsed
            .effects
            .iter()
            .any(|effect| effect_contains_mana_notation_symbol(effect, symbol))
}

fn cost_contains_mana_notation_symbol(cost: &Cost, symbol: &TypedManaSymbol) -> bool {
    match cost {
        Cost::Mana(cost) => mana_cost_contains_mana_notation_symbol(cost, symbol),
        Cost::AtomicResource(cost) => {
            resource_cost_contains_mana_notation_symbol(cost.expression(), symbol)
        }
        _ => false,
    }
}

fn mana_cost_contains_mana_notation_symbol(cost: &ManaCost, symbol: &TypedManaSymbol) -> bool {
    parse_resource_cost_expression(&cost.0)
        .ok()
        .is_some_and(|expression| resource_cost_contains_mana_notation_symbol(&expression, symbol))
}

fn resource_cost_contains_mana_notation_symbol(
    cost: &ResourceCostExpression,
    symbol: &TypedManaSymbol,
) -> bool {
    cost.components.iter().any(|component| {
        matches!(
            component,
            TypedResourceCostComponent::Mana(mana) if mana.symbols.contains(symbol)
        )
    })
}

fn condition_contains_mana_notation_symbol(
    condition: &Condition,
    symbol: &TypedManaSymbol,
) -> bool {
    match condition {
        Condition::PaymentDeclined(cost) | Condition::PaymentAccepted(cost) => {
            cost_contains_mana_notation_symbol(cost, symbol)
        }
        Condition::UnlessPaid { cost, .. } => cost_contains_mana_notation_symbol(cost, symbol),
        _ => false,
    }
}

fn effect_contains_mana_notation_symbol(effect: &Effect, symbol: &TypedManaSymbol) -> bool {
    match effect {
        Effect::PayCost(cost) => cost_contains_mana_notation_symbol(cost, symbol),
        Effect::AddMana(production) => {
            exact_mana_production_contains_notation_symbol(production, symbol)
        }
        Effect::Optional(effects) => effects
            .iter()
            .any(|effect| effect_contains_mana_notation_symbol(effect, symbol)),
        Effect::Conditional {
            condition,
            if_true,
            if_false,
        } => {
            condition_contains_mana_notation_symbol(condition, symbol)
                || if_true
                    .iter()
                    .any(|effect| effect_contains_mana_notation_symbol(effect, symbol))
                || if_false
                    .iter()
                    .any(|effect| effect_contains_mana_notation_symbol(effect, symbol))
        }
        Effect::GrantAbility { ability, .. } => {
            granted_ability_contains_mana_notation_symbol(ability, symbol)
        }
        Effect::CreateToken(creation)
        | Effect::CreateTokenAttached { creation, .. }
        | Effect::CreateTokenAndAttachSource { creation, .. }
        | Effect::CreateTokenWithDelayedMove { creation, .. } => {
            token_creation_contains_mana_notation_symbol(creation, symbol)
        }
        Effect::Replacement(replacement) => {
            replacement_contains_mana_notation_symbol(replacement, symbol)
        }
        Effect::Copy(copy) => copy.exceptions.iter().any(|exception| {
            matches!(
                exception,
                CopyException::AddGrantedAbility(ability)
                    if granted_ability_contains_mana_notation_symbol(ability, symbol)
            )
        }),
        Effect::ResolveWard { cost, .. } => ward_cost_contains_mana_notation_symbol(cost, symbol),
        Effect::SchedulePaymentOrLose(payment) => {
            cost_contains_mana_notation_symbol(&payment.cost, symbol)
        }
        _ => false,
    }
}

fn exact_mana_production_contains_notation_symbol(
    production: &ManaProduction,
    symbol: &TypedManaSymbol,
) -> bool {
    let TypedManaSymbol::Color(expected) = symbol else {
        return false;
    };
    matches!(
        production.typed.as_ref().map(|typed| &typed.composition),
        Some(TypedManaComposition::Exact(colors)) if colors.contains(expected)
    )
}

fn granted_ability_contains_mana_notation_symbol(
    ability: &GrantedAbility,
    symbol: &TypedManaSymbol,
) -> bool {
    ability
        .costs
        .iter()
        .any(|cost| cost_contains_mana_notation_symbol(cost, symbol))
        || ability
            .effects
            .iter()
            .any(|effect| effect_contains_mana_notation_symbol(effect, symbol))
}

fn token_creation_contains_mana_notation_symbol(
    creation: &TokenCreation,
    symbol: &TypedManaSymbol,
) -> bool {
    matches!(
        &creation.specification,
        TokenSpecification::Defined(definition)
            if definition.abilities.iter().any(|ability| {
                granted_ability_contains_mana_notation_symbol(ability, symbol)
            })
    )
}

fn replacement_contains_mana_notation_symbol(
    replacement: &ReplacementEffect,
    symbol: &TypedManaSymbol,
) -> bool {
    match replacement {
        ReplacementEffect::ConditionalTokenSubstitution {
            ordinary,
            replacement,
            ..
        } => {
            token_creation_contains_mana_notation_symbol(ordinary, symbol)
                || token_creation_contains_mana_notation_symbol(replacement, symbol)
        }
        _ => false,
    }
}

fn ward_cost_contains_mana_notation_symbol(cost: &WardCost, symbol: &TypedManaSymbol) -> bool {
    matches!(
        cost,
        WardCost::Mana(cost) if mana_cost_contains_mana_notation_symbol(cost, symbol)
    )
}

fn parse_predefined_token_reminder(
    reminder: &str,
) -> Option<(PredefinedArtifactTokenKind, TokenGrammaticalNumber)> {
    match reminder {
        "they're artifacts with \"{t}, sacrifice this token: add one mana of any color.\"" => {
            Some((
                PredefinedArtifactTokenKind::Treasure,
                TokenGrammaticalNumber::Plural,
            ))
        }
        "it's an artifact with \"{t}, sacrifice this token: add one mana of any color.\""
        | "it's an artifact with \"{t}, sacrifice this artifact: add one mana of any color.\"" => {
            Some((
                PredefinedArtifactTokenKind::Treasure,
                TokenGrammaticalNumber::Singular,
            ))
        }
        "it's an artifact with \"{2}, {t}, sacrifice this token: you gain 3 life.\""
        | "it's an artifact with \"{2}, {t}, sacrifice this artifact: you gain 3 life.\"" => {
            Some((
                PredefinedArtifactTokenKind::Food,
                TokenGrammaticalNumber::Singular,
            ))
        }
        "it's an artifact with \"{2}, sacrifice this token: draw a card.\"" => Some((
            PredefinedArtifactTokenKind::Clue,
            TokenGrammaticalNumber::Singular,
        )),
        "they're artifacts with \"{2}, sacrifice this token: draw a card.\"" => Some((
            PredefinedArtifactTokenKind::Clue,
            TokenGrammaticalNumber::Plural,
        )),
        "it's an artifact with \"{1}, {t}, discard a card, sacrifice this token: draw a card.\"" => {
            Some((
                PredefinedArtifactTokenKind::Blood,
                TokenGrammaticalNumber::Singular,
            ))
        }
        "they're artifacts with \"{1}, {t}, discard a card, sacrifice this token: draw a card.\"" => {
            Some((
                PredefinedArtifactTokenKind::Blood,
                TokenGrammaticalNumber::Plural,
            ))
        }
        "it's an artifact with \"sacrifice this token: add one mana of any color.\"" => Some((
            PredefinedArtifactTokenKind::Gold,
            TokenGrammaticalNumber::Singular,
        )),
        "they're artifacts with \"sacrifice this token: add one mana of any color.\"" => Some((
            PredefinedArtifactTokenKind::Gold,
            TokenGrammaticalNumber::Plural,
        )),
        "it's an artifact with \"{2}, {t}, sacrifice this token: search your library for a basic land card, put it onto the battlefield tapped, then shuffle.\""
        | "a lander token is an artifact with \"{2}, {t}, sacrifice this token: search your library for a basic land card, put it onto the battlefield tapped, then shuffle.\"" => {
            Some((
                PredefinedArtifactTokenKind::Lander,
                TokenGrammaticalNumber::Singular,
            ))
        }
        "they're artifacts with \"{2}, {t}, sacrifice this token: search your library for a basic land card, put it onto the battlefield tapped, then shuffle.\""
        | "the tokens are artifacts with \"{2}, {t}, sacrifice this token: search your library for a basic land card, put it onto the battlefield tapped, then shuffle.\"" => {
            Some((
                PredefinedArtifactTokenKind::Lander,
                TokenGrammaticalNumber::Plural,
            ))
        }
        _ => None,
    }
}

fn parsed_predefined_token_number(
    parsed: &ParsedClause,
    kind: PredefinedArtifactTokenKind,
) -> Option<TokenGrammaticalNumber> {
    let mut numbers = parsed
        .predefined_token_creations
        .iter()
        .filter(|creation| creation.kind == kind)
        .map(|creation| creation.number);
    let first = numbers.next()?;
    numbers.all(|number| number == first).then_some(first)
}

fn canonical_reminder_text(reminder: &str) -> String {
    let mut normalized = collapse_whitespace(&reminder.replace('\u{2019}', "'"))
        .to_ascii_lowercase()
        .replace("this creature", "this object")
        .replace("this permanent", "this object")
        .replace("this card", "this object")
        .replace("this spell", "this object spell");
    normalized = normalized
        .replace(
            " as soon as he comes under your control",
            " as soon as it comes under your control",
        )
        .replace(
            " as soon as she comes under your control",
            " as soon as it comes under your control",
        )
        .replace(" the player he's attacking", " the player it's attacking")
        .replace(" the player she's attacking", " the player it's attacking")
        .replace(
            " the player they're attacking",
            " the player it's attacking",
        );
    normalized
}

fn explained_keyword(body: &str) -> Option<Keyword> {
    match body
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
        "deathtouch" => Some(Keyword::Deathtouch),
        "defender" => Some(Keyword::Defender),
        "double strike" => Some(Keyword::DoubleStrike),
        "first strike" => Some(Keyword::FirstStrike),
        "flying" => Some(Keyword::Flying),
        "haste" => Some(Keyword::Haste),
        "hexproof" => Some(Keyword::Hexproof),
        "indestructible" => Some(Keyword::Indestructible),
        "lifelink" => Some(Keyword::Lifelink),
        "menace" => Some(Keyword::Menace),
        "reach" => Some(Keyword::Reach),
        "shroud" => Some(Keyword::Shroud),
        "trample" => Some(Keyword::Trample),
        "vigilance" => Some(Keyword::Vigilance),
        _ => None,
    }
}

fn keyword_reminder_matches(keyword: &Keyword, reminder: &str) -> bool {
    match keyword {
        Keyword::Deathtouch => {
            reminder == "any amount of damage this deals to a creature is enough to destroy it."
                || reminder
                    == "any amount of damage this object deals to a creature is enough to destroy it."
        }
        Keyword::Defender => reminder == "this object can't attack.",
        Keyword::DoubleStrike => {
            reminder == "this object deals both first-strike and regular combat damage."
        }
        Keyword::FirstStrike => {
            reminder == "this object deals combat damage before creatures without first strike."
        }
        Keyword::Flying => {
            reminder == "this object can't be blocked except by creatures with flying or reach."
        }
        Keyword::Haste => {
            reminder == "this object can attack and {t} as soon as it comes under your control."
        }
        Keyword::Hexproof => {
            reminder
                == "this object can't be the target of spells or abilities your opponents control."
        }
        Keyword::Indestructible => {
            reminder == "effects that say \"destroy\" don't destroy this object."
                || reminder == "damage and effects that say \"destroy\" don't destroy this object."
        }
        Keyword::Lifelink => {
            reminder == "damage dealt by this object also causes you to gain that much life."
                || reminder
                    == "damage dealt by this object also causes its controller to gain that much life."
        }
        Keyword::Menace => {
            reminder == "this object can't be blocked except by two or more creatures."
        }
        Keyword::Reach => reminder == "this object can block creatures with flying.",
        Keyword::Shroud => reminder == "this object can't be the target of spells or abilities.",
        Keyword::Trample => {
            reminder
                == "this object can deal excess combat damage to the player or planeswalker it's attacking."
                || reminder
                    == "this object can deal excess combat damage to the player it's attacking."
        }
        Keyword::Vigilance => reminder == "attacking doesn't cause this object to tap.",
        Keyword::Ward(_) => false,
    }
}

fn parse_exact_keyword_explanations(
    address: ClauseAddress,
    reminder: &str,
) -> Result<Option<Vec<Keyword>>, CompileError> {
    let keywords = match reminder {
        "any amount of damage it deals to a creature is enough to destroy it."
        | "any amount of damage this object deals to a creature is enough to destroy it."
        | "any amount of damage it deals to a creature is enough to destroy that creature."
        | "any amount of damage they deal to a creature is enough to destroy it." => {
            vec![Keyword::Deathtouch]
        }
        "this object can't attack." => vec![Keyword::Defender],
        "it deals both first-strike and regular combat damage."
        | "they deal both first-strike and regular combat damage."
        | "this object deals both first-strike and regular combat damage." => {
            vec![Keyword::DoubleStrike]
        }
        "it deals combat damage before creatures without first strike."
        | "they deal combat damage before creatures without first strike."
        | "this object deals combat damage before creatures without first strike." => {
            vec![Keyword::FirstStrike]
        }
        "it can't be blocked except by creatures with flying or reach."
        | "she can't be blocked except by creatures with flying or reach."
        | "a creature with flying can't be blocked except by creatures with flying or reach."
        | "this object can't be blocked except by creatures with flying or reach." => {
            vec![Keyword::Flying]
        }
        "it can attack this turn."
        | "it can attack and {t} this turn."
        | "they can attack and {t} this turn."
        | "this object can attack and {t} this turn."
        | "it can attack and {t} as soon as it comes under your control."
        | "this object can attack and {t} as soon as it comes under your control."
        | "they can attack and {t} as soon as they come under your control."
        | "they can attack and {t} even if they just came under your control." => {
            vec![Keyword::Haste]
        }
        "it can't be the target of spells or abilities your opponents control."
        | "they can't be the targets of spells or abilities your opponents control."
        | "a creature with hexproof can't be the target of spells or abilities your opponents control."
        | "this object can't be the target of spells or abilities your opponents control." => {
            vec![Keyword::Hexproof]
        }
        "damage and effects that say \"destroy\" don't destroy it."
        | "damage and effects that say \"destroy\" don't destroy them."
        | "damage and effects that say \"destroy\" don't destroy him."
        | "damage and effects that say \"destroy\" don't destroy this object."
        | "damage and effects that say \"destroy\" don't destroy the creature."
        | "damage and effects that say \"destroy\" don't destroy those creatures."
        | "effects that say \"destroy\" don't destroy this object."
        | "effects that say \"destroy\" don't destroy a permanent with indestructible, and if it's a creature, it can't be destroyed by damage."
        | "damage and effects that say \"destroy\" don't destroy it. if its toughness is 0 or less, it still dies." =>
        {
            vec![Keyword::Indestructible]
        }
        "damage dealt by the creature also causes its controller to gain that much life."
        | "damage dealt by this object also causes its controller to gain that much life."
        | "damage dealt by this object also causes you to gain that much life."
        | "damage dealt by the creature also causes you to gain that much life."
        | "damage dealt by those warriors also causes their controller to gain that much life."
        | "damage dealt by it also causes you to gain that much life." => {
            vec![Keyword::Lifelink]
        }
        "it can't be blocked except by two or more creatures."
        | "they can't be blocked except by two or more creatures."
        | "a creature with menace can't be blocked except by two or more creatures."
        | "this object can't be blocked except by two or more creatures." => {
            vec![Keyword::Menace]
        }
        "it can block creatures with flying."
        | "they can block creatures with flying."
        | "a creature with reach can block creatures with flying."
        | "this object can block creatures with flying." => vec![Keyword::Reach],
        "it can't be the target of spells or abilities."
        | "they can't be the targets of spells or abilities."
        | "this object can't be the target of spells or abilities." => vec![Keyword::Shroud],
        "it can deal excess combat damage to the player or planeswalker it's attacking."
        | "it can deal excess combat damage to the player it's attacking."
        | "a creature with trample can deal excess combat damage to the player or planeswalker it's attacking."
        | "this object can deal excess combat damage to the player or planeswalker it's attacking."
        | "this object can deal excess combat damage to the player it's attacking."
        | "each of those creatures can deal excess combat damage to the player or planeswalker it's attacking." =>
        {
            vec![Keyword::Trample]
        }
        "attacking doesn't cause it to tap."
        | "attacking doesn't cause them to tap."
        | "attacking doesn't cause this object to tap." => vec![Keyword::Vigilance],
        "a permanent with hexproof and indestructible can't be the target of spells or abilities your opponents control. damage and effects that say \"destroy\" don't destroy it." =>
        {
            vec![Keyword::Hexproof, Keyword::Indestructible]
        }
        "it can't be the target of spells or abilities your opponents control. damage and effects that say \"destroy\" don't destroy it." =>
        {
            vec![Keyword::Hexproof, Keyword::Indestructible]
        }
        "damage dealt by that creature also causes its controller to gain that much life, and it can't be destroyed by damage or effects that say \"destroy.\"" =>
        {
            vec![Keyword::Lifelink, Keyword::Indestructible]
        }
        "attacking doesn't cause it to tap. damage dealt by it also causes you to gain that much life."
        | "attacking doesn't cause this object to tap. damage dealt by this object also causes you to gain that much life." =>
        {
            vec![Keyword::Vigilance, Keyword::Lifelink]
        }
        "attacking doesn't cause this object to tap. he can deal excess combat damage to the player it's attacking." =>
        {
            vec![Keyword::Vigilance, Keyword::Trample]
        }
        "it deals combat damage before creatures without first strike, and it can attack and {t} as soon as it comes under your control." =>
        {
            vec![Keyword::FirstStrike, Keyword::Haste]
        }
        "this object can deal excess combat damage to the player or planeswalker it's attacking. this object can attack and {t} as soon as it comes under your control." =>
        {
            vec![Keyword::Trample, Keyword::Haste]
        }
        "this object can't attack, and it can block creatures with flying." => {
            vec![Keyword::Defender, Keyword::Flying]
        }
        "this object can't be blocked except by creatures with flying or reach, and attacking doesn't cause this object to tap." =>
        {
            vec![Keyword::Flying, Keyword::Vigilance]
        }
        "this object can't be blocked except by creatures with flying or reach, and it deals combat damage before creatures without first strike." =>
        {
            vec![Keyword::Flying, Keyword::FirstStrike]
        }
        _ => {
            let ward_prefix = "whenever this object becomes the target of a spell or ability an opponent controls, counter it unless that player pays ";
            let Some(cost_text) = reminder
                .strip_prefix(ward_prefix)
                .and_then(|text| text.strip_suffix('.'))
            else {
                return Ok(None);
            };
            if !cost_text.starts_with('{') || !cost_text.ends_with('}') {
                return Ok(None);
            }
            vec![Keyword::Ward(Box::new(WardCost::Mana(parse_mana_cost(
                address, cost_text,
            )?)))]
        }
    };
    Ok(Some(keywords))
}

fn parsed_has_keyword_carrier(parsed: &ParsedClause, required: &[Keyword]) -> bool {
    !required.is_empty()
        && parsed
            .effects
            .iter()
            .any(|effect| effect_has_keyword_carrier(effect, required))
}

fn effect_has_keyword_carrier(effect: &Effect, required: &[Keyword]) -> bool {
    match effect {
        Effect::GrantKeyword { keywords, .. } => {
            required.iter().all(|required| keywords.contains(required))
        }
        Effect::Animate(animation) => required
            .iter()
            .all(|required| animation.keywords.contains(required)),
        Effect::CreateToken(creation)
        | Effect::CreateTokenAttached { creation, .. }
        | Effect::CreateTokenAndAttachSource { creation, .. }
        | Effect::CreateTokenWithDelayedMove { creation, .. } => {
            token_creation_has_keyword_carrier(creation, required)
        }
        Effect::GrantAbility { ability, .. } => ability
            .effects
            .iter()
            .any(|effect| effect_has_keyword_carrier(effect, required)),
        Effect::Optional(effects) => effects
            .iter()
            .any(|effect| effect_has_keyword_carrier(effect, required)),
        Effect::Conditional {
            if_true, if_false, ..
        } => if_true
            .iter()
            .chain(if_false)
            .any(|effect| effect_has_keyword_carrier(effect, required)),
        _ => false,
    }
}

fn token_creation_has_keyword_carrier(creation: &TokenCreation, required: &[Keyword]) -> bool {
    let TokenSpecification::Defined(definition) = &creation.specification else {
        return false;
    };
    required
        .iter()
        .all(|required| definition.keywords.contains(required))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LibraryReminderAction {
    Scry,
    Surveil,
}

fn parsed_library_action_amount(
    effects: &[Effect],
    action: LibraryReminderAction,
    reminder: &str,
) -> Option<Amount> {
    for effect in effects {
        match effect {
            Effect::Scry {
                player: PlayerRef::You,
                amount,
            } if action == LibraryReminderAction::Scry
                && exact_ordered_library_reminder_matches(action, amount, reminder) =>
            {
                return Some(amount.clone());
            }
            Effect::Surveil {
                player: PlayerRef::You,
                amount,
            } if action == LibraryReminderAction::Surveil
                && exact_ordered_library_reminder_matches(action, amount, reminder) =>
            {
                return Some(amount.clone());
            }
            Effect::Optional(nested) => {
                if let Some(amount) = parsed_library_action_amount(nested, action, reminder) {
                    return Some(amount);
                }
            }
            Effect::Conditional {
                if_true, if_false, ..
            } => {
                if let Some(amount) = parsed_library_action_amount(if_true, action, reminder)
                    .or_else(|| parsed_library_action_amount(if_false, action, reminder))
                {
                    return Some(amount);
                }
            }
            Effect::GrantAbility { ability, .. } => {
                if let Some(amount) =
                    parsed_library_action_amount(&ability.effects, action, reminder)
                {
                    return Some(amount);
                }
            }
            _ => {}
        }
    }
    None
}

fn exact_ordered_library_reminder_matches(
    action: LibraryReminderAction,
    amount: &Amount,
    reminder: &str,
) -> bool {
    let Amount::Constant(amount) = amount else {
        return false;
    };
    if *amount == 1 {
        return match action {
            LibraryReminderAction::Scry => matches!(
                reminder,
                "look at the top card of your library. you may put it on the bottom."
                    | "look at the top card of your library. you may put that card on the bottom."
                    | "to scry 1, look at the top card of your library. you may put that card on the bottom."
            ),
            LibraryReminderAction::Surveil => matches!(
                reminder,
                "look at the top card of your library. you may put it into your graveyard."
                    | "look at the top card of your library. you may put that card into your graveyard."
                    | "to surveil 1, look at the top card of your library. you may put it into your graveyard."
            ),
        };
    }
    let Some(amount_text) = english_constant_text(*amount) else {
        return false;
    };
    match action {
        LibraryReminderAction::Scry => {
            reminder
                == format!(
                    "look at the top {amount_text} cards of your library, then put any number of them on the bottom and the rest on top in any order."
                )
                || reminder
                    == format!(
                        "look at the top {amount_text} cards of your library, then put any number of them on the bottom of your library and the rest on top in any order."
                    )
        }
        LibraryReminderAction::Surveil => {
            reminder
                == format!(
                    "look at the top {amount_text} cards of your library, then put any number of them into your graveyard and the rest on top of your library in any order."
                )
                || reminder
                    == format!(
                        "to surveil {amount}, look at the top {amount_text} cards of your library, then put any number of them into your graveyard and the rest on top of your library in any order."
                    )
        }
    }
}

fn parsed_mill_reminder_semantics(
    effects: &[Effect],
    reminder: &str,
) -> Option<(PlayerRef, Amount, bool)> {
    parsed_mill_reminder_semantics_with_optionality(effects, reminder, false)
}

fn parsed_mill_reminder_semantics_with_optionality(
    effects: &[Effect],
    reminder: &str,
    optional: bool,
) -> Option<(PlayerRef, Amount, bool)> {
    for effect in effects {
        match effect {
            Effect::Mill { player, amount }
                if exact_mill_reminder_matches(player, amount, optional, reminder)
                    || mill_definition_scope_matches(player, amount, optional, reminder) =>
            {
                return Some((player.clone(), amount.clone(), optional));
            }
            Effect::Optional(nested) => {
                if let Some(semantics) =
                    parsed_mill_reminder_semantics_with_optionality(nested, reminder, true)
                {
                    return Some(semantics);
                }
            }
            Effect::Conditional {
                if_true, if_false, ..
            } => {
                if let Some(semantics) =
                    parsed_mill_reminder_semantics_with_optionality(if_true, reminder, optional)
                        .or_else(|| {
                            parsed_mill_reminder_semantics_with_optionality(
                                if_false, reminder, optional,
                            )
                        })
                {
                    return Some(semantics);
                }
            }
            Effect::GrantAbility { ability, .. } => {
                if let Some(semantics) = parsed_mill_reminder_semantics_with_optionality(
                    &ability.effects,
                    reminder,
                    optional,
                ) {
                    return Some(semantics);
                }
            }
            _ => {}
        }
    }
    None
}

fn exact_mill_reminder_matches(
    player: &PlayerRef,
    amount: &Amount,
    optional: bool,
    reminder: &str,
) -> bool {
    let Amount::Constant(amount) = amount else {
        return false;
    };
    let Some(card_count) = english_card_count(*amount) else {
        return false;
    };
    match player {
        PlayerRef::You if optional => {
            reminder
                == format!("you may put the top {card_count} of your library into your graveyard.")
        }
        PlayerRef::You => {
            reminder == format!("put the top {card_count} of your library into your graveyard.")
        }
        PlayerRef::Opponent if !optional => {
            reminder
                == format!(
                    "each opponent puts the top {card_count} of their library into their graveyard."
                )
                || reminder
                    == format!(
                        "they put the top {card_count} of their library into their graveyard."
                    )
        }
        PlayerRef::TargetPlayer(_) | PlayerRef::ThatPlayer | PlayerRef::Any if !optional => {
            reminder
                == format!("they put the top {card_count} of their library into their graveyard.")
        }
        _ => false,
    }
}

fn mill_definition_scope_matches(
    player: &PlayerRef,
    amount: &Amount,
    optional: bool,
    reminder: &str,
) -> bool {
    if optional {
        return false;
    }
    let Amount::Constant(amount) = amount else {
        return false;
    };
    let you = matches!(player, PlayerRef::You);
    let Some(card_count) = english_card_count(*amount) else {
        return false;
    };
    let amount_text = if *amount == 1 {
        "a".to_owned()
    } else {
        let Some(amount_text) = english_constant_text(*amount) else {
            return false;
        };
        amount_text.to_owned()
    };
    let noun = if *amount == 1 { "card" } else { "cards" };
    if you {
        reminder
            == format!(
                "to mill {amount_text} {noun}, put the top {card_count} of your library into your graveyard."
            )
    } else {
        reminder
            == format!(
                "to mill {amount_text} {noun}, a player puts the top {card_count} of their library into their graveyard."
            )
    }
}

fn english_constant_text(amount: u32) -> Option<&'static str> {
    match amount {
        1 => Some("one"),
        2 => Some("two"),
        3 => Some("three"),
        4 => Some("four"),
        5 => Some("five"),
        6 => Some("six"),
        7 => Some("seven"),
        8 => Some("eight"),
        9 => Some("nine"),
        10 => Some("ten"),
        _ => None,
    }
}

fn english_card_count(amount: u32) -> Option<String> {
    if amount == 1 {
        Some("card".to_owned())
    } else {
        english_constant_text(amount).map(|amount| format!("{amount} cards"))
    }
}

fn treasure_definition() -> TokenDefinition {
    TokenDefinition {
        name: Some("Treasure".into()),
        power: None,
        toughness: None,
        colors: Vec::new(),
        card_types: vec![CardType::Artifact],
        subtypes: vec!["Treasure".into()],
        keywords: Vec::new(),
        abilities: vec![GrantedAbility {
            costs: vec![
                Cost::Tap(ObjectRef::Source),
                Cost::SacrificeObject(ObjectRef::Source),
            ],
            effects: vec![Effect::AddMana(ManaProduction {
                player: PlayerRef::You,
                choices: any_color_choices(),
                amount: Amount::Constant(1),
                commander_identity_only: false,
                scales_with: None,
                typed: None,
            })],
        }],
    }
}

fn food_definition() -> TokenDefinition {
    TokenDefinition {
        name: Some("Food".into()),
        power: None,
        toughness: None,
        colors: Vec::new(),
        card_types: vec![CardType::Artifact],
        subtypes: vec!["Food".into()],
        keywords: Vec::new(),
        abilities: vec![GrantedAbility {
            costs: vec![
                Cost::Mana(ManaCost("{2}".into())),
                Cost::Tap(ObjectRef::Source),
                Cost::SacrificeObject(ObjectRef::Source),
            ],
            effects: vec![Effect::GainLife {
                player: PlayerRef::You,
                amount: Amount::Constant(3),
            }],
        }],
    }
}

fn clue_definition() -> TokenDefinition {
    TokenDefinition {
        name: Some("Clue".into()),
        power: None,
        toughness: None,
        colors: Vec::new(),
        card_types: vec![CardType::Artifact],
        subtypes: vec!["Clue".into()],
        keywords: Vec::new(),
        abilities: vec![GrantedAbility {
            costs: vec![
                Cost::Mana(ManaCost("{2}".into())),
                Cost::SacrificeObject(ObjectRef::Source),
            ],
            effects: vec![Effect::Draw {
                player: PlayerRef::You,
                amount: Amount::Constant(1),
                optional: false,
                delayed_until: None,
            }],
        }],
    }
}

fn blood_definition() -> TokenDefinition {
    TokenDefinition {
        name: Some("Blood".into()),
        power: None,
        toughness: None,
        colors: Vec::new(),
        card_types: vec![CardType::Artifact],
        subtypes: vec!["Blood".into()],
        keywords: Vec::new(),
        abilities: vec![GrantedAbility {
            costs: vec![
                Cost::Mana(ManaCost("{1}".into())),
                Cost::Tap(ObjectRef::Source),
                Cost::DiscardSelection(ObjectSelection {
                    id: 0,
                    chooser: PlayerRef::You,
                    filter: ObjectFilter {
                        zones: vec![Zone::Hand],
                        owner: Some(PlayerRef::You),
                        ..ObjectFilter::default()
                    },
                    amount: TargetAmount::Exactly(1),
                }),
                Cost::SacrificeObject(ObjectRef::Source),
            ],
            effects: vec![Effect::Draw {
                player: PlayerRef::You,
                amount: Amount::Constant(1),
                optional: false,
                delayed_until: None,
            }],
        }],
    }
}

fn gold_definition() -> TokenDefinition {
    TokenDefinition {
        name: Some("Gold".into()),
        power: None,
        toughness: None,
        colors: Vec::new(),
        card_types: vec![CardType::Artifact],
        subtypes: vec!["Gold".into()],
        keywords: Vec::new(),
        abilities: vec![GrantedAbility {
            costs: vec![Cost::SacrificeObject(ObjectRef::Source)],
            effects: vec![Effect::AddMana(ManaProduction {
                player: PlayerRef::You,
                choices: any_color_choices(),
                amount: Amount::Constant(1),
                commander_identity_only: false,
                scales_with: None,
                typed: None,
            })],
        }],
    }
}

fn lander_definition() -> TokenDefinition {
    TokenDefinition {
        name: Some("Lander".into()),
        power: None,
        toughness: None,
        colors: Vec::new(),
        card_types: vec![CardType::Artifact],
        subtypes: vec!["Lander".into()],
        keywords: Vec::new(),
        abilities: vec![GrantedAbility {
            costs: vec![
                Cost::Mana(ManaCost("{2}".into())),
                Cost::Tap(ObjectRef::Source),
                Cost::SacrificeObject(ObjectRef::Source),
            ],
            effects: vec![Effect::SearchLibrary(SearchLibrary {
                player: PlayerRef::You,
                chooser: PlayerRef::You,
                optional: false,
                allow_fail_to_find: true,
                amount: Amount::Constant(1),
                predicate: ObjectFilter {
                    zones: vec![Zone::Library],
                    card_types: vec![CardType::Land],
                    supertypes: vec![Supertype::Basic],
                    ..ObjectFilter::default()
                },
                reveal: false,
                destinations: vec![SearchDestination {
                    selected_ordinal: SearchOrdinal::Each,
                    zone: Zone::Battlefield,
                    tapped: true,
                }],
                shuffle_before_destination: false,
                shuffle_after: true,
            })],
        }],
    }
}

fn amount_text(amount: &Amount) -> Option<String> {
    match amount {
        Amount::Constant(value) => Some(value.to_string()),
        Amount::X => Some("x".into()),
        _ => None,
    }
}

fn parse_ward_cost(address: ClauseAddress, body: &str) -> Option<Result<WardCost, CompileError>> {
    let trimmed = body.trim().trim_end_matches('.').trim();
    let lower = trimmed.to_ascii_lowercase();
    let prefix = ["ward\u{2014}", "ward \u{2014} ", "ward "]
        .into_iter()
        .find(|prefix| lower.starts_with(prefix))?;
    let cost_text = trimmed[prefix.len()..].trim();
    if cost_text.starts_with('{') {
        return Some(parse_mana_cost(address, cost_text).map(WardCost::Mana));
    }
    let cost_lower = cost_text.to_ascii_lowercase();
    if let Some(amount_text) = cost_lower
        .strip_prefix("pay ")
        .and_then(|text| text.strip_suffix(" life"))
    {
        let result = parse_english_amount(amount_text)
            .map(WardCost::PayLife)
            .ok_or_else(|| unsupported(address, body));
        return Some(result);
    }
    Some(Err(unsupported(address, body)))
}

fn any_color_choices() -> Vec<ManaChoice> {
    [
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ]
    .into_iter()
    .map(|color| ManaChoice {
        symbols: vec![color],
    })
    .collect()
}

fn parse_mana_cost(address: ClauseAddress, text: &str) -> Result<ManaCost, CompileError> {
    let text = text.trim();
    let expression = parse_resource_cost_expression(text)
        .map_err(|error| invalid_mana_expression(address, text, &error))?;
    let [TypedResourceCostComponent::Mana(cost)] = expression.components.as_slice() else {
        return Err(invalid_mana_detail(
            address,
            text,
            "a mana-only payment cannot contain action or nonmana resources",
        ));
    };
    canonical_executable_mana_cost(address, text, cost)
}

fn parse_payment_cost(address: ClauseAddress, text: &str) -> Result<Cost, CompileError> {
    let text = text.trim();
    let (resource_text, inline_energy_amount) =
        if let Some(resource_text) = text.strip_suffix(" (two energy counters)") {
            (resource_text.trim(), Some(2u32))
        } else {
            (text, None)
        };
    let expression = parse_resource_cost_expression(resource_text)
        .map_err(|error| invalid_mana_expression(address, text, &error))?;
    let mut costs = compile_resource_cost(address, resource_text, &expression)?;
    if costs.len() != 1 {
        return Err(invalid_mana_detail(
            address,
            text,
            "a single payment instruction must retain one complete atomic cost",
        ));
    }
    let cost = costs
        .pop()
        .expect("one payment cost remains after the exact cardinality check");
    if let Some(expected_energy) = inline_energy_amount {
        let Cost::AtomicResource(atomic) = &cost else {
            return Err(invalid_mana_detail(
                address,
                text,
                "the energy reminder is not backed by an energy resource cost",
            ));
        };
        let symbols = atomic.special().symbols();
        if symbols.len() != expected_energy as usize
            || !symbols
                .iter()
                .all(|symbol| *symbol == SpecialCostSymbol::Energy)
        {
            return Err(invalid_mana_detail(
                address,
                text,
                "the energy reminder does not match the exact energy payment",
            ));
        }
    }
    Ok(cost)
}

fn canonical_executable_mana_cost(
    address: ClauseAddress,
    original: &str,
    cost: &crate::bounded_oracle_mana::ManaCost,
) -> Result<ManaCost, CompileError> {
    let mut canonical = String::new();
    for symbol in &cost.symbols {
        let text = match symbol {
            TypedManaSymbol::Generic(amount) => amount.to_string(),
            TypedManaSymbol::Color(color) => typed_mana_color_symbol(*color).to_owned(),
            TypedManaSymbol::VariableX => "X".to_owned(),
            TypedManaSymbol::Hybrid(left, right) => format!(
                "{}/{}",
                typed_mana_color_symbol(*left),
                typed_mana_color_symbol(*right)
            ),
            TypedManaSymbol::GenericHybrid { generic, color } => {
                format!("{generic}/{}", typed_mana_color_symbol(*color))
            }
            TypedManaSymbol::Phyrexian(color) => {
                format!("{}/P", typed_mana_color_symbol(*color))
            }
            TypedManaSymbol::Snow => {
                return Err(invalid_mana_detail(
                    address,
                    original,
                    "snow payment requires tracked snow-source provenance",
                ));
            }
        };
        canonical.push('{');
        canonical.push_str(&text);
        canonical.push('}');
    }
    if canonical.is_empty() {
        return Err(invalid_mana_detail(
            address,
            original,
            "the mana payment has no mana symbols",
        ));
    }
    Ok(ManaCost(canonical))
}

fn invalid_mana_expression(
    address: ClauseAddress,
    original: &str,
    error: &ManaExpressionError,
) -> CompileError {
    invalid_mana_detail(address, original, &error.to_string())
}

fn invalid_mana_detail(address: ClauseAddress, original: &str, detail: &str) -> CompileError {
    CompileError::InvalidMana {
        address,
        text: format!("{original} ({detail})"),
    }
}

fn typed_mana_color_symbol(color: TypedManaColor) -> &'static str {
    match color {
        TypedManaColor::White => "W",
        TypedManaColor::Blue => "U",
        TypedManaColor::Black => "B",
        TypedManaColor::Red => "R",
        TypedManaColor::Green => "G",
        TypedManaColor::Colorless => "C",
    }
}

fn parse_english_amount(text: &str) -> Option<Amount> {
    let normalized = text
        .trim()
        .trim_matches(|character: char| matches!(character, '.' | ','))
        .to_ascii_lowercase();
    if let Ok(value) = normalized.parse::<u32>() {
        return Some(Amount::Constant(value));
    }
    let constant = match normalized.as_str() {
        "a" | "an" | "one" | "1" => 1,
        "two" | "2" => 2,
        "three" | "3" => 3,
        "four" | "4" => 4,
        "five" | "5" => 5,
        "six" | "6" => 6,
        "seven" | "7" => 7,
        "eight" | "8" => 8,
        "nine" => 9,
        "ten" => 10,
        "thirty" => 30,
        _ => {
            return match normalized.as_str() {
                "x" => Some(Amount::X),
                "one or more" => Some(Amount::OneOrMore),
                "any number" => Some(Amount::Any),
                _ => None,
            };
        }
    };
    Some(Amount::Constant(constant))
}

fn parse_saga_lore_clause(
    address: ClauseAddress,
    clause: &str,
    source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let trimmed = clause.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.contains("as this saga enters and after your draw step") {
        return None;
    }
    let source_is_saga = words(source_type_line).iter().any(|word| word == "saga");
    if !source_is_saga || !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return Some(Err(unsupported(address, clause)));
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let inner_lower = inner.to_ascii_lowercase();
    let base = [
        "as this saga enters and after your draw step, add a lore counter.",
        "as this saga enters and after your draw step add a lore counter.",
    ]
    .into_iter()
    .find(|prefix| inner_lower.starts_with(prefix));
    let Some(base) = base else {
        return Some(Err(unsupported(address, clause)));
    };
    let remainder = inner[base.len()..].trim();
    let final_chapter = if remainder.is_empty() {
        SagaFinalChapter::HighestPrintedChapterOnFace
    } else {
        let remainder_lower = remainder.to_ascii_lowercase();
        let Some(roman_lower) = remainder_lower
            .strip_prefix("sacrifice after ")
            .and_then(|text| text.strip_suffix('.'))
        else {
            return Some(Err(unsupported(address, clause)));
        };
        let roman_start = "sacrifice after ".len();
        let roman = &remainder[roman_start..roman_start + roman_lower.len()];
        let Some(chapter) = parse_canonical_roman_chapter(roman) else {
            return Some(Err(unsupported(address, clause)));
        };
        SagaFinalChapter::PrintedUnvalidated(chapter)
    };

    let mut parsed = ParsedClause::new(Timing::Static);
    parsed.saga_lore_procedure = Some(SagaLoreProcedure {
        object: ObjectRef::Source,
        lore_counter: CounterKind::Named("lore".to_owned()),
        enters_with: Amount::Constant(1),
        after_controller_draw_step: Amount::Constant(1),
        final_chapter,
        state_based_sacrifice: SagaStateBasedSacrifice {
            sacrifice_source: true,
            lore_at_least_final_chapter: true,
            no_source_chapter_ability_on_stack: true,
        },
    });
    Some(Ok(parsed))
}

fn parse_saga_chapter_header(clause: &str) -> Option<Vec<u16>> {
    let normalized = collapse_whitespace(clause.trim());
    let (header, body) = normalized.split_once(" \u{2014} ")?;
    if body.trim().is_empty() {
        return None;
    }
    let chapters = header
        .split(',')
        .map(str::trim)
        .map(parse_canonical_roman_chapter)
        .collect::<Option<Vec<_>>>()?;
    (!chapters.is_empty()).then_some(chapters)
}

fn parse_canonical_roman_chapter(text: &str) -> Option<u16> {
    if text.is_empty()
        || text
            .chars()
            .any(|character| !matches!(character, 'I' | 'V' | 'X' | 'L' | 'C'))
    {
        return None;
    }
    let symbols = text
        .chars()
        .map(|character| match character {
            'I' => 1u16,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            _ => unreachable!("validated Roman chapter symbol"),
        })
        .collect::<Vec<_>>();
    let mut value = 0i32;
    for (index, symbol) in symbols.iter().copied().enumerate() {
        if symbols.get(index + 1).is_some_and(|next| symbol < *next) {
            value = value.checked_sub(i32::from(symbol))?;
        } else {
            value = value.checked_add(i32::from(symbol))?;
        }
    }
    let value = u16::try_from(value).ok()?;
    (value > 0 && canonical_roman_chapter(value).as_deref() == Some(text)).then_some(value)
}

fn canonical_roman_chapter(mut value: u16) -> Option<String> {
    if value == 0 || value > 399 {
        return None;
    }
    let mut roman = String::new();
    for (amount, symbol) in [
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ] {
        while value >= amount {
            roman.push_str(symbol);
            value -= amount;
        }
    }
    Some(roman)
}

fn parse_modal_clause(
    address: ClauseAddress,
    clause: &str,
    _source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let trimmed = clause.trim();
    if trimmed.eq_ignore_ascii_case("Choose one.") {
        let choices = ChoiceCount::Exactly(1);
        let mut parsed = ParsedClause::new(Timing::ModalHeader {
            choices: choices.clone(),
        });
        parsed.effects.push(Effect::ChooseMode { count: choices });
        return Some(Ok(parsed));
    }
    if trimmed.eq_ignore_ascii_case(
        "Choose one. If this object spell was cast using teamwork, choose both instead.",
    ) {
        let choices = ChoiceCount::OneOrBothIfTeamwork;
        let mut parsed = ParsedClause::new(Timing::ModalHeader {
            choices: choices.clone(),
        });
        parsed.effects.push(Effect::ChooseModeFrom {
            count: choices,
            option_count: 2,
        });
        return Some(Ok(parsed));
    }
    if trimmed.eq_ignore_ascii_case("An opponent chooses one \u{2014}") {
        let choices = ChoiceCount::Exactly(1);
        let mut parsed = ParsedClause::new(Timing::ModalHeader {
            choices: choices.clone(),
        });
        parsed.effects.push(Effect::ChooseModeBy {
            chooser: PlayerRef::Opponent,
            count: choices,
        });
        return Some(Ok(parsed));
    }
    if trimmed.eq_ignore_ascii_case(
        "At the beginning of your upkeep, choose one that hasn't been chosen \u{2014}",
    ) {
        let choices = ChoiceCount::Exactly(1);
        let trigger = Trigger::BeginningOf {
            step: Step::Upkeep,
            player: TurnPlayer::You,
        };
        let mut parsed = ParsedClause::new(Timing::TriggeredModalHeader {
            trigger: Box::new(trigger),
            choices: choices.clone(),
        });
        parsed
            .effects
            .push(Effect::ChooseModeNotPreviouslyChosen { count: choices });
        return Some(Ok(parsed));
    }
    if let Some(branch) = trimmed.strip_prefix('\u{2022}') {
        let branch = branch.trim();
        return Some(parse_effect_body(
            address,
            branch,
            Timing::ModalBranch {
                header_clause_index: None,
                branch_index: 0,
            },
        ));
    }
    let lower = trimmed.to_ascii_lowercase();
    let (suffix, choices) = [
        (
            "choose three. you may choose the same mode more than once.",
            ChoiceCount::ExactlyWithRepeats(3),
        ),
        (
            "choose two. you may choose the same mode more than once.",
            ChoiceCount::ExactlyWithRepeats(2),
        ),
        ("choose one or more \u{2014}", ChoiceCount::OneOrMore),
        (
            "choose one or both \u{2014}",
            ChoiceCount::Between {
                minimum: 1,
                maximum: 2,
            },
        ),
        ("choose up to two \u{2014}", ChoiceCount::UpTo(2)),
        ("choose up to one \u{2014}", ChoiceCount::UpTo(1)),
        ("choose two \u{2014}", ChoiceCount::Exactly(2)),
        ("choose one \u{2014}", ChoiceCount::Exactly(1)),
    ]
    .into_iter()
    .find(|(suffix, _)| lower.ends_with(suffix))?;
    let cost_text = trimmed[..trimmed.len() - suffix.len()].trim();
    if let Some(trigger_text) = cost_text.strip_suffix(',') {
        let trigger = match parse_trigger(trigger_text.trim()) {
            Some(trigger) => trigger,
            None => return Some(Err(unsupported(address, clause))),
        };
        let mut parsed = ParsedClause::new(Timing::TriggeredModalHeader {
            trigger: Box::new(trigger),
            choices: choices.clone(),
        });
        parsed.effects.push(Effect::ChooseMode { count: choices });
        return Some(Ok(parsed));
    }
    let costs = if cost_text.is_empty() {
        Vec::new()
    } else {
        let cost_text = cost_text.strip_suffix(':')?.trim();
        match parse_costs(address, cost_text) {
            Ok(costs) => costs,
            Err(error) => return Some(Err(error)),
        }
    };
    let mut parsed = ParsedClause::new(Timing::ModalHeader {
        choices: choices.clone(),
    });
    parsed.costs = costs;
    parsed.effects.push(Effect::ChooseMode { count: choices });
    Some(Ok(parsed))
}

fn parse_additional_cast_cost_clause(
    address: ClauseAddress,
    clause: &str,
    _source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let lower = clause.to_ascii_lowercase();
    let cost_text = lower
        .strip_prefix("as an additional cost to cast this object spell, ")
        .or_else(|| lower.strip_prefix("as an additional cost to cast this object, "))
        .and_then(|text| text.strip_suffix('.'))?;
    if matches!(
        cost_text,
        "you may collect evidence 6"
            | "you may collect evidence 6. (exile cards with total mana value 6 or greater from your graveyard.)"
    ) {
        let mut filter = ObjectFilter::in_zone(Zone::Graveyard);
        filter.owner = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed.costs.push(Cost::Optional(Box::new(
            Cost::ExileSelectionWithTotalManaValue {
                selection: ObjectSelection {
                    id: 0,
                    chooser: PlayerRef::You,
                    filter,
                    amount: TargetAmount::AnyNumber,
                },
                minimum: Amount::Constant(6),
            },
        )));
        return Some(Ok(parsed));
    }
    if matches!(
        cost_text,
        "you may blight 1"
            | "you may blight 1. (you may put a -1/-1 counter on a creature you control.)"
    ) {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed
            .costs
            .push(Cost::Optional(Box::new(Cost::PutCounterSelection {
                selection: ObjectSelection {
                    id: 0,
                    chooser: PlayerRef::You,
                    filter,
                    amount: TargetAmount::Exactly(1),
                },
                counter: CounterKind::MinusOneMinusOne,
                amount: Amount::Constant(1),
            })));
        return Some(Ok(parsed));
    }
    if matches!(
        cost_text,
        "you may behold a dragon"
            | "you may behold a dragon. (you may choose a dragon you control or reveal a dragon card from your hand.)"
    ) {
        let battlefield = ObjectFilter {
            zones: vec![Zone::Battlefield],
            controller: Some(PlayerRef::You),
            subtypes: vec!["Dragon".to_owned()],
            ..ObjectFilter::default()
        };
        let hand = ObjectFilter {
            zones: vec![Zone::Hand],
            owner: Some(PlayerRef::You),
            subtypes: vec!["Dragon".to_owned()],
            ..ObjectFilter::default()
        };
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed
            .costs
            .push(Cost::Optional(Box::new(Cost::BeholdSelection {
                choice_id: 0,
                battlefield,
                hand,
            })));
        return Some(Ok(parsed));
    }
    if matches!(
        cost_text,
        "waterbend {x}"
            | "waterbend {x}. (while paying a waterbend cost, you can tap your artifacts and creatures to help. each one pays for {1}.)"
    ) {
        let filter = ObjectFilter {
            zones: vec![Zone::Battlefield],
            controller: Some(PlayerRef::You),
            card_types: vec![CardType::Artifact, CardType::Creature],
            card_type_match_any: true,
            tapped: Some(false),
            ..ObjectFilter::default()
        };
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed.costs.push(Cost::Waterbend {
            selection: ObjectSelection {
                id: 0,
                chooser: PlayerRef::You,
                filter,
                amount: TargetAmount::AnyNumber,
            },
            amount: Amount::X,
        });
        return Some(Ok(parsed));
    }
    if cost_text == "sacrifice a creature or enchantment" {
        let mut filter = ObjectFilter::in_zone(Zone::Battlefield);
        filter.controller = Some(PlayerRef::You);
        filter.card_types = vec![CardType::Creature, CardType::Enchantment];
        filter.card_type_match_any = true;
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed.costs.push(Cost::Sacrifice {
            amount: Amount::Constant(1),
            filter,
        });
        return Some(Ok(parsed));
    }
    let reveal = match cost_text {
        "you may reveal a dragon card from your hand" => Some((
            ObjectFilter {
                zones: vec![Zone::Hand],
                owner: Some(PlayerRef::You),
                subtypes: vec!["Dragon".to_owned()],
                ..ObjectFilter::default()
            },
            true,
        )),
        "reveal a creature card from your hand" => Some((
            ObjectFilter {
                zones: vec![Zone::Hand],
                owner: Some(PlayerRef::You),
                card_types: vec![CardType::Creature],
                ..ObjectFilter::default()
            },
            false,
        )),
        _ => None,
    };
    if let Some((filter, optional)) = reveal {
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed.costs.push(Cost::RevealSelection {
            selection: ObjectSelection {
                id: 0,
                chooser: PlayerRef::You,
                filter,
                amount: TargetAmount::Exactly(1),
            },
            optional,
        });
        return Some(Ok(parsed));
    }
    if cost_text.starts_with("you may ")
        || cost_text.contains(". ")
        || cost_text.contains(" or ")
        || cost_text.contains("chosen at random")
        || cost_text.contains("reveal ")
        || cost_text.contains("choose ")
        || cost_text.contains("return ")
        || cost_text.contains("put ")
        || cost_text.contains("tap ")
    {
        return Some(Err(unsupported(address, clause)));
    }
    let costs = match parse_costs(address, cost_text) {
        Ok(costs) => costs,
        Err(error) => return Some(Err(error)),
    };
    let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
    parsed.costs = costs;
    Some(Ok(parsed))
}

fn parse_common_oracle_family_clause(
    address: ClauseAddress,
    clause: &str,
    _source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let lower = clause.trim().to_ascii_lowercase();

    if lower == "for each token you control, create a token that's a copy of that permanent." {
        let mut tokens = ObjectFilter::in_zone(Zone::Battlefield);
        tokens.controller = Some(PlayerRef::You);
        tokens.token = Some(true);
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.push(Effect::CreateToken(TokenCreation {
            player: PlayerRef::You,
            amount: Amount::Constant(1),
            specification: TokenSpecification::CopyOf(ObjectRef::EachMatching(tokens)),
            tapped: false,
            attacking: false,
        }));
        return Some(Ok(parsed));
    }

    if lower == "station" {
        let is_station_permanent =
            _source_type_line
                .split_once('\u{2014}')
                .is_some_and(|(_, subtypes)| {
                    subtypes.split_whitespace().any(|subtype| {
                        subtype.eq_ignore_ascii_case("Spacecraft")
                            || subtype.eq_ignore_ascii_case("Planet")
                    })
                });
        if !is_station_permanent {
            return Some(Err(unsupported(address, clause)));
        }
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        creature.controller = Some(PlayerRef::You);
        creature.tapped = Some(false);
        creature.other_than_source = true;
        let selection = ObjectSelection {
            id: 0,
            chooser: PlayerRef::You,
            filter: creature,
            amount: TargetAmount::Exactly(1),
        };
        let mut parsed = ParsedClause::new(Timing::Activated);
        parsed.costs.push(Cost::TapSelection(selection));
        parsed.effects.push(Effect::PutCounter {
            object: ObjectRef::Source,
            counter: CounterKind::Named("charge".to_owned()),
            amount: Amount::Count(Box::new(CountExpression::SelectedObjectsTotalPower {
                selection_id: 0,
            })),
        });
        parsed.activation_restriction = Some(ActivationRestriction::SorceryTiming);
        return Some(Ok(parsed));
    }

    if let Some(printed_cost) = lower.strip_prefix("outlast ") {
        let printed_cost = match parse_mana_cost(address, printed_cost) {
            Ok(cost) => cost,
            Err(error) => return Some(Err(error)),
        };
        let mut parsed = ParsedClause::new(Timing::Activated);
        parsed.costs = vec![Cost::Mana(printed_cost), Cost::Tap(ObjectRef::Source)];
        parsed.effects.push(Effect::PutCounter {
            object: ObjectRef::Source,
            counter: CounterKind::PlusOnePlusOne,
            amount: Amount::Constant(1),
        });
        parsed.activation_restriction = Some(ActivationRestriction::SorceryTiming);
        return Some(Ok(parsed));
    }

    if let Some(amount) = lower
        .strip_prefix("starting intensity ")
        .and_then(|text| text.trim_end_matches('.').parse::<u16>().ok())
    {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::InitializeIntensity { amount });
        return Some(Ok(parsed));
    }

    if lower == "roll a d20." {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.push(Effect::RollDie { sides: 20 });
        return Some(Ok(parsed));
    }
    if lower == "flip a coin." {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.push(Effect::FlipCoin);
        return Some(Ok(parsed));
    }

    if let Some(amount_text) = lower.trim_end_matches('.').strip_prefix("hideaway ")
        && let Some(amount) = parse_english_amount(amount_text)
    {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceEnters)));
        parsed.effects.extend([
            Effect::LookAtTop {
                player: PlayerRef::You,
                amount: amount.clone(),
            },
            Effect::SelectFromLookedAt {
                player: PlayerRef::You,
                amount: Amount::Constant(1),
                predicate: ObjectFilter::default(),
                reveal: false,
                face_down: true,
                tapped: false,
                destination: Zone::Exile,
            },
            Effect::PutRestOnLibraryBottom {
                player: PlayerRef::You,
                order: BottomOrder::ReplaySeededRandom,
            },
        ]);
        return Some(Ok(parsed));
    }

    if lower == "as this object enters, choose a nonland card name." {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed
            .effects
            .push(Effect::ChooseCardName { nonland: true });
        return Some(Ok(parsed));
    }

    if lower
        == "activated abilities of sources with the chosen name can't be activated unless they're mana abilities."
    {
        let objects = ObjectFilter {
            chosen_name_of_source: true,
            ..ObjectFilter::default()
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Restriction(
            Restriction::ActivatedAbilitiesOfMatchingSourcesCannotBeActivated {
                objects,
                except_mana_abilities: true,
                duration: Duration::WhileSourceOnBattlefield,
            },
        ));
        return Some(Ok(parsed));
    }

    if lower == "spells with the chosen name can't be cast." {
        let mut filter = ObjectFilter::with_type(CardType::Spell);
        filter.zones = vec![Zone::Stack];
        filter.chosen_name_of_source = true;
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotCast {
                affected: PlayerRef::Any,
                filter,
                duration: Duration::WhileSourceOnBattlefield,
                during_turn_of: None,
            }));
        return Some(Ok(parsed));
    }

    if lower
        == "creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you."
    {
        let mut attackers = ObjectFilter::with_type(CardType::Creature);
        attackers.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AttackCost {
                attackers,
                attacked_player: PlayerRef::You,
                mana_per_attacker: ManaCost("{2}".to_owned()),
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower == "lands you control enter untapped." {
        let mut objects = ObjectFilter::with_type(CardType::Land);
        objects.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::EntersUntapped {
                objects,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower == "draw three cards. then discard two cards unless you discard a creature card." {
        let mut alternative = ObjectFilter::with_type(CardType::Creature);
        alternative.zones = vec![Zone::Hand];
        alternative.owner = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.push(Effect::DrawThenDiscardUnless {
            player: PlayerRef::You,
            draw: 3,
            discard: 2,
            alternative,
            choice_id: 0,
        });
        return Some(Ok(parsed));
    }

    if lower
        == "when this object enters, create a 1/1 colorless eldrazi scion creature token. it has \"sacrifice this token: add {c}.\""
    {
        let token = TokenDefinition {
            name: Some("Eldrazi Scion".to_owned()),
            power: Some(Amount::Constant(1)),
            toughness: Some(Amount::Constant(1)),
            colors: Vec::new(),
            card_types: vec![CardType::Creature],
            subtypes: vec!["Eldrazi".to_owned(), "Scion".to_owned()],
            keywords: Vec::new(),
            abilities: vec![GrantedAbility {
                costs: vec![Cost::SacrificeObject(ObjectRef::Source)],
                effects: vec![Effect::AddMana(ManaProduction {
                    player: PlayerRef::ControllerOf(Box::new(ObjectRef::Source)),
                    choices: vec![ManaChoice {
                        symbols: vec![Color::Colorless],
                    }],
                    amount: Amount::Constant(1),
                    commander_identity_only: false,
                    scales_with: None,
                    typed: None,
                })],
            }],
        };
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceEnters)));
        parsed.effects.push(Effect::CreateToken(TokenCreation {
            player: PlayerRef::You,
            amount: Amount::Constant(1),
            specification: TokenSpecification::Defined(Box::new(token)),
            tapped: false,
            attacking: false,
        }));
        return Some(Ok(parsed));
    }

    if lower == "increment" {
        let mut spell = ObjectFilter::with_type(CardType::Spell);
        spell.zones = vec![Zone::Stack];
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::Cast {
            player: PlayerRef::You,
            spell,
        })));
        parsed
            .conditions
            .push(Condition::ManaSpentGreaterThanSourcePowerOrToughness);
        parsed.effects.push(Effect::PutCounter {
            object: ObjectRef::Source,
            counter: CounterKind::PlusOnePlusOne,
            amount: Amount::Constant(1),
        });
        return Some(Ok(parsed));
    }

    if lower == "you may have this object assign its combat damage as though it weren't blocked." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Restriction(
            Restriction::AssignCombatDamageAsThoughUnblocked {
                objects: ObjectRef::Source,
                duration: Duration::WhileSourceOnBattlefield,
            },
        ));
        return Some(Ok(parsed));
    }

    if lower == "you can't lose the game and your opponents can't win the game." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.extend([
            Effect::Restriction(Restriction::PlayerCannotLoseGame {
                players: PlayerRef::You,
                duration: Duration::WhileSourceOnBattlefield,
            }),
            Effect::Restriction(Restriction::PlayerCannotWinGame {
                players: PlayerRef::Opponent,
                duration: Duration::WhileSourceOnBattlefield,
            }),
        ]);
        return Some(Ok(parsed));
    }

    if lower == "you don't lose the game for having 0 or less life." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Restriction(
            Restriction::NonpositiveLifeDoesNotCauseLoss {
                players: PlayerRef::You,
                duration: Duration::WhileSourceOnBattlefield,
            },
        ));
        return Some(Ok(parsed));
    }

    if lower == "lands you control are every basic land type in addition to their other types." {
        let mut lands = ObjectFilter::with_type(CardType::Land);
        lands.zones = vec![Zone::Battlefield];
        lands.controller = Some(PlayerRef::You);
        let objects = ObjectRef::EachMatching(lands);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::SetCharacteristics(SetCharacteristics {
                object: objects.clone(),
                colors: None,
                card_types: None,
                subtypes: Some(
                    ["Plains", "Island", "Swamp", "Mountain", "Forest"]
                        .into_iter()
                        .map(str::to_owned)
                        .collect(),
                ),
                name: None,
                base_power: None,
                base_toughness: None,
                retain_other_card_types: true,
                retain_other_subtypes: true,
                retain_other_colors: true,
                retain_other_names: true,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        for color in [
            Color::White,
            Color::Blue,
            Color::Black,
            Color::Red,
            Color::Green,
        ] {
            parsed.effects.push(Effect::GrantAbility {
                objects: objects.clone(),
                ability: GrantedAbility {
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
                },
                duration: Duration::WhileSourceOnBattlefield,
            });
        }
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "enchanted land is an island." | "enchanted land is a swamp."
    ) {
        let (subtype, color) = if lower.contains("island") {
            ("Island", Color::Blue)
        } else {
            ("Swamp", Color::Black)
        };
        let attached = ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Aura,
        };
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
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.extend([
            Effect::LoseAllAbilities {
                object: attached.clone(),
                duration: Duration::WhileSourceOnBattlefield,
            },
            Effect::SetCharacteristics(SetCharacteristics {
                object: attached.clone(),
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
                duration: Duration::WhileSourceOnBattlefield,
            }),
            Effect::GrantAbility {
                objects: attached,
                ability: mana_ability,
                duration: Duration::WhileSourceOnBattlefield,
            },
        ]);
        return Some(Ok(parsed));
    }

    if lower == "cowards can't block warriors." {
        let mut cowards = ObjectFilter::with_type(CardType::Creature);
        cowards.zones = vec![Zone::Battlefield];
        cowards.subtypes.push("Coward".to_owned());
        let mut warriors = ObjectFilter::with_type(CardType::Creature);
        warriors.zones = vec![Zone::Battlefield];
        warriors.subtypes.push("Warrior".to_owned());
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotBlockMatching {
                blocker: ObjectRef::EachMatching(cowards),
                attacker_filter: warriors,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower == "enchanted creature has fear." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::BlockerMustMatch {
                attacker: ObjectRef::AttachmentTarget {
                    kind: AttachmentKind::Aura,
                },
                blocker_filter: fear_blocker_target_filter(),
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower == "cards in graveyards can't be the targets of spells or abilities." {
        let cards = ObjectFilter::in_zone(Zone::Graveyard);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::ObjectsCannotBeTargeted {
                objects: cards,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower
        == "at the beginning of the end step, if there are two or more ki counters on this object, you may flip it."
    {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::BeginningOf {
            step: Step::EndStep,
            player: TurnPlayer::EachPlayer,
        })));
        parsed.conditions.push(Condition::SourceCounterCount {
            counter: CounterKind::Named("ki".to_owned()),
            comparison: Comparison::AtLeast,
            amount: 2,
        });
        parsed
            .effects
            .push(Effect::Optional(vec![Effect::Transform {
                object: ObjectRef::Source,
            }]));
        return Some(Ok(parsed));
    }

    let kicker_sacrifice_filter = match lower.as_str() {
        "kicker\u{2014}sacrifice an artifact or creature."
        | "kicker\u{2014}sacrifice an artifact or creature. (you may sacrifice an artifact or creature in addition to any other costs as you cast this object spell.)" =>
        {
            let mut filter = ObjectFilter::in_zone(Zone::Battlefield);
            filter.controller = Some(PlayerRef::You);
            filter.card_types = vec![CardType::Artifact, CardType::Creature];
            filter.card_type_match_any = true;
            Some((filter, 1))
        }
        "kicker\u{2014}sacrifice a land."
        | "kicker\u{2014}sacrifice a land. (you may sacrifice a land in addition to any other costs as you cast this object spell.)" =>
        {
            let mut filter = ObjectFilter::with_type(CardType::Land);
            filter.zones = vec![Zone::Battlefield];
            filter.controller = Some(PlayerRef::You);
            Some((filter, 1))
        }
        "kicker\u{2014}sacrifice a creature."
        | "kicker\u{2014}sacrifice a creature. (you may sacrifice a creature in addition to any other costs as you cast this object spell.)" =>
        {
            let mut filter = ObjectFilter::with_type(CardType::Creature);
            filter.zones = vec![Zone::Battlefield];
            filter.controller = Some(PlayerRef::You);
            Some((filter, 1))
        }
        "kicker\u{2014}sacrifice two lands."
        | "kicker\u{2014}sacrifice two lands. (you may sacrifice two lands in addition to any other costs as you cast this object spell.)" =>
        {
            let mut filter = ObjectFilter::with_type(CardType::Land);
            filter.zones = vec![Zone::Battlefield];
            filter.controller = Some(PlayerRef::You);
            Some((filter, 2))
        }
        _ => None,
    };
    if let Some((filter, amount)) = kicker_sacrifice_filter {
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed.costs.push(Cost::Optional(Box::new(Cost::Sacrifice {
            amount: Amount::Constant(amount),
            filter,
        })));
        return Some(Ok(parsed));
    }

    if lower == "put target creature into its owner's library second from the top." {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(creature),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::PutInLibraryAtPosition {
            object: ObjectRef::Target(0),
            position_from_top: 2,
        });
        return Some(Ok(parsed));
    }

    let power_blocker_filter = lower
        .strip_prefix("this object can't be blocked by creatures with power ")
        .and_then(|remainder| remainder.strip_suffix('.'))
        .and_then(|remainder| {
            if let Some(amount) = remainder.strip_suffix(" or greater") {
                let amount = amount.parse::<u32>().ok()?;
                let maximum_allowed = amount.checked_sub(1)?;
                let mut filter = ObjectFilter::with_type(CardType::Creature);
                filter.zones = vec![Zone::Battlefield];
                filter.power = Some((
                    Comparison::AtMost,
                    Box::new(Amount::Constant(maximum_allowed)),
                ));
                Some(TargetFilter::Object(filter))
            } else if let Some(amount) = remainder.strip_suffix(" or less") {
                let amount = amount.parse::<u32>().ok()?;
                let minimum_allowed = amount.checked_add(1)?;
                let mut filter = ObjectFilter::with_type(CardType::Creature);
                filter.zones = vec![Zone::Battlefield];
                filter.power = Some((
                    Comparison::AtLeast,
                    Box::new(Amount::Constant(minimum_allowed)),
                ));
                Some(TargetFilter::Object(filter))
            } else {
                None
            }
        });
    if matches!(
        lower.as_str(),
        "this object can't be blocked except by creatures with flying."
            | "this object can't be blocked except by creatures with flying or reach."
    ) || power_blocker_filter.is_some()
    {
        let blocker_filter = match lower.as_str() {
            "this object can't be blocked except by creatures with flying." => {
                let mut filter = ObjectFilter::with_type(CardType::Creature);
                filter.zones = vec![Zone::Battlefield];
                filter.keywords.push(Keyword::Flying);
                TargetFilter::Object(filter)
            }
            "this object can't be blocked except by creatures with flying or reach." => {
                let mut flying = ObjectFilter::with_type(CardType::Creature);
                flying.zones = vec![Zone::Battlefield];
                flying.keywords.push(Keyword::Flying);
                let mut reach = ObjectFilter::with_type(CardType::Creature);
                reach.zones = vec![Zone::Battlefield];
                reach.keywords.push(Keyword::Reach);
                TargetFilter::Any(vec![
                    TargetFilter::Object(flying),
                    TargetFilter::Object(reach),
                ])
            }
            _ => power_blocker_filter.expect("power blocker form was validated above"),
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::BlockerMustMatch {
                attacker: ObjectRef::Source,
                blocker_filter,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower == "this object can't block creatures with power 2 or greater." {
        let mut attacker_filter = ObjectFilter::with_type(CardType::Creature);
        attacker_filter.zones = vec![Zone::Battlefield];
        attacker_filter.power = Some((Comparison::AtLeast, Box::new(Amount::Constant(2))));
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotBlockMatching {
                blocker: ObjectRef::Source,
                attacker_filter,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "this object can't be blocked as long as it's attacking alone."
            | "this object can't be blocked as long as defending player controls an artifact."
    ) {
        let condition = if lower.contains("attacking alone") {
            Condition::SourceAttackingAlone
        } else {
            let mut artifact = ObjectFilter::with_type(CardType::Artifact);
            artifact.zones = vec![Zone::Battlefield];
            Condition::ControlCount {
                player: PlayerRef::DefendingPlayer,
                filter: artifact,
                comparison: Comparison::AtLeast,
                amount: Amount::Constant(1),
            }
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotBeBlockedWhen {
                object: ObjectRef::Source,
                condition,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower
        == "you may activate abilities of creatures you control as though those creatures had haste."
    {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Restriction(
            Restriction::IgnoreSummoningSicknessForActivatedAbilities {
                objects: ObjectRef::EachMatching(creatures),
                duration: Duration::WhileSourceOnBattlefield,
            },
        ));
        return Some(Ok(parsed));
    }

    if lower == "spells you cast from anywhere other than your hand cost {2} less to cast." {
        let mut spells = ObjectFilter::in_zone(Zone::Stack);
        spells.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCostWhen {
            object: ObjectRef::EachMatching(spells),
            mana: ManaCost("{2}".to_owned()),
            condition: Condition::SpellCastFromNonHand,
        });
        return Some(Ok(parsed));
    }

    if lower
        == "this object spell costs {1} less to cast for each creature that attacked this turn."
    {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCost {
            object: ObjectRef::Source,
            mana: ManaCost("{1}".to_owned()),
            per: CountExpression::CreaturesAttackedThisTurn {
                player: PlayerRef::Any,
            },
            maximum_reduction: None,
        });
        return Some(Ok(parsed));
    }

    if lower == "the second spell you cast each turn costs {1} less to cast." {
        let mut spells = ObjectFilter::in_zone(Zone::Stack);
        spells.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCostWhen {
            object: ObjectRef::EachMatching(spells),
            mana: ManaCost("{1}".to_owned()),
            condition: Condition::SpellsCastByActorThisTurn {
                comparison: Comparison::Exactly,
                amount: 1,
            },
        });
        return Some(Ok(parsed));
    }

    if lower == "artifacts, creatures, and lands your opponents control enter tapped." {
        let mut permanents = ObjectFilter::in_zone(Zone::Battlefield);
        permanents.controller = Some(PlayerRef::Opponent);
        permanents.card_types = vec![CardType::Artifact, CardType::Creature, CardType::Land];
        permanents.card_type_match_any = true;
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::Replacement(Box::new(
            ReplacementEffect::EntersTapped(Box::new(EntersTappedReplacement {
                object: ObjectRef::EachMatching(permanents),
                when: None,
                unless: None,
                optional_cost: None,
                optional_reveal: None,
            })),
        )));
        return Some(Ok(parsed));
    }

    if lower
        == "enchanted permanent doesn't untap during its controller's untap step and its activated abilities can't be activated."
    {
        let attached = ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Aura,
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.extend([
            Effect::Restriction(Restriction::DoesNotUntapDuring {
                object: attached.clone(),
                step: Step::UntapStep,
            }),
            Effect::Restriction(Restriction::ActivatedAbilitiesCannotBeActivated {
                object: attached,
                duration: Duration::WhileSourceOnBattlefield,
            }),
        ]);
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "you may play an additional land this turn." | "play an additional land this turn."
    ) {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AdditionalLandPlays {
                player: PlayerRef::You,
                amount: 1,
                duration: Duration::ThisTurn,
            }));
        return Some(Ok(parsed));
    }

    if lower == "all creatures able to block target creature this turn do so." {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(creature.clone()),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed
            .effects
            .push(Effect::Restriction(Restriction::MustBlockIfAble {
                blockers: ObjectRef::EachMatching(creature),
                attacker: Some(ObjectRef::Target(0)),
                duration: Duration::UntilEndOfTurn,
            }));
        return Some(Ok(parsed));
    }

    if let Some(minimum_text) = lower.strip_prefix("teamwork ")
        && let Some(minimum) = parse_english_amount(minimum_text.trim_end_matches('.'))
    {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        creatures.tapped = Some(false);
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed.costs.push(Cost::Optional(Box::new(
            Cost::TapCreatureSelectionWithTotalPower {
                selection: ObjectSelection {
                    id: 0,
                    chooser: PlayerRef::You,
                    filter: creatures,
                    amount: TargetAmount::AnyNumber,
                },
                minimum,
            },
        )));
        return Some(Ok(parsed));
    }

    if lower == "destroy all creatures. they can't be regenerated." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.push(Effect::DestroyWithoutRegeneration {
            object: ObjectRef::EachMatching(creatures),
        });
        return Some(Ok(parsed));
    }

    let counter_to_exile_filter = match lower.as_str() {
        "counter target spell. if that spell is countered this way, exile it instead of putting it into its owner's graveyard." => {
            Some(ObjectFilter::default())
        }
        "counter target noncreature spell. if that spell is countered this way, exile it instead of putting it into its owner's graveyard." =>
        {
            let mut filter = ObjectFilter::default();
            filter.excluded_card_types.push(CardType::Creature);
            Some(filter)
        }
        "counter target creature spell. if that spell is countered this way, exile it instead of putting it into its owner's graveyard." => {
            Some(ObjectFilter::with_type(CardType::Creature))
        }
        "counter target creature or enchantment spell. if that spell is countered this way, exile it instead of putting it into its owner's graveyard." =>
        {
            let mut filter = ObjectFilter::default();
            filter.card_types = vec![CardType::Creature, CardType::Enchantment];
            filter.card_type_match_any = true;
            Some(filter)
        }
        "counter target creature or planeswalker spell. if that spell is countered this way, exile it instead of putting it into its owner's graveyard." =>
        {
            let mut filter = ObjectFilter::default();
            filter.card_types = vec![CardType::Creature, CardType::Planeswalker];
            filter.card_type_match_any = true;
            Some(filter)
        }
        "counter target artifact or enchantment spell. if that spell is countered this way, exile it instead of putting it into its owner's graveyard."
        | "• counter target artifact or enchantment spell. if that spell is countered this way, exile it instead of putting it into its owner's graveyard." =>
        {
            let mut filter = ObjectFilter::default();
            filter.card_types = vec![CardType::Artifact, CardType::Enchantment];
            filter.card_type_match_any = true;
            Some(filter)
        }
        "counter target creature spell with mana value 4 or less. if that spell is countered this way, exile it instead of putting it into its owner's graveyard." =>
        {
            let mut filter = ObjectFilter::with_type(CardType::Creature);
            filter.mana_value = Some((Comparison::AtMost, Box::new(Amount::Constant(4))));
            Some(filter)
        }
        _ => None,
    };
    if let Some(mut spell) = counter_to_exile_filter {
        spell.zones = vec![Zone::Stack];
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Spell(spell),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::CounterToZone {
            object: ObjectRef::Target(0),
            zone: Zone::Exile,
        });
        return Some(Ok(parsed));
    }

    if lower == "destroy target artifact, enchantment, or creature with flying." {
        let mut artifact = ObjectFilter::with_type(CardType::Artifact);
        artifact.zones = vec![Zone::Battlefield];
        let mut enchantment = ObjectFilter::with_type(CardType::Enchantment);
        enchantment.zones = vec![Zone::Battlefield];
        let mut flying_creature = ObjectFilter::with_type(CardType::Creature);
        flying_creature.zones = vec![Zone::Battlefield];
        flying_creature.keywords.push(Keyword::Flying);
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Any(vec![
                TargetFilter::Object(artifact),
                TargetFilter::Object(enchantment),
                TargetFilter::Object(flying_creature),
            ]),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::Destroy {
            object: ObjectRef::Target(0),
        });
        return Some(Ok(parsed));
    }

    if lower == "destroy target creature with toughness 4 or greater." {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        creature.toughness = Some((Comparison::AtLeast, Box::new(Amount::Constant(4))));
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(creature),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::Destroy {
            object: ObjectRef::Target(0),
        });
        return Some(Ok(parsed));
    }

    if lower == "put target attacking or blocking creature on top of its owner's library." {
        let mut attacking = ObjectFilter::with_type(CardType::Creature);
        attacking.zones = vec![Zone::Battlefield];
        attacking.attacking = Some(true);
        let mut blocking = ObjectFilter::with_type(CardType::Creature);
        blocking.zones = vec![Zone::Battlefield];
        blocking.blocking = Some(true);
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Any(vec![
                TargetFilter::Object(attacking),
                TargetFilter::Object(blocking),
            ]),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::PutOnLibraryTopInOrder {
            objects: ObjectRef::Target(0),
        });
        return Some(Ok(parsed));
    }

    if lower == "if this object was kicked, it enters with four +1/+1 counters on it." {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::Conditional {
            condition: Condition::CardWasKicked,
            if_true: vec![Effect::PutCounter {
                object: ObjectRef::Source,
                counter: CounterKind::PlusOnePlusOne,
                amount: Amount::Constant(4),
            }],
            if_false: Vec::new(),
        });
        return Some(Ok(parsed));
    }

    if lower
        == "when this object enters, sacrifice it unless you return a non-lair land you control to its owner's hand."
    {
        let mut land = ObjectFilter::with_type(CardType::Land);
        land.zones = vec![Zone::Battlefield];
        land.controller = Some(PlayerRef::You);
        land.excluded_subtypes.push("Lair".to_owned());
        let cost = Cost::ReturnSelectionToHand(ObjectSelection {
            id: 0,
            chooser: PlayerRef::You,
            filter: land,
            amount: TargetAmount::Exactly(1),
        });
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceEnters)));
        parsed.effects.push(Effect::Conditional {
            condition: Condition::UnlessPaid {
                player: PlayerRef::You,
                cost: cost.clone(),
            },
            if_true: vec![Effect::PayCost(Cost::SacrificeObject(ObjectRef::Source))],
            if_false: vec![Effect::PayCost(cost)],
        });
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "if you weren't the starting player, this object spell costs {1} less to cast."
            | "if you weren't the starting player, this object costs {1} less to cast."
    ) {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCostWhen {
            object: ObjectRef::Source,
            mana: ManaCost("{1}".to_owned()),
            condition: Condition::NotPlayingFirst,
        });
        return Some(Ok(parsed));
    }

    if lower == "when this object enters, look at target opponent's hand." {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceEnters)));
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Opponent,
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::LookAtHand {
            viewer: PlayerRef::You,
            player: PlayerRef::TargetPlayer(0),
        });
        return Some(Ok(parsed));
    }

    if lower
        == "target creature's owner puts it on their choice of the top or bottom of their library."
    {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(creature),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::MoveToChosenLibraryEnd {
            object: ObjectRef::Target(0),
            choice_id: 0,
        });
        return Some(Ok(parsed));
    }

    if lower == "look at target player's hand." {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Player,
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::LookAtHand {
            viewer: PlayerRef::You,
            player: PlayerRef::TargetPlayer(0),
        });
        return Some(Ok(parsed));
    }

    if lower == "target player exiles a card from their graveyard." {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Player,
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        let player = PlayerRef::TargetPlayer(0);
        let mut filter = ObjectFilter::in_zone(Zone::Graveyard);
        filter.owner = Some(player.clone());
        parsed.effects.push(Effect::MoveSelected(SelectedZoneMove {
            selection: ObjectSelection {
                id: 0,
                chooser: player,
                filter,
                amount: TargetAmount::Exactly(1),
            },
            to: Zone::Exile,
            tapped: false,
            face_down: false,
        }));
        return Some(Ok(parsed));
    }

    if lower
        == "gain control of target creature until end of turn. untap that creature. it gains haste until end of turn."
    {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(creature),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.extend([
            Effect::ChangeControlUntil {
                object: ObjectRef::Target(0),
                controller: PlayerRef::You,
                duration: Duration::UntilEndOfTurn,
            },
            Effect::Untap {
                object: ObjectRef::Target(0),
            },
            Effect::GrantKeyword {
                objects: ObjectRef::Target(0),
                keywords: vec![Keyword::Haste],
                duration: Duration::UntilEndOfTurn,
            },
        ]);
        return Some(Ok(parsed));
    }

    if lower == "this object has first strike as long as it's attacking." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::GrantKeyword {
            objects: ObjectRef::Source,
            keywords: vec![Keyword::FirstStrike],
            duration: Duration::WhileCondition(Box::new(Condition::SourceState(
                ObjectState::Attacking,
            ))),
        });
        return Some(Ok(parsed));
    }

    if lower == "this object has hexproof as long as it's untapped." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::TargetingProtection {
                object: ObjectRef::Source,
                forbidden_controller: PlayerRef::Opponent,
                duration: Duration::WhileCondition(Box::new(Condition::SourceState(
                    ObjectState::Untapped,
                ))),
            }));
        return Some(Ok(parsed));
    }

    if lower == "all creatures attack each combat if able." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Restriction(
            Restriction::MustAttackEachCombatIfAble {
                object: ObjectRef::EachMatching(creatures),
                duration: Duration::WhileSourceOnBattlefield,
            },
        ));
        return Some(Ok(parsed));
    }

    if lower == "creatures your opponents control attack each combat if able." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::Opponent);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Restriction(
            Restriction::MustAttackEachCombatIfAble {
                object: ObjectRef::EachMatching(creatures),
                duration: Duration::WhileSourceOnBattlefield,
            },
        ));
        return Some(Ok(parsed));
    }

    if lower == "creatures without flying can't attack." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.excluded_keywords.push(Keyword::Flying);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotAttack {
                object: ObjectRef::EachMatching(creatures),
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "creatures don't untap during their controllers' untap steps."
            | "creatures with flying don't untap during their controllers' untap steps."
    ) {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        if lower.starts_with("creatures with flying") {
            creatures.keywords.push(Keyword::Flying);
        }
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::DoesNotUntapDuring {
                object: ObjectRef::EachMatching(creatures),
                step: Step::UntapStep,
            }));
        return Some(Ok(parsed));
    }

    if lower == "creatures enter tapped." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::Replacement(Box::new(
            ReplacementEffect::EntersTapped(Box::new(EntersTappedReplacement {
                object: ObjectRef::EachMatching(creatures),
                when: None,
                unless: None,
                optional_cost: None,
                optional_reveal: None,
            })),
        )));
        return Some(Ok(parsed));
    }

    let filtered_static_restriction = [
        (" attack each combat if able.", 0u8),
        (" can't attack or block.", 1u8),
        (" can't attack.", 2u8),
        (" can't block.", 3u8),
        (" don't untap during their controllers' untap steps.", 4u8),
    ]
    .into_iter()
    .find_map(|(suffix, kind)| lower.strip_suffix(suffix).map(|subject| (subject, kind)));
    if let Some((subject, kind)) = filtered_static_restriction
        && !subject.starts_with("target ")
        && !subject.starts_with("this ")
        && !subject.starts_with("that ")
        && !subject.starts_with("enchanted ")
        && !subject.starts_with("equipped ")
        && let Some(mut creatures) = parse_card_filter_phrase(subject)
    {
        creatures.zones = vec![Zone::Battlefield];
        if creatures.card_types.is_empty() && !creatures.subtypes.is_empty() {
            creatures.card_types.push(CardType::Creature);
        }
        let objects = ObjectRef::EachMatching(creatures);
        let mut parsed = ParsedClause::new(Timing::Static);
        match kind {
            0 => parsed.effects.push(Effect::Restriction(
                Restriction::MustAttackEachCombatIfAble {
                    object: objects,
                    duration: Duration::WhileSourceOnBattlefield,
                },
            )),
            1 => parsed.effects.extend([
                Effect::Restriction(Restriction::CannotAttack {
                    object: objects.clone(),
                    duration: Duration::WhileSourceOnBattlefield,
                }),
                Effect::Restriction(Restriction::CannotBlock {
                    object: objects,
                    duration: Duration::WhileSourceOnBattlefield,
                }),
            ]),
            2 => parsed
                .effects
                .push(Effect::Restriction(Restriction::CannotAttack {
                    object: objects,
                    duration: Duration::WhileSourceOnBattlefield,
                })),
            3 => parsed
                .effects
                .push(Effect::Restriction(Restriction::CannotBlock {
                    object: objects,
                    duration: Duration::WhileSourceOnBattlefield,
                })),
            4 => parsed
                .effects
                .push(Effect::Restriction(Restriction::DoesNotUntapDuring {
                    object: objects,
                    step: Step::UntapStep,
                })),
            _ => unreachable!("closed filtered static restriction kind"),
        }
        return Some(Ok(parsed));
    }

    if let Some(subject) = lower.strip_suffix(" enter tapped.")
        && !subject.starts_with("target ")
        && !subject.starts_with("this ")
        && !subject.starts_with("that ")
        && let Some(mut creatures) = parse_card_filter_phrase(subject)
    {
        creatures.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::Replacement(Box::new(
            ReplacementEffect::EntersTapped(Box::new(EntersTappedReplacement {
                object: ObjectRef::EachMatching(creatures),
                when: None,
                unless: None,
                optional_cost: None,
                optional_reveal: None,
            })),
        )));
        return Some(Ok(parsed));
    }

    let filtered_static_change = match lower.as_str() {
        "creatures with flying get +2/+0." => {
            let mut creatures = ObjectFilter::with_type(CardType::Creature);
            creatures.zones = vec![Zone::Battlefield];
            creatures.keywords.push(Keyword::Flying);
            Some((creatures, PowerToughnessOperation::Add, 2, 0))
        }
        "creatures without flying get -2/-0." => {
            let mut creatures = ObjectFilter::with_type(CardType::Creature);
            creatures.zones = vec![Zone::Battlefield];
            creatures.excluded_keywords.push(Keyword::Flying);
            Some((creatures, PowerToughnessOperation::Subtract, 2, 0))
        }
        "creatures of the chosen type get +1/+1." => {
            let mut creatures = ObjectFilter::with_type(CardType::Creature);
            creatures.zones = vec![Zone::Battlefield];
            creatures.chosen_creature_type = true;
            Some((creatures, PowerToughnessOperation::Add, 1, 1))
        }
        _ => None,
    };
    if let Some((creatures, operation, power, toughness)) = filtered_static_change {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::EachMatching(creatures),
                operation,
                power: Amount::Constant(power),
                toughness: Amount::Constant(toughness),
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower == "creatures you control that are enchanted get +1/+1." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        creatures.attached_by = Some(AttachmentKind::Aura);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::EachMatching(creatures),
                operation: PowerToughnessOperation::Add,
                power: Amount::Constant(1),
                toughness: Amount::Constant(1),
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower == "equipped creatures you control get +1/+1." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        creatures.attached_by = Some(AttachmentKind::Equipment);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::EachMatching(creatures),
                operation: PowerToughnessOperation::Add,
                power: Amount::Constant(1),
                toughness: Amount::Constant(1),
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower == "creatures you control with flying get +1/+1." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        creatures.keywords.push(Keyword::Flying);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::EachMatching(creatures),
                operation: PowerToughnessOperation::Add,
                power: Amount::Constant(1),
                toughness: Amount::Constant(1),
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower == "creatures without flying can't block this turn." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.excluded_keywords.push(Keyword::Flying);
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotBlock {
                object: ObjectRef::EachMatching(creatures),
                duration: Duration::ThisTurn,
            }));
        return Some(Ok(parsed));
    }

    if lower == "cast this object spell only if you've cast another spell this turn."
        || lower == "cast this object only if you've cast another spell this turn."
    {
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed.conditions.push(Condition::AnotherSpellCastThisTurn);
        return Some(Ok(parsed));
    }

    if lower == "cast this object spell only during combat."
        || lower == "cast this object only during combat."
    {
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed.conditions.push(Condition::CastOnlyDuringCombat);
        return Some(Ok(parsed));
    }

    if lower == "cast this object spell only during combat before blockers are declared."
        || lower == "cast this object only during combat before blockers are declared."
    {
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed
            .conditions
            .push(Condition::CastOnlyDuringCombatBeforeBlockers);
        return Some(Ok(parsed));
    }

    if lower == "cast this object spell only during the declare blockers step."
        || lower == "cast this object only during the declare blockers step."
    {
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed
            .conditions
            .push(Condition::CastOnlyDuringDeclareBlockers);
        return Some(Ok(parsed));
    }

    if lower == "cast this object spell only during combat after blockers are declared."
        || lower == "cast this object only during combat after blockers are declared."
    {
        let mut parsed = ParsedClause::new(Timing::CastingAdditionalCost);
        parsed
            .conditions
            .push(Condition::CastOnlyDuringCombatAfterBlockers);
        return Some(Ok(parsed));
    }

    if lower == "whenever this object deals damage to a player, that player discards a card." {
        let mut parsed =
            ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceDamageToPlayer)));
        let mut filter = ObjectFilter::in_zone(Zone::Hand);
        filter.owner = Some(PlayerRef::ThatPlayer);
        parsed.effects.push(Effect::Discard(ObjectSelection {
            id: 0,
            chooser: PlayerRef::ThatPlayer,
            filter,
            amount: TargetAmount::Exactly(1),
        }));
        return Some(Ok(parsed));
    }

    if lower == "whenever a creature you control with a +1/+1 counter on it dies, draw a card." {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.controller = Some(PlayerRef::You);
        creature.minimum_counter = Some((CounterKind::PlusOnePlusOne, 1));
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::ObjectEvent {
            subject: TriggerSubject::Matching(creature),
            event: ObjectEventKind::Dies,
        })));
        parsed.effects.push(Effect::Draw {
            player: PlayerRef::You,
            amount: Amount::Constant(1),
            optional: false,
            delayed_until: None,
        });
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "threshold \u{2014} as long as there are seven or more cards in your graveyard, this object gets +2/+2 and can't block."
            | "as long as there are seven or more cards in your graveyard, this object gets +2/+2 and can't block."
    ) {
        let condition = Condition::GraveyardCardCount {
            player: PlayerRef::You,
            comparison: Comparison::AtLeast,
            amount: Amount::Constant(7),
        };
        let duration = Duration::WhileCondition(Box::new(condition));
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.extend([
            Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::Source,
                operation: PowerToughnessOperation::Add,
                power: Amount::Constant(2),
                toughness: Amount::Constant(2),
                duration: duration.clone(),
            }),
            Effect::Restriction(Restriction::CannotBlock {
                object: ObjectRef::Source,
                duration,
            }),
        ]);
        return Some(Ok(parsed));
    }

    const DELIRIUM_CONDITION: &str =
        "there are four or more card types among cards in your graveyard";
    if let Some(modifier_text) = lower
        .strip_prefix("this object gets ")
        .and_then(|text| text.strip_suffix(&format!(" as long as {DELIRIUM_CONDITION}.")))
        && let Some((operation, power, toughness)) =
            parse_power_toughness_modifier_pair(modifier_text)
    {
        let condition = Condition::CardTypesInGraveyard {
            player: PlayerRef::You,
            comparison: Comparison::AtLeast,
            amount: Amount::Constant(4),
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::Source,
                operation,
                power,
                toughness,
                duration: Duration::WhileCondition(Box::new(condition)),
            }));
        return Some(Ok(parsed));
    }
    if lower == format!("this object spell costs {{2}} less to cast if {DELIRIUM_CONDITION}.") {
        let condition = Condition::CardTypesInGraveyard {
            player: PlayerRef::You,
            comparison: Comparison::AtLeast,
            amount: Amount::Constant(4),
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCostWhen {
            object: ObjectRef::Source,
            mana: ManaCost("{2}".to_owned()),
            condition,
        });
        return Some(Ok(parsed));
    }

    if lower == "whenever this object becomes blocked, you may untap it and remove it from combat."
    {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::ObjectEvent {
            subject: TriggerSubject::Source,
            event: ObjectEventKind::BecomesBlocked,
        })));
        parsed.effects.push(Effect::Optional(vec![
            Effect::Untap {
                object: ObjectRef::Source,
            },
            Effect::RemoveFromCombat {
                object: ObjectRef::Source,
            },
        ]));
        return Some(Ok(parsed));
    }

    if lower
        == "whenever this object attacks, it gets +1/+0 until end of turn for each other attacking aurochs."
    {
        let mut aurochs = ObjectFilter::with_type(CardType::Creature);
        aurochs.zones = vec![Zone::Battlefield];
        aurochs.subtypes.push("Aurochs".to_owned());
        aurochs.attacking = Some(true);
        aurochs.other_than_source = true;
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceAttacks)));
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::Source,
                operation: PowerToughnessOperation::Add,
                power: Amount::Count(Box::new(CountExpression::MatchingObjects {
                    player: PlayerRef::Any,
                    filter: aurochs,
                })),
                toughness: Amount::Constant(0),
                duration: Duration::UntilEndOfTurn,
            }));
        return Some(Ok(parsed));
    }

    let afterlife_amount = match lower.as_str() {
        "afterlife 1" => Some(1),
        "afterlife 2" => Some(2),
        "afterlife 3" => Some(3),
        "afterlife 1 (when this object dies, create a 1/1 white and black spirit creature token with flying.)"
        | "afterlife 1 (when this object is put into a graveyard from the battlefield, create a 1/1 white and black spirit creature token with flying.)" => {
            Some(1)
        }
        "afterlife 2 (when this object dies, create two 1/1 white and black spirit creature tokens with flying.)" => {
            Some(2)
        }
        "afterlife 3 (when this object dies, create three 1/1 white and black spirit creature tokens with flying.)" => {
            Some(3)
        }
        _ => None,
    };
    if let Some(amount) = afterlife_amount {
        let event = if lower.contains(" is put into a graveyard from the battlefield,") {
            ObjectEventKind::PutIntoGraveyardFromBattlefield
        } else {
            ObjectEventKind::Dies
        };
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::ObjectEvent {
            subject: TriggerSubject::Source,
            event,
        })));
        parsed.effects.push(Effect::CreateToken(TokenCreation {
            player: PlayerRef::You,
            amount: Amount::Constant(amount),
            specification: TokenSpecification::Defined(Box::new(TokenDefinition {
                name: Some("Spirit".to_owned()),
                power: Some(Amount::Constant(1)),
                toughness: Some(Amount::Constant(1)),
                colors: vec![Color::White, Color::Black],
                card_types: vec![CardType::Creature],
                subtypes: vec!["Spirit".to_owned()],
                keywords: vec![Keyword::Flying],
                abilities: Vec::new(),
            })),
            tapped: false,
            attacking: false,
        }));
        return Some(Ok(parsed));
    }

    if lower
        == "put any number of target creature cards from your graveyard on top of your library."
    {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Graveyard];
        creatures.owner = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(creatures),
            amount: TargetAmount::AnyNumber,
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::PutOnLibraryTopInOrder {
            objects: ObjectRef::Target(0),
        });
        return Some(Ok(parsed));
    }

    if lower == "put target card from your graveyard on top of your library." {
        let mut card = ObjectFilter::in_zone(Zone::Graveyard);
        card.owner = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(card),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::PutOnLibraryTopInOrder {
            objects: ObjectRef::Target(0),
        });
        return Some(Ok(parsed));
    }

    if lower == "put target creature card from a graveyard onto the battlefield under your control."
    {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Graveyard];
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(creature),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::MoveZoneUnderControl {
            object: ObjectRef::Target(0),
            from: Zone::Graveyard,
            to: Zone::Battlefield,
            controller: PlayerRef::You,
            tapped: false,
            face_down: false,
            delayed_until: None,
        });
        return Some(Ok(parsed));
    }

    if lower == "tap target creature. it doesn't untap during its controller's next untap step." {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(filter),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.extend([
            Effect::Tap {
                object: ObjectRef::Target(0),
            },
            Effect::PreventNextUntap {
                object: ObjectRef::Target(0),
            },
        ]);
        return Some(Ok(parsed));
    }

    if lower
        == "whenever this object deals combat damage to a creature, tap that creature and it doesn't untap during its controller's next untap step."
    {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(
            Trigger::SourceCombatDamageToObject { object: creature },
        )));
        parsed.effects.extend([
            Effect::Tap {
                object: ObjectRef::TriggeringObject,
            },
            Effect::PreventNextUntap {
                object: ObjectRef::TriggeringObject,
            },
        ]);
        return Some(Ok(parsed));
    }

    if lower
        == "whenever this object blocks a creature, that creature doesn't untap during its controller's next untap step."
    {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceBlocks {
            object: creature,
        })));
        parsed.effects.push(Effect::PreventNextUntap {
            object: ObjectRef::TriggeringObject,
        });
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "tap up to two target creatures. those creatures don't untap during their controllers' next untap steps."
            | "tap up to two target creatures. those creatures don't untap during their controller's next untap step."
    ) {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(filter),
            amount: TargetAmount::UpTo(2),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.extend([
            Effect::Tap {
                object: ObjectRef::Target(0),
            },
            Effect::PreventNextUntap {
                object: ObjectRef::Target(0),
            },
        ]);
        return Some(Ok(parsed));
    }

    if lower
        == "when this object enters, tap target creature an opponent controls and put a stun counter on it."
    {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::Opponent);
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceEnters)));
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(filter),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.extend([
            Effect::Tap {
                object: ObjectRef::Target(0),
            },
            Effect::PutCounter {
                object: ObjectRef::Target(0),
                counter: CounterKind::Named("stun".to_owned()),
                amount: Amount::Constant(1),
            },
        ]);
        return Some(Ok(parsed));
    }

    if lower
        == "when this object enters, tap target creature an opponent controls. that creature doesn't untap during its controller's next untap step."
    {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::Opponent);
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceEnters)));
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(filter),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.extend([
            Effect::Tap {
                object: ObjectRef::Target(0),
            },
            Effect::PreventNextUntap {
                object: ObjectRef::Target(0),
            },
        ]);
        return Some(Ok(parsed));
    }

    if lower == "creatures your opponents control enter tapped." {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::Opponent);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Replacement(Box::new(
            ReplacementEffect::EntersTapped(Box::new(EntersTappedReplacement {
                object: ObjectRef::EachMatching(filter),
                when: None,
                unless: None,
                optional_cost: None,
                optional_reveal: None,
            })),
        )));
        return Some(Ok(parsed));
    }

    if lower == "if you control two or more other lands, this object enters tapped." {
        let mut filter = ObjectFilter::with_type(CardType::Land);
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::You);
        filter.other_than_source = true;
        let condition = Condition::ControlCount {
            player: PlayerRef::You,
            filter,
            comparison: Comparison::AtLeast,
            amount: Amount::Constant(2),
        };
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.conditions.push(condition.clone());
        parsed.effects.push(Effect::Replacement(Box::new(
            ReplacementEffect::EntersTapped(Box::new(EntersTappedReplacement {
                object: ObjectRef::Source,
                when: Some(condition),
                unless: None,
                optional_cost: None,
                optional_reveal: None,
            })),
        )));
        return Some(Ok(parsed));
    }

    if lower == "noncreature spells you cast cost {1} less to cast." {
        let mut filter = ObjectFilter::in_zone(Zone::Stack);
        filter.controller = Some(PlayerRef::You);
        filter.excluded_card_types.push(CardType::Creature);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCost {
            object: ObjectRef::EachMatching(filter),
            mana: ManaCost("{1}".to_owned()),
            per: CountExpression::Constant(1),
            maximum_reduction: None,
        });
        return Some(Ok(parsed));
    }

    if lower
        == "this object spell costs {x} less to cast, where x is the greatest power among creatures you control."
        || lower
            == "this object costs {x} less to cast, where x is the greatest power among creatures you control."
    {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCost {
            object: ObjectRef::Source,
            mana: ManaCost("{1}".to_owned()),
            per: CountExpression::GreatestPower {
                player: PlayerRef::You,
                filter: creatures,
            },
            maximum_reduction: None,
        });
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "this object spell costs {1} less to cast for each creature in your party."
            | "this object costs {1} less to cast for each creature in your party."
    ) {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCost {
            object: ObjectRef::Source,
            mana: ManaCost("{1}".to_owned()),
            per: CountExpression::PartySize {
                player: PlayerRef::You,
            },
            maximum_reduction: None,
        });
        return Some(Ok(parsed));
    }

    if lower == "noncreature spells cost {1} more to cast." {
        let mut filter = ObjectFilter::in_zone(Zone::Stack);
        filter.excluded_card_types.push(CardType::Creature);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::IncreaseSpellCost {
            object: ObjectRef::EachMatching(filter),
            mana: ManaCost("{1}".to_owned()),
            per: CountExpression::Constant(1),
        });
        return Some(Ok(parsed));
    }

    if lower == "spells your opponents cast that target this object cost {2} more to cast." {
        let mut filter = ObjectFilter::in_zone(Zone::Stack);
        filter.controller = Some(PlayerRef::Opponent);
        filter.targets_source = true;
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::IncreaseSpellCost {
            object: ObjectRef::EachMatching(filter),
            mana: ManaCost("{2}".to_owned()),
            per: CountExpression::Constant(1),
        });
        return Some(Ok(parsed));
    }

    if lower == "spells you cast of the chosen type cost {1} less to cast." {
        let mut filter = ObjectFilter::in_zone(Zone::Stack);
        filter.card_types.push(CardType::Spell);
        filter.controller = Some(PlayerRef::You);
        filter.chosen_creature_type = true;
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCost {
            object: ObjectRef::EachMatching(filter),
            mana: ManaCost("{1}".to_owned()),
            per: CountExpression::Constant(1),
            maximum_reduction: None,
        });
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "undaunted"
            | "undaunted (this spell costs {1} less to cast for each opponent.)"
            | "undaunted (this object spell costs {1} less to cast for each opponent.)"
    ) {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCost {
            object: ObjectRef::Source,
            mana: ManaCost("{1}".to_owned()),
            per: CountExpression::OpponentCount {
                player: PlayerRef::You,
            },
            maximum_reduction: None,
        });
        return Some(Ok(parsed));
    }

    if lower
        == "this object doesn't untap during your untap step if it has a depletion counter on it."
    {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::DoesNotUntapDuringIf {
                object: ObjectRef::Source,
                step: Step::UntapStep,
                condition: Condition::SourceHasCounter {
                    counter: CounterKind::Named("depletion".to_owned()),
                },
            }));
        return Some(Ok(parsed));
    }

    if lower
        == "at the beginning of the end step, if no creatures are on the battlefield, sacrifice this object."
    {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::BeginningOf {
            step: Step::EndStep,
            player: TurnPlayer::EachPlayer,
        })));
        parsed.conditions.push(Condition::ControlCount {
            player: PlayerRef::Any,
            filter,
            comparison: Comparison::Exactly,
            amount: Amount::Constant(0),
        });
        parsed
            .effects
            .push(Effect::PayCost(Cost::SacrificeObject(ObjectRef::Source)));
        return Some(Ok(parsed));
    }

    if lower == "when this object dies, you gain life equal to its power." {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::ObjectEvent {
            subject: TriggerSubject::Source,
            event: ObjectEventKind::Dies,
        })));
        parsed.effects.push(Effect::GainLife {
            player: PlayerRef::You,
            amount: Amount::Count(Box::new(CountExpression::PowerOf {
                object: ObjectRef::Source,
            })),
        });
        return Some(Ok(parsed));
    }

    if let Some(counter_text) = lower
        .strip_prefix("whenever you cast a spell that targets this object, put ")
        .and_then(|text| {
            text.strip_suffix(" counters on this object.")
                .or_else(|| text.strip_suffix(" counter on this object."))
                .or_else(|| text.strip_suffix(" counters on it."))
                .or_else(|| text.strip_suffix(" counter on it."))
        })
        && let Some((amount, counter_name)) = parse_counter_amount_and_name(counter_text)
    {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::BecomesTarget {
            object: ObjectRef::Source,
            controller: PlayerRef::You,
            source_kinds: vec![CardType::Spell],
        })));
        parsed.effects.push(Effect::PutCounter {
            object: ObjectRef::Source,
            counter: parse_counter_kind(counter_name),
            amount,
        });
        return Some(Ok(parsed));
    }

    if lower
        == "whenever you cast a spell that targets this object, creatures you control get +1/+0 until end of turn."
    {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::BecomesTarget {
            object: ObjectRef::Source,
            controller: PlayerRef::You,
            source_kinds: vec![CardType::Spell],
        })));
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::EachMatching(creatures),
                operation: PowerToughnessOperation::Add,
                power: Amount::Constant(1),
                toughness: Amount::Constant(0),
                duration: Duration::UntilEndOfTurn,
            }));
        return Some(Ok(parsed));
    }

    if lower == "this object escapes with a +1/+1 counter on it." {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::Conditional {
            condition: Condition::CardWasCastUsingEscape,
            if_true: vec![Effect::PutCounter {
                object: ObjectRef::Source,
                counter: CounterKind::PlusOnePlusOne,
                amount: Amount::Constant(1),
            }],
            if_false: Vec::new(),
        });
        return Some(Ok(parsed));
    }

    let graveyard_threshold_suffix = " as long as there are seven or more cards in your graveyard.";
    let graveyard_threshold_prefix = "as long as there are seven or more cards in your graveyard, ";
    let threshold_bonus = lower
        .strip_prefix("this object gets ")
        .and_then(|text| text.strip_suffix(graveyard_threshold_suffix))
        .map(|pair| (ObjectRef::Source, pair))
        .or_else(|| {
            lower
                .strip_prefix(graveyard_threshold_prefix)
                .and_then(|text| text.strip_prefix("this object gets "))
                .and_then(|text| text.strip_suffix('.'))
                .map(|pair| (ObjectRef::Source, pair))
        })
        .or_else(|| {
            lower
                .strip_prefix("enchanted creature gets ")
                .and_then(|text| text.strip_suffix(graveyard_threshold_suffix))
                .map(|pair| {
                    (
                        ObjectRef::AttachmentTarget {
                            kind: AttachmentKind::Aura,
                        },
                        pair.strip_prefix("an additional ").unwrap_or(pair),
                    )
                })
        })
        .or_else(|| {
            lower
                .strip_prefix("other creatures you control get ")
                .and_then(|text| text.strip_suffix(graveyard_threshold_suffix))
                .map(|pair| {
                    let mut creatures = ObjectFilter::with_type(CardType::Creature);
                    creatures.zones = vec![Zone::Battlefield];
                    creatures.controller = Some(PlayerRef::You);
                    creatures.other_than_source = true;
                    (ObjectRef::EachMatching(creatures), pair)
                })
        })
        .or_else(|| {
            lower
                .strip_prefix(graveyard_threshold_prefix)
                .and_then(|text| text.strip_prefix("creatures your opponents control get "))
                .and_then(|text| text.strip_suffix('.'))
                .map(|pair| {
                    let mut creatures = ObjectFilter::with_type(CardType::Creature);
                    creatures.zones = vec![Zone::Battlefield];
                    creatures.controller = Some(PlayerRef::Opponent);
                    (ObjectRef::EachMatching(creatures), pair)
                })
        })
        .or_else(|| {
            lower
                .strip_prefix("all squirrels get ")
                .and_then(|text| text.strip_suffix(graveyard_threshold_suffix))
                .map(|pair| {
                    let mut squirrels = ObjectFilter::with_type(CardType::Creature);
                    squirrels.zones = vec![Zone::Battlefield];
                    squirrels.subtypes = vec!["squirrel".to_owned()];
                    (ObjectRef::EachMatching(squirrels), pair)
                })
        });
    if let Some((objects, pair_text)) = threshold_bonus
        && let Some((operation, power, toughness)) = parse_power_toughness_modifier_pair(pair_text)
    {
        let condition = Condition::GraveyardCardCount {
            player: PlayerRef::You,
            comparison: Comparison::AtLeast,
            amount: Amount::Constant(7),
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects,
                operation,
                power,
                toughness,
                duration: Duration::WhileCondition(Box::new(condition)),
            }));
        return Some(Ok(parsed));
    }

    if let Some(pair_text) = lower
        .strip_prefix("this object gets ")
        .and_then(|text| text.strip_suffix(" as long as you control three or more artifacts."))
        && let Some((operation, power, toughness)) = parse_power_toughness_modifier_pair(pair_text)
    {
        let mut artifacts = ObjectFilter::with_type(CardType::Artifact);
        artifacts.zones = vec![Zone::Battlefield];
        artifacts.controller = Some(PlayerRef::You);
        let condition = Condition::ControlCount {
            player: PlayerRef::You,
            filter: artifacts,
            comparison: Comparison::AtLeast,
            amount: Amount::Constant(3),
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::Source,
                operation,
                power,
                toughness,
                duration: Duration::WhileCondition(Box::new(condition)),
            }));
        return Some(Ok(parsed));
    }

    if let Some(pair_text) = lower
        .strip_prefix("creatures you control get ")
        .and_then(|text| text.strip_suffix(" as long as you control three or more artifacts."))
        && let Some((operation, power, toughness)) = parse_power_toughness_modifier_pair(pair_text)
    {
        let mut artifacts = ObjectFilter::with_type(CardType::Artifact);
        artifacts.zones = vec![Zone::Battlefield];
        artifacts.controller = Some(PlayerRef::You);
        let condition = Condition::ControlCount {
            player: PlayerRef::You,
            filter: artifacts,
            comparison: Comparison::AtLeast,
            amount: Amount::Constant(3),
        };
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::EachMatching(creatures),
                operation,
                power,
                toughness,
                duration: Duration::WhileCondition(Box::new(condition)),
            }));
        return Some(Ok(parsed));
    }

    if lower == "this object enters with a +1/+1 counter on it if you attacked this turn." {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::Conditional {
            condition: Condition::YouAttackedThisTurn,
            if_true: vec![Effect::PutCounter {
                object: ObjectRef::Source,
                counter: CounterKind::PlusOnePlusOne,
                amount: Amount::Constant(1),
            }],
            if_false: Vec::new(),
        });
        return Some(Ok(parsed));
    }

    if lower == "when this object enters, sacrifice it unless you pay {1}." {
        let cost = Cost::Mana(ManaCost("{1}".to_owned()));
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceEnters)));
        parsed.effects.push(Effect::Conditional {
            condition: Condition::UnlessPaid {
                player: PlayerRef::You,
                cost: cost.clone(),
            },
            if_true: vec![Effect::MoveZone(ZoneMove {
                object: ObjectRef::Source,
                from: Some(Zone::Battlefield),
                to: Zone::Graveyard,
                tapped: false,
                face_down: false,
                delayed_until: None,
            })],
            if_false: vec![Effect::PayCost(cost)],
        });
        return Some(Ok(parsed));
    }

    if lower == "this object enters with a +1/+1 counter on it if an opponent lost life this turn."
    {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::Conditional {
            condition: Condition::OpponentLostLifeThisTurn,
            if_true: vec![Effect::PutCounter {
                object: ObjectRef::Source,
                counter: CounterKind::PlusOnePlusOne,
                amount: Amount::Constant(1),
            }],
            if_false: Vec::new(),
        });
        return Some(Ok(parsed));
    }

    if let Some(pair_text) = lower
        .strip_prefix("attacking creatures get ")
        .and_then(|text| text.strip_suffix(" until end of turn."))
        && let Some((operation, power, toughness)) = parse_power_toughness_modifier_pair(pair_text)
    {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.attacking = Some(true);
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::EachMatching(creatures),
                operation,
                power,
                toughness,
                duration: Duration::UntilEndOfTurn,
            }));
        return Some(Ok(parsed));
    }

    if let Some(pair_text) = lower
        .strip_prefix("enchanted creature gets ")
        .and_then(|text| text.strip_suffix('.'))
        && let Some((operation, power, toughness)) = parse_power_toughness_modifier_pair(pair_text)
        && source_type_line_can_supply_attachment_kind(_source_type_line, AttachmentKind::Aura)
    {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::AttachmentTarget {
                    kind: AttachmentKind::Aura,
                },
                operation,
                power,
                toughness,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower == "equipped creature gets +1/+1 for each creature you control." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        let amount = Amount::Count(Box::new(CountExpression::MatchingObjects {
            player: PlayerRef::You,
            filter: creatures,
        }));
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::AttachmentTarget {
                    kind: AttachmentKind::Equipment,
                },
                operation: PowerToughnessOperation::Add,
                power: amount.clone(),
                toughness: amount,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower == "this object gets +2/+2 for each aura attached to it." {
        let amount = Amount::Product {
            factor: 2,
            value: Box::new(Amount::Count(Box::new(CountExpression::AttachmentsOn {
                object: ObjectRef::Source,
                kind: AttachmentKind::Aura,
            }))),
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::Source,
                operation: PowerToughnessOperation::Add,
                power: amount.clone(),
                toughness: amount,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }

    if lower
        == "enchanted permanent loses all abilities and doesn't untap during its controller's untap step."
    {
        let enchanted = ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Aura,
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.extend([
            Effect::LoseAllAbilities {
                object: enchanted.clone(),
                duration: Duration::WhileSourceOnBattlefield,
            },
            Effect::Restriction(Restriction::DoesNotUntapDuring {
                object: enchanted,
                step: Step::UntapStep,
            }),
        ]);
        return Some(Ok(parsed));
    }

    if lower
        == "return target nonland permanent to its owner's hand. if this object spell was kicked, draw a card."
    {
        let mut permanent = ObjectFilter::with_type(CardType::Permanent);
        permanent.zones = vec![Zone::Battlefield];
        permanent.excluded_card_types.push(CardType::Land);
        let target = Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(permanent),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        };
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(target);
        parsed.effects.extend([
            Effect::MoveZone(ZoneMove {
                object: ObjectRef::Target(0),
                from: Some(Zone::Battlefield),
                to: Zone::Hand,
                tapped: false,
                face_down: false,
                delayed_until: None,
            }),
            Effect::Conditional {
                condition: Condition::CardWasKicked,
                if_true: vec![Effect::Draw {
                    player: PlayerRef::You,
                    amount: Amount::Constant(1),
                    optional: false,
                    delayed_until: None,
                }],
                if_false: Vec::new(),
            },
        ]);
        return Some(Ok(parsed));
    }

    if let Some(pair_text) = lower
        .strip_prefix("enchanted creature gets ")
        .and_then(|text| text.strip_suffix(" and attacks each combat if able."))
        && let Some((operation, power, toughness)) = parse_power_toughness_modifier_pair(pair_text)
    {
        let enchanted = ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Aura,
        };
        let duration = Duration::WhileSourceOnBattlefield;
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.extend([
            Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: enchanted.clone(),
                operation,
                power,
                toughness,
                duration: duration.clone(),
            }),
            Effect::Restriction(Restriction::MustAttackEachCombatIfAble {
                object: enchanted,
                duration,
            }),
        ]);
        return Some(Ok(parsed));
    }

    if let Some((cost_text, state)) = lower
        .strip_prefix("this object spell costs ")
        .or_else(|| lower.strip_prefix("this spell costs "))
        .and_then(|text| {
            text.strip_suffix(" less to cast if it targets a tapped creature.")
                .map(|cost| (cost, ObjectState::Tapped))
                .or_else(|| {
                    text.strip_suffix(" less to cast if it targets an attacking creature.")
                        .map(|cost| (cost, ObjectState::Attacking))
                })
        })
        && matches!(cost_text, "{2}" | "{3}")
    {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCostWhen {
            object: ObjectRef::Source,
            mana: ManaCost(cost_text.to_owned()),
            condition: Condition::SpellTargetsState {
                card_type: CardType::Creature,
                state,
            },
        });
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "you may cast this object as though it had flash."
            | "you may cast this object spell as though it had flash."
            | "cast this object as though it had flash."
            | "cast this object spell as though it had flash."
    ) {
        let mut filter = ObjectFilter::in_zone(Zone::Hand);
        filter.owner = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::GrantCastPermission(CastPermission {
                affected: PlayerRef::You,
                objects: Some(ObjectRef::Source),
                filter,
                from: Zone::Hand,
                timing: CastTiming::AsThoughFlash,
                duration: Duration::Permanent,
                alternative_cost: None,
                additional_costs: Vec::new(),
                mana_as_any_type: false,
                exile_after_resolution: false,
            }));
        return Some(Ok(parsed));
    }

    if lower
        == "you may pay {1} and return a basic land you control to its owner's hand rather than pay this object spell's mana cost."
    {
        let mut basic_land = ObjectFilter::with_type(CardType::Land);
        basic_land.zones = vec![Zone::Battlefield];
        basic_land.controller = Some(PlayerRef::You);
        basic_land.supertypes.push(Supertype::Basic);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AlternativeCastPermission(
                Box::new(AlternativeCastPermission {
                    object: ObjectRef::Source,
                    from: Zone::Hand,
                    cost: AlternativeCost::Costs(vec![
                        Cost::Mana(ManaCost("{1}".to_owned())),
                        Cost::ReturnSelectionToHand(ObjectSelection {
                            id: 0,
                            chooser: PlayerRef::You,
                            filter: basic_land,
                            amount: TargetAmount::Exactly(1),
                        }),
                    ]),
                    timing: Trigger::SourceCast,
                    condition: None,
                }),
            )));
        return Some(Ok(parsed));
    }

    if lower
        == "if you control a plains, you may tap an untapped creature you control rather than pay this object spell's mana cost."
    {
        let mut plains = ObjectFilter::with_type(CardType::Land);
        plains.zones = vec![Zone::Battlefield];
        plains.controller = Some(PlayerRef::You);
        plains.subtypes.push("Plains".to_owned());
        let condition = Condition::ControlCount {
            player: PlayerRef::You,
            filter: plains,
            comparison: Comparison::AtLeast,
            amount: Amount::Constant(1),
        };
        let mut untapped_creature = ObjectFilter::with_type(CardType::Creature);
        untapped_creature.zones = vec![Zone::Battlefield];
        untapped_creature.controller = Some(PlayerRef::You);
        untapped_creature.tapped = Some(false);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.conditions.push(condition.clone());
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AlternativeCastPermission(
                Box::new(AlternativeCastPermission {
                    object: ObjectRef::Source,
                    from: Zone::Hand,
                    cost: AlternativeCost::Costs(vec![Cost::TapSelection(ObjectSelection {
                        id: 0,
                        chooser: PlayerRef::You,
                        filter: untapped_creature,
                        amount: TargetAmount::Exactly(1),
                    })]),
                    timing: Trigger::SourceCast,
                    condition: Some(condition),
                }),
            )));
        return Some(Ok(parsed));
    }

    if let Some(cost_text) = lower
        .strip_prefix("you may cast this object spell as though it had flash if you pay ")
        .and_then(|text| text.strip_suffix(" more to cast it."))
    {
        let extra_cost = match parse_mana_cost(address, cost_text) {
            Ok(cost) => Cost::Mana(cost),
            Err(error) => return Some(Err(error)),
        };
        let mut filter = ObjectFilter::in_zone(Zone::Hand);
        filter.owner = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::GrantCastPermission(CastPermission {
                affected: PlayerRef::You,
                objects: Some(ObjectRef::Source),
                filter,
                from: Zone::Hand,
                timing: CastTiming::AsThoughFlash,
                duration: Duration::Permanent,
                alternative_cost: None,
                additional_costs: vec![extra_cost],
                mana_as_any_type: false,
                exile_after_resolution: false,
            }));
        return Some(Ok(parsed));
    }

    if let Some(amount_text) = lower.strip_prefix("look at the top ").and_then(|text| {
        text.strip_suffix(" cards of your library, then put them back in any order.")
    }) && let Some(amount) = parse_english_amount(amount_text)
    {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.extend([
            Effect::LookAtTop {
                player: PlayerRef::You,
                amount,
            },
            Effect::ReorderLookedAtOnLibraryTop {
                player: PlayerRef::You,
            },
        ]);
        return Some(Ok(parsed));
    }

    if lower
        == "look at the top three cards of your library. put one of them into your hand and the rest into your graveyard."
    {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.extend([
            Effect::LookAtTop {
                player: PlayerRef::You,
                amount: Amount::Constant(3),
            },
            Effect::SelectFromLookedAt {
                player: PlayerRef::You,
                amount: Amount::Constant(1),
                predicate: ObjectFilter::default(),
                reveal: false,
                face_down: false,
                tapped: false,
                destination: Zone::Hand,
            },
            Effect::PutRestOfLookedAt {
                player: PlayerRef::You,
                destination: Zone::Graveyard,
            },
        ]);
        return Some(Ok(parsed));
    }
    if lower
        == "look at the top two cards of your library. put one of them into your hand and the other on the bottom of your library."
    {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.extend([
            Effect::LookAtTop {
                player: PlayerRef::You,
                amount: Amount::Constant(2),
            },
            Effect::SelectFromLookedAt {
                player: PlayerRef::You,
                amount: Amount::Constant(1),
                predicate: ObjectFilter::default(),
                reveal: false,
                face_down: false,
                tapped: false,
                destination: Zone::Hand,
            },
            Effect::PutRestOnLibraryBottom {
                player: PlayerRef::You,
                order: BottomOrder::AnyOrder,
            },
        ]);
        return Some(Ok(parsed));
    }

    if lower == "({u/p} can be paid with either {u} or 2 life.)" {
        let mut parsed = ParsedClause::new(Timing::Static);
        let explanation =
            parse_exact_mana_notation_sentence("{u/p} can be paid with either {u} or 2 life.")
                .expect("the exact standalone blue Phyrexian reminder is canonical");
        parsed.reminder = Some(ReminderSemantics::ManaNotationExplanation(explanation));
        return Some(Ok(parsed));
    }

    if lower
        == "take an extra turn after this one. at the beginning of that turn's end step, you lose the game."
    {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.push(Effect::TakeExtraTurn(ExtraTurnEffect {
            player: PlayerRef::You,
            lose_at_end_step: true,
        }));
        return Some(Ok(parsed));
    }
    if lower == "take an extra turn after this one."
        || lower == "take one extra turn after this one."
    {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.push(Effect::TakeExtraTurn(ExtraTurnEffect {
            player: PlayerRef::You,
            lose_at_end_step: false,
        }));
        return Some(Ok(parsed));
    }

    if let Some(cost_text) = lower
        .strip_prefix("at the beginning of your next upkeep, pay ")
        .and_then(|text| text.strip_suffix(". if you don't, you lose the game."))
    {
        let cost = match parse_mana_cost(address, cost_text) {
            Ok(cost) => Cost::Mana(cost),
            Err(error) => return Some(Err(error)),
        };
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::BeginningOf {
            step: Step::Upkeep,
            player: TurnPlayer::NextTurn,
        })));
        parsed
            .effects
            .push(Effect::SchedulePaymentOrLose(PaymentOrLoseEffect {
                player: PlayerRef::You,
                cost,
                trigger: Trigger::BeginningOf {
                    step: Step::Upkeep,
                    player: TurnPlayer::NextTurn,
                },
            }));
        return Some(Ok(parsed));
    }

    if lower == "each player discards their hand, then draws seven cards." {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::DiscardHandsAndDraw {
                player: PlayerRef::Any,
                amount: Amount::Constant(7),
            },
        ));
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "target opponent reveals their hand. you choose a nonland card from it. that player discards that card."
            | "target player reveals their hand. you choose a nonland card from it. that player discards that card."
    ) {
        let target_player = PlayerRef::TargetPlayer(0);
        let mut nonland = ObjectFilter::in_zone(Zone::Hand);
        nonland.owner = Some(target_player.clone());
        nonland.excluded_card_types.push(CardType::Land);
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: if lower.starts_with("target opponent") {
                TargetFilter::Opponent
            } else {
                TargetFilter::Player
            },
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.extend([
            Effect::RevealHand {
                player: target_player,
            },
            Effect::Discard(ObjectSelection {
                id: 0,
                chooser: PlayerRef::You,
                filter: nonland,
                amount: TargetAmount::Exactly(1),
            }),
        ]);
        return Some(Ok(parsed));
    }

    for (prefix, player) in [
        (
            "discard all the cards in your hand, then draw ",
            PlayerRef::You,
        ),
        (
            "each player discards all the cards in their hand, then draws ",
            PlayerRef::Any,
        ),
        (
            "target player discards all the cards in their hand, then draws ",
            PlayerRef::TargetPlayer(0),
        ),
    ] {
        let Some(relative_draw) = lower.strip_prefix(prefix) else {
            continue;
        };
        let adjustment = match relative_draw {
            "that many cards." => 0,
            "that many cards plus one." => 1,
            "that many cards minus one." => -1,
            _ => continue,
        };
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        if matches!(player, PlayerRef::TargetPlayer(0)) {
            parsed.targets.push(Target {
                id: 0,
                chooser: PlayerRef::You,
                filter: TargetFilter::Player,
                amount: TargetAmount::Exactly(1),
                relationship: TargetRelationship::Independent,
            });
        }
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::DiscardHandsAndDrawDiscarded { player, adjustment },
        ));
        return Some(Ok(parsed));
    }

    if let Some((surveil_text, remainder)) = lower
        .strip_prefix("surveil ")
        .and_then(|text| text.split_once(", then draw "))
        && let Some((draw_text, life_text)) = remainder.split_once(". you lose ")
        && let Some(draw_text) = draw_text.strip_suffix(" cards")
        && let Some(life_text) = life_text.strip_suffix(" life.")
        && let (Some(surveil), Some(draw), Some(life)) = (
            parse_english_amount(surveil_text),
            parse_english_amount(draw_text),
            parse_english_amount(life_text),
        )
    {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.extend([
            Effect::Surveil {
                player: PlayerRef::You,
                amount: surveil,
            },
            Effect::Draw {
                player: PlayerRef::You,
                amount: draw,
                optional: false,
                delayed_until: None,
            },
            Effect::LoseLife {
                player: PlayerRef::You,
                amount: life,
            },
        ]);
        return Some(Ok(parsed));
    }

    if lower
        == "reveal the top card of your library and put that card into your hand. you lose life equal to its mana value. you may repeat this process any number of times."
    {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::RevealTopToHandLoseManaValue {
                player: PlayerRef::You,
                repeat: Amount::Any,
            },
        ));
        return Some(Ok(parsed));
    }

    if lower
        == "choose a card name. exile the top six cards of your library, then reveal cards from the top of your library until you reveal a card with the chosen name. put that card into your hand and exile all other cards revealed this way."
    {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::ExileUntilNamedCard {
                player: PlayerRef::You,
                initial_exile: 6,
            },
        ));
        return Some(Ok(parsed));
    }

    if lower
        == "exile the top card of your library. you may put that card into your hand unless it has the same name as another card exiled this way. repeat this process until you put a card into your hand or you exile two cards with the same name, whichever comes first."
    {
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::ExileUntilAcceptedOrDuplicate {
                player: PlayerRef::You,
            },
        ));
        return Some(Ok(parsed));
    }

    if lower == "destroy target nonblack creature. it can't be regenerated." {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        creature.excluded_colors.push(Color::Black);
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(creature),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::DestroyWithoutRegeneration {
            object: ObjectRef::Target(0),
        });
        return Some(Ok(parsed));
    }

    if lower
        == "whenever enchanted land is tapped for mana, its controller adds an additional one mana of any color."
    {
        let enchanted = ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Aura,
        };
        let mut parsed =
            ParsedClause::new(Timing::Triggered(Box::new(Trigger::ObjectTappedForMana {
                object: enchanted.clone(),
            })));
        parsed.effects.push(Effect::AddMana(ManaProduction {
            player: PlayerRef::ControllerOf(Box::new(enchanted)),
            choices: any_color_choices(),
            amount: Amount::Constant(1),
            commander_identity_only: false,
            scales_with: None,
            typed: None,
        }));
        return Some(Ok(parsed));
    }

    if lower
        == "when enchanted creature dies, return that card to the battlefield under your control."
    {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(
            Trigger::AttachmentTargetEvent {
                kind: AttachmentKind::Aura,
                event: ObjectEventKind::Dies,
            },
        )));
        parsed.effects.push(Effect::MoveZoneUnderControl {
            object: ObjectRef::TriggeringObject,
            from: Zone::Graveyard,
            to: Zone::Battlefield,
            controller: PlayerRef::You,
            tapped: false,
            face_down: false,
            delayed_until: None,
        });
        return Some(Ok(parsed));
    }

    if lower == "when enchanted creature is dealt damage, destroy it." {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(
            Trigger::AttachmentTargetEvent {
                kind: AttachmentKind::Aura,
                event: ObjectEventKind::DealtDamage,
            },
        )));
        parsed.effects.push(Effect::Destroy {
            object: ObjectRef::TriggeringObject,
        });
        return Some(Ok(parsed));
    }

    if lower == "when this object dies, put its counters on target creature you control." {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        creature.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::ObjectEvent {
            subject: TriggerSubject::Source,
            event: ObjectEventKind::Dies,
        })));
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(creature),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::MoveAllCounters {
            from: ObjectRef::Source,
            to: ObjectRef::Target(0),
        });
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "exile all cards from all opponents' graveyards. you may cast spells from among those cards this turn, and you may spend mana as though it were mana of any type to cast those spells. at the beginning of the next end step, if any of those cards remain exiled, return them to their owners' graveyards."
            | "exile all opponents' graveyards. you may cast spells from among those cards this turn, and mana of any type can be spent to cast them. at the beginning of the next end step, if any of those cards remain exiled, return them to their owners' graveyards."
    ) {
        let mut cards = ObjectFilter::in_zone(Zone::Graveyard);
        cards.owner = Some(PlayerRef::Opponent);
        let mut castable = ObjectFilter::in_zone(Zone::Exile);
        castable.owner = Some(PlayerRef::Opponent);
        castable.excluded_card_types.push(CardType::Land);
        let permission = CastPermission {
            affected: PlayerRef::You,
            objects: None,
            filter: castable,
            from: Zone::Exile,
            timing: CastTiming::Normal,
            duration: Duration::ThisTurn,
            alternative_cost: None,
            additional_costs: Vec::new(),
            mana_as_any_type: true,
            exile_after_resolution: false,
        };
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed
            .effects
            .push(Effect::ExileCollection(ExileCollectionEffect {
                objects: ObjectRef::EachMatching(cards),
                from: Zone::Graveyard,
                cast_permission: Some(permission),
                delayed_destination: Some((Zone::Graveyard, Trigger::BeginningOfNextEndStep)),
            }));
        return Some(Ok(parsed));
    }

    if matches!(
        lower.as_str(),
        "return target nonland permanent to its owner's hand. then that permanent's controller may sacrifice a land. if that player does, they may copy this object spell and may choose a new target for that copy."
            | "return target nonland permanent to its owner's hand. then that permanent's controller may sacrifice a land of their choice. if the player does, they may copy this object spell and may choose a new target for that copy."
    ) {
        let mut permanent = ObjectFilter::with_type(CardType::Permanent);
        permanent.zones = vec![Zone::Battlefield];
        permanent.excluded_card_types.push(CardType::Land);
        let mut land = ObjectFilter::with_type(CardType::Land);
        land.zones = vec![Zone::Battlefield];
        land.controller = Some(PlayerRef::ControllerOf(Box::new(ObjectRef::Target(0))));
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(permanent),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        });
        parsed.effects.push(Effect::BounceWithControllerCopy(
            BounceWithControllerCopyEffect {
                object: ObjectRef::Target(0),
                sacrifice: ObjectSelection {
                    id: 0,
                    chooser: PlayerRef::ControllerOf(Box::new(ObjectRef::Target(0))),
                    filter: land,
                    amount: TargetAmount::Exactly(1),
                },
                copy_source: ObjectRef::Source,
                may_choose_new_targets: true,
            },
        ));
        return Some(Ok(parsed));
    }

    if let Some(subject) = lower
        .strip_prefix("whenever one or more ")
        .and_then(|text| {
            text.strip_suffix(" deal combat damage to a player, create a treasure token.")
        })
    {
        let mut source = match parse_event_object_filter(subject) {
            Some(filter) => filter,
            None => return Some(Err(unsupported(address, clause))),
        };
        source.zones = vec![Zone::Battlefield];
        let mut parsed =
            ParsedClause::new(Timing::Triggered(Box::new(Trigger::CombatDamageToPlayer {
                source,
            })));
        parsed.effects.push(Effect::CreateToken(TokenCreation {
            player: PlayerRef::You,
            amount: Amount::Constant(1),
            specification: TokenSpecification::Defined(Box::new(treasure_definition())),
            tapped: false,
            attacking: false,
        }));
        return Some(Ok(parsed));
    }

    if lower == "other creatures you control have \"ward\u{2014}pay 2 life.\""
        || lower == "other creatures you control have \"ward-pay 2 life.\""
    {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        creatures.other_than_source = true;
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::GrantKeyword {
            objects: ObjectRef::EachMatching(creatures),
            keywords: vec![Keyword::Ward(Box::new(WardCost::PayLife(
                Amount::Constant(2),
            )))],
            duration: Duration::WhileSourceOnBattlefield,
        });
        return Some(Ok(parsed));
    }

    if (lower.starts_with("when this object enters, look at the top x cards of your library,")
        || lower.starts_with(
            "when this object enters the battlefield, look at the top x cards of your library,",
        ))
        && lower.contains("where x is your devotion to blue")
        && lower.contains("you win the game")
    {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceEnters)));
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::DevotionLookAndWin {
                player: PlayerRef::You,
                color: Color::Blue,
            },
        ));
        return Some(Ok(parsed));
    }

    if lower
        == "choose one. if you control a commander as you cast this object spell, you may choose both instead."
    {
        let mut parsed = ParsedClause::new(Timing::ModalHeader {
            choices: ChoiceCount::UpTo(2),
        });
        parsed.conditions.push(Condition::CommanderControlled {
            player: PlayerRef::You,
        });
        parsed.effects.push(Effect::ChooseMode {
            count: ChoiceCount::UpTo(2),
        });
        return Some(Ok(parsed));
    }

    if lower
        == "target instant or sorcery card in your graveyard gains flashback until end of turn. the flashback cost is equal to its mana cost."
    {
        let mut filter = ObjectFilter::default();
        filter.zones = vec![Zone::Graveyard];
        filter.owner = Some(PlayerRef::You);
        filter.card_types = vec![CardType::Instant, CardType::Sorcery];
        filter.card_type_match_any = true;
        let target = Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Object(filter.clone()),
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        };
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(target);
        parsed
            .effects
            .push(Effect::GrantCastPermission(CastPermission {
                affected: PlayerRef::You,
                objects: Some(ObjectRef::Target(0)),
                filter,
                from: Zone::Graveyard,
                timing: CastTiming::Normal,
                duration: Duration::UntilEndOfTurn,
                alternative_cost: Some(AlternativeCost::PrintedManaCost),
                additional_costs: Vec::new(),
                mana_as_any_type: false,
                exile_after_resolution: true,
            }));
        return Some(Ok(parsed));
    }

    if lower
        == "{t}: add {c}. if this object has a luck counter on it, instead add one mana of any color."
    {
        let mut parsed = ParsedClause::new(Timing::Activated);
        parsed.costs.push(Cost::Tap(ObjectRef::Source));
        parsed.effects.push(Effect::Conditional {
            condition: Condition::SourceHasCounter {
                counter: CounterKind::Named("luck".to_owned()),
            },
            if_true: vec![Effect::AddMana(ManaProduction {
                player: PlayerRef::You,
                choices: any_color_choices(),
                amount: Amount::Constant(1),
                commander_identity_only: false,
                scales_with: None,
                typed: None,
            })],
            if_false: vec![Effect::AddMana(ManaProduction {
                player: PlayerRef::You,
                choices: vec![ManaChoice {
                    symbols: vec![Color::Colorless],
                }],
                amount: Amount::Constant(1),
                commander_identity_only: false,
                scales_with: None,
                typed: None,
            })],
        });
        return Some(Ok(parsed));
    }

    if lower.starts_with("each nonland card in your graveyard has escape.")
        && lower.contains("exile three other cards from your graveyard")
    {
        let mut filter = ObjectFilter::in_zone(Zone::Graveyard);
        filter.owner = Some(PlayerRef::You);
        filter.excluded_card_types.push(CardType::Land);
        let mut exile_filter = ObjectFilter::in_zone(Zone::Graveyard);
        exile_filter.owner = Some(PlayerRef::You);
        exile_filter.other_than_source = true;
        let exile = Cost::ExileSelection(ObjectSelection {
            id: 0,
            chooser: PlayerRef::You,
            filter: exile_filter,
            amount: TargetAmount::Exactly(3),
        });
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::GrantCastPermission(CastPermission {
                affected: PlayerRef::You,
                objects: None,
                filter,
                from: Zone::Graveyard,
                timing: CastTiming::Normal,
                duration: Duration::WhileSourceOnBattlefield,
                alternative_cost: Some(AlternativeCost::PrintedManaCostPlus(vec![exile])),
                additional_costs: Vec::new(),
                mana_as_any_type: false,
                exile_after_resolution: false,
            }));
        return Some(Ok(parsed));
    }

    if let Some(filter_text) = lower
        .strip_prefix("you may cast ")
        .and_then(|text| text.strip_suffix(" as though they had flash."))
    {
        let mut filter = if filter_text == "spells this turn" {
            ObjectFilter::with_type(CardType::Spell)
        } else {
            parse_card_filter_phrase(filter_text.trim_end_matches(" spells"))
                .unwrap_or_else(|| ObjectFilter::with_type(CardType::Spell))
        };
        filter.zones = vec![Zone::Hand];
        let duration = if filter_text.ends_with(" this turn") {
            Duration::ThisTurn
        } else {
            Duration::WhileSourceOnBattlefield
        };
        let mut parsed = ParsedClause::new(if duration == Duration::ThisTurn {
            Timing::SpellResolution
        } else {
            Timing::Static
        });
        parsed
            .effects
            .push(Effect::GrantCastPermission(CastPermission {
                affected: PlayerRef::You,
                objects: None,
                filter,
                from: Zone::Hand,
                timing: CastTiming::AsThoughFlash,
                duration,
                alternative_cost: None,
                additional_costs: Vec::new(),
                mana_as_any_type: false,
                exile_after_resolution: false,
            }));
        return Some(Ok(parsed));
    }

    if lower == "you may cast spells this turn as though they had flash." {
        let mut filter = ObjectFilter::with_type(CardType::Spell);
        filter.zones = vec![Zone::Hand];
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed
            .effects
            .push(Effect::GrantCastPermission(CastPermission {
                affected: PlayerRef::You,
                objects: None,
                filter,
                from: Zone::Hand,
                timing: CastTiming::AsThoughFlash,
                duration: Duration::ThisTurn,
                alternative_cost: None,
                additional_costs: Vec::new(),
                mana_as_any_type: false,
                exile_after_resolution: false,
            }));
        return Some(Ok(parsed));
    }

    if lower == "your opponents can't cast spells this turn."
        || lower == "your opponents can't cast spells during your turn."
        || lower == "your opponents can't cast noncreature spells this turn."
    {
        let mut filter = ObjectFilter::with_type(CardType::Spell);
        filter.zones = vec![Zone::Stack];
        if lower.contains("noncreature") {
            filter.excluded_card_types.push(CardType::Creature);
        }
        let during_turn_of = lower.contains("during your turn").then_some(PlayerRef::You);
        let mut parsed = ParsedClause::new(if during_turn_of.is_some() {
            Timing::Static
        } else {
            Timing::SpellResolution
        });
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotCast {
                affected: PlayerRef::Opponent,
                filter,
                duration: if during_turn_of.is_some() {
                    Duration::WhileSourceOnBattlefield
                } else {
                    Duration::ThisTurn
                },
                during_turn_of,
            }));
        return Some(Ok(parsed));
    }

    if lower == "spells you control can't be countered." {
        let mut filter = ObjectFilter::with_type(CardType::Spell);
        filter.zones = vec![Zone::Stack];
        filter.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Restriction(
            Restriction::MatchingSpellsCannotBeCountered {
                player: PlayerRef::You,
                filter,
            },
        ));
        return Some(Ok(parsed));
    }

    if lower == "this object doesn't untap during your untap step." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::DoesNotUntapDuring {
                object: ObjectRef::Source,
                step: Step::UntapStep,
            }));
        return Some(Ok(parsed));
    }

    if lower == "skip your draw step." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::SkipStep {
            player: PlayerRef::You,
            step: Step::DrawStep,
        });
        return Some(Ok(parsed));
    }

    if lower == "creatures you control can boast twice during each of your turns rather than once."
    {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.controller = Some(PlayerRef::You);
        filter.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AbilityUseLimit {
                object: ObjectRef::EachMatching(filter),
                label: "boast".to_owned(),
                uses_per_turn: 2,
            }));
        return Some(Ok(parsed));
    }

    if lower
        == "return target creature an opponent controls to its owner's hand. if the gift was promised, instead return target nonland permanent an opponent controls to its owner's hand."
    {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        creature.controller = Some(PlayerRef::Opponent);
        let mut nonland = ObjectFilter::with_type(CardType::Permanent);
        nonland.zones = vec![Zone::Battlefield];
        nonland.controller = Some(PlayerRef::Opponent);
        nonland.excluded_card_types.push(CardType::Land);
        let target = Target {
            id: 0,
            chooser: PlayerRef::You,
            filter: TargetFilter::Conditional {
                condition: Condition::GiftPromised,
                if_true: Box::new(TargetFilter::Object(nonland)),
                if_false: Box::new(TargetFilter::Object(creature)),
            },
            amount: TargetAmount::Exactly(1),
            relationship: TargetRelationship::Independent,
        };
        let mut parsed = ParsedClause::new(Timing::SpellResolution);
        parsed.targets.push(target);
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: ObjectRef::Target(0),
            from: Some(Zone::Battlefield),
            to: Zone::Hand,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        return Some(Ok(parsed));
    }

    if lower
        == "whenever you cast a noncreature spell, birds, frogs, otters, and rats you control get +1/+1 until end of turn. untap them."
    {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::You);
        filter.subtypes = vec![
            "Bird".to_owned(),
            "Frog".to_owned(),
            "Otter".to_owned(),
            "Rat".to_owned(),
        ];
        filter.subtype_match_any = true;
        let mut spell = ObjectFilter::with_type(CardType::Spell);
        spell.zones = vec![Zone::Stack];
        spell.excluded_card_types.push(CardType::Creature);
        let objects = ObjectRef::EachMatching(filter);
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::Cast {
            player: PlayerRef::You,
            spell,
        })));
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: objects.clone(),
                operation: PowerToughnessOperation::Add,
                power: Amount::Constant(1),
                toughness: Amount::Constant(1),
                duration: Duration::UntilEndOfTurn,
            }));
        parsed.effects.push(Effect::Untap { object: objects });
        return Some(Ok(parsed));
    }

    if lower == "at the beginning of your upkeep, you may pay {4}. if you do, untap this object." {
        let cost = Cost::Mana(ManaCost("{4}".to_owned()));
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::BeginningOf {
            step: Step::Upkeep,
            player: TurnPlayer::You,
        })));
        parsed.effects.push(Effect::Conditional {
            condition: Condition::PaymentAccepted(cost.clone()),
            if_true: vec![
                Effect::PayCost(cost),
                Effect::Untap {
                    object: ObjectRef::Source,
                },
            ],
            if_false: Vec::new(),
        });
        return Some(Ok(parsed));
    }

    if lower
        == "at the beginning of your draw step, if this object is tapped, it deals 1 damage to you."
    {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::BeginningOf {
            step: Step::DrawStep,
            player: TurnPlayer::You,
        })));
        parsed
            .conditions
            .push(Condition::SourceState(ObjectState::Tapped));
        parsed.effects.push(Effect::Damage {
            source: ObjectRef::Source,
            recipient: PlayerRef::You,
            amount: Amount::Constant(1),
        });
        return Some(Ok(parsed));
    }

    if lower.starts_with("at the beginning of each of your postcombat main phases,")
        && lower.contains("number of opponents that were dealt combat damage this turn")
        && lower.ends_with("if you do, draw x cards.")
    {
        let amount = Amount::Count(Box::new(CountExpression::OpponentsDealtCombatDamage {
            player: PlayerRef::You,
        }));
        let cost = Cost::PayLife(amount.clone());
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::BeginningOf {
            step: Step::PostcombatMainPhase,
            player: TurnPlayer::You,
        })));
        parsed.effects.push(Effect::Conditional {
            condition: Condition::PaymentAccepted(cost.clone()),
            if_true: vec![
                Effect::PayCost(cost),
                Effect::Draw {
                    player: PlayerRef::You,
                    amount,
                    optional: false,
                    delayed_until: None,
                },
            ],
            if_false: Vec::new(),
        });
        return Some(Ok(parsed));
    }

    if lower
        .starts_with("as long as there are four or more card types among cards in your graveyard,")
        && lower.contains("gets +2/+2")
        && lower.contains("has flying")
        && lower.contains("attacks each combat if able")
    {
        let condition = Condition::CardTypesInGraveyard {
            player: PlayerRef::You,
            comparison: Comparison::AtLeast,
            amount: Amount::Constant(4),
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.conditions.push(condition.clone());
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::Source,
                operation: PowerToughnessOperation::Add,
                power: Amount::Constant(2),
                toughness: Amount::Constant(2),
                duration: Duration::WhileCondition(Box::new(condition.clone())),
            }));
        parsed.effects.push(Effect::GrantKeyword {
            objects: ObjectRef::Source,
            keywords: vec![Keyword::Flying],
            duration: Duration::WhileCondition(Box::new(condition.clone())),
        });
        parsed.effects.push(Effect::Restriction(
            Restriction::MustAttackEachCombatIfAble {
                object: ObjectRef::Source,
                duration: Duration::WhileCondition(Box::new(condition)),
            },
        ));
        return Some(Ok(parsed));
    }

    if lower == "add {b}{b}{b}{b}{b} instead if there are seven or more cards in your graveyard." {
        let condition = Condition::GraveyardCardCount {
            player: PlayerRef::You,
            comparison: Comparison::AtLeast,
            amount: Amount::Constant(7),
        };
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.conditions.push(condition);
        parsed.effects.push(Effect::AddMana(ManaProduction {
            player: PlayerRef::You,
            choices: vec![ManaChoice {
                symbols: vec![
                    Color::Black,
                    Color::Black,
                    Color::Black,
                    Color::Black,
                    Color::Black,
                ],
            }],
            amount: Amount::Constant(5),
            commander_identity_only: false,
            scales_with: None,
            typed: None,
        }));
        return Some(Ok(parsed));
    }

    if let Some(counter_text) = lower
        .strip_prefix("this object enters with ")
        .and_then(|text| text.strip_suffix('.'))
        .and_then(|text| text.split_once(" counter on it for each "))
        && let Some((Amount::Constant(1), name)) = parse_counter_amount_and_name(counter_text.0)
        && let Some(count) = parse_count_expression(counter_text.1)
    {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::PutCounter {
            object: ObjectRef::Source,
            counter: parse_counter_kind(name),
            amount: Amount::Count(Box::new(count)),
        });
        return Some(Ok(parsed));
    }

    if let Some(counter_text) = lower
        .strip_prefix("this object enters with ")
        .and_then(|text| text.strip_suffix(" counters on it."))
        .or_else(|| {
            lower
                .strip_prefix("this object enters with ")
                .and_then(|text| text.strip_suffix(" counter on it."))
        })
        && let Some((amount, name)) = parse_counter_amount_and_name(counter_text)
    {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::PutCounter {
            object: ObjectRef::Source,
            counter: parse_counter_kind(name),
            amount,
        });
        return Some(Ok(parsed));
    }

    if let Some(cost_text) = lower
        .strip_prefix("you may pay ")
        .and_then(|text| text.strip_suffix(" rather than pay this object spell's mana cost."))
        && let Ok(cost) = parse_mana_cost(address, cost_text)
    {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AlternativeCastPermission(
                Box::new(AlternativeCastPermission {
                    object: ObjectRef::Source,
                    from: Zone::Hand,
                    cost: AlternativeCost::Mana(cost),
                    timing: Trigger::SourceCast,
                    condition: None,
                }),
            )));
        return Some(Ok(parsed));
    }

    for (prefix, condition) in [
        (
            "if it's not your turn, you may ",
            Some(Condition::NotYourTurn),
        ),
        ("you may ", None),
    ] {
        let Some(cost_text) = lower
            .strip_prefix(prefix)
            .and_then(|text| text.strip_suffix(" rather than pay this object spell's mana cost."))
        else {
            continue;
        };
        let alternative_cost = if cost_text == "exile a blue card from your hand" {
            let mut filter = ObjectFilter::in_zone(Zone::Hand);
            filter.owner = Some(PlayerRef::You);
            filter.colors.push(Color::Blue);
            AlternativeCost::Costs(vec![Cost::ExileSelection(ObjectSelection {
                id: 0,
                chooser: PlayerRef::You,
                filter,
                amount: TargetAmount::Exactly(1),
            })])
        } else if cost_text == "pay 1 life and exile a blue card from your hand" {
            let mut filter = ObjectFilter::in_zone(Zone::Hand);
            filter.owner = Some(PlayerRef::You);
            filter.colors.push(Color::Blue);
            AlternativeCost::Costs(vec![
                Cost::PayLife(Amount::Constant(1)),
                Cost::ExileSelection(ObjectSelection {
                    id: 0,
                    chooser: PlayerRef::You,
                    filter,
                    amount: TargetAmount::Exactly(1),
                }),
            ])
        } else if cost_text == "sacrifice a nontoken red creature" {
            let mut filter = ObjectFilter::with_type(CardType::Creature);
            filter.zones = vec![Zone::Battlefield];
            filter.controller = Some(PlayerRef::You);
            filter.token = Some(false);
            filter.colors.push(Color::Red);
            AlternativeCost::Costs(vec![Cost::Sacrifice {
                amount: Amount::Constant(1),
                filter,
            }])
        } else {
            continue;
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        if let Some(condition) = condition.clone() {
            parsed.conditions.push(condition);
        }
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AlternativeCastPermission(
                Box::new(AlternativeCastPermission {
                    object: ObjectRef::Source,
                    from: Zone::Hand,
                    cost: alternative_cost,
                    timing: Trigger::SourceCast,
                    condition,
                }),
            )));
        return Some(Ok(parsed));
    }

    None
}

fn parse_transform_annotation_clause(
    _address: ClauseAddress,
    clause: &str,
    _source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let inner = clause
        .strip_prefix("(Transforms from ")?
        .strip_suffix(".)")?;
    if inner.trim().is_empty() {
        return None;
    }
    let mut parsed = ParsedClause::new(Timing::SpecialAction(
        SpecialActionTiming::TransformBackFaceAnnotation,
    ));
    parsed.reminder = Some(ReminderSemantics::TransformOrigin {
        front_face_name: inner.to_string(),
    });
    Some(Ok(parsed))
}

fn parse_parenthetical_activated_clause(
    address: ClauseAddress,
    clause: &str,
    source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let inner = clause.strip_prefix('(')?.strip_suffix(')')?.trim();
    find_top_level(inner, ':')?;
    parse_activated_clause(address, inner, source_type_line)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CyclingSpecification {
    cost: ManaCost,
    kind: CyclingKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CyclingKind {
    Draw,
    Type {
        type_name: String,
        filter: ObjectFilter,
    },
}

fn parse_cycling_specification(
    address: ClauseAddress,
    clause: &str,
) -> Option<Result<CyclingSpecification, CompileError>> {
    let trimmed = clause.trim().trim_end_matches('.').trim();
    let lower = trimmed.to_ascii_lowercase();
    let cycling_index = lower.rfind("cycling ")?;
    let type_name = lower[..cycling_index].trim();
    if type_name.contains(|character: char| {
        !character.is_ascii_alphanumeric() && character != ' ' && character != '-'
    }) {
        return Some(Err(unsupported(address, clause)));
    }
    let cost_text = trimmed[cycling_index + "cycling ".len()..].trim();
    let cost = match parse_mana_cost(address, cost_text) {
        Ok(cost) => cost,
        Err(error) => return Some(Err(error)),
    };
    let kind = if type_name.is_empty() {
        CyclingKind::Draw
    } else {
        let mut filter = if type_name == "basic land" {
            let mut filter = ObjectFilter::with_type(CardType::Land);
            filter.supertypes.push(Supertype::Basic);
            filter
        } else if let Some(card_type) = parse_card_type(type_name) {
            ObjectFilter::with_type(card_type)
        } else {
            let mut filter = ObjectFilter::default();
            filter.subtypes = type_name
                .split_whitespace()
                .map(title_case)
                .collect::<Vec<_>>();
            filter
        };
        filter.zones = vec![Zone::Library];
        CyclingKind::Type {
            type_name: type_name.to_string(),
            filter,
        }
    };
    Some(Ok(CyclingSpecification { cost, kind }))
}

fn parse_keyword_clause(
    address: ClauseAddress,
    clause: &str,
    _source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let lower = clause.to_ascii_lowercase();
    if lower.trim_end_matches('.') == "flash" {
        let mut filter = ObjectFilter::in_zone(Zone::Hand);
        filter.owner = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::GrantCastPermission(CastPermission {
                affected: PlayerRef::You,
                objects: Some(ObjectRef::Source),
                filter,
                from: Zone::Hand,
                timing: CastTiming::AsThoughFlash,
                duration: Duration::Permanent,
                alternative_cost: None,
                additional_costs: Vec::new(),
                mana_as_any_type: false,
                exile_after_resolution: false,
            }));
        return Some(Ok(parsed));
    }
    if let Some(cost_text) = clause
        .trim()
        .strip_prefix("Dash ")
        .or_else(|| clause.trim().strip_prefix("dash "))
    {
        let cost = match parse_mana_cost(address, cost_text) {
            Ok(cost) => cost,
            Err(error) => return Some(Err(error)),
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AlternativeCastPermission(
                Box::new(AlternativeCastPermission {
                    object: ObjectRef::Source,
                    from: Zone::Hand,
                    cost: AlternativeCost::Mana(cost),
                    timing: Trigger::SourceCast,
                    condition: None,
                }),
            )));
        parsed.effects.push(Effect::Conditional {
            condition: Condition::CardWasCastWithAlternativeCost,
            if_true: vec![
                Effect::GrantKeyword {
                    objects: ObjectRef::Source,
                    keywords: vec![Keyword::Haste],
                    duration: Duration::Permanent,
                },
                Effect::MoveZone(ZoneMove {
                    object: ObjectRef::Source,
                    from: Some(Zone::Battlefield),
                    to: Zone::Hand,
                    tapped: false,
                    face_down: false,
                    delayed_until: Some(Trigger::BeginningOfNextEndStep),
                }),
            ],
            if_false: Vec::new(),
        });
        return Some(Ok(parsed));
    }
    if let Some(cost_text) = clause
        .trim()
        .strip_prefix("Flashback ")
        .or_else(|| clause.trim().strip_prefix("flashback "))
    {
        let cost = match parse_mana_cost(address, cost_text) {
            Ok(cost) => cost,
            Err(error) => return Some(Err(error)),
        };
        let mut filter = ObjectFilter::in_zone(Zone::Graveyard);
        filter.owner = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::GrantCastPermission(CastPermission {
                affected: PlayerRef::You,
                objects: Some(ObjectRef::Source),
                filter,
                from: Zone::Graveyard,
                timing: CastTiming::Normal,
                duration: Duration::Permanent,
                alternative_cost: Some(AlternativeCost::Mana(cost)),
                additional_costs: Vec::new(),
                mana_as_any_type: false,
                exile_after_resolution: true,
            }));
        return Some(Ok(parsed));
    }
    if lower.trim_end_matches('.') == "gift a tapped fish" {
        let token = TokenDefinition {
            name: Some("Fish".to_owned()),
            power: Some(Amount::Constant(1)),
            toughness: Some(Amount::Constant(1)),
            colors: vec![Color::Blue],
            card_types: vec![CardType::Creature],
            subtypes: vec!["Fish".to_owned()],
            keywords: Vec::new(),
            abilities: Vec::new(),
        };
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceCast)));
        parsed.effects.push(Effect::Conditional {
            condition: Condition::GiftPromised,
            if_true: vec![Effect::CreateToken(TokenCreation {
                player: PlayerRef::ThatPlayer,
                amount: Amount::Constant(1),
                specification: TokenSpecification::Defined(Box::new(token)),
                tapped: true,
                attacking: false,
            })],
            if_false: Vec::new(),
        });
        return Some(Ok(parsed));
    }
    if lower.trim_end_matches('.') == "gift a card" {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceCast)));
        parsed.effects.push(Effect::Conditional {
            condition: Condition::GiftPromised,
            if_true: vec![Effect::Draw {
                player: PlayerRef::ThatPlayer,
                amount: Amount::Constant(1),
                optional: false,
                delayed_until: None,
            }],
            if_false: Vec::new(),
        });
        return Some(Ok(parsed));
    }
    if let Some(amount_text) = lower.trim_end_matches('.').strip_prefix("mobilize ")
        && let Some(amount) = parse_english_amount(amount_text)
    {
        let token = TokenDefinition {
            name: Some("Warrior".to_owned()),
            power: Some(Amount::Constant(1)),
            toughness: Some(Amount::Constant(1)),
            colors: vec![Color::Red],
            card_types: vec![CardType::Creature],
            subtypes: vec!["Warrior".to_owned()],
            keywords: Vec::new(),
            abilities: Vec::new(),
        };
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceAttacks)));
        parsed.effects.push(Effect::CreateTokenWithDelayedMove {
            creation: TokenCreation {
                player: PlayerRef::You,
                amount,
                specification: TokenSpecification::Defined(Box::new(token)),
                tapped: true,
                attacking: true,
            },
            destination: Zone::Graveyard,
            trigger: Trigger::BeginningOfNextEndStep,
        });
        return Some(Ok(parsed));
    }
    if let Some(specification) = parse_cycling_specification(address, clause) {
        let specification = match specification {
            Ok(specification) => specification,
            Err(error) => return Some(Err(error)),
        };
        let mut parsed = ParsedClause::new(Timing::Activated);
        parsed.costs = vec![
            Cost::Mana(specification.cost.clone()),
            Cost::Discard(ObjectRef::Source),
        ];
        parsed.activation_restriction = Some(ActivationRestriction::SourceZone(Zone::Hand));
        match specification.kind {
            CyclingKind::Draw => parsed.effects.push(Effect::Draw {
                player: PlayerRef::You,
                amount: Amount::Constant(1),
                optional: false,
                delayed_until: None,
            }),
            CyclingKind::Type { filter, .. } => {
                parsed.effects.push(Effect::SearchLibrary(SearchLibrary {
                    player: PlayerRef::You,
                    chooser: PlayerRef::You,
                    optional: false,
                    allow_fail_to_find: true,
                    amount: Amount::Constant(1),
                    predicate: filter,
                    reveal: true,
                    destinations: vec![SearchDestination {
                        selected_ordinal: SearchOrdinal::Each,
                        zone: Zone::Hand,
                        tapped: false,
                    }],
                    shuffle_before_destination: false,
                    shuffle_after: true,
                }));
            }
        }
        return Some(Ok(parsed));
    }
    if lower.trim_end_matches('.') == "prowess" {
        let mut spell = ObjectFilter::in_zone(Zone::Stack);
        spell.card_types.push(CardType::Spell);
        spell.excluded_card_types.push(CardType::Creature);
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::Cast {
            player: PlayerRef::You,
            spell,
        })));
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::Source,
                operation: PowerToughnessOperation::Add,
                power: Amount::Constant(1),
                toughness: Amount::Constant(1),
                duration: Duration::UntilEndOfTurn,
            }));
        return Some(Ok(parsed));
    }
    if let Some(cost) = parse_ward_cost(address, clause) {
        let cost = match cost {
            Ok(cost) => cost,
            Err(error) => return Some(Err(error)),
        };
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::BecomesTarget {
            object: ObjectRef::Source,
            controller: PlayerRef::Opponent,
            source_kinds: Vec::new(),
        })));
        parsed.effects.push(Effect::ResolveWard {
            payer: PlayerRef::ThatPlayer,
            source: ObjectRef::TriggeringObject,
            cost: Box::new(cost),
        });
        return Some(Ok(parsed));
    }
    if lower == "enchant creature" {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::EnchantRestriction {
                filter: ObjectFilter::with_type(CardType::Creature),
            }));
        return Some(Ok(parsed));
    }

    let mut keywords = Vec::new();
    for part in lower.split(',').map(str::trim) {
        let keyword = match part {
            "deathtouch" => Keyword::Deathtouch,
            "defender" => Keyword::Defender,
            "double strike" => Keyword::DoubleStrike,
            "first strike" => Keyword::FirstStrike,
            "flying" => Keyword::Flying,
            "haste" => Keyword::Haste,
            "hexproof" => Keyword::Hexproof,
            "indestructible" => Keyword::Indestructible,
            "lifelink" => Keyword::Lifelink,
            "menace" => Keyword::Menace,
            "reach" => Keyword::Reach,
            "shroud" => Keyword::Shroud,
            "trample" => Keyword::Trample,
            "vigilance" => Keyword::Vigilance,
            value if value.starts_with("ward ") || value.starts_with("ward\u{2014}") => {
                match parse_ward_cost(address, part) {
                    Some(Ok(cost)) => Keyword::Ward(Box::new(cost)),
                    Some(Err(error)) => return Some(Err(error)),
                    None => {
                        keywords.clear();
                        break;
                    }
                }
            }
            _ => {
                keywords.clear();
                break;
            }
        };
        keywords.push(keyword);
    }
    if !keywords.is_empty() {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::GrantKeyword {
            objects: ObjectRef::Source,
            keywords,
            duration: Duration::Permanent,
        });
        return Some(Ok(parsed));
    }

    if let Some(cost_text) = clause.strip_prefix("Evoke ") {
        let cost = match parse_mana_cost(address, cost_text.trim()) {
            Ok(cost) => cost,
            Err(error) => return Some(Err(error)),
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AlternativeCastPermission(
                Box::new(AlternativeCastPermission {
                    object: ObjectRef::Source,
                    from: Zone::Hand,
                    cost: AlternativeCost::Mana(cost),
                    timing: Trigger::Cast {
                        player: PlayerRef::You,
                        spell: ObjectFilter::in_zone(Zone::Hand),
                    },
                    condition: None,
                }),
            )));
        return Some(Ok(parsed));
    }
    if lower.starts_with("crew ")
        && parse_english_amount(lower.strip_prefix("crew ").unwrap_or_default()).is_some()
    {
        let required_power =
            parse_english_amount(lower.strip_prefix("crew ").unwrap_or_default()).unwrap();
        let mut parsed = ParsedClause::new(Timing::Activated);
        parsed.costs.push(Cost::TapCreaturesWithTotalPower {
            player: PlayerRef::You,
            minimum: required_power,
        });
        parsed.effects.push(Effect::Animate(AnimateEffect {
            object: ObjectRef::Source,
            power: Amount::Constant(0),
            toughness: Amount::Constant(0),
            retain_printed_power_toughness: true,
            colors: Vec::new(),
            subtypes: Vec::new(),
            keywords: Vec::new(),
            retain_land: false,
            duration: Duration::UntilEndOfTurn,
        }));
        return Some(Ok(parsed));
    }
    if lower == "split second" {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.extend([
            Effect::Restriction(Restriction::CannotCastNonManaSpellsWhileOnStack {
                affected: PlayerRef::Any,
            }),
            Effect::Restriction(Restriction::CannotActivateNonManaAbilitiesWhileOnStack {
                affected: PlayerRef::Any,
            }),
        ]);
        return Some(Ok(parsed));
    }
    if matches!(
        lower.as_str(),
        "partner" | "friends forever" | "ready to run"
    ) || lower
        .strip_prefix("partner\u{2014}")
        .is_some_and(|label| !label.trim().is_empty())
    {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::PartnerCommanderPairing));
        return Some(Ok(parsed));
    }
    if lower == "spell commander" {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Restriction(
            Restriction::SpellCommanderEligibility {
                limited_partner: true,
            },
        ));
        return Some(Ok(parsed));
    }
    if lower == "paradigm" {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ExileSpellAfterResolution {
            object: ObjectRef::Source,
        });
        parsed.effects.push(Effect::CastCopy(CastCopyEffect {
            source: ObjectRef::Source,
            from: Zone::Exile,
            without_paying_mana_cost: true,
            timing: Trigger::BeginningOf {
                step: Step::FirstMainPhase,
                player: TurnPlayer::You,
            },
            repeat: RepeatSchedule::EachFirstMainPhase,
        }));
        return Some(Ok(parsed));
    }
    None
}

fn parse_prepared_clause(
    _address: ClauseAddress,
    clause: &str,
    _source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    if !clause.eq_ignore_ascii_case("this object enters prepared.") {
        return None;
    }
    let mut parsed = ParsedClause::new(Timing::SpecialAction(SpecialActionTiming::EntersPrepared));
    parsed
        .effects
        .push(Effect::Restriction(Restriction::PreparedCastPermission));
    Some(Ok(parsed))
}

fn parse_entry_copy_clause(
    address: ClauseAddress,
    clause: &str,
    _source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let lower = clause.to_ascii_lowercase();
    if !lower.starts_with("you may have ") && !lower.starts_with("you may have this object enter") {
        return None;
    }
    if !lower.contains(" enter as a copy of ") {
        return None;
    }
    let (before_except, exception_text) = split_ascii_case_once(clause, ", except ")
        .map_or((clause, None), |(before, exception)| {
            (before, Some(exception))
        });
    let Some((_, original_text)) = split_ascii_case_once(before_except, " enter as a copy of ")
    else {
        return Some(Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: clause.to_string(),
        }));
    };
    let filter = match parse_copy_filter(original_text.trim()) {
        Some(filter) => filter,
        None => {
            return Some(Err(CompileError::UnsupportedSyntax {
                address,
                normalized_clause: clause.to_string(),
            }));
        }
    };
    let exceptions = match exception_text {
        Some(exception_text) => {
            match parse_copy_exceptions(address, exception_text.trim_end_matches('.')) {
                Ok(exceptions) => exceptions,
                Err(error) => return Some(Err(error)),
            }
        }
        None => Vec::new(),
    };
    let mut parsed = ParsedClause::new(Timing::Replacement);
    parsed.effects.push(Effect::Replacement(Box::new(
        ReplacementEffect::EnterAsCopy(CopyEffect {
            destination: CopyDestination::SourceAsItEnters,
            original: ObjectRef::ThatObject(0),
            filter: filter.clone(),
            exceptions,
            optional: true,
        }),
    )));
    Some(Ok(parsed))
}

fn parse_copy_filter(text: &str) -> Option<ObjectFilter> {
    let lower = text.trim().to_ascii_lowercase();
    let mut filter = ObjectFilter {
        controller: if lower.contains(" you control") {
            Some(PlayerRef::You)
        } else if lower.contains(" an opponent controls") {
            Some(PlayerRef::Opponent)
        } else {
            None
        },
        ..Default::default()
    };
    if lower.contains(" that ") || lower.contains(" with ") {
        return None;
    }
    filter.zones = if lower.contains(" in a graveyard") {
        vec![Zone::Graveyard]
    } else if lower.contains(" in exile") {
        vec![Zone::Exile]
    } else {
        vec![Zone::Battlefield]
    };
    if lower.contains("artifact or creature") {
        filter.card_types = vec![CardType::Artifact, CardType::Creature];
        filter.card_type_match_any = true;
    } else if lower.contains("artifact or enchantment") {
        filter.card_types = vec![CardType::Artifact, CardType::Enchantment];
        filter.card_type_match_any = true;
    } else if lower.contains("creature or planeswalker") {
        filter.card_types = vec![CardType::Creature, CardType::Planeswalker];
        filter.card_type_match_any = true;
    } else if lower.contains("nonland permanent") {
        filter.card_types = vec![CardType::Permanent];
        filter.excluded_card_types = vec![CardType::Land];
    } else if lower.contains("artifact") {
        filter.card_types = vec![CardType::Artifact];
    } else if lower.contains("enchantment") {
        filter.card_types = vec![CardType::Enchantment];
    } else if lower.contains("land") {
        filter.card_types = vec![CardType::Land];
    } else if lower.contains("creature") {
        filter.card_types = vec![CardType::Creature];
        let words = words(&lower);
        if let Some(creature_index) = words.iter().position(|word| word == "creature")
            && let Some(subtype) = creature_index
                .checked_sub(1)
                .and_then(|index| words.get(index))
                .filter(|word| !matches!(word.as_str(), "a" | "any" | "another"))
                .and_then(|word| canonical_subtype(word))
        {
            filter.subtypes.push(subtype);
        }
    } else if lower.contains("permanent") {
        filter.card_types = vec![CardType::Permanent];
    } else {
        return None;
    }
    if lower.contains("with mana value less than or equal to the amount of mana spent") {
        filter.mana_value = Some((Comparison::AtMost, Box::new(Amount::X)));
    }
    Some(filter)
}

fn parse_copy_exceptions(
    address: ClauseAddress,
    text: &str,
) -> Result<Vec<CopyException>, CompileError> {
    if find_top_level(text, '.').is_some() {
        return Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: text.to_string(),
        });
    }
    let lower = text.to_ascii_lowercase();
    let mut exceptions = Vec::new();
    if lower == "it has this object's other abilities" {
        exceptions.push(CopyException::RetainSourceAbilities);
        return Ok(exceptions);
    }
    if lower.contains("it isn't legendary") {
        exceptions.push(CopyException::RemoveLegendary);
    }
    if lower.contains("it's an artifact in addition to its other types") {
        exceptions.push(CopyException::AddCardType(CardType::Artifact));
    }
    if lower.contains("it's an enchantment in addition to its other types") {
        exceptions.push(CopyException::AddCardType(CardType::Enchantment));
    }
    if lower.contains("it's legendary in addition to its other types") {
        exceptions.push(CopyException::AddLegendary);
    }
    if lower.contains("it's a bird in addition to its other types") {
        exceptions.push(CopyException::AddSubtype("Bird".to_owned()));
    }
    if lower.contains("it has flying") {
        exceptions.push(CopyException::AddKeyword(Keyword::Flying));
    }
    if lower.contains("additional +1/+1 counter") && lower.contains("if it's a creature") {
        exceptions.push(CopyException::AddCounterIfType {
            card_type: CardType::Creature,
            counter: CounterKind::PlusOnePlusOne,
            amount: Box::new(Amount::Constant(1)),
        });
    }
    if lower.contains("additional loyalty counter") && lower.contains("if it's a planeswalker") {
        exceptions.push(CopyException::AddCounterIfType {
            card_type: CardType::Planeswalker,
            counter: CounterKind::Loyalty,
            amount: Box::new(Amount::Constant(1)),
        });
    }
    if let Some(name_start) = lower.find("its name is ") {
        let original = &text[name_start + "its name is ".len()..];
        let name = original
            .split(',')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        if name.is_empty() {
            return Err(CompileError::UnsupportedSyntax {
                address,
                normalized_clause: text.to_string(),
            });
        }
        exceptions.push(CopyException::SetName(name));
    }
    if let Some(ability_start) = text.find("and it has \"") {
        let ability = &text[ability_start + "and it has \"".len()..];
        let Some(ability) = ability.strip_suffix('"') else {
            return Err(CompileError::UnsupportedSyntax {
                address,
                normalized_clause: text.to_string(),
            });
        };
        let Some(colon) = find_top_level(ability, ':') else {
            return Err(CompileError::UnsupportedSyntax {
                address,
                normalized_clause: text.to_string(),
            });
        };
        let costs = parse_costs(address, &ability[..colon])?;
        let effect_body = &ability[colon + 1..];
        let nested = parse_effect_body(address, effect_body, Timing::Activated)?;
        exceptions.push(CopyException::AddGrantedAbility(GrantedAbility {
            costs,
            effects: nested.effects,
        }));
    }
    let recognized = !exceptions.is_empty()
        && (lower == "it has this object's other abilities"
            || lower.contains("additional +1/+1 counter")
            || lower.contains("its name is ")
            || lower.contains("isn't legendary")
            || lower.contains("artifact in addition")
            || lower.contains("enchantment in addition")
            || lower.contains("bird in addition")
            || lower.contains("it has flying"));
    if recognized {
        Ok(exceptions)
    } else {
        Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: text.to_string(),
        })
    }
}

fn split_ascii_case_once<'a>(text: &'a str, needle: &str) -> Option<(&'a str, &'a str)> {
    let index = text
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())?;
    Some((&text[..index], &text[index + needle.len()..]))
}

fn parse_replacement_clause(
    address: ClauseAddress,
    clause: &str,
    _source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let lower = clause.to_ascii_lowercase();
    if lower == "as this object enters, choose khans or dragons." {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::ChooseNamedOption {
            options: vec!["Khans".to_owned(), "Dragons".to_owned()],
        });
        return Some(Ok(parsed));
    }
    if lower == "as this object enters, choose a card name." {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed
            .effects
            .push(Effect::ChooseCardName { nonland: false });
        return Some(Ok(parsed));
    }
    if let Some(kind) = match lower.as_str() {
        "as this object enters, choose a word." => Some(RulesTextChoiceKind::Word),
        "as this object enters, choose an artist." => Some(RulesTextChoiceKind::Artist),
        _ => None,
    } {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::ChooseRulesText { kind });
        return Some(Ok(parsed));
    }
    if lower == "if it's neither day nor night, it becomes day as this object enters." {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::EstablishDayIfUnset);
        return Some(Ok(parsed));
    }
    if lower == "this object enters tapped. as it enters, choose a color." {
        let mut parsed = enters_tapped_clause(None, None, None);
        parsed.effects.push(Effect::ChooseColor);
        return Some(Ok(parsed));
    }
    if lower
        == "if one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead."
    {
        let mut object = ObjectFilter::with_type(CardType::Creature);
        object.zones = vec![Zone::Battlefield];
        object.controller = Some(PlayerRef::You);
        let event = ReplacementEvent::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            object: Box::new(object),
        };
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed
            .conditions
            .push(Condition::EventWouldOccur(event.clone()));
        parsed.effects.push(Effect::Replacement(Box::new(
            ReplacementEffect::IncreaseEvent { event, addend: 1 },
        )));
        return Some(Ok(parsed));
    }
    if lower.starts_with("if ") && lower.contains(" would ") && lower.ends_with(" instead.") {
        if lower.contains("token") && lower.contains("twice that many") {
            let player = if lower.contains("under your control") {
                PlayerRef::You
            } else {
                PlayerRef::Any
            };
            let event = ReplacementEvent::CreateTokens { player };
            let mut parsed = ParsedClause::new(Timing::Replacement);
            parsed
                .conditions
                .push(Condition::EventWouldOccur(event.clone()));
            parsed.effects.push(Effect::Replacement(Box::new(
                ReplacementEffect::MultiplyEvent {
                    event,
                    multiplier: 2,
                },
            )));
            return Some(Ok(parsed));
        }
        if lower.contains("counter") && lower.contains("twice that many") {
            let counter = if lower.contains("+1/+1") {
                CounterKind::PlusOnePlusOne
            } else {
                CounterKind::Named("same kind as replacement event".into())
            };
            let mut object = if lower.contains("permanent you control") {
                ObjectFilter::with_type(CardType::Permanent)
            } else {
                ObjectFilter::with_type(CardType::Creature)
            };
            if lower.contains("you control") {
                object.controller = Some(PlayerRef::You);
            }
            let event = ReplacementEvent::PutCounters {
                counter,
                object: Box::new(object),
            };
            let mut parsed = ParsedClause::new(Timing::Replacement);
            parsed
                .conditions
                .push(Condition::EventWouldOccur(event.clone()));
            parsed.effects.push(Effect::Replacement(Box::new(
                ReplacementEffect::MultiplyEvent {
                    event,
                    multiplier: 2,
                },
            )));
            return Some(Ok(parsed));
        }
        return Some(Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: clause.to_string(),
        }));
    }

    if let Some(counter_text) = lower
        .strip_prefix("this object enters tapped with ")
        .and_then(|text| text.strip_suffix(" counters on it."))
        .or_else(|| {
            lower
                .strip_prefix("this object enters tapped with ")
                .and_then(|text| text.strip_suffix(" counter on it."))
        })
        && let Some((amount, name)) = parse_counter_amount_and_name(counter_text)
    {
        let mut parsed = enters_tapped_clause(None, None, None);
        parsed.effects.push(Effect::PutCounter {
            object: ObjectRef::Source,
            counter: parse_counter_kind(name),
            amount,
        });
        return Some(Ok(parsed));
    }
    if lower == "this object enters tapped." {
        return Some(Ok(enters_tapped_clause(None, None, None)));
    }
    if let Some(condition_text) = lower
        .strip_prefix("this object enters tapped unless ")
        .and_then(|text| text.strip_suffix('.'))
    {
        let condition = match parse_control_condition(condition_text) {
            Some(condition) => condition,
            None => {
                return Some(Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: clause.to_string(),
                }));
            }
        };
        return Some(Ok(enters_tapped_clause(Some(condition), None, None)));
    }
    if lower.starts_with("as this object enters, you may pay ")
        && lower.ends_with("if you don't, it enters tapped.")
    {
        let Some((cost_text, _)) = lower
            .strip_prefix("as this object enters, you may pay ")
            .and_then(|text| text.split_once(" life. if you don't,"))
        else {
            return Some(Err(CompileError::UnsupportedSyntax {
                address,
                normalized_clause: clause.to_string(),
            }));
        };
        let Some(amount) = parse_english_amount(cost_text) else {
            return Some(Err(CompileError::UnsupportedSyntax {
                address,
                normalized_clause: clause.to_string(),
            }));
        };
        return Some(Ok(enters_tapped_clause(
            None,
            Some(Cost::PayLife(amount)),
            None,
        )));
    }
    if lower.starts_with("as this object enters, you may reveal ")
        && lower.ends_with("from your hand. if you don't, this object enters tapped.")
    {
        let reveal_text = lower
            .strip_prefix("as this object enters, you may reveal ")
            .and_then(|text| {
                text.strip_suffix(" from your hand. if you don't, this object enters tapped.")
            })
            .unwrap_or_default();
        let filter = match parse_card_filter_phrase(reveal_text) {
            Some(filter) => filter,
            None => {
                return Some(Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: clause.to_string(),
                }));
            }
        };
        return Some(Ok(enters_tapped_clause(None, None, Some(filter))));
    }
    None
}

fn enters_tapped_clause(
    unless: Option<Condition>,
    optional_cost: Option<Cost>,
    optional_reveal: Option<ObjectFilter>,
) -> ParsedClause {
    let mut parsed = ParsedClause::new(Timing::Replacement);
    parsed.effects.push(Effect::Replacement(Box::new(
        ReplacementEffect::EntersTapped(Box::new(EntersTappedReplacement {
            object: ObjectRef::Source,
            when: None,
            unless,
            optional_cost,
            optional_reveal,
        })),
    )));
    parsed
}

fn parse_control_condition(text: &str) -> Option<Condition> {
    let lower = text.trim().trim_end_matches('.').to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("your opponents control ")
        && let Some((amount_text, filter_text)) = rest.split_once(" or more ")
    {
        let amount = parse_english_amount(amount_text)?;
        let mut filter = parse_card_filter_phrase(filter_text)?;
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::Opponent);
        return Some(Condition::ControlCount {
            player: PlayerRef::Opponent,
            filter,
            comparison: Comparison::AtLeast,
            amount,
        });
    }
    if let Some(rest) = lower.strip_prefix("you control ") {
        if let Some((amount_text, filter_text)) = rest.split_once(" or more ") {
            let amount = parse_english_amount(amount_text)?;
            let mut filter = parse_card_filter_phrase(filter_text)?;
            filter.zones = vec![Zone::Battlefield];
            filter.controller = Some(PlayerRef::You);
            return Some(Condition::ControlCount {
                player: PlayerRef::You,
                filter,
                comparison: Comparison::AtLeast,
                amount,
            });
        }
        if rest == "a commander" {
            let mut filter = ObjectFilter::with_type(CardType::Permanent);
            filter.zones = vec![Zone::Battlefield, Zone::Command];
            return Some(Condition::ControlAny {
                player: PlayerRef::You,
                filters: vec![filter],
            });
        }
        if let Some(filter_text) = rest.strip_prefix("a ") {
            let parts = filter_text
                .split(" or an ")
                .flat_map(|part| part.split(" or a "))
                .collect::<Vec<_>>();
            let filters = parts
                .iter()
                .map(|part| parse_card_filter_phrase(part))
                .collect::<Option<Vec<_>>>();
            if let Some(filters) = filters
                && !filters.is_empty()
            {
                return Some(Condition::ControlAny {
                    player: PlayerRef::You,
                    filters,
                });
            }
        }
    }
    None
}

fn parse_card_filter_phrase(text: &str) -> Option<ObjectFilter> {
    let trimmed = text
        .trim()
        .trim_matches(|character: char| matches!(character, '.' | ','));
    if trimmed.contains('.') || trimmed.contains(':') || trimmed.contains('"') {
        return None;
    }
    let mut lower = trimmed
        .trim_matches(|character: char| matches!(character, '.' | ','))
        .to_ascii_lowercase();

    let mut filter = ObjectFilter::default();
    if let Some(core) = lower.strip_suffix(" without flying") {
        filter.excluded_keywords.push(Keyword::Flying);
        lower = core.trim().to_owned();
    }
    if let Some(core) = lower.strip_suffix(" with a +1/+1 counter on it") {
        filter.minimum_counter = Some((CounterKind::PlusOnePlusOne, 1));
        lower = core.trim().to_owned();
    }
    for (marker, field) in [(" with mana value ", 0u8), (" with power ", 1u8)] {
        let Some((core, comparison_text)) = lower.rsplit_once(marker) else {
            continue;
        };
        let (comparison, amount) = parse_filter_comparison(comparison_text)?;
        if field == 0 {
            filter.mana_value = Some((comparison, Box::new(amount)));
        } else {
            filter.power = Some((comparison, Box::new(amount)));
        }
        lower = core.trim().to_owned();
        break;
    }
    if [
        " modified ",
        " equipped ",
        " enchanted ",
        " commander ",
        " monocolored ",
        " multicolored ",
        " with a ",
        " with an ",
        " named ",
        " that ",
    ]
    .iter()
    .any(|marker| format!(" {lower} ").contains(marker))
    {
        return None;
    }

    let lexical_lower = lower.replace("non-", "non");
    let lexemes = words(&lexical_lower)
        .into_iter()
        .map(|word| singular_filter_word(&word))
        .collect::<Vec<_>>();
    if lexemes.iter().any(|lexeme| lexeme == "historic") {
        filter.historic = true;
    }
    if lower.contains("other ") {
        filter.other_than_source = true;
    }
    if lower.contains("nontoken") {
        filter.token = Some(false);
    } else if lower.contains("token") {
        filter.token = Some(true);
    }
    if lower.contains("attacking") {
        filter.attacking = Some(true);
    }
    if lower.contains("blocking") {
        filter.blocking = Some(true);
    }
    if lower.contains("untapped") {
        filter.tapped = Some(false);
    } else if lower.contains("tapped") {
        filter.tapped = Some(true);
    }
    if lower.contains("you control") {
        filter.controller = Some(PlayerRef::You);
    } else if lower.contains("opponent controls")
        || lower.contains("an opponent controls")
        || lower.contains("opponents control")
    {
        filter.controller = Some(PlayerRef::Opponent);
    }
    if lower.contains("you own") {
        filter.owner = Some(PlayerRef::You);
    } else if lower.contains("an opponent owns") || lower.contains("opponents own") {
        filter.owner = Some(PlayerRef::Opponent);
    }
    if lower.contains("nonbasic") {
        filter.supertypes.push(Supertype::Nonbasic);
    }
    if lexemes.iter().any(|lexeme| lexeme == "basic") {
        filter.supertypes.push(Supertype::Basic);
    }
    if lexemes.iter().any(|lexeme| lexeme == "legendary") {
        filter.supertypes.push(Supertype::Legendary);
    }
    if lexemes.iter().any(|lexeme| lexeme == "snow") {
        filter.supertypes.push(Supertype::Snow);
    }
    for (word, color) in [
        ("white", Color::White),
        ("blue", Color::Blue),
        ("black", Color::Black),
        ("red", Color::Red),
        ("green", Color::Green),
        ("colorless", Color::Colorless),
    ] {
        if lexemes.iter().any(|candidate| candidate == word) {
            filter.colors.push(color);
        }
    }
    for (word, color) in [
        ("nonwhite", Color::White),
        ("nonblue", Color::Blue),
        ("nonblack", Color::Black),
        ("nonred", Color::Red),
        ("nongreen", Color::Green),
    ] {
        if lexemes.iter().any(|candidate| candidate == word) {
            filter.excluded_colors.push(color);
        }
    }
    if lower.contains("basic land type") {
        filter.card_types.push(CardType::Land);
        return Some(filter);
    }
    if lower.contains("basic land") {
        filter.supertypes.push(Supertype::Basic);
        filter.card_types.push(CardType::Land);
        return Some(filter);
    }
    if lower.contains("chosen type") {
        filter.card_types.push(CardType::Creature);
        filter.chosen_creature_type = true;
        return Some(filter);
    }
    let known_types = [
        ("artifact", CardType::Artifact),
        ("battle", CardType::Battle),
        ("creature", CardType::Creature),
        ("enchantment", CardType::Enchantment),
        ("instant", CardType::Instant),
        ("land", CardType::Land),
        ("planeswalker", CardType::Planeswalker),
        ("sorcery", CardType::Sorcery),
        ("spell", CardType::Spell),
        ("permanent", CardType::Permanent),
    ];
    for (word, card_type) in known_types {
        if lexemes.iter().any(|candidate| candidate == word) {
            filter.card_types.push(card_type);
        }
    }
    for (word, excluded) in [
        ("nonartifact", CardType::Artifact),
        ("noncreature", CardType::Creature),
        ("nonenchantment", CardType::Enchantment),
        ("nonland", CardType::Land),
        ("nonplaneswalker", CardType::Planeswalker),
    ] {
        if lexemes.iter().any(|candidate| candidate == word) {
            filter.card_types.retain(|kind| *kind != excluded);
            filter.excluded_card_types.push(excluded);
        }
    }
    if lower.contains("noncreature spell") && !filter.card_types.contains(&CardType::Spell) {
        filter.card_types.push(CardType::Spell);
    }
    if lower.contains("legendary creature") {
        filter.supertypes.push(Supertype::Legendary);
        if !filter.card_types.contains(&CardType::Creature) {
            filter.card_types.push(CardType::Creature);
        }
    }

    for lexeme in &lexemes {
        if filter_word_is_structural(lexeme) {
            continue;
        }
        if let Some(subtype) = lexeme.strip_prefix("non") {
            filter
                .excluded_subtypes
                .push(canonical_subtype(subtype.trim_start_matches('-'))?);
            continue;
        }
        filter.subtypes.push(canonical_subtype(lexeme)?);
    }

    let disjunctive = lower.contains(" or ") || lower.contains("and/or") || lower.contains(", or ");
    let type_list = disjunctive
        || (lower.contains(" and ") && lower.contains(" card") && filter.card_types.len() > 1);
    filter.card_type_match_any = type_list && filter.card_types.len() > 1;
    filter.subtype_match_any = disjunctive && filter.subtypes.len() > 1;
    filter.color_match_any = disjunctive && filter.colors.len() > 1;
    filter.card_types.sort_by_key(|kind| format!("{kind:?}"));
    filter.card_types.dedup();
    filter
        .excluded_card_types
        .sort_by_key(|kind| format!("{kind:?}"));
    filter.excluded_card_types.dedup();
    filter.subtypes.sort();
    filter.subtypes.dedup();
    filter.excluded_subtypes.sort();
    filter.excluded_subtypes.dedup();
    (!filter.card_types.is_empty()
        || !filter.subtypes.is_empty()
        || !filter.excluded_subtypes.is_empty()
        || !filter.colors.is_empty()
        || !filter.excluded_keywords.is_empty()
        || filter.minimum_counter.is_some()
        || filter.historic
        || filter.token.is_some()
        || filter.tapped.is_some()
        || filter.attacking.is_some()
        || filter.blocking.is_some())
    .then_some(filter)
}

fn parse_filter_comparison(text: &str) -> Option<(Comparison, Amount)> {
    let lower = text.trim().to_ascii_lowercase();
    for (suffix, comparison) in [
        (" or less", Comparison::AtMost),
        (" or fewer", Comparison::AtMost),
        (" or greater", Comparison::AtLeast),
        (" or more", Comparison::AtLeast),
    ] {
        if let Some(amount) = lower.strip_suffix(suffix).and_then(parse_english_amount) {
            return Some((comparison, amount));
        }
    }
    parse_english_amount(&lower).map(|amount| (Comparison::Exactly, amount))
}

fn singular_filter_word(word: &str) -> String {
    match word {
        // `words` removes a leading indefinite article; preserve the Oracle
        // adjective "another" instead of treating the remainder as a subtype.
        "nother" => "another".to_owned(),
        "artifacts" => "artifact".to_owned(),
        "battles" => "battle".to_owned(),
        "cards" => "card".to_owned(),
        "creatures" => "creature".to_owned(),
        "enchantments" => "enchantment".to_owned(),
        "instants" => "instant".to_owned(),
        "lands" => "land".to_owned(),
        "permanents" => "permanent".to_owned(),
        "planeswalkers" => "planeswalker".to_owned(),
        "sorceries" => "sorcery".to_owned(),
        "spells" => "spell".to_owned(),
        "opponents" => "opponent".to_owned(),
        other => other.to_owned(),
    }
}

fn filter_word_is_structural(word: &str) -> bool {
    matches!(
        word,
        "a" | "all"
            | "an"
            | "and"
            | "another"
            | "any"
            | "artifact"
            | "attacking"
            | "basic"
            | "battle"
            | "black"
            | "blue"
            | "card"
            | "cards"
            | "colorless"
            | "control"
            | "controls"
            | "creature"
            | "each"
            | "enchantment"
            | "green"
            | "historic"
            | "instant"
            | "land"
            | "legendary"
            | "nonartifact"
            | "nonbasic"
            | "noncreature"
            | "nonenchantment"
            | "nonland"
            | "nonplaneswalker"
            | "nonwhite"
            | "nonblue"
            | "nonblack"
            | "nonred"
            | "nongreen"
            | "nontoken"
            | "of"
            | "one"
            | "opponent"
            | "or"
            | "other"
            | "own"
            | "permanent"
            | "planeswalker"
            | "red"
            | "snow"
            | "sorcery"
            | "spell"
            | "tapped"
            | "target"
            | "the"
            | "token"
            | "two"
            | "untapped"
            | "up"
            | "with"
            | "white"
            | "you"
            | "your"
    )
}

fn title_case(text: &str) -> String {
    let mut characters = text.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

fn parse_activated_clause(
    address: ClauseAddress,
    clause: &str,
    source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let colon = find_top_level(clause, ':')?;
    let cost_text = clause[..colon].trim();
    let effect_text = clause[colon + 1..].trim();
    if cost_text.is_empty() || effect_text.is_empty() {
        return Some(Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: clause.to_string(),
        }));
    }
    let (mut costs, is_loyalty_ability) =
        match parse_loyalty_activation_cost(address, cost_text, source_type_line) {
            Some(Ok(cost)) => (vec![cost], true),
            Some(Err(error)) => return Some(Err(error)),
            None => match parse_costs(address, cost_text) {
                Ok(costs) => (costs, false),
                Err(error) => return Some(Err(error)),
            },
        };
    let (effect_text, printed_restriction) = strip_activation_restriction(effect_text);
    let mut activation_restriction = if is_loyalty_ability {
        match printed_restriction {
            None | Some(ActivationRestriction::SorceryTiming) => {
                Some(ActivationRestriction::SorceryTiming)
            }
            Some(_) => {
                return Some(Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: clause.to_owned(),
                }));
            }
        }
    } else {
        printed_restriction
    };
    let (effect_text, activation_condition) = strip_activation_condition(effect_text);
    let mut parsed = if effect_text
        .eq_ignore_ascii_case("Draw a card and reveal it. If it isn't a land card, discard it.")
    {
        let mut parsed = ParsedClause::new(Timing::Activated);
        parsed.effects.push(Effect::DrawRevealDiscardIfNonland {
            player: PlayerRef::You,
        });
        parsed
    } else {
        let mut effect_state = EffectParseState::new();
        effect_state.source_attachment_kind = [AttachmentKind::Aura, AttachmentKind::Equipment]
            .into_iter()
            .find(|kind| source_type_line_has_attachment_kind(source_type_line, *kind));
        match parse_effect_body_with_state(address, effect_text, Timing::Activated, effect_state) {
            Ok(parsed) => parsed,
            Err(error) => return Some(Err(error)),
        }
    };
    if matches!(parsed.effects.as_slice(), [Effect::SetClassLevel { .. }]) {
        let is_class = source_type_line
            .split_once('\u{2014}')
            .is_some_and(|(_, subtypes)| {
                subtypes
                    .split_whitespace()
                    .any(|subtype| subtype.eq_ignore_ascii_case("Class"))
            });
        if !is_class || activation_restriction.is_some() {
            return Some(Err(unsupported(address, clause)));
        }
        activation_restriction = Some(ActivationRestriction::SorceryTiming);
    }
    if effect_text
        .to_ascii_lowercase()
        .contains("the sacrificed creature's toughness")
    {
        let sacrifice_indices = costs
            .iter()
            .enumerate()
            .filter_map(|(index, cost)| {
                matches!(
                    cost,
                    Cost::Sacrifice {
                        amount: Amount::Constant(1),
                        filter,
                    } if filter.card_types.contains(&CardType::Creature)
                )
                .then_some(index)
            })
            .collect::<Vec<_>>();
        let [index] = sacrifice_indices.as_slice() else {
            return Some(Err(unsupported(address, clause)));
        };
        let Cost::Sacrifice { filter, .. } = &costs[*index] else {
            unreachable!("the filtered index is a sacrifice cost");
        };
        costs[*index] = Cost::SacrificeSelection(ObjectSelection {
            id: 0,
            chooser: PlayerRef::You,
            filter: filter.clone(),
            amount: TargetAmount::Exactly(1),
        });
    }
    parsed.costs = costs;
    parsed.activation_restriction = activation_restriction;
    if let Some(condition) = activation_condition {
        parsed.conditions.push(condition);
    }
    Some(Ok(parsed))
}

fn parse_loyalty_activation_cost(
    address: ClauseAddress,
    text: &str,
    source_type_line: &str,
) -> Option<Result<Cost, CompileError>> {
    let text = text.trim();
    let loyalty = if text == "0" {
        LoyaltyCost::Zero
    } else if let Some(amount) = text.strip_prefix('+') {
        let Some(amount) = parse_strict_positive_decimal(amount) else {
            return Some(Err(unsupported(address, text)));
        };
        LoyaltyCost::Add(amount)
    } else {
        let amount = text
            .strip_prefix('\u{2212}')
            .or_else(|| text.strip_prefix('-'))?;
        if amount == "X" {
            LoyaltyCost::Remove(Amount::X)
        } else {
            let Some(amount) = parse_strict_positive_decimal(amount) else {
                return Some(Err(unsupported(address, text)));
            };
            LoyaltyCost::Remove(Amount::Constant(amount))
        }
    };

    if !words(source_type_line)
        .iter()
        .any(|word| word == "planeswalker")
    {
        return Some(Err(unsupported(address, text)));
    }
    Some(Ok(Cost::Loyalty(loyalty)))
}

fn parse_strict_positive_decimal(text: &str) -> Option<u32> {
    if text.is_empty() || text.starts_with('0') || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u32>().ok().filter(|amount| *amount > 0)
}

fn strip_activation_condition(text: &str) -> (&str, Option<Condition>) {
    let lower = text.to_ascii_lowercase();
    if lower.ends_with(" activate only if you have no cards in hand.") {
        let suffix = " Activate only if you have no cards in hand.";
        let end = text.len() - suffix.len();
        return (
            text[..end].trim_end(),
            Some(Condition::HandCardCount {
                player: PlayerRef::You,
                comparison: Comparison::Exactly,
                amount: Amount::Constant(0),
            }),
        );
    }
    if lower.ends_with(
        " activate only if there are four or more card types among cards in your graveyard.",
    ) {
        let suffix =
            " Activate only if there are four or more card types among cards in your graveyard.";
        let end = text.len() - suffix.len();
        return (
            text[..end].trim_end(),
            Some(Condition::CardTypesInGraveyard {
                player: PlayerRef::You,
                comparison: Comparison::AtLeast,
                amount: Amount::Constant(4),
            }),
        );
    }
    if lower.ends_with(" activate only if there are seven or more cards in your graveyard.") {
        let suffix = " Activate only if there are seven or more cards in your graveyard.";
        let end = text.len() - suffix.len();
        return (
            text[..end].trim_end(),
            Some(Condition::GraveyardCardCount {
                player: PlayerRef::You,
                comparison: Comparison::AtLeast,
                amount: Amount::Constant(7),
            }),
        );
    }
    if let Some(counter_name) = lower
        .strip_suffix(" on it.")
        .and_then(|prefix| prefix.rsplit_once(" activate only if this object has a "))
        .filter(|(effect, _)| !effect.trim().is_empty())
        .map(|(_, counter)| counter)
        .and_then(|counter| counter.strip_suffix(" counter"))
    {
        let suffix = format!(" Activate only if this object has a {counter_name} counter on it.");
        let end = text.len().saturating_sub(suffix.len());
        return (
            text[..end].trim_end(),
            Some(Condition::SourceHasCounter {
                counter: parse_counter_kind(counter_name),
            }),
        );
    }
    if lower.ends_with(" activate only if you control an artifact.") {
        let end = text.len() - " Activate only if you control an artifact.".len();
        let mut filter = ObjectFilter::with_type(CardType::Artifact);
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::You);
        return (
            text[..end].trim_end(),
            Some(Condition::ControlCount {
                player: PlayerRef::You,
                filter,
                comparison: Comparison::AtLeast,
                amount: Amount::Constant(1),
            }),
        );
    }
    (text, None)
}

fn strip_activation_restriction(text: &str) -> (&str, Option<ActivationRestriction>) {
    let lower = text.to_ascii_lowercase();
    if lower.ends_with(" activate only as a sorcery and only once each turn.") {
        let suffix = " Activate only as a sorcery and only once each turn.";
        let end = text.len() - suffix.len();
        return (
            text[..end].trim_end(),
            Some(ActivationRestriction::All(vec![
                ActivationRestriction::SorceryTiming,
                ActivationRestriction::OnceEachTurn,
            ])),
        );
    }
    if lower.ends_with(" activate only during your turn, before attackers are declared.") {
        let suffix = " Activate only during your turn, before attackers are declared.";
        let end = text.len() - suffix.len();
        return (
            text[..end].trim_end(),
            Some(ActivationRestriction::DuringYourTurnBeforeAttackersDeclared),
        );
    }
    if lower.ends_with(" activate only during your upkeep.") {
        let suffix = " Activate only during your upkeep.";
        let end = text.len() - suffix.len();
        return (
            text[..end].trim_end(),
            Some(ActivationRestriction::DuringYourUpkeep),
        );
    }
    if lower.ends_with(" any player may activate this ability.") {
        let end = text.len() - " Any player may activate this ability.".len();
        return (
            text[..end].trim_end(),
            Some(ActivationRestriction::AnyPlayerMayActivate),
        );
    }
    if lower.ends_with(" activate no more than twice each turn.") {
        let end = text.len() - " Activate no more than twice each turn.".len();
        return (
            text[..end].trim_end(),
            Some(ActivationRestriction::TimesEachTurn(2)),
        );
    }
    for suffix in [
        " activate only once each turn.",
        " activate only once per turn.",
    ] {
        if lower.ends_with(suffix) {
            let end = text.len() - suffix.len();
            return (
                text[..end].trim_end(),
                Some(ActivationRestriction::OnceEachTurn),
            );
        }
    }
    if lower.ends_with(" activate only as an instant.") {
        let end = text.len() - " Activate only as an instant.".len();
        return (
            text[..end].trim_end(),
            Some(ActivationRestriction::InstantTiming),
        );
    }
    if lower.ends_with(" activate only as a sorcery.") {
        let end = text.len() - " Activate only as a sorcery.".len();
        return (
            text[..end].trim_end(),
            Some(ActivationRestriction::SorceryTiming),
        );
    }
    if lower.ends_with(" activate only during your turn.") {
        let end = text.len() - " Activate only during your turn.".len();
        return (
            text[..end].trim_end(),
            Some(ActivationRestriction::YourTurn),
        );
    }
    (text, None)
}

fn parse_costs(address: ClauseAddress, text: &str) -> Result<Vec<Cost>, CompileError> {
    let expanded_text;
    let text = if text.to_ascii_lowercase().ends_with(" and sacrifice it") {
        let prefix_end = text.len() - " and sacrifice it".len();
        expanded_text = format!("{}, sacrifice this object", text[..prefix_end].trim_end());
        expanded_text.as_str()
    } else {
        text
    };
    let parts = split_top_level_commas(text);
    let mut costs = Vec::new();
    let mut part_index = 0usize;
    while part_index < parts.len() {
        let part = parts[part_index];
        let trimmed = part.trim();
        let lower = trimmed.to_ascii_lowercase();
        if trimmed.starts_with('{') {
            let resource_start = part_index;
            while part_index + 1 < parts.len() && parts[part_index + 1].trim().starts_with('{') {
                part_index += 1;
            }
            let complete_resource_cost = parts[resource_start..=part_index].join(", ");
            let resource_cost =
                parse_resource_cost_expression(&complete_resource_cost).map_err(|error| {
                    invalid_mana_expression(address, &complete_resource_cost, &error)
                })?;
            costs.extend(compile_resource_cost(
                address,
                &complete_resource_cost,
                &resource_cost,
            )?);
            part_index += 1;
            continue;
        }
        if lower.starts_with("pay {") {
            costs.push(parse_payment_cost(address, trimmed["Pay ".len()..].trim())?);
            part_index += 1;
            continue;
        }
        if let Some(amount_text) = lower
            .strip_prefix("pay ")
            .and_then(|value| value.strip_suffix(" life"))
        {
            let Some(amount) = parse_english_amount(amount_text) else {
                return Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: text.to_string(),
                });
            };
            costs.push(Cost::PayLife(amount));
            part_index += 1;
            continue;
        }
        if lower == "discard this object" {
            costs.push(Cost::Discard(ObjectRef::Source));
            part_index += 1;
            continue;
        }
        if lower == "discard your hand" {
            costs.push(Cost::DiscardHand {
                player: PlayerRef::You,
            });
            part_index += 1;
            continue;
        }
        if let Some((amount_text, filter_text)) = lower
            .strip_prefix("tap ")
            .and_then(|text| text.split_once(" untapped "))
            .and_then(|(amount, filter)| {
                filter
                    .strip_suffix(" you control")
                    .map(|filter| (amount, filter))
            })
            && let Some(Amount::Constant(amount)) =
                parse_amount_prefix(amount_text).map(|value| value.0)
            && let Ok(amount) = u16::try_from(amount)
            && amount > 0
        {
            let Some(mut filter) = parse_card_filter_phrase(filter_text)
                .or_else(|| parse_simple_event_object_filter(filter_text))
            else {
                return Err(unsupported(address, text));
            };
            filter.zones = vec![Zone::Battlefield];
            filter.controller = Some(PlayerRef::You);
            filter.tapped = Some(false);
            costs.push(Cost::TapSelection(ObjectSelection {
                id: 0,
                chooser: PlayerRef::You,
                filter,
                amount: TargetAmount::Exactly(amount),
            }));
            part_index += 1;
            continue;
        }
        if let Some(filter_text) = lower
            .strip_prefix("tap an untapped ")
            .and_then(|text| text.strip_suffix(" you control"))
        {
            let Some(mut filter) = parse_card_filter_phrase(filter_text)
                .or_else(|| parse_simple_event_object_filter(filter_text))
            else {
                return Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: text.to_string(),
                });
            };
            filter.zones = vec![Zone::Battlefield];
            filter.controller = Some(PlayerRef::You);
            filter.tapped = Some(false);
            costs.push(Cost::TapSelection(ObjectSelection {
                id: 0,
                chooser: PlayerRef::You,
                filter,
                amount: TargetAmount::Exactly(1),
            }));
            part_index += 1;
            continue;
        }
        if lower == "discard a card at random" {
            costs.push(Cost::DiscardRandom {
                player: PlayerRef::You,
            });
            part_index += 1;
            continue;
        }
        if lower == "discard a card" {
            if costs
                .iter()
                .any(|cost| matches!(cost, Cost::DiscardSelection(_)))
            {
                return Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: text.to_string(),
                });
            }
            let mut filter = ObjectFilter::in_zone(Zone::Hand);
            filter.owner = Some(PlayerRef::You);
            costs.push(Cost::DiscardSelection(ObjectSelection {
                id: 0,
                chooser: PlayerRef::You,
                filter,
                amount: TargetAmount::Exactly(1),
            }));
            part_index += 1;
            continue;
        }
        if let Some(filter) = parse_single_filtered_discard_cost(&lower) {
            if costs
                .iter()
                .any(|cost| matches!(cost, Cost::DiscardSelection(_)))
            {
                return Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: text.to_string(),
                });
            }
            costs.push(Cost::DiscardSelection(ObjectSelection {
                id: 0,
                chooser: PlayerRef::You,
                filter,
                amount: TargetAmount::Exactly(1),
            }));
            part_index += 1;
            continue;
        }
        if lower == "exile this object" {
            costs.push(Cost::ExileSourceFromBattlefield);
            part_index += 1;
            continue;
        }
        if lower == "exile this object from your hand" {
            costs.push(Cost::ExileObject(ObjectRef::Source));
            part_index += 1;
            continue;
        }
        if lower == "exile this object from your graveyard" {
            costs.push(Cost::ExileSourceFromOwnGraveyard);
            part_index += 1;
            continue;
        }
        if let Some(filter_text) = lower.strip_prefix("exile ") {
            let (amount, filter_text) =
                parse_amount_prefix(filter_text).unwrap_or((Amount::Constant(1), filter_text));
            let Some(mut filter) = parse_card_filter_phrase(filter_text) else {
                return Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: text.to_string(),
                });
            };
            if filter.controller.is_none() {
                return Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: text.to_string(),
                });
            }
            filter.zones = vec![Zone::Battlefield];
            costs.push(Cost::ExileSelection(ObjectSelection {
                id: 0,
                chooser: PlayerRef::You,
                filter,
                amount: match amount {
                    Amount::Constant(value) => TargetAmount::Exactly(
                        u16::try_from(value)
                            .ok()
                            .filter(|value| *value > 0)
                            .ok_or_else(|| unsupported(address, text))?,
                    ),
                    _ => return Err(unsupported(address, text)),
                },
            }));
            part_index += 1;
            continue;
        }
        if let Some((counter, amount)) = parse_fixed_source_counter_removal_cost(&lower) {
            costs.push(Cost::RemoveCounter {
                object: ObjectRef::Source,
                counter,
                amount: Amount::Constant(amount),
            });
            part_index += 1;
            continue;
        }
        if lower == "put a -1/-1 counter on this object" {
            costs.push(Cost::PutCounter {
                object: ObjectRef::Source,
                counter: CounterKind::MinusOneMinusOne,
                amount: Amount::Constant(1),
            });
            part_index += 1;
            continue;
        }
        if let Some(filter_text) = lower
            .strip_prefix("return ")
            .and_then(|text| text.strip_suffix(" to its owner's hand"))
        {
            let (amount, filter_text) =
                parse_amount_prefix(filter_text).unwrap_or((Amount::Constant(1), filter_text));
            let Amount::Constant(amount) = amount else {
                return Err(unsupported(address, text));
            };
            let Some(mut filter) = parse_card_filter_phrase(filter_text)
                .or_else(|| parse_simple_event_object_filter(filter_text))
            else {
                return Err(unsupported(address, text));
            };
            if filter.controller != Some(PlayerRef::You) || amount == 0 {
                return Err(unsupported(address, text));
            }
            filter.zones = vec![Zone::Battlefield];
            costs.push(Cost::ReturnSelectionToHand(ObjectSelection {
                id: 0,
                chooser: PlayerRef::You,
                filter,
                amount: TargetAmount::Exactly(
                    u16::try_from(amount).map_err(|_| unsupported(address, text))?,
                ),
            }));
            part_index += 1;
            continue;
        }
        if let Some(filter_text) = lower.strip_prefix("sacrifice ") {
            if filter_text == "this object" {
                costs.push(Cost::SacrificeObject(ObjectRef::Source));
                part_index += 1;
                continue;
            }
            let (amount, filter_text) =
                parse_amount_prefix(filter_text).unwrap_or((Amount::Constant(1), filter_text));
            let Some(mut filter) = parse_card_filter_phrase(filter_text)
                .or_else(|| parse_simple_event_object_filter(filter_text))
            else {
                return Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: text.to_string(),
                });
            };
            filter.zones = vec![Zone::Battlefield];
            costs.push(Cost::Sacrifice { amount, filter });
            part_index += 1;
            continue;
        }
        return Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: text.to_string(),
        });
    }
    if costs.is_empty() {
        return Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: text.to_string(),
        });
    }
    Ok(costs)
}

fn parse_single_filtered_discard_cost(text: &str) -> Option<ObjectFilter> {
    let mut filter = ObjectFilter::in_zone(Zone::Hand);
    filter.owner = Some(PlayerRef::You);
    match text {
        "discard a creature card" => filter.card_types.push(CardType::Creature),
        "discard a land card" => filter.card_types.push(CardType::Land),
        "discard a legendary card" => filter.supertypes.push(Supertype::Legendary),
        "discard a historic card" => filter.historic = true,
        "discard a black card" => filter.colors.push(Color::Black),
        "discard a nonblack card" => filter.excluded_colors.push(Color::Black),
        _ => return None,
    }
    Some(filter)
}

fn parse_fixed_source_counter_removal_cost(text: &str) -> Option<(CounterKind, u32)> {
    let body = text
        .strip_prefix("remove ")?
        .strip_suffix(" from this object")?;
    let (counter_expression, plural) = if let Some(expression) = body.strip_suffix(" counters") {
        (expression, true)
    } else {
        (body.strip_suffix(" counter")?, false)
    };
    let (amount, counter_name) = parse_counter_amount_and_name(counter_expression)?;
    let Amount::Constant(amount) = amount else {
        return None;
    };
    if amount == 0 || plural == (amount == 1) {
        return None;
    }
    if counter_name
        .split_ascii_whitespace()
        .any(|word| matches!(word, "and" | "or"))
    {
        return None;
    }
    Some((parse_counter_kind(counter_name), amount))
}

fn compile_resource_cost(
    address: ClauseAddress,
    original: &str,
    expression: &ResourceCostExpression,
) -> Result<Vec<Cost>, CompileError> {
    if let Some(cost) = compile_atomic_special_resource_cost(address, original, expression)? {
        return Ok(vec![Cost::AtomicResource(cost)]);
    }

    let mut costs = Vec::new();
    for component in &expression.components {
        match component {
            TypedResourceCostComponent::Mana(mana) => {
                costs.push(Cost::Mana(canonical_executable_mana_cost(
                    address, original, mana,
                )?));
            }
            TypedResourceCostComponent::TapSource => costs.push(Cost::Tap(ObjectRef::Source)),
            TypedResourceCostComponent::UntapSource => costs.push(Cost::Untap(ObjectRef::Source)),
            TypedResourceCostComponent::Energy(amount) => {
                return Err(invalid_mana_detail(
                    address,
                    original,
                    &format!(
                        "the {amount}-energy payment requires a tracked energy-counter balance"
                    ),
                ));
            }
            TypedResourceCostComponent::Tickets(amount) => {
                return Err(invalid_mana_detail(
                    address,
                    original,
                    &format!(
                        "the {amount}-ticket payment requires a tracked ticket-counter balance"
                    ),
                ));
            }
        }
    }
    if costs.is_empty() {
        return Err(invalid_mana_detail(
            address,
            original,
            "the resource payment has no executable cost components",
        ));
    }
    Ok(costs)
}

fn compile_atomic_special_resource_cost(
    address: ClauseAddress,
    original: &str,
    expression: &ResourceCostExpression,
) -> Result<Option<AtomicResourceCost>, CompileError> {
    let mut symbols = Vec::new();
    let mut exact_special_symbols = String::new();
    for component in &expression.components {
        match component {
            TypedResourceCostComponent::Mana(mana) => {
                for symbol in &mana.symbols {
                    if matches!(symbol, TypedManaSymbol::Snow) {
                        symbols.push(SpecialCostSymbol::Snow);
                        exact_special_symbols.push_str("{S}");
                    }
                }
            }
            TypedResourceCostComponent::Energy(amount) => {
                if *amount == 0 {
                    return Err(invalid_mana_detail(
                        address,
                        original,
                        "an energy payment must contain at least one energy symbol",
                    ));
                }
                for _ in 0..*amount {
                    symbols.push(SpecialCostSymbol::Energy);
                    exact_special_symbols.push_str("{E}");
                }
            }
            TypedResourceCostComponent::Tickets(amount) => {
                return Err(invalid_mana_detail(
                    address,
                    original,
                    &format!(
                        "the {amount}-ticket payment requires a separately typed ticket procedure"
                    ),
                ));
            }
            TypedResourceCostComponent::TapSource | TypedResourceCostComponent::UntapSource => {}
        }
    }
    if symbols.is_empty() {
        return Ok(None);
    }
    let special = SpecialResourceCost::from_compiled_symbols(exact_special_symbols, symbols)
        .map_err(|error| {
            invalid_mana_detail(
                address,
                original,
                &format!("the special resource boundary is invalid: {error}"),
            )
        })?;
    Ok(Some(AtomicResourceCost {
        expression: expression.clone(),
        special,
    }))
}

fn atomic_resource_cost_contains(cost: &AtomicResourceCost, symbol: SpecialCostSymbol) -> bool {
    cost.special.symbols().contains(&symbol)
}

fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0u16;
    let mut quoted = false;
    for (index, character) in text.char_indices() {
        match character {
            '"' => quoted = !quoted,
            '(' if !quoted => depth = depth.saturating_add(1),
            ')' if !quoted => depth = depth.saturating_sub(1),
            ',' if !quoted && depth == 0 => {
                parts.push(text[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

fn parse_amount_prefix(text: &str) -> Option<(Amount, &str)> {
    for prefix in [
        "one or more",
        "any number of",
        "four",
        "three",
        "two",
        "one",
        "a",
        "an",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            let amount = if prefix == "any number of" {
                Amount::Any
            } else {
                parse_english_amount(prefix)?
            };
            return Some((amount, rest.trim()));
        }
    }
    None
}

fn parse_exert_attack_clause(
    address: ClauseAddress,
    clause: &str,
    _source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    const PREFIX: &str = "you may exert this object as it attacks.";
    let lower = clause.to_ascii_lowercase();
    let rest = lower.strip_prefix(PREFIX)?.trim();
    let mut optional_effects = vec![Effect::Exert {
        object: ObjectRef::Source,
    }];
    let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceAttacks)));
    if !rest.is_empty() {
        let dependent = rest
            .strip_prefix("when you do, ")
            .ok_or_else(|| unsupported(address, clause));
        let dependent = match dependent {
            Ok(dependent) => dependent,
            Err(error) => return Some(Err(error)),
        };
        let mut state = EffectParseState::new();
        state.last_object = Some(ObjectRef::Source);
        let nested = match parse_effect_body_with_state(
            address,
            dependent,
            Timing::SpellResolution,
            state,
        ) {
            Ok(nested) => nested,
            Err(error) => return Some(Err(error)),
        };
        if !nested.costs.is_empty()
            || !nested.conditions.is_empty()
            || nested.activation_restriction.is_some()
            || nested.effects.is_empty()
        {
            return Some(Err(unsupported(address, clause)));
        }
        parsed.targets = nested.targets;
        parsed.predefined_token_creations = nested.predefined_token_creations;
        optional_effects.extend(nested.effects);
    }
    parsed.effects.push(Effect::Optional(optional_effects));
    Some(Ok(parsed))
}

fn parse_triggered_clause(
    address: ClauseAddress,
    clause: &str,
    source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let lower = clause.to_ascii_lowercase();
    if lower == "when this object enters, tap all other creatures." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.other_than_source = true;
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::SourceEnters)));
        parsed.effects.push(Effect::Tap {
            object: ObjectRef::EachMatching(creatures),
        });
        return Some(Ok(parsed));
    }
    if lower
        == "whenever this object becomes blocked by a creature, this object gets +1/+1 until end of turn."
    {
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(Trigger::ObjectEvent {
            subject: TriggerSubject::Source,
            event: ObjectEventKind::BecomesBlocked,
        })));
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::Source,
                operation: PowerToughnessOperation::Add,
                power: Amount::Constant(1),
                toughness: Amount::Constant(1),
                duration: Duration::UntilEndOfTurn,
            }));
        return Some(Ok(parsed));
    }
    if lower == "when enchanted land becomes tapped, destroy it." {
        let enchanted = ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Aura,
        };
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(
            Trigger::AttachmentTargetEvent {
                kind: AttachmentKind::Aura,
                event: ObjectEventKind::BecomesTapped,
            },
        )));
        parsed.effects.push(Effect::Destroy { object: enchanted });
        return Some(Ok(parsed));
    }
    if lower == "whenever enchanted land becomes tapped, its controller loses 2 life." {
        let enchanted = ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Aura,
        };
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(
            Trigger::AttachmentTargetEvent {
                kind: AttachmentKind::Aura,
                event: ObjectEventKind::BecomesTapped,
            },
        )));
        parsed.effects.push(Effect::LoseLife {
            player: PlayerRef::ControllerOf(Box::new(enchanted)),
            amount: Amount::Constant(2),
        });
        return Some(Ok(parsed));
    }
    if lower
        == "whenever enchanted creature attacks, put a +1/+1 counter on it. then if it has three or more +1/+1 counters on it, sacrifice this object."
    {
        let enchanted = ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Aura,
        };
        let mut parsed = ParsedClause::new(Timing::Triggered(Box::new(
            Trigger::AttachmentTargetEvent {
                kind: AttachmentKind::Aura,
                event: ObjectEventKind::Attacks,
            },
        )));
        parsed.effects.extend([
            Effect::PutCounter {
                object: enchanted.clone(),
                counter: CounterKind::PlusOnePlusOne,
                amount: Amount::Constant(1),
            },
            Effect::Conditional {
                condition: Condition::ObjectCounterCount {
                    object: enchanted,
                    counter: CounterKind::PlusOnePlusOne,
                    comparison: Comparison::AtLeast,
                    amount: 3,
                },
                if_true: vec![Effect::MoveZone(ZoneMove {
                    object: ObjectRef::Source,
                    from: Some(Zone::Battlefield),
                    to: Zone::Graveyard,
                    tapped: false,
                    face_down: false,
                    delayed_until: None,
                })],
                if_false: Vec::new(),
            },
        ]);
        return Some(Ok(parsed));
    }
    if !starts_trigger(&lower) {
        return None;
    }
    let Some(comma) = find_top_level(clause, ',') else {
        return Some(Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: clause.to_string(),
        }));
    };
    let trigger_text = clause[..comma].trim();
    let mut body = clause[comma + 1..].trim();
    let once_per_turn = body
        .to_ascii_lowercase()
        .ends_with(" this ability triggers only once each turn.");
    if once_per_turn {
        let end = body.len() - " This ability triggers only once each turn.".len();
        body = body[..end].trim_end();
    }
    let trigger = match parse_trigger(trigger_text) {
        Some(trigger) => trigger,
        None => {
            return Some(Err(CompileError::UnsupportedSyntax {
                address,
                normalized_clause: clause.to_string(),
            }));
        }
    };
    let mut intervening_condition = None;
    if body.to_ascii_lowercase().starts_with("if ")
        && let Some(condition_comma) = find_top_level(body, ',')
    {
        let condition_text = body[..condition_comma].trim();
        intervening_condition = parse_condition(condition_text);
        if intervening_condition.is_none() {
            return Some(Err(CompileError::UnsupportedSyntax {
                address,
                normalized_clause: clause.to_string(),
            }));
        }
        body = body[condition_comma + 1..].trim();
    }
    let trigger = if once_per_turn {
        Trigger::OncePerTurn(Box::new(trigger))
    } else {
        trigger
    };
    let mut effect_state = EffectParseState::new();
    effect_state.last_object = initial_effect_object_for_trigger(&trigger);
    effect_state.last_player = initial_effect_player_for_trigger(&trigger);
    let source_is_equipment =
        source_type_line
            .split_once('\u{2014}')
            .is_some_and(|(_, subtypes)| {
                subtypes
                    .split_whitespace()
                    .any(|subtype| subtype.eq_ignore_ascii_case("Equipment"))
            });
    if source_type_line_has_attachment_kind(source_type_line, AttachmentKind::Aura) {
        effect_state.source_attachment_kind = Some(AttachmentKind::Aura);
    } else if source_is_equipment || trigger_text.to_ascii_lowercase().contains("this equipment") {
        effect_state.source_attachment_kind = Some(AttachmentKind::Equipment);
    }
    if trigger_text.eq_ignore_ascii_case(
        "whenever this object becomes the target of a spell or ability you control for the first time each turn",
    ) {
        effect_state.last_object = Some(ObjectRef::Source);
    }
    if matches!(&trigger, Trigger::DamageToPlayer { .. }) {
        effect_state.last_object = Some(ObjectRef::TriggeringObject);
    }
    let mut parsed = match parse_effect_body_with_state(
        address,
        body,
        Timing::Triggered(Box::new(trigger)),
        effect_state,
    ) {
        Ok(parsed) => parsed,
        Err(error) => return Some(Err(error)),
    };
    if let Some(condition) = intervening_condition {
        parsed.conditions.push(condition);
    }
    Some(Ok(parsed))
}

fn initial_effect_object_for_trigger(trigger: &Trigger) -> Option<ObjectRef> {
    match trigger {
        Trigger::OncePerTurn(inner) => initial_effect_object_for_trigger(inner),
        Trigger::AnyOf(triggers) => {
            let mut objects = triggers.iter().map(initial_effect_object_for_trigger);
            let first = objects.next()??;
            objects
                .all(|object| object.as_ref() == Some(&first))
                .then_some(first)
        }
        Trigger::SourceEnters
        | Trigger::SourceCast
        | Trigger::SourceAttacks
        | Trigger::SourceCombatDamageToPlayer
        | Trigger::SourceDamageToPlayer
        | Trigger::SourceCombatDamageToObject { .. } => Some(ObjectRef::Source),
        Trigger::ObjectEnters(_)
        | Trigger::ObjectAttacks(_)
        | Trigger::CombatDamageToPlayer { .. }
        | Trigger::DamageToPlayer { .. }
        | Trigger::Cast { .. } => Some(ObjectRef::TriggeringObject),
        Trigger::ObjectTappedForMana { object } | Trigger::BecomesTarget { object, .. } => {
            Some(object.clone())
        }
        Trigger::AttachmentTargetEvent { kind, .. } => {
            Some(ObjectRef::AttachmentTarget { kind: *kind })
        }
        Trigger::AttachmentTargetCombatDamageToPlayer { kind } => {
            Some(ObjectRef::AttachmentTarget { kind: *kind })
        }
        Trigger::ObjectEvent { subject, .. } => Some(match subject {
            TriggerSubject::Source => ObjectRef::Source,
            TriggerSubject::Matching(_) => ObjectRef::TriggeringObject,
        }),
        Trigger::PlayerAction {
            subject: Some(subject),
            ..
        } => Some(match subject {
            TriggerSubject::Source => ObjectRef::Source,
            TriggerSubject::Matching(_) => ObjectRef::TriggeringObject,
        }),
        Trigger::SourceBlocks { .. }
        | Trigger::LifeGained { .. }
        | Trigger::TokenCreated { .. }
        | Trigger::PlayerAction { subject: None, .. }
        | Trigger::NthSpellCast { .. }
        | Trigger::CardDrawn { .. }
        | Trigger::BeginningOf { .. }
        | Trigger::BeginningOfNextEndStep
        | Trigger::SagaChapterReached { .. }
        | Trigger::SchemeSetInMotion => None,
    }
}

fn initial_effect_player_for_trigger(trigger: &Trigger) -> Option<PlayerRef> {
    match trigger {
        Trigger::OncePerTurn(inner) => initial_effect_player_for_trigger(inner),
        Trigger::AnyOf(triggers) => {
            let mut players = triggers.iter().map(initial_effect_player_for_trigger);
            let first = players.next()??;
            players
                .all(|player| player.as_ref() == Some(&first))
                .then_some(first)
        }
        Trigger::LifeGained { .. }
        | Trigger::TokenCreated { .. }
        | Trigger::PlayerAction { .. }
        | Trigger::Cast { .. }
        | Trigger::NthSpellCast { .. }
        | Trigger::CardDrawn { .. }
        | Trigger::CombatDamageToPlayer { .. }
        | Trigger::AttachmentTargetCombatDamageToPlayer { .. }
        | Trigger::DamageToPlayer { .. }
        | Trigger::SourceCombatDamageToPlayer
        | Trigger::SourceDamageToPlayer
        | Trigger::BecomesTarget { .. } => Some(PlayerRef::ThatPlayer),
        Trigger::SourceEnters
        | Trigger::SourceCast
        | Trigger::SchemeSetInMotion
        | Trigger::ObjectEnters(_)
        | Trigger::ObjectAttacks(_)
        | Trigger::ObjectTappedForMana { .. }
        | Trigger::AttachmentTargetEvent { .. }
        | Trigger::SourceAttacks
        | Trigger::SourceBlocks { .. }
        | Trigger::ObjectEvent { .. }
        | Trigger::SourceCombatDamageToObject { .. }
        | Trigger::BeginningOf { .. }
        | Trigger::BeginningOfNextEndStep
        | Trigger::SagaChapterReached { .. } => None,
    }
}

fn parse_trigger(text: &str) -> Option<Trigger> {
    let lower = text.trim().to_ascii_lowercase();
    if lower.starts_with("when ") || lower.starts_with("whenever ") {
        let event = lower
            .strip_prefix("when ")
            .or_else(|| lower.strip_prefix("whenever "))?;
        if event == "you set this scheme in motion" {
            return Some(Trigger::SchemeSetInMotion);
        }
        if let Some(trigger) = parse_cast_trigger_event(event) {
            return Some(trigger);
        }
        if let Some(trigger) = parse_card_draw_trigger_event(event) {
            return Some(trigger);
        }
        if let Some(trigger) = parse_player_action_trigger_event(event) {
            return Some(trigger);
        }
        if event == "you gain life" {
            return Some(Trigger::LifeGained {
                player: PlayerRef::You,
            });
        }
        if event == "an opponent gains life" {
            return Some(Trigger::LifeGained {
                player: PlayerRef::Opponent,
            });
        }
        if event == "you create a token" {
            return Some(Trigger::TokenCreated {
                player: PlayerRef::You,
            });
        }
        if matches!(event, "this object enters" | "this equipment enters") {
            return Some(Trigger::SourceEnters);
        }
        if event == "this equipment is turned face up" {
            return Some(Trigger::ObjectEvent {
                subject: TriggerSubject::Source,
                event: ObjectEventKind::TurnedFaceUp,
            });
        }
        if event == "this object attacks" {
            return Some(Trigger::SourceAttacks);
        }
        if event == "this object enters or attacks" {
            return Some(Trigger::AnyOf(vec![
                Trigger::SourceEnters,
                Trigger::SourceAttacks,
            ]));
        }
        if event == "this object attacks or blocks" {
            return Some(Trigger::AnyOf(vec![
                Trigger::SourceAttacks,
                Trigger::ObjectEvent {
                    subject: TriggerSubject::Source,
                    event: ObjectEventKind::Blocks,
                },
            ]));
        }
        if let Some(subject) = event.strip_prefix("this object blocks ") {
            return Some(Trigger::SourceBlocks {
                object: parse_event_object_filter(subject)?,
            });
        }
        if event
            == "this object becomes the target of a spell or ability you control for the first time each turn"
        {
            return Some(Trigger::OncePerTurn(Box::new(Trigger::BecomesTarget {
                object: ObjectRef::Source,
                controller: PlayerRef::You,
                source_kinds: Vec::new(),
            })));
        }
        if event
            == "this object becomes the target of a spell or ability for the first time each turn"
        {
            return Some(Trigger::OncePerTurn(Box::new(Trigger::BecomesTarget {
                object: ObjectRef::Source,
                controller: PlayerRef::Any,
                source_kinds: Vec::new(),
            })));
        }
        if event == "this object enters or dies" {
            return Some(Trigger::AnyOf(vec![
                Trigger::SourceEnters,
                Trigger::ObjectEvent {
                    subject: TriggerSubject::Source,
                    event: ObjectEventKind::Dies,
                },
            ]));
        }
        if event == "this object enters or leaves the battlefield" {
            return Some(Trigger::AnyOf(vec![
                Trigger::SourceEnters,
                Trigger::ObjectEvent {
                    subject: TriggerSubject::Source,
                    event: ObjectEventKind::LeavesBattlefield,
                },
            ]));
        }
        if let Some((source_event, other_subject)) =
            parse_source_or_other_object_event(event, " enters", Trigger::SourceEnters)
        {
            return Some(Trigger::AnyOf(vec![
                source_event,
                Trigger::ObjectEnters(other_subject),
            ]));
        }
        if let Some((source_event, other_subject)) = parse_source_or_other_object_event(
            event,
            " dies",
            Trigger::ObjectEvent {
                subject: TriggerSubject::Source,
                event: ObjectEventKind::Dies,
            },
        ) {
            return Some(Trigger::AnyOf(vec![
                source_event,
                Trigger::ObjectEvent {
                    subject: TriggerSubject::Matching(other_subject),
                    event: ObjectEventKind::Dies,
                },
            ]));
        }
        if let Some(subject) = event.strip_suffix(" enters or attacks") {
            let filter = parse_event_object_filter(subject)?;
            return Some(Trigger::AnyOf(vec![
                Trigger::ObjectEnters(filter.clone()),
                Trigger::ObjectAttacks(filter),
            ]));
        }
        if let Some(subject) = event.strip_suffix(" enters") {
            return Some(Trigger::ObjectEnters(parse_event_object_filter(subject)?));
        }
        for (suffix, object_event) in [
            (
                " is put into a graveyard from anywhere",
                ObjectEventKind::PutIntoGraveyardFromAnywhere,
            ),
            (
                " is put into a graveyard from the battlefield",
                ObjectEventKind::PutIntoGraveyardFromBattlefield,
            ),
            (
                " leaves the battlefield",
                ObjectEventKind::LeavesBattlefield,
            ),
            (" is turned face up", ObjectEventKind::TurnedFaceUp),
            (" becomes tapped", ObjectEventKind::BecomesTapped),
            (" becomes blocked", ObjectEventKind::BecomesBlocked),
            (" blocks", ObjectEventKind::Blocks),
            (" is dealt damage", ObjectEventKind::DealtDamage),
            (" mutates", ObjectEventKind::Mutates),
            (" dies", ObjectEventKind::Dies),
        ] {
            if let Some(subject) = event.strip_suffix(suffix) {
                let subject = parse_trigger_subject(subject)?;
                return Some(Trigger::ObjectEvent {
                    subject,
                    event: object_event,
                });
            }
        }
        if event == "this object deals combat damage to a player" {
            return Some(Trigger::SourceCombatDamageToPlayer);
        }
        if event == "equipped creature deals combat damage to a player" {
            return Some(Trigger::AttachmentTargetCombatDamageToPlayer {
                kind: AttachmentKind::Equipment,
            });
        }
        if event == "enchanted creature deals combat damage to a player" {
            return Some(Trigger::AttachmentTargetCombatDamageToPlayer {
                kind: AttachmentKind::Aura,
            });
        }
        if let Some(subject) = event.strip_suffix(" deals damage to you") {
            return Some(Trigger::DamageToPlayer {
                source: parse_event_object_filter(subject)?,
                player: PlayerRef::You,
            });
        }
        if let Some(subject) = event.strip_suffix(" deals combat damage to a player") {
            let subject = subject.strip_prefix("one or more ").unwrap_or(subject);
            return Some(Trigger::CombatDamageToPlayer {
                source: parse_event_object_filter(subject)?,
            });
        }
    }
    if let Some(step_text) = lower.strip_prefix("at the beginning of ") {
        return match step_text {
            "each upkeep" => Some(Trigger::BeginningOf {
                step: Step::Upkeep,
                player: TurnPlayer::EachPlayer,
            }),
            "your upkeep" => Some(Trigger::BeginningOf {
                step: Step::Upkeep,
                player: TurnPlayer::You,
            }),
            "your draw step" => Some(Trigger::BeginningOf {
                step: Step::DrawStep,
                player: TurnPlayer::You,
            }),
            "your first main phase" => Some(Trigger::BeginningOf {
                step: Step::FirstMainPhase,
                player: TurnPlayer::You,
            }),
            "each player's first main phase" => Some(Trigger::BeginningOf {
                step: Step::FirstMainPhase,
                player: TurnPlayer::EachPlayer,
            }),
            "each of your postcombat main phases" | "your postcombat main phase" => {
                Some(Trigger::BeginningOf {
                    step: Step::PostcombatMainPhase,
                    player: TurnPlayer::You,
                })
            }
            "your next upkeep" => Some(Trigger::BeginningOf {
                step: Step::Upkeep,
                player: TurnPlayer::NextTurn,
            }),
            "each combat" => Some(Trigger::BeginningOf {
                step: Step::Combat,
                player: TurnPlayer::EachPlayer,
            }),
            "combat on your turn" | "your combat" => Some(Trigger::BeginningOf {
                step: Step::Combat,
                player: TurnPlayer::You,
            }),
            "your end step" => Some(Trigger::BeginningOf {
                step: Step::EndStep,
                player: TurnPlayer::You,
            }),
            "the end step" | "each end step" | "each player's end step" => {
                Some(Trigger::BeginningOf {
                    step: Step::EndStep,
                    player: TurnPlayer::EachPlayer,
                })
            }
            _ => None,
        };
    }
    None
}

fn parse_source_or_other_object_event(
    event: &str,
    suffix: &str,
    source_event: Trigger,
) -> Option<(Trigger, ObjectFilter)> {
    let subject = event.strip_suffix(suffix)?;
    let other = subject.strip_prefix("this object or another ")?;
    let mut filter = parse_event_object_filter(other)?;
    filter.other_than_source = true;
    Some((source_event, filter))
}

fn parse_trigger_subject(text: &str) -> Option<TriggerSubject> {
    let text = text.trim();
    if text == "this object" {
        Some(TriggerSubject::Source)
    } else {
        parse_event_object_filter(text).map(TriggerSubject::Matching)
    }
}

fn parse_cast_trigger_event(event: &str) -> Option<Trigger> {
    let (player, spell_text) = if let Some(spell) = event.strip_prefix("you cast ") {
        (PlayerRef::You, spell)
    } else if let Some(spell) = event.strip_prefix("an opponent casts ") {
        (PlayerRef::Opponent, spell)
    } else {
        let spell = event.strip_prefix("a player casts ")?;
        (PlayerRef::Any, spell)
    };
    if matches!(spell_text, "this object" | "this object spell") {
        return Some(Trigger::SourceCast);
    }
    let ordinal_text = match &player {
        PlayerRef::You => spell_text.strip_prefix("your "),
        PlayerRef::Opponent | PlayerRef::Any => spell_text.strip_prefix("their "),
        _ => None,
    };
    if let Some(ordinal_text) = ordinal_text {
        let occurrence_this_turn = if ordinal_text == "first spell each turn" {
            1
        } else if ordinal_text == "second spell each turn" {
            2
        } else {
            0
        };
        if occurrence_this_turn > 0 {
            return Some(Trigger::NthSpellCast {
                player,
                occurrence_this_turn,
            });
        }
    }
    let filters = parse_exact_spell_filters(spell_text)?;
    let mut triggers = filters
        .into_iter()
        .map(|spell| Trigger::Cast {
            player: player.clone(),
            spell,
        })
        .collect::<Vec<_>>();
    if triggers.len() == 1 {
        triggers.pop()
    } else {
        Some(Trigger::AnyOf(triggers))
    }
}

fn parse_card_draw_trigger_event(event: &str) -> Option<Trigger> {
    let (player, draw_text) = if let Some(text) = event.strip_prefix("you draw ") {
        (PlayerRef::You, text)
    } else if let Some(text) = event.strip_prefix("an opponent draws ") {
        (PlayerRef::Opponent, text)
    } else {
        let text = event.strip_prefix("a player draws ")?;
        (PlayerRef::Any, text)
    };
    let occurrence_this_turn = match draw_text {
        "a card" => None,
        "your first card each turn" | "their first card each turn" => Some(1),
        "your second card each turn" | "their second card each turn" => Some(2),
        _ => return None,
    };
    Some(Trigger::CardDrawn {
        player,
        occurrence_this_turn,
    })
}

fn parse_player_action_trigger_event(event: &str) -> Option<Trigger> {
    for (prefix, player) in [
        ("you ", PlayerRef::You),
        ("an opponent ", PlayerRef::Opponent),
        ("a player ", PlayerRef::Any),
    ] {
        let Some(action_text) = event.strip_prefix(prefix) else {
            continue;
        };
        for (verbs, action) in [
            (["attack", "attacks"], PlayerActionKind::Attack),
            (["cycle", "cycles"], PlayerActionKind::Cycle),
            (["discard", "discards"], PlayerActionKind::Discard),
            (["exert", "exerts"], PlayerActionKind::Exert),
            (["sacrifice", "sacrifices"], PlayerActionKind::Sacrifice),
            (["scry", "scries"], PlayerActionKind::Scry),
            (["surveil", "surveils"], PlayerActionKind::Surveil),
        ] {
            if verbs.contains(&action_text) {
                return Some(Trigger::PlayerAction {
                    player: player.clone(),
                    action,
                    subject: None,
                });
            }
            let Some(subject_text) = verbs
                .iter()
                .find_map(|verb| action_text.strip_prefix(&format!("{verb} ")))
            else {
                continue;
            };
            let subject = if subject_text == "this object" {
                Some(TriggerSubject::Source)
            } else if subject_text == "a card" {
                None
            } else {
                let subject_text = subject_text.strip_suffix(" card").unwrap_or(subject_text);
                let mut filter = parse_card_filter_phrase(subject_text)
                    .or_else(|| parse_simple_event_object_filter(subject_text))?;
                filter.zones.clear();
                Some(TriggerSubject::Matching(filter))
            };
            return Some(Trigger::PlayerAction {
                player: player.clone(),
                action,
                subject,
            });
        }
    }
    None
}

fn parse_exact_spell_filters(text: &str) -> Option<Vec<ObjectFilter>> {
    let lower = text.trim().to_ascii_lowercase();
    if [
        " first ",
        " second ",
        " during ",
        " from ",
        " that ",
        " with mana value ",
        " of the chosen ",
        " chosen color",
        " kicked ",
        " foretold ",
        " each turn",
        " targets ",
    ]
    .iter()
    .any(|marker| format!(" {lower} ").contains(marker))
    {
        return None;
    }
    let description = lower
        .strip_suffix(" spells")
        .or_else(|| lower.strip_suffix(" spell"))?
        .trim();
    let alternatives = description
        .split(" or ")
        .map(|part| {
            let part = part.trim();
            if matches!(part, "a" | "an") {
                ""
            } else {
                part.trim_start_matches("a ")
                    .trim_start_matches("an ")
                    .trim()
            }
        })
        .collect::<Vec<_>>();
    let mut filters = Vec::new();
    for alternative in alternatives {
        let mut filter = ObjectFilter::in_zone(Zone::Stack);
        filter.card_types.push(CardType::Spell);
        let mut semantic_words = Vec::new();
        for word in alternative.split_whitespace() {
            match word {
                "white" => filter.colors.push(Color::White),
                "blue" => filter.colors.push(Color::Blue),
                "black" => filter.colors.push(Color::Black),
                "red" => filter.colors.push(Color::Red),
                "green" => filter.colors.push(Color::Green),
                "colorless" => filter.colors.push(Color::Colorless),
                "noncreature" => filter.excluded_card_types.push(CardType::Creature),
                "artifact" => filter.card_types.push(CardType::Artifact),
                "battle" => filter.card_types.push(CardType::Battle),
                "creature" => filter.card_types.push(CardType::Creature),
                "enchantment" => filter.card_types.push(CardType::Enchantment),
                "instant" => filter.card_types.push(CardType::Instant),
                "land" => filter.card_types.push(CardType::Land),
                "planeswalker" => filter.card_types.push(CardType::Planeswalker),
                "sorcery" => filter.card_types.push(CardType::Sorcery),
                "permanent" => {
                    filter.excluded_card_types.push(CardType::Instant);
                    filter.excluded_card_types.push(CardType::Sorcery);
                }
                "historic" => filter.historic = true,
                "colored" => {}
                other => semantic_words.push(other),
            }
        }
        if semantic_words.len() > 1 {
            return None;
        }
        if let Some(subtype) = semantic_words.first() {
            filter.subtypes.push(canonical_subtype(subtype)?);
        }
        filters.push(filter);
    }
    (!filters.is_empty()).then_some(filters)
}

fn parse_event_object_filter(text: &str) -> Option<ObjectFilter> {
    let lower = text.trim().to_ascii_lowercase();
    let (lower, required_keywords) = if let Some((core, keyword_text)) = lower.rsplit_once(" with ")
        && !keyword_text.starts_with("power ")
        && let Ok(keywords) = parse_keyword_list(
            ClauseAddress {
                face_index: 0,
                clause_index: 0,
            },
            keyword_text,
        ) {
        (core.to_owned(), keywords)
    } else {
        (lower, Vec::new())
    };
    if [
        "one or more ",
        " with a ",
        " with an ",
        " during ",
        " that ",
        " who ",
        " was ",
        " equal to ",
        " and/or ",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        && !(lower.contains(" with power ") && lower.ends_with(" or greater"))
    {
        return None;
    }
    let mut filter =
        parse_card_filter_phrase(&lower).or_else(|| parse_simple_event_object_filter(&lower))?;
    filter.keywords.extend(required_keywords);
    filter.zones = vec![Zone::Battlefield];
    if lower.contains("you control") || lower.contains("under your control") {
        filter.controller = Some(PlayerRef::You);
    }
    if lower.starts_with("another ") {
        filter.other_than_source = true;
    }
    if let Some((_, power_text)) = lower.split_once(" with power ")
        && let Some(amount_text) = power_text.strip_suffix(" or greater")
    {
        filter.power = Some((
            Comparison::AtLeast,
            Box::new(parse_english_amount(amount_text)?),
        ));
    }
    Some(filter)
}

fn parse_simple_event_object_filter(text: &str) -> Option<ObjectFilter> {
    let mut filter = ObjectFilter::default();
    if text.contains("nontoken") {
        filter.token = Some(false);
    } else if text.contains("token") {
        filter.token = Some(true);
    }
    if text.contains("another ") || text.contains("other ") {
        filter.other_than_source = true;
    }
    let mut semantic_words = Vec::new();
    for word in text.split_whitespace() {
        let word = word.trim_matches(|character: char| !character.is_ascii_alphanumeric());
        match word {
            "" | "a" | "an" | "another" | "card" | "cards" | "other" | "nontoken" | "token"
            | "you" | "control" | "controls" | "under" | "your" | "opponent" | "opponents"
            | "own" => {}
            "artifact" | "artifacts" => filter.card_types.push(CardType::Artifact),
            "battle" | "battles" => filter.card_types.push(CardType::Battle),
            "creature" | "creatures" => filter.card_types.push(CardType::Creature),
            "enchantment" | "enchantments" => filter.card_types.push(CardType::Enchantment),
            "land" | "lands" => filter.card_types.push(CardType::Land),
            "planeswalker" | "planeswalkers" => filter.card_types.push(CardType::Planeswalker),
            "permanent" | "permanents" => filter.card_types.push(CardType::Permanent),
            "white" => filter.colors.push(Color::White),
            "blue" => filter.colors.push(Color::Blue),
            "black" => filter.colors.push(Color::Black),
            "red" => filter.colors.push(Color::Red),
            "green" => filter.colors.push(Color::Green),
            "colorless" => filter.colors.push(Color::Colorless),
            candidate => semantic_words.push(candidate),
        }
    }
    if semantic_words.len() > 1 {
        return None;
    }
    if let Some(subtype) = semantic_words.first() {
        filter.subtypes.push(canonical_subtype(subtype)?);
    }
    (!filter.card_types.is_empty()
        || !filter.subtypes.is_empty()
        || filter.token.is_some()
        || !filter.colors.is_empty())
    .then_some(filter)
}

fn canonical_subtype(word: &str) -> Option<String> {
    if word.is_empty()
        || !word
            .chars()
            .all(|character| character.is_ascii_alphabetic() || matches!(character, '-' | '\''))
    {
        return None;
    }
    let lower = word.to_ascii_lowercase();
    let singular = match lower.as_str() {
        "aetherborn" | "dwarves" | "elves" | "faeries" | "wolves" => match lower.as_str() {
            "dwarves" => "dwarf",
            "elves" => "elf",
            "faeries" => "faerie",
            "wolves" => "wolf",
            _ => "aetherborn",
        },
        _ if lower.ends_with("ies") && lower.len() > 3 => {
            return Some(title_case(&format!("{}y", &lower[..lower.len() - 3])));
        }
        _ if lower.ends_with('s') && !lower.ends_with("ss") && lower.len() > 1 => {
            return Some(title_case(&lower[..lower.len() - 1]));
        }
        _ => lower.as_str(),
    };
    Some(title_case(singular))
}

fn parse_condition(text: &str) -> Option<Condition> {
    let lower = text
        .trim()
        .trim_start_matches("if ")
        .trim_end_matches(',')
        .to_ascii_lowercase();
    if lower == "it isn't that player's turn" {
        return Some(Condition::NotThatPlayersTurn);
    }
    if lower == "this object is tapped" {
        return Some(Condition::SourceState(ObjectState::Tapped));
    }
    if let Some(counter_name) = lower
        .strip_prefix("there are no ")
        .and_then(|text| text.strip_suffix(" counters on this object"))
        .filter(|name| !name.trim().is_empty())
    {
        return Some(Condition::SourceCounterCount {
            counter: parse_counter_kind(counter_name),
            comparison: Comparison::Exactly,
            amount: 0,
        });
    }
    if lower == "you control a commander" {
        return Some(Condition::CommanderControlled {
            player: PlayerRef::You,
        });
    }
    if matches!(lower.as_str(), "it was kicked" | "this object was kicked") {
        return Some(Condition::CardWasKicked);
    }
    parse_control_condition(&lower).or_else(|| {
        lower
            .strip_prefix("it's a ")
            .and_then(|kind| parse_card_type(kind.trim()))
            .map(|card_type| Condition::ObjectIsCardType {
                object: ObjectRef::Source,
                card_type,
            })
    })
}

fn parse_card_type(text: &str) -> Option<CardType> {
    match text
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
        "artifact" => Some(CardType::Artifact),
        "battle" => Some(CardType::Battle),
        "creature" => Some(CardType::Creature),
        "enchantment" => Some(CardType::Enchantment),
        "instant" => Some(CardType::Instant),
        "land" => Some(CardType::Land),
        "planeswalker" => Some(CardType::Planeswalker),
        "sorcery" => Some(CardType::Sorcery),
        "spell" => Some(CardType::Spell),
        "permanent" => Some(CardType::Permanent),
        _ => None,
    }
}

fn parse_static_clause(
    address: ClauseAddress,
    clause: &str,
    source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let lower = clause.to_ascii_lowercase();
    if lower == "you have shroud." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::PlayersCannotBeTargeted {
                players: PlayerRef::You,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }
    if lower == "creatures entering don't cause abilities to trigger." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Restriction(
            Restriction::EnteringCreaturesDoNotCauseAbilitiesToTrigger {
                duration: Duration::WhileSourceOnBattlefield,
            },
        ));
        return Some(Ok(parsed));
    }
    if lower == "each creature you control can block an additional creature each combat." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AdditionalBlockCapacity {
                objects: ObjectRef::EachMatching(creatures),
                amount: 1,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }
    if lower == "no more than one creature can attack each combat." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AttackLimit {
                player: PlayerRef::Any,
                amount: 1,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }
    if lower == "each opponent can cast spells only any time they could cast a sorcery." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::SorcerySpeedCastingOnly {
                players: PlayerRef::Opponent,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }
    if lower == "this object spell can't be copied." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::SpellCannotBeCopied {
                object: ObjectRef::Source,
            }));
        return Some(Ok(parsed));
    }
    if lower == "x can't be 0." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::MinimumX {
                object: ObjectRef::Source,
                minimum: 1,
            }));
        return Some(Ok(parsed));
    }
    if matches!(
        lower.as_str(),
        "each creature you control assigns combat damage equal to its toughness rather than its power."
            | "each creature you control with toughness greater than its power assigns combat damage equal to its toughness rather than its power."
    ) {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Restriction(
            Restriction::AssignCombatDamageUsingToughness {
                objects: ObjectRef::EachMatching(creatures),
                only_if_toughness_greater: lower.contains("with toughness greater than its power"),
                duration: Duration::WhileSourceOnBattlefield,
            },
        ));
        return Some(Ok(parsed));
    }
    if lower == "this object blocks each combat if able." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::MustBlockIfAble {
                blockers: ObjectRef::Source,
                attacker: None,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }
    if lower == "this object spell costs {1} less to cast if you control a wizard." {
        let mut wizard = ObjectFilter::with_type(CardType::Creature);
        wizard.zones = vec![Zone::Battlefield];
        wizard.subtypes.push("Wizard".to_owned());
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCostWhen {
            object: ObjectRef::Source,
            mana: ManaCost("{1}".to_owned()),
            condition: Condition::ControlCount {
                player: PlayerRef::You,
                filter: wizard,
                comparison: Comparison::AtLeast,
                amount: Amount::Constant(1),
            },
        });
        return Some(Ok(parsed));
    }
    if let Some(amount_text) = lower
        .strip_prefix("this object's power and toughness are each equal to ")
        .and_then(|text| text.strip_suffix('.'))
    {
        let Some(amount) = parse_counted_amount(amount_text) else {
            return Some(Err(unsupported(address, clause)));
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::Source,
                operation: PowerToughnessOperation::SetBase,
                power: amount.clone(),
                toughness: amount,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }
    for (prefix, operation) in [
        (
            "this object's power is equal to ",
            PowerToughnessOperation::SetPower,
        ),
        (
            "this object's toughness is equal to ",
            PowerToughnessOperation::SetToughness,
        ),
    ] {
        let Some(amount_text) = lower
            .strip_prefix(prefix)
            .and_then(|text| text.strip_suffix('.'))
        else {
            continue;
        };
        let Some(amount) = parse_counted_amount(amount_text) else {
            return Some(Err(unsupported(address, clause)));
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::Source,
                operation,
                power: amount.clone(),
                toughness: amount,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }
    if let Some((mana_text, per_text)) = lower
        .strip_prefix("this object spell costs ")
        .and_then(|text| text.strip_suffix('.'))
        .and_then(|text| text.split_once(" less to cast for each "))
    {
        let mana = match parse_mana_cost(address, mana_text) {
            Ok(mana) => mana,
            Err(error) => return Some(Err(error)),
        };
        let Some(per) = parse_spell_cost_reduction_count(per_text) else {
            return Some(Err(unsupported(address, clause)));
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ReduceSpellCost {
            object: ObjectRef::Source,
            mana,
            per,
            maximum_reduction: None,
        });
        return Some(Ok(parsed));
    }
    if lower == "this object spell can't be countered." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::SpellCannotBeCountered {
                object: ObjectRef::Source,
            }));
        return Some(Ok(parsed));
    }
    if lower == "you have no maximum hand size." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::MaximumHandSize {
                player: PlayerRef::You,
                maximum: None,
            }));
        return Some(Ok(parsed));
    }
    if lower == "your opponents play with their hands revealed." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::HandsRevealed {
                players: PlayerRef::Opponent,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }
    if lower == "the \"legend rule\" doesn't apply to permanents you control." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::LegendRuleDoesNotApply {
                player: PlayerRef::You,
            }));
        return Some(Ok(parsed));
    }
    if lower == "this object attacks each combat if able." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::Restriction(
            Restriction::MustAttackEachCombatIfAble {
                object: ObjectRef::Source,
                duration: Duration::WhileSourceOnBattlefield,
            },
        ));
        return Some(Ok(parsed));
    }
    if matches!(
        lower.as_str(),
        "this object can't attack."
            | "this object can't attack or block."
            | "this object can't block and can't be blocked."
            | "this object can't attack or block and can't be blocked."
            | "this object can't attack, block, or be blocked."
    ) {
        let duration = Duration::WhileSourceOnBattlefield;
        let mut parsed = ParsedClause::new(Timing::Static);
        if lower.contains("can't attack") {
            parsed
                .effects
                .push(Effect::Restriction(Restriction::CannotAttack {
                    object: ObjectRef::Source,
                    duration: duration.clone(),
                }));
        }
        if lower.contains("can't block") || lower.contains("attack, block") {
            parsed
                .effects
                .push(Effect::Restriction(Restriction::CannotBlock {
                    object: ObjectRef::Source,
                    duration: duration.clone(),
                }));
        }
        if lower.contains("can't be blocked") || lower.contains("or be blocked") {
            parsed
                .effects
                .push(Effect::Restriction(Restriction::CannotBeBlocked {
                    object: ObjectRef::Source,
                    duration,
                }));
        }
        return Some(Ok(parsed));
    }
    if lower == "this object can't block." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotBlock {
                object: ObjectRef::Source,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }
    if lower == "this object can't be blocked." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotBeBlocked {
                object: ObjectRef::Source,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }
    if lower.starts_with("enchanted creature ")
        || lower.starts_with("enchanted permanent ")
        || lower.starts_with("enchanted land ")
        || lower.starts_with("equipped creature ")
    {
        return Some(parse_attachment_static_clause(
            address,
            clause,
            source_type_line,
        ));
    }
    if lower == "you control enchanted creature." || lower == "you control enchanted permanent." {
        if !source_type_line_has_attachment_kind(source_type_line, AttachmentKind::Aura) {
            return Some(Err(unsupported(address, clause)));
        }
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::ChangeControl {
            object: ObjectRef::AttachmentTarget {
                kind: AttachmentKind::Aura,
            },
            controller: PlayerRef::You,
        });
        return Some(Ok(parsed));
    }
    if lower
        == "as long as this object is untapped, players can't untap more than one land during their untap steps."
    {
        let condition = Condition::SourceState(ObjectState::Untapped);
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.conditions.push(condition.clone());
        parsed
            .effects
            .push(Effect::Restriction(Restriction::UntapLimit {
                player: PlayerRef::Any,
                filter: ObjectFilter::with_type(CardType::Land),
                amount: 1,
                step: Step::UntapStep,
            }));
        return Some(Ok(parsed));
    }
    if lower.starts_with("if you control a commander, you may cast this object spell ")
        && lower.ends_with("without paying its mana cost.")
    {
        let condition = parse_control_condition("you control a commander").unwrap();
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.conditions.push(condition.clone());
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AlternativeCastPermission(
                Box::new(AlternativeCastPermission {
                    object: ObjectRef::Source,
                    from: Zone::Hand,
                    cost: AlternativeCost::WithoutPayingManaCost,
                    timing: Trigger::Cast {
                        player: PlayerRef::You,
                        spell: ObjectFilter::in_zone(Zone::Hand),
                    },
                    condition: Some(condition),
                }),
            )));
        return Some(Ok(parsed));
    }
    if lower == "as this object enters, choose a creature type." {
        let mut parsed = ParsedClause::new(Timing::Replacement);
        parsed.effects.push(Effect::ChooseCreatureType {
            player: PlayerRef::You,
        });
        return Some(Ok(parsed));
    }
    if let Some(body) = lower.strip_suffix('.')
        && let Some((subject_text, pair_text)) = body.split_once(" get ")
        && !subject_text.starts_with("target ")
        && !subject_text.starts_with("enchanted ")
        && !subject_text.starts_with("equipped ")
        && let Some((operation, power, toughness)) = parse_power_toughness_modifier_pair(pair_text)
        && let Some(mut filter) = parse_card_filter_phrase(subject_text)
    {
        filter.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::EachMatching(filter),
                operation,
                power,
                toughness,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        return Some(Ok(parsed));
    }
    if let Some(body) = lower.strip_suffix('.')
        && let Some((subject_text, keyword_text)) = body.split_once(" have ")
        && !subject_text.starts_with("target ")
        && !subject_text.starts_with("enchanted ")
        && !subject_text.starts_with("equipped ")
        && let Ok(keywords) = parse_keyword_list(address, keyword_text)
        && !keywords.is_empty()
        && keywords
            .iter()
            .all(|keyword| matches!(keyword, Keyword::Defender | Keyword::Vigilance))
        && let Some(mut filter) = parse_card_filter_phrase(subject_text)
    {
        filter.zones = vec![Zone::Battlefield];
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::GrantKeyword {
            objects: ObjectRef::EachMatching(filter),
            keywords,
            duration: Duration::WhileSourceOnBattlefield,
        });
        return Some(Ok(parsed));
    }
    if lower.starts_with("creature tokens you control have ")
        || lower.starts_with("creatures you control have ")
        || lower.starts_with("attacking creatures you control have ")
    {
        if clause.contains('"') {
            // Quoted activated abilities are handled by the exact grant parser
            // below, not by the keyword-list grammar.
        } else {
            let (subject_text, keyword_text) = clause.split_once(" have ")?;
            let mut filter = parse_card_filter_phrase(subject_text)?;
            filter.zones = vec![Zone::Battlefield];
            filter.controller = Some(PlayerRef::You);
            if let Ok(keywords) = parse_keyword_list(address, keyword_text.trim_end_matches('.')) {
                let mut parsed = ParsedClause::new(Timing::Static);
                parsed.effects.push(Effect::GrantKeyword {
                    objects: ObjectRef::EachMatching(filter),
                    keywords,
                    duration: Duration::WhileSourceOnBattlefield,
                });
                return Some(Ok(parsed));
            }
        }
    }
    if lower.starts_with("lands you control have \"")
        || lower.starts_with("creatures you control have \"")
    {
        let Some((subject_text, quoted)) = clause.split_once(" have \"") else {
            return Some(Err(CompileError::UnsupportedSyntax {
                address,
                normalized_clause: clause.to_string(),
            }));
        };
        let Some(ability) = quoted.strip_suffix('"') else {
            return Some(Err(CompileError::UnsupportedSyntax {
                address,
                normalized_clause: clause.to_string(),
            }));
        };
        let mut filter = match parse_card_filter_phrase(subject_text) {
            Some(filter) => filter,
            None => {
                return Some(Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: clause.to_string(),
                }));
            }
        };
        filter.controller = Some(PlayerRef::You);
        filter.zones = vec![Zone::Battlefield];
        let Some(colon) = find_top_level(ability, ':') else {
            return Some(Err(CompileError::UnsupportedSyntax {
                address,
                normalized_clause: clause.to_string(),
            }));
        };
        let costs = match parse_costs(address, &ability[..colon]) {
            Ok(costs) => costs,
            Err(error) => return Some(Err(error)),
        };
        let nested = match parse_effect_body(address, &ability[colon + 1..], Timing::Activated) {
            Ok(parsed) => parsed,
            Err(error) => return Some(Err(error)),
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::GrantAbility {
            objects: ObjectRef::EachMatching(filter),
            ability: GrantedAbility {
                costs,
                effects: nested.effects,
            },
            duration: Duration::WhileSourceOnBattlefield,
        });
        return Some(Ok(parsed));
    }
    if lower.starts_with("enchanted creature loses all abilities and is ") {
        return Some(parse_aura_characteristic_clause(address, clause));
    }
    None
}

fn parse_attachment_static_clause(
    address: ClauseAddress,
    clause: &str,
    source_type_line: &str,
) -> Result<ParsedClause, CompileError> {
    let lower = clause.to_ascii_lowercase();
    let (kind, predicate) = if let Some(predicate) = lower.strip_prefix("enchanted creature ") {
        (AttachmentKind::Aura, predicate)
    } else if let Some(predicate) = lower.strip_prefix("enchanted permanent ") {
        (AttachmentKind::Aura, predicate)
    } else if let Some(predicate) = lower.strip_prefix("enchanted land ") {
        (AttachmentKind::Aura, predicate)
    } else if let Some(predicate) = lower.strip_prefix("equipped creature ") {
        (AttachmentKind::Equipment, predicate)
    } else {
        return Err(unsupported(address, clause));
    };
    if !source_type_line_can_supply_attachment_kind(source_type_line, kind) {
        return Err(unsupported(address, clause));
    }
    let object = ObjectRef::AttachmentTarget { kind };

    if kind == AttachmentKind::Aura
        && let Some(ability) = predicate
            .strip_prefix("has \"")
            .and_then(|text| text.strip_suffix('"'))
    {
        let Some(colon) = find_top_level(ability, ':') else {
            return Err(unsupported(address, clause));
        };
        let costs = parse_costs(address, &ability[..colon])?;
        let nested = parse_effect_body(address, &ability[colon + 1..], Timing::Activated)?;
        if !nested.conditions.is_empty()
            || !nested.costs.is_empty()
            || !nested.targets.is_empty()
            || nested.activation_restriction.is_some()
            || nested.effects.is_empty()
        {
            return Err(unsupported(address, clause));
        }
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::GrantAbility {
            objects: object,
            ability: GrantedAbility {
                costs,
                effects: nested.effects,
            },
            duration: Duration::WhileSourceOnBattlefield,
        });
        return Ok(parsed);
    }

    if kind == AttachmentKind::Aura && predicate.starts_with("loses all abilities and is ") {
        return parse_aura_characteristic_clause(address, clause);
    }
    if let Some(keyword_text) = predicate
        .strip_prefix("has ")
        .and_then(|text| text.strip_suffix('.'))
    {
        let keywords = parse_keyword_list(address, keyword_text)?;
        if keywords.is_empty() {
            return Err(unsupported(address, clause));
        }
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::GrantKeyword {
            objects: object,
            keywords,
            duration: Duration::WhileSourceOnBattlefield,
        });
        return Ok(parsed);
    }
    if let Some((pair_text, keyword_text)) = predicate
        .strip_prefix("gets ")
        .and_then(|text| text.strip_suffix('.'))
        .and_then(|text| text.split_once(" and has "))
    {
        let Some((operation, power, toughness)) = parse_power_toughness_modifier_pair(pair_text)
        else {
            return Err(unsupported(address, clause));
        };
        let keywords = parse_keyword_list(address, keyword_text)?;
        if keywords.is_empty() {
            return Err(unsupported(address, clause));
        }
        let duration = Duration::WhileSourceOnBattlefield;
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.extend([
            Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: object.clone(),
                operation,
                power,
                toughness,
                duration: duration.clone(),
            }),
            Effect::GrantKeyword {
                objects: object,
                keywords,
                duration,
            },
        ]);
        return Ok(parsed);
    }
    if let Some(pair_text) = predicate
        .strip_prefix("loses all abilities and has base power and toughness ")
        .and_then(|text| text.strip_suffix('.'))
    {
        let Some((power, toughness)) = parse_power_toughness_pair(pair_text) else {
            return Err(unsupported(address, clause));
        };
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed.effects.push(Effect::LoseAllAbilities {
            object: object.clone(),
            duration: Duration::WhileSourceOnBattlefield,
        });
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: object,
                operation: PowerToughnessOperation::SetBase,
                power,
                toughness,
                duration: Duration::WhileSourceOnBattlefield,
            }));
        if parsed
            .effects
            .iter()
            .all(exact_attachment_static_effect_is_live)
        {
            return Ok(parsed);
        }
        return Err(unsupported(address, clause));
    }

    if predicate == "doesn't untap during its controller's untap step." {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::DoesNotUntapDuring {
                object,
                step: Step::UntapStep,
            }));
        return Ok(parsed);
    }
    if let Some(restrictions) = exact_attachment_restrictions(&object, predicate) {
        let mut parsed = ParsedClause::new(Timing::Static);
        parsed
            .effects
            .extend(restrictions.into_iter().map(Effect::Restriction));
        return Ok(parsed);
    }

    let mut parsed = ParsedClause::new(Timing::Static);
    let mut state = EffectParseState::new();
    let body = clause.trim_end_matches('.');
    if !parse_characteristic_with_duration(
        address,
        body,
        Duration::WhileSourceOnBattlefield,
        &mut state,
        &mut parsed,
    )? || !parsed.targets.is_empty()
        || parsed.effects.is_empty()
        || !parsed
            .effects
            .iter()
            .all(exact_attachment_static_effect_is_live)
    {
        return Err(unsupported(address, clause));
    }
    Ok(parsed)
}

fn source_type_line_has_attachment_kind(source_type_line: &str, kind: AttachmentKind) -> bool {
    let lower = source_type_line.trim().to_ascii_lowercase();
    let (card_types, subtypes) = ['\u{2014}', '\u{2013}']
        .into_iter()
        .find_map(|separator| lower.split_once(separator))
        .or_else(|| lower.split_once(" - "))
        .unwrap_or((lower.as_str(), ""));
    let required_card_type = match kind {
        AttachmentKind::Aura => "enchantment",
        AttachmentKind::Equipment => "artifact",
    };
    let required_subtype = match kind {
        AttachmentKind::Aura => "aura",
        AttachmentKind::Equipment => "equipment",
    };
    card_types
        .split_whitespace()
        .any(|word| word == required_card_type)
        && subtypes
            .split_whitespace()
            .map(|word| word.trim_matches(|character: char| !character.is_ascii_alphabetic()))
            .any(|word| word == required_subtype)
}

fn source_type_line_can_supply_attachment_kind(
    source_type_line: &str,
    kind: AttachmentKind,
) -> bool {
    if source_type_line_has_attachment_kind(source_type_line, kind) {
        return true;
    }
    if kind != AttachmentKind::Aura {
        return false;
    }

    // Bestow cards are printed as enchantment creatures, then become Aura
    // spells and Aura permanents while bestowed. Their separate static text is
    // therefore allowed to address the enchanted creature even though the
    // printed type line does not contain the Aura subtype. Merely being a
    // creature, an enchantment, or carrying an Aura subtype is not enough.
    let lower = source_type_line.trim().to_ascii_lowercase();
    let card_types = ['\u{2014}', '\u{2013}']
        .into_iter()
        .find_map(|separator| lower.split_once(separator).map(|(types, _)| types))
        .or_else(|| lower.split_once(" - ").map(|(types, _)| types))
        .unwrap_or(lower.as_str());
    let card_types = card_types.split_whitespace().collect::<Vec<_>>();
    card_types.contains(&"enchantment") && card_types.contains(&"creature")
}

fn exact_attachment_static_effect_is_live(effect: &Effect) -> bool {
    match effect {
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
        Effect::GrantKeyword {
            objects: ObjectRef::AttachmentTarget { .. },
            keywords,
            duration: Duration::WhileSourceOnBattlefield,
        } => !keywords.is_empty(),
        _ => false,
    }
}

fn exact_attachment_restrictions(object: &ObjectRef, predicate: &str) -> Option<Vec<Restriction>> {
    let duration = Duration::WhileSourceOnBattlefield;
    let cannot_attack = || Restriction::CannotAttack {
        object: object.clone(),
        duration: duration.clone(),
    };
    let cannot_block = || Restriction::CannotBlock {
        object: object.clone(),
        duration: duration.clone(),
    };
    let activation_lock = || Restriction::ActivatedAbilitiesCannotBeActivated {
        object: object.clone(),
        duration: duration.clone(),
    };
    Some(match predicate {
        "can't attack." => vec![cannot_attack()],
        "can't block." => vec![cannot_block()],
        "can't attack or block." => vec![cannot_attack(), cannot_block()],
        "can't be blocked." => vec![Restriction::CannotBeBlocked {
            object: object.clone(),
            duration,
        }],
        "attacks each combat if able." => {
            vec![Restriction::MustAttackEachCombatIfAble {
                object: object.clone(),
                duration,
            }]
        }
        "activated abilities can't be activated." => vec![activation_lock()],
        "can't block, and its activated abilities can't be activated." => {
            vec![cannot_block(), activation_lock()]
        }
        "can't attack or block, and its activated abilities can't be activated." => {
            vec![cannot_attack(), cannot_block(), activation_lock()]
        }
        _ => return None,
    })
}

fn parse_spell_cost_reduction_count(text: &str) -> Option<CountExpression> {
    let lower = text.trim().to_ascii_lowercase();
    if lower == "basic land type among lands you control" {
        return Some(CountExpression::DistinctBasicLandTypes {
            player: PlayerRef::You,
        });
    }
    if matches!(lower.as_str(), "attacking creature" | "attacking creatures") {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        filter.attacking = Some(true);
        return Some(CountExpression::MatchingObjects {
            player: PlayerRef::Any,
            filter,
        });
    }
    if matches!(
        lower.as_str(),
        "attacking creature you control" | "attacking creatures you control"
    ) {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::You);
        filter.attacking = Some(true);
        return Some(CountExpression::MatchingObjects {
            player: PlayerRef::You,
            filter,
        });
    }
    if let Some(subject) = lower
        .strip_suffix(" on the battlefield")
        .or_else(|| lower.strip_suffix(" on the battlefield"))
    {
        let mut filter = parse_card_filter_phrase(subject)?;
        filter.zones = vec![Zone::Battlefield];
        return Some(CountExpression::MatchingObjects {
            player: PlayerRef::Any,
            filter,
        });
    }
    if let Some(subject) = lower.strip_suffix(" you control") {
        let mut filter = parse_card_filter_phrase(subject)?;
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::You);
        return Some(CountExpression::MatchingObjects {
            player: PlayerRef::You,
            filter,
        });
    }
    if let Some(subject) = lower.strip_suffix(" your opponents control") {
        let mut filter = parse_card_filter_phrase(subject)?;
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::Opponent);
        return Some(CountExpression::MatchingObjects {
            player: PlayerRef::Opponent,
            filter,
        });
    }
    if let Some(subject) = lower.strip_suffix(" in your graveyard") {
        let subject = subject
            .strip_suffix(" card")
            .or_else(|| subject.strip_suffix(" cards"))
            .unwrap_or(subject);
        let mut filter = parse_card_filter_phrase(subject)?;
        filter.card_type_match_any = filter.card_types.len() > 1
            && (subject.contains(" and ")
                || subject.contains(" or ")
                || subject.contains("and/or")
                || subject.contains(','));
        filter.zones = vec![Zone::Graveyard];
        filter.owner = Some(PlayerRef::You);
        return Some(CountExpression::CardsInZone {
            player: PlayerRef::You,
            zone: Zone::Graveyard,
            filter,
        });
    }
    None
}

fn parse_keyword_list(address: ClauseAddress, text: &str) -> Result<Vec<Keyword>, CompileError> {
    let mut keywords = Vec::new();
    let normalized = text
        .replace(" and ", ", ")
        .replace("gets ", "")
        .replace("gains ", "");
    for part in normalized
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let keyword = match part.to_ascii_lowercase().as_str() {
            "deathtouch" => Keyword::Deathtouch,
            "defender" => Keyword::Defender,
            "double strike" => Keyword::DoubleStrike,
            "first strike" => Keyword::FirstStrike,
            "flying" => Keyword::Flying,
            "haste" => Keyword::Haste,
            "hexproof" => Keyword::Hexproof,
            "indestructible" => Keyword::Indestructible,
            "lifelink" => Keyword::Lifelink,
            "menace" => Keyword::Menace,
            "reach" => Keyword::Reach,
            "shroud" => Keyword::Shroud,
            "trample" => Keyword::Trample,
            "vigilance" => Keyword::Vigilance,
            _ => {
                return Err(CompileError::UnsupportedSyntax {
                    address,
                    normalized_clause: text.to_string(),
                });
            }
        };
        keywords.push(keyword);
    }
    (!keywords.is_empty())
        .then_some(keywords)
        .ok_or_else(|| CompileError::UnsupportedSyntax {
            address,
            normalized_clause: text.to_string(),
        })
}

fn fear_blocker_target_filter() -> TargetFilter {
    let mut artifact_creature = ObjectFilter::with_type(CardType::Creature);
    artifact_creature.zones = vec![Zone::Battlefield];
    artifact_creature.card_types.push(CardType::Artifact);
    let mut black_creature = ObjectFilter::with_type(CardType::Creature);
    black_creature.zones = vec![Zone::Battlefield];
    black_creature.colors.push(Color::Black);
    TargetFilter::Any(vec![
        TargetFilter::Object(artifact_creature),
        TargetFilter::Object(black_creature),
    ])
}

fn parse_aura_characteristic_clause(
    address: ClauseAddress,
    clause: &str,
) -> Result<ParsedClause, CompileError> {
    let lower = clause.to_ascii_lowercase();
    let prefix = "enchanted creature loses all abilities and is ";
    let description = lower
        .strip_prefix(prefix)
        .and_then(|text| text.strip_suffix('.'))
        .ok_or_else(|| CompileError::UnsupportedSyntax {
            address,
            normalized_clause: clause.to_string(),
        })?;
    if description.contains('.') || description.contains('(') || description.contains(')') {
        return Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: clause.to_string(),
        });
    }
    let (before_power, power_and_name) = description
        .split_once(" with base power and toughness ")
        .ok_or_else(|| CompileError::UnsupportedSyntax {
            address,
            normalized_clause: clause.to_string(),
        })?;
    let (power_text, trailing_name) = power_and_name
        .split_once(" named ")
        .map_or((power_and_name, None), |(pair, name)| (pair, Some(name)));
    let mut description_words = before_power.split_whitespace().collect::<Vec<_>>();
    if description_words.first() == Some(&"a") || description_words.first() == Some(&"an") {
        description_words.remove(0);
    }
    let mut colors = Vec::new();
    while let Some(color) = description_words
        .first()
        .and_then(|word| parse_color_word(word.trim_end_matches(',')))
    {
        colors.push(color);
        description_words.remove(0);
        if description_words.first() == Some(&"and") {
            description_words.remove(0);
        }
    }
    let name = trailing_name.map(|name| {
        name.split_whitespace()
            .map(title_case)
            .collect::<Vec<_>>()
            .join(" ")
    });
    let subtype_end = description_words.len();
    if description_words.get(subtype_end.saturating_sub(1)) != Some(&"creature") {
        return Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: clause.to_string(),
        });
    }
    let subtypes = description_words[..subtype_end.saturating_sub(1)]
        .iter()
        .map(|word| title_case(word))
        .collect::<Vec<_>>();
    let Some((power, toughness)) = parse_power_toughness_pair(power_text) else {
        return Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: clause.to_string(),
        });
    };
    let object = ObjectRef::AttachmentTarget {
        kind: AttachmentKind::Aura,
    };
    let mut parsed = ParsedClause::new(Timing::Static);
    parsed.effects.push(Effect::LoseAllAbilities {
        object: object.clone(),
        duration: Duration::WhileSourceOnBattlefield,
    });
    parsed
        .effects
        .push(Effect::SetCharacteristics(SetCharacteristics {
            object,
            colors: Some(colors),
            card_types: Some(vec![CardType::Creature]),
            subtypes: Some(subtypes),
            name,
            base_power: Some(power),
            base_toughness: Some(toughness),
            retain_other_card_types: false,
            retain_other_subtypes: false,
            retain_other_colors: false,
            retain_other_names: false,
            duration: Duration::WhileSourceOnBattlefield,
        }));
    Ok(parsed)
}

fn parse_color_word(word: &str) -> Option<Color> {
    match word.to_ascii_lowercase().as_str() {
        "white" => Some(Color::White),
        "blue" => Some(Color::Blue),
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "colorless" => Some(Color::Colorless),
        _ => None,
    }
}

fn parse_power_toughness_pair(text: &str) -> Option<(Amount, Amount)> {
    let clean = text.trim().trim_end_matches('.');
    let (power, toughness) = clean.split_once('/')?;
    Some((
        parse_english_amount(power.trim().trim_start_matches('+'))?,
        parse_english_amount(toughness.trim().trim_start_matches('+'))?,
    ))
}

fn parse_power_toughness_modifier_pair(
    text: &str,
) -> Option<(PowerToughnessOperation, Amount, Amount)> {
    let clean = text.trim().trim_end_matches('.');
    let (power, toughness) = clean.split_once('/')?;
    let (power_negative, power) = parse_signed_power_toughness_amount(power)?;
    let (toughness_negative, toughness) = parse_signed_power_toughness_amount(toughness)?;
    let operation = match (power_negative, toughness_negative) {
        (false, false) => PowerToughnessOperation::Add,
        (true, true) => PowerToughnessOperation::Subtract,
        (false, true) => PowerToughnessOperation::AddPowerSubtractToughness,
        (true, false) => PowerToughnessOperation::SubtractPowerAddToughness,
    };
    Some((operation, power, toughness))
}

fn parse_signed_power_toughness_amount(text: &str) -> Option<(bool, Amount)> {
    let text = text.trim();
    let (negative, magnitude) = if let Some(magnitude) = text.strip_prefix('-') {
        (true, magnitude)
    } else {
        let magnitude = text.strip_prefix('+')?;
        (false, magnitude)
    };
    Some((negative, parse_english_amount(magnitude)?))
}

fn parse_duration_leading_clause(
    address: ClauseAddress,
    clause: &str,
    _source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    clause
        .to_ascii_lowercase()
        .starts_with("until end of turn,")
        .then(|| parse_effect_body(address, clause, Timing::SpellResolution))
}

fn parse_resolution_clause(
    address: ClauseAddress,
    clause: &str,
    source_type_line: &str,
) -> Option<Result<ParsedClause, CompileError>> {
    let lower = clause.to_ascii_lowercase();
    let starts_like_effect = [
        "add ",
        "choose ",
        "copy ",
        "counter ",
        "create ",
        "discard ",
        "creatures ",
        "destroy ",
        "draw ",
        "each opponent ",
        "each player ",
        "exile ",
        "for each ",
        "gain ",
        "investigate",
        "look ",
        "manifest ",
        "mill ",
        "permanents ",
        "play ",
        "prevent ",
        "proliferate",
        "put ",
        "return ",
        "reveal ",
        "search ",
        "shuffle ",
        "scry ",
        "surveil ",
        "switch ",
        "tap ",
        "target ",
        "untap ",
        "up to ",
        "you draw ",
        "you gain ",
        "you get ",
        "you may ",
        "you mill ",
        "your opponents ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    starts_like_effect.then(|| {
        let mut state = EffectParseState::new();
        state.source_attachment_kind = [AttachmentKind::Aura, AttachmentKind::Equipment]
            .into_iter()
            .find(|kind| source_type_line_has_attachment_kind(source_type_line, *kind));
        parse_effect_body_with_state(address, clause, Timing::SpellResolution, state)
    })
}

#[derive(Debug, Clone)]
struct EffectParseState {
    next_target_id: u8,
    next_selection_id: u8,
    last_object: Option<ObjectRef>,
    last_player: Option<PlayerRef>,
    pending_hand_choice: Option<ObjectSelection>,
    selected_targets: Vec<u8>,
    source_attachment_kind: Option<AttachmentKind>,
}

impl EffectParseState {
    fn new() -> Self {
        Self {
            next_target_id: 0,
            next_selection_id: 0,
            last_object: None,
            last_player: None,
            pending_hand_choice: None,
            selected_targets: Vec::new(),
            source_attachment_kind: None,
        }
    }

    fn allocate_target(
        &mut self,
        filter: TargetFilter,
        amount: TargetAmount,
        relationship: TargetRelationship,
    ) -> Target {
        let id = self.next_target_id;
        self.next_target_id = self.next_target_id.saturating_add(1);
        self.selected_targets.push(id);
        Target {
            id,
            chooser: PlayerRef::You,
            filter,
            amount,
            relationship,
        }
    }

    fn allocate_selection(
        &mut self,
        chooser: PlayerRef,
        filter: ObjectFilter,
        amount: TargetAmount,
    ) -> ObjectSelection {
        let id = self.next_selection_id;
        self.next_selection_id = self.next_selection_id.saturating_add(1);
        ObjectSelection {
            id,
            chooser,
            filter,
            amount,
        }
    }
}

fn parse_effect_body(
    address: ClauseAddress,
    text: &str,
    timing: Timing,
) -> Result<ParsedClause, CompileError> {
    parse_effect_body_with_state(address, text, timing, EffectParseState::new())
}

fn parse_effect_body_with_state(
    address: ClauseAddress,
    text: &str,
    timing: Timing,
    mut state: EffectParseState,
) -> Result<ParsedClause, CompileError> {
    let mut parsed = ParsedClause::new(timing);
    if text.eq_ignore_ascii_case(
        "reveal the top card of your library and put that card into your hand. you lose life equal to its mana value.",
    ) {
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::RevealTopToHandLoseManaValue {
                player: PlayerRef::You,
                repeat: Amount::Constant(1),
            },
        ));
        return Ok(parsed);
    }
    let statements = split_top_level_sentences(text);
    if statements.is_empty() {
        return Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: text.to_string(),
        });
    }
    let mut index = 0usize;
    while index < statements.len() {
        if let Some(consumed) =
            parse_optional_effect_group(address, &statements, index, &mut state, &mut parsed)?
        {
            index += consumed;
            continue;
        }
        if parse_standalone_optional_effect(address, statements[index], &mut state, &mut parsed)? {
            index += 1;
            continue;
        }
        parse_effect_statement(address, statements[index], &mut state, &mut parsed)?;
        index += 1;
    }
    if parsed.effects.is_empty() || state.pending_hand_choice.is_some() {
        return Err(CompileError::UnsupportedSyntax {
            address,
            normalized_clause: text.to_string(),
        });
    }
    Ok(parsed)
}

fn parse_optional_effect_group(
    address: ClauseAddress,
    statements: &[&str],
    index: usize,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<Option<usize>, CompileError> {
    let statement = statements[index].trim();
    let lower = statement.to_ascii_lowercase();
    let Some(required_text) = lower.strip_prefix("you may ") else {
        return Ok(None);
    };
    let Some(continuation) = statements.get(index + 1) else {
        return Ok(None);
    };
    let continuation = continuation.trim();
    let continuation_lower = continuation.to_ascii_lowercase();
    let Some(body_lower) = continuation_lower.strip_prefix("if you do, ") else {
        return Ok(None);
    };
    let required_len = statement.len() - required_text.len();
    let required = &statement[required_len..];
    let mut trial_state = state.clone();
    let mut nested = ParsedClause::new(Timing::SpellResolution);
    if parse_effect_statement(address, required, &mut trial_state, &mut nested).is_err() {
        return Ok(None);
    }

    let body_start = continuation.len() - body_lower.len();
    parse_effect_statement(
        address,
        &continuation[body_start..],
        &mut trial_state,
        &mut nested,
    )?;

    if nested.effects.is_empty()
        || !nested.conditions.is_empty()
        || !nested.costs.is_empty()
        || nested.activation_restriction.is_some()
    {
        return Ok(None);
    }
    parsed.targets.extend(nested.targets);
    parsed
        .predefined_token_creations
        .extend(nested.predefined_token_creations);
    parsed.effects.push(Effect::Optional(nested.effects));
    *state = trial_state;
    Ok(Some(2))
}

fn parse_standalone_optional_effect(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let statement = statement.trim();
    let lower = statement.to_ascii_lowercase();

    let Some(required_text) = lower.strip_prefix("you may ") else {
        return Ok(false);
    };

    let mut direct_state = state.clone();
    let mut direct_parsed = parsed.clone();
    match parse_effect_statement(address, statement, &mut direct_state, &mut direct_parsed) {
        Ok(()) => {
            *state = direct_state;
            *parsed = direct_parsed;
            return Ok(true);
        }
        Err(CompileError::UnsupportedSyntax { .. }) => {}
        Err(error) => return Err(error),
    }

    let required_start = statement.len() - required_text.len();
    let required = &statement[required_start..];
    let mut trial_state = state.clone();
    let mut nested = ParsedClause::new(Timing::SpellResolution);
    parse_effect_statement(address, required, &mut trial_state, &mut nested)?;
    if nested.effects.is_empty()
        || !nested.conditions.is_empty()
        || !nested.costs.is_empty()
        || nested.activation_restriction.is_some()
    {
        return Err(unsupported(address, statement));
    }
    parsed.targets.extend(nested.targets);
    parsed
        .predefined_token_creations
        .extend(nested.predefined_token_creations);
    parsed.effects.push(Effect::Optional(nested.effects));
    *state = trial_state;
    Ok(true)
}

fn parse_effect_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<(), CompileError> {
    let statement = statement.trim();
    let lower = statement.to_ascii_lowercase();

    if matches!(
        lower.as_str(),
        "take an extra turn after this one."
            | "take one extra turn after this one."
            | "you take an extra turn after this one."
            | "you take one extra turn after this one."
    ) {
        parsed.effects.push(Effect::TakeExtraTurn(ExtraTurnEffect {
            player: PlayerRef::You,
            lose_at_end_step: false,
        }));
        return Ok(());
    }

    if lower
        == "if that spell is countered this way, exile it instead of putting it into its owner's graveyard."
    {
        let Some(Effect::Counter { object }) = parsed.effects.last().cloned() else {
            return Err(unsupported(address, statement));
        };
        if state.last_object.as_ref() != Some(&object) {
            return Err(unsupported(address, statement));
        }
        parsed.effects.pop();
        parsed.effects.push(Effect::CounterToZone {
            object,
            zone: Zone::Exile,
        });
        return Ok(());
    }

    if lower == "this object becomes an artifact creature until end of turn." {
        parsed.effects.push(Effect::Animate(AnimateEffect {
            object: ObjectRef::Source,
            power: Amount::Constant(0),
            toughness: Amount::Constant(0),
            retain_printed_power_toughness: true,
            colors: Vec::new(),
            subtypes: Vec::new(),
            keywords: Vec::new(),
            retain_land: false,
            duration: Duration::UntilEndOfTurn,
        }));
        return Ok(());
    }

    if let Some(body) = lower
        .trim_end_matches('.')
        .strip_suffix(" until end of turn and can't be blocked this turn")
    {
        let effect_start = parsed.effects.len();
        if !parse_characteristic_with_duration(
            address,
            body,
            Duration::UntilEndOfTurn,
            state,
            parsed,
        )? || parsed.effects.len() == effect_start
        {
            return Err(unsupported(address, statement));
        }
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotBeBlocked {
                object,
                duration: Duration::ThisTurn,
            }));
        return Ok(());
    }

    if matches!(
        lower.as_str(),
        "it has \"sacrifice this token: add {c}.\""
            | "they have \"sacrifice this token: add {c}.\""
    ) {
        let singular = lower.starts_with("it has ");
        let Some(Effect::CreateToken(creation)) = parsed.effects.last_mut() else {
            return Err(unsupported(address, statement));
        };
        if singular != matches!(creation.amount, Amount::Constant(1)) {
            return Err(unsupported(address, statement));
        }
        let TokenSpecification::Defined(definition) = &mut creation.specification else {
            return Err(unsupported(address, statement));
        };
        let is_scion_or_spawn = definition.subtypes.iter().any(|subtype| {
            subtype.eq_ignore_ascii_case("Scion") || subtype.eq_ignore_ascii_case("Spawn")
        });
        if !definition.card_types.contains(&CardType::Creature)
            || !definition
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case("Eldrazi"))
            || !is_scion_or_spawn
            || !definition.abilities.is_empty()
        {
            return Err(unsupported(address, statement));
        }
        definition
            .abilities
            .push(colorless_sacrifice_mana_ability());
        return Ok(());
    }

    if lower == "put a creature card from among them onto the battlefield." {
        let mut predicate = ObjectFilter::with_type(CardType::Creature);
        predicate.zones = vec![Zone::Library];
        parsed.effects.push(Effect::SelectFromLookedAt {
            player: PlayerRef::You,
            amount: Amount::Constant(1),
            predicate,
            reveal: false,
            face_down: false,
            tapped: false,
            destination: Zone::Battlefield,
        });
        return Ok(());
    }

    if matches!(
        lower.as_str(),
        "choose target creature." | "choose target creature you control."
    ) {
        let description = lower
            .strip_prefix("choose ")
            .expect("matched exact choose-target instruction");
        let target = parse_target_description(address, description, state)?;
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        parsed.effects.push(Effect::ResolveTargetChoice {
            object: object.clone(),
        });
        state.last_object = Some(object);
        return Ok(());
    }

    if lower == "choose target opponent." {
        let target = state.allocate_target(
            TargetFilter::Opponent,
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let player = PlayerRef::TargetPlayer(target.id);
        parsed.targets.push(target);
        parsed.effects.push(Effect::ResolvePlayerTargetChoice {
            player: player.clone(),
        });
        state.last_player = Some(player);
        return Ok(());
    }

    if matches!(
        lower.as_str(),
        "choose an opponent." | "then choose an opponent."
    ) {
        parsed.effects.push(Effect::ChoosePlayer {
            chooser: PlayerRef::You,
            eligible: PlayerRef::Opponent,
        });
        state.last_player = Some(PlayerRef::ThatPlayer);
        return Ok(());
    }

    if matches!(
        lower.as_str(),
        "choose another player." | "then choose another player."
    ) {
        parsed.effects.push(Effect::ChoosePlayer {
            chooser: PlayerRef::You,
            eligible: PlayerRef::OtherPlayer,
        });
        state.last_player = Some(PlayerRef::ThatPlayer);
        return Ok(());
    }

    if lower == "this object becomes prepared." {
        parsed.effects.push(Effect::Prepare {
            object: ObjectRef::Source,
        });
        state.last_object = Some(ObjectRef::Source);
        return Ok(());
    }

    if lower == "it gains haste." {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::GrantKeyword {
            objects: object.clone(),
            keywords: vec![Keyword::Haste],
            duration: Duration::Permanent,
        });
        state.last_object = Some(object);
        return Ok(());
    }

    if lower == "target creature doesn't untap during its controller's next untap step." {
        let target = parse_target_description(address, "target creature.", state)?;
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        parsed.effects.push(Effect::PreventNextUntap {
            object: object.clone(),
        });
        state.last_object = Some(object);
        return Ok(());
    }

    if matches!(
        lower.as_str(),
        "it doesn't untap during its controller's next untap step."
            | "that creature doesn't untap during its controller's next untap step."
    ) {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::PreventNextUntap {
            object: object.clone(),
        });
        state.last_object = Some(object);
        return Ok(());
    }

    if lower
        == "it doesn't untap during its controller's untap step for as long as this object remains tapped."
    {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed
            .effects
            .push(Effect::Restriction(Restriction::DoesNotUntapDuringIf {
                object: object.clone(),
                step: Step::UntapStep,
                condition: Condition::SourceState(ObjectState::Tapped),
            }));
        state.last_object = Some(object);
        return Ok(());
    }

    if lower == "target creature can't block this object this turn." {
        let target = state.allocate_target(
            TargetFilter::Object(ObjectFilter::with_type(CardType::Creature)),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let blocker = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotBlockObject {
                blocker: blocker.clone(),
                attacker: ObjectRef::Source,
                duration: Duration::ThisTurn,
            }));
        state.last_object = Some(blocker);
        return Ok(());
    }

    if lower == "exile it at the beginning of the next end step." {
        if matches!(parsed.effects.last(), Some(Effect::CreateToken(_))) {
            let Some(Effect::CreateToken(creation)) = parsed.effects.pop() else {
                unreachable!("checked token creation effect")
            };
            parsed.effects.push(Effect::CreateTokenWithDelayedMove {
                creation,
                destination: Zone::Exile,
                trigger: Trigger::BeginningOfNextEndStep,
            });
            return Ok(());
        }
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Battlefield),
            to: Zone::Exile,
            tapped: false,
            face_down: false,
            delayed_until: Some(Trigger::BeginningOfNextEndStep),
        }));
        state.last_object = Some(object);
        return Ok(());
    }

    if lower == "at the beginning of your next end step, exile it." {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Battlefield),
            to: Zone::Exile,
            tapped: false,
            face_down: false,
            delayed_until: Some(Trigger::BeginningOf {
                step: Step::EndStep,
                player: TurnPlayer::NextTurn,
            }),
        }));
        state.last_object = Some(object);
        return Ok(());
    }

    if matches!(
        lower.as_str(),
        "sacrifice it at the beginning of the next end step."
            | "sacrifice it at the beginning of your next end step."
    ) {
        if matches!(parsed.effects.last(), Some(Effect::CreateToken(_))) {
            let Some(Effect::CreateToken(creation)) = parsed.effects.pop() else {
                unreachable!("checked token creation effect")
            };
            parsed.effects.push(Effect::CreateTokenWithDelayedMove {
                creation,
                destination: Zone::Graveyard,
                trigger: if lower.contains("your next end step") {
                    Trigger::BeginningOf {
                        step: Step::EndStep,
                        player: TurnPlayer::NextTurn,
                    }
                } else {
                    Trigger::BeginningOfNextEndStep
                },
            });
            return Ok(());
        }
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Battlefield),
            to: Zone::Graveyard,
            tapped: false,
            face_down: false,
            delayed_until: Some(if lower.contains("your next end step") {
                Trigger::BeginningOf {
                    step: Step::EndStep,
                    player: TurnPlayer::NextTurn,
                }
            } else {
                Trigger::BeginningOfNextEndStep
            }),
        }));
        state.last_object = Some(object);
        return Ok(());
    }

    if lower == "return it to its owner's hand at the beginning of the next end step." {
        if !timing_tracks_an_object_put_into_a_graveyard(&parsed.timing) {
            return Err(unsupported(address, statement));
        }
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Graveyard),
            to: Zone::Hand,
            tapped: false,
            face_down: false,
            delayed_until: Some(Trigger::BeginningOfNextEndStep),
        }));
        state.last_object = Some(object);
        return Ok(());
    }

    if lower == "return it to its owner's hand at the beginning of your next end step." {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Battlefield),
            to: Zone::Hand,
            tapped: false,
            face_down: false,
            delayed_until: Some(Trigger::BeginningOf {
                step: Step::EndStep,
                player: TurnPlayer::NextTurn,
            }),
        }));
        state.last_object = Some(object);
        return Ok(());
    }

    if lower == "attach it to target creature you control." {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        creature.controller = Some(PlayerRef::You);
        let target = state.allocate_target(
            TargetFilter::Object(creature),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let target_ref = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        if matches!(parsed.effects.last(), Some(Effect::CreateToken(_))) {
            let Some(Effect::CreateToken(creation)) = parsed.effects.pop() else {
                unreachable!("checked token creation")
            };
            let is_single_equipment = matches!(creation.amount, Amount::Constant(1))
                && matches!(
                    &creation.specification,
                    TokenSpecification::Defined(definition)
                        if definition.card_types.contains(&CardType::Artifact)
                            && definition
                                .subtypes
                                .iter()
                                .any(|subtype| subtype.eq_ignore_ascii_case("Equipment"))
                );
            if !is_single_equipment {
                return Err(unsupported(address, statement));
            }
            parsed.effects.push(Effect::CreateTokenAttached {
                creation,
                target: target_ref.clone(),
                kind: AttachmentKind::Equipment,
            });
        } else if let Some(kind) = state.source_attachment_kind {
            parsed.effects.push(Effect::Attach {
                attachment: ObjectRef::Source,
                target: target_ref.clone(),
                kind,
            });
        } else {
            return Err(unsupported(address, statement));
        }
        state.last_object = Some(target_ref);
        return Ok(());
    }

    if lower == "attach this equipment to it." {
        let Some(kind) = state.source_attachment_kind else {
            return Err(unsupported(address, statement));
        };
        if matches!(parsed.effects.last(), Some(Effect::CreateToken(_))) {
            let Some(Effect::CreateToken(creation)) = parsed.effects.pop() else {
                unreachable!("checked token creation")
            };
            if !matches!(creation.amount, Amount::Constant(1)) {
                return Err(unsupported(address, statement));
            }
            parsed.effects.push(Effect::CreateTokenAndAttachSource {
                creation,
                attachment: ObjectRef::Source,
                kind,
            });
        } else {
            let target = state
                .last_object
                .clone()
                .unwrap_or(ObjectRef::TriggeringObject);
            parsed.effects.push(Effect::Attach {
                attachment: ObjectRef::Source,
                target,
                kind,
            });
        }
        return Ok(());
    }

    if lower == "return another creature you control to its owner's hand." {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        creature.controller = Some(PlayerRef::You);
        creature.other_than_source = true;
        let selection =
            state.allocate_selection(PlayerRef::You, creature, TargetAmount::Exactly(1));
        parsed.effects.push(Effect::MoveSelected(SelectedZoneMove {
            selection,
            to: Zone::Hand,
            tapped: false,
            face_down: false,
        }));
        return Ok(());
    }

    if lower == "return another permanent you control to its owner's hand." {
        let mut permanent = ObjectFilter::in_zone(Zone::Battlefield);
        permanent.controller = Some(PlayerRef::You);
        permanent.other_than_source = true;
        let selection =
            state.allocate_selection(PlayerRef::You, permanent, TargetAmount::Exactly(1));
        parsed.effects.push(Effect::MoveSelected(SelectedZoneMove {
            selection,
            to: Zone::Hand,
            tapped: false,
            face_down: false,
        }));
        return Ok(());
    }

    if matches!(
        lower.as_str(),
        "put target creature card from a graveyard onto the battlefield under your control."
            | "put target creature card from a graveyard onto the battlefield under your control tapped."
    ) {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Graveyard];
        let target = state.allocate_target(
            TargetFilter::Object(creature),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        parsed.effects.push(Effect::MoveZoneUnderControl {
            object: object.clone(),
            from: Zone::Graveyard,
            to: Zone::Battlefield,
            controller: PlayerRef::You,
            tapped: lower.ends_with("under your control tapped."),
            face_down: false,
            delayed_until: None,
        });
        state.last_object = Some(object);
        return Ok(());
    }

    if lower
        == "put target creature card from an opponent's graveyard onto the battlefield under your control."
    {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Graveyard];
        creature.owner = Some(PlayerRef::Opponent);
        let target = state.allocate_target(
            TargetFilter::Object(creature),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        parsed.effects.push(Effect::MoveZoneUnderControl {
            object: object.clone(),
            from: Zone::Graveyard,
            to: Zone::Battlefield,
            controller: PlayerRef::You,
            tapped: false,
            face_down: false,
            delayed_until: None,
        });
        state.last_object = Some(object);
        return Ok(());
    }

    if lower == "return target vehicle card from your graveyard to the battlefield." {
        let mut vehicle = ObjectFilter::in_zone(Zone::Graveyard);
        vehicle.owner = Some(PlayerRef::You);
        vehicle.subtypes.push("Vehicle".to_owned());
        let target = state.allocate_target(
            TargetFilter::Object(vehicle),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Graveyard),
            to: Zone::Battlefield,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        state.last_object = Some(object);
        return Ok(());
    }
    if lower == "play it this turn." {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        let mut filter = ObjectFilter::in_zone(Zone::Exile);
        filter.owner = None;
        parsed
            .effects
            .push(Effect::GrantCastPermission(CastPermission {
                affected: PlayerRef::You,
                objects: Some(object),
                filter,
                from: Zone::Exile,
                timing: CastTiming::Normal,
                duration: Duration::ThisTurn,
                alternative_cost: None,
                additional_costs: Vec::new(),
                mana_as_any_type: false,
                exile_after_resolution: false,
            }));
        return Ok(());
    }

    if let Some((filter, player)) = match lower.as_str() {
        "target opponent reveals their hand." => Some((
            TargetFilter::Opponent,
            PlayerRef::TargetPlayer(state.next_target_id),
        )),
        "target player reveals their hand." => Some((
            TargetFilter::Player,
            PlayerRef::TargetPlayer(state.next_target_id),
        )),
        _ => None,
    } {
        let target = state.allocate_target(
            filter,
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        debug_assert_eq!(player, PlayerRef::TargetPlayer(target.id));
        parsed.targets.push(target);
        state.last_player = Some(player.clone());
        parsed.effects.push(Effect::RevealHand { player });
        return Ok(());
    }
    if lower == "you reveal your hand." {
        state.last_player = Some(PlayerRef::You);
        parsed.effects.push(Effect::RevealHand {
            player: PlayerRef::You,
        });
        return Ok(());
    }
    if lower == "each player reveals their hand." {
        state.last_player = Some(PlayerRef::Any);
        parsed.effects.push(Effect::RevealHand {
            player: PlayerRef::Any,
        });
        return Ok(());
    }
    if matches!(
        lower.as_str(),
        "you choose a nonland card from it." | "you choose a card from it."
    ) {
        if state.pending_hand_choice.is_some() {
            return Err(unsupported(address, statement));
        }
        let player = state
            .last_player
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        if player == PlayerRef::Any {
            return Err(unsupported(address, statement));
        }
        let mut filter = ObjectFilter::in_zone(Zone::Hand);
        filter.owner = Some(player);
        if lower.contains("nonland") {
            filter.excluded_card_types.push(CardType::Land);
        }
        state.pending_hand_choice =
            Some(state.allocate_selection(PlayerRef::You, filter, TargetAmount::Exactly(1)));
        return Ok(());
    }

    if lower == "each player sacrifices a creature of their choice." {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        parsed.effects.push(Effect::EachPlayerSacrifices {
            filter: creature,
            amount: 1,
        });
        return Ok(());
    }

    if lower == "each opponent sacrifices a creature of their choice." {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        parsed.effects.push(Effect::PlayersSacrifice {
            players: PlayerRef::Opponent,
            filter: creature,
            amount: 1,
        });
        return Ok(());
    }

    if let Some(maximum) = lower
        .strip_prefix("return target creature card with mana value ")
        .and_then(|text| text.strip_suffix(" or less from your graveyard to the battlefield."))
        .and_then(parse_english_amount)
    {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Graveyard];
        creature.owner = Some(PlayerRef::You);
        creature.mana_value = Some((Comparison::AtMost, Box::new(maximum)));
        let target = state.allocate_target(
            TargetFilter::Object(creature),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Graveyard),
            to: Zone::Battlefield,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        parsed.targets.push(target);
        state.last_object = Some(object);
        return Ok(());
    }

    if lower
        == "return two target creature cards that share a creature type from your graveyard to your hand."
    {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Graveyard];
        creature.owner = Some(PlayerRef::You);
        let target = state.allocate_target(
            TargetFilter::Object(creature),
            TargetAmount::Exactly(2),
            TargetRelationship::ShareCreatureType,
        );
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: ObjectRef::Target(target.id),
            from: Some(Zone::Graveyard),
            to: Zone::Hand,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        parsed.targets.push(target);
        return Ok(());
    }

    if lower == "this object becomes the creature type of your choice until end of turn." {
        parsed.effects.push(Effect::SetCreatureTypeToChoice {
            object: ObjectRef::Source,
            duration: Duration::UntilEndOfTurn,
        });
        return Ok(());
    }
    if matches!(
        lower.as_str(),
        "target land becomes the basic land type of your choice until end of turn."
            | "target land you control becomes the basic land type of your choice until end of turn."
    ) {
        let controlled = lower.starts_with("target land you control");
        let mut land = ObjectFilter::with_type(CardType::Land);
        land.zones = vec![Zone::Battlefield];
        if controlled {
            land.controller = Some(PlayerRef::You);
        }
        let target = state.allocate_target(
            TargetFilter::Object(land),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        parsed.effects.push(Effect::SetBasicLandTypeToChoice {
            object: object.clone(),
            duration: Duration::UntilEndOfTurn,
        });
        state.last_object = Some(object);
        return Ok(());
    }
    if lower == "draw a card and reveal it. if it isn't a land card, discard it." {
        parsed.effects.push(Effect::DrawRevealDiscardIfNonland {
            player: PlayerRef::You,
        });
        return Ok(());
    }

    if lower == "this object becomes colorless until end of turn." {
        parsed
            .effects
            .push(Effect::SetCharacteristics(SetCharacteristics {
                object: ObjectRef::Source,
                colors: Some(Vec::new()),
                card_types: None,
                subtypes: None,
                name: None,
                base_power: None,
                base_toughness: None,
                retain_other_card_types: true,
                retain_other_subtypes: true,
                retain_other_colors: false,
                retain_other_names: true,
                duration: Duration::UntilEndOfTurn,
            }));
        return Ok(());
    }

    if lower == "this object becomes the color of your choice until end of turn." {
        parsed.effects.push(Effect::SetColorToChoice {
            object: ObjectRef::Source,
            duration: Duration::UntilEndOfTurn,
        });
        return Ok(());
    }

    if lower == "this object gets +1/-1 or -1/+1 until end of turn." {
        parsed.effects.extend([
            Effect::ChooseModeFrom {
                count: ChoiceCount::Exactly(1),
                option_count: 2,
            },
            Effect::Conditional {
                condition: Condition::ModeSelected(0),
                if_true: vec![Effect::ModifyPowerToughness(PowerToughnessChange {
                    objects: ObjectRef::Source,
                    operation: PowerToughnessOperation::AddPowerSubtractToughness,
                    power: Amount::Constant(1),
                    toughness: Amount::Constant(1),
                    duration: Duration::UntilEndOfTurn,
                })],
                if_false: vec![Effect::ModifyPowerToughness(PowerToughnessChange {
                    objects: ObjectRef::Source,
                    operation: PowerToughnessOperation::SubtractPowerAddToughness,
                    power: Amount::Constant(1),
                    toughness: Amount::Constant(1),
                    duration: Duration::UntilEndOfTurn,
                })],
            },
        ]);
        return Ok(());
    }

    if let Some(amount_text) = lower.strip_prefix("look at the top ").and_then(|text| {
        text.strip_suffix(" cards of your library, then put them back in any order.")
    }) && let Some(amount) = parse_english_amount(amount_text)
    {
        parsed.effects.extend([
            Effect::LookAtTop {
                player: PlayerRef::You,
                amount,
            },
            Effect::ReorderLookedAtOnLibraryTop {
                player: PlayerRef::You,
            },
        ]);
        return Ok(());
    }

    if let Some(amount_text) = lower
        .strip_prefix("investigate ")
        .and_then(|text| text.strip_suffix('.'))
        && let Some(amount) = (amount_text == "twice")
            .then_some(Amount::Constant(2))
            .or_else(|| parse_english_amount(amount_text))
        && !matches!(amount, Amount::Constant(0))
    {
        let number = if amount == Amount::Constant(1) {
            TokenGrammaticalNumber::Singular
        } else {
            TokenGrammaticalNumber::Plural
        };
        parsed.effects.push(Effect::CreateToken(TokenCreation {
            player: PlayerRef::You,
            amount: amount.clone(),
            specification: TokenSpecification::Defined(Box::new(clue_definition())),
            tapped: false,
            attacking: false,
        }));
        parsed
            .predefined_token_creations
            .push(ParsedPredefinedTokenCreation {
                kind: PredefinedArtifactTokenKind::Clue,
                number,
            });
        return Ok(());
    }

    if matches!(lower.as_str(), "investigate" | "investigate.") {
        parsed.effects.push(Effect::CreateToken(TokenCreation {
            player: PlayerRef::You,
            amount: Amount::Constant(1),
            specification: TokenSpecification::Defined(Box::new(clue_definition())),
            tapped: false,
            attacking: false,
        }));
        parsed
            .predefined_token_creations
            .push(ParsedPredefinedTokenCreation {
                kind: PredefinedArtifactTokenKind::Clue,
                number: TokenGrammaticalNumber::Singular,
            });
        return Ok(());
    }

    if matches!(lower.as_str(), "it connives." | "this object connives.") {
        let object = state.last_object.clone().unwrap_or(ObjectRef::Source);
        let player = PlayerRef::ControllerOf(Box::new(object.clone()));
        let mut filter = ObjectFilter::in_zone(Zone::Hand);
        filter.owner = Some(player.clone());
        let discard = state.allocate_selection(player, filter, TargetAmount::Exactly(1));
        parsed.effects.push(Effect::Connive { object, discard });
        return Ok(());
    }

    if matches!(
        lower.as_str(),
        "tap enchanted creature." | "tap enchanted permanent." | "tap enchanted land."
    ) {
        parsed.effects.push(Effect::Tap {
            object: ObjectRef::AttachmentTarget {
                kind: AttachmentKind::Aura,
            },
        });
        return Ok(());
    }

    let graveyard_bottom_owner = match lower.as_str() {
        "put target card from your graveyard on the bottom of your library." => {
            Some(Some(PlayerRef::You))
        }
        "put target card from a graveyard on the bottom of its owner's library." => Some(None),
        _ => None,
    };
    if let Some(owner) = graveyard_bottom_owner {
        let mut filter = ObjectFilter::in_zone(Zone::Graveyard);
        filter.owner = owner;
        let target = state.allocate_target(
            TargetFilter::Object(filter),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::MoveToLibraryBottom { object });
        return Ok(());
    }

    if lower == "target player shuffles their graveyard into their library." {
        let target = state.allocate_target(
            TargetFilter::Player,
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let player = PlayerRef::TargetPlayer(target.id);
        parsed.targets.push(target);
        state.last_player = Some(player.clone());
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::ShuffleGraveyardIntoLibrary { player },
        ));
        return Ok(());
    }

    if lower == "shuffle the cards from your hand into your library, then draw that many cards." {
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::ShuffleHandIntoLibraryAndDrawSame {
                player: PlayerRef::You,
            },
        ));
        return Ok(());
    }

    if lower
        == "each player shuffles their hand and graveyard into their library, then draws seven cards."
    {
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::ShuffleHandAndGraveyardIntoLibraryAndDraw {
                player: PlayerRef::Any,
                amount: Amount::Constant(7),
            },
        ));
        return Ok(());
    }

    if lower == "counter that spell or ability." {
        parsed.effects.push(Effect::Counter {
            object: ObjectRef::TriggeringObject,
        });
        return Ok(());
    }

    if lower == "destroy it." {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::Destroy { object });
        return Ok(());
    }

    if lower == "destroy this object." {
        parsed.effects.push(Effect::Destroy {
            object: ObjectRef::Source,
        });
        state.last_object = Some(ObjectRef::Source);
        return Ok(());
    }

    if lower == "exile enchanted creature."
        && state.source_attachment_kind == Some(AttachmentKind::Aura)
    {
        let object = ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Aura,
        };
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Battlefield),
            to: Zone::Exile,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        state.last_object = Some(object);
        return Ok(());
    }

    if lower == "shuffle this object into its owner's library." {
        let object = ObjectRef::Source;
        parsed.effects.extend([
            Effect::MoveZone(ZoneMove {
                object: object.clone(),
                from: None,
                to: Zone::Library,
                tapped: false,
                face_down: false,
                delayed_until: None,
            }),
            Effect::ShuffleLibrary {
                player: PlayerRef::OwnerOf(Box::new(object.clone())),
            },
        ]);
        state.last_object = Some(object);
        return Ok(());
    }

    if lower == "play an additional land this turn." {
        parsed
            .effects
            .push(Effect::Restriction(Restriction::AdditionalLandPlays {
                player: PlayerRef::You,
                amount: 1,
                duration: Duration::ThisTurn,
            }));
        return Ok(());
    }

    if let Some(target_text) = lower
        .strip_prefix("gain control of ")
        .and_then(|text| text.strip_suffix(" until end of turn."))
        && matches!(
            target_text,
            "target creature" | "target creature an opponent controls"
        )
    {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        if target_text.ends_with("an opponent controls") {
            filter.controller = Some(PlayerRef::Opponent);
        }
        let target = state.allocate_target(
            TargetFilter::Object(filter),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        parsed.effects.push(Effect::ChangeControlUntil {
            object: object.clone(),
            controller: PlayerRef::You,
            duration: Duration::UntilEndOfTurn,
        });
        state.last_object = Some(object);
        return Ok(());
    }

    if let Some(target_text) = lower
        .strip_prefix("gain control of ")
        .and_then(|text| text.strip_suffix('.'))
        && matches!(
            target_text,
            "target creature"
                | "target creature an opponent controls"
                | "target permanent"
                | "target permanent an opponent controls"
        )
    {
        let mut filter = if target_text.starts_with("target creature") {
            ObjectFilter::with_type(CardType::Creature)
        } else {
            ObjectFilter::default()
        };
        filter.zones = vec![Zone::Battlefield];
        if target_text.ends_with("an opponent controls") {
            filter.controller = Some(PlayerRef::Opponent);
        }
        let target = state.allocate_target(
            TargetFilter::Object(filter),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        parsed.effects.push(Effect::ChangeControl {
            object: object.clone(),
            controller: PlayerRef::You,
        });
        state.last_object = Some(object);
        return Ok(());
    }

    if matches!(
        lower.as_str(),
        "target creature blocks this turn if able."
            | "target creature can't attack this turn."
            | "target creature can't block this turn."
            | "target creature can't attack or block this turn."
    ) {
        let target = state.allocate_target(
            TargetFilter::Object(ObjectFilter::with_type(CardType::Creature)),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        if lower == "target creature blocks this turn if able." {
            parsed
                .effects
                .push(Effect::Restriction(Restriction::MustBlockIfAble {
                    blockers: object.clone(),
                    attacker: None,
                    duration: Duration::ThisTurn,
                }));
        } else {
            if lower.contains("can't attack") {
                parsed
                    .effects
                    .push(Effect::Restriction(Restriction::CannotAttack {
                        object: object.clone(),
                        duration: Duration::ThisTurn,
                    }));
            }
            if lower.contains("can't block") || lower.contains("or block") {
                parsed
                    .effects
                    .push(Effect::Restriction(Restriction::CannotBlock {
                        object: object.clone(),
                        duration: Duration::ThisTurn,
                    }));
            }
        }
        state.last_object = Some(object);
        return Ok(());
    }

    if lower == "attach this equipment to target creature you control."
        && state.source_attachment_kind == Some(AttachmentKind::Equipment)
    {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::You);
        let target = state.allocate_target(
            TargetFilter::Object(filter),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        parsed.effects.push(Effect::Attach {
            attachment: ObjectRef::Source,
            target: object.clone(),
            kind: AttachmentKind::Equipment,
        });
        state.last_object = Some(object);
        return Ok(());
    }

    if let Some(mana_text) = lower
        .strip_prefix("that player adds ")
        .and_then(|text| text.strip_suffix('.'))
    {
        let expression = parse_mana_production_expression(&format!("Add {mana_text}."))
            .map_err(|_| unsupported(address, statement))?;
        let mut production = compile_typed_mana_production(expression);
        production.player = PlayerRef::ThatPlayer;
        parsed.effects.push(Effect::AddMana(production));
        return Ok(());
    }

    if lower
        == "reveal the top card of your library and put that card into your hand. you lose life equal to its mana value."
    {
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::RevealTopToHandLoseManaValue {
                player: PlayerRef::You,
                repeat: Amount::Constant(1),
            },
        ));
        return Ok(());
    }

    if lower == "target player exiles a card from their graveyard." {
        let target = state.allocate_target(
            TargetFilter::Player,
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let player = PlayerRef::TargetPlayer(target.id);
        parsed.targets.push(target);
        state.last_player = Some(player.clone());
        let mut filter = ObjectFilter::in_zone(Zone::Graveyard);
        filter.owner = Some(player.clone());
        let selection = state.allocate_selection(player, filter, TargetAmount::Exactly(1));
        parsed.effects.push(Effect::MoveSelected(SelectedZoneMove {
            selection,
            to: Zone::Exile,
            tapped: false,
            face_down: false,
        }));
        return Ok(());
    }

    if lower == "roll a d20." {
        parsed.effects.push(Effect::RollDie { sides: 20 });
        return Ok(());
    }

    if lower == "roll a six-sided die." {
        parsed.effects.push(Effect::RollDie { sides: 6 });
        return Ok(());
    }
    if lower == "flip a coin." {
        parsed.effects.push(Effect::FlipCoin);
        return Ok(());
    }

    if lower == "proliferate." {
        let choice_id = state.next_selection_id;
        state.next_selection_id = state.next_selection_id.saturating_add(1);
        parsed.effects.push(Effect::Proliferate { choice_id });
        return Ok(());
    }

    let experience_amount = if lower == "you get an experience counter." {
        Some(Amount::Constant(1))
    } else {
        lower
            .strip_prefix("you get ")
            .and_then(|text| text.strip_suffix(" experience counters."))
            .and_then(parse_english_amount)
    };
    if let Some(amount) = experience_amount {
        parsed.effects.push(Effect::PutPlayerCounter {
            player: PlayerRef::You,
            counter: "experience".to_owned(),
            amount,
        });
        state.last_player = Some(PlayerRef::You);
        return Ok(());
    }

    let (energy_statement, inline_energy_reminder) = lower
        .strip_suffix(").")
        .and_then(|text| text.rsplit_once(" ("))
        .map_or((lower.as_str(), None), |(instruction, reminder)| {
            let amount = reminder
                .strip_suffix(" energy counter")
                .or_else(|| reminder.strip_suffix(" energy counters"))
                .and_then(parse_english_amount);
            (instruction, amount)
        });
    if let Some(mut symbols) = energy_statement
        .strip_prefix("you get ")
        .and_then(|text| text.strip_suffix('.'))
        .or_else(|| energy_statement.strip_prefix("you get "))
    {
        let mut amount = 0u32;
        while let Some(rest) = symbols.strip_prefix("{e}") {
            amount = amount.saturating_add(1);
            symbols = rest;
        }
        if amount > 0 && symbols.is_empty() {
            let amount = Amount::Constant(amount);
            if let Some(expected) = inline_energy_reminder.as_ref()
                && expected != &amount
            {
                return Err(unsupported(address, statement));
            }
            parsed.effects.push(Effect::PutPlayerCounter {
                player: PlayerRef::You,
                counter: "energy".to_owned(),
                amount: amount.clone(),
            });
            if inline_energy_reminder.is_some() {
                parsed.reminder = Some(ReminderSemantics::EnergyCounterExplanation { amount });
            }
            state.last_player = Some(PlayerRef::You);
            return Ok(());
        }
    }

    if let Some((scry_amount, draw_amount)) = lower
        .strip_prefix("scry ")
        .and_then(|text| text.split_once(", then draw "))
        .and_then(|(scry_text, draw_text)| {
            let scry_amount = parse_english_amount(scry_text)?;
            let draw_amount = if draw_text == "a card." {
                Amount::Constant(1)
            } else {
                draw_text
                    .strip_suffix(" cards.")
                    .and_then(parse_english_amount)?
            };
            Some((scry_amount, draw_amount))
        })
    {
        parsed.effects.extend([
            Effect::Scry {
                player: PlayerRef::You,
                amount: scry_amount,
            },
            Effect::Draw {
                player: PlayerRef::You,
                amount: draw_amount,
                optional: false,
                delayed_until: None,
            },
        ]);
        return Ok(());
    }

    if lower == "draw cards equal to its power." {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::Draw {
            player: PlayerRef::You,
            amount: Amount::Count(Box::new(CountExpression::PowerOf { object })),
            optional: false,
            delayed_until: None,
        });
        return Ok(());
    }

    if let Some((counter_text, subject_text)) = lower
        .strip_prefix("remove ")
        .and_then(|text| text.strip_suffix('.'))
        .and_then(|text| text.split_once(" counter from "))
        && !subject_text.contains(" at ")
        && !subject_text.contains(" during ")
    {
        let (amount, counter_name) = parse_counter_amount_and_name(counter_text)
            .ok_or_else(|| unsupported(address, statement))?;
        let object = parse_subject_object_ref(address, subject_text, state, parsed)?;
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::RemoveCounter {
            object,
            counter: parse_counter_kind(counter_name),
            amount,
        });
        return Ok(());
    }

    if lower.starts_with("until ")
        && let Some(production) = parsed
            .effects
            .iter_mut()
            .rev()
            .find_map(|effect| match effect {
                Effect::AddMana(production) if production.typed.is_some() => Some(production),
                _ => None,
            })
    {
        let retention = parse_mana_retention_clause(statement)
            .map_err(|error| invalid_mana_expression(address, statement, &error))?;
        let typed = production
            .typed
            .as_mut()
            .expect("typed mana production was selected");
        if typed.retention != TypedManaRetention::Normal {
            return Err(invalid_mana_detail(
                address,
                statement,
                "mana retention is specified more than once",
            ));
        }
        typed.retention = retention;
        return Ok(());
    }
    if lower.starts_with("spend this mana only to ")
        && let Some(production) = parsed
            .effects
            .iter_mut()
            .rev()
            .find_map(|effect| match effect {
                Effect::AddMana(production) if production.typed.is_some() => Some(production),
                _ => None,
            })
    {
        let restriction = parse_mana_spend_restriction_clause(statement)
            .map_err(|error| invalid_mana_expression(address, statement, &error))?;
        let typed = production
            .typed
            .as_mut()
            .expect("typed mana production was selected");
        if typed.spend_restriction.is_some() {
            return Err(invalid_mana_detail(
                address,
                statement,
                "mana spend restriction is specified more than once",
            ));
        }
        typed.spend_restriction = Some(restriction);
        return Ok(());
    }

    if lower == "exile that card from your graveyard." {
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: ObjectRef::TriggeringObject,
            from: Some(Zone::Graveyard),
            to: Zone::Exile,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        state.last_object = Some(ObjectRef::TriggeringObject);
        return Ok(());
    }
    if lower == "exile that card." {
        if let Some(selection) = state.pending_hand_choice.take() {
            parsed.effects.push(Effect::MoveSelected(SelectedZoneMove {
                selection,
                to: Zone::Exile,
                tapped: false,
                face_down: false,
            }));
        } else {
            let object = state
                .last_object
                .clone()
                .ok_or_else(|| unsupported(address, statement))?;
            parsed.effects.push(Effect::MoveZone(ZoneMove {
                object: object.clone(),
                from: None,
                to: Zone::Exile,
                tapped: false,
                face_down: false,
                delayed_until: None,
            }));
            state.last_object = Some(object);
        }
        return Ok(());
    }
    if lower == "exile a card from a graveyard." {
        let selection = state.allocate_selection(
            PlayerRef::You,
            ObjectFilter::in_zone(Zone::Graveyard),
            TargetAmount::Exactly(1),
        );
        parsed.effects.push(Effect::MoveSelected(SelectedZoneMove {
            selection,
            to: Zone::Exile,
            tapped: false,
            face_down: false,
        }));
        return Ok(());
    }
    if lower == "your opponents can't cast noncreature spells this turn." {
        let mut filter = ObjectFilter::with_type(CardType::Spell);
        filter.zones = vec![Zone::Stack];
        filter.excluded_card_types.push(CardType::Creature);
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotCast {
                affected: PlayerRef::Opponent,
                filter,
                duration: Duration::ThisTurn,
                during_turn_of: None,
            }));
        return Ok(());
    }
    if let Some(target_text) = lower.strip_suffix(" can't be blocked this turn.") {
        let object = if target_text == "this creature" || target_text == "this object" {
            ObjectRef::Source
        } else if target_text == "it" || target_text == "that creature" {
            state
                .last_object
                .clone()
                .ok_or_else(|| unsupported(address, statement))?
        } else if target_text.contains("target ") {
            let target = parse_target_description(address, target_text, state)?;
            let object = ObjectRef::Target(target.id);
            parsed.targets.push(target);
            object
        } else {
            return Err(unsupported(address, statement));
        };
        state.last_object = Some(object.clone());
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotBeBlocked {
                object,
                duration: Duration::ThisTurn,
            }));
        return Ok(());
    }
    if let Some(target_text) = lower.strip_suffix(" can't block this turn.") {
        let object = if target_text == "it" || target_text == "that creature" {
            state
                .last_object
                .clone()
                .ok_or_else(|| unsupported(address, statement))?
        } else if target_text.contains("target ") {
            let target = parse_target_description(address, target_text, state)?;
            let object = ObjectRef::Target(target.id);
            parsed.targets.push(target);
            object
        } else {
            return Err(unsupported(address, statement));
        };
        state.last_object = Some(object.clone());
        parsed
            .effects
            .push(Effect::Restriction(Restriction::CannotBlock {
                object,
                duration: Duration::ThisTurn,
            }));
        return Ok(());
    }
    if let Some(target_text) = lower.strip_suffix(" attacks this turn if able.") {
        let object = if target_text == "it" || target_text == "that creature" {
            state
                .last_object
                .clone()
                .ok_or_else(|| unsupported(address, statement))?
        } else if target_text.contains("target ") {
            let target = parse_target_description(address, target_text, state)?;
            let object = ObjectRef::Target(target.id);
            parsed.targets.push(target);
            object
        } else {
            return Err(unsupported(address, statement));
        };
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::Restriction(
            Restriction::MustAttackEachCombatIfAble {
                object,
                duration: Duration::ThisTurn,
            },
        ));
        return Ok(());
    }
    if let Some(rest) = lower.strip_prefix("exile the top ") {
        let (amount_text, player, face_down) = if let Some(amount_text) =
            rest.strip_suffix(" cards of your library.")
        {
            (amount_text, PlayerRef::You, false)
        } else if rest == "card of your library." {
            ("one", PlayerRef::You, false)
        } else if let Some(amount_text) = rest.strip_suffix(" card of your library.") {
            (amount_text, PlayerRef::You, false)
        } else if rest == "card of your library face down." {
            ("one", PlayerRef::You, true)
        } else if let Some(amount_text) = rest.strip_suffix(" card of your library face down.") {
            (amount_text, PlayerRef::You, true)
        } else if rest == "card of that player's library." {
            ("one", PlayerRef::ThatPlayer, false)
        } else if let Some(amount_text) = rest.strip_suffix(" card of that player's library.") {
            (amount_text, PlayerRef::ThatPlayer, false)
        } else {
            ("", PlayerRef::You, false)
        };
        if !amount_text.is_empty() {
            let amount =
                parse_english_amount(amount_text).ok_or_else(|| unsupported(address, statement))?;
            parsed.effects.push(Effect::ExileTop(TopLibraryExile {
                player,
                amount,
                face_down,
                cast_permission: None,
                delayed_destination: None,
            }));
            return Ok(());
        }
    }
    if matches!(
        lower.as_str(),
        "you may play them this turn."
            | "you may play it this turn."
            | "you may play those cards this turn."
            | "you may play that card this turn."
            | "until end of turn, you may play that card."
            | "until end of turn, you may cast that card."
            | "until the end of your next turn, you may play that card."
            | "until the end of your next turn, you may play those cards."
    ) {
        let Some(Effect::ExileTop(exile)) = parsed
            .effects
            .iter_mut()
            .rev()
            .find(|effect| matches!(effect, Effect::ExileTop(_)))
        else {
            return Err(unsupported(address, statement));
        };
        let mut filter = ObjectFilter::in_zone(Zone::Exile);
        filter.owner = None;
        let duration = if matches!(
            lower.as_str(),
            "until the end of your next turn, you may play that card."
                | "until the end of your next turn, you may play those cards."
        ) {
            Duration::UntilEndOfNextTurn
        } else {
            Duration::ThisTurn
        };
        exile.cast_permission = Some(CastPermission {
            affected: PlayerRef::You,
            objects: None,
            filter,
            from: Zone::Exile,
            timing: CastTiming::Normal,
            duration,
            alternative_cost: None,
            additional_costs: Vec::new(),
            mana_as_any_type: false,
            exile_after_resolution: false,
        });
        return Ok(());
    }
    if lower == "put that card into your hand at the beginning of your next end step." {
        let Some(Effect::ExileTop(exile)) = parsed
            .effects
            .iter_mut()
            .rev()
            .find(|effect| matches!(effect, Effect::ExileTop(_)))
        else {
            return Err(unsupported(address, statement));
        };
        exile.delayed_destination = Some((
            Zone::Hand,
            Trigger::BeginningOf {
                step: Step::EndStep,
                player: TurnPlayer::NextTurn,
            },
        ));
        return Ok(());
    }
    if lower == "until end of turn, you don't lose this mana as steps and phases end." {
        return Err(invalid_mana_detail(
            address,
            statement,
            "retained mana requires typed pool provenance and phase-boundary expiration",
        ));
    }
    if lower
        == "spend this mana only to cast a creature spell of the chosen type, and that spell can't be countered."
    {
        return Err(invalid_mana_detail(
            address,
            statement,
            "restricted mana requires per-unit source provenance during payment",
        ));
    }
    if lower == "an opponent gains control of this object." {
        parsed.effects.push(Effect::ChangeControl {
            object: ObjectRef::Source,
            controller: PlayerRef::ThatPlayer,
        });
        return Ok(());
    }
    if lower == "target opponent gains control of this object." {
        let target = state.allocate_target(
            TargetFilter::Opponent,
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let controller = PlayerRef::TargetPlayer(target.id);
        parsed.targets.push(target);
        state.last_player = Some(controller.clone());
        parsed.effects.push(Effect::ChangeControl {
            object: ObjectRef::Source,
            controller,
        });
        return Ok(());
    }
    if lower == "you may choose new targets for the copy." {
        let object = state
            .last_object
            .clone()
            .unwrap_or(ObjectRef::TriggeringObject);
        parsed.effects.push(Effect::ChooseNewTargets { object });
        return Ok(());
    }
    if lower == "you may choose new targets for target spell or ability." {
        let target = state.allocate_target(
            TargetFilter::Any(vec![
                TargetFilter::Spell(ObjectFilter::with_type(CardType::Spell)),
                TargetFilter::Object(ObjectFilter::default()),
            ]),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::ChooseNewTargets { object });
        return Ok(());
    }
    if lower == "you win the game." {
        parsed.effects.push(Effect::WinGame {
            player: PlayerRef::You,
        });
        return Ok(());
    }
    if lower == "you lose the game." {
        parsed.effects.push(Effect::LoseGame {
            player: PlayerRef::You,
        });
        return Ok(());
    }
    if lower == "sacrifice this object." {
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: ObjectRef::Source,
            from: Some(Zone::Battlefield),
            to: Zone::Graveyard,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        return Ok(());
    }
    if lower == "sacrifice it." {
        let object = state.last_object.clone().unwrap_or(ObjectRef::Source);
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Battlefield),
            to: Zone::Graveyard,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        state.last_object = Some(object);
        return Ok(());
    }
    if let Some(cost_text) = lower
        .strip_prefix("sacrifice this object unless you pay ")
        .and_then(|text| text.strip_suffix('.'))
    {
        let cost = parse_payment_cost(address, cost_text)?;
        parsed.effects.push(Effect::Conditional {
            condition: Condition::UnlessPaid {
                player: PlayerRef::You,
                cost: cost.clone(),
            },
            if_true: vec![Effect::MoveZone(ZoneMove {
                object: ObjectRef::Source,
                from: Some(Zone::Battlefield),
                to: Zone::Graveyard,
                tapped: false,
                face_down: false,
                delayed_until: None,
            })],
            if_false: vec![Effect::PayCost(cost)],
        });
        return Ok(());
    }
    if let Some(counter_text) = lower
        .strip_prefix("exile this object with ")
        .and_then(|text| text.strip_suffix(" counters on it."))
        && let Some((amount, counter_name)) = parse_counter_amount_and_name(counter_text)
    {
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: ObjectRef::Source,
            from: None,
            to: Zone::Exile,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        parsed.effects.push(Effect::PutCounter {
            object: ObjectRef::Source,
            counter: parse_counter_kind(counter_name),
            amount,
        });
        state.last_object = Some(ObjectRef::Source);
        return Ok(());
    }
    if lower == "exile this object." {
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: ObjectRef::Source,
            from: None,
            to: Zone::Exile,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        return Ok(());
    }
    if parse_optional_payment_statement(address, statement, state, parsed)? {
        return Ok(());
    }

    if lower
        == "exile target creature you control, then return that card to the battlefield under its owner's control."
    {
        let mut creature = ObjectFilter::with_type(CardType::Creature);
        creature.zones = vec![Zone::Battlefield];
        creature.controller = Some(PlayerRef::You);
        let target = state.allocate_target(
            TargetFilter::Object(creature),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Battlefield),
            to: Zone::Exile,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        parsed.effects.push(Effect::MoveZoneUnderControl {
            object: object.clone(),
            from: Zone::Exile,
            to: Zone::Battlefield,
            controller: PlayerRef::OwnerOf(Box::new(object.clone())),
            tapped: false,
            face_down: false,
            delayed_until: None,
        });
        state.last_object = Some(object);
        return Ok(());
    }

    if parse_simple_sequence_statement(address, statement, state, parsed)? {
        return Ok(());
    }
    if parse_add_mana_statement(address, statement, state, parsed)? {
        return Ok(());
    }
    if parse_counter_destroy_return_statement(address, statement, state, parsed)? {
        return Ok(());
    }
    if parse_search_statement(address, statement, state, parsed)? {
        return Ok(());
    }
    if parse_token_statement(address, statement, state, parsed)? {
        return Ok(());
    }
    if parse_draw_life_statement(address, statement, state, parsed)? {
        return Ok(());
    }
    if parse_characteristic_statement(address, statement, state, parsed)? {
        return Ok(());
    }
    if parse_utility_statement(address, statement, state, parsed)? {
        return Ok(());
    }

    if lower == "its activated abilities can't be activated this turn." {
        let Some(object) = state.last_object.clone() else {
            return Err(unsupported(address, statement));
        };
        parsed.effects.push(Effect::Restriction(
            Restriction::ActivatedAbilitiesCannotBeActivated {
                object,
                duration: Duration::ThisTurn,
            },
        ));
        return Ok(());
    }
    if lower == "target creature blocks this object this turn if able." {
        let target = parse_target_description(address, "target creature", state)?;
        let blocker = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        state.last_object = Some(blocker.clone());
        parsed
            .effects
            .push(Effect::Restriction(Restriction::MustBlockIfAble {
                blockers: blocker,
                attacker: Some(ObjectRef::Source),
                duration: Duration::ThisTurn,
            }));
        return Ok(());
    }
    if lower == "it's still a land." {
        // This is consumed only as the explicit retention component of the
        // immediately preceding animation instruction.
        let Some(Effect::Animate(animation)) = parsed.effects.last_mut() else {
            return Err(unsupported(address, statement));
        };
        animation.retain_land = true;
        return Ok(());
    }
    if lower.starts_with("this ability costs ")
        && lower.contains(" less to activate for each legendary creature you control.")
    {
        let amount_text = lower
            .strip_prefix("this ability costs ")
            .and_then(|text| {
                text.split_once(" less to activate")
                    .map(|(amount, _)| amount)
            })
            .ok_or_else(|| unsupported(address, statement))?;
        let mana = parse_mana_cost(address, amount_text)?;
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.supertypes.push(Supertype::Legendary);
        filter.controller = Some(PlayerRef::You);
        parsed.effects.push(Effect::ReduceActivationCost {
            mana,
            per: CountExpression::MatchingObjects {
                player: PlayerRef::You,
                filter,
            },
            minimum_total: None,
        });
        return Ok(());
    }
    if lower.starts_with("if ") || lower.starts_with("then if ") {
        return parse_conditional_statement(address, statement, state, parsed);
    }

    Err(unsupported(address, statement))
}

fn timing_tracks_an_object_put_into_a_graveyard(timing: &Timing) -> bool {
    fn trigger_tracks_an_object_put_into_a_graveyard(trigger: &Trigger) -> bool {
        match trigger {
            Trigger::AnyOf(triggers) => {
                !triggers.is_empty()
                    && triggers
                        .iter()
                        .all(trigger_tracks_an_object_put_into_a_graveyard)
            }
            Trigger::OncePerTurn(trigger) => trigger_tracks_an_object_put_into_a_graveyard(trigger),
            Trigger::ObjectEvent {
                event: ObjectEventKind::Dies | ObjectEventKind::PutIntoGraveyardFromBattlefield,
                ..
            } => true,
            _ => false,
        }
    }

    matches!(timing, Timing::Triggered(trigger) if trigger_tracks_an_object_put_into_a_graveyard(trigger))
}

fn parse_optional_payment_statement(
    address: ClauseAddress,
    statement: &str,
    _state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = statement.trim().trim_end_matches('.').to_ascii_lowercase();
    if let Some(cost_text) = lower.strip_prefix("pay ") {
        let cost = if let Some(life_text) = cost_text.strip_suffix(" life") {
            Cost::PayLife(
                parse_english_amount(life_text).ok_or_else(|| unsupported(address, statement))?,
            )
        } else {
            parse_payment_cost(address, cost_text)?
        };
        parsed.effects.push(Effect::PayCost(cost));
        return Ok(true);
    }
    if let Some(subject) = lower.strip_prefix("sacrifice ") {
        let other_than_source = subject.starts_with("another ");
        let subject = subject.strip_prefix("another ").unwrap_or(subject).trim();
        let (amount, filter_text) =
            parse_amount_prefix(subject).unwrap_or((Amount::Constant(1), subject));
        let mut filter = parse_card_filter_phrase(filter_text)
            .or_else(|| parse_simple_event_object_filter(filter_text))
            .ok_or_else(|| unsupported(address, statement))?;
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(PlayerRef::You);
        if other_than_source {
            filter.other_than_source = true;
        }
        parsed
            .effects
            .push(Effect::PayCost(Cost::Sacrifice { amount, filter }));
        return Ok(true);
    }
    Ok(false)
}

fn parse_simple_sequence_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = statement.to_ascii_lowercase();
    for conjunction in [" and exile ", " and target opponent "] {
        let Some(index) = lower.find(conjunction) else {
            continue;
        };
        let first_lower = lower[..index].trim();
        if !(first_lower.starts_with("create ") || first_lower.starts_with("you draw ")) {
            continue;
        }
        let first = format!("{}.", statement[..index].trim_end_matches('.'));
        let continuation = statement[index + conjunction.len()..].trim_end_matches('.');
        let second = if conjunction.contains("exile") {
            format!("exile {continuation}.")
        } else {
            format!("target opponent {continuation}.")
        };
        let mut trial_state = state.clone();
        let mut trial_parsed = parsed.clone();
        if parse_effect_statement(address, &first, &mut trial_state, &mut trial_parsed).is_ok()
            && parse_effect_statement(address, &second, &mut trial_state, &mut trial_parsed).is_ok()
        {
            *state = trial_state;
            *parsed = trial_parsed;
            return Ok(true);
        }
    }
    if let Some(index) = lower.find(", then ") {
        let first = statement[..index].trim();
        let second = statement[index + ", then ".len()..].trim();
        if [
            "create ",
            "discard ",
            "draw ",
            "exile ",
            "put ",
            "return ",
            "scry ",
            "surveil ",
            "mill ",
            "tap ",
            "transform ",
            "untap ",
            "you discard ",
            "you draw ",
            "you gain ",
        ]
        .iter()
        .any(|prefix| first.to_ascii_lowercase().starts_with(prefix))
        {
            let mut trial_state = state.clone();
            let mut trial_parsed = parsed.clone();
            let first = format!("{}.", first.trim_end_matches('.'));
            let second = format!("{}.", second.trim_end_matches('.'));
            if parse_effect_statement(address, &first, &mut trial_state, &mut trial_parsed).is_ok()
                && parse_effect_statement(address, &second, &mut trial_state, &mut trial_parsed)
                    .is_ok()
            {
                *state = trial_state;
                *parsed = trial_parsed;
                return Ok(true);
            }
        }
    }

    for conjunction in [
        " and you lose ",
        " and lose ",
        " and draw ",
        " and you draw ",
    ] {
        let Some(index) = lower.find(conjunction) else {
            continue;
        };
        let first_lower = lower[..index].trim();
        if !(first_lower.starts_with("draw ")
            || first_lower.starts_with("you draw ")
            || first_lower.starts_with("gain ")
            || first_lower.starts_with("you gain ")
            || first_lower.starts_with("put "))
        {
            continue;
        }
        let first = format!("{}.", statement[..index].trim_end_matches('.'));
        let continuation = statement[index + conjunction.len()..].trim_end_matches('.');
        let second = if conjunction.contains("lose") {
            format!("you lose {continuation}.")
        } else {
            format!("you draw {continuation}.")
        };
        let mut trial_state = state.clone();
        let mut trial_parsed = parsed.clone();
        if parse_effect_statement(address, &first, &mut trial_state, &mut trial_parsed).is_ok()
            && parse_effect_statement(address, &second, &mut trial_state, &mut trial_parsed).is_ok()
        {
            *state = trial_state;
            *parsed = trial_parsed;
            return Ok(true);
        }
    }
    for index in find_top_level_phrases(statement, " and ").into_iter().rev() {
        let first = statement[..index].trim().trim_end_matches('.');
        let second = statement[index + " and ".len()..]
            .trim()
            .trim_end_matches('.');
        let starts_effect = |text: &str| {
            [
                "add ",
                "counter ",
                "create ",
                "destroy ",
                "discard ",
                "draw ",
                "each opponent ",
                "each player ",
                "exile ",
                "gain ",
                "its controller ",
                "lose ",
                "mill ",
                "pay ",
                "put ",
                "return ",
                "sacrifice ",
                "scry ",
                "surveil ",
                "tap ",
                "target opponent ",
                "target player ",
                "that player ",
                "that card ",
                "that creature ",
                "they ",
                "it ",
                "this artifact ",
                "this creature ",
                "this land ",
                "this object ",
                "this permanent ",
                "transform ",
                "untap ",
                "you ",
            ]
            .iter()
            .any(|prefix| text.to_ascii_lowercase().starts_with(prefix))
        };
        if !starts_effect(first) || !starts_effect(second) {
            continue;
        }
        let mut trial_state = state.clone();
        let mut trial_parsed = parsed.clone();
        let first = format!("{first}.");
        let second = format!("{second}.");
        if parse_effect_statement(address, &first, &mut trial_state, &mut trial_parsed).is_ok()
            && parse_effect_statement(address, &second, &mut trial_state, &mut trial_parsed).is_ok()
        {
            *state = trial_state;
            *parsed = trial_parsed;
            return Ok(true);
        }
    }
    Ok(false)
}

fn unsupported(address: ClauseAddress, text: &str) -> CompileError {
    CompileError::UnsupportedSyntax {
        address,
        normalized_clause: text.to_string(),
    }
}

fn compile_typed_mana_production(expression: TypedManaProductionExpression) -> ManaProduction {
    let choices = legacy_choices_for_typed_composition(&expression.composition);
    let commander_identity_only = matches!(
        &expression.composition,
        TypedManaComposition::Derived(TypedDerivedManaTypes::CommanderColorIdentity)
    );
    let amount = match &expression.quantity {
        TypedManaQuantity::Fixed(amount) => Amount::Constant(*amount),
        TypedManaQuantity::X { defined_as: None } => Amount::X,
        TypedManaQuantity::X {
            defined_as: Some(_),
        }
        | TypedManaQuantity::Calculated(_) => Amount::X,
    };
    ManaProduction {
        player: PlayerRef::You,
        choices,
        amount,
        commander_identity_only,
        scales_with: None,
        typed: Some(expression),
    }
}

fn legacy_choices_for_typed_composition(composition: &TypedManaComposition) -> Vec<ManaChoice> {
    match composition {
        TypedManaComposition::Exact(colors) => vec![ManaChoice {
            symbols: colors.iter().copied().map(runtime_mana_color).collect(),
        }],
        TypedManaComposition::OneOf(choices) => choices
            .iter()
            .map(|choice| ManaChoice {
                symbols: choice.iter().copied().map(runtime_mana_color).collect(),
            })
            .collect(),
        TypedManaComposition::Alternatives(alternatives) => alternatives
            .iter()
            .flat_map(legacy_choices_for_typed_composition)
            .collect(),
        TypedManaComposition::AnyOneColor => any_color_choices(),
        TypedManaComposition::AnyCombination(domain)
        | TypedManaComposition::DifferentColors(domain) => typed_domain_colors(domain)
            .into_iter()
            .map(|color| ManaChoice {
                symbols: vec![runtime_mana_color(color)],
            })
            .collect(),
        TypedManaComposition::Derived(TypedDerivedManaTypes::CommanderColorIdentity) => {
            any_color_choices()
        }
        TypedManaComposition::Derived(_) => Vec::new(),
    }
}

fn typed_domain_colors(domain: &TypedManaColorDomain) -> Vec<TypedManaColor> {
    match domain {
        TypedManaColorDomain::Colors => vec![
            TypedManaColor::White,
            TypedManaColor::Blue,
            TypedManaColor::Black,
            TypedManaColor::Red,
            TypedManaColor::Green,
        ],
        TypedManaColorDomain::ManaTypes => vec![
            TypedManaColor::White,
            TypedManaColor::Blue,
            TypedManaColor::Black,
            TypedManaColor::Red,
            TypedManaColor::Green,
            TypedManaColor::Colorless,
        ],
        TypedManaColorDomain::Explicit(colors) => colors.clone(),
    }
}

fn runtime_mana_color(color: TypedManaColor) -> Color {
    match color {
        TypedManaColor::White => Color::White,
        TypedManaColor::Blue => Color::Blue,
        TypedManaColor::Black => Color::Black,
        TypedManaColor::Red => Color::Red,
        TypedManaColor::Green => Color::Green,
        TypedManaColor::Colorless => Color::Colorless,
    }
}

fn parse_add_mana_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = statement.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("add ") else {
        return Ok(false);
    };
    let rest = rest.trim_end_matches('.');

    if let Some(fixed_text) = rest.strip_suffix(" or one mana of the chosen color") {
        let fixed =
            parse_mana_production_expression(&format!("Add {fixed_text}.")).map_err(|error| {
                CompileError::InvalidMana {
                    address,
                    text: error.to_string(),
                }
            })?;
        if fixed.quantity != TypedManaQuantity::Fixed(1)
            || !matches!(fixed.composition, TypedManaComposition::Exact(_))
            || fixed.spend_restriction.is_some()
            || fixed.retention != TypedManaRetention::Normal
        {
            return Err(unsupported(address, statement));
        }
        parsed
            .effects
            .push(Effect::AddMana(compile_typed_mana_production(
                TypedManaProductionExpression {
                    version: BOUNDED_ORACLE_MANA_EXPRESSION_VERSION,
                    quantity: TypedManaQuantity::Fixed(1),
                    composition: TypedManaComposition::Alternatives(vec![
                        fixed.composition,
                        TypedManaComposition::Derived(TypedDerivedManaTypes::ChosenColor),
                    ]),
                    derived_source_scope: None,
                    spend_restriction: None,
                    retention: TypedManaRetention::Normal,
                },
            )));
        return Ok(true);
    }

    if let Some(count_text) =
        rest.strip_prefix("x mana of any one color, where x is the number of ")
        && let Some(count) = parse_count_expression(count_text)
            .or_else(|| parse_mana_count_expression(count_text, state, parsed))
    {
        parsed.effects.push(Effect::AddMana(ManaProduction {
            player: PlayerRef::You,
            choices: any_color_choices(),
            amount: Amount::Count(Box::new(count)),
            commander_identity_only: false,
            scales_with: None,
            typed: None,
        }));
        return Ok(true);
    }

    let exact_expression = match rest {
        "x mana of any one color" => Some(TypedManaProductionExpression {
            version: BOUNDED_ORACLE_MANA_EXPRESSION_VERSION,
            quantity: TypedManaQuantity::X { defined_as: None },
            composition: TypedManaComposition::AnyOneColor,
            derived_source_scope: None,
            spend_restriction: None,
            retention: TypedManaRetention::Normal,
        }),
        "one mana of the chosen color" => Some(TypedManaProductionExpression {
            version: BOUNDED_ORACLE_MANA_EXPRESSION_VERSION,
            quantity: TypedManaQuantity::Fixed(1),
            composition: TypedManaComposition::Derived(TypedDerivedManaTypes::ChosenColor),
            derived_source_scope: None,
            spend_restriction: None,
            retention: TypedManaRetention::Normal,
        }),
        _ => None,
    };
    if let Some(expression) = exact_expression {
        parsed
            .effects
            .push(Effect::AddMana(compile_typed_mana_production(expression)));
        return Ok(true);
    }

    if let Ok(expression) = parse_mana_production_expression(statement) {
        parsed
            .effects
            .push(Effect::AddMana(compile_typed_mana_production(expression)));
        return Ok(true);
    }
    if let Some(symbol_text) = rest.strip_suffix(" for each card in target opponent's hand") {
        let target = state.allocate_target(
            TargetFilter::Player,
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let player = PlayerRef::TargetPlayer(target.id);
        parsed.targets.push(target);
        let mut filter = ObjectFilter::in_zone(Zone::Hand);
        filter.owner = Some(player.clone());
        parsed.effects.push(Effect::AddMana(ManaProduction {
            player: PlayerRef::You,
            choices: parse_mana_choices(address, symbol_text)?,
            amount: Amount::Count(Box::new(CountExpression::CardsInZone {
                player,
                zone: Zone::Hand,
                filter,
            })),
            commander_identity_only: false,
            scales_with: None,
            typed: None,
        }));
        return Ok(true);
    }
    if let Some((symbol_text, count_text)) = rest.split_once(" for each ")
        && let Some(count) = parse_mana_count_expression(count_text, state, parsed)
    {
        parsed.effects.push(Effect::AddMana(ManaProduction {
            player: PlayerRef::You,
            choices: parse_mana_choices(address, symbol_text)?,
            amount: Amount::Count(Box::new(count)),
            commander_identity_only: false,
            scales_with: None,
            typed: None,
        }));
        return Ok(true);
    }
    let choices = parse_mana_choices(address, rest)?;
    let amount = choices
        .first()
        .map(|choice| Amount::Constant(choice.symbols.len() as u32))
        .unwrap_or(Amount::Constant(0));
    if choices.is_empty()
        || !choices
            .iter()
            .all(|choice| choice.symbols.len() == amount_as_constant(&amount).unwrap_or(0) as usize)
    {
        return Err(unsupported(address, statement));
    }
    parsed.effects.push(Effect::AddMana(ManaProduction {
        player: PlayerRef::You,
        choices,
        amount,
        commander_identity_only: false,
        scales_with: None,
        typed: None,
    }));
    Ok(true)
}

fn parse_mana_count_expression(
    text: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Option<CountExpression> {
    let lower = text.trim().trim_end_matches('.').to_ascii_lowercase();
    if let Some(counter_name) = lower
        .strip_suffix(" counter on this object")
        .or_else(|| lower.strip_suffix(" counter on this permanent"))
    {
        return Some(CountExpression::CountersOn {
            object: ObjectRef::Source,
            counter: parse_counter_kind(counter_name),
        });
    }
    if let Some(subject) = lower.strip_suffix(" in your graveyard") {
        let subject = subject.strip_suffix(" card").unwrap_or(subject).trim();
        let mut filter = if subject.is_empty() || subject == "card" {
            ObjectFilter::default()
        } else {
            parse_card_filter_phrase(subject)
                .or_else(|| parse_simple_event_object_filter(subject))?
        };
        filter.zones = vec![Zone::Graveyard];
        filter.owner = Some(PlayerRef::You);
        return Some(CountExpression::CardsInZone {
            player: PlayerRef::You,
            zone: Zone::Graveyard,
            filter,
        });
    }
    if let Some(subject) = lower.strip_suffix(" in your hand") {
        let subject = subject.strip_suffix(" card").unwrap_or(subject).trim();
        let mut filter = if subject.is_empty() || subject == "card" {
            ObjectFilter::default()
        } else {
            parse_card_filter_phrase(subject)
                .or_else(|| parse_simple_event_object_filter(subject))?
        };
        filter.zones = vec![Zone::Hand];
        filter.owner = Some(PlayerRef::You);
        return Some(CountExpression::CardsInZone {
            player: PlayerRef::You,
            zone: Zone::Hand,
            filter,
        });
    }
    if let Some(subject) = lower.strip_suffix(" target opponent controls") {
        let target = state.allocate_target(
            TargetFilter::Player,
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let player = PlayerRef::TargetPlayer(target.id);
        let mut filter = parse_card_filter_phrase(subject)
            .or_else(|| parse_simple_event_object_filter(subject))?;
        filter.zones = vec![Zone::Battlefield];
        filter.controller = Some(player.clone());
        parsed.targets.push(target);
        return Some(CountExpression::MatchingObjects { player, filter });
    }
    let (subject, player, controller) = if let Some(subject) = lower.strip_suffix(" you control") {
        (subject, PlayerRef::You, Some(PlayerRef::You))
    } else {
        let subject = lower.strip_suffix(" on the battlefield")?;
        (subject, PlayerRef::Any, None)
    };
    let subject = subject.strip_suffix(" card").unwrap_or(subject).trim();
    let mut filter =
        parse_card_filter_phrase(subject).or_else(|| parse_simple_event_object_filter(subject))?;
    filter.zones = vec![Zone::Battlefield];
    filter.controller = controller;
    Some(CountExpression::MatchingObjects { player, filter })
}

fn parse_counted_amount(text: &str) -> Option<Amount> {
    let lower = text
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_ascii_lowercase();
    if matches!(lower.as_str(), "that much" | "that many") {
        return Some(Amount::Count(Box::new(CountExpression::TriggerEventAmount)));
    }
    if lower == "the sacrificed creature's toughness" {
        return Some(Amount::Count(Box::new(
            CountExpression::SelectedObjectsTotalToughness { selection_id: 0 },
        )));
    }
    if let Some(color) = lower
        .strip_prefix("your devotion to ")
        .and_then(parse_color_word)
    {
        return Some(Amount::Count(Box::new(CountExpression::Devotion {
            player: PlayerRef::You,
            color,
        })));
    }
    if let Some(inner) = lower.strip_prefix("twice ") {
        return Some(Amount::Product {
            factor: 2,
            value: Box::new(parse_counted_amount(inner)?),
        });
    }
    let count_text = lower
        .strip_prefix("the number of ")
        .or_else(|| lower.strip_prefix("a number of "))
        .or_else(|| lower.strip_prefix("number of "))?;
    parse_count_expression(count_text).map(|count| Amount::Count(Box::new(count)))
}

fn parse_count_expression(text: &str) -> Option<CountExpression> {
    let lower = text
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_ascii_lowercase();
    if let Some(counter_name) = lower
        .strip_suffix(" counters on this object")
        .or_else(|| lower.strip_suffix(" counter on this object"))
        .or_else(|| lower.strip_suffix(" counters on it"))
        .or_else(|| lower.strip_suffix(" counter on it"))
    {
        return Some(CountExpression::CountersOn {
            object: ObjectRef::Source,
            counter: parse_counter_kind(counter_name),
        });
    }

    for (suffix, player, zone) in [
        (" in your hand", PlayerRef::You, Zone::Hand),
        (" in your graveyard", PlayerRef::You, Zone::Graveyard),
        (" in all graveyards", PlayerRef::Any, Zone::Graveyard),
        (
            " in your opponents' graveyards",
            PlayerRef::Opponent,
            Zone::Graveyard,
        ),
        (
            " in an opponent's graveyard",
            PlayerRef::Opponent,
            Zone::Graveyard,
        ),
    ] {
        let Some(subject) = lower.strip_suffix(suffix) else {
            continue;
        };
        let subject = match subject {
            "card" | "cards" => "",
            _ => subject
                .strip_suffix(" cards")
                .or_else(|| subject.strip_suffix(" card"))
                .unwrap_or(subject)
                .trim(),
        };
        let mut filter = if subject.is_empty() {
            ObjectFilter::default()
        } else {
            parse_card_filter_phrase(subject)?
        };
        filter.zones = vec![zone];
        filter.owner = (!matches!(player, PlayerRef::Any)).then_some(player.clone());
        return Some(CountExpression::CardsInZone {
            player,
            zone,
            filter,
        });
    }

    let (subject, player, controller) = if let Some((subject, qualifier)) =
        lower.split_once(" you control")
        && (qualifier.is_empty() || qualifier.starts_with(" with "))
    {
        (
            format!("{subject}{qualifier}"),
            PlayerRef::You,
            Some(PlayerRef::You),
        )
    } else if let Some((subject, qualifier)) = lower.split_once(" your opponents control")
        && (qualifier.is_empty() || qualifier.starts_with(" with "))
    {
        (
            format!("{subject}{qualifier}"),
            PlayerRef::Opponent,
            Some(PlayerRef::Opponent),
        )
    } else {
        let subject = lower.strip_suffix(" on the battlefield")?;
        (subject.to_owned(), PlayerRef::Any, None)
    };
    let subject = subject
        .strip_suffix(" cards")
        .or_else(|| subject.strip_suffix(" card"))
        .unwrap_or(&subject)
        .trim();
    let mut filter = parse_card_filter_phrase(subject)?;
    filter.zones = vec![Zone::Battlefield];
    filter.controller = controller;
    Some(CountExpression::MatchingObjects { player, filter })
}

fn amount_as_constant(amount: &Amount) -> Option<u32> {
    match amount {
        Amount::Constant(value) => Some(*value),
        _ => None,
    }
}

fn parse_mana_choices(address: ClauseAddress, text: &str) -> Result<Vec<ManaChoice>, CompileError> {
    let normalized = text.replace(", or ", " or ").replace(", ", " or ");
    normalized
        .split(" or ")
        .map(|choice| {
            let mana = parse_mana_cost(address, choice.trim())?;
            let mut symbols = Vec::new();
            let mut cursor = 0usize;
            while cursor < mana.0.len() {
                let end = mana.0[cursor + 1..]
                    .find('}')
                    .map(|relative| cursor + 1 + relative)
                    .ok_or_else(|| CompileError::InvalidMana {
                        address,
                        text: mana.0.clone(),
                    })?;
                let symbol = &mana.0[cursor + 1..end];
                let color = match symbol {
                    "W" => Color::White,
                    "U" => Color::Blue,
                    "B" => Color::Black,
                    "R" => Color::Red,
                    "G" => Color::Green,
                    "C" => Color::Colorless,
                    _ => {
                        return Err(CompileError::InvalidMana {
                            address,
                            text: mana.0.clone(),
                        });
                    }
                };
                symbols.push(color);
                cursor = end + 1;
            }
            Ok(ManaChoice { symbols })
        })
        .collect()
}

fn parse_counter_destroy_return_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = statement.to_ascii_lowercase();
    if parse_general_zone_move_statement(address, statement, state, parsed)? {
        return Ok(true);
    }
    if matches!(
        lower.as_str(),
        "it can't be regenerated." | "they can't be regenerated."
    ) {
        let Some(effect) = parsed.effects.last_mut() else {
            return Err(unsupported(address, statement));
        };
        let Effect::Destroy { object } = effect else {
            return Err(unsupported(address, statement));
        };
        *effect = Effect::DestroyWithoutRegeneration {
            object: object.clone(),
        };
        return Ok(true);
    }
    if parse_collection_removal_statement(address, statement, state, parsed)? {
        return Ok(true);
    }
    for (verb, tapped) in [("tap up to ", true), ("untap up to ", false)] {
        let Some(selection_text) = lower
            .strip_prefix(verb)
            .and_then(|text| text.strip_suffix('.'))
        else {
            continue;
        };
        let Some((amount_text, filter_text)) = selection_text.split_once(' ') else {
            return Err(unsupported(address, statement));
        };
        let Some(Amount::Constant(amount)) = parse_english_amount(amount_text) else {
            return Err(unsupported(address, statement));
        };
        let amount = u16::try_from(amount).map_err(|_| unsupported(address, statement))?;
        if filter_text.starts_with("target ") {
            let mut target = parse_target_description(address, filter_text, state)?;
            target.amount = TargetAmount::UpTo(amount);
            let object = ObjectRef::Target(target.id);
            parsed.targets.push(target);
            state.last_object = Some(object.clone());
            parsed.effects.push(if tapped {
                Effect::Tap { object }
            } else {
                Effect::Untap { object }
            });
        } else {
            let mut filter = parse_card_filter_phrase(filter_text)
                .or_else(|| parse_simple_event_object_filter(filter_text))
                .ok_or_else(|| unsupported(address, statement))?;
            filter.zones = vec![Zone::Battlefield];
            let selection =
                state.allocate_selection(PlayerRef::You, filter, TargetAmount::UpTo(amount));
            parsed
                .effects
                .push(Effect::SetSelectedTapped { selection, tapped });
        }
        return Ok(true);
    }
    if let Some((target_text, payment_text)) = lower
        .strip_prefix("counter ")
        .and_then(|text| text.strip_suffix('.'))
        .and_then(|text| text.split_once(" unless its controller pays "))
    {
        let target = parse_target_description(address, target_text, state)?;
        let object = ObjectRef::Target(target.id);
        let cost = parse_payment_cost(address, payment_text)?;
        parsed.targets.push(target);
        state.last_object = Some(object.clone());
        parsed.conditions.push(Condition::UnlessPaid {
            player: PlayerRef::ControllerOf(Box::new(object.clone())),
            cost,
        });
        parsed.effects.push(Effect::Counter { object });
        return Ok(true);
    }
    if let Some(target_text) = lower
        .strip_prefix("copy ")
        .and_then(|text| text.strip_suffix('.'))
    {
        if target_text.contains("target ") {
            let target = parse_target_description(address, target_text, state)?;
            let object = ObjectRef::Target(target.id);
            parsed.targets.push(target);
            state.last_object = Some(object.clone());
            parsed.effects.push(Effect::CopyStackObject {
                object,
                may_choose_new_targets: false,
            });
            return Ok(true);
        }
        if matches!(target_text, "it" | "that spell") {
            let object = state
                .last_object
                .clone()
                .ok_or_else(|| unsupported(address, statement))?;
            parsed.effects.push(Effect::CopyStackObject {
                object,
                may_choose_new_targets: false,
            });
            return Ok(true);
        }
    }
    if let Some(exile_text) = lower
        .strip_prefix("exile ")
        .and_then(|text| text.strip_suffix('.'))
    {
        for (origin, owner) in [
            (" from an opponent's graveyard", Some(PlayerRef::Opponent)),
            (" from your graveyard", Some(PlayerRef::You)),
            (" from a graveyard", None),
            (" from the graveyard", None),
        ] {
            let Some(target_text) = exile_text.strip_suffix(origin) else {
                continue;
            };
            let mut target = parse_target_description(address, target_text, state)?;
            if !set_target_filter_zone(&mut target.filter, Zone::Graveyard)
                || owner.as_ref().is_some_and(|owner| {
                    !set_target_filter_owner(&mut target.filter, owner.clone())
                })
            {
                return Err(unsupported(address, statement));
            }
            let object = ObjectRef::Target(target.id);
            parsed.targets.push(target);
            state.last_object = Some(object.clone());
            parsed.effects.push(Effect::MoveZone(ZoneMove {
                object,
                from: Some(Zone::Graveyard),
                to: Zone::Exile,
                tapped: false,
                face_down: false,
                delayed_until: None,
            }));
            return Ok(true);
        }
    }
    if let Some(subject) = lower
        .strip_prefix("exile ")
        .and_then(|text| text.strip_suffix('.'))
        && matches!(
            subject,
            "it" | "them" | "that card" | "that creature" | "that permanent" | "that object"
        )
    {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: None,
            to: Zone::Exile,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        state.last_object = Some(object);
        return Ok(true);
    }
    for (verb, constructor) in [
        ("counter ", 0u8),
        ("destroy ", 1u8),
        ("exile ", 2u8),
        ("tap ", 3u8),
        ("untap ", 4u8),
    ] {
        if let Some(target_text) = lower
            .strip_prefix(verb)
            .and_then(|text| text.strip_suffix('.'))
        {
            if !target_text.contains("target ") {
                continue;
            }
            let target = parse_target_description(address, target_text, state)?;
            let target_zone = target_filter_single_zone(&target.filter);
            let object = ObjectRef::Target(target.id);
            parsed.targets.push(target);
            state.last_object = Some(object.clone());
            parsed.effects.push(match constructor {
                0 => Effect::Counter { object },
                1 => Effect::Destroy { object },
                2 => Effect::MoveZone(ZoneMove {
                    object,
                    from: Some(target_zone.unwrap_or(Zone::Battlefield)),
                    to: Zone::Exile,
                    tapped: false,
                    face_down: false,
                    delayed_until: None,
                }),
                3 => Effect::Tap { object },
                4 => Effect::Untap { object },
                _ => unreachable!(),
            });
            return Ok(true);
        }
    }
    for (verb, tapped) in [("tap ", true), ("untap ", false)] {
        let Some(subject) = lower
            .strip_prefix(verb)
            .and_then(|text| text.strip_suffix('.'))
        else {
            continue;
        };
        let object = if subject == "this object" {
            ObjectRef::Source
        } else if matches!(
            subject,
            "it" | "them" | "that creature" | "that permanent" | "each of them"
        ) {
            state
                .last_object
                .clone()
                .ok_or_else(|| unsupported(address, statement))?
        } else {
            continue;
        };
        state.last_object = Some(object.clone());
        parsed.effects.push(if tapped {
            Effect::Tap { object }
        } else {
            Effect::Untap { object }
        });
        return Ok(true);
    }
    if let Some(target_text) = lower
        .strip_prefix("return ")
        .and_then(|text| text.strip_suffix(" to its owner's hand."))
    {
        let target = parse_target_description(address, target_text, state)?;
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object,
            from: Some(Zone::Battlefield),
            to: Zone::Hand,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        return Ok(true);
    }
    if lower == "return this object to its owner's hand at the beginning of the next end step." {
        let object = ObjectRef::Source;
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object,
            from: Some(Zone::Battlefield),
            to: Zone::Hand,
            tapped: false,
            face_down: false,
            delayed_until: Some(Trigger::BeginningOfNextEndStep),
        }));
        return Ok(true);
    }
    if lower == "return all attacking creatures to their owner's hand." {
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.zones = vec![Zone::Battlefield];
        filter.attacking = Some(true);
        let object = ObjectRef::EachMatching(filter);
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object,
            from: Some(Zone::Battlefield),
            to: Zone::Hand,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        return Ok(true);
    }
    if lower == "return those creatures to their owners' hands." {
        if state.selected_targets.len() != 2 {
            return Err(unsupported(address, statement));
        }
        let object = ObjectRef::TargetSet(state.selected_targets.clone());
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object,
            from: Some(Zone::Battlefield),
            to: Zone::Hand,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        return Ok(true);
    }
    if lower == "choose two target creatures controlled by different players." {
        let filter = ObjectFilter::with_type(CardType::Creature);
        let first = state.allocate_target(
            TargetFilter::Object(filter.clone()),
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let second = state.allocate_target(
            TargetFilter::Object(filter),
            TargetAmount::Exactly(1),
            TargetRelationship::DifferentControllers,
        );
        parsed.targets.extend([first, second]);
        return Ok(true);
    }
    if lower == "prevent all combat damage that would be dealt this turn." {
        parsed.effects.push(Effect::PreventDamage {
            combat_only: true,
            amount: Amount::Any,
            duration: Duration::ThisTurn,
        });
        return Ok(true);
    }
    Ok(false)
}

fn parse_collection_removal_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = statement.trim().trim_end_matches('.').to_ascii_lowercase();
    let (verb, body) = if let Some(body) = lower.strip_prefix("exile all ") {
        ("exile", body)
    } else if let Some(body) = lower.strip_prefix("exile each ") {
        ("exile", body)
    } else if let Some(body) = lower.strip_prefix("destroy all ") {
        ("destroy", body)
    } else if let Some(body) = lower.strip_prefix("destroy each ") {
        ("destroy", body)
    } else {
        return Ok(false);
    };

    let (filter_text, zone, owner) = if let Some(filter) = body.strip_suffix(" from your graveyard")
    {
        (filter, Zone::Graveyard, Some(PlayerRef::You))
    } else if let Some(filter) = body.strip_suffix(" from all graveyards") {
        (filter, Zone::Graveyard, None)
    } else if let Some(filter) = body.strip_suffix(" from opponents' graveyards") {
        (filter, Zone::Graveyard, Some(PlayerRef::Opponent))
    } else if let Some(filter) = body.strip_suffix(" from target player's graveyard") {
        let target = state.allocate_target(
            TargetFilter::Player,
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let player = PlayerRef::TargetPlayer(target.id);
        parsed.targets.push(target);
        state.last_player = Some(player.clone());
        (filter, Zone::Graveyard, Some(player))
    } else {
        (body, Zone::Battlefield, None)
    };
    let filter_text = filter_text
        .strip_suffix(" cards")
        .or_else(|| filter_text.strip_suffix(" card"))
        .unwrap_or(filter_text)
        .trim();
    let mut filter = if filter_text.is_empty() {
        ObjectFilter::default()
    } else {
        parse_card_filter_phrase(filter_text)
            .or_else(|| parse_simple_event_object_filter(filter_text))
            .ok_or_else(|| unsupported(address, statement))?
    };
    filter.zones = vec![zone];
    filter.owner = owner;
    let objects = ObjectRef::EachMatching(filter);
    state.last_object = Some(objects.clone());
    parsed.effects.push(if verb == "destroy" {
        Effect::Destroy { object: objects }
    } else {
        Effect::MoveZone(ZoneMove {
            object: objects,
            from: Some(zone),
            to: Zone::Exile,
            tapped: false,
            face_down: false,
            delayed_until: None,
        })
    });
    Ok(true)
}

fn parse_general_zone_move_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = statement.trim().trim_end_matches('.').to_ascii_lowercase();
    const DELAYED_OWNER_RETURN_SUFFIX: &str =
        " to the battlefield under its owner's control at the beginning of the next end step";
    if lower == "at the beginning of the next end step, return that creature to its owner's hand" {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Battlefield),
            to: Zone::Hand,
            tapped: false,
            face_down: false,
            delayed_until: Some(Trigger::BeginningOfNextEndStep),
        }));
        state.last_object = Some(object);
        return Ok(true);
    }
    for (statement_text, destination, tapped) in [
        (
            "put a land card from among them into your hand",
            Zone::Hand,
            false,
        ),
        (
            "put a land card from among them onto the battlefield tapped",
            Zone::Battlefield,
            true,
        ),
    ] {
        if lower == statement_text {
            let mut predicate = ObjectFilter::with_type(CardType::Land);
            predicate.zones = vec![Zone::Library];
            parsed.effects.push(Effect::SelectFromLookedAt {
                player: PlayerRef::You,
                amount: Amount::Constant(1),
                predicate,
                reveal: false,
                face_down: false,
                tapped,
                destination,
            });
            return Ok(true);
        }
    }
    if lower == "put two of them into your hand and the rest into your graveyard" {
        parsed.effects.extend([
            Effect::SelectFromLookedAt {
                player: PlayerRef::You,
                amount: Amount::Constant(2),
                predicate: ObjectFilter::default(),
                reveal: false,
                face_down: false,
                tapped: false,
                destination: Zone::Hand,
            },
            Effect::PutRestOfLookedAt {
                player: PlayerRef::You,
                destination: Zone::Graveyard,
            },
        ]);
        return Ok(true);
    }
    if let Some(subject) = lower
        .strip_prefix("return ")
        .and_then(|body| body.strip_suffix(DELAYED_OWNER_RETURN_SUFFIX))
    {
        let object = if matches!(subject, "it" | "that card" | "that object") {
            state
                .last_object
                .clone()
                .unwrap_or(ObjectRef::TriggeringObject)
        } else {
            return Err(unsupported(address, statement));
        };
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::MoveZoneUnderControl {
            object: object.clone(),
            from: Zone::Exile,
            to: Zone::Battlefield,
            controller: PlayerRef::OwnerOf(Box::new(object)),
            tapped: false,
            face_down: false,
            delayed_until: Some(Trigger::BeginningOfNextEndStep),
        });
        return Ok(true);
    }
    if matches!(
        lower.as_str(),
        "return all attacking creatures to their owner's hand"
            | "return those creatures to their owners' hands"
            | "return this object to its owner's hand at the beginning of the next end step"
    ) {
        return Ok(false);
    }
    let (raw_body, verb) = if let Some(body) = lower.strip_prefix("return ") {
        (body, "return")
    } else if let Some(body) = lower.strip_prefix("put ") {
        (body, "put")
    } else {
        return Ok(false);
    };
    let (body, entry_counter) = split_zone_move_entry_counter(raw_body);
    let body = body.as_str();
    let destinations = if verb == "return" {
        [
            (" to its owner's hand", Zone::Hand, false),
            (" to their owners' hands", Zone::Hand, false),
            (" to your hand", Zone::Hand, false),
            (" to the battlefield tapped", Zone::Battlefield, true),
            (" to the battlefield", Zone::Battlefield, false),
        ]
        .as_slice()
    } else {
        [
            (" into its owner's hand", Zone::Hand, false),
            (" into their owners' hands", Zone::Hand, false),
            (" into your hand", Zone::Hand, false),
            (" onto the battlefield tapped", Zone::Battlefield, true),
            (" onto the battlefield", Zone::Battlefield, false),
        ]
        .as_slice()
    };
    let Some((subject_and_origin, destination, tapped)) =
        destinations.iter().find_map(|(suffix, zone, tapped)| {
            body.strip_suffix(suffix)
                .map(|subject| (subject.trim(), *zone, *tapped))
        })
    else {
        return Ok(false);
    };
    let (subject_text, from) =
        if let Some((subject, origin)) = subject_and_origin.rsplit_once(" from ") {
            let from = match origin.trim() {
                "your graveyard" | "a graveyard" | "the graveyard" => Zone::Graveyard,
                "your hand" | "a hand" | "the hand" => Zone::Hand,
                "exile" => Zone::Exile,
                "the battlefield" => Zone::Battlefield,
                "the command zone" => Zone::Command,
                _ => return Err(unsupported(address, statement)),
            };
            (subject.trim(), Some(from))
        } else {
            (subject_and_origin, None)
        };
    if let Some((amount, filter_text)) = parse_object_selection_prefix(subject_text) {
        if entry_counter.is_some() {
            return Err(unsupported(address, statement));
        }
        let mut filter = parse_simple_event_object_filter(filter_text)
            .or_else(|| parse_card_filter_phrase(filter_text))
            .ok_or_else(|| unsupported(address, statement))?;
        filter.zones = vec![from.unwrap_or(Zone::Battlefield)];
        if filter_text.contains("you control") {
            filter.controller = Some(PlayerRef::You);
        }
        if filter_text.contains("you own")
            || matches!(from, Some(Zone::Hand | Zone::Graveyard | Zone::Command))
        {
            filter.owner = Some(PlayerRef::You);
        }
        let selection = state.allocate_selection(PlayerRef::You, filter, amount);
        parsed.effects.push(Effect::MoveSelected(SelectedZoneMove {
            selection,
            to: destination,
            tapped,
            face_down: false,
        }));
        return Ok(true);
    }
    let object = if subject_text == "this object" {
        ObjectRef::Source
    } else if matches!(subject_text, "it" | "that card" | "that object") {
        state
            .last_object
            .clone()
            .unwrap_or(ObjectRef::TriggeringObject)
    } else if subject_text.contains("target ") {
        let mut target = parse_target_description(address, subject_text, state)?;
        if let Some(from) = from
            && !set_target_filter_zone(&mut target.filter, from)
        {
            return Err(unsupported(address, statement));
        }
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        object
    } else {
        return Err(unsupported(address, statement));
    };
    let from = from.or_else(|| {
        (verb == "return" && matches!(object, ObjectRef::Source) && destination == Zone::Hand)
            .then_some(Zone::Battlefield)
    });
    state.last_object = Some(object.clone());
    parsed.effects.push(Effect::MoveZone(ZoneMove {
        object: object.clone(),
        from,
        to: destination,
        tapped,
        face_down: false,
        delayed_until: None,
    }));
    if let Some((amount, counter)) = entry_counter {
        if destination != Zone::Battlefield {
            return Err(unsupported(address, statement));
        }
        parsed.effects.push(Effect::PutCounter {
            object,
            counter,
            amount,
        });
    }
    Ok(true)
}

fn parse_object_selection_prefix(text: &str) -> Option<(TargetAmount, &str)> {
    for (prefix, amount) in [
        ("all ", TargetAmount::All),
        ("up to two ", TargetAmount::UpTo(2)),
        ("up to one ", TargetAmount::UpTo(1)),
        ("one ", TargetAmount::Exactly(1)),
        ("a ", TargetAmount::Exactly(1)),
        ("an ", TargetAmount::Exactly(1)),
    ] {
        if let Some(filter) = text.strip_prefix(prefix) {
            return Some((amount, filter.trim()));
        }
    }
    None
}

fn split_zone_move_entry_counter(body: &str) -> (String, Option<(Amount, CounterKind)>) {
    let Some((core, suffix)) = body.rsplit_once(" with ") else {
        return (body.to_owned(), None);
    };
    let Some(counter_text) = suffix
        .strip_suffix(" counters on it")
        .or_else(|| suffix.strip_suffix(" counter on it"))
    else {
        return (body.to_owned(), None);
    };
    let normalized = if let Some(rest) = counter_text.strip_prefix("an additional ") {
        format!("an {rest}")
    } else if let Some(rest) = counter_text.strip_prefix("a additional ") {
        format!("a {rest}")
    } else if let Some((amount, name)) = counter_text.split_once(" additional ") {
        format!("{amount} {name}")
    } else {
        counter_text.to_owned()
    };
    let Some((amount, counter_name)) = parse_counter_amount_and_name(&normalized) else {
        return (body.to_owned(), None);
    };
    (
        core.trim().to_owned(),
        Some((amount, parse_counter_kind(counter_name))),
    )
}

fn parse_target_description(
    address: ClauseAddress,
    text: &str,
    state: &mut EffectParseState,
) -> Result<Target, CompileError> {
    let mut lower = text.trim().trim_end_matches('.').to_ascii_lowercase();
    let amount = if let Some(rest) = lower.strip_prefix("up to ") {
        let (amount_text, description) = rest
            .split_once(' ')
            .ok_or_else(|| unsupported(address, text))?;
        let amount = parse_english_amount(amount_text)
            .and_then(|amount| match amount {
                Amount::Constant(value) => u16::try_from(value).ok(),
                _ => None,
            })
            .filter(|amount| *amount > 0)
            .ok_or_else(|| unsupported(address, text))?;
        lower = description.to_string();
        TargetAmount::UpTo(amount)
    } else {
        TargetAmount::Exactly(1)
    };
    let relationship = if lower.contains("other target") {
        TargetRelationship::OtherThan(ObjectRef::Source)
    } else {
        TargetRelationship::Independent
    };
    let Some(target_index) = lower.find("target ") else {
        return Err(unsupported(address, text));
    };
    let description = lower[target_index + "target ".len()..].trim();
    let (description, mana_value) =
        if let Some((core, value)) = description.split_once(" with mana value ") {
            let (comparison, amount) =
                parse_filter_comparison(value).ok_or_else(|| unsupported(address, text))?;
            (core.trim(), Some((comparison, Box::new(amount))))
        } else {
            (description, None)
        };
    let reviewed_filter_modifier = description.ends_with(" without flying")
        || description.ends_with(" with a +1/+1 counter on it");
    if !reviewed_filter_modifier
        && [
            " without ",
            " named ",
            " of your choice",
            " that ",
            " with a ",
            " with an ",
        ]
        .iter()
        .any(|marker| format!(" {description} ").contains(marker))
    {
        return Err(unsupported(address, text));
    }
    if description == "player" {
        return Ok(state.allocate_target(TargetFilter::Player, amount, relationship));
    }
    let disjunction_probe = description
        .replace(" or less", "")
        .replace(" or fewer", "")
        .replace(" or greater", "")
        .replace(" or more", "");
    if disjunction_probe.contains(" or ") {
        let filters = parse_disjunctive_target_filters(description)
            .ok_or_else(|| unsupported(address, text))?;
        return Ok(state.allocate_target(TargetFilter::Any(filters), amount, relationship));
    }
    let mut filter = if description == "card" {
        ObjectFilter::default()
    } else {
        parse_card_filter_phrase(description).ok_or_else(|| unsupported(address, text))?
    };
    filter.mana_value = mana_value;
    let is_spell = words(description)
        .iter()
        .any(|word| matches!(word.as_str(), "spell" | "spells"));
    Ok(state.allocate_target(
        if is_spell {
            TargetFilter::Spell(filter)
        } else {
            TargetFilter::Object(filter)
        },
        amount,
        relationship,
    ))
}

fn parse_disjunctive_target_filters(description: &str) -> Option<Vec<TargetFilter>> {
    if description == "attacking or blocking creature" {
        let mut attacking = ObjectFilter::with_type(CardType::Creature);
        attacking.attacking = Some(true);
        let mut blocking = ObjectFilter::with_type(CardType::Creature);
        blocking.blocking = Some(true);
        return Some(vec![
            TargetFilter::Object(attacking),
            TargetFilter::Object(blocking),
        ]);
    }
    let (description, zone, owner) =
        if let Some(core) = description.strip_suffix(" in your graveyard") {
            (core, Some(Zone::Graveyard), Some(PlayerRef::You))
        } else if let Some(core) = description.strip_suffix(" in a graveyard") {
            (core, Some(Zone::Graveyard), None)
        } else if let Some(core) = description.strip_suffix(" in exile") {
            (core, Some(Zone::Exile), None)
        } else {
            (description, None, None)
        };
    let description_is_spell = words(description)
        .iter()
        .any(|word| matches!(word.as_str(), "spell" | "spells"));
    let (description, controller) =
        if let Some(core) = description.strip_suffix(" an opponent controls") {
            (core, Some(PlayerRef::Opponent))
        } else if let Some(core) = description.strip_suffix(" you control") {
            (core, Some(PlayerRef::You))
        } else {
            (description, None)
        };
    let normalized = description.replace(", or ", ", ").replace(" or ", ", ");
    let alternatives = normalized
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if alternatives.len() < 2 {
        return None;
    }

    let allowed_words = [
        "a",
        "an",
        "another",
        "artifact",
        "artifacts",
        "battle",
        "battles",
        "basic",
        "card",
        "cards",
        "creature",
        "creatures",
        "enchantment",
        "enchantments",
        "instant",
        "instants",
        "land",
        "lands",
        "legendary",
        "nonbasic",
        "noncreature",
        "nontoken",
        "other",
        "permanent",
        "permanents",
        "planeswalker",
        "planeswalkers",
        "snow",
        "sorcery",
        "sorceries",
        "spell",
        "spells",
        "token",
        "vehicle",
        "vehicles",
        "with",
        "flying",
    ];
    let mut filters = Vec::with_capacity(alternatives.len());
    for alternative in alternatives {
        if words(alternative)
            .iter()
            .any(|word| !allowed_words.contains(&word.as_str()))
        {
            return None;
        }
        let mut filter = if alternative == "creature with flying" {
            let mut filter = ObjectFilter::with_type(CardType::Creature);
            filter.keywords = vec![Keyword::Flying];
            filter
        } else {
            parse_card_filter_phrase(alternative)?
        };
        filter.controller = controller.clone();
        if let Some(zone) = zone {
            filter.zones = vec![zone];
        }
        filter.owner = owner.clone();
        filters.push(if description_is_spell {
            TargetFilter::Spell(filter)
        } else {
            TargetFilter::Object(filter)
        });
    }
    Some(filters)
}

fn set_target_filter_zone(filter: &mut TargetFilter, zone: Zone) -> bool {
    match filter {
        TargetFilter::Player | TargetFilter::Opponent => false,
        TargetFilter::Object(filter) | TargetFilter::Spell(filter) => {
            filter.zones = vec![zone];
            true
        }
        TargetFilter::Any(filters) => {
            !filters.is_empty()
                && filters
                    .iter_mut()
                    .all(|filter| set_target_filter_zone(filter, zone))
        }
        TargetFilter::Conditional {
            if_true, if_false, ..
        } => set_target_filter_zone(if_true, zone) && set_target_filter_zone(if_false, zone),
    }
}

fn target_filter_single_zone(filter: &TargetFilter) -> Option<Zone> {
    match filter {
        TargetFilter::Player | TargetFilter::Opponent => None,
        TargetFilter::Object(filter) | TargetFilter::Spell(filter) => {
            let [zone] = filter.zones.as_slice() else {
                return None;
            };
            Some(*zone)
        }
        TargetFilter::Any(filters) => {
            let mut zones = filters.iter().map(target_filter_single_zone);
            let first = zones.next()??;
            zones.all(|zone| zone == Some(first)).then_some(first)
        }
        TargetFilter::Conditional {
            if_true, if_false, ..
        } => {
            let first = target_filter_single_zone(if_true)?;
            (target_filter_single_zone(if_false) == Some(first)).then_some(first)
        }
    }
}

fn set_target_filter_owner(filter: &mut TargetFilter, owner: PlayerRef) -> bool {
    match filter {
        TargetFilter::Player | TargetFilter::Opponent => false,
        TargetFilter::Object(filter) | TargetFilter::Spell(filter) => {
            filter.owner = Some(owner);
            true
        }
        TargetFilter::Any(filters) => {
            !filters.is_empty()
                && filters
                    .iter_mut()
                    .all(|filter| set_target_filter_owner(filter, owner.clone()))
        }
        TargetFilter::Conditional {
            if_true, if_false, ..
        } => {
            set_target_filter_owner(if_true, owner.clone())
                && set_target_filter_owner(if_false, owner)
        }
    }
}

fn parse_search_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = statement.to_ascii_lowercase();
    let (player, optional, search_body) = if let Some(body) =
        lower.strip_prefix("search your library for ")
    {
        (PlayerRef::You, false, body)
    } else if let Some(body) = lower.strip_prefix("you may search your library for ") {
        (PlayerRef::You, true, body)
    } else if let Some(body) = lower.strip_prefix("that player may search their library for ") {
        let player = state
            .last_object
            .clone()
            .map(|object| PlayerRef::ControllerOf(Box::new(object)))
            .unwrap_or(PlayerRef::ThatPlayer);
        (player, true, body)
    } else if let Some(body) = lower.strip_prefix("its controller may search their library for ") {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        (PlayerRef::ControllerOf(Box::new(object)), true, body)
    } else {
        return Ok(false);
    };
    let Some((selection_text, sequence_text)) = split_search_selection_and_sequence(search_body)
    else {
        return Err(unsupported(address, statement));
    };
    let (amount, predicate_text) =
        parse_search_amount(selection_text).ok_or_else(|| unsupported(address, statement))?;
    let mut predicate =
        parse_search_filter(predicate_text).ok_or_else(|| unsupported(address, statement))?;
    let allow_fail_to_find =
        predicate != ObjectFilter::default() || matches!(amount, Amount::UpTo(_) | Amount::Any);
    predicate.zones = vec![Zone::Library];

    let sequence = sequence_text.trim();
    let reveal = sequence.starts_with("reveal those cards,")
        || sequence.starts_with("reveal that card,")
        || sequence.starts_with("reveal them,")
        || sequence.starts_with("reveal it,");
    let movement = sequence
        .strip_prefix("reveal those cards,")
        .or_else(|| sequence.strip_prefix("reveal that card,"))
        .or_else(|| sequence.strip_prefix("reveal them,"))
        .or_else(|| sequence.strip_prefix("reveal it,"))
        .unwrap_or(sequence)
        .trim()
        .strip_prefix("and ")
        .unwrap_or_else(|| {
            sequence
                .strip_prefix("reveal those cards,")
                .or_else(|| sequence.strip_prefix("reveal that card,"))
                .or_else(|| sequence.strip_prefix("reveal them,"))
                .or_else(|| sequence.strip_prefix("reveal it,"))
                .unwrap_or(sequence)
                .trim()
        })
        .trim();
    let shuffle_before_destination = matches!(
        movement,
        "then shuffle and put that card on top."
            | "then shuffle your library and put that card on top."
            | "shuffle and put that card on top."
            | "shuffle your library and put that card on top."
            | "then shuffle and put it on top."
            | "then shuffle your library and put it on top."
            | "shuffle and put it on top."
            | "shuffle your library and put it on top."
    );
    if shuffle_before_destination {
        parsed.effects.push(Effect::SearchLibrary(SearchLibrary {
            player: player.clone(),
            chooser: player.clone(),
            optional,
            allow_fail_to_find,
            amount,
            predicate,
            reveal,
            destinations: vec![SearchDestination {
                selected_ordinal: SearchOrdinal::Each,
                zone: Zone::Library,
                tapped: false,
            }],
            shuffle_before_destination: true,
            shuffle_after: false,
        }));
        state.last_object = Some(ObjectRef::SearchedCard(0));
        state.last_player = Some(player);
        return Ok(true);
    }
    let shuffle_after =
        movement.ends_with("then shuffle.") || movement.ends_with("then shuffle your library.");
    let movement = movement
        .strip_suffix(", then shuffle.")
        .or_else(|| movement.strip_suffix(", then shuffle your library."))
        .unwrap_or(movement)
        .trim();
    let destinations =
        parse_search_destinations(movement).ok_or_else(|| unsupported(address, statement))?;
    let chooser = player.clone();
    parsed.effects.push(Effect::SearchLibrary(SearchLibrary {
        player: player.clone(),
        chooser,
        optional,
        allow_fail_to_find,
        amount,
        predicate,
        reveal,
        destinations,
        shuffle_before_destination: false,
        shuffle_after,
    }));
    state.last_object = Some(ObjectRef::SearchedCard(0));
    state.last_player = Some(player);
    Ok(true)
}

fn split_search_selection_and_sequence(search_body: &str) -> Option<(&str, &str)> {
    [
        ", reveal those cards,",
        ", reveal that card,",
        ", reveal them,",
        ", reveal it,",
        ", put ",
        ", exile ",
        ", then shuffle",
        ", shuffle",
    ]
    .into_iter()
    .filter_map(|marker| {
        search_body.find(marker).map(|index| {
            (
                index,
                search_body[..index].trim(),
                search_body[index + 1..].trim(),
            )
        })
    })
    .min_by_key(|(index, _, _)| *index)
    .map(|(_, selection, sequence)| (selection, sequence))
}

fn parse_search_amount(selection: &str) -> Option<(Amount, &str)> {
    for (prefix, maximum) in [
        ("up to one ", 1),
        ("up to two ", 2),
        ("up to three ", 3),
        ("up to four ", 4),
    ] {
        if let Some(predicate) = selection.strip_prefix(prefix) {
            return Some((Amount::UpTo(Box::new(Amount::Constant(maximum))), predicate));
        }
    }
    selection
        .strip_prefix("any number of ")
        .map_or(Some((Amount::Constant(1), selection)), |predicate| {
            Some((Amount::Any, predicate))
        })
}

fn parse_search_filter(text: &str) -> Option<ObjectFilter> {
    let raw = text
        .trim()
        .trim_matches(|character: char| matches!(character, '.' | ','))
        .to_ascii_lowercase();
    if matches!(raw.as_str(), "a card" | "card" | "one card") {
        return Some(ObjectFilter::default());
    }

    let (description, mana_value) = if let Some((description, amount_text)) =
        raw.rsplit_once(" with mana value ")
    {
        let (comparison, amount_text) = if let Some(amount) = amount_text.strip_suffix(" or less") {
            (Comparison::AtMost, amount)
        } else if let Some(amount) = amount_text.strip_suffix(" or greater") {
            (Comparison::AtLeast, amount)
        } else {
            (Comparison::Exactly, amount_text)
        };
        let amount = parse_english_amount(amount_text)?;
        (description.trim(), Some((comparison, Box::new(amount))))
    } else {
        (raw.as_str(), None)
    };

    if matches!(
        description,
        "a card with a basic land type"
            | "a land card with a basic land type"
            | "land card with a basic land type"
    ) {
        return Some(ObjectFilter {
            card_types: vec![CardType::Land],
            mana_value,
            ..Default::default()
        });
    }

    for prefix in [
        "a card named ",
        "card named ",
        "cards named ",
        "one card named ",
    ] {
        if let Some(name) = description.strip_prefix(prefix) {
            let name = name.trim();
            if name.is_empty()
                || name.contains(" and/or ")
                || name.contains(" with ")
                || name.contains(" that ")
            {
                return None;
            }
            return Some(ObjectFilter {
                names: vec![name.to_owned()],
                mana_value,
                ..Default::default()
            });
        }
    }
    if description.contains(" named ")
        || description.contains("not named")
        || description.contains("same name")
        || description.contains("different name")
        || description.contains("chosen name")
        || description.contains("you noted")
        || description.contains("could enchant")
        || description.contains(" with ")
        || description.contains(" that ")
        || description.contains('-')
    {
        return None;
    }

    let lower = description
        .strip_prefix("a ")
        .or_else(|| description.strip_prefix("an "))
        .or_else(|| description.strip_prefix("one "))
        .unwrap_or(description)
        .trim_end_matches(" card")
        .trim_end_matches(" cards")
        .to_string();
    if lower.contains(" card ") || lower.is_empty() {
        return None;
    }

    let lexemes = words(&lower);
    let mut filter = ObjectFilter::default();
    filter.mana_value = mana_value;
    if lower == "basic land type" || lower == "a basic land type" {
        filter.card_types.push(CardType::Land);
        return Some(filter);
    }
    if lower == "basic land" {
        filter.card_types.push(CardType::Land);
        filter.supertypes.push(Supertype::Basic);
        return Some(filter);
    }
    let types = [
        ("artifact", CardType::Artifact),
        ("battle", CardType::Battle),
        ("creature", CardType::Creature),
        ("enchantment", CardType::Enchantment),
        ("instant", CardType::Instant),
        ("land", CardType::Land),
        ("permanent", CardType::Permanent),
        ("planeswalker", CardType::Planeswalker),
        ("sorcery", CardType::Sorcery),
    ];
    let mut recognized = 0usize;
    for (word, card_type) in types {
        if lexemes.iter().any(|candidate| candidate == word) {
            filter.card_types.push(card_type);
            recognized += 1;
        }
    }
    for (word, supertype) in [
        ("basic", Supertype::Basic),
        ("legendary", Supertype::Legendary),
        ("snow", Supertype::Snow),
    ] {
        if lexemes.iter().any(|candidate| candidate == word) {
            filter.supertypes.push(supertype);
            recognized += 1;
        }
    }

    let ignored = ["a", "an", "and", "or"];
    let known_type_words = [
        "artifact",
        "battle",
        "creature",
        "enchantment",
        "instant",
        "land",
        "permanent",
        "planeswalker",
        "sorcery",
    ];
    let known_supertype_words = ["basic", "legendary", "snow"];
    for lexeme in &lexemes {
        if ignored.contains(&lexeme.as_str())
            || known_type_words.contains(&lexeme.as_str())
            || known_supertype_words.contains(&lexeme.as_str())
        {
            continue;
        }
        if lexeme.starts_with("non") || lexeme == "colorless" || lexeme == "multicolored" {
            return None;
        }
        filter.subtypes.push(title_case(lexeme));
        recognized += 1;
    }

    let disjunctive = lower.contains(" or ") || lower.contains(", ");
    if disjunctive {
        filter.card_type_match_any = filter.card_types.len() > 1;
        filter.subtype_match_any = filter.subtypes.len() > 1;
        if !filter.card_types.is_empty() && !filter.subtypes.is_empty() {
            let only_basic_land_subtypes = filter.card_types == vec![CardType::Land]
                && filter.subtypes.iter().all(|subtype| {
                    ["Plains", "Island", "Swamp", "Mountain", "Forest"].contains(&subtype.as_str())
                });
            if !only_basic_land_subtypes {
                return None;
            }
        }
    }
    if !filter.subtypes.is_empty()
        && filter.subtypes.iter().all(|subtype| {
            ["Plains", "Island", "Swamp", "Mountain", "Forest"].contains(&subtype.as_str())
        })
        && !filter.card_types.contains(&CardType::Land)
    {
        filter.card_types.push(CardType::Land);
    }
    filter.card_types.sort_by_key(|kind| format!("{kind:?}"));
    filter.card_types.dedup();
    filter.supertypes.sort_by_key(|kind| format!("{kind:?}"));
    filter.supertypes.dedup();
    filter.subtypes.sort();
    filter.subtypes.dedup();
    (recognized > 0).then_some(filter)
}

fn parse_search_destinations(text: &str) -> Option<Vec<SearchDestination>> {
    let lower = text
        .trim()
        .trim_end_matches('.')
        .strip_prefix("and ")
        .unwrap_or(text.trim().trim_end_matches('.'))
        .to_ascii_lowercase();
    if lower == "put one onto the battlefield tapped and the other into your hand" {
        return Some(vec![
            SearchDestination {
                selected_ordinal: SearchOrdinal::First,
                zone: Zone::Battlefield,
                tapped: true,
            },
            SearchDestination {
                selected_ordinal: SearchOrdinal::Other,
                zone: Zone::Hand,
                tapped: false,
            },
        ]);
    }
    let subjects = [
        "it",
        "that card",
        "those cards",
        "them",
        "the card",
        "the cards",
    ];
    let mut destination = None;
    for subject in subjects {
        destination = [
            (
                format!("put {subject} onto the battlefield tapped"),
                Zone::Battlefield,
                true,
            ),
            (
                format!("put {subject} onto the battlefield"),
                Zone::Battlefield,
                false,
            ),
            (format!("put {subject} into your hand"), Zone::Hand, false),
            (
                format!("put {subject} into your graveyard"),
                Zone::Graveyard,
                false,
            ),
            (
                format!("put {subject} on top of your library"),
                Zone::Library,
                false,
            ),
            (format!("exile {subject}"), Zone::Exile, false),
        ]
        .into_iter()
        .find_map(|(candidate, zone, tapped)| (lower == candidate).then_some((zone, tapped)));
        if destination.is_some() {
            break;
        }
    }
    let (zone, tapped) = destination?;
    Some(vec![SearchDestination {
        selected_ordinal: SearchOrdinal::Each,
        zone,
        tapped,
    }])
}

fn parsed_has_disallowed_predefined_token_context(parsed: &ParsedClause, body: &str) -> bool {
    if parsed
        .predefined_token_creations
        .iter()
        .any(|creation| creation.kind.requires_fixed_amount())
    {
        let lower = body.to_ascii_lowercase();
        if [
            "sacrifice it at the beginning ",
            "sacrifice them at the beginning ",
            "exile it at the beginning ",
            "exile them at the beginning ",
        ]
        .iter()
        .any(|phrase| lower.contains(phrase))
        {
            return true;
        }
    }
    effects_have_disallowed_predefined_token_context(&parsed.effects, true)
}

fn effects_have_disallowed_predefined_token_context(
    effects: &[Effect],
    direct_creation_allowed: bool,
) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::CreateToken(creation) => {
            !direct_creation_allowed && creation_has_new_predefined_token(creation)
        }
        Effect::CreateTokenWithDelayedMove { creation, .. } => {
            creation_has_new_predefined_token(creation)
        }
        Effect::Optional(effects) => {
            effects_have_disallowed_predefined_token_context(effects, direct_creation_allowed)
        }
        Effect::Conditional {
            if_true, if_false, ..
        } => {
            effects_have_disallowed_predefined_token_context(if_true, direct_creation_allowed)
                || effects_have_disallowed_predefined_token_context(
                    if_false,
                    direct_creation_allowed,
                )
        }
        Effect::GrantAbility { ability, .. } => {
            effects_have_disallowed_predefined_token_context(&ability.effects, false)
        }
        Effect::Copy(copy) => copy_has_disallowed_predefined_token_context(copy),
        Effect::Replacement(replacement) => match replacement.as_ref() {
            ReplacementEffect::ConditionalTokenSubstitution {
                ordinary,
                replacement,
                ..
            } => {
                creation_has_new_predefined_token(ordinary)
                    || creation_has_new_predefined_token(replacement)
            }
            ReplacementEffect::EnterAsCopy(copy) => {
                copy_has_disallowed_predefined_token_context(copy)
            }
            ReplacementEffect::MultiplyEvent { .. }
            | ReplacementEffect::IncreaseEvent { .. }
            | ReplacementEffect::EntersTapped(_) => false,
        },
        _ => false,
    })
}

fn copy_has_disallowed_predefined_token_context(copy: &CopyEffect) -> bool {
    copy.exceptions.iter().any(|exception| {
        matches!(
            exception,
            CopyException::AddGrantedAbility(GrantedAbility { effects, .. })
                if effects_have_disallowed_predefined_token_context(effects, false)
        )
    })
}

fn creation_has_new_predefined_token(creation: &TokenCreation) -> bool {
    let TokenSpecification::Defined(definition) = &creation.specification else {
        return false;
    };
    predefined_artifact_token_kind(definition)
        .is_some_and(PredefinedArtifactTokenKind::requires_fixed_amount)
}

fn predefined_artifact_token_kind(
    definition: &TokenDefinition,
) -> Option<PredefinedArtifactTokenKind> {
    [
        PredefinedArtifactTokenKind::Treasure,
        PredefinedArtifactTokenKind::Food,
        PredefinedArtifactTokenKind::Clue,
        PredefinedArtifactTokenKind::Blood,
        PredefinedArtifactTokenKind::Gold,
        PredefinedArtifactTokenKind::Lander,
    ]
    .into_iter()
    .find(|kind| definition == &kind.definition())
}

fn parse_token_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = statement.to_ascii_lowercase();
    if let Some(creation_text) = lower
        .strip_prefix("you and target opponent each create ")
        .and_then(|text| text.strip_suffix('.'))
    {
        let target = state.allocate_target(
            TargetFilter::Player,
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let opponent = PlayerRef::TargetPlayer(target.id);
        parsed.targets.push(target);
        state.last_player = Some(opponent.clone());
        let (amount, specification_text, number) =
            parse_token_amount(creation_text).ok_or_else(|| unsupported(address, statement))?;
        let parsed_specification =
            parse_token_specification(address, specification_text, number, state, parsed)?;
        record_predefined_token_creation(
            address,
            statement,
            &amount,
            parsed_specification.predefined_kind,
            number,
            parsed,
        )?;
        let specification = parsed_specification.specification;
        for player in [PlayerRef::You, opponent] {
            parsed.effects.push(Effect::CreateToken(TokenCreation {
                player,
                amount: amount.clone(),
                specification: specification.clone(),
                tapped: false,
                attacking: false,
            }));
        }
        return Ok(true);
    }
    let (player, amount_override, creation_text) =
        if let Some(rest) = lower.strip_prefix("for each token you control, create ") {
            let filter = ObjectFilter {
                zones: vec![Zone::Battlefield],
                controller: Some(PlayerRef::You),
                token: Some(true),
                ..Default::default()
            };
            (
                PlayerRef::You,
                Some(Amount::Count(Box::new(CountExpression::MatchingObjects {
                    player: PlayerRef::You,
                    filter,
                }))),
                rest,
            )
        } else if let Some(rest) = lower.strip_prefix("target player creates ") {
            let target = state.allocate_target(
                TargetFilter::Player,
                TargetAmount::Exactly(1),
                TargetRelationship::Independent,
            );
            let player = PlayerRef::TargetPlayer(target.id);
            parsed.targets.push(target);
            state.last_player = Some(player.clone());
            (player, None, rest)
        } else if let Some(rest) = lower.strip_prefix("its controller creates ") {
            let object = state
                .last_object
                .clone()
                .ok_or_else(|| unsupported(address, statement))?;
            (PlayerRef::ControllerOf(Box::new(object)), None, rest)
        } else if let Some(rest) = lower.strip_prefix("that player creates ") {
            let player = state
                .last_player
                .clone()
                .ok_or_else(|| unsupported(address, statement))?;
            (player, None, rest)
        } else if let Some(rest) = lower.strip_prefix("each opponent creates ") {
            (PlayerRef::Opponent, None, rest)
        } else if let Some(rest) = lower.strip_prefix("each player creates ") {
            (PlayerRef::Any, None, rest)
        } else if let Some(rest) = lower.strip_prefix("you create ") {
            (PlayerRef::You, None, rest)
        } else if let Some(rest) = lower.strip_prefix("create ") {
            (PlayerRef::You, None, rest)
        } else {
            return Ok(false);
        };

    let creation_text = match creation_text.strip_suffix('.') {
        Some(text) if text.ends_with('.') => return Err(unsupported(address, statement)),
        Some(text) => text,
        None => creation_text,
    };
    let counted_creation = creation_text
        .strip_prefix("a number of ")
        .and_then(|text| text.rsplit_once(" equal to "))
        .and_then(|(specification, count)| {
            parse_counted_amount(count)
                .map(|amount| (amount, specification, TokenGrammaticalNumber::Plural))
        });
    let (mut amount, mut specification_text, number) = if let Some(amount) = amount_override {
        let (_, specification_text, number) =
            parse_token_amount(creation_text).ok_or_else(|| unsupported(address, statement))?;
        (amount, specification_text, number)
    } else if let Some(counted) = counted_creation {
        counted
    } else {
        parse_token_amount(creation_text).ok_or_else(|| unsupported(address, statement))?
    };
    if let Some((definition, count_text)) = specification_text.rsplit_once(" for each ")
        && let Some(count) = parse_count_expression(count_text)
    {
        specification_text = definition.trim();
        amount = Amount::Count(Box::new(count));
    }
    let (specification_text, attacking) = specification_text
        .strip_suffix(" that are tapped and attacking")
        .map_or((specification_text, false), |text| (text, true));
    let (specification_text, tapped) = specification_text
        .strip_prefix("tapped ")
        .map_or((specification_text, false), |text| (text, true));
    let parsed_specification =
        parse_token_specification(address, specification_text, number, state, parsed)?;
    record_predefined_token_creation(
        address,
        statement,
        &amount,
        parsed_specification.predefined_kind,
        number,
        parsed,
    )?;
    let specification = parsed_specification.specification;
    let creation = TokenCreation {
        player,
        amount,
        specification,
        tapped: tapped || attacking,
        attacking,
    };
    parsed.effects.push(Effect::CreateToken(creation));
    Ok(true)
}

fn parse_token_amount(text: &str) -> Option<(Amount, &str, TokenGrammaticalNumber)> {
    for (prefix, number) in [
        ("one or more ", TokenGrammaticalNumber::Plural),
        ("seven ", TokenGrammaticalNumber::Plural),
        ("six ", TokenGrammaticalNumber::Plural),
        ("five ", TokenGrammaticalNumber::Plural),
        ("four ", TokenGrammaticalNumber::Plural),
        ("three ", TokenGrammaticalNumber::Plural),
        ("two ", TokenGrammaticalNumber::Plural),
        ("one ", TokenGrammaticalNumber::Singular),
        ("a ", TokenGrammaticalNumber::Singular),
        ("an ", TokenGrammaticalNumber::Singular),
        ("x ", TokenGrammaticalNumber::Plural),
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            let amount_text = prefix.trim();
            return Some((parse_english_amount(amount_text)?, rest, number));
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedTokenSpecification {
    specification: TokenSpecification,
    predefined_kind: Option<PredefinedArtifactTokenKind>,
}

fn parse_token_specification(
    address: ClauseAddress,
    text: &str,
    expected_number: TokenGrammaticalNumber,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<ParsedTokenSpecification, CompileError> {
    let lower = text.to_ascii_lowercase();
    let copy_prefixes = [
        ("token that's a copy of ", TokenGrammaticalNumber::Singular),
        ("tokens that are copies of ", TokenGrammaticalNumber::Plural),
    ];
    for (prefix, printed_number) in copy_prefixes {
        if let Some(original_text) = lower.strip_prefix(prefix) {
            if printed_number != expected_number {
                return Err(unsupported(address, text));
            }
            let original = if original_text == "this object" {
                ObjectRef::Source
            } else if matches!(
                original_text,
                "that permanent" | "that creature" | "that artifact"
            ) {
                state
                    .last_object
                    .clone()
                    .ok_or_else(|| unsupported(address, text))?
            } else if original_text.starts_with("target ") {
                let target = parse_target_description(address, original_text, state)?;
                let object = ObjectRef::Target(target.id);
                parsed.targets.push(target);
                state.last_object = Some(object.clone());
                object
            } else {
                return Err(unsupported(address, text));
            };
            return Ok(ParsedTokenSpecification {
                specification: TokenSpecification::CopyOf(original),
                predefined_kind: None,
            });
        }
    }
    if let Some((kind, printed_number)) = parse_predefined_token_noun(&lower) {
        if printed_number != expected_number {
            return Err(unsupported(address, text));
        }
        return Ok(ParsedTokenSpecification {
            specification: TokenSpecification::Defined(Box::new(kind.definition())),
            predefined_kind: Some(kind),
        });
    }
    parse_creature_token_definition(text)
        .filter(|(_, printed_number)| *printed_number == expected_number)
        .map(|(definition, _)| ParsedTokenSpecification {
            specification: TokenSpecification::Defined(Box::new(definition)),
            predefined_kind: None,
        })
        .ok_or_else(|| unsupported(address, text))
}

fn parse_predefined_token_noun(
    text: &str,
) -> Option<(PredefinedArtifactTokenKind, TokenGrammaticalNumber)> {
    let (kind, suffix) = [
        ("treasure", PredefinedArtifactTokenKind::Treasure),
        ("food", PredefinedArtifactTokenKind::Food),
        ("clue", PredefinedArtifactTokenKind::Clue),
        ("blood", PredefinedArtifactTokenKind::Blood),
        ("gold", PredefinedArtifactTokenKind::Gold),
        ("lander", PredefinedArtifactTokenKind::Lander),
    ]
    .into_iter()
    .find_map(|(name, kind)| text.strip_prefix(name).map(|suffix| (kind, suffix)))?;
    match suffix {
        " token" => Some((kind, TokenGrammaticalNumber::Singular)),
        " tokens" => Some((kind, TokenGrammaticalNumber::Plural)),
        _ => None,
    }
}

fn record_predefined_token_creation(
    address: ClauseAddress,
    statement: &str,
    amount: &Amount,
    kind: Option<PredefinedArtifactTokenKind>,
    number: TokenGrammaticalNumber,
    parsed: &mut ParsedClause,
) -> Result<(), CompileError> {
    let Some(kind) = kind else {
        return Ok(());
    };
    if kind.requires_fixed_amount() && !matches!(amount, Amount::Constant(_)) {
        return Err(unsupported(address, statement));
    }
    parsed
        .predefined_token_creations
        .push(ParsedPredefinedTokenCreation { kind, number });
    Ok(())
}

fn parse_creature_token_definition(
    text: &str,
) -> Option<(TokenDefinition, TokenGrammaticalNumber)> {
    let lower = text.to_ascii_lowercase();
    let token_start = lower.find(" creature token")?;
    let description = text[..token_start].trim();
    let (token_phrase_len, number) = if lower[token_start..].starts_with(" creature tokens") {
        (" creature tokens".len(), TokenGrammaticalNumber::Plural)
    } else {
        (" creature token".len(), TokenGrammaticalNumber::Singular)
    };
    let suffix = text[token_start + token_phrase_len..].trim();
    let words = description.split_whitespace().collect::<Vec<_>>();
    let pt_index = words.iter().position(|word| word.contains('/'))?;
    let (power, toughness) = parse_power_toughness_pair(words[pt_index])?;
    let mut colors = Vec::new();
    let mut index = pt_index + 1;
    while let Some(color) = words
        .get(index)
        .and_then(|word| parse_color_word(word.trim_end_matches(',')))
    {
        colors.push(color);
        index += 1;
        if words.get(index) == Some(&"and") {
            index += 1;
        }
    }
    let artifact = words[index..]
        .iter()
        .any(|word| word.trim_matches(',').eq_ignore_ascii_case("artifact"));
    let subtypes = words[index..]
        .iter()
        .filter(|word| !word.trim_matches(',').eq_ignore_ascii_case("artifact"))
        .map(|word| title_case(word.trim_matches(',')))
        .collect::<Vec<_>>();
    if subtypes.is_empty() {
        return None;
    }
    let mut name = None;
    let mut keywords = Vec::new();
    let mut abilities = Vec::new();
    let suffix_lower = suffix.to_ascii_lowercase();
    if let Some(name_text) = suffix.strip_prefix("named ") {
        if let Some((token_name, keyword_text)) = name_text.split_once(" with ") {
            name = Some(token_name.to_string());
            keywords = parse_keyword_list(
                ClauseAddress {
                    face_index: 0,
                    clause_index: 0,
                },
                keyword_text,
            )
            .ok()?;
        } else {
            name = Some(name_text.to_string());
        }
    } else if let Some(keyword_text) = suffix_lower.strip_prefix("with ") {
        if keyword_text == "\"sacrifice this token: add {c}.\""
            && subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case("Eldrazi"))
            && subtypes.iter().any(|subtype| {
                subtype.eq_ignore_ascii_case("Scion") || subtype.eq_ignore_ascii_case("Spawn")
            })
        {
            abilities.push(colorless_sacrifice_mana_ability());
        } else {
            keywords = parse_keyword_list(
                ClauseAddress {
                    face_index: 0,
                    clause_index: 0,
                },
                keyword_text,
            )
            .ok()?;
        }
    } else if !suffix.is_empty() {
        return None;
    }
    Some((
        TokenDefinition {
            name,
            power: Some(power),
            toughness: Some(toughness),
            colors,
            card_types: if artifact {
                vec![CardType::Artifact, CardType::Creature]
            } else {
                vec![CardType::Creature]
            },
            subtypes,
            keywords,
            abilities,
        },
        number,
    ))
}

fn colorless_sacrifice_mana_ability() -> GrantedAbility {
    GrantedAbility {
        costs: vec![Cost::SacrificeObject(ObjectRef::Source)],
        effects: vec![Effect::AddMana(ManaProduction {
            player: PlayerRef::You,
            choices: vec![ManaChoice {
                symbols: vec![Color::Colorless],
            }],
            amount: Amount::Constant(1),
            commander_identity_only: false,
            scales_with: None,
            typed: Some(TypedManaProductionExpression {
                version: BOUNDED_ORACLE_MANA_EXPRESSION_VERSION,
                quantity: TypedManaQuantity::Fixed(1),
                composition: TypedManaComposition::Exact(vec![TypedManaColor::Colorless]),
                derived_source_scope: None,
                spend_restriction: None,
                retention: TypedManaRetention::Normal,
            }),
        })],
    }
}

fn parse_draw_life_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = statement.to_ascii_lowercase();
    let action_probe = [
        "target opponent ",
        "target player ",
        "its controller ",
        "you ",
        "each opponent ",
        "each player ",
        "that player ",
    ]
    .into_iter()
    .find_map(|prefix| lower.strip_prefix(prefix))
    .unwrap_or(lower.as_str());
    let action_probe = action_probe.strip_prefix("may ").unwrap_or(action_probe);
    if ![
        "draw ",
        "draws ",
        "discard ",
        "discards ",
        "gain ",
        "gains ",
        "lose ",
        "loses ",
        "this object deals ",
        "it deals ",
    ]
    .iter()
    .any(|prefix| action_probe.starts_with(prefix))
    {
        return Ok(false);
    }
    let (player, mut text) = if let Some(rest) = lower
        .strip_prefix("target opponent ")
        .or_else(|| lower.strip_prefix("target player "))
    {
        let target = state.allocate_target(
            TargetFilter::Player,
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let player = PlayerRef::TargetPlayer(target.id);
        parsed.targets.push(target);
        state.last_player = Some(player.clone());
        (player, rest)
    } else if let Some(rest) = lower.strip_prefix("its controller ") {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        (PlayerRef::ControllerOf(Box::new(object)), rest)
    } else if let Some(rest) = lower.strip_prefix("you ") {
        (PlayerRef::You, rest)
    } else if let Some(rest) = lower.strip_prefix("each opponent ") {
        (PlayerRef::Opponent, rest)
    } else if let Some(rest) = lower.strip_prefix("each player ") {
        (PlayerRef::Any, rest)
    } else if let Some(rest) = lower.strip_prefix("that player ") {
        (
            state.last_player.clone().unwrap_or(PlayerRef::ThatPlayer),
            rest,
        )
    } else {
        (PlayerRef::You, lower.as_str())
    };
    let mut optional = false;
    if let Some(rest) = text.strip_prefix("may ") {
        optional = true;
        text = rest;
    }
    let relative_draw = [
        "discard all the cards in your hand, then draw ",
        "discards all the cards in their hand, then draws ",
        "discard all the cards in their hand, then draw ",
        "discards all cards from their hand, then draws ",
        "discard all cards from your hand, then draw ",
    ]
    .into_iter()
    .find_map(|prefix| text.strip_prefix(prefix));
    if let Some(relative_draw) = relative_draw {
        if optional {
            return Err(unsupported(address, statement));
        }
        let adjustment = match relative_draw {
            "that many cards." => 0,
            "that many cards plus one." => 1,
            "that many cards minus one." => -1,
            _ => return Err(unsupported(address, statement)),
        };
        parsed.effects.push(Effect::LibraryProcedure(
            LibraryProcedure::DiscardHandsAndDrawDiscarded { player, adjustment },
        ));
        return Ok(true);
    }
    if let Some(rest) = text
        .strip_prefix("draw ")
        .or_else(|| text.strip_prefix("draws "))
    {
        let (draw_text, unless_text) = rest
            .split_once(" unless ")
            .map_or((rest, None), |(draw, unless)| (draw, Some(unless)));
        let (draw_text, delayed_until) = draw_text
            .strip_suffix(" at the beginning of the next turn's upkeep.")
            .map_or((draw_text, None), |draw| {
                (
                    draw,
                    Some(Trigger::BeginningOf {
                        step: Step::Upkeep,
                        player: TurnPlayer::NextTurn,
                    }),
                )
            });
        let amount = if let Some((card_text, count_text)) = draw_text.split_once(" for each ")
            && matches!(card_text.trim(), "a card" | "one card")
        {
            Amount::Count(Box::new(
                parse_count_expression(count_text)
                    .ok_or_else(|| unsupported(address, statement))?,
            ))
        } else if let Some(count_text) = draw_text
            .strip_prefix("cards equal to ")
            .or_else(|| draw_text.strip_prefix("a number of cards equal to "))
        {
            parse_counted_amount(count_text).ok_or_else(|| unsupported(address, statement))?
        } else if matches!(
            draw_text.trim().trim_end_matches('.'),
            "that many cards" | "that much cards"
        ) {
            Amount::Count(Box::new(CountExpression::TriggerEventAmount))
        } else {
            parse_card_count(draw_text).ok_or_else(|| unsupported(address, statement))?
        };
        if let Some(unless_text) = unless_text {
            let payer = if unless_text.starts_with("that player pays ") {
                PlayerRef::ThatPlayer
            } else {
                return Err(unsupported(address, statement));
            };
            let cost_text = unless_text
                .strip_prefix("that player pays ")
                .unwrap_or_default()
                .trim_end_matches('.');
            parsed.conditions.push(Condition::UnlessPaid {
                player: payer,
                cost: parse_payment_cost(address, cost_text)?,
            });
        }
        parsed.effects.push(Effect::Draw {
            player,
            amount,
            optional,
            delayed_until,
        });
        return Ok(true);
    }
    if let Some(rest) = text
        .strip_prefix("discard ")
        .or_else(|| text.strip_prefix("discards "))
    {
        if optional {
            return Err(unsupported(address, statement));
        }
        if rest == "that card." {
            let selection = state
                .pending_hand_choice
                .take()
                .ok_or_else(|| unsupported(address, statement))?;
            if selection.filter.owner.as_ref() != Some(&player) {
                return Err(unsupported(address, statement));
            }
            parsed.effects.push(Effect::Discard(selection));
            return Ok(true);
        }
        if rest == "your hand." && player == PlayerRef::You {
            let mut filter = ObjectFilter::in_zone(Zone::Hand);
            filter.owner = Some(PlayerRef::You);
            let selection = state.allocate_selection(PlayerRef::You, filter, TargetAmount::All);
            parsed.effects.push(Effect::Discard(selection));
            return Ok(true);
        }
        let amount = parse_card_count(rest).ok_or_else(|| unsupported(address, statement))?;
        let Some(amount) = amount_as_constant(&amount) else {
            return Err(unsupported(address, statement));
        };
        let amount = u16::try_from(amount).map_err(|_| unsupported(address, statement))?;
        let mut filter = ObjectFilter::in_zone(Zone::Hand);
        filter.owner = Some(player.clone());
        let selection = state.allocate_selection(player, filter, TargetAmount::Exactly(amount));
        parsed.effects.push(Effect::Discard(selection));
        return Ok(true);
    }
    if let Some(rest) = text
        .strip_prefix("gain ")
        .or_else(|| text.strip_prefix("gains "))
    {
        if rest == "life equal to the life lost this way." {
            let (players, amount_each) = parsed
                .effects
                .iter()
                .rev()
                .find_map(|effect| match effect {
                    Effect::LoseLife { player, amount } => Some((player.clone(), amount.clone())),
                    _ => None,
                })
                .ok_or_else(|| unsupported(address, statement))?;
            parsed.effects.push(Effect::GainLife {
                player,
                amount: Amount::Count(Box::new(CountExpression::LifeLostThisWay {
                    players,
                    amount_each: Box::new(amount_each),
                })),
            });
            return Ok(true);
        }
        if let Some(count_text) = rest
            .strip_prefix("life equal to ")
            .and_then(|text| text.strip_suffix('.'))
        {
            let amount = if count_text == "that creature's toughness" {
                Amount::Count(Box::new(CountExpression::ToughnessOf {
                    object: state
                        .last_object
                        .clone()
                        .unwrap_or(ObjectRef::TriggeringObject),
                }))
            } else {
                parse_counted_amount(count_text).ok_or_else(|| unsupported(address, statement))?
            };
            parsed.effects.push(Effect::GainLife { player, amount });
            return Ok(true);
        }
        if matches!(
            rest.trim().trim_end_matches('.'),
            "that much life" | "that many life"
        ) {
            parsed.effects.push(Effect::GainLife {
                player,
                amount: Amount::Count(Box::new(CountExpression::TriggerEventAmount)),
            });
            return Ok(true);
        }
        let (amount_text, count_filter) =
            if let Some((amount, filter)) = rest.split_once(" life for each ") {
                (amount, Some(filter.trim_end_matches('.')))
            } else {
                (
                    rest.strip_suffix(" life.")
                        .ok_or_else(|| unsupported(address, statement))?,
                    None,
                )
            };
        let base =
            parse_english_amount(amount_text).ok_or_else(|| unsupported(address, statement))?;
        let amount = if let Some(filter_text) = count_filter {
            let count = Amount::Count(Box::new(
                parse_count_expression(filter_text)
                    .ok_or_else(|| unsupported(address, statement))?,
            ));
            match amount_as_constant(&base) {
                Some(1) => count,
                Some(factor) => Amount::Product {
                    factor,
                    value: Box::new(count),
                },
                None => return Err(unsupported(address, statement)),
            }
        } else {
            base
        };
        parsed.effects.push(Effect::GainLife { player, amount });
        return Ok(true);
    }
    if matches!(
        text,
        "lose half your life, rounded up." | "loses half their life, rounded up."
    ) {
        parsed.effects.push(Effect::LoseLife {
            player: player.clone(),
            amount: Amount::Count(Box::new(CountExpression::HalfLifeTotal {
                player,
                round_up: true,
            })),
        });
        return Ok(true);
    }
    if let Some(amount_text) = text
        .strip_prefix("lose ")
        .or_else(|| text.strip_prefix("loses "))
        .and_then(|rest| rest.strip_suffix(" life."))
    {
        let amount = if matches!(amount_text, "that much" | "that many") {
            Amount::Count(Box::new(CountExpression::TriggerEventAmount))
        } else {
            parse_english_amount(amount_text).ok_or_else(|| unsupported(address, statement))?
        };
        parsed.effects.push(Effect::LoseLife { player, amount });
        return Ok(true);
    }
    let damage_amount = lower
        .strip_prefix("this object deals ")
        .or_else(|| lower.strip_prefix("it deals "))
        .and_then(|text| text.strip_suffix(" damage to you."))
        .and_then(parse_english_amount);
    if let Some(amount) = damage_amount {
        parsed.effects.push(Effect::Damage {
            source: ObjectRef::Source,
            recipient: PlayerRef::You,
            amount,
        });
        return Ok(true);
    }
    Ok(false)
}

fn parse_card_count(text: &str) -> Option<Amount> {
    let lower = text.trim().trim_end_matches('.').to_ascii_lowercase();
    if let Some(amount_text) = lower.strip_suffix(" cards") {
        if let Some(amount_text) = amount_text.strip_prefix("up to ") {
            return Some(Amount::UpTo(Box::new(parse_english_amount(amount_text)?)));
        }
        return parse_english_amount(amount_text);
    }
    if let Some(amount_text) = lower.strip_suffix(" card") {
        return parse_english_amount(amount_text);
    }
    None
}

fn parse_characteristic_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = statement.to_ascii_lowercase();
    if let Some((subject_text, color)) = lower
        .strip_suffix(" until end of turn.")
        .and_then(|body| body.split_once(" becomes "))
        .and_then(|(subject, color)| parse_color_word(color).map(|color| (subject, color)))
    {
        let object = parse_subject_object_ref(address, subject_text, state, parsed)?;
        state.last_object = Some(object.clone());
        parsed
            .effects
            .push(Effect::SetCharacteristics(SetCharacteristics {
                object,
                colors: Some(vec![color]),
                card_types: None,
                subtypes: None,
                name: None,
                base_power: None,
                base_toughness: None,
                retain_other_card_types: true,
                retain_other_subtypes: true,
                retain_other_colors: false,
                retain_other_names: true,
                duration: Duration::UntilEndOfTurn,
            }));
        return Ok(true);
    }
    if lower
        == "target creature gets +1/+1 until end of turn for each basic land type among lands you control."
    {
        let target = parse_target_description(address, "target creature", state)?;
        let objects = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        state.last_object = Some(objects.clone());
        let amount = Amount::Count(Box::new(CountExpression::DistinctBasicLandTypes {
            player: PlayerRef::You,
        }));
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects,
                operation: PowerToughnessOperation::Add,
                power: amount.clone(),
                toughness: amount,
                duration: Duration::UntilEndOfTurn,
            }));
        return Ok(true);
    }
    if let Some(subject_text) = lower
        .strip_prefix("double ")
        .and_then(|text| text.strip_suffix("'s power until end of turn."))
    {
        let objects = parse_subject_object_ref(address, subject_text, state, parsed)?;
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: objects.clone(),
                operation: PowerToughnessOperation::Add,
                power: Amount::Count(Box::new(CountExpression::PowerOf { object: objects })),
                toughness: Amount::Constant(0),
                duration: Duration::UntilEndOfTurn,
            }));
        return Ok(true);
    }
    if let Some(subject_text) = lower
        .strip_prefix("switch ")
        .and_then(|text| text.strip_suffix("'s power and toughness until end of turn."))
    {
        let objects = parse_subject_object_ref(address, subject_text, state, parsed)?;
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects,
                operation: PowerToughnessOperation::Switch,
                power: Amount::Constant(0),
                toughness: Amount::Constant(0),
                duration: Duration::UntilEndOfTurn,
            }));
        return Ok(true);
    }
    if let Some(rest) = lower.strip_prefix("put a number of ")
        && let Some((placement, amount_text)) = rest.trim_end_matches('.').rsplit_once(" equal to ")
        && let Some((counter_name, subject_text)) = placement.split_once(" counters on ")
        && let Some(amount) = parse_counted_amount(amount_text)
    {
        let object = if matches!(
            subject_text,
            "it" | "them" | "each of them" | "those creatures"
        ) {
            state
                .last_object
                .clone()
                .unwrap_or(ObjectRef::TriggeringObject)
        } else {
            parse_subject_object_ref(address, subject_text, state, parsed)?
        };
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::PutCounter {
            object,
            counter: parse_counter_kind(counter_name),
            amount,
        });
        return Ok(true);
    }
    if let Some(rest) = lower.strip_prefix("put ")
        && let Some((counter_text, subject_text)) = rest.strip_suffix('.').and_then(|text| {
            text.split_once(" counters on ")
                .map(|(counter, subject)| (counter, subject))
                .or_else(|| {
                    text.split_once(" counter on ")
                        .map(|(counter, subject)| (counter, subject))
                })
        })
        && !counter_text.contains(" and ")
    {
        let (amount, counter_name) = if let Some(counter_name) = counter_text
            .strip_prefix("that many ")
            .filter(|name| !name.is_empty())
        {
            (
                Amount::Count(Box::new(CountExpression::TriggerEventAmount)),
                counter_name,
            )
        } else {
            parse_counter_amount_and_name(counter_text)
                .ok_or_else(|| unsupported(address, statement))?
        };
        let object = if matches!(
            subject_text,
            "it" | "them" | "each of them" | "those creatures"
        ) {
            state
                .last_object
                .clone()
                .unwrap_or(ObjectRef::TriggeringObject)
        } else {
            parse_subject_object_ref(address, subject_text, state, parsed)?
        };
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::PutCounter {
            object,
            counter: parse_counter_kind(counter_name),
            amount,
        });
        return Ok(true);
    }
    if let Some(rest) = lower.strip_prefix("put a +1/+1 counter on ") {
        let target_text = rest.trim_end_matches('.');
        let target = parse_target_description(address, target_text, state)?;
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        state.last_object = Some(object.clone());
        parsed.effects.push(Effect::PutCounter {
            object,
            counter: CounterKind::PlusOnePlusOne,
            amount: Amount::Constant(1),
        });
        return Ok(true);
    }
    if let Some(rest) = lower.strip_prefix("put an indestructible counter on ") {
        let object = if rest.trim_end_matches('.') == "this object" {
            ObjectRef::Source
        } else {
            return Err(unsupported(address, statement));
        };
        parsed.effects.push(Effect::PutCounter {
            object,
            counter: CounterKind::Indestructible,
            amount: Amount::Constant(1),
        });
        return Ok(true);
    }
    if let Some(subject_text) = lower
        .strip_prefix("double the power and toughness of ")
        .and_then(|text| text.strip_suffix(" until end of turn."))
    {
        let objects = parse_subject_object_ref(address, subject_text, state, parsed)?;
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects,
                operation: PowerToughnessOperation::Double,
                power: Amount::Constant(2),
                toughness: Amount::Constant(2),
                duration: Duration::UntilEndOfTurn,
            }));
        return Ok(true);
    }
    if lower.starts_with("until end of turn, ") {
        let body = statement["Until end of turn, ".len()..].trim();
        return parse_characteristic_with_duration(
            address,
            body,
            Duration::UntilEndOfTurn,
            state,
            parsed,
        );
    }
    if lower.contains(" until end of turn") {
        let body = statement
            .strip_suffix(" until end of turn.")
            .or_else(|| {
                statement
                    .find(" until end of turn,")
                    .map(|index| &statement[..index])
            })
            .ok_or_else(|| unsupported(address, statement))?;
        let effect_start = parsed.effects.len();
        if parse_characteristic_with_duration(
            address,
            body,
            Duration::UntilEndOfTurn,
            state,
            parsed,
        )? {
            if let Some(color) = lower
                .split_once("where x is your devotion to ")
                .and_then(|(_, color)| parse_color_word(color.trim_end_matches('.')))
            {
                let devotion = Amount::Count(Box::new(CountExpression::Devotion {
                    player: PlayerRef::You,
                    color,
                }));
                for effect in &mut parsed.effects[effect_start..] {
                    if let Effect::ModifyPowerToughness(change) = effect {
                        if matches!(change.power, Amount::X) {
                            change.power = devotion.clone();
                        }
                        if matches!(change.toughness, Amount::X) {
                            change.toughness = devotion.clone();
                        }
                    }
                }
            }
            return Ok(true);
        }
    }
    if lower.starts_with("creatures you control have base power and toughness ")
        && lower.ends_with('.')
    {
        let pair = lower
            .strip_prefix("creatures you control have base power and toughness ")
            .and_then(|text| text.strip_suffix('.'))
            .and_then(parse_power_toughness_pair)
            .ok_or_else(|| unsupported(address, statement))?;
        let mut filter = ObjectFilter::with_type(CardType::Creature);
        filter.controller = Some(PlayerRef::You);
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects: ObjectRef::EachMatching(filter),
                operation: PowerToughnessOperation::SetBase,
                power: pair.0,
                toughness: pair.1,
                duration: Duration::Permanent,
            }));
        return Ok(true);
    }
    Ok(false)
}

fn parse_counter_amount_and_name(text: &str) -> Option<(Amount, &str)> {
    let text = text.trim();
    for prefix in [
        "ten ", "nine ", "eight ", "seven ", "six ", "five ", "four ", "three ", "two ", "one ",
        "an ", "a ", "x ",
    ] {
        if let Some(name) = text.strip_prefix(prefix) {
            let amount = parse_english_amount(prefix.trim())?;
            let name = name.trim();
            if name.is_empty()
                || name.contains(|character: char| {
                    !character.is_ascii_alphanumeric()
                        && !matches!(character, '+' | '-' | '/' | '\'' | ' ')
                })
            {
                return None;
            }
            return Some((amount, name));
        }
    }
    None
}

fn parse_counter_kind(name: &str) -> CounterKind {
    match name.trim() {
        "+1/+1" => CounterKind::PlusOnePlusOne,
        "-1/-1" => CounterKind::MinusOneMinusOne,
        "loyalty" => CounterKind::Loyalty,
        "indestructible" => CounterKind::Indestructible,
        other => CounterKind::Named(other.to_string()),
    }
}

fn parse_characteristic_with_duration(
    address: ClauseAddress,
    body: &str,
    duration: Duration,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = body.to_ascii_lowercase();
    if let Some((subject_text, animation_text)) = lower.split_once(" becomes a ")
        && subject_text == "this object"
    {
        let animation =
            parse_animation(animation_text, duration).ok_or_else(|| unsupported(address, body))?;
        parsed.effects.push(Effect::Animate(animation));
        state.last_object = Some(ObjectRef::Source);
        return Ok(true);
    }

    if let Some((subject_text, pair_text)) = lower.split_once(" has base power and toughness ") {
        let pair =
            parse_power_toughness_pair(pair_text).ok_or_else(|| unsupported(address, body))?;
        let objects = parse_subject_object_ref(address, subject_text, state, parsed)?;
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects,
                operation: PowerToughnessOperation::SetBase,
                power: pair.0,
                toughness: pair.1,
                duration,
            }));
        return Ok(true);
    }
    if let Some((subject_text, pair_text)) = lower.split_once(" have base power and toughness ") {
        let pair =
            parse_power_toughness_pair(pair_text).ok_or_else(|| unsupported(address, body))?;
        let objects = parse_subject_object_ref(address, subject_text, state, parsed)?;
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects,
                operation: PowerToughnessOperation::SetBase,
                power: pair.0,
                toughness: pair.1,
                duration,
            }));
        return Ok(true);
    }

    let (subject_text, grant_text) = if let Some(value) = lower.split_once(" gets ") {
        value
    } else if let Some(value) = lower.split_once(" get ") {
        value
    } else if let Some(value) = lower.split_once(" gains ") {
        value
    } else if let Some(value) = lower.split_once(" gain ") {
        value
    } else if let Some(value) = lower.split_once(" has ") {
        value
    } else if let Some(value) = lower.split_once(" have ") {
        value
    } else {
        return Ok(false);
    };
    let objects = if matches!(
        subject_text,
        "it" | "that creature" | "that permanent" | "that card" | "that object"
    ) {
        state
            .last_object
            .clone()
            .unwrap_or(ObjectRef::TriggeringObject)
    } else {
        parse_subject_object_ref(address, subject_text, state, parsed)?
    };
    state.last_object = Some(objects.clone());

    let grant_text = grant_text
        .split_once(", where x is ")
        .map_or(grant_text, |(grant, _)| grant)
        .trim();
    let grant_text = grant_text
        .replace(" and gains ", ", ")
        .replace(" and gain ", ", ")
        .replace(" and gets ", ", ")
        .replace(" and get ", ", ")
        .replace(" and has ", ", ")
        .replace(" and have ", ", ");
    let mut keywords = Vec::new();
    let mut grants_fear = false;
    let mut pt = None;
    for part in grant_text
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if (part.starts_with('+') || part.starts_with('-')) && part.contains('/') {
            pt = Some(
                parse_power_toughness_modifier_pair(part)
                    .ok_or_else(|| unsupported(address, body))?,
            );
        } else if part.eq_ignore_ascii_case("fear") {
            grants_fear = true;
        } else {
            keywords.extend(parse_keyword_list(address, part)?);
        }
    }
    if !keywords.is_empty() {
        parsed.effects.push(Effect::GrantKeyword {
            objects: objects.clone(),
            keywords,
            duration: duration.clone(),
        });
    }
    if grants_fear {
        parsed
            .effects
            .push(Effect::Restriction(Restriction::BlockerMustMatch {
                attacker: objects.clone(),
                blocker_filter: fear_blocker_target_filter(),
                duration: duration.clone(),
            }));
    }
    if let Some((operation, mut power, mut toughness)) = pt {
        if lower.contains("where x is the number of creatures you control") {
            if operation != PowerToughnessOperation::Add {
                return Err(unsupported(address, body));
            }
            let mut filter = ObjectFilter::with_type(CardType::Creature);
            filter.controller = Some(PlayerRef::You);
            power = Amount::Count(Box::new(CountExpression::MatchingObjects {
                player: PlayerRef::You,
                filter: filter.clone(),
            }));
            toughness = Amount::Count(Box::new(CountExpression::MatchingObjects {
                player: PlayerRef::You,
                filter,
            }));
        } else if lower.contains("where x is the greatest power among creatures you control") {
            if operation != PowerToughnessOperation::Add {
                return Err(unsupported(address, body));
            }
            let mut filter = ObjectFilter::with_type(CardType::Creature);
            filter.controller = Some(PlayerRef::You);
            power = Amount::Count(Box::new(CountExpression::GreatestPower {
                player: PlayerRef::You,
                filter: filter.clone(),
            }));
            toughness = Amount::Count(Box::new(CountExpression::GreatestPower {
                player: PlayerRef::You,
                filter,
            }));
        } else if let Some(color) = lower
            .split_once("where x is your devotion to ")
            .and_then(|(_, color)| parse_color_word(color.trim_end_matches('.')))
        {
            if operation != PowerToughnessOperation::Add {
                return Err(unsupported(address, body));
            }
            let devotion = Amount::Count(Box::new(CountExpression::Devotion {
                player: PlayerRef::You,
                color,
            }));
            if matches!(power, Amount::X) {
                power = devotion.clone();
            }
            if matches!(toughness, Amount::X) {
                toughness = devotion;
            }
        }
        parsed
            .effects
            .push(Effect::ModifyPowerToughness(PowerToughnessChange {
                objects,
                operation,
                power,
                toughness,
                duration,
            }));
    }
    Ok(!parsed.effects.is_empty())
}

fn parse_subject_object_ref(
    address: ClauseAddress,
    subject_text: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<ObjectRef, CompileError> {
    let lower = subject_text.trim().to_ascii_lowercase();
    if lower == "this object" {
        return Ok(ObjectRef::Source);
    }
    if matches!(
        lower.as_str(),
        "it" | "that creature" | "that permanent" | "that card" | "that object"
    ) {
        return Ok(state
            .last_object
            .clone()
            .unwrap_or(ObjectRef::TriggeringObject));
    }
    if lower == "enchanted creature" {
        return Ok(ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Aura,
        });
    }
    if matches!(lower.as_str(), "enchanted permanent" | "enchanted land") {
        return Ok(ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Aura,
        });
    }
    if lower == "equipped creature" {
        return Ok(ObjectRef::AttachmentTarget {
            kind: AttachmentKind::Equipment,
        });
    }
    if lower.contains("target ") {
        let target = parse_target_description(address, &lower, state)?;
        let object = ObjectRef::Target(target.id);
        parsed.targets.push(target);
        return Ok(object);
    }
    let mut filter =
        parse_card_filter_phrase(&lower).ok_or_else(|| unsupported(address, subject_text))?;
    filter.zones = vec![Zone::Battlefield];
    Ok(ObjectRef::EachMatching(filter))
}

fn parse_animation(text: &str, duration: Duration) -> Option<AnimateEffect> {
    let lower = text.trim().trim_end_matches('.').to_ascii_lowercase();
    let words = lower.split_whitespace().collect::<Vec<_>>();
    let pt_index = words.iter().position(|word| word.contains('/'))?;
    let (power, toughness) = parse_power_toughness_pair(words[pt_index])?;
    let mut colors = Vec::new();
    let mut index = pt_index + 1;
    while let Some(color) = words
        .get(index)
        .and_then(|word| parse_color_word(word.trim_end_matches(',')))
    {
        colors.push(color);
        index += 1;
        if words.get(index) == Some(&"and") {
            index += 1;
        }
    }
    let creature_index = words[index..]
        .iter()
        .position(|word| word.trim_end_matches(',') == "creature")?
        + index;
    let subtypes = words[index..creature_index]
        .iter()
        .map(|word| title_case(word.trim_matches(',')))
        .collect::<Vec<_>>();
    let mut keywords = Vec::new();
    if let Some(with_index) = words.iter().position(|word| *word == "with") {
        let keyword_text = words[with_index + 1..].join(" ");
        keywords = parse_keyword_list(
            ClauseAddress {
                face_index: 0,
                clause_index: 0,
            },
            &keyword_text,
        )
        .ok()?;
    }
    Some(AnimateEffect {
        object: ObjectRef::Source,
        power,
        toughness,
        retain_printed_power_toughness: false,
        colors,
        subtypes,
        keywords,
        retain_land: false,
        duration,
    })
}

fn parse_utility_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<bool, CompileError> {
    let lower = statement.to_ascii_lowercase();
    if let Some(level) = lower
        .trim_end_matches('.')
        .strip_prefix("level ")
        .and_then(|level| level.parse::<u8>().ok())
        && matches!(level, 2 | 3)
    {
        parsed.effects.push(Effect::SetClassLevel { level });
        return Ok(true);
    }
    let mill_body = lower
        .strip_prefix("target player mills ")
        .map(|text| (None, text))
        .or_else(|| {
            lower
                .strip_prefix("each player mills ")
                .map(|text| (Some(PlayerRef::Any), text))
        })
        .or_else(|| {
            lower
                .strip_prefix("target opponent mills ")
                .map(|text| (None, text))
        })
        .or_else(|| {
            lower
                .strip_prefix("each opponent mills ")
                .map(|text| (Some(PlayerRef::Opponent), text))
        })
        .or_else(|| {
            lower
                .strip_prefix("you mill ")
                .map(|text| (Some(PlayerRef::You), text))
        })
        .or_else(|| {
            lower
                .strip_prefix("mill ")
                .map(|text| (Some(PlayerRef::You), text))
        })
        .or_else(|| {
            lower
                .strip_prefix("that player mills ")
                .map(|text| (Some(PlayerRef::ThatPlayer), text))
        });
    if let Some((player, amount_text)) = mill_body {
        let amount_text = amount_text
            .trim_end_matches('.')
            .trim_end_matches(" cards")
            .trim_end_matches(" card");
        let amount =
            parse_english_amount(amount_text).ok_or_else(|| unsupported(address, statement))?;
        let player = if let Some(player) = player {
            player
        } else {
            let target = state.allocate_target(
                TargetFilter::Player,
                TargetAmount::Exactly(1),
                TargetRelationship::Independent,
            );
            let player = PlayerRef::TargetPlayer(target.id);
            parsed.targets.push(target);
            state.last_player = Some(player.clone());
            player
        };
        parsed.effects.push(Effect::Mill { player, amount });
        return Ok(true);
    }
    for (verb, constructor) in [("scry ", 0u8), ("surveil ", 1u8)] {
        if let Some(amount_text) = lower
            .strip_prefix(verb)
            .and_then(|text| text.strip_suffix('.'))
        {
            let amount =
                parse_english_amount(amount_text).ok_or_else(|| unsupported(address, statement))?;
            parsed.effects.push(if constructor == 0 {
                Effect::Scry {
                    player: PlayerRef::You,
                    amount,
                }
            } else {
                Effect::Surveil {
                    player: PlayerRef::You,
                    amount,
                }
            });
            return Ok(true);
        }
    }
    if lower == "untap all lands you control." {
        let mut filter = ObjectFilter::with_type(CardType::Land);
        filter.controller = Some(PlayerRef::You);
        filter.zones = vec![Zone::Battlefield];
        parsed.effects.push(Effect::Untap {
            object: ObjectRef::EachMatching(filter),
        });
        return Ok(true);
    }
    if lower == "untap that land." {
        let object = state
            .last_object
            .clone()
            .unwrap_or(ObjectRef::SearchedCard(0));
        parsed.effects.push(Effect::Untap { object });
        return Ok(true);
    }
    if lower == "untap all creatures you control." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::You);
        let objects = ObjectRef::EachMatching(creatures);
        parsed.effects.push(Effect::Untap {
            object: objects.clone(),
        });
        state.last_object = Some(objects);
        return Ok(true);
    }
    if lower == "tap all creatures your opponents control." {
        let mut creatures = ObjectFilter::with_type(CardType::Creature);
        creatures.zones = vec![Zone::Battlefield];
        creatures.controller = Some(PlayerRef::Opponent);
        let objects = ObjectRef::EachMatching(creatures);
        parsed.effects.push(Effect::Tap {
            object: objects.clone(),
        });
        state.last_object = Some(objects);
        return Ok(true);
    }
    if lower == "untap those creatures." {
        let objects = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::Untap { object: objects });
        return Ok(true);
    }
    if lower == "transform this object." {
        parsed.effects.push(Effect::Transform {
            object: ObjectRef::Source,
        });
        return Ok(true);
    }
    let exile_source_return_transformed = lower
        .strip_prefix("exile this ")
        .and_then(|body| {
            body.strip_suffix(
                ", then return it to the battlefield transformed under its owner's control.",
            )
        })
        .is_some_and(|source_description| !source_description.trim().is_empty());
    if exile_source_return_transformed {
        let object = ObjectRef::Source;
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Battlefield),
            to: Zone::Exile,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        parsed.effects.push(Effect::MoveZoneUnderControl {
            object: object.clone(),
            from: Zone::Exile,
            to: Zone::Battlefield,
            controller: PlayerRef::OwnerOf(Box::new(object.clone())),
            tapped: false,
            face_down: false,
            delayed_until: None,
        });
        parsed.effects.push(Effect::Transform { object });
        state.last_object = Some(ObjectRef::Source);
        return Ok(true);
    }
    if lower == "return it to the battlefield transformed under its owner's control."
        && matches!(
            &parsed.timing,
            Timing::Triggered(trigger)
                if matches!(
                    trigger.as_ref(),
                    Trigger::ObjectEvent {
                        subject: TriggerSubject::Source,
                        event: ObjectEventKind::Dies,
                    }
                )
        )
    {
        let object = ObjectRef::Source;
        parsed.effects.push(Effect::MoveZoneUnderControl {
            object: object.clone(),
            from: Zone::Graveyard,
            to: Zone::Battlefield,
            controller: PlayerRef::OwnerOf(Box::new(object.clone())),
            tapped: false,
            face_down: false,
            delayed_until: None,
        });
        parsed.effects.push(Effect::Transform {
            object: object.clone(),
        });
        state.last_object = Some(object);
        return Ok(true);
    }
    if lower == "shuffle it into its owner's library."
        && matches!(
            &parsed.timing,
            Timing::Triggered(trigger)
                if matches!(
                    trigger.as_ref(),
                    Trigger::ObjectEvent {
                        subject: TriggerSubject::Source,
                        event: ObjectEventKind::PutIntoGraveyardFromAnywhere,
                    }
                )
        )
    {
        let object = ObjectRef::Source;
        let owner = PlayerRef::OwnerOf(Box::new(object.clone()));
        parsed.effects.push(Effect::MoveZone(ZoneMove {
            object: object.clone(),
            from: Some(Zone::Graveyard),
            to: Zone::Library,
            tapped: false,
            face_down: false,
            delayed_until: None,
        }));
        parsed
            .effects
            .push(Effect::ShuffleLibrary { player: owner });
        state.last_object = Some(object);
        return Ok(true);
    }
    if lower == "this object doesn't untap during your next untap step." {
        parsed.effects.push(Effect::PreventNextUntap {
            object: ObjectRef::Source,
        });
        return Ok(true);
    }
    if lower == "look at the top card of your library." {
        state.last_object = Some(ObjectRef::TopCard {
            player: Box::new(PlayerRef::You),
        });
        parsed.effects.push(Effect::LookAtTop {
            player: PlayerRef::You,
            amount: Amount::Constant(1),
        });
        return Ok(true);
    }
    if lower == "look at the top card of target player's library." {
        let target = state.allocate_target(
            TargetFilter::Player,
            TargetAmount::Exactly(1),
            TargetRelationship::Independent,
        );
        let player = PlayerRef::TargetPlayer(target.id);
        parsed.targets.push(target);
        state.last_player = Some(player.clone());
        state.last_object = Some(ObjectRef::TopCard {
            player: Box::new(player.clone()),
        });
        parsed.effects.push(Effect::LookAtTop {
            player,
            amount: Amount::Constant(1),
        });
        return Ok(true);
    }
    if let Some(amount_text) = lower
        .strip_prefix("look at the top ")
        .and_then(|text| text.strip_suffix(" cards of your library."))
    {
        let amount =
            parse_english_amount(amount_text).ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::LookAtTop {
            player: PlayerRef::You,
            amount,
        });
        return Ok(true);
    }
    let reveal_top_amount = if lower == "reveal the top card of your library." {
        Some(Amount::Constant(1))
    } else {
        lower
            .strip_prefix("reveal the top ")
            .and_then(|text| text.strip_suffix(" cards of your library."))
            .and_then(parse_english_amount)
    };
    if let Some(amount) = reveal_top_amount {
        if amount == Amount::Constant(1) {
            state.last_object = Some(ObjectRef::TopCard {
                player: Box::new(PlayerRef::You),
            });
        }
        parsed.effects.push(Effect::RevealTop {
            player: PlayerRef::You,
            amount,
        });
        return Ok(true);
    }
    if lower == "each player reveals the top card of their library." {
        parsed.effects.push(Effect::RevealTop {
            player: PlayerRef::Any,
            amount: Amount::Constant(1),
        });
        return Ok(true);
    }
    if lower == "if it's a land card, you may put it onto the battlefield tapped." {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        parsed.effects.push(Effect::Conditional {
            condition: Condition::ObjectIsCardType {
                object: object.clone(),
                card_type: CardType::Land,
            },
            if_true: vec![Effect::Optional(vec![Effect::MoveZoneUnderControl {
                object: object.clone(),
                from: Zone::Library,
                to: Zone::Battlefield,
                controller: PlayerRef::You,
                tapped: true,
                face_down: false,
                delayed_until: None,
            }])],
            if_false: Vec::new(),
        });
        state.last_object = Some(object);
        return Ok(true);
    }
    if lower == "you may reveal a creature card from among them and put it into your hand." {
        let mut predicate = ObjectFilter::with_type(CardType::Creature);
        predicate.zones = vec![Zone::Library];
        parsed.effects.push(Effect::SelectFromLookedAt {
            player: PlayerRef::You,
            amount: Amount::UpTo(Box::new(Amount::Constant(1))),
            predicate,
            reveal: true,
            face_down: false,
            tapped: false,
            destination: Zone::Hand,
        });
        return Ok(true);
    }
    if lower == "reveal an artifact card from among them and put it into your hand." {
        let mut predicate = ObjectFilter::with_type(CardType::Artifact);
        predicate.zones = vec![Zone::Library];
        parsed.effects.push(Effect::SelectFromLookedAt {
            player: PlayerRef::You,
            amount: Amount::Constant(1),
            predicate,
            reveal: true,
            face_down: false,
            tapped: false,
            destination: Zone::Hand,
        });
        return Ok(true);
    }
    if lower == "reveal a creature or land card from among them and put it into your hand." {
        let mut predicate = ObjectFilter::default();
        predicate.card_types = vec![CardType::Creature, CardType::Land];
        predicate.card_type_match_any = true;
        predicate.zones = vec![Zone::Library];
        parsed.effects.push(Effect::SelectFromLookedAt {
            player: PlayerRef::You,
            amount: Amount::Constant(1),
            predicate,
            reveal: true,
            face_down: false,
            tapped: false,
            destination: Zone::Hand,
        });
        return Ok(true);
    }
    if lower
        == "put one of them into your hand and the rest on the bottom of your library in any order."
    {
        parsed.effects.extend([
            Effect::SelectFromLookedAt {
                player: PlayerRef::You,
                amount: Amount::Constant(1),
                predicate: ObjectFilter::default(),
                reveal: false,
                face_down: false,
                tapped: false,
                destination: Zone::Hand,
            },
            Effect::PutRestOnLibraryBottom {
                player: PlayerRef::You,
                order: BottomOrder::AnyOrder,
            },
        ]);
        return Ok(true);
    }
    if lower == "put one of them into your hand and the other into your graveyard." {
        parsed.effects.extend([
            Effect::SelectFromLookedAt {
                player: PlayerRef::You,
                amount: Amount::Constant(1),
                predicate: ObjectFilter::default(),
                reveal: false,
                face_down: false,
                tapped: false,
                destination: Zone::Hand,
            },
            Effect::PutRestOfLookedAt {
                player: PlayerRef::You,
                destination: Zone::Graveyard,
            },
        ]);
        return Ok(true);
    }
    if lower == "put one of them into your hand and the rest into your graveyard." {
        parsed.effects.extend([
            Effect::SelectFromLookedAt {
                player: PlayerRef::You,
                amount: Amount::Constant(1),
                predicate: ObjectFilter::default(),
                reveal: false,
                face_down: false,
                tapped: false,
                destination: Zone::Hand,
            },
            Effect::PutRestOfLookedAt {
                player: PlayerRef::You,
                destination: Zone::Graveyard,
            },
        ]);
        return Ok(true);
    }
    if lower == "put the rest on the bottom of your library in any order." {
        parsed.effects.push(Effect::PutRestOnLibraryBottom {
            player: PlayerRef::You,
            order: BottomOrder::AnyOrder,
        });
        return Ok(true);
    }
    if lower == "put the rest on the bottom of your library in a random order." {
        parsed.effects.push(Effect::PutRestOnLibraryBottom {
            player: PlayerRef::You,
            order: BottomOrder::ReplaySeededRandom,
        });
        return Ok(true);
    }
    if lower == "its controller manifests the top card of their library." {
        let object = state
            .last_object
            .clone()
            .ok_or_else(|| unsupported(address, statement))?;
        let player = PlayerRef::ControllerOf(Box::new(object));
        parsed.effects.push(Effect::Manifest {
            player: player.clone(),
            card: ObjectRef::TopCard {
                player: Box::new(player),
            },
        });
        return Ok(true);
    }
    if lower == "manifest the top card of your library." {
        parsed.effects.push(Effect::Manifest {
            player: PlayerRef::You,
            card: ObjectRef::TopCard {
                player: Box::new(PlayerRef::You),
            },
        });
        return Ok(true);
    }
    if lower == "choose a creature type." {
        parsed.effects.push(Effect::ChooseCreatureType {
            player: PlayerRef::You,
        });
        return Ok(true);
    }
    if lower == "choose a color." {
        parsed.effects.push(Effect::ChooseColor);
        return Ok(true);
    }
    if lower == "choose a card name." {
        parsed
            .effects
            .push(Effect::ChooseCardName { nonland: false });
        return Ok(true);
    }
    if lower == "choose a nonland card name." {
        parsed
            .effects
            .push(Effect::ChooseCardName { nonland: true });
        return Ok(true);
    }
    Ok(false)
}

fn parse_conditional_statement(
    address: ClauseAddress,
    statement: &str,
    state: &mut EffectParseState,
    parsed: &mut ParsedClause,
) -> Result<(), CompileError> {
    let lower = statement.to_ascii_lowercase();
    let conditional = lower
        .strip_prefix("then if ")
        .or_else(|| lower.strip_prefix("if "));
    let Some(conditional) = conditional else {
        return Err(unsupported(address, statement));
    };
    let Some((condition_text, effect_text)) = conditional.split_once(',') else {
        return Err(unsupported(address, statement));
    };
    let condition = parse_condition(&format!("if {condition_text}"))
        .ok_or_else(|| unsupported(address, statement))?;
    let effect_text = effect_text.trim();
    let replacement = effect_text.ends_with(" instead.");
    let effect_text = effect_text
        .strip_suffix(" instead.")
        .map(|text| format!("{text}."))
        .unwrap_or_else(|| effect_text.to_string());
    let mut nested = ParsedClause::new(Timing::SpellResolution);
    let mut nested_state = EffectParseState {
        next_target_id: state.next_target_id,
        next_selection_id: state.next_selection_id,
        last_object: state.last_object.clone(),
        last_player: state.last_player.clone(),
        pending_hand_choice: state.pending_hand_choice.clone(),
        selected_targets: state.selected_targets.clone(),
        source_attachment_kind: state.source_attachment_kind,
    };
    parse_effect_statement(address, &effect_text, &mut nested_state, &mut nested)?;
    state.next_target_id = nested_state.next_target_id;
    state.next_selection_id = nested_state.next_selection_id;
    for target in nested.targets {
        parsed.targets.push(target);
    }
    parsed
        .predefined_token_creations
        .extend(nested.predefined_token_creations.iter().copied());

    if replacement {
        let Some(Effect::CreateToken(replacement_creation)) = nested.effects.first().cloned()
        else {
            return Err(unsupported(address, statement));
        };
        let Some(previous_index) = parsed
            .effects
            .iter()
            .rposition(|effect| matches!(effect, Effect::CreateToken(_)))
        else {
            return Err(unsupported(address, statement));
        };
        let Effect::CreateToken(ordinary) = parsed.effects.remove(previous_index) else {
            unreachable!()
        };
        parsed.effects.push(Effect::Replacement(Box::new(
            ReplacementEffect::ConditionalTokenSubstitution {
                condition: condition.clone(),
                ordinary,
                replacement: Box::new(replacement_creation),
            },
        )));
        parsed.conditions.push(condition);
    } else {
        parsed.effects.push(Effect::Conditional {
            condition,
            if_true: nested.effects,
            if_false: Vec::new(),
        });
    }
    Ok(())
}
