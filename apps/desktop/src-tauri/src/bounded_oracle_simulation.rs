//! Simulation-facing bridge for the bounded Oracle executor.
//!
//! The parser and consumer deliberately do not know about the trajectory
//! simulator. This module is the narrow integration layer between them. It
//! binds compiled clauses to stable physical object identities, exposes the
//! simulator's resolution and event entry points, and returns a deterministic
//! typed delta after every committed batch.

use std::collections::{BTreeMap, BTreeSet};

use crate::ability_clause_bridge::{AbilityClauseTimingEnvelope, AbilityClauseTriggerEventKind};
use crate::bounded_oracle_consumer::{
    ActionDefinition, ActionWindow, ActivationReductionRecord, AttachmentRecord,
    CastPermissionRecord, ContinuousEffectRecord, DelayedTriggerRecord, ExecutionContext,
    ExecutionError, ExecutionReceipt, ExecutionStatus, ExtraTurnRecord, GameResultRecord,
    InMemoryOracleState, ObjectCharacteristics, ObjectId, OracleStateAdapter, PaymentOrLoseRecord,
    PhysicalObject, PlayerId, PlayerState, ReplacementRecord, RestrictionRecord,
    RevealedCardRecord, ScheduledCopyRecord, SkippedStepRecord, SpellReductionRecord, TriggerEvent,
    authorize_global_alternative_spell_cost, clause_has_executable_contract,
    commit_regeneration_runtime_state, effective_object, execute_action,
    execute_active_level_trigger, execute_clause, execute_granted_ability, object_can_attack,
    object_can_be_blocked, object_can_block, object_can_untap_during,
    object_must_attack_each_combat, pay_reduced_spell_mana_cost, synchronized_library_access_state,
    synchronized_regeneration_runtime_state, trigger_matches,
};
use crate::bounded_oracle_runtime::{
    AttachmentKind, BoundedOracleClause, CardType, ClauseAddress, Color, Comparison, Condition,
    CounterKind, Duration, Effect, Keyword, ObjectRef, OracleFaceModalLineProgram, Restriction,
    StandaloneRuleProgram, Step, Supertype, Timing, Zone,
};
use crate::combat_restriction_runtime::{
    AttackDeclaration as RestrictionAttackDeclaration, BattlefieldPermanent, BattlefieldSnapshot,
    BlockAssignment as RestrictionBlockAssignment, CombatLegalityReport,
    ObjectRef as CombatRestrictionObjectRef, PermanentSubtype,
};
use crate::damage_transaction_runtime::{
    DamageEventMatcher, DamageKindMatcher, DamageModifier, DamageModifierOperation,
    DamageModifierPersistence, DamageModifierRequirement, DamagePrevention, DamageRecipient,
    DamageRecipientMatcher, DamageSourceKeyword, DamageSourceMatcher,
};
use crate::entry_choice_keyword_runtime::{
    EntryAttemptEvidence, EntryChoiceKeywordBinding, EntryChoiceKeywordResolution,
    EntryChoiceResolutionInput, PendingRavenousDrawTrigger, RavenousTriggerId,
    begin_entry_choice_keyword_transaction,
};
use crate::graveyard_transform_keyword_runtime::{
    CardLayout as GraveyardCardLayout, CraftActivationEvidence, CraftActivationReceipt,
    CraftResolutionReceipt, DisturbCastEvidence, DisturbCastPermissionEvidence, DisturbCastReceipt,
    DisturbResolutionReceipt, FaceCharacteristics as GraveyardFaceCharacteristics,
    FaceId as GraveyardFaceId, FaceSemanticContext as GraveyardFaceSemanticContext,
    GraveyardTransformKeywordKind, GraveyardTransformKeywordProgram,
    GraveyardTransformKeywordRuntime, ManaColor as GraveyardManaColor,
    ManaPaymentEvidence as GraveyardManaPaymentEvidence, ManaUnit as GraveyardManaUnit,
    ManaUnitId as GraveyardManaUnitId, ObjectId as GraveyardObjectId,
    ObjectRef as GraveyardObjectRef, OtherCastCostEvidence as GraveyardOtherCastCostEvidence,
    PendingCraftAbilityId, PendingSoulshiftTriggerId, Phase as GraveyardPhase,
    PhysicalCardDefinition as GraveyardCardDefinition, PlayerId as GraveyardPlayerId,
    PlayerState as GraveyardPlayerState, PriorityWindow as GraveyardPriorityWindow,
    SoulshiftDeathReceipt, SoulshiftResolutionChoice, SoulshiftResolutionReceipt,
    SoulshiftTargetDeclaration, SourceSemanticContext as GraveyardSourceSemanticContext,
    TrackedObject as GraveyardTrackedObject, Zone as GraveyardZone,
    ZoneChangeReceipt as GraveyardZoneChangeReceipt,
    ZoneChangeReplacementEvidence as GraveyardReplacementEvidence,
};
use crate::keyword_production_bridge::{
    COMBAT_EVASION_PRODUCTION_BRIDGE_VERSION, CombatEvasionProductionBridgeError,
    StaticKeywordObjectBinding, evaluate_combat_evasion_keywords,
    validate_combat_evasion_program_set,
};
use crate::keyword_rules_runtime::{
    CardType as KeywordCardType, KeywordProgram, KeywordReceipt, ManaColor as KeywordManaColor,
    ObjectCharacteristics as KeywordObjectCharacteristics, ObjectId as KeywordObjectId,
    OfficialKeyword, PlayerId as KeywordPlayerId, RegenerationChoice, Zone as KeywordZone,
};
use crate::library_access_runtime::{
    LibraryAccessAuthorization, LibraryAccessError, LibraryTopAction,
};
use crate::object_state_clause_runtime::ObjectStateClauseKind;
use crate::old_transform_runtime::{
    OldTransformResolution, OldTransformRuntime, OldTransformZone, TransformableObject,
};
use crate::oracle_clause_backend::{
    DelegatedKeywordClause, LiveBridgeCapability, ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION,
};
use crate::oracle_static_replacement_runtime::{
    CardType as StaticCardType, Color as StaticColor, CounterKind as StaticCounterKind,
    DamageKind as StaticDamageKind, EffectiveCharacteristics as StaticEffectiveCharacteristics,
    IncarnationId as StaticIncarnationId, KeywordAbility as StaticKeyword,
    ObjectRef as StaticObjectRef, ObjectState as StaticObjectState,
    OracleStaticReplacementProgramKind, OracleStaticReplacementRuntime,
    PendingReplacementEvent as StaticPendingReplacementEvent,
    ReplacementDecision as StaticReplacementDecision,
    ReplacementOperation as StaticReplacementOperation,
    ReplacementOrderEvidence as StaticReplacementOrderEvidence,
    ReplacementStep as StaticReplacementStep, RuntimeEvent as StaticRuntimeEvent,
    RuntimeRecipient as StaticRuntimeRecipient, RuntimeSnapshot as StaticRuntimeSnapshot,
    Supertype as StaticSupertype, Zone as StaticZone,
};
use crate::pregame_clause_runtime::{
    DueEffectId as PregameDueEffectId, DuePregameEffect, ObjectRef as PregameObjectRef,
    OpeningHandDecision, OpeningHandEvidence, OpeningHandProgram, PregameAction, PregameClauseKind,
    PregameClauseProgram, PregameCounterKind, PregameDelayedEffectRegistry, PregameEvent,
    PregameResolutionTransaction, PregameTokenDefinition, PregameTransaction, PregameZone,
    RevealEffect as PregameRevealEffect, prepare_opening_hand_transaction,
};
use crate::printed_cost_runtime::{
    PRINTED_COST_PAYMENT_BRIDGE_VERSION, PrintedManaCost, PrintedManaPaymentChoices,
    PrintedManaPaymentError, PrintedManaPaymentReceipt, PrintedManaPaymentResources,
    pay_printed_mana_cost, printed_mana_cost_has_exact_payment_contract,
};
use crate::regeneration_action_runtime::{
    IncarnationId as RegenerationIncarnationId, ObjectReference as RegenerationObjectReference,
    PendingRegenerationAction, RegenerationActionKind, RegenerationActionProgram,
    RegenerationActivationPayment, RegenerationActivationReceipt, RegenerationDestructionOutcome,
    RegenerationResolutionReceipt, RegenerationRuntimeState, RegenerationStackActionId,
    RegenerationTriggerEvent, begin_regeneration_activation, begin_regeneration_trigger,
    compile_regeneration_action_program, compile_regeneration_resolution_leaf_program,
    resolve_pending_regeneration, resolve_regeneration_destruction,
    resolve_regeneration_instruction,
};
use crate::residual_cost_keyword_runtime::{
    AbilityInstanceId as ResidualAbilityInstanceId, CardType as ResidualCardType,
    GameObject as ResidualGameObject, IncarnationId as ResidualIncarnationId,
    ManaColor as ResidualManaColor, ManaUnit as ResidualManaUnit, ManaUnitId as ResidualManaUnitId,
    ObjectCharacteristics as ResidualCharacteristics, ObjectId as ResidualObjectId,
    ObjectRef as ResidualObjectRef, PaymentId as ResidualPaymentId, PendingWardTrigger,
    PlayerId as ResidualPlayerId, PlayerState as ResidualPlayerState, ResidualCostGameState,
    ResidualCostKeywordKind, StackIncarnationId as ResidualStackIncarnationId,
    StackObject as ResidualStackObject, StackObjectId as ResidualStackObjectId,
    StackObjectKind as ResidualStackObjectKind, StackObjectRef as ResidualStackObjectRef,
    StackObjectStatus as ResidualStackObjectStatus, Supertype as ResidualSupertype, TargetEvent,
    TargetEventId, TriggerBatchId, WardAbilityInstance, WardPaymentEvidence, WardResolution,
    WardTriggerId, WardTriggerOrder, Zone as ResidualZone, resolve_ward_trigger,
};
use crate::saga_transform_runtime::{
    SagaTransformFaceRole, SagaTransformMovementReplacement, SagaTransformObject,
    SagaTransformOutcome, SagaTransformResolution, SagaTransformRuntime, SagaTransformZone,
};
use crate::semantics::CompiledCard;

pub const BOUNDED_ORACLE_SIMULATION_BRIDGE_VERSION: &str = "bounded-oracle-simulation-bridge-0.75";
pub(crate) const COMBAT_BLOCK_DECLARATION_PRODUCTION_BRIDGE_VERSION: &str =
    "bounded-combat-block-declaration-bridge/v1";

const COMBAT_BLOCK_LEGALITY_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatBlockLegality,
];
const INTIMIDATE_BLOCK_LEGALITY_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::EffectiveColorCharacteristics,
    LiveBridgeCapability::CombatBlockLegality,
];
const DAMAGE_SOURCE_KEYWORD_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::DamageEventModification,
    LiveBridgeCapability::CounterLifecycle,
];

fn static_replacement_object(object: &PhysicalObject) -> Result<StaticObjectState, ExecutionError> {
    let characteristics = object.characteristics();
    let card_types = characteristics
        .card_types
        .iter()
        .filter_map(|card_type| match card_type {
            CardType::Artifact => Some(StaticCardType::Artifact),
            CardType::Battle => Some(StaticCardType::Battle),
            CardType::Creature => Some(StaticCardType::Creature),
            CardType::Enchantment => Some(StaticCardType::Enchantment),
            CardType::Instant => Some(StaticCardType::Instant),
            CardType::Land => Some(StaticCardType::Land),
            CardType::Planeswalker => Some(StaticCardType::Planeswalker),
            CardType::Sorcery => Some(StaticCardType::Sorcery),
            CardType::Spell | CardType::Permanent => None,
        })
        .collect();
    let colors = characteristics
        .colors
        .iter()
        .map(|color| match color {
            Color::White => StaticColor::White,
            Color::Blue => StaticColor::Blue,
            Color::Black => StaticColor::Black,
            Color::Red => StaticColor::Red,
            Color::Green => StaticColor::Green,
            Color::Colorless => StaticColor::Colorless,
        })
        .collect();
    let supertypes = characteristics
        .supertypes
        .iter()
        .map(|supertype| match supertype {
            Supertype::Basic => StaticSupertype::Basic,
            Supertype::Legendary => StaticSupertype::Legendary,
            Supertype::Snow => StaticSupertype::Snow,
            Supertype::Nonbasic => StaticSupertype::Nonbasic,
        })
        .collect();
    let keywords = characteristics
        .keywords
        .iter()
        .filter_map(|keyword| match keyword {
            Keyword::Changeling => None,
            Keyword::Deathtouch => Some(StaticKeyword::Deathtouch),
            Keyword::Defender => Some(StaticKeyword::Defender),
            Keyword::DoubleStrike => Some(StaticKeyword::DoubleStrike),
            Keyword::FirstStrike => Some(StaticKeyword::FirstStrike),
            Keyword::Flying => Some(StaticKeyword::Flying),
            Keyword::Haste => Some(StaticKeyword::Haste),
            Keyword::Hexproof => Some(StaticKeyword::Hexproof),
            Keyword::Indestructible => Some(StaticKeyword::Indestructible),
            Keyword::Lifelink => Some(StaticKeyword::Lifelink),
            Keyword::Menace => Some(StaticKeyword::Menace),
            Keyword::Reach => Some(StaticKeyword::Reach),
            Keyword::Shroud => Some(StaticKeyword::Shroud),
            Keyword::Trample => Some(StaticKeyword::Trample),
            Keyword::Vigilance => Some(StaticKeyword::Vigilance),
            Keyword::Ward(_) => Some(StaticKeyword::Ward),
            Keyword::Shadow => Some(StaticKeyword::Shadow),
        })
        .collect();
    let counters = object
        .counters
        .iter()
        .map(|(counter, amount)| {
            let kind = match counter.as_str() {
                "+1/+1" => StaticCounterKind::PlusOnePlusOne,
                "-1/-1" => StaticCounterKind::MinusOneMinusOne,
                "loyalty" => StaticCounterKind::Loyalty,
                "charge" => StaticCounterKind::Charge,
                other => StaticCounterKind::Named(other.to_owned()),
            };
            (kind, *amount)
        })
        .collect();
    Ok(StaticObjectState {
        object_ref: StaticObjectRef {
            object_id: object.id,
            incarnation_id: StaticIncarnationId(object.origin_id.max(1)),
        },
        owner: u16::from(object.owner),
        controller: u16::from(object.controller),
        zone: match object.zone {
            Zone::Library => StaticZone::Library,
            Zone::Hand => StaticZone::Hand,
            Zone::Battlefield => StaticZone::Battlefield,
            Zone::Graveyard => StaticZone::Graveyard,
            Zone::Exile => StaticZone::Exile,
            Zone::Stack => StaticZone::Stack,
            Zone::Command => StaticZone::Command,
            Zone::Merged => unreachable!("merged components are not independent objects"),
        },
        names: characteristics.names.iter().cloned().collect(),
        card_types,
        supertypes,
        colors,
        subtypes: characteristics.subtypes.iter().cloned().collect(),
        enchanting_sources: BTreeSet::new(),
        equipping_sources: BTreeSet::new(),
        counters,
        keywords,
        token: object.token,
        tapped: object.tapped,
        attacking: object.attacking,
        blocking: object.blocking,
        mana_value: characteristics.mana_value,
        power: Some(
            i32::try_from(characteristics.power).map_err(|_| {
                ExecutionError::Adapter("power exceeds static runtime range".into())
            })?,
        ),
        toughness: Some(i32::try_from(characteristics.toughness).map_err(|_| {
            ExecutionError::Adapter("toughness exceeds static runtime range".into())
        })?),
    })
}

pub fn clause_has_live_bridge_contract(clause: &BoundedOracleClause) -> bool {
    (clause_has_executable_contract(clause)
        || old_transform_clause_has_live_bridge_contract(clause)
        || saga_transform_clause_has_live_bridge_contract(clause)
        || entry_choice_clause_has_live_bridge_contract(clause)
        || deck_construction_clause_has_live_bridge_contract(clause)
        || regeneration_action_clause_has_live_bridge_contract(clause)
        || graveyard_transform_clause_has_live_bridge_contract(clause)
        || oracle_static_replacement_clause_has_live_bridge_contract(clause)
        || residual_cost_clause_has_live_bridge_contract(clause))
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
                | Timing::TypedStandaloneProgram
                | Timing::SpecialAction(_)
        )
}

fn oracle_face_modal_line_program(
    clause: &BoundedOracleClause,
) -> Option<&OracleFaceModalLineProgram> {
    let [Effect::StandaloneRuleProgram(StandaloneRuleProgram::OracleFaceModalLine(program))] =
        clause.effects()
    else {
        return None;
    };
    Some(program)
}

/// Whether this clause can participate in the simulator's complete spell-cost
/// payment batch. The batch, rather than an individual `execute_clause` call,
/// is the production contract that binds the printed mana cost and every
/// retained additional-cost clause to one rollback checkpoint.
pub fn casting_additional_cost_has_atomic_payment_contract(clause: &BoundedOracleClause) -> bool {
    matches!(clause.timing(), Timing::CastingAdditionalCost)
        && !clause.costs().is_empty()
        && clause_has_executable_contract(clause)
}

fn oracle_static_replacement_clause_has_live_bridge_contract(clause: &BoundedOracleClause) -> bool {
    let [
        Effect::StandaloneRuleProgram(
            crate::bounded_oracle_runtime::StandaloneRuleProgram::OracleStaticReplacement(program),
        ),
    ] = clause.effects()
    else {
        return false;
    };
    matches!(clause.timing(), Timing::Static | Timing::Replacement)
        && program.production_adapter_connected()
        && !program.semantic_digest().is_empty()
}

fn residual_cost_clause_has_live_bridge_contract(clause: &BoundedOracleClause) -> bool {
    let [
        Effect::StandaloneRuleProgram(
            crate::bounded_oracle_runtime::StandaloneRuleProgram::ResidualCostKeyword(program),
        ),
    ] = clause.effects()
    else {
        return false;
    };
    matches!(clause.timing(), Timing::Triggered(_))
        && matches!(program.kind(), ResidualCostKeywordKind::Ward(_))
        && !program.semantic_digest().is_empty()
}

fn exact_delegated_damage_source_keywords(
    card: &CompiledCard,
) -> Result<BTreeMap<u8, BTreeSet<DamageSourceKeyword>>, ExecutionError> {
    let mut by_face = BTreeMap::<u8, BTreeSet<DamageSourceKeyword>>::new();
    for clause in &card.effects.delegated_oracle {
        let keyword = match clause.keyword_program().keyword() {
            OfficialKeyword::Infect => DamageSourceKeyword::Infect,
            OfficialKeyword::Wither => DamageSourceKeyword::Wither,
            _ => continue,
        };
        let address = clause.address();
        let program = clause.keyword_program();
        if clause.runtime_version() != ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION
            || clause.required_live_bridge_capabilities() != DAMAGE_SOURCE_KEYWORD_CAPABILITIES
            || program.source().face_index != address.face_index
            || program.source().clause_index != address.clause_index
        {
            return Err(ExecutionError::Adapter(
                "delegated damage keyword program is not exact".into(),
            ));
        }
        let face_index = u8::try_from(address.face_index).map_err(|_| {
            ExecutionError::Adapter("damage keyword face index exceeds runtime range".into())
        })?;
        by_face.entry(face_index).or_default().insert(keyword);
    }
    Ok(by_face)
}

fn exact_affinity_faces(card: &CompiledCard) -> Result<BTreeSet<u8>, ExecutionError> {
    let mut faces = BTreeSet::new();
    for clause in &card.effects.delegated_oracle {
        if clause.keyword_program().keyword() != OfficialKeyword::Affinity {
            continue;
        }
        if clause.runtime_version() != ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION
            || clause.keyword_program().source().face_index != clause.address().face_index
            || clause.keyword_program().source().clause_index != clause.address().clause_index
        {
            return Err(ExecutionError::Adapter(
                "delegated Affinity program is not exact".into(),
            ));
        }
        faces.insert(u8::try_from(clause.address().face_index).map_err(|_| {
            ExecutionError::Adapter("Affinity face index exceeds runtime range".into())
        })?);
    }
    for clause in &card.effects.bounded_oracle {
        let [
            Effect::StandaloneRuleProgram(
                crate::bounded_oracle_runtime::StandaloneRuleProgram::ResidualCostKeyword(program),
            ),
        ] = clause.effects()
        else {
            continue;
        };
        if !matches!(program.kind(), ResidualCostKeywordKind::Affinity(_)) {
            continue;
        }
        if program.semantic_digest().is_empty() {
            return Err(ExecutionError::Adapter(
                "Affinity program has no exact semantic identity".into(),
            ));
        }
        faces.insert(u8::try_from(clause.address().face_index).map_err(|_| {
            ExecutionError::Adapter("Affinity face index exceeds runtime range".into())
        })?);
    }
    Ok(faces)
}

fn graveyard_transform_clause_has_live_bridge_contract(clause: &BoundedOracleClause) -> bool {
    let [
        Effect::StandaloneRuleProgram(
            crate::bounded_oracle_runtime::StandaloneRuleProgram::GraveyardTransformKeyword(
                program,
            ),
        ),
    ] = clause.effects()
    else {
        return false;
    };
    program.production_adapter_connected()
        && match (clause.timing(), program.kind()) {
            (Timing::Triggered(_), GraveyardTransformKeywordKind::Soulshift(_)) => true,
            (Timing::TypedStandaloneProgram, GraveyardTransformKeywordKind::Disturb(disturb)) => {
                !disturb.alternative_cost.symbols.iter().any(|symbol| {
                    matches!(
                        symbol,
                        crate::graveyard_transform_keyword_runtime::ManaSymbol::Snow
                    )
                })
            }
            (Timing::Activated, GraveyardTransformKeywordKind::Craft(craft)) => {
                !craft.activation_cost.symbols.iter().any(|symbol| {
                    matches!(
                        symbol,
                        crate::graveyard_transform_keyword_runtime::ManaSymbol::Snow
                    )
                })
            }
            _ => false,
        }
}

fn regeneration_action_clause_has_live_bridge_contract(clause: &BoundedOracleClause) -> bool {
    match (clause.timing(), clause.effects()) {
        (
            Timing::TypedStandaloneProgram,
            [
                crate::bounded_oracle_runtime::Effect::StandaloneRuleProgram(
                    crate::bounded_oracle_runtime::StandaloneRuleProgram::RegenerationAction(
                        program,
                    ),
                ),
            ],
        ) => regeneration_action_program_has_live_bridge_contract(program),
        (
            Timing::Activated,
            [
                crate::bounded_oracle_runtime::Effect::StandaloneRuleProgram(
                    crate::bounded_oracle_runtime::StandaloneRuleProgram::RegenerationAction(
                        program,
                    ),
                ),
            ],
        ) if matches!(
            program.kind(),
            RegenerationActionKind::StandaloneResolution(_)
        ) =>
        {
            normalized_regeneration_activation_program(clause).is_some_and(|program| {
                matches!(program.kind(), RegenerationActionKind::Activated(_))
            })
        }
        _ => false,
    }
}

fn normalized_regeneration_activation_program(
    clause: &BoundedOracleClause,
) -> Option<RegenerationActionProgram> {
    let normalized = clause
        .normalized_clause()
        .replace("Regenerate this object", "Regenerate this creature");
    compile_regeneration_action_program(&normalized, &normalized)
}

fn regeneration_action_program_has_live_bridge_contract(
    program: &RegenerationActionProgram,
) -> bool {
    !program.semantic_digest().is_empty()
        && match program.kind() {
            RegenerationActionKind::Triggered(_) => true,
            RegenerationActionKind::Activated(_) => true,
            RegenerationActionKind::StaticDestructionReplacement(_)
            | RegenerationActionKind::StandaloneResolution(_) => false,
        }
}

fn deck_construction_clause_has_live_bridge_contract(clause: &BoundedOracleClause) -> bool {
    let [
        Effect::StandaloneRuleProgram(
            crate::bounded_oracle_runtime::StandaloneRuleProgram::Pregame(program),
        ),
    ] = clause.effects()
    else {
        return false;
    };
    !program.semantic_digest().is_empty()
        && match (clause.timing(), program.kind()) {
            (
                Timing::TypedStandaloneProgram,
                PregameClauseKind::ExplicitSelfCommanderPermission
                | PregameClauseKind::RemoveFromDeckWithoutAnte
                | PregameClauseKind::DeckCopyLimit(_),
            ) => true,
            (
                Timing::SpecialAction(crate::bounded_oracle_runtime::SpecialActionTiming::Pregame),
                PregameClauseKind::OpeningHand(OpeningHandProgram::BeginOnBattlefield(_))
                | PregameClauseKind::OpeningHand(OpeningHandProgram::BeginInGraveyard(_))
                | PregameClauseKind::OpeningHand(OpeningHandProgram::RevealAndSchedule(_)),
            ) => true,
            _ => false,
        }
}

fn entry_choice_clause_has_live_bridge_contract(clause: &BoundedOracleClause) -> bool {
    matches!(
        (clause.timing(), clause.effects()),
        (
            Timing::TypedStandaloneProgram,
            [crate::bounded_oracle_runtime::Effect::StandaloneRuleProgram(
                crate::bounded_oracle_runtime::StandaloneRuleProgram::EntryChoiceKeyword(program),
            )]
        ) if !program.semantic_digest().is_empty()
    )
}

fn saga_transform_clause_has_live_bridge_contract(clause: &BoundedOracleClause) -> bool {
    matches!(
        (clause.timing(), clause.effects()),
        (
            Timing::Triggered(trigger),
            [crate::bounded_oracle_runtime::Effect::StandaloneRuleProgram(
                crate::bounded_oracle_runtime::StandaloneRuleProgram::SagaTransform(program),
            )]
        ) if matches!(
            trigger.as_ref(),
            crate::bounded_oracle_runtime::Trigger::SagaChapterReached { chapter: 3 }
        ) && program.source_context().is_complete()
            && !program.semantic_digest().is_empty()
    )
}

fn old_transform_clause_has_live_bridge_contract(clause: &BoundedOracleClause) -> bool {
    matches!(
        (clause.timing(), clause.effects()),
        (
            Timing::Triggered(trigger),
            [crate::bounded_oracle_runtime::Effect::StandaloneRuleProgram(
                crate::bounded_oracle_runtime::StandaloneRuleProgram::OldTransform(program),
            )]
        ) if matches!(
            trigger.as_ref(),
            crate::bounded_oracle_runtime::Trigger::BeginningOf {
                player: crate::bounded_oracle_runtime::TurnPlayer::EachPlayer,
                step: Step::Upkeep,
            }
        ) && !program.semantic_digest().is_empty()
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCostPaymentBatch {
    pub source: ObjectId,
    pub source_incarnation: u64,
    pub player: PlayerId,
    pub printed_cost: crate::bounded_oracle_runtime::ManaCost,
    pub reduced_cost: crate::bounded_oracle_runtime::ManaCost,
    pub generic_reduction: u32,
    pub additional_cost_receipts: Vec<(ObjectId, ClauseAddress, ExecutionReceipt)>,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalAlternativeSpellCostPaymentBatch {
    pub permission_source: ObjectId,
    pub permission_source_incarnation: u64,
    pub source: ObjectId,
    pub source_incarnation: u64,
    pub player: PlayerId,
    pub cast_from: Zone,
    pub alternative_cost: crate::bounded_oracle_runtime::ManaCost,
    pub reduced_cost: crate::bounded_oracle_runtime::ManaCost,
    pub generic_reduction: u32,
    pub additional_cost_receipts: Vec<(ObjectId, ClauseAddress, ExecutionReceipt)>,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaTransformBatch {
    pub source: ObjectId,
    pub clause_address: ClauseAddress,
    pub resolution: Option<SagaTransformResolution>,
    pub receipt: ExecutionReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryChoiceKeywordBatch {
    pub source: ObjectId,
    pub clause_addresses: Vec<ClauseAddress>,
    pub resolution: EntryChoiceKeywordResolution,
    pub receipt: ExecutionReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RavenousDrawBatch {
    pub trigger: PendingRavenousDrawTrigger,
    pub drawn_card: Option<ObjectId>,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegenerationActivationBatch {
    pub receipt: RegenerationActivationReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegenerationTriggerBatch {
    pub action: PendingRegenerationAction,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegenerationResolutionBatch {
    pub receipt: RegenerationResolutionReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningHandBatch {
    pub transaction: PregameTransaction,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PregameResolutionChoices {
    pub library_order_after_shuffle: Option<Vec<ObjectId>>,
    pub keep_on_top: Option<ObjectId>,
    pub scry_to_bottom: Vec<ObjectId>,
    pub ward_payment: Option<PregameSpellWardPayment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PregameSpellWardPayment {
    Decline,
    Unrestricted,
    Colored(Color),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PregameResolutionBatch {
    pub transaction: PregameResolutionTransaction,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoulshiftDeathBatch {
    pub receipt: SoulshiftDeathReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoulshiftResolutionBatch {
    pub receipt: SoulshiftResolutionReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisturbCastBatch {
    pub receipt: DisturbCastReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisturbResolutionBatch {
    pub receipt: DisturbResolutionReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraveyardTransformMoveBatch {
    pub receipt: GraveyardZoneChangeReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftActivationBatch {
    pub receipt: CraftActivationReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftResolutionBatch {
    pub receipt: CraftResolutionReceipt,
    pub delta: SimulationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WardTriggerBatch {
    pub pending: Vec<PendingWardTrigger>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WardResolutionBatch {
    pub resolution: WardResolution,
    pub delta: SimulationDelta,
}

/// Owns the bounded state and the exact program bound to each physical object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedOracleSimulation {
    state: InMemoryOracleState,
    programs: BTreeMap<ObjectId, Vec<BoundedOracleClause>>,
    combat_evasion_programs: BTreeMap<ObjectId, Vec<DelegatedKeywordClause>>,
    delegated_regeneration_programs: BTreeMap<ObjectId, Vec<DelegatedKeywordClause>>,
    old_transform: OldTransformRuntime,
    old_transform_incarnations: BTreeMap<ObjectId, u64>,
    saga_transform: SagaTransformRuntime,
    saga_transform_incarnations: BTreeMap<ObjectId, u64>,
    static_replacement: OracleStaticReplacementRuntime,
    pending_ravenous_draws: BTreeMap<RavenousTriggerId, PendingRavenousDrawTrigger>,
    graveyard_transform: GraveyardTransformKeywordRuntime,
    pregame_delayed: PregameDelayedEffectRegistry,
    residual_cost: ResidualCostGameState,
    residual_ward_abilities: BTreeMap<(ObjectId, ClauseAddress), ResidualAbilityInstanceId>,
    next_residual_runtime_id: u64,
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
            delegated_regeneration_programs: BTreeMap::new(),
            old_transform: OldTransformRuntime::default(),
            old_transform_incarnations: BTreeMap::new(),
            saga_transform: SagaTransformRuntime::default(),
            saga_transform_incarnations: BTreeMap::new(),
            static_replacement: OracleStaticReplacementRuntime::new(),
            pending_ravenous_draws: BTreeMap::new(),
            graveyard_transform: GraveyardTransformKeywordRuntime::new(),
            pregame_delayed: PregameDelayedEffectRegistry::default(),
            residual_cost: ResidualCostGameState::default(),
            residual_ward_abilities: BTreeMap::new(),
            next_residual_runtime_id: 1,
        }
    }

    pub fn state(&self) -> &InMemoryOracleState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut InMemoryOracleState {
        &mut self.state
    }

    pub fn static_replacement_runtime(&self) -> &OracleStaticReplacementRuntime {
        &self.static_replacement
    }

    fn static_replacement_reference(&self, object_id: ObjectId) -> StaticObjectRef {
        let incarnation = self
            .state
            .object(object_id)
            .map(|object| object.origin_id.max(1))
            .unwrap_or(object_id.max(1));
        StaticObjectRef {
            object_id,
            incarnation_id: StaticIncarnationId(incarnation),
        }
    }

    pub fn static_replacement_snapshot(
        &self,
        perspective_player: PlayerId,
        active_player: PlayerId,
    ) -> Result<StaticRuntimeSnapshot, ExecutionError> {
        let mut objects = self
            .state
            .object_ids()
            .into_iter()
            .map(|object_id| {
                let object = self
                    .state
                    .object(object_id)
                    .ok_or(ExecutionError::MissingObject(object_id))?;
                let projected = static_replacement_object(&object)?;
                Ok((projected.object_ref, projected))
            })
            .collect::<Result<BTreeMap<_, _>, ExecutionError>>()?;
        for source in self.state.attachments.keys().copied().collect::<Vec<_>>() {
            let Some(attachment) = self.state.attachment(source) else {
                continue;
            };
            let source_ref = self.static_replacement_reference(attachment.source);
            let target_ref = self.static_replacement_reference(attachment.target);
            let target = objects
                .get_mut(&target_ref)
                .ok_or(ExecutionError::MissingObject(attachment.target))?;
            match attachment.kind {
                AttachmentKind::Aura => {
                    target.enchanting_sources.insert(source_ref);
                }
                AttachmentKind::Equipment => {
                    target.equipping_sources.insert(source_ref);
                }
            }
        }
        let mut life_totals = BTreeMap::new();
        let mut complete_players = BTreeSet::new();
        for player_id in self.state.player_ids() {
            let player = self
                .state
                .player(player_id)
                .ok_or(ExecutionError::MissingPlayer(player_id))?;
            life_totals.insert(
                u16::from(player_id),
                i32::try_from(player.life).map_err(|_| {
                    ExecutionError::Adapter("life total exceeds static runtime range".into())
                })?,
            );
            complete_players.insert(u16::from(player_id));
        }
        Ok(StaticRuntimeSnapshot {
            perspective_player: u16::from(perspective_player),
            active_player: u16::from(active_player),
            objects,
            life_totals,
            x_values: BTreeMap::new(),
            kicker_payments: BTreeMap::new(),
            complete_players,
            complete_zones: BTreeSet::from([
                StaticZone::Library,
                StaticZone::Hand,
                StaticZone::Battlefield,
                StaticZone::Graveyard,
                StaticZone::Exile,
                StaticZone::Command,
                StaticZone::Stack,
            ]),
            legal_creature_types: BTreeSet::new(),
        })
    }

    /// Evaluate all installed typed continuous effects against the current
    /// bounded host state. The projection is rebuilt for every query so stale
    /// characteristics cannot leak across host mutations.
    pub fn effective_static_characteristics(
        &self,
        object_id: ObjectId,
        perspective_player: PlayerId,
        active_player: PlayerId,
    ) -> Result<StaticEffectiveCharacteristics, ExecutionError> {
        let snapshot = self.static_replacement_snapshot(perspective_player, active_player)?;
        self.static_replacement
            .effective_characteristics(&snapshot, self.static_replacement_reference(object_id))
            .map_err(|error| ExecutionError::Adapter(format!("{error:?}")))
    }

    /// Start an exact typed replacement transaction. The returned event owns
    /// its original and staged forms, so callers can commit the final event to
    /// the relevant host mutation only after every applicable replacement has
    /// been ordered or declined.
    pub fn begin_static_replacement_event(
        &mut self,
        event: StaticRuntimeEvent,
    ) -> Result<StaticPendingReplacementEvent, ExecutionError> {
        self.static_replacement
            .begin_replacement_event(event)
            .map_err(|error| ExecutionError::Adapter(format!("{error:?}")))
    }

    pub fn applicable_static_replacements(
        &self,
        event: &StaticPendingReplacementEvent,
        perspective_player: PlayerId,
        active_player: PlayerId,
    ) -> Result<Vec<u64>, ExecutionError> {
        let snapshot = self.static_replacement_snapshot(perspective_player, active_player)?;
        self.static_replacement
            .applicable_replacements(&snapshot, event)
            .map_err(|error| ExecutionError::Adapter(format!("{error:?}")))
    }

    pub fn apply_static_replacement_step(
        &self,
        event: &mut StaticPendingReplacementEvent,
        evidence: StaticReplacementOrderEvidence,
        decision: Option<StaticReplacementDecision>,
        perspective_player: PlayerId,
        active_player: PlayerId,
    ) -> Result<StaticReplacementStep, ExecutionError> {
        let snapshot = self.static_replacement_snapshot(perspective_player, active_player)?;
        self.static_replacement
            .apply_replacement_step(&snapshot, event, evidence, decision)
            .map_err(|error| ExecutionError::Adapter(format!("{error:?}")))
    }

    /// Returns the exact projected mana identities and objects accepted by a
    /// regeneration activation payment.
    pub fn regeneration_activation_state(
        &self,
    ) -> Result<RegenerationRuntimeState, ExecutionError> {
        synchronized_regeneration_runtime_state(&self.state)
    }

    /// Commits one exact opening-hand procedure atomically, including reveal
    /// evidence and installation of its delayed first-turn program.
    pub fn resolve_opening_hand_procedure(
        &mut self,
        source: ObjectId,
        clause_address: ClauseAddress,
        player: PlayerId,
        starting_player: PlayerId,
        kept_hand: &BTreeSet<ObjectId>,
        decision: OpeningHandDecision,
    ) -> Result<OpeningHandBatch, ExecutionError> {
        let before_simulation = self.clone();
        let before = self.state.clone();
        let program = self.bound_direct_opening_hand_program(source, clause_address)?;
        let source_reference = pregame_object_reference(source);
        let kept_hand = kept_hand
            .iter()
            .copied()
            .map(pregame_object_reference)
            .collect::<BTreeSet<_>>();
        for reference in &kept_hand {
            let object = self
                .state
                .object(reference.object_id)
                .ok_or(ExecutionError::MissingObject(reference.object_id))?;
            if object.zone != Zone::Hand || object.owner != player {
                return Err(ExecutionError::Adapter(
                    "kept opening-hand evidence does not match the current hidden zone".into(),
                ));
            }
        }
        let transaction = prepare_opening_hand_transaction(
            &program,
            &OpeningHandEvidence {
                player,
                starting_player,
                source: source_reference,
                source_zone: PregameZone::Hand,
                kept_hand,
            },
            decision,
        )
        .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        let result = (|| {
            for action in &transaction.ordered_actions {
                match action {
                    PregameAction::MoveSource { source, from, to } => {
                        if *from != PregameZone::Hand
                            || source.object_id != source_reference.object_id
                        {
                            return Err(ExecutionError::Adapter(
                                "opening-hand move receipt does not match its source".into(),
                            ));
                        }
                        self.state
                            .move_object(source.object_id, bounded_pregame_zone(*to)?)
                            .map_err(ExecutionError::Adapter)?;
                    }
                    PregameAction::PutCounterOnMovedSource { kind, amount } => {
                        let counter = match kind {
                            PregameCounterKind::Luck => "luck",
                        };
                        let mut object = self
                            .state
                            .object(source)
                            .ok_or(ExecutionError::MissingObject(source))?;
                        let next = object
                            .counters
                            .get(counter)
                            .copied()
                            .unwrap_or(0)
                            .checked_add(*amount)
                            .ok_or_else(|| {
                                ExecutionError::Adapter("pregame counter overflow".into())
                            })?;
                        object.counters.insert(counter.into(), next);
                        self.state
                            .put_object(object)
                            .map_err(ExecutionError::Adapter)?;
                    }
                    PregameAction::ExileCardFromHand { card } => self
                        .state
                        .move_object(card.object_id, Zone::Exile)
                        .map_err(ExecutionError::Adapter)?,
                    PregameAction::LoseLife { player, amount } => {
                        let player_id = *player;
                        let mut state = self
                            .state
                            .player(player_id)
                            .ok_or(ExecutionError::MissingPlayer(player_id))?;
                        state.life = state.life.checked_sub(*amount).ok_or_else(|| {
                            ExecutionError::Adapter("pregame life total overflow".into())
                        })?;
                        self.state
                            .put_player(state)
                            .map_err(ExecutionError::Adapter)?;
                    }
                    PregameAction::RevealSourceFromOpeningHand { source } => {
                        let order = self.state.next_order();
                        self.state.register_revealed_card(RevealedCardRecord {
                            order,
                            source_identity: source.object_id,
                            player,
                            card: source.object_id,
                            as_additional_cost: false,
                        });
                    }
                    PregameAction::RegisterDelayedEffect { .. } => {}
                }
            }
            if transaction
                .ordered_actions
                .iter()
                .any(|action| matches!(action, PregameAction::RegisterDelayedEffect { .. }))
            {
                let players = self.state.player_ids().into_iter().collect::<BTreeSet<_>>();
                self.pregame_delayed
                    .install_from_transaction(&transaction, &players)
                    .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            *self = before_simulation;
            return Err(error);
        }
        Ok(OpeningHandBatch {
            transaction,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    fn bound_direct_opening_hand_program(
        &self,
        source: ObjectId,
        clause_address: ClauseAddress,
    ) -> Result<PregameClauseProgram, ExecutionError> {
        self.programs
            .get(&source)
            .into_iter()
            .flatten()
            .find(|clause| clause.address() == clause_address)
            .and_then(|clause| match clause.effects() {
                [
                    Effect::StandaloneRuleProgram(
                        crate::bounded_oracle_runtime::StandaloneRuleProgram::Pregame(program),
                    ),
                ] if deck_construction_clause_has_live_bridge_contract(clause) => {
                    match program.kind() {
                        PregameClauseKind::OpeningHand(
                            OpeningHandProgram::BeginOnBattlefield(_)
                            | OpeningHandProgram::BeginInGraveyard(_)
                            | OpeningHandProgram::RevealAndSchedule(_),
                        ) => Some(program.as_ref().clone()),
                        _ => None,
                    }
                }
                _ => None,
            })
            .ok_or_else(|| {
                ExecutionError::Adapter("exact opening-hand program is not bound".into())
            })
    }

    /// Converts one exact first-turn event into due pregame effects without
    /// resolving them early.
    pub fn observe_pregame_event(
        &mut self,
        event: PregameEvent,
    ) -> Result<Vec<DuePregameEffect>, ExecutionError> {
        self.pregame_delayed
            .observe_event(event)
            .map_err(|error| ExecutionError::Adapter(error.to_string()))
    }

    /// Resolves one due opening-hand effect with complete hidden-zone and
    /// payment choices, committing atomically to the main simulation.
    pub fn resolve_pregame_effect(
        &mut self,
        due_effect_id: PregameDueEffectId,
        choices: PregameResolutionChoices,
    ) -> Result<PregameResolutionBatch, ExecutionError> {
        let before_simulation = self.clone();
        let before_state = self.state.clone();
        let result = (|| {
            let transaction = self
                .pregame_delayed
                .resolve(due_effect_id)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            self.apply_pregame_reveal_effect(&transaction, choices)?;
            Ok(PregameResolutionBatch {
                transaction,
                delta: SimulationDelta::between(&before_state, &self.state),
            })
        })();
        if result.is_err() {
            *self = before_simulation;
        }
        result
    }

    fn apply_pregame_reveal_effect(
        &mut self,
        transaction: &PregameResolutionTransaction,
        choices: PregameResolutionChoices,
    ) -> Result<(), ExecutionError> {
        let controller = transaction.controller;
        match &transaction.effect {
            PregameRevealEffect::CreateToken { token, amount } => {
                for _ in 0..*amount {
                    self.create_pregame_token(controller, token)?;
                }
            }
            PregameRevealEffect::EachOpponentLosesLifeThenControllerGainsTotalLost {
                amount_each,
            } => {
                if !self.state.replacements().is_empty() {
                    return Err(ExecutionError::Adapter(
                        "pregame life effect requires complete replacement evidence".into(),
                    ));
                }
                let opponents = self
                    .state
                    .player_ids()
                    .into_iter()
                    .filter(|player| *player != controller)
                    .collect::<Vec<_>>();
                for opponent in &opponents {
                    let mut player = self
                        .state
                        .player(*opponent)
                        .ok_or(ExecutionError::MissingPlayer(*opponent))?;
                    player.life = player
                        .life
                        .checked_sub(i64::from(*amount_each))
                        .ok_or(ExecutionError::ArithmeticOverflow)?;
                    self.state
                        .put_player(player)
                        .map_err(ExecutionError::Adapter)?;
                }
                let gained = i64::from(*amount_each)
                    .checked_mul(i64::try_from(opponents.len()).map_err(|_| {
                        ExecutionError::Adapter("opponent count is out of range".into())
                    })?)
                    .ok_or(ExecutionError::ArithmeticOverflow)?;
                let mut player = self
                    .state
                    .player(controller)
                    .ok_or(ExecutionError::MissingPlayer(controller))?;
                player.life = player
                    .life
                    .checked_add(gained)
                    .ok_or(ExecutionError::ArithmeticOverflow)?;
                self.state
                    .put_player(player)
                    .map_err(ExecutionError::Adapter)?;
            }
            PregameRevealEffect::EachOpponentMills { amount_each } => {
                let opponents = self
                    .state
                    .player_ids()
                    .into_iter()
                    .filter(|player| *player != controller)
                    .collect::<Vec<_>>();
                for opponent in opponents {
                    for _ in 0..*amount_each {
                        let Some(card) = self
                            .state
                            .player(opponent)
                            .ok_or(ExecutionError::MissingPlayer(opponent))?
                            .library
                            .first()
                            .copied()
                        else {
                            break;
                        };
                        self.state
                            .move_object(card, Zone::Graveyard)
                            .map_err(ExecutionError::Adapter)?;
                    }
                }
            }
            PregameRevealEffect::ShuffleRevealedSourceIntoLibraryThenDraw { amount } => {
                self.state
                    .move_object(transaction.source.object_id, Zone::Library)
                    .map_err(ExecutionError::Adapter)?;
                let order = choices.library_order_after_shuffle.ok_or_else(|| {
                    ExecutionError::Adapter("complete shuffled library order is required".into())
                })?;
                let mut player = self
                    .state
                    .player(controller)
                    .ok_or(ExecutionError::MissingPlayer(controller))?;
                let current = player.library.iter().copied().collect::<BTreeSet<_>>();
                let supplied = order.iter().copied().collect::<BTreeSet<_>>();
                if current != supplied || supplied.len() != order.len() {
                    return Err(ExecutionError::Adapter(
                        "shuffled library order is not an exact permutation".into(),
                    ));
                }
                player.library = order;
                self.state
                    .put_player(player)
                    .map_err(ExecutionError::Adapter)?;
                for _ in 0..*amount {
                    let card = self
                        .state
                        .player(controller)
                        .ok_or(ExecutionError::MissingPlayer(controller))?
                        .library
                        .first()
                        .copied()
                        .ok_or_else(|| {
                            ExecutionError::Adapter("cannot draw empty library".into())
                        })?;
                    self.state
                        .move_object(card, Zone::Hand)
                        .map_err(ExecutionError::Adapter)?;
                }
            }
            PregameRevealEffect::SetControllersLifeTotal { total } => {
                let mut player = self
                    .state
                    .player(controller)
                    .ok_or(ExecutionError::MissingPlayer(controller))?;
                player.life = i64::from(*total);
                self.state
                    .put_player(player)
                    .map_err(ExecutionError::Adapter)?;
            }
            PregameRevealEffect::AddMana { kind, amount } => {
                let color = match kind {
                    crate::pregame_clause_runtime::PregameManaKind::Green => Color::Green,
                };
                let mut player = self
                    .state
                    .player(controller)
                    .ok_or(ExecutionError::MissingPlayer(controller))?;
                let index = simulation_color_index(color);
                player.mana.colored[index] = player.mana.colored[index]
                    .checked_add(*amount)
                    .ok_or(ExecutionError::ArithmeticOverflow)?;
                self.state
                    .put_player(player)
                    .map_err(ExecutionError::Adapter)?;
            }
            PregameRevealEffect::LookAtTopKeepOneOnTopExileRest { amount, keep_up_to } => {
                let library = self
                    .state
                    .player(controller)
                    .ok_or(ExecutionError::MissingPlayer(controller))?
                    .library;
                let count = usize::try_from(*amount)
                    .unwrap_or(usize::MAX)
                    .min(library.len());
                let looked = library[..count].to_vec();
                if *keep_up_to != 1
                    || choices
                        .keep_on_top
                        .is_some_and(|card| !looked.contains(&card))
                {
                    return Err(ExecutionError::Adapter(
                        "top-card keep choice is incomplete or illegal".into(),
                    ));
                }
                self.state.put_looked_at(controller, looked.clone());
                for card in looked {
                    if Some(card) != choices.keep_on_top {
                        self.state
                            .move_object(card, Zone::Exile)
                            .map_err(ExecutionError::Adapter)?;
                    }
                }
            }
            PregameRevealEffect::Scry { amount } => {
                let mut player = self
                    .state
                    .player(controller)
                    .ok_or(ExecutionError::MissingPlayer(controller))?;
                let count = usize::try_from(*amount)
                    .unwrap_or(usize::MAX)
                    .min(player.library.len());
                let viewed = player.library[..count]
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>();
                let selected = choices
                    .scry_to_bottom
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>();
                if selected.len() != choices.scry_to_bottom.len() || !selected.is_subset(&viewed) {
                    return Err(ExecutionError::Adapter(
                        "scry bottom choice is incomplete or illegal".into(),
                    ));
                }
                player.library.retain(|card| !selected.contains(card));
                player.library.extend(choices.scry_to_bottom);
                self.state
                    .put_player(player)
                    .map_err(ExecutionError::Adapter)?;
            }
            PregameRevealEffect::CounterEachOpponentsFirstSpellUnlessTheyPayGeneric {
                generic_mana,
            } => {
                let PregameEvent::PlayerCastsFirstSpell { player, spell } = transaction.event
                else {
                    return Err(ExecutionError::Adapter(
                        "spell ward resolved for the wrong event".into(),
                    ));
                };
                let spell_object = self
                    .state
                    .object(spell.object_id)
                    .ok_or(ExecutionError::MissingObject(spell.object_id))?;
                if spell != pregame_object_reference(spell.object_id)
                    || spell_object.zone != Zone::Stack
                    || spell_object.controller != player
                {
                    return Err(ExecutionError::Adapter(
                        "first-spell evidence does not match the current stack object".into(),
                    ));
                }
                if *generic_mana != 1 {
                    return Err(ExecutionError::Adapter(
                        "unsupported pregame generic payment".into(),
                    ));
                }
                match choices.ward_payment.ok_or_else(|| {
                    ExecutionError::Adapter("spell ward payment choice is required".into())
                })? {
                    PregameSpellWardPayment::Decline => self
                        .state
                        .move_object(spell.object_id, Zone::Graveyard)
                        .map_err(ExecutionError::Adapter)?,
                    PregameSpellWardPayment::Unrestricted => {
                        let mut payer = self
                            .state
                            .player(player)
                            .ok_or(ExecutionError::MissingPlayer(player))?;
                        payer.mana.unrestricted =
                            payer.mana.unrestricted.checked_sub(1).ok_or_else(|| {
                                ExecutionError::Adapter("unpaid spell ward".into())
                            })?;
                        self.state
                            .put_player(payer)
                            .map_err(ExecutionError::Adapter)?;
                    }
                    PregameSpellWardPayment::Colored(color) => {
                        let mut payer = self
                            .state
                            .player(player)
                            .ok_or(ExecutionError::MissingPlayer(player))?;
                        let amount = &mut payer.mana.colored[simulation_color_index(color)];
                        *amount = amount
                            .checked_sub(1)
                            .ok_or_else(|| ExecutionError::Adapter("unpaid spell ward".into()))?;
                        self.state
                            .put_player(payer)
                            .map_err(ExecutionError::Adapter)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn create_pregame_token(
        &mut self,
        controller: PlayerId,
        token: &PregameTokenDefinition,
    ) -> Result<(), ExecutionError> {
        let id = self.state.allocate_object_id();
        let power = i64::from(token.power);
        let toughness = i64::from(token.toughness);
        let object = PhysicalObject {
            id,
            origin_id: id,
            copy_of: None,
            owner: controller,
            controller,
            zone: Zone::Battlefield,
            token: true,
            tapped: false,
            attacking: false,
            blocking: false,
            prepared: false,
            face_down: false,
            active_face: 0,
            class_level: 0,
            crew_power_bonus: 0,
            front: ObjectCharacteristics {
                names: vec![token.name.clone()],
                card_types: token
                    .card_types
                    .iter()
                    .filter_map(|kind| match kind.as_str() {
                        "creature" => Some(CardType::Creature),
                        "artifact" => Some(CardType::Artifact),
                        _ => None,
                    })
                    .collect(),
                supertypes: Vec::new(),
                subtypes: token.subtypes.iter().cloned().collect(),
                colors: token
                    .colors
                    .iter()
                    .filter_map(|color| match color.as_str() {
                        "red" => Some(Color::Red),
                        "green" => Some(Color::Green),
                        "white" => Some(Color::White),
                        "blue" => Some(Color::Blue),
                        "black" => Some(Color::Black),
                        _ => None,
                    })
                    .collect(),
                mana_value: 0,
                power,
                toughness,
                keywords: token
                    .keywords
                    .iter()
                    .filter_map(|keyword| match keyword.as_str() {
                        "haste" => Some(Keyword::Haste),
                        _ => None,
                    })
                    .collect(),
                abilities: Vec::new(),
            },
            back: None,
            counters: BTreeMap::new(),
        };
        self.state
            .insert_physical_object(object)
            .map_err(ExecutionError::Adapter)
    }

    pub fn begin_turn(&mut self, active_player: PlayerId) -> Result<(), ExecutionError> {
        if !self.state.players.contains_key(&active_player) {
            return Err(ExecutionError::MissingPlayer(active_player));
        }
        self.state.begin_turn();
        self.old_transform
            .begin_turn()
            .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        let mut access = synchronized_library_access_state(&self.state)?;
        access.begin_turn(active_player);
        self.state.library_access = access;
        Ok(())
    }

    /// Resolves a destruction event through the exact regeneration lifecycle.
    pub fn destroy_permanent(
        &mut self,
        permanent: ObjectId,
        replacement_choice: Option<RegenerationChoice>,
    ) -> Result<RegenerationDestructionOutcome, ExecutionError> {
        let mut runtime = synchronized_regeneration_runtime_state(&self.state)?;
        let outcome = resolve_regeneration_destruction(
            &mut runtime,
            RegenerationObjectReference {
                object: KeywordObjectId(permanent),
                incarnation: RegenerationIncarnationId(permanent),
            },
            replacement_choice,
        )
        .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        commit_regeneration_runtime_state(&mut self.state, runtime)?;
        Ok(outcome)
    }

    /// Pays an exact regeneration activation cost and places its pending action on the stack.
    pub fn activate_regeneration(
        &mut self,
        source: ObjectId,
        clause_address: ClauseAddress,
        controller: PlayerId,
        target: Option<ObjectId>,
        payment: RegenerationActivationPayment,
    ) -> Result<RegenerationActivationBatch, ExecutionError> {
        let before = self.state.clone();
        let program = self.bound_regeneration_program(source, clause_address)?;
        let mut runtime = synchronized_regeneration_runtime_state(&self.state)?;
        let receipt = begin_regeneration_activation(
            &mut runtime,
            &program,
            KeywordPlayerId(u16::from(controller)),
            regeneration_object_reference(source),
            target.map(regeneration_object_reference),
            payment,
        )
        .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        if let Err(error) = commit_regeneration_runtime_state(&mut self.state, runtime) {
            self.state = before;
            return Err(error);
        }
        Ok(RegenerationActivationBatch {
            receipt,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    /// Places a matched exact regeneration trigger on the pending-action stack.
    pub fn trigger_regeneration(
        &mut self,
        source: ObjectId,
        clause_address: ClauseAddress,
        controller: PlayerId,
        event: RegenerationTriggerEvent,
        target: Option<ObjectId>,
    ) -> Result<RegenerationTriggerBatch, ExecutionError> {
        let before = self.state.clone();
        let program = self.bound_regeneration_program(source, clause_address)?;
        let mut runtime = synchronized_regeneration_runtime_state(&self.state)?;
        let action = begin_regeneration_trigger(
            &mut runtime,
            &program,
            KeywordPlayerId(u16::from(controller)),
            regeneration_object_reference(source),
            event,
            target.map(regeneration_object_reference),
        )
        .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        if let Err(error) = commit_regeneration_runtime_state(&mut self.state, runtime) {
            self.state = before;
            return Err(error);
        }
        Ok(RegenerationTriggerBatch {
            action,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    /// Resolves one exact pending regeneration activation or trigger.
    pub fn resolve_regeneration_action(
        &mut self,
        action_id: RegenerationStackActionId,
    ) -> Result<RegenerationResolutionBatch, ExecutionError> {
        let before = self.state.clone();
        let mut runtime = synchronized_regeneration_runtime_state(&self.state)?;
        let receipt = resolve_pending_regeneration(&mut runtime, action_id)
            .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        if let Err(error) = commit_regeneration_runtime_state(&mut self.state, runtime) {
            self.state = before;
            return Err(error);
        }
        Ok(RegenerationResolutionBatch {
            receipt,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    /// Resolves an exact standalone delegated Regenerate instruction for its
    /// declared target or complete controlled-creature set.
    pub fn resolve_delegated_regeneration(
        &mut self,
        source: ObjectId,
        clause_address: ClauseAddress,
        controller: PlayerId,
        target: Option<ObjectId>,
    ) -> Result<RegenerationResolutionBatch, ExecutionError> {
        let before = self.state.clone();
        let clause = self
            .delegated_regeneration_programs
            .get(&source)
            .into_iter()
            .flatten()
            .find(|clause| clause.address() == clause_address)
            .ok_or_else(|| {
                ExecutionError::Adapter("exact delegated Regenerate program is not bound".into())
            })?;
        let fragment = clause
            .keyword_program()
            .source()
            .oracle_fragment
            .as_deref()
            .ok_or_else(|| ExecutionError::Adapter("Regenerate instruction is absent".into()))?;
        let program = compile_regeneration_resolution_leaf_program(fragment, fragment)
            .ok_or_else(|| ExecutionError::Adapter("Regenerate instruction is inexact".into()))?;
        let mut runtime = synchronized_regeneration_runtime_state(&self.state)?;
        let source_ref = runtime
            .current_reference(KeywordObjectId(source))
            .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        let target_ref = target
            .map(|target| {
                runtime
                    .current_reference(KeywordObjectId(target))
                    .map_err(|error| ExecutionError::Adapter(error.to_string()))
            })
            .transpose()?;
        let receipt = resolve_regeneration_instruction(
            &mut runtime,
            &program,
            KeywordPlayerId(u16::from(controller)),
            source_ref,
            target_ref,
        )
        .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        if let Err(error) = commit_regeneration_runtime_state(&mut self.state, runtime) {
            self.state = before;
            return Err(error);
        }
        Ok(RegenerationResolutionBatch {
            receipt,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    fn bound_regeneration_program(
        &self,
        source: ObjectId,
        clause_address: ClauseAddress,
    ) -> Result<RegenerationActionProgram, ExecutionError> {
        self.programs
            .get(&source)
            .into_iter()
            .flatten()
            .find(|clause| clause.address() == clause_address)
            .and_then(|clause| {
                let [
                    Effect::StandaloneRuleProgram(
                        crate::bounded_oracle_runtime::StandaloneRuleProgram::RegenerationAction(
                            program,
                        ),
                    ),
                ] = clause.effects()
                else {
                    return None;
                };
                if !regeneration_action_clause_has_live_bridge_contract(clause) {
                    return None;
                }
                match (clause.timing(), program.kind()) {
                    (Timing::Activated, RegenerationActionKind::StandaloneResolution(_)) => {
                        normalized_regeneration_activation_program(clause)
                    }
                    _ => Some(program.as_ref().clone()),
                }
            })
            .ok_or_else(|| {
                ExecutionError::Adapter(
                    "exact activated or triggered regeneration program is not bound".into(),
                )
            })
    }

    /// Moves a Soulshift source with complete replacement evidence and creates
    /// one independently targetable pending trigger per printed instance.
    pub fn move_soulshift_source_to_graveyard(
        &mut self,
        source: ObjectId,
        replacement_evidence: GraveyardReplacementEvidence,
    ) -> Result<SoulshiftDeathBatch, ExecutionError> {
        if !self.programs.get(&source).is_some_and(|clauses| {
            clauses
                .iter()
                .any(graveyard_transform_clause_has_live_bridge_contract)
        }) {
            return Err(ExecutionError::Adapter(
                "exact Soulshift program is not bound".into(),
            ));
        }
        let before_simulation = self.clone();
        let before_state = self.state.clone();
        let result = (|| {
            let source_ref = self
                .graveyard_transform
                .object(GraveyardObjectId(source))
                .ok_or(ExecutionError::MissingObject(source))?
                .object_ref;
            let receipt = self
                .graveyard_transform
                .move_battlefield_object_with_soulshift_evidence(
                    source_ref,
                    GraveyardZone::Graveyard,
                    replacement_evidence,
                )
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            if let Some(destination) = receipt.destination {
                self.state
                    .move_object(source, bounded_graveyard_zone(destination)?)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(SoulshiftDeathBatch {
                receipt,
                delta: SimulationDelta::between(&before_state, &self.state),
            })
        })();
        if result.is_err() {
            *self = before_simulation;
        }
        result
    }

    /// Declares the exact target (or proves there is none) for a pending
    /// Soulshift trigger without mutating host zones.
    pub fn declare_soulshift_target(
        &mut self,
        trigger_id: PendingSoulshiftTriggerId,
        declaring_player: PlayerId,
        target: Option<ObjectId>,
    ) -> Result<(), ExecutionError> {
        let declaration = match target {
            Some(object_id) => {
                let target = self
                    .graveyard_transform
                    .object(GraveyardObjectId(object_id))
                    .ok_or(ExecutionError::MissingObject(object_id))?
                    .object_ref;
                SoulshiftTargetDeclaration::Target(target)
            }
            None => SoulshiftTargetDeclaration::NoLegalTarget {
                graveyard_inventory_complete: true,
            },
        };
        self.graveyard_transform
            .declare_soulshift_target(
                trigger_id,
                GraveyardPlayerId(u16::from(declaring_player)),
                declaration,
            )
            .map_err(|error| ExecutionError::Adapter(error.to_string()))
    }

    /// Resolves a pending Soulshift trigger and commits its optional return to
    /// the main hidden-zone state atomically.
    pub fn resolve_soulshift_trigger(
        &mut self,
        trigger_id: PendingSoulshiftTriggerId,
        choice: SoulshiftResolutionChoice,
    ) -> Result<SoulshiftResolutionBatch, ExecutionError> {
        let before_simulation = self.clone();
        let before_state = self.state.clone();
        let result = (|| {
            let receipt = self
                .graveyard_transform
                .resolve_soulshift_trigger(trigger_id, choice)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            if let crate::graveyard_transform_keyword_runtime::SoulshiftResolution::ReturnedToHand {
                old_target,
                ..
            } = receipt.resolution
            {
                self.state
                    .move_object(old_target.object_id.0, Zone::Hand)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(SoulshiftResolutionBatch {
                receipt,
                delta: SimulationDelta::between(&before_state, &self.state),
            })
        })();
        if result.is_err() {
            *self = before_simulation;
        }
        result
    }

    /// Returns the exact mana identities that can be named in a Disturb cast
    /// payment after projecting the host pool into the lifecycle runtime.
    pub fn disturb_mana_units(
        &mut self,
        player: PlayerId,
    ) -> Result<Vec<GraveyardManaUnitId>, ExecutionError> {
        let origins = self.synchronize_graveyard_transform_mana(player)?;
        Ok(origins.keys().copied().collect())
    }

    /// Pays Disturb's exact alternative cost and moves the physical card onto
    /// the stack with its back face up.
    pub fn cast_with_disturb(
        &mut self,
        source: ObjectId,
        clause_address: ClauseAddress,
        evidence: DisturbCastEvidence,
    ) -> Result<DisturbCastBatch, ExecutionError> {
        let before_simulation = self.clone();
        let before_state = self.state.clone();
        let result = (|| {
            let program = self.bound_disturb_program(source, clause_address)?;
            let caster = bounded_graveyard_player(evidence.caster)?;
            let origins = self.synchronize_graveyard_transform_mana(caster)?;
            let source_ref = self
                .graveyard_transform
                .object(GraveyardObjectId(source))
                .ok_or(ExecutionError::MissingObject(source))?
                .object_ref;
            let receipt = self
                .graveyard_transform
                .cast_with_disturb(source_ref, program.semantic_digest(), evidence)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            self.commit_graveyard_mana_spend(caster, &receipt.mana_units_spent, &origins)?;
            self.state
                .move_object(source, Zone::Stack)
                .map_err(ExecutionError::Adapter)?;
            let mut physical = self
                .state
                .object(source)
                .ok_or(ExecutionError::MissingObject(source))?;
            physical.active_face = 1;
            physical.controller = caster;
            self.state
                .put_object(physical)
                .map_err(ExecutionError::Adapter)?;
            Ok(DisturbCastBatch {
                receipt,
                delta: SimulationDelta::between(&before_state, &self.state),
            })
        })();
        if result.is_err() {
            *self = before_simulation;
        }
        result
    }

    /// Resolves a disturbed spell onto the battlefield with its back face up.
    pub fn resolve_disturbed_spell(
        &mut self,
        source: ObjectId,
    ) -> Result<DisturbResolutionBatch, ExecutionError> {
        let before_simulation = self.clone();
        let before_state = self.state.clone();
        let result = (|| {
            let stack_ref = self
                .graveyard_transform
                .object(GraveyardObjectId(source))
                .ok_or(ExecutionError::MissingObject(source))?
                .object_ref;
            let receipt = self
                .graveyard_transform
                .resolve_disturbed_spell(stack_ref)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            self.state
                .move_object(source, Zone::Battlefield)
                .map_err(ExecutionError::Adapter)?;
            let mut physical = self
                .state
                .object(source)
                .ok_or(ExecutionError::MissingObject(source))?;
            physical.active_face = 1;
            physical.controller = bounded_graveyard_player(receipt.controller)?;
            self.state
                .put_object(physical)
                .map_err(ExecutionError::Adapter)?;
            Ok(DisturbResolutionBatch {
                receipt,
                delta: SimulationDelta::between(&before_state, &self.state),
            })
        })();
        if result.is_err() {
            *self = before_simulation;
        }
        result
    }

    /// Moves an object tracked by Disturb through its exact intrinsic
    /// graveyard-to-exile replacement and any explicitly ordered external
    /// replacements.
    pub fn move_disturbed_object(
        &mut self,
        source: ObjectId,
        requested_destination: GraveyardZone,
        destination_controller: Option<PlayerId>,
        replacement_evidence: GraveyardReplacementEvidence,
    ) -> Result<GraveyardTransformMoveBatch, ExecutionError> {
        let before_simulation = self.clone();
        let before_state = self.state.clone();
        let result = (|| {
            let source_ref = self
                .graveyard_transform
                .object(GraveyardObjectId(source))
                .ok_or(ExecutionError::MissingObject(source))?
                .object_ref;
            let receipt = self
                .graveyard_transform
                .move_object(
                    source_ref,
                    requested_destination,
                    destination_controller.map(|player| GraveyardPlayerId(u16::from(player))),
                    replacement_evidence,
                )
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            if let Some(destination) = receipt.actual_destination {
                self.state
                    .move_object(source, bounded_graveyard_zone(destination)?)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(GraveyardTransformMoveBatch {
                receipt,
                delta: SimulationDelta::between(&before_state, &self.state),
            })
        })();
        if result.is_err() {
            *self = before_simulation;
        }
        result
    }

    /// Pays an exact Craft activation, exiles the source and chosen materials
    /// as costs, and preserves their linked exile incarnations while the
    /// ability is pending.
    pub fn activate_craft(
        &mut self,
        source: ObjectId,
        clause_address: ClauseAddress,
        activator: PlayerId,
        priority: GraveyardPriorityWindow,
        material_choices: Vec<ObjectId>,
        mana_units: Vec<GraveyardManaUnitId>,
        replacement_evidence: BTreeMap<ObjectId, GraveyardReplacementEvidence>,
    ) -> Result<CraftActivationBatch, ExecutionError> {
        let before_simulation = self.clone();
        let before_state = self.state.clone();
        let result = (|| {
            let program = self.bound_craft_program(source, clause_address)?;
            let origins = self.synchronize_graveyard_transform_mana(activator)?;
            let source_ref = self
                .graveyard_transform
                .object(GraveyardObjectId(source))
                .ok_or(ExecutionError::MissingObject(source))?
                .object_ref;
            let materials = material_choices
                .iter()
                .map(|object_id| {
                    self.graveyard_transform
                        .object(GraveyardObjectId(*object_id))
                        .map(|object| object.object_ref)
                        .ok_or(ExecutionError::MissingObject(*object_id))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let replacements = replacement_evidence
                .into_iter()
                .map(|(object_id, evidence)| {
                    let reference = self
                        .graveyard_transform
                        .object(GraveyardObjectId(object_id))
                        .map(|object| object.object_ref)
                        .ok_or(ExecutionError::MissingObject(object_id))?;
                    Ok((reference, evidence))
                })
                .collect::<Result<BTreeMap<_, _>, ExecutionError>>()?;
            let receipt = self
                .graveyard_transform
                .activate_craft(
                    source_ref,
                    program.semantic_digest(),
                    materials,
                    CraftActivationEvidence {
                        activator: GraveyardPlayerId(u16::from(activator)),
                        priority,
                        mana_payment: GraveyardManaPaymentEvidence {
                            player: GraveyardPlayerId(u16::from(activator)),
                            mana_units,
                        },
                        cost_choice_inventory_complete: true,
                        zone_change_replacements: replacements,
                    },
                )
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            self.commit_graveyard_mana_spend(activator, &receipt.mana_units_spent, &origins)?;
            for event in &receipt.exile_events {
                self.state
                    .move_object(event.old_object.object_id.0, Zone::Exile)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(CraftActivationBatch {
                receipt,
                delta: SimulationDelta::between(&before_state, &self.state),
            })
        })();
        if result.is_err() {
            *self = before_simulation;
        }
        result
    }

    /// Resolves a pending Craft ability, returning the still-linked source
    /// transformed under its owner's control while retaining exact material
    /// links that remain in exile.
    pub fn resolve_craft_ability(
        &mut self,
        ability_id: PendingCraftAbilityId,
    ) -> Result<CraftResolutionBatch, ExecutionError> {
        let before_simulation = self.clone();
        let before_state = self.state.clone();
        let result = (|| {
            let receipt = self
                .graveyard_transform
                .resolve_craft_ability(ability_id)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            if let crate::graveyard_transform_keyword_runtime::CraftResolution::ReturnedTransformed {
                battlefield_object,
                controller,
                ..
            } = &receipt.resolution
            {
                let object_id = battlefield_object.object_id.0;
                self.state
                    .move_object(object_id, Zone::Battlefield)
                    .map_err(ExecutionError::Adapter)?;
                let mut physical = self
                    .state
                    .object(object_id)
                    .ok_or(ExecutionError::MissingObject(object_id))?;
                physical.active_face = 1;
                physical.controller = bounded_graveyard_player(*controller)?;
                self.state
                    .put_object(physical)
                    .map_err(ExecutionError::Adapter)?;
            }
            Ok(CraftResolutionBatch {
                receipt,
                delta: SimulationDelta::between(&before_state, &self.state),
            })
        })();
        if result.is_err() {
            *self = before_simulation;
        }
        result
    }

    fn bound_craft_program(
        &self,
        source: ObjectId,
        clause_address: ClauseAddress,
    ) -> Result<GraveyardTransformKeywordProgram, ExecutionError> {
        self.programs
            .get(&source)
            .into_iter()
            .flatten()
            .find(|clause| clause.address() == clause_address)
            .and_then(|clause| {
                match clause.effects() {
                [Effect::StandaloneRuleProgram(
                    crate::bounded_oracle_runtime::StandaloneRuleProgram::GraveyardTransformKeyword(
                        program,
                    ),
                )] if graveyard_transform_clause_has_live_bridge_contract(clause)
                    && matches!(program.kind(), GraveyardTransformKeywordKind::Craft(_)) =>
                {
                    Some(program.as_ref().clone())
                }
                _ => None,
            }
            })
            .ok_or_else(|| ExecutionError::Adapter("exact Craft program is not bound".into()))
    }

    fn bound_disturb_program(
        &self,
        source: ObjectId,
        clause_address: ClauseAddress,
    ) -> Result<GraveyardTransformKeywordProgram, ExecutionError> {
        self.programs
            .get(&source)
            .into_iter()
            .flatten()
            .find(|clause| clause.address() == clause_address)
            .and_then(|clause| {
                match clause.effects() {
                [Effect::StandaloneRuleProgram(
                    crate::bounded_oracle_runtime::StandaloneRuleProgram::GraveyardTransformKeyword(
                        program,
                    ),
                )] if graveyard_transform_clause_has_live_bridge_contract(clause)
                    && matches!(program.kind(), GraveyardTransformKeywordKind::Disturb(_)) =>
                {
                    Some(program.as_ref().clone())
                }
                _ => None,
            }
            })
            .ok_or_else(|| ExecutionError::Adapter("exact Disturb program is not bound".into()))
    }

    fn synchronize_graveyard_transform_mana(
        &mut self,
        player_id: PlayerId,
    ) -> Result<BTreeMap<GraveyardManaUnitId, Option<Color>>, ExecutionError> {
        let player = self
            .state
            .player(player_id)
            .ok_or(ExecutionError::MissingPlayer(player_id))?;
        let mut runtime_player = GraveyardPlayerState::new(GraveyardPlayerId(u16::from(player_id)));
        let mut origins = BTreeMap::new();
        let mut represented = [0u32; 6];
        for unit in &player.expiring_mana {
            let color = bounded_to_graveyard_mana_color(unit.color);
            let id = GraveyardManaUnitId(unit.id);
            runtime_player.mana_pool.insert(
                id,
                GraveyardManaUnit {
                    id,
                    color,
                    produced_by_snow_source: self
                        .state
                        .object(unit.source_identity)
                        .is_some_and(|source| source.front.supertypes.contains(&Supertype::Snow)),
                },
            );
            represented[simulation_color_index(unit.color)] = represented
                [simulation_color_index(unit.color)]
            .checked_add(1)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
            origins.insert(id, Some(unit.color));
        }
        for (index, total) in player.mana.colored.into_iter().enumerate() {
            let remaining = total
                .checked_sub(represented[index])
                .ok_or_else(|| ExecutionError::Adapter("expiring mana exceeds host pool".into()))?;
            let color = simulation_indexed_color(index);
            for ordinal in 0..remaining {
                let id = GraveyardManaUnitId(
                    0xA000_0000_0000_0000
                        | (u64::from(player_id) << 40)
                        | ((index as u64) << 32)
                        | u64::from(ordinal),
                );
                runtime_player.mana_pool.insert(
                    id,
                    GraveyardManaUnit {
                        id,
                        color: bounded_to_graveyard_mana_color(color),
                        produced_by_snow_source: false,
                    },
                );
                origins.insert(id, Some(color));
            }
        }
        for ordinal in 0..player.mana.unrestricted {
            let id = GraveyardManaUnitId(
                0xB000_0000_0000_0000 | (u64::from(player_id) << 40) | u64::from(ordinal),
            );
            runtime_player.mana_pool.insert(
                id,
                GraveyardManaUnit {
                    id,
                    color: GraveyardManaColor::Colorless,
                    produced_by_snow_source: false,
                },
            );
            origins.insert(id, None);
        }
        self.graveyard_transform.replace_player(runtime_player);
        Ok(origins)
    }

    fn commit_graveyard_mana_spend(
        &mut self,
        player_id: PlayerId,
        spent: &[GraveyardManaUnitId],
        origins: &BTreeMap<GraveyardManaUnitId, Option<Color>>,
    ) -> Result<(), ExecutionError> {
        let mut player = self
            .state
            .player(player_id)
            .ok_or(ExecutionError::MissingPlayer(player_id))?;
        for id in spent {
            match origins
                .get(id)
                .ok_or_else(|| ExecutionError::Adapter("Disturb spent unknown mana".into()))?
            {
                Some(color) => {
                    let amount = &mut player.mana.colored[simulation_color_index(*color)];
                    *amount = amount
                        .checked_sub(1)
                        .ok_or(ExecutionError::ArithmeticOverflow)?;
                    player.expiring_mana.retain(|unit| unit.id != id.0);
                }
                None => {
                    player.mana.unrestricted = player
                        .mana
                        .unrestricted
                        .checked_sub(1)
                        .ok_or(ExecutionError::ArithmeticOverflow)?;
                }
            }
        }
        self.state
            .put_player(player)
            .map_err(ExecutionError::Adapter)
    }

    /// Moves an object while applying its exact self zone-change and entry replacements.
    pub fn move_object_with_replacements(
        &mut self,
        object: ObjectId,
        requested_zone: Zone,
    ) -> Result<Zone, ExecutionError> {
        let kinds = self.object_state_kinds(object);
        let destination = if requested_zone == Zone::Graveyard
            && kinds.contains(&ObjectStateClauseKind::SelfGraveyardMoveBecomesExile)
        {
            Zone::Exile
        } else {
            requested_zone
        };
        self.state
            .move_object(object, destination)
            .map_err(ExecutionError::Adapter)?;
        if destination == Zone::Battlefield
            && kinds.contains(&ObjectStateClauseKind::EntersBattlefieldTapped)
        {
            let mut physical = self
                .state
                .object(object)
                .ok_or(ExecutionError::MissingObject(object))?;
            physical.tapped = true;
            self.state
                .put_object(physical)
                .map_err(ExecutionError::Adapter)?;
        }
        if self.programs.get(&object).is_some_and(|clauses| {
            clauses
                .iter()
                .any(old_transform_clause_has_live_bridge_contract)
        }) {
            self.install_old_transform_programs(object, true)?;
        }
        self.move_saga_transform_object(object, destination)?;
        Ok(destination)
    }

    /// Commits the active player's simultaneous untap choices.
    pub fn resolve_untap_step(
        &mut self,
        active_player: PlayerId,
        remain_tapped: &BTreeSet<ObjectId>,
    ) -> Result<(), ExecutionError> {
        if self.state.player(active_player).is_none() {
            return Err(ExecutionError::MissingPlayer(active_player));
        }
        let objects = self.state.object_ids();
        if remain_tapped.iter().any(|id| {
            self.state.object(*id).is_none_or(|object| {
                object.zone != Zone::Battlefield
                    || object.controller != active_player
                    || !object.tapped
                    || !self
                        .object_state_kinds(*id)
                        .contains(&ObjectStateClauseKind::OptionalUntapDuringYourUntapStep)
            })
        }) {
            return Err(ExecutionError::Adapter(
                "untap choice does not name an eligible optional-untap object".into(),
            ));
        }
        for id in objects {
            let Some(mut object) = self.state.object(id) else {
                continue;
            };
            if object.zone == Zone::Battlefield
                && object.controller == active_player
                && object.tapped
                && !remain_tapped.contains(&id)
            {
                object.tapped = false;
                self.state
                    .put_object(object)
                    .map_err(ExecutionError::Adapter)?;
            }
        }
        Ok(())
    }

    fn object_state_kinds(&self, object: ObjectId) -> BTreeSet<ObjectStateClauseKind> {
        self.programs
            .get(&object)
            .into_iter()
            .flatten()
            .flat_map(|clause| clause.effects())
            .filter_map(|effect| match effect {
                crate::bounded_oracle_runtime::Effect::StandaloneRuleProgram(
                    crate::bounded_oracle_runtime::StandaloneRuleProgram::ObjectState(program),
                ) => Some(program.kind()),
                _ => None,
            })
            .collect()
    }

    pub fn player_may_inspect_library_top(
        &self,
        viewer: PlayerId,
        owner: PlayerId,
    ) -> Result<bool, ExecutionError> {
        Ok(synchronized_library_access_state(&self.state)?.player_may_inspect_top(viewer, owner))
    }

    pub fn library_top_is_publicly_revealed(
        &self,
        owner: PlayerId,
    ) -> Result<bool, ExecutionError> {
        Ok(synchronized_library_access_state(&self.state)?.top_is_publicly_revealed(owner))
    }

    pub fn authorize_library_top_action(
        &self,
        actor: PlayerId,
        card: ObjectId,
        action: LibraryTopAction,
    ) -> Result<LibraryAccessAuthorization, LibraryAccessError> {
        synchronized_library_access_state(&self.state)
            .map_err(|_| LibraryAccessError::MissingCard(card))?
            .authorize_top_action(actor, card, action)
    }

    pub fn record_land_play_from_library_top(
        &mut self,
        actor: PlayerId,
        card: ObjectId,
    ) -> Result<LibraryAccessAuthorization, LibraryAccessError> {
        let mut access = synchronized_library_access_state(&self.state)
            .map_err(|_| LibraryAccessError::MissingCard(card))?;
        let authorization = access.record_land_play_from_top(actor, card)?;
        self.state.library_access = access;
        Ok(authorization)
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
        self.install_level_progression_program(object_id)?;
        self.install_old_transform_programs(object_id, false)?;
        self.install_saga_transform_program(object_id)?;
        self.install_graveyard_transform_programs(object_id)?;
        self.install_residual_ward_programs(object_id)?;
        Ok(())
    }

    fn install_level_progression_program(
        &mut self,
        object_id: ObjectId,
    ) -> Result<(), ExecutionError> {
        let mut programs = self
            .programs
            .get(&object_id)
            .into_iter()
            .flatten()
            .filter_map(|clause| match clause.effects() {
                [
                    Effect::StandaloneRuleProgram(
                        crate::bounded_oracle_runtime::StandaloneRuleProgram::LevelProgression(
                            program,
                        ),
                    ),
                ] if program.production_adapter_connected() => Some(program.as_ref().clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        programs.sort_by(|left, right| left.semantic_sha256().cmp(right.semantic_sha256()));
        programs.dedup_by(|left, right| left.semantic_sha256() == right.semantic_sha256());
        let ([] | [_]) = programs.as_slice() else {
            return Err(ExecutionError::Adapter(
                "object has conflicting level progression programs".into(),
            ));
        };
        let Some(program) = programs.pop() else {
            return Ok(());
        };
        let source_incarnation = self
            .state
            .object_incarnation(object_id)
            .ok_or(ExecutionError::MissingObject(object_id))?;
        let order = self.state.next_order();
        self.state.install_level_progression_program(
            object_id,
            crate::bounded_oracle_consumer::InstalledLevelProgressionProgram {
                order,
                source_incarnation,
                address: crate::bounded_oracle_runtime::ClauseAddress {
                    face_index: 0,
                    clause_index: 0,
                },
                program,
            },
        );
        Ok(())
    }

    fn install_residual_ward_programs(
        &mut self,
        object_id: ObjectId,
    ) -> Result<(), ExecutionError> {
        let object = self
            .state
            .object(object_id)
            .ok_or(ExecutionError::MissingObject(object_id))?;
        if object.zone != Zone::Battlefield {
            return Ok(());
        }
        let programs = self
            .programs
            .get(&object_id)
            .into_iter()
            .flatten()
            .filter_map(|clause| match clause.effects() {
                [
                    Effect::StandaloneRuleProgram(
                        crate::bounded_oracle_runtime::StandaloneRuleProgram::ResidualCostKeyword(
                            program,
                        ),
                    ),
                ] if matches!(program.kind(), ResidualCostKeywordKind::Ward(_)) => {
                    Some((clause.address(), program.as_ref().clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if programs.is_empty() {
            return Ok(());
        }
        let (mut runtime, _) =
            synchronized_residual_cost_state(&self.state, self.residual_cost.clone())?;
        for (address, program) in programs {
            if self
                .residual_ward_abilities
                .contains_key(&(object_id, address))
            {
                continue;
            }
            let ability = ResidualAbilityInstanceId(self.next_residual_runtime_id);
            self.next_residual_runtime_id = self
                .next_residual_runtime_id
                .checked_add(1)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
            runtime
                .install_ward(WardAbilityInstance {
                    ability_instance_id: ability,
                    protected_object: residual_object_ref(object_id),
                    controller: ResidualPlayerId(u16::from(object.controller)),
                    program,
                })
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            self.residual_ward_abilities
                .insert((object_id, address), ability);
        }
        self.residual_cost = runtime;
        Ok(())
    }

    pub fn begin_ward_target_event(
        &mut self,
        source_on_stack: ObjectId,
        source_is_spell: bool,
        source_is_counterable: bool,
        target: ObjectId,
        event_id: u64,
        batch_id: u64,
        ordered_clause_addresses: &[ClauseAddress],
    ) -> Result<WardTriggerBatch, ExecutionError> {
        self.install_residual_ward_programs(target)?;
        let (mut runtime, _) =
            synchronized_residual_cost_state(&self.state, self.residual_cost.clone())?;
        let stack = runtime
            .stack
            .get_mut(&ResidualStackObjectId(source_on_stack))
            .ok_or_else(|| ExecutionError::Adapter("Ward source is not on the stack".into()))?;
        stack.kind = if source_is_spell {
            ResidualStackObjectKind::Spell
        } else {
            ResidualStackObjectKind::Ability { source: None }
        };
        stack.counterable = source_is_counterable;
        let expected = self
            .residual_ward_abilities
            .iter()
            .filter(|((object, _), _)| *object == target)
            .map(|((_, address), ability)| (*address, *ability))
            .collect::<BTreeMap<_, _>>();
        let supplied = ordered_clause_addresses
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if supplied.len() != ordered_clause_addresses.len()
            || supplied != expected.keys().copied().collect()
        {
            return Err(ExecutionError::Adapter(
                "Ward trigger order does not cover every exact bound instance".into(),
            ));
        }
        let mut ordered_triggers = Vec::with_capacity(ordered_clause_addresses.len());
        for address in ordered_clause_addresses {
            let trigger = WardTriggerId(self.next_residual_runtime_id);
            self.next_residual_runtime_id = self
                .next_residual_runtime_id
                .checked_add(1)
                .ok_or(ExecutionError::ArithmeticOverflow)?;
            ordered_triggers.push((expected[address], trigger));
        }
        let pending = runtime
            .create_ward_triggers(
                TargetEvent {
                    event_id: TargetEventId(event_id),
                    source: residual_stack_ref(source_on_stack),
                    target: residual_object_ref(target),
                    target_was_newly_chosen: true,
                    target_choice_complete: true,
                },
                WardTriggerOrder {
                    batch_id: TriggerBatchId(batch_id),
                    ordered_triggers,
                },
            )
            .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        self.residual_cost = runtime;
        Ok(WardTriggerBatch { pending })
    }

    pub fn resolve_residual_ward_trigger(
        &mut self,
        trigger_id: WardTriggerId,
        payment: Option<WardPaymentEvidence>,
    ) -> Result<WardResolutionBatch, ExecutionError> {
        let before = self.state.clone();
        let (mut runtime, mana_origins) =
            synchronized_residual_cost_state(&self.state, self.residual_cost.clone())?;
        let resolution = resolve_ward_trigger(&mut runtime, trigger_id, payment)
            .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        commit_residual_cost_state(&mut self.state, &runtime, &mana_origins)?;
        self.residual_cost = runtime;
        Ok(WardResolutionBatch {
            resolution,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    fn install_graveyard_transform_programs(
        &mut self,
        object_id: ObjectId,
    ) -> Result<(), ExecutionError> {
        let programs = self
            .programs
            .get(&object_id)
            .into_iter()
            .flatten()
            .filter_map(|clause| {
                match clause.effects() {
                [Effect::StandaloneRuleProgram(
                    crate::bounded_oracle_runtime::StandaloneRuleProgram::GraveyardTransformKeyword(
                        program,
                    ),
                )] if graveyard_transform_clause_has_live_bridge_contract(clause) => {
                    Some(program.as_ref().clone())
                }
                _ => None,
            }
            })
            .collect::<Vec<_>>();
        if programs.is_empty() {
            return Ok(());
        }
        self.synchronize_graveyard_transform_objects(object_id, &programs[0])?;
        let source = self
            .graveyard_transform
            .object(GraveyardObjectId(object_id))
            .ok_or(ExecutionError::MissingObject(object_id))?
            .object_ref;
        for program in programs {
            self.graveyard_transform
                .install_program(source, program)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        }
        Ok(())
    }

    fn synchronize_graveyard_transform_objects(
        &mut self,
        source_id: ObjectId,
        source_program: &GraveyardTransformKeywordProgram,
    ) -> Result<(), ExecutionError> {
        for player_id in self.state.player_ids() {
            let player = GraveyardPlayerId(u16::from(player_id));
            if self
                .graveyard_transform
                .insert_player(GraveyardPlayerState::new(player))
                .is_err()
            {
                // An existing player retains pending runtime state.
            }
        }
        for object_id in self.state.object_ids() {
            let existing = self
                .graveyard_transform
                .object(GraveyardObjectId(object_id))
                .map(|object| object.object_ref);
            if let Some(reference) = existing {
                if object_id == source_id {
                    self.graveyard_transform
                        .replace_unbound_object_definition(
                            reference,
                            graveyard_definition_from_source_context(
                                source_program.source_context(),
                            ),
                        )
                        .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
                }
                continue;
            }
            let physical = self
                .state
                .object(object_id)
                .ok_or(ExecutionError::MissingObject(object_id))?;
            let definition = if object_id == source_id {
                graveyard_definition_from_source_context(source_program.source_context())
            } else {
                graveyard_definition_from_host_object(&physical)
            };
            let reference = GraveyardObjectRef {
                object_id: GraveyardObjectId(object_id),
                incarnation_id: crate::graveyard_transform_keyword_runtime::IncarnationId(
                    object_id,
                ),
            };
            let owner = GraveyardPlayerId(u16::from(physical.owner));
            let tracked = if physical.zone == Zone::Battlefield {
                GraveyardTrackedObject::permanent(
                    reference,
                    owner,
                    GraveyardPlayerId(u16::from(physical.controller)),
                    if physical.active_face == 1 {
                        GraveyardFaceId::Back
                    } else {
                        GraveyardFaceId::Front
                    },
                    definition,
                )
            } else {
                GraveyardTrackedObject::card(
                    reference,
                    owner,
                    graveyard_zone(physical.zone),
                    definition,
                )
            };
            self.graveyard_transform
                .insert_object(tracked)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        }
        Ok(())
    }

    fn install_saga_transform_program(
        &mut self,
        object_id: ObjectId,
    ) -> Result<(), ExecutionError> {
        let Some(program) = self
            .programs
            .get(&object_id)
            .into_iter()
            .flatten()
            .find_map(|clause| match clause.effects() {
                [
                    crate::bounded_oracle_runtime::Effect::StandaloneRuleProgram(
                        crate::bounded_oracle_runtime::StandaloneRuleProgram::SagaTransform(
                            program,
                        ),
                    ),
                ] => Some(program.as_ref().clone()),
                _ => None,
            })
        else {
            return Ok(());
        };
        let physical = self
            .state
            .object(object_id)
            .ok_or(ExecutionError::MissingObject(object_id))?;
        if physical.zone != Zone::Battlefield || physical.active_face != 0 {
            return Err(ExecutionError::Adapter(
                "Saga transform program must bind to its battlefield front face".into(),
            ));
        }
        let incarnation = *self
            .saga_transform_incarnations
            .entry(object_id)
            .or_insert(1);
        if self.saga_transform.object(object_id).is_none() {
            self.saga_transform
                .insert_object(SagaTransformObject {
                    id: object_id,
                    incarnation,
                    owner: physical.owner,
                    controller: Some(physical.controller),
                    zone: SagaTransformZone::Battlefield,
                    active_face: SagaTransformFaceRole::Front,
                    is_token: physical.token,
                    lore_counters: physical.counters.get("lore").copied().unwrap_or(0),
                    source_context: program.source_context(),
                })
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        }
        self.saga_transform
            .install_program(object_id, incarnation, program)
            .map_err(|error| ExecutionError::Adapter(error.to_string()))
    }

    fn move_saga_transform_object(
        &mut self,
        object_id: ObjectId,
        destination: Zone,
    ) -> Result<(), ExecutionError> {
        let Some(incarnation) = self.saga_transform_incarnations.get(&object_id).copied() else {
            return Ok(());
        };
        let controller = self.state.object(object_id).map(|object| object.controller);
        let next = self
            .saga_transform
            .move_object_for_external_effect(
                object_id,
                incarnation,
                saga_transform_zone(destination),
                controller,
            )
            .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        self.saga_transform_incarnations.insert(object_id, next);
        if destination == Zone::Battlefield {
            self.install_saga_transform_program(object_id)?;
        }
        Ok(())
    }

    /// Adds lore counters through the exact transforming-Saga lifecycle and,
    /// when chapter III is crossed, resolves its exile-and-return transaction.
    pub fn add_saga_lore_counters(
        &mut self,
        source: ObjectId,
        amount: u32,
        replacement: SagaTransformMovementReplacement,
    ) -> Result<SagaTransformBatch, ExecutionError> {
        let before_simulation = self.clone();
        let before_state = self.state.clone();
        let result = (|| {
            let (clause_address, _) = self
                .programs
                .get(&source)
                .into_iter()
                .flatten()
                .find(|clause| saga_transform_clause_has_live_bridge_contract(clause))
                .map(|clause| (clause.address(), clause))
                .ok_or_else(|| {
                    ExecutionError::Adapter("Saga transform program is not bound".into())
                })?;
            let incarnation = self
                .saga_transform_incarnations
                .get(&source)
                .copied()
                .ok_or_else(|| ExecutionError::Adapter("Saga incarnation is unavailable".into()))?;
            let trigger = self
                .saga_transform
                .put_lore_counters(source, incarnation, amount)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            if trigger.is_none() {
                let mut physical = self
                    .state
                    .object(source)
                    .ok_or(ExecutionError::MissingObject(source))?;
                let next = physical
                    .counters
                    .get("lore")
                    .copied()
                    .unwrap_or(0)
                    .checked_add(amount)
                    .ok_or(ExecutionError::InvalidAmount("lore counter overflow"))?;
                physical.counters.insert("lore".into(), next);
                self.state
                    .put_object(physical)
                    .map_err(ExecutionError::Adapter)?;
                return Ok(SagaTransformBatch {
                    source,
                    clause_address,
                    resolution: None,
                    receipt: ExecutionReceipt {
                        status: ExecutionStatus::Committed,
                        costs_paid: 0,
                        effects_applied: 1,
                        selected_targets: BTreeMap::new(),
                    },
                    delta: SimulationDelta::between(&before_state, &self.state),
                });
            }
            let resolution = self
                .saga_transform
                .resolve(trigger.expect("checked Saga trigger").id, replacement)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
            match resolution.outcome {
                SagaTransformOutcome::ReturnedTransformed { .. } => {
                    self.state
                        .move_object(source, Zone::Exile)
                        .map_err(ExecutionError::Adapter)?;
                    self.state
                        .move_object(source, Zone::Battlefield)
                        .map_err(ExecutionError::Adapter)?;
                    let mut physical = self
                        .state
                        .object(source)
                        .ok_or(ExecutionError::MissingObject(source))?;
                    physical.active_face = 1;
                    physical.controller = resolution.trigger.controller;
                    self.state
                        .put_object(physical)
                        .map_err(ExecutionError::Adapter)?;
                }
                SagaTransformOutcome::NoEffect(_) => {
                    if let Some(runtime_object) = self.saga_transform.object(source) {
                        let destination = bounded_oracle_zone(runtime_object.zone);
                        if self
                            .state
                            .object(source)
                            .is_some_and(|physical| physical.zone != destination)
                        {
                            self.state
                                .move_object(source, destination)
                                .map_err(ExecutionError::Adapter)?;
                        }
                    } else {
                        self.state.objects.remove(&source);
                    }
                }
            }
            if let Some(runtime_object) = self.saga_transform.object(source) {
                self.saga_transform_incarnations
                    .insert(source, runtime_object.incarnation);
            } else {
                self.saga_transform_incarnations.remove(&source);
            }
            self.state
                .record_mutation(format!("saga_transform_chapter_three:{source}"));
            Ok(SagaTransformBatch {
                source,
                clause_address,
                resolution: Some(resolution),
                receipt: ExecutionReceipt {
                    status: ExecutionStatus::Committed,
                    costs_paid: 0,
                    effects_applied: 1,
                    selected_targets: BTreeMap::new(),
                },
                delta: SimulationDelta::between(&before_state, &self.state),
            })
        })();
        if result.is_err() {
            *self = before_simulation;
        }
        result
    }

    pub fn entry_choice_keyword_bindings(
        &self,
        source: ObjectId,
        source_incarnation: u64,
    ) -> Vec<EntryChoiceKeywordBinding> {
        self.programs
            .get(&source)
            .into_iter()
            .flatten()
            .filter_map(|clause| match clause.effects() {
                [
                    crate::bounded_oracle_runtime::Effect::StandaloneRuleProgram(
                        crate::bounded_oracle_runtime::StandaloneRuleProgram::EntryChoiceKeyword(
                            program,
                        ),
                    ),
                ] => Some(EntryChoiceKeywordBinding {
                    binding_id: entry_choice_binding_id(clause.address()),
                    source: crate::entry_choice_keyword_runtime::ObjectRef {
                        object_id: source,
                        incarnation_id: source_incarnation,
                    },
                    program: program.as_ref().clone(),
                }),
                _ => None,
            })
            .collect()
    }

    /// Applies every exact Unleash, Riot, Bloodthirst, and Ravenous entry
    /// replacement bound to one object as a single ordered transaction.
    pub fn resolve_entry_choice_keywords(
        &mut self,
        attempt: EntryAttemptEvidence,
        input: EntryChoiceResolutionInput,
    ) -> Result<EntryChoiceKeywordBatch, ExecutionError> {
        let before = self.state.clone();
        let pending_before = self.pending_ravenous_draws.clone();
        let source = attempt.source.object_id;
        let bindings = self.entry_choice_keyword_bindings(source, attempt.source.incarnation_id);
        if bindings.is_empty() {
            return Err(ExecutionError::Adapter(
                "entry-choice keyword program is not bound".into(),
            ));
        }
        let addresses = self
            .programs
            .get(&source)
            .into_iter()
            .flatten()
            .filter(|clause| entry_choice_clause_has_live_bridge_contract(clause))
            .map(BoundedOracleClause::address)
            .collect::<Vec<_>>();
        let physical = self
            .state
            .object(source)
            .ok_or(ExecutionError::MissingObject(source))?;
        if physical.zone != Zone::Battlefield || physical.controller != attempt.controller {
            return Err(ExecutionError::Adapter(
                "entry-choice evidence does not match the entering battlefield object".into(),
            ));
        }
        if physical.counters.get("+1/+1").copied().unwrap_or(0) != attempt.prior_plus_one_counters {
            return Err(ExecutionError::Adapter(
                "entry-choice prior counter evidence does not match production state".into(),
            ));
        }
        let resolution = begin_entry_choice_keyword_transaction(attempt, bindings)
            .and_then(|pending| pending.resolve(input))
            .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        let result = (|| {
            let mut physical = self
                .state
                .object(source)
                .ok_or(ExecutionError::MissingObject(source))?;
            if resolution.plus_one_counters == 0 {
                physical.counters.remove("+1/+1");
            } else {
                physical
                    .counters
                    .insert("+1/+1".into(), resolution.plus_one_counters);
            }
            self.state
                .put_object(physical)
                .map_err(ExecutionError::Adapter)?;

            let source_incarnation = self
                .state
                .object_incarnation(source)
                .ok_or(ExecutionError::MissingObject(source))?;
            for restriction in &resolution.unleash_restrictions {
                let order = self.state.next_order();
                self.state.register_restriction(RestrictionRecord {
                    order,
                    source_identity: source,
                    source_incarnation,
                    restriction: Restriction::CannotBlock {
                        object: ObjectRef::Source,
                        duration: Duration::WhileCondition(Box::new(
                            Condition::SourceCounterCount {
                                counter: CounterKind::PlusOnePlusOne,
                                comparison: Comparison::AtLeast,
                                amount: 1,
                            },
                        )),
                    },
                });
                self.state.record_mutation(format!(
                    "install_unleash_block_restriction:{}:{}",
                    source, restriction.binding_id
                ));
            }
            if resolution.effective_riot_haste.is_some() {
                let order = self.state.next_order();
                self.state.register_continuous(ContinuousEffectRecord {
                    order,
                    source_identity: source,
                    object_identities: vec![source],
                    effect: Effect::GrantKeyword {
                        objects: ObjectRef::Source,
                        keywords: vec![Keyword::Haste],
                        duration: Duration::WhileSourceOnBattlefield,
                    },
                    duration: Duration::WhileSourceOnBattlefield,
                });
                self.state
                    .record_mutation(format!("install_riot_haste:{source}"));
            }
            for trigger in &resolution.ravenous_draw_triggers {
                if self
                    .pending_ravenous_draws
                    .insert(trigger.trigger_id, trigger.clone())
                    .is_some()
                {
                    return Err(ExecutionError::Adapter(
                        "duplicate Ravenous draw trigger identity".into(),
                    ));
                }
            }
            self.state
                .record_mutation(format!("entry_choice_keywords:{source}"));
            Ok(EntryChoiceKeywordBatch {
                source,
                clause_addresses: addresses,
                receipt: ExecutionReceipt {
                    status: ExecutionStatus::Committed,
                    costs_paid: 0,
                    effects_applied: resolution.receipts.len(),
                    selected_targets: BTreeMap::new(),
                },
                resolution,
                delta: SimulationDelta::between(&before, &self.state),
            })
        })();
        if result.is_err() {
            self.state = before;
            self.pending_ravenous_draws = pending_before;
        }
        result
    }

    pub fn resolve_ravenous_draw(
        &mut self,
        trigger_id: RavenousTriggerId,
    ) -> Result<RavenousDrawBatch, ExecutionError> {
        let before = self.state.clone();
        let trigger = self
            .pending_ravenous_draws
            .remove(&trigger_id)
            .ok_or_else(|| ExecutionError::Adapter("unknown Ravenous draw trigger".into()))?;
        let drawn_card = self
            .state
            .player(trigger.controller)
            .ok_or(ExecutionError::MissingPlayer(trigger.controller))?
            .library
            .first()
            .copied();
        if let Some(card) = drawn_card
            && let Err(error) = self.state.move_object(card, Zone::Hand)
        {
            self.pending_ravenous_draws
                .insert(trigger_id, trigger.clone());
            return Err(ExecutionError::Adapter(error));
        }
        self.state
            .record_mutation(format!("resolve_ravenous_draw:{}", trigger.controller));
        Ok(RavenousDrawBatch {
            trigger,
            drawn_card,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    fn install_old_transform_programs(
        &mut self,
        object_id: ObjectId,
        force_new_incarnation: bool,
    ) -> Result<(), ExecutionError> {
        let programs = self
            .programs
            .get(&object_id)
            .into_iter()
            .flatten()
            .filter_map(|clause| match clause.effects() {
                [
                    crate::bounded_oracle_runtime::Effect::StandaloneRuleProgram(
                        crate::bounded_oracle_runtime::StandaloneRuleProgram::OldTransform(program),
                    ),
                ] => Some((clause.address().face_index, program.as_ref().clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        if programs.is_empty() {
            return Ok(());
        }
        let physical = self
            .state
            .object(object_id)
            .ok_or(ExecutionError::MissingObject(object_id))?;
        let incarnation = self
            .old_transform_incarnations
            .entry(object_id)
            .and_modify(|incarnation| {
                if force_new_incarnation {
                    *incarnation = incarnation.saturating_add(1);
                }
            })
            .or_insert(1);
        let object = TransformableObject {
            id: object_id,
            incarnation: *incarnation,
            zone: if physical.zone == Zone::Battlefield {
                OldTransformZone::Battlefield
            } else {
                OldTransformZone::Other
            },
            transforming_double_faced: physical.back.is_some(),
            face_index: physical.active_face,
        };
        if self.old_transform.object(object_id).is_some() {
            self.old_transform
                .replace_object_incarnation(object)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        } else {
            self.old_transform
                .insert_object(object)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        }
        for (face_index, program) in programs {
            let face_index = u8::try_from(face_index).map_err(|_| {
                ExecutionError::Adapter("old transform face index exceeds runtime range".into())
            })?;
            self.old_transform
                .install_face_program(object_id, *incarnation, face_index, program)
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        }
        Ok(())
    }

    pub fn bind_compiled_card(
        &mut self,
        object_id: ObjectId,
        card: &CompiledCard,
    ) -> Result<CompiledCardProgramBinding, ExecutionError> {
        let damage_source_keywords = exact_delegated_damage_source_keywords(card)?;
        let affinity_faces = exact_affinity_faces(card)?;
        let prior_bounded = self.programs.get(&object_id).cloned();
        let prior_combat = self.combat_evasion_programs.get(&object_id).cloned();
        let prior_regeneration = self
            .delegated_regeneration_programs
            .get(&object_id)
            .cloned();
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
                    OfficialKeyword::Fear
                        | OfficialKeyword::Intimidate
                        | OfficialKeyword::Skulk
                        | OfficialKeyword::Shadow
                        | OfficialKeyword::Landwalk
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let regeneration_clauses = card
            .effects
            .delegated_oracle
            .iter()
            .filter(|clause| clause.keyword_program().keyword() == OfficialKeyword::Regenerate)
            .cloned()
            .collect::<Vec<_>>();
        if let Err(error) =
            self.bind_delegated_regeneration_programs(object_id, regeneration_clauses)
        {
            match prior_bounded {
                Some(clauses) => {
                    self.programs.insert(object_id, clauses);
                }
                None => {
                    self.programs.remove(&object_id);
                }
            }
            match prior_regeneration {
                Some(clauses) => {
                    self.delegated_regeneration_programs
                        .insert(object_id, clauses);
                }
                None => {
                    self.delegated_regeneration_programs.remove(&object_id);
                }
            }
            return Err(error);
        }
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
            match prior_regeneration {
                Some(clauses) => {
                    self.delegated_regeneration_programs
                        .insert(object_id, clauses);
                }
                None => {
                    self.delegated_regeneration_programs.remove(&object_id);
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
        self.state
            .damage_source_keywords
            .retain(|(object, _), _| *object != object_id);
        self.state.damage_source_keywords.extend(
            damage_source_keywords
                .into_iter()
                .map(|(face, keywords)| ((object_id, face), keywords)),
        );
        self.state
            .affinity_objects
            .retain(|(object, _)| *object != object_id);
        self.state
            .affinity_objects
            .extend(affinity_faces.into_iter().map(|face| (object_id, face)));
        Ok(CompiledCardProgramBinding {
            object_id,
            bounded_clause_addresses,
            combat_evasion_clause_addresses,
            blocked_bounded_clauses,
        })
    }

    fn bind_delegated_regeneration_programs(
        &mut self,
        object_id: ObjectId,
        clauses: impl IntoIterator<Item = DelegatedKeywordClause>,
    ) -> Result<(), ExecutionError> {
        if self.state.object(object_id).is_none() {
            return Err(ExecutionError::MissingObject(object_id));
        }
        let mut clauses = clauses.into_iter().collect::<Vec<_>>();
        for clause in &clauses {
            let program = clause.keyword_program();
            let fragment = program.source().oracle_fragment.as_deref().ok_or_else(|| {
                ExecutionError::Adapter("Regenerate program has no exact instruction".into())
            })?;
            if clause.runtime_version() != ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION
                || program.keyword() != OfficialKeyword::Regenerate
                || compile_regeneration_resolution_leaf_program(fragment, fragment).is_none()
            {
                return Err(ExecutionError::Adapter(
                    "delegated Regenerate program is not an exact resolution leaf".into(),
                ));
            }
        }
        clauses.sort_by_key(DelegatedKeywordClause::address);
        if clauses.is_empty() {
            self.delegated_regeneration_programs.remove(&object_id);
        } else {
            self.delegated_regeneration_programs
                .insert(object_id, clauses);
        }
        Ok(())
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
            let required_capabilities = match program.keyword() {
                OfficialKeyword::Intimidate => INTIMIDATE_BLOCK_LEGALITY_CAPABILITIES,
                OfficialKeyword::Fear
                | OfficialKeyword::Horsemanship
                | OfficialKeyword::Skulk
                | OfficialKeyword::Shadow
                | OfficialKeyword::Landwalk => COMBAT_BLOCK_LEGALITY_CAPABILITIES,
                _ => {
                    return Err(CombatBlockDeclarationError::InexactDelegatedProgram {
                        object: object_id,
                        address,
                    });
                }
            };
            if clause.runtime_version() != ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION
                || clause.required_live_bridge_capabilities() != required_capabilities
                || program.source().face_index != address.face_index
                || program.source().clause_index != address.clause_index
                || !matches!(
                    program.keyword(),
                    OfficialKeyword::Fear
                        | OfficialKeyword::Horsemanship
                        | OfficialKeyword::Intimidate
                        | OfficialKeyword::Skulk
                        | OfficialKeyword::Shadow
                        | OfficialKeyword::Landwalk
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
        let static_reference = self.static_replacement_reference(object_id);
        self.static_replacement.remove_source(static_reference);
        self.programs.remove(&object_id);
        self.combat_evasion_programs.remove(&object_id);
        self.delegated_regeneration_programs.remove(&object_id);
        self.state
            .damage_source_keywords
            .retain(|(object, _), _| *object != object_id);
        self.state
            .affinity_objects
            .retain(|(object, _)| *object != object_id);
        self.residual_ward_abilities
            .retain(|(object, _), _| *object != object_id);
        self.residual_cost
            .uninstall_ward_for_object(ResidualObjectId(object_id));
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

    /// Validate a complete simultaneous attack and block declaration against
    /// every active exact combat-restriction program.
    pub fn validate_combat_declarations(
        &self,
        attacks: &[(ObjectId, PlayerId)],
        blocks: &[(ObjectId, ObjectId)],
    ) -> Result<CombatLegalityReport, ExecutionError> {
        let mut battlefield = BattlefieldSnapshot::new();
        for player in self.state.player_ids() {
            battlefield.mark_player_battlefield_complete(player);
        }
        for object_id in self.state.object_ids() {
            let object = self.effective_object(object_id)?;
            if object.zone != Zone::Battlefield {
                continue;
            }
            let characteristics = object.characteristics();
            let power = i32::try_from(characteristics.power).map_err(|_| {
                ExecutionError::Adapter(format!(
                    "effective power for object {object_id} exceeds combat runtime range"
                ))
            })?;
            battlefield
                .insert(BattlefieldPermanent {
                    object_ref: CombatRestrictionObjectRef {
                        object_id,
                        incarnation_id: object_id,
                    },
                    controller: object.controller,
                    is_creature: characteristics.card_types.contains(&CardType::Creature),
                    has_flying: Some(characteristics.keywords.contains(&Keyword::Flying)),
                    effective_power_at_block_declaration: Some(power),
                    subtypes: characteristics
                        .subtypes
                        .iter()
                        .filter_map(|subtype| {
                            subtype
                                .eq_ignore_ascii_case("Island")
                                .then_some(PermanentSubtype::Island)
                        })
                        .collect(),
                })
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
        }
        let attacks = attacks
            .iter()
            .map(
                |(attacker, defending_player)| RestrictionAttackDeclaration {
                    attacker: CombatRestrictionObjectRef {
                        object_id: *attacker,
                        incarnation_id: *attacker,
                    },
                    defending_player: *defending_player,
                },
            )
            .collect::<Vec<_>>();
        let blocks = blocks
            .iter()
            .map(|(attacker, blocker)| RestrictionBlockAssignment {
                attacker: CombatRestrictionObjectRef {
                    object_id: *attacker,
                    incarnation_id: *attacker,
                },
                blocker: CombatRestrictionObjectRef {
                    object_id: *blocker,
                    incarnation_id: *blocker,
                },
            })
            .collect::<Vec<_>>();
        let attack_report = self
            .state
            .combat_restrictions
            .validate_attacks(&attacks, &battlefield);
        if !attack_report.is_legal() {
            return Ok(attack_report);
        }
        Ok(self
            .state
            .combat_restrictions
            .validate_blocks(&attacks, &blocks, &battlefield))
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
        let source_object = self
            .state
            .object(source)
            .ok_or(ExecutionError::MissingObject(source))?;
        if source_object.zone != Zone::Stack || source_object.controller != player {
            return Err(ExecutionError::Adapter(
                "spell-mana payment requires the caster's exact stack object".into(),
            ));
        }
        let source_incarnation = self
            .state
            .object_incarnation(source)
            .ok_or(ExecutionError::MissingObject(source))?;
        if self
            .state
            .committed_spell_cost_payments
            .get(&source)
            .is_some_and(|paid_incarnation| *paid_incarnation == source_incarnation)
        {
            return Err(ExecutionError::Adapter(
                "this exact stack object's casting cost has already been paid".into(),
            ));
        }
        if self.programs.get(&source).is_some_and(|clauses| {
            clauses
                .iter()
                .any(|clause| matches!(clause.timing(), Timing::CastingAdditionalCost))
        }) {
            return Err(ExecutionError::Adapter(
                "a spell with retained additional costs requires the complete spell-cost payment batch"
                    .into(),
            ));
        }
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
        self.state
            .committed_spell_cost_payments
            .insert(source, source_incarnation);
        self.state.record_mutation(format!(
            "spell_mana_batch_commit:{source}:{source_incarnation}:{player}"
        ));
        Ok(SpellManaPaymentBatch {
            source,
            player,
            printed_cost,
            reduced_cost,
            generic_reduction,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    /// Pays one spell's complete retained casting cost as a single state
    /// transaction. Additional-cost clauses execute first so choices such as
    /// tapping permanents for a spell reduction are present when the locked
    /// printed mana cost is calculated. If any additional cost or the printed
    /// mana payment fails, every tap, sacrifice, discard, reveal, counter,
    /// life payment, mana payment, and registration is restored.
    pub fn pay_spell_and_additional_costs<F>(
        &mut self,
        source: ObjectId,
        player: PlayerId,
        printed_cost: crate::bounded_oracle_runtime::ManaCost,
        x_value: u32,
        mut printed_payment_context: ExecutionContext,
        mut context_for: F,
    ) -> Result<SpellCostPaymentBatch, ExecutionError>
    where
        F: FnMut(&BoundedOracleClause) -> ExecutionContext,
    {
        let source_object = self
            .state
            .object(source)
            .ok_or(ExecutionError::MissingObject(source))?;
        if source_object.zone != Zone::Stack || source_object.controller != player {
            return Err(ExecutionError::Adapter(
                "complete spell-cost payment requires the caster's exact stack object".into(),
            ));
        }
        if printed_payment_context.actor != player {
            return Err(ExecutionError::Adapter(
                "printed spell-cost payment actor does not match the caster".into(),
            ));
        }
        let source_incarnation = self
            .state
            .object_incarnation(source)
            .ok_or(ExecutionError::MissingObject(source))?;
        if self
            .state
            .committed_spell_cost_payments
            .get(&source)
            .is_some_and(|paid_incarnation| *paid_incarnation == source_incarnation)
        {
            return Err(ExecutionError::Adapter(
                "this exact stack object's casting cost has already been paid".into(),
            ));
        }
        let clauses = self
            .programs
            .get(&source)
            .cloned()
            .ok_or(ExecutionError::MissingObject(source))?;
        let before = self.state.clone();
        let payment = (|| {
            let mut additional_cost_receipts = Vec::new();
            for clause in clauses {
                if !matches!(clause.timing(), Timing::CastingAdditionalCost) {
                    continue;
                }
                let mut context = context_for(&clause);
                if context.actor != player {
                    return Err(ExecutionError::Adapter(
                        "additional-cost payment actor does not match the caster".into(),
                    ));
                }
                context.source = source;
                context.window = ActionWindow::CastingAdditionalCost;
                context.x_value = x_value;
                let receipt = execute_clause(&mut self.state, &clause, &context)?;
                additional_cost_receipts.push((source, clause.address(), receipt));
            }

            printed_payment_context.source = source;
            printed_payment_context.window = ActionWindow::CastingAdditionalCost;
            printed_payment_context.x_value = x_value;
            let (reduced_cost, generic_reduction) = pay_reduced_spell_mana_cost(
                &mut self.state,
                source,
                player,
                &printed_cost,
                x_value,
                &printed_payment_context,
            )?;
            self.state
                .committed_spell_cost_payments
                .insert(source, source_incarnation);
            self.state.record_mutation(format!(
                "spell_cost_batch_commit:{source}:{source_incarnation}:{player}"
            ));
            Ok((reduced_cost, generic_reduction, additional_cost_receipts))
        })();
        let (reduced_cost, generic_reduction, additional_cost_receipts) = match payment {
            Ok(payment) => payment,
            Err(error) => {
                self.state = before;
                return Err(error);
            }
        };
        Ok(SpellCostPaymentBatch {
            source,
            source_incarnation,
            player,
            printed_cost,
            reduced_cost,
            generic_reduction,
            additional_cost_receipts,
            delta: SimulationDelta::between(&before, &self.state),
        })
    }

    /// Pays the exact global `{W}{U}{B}{R}{G}` alternative mana cost and all
    /// retained additional costs as one cast transaction. The registered
    /// battlefield permission is locked before mutation, while cast origin,
    /// timing, caster, stack incarnation, and external-cost completeness must
    /// already be evidenced by `payment_context`.
    pub fn pay_spell_and_additional_costs_with_global_alternative<F>(
        &mut self,
        source: ObjectId,
        player: PlayerId,
        mut payment_context: ExecutionContext,
        mut context_for: F,
    ) -> Result<GlobalAlternativeSpellCostPaymentBatch, ExecutionError>
    where
        F: FnMut(&BoundedOracleClause) -> ExecutionContext,
    {
        let authorization =
            authorize_global_alternative_spell_cost(&self.state, source, player, &payment_context)?;
        if self
            .state
            .committed_spell_cost_payments
            .get(&source)
            .is_some_and(|paid_incarnation| *paid_incarnation == authorization.spell_incarnation)
        {
            return Err(ExecutionError::Adapter(
                "this exact stack object's casting cost has already been paid".into(),
            ));
        }
        let clauses = self
            .programs
            .get(&source)
            .cloned()
            .ok_or(ExecutionError::MissingObject(source))?;
        let before = self.state.clone();
        let payment = (|| {
            let mut additional_cost_receipts = Vec::new();
            for clause in clauses {
                if !matches!(clause.timing(), Timing::CastingAdditionalCost) {
                    continue;
                }
                let mut context = context_for(&clause);
                if context.actor != player {
                    return Err(ExecutionError::Adapter(
                        "alternative additional-cost payment actor does not match the caster"
                            .into(),
                    ));
                }
                context.source = source;
                context.window = ActionWindow::CastingAdditionalCost;
                context.x_value = 0;
                context.cast_from_zone = Some(authorization.cast_from);
                context.cast_source_incarnation = Some(authorization.spell_incarnation);
                context.cast_origin_and_timing_legal = true;
                context.card_was_cast_with_alternative_cost = true;
                context.alternate_cast_other_costs_paid = true;
                let receipt = execute_clause(&mut self.state, &clause, &context)?;
                additional_cost_receipts.push((source, clause.address(), receipt));
            }

            payment_context.source = source;
            payment_context.actor = player;
            payment_context.window = ActionWindow::CastingAdditionalCost;
            payment_context.x_value = 0;
            let (reduced_cost, generic_reduction) = pay_reduced_spell_mana_cost(
                &mut self.state,
                source,
                player,
                &authorization.cost,
                0,
                &payment_context,
            )?;
            self.state
                .committed_spell_cost_payments
                .insert(source, authorization.spell_incarnation);
            self.state.record_mutation(format!(
                "global_alternative_spell_cost_batch_commit:{source}:{}:{player}:{}:{}",
                authorization.spell_incarnation,
                authorization.permission_source,
                authorization.permission_source_incarnation
            ));
            Ok((reduced_cost, generic_reduction, additional_cost_receipts))
        })();
        let (reduced_cost, generic_reduction, additional_cost_receipts) = match payment {
            Ok(payment) => payment,
            Err(error) => {
                self.state = before;
                return Err(error);
            }
        };
        Ok(GlobalAlternativeSpellCostPaymentBatch {
            permission_source: authorization.permission_source,
            permission_source_incarnation: authorization.permission_source_incarnation,
            source,
            source_incarnation: authorization.spell_incarnation,
            player,
            cast_from: authorization.cast_from,
            alternative_cost: authorization.cost,
            reduced_cost,
            generic_reduction,
            additional_cost_receipts,
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
            |clause| {
                matches!(clause.timing(), Timing::SpellResolution)
                    || oracle_face_modal_line_program(clause)
                        .is_some_and(OracleFaceModalLineProgram::is_resolution_header)
            },
            |clause| {
                let mut context = context_for(clause);
                context.source = source;
                context.window = ActionWindow::SpellResolution;
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
        let before_simulation = self.clone();
        let before = self.state.clone();
        let mut receipts = Vec::new();
        let static_reference = self.static_replacement_reference(source);
        self.static_replacement.remove_source(static_reference);
        self.state.damage_modifiers.retain(|_, modifier| {
            !matches!(
                &modifier.operation,
                DamageModifierOperation::Prevention(
                    DamagePrevention::PreventAllAndRemovePlusOneCounter { object, .. }
                ) if *object == source
            )
        });
        for clause in clauses {
            let window = match clause.timing() {
                Timing::Static => ActionWindow::Static,
                Timing::Replacement => ActionWindow::Replacement,
                _ => continue,
            };
            let context = ExecutionContext::new(actor, source, window);
            if let [
                Effect::StandaloneRuleProgram(
                    crate::bounded_oracle_runtime::StandaloneRuleProgram::OracleStaticReplacement(
                        program,
                    ),
                ),
            ] = clause.effects()
            {
                let snapshot = match self.static_replacement_snapshot(actor, actor) {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        *self = before_simulation;
                        return Err(error);
                    }
                };
                if let Err(error) = self.static_replacement.install_batch(
                    &snapshot,
                    static_reference,
                    u16::from(actor),
                    vec![program.as_ref().clone()],
                ) {
                    *self = before_simulation;
                    return Err(ExecutionError::Adapter(format!("{error:?}")));
                }
                if matches!(
                    program.kind(),
                    OracleStaticReplacementProgramKind::Replacement(replacement)
                        if matches!(
                            &replacement.operation,
                            StaticReplacementOperation::PreventDamageAndRemovePlusOneCounter
                        )
                ) {
                    let incarnation = self
                        .state
                        .object_incarnation(source)
                        .ok_or(ExecutionError::MissingObject(source))?;
                    let modifier_id = self.state.next_order();
                    self.state.damage_modifiers.insert(
                        modifier_id,
                        DamageModifier {
                            id: modifier_id,
                            matcher: DamageEventMatcher {
                                source: DamageSourceMatcher::Any,
                                recipient: DamageRecipientMatcher::ExactObjectIncarnation {
                                    recipient: DamageRecipient::Creature(source),
                                    incarnation,
                                },
                                kind: DamageKindMatcher::Any,
                            },
                            operation: DamageModifierOperation::Prevention(
                                DamagePrevention::PreventAllAndRemovePlusOneCounter {
                                    object: source,
                                    incarnation,
                                },
                            ),
                            persistence: DamageModifierPersistence::Persistent,
                            requirement: DamageModifierRequirement::Mandatory,
                        },
                    );
                }
                receipts.push((
                    source,
                    clause.address(),
                    ExecutionReceipt {
                        status: ExecutionStatus::Committed,
                        costs_paid: 0,
                        effects_applied: 1,
                        selected_targets: BTreeMap::new(),
                    },
                ));
                continue;
            }
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
                    *self = before_simulation;
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
            |clause| {
                matches!(clause.timing(), Timing::Activated)
                    || oracle_face_modal_line_program(clause)
                        .is_some_and(OracleFaceModalLineProgram::is_activated_header)
            },
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
        validate_land_mana_trigger_event(&self.state, &event)?;
        let mut receipts = Vec::new();
        if let TriggerEvent::SpellCast { player, .. } = &event {
            self.old_transform.record_spell_cast(*player);
        }
        let old_transform_triggers = if matches!(
            &event,
            TriggerEvent::BeginningOf {
                step: Step::Upkeep,
                ..
            }
        ) {
            self.old_transform
                .begin_upkeep()
                .map_err(|error| ExecutionError::Adapter(error.to_string()))?
        } else {
            Vec::new()
        };
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
                        if let [
                            crate::bounded_oracle_runtime::Effect::StandaloneRuleProgram(
                                crate::bounded_oracle_runtime::StandaloneRuleProgram::OldTransform(
                                    program,
                                ),
                            ),
                        ] = clause.effects()
                        {
                            let Some(pending) = old_transform_triggers.iter().find(|pending| {
                                pending.source == source
                                    && pending.program_semantic_digest == program.semantic_digest()
                            }) else {
                                continue;
                            };
                            let resolution = self
                                .old_transform
                                .resolve(pending.id)
                                .map_err(|error| ExecutionError::Adapter(error.to_string()))?;
                            let effects_applied = match resolution {
                                OldTransformResolution::Transformed { to_face, .. } => {
                                    let mut physical = self
                                        .state
                                        .object(source)
                                        .ok_or(ExecutionError::MissingObject(source))?;
                                    physical.active_face = to_face;
                                    self.state
                                        .put_object(physical)
                                        .map_err(ExecutionError::Adapter)?;
                                    self.state.record_mutation(format!(
                                        "old_transform:{}:{}",
                                        source, to_face
                                    ));
                                    1
                                }
                                OldTransformResolution::NoEffect(_) => 0,
                            };
                            receipts.push((
                                source,
                                clause.address(),
                                ExecutionReceipt {
                                    status: ExecutionStatus::Committed,
                                    costs_paid: 0,
                                    effects_applied,
                                    selected_targets: BTreeMap::new(),
                                },
                            ));
                            continue;
                        }
                        if matches!(
                            clause.effects(),
                            [Effect::StandaloneRuleProgram(
                                crate::bounded_oracle_runtime::StandaloneRuleProgram::ResidualCostKeyword(program),
                            )] if matches!(program.kind(), ResidualCostKeywordKind::Ward(_))
                        ) {
                            // Ward target events use the explicit ordered trigger
                            // and payment lifecycle exposed by
                            // `begin_ward_target_event`.
                            continue;
                        }
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
                    Timing::TypedStandaloneProgram
                        if oracle_face_modal_line_program(clause)
                            .is_some_and(OracleFaceModalLineProgram::is_triggered_header) =>
                    {
                        let mut context = context_for(source, clause, &event);
                        context.source = source;
                        context.window = ActionWindow::Triggered(event.clone());
                        populate_trigger_context(&mut context, &event);
                        match execute_clause(&mut self.state, clause, &context) {
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
                    }
                    Timing::TypedStandaloneProgram
                        if triggered_ability_clause_accepts_event(clause, &event) =>
                    {
                        let mut context = context_for(source, clause, &event);
                        context.source = source;
                        context.window = ActionWindow::Triggered(event.clone());
                        populate_trigger_context(&mut context, &event);
                        match execute_clause(&mut self.state, clause, &context) {
                            Ok(receipt) => receipts.push((source, clause.address(), receipt)),
                            Err(
                                ExecutionError::TimingMismatch
                                | ExecutionError::ConditionFailed { .. }
                                | ExecutionError::ActivationRestrictionFailed,
                            ) => {}
                            Err(error) => {
                                self.state = before;
                                return Err(error);
                            }
                        }
                    }
                    Timing::TypedStandaloneProgram
                        if matches!(
                            clause.effects(),
                            [Effect::StandaloneRuleProgram(
                                crate::bounded_oracle_runtime::StandaloneRuleProgram::LevelProgression(_)
                            )]
                        ) =>
                    {
                        let mut context = context_for(source, clause, &event);
                        context.source = source;
                        context.window = ActionWindow::Triggered(event.clone());
                        populate_trigger_context(&mut context, &event);
                        if let Some(receipt) =
                            execute_active_level_trigger(&mut self.state, source, &context)?
                        {
                            receipts.push((source, clause.address(), receipt));
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
        P: FnMut(&BoundedOracleClause) -> bool,
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
            if !predicate(&clause) {
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
        Zone::Merged => unreachable!("merged components are not independent objects"),
    }
}

fn graveyard_zone(zone: Zone) -> GraveyardZone {
    match zone {
        Zone::Library => GraveyardZone::Library,
        Zone::Hand => GraveyardZone::Hand,
        Zone::Battlefield => GraveyardZone::Battlefield,
        Zone::Graveyard => GraveyardZone::Graveyard,
        Zone::Exile => GraveyardZone::Exile,
        Zone::Stack => GraveyardZone::Stack,
        Zone::Command => GraveyardZone::Command,
        Zone::Merged => unreachable!("merged components are not independent objects"),
    }
}

fn simulation_color_index(color: Color) -> usize {
    match color {
        Color::White => 0,
        Color::Blue => 1,
        Color::Black => 2,
        Color::Red => 3,
        Color::Green => 4,
        Color::Colorless => 5,
    }
}

fn simulation_indexed_color(index: usize) -> Color {
    match index {
        0 => Color::White,
        1 => Color::Blue,
        2 => Color::Black,
        3 => Color::Red,
        4 => Color::Green,
        _ => Color::Colorless,
    }
}

fn bounded_to_graveyard_mana_color(color: Color) -> GraveyardManaColor {
    match color {
        Color::White => GraveyardManaColor::White,
        Color::Blue => GraveyardManaColor::Blue,
        Color::Black => GraveyardManaColor::Black,
        Color::Red => GraveyardManaColor::Red,
        Color::Green => GraveyardManaColor::Green,
        Color::Colorless => GraveyardManaColor::Colorless,
    }
}

fn bounded_graveyard_player(player: GraveyardPlayerId) -> Result<PlayerId, ExecutionError> {
    PlayerId::try_from(player.0)
        .map_err(|_| ExecutionError::Adapter("graveyard runtime player is out of range".into()))
}

fn bounded_graveyard_zone(zone: GraveyardZone) -> Result<Zone, ExecutionError> {
    Ok(match zone {
        GraveyardZone::Library => Zone::Library,
        GraveyardZone::Hand => Zone::Hand,
        GraveyardZone::Battlefield => Zone::Battlefield,
        GraveyardZone::Graveyard => Zone::Graveyard,
        GraveyardZone::Exile => Zone::Exile,
        GraveyardZone::Stack => Zone::Stack,
        GraveyardZone::Command => Zone::Command,
        GraveyardZone::OutsideGame => {
            return Err(ExecutionError::Adapter(
                "outside-game zone is not representable in the bounded host".into(),
            ));
        }
    })
}

fn graveyard_definition_from_source_context(
    context: &GraveyardSourceSemanticContext,
) -> GraveyardCardDefinition {
    match context {
        GraveyardSourceSemanticContext::SingleFace {
            layout,
            type_line,
            normalized_oracle_text,
        } => GraveyardCardDefinition {
            layout: *layout,
            front: GraveyardFaceCharacteristics {
                display_name: "bound source".into(),
                semantic: GraveyardFaceSemanticContext {
                    normalized_oracle_text: normalized_oracle_text.clone(),
                    type_line: type_line.clone(),
                    mana_cost: String::new(),
                    colors: BTreeSet::new(),
                    color_indicator: BTreeSet::new(),
                    root_mana_value: 0,
                    power: None,
                    toughness: None,
                    loyalty: None,
                    defense: None,
                },
                has_activated_ability: Some(false),
            },
            back: None,
        },
        GraveyardSourceSemanticContext::Transform(context) => GraveyardCardDefinition {
            layout: GraveyardCardLayout::Transform,
            front: GraveyardFaceCharacteristics {
                display_name: "bound front".into(),
                semantic: context.front.clone(),
                has_activated_ability: Some(false),
            },
            back: Some(GraveyardFaceCharacteristics {
                display_name: "bound back".into(),
                semantic: context.back.clone(),
                has_activated_ability: Some(false),
            }),
        },
    }
}

fn graveyard_definition_from_host_object(object: &PhysicalObject) -> GraveyardCardDefinition {
    let front = graveyard_face_from_host_characteristics(&object.front);
    let back = object
        .back
        .as_ref()
        .map(graveyard_face_from_host_characteristics);
    GraveyardCardDefinition {
        layout: if back.is_some() {
            GraveyardCardLayout::Transform
        } else {
            GraveyardCardLayout::Normal
        },
        front,
        back,
    }
}

fn graveyard_face_from_host_characteristics(
    characteristics: &ObjectCharacteristics,
) -> GraveyardFaceCharacteristics {
    let mut type_words = characteristics
        .card_types
        .iter()
        .filter_map(|card_type| match card_type {
            CardType::Artifact => Some("Artifact"),
            CardType::Battle => Some("Battle"),
            CardType::Creature => Some("Creature"),
            CardType::Enchantment => Some("Enchantment"),
            CardType::Instant => Some("Instant"),
            CardType::Land => Some("Land"),
            CardType::Planeswalker => Some("Planeswalker"),
            CardType::Sorcery => Some("Sorcery"),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    if !characteristics.subtypes.is_empty() {
        type_words.push_str(" \u{2014} ");
        type_words.push_str(&characteristics.subtypes.join(" "));
    }
    GraveyardFaceCharacteristics {
        display_name: characteristics.names.first().cloned().unwrap_or_default(),
        semantic: GraveyardFaceSemanticContext {
            normalized_oracle_text: String::new(),
            type_line: type_words,
            mana_cost: String::new(),
            colors: BTreeSet::new(),
            color_indicator: BTreeSet::new(),
            root_mana_value: characteristics.mana_value,
            power: Some(characteristics.power.to_string()),
            toughness: Some(characteristics.toughness.to_string()),
            loyalty: None,
            defense: None,
        },
        has_activated_ability: Some(!characteristics.abilities.is_empty()),
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

fn triggered_ability_clause_accepts_event(
    clause: &BoundedOracleClause,
    event: &TriggerEvent,
) -> bool {
    let [Effect::StandaloneRuleProgram(StandaloneRuleProgram::AbilityClause(program))] =
        clause.effects()
    else {
        return false;
    };
    let AbilityClauseTimingEnvelope::Triggered { event: trigger } = program.timing() else {
        return false;
    };
    match trigger.kind {
        AbilityClauseTriggerEventKind::PermanentTappedForMana => matches!(
            event,
            TriggerEvent::NonlandPermanentTappedForMana { .. }
                | TriggerEvent::LandTappedForMana { .. }
        ),
        AbilityClauseTriggerEventKind::OneOrMoreCreaturesDealCombatDamageToPlayer => matches!(
            event,
            TriggerEvent::CreatureCombatDamageBatchToPlayer { .. }
        ),
        _ => false,
    }
}

fn validate_land_mana_trigger_event<S: OracleStateAdapter>(
    state: &S,
    event: &TriggerEvent,
) -> Result<(), ExecutionError> {
    let TriggerEvent::LandTappedForMana { evidence } = event else {
        return Ok(());
    };
    let source = state
        .object(evidence.source)
        .ok_or(ExecutionError::MissingObject(evidence.source))?;
    if evidence.source_zone != Zone::Battlefield
        || evidence.mana_ability_occurrence == 0
        || state.object_incarnation(evidence.source) != Some(evidence.source_incarnation)
        || state.player(evidence.source_controller).is_none()
        || source.zone != Zone::Battlefield
        || source.controller != evidence.source_controller
        || !source
            .characteristics()
            .card_types
            .contains(&CardType::Land)
        || evidence.produced_mana_types.is_empty()
        || evidence
            .produced_mana_types
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != evidence.produced_mana_types.len()
    {
        return Err(ExecutionError::Adapter(
            "land-mana trigger event lacks exact live source, incarnation, zone, controller, occurrence, or produced-type evidence"
                .into(),
        ));
    }
    Ok(())
}

fn event_player(event: &TriggerEvent) -> Option<PlayerId> {
    match event {
        TriggerEvent::ChaosEnsued {
            planar_controller: player,
            ..
        }
        | TriggerEvent::SpellCast { player, .. }
        | TriggerEvent::CardDrawn { player, .. }
        | TriggerEvent::LifeGained { player, .. }
        | TriggerEvent::TokenCreated { player, .. }
        | TriggerEvent::PlayerAction { player, .. }
        | TriggerEvent::CombatDamageToPlayer { player, .. }
        | TriggerEvent::CreatureCombatDamageBatchToPlayer { player, .. }
        | TriggerEvent::DamageToPlayer { player, .. }
        | TriggerEvent::BattlefieldConditionChanged { player, .. } => Some(*player),
        TriggerEvent::BecameTarget { controller, .. }
        | TriggerEvent::NonlandPermanentTappedForMana { controller, .. } => Some(*controller),
        TriggerEvent::LandTappedForMana { evidence } => Some(evidence.source_controller),
        TriggerEvent::BeginningOf { active_player, .. } => Some(*active_player),
        TriggerEvent::ObjectEntered { .. }
        | TriggerEvent::ObjectAttacked { .. }
        | TriggerEvent::ObjectBlocked { .. }
        | TriggerEvent::SchemeSetInMotion { .. }
        | TriggerEvent::ObjectTappedForMana { .. }
        | TriggerEvent::AttachmentTargetEvent { .. }
        | TriggerEvent::CombatDamageToObject { .. }
        | TriggerEvent::DamageToObject { .. }
        | TriggerEvent::ObjectEvent { .. }
        | TriggerEvent::CountersPlaced { .. } => None,
    }
}

fn saga_transform_zone(zone: Zone) -> SagaTransformZone {
    match zone {
        Zone::Battlefield => SagaTransformZone::Battlefield,
        Zone::Exile => SagaTransformZone::Exile,
        Zone::Command => SagaTransformZone::Command,
        Zone::Graveyard => SagaTransformZone::Graveyard,
        Zone::Hand => SagaTransformZone::Hand,
        Zone::Library => SagaTransformZone::Library,
        Zone::Stack => SagaTransformZone::OutsideGame,
        Zone::Merged => SagaTransformZone::OutsideGame,
    }
}

fn bounded_oracle_zone(zone: SagaTransformZone) -> Zone {
    match zone {
        SagaTransformZone::Battlefield => Zone::Battlefield,
        SagaTransformZone::Exile => Zone::Exile,
        SagaTransformZone::Command => Zone::Command,
        SagaTransformZone::Graveyard => Zone::Graveyard,
        SagaTransformZone::Hand => Zone::Hand,
        SagaTransformZone::Library => Zone::Library,
        SagaTransformZone::OutsideGame => Zone::Stack,
    }
}

fn entry_choice_binding_id(address: ClauseAddress) -> u64 {
    (u64::from(address.face_index) << 32) | (u64::from(address.clause_index) + 1)
}

fn populate_trigger_context(context: &mut ExecutionContext, event: &TriggerEvent) {
    match event {
        TriggerEvent::ChaosEnsued {
            affected_planes,
            planar_controller,
            ..
        } => {
            if affected_planes
                .iter()
                .any(|evidence| evidence.object == context.source)
            {
                context.triggering_object = Some(context.source);
            }
            context.that_player = Some(*planar_controller);
        }
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
        | TriggerEvent::ObjectTappedForMana { object }
        | TriggerEvent::NonlandPermanentTappedForMana { object, .. } => {
            context.triggering_object = Some(*object);
        }
        TriggerEvent::LandTappedForMana { evidence } => {
            context.triggering_object = Some(evidence.source);
            context.that_player = Some(evidence.source_controller);
        }
        TriggerEvent::ObjectBlocked { blocked, .. } => {
            context.triggering_object = Some(*blocked);
        }
        TriggerEvent::ObjectEvent { object, .. } | TriggerEvent::CountersPlaced { object, .. } => {
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
        TriggerEvent::CreatureCombatDamageBatchToPlayer {
            sources, player, ..
        } => {
            context.triggering_object = sources.first().map(|source| source.object);
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
        TriggerEvent::BattlefieldConditionChanged { player, .. } => {
            context.that_player = Some(*player);
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
        class_level: subtypes_from_type_line(&card.type_line)
            .iter()
            .any(|subtype| subtype.eq_ignore_ascii_case("Class"))
            .then_some(1)
            .unwrap_or(0),
        crew_power_bonus: 0,
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

fn regeneration_object_reference(object: ObjectId) -> RegenerationObjectReference {
    RegenerationObjectReference {
        object: KeywordObjectId(object),
        incarnation: RegenerationIncarnationId(object),
    }
}

const RESIDUAL_SYNTHETIC_MANA_TAG: u64 = 0xB000_0000_0000_0000;

fn residual_object_ref(object: ObjectId) -> ResidualObjectRef {
    ResidualObjectRef {
        object_id: ResidualObjectId(object),
        incarnation_id: ResidualIncarnationId(object),
    }
}

fn residual_stack_ref(object: ObjectId) -> ResidualStackObjectRef {
    ResidualStackObjectRef {
        stack_id: ResidualStackObjectId(object),
        incarnation_id: ResidualStackIncarnationId(object),
    }
}

fn residual_zone(zone: Zone) -> ResidualZone {
    match zone {
        Zone::Library => ResidualZone::Library,
        Zone::Hand => ResidualZone::Hand,
        Zone::Battlefield => ResidualZone::Battlefield,
        Zone::Graveyard => ResidualZone::Graveyard,
        Zone::Exile => ResidualZone::Exile,
        Zone::Stack => ResidualZone::Stack,
        Zone::Command => ResidualZone::Command,
        Zone::Merged => unreachable!("merged components are not independent objects"),
    }
}

fn bounded_residual_zone(zone: ResidualZone) -> Zone {
    match zone {
        ResidualZone::Library => Zone::Library,
        ResidualZone::Hand => Zone::Hand,
        ResidualZone::Battlefield => Zone::Battlefield,
        ResidualZone::Graveyard => Zone::Graveyard,
        ResidualZone::Exile | ResidualZone::OutsideGame => Zone::Exile,
        ResidualZone::Stack => Zone::Stack,
        ResidualZone::Command => Zone::Command,
    }
}

fn residual_mana_color(color: Color) -> ResidualManaColor {
    match color {
        Color::White => ResidualManaColor::White,
        Color::Blue => ResidualManaColor::Blue,
        Color::Black => ResidualManaColor::Black,
        Color::Red => ResidualManaColor::Red,
        Color::Green => ResidualManaColor::Green,
        Color::Colorless => ResidualManaColor::Colorless,
    }
}

fn residual_card_type(card_type: CardType) -> Option<ResidualCardType> {
    match card_type {
        CardType::Artifact => Some(ResidualCardType::Artifact),
        CardType::Battle => Some(ResidualCardType::Battle),
        CardType::Creature => Some(ResidualCardType::Creature),
        CardType::Enchantment => Some(ResidualCardType::Enchantment),
        CardType::Instant => Some(ResidualCardType::Instant),
        CardType::Land => Some(ResidualCardType::Land),
        CardType::Planeswalker => Some(ResidualCardType::Planeswalker),
        CardType::Sorcery => Some(ResidualCardType::Sorcery),
        CardType::Spell | CardType::Permanent => None,
    }
}

fn synchronized_residual_cost_state(
    state: &InMemoryOracleState,
    mut runtime: ResidualCostGameState,
) -> Result<
    (
        ResidualCostGameState,
        BTreeMap<ResidualManaUnitId, Option<Color>>,
    ),
    ExecutionError,
> {
    runtime.players.clear();
    runtime.objects.clear();
    runtime.stack.clear();
    let mut origins = BTreeMap::new();
    for player in state.players.values() {
        let residual_player = ResidualPlayerId(u16::from(player.id));
        let mut projected = ResidualPlayerState {
            life: u32::try_from(player.life).map_err(|_| {
                ExecutionError::Adapter("Ward player life is outside runtime range".into())
            })?,
            poison_counters: player.counters.get("poison").copied().unwrap_or(0),
            mana_pool: BTreeMap::new(),
        };
        let mut represented = [0u32; 6];
        for unit in &player.expiring_mana {
            if unit.id & RESIDUAL_SYNTHETIC_MANA_TAG != 0 {
                return Err(ExecutionError::Adapter(
                    "host mana identity collides with Ward projection namespace".into(),
                ));
            }
            represented[simulation_color_index(unit.color)] = represented
                [simulation_color_index(unit.color)]
            .checked_add(1)
            .ok_or(ExecutionError::ArithmeticOverflow)?;
            let id = ResidualManaUnitId(unit.id);
            projected.mana_pool.insert(
                id,
                ResidualManaUnit {
                    id,
                    color: residual_mana_color(unit.color),
                    from_snow_source: state.object(unit.source_identity).is_some_and(|source| {
                        source
                            .characteristics()
                            .supertypes
                            .contains(&Supertype::Snow)
                    }),
                },
            );
            origins.insert(id, Some(unit.color));
        }
        for (index, total) in player.mana.colored.iter().copied().enumerate() {
            let remaining = total.checked_sub(represented[index]).ok_or_else(|| {
                ExecutionError::Adapter("expiring Ward mana exceeds host pool".into())
            })?;
            for ordinal in 0..remaining {
                let id = ResidualManaUnitId(
                    RESIDUAL_SYNTHETIC_MANA_TAG
                        | (u64::from(player.id) << 40)
                        | ((index as u64) << 32)
                        | u64::from(ordinal),
                );
                projected.mana_pool.insert(
                    id,
                    ResidualManaUnit {
                        id,
                        color: residual_mana_color(simulation_indexed_color(index)),
                        from_snow_source: false,
                    },
                );
                origins.insert(id, Some(simulation_indexed_color(index)));
            }
        }
        for ordinal in 0..player.mana.unrestricted {
            let id = ResidualManaUnitId(
                RESIDUAL_SYNTHETIC_MANA_TAG
                    | (u64::from(player.id) << 40)
                    | (7u64 << 32)
                    | u64::from(ordinal),
            );
            projected.mana_pool.insert(
                id,
                ResidualManaUnit {
                    id,
                    color: ResidualManaColor::Colorless,
                    from_snow_source: false,
                },
            );
            origins.insert(id, None);
        }
        runtime.players.insert(residual_player, projected);
    }
    for object in state.objects.values() {
        let characteristics = object.characteristics();
        let reference = residual_object_ref(object.id);
        runtime.objects.insert(
            ResidualObjectId(object.id),
            ResidualGameObject {
                object_ref: reference,
                owner: ResidualPlayerId(u16::from(object.owner)),
                controller: ResidualPlayerId(u16::from(object.controller)),
                zone: residual_zone(object.zone),
                characteristics: ResidualCharacteristics {
                    card_types: characteristics
                        .card_types
                        .iter()
                        .copied()
                        .filter_map(residual_card_type)
                        .collect(),
                    supertypes: characteristics
                        .supertypes
                        .iter()
                        .filter_map(|kind| match kind {
                            Supertype::Basic => Some(ResidualSupertype::Basic),
                            Supertype::Legendary => Some(ResidualSupertype::Legendary),
                            Supertype::Snow => Some(ResidualSupertype::Snow),
                            _ => None,
                        })
                        .collect(),
                    subtypes: characteristics.subtypes.iter().cloned().collect(),
                    mana_value: characteristics.mana_value,
                    power: i32::try_from(characteristics.power).map_err(|_| {
                        ExecutionError::Adapter("Ward object power is outside runtime range".into())
                    })?,
                    is_token: object.token,
                    has_affinity: false,
                },
                tapped: object.tapped,
                can_receive_minus_one_minus_one_counters: true,
                minus_one_minus_one_counters: object.counters.get("-1/-1").copied().unwrap_or(0),
            },
        );
        if object.zone == Zone::Stack {
            let stack_ref = residual_stack_ref(object.id);
            runtime.stack.insert(
                stack_ref.stack_id,
                ResidualStackObject {
                    stack_ref,
                    controller: ResidualPlayerId(u16::from(object.controller)),
                    kind: ResidualStackObjectKind::Spell,
                    counterable: true,
                    status: ResidualStackObjectStatus::OnStack,
                },
            );
        }
    }
    Ok((runtime, origins))
}

fn commit_residual_cost_state(
    state: &mut InMemoryOracleState,
    runtime: &ResidualCostGameState,
    origins: &BTreeMap<ResidualManaUnitId, Option<Color>>,
) -> Result<(), ExecutionError> {
    for (id, projected) in &runtime.players {
        let player_id = u8::try_from(id.0)
            .map_err(|_| ExecutionError::Adapter("Ward player id overflow".into()))?;
        let player = state
            .players
            .get_mut(&player_id)
            .ok_or(ExecutionError::MissingPlayer(player_id))?;
        player.life = i64::from(projected.life);
        if projected.poison_counters == 0 {
            player.counters.remove("poison");
        } else {
            player
                .counters
                .insert("poison".into(), projected.poison_counters);
        }
        let remaining = projected.mana_pool.keys().copied().collect::<BTreeSet<_>>();
        player
            .expiring_mana
            .retain(|unit| remaining.contains(&ResidualManaUnitId(unit.id)));
        player.mana = crate::bounded_oracle_consumer::ManaPool::default();
        for mana_id in remaining {
            match origins.get(&mana_id) {
                Some(Some(color)) => {
                    player.mana.colored[simulation_color_index(*color)] = player.mana.colored
                        [simulation_color_index(*color)]
                    .checked_add(1)
                    .ok_or(ExecutionError::ArithmeticOverflow)?;
                }
                Some(None) => {
                    player.mana.unrestricted = player
                        .mana
                        .unrestricted
                        .checked_add(1)
                        .ok_or(ExecutionError::ArithmeticOverflow)?;
                }
                None => {
                    return Err(ExecutionError::Adapter(
                        "Ward runtime returned unknown mana identity".into(),
                    ));
                }
            }
        }
    }
    for projected in runtime.objects.values() {
        if let Some(object) = state.objects.get_mut(&projected.object_ref.object_id.0) {
            object.zone = bounded_residual_zone(projected.zone);
            object.tapped = projected.tapped;
            if projected.minus_one_minus_one_counters == 0 {
                object.counters.remove("-1/-1");
            } else {
                object
                    .counters
                    .insert("-1/-1".into(), projected.minus_one_minus_one_counters);
            }
        }
    }
    for stack in runtime.stack.values() {
        if stack.status != ResidualStackObjectStatus::Countered {
            continue;
        }
        let object_id = stack.stack_ref.stack_id.0;
        match stack.kind {
            ResidualStackObjectKind::Spell => {
                let object = state
                    .objects
                    .get_mut(&object_id)
                    .ok_or(ExecutionError::MissingObject(object_id))?;
                object.zone = Zone::Graveyard;
            }
            ResidualStackObjectKind::Ability { .. } => {
                state.objects.remove(&object_id);
            }
        }
    }
    Ok(())
}

fn pregame_object_reference(object: ObjectId) -> PregameObjectRef {
    PregameObjectRef {
        object_id: object,
        incarnation_id: object,
    }
}

fn bounded_pregame_zone(zone: PregameZone) -> Result<Zone, ExecutionError> {
    match zone {
        PregameZone::Battlefield => Ok(Zone::Battlefield),
        PregameZone::Command => Ok(Zone::Command),
        PregameZone::Exile => Ok(Zone::Exile),
        PregameZone::Graveyard => Ok(Zone::Graveyard),
        PregameZone::Hand => Ok(Zone::Hand),
        PregameZone::Library => Ok(Zone::Library),
        PregameZone::OutsideGame => Err(ExecutionError::Adapter(
            "bounded simulation has no outside-game zone".into(),
        )),
    }
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
        expiring_mana: Vec::new(),
        commander_identity,
        library: Vec::new(),
        counters: Default::default(),
        chosen_creature_type: None,
        maximum_hand_size: Some(7),
        land_plays_remaining: 1,
    });
}
