//! Explicit compiler backends for complete Oracle clauses.
//!
//! Native bounded clauses, delegated keyword programs, typed ability programs,
//! and lossless syntax recognition remain different products. Syntax-only
//! recognition preserves complete source structure and compiler diagnostics,
//! but never claims executable semantics or live bridge capabilities.

use std::fmt;

use sha2::{Digest, Sha256};

use crate::ability_clause_bridge::{AbilityClauseTimingEnvelope, compile_ability_clause_bridge};
use crate::alternate_zone_cast_keyword_runtime::{
    SnapshotCandidateClass as AlternateZoneCandidateClass,
    classify_snapshot_candidate as classify_alternate_zone_candidate,
    compile_alternate_zone_cast_keyword_program,
};
use crate::attachment_filter_runtime::{
    AttachmentFilterCompilerInput, compile_attachment_filter_program, prior_attachment_owner,
};
use crate::bounded_oracle_runtime::{
    BoundedOracleCardContext, BoundedOracleClause, ClauseAddress, CompileError, Effect,
    OracleClauseInput, OracleCompositionChildBinding, OracleCompositionChildProgram,
    OracleFaceModalLineProgram, OracleFaceModalLineRole, StandaloneRuleProgram, Timing,
    compile_bounded_oracle_clause_after_syntax_validation,
    compile_bounded_oracle_clause_after_syntax_validation_with_context, normalize_oracle_clause,
    retain_ability_clause_bridge_program, retain_alternate_zone_cast_keyword_program,
    retain_attachment_filter_program, retain_cast_choice_keyword_program,
    retain_cast_modifier_keyword_program, retain_combat_special_keyword_program,
    retain_combat_trigger_keyword_program, retain_common_action_procedure_program,
    retain_creature_counter_keyword_program, retain_delayed_counter_keyword_program,
    retain_extended_cast_zone_keyword_program, retain_face_down_merge_keyword_program,
    retain_graveyard_hand_library_keyword_program, retain_graveyard_transform_keyword_program,
    retain_level_progression_program, retain_linked_cast_cost_keyword_program,
    retain_oracle_action_program, retain_oracle_clause_composition_program,
    retain_oracle_face_modal_line_program, retain_regeneration_action_program,
    retain_residual_cost_keyword_program, retain_static_special_keyword_program,
};
use crate::cast_choice_keyword_runtime::{
    CastChoiceClauseClassification, classify_cast_choice_keyword_clause,
    compile_cast_choice_keyword_program, derive_cast_choice_source_context,
};
use crate::cast_modifier_keyword_runtime::{
    SnapshotCandidateClass as CastModifierCandidateClass,
    classify_snapshot_candidate as classify_cast_modifier_candidate,
    compile_cast_modifier_keyword_program,
};
use crate::combat_special_keyword_runtime::{
    CombatSpecialClauseClassification, classify_combat_special_keyword_clause,
    reviewed_normalized_source as reviewed_combat_special_normalized_source,
};
use crate::combat_trigger_keyword_runtime::{
    CombatTriggerClauseClassification, classify_combat_trigger_keyword_clause,
    reviewed_combat_trigger_normalized_source,
};
use crate::common_action_procedure_runtime::{
    CommonActionClauseClassification, classify_common_action_clause,
    reviewed_common_action_normalized_source,
};
use crate::creature_counter_keyword_runtime::{
    SnapshotCandidateClass as CreatureCounterCandidateClass,
    classify_creature_counter_snapshot_candidate, compile_creature_counter_keyword_program,
};
use crate::delayed_counter_keyword_runtime::{
    DelayedCounterClauseClassification, classify_delayed_counter_keyword_clause,
};
use crate::extended_cast_zone_keyword_runtime::{
    SnapshotCandidateClass as ExtendedCastZoneCandidateClass,
    classify_extended_cast_zone_snapshot_candidate, compile_extended_cast_zone_keyword_program,
};
use crate::face_down_merge_keyword_runtime::{
    SnapshotCandidateClass as FaceDownMergeCandidateClass,
    classify_snapshot_candidate as classify_face_down_merge_candidate,
    compile_face_down_merge_keyword_program,
};
use crate::graveyard_hand_library_keyword_runtime::{
    SnapshotCandidateClass as ZoneKeywordCandidateClass,
    SourceSemanticContext as ZoneKeywordSourceSemanticContext,
    classify_snapshot_candidate as classify_zone_keyword_snapshot_candidate,
    compile_zone_keyword_program,
};
use crate::graveyard_transform_keyword_runtime::{
    CardLayout as GraveyardTransformCardLayout, FaceId as GraveyardTransformFaceId,
    SnapshotCandidateClass as GraveyardTransformCandidateClass,
    SourceSemanticContext as GraveyardTransformSourceSemanticContext,
    classify_snapshot_candidate as classify_graveyard_transform_candidate,
    compile_graveyard_transform_keyword_program,
};
use crate::keyword_rules_runtime::{
    AffinityCountedObjects, AscendPermanentApplication, AscendSpellApplication,
    BackupGrantedAbilitySet, BargainSacrificeChoice, CascadeExileProcedure,
    CascadeTriggerTransition, CascadeUncastCardDestination, ChangelingCharacteristic,
    ChangelingFunctionScope, CipherEncodeChoice, CommanderPartnerCounterpartRequirement,
    CommanderPartnerReference, CommanderPartnerSourceRequirement, CommanderPartnerTracking,
    CommanderPartnerVariant, ConvokePaymentExchange, ConvokePaymentTiming,
    DayNightDesignationTransform, DayNightEntryBehavior, DayNightFaceRole, DayNightGlobalLifecycle,
    DayNightImmediateAlignment, DayNightInitialDesignation, DayNightInvalidEntryDestination,
    DayNightSharedTeamSpellCountRule, DayNightTransformBatch, DayNightZoneScope,
    DeathReturnBattlefieldController, DeathReturnCardIdentity, DeathReturnCounterCondition,
    DeathReturnCounterKind, DeathReturnReplacementInteraction, DeathReturnResolutionRequirement,
    DeathReturnTokenInteraction, DeathReturnTriggerMultiplicity, DeathReturnTriggerTransition,
    DelvePaymentExchange, EvolveComparison, EvolveEventDefinition, EvolveInformationRule,
    EvolveTriggerTransition, ExaltedEffectDuration, ExaltedTriggerTransition,
    ExploitEventDefinition, ExploitSacrificeChoice, ExploitTriggerTransition,
    ExtortTriggerTransition, FlankingBlockerPredicate, FlankingEffectDuration,
    FlankingEffectRecipient, FlankingTriggerMultiplicity, FlankingTriggerTransition,
    FuseCastChoice, FuseFunctionScope, FuseResolutionOrder, HorsemanshipBlockRestriction,
    ImproviseFunctionZone, ImprovisePaymentExchange, ImprovisePaymentTiming,
    InfectCreatureDamageResult, InfectPlayerDamageResult, IntimidateBlockerQualification,
    KeywordCompileError, KeywordProgram, KeywordProgramInput, KeywordProgramKind,
    LivingWeaponTokenDefinition, MentorTargetRestriction, MentorTriggerTransition,
    MyriadTriggerTransition, OfficialKeyword, ReboundDelayedTrigger, ReboundReplacementEvent,
    RegenerationProgram, RegenerationProtectionWindow, RegenerationRecipientCardinality,
    RegenerationRecipientScope, RegenerationRecipientSelectionTime, RegenerationReminderEvidence,
    RegenerationReminderReferent, RegenerationReplacement, RegenerationTargetFilter,
    RetraceFunctionZone, SoulbondEligibility, SoulbondPairChoice, SoulbondPairLifecycle,
    SoulbondTriggerSet, SoulbondUnpairTransition, SpeedIncreaseEvent, SpeedIncreaseLimit,
    SpeedInitialization, SpeedPersistence, SpeedSourceScope, SpellStackFunctionScope,
    SpreeFunctionZone, SpreeModeChoice, SpreeModeCostBinding, ToxicDamageEvent,
    ToxicPoisonApplication, ToxicValueCombination, WitherCreatureDamageApplication,
    compile_keyword_program,
};
use crate::level_progression_runtime::LevelProgressionProgram;
use crate::linked_cast_cost_keyword_runtime::{
    SnapshotCandidateClass as LinkedCastCostCandidateClass,
    classify_linked_cast_cost_snapshot_candidate, compile_linked_cast_cost_keyword_program,
};
use crate::oracle_action_algebra_runtime::{
    OracleActionClassification, OracleActionCompileInput, OracleActionSemanticContext,
    classify_oracle_action_instruction, reviewed_oracle_action_normalized_source,
};
use crate::oracle_clause_composition::{
    OracleClauseCompositionInput, OracleCompositionNode, SemanticCapability, SourceSpan,
    TypedChildBinding, TypedOracleChildProgram, parse_oracle_clause_composition,
};
use crate::oracle_clause_syntax::{
    OracleClauseSyntaxError, OracleClauseSyntaxInput, OracleSyntaxProvenance,
    OracleSyntaxSemanticContext, RecognizedOracleClauseSyntax, ValidatedOracleClauseLine,
    recognize_oracle_clause_syntax, validate_oracle_clause_line,
};
use crate::oracle_face_program_assembler::{
    ClosedModalChildCompiler, ClosedModalChildProgram, ModalChildCompilation, ModalChildSource,
    OracleFaceProgramInput, OracleFaceProvenance,
    assemble_oracle_face_modal_program_containing_offset,
};
use crate::regeneration_action_runtime::{
    RegenerationClauseClassification, classify_regeneration_action_clause,
    contains_regeneration_lexeme,
};
use crate::residual_cost_keyword_runtime::compile_residual_cost_keyword_program;
use crate::static_special_keyword_runtime::{
    StaticSpecialClauseClassification, StaticSpecialSourceContext,
    classify_static_special_keyword_clause, reviewed_static_special_normalized_source,
};

pub const ORACLE_CLAUSE_BACKEND_COMPILER_VERSION: &str = "oracle-clause-backend-compiler-0.32";
pub const ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION: &str = "oracle-clause-backend-runtime-0.11";

const DEVOID_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::EffectiveColorCharacteristics,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LiveBridgeCapability {
    StaticKeywordInstallation,
    StaticEffectLifecycle,
    GlobalDayNightLifecycle,
    TurnSpellCountTracking,
    DoubleFacedTransformLifecycle,
    PlayerSpeedLifecycle,
    OpponentLifeLossEventTracking,
    StateBasedActionLifecycle,
    EffectiveColorCharacteristics,
    CastTimingPermission,
    CombatAttackLegality,
    CombatAttackDeclaration,
    SummoningSicknessPermission,
    TapAbilityActivationLegality,
    CombatBlockLegality,
    CombatBlockTransition,
    CombatDamageAssignment,
    CombatDamageSteps,
    DamageEventModification,
    DamageTransaction,
    DamagePrevention,
    TargetingLegality,
    ControlledObjectSetResolution,
    DestructionPrevention,
    RegenerationReplacementLifecycle,
    AdditionalCostPayment,
    LinkedAbilityState,
    AlternativeCastingLifecycle,
    FaceDownCharacteristics,
    SpecialActionExecution,
    SpellCostPayment,
    ResourceCostPayment,
    TokenCreation,
    TokenAbilityInstallation,
    ActivatedCostPayment,
    AttachmentLifecycle,
    CounterLifecycle,
    PoisonCounterLifecycle,
    ActivatedAbilityExecution,
    NoncreatureCastEventTracking,
    TriggeredAbilityResolution,
    TemporaryPowerToughnessMutation,
    LibraryProcedure,
    DrawResolution,
    ZoneChangeReplacement,
    ZoneChangeTriggerLifecycle,
    PublicZoneObjectTracking,
    TokenZoneLifecycle,
    PermanentSacrifice,
    CommanderDeckConstructionLifecycle,
    CommanderPairRulesLifecycle,
    ExploitEventTracking,
    OptionalCreatureSacrificeChoice,
    CreaturePairingLifecycle,
    ControlChangeTracking,
    DynamicPowerToughnessEvaluation,
    EvolveEventTracking,
    CreatureTapCostPayment,
    ArtifactTapCostPayment,
    ModalSpellChoice,
    ModeAssociatedAdditionalCostBinding,
    PlayerLifeTotalMutation,
    DelayedTriggeredAbilityLifecycle,
    SpellCopyLifecycle,
    CastWithoutManaCost,
    TemporaryAbilityGrantLifecycle,
    BattlefieldDesignationLifecycle,
}

impl LiveBridgeCapability {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::StaticKeywordInstallation => "static-keyword-installation/v1",
            Self::StaticEffectLifecycle => "static-effect-lifecycle/v1",
            Self::GlobalDayNightLifecycle => "global-day-night-lifecycle/v1",
            Self::TurnSpellCountTracking => "turn-spell-count-tracking/v1",
            Self::DoubleFacedTransformLifecycle => "double-faced-transform-lifecycle/v1",
            Self::PlayerSpeedLifecycle => "player-speed-lifecycle/v1",
            Self::OpponentLifeLossEventTracking => "opponent-life-loss-event-tracking/v1",
            Self::StateBasedActionLifecycle => "state-based-action-lifecycle/v1",
            Self::EffectiveColorCharacteristics => "effective-color-characteristics/v1",
            Self::CastTimingPermission => "cast-timing-permission/v1",
            Self::CombatAttackLegality => "combat-attack-legality/v1",
            Self::CombatAttackDeclaration => "combat-attack-declaration/v1",
            Self::SummoningSicknessPermission => "summoning-sickness-permission/v1",
            Self::TapAbilityActivationLegality => "tap-ability-activation-legality/v1",
            Self::CombatBlockLegality => "combat-block-legality/v1",
            Self::CombatBlockTransition => "combat-block-transition/v1",
            Self::CombatDamageAssignment => "combat-damage-assignment/v1",
            Self::CombatDamageSteps => "combat-damage-steps/v1",
            Self::DamageEventModification => "damage-event-modification/v1",
            Self::DamageTransaction => "damage-transaction/v1",
            Self::DamagePrevention => "damage-prevention/v1",
            Self::TargetingLegality => "targeting-legality/v1",
            Self::ControlledObjectSetResolution => "controlled-object-set-resolution/v1",
            Self::DestructionPrevention => "destruction-prevention/v1",
            Self::RegenerationReplacementLifecycle => "regeneration-replacement-lifecycle/v1",
            Self::AdditionalCostPayment => "additional-cost-payment/v1",
            Self::LinkedAbilityState => "linked-ability-state/v1",
            Self::AlternativeCastingLifecycle => "alternative-casting-lifecycle/v1",
            Self::FaceDownCharacteristics => "face-down-characteristics/v1",
            Self::SpecialActionExecution => "special-action-execution/v1",
            Self::SpellCostPayment => "spell-cost-payment/v1",
            Self::ResourceCostPayment => "resource-cost-payment/v1",
            Self::TokenCreation => "token-creation/v1",
            Self::TokenAbilityInstallation => "token-ability-installation/v1",
            Self::ActivatedCostPayment => "activated-cost-payment/v1",
            Self::AttachmentLifecycle => "attachment-lifecycle/v1",
            Self::CounterLifecycle => "counter-lifecycle/v1",
            Self::PoisonCounterLifecycle => "poison-counter-lifecycle/v1",
            Self::ActivatedAbilityExecution => "activated-ability-execution/v1",
            Self::NoncreatureCastEventTracking => "noncreature-cast-event-tracking/v1",
            Self::TriggeredAbilityResolution => "triggered-ability-resolution/v1",
            Self::TemporaryPowerToughnessMutation => "temporary-power-toughness-mutation/v1",
            Self::LibraryProcedure => "library-procedure/v1",
            Self::DrawResolution => "draw-resolution/v1",
            Self::ZoneChangeReplacement => "zone-change-replacement/v1",
            Self::ZoneChangeTriggerLifecycle => "zone-change-trigger-lifecycle/v1",
            Self::PublicZoneObjectTracking => "public-zone-object-tracking/v1",
            Self::TokenZoneLifecycle => "token-zone-lifecycle/v1",
            Self::PermanentSacrifice => "permanent-sacrifice/v1",
            Self::CommanderDeckConstructionLifecycle => "commander-deck-construction-lifecycle/v1",
            Self::CommanderPairRulesLifecycle => "commander-pair-rules-lifecycle/v1",
            Self::ExploitEventTracking => "exploit-event-tracking/v1",
            Self::OptionalCreatureSacrificeChoice => "optional-creature-sacrifice-choice/v1",
            Self::CreaturePairingLifecycle => "creature-pairing-lifecycle/v1",
            Self::ControlChangeTracking => "control-change-tracking/v1",
            Self::DynamicPowerToughnessEvaluation => "dynamic-power-toughness-evaluation/v1",
            Self::EvolveEventTracking => "evolve-event-tracking/v1",
            Self::CreatureTapCostPayment => "creature-tap-cost-payment/v1",
            Self::ArtifactTapCostPayment => "artifact-tap-cost-payment/v1",
            Self::ModalSpellChoice => "modal-spell-choice/v1",
            Self::ModeAssociatedAdditionalCostBinding => {
                "mode-associated-additional-cost-binding/v1"
            }
            Self::PlayerLifeTotalMutation => "player-life-total-mutation/v1",
            Self::DelayedTriggeredAbilityLifecycle => "delayed-triggered-ability-lifecycle/v1",
            Self::SpellCopyLifecycle => "spell-copy-lifecycle/v1",
            Self::CastWithoutManaCost => "cast-without-mana-cost/v1",
            Self::TemporaryAbilityGrantLifecycle => "temporary-ability-grant-lifecycle/v1",
            Self::BattlefieldDesignationLifecycle => "battlefield-designation-lifecycle/v1",
        }
    }
}

const STATIC_CAST_TIMING_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CastTimingPermission,
];
const STATIC_ATTACK_LEGALITY_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatAttackLegality,
];
const STATIC_BLOCK_LEGALITY_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatBlockLegality,
];
const HASTE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatAttackLegality,
    LiveBridgeCapability::SummoningSicknessPermission,
    LiveBridgeCapability::TapAbilityActivationLegality,
];
const VIGILANCE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatAttackDeclaration,
];
const TRAMPLE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatDamageAssignment,
];
const DEATHTOUCH_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatDamageAssignment,
    LiveBridgeCapability::DamageEventModification,
];
const LIFELINK_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::DamageEventModification,
];
const COMBAT_DAMAGE_STEPS_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatDamageSteps,
];
const STATIC_TARGETING_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::TargetingLegality,
];
const INDESTRUCTIBLE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::DestructionPrevention,
];
const PROWESS_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::NoncreatureCastEventTracking,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::TemporaryPowerToughnessMutation,
];
const KICKER_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::AdditionalCostPayment,
    LiveBridgeCapability::LinkedAbilityState,
];
const INVESTIGATE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::TokenCreation,
    LiveBridgeCapability::TokenAbilityInstallation,
    LiveBridgeCapability::ActivatedCostPayment,
    LiveBridgeCapability::DrawResolution,
];
const FLASHBACK_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::AlternativeCastingLifecycle,
    LiveBridgeCapability::ZoneChangeReplacement,
];
const MORPH_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::AlternativeCastingLifecycle,
    LiveBridgeCapability::FaceDownCharacteristics,
    LiveBridgeCapability::SpecialActionExecution,
];
const CONVOKE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::SpellCostPayment,
    LiveBridgeCapability::CreatureTapCostPayment,
];
const CUMULATIVE_UPKEEP_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::ResourceCostPayment,
    LiveBridgeCapability::CounterLifecycle,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::PermanentSacrifice,
];
const WARD_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::TargetingLegality,
    LiveBridgeCapability::ResourceCostPayment,
    LiveBridgeCapability::TriggeredAbilityResolution,
];
const CYCLING_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::ResourceCostPayment,
    LiveBridgeCapability::ActivatedAbilityExecution,
    LiveBridgeCapability::LibraryProcedure,
];
const EQUIP_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::TargetingLegality,
    LiveBridgeCapability::ResourceCostPayment,
    LiveBridgeCapability::AttachmentLifecycle,
    LiveBridgeCapability::ActivatedAbilityExecution,
];
const PROTECTION_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatBlockLegality,
    LiveBridgeCapability::DamagePrevention,
    LiveBridgeCapability::TargetingLegality,
    LiveBridgeCapability::AttachmentLifecycle,
];
const ENCHANT_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::TargetingLegality,
    LiveBridgeCapability::AttachmentLifecycle,
];
const LIBRARY_ACTION_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] =
    &[LiveBridgeCapability::LibraryProcedure];
const FIGHT_ACTION_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::TargetingLegality,
    LiveBridgeCapability::DamageEventModification,
];
const REGENERATE_SOURCE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] =
    &[LiveBridgeCapability::RegenerationReplacementLifecycle];
const REGENERATE_TARGET_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::TargetingLegality,
    LiveBridgeCapability::RegenerationReplacementLifecycle,
];
const REGENERATE_CONTROLLED_CREATURE_SET_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::ControlledObjectSetResolution,
    LiveBridgeCapability::RegenerationReplacementLifecycle,
];
const REGENERATE_STATIC_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticEffectLifecycle,
    LiveBridgeCapability::RegenerationReplacementLifecycle,
];
const CHANGELING_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] =
    &[LiveBridgeCapability::StaticKeywordInstallation];
const INFECT_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::DamageEventModification,
    LiveBridgeCapability::CounterLifecycle,
];
const AFFINITY_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] =
    &[LiveBridgeCapability::SpellCostPayment];
const CASCADE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::AlternativeCastingLifecycle,
    LiveBridgeCapability::SpellCostPayment,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::LibraryProcedure,
];
const DELVE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::SpellCostPayment,
    LiveBridgeCapability::ResourceCostPayment,
];
const FUSE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::AlternativeCastingLifecycle,
    LiveBridgeCapability::SpellCostPayment,
];
const AFTERMATH_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::AlternativeCastingLifecycle,
    LiveBridgeCapability::SpellCostPayment,
    LiveBridgeCapability::ZoneChangeReplacement,
];
const REBOUND_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::AlternativeCastingLifecycle,
    LiveBridgeCapability::SpellCostPayment,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::ZoneChangeReplacement,
];
const EXALTED_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::CombatAttackDeclaration,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::TemporaryPowerToughnessMutation,
];
const BUSHIDO_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::CombatBlockTransition,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::TemporaryPowerToughnessMutation,
];
const WITHER_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::DamageEventModification,
    LiveBridgeCapability::CounterLifecycle,
];
const HORSEMANSHIP_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatBlockLegality,
];
const FLANKING_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::CombatBlockTransition,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::TemporaryPowerToughnessMutation,
];
const DEATH_RETURN_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CounterLifecycle,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::ZoneChangeReplacement,
    LiveBridgeCapability::ZoneChangeTriggerLifecycle,
    LiveBridgeCapability::PublicZoneObjectTracking,
    LiveBridgeCapability::TokenZoneLifecycle,
];
const TOXIC_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::CombatDamageSteps,
    LiveBridgeCapability::DamageTransaction,
    LiveBridgeCapability::CounterLifecycle,
    LiveBridgeCapability::PoisonCounterLifecycle,
];
const DAY_NIGHT_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::StaticEffectLifecycle,
    LiveBridgeCapability::GlobalDayNightLifecycle,
    LiveBridgeCapability::TurnSpellCountTracking,
    LiveBridgeCapability::DoubleFacedTransformLifecycle,
];
const START_YOUR_ENGINES_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::StaticEffectLifecycle,
    LiveBridgeCapability::PlayerSpeedLifecycle,
    LiveBridgeCapability::OpponentLifeLossEventTracking,
    LiveBridgeCapability::StateBasedActionLifecycle,
    LiveBridgeCapability::TriggeredAbilityResolution,
];
const COMMANDER_PARTNER_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::CommanderDeckConstructionLifecycle,
    LiveBridgeCapability::CommanderPairRulesLifecycle,
];
const EXPLOIT_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::ZoneChangeReplacement,
    LiveBridgeCapability::ZoneChangeTriggerLifecycle,
    LiveBridgeCapability::PermanentSacrifice,
    LiveBridgeCapability::ExploitEventTracking,
    LiveBridgeCapability::OptionalCreatureSacrificeChoice,
];
const SOULBOND_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::ZoneChangeTriggerLifecycle,
    LiveBridgeCapability::CreaturePairingLifecycle,
    LiveBridgeCapability::ControlChangeTracking,
];
const EVOLVE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::CounterLifecycle,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::ZoneChangeTriggerLifecycle,
    LiveBridgeCapability::DynamicPowerToughnessEvaluation,
    LiveBridgeCapability::EvolveEventTracking,
];
const IMPROVISE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticEffectLifecycle,
    LiveBridgeCapability::SpellCostPayment,
    LiveBridgeCapability::ResourceCostPayment,
    LiveBridgeCapability::ArtifactTapCostPayment,
];
const INTIMIDATE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::EffectiveColorCharacteristics,
    LiveBridgeCapability::CombatBlockLegality,
];
const SPREE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticEffectLifecycle,
    LiveBridgeCapability::TargetingLegality,
    LiveBridgeCapability::AdditionalCostPayment,
    LiveBridgeCapability::SpellCostPayment,
    LiveBridgeCapability::ModalSpellChoice,
    LiveBridgeCapability::ModeAssociatedAdditionalCostBinding,
];
const BARGAIN_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticEffectLifecycle,
    LiveBridgeCapability::TargetingLegality,
    LiveBridgeCapability::AdditionalCostPayment,
    LiveBridgeCapability::LinkedAbilityState,
    LiveBridgeCapability::SpellCostPayment,
    LiveBridgeCapability::PermanentSacrifice,
];
const MENTOR_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::CombatAttackDeclaration,
    LiveBridgeCapability::TargetingLegality,
    LiveBridgeCapability::CounterLifecycle,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::DynamicPowerToughnessEvaluation,
];
const EXTORT_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::OpponentLifeLossEventTracking,
    LiveBridgeCapability::ResourceCostPayment,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::PlayerLifeTotalMutation,
];
const LIVING_WEAPON_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::TokenCreation,
    LiveBridgeCapability::AttachmentLifecycle,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::ZoneChangeTriggerLifecycle,
];
const MYRIAD_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::CombatAttackDeclaration,
    LiveBridgeCapability::TokenCreation,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::TokenZoneLifecycle,
    LiveBridgeCapability::DelayedTriggeredAbilityLifecycle,
];
const RETRACE_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticEffectLifecycle,
    LiveBridgeCapability::AdditionalCostPayment,
    LiveBridgeCapability::AlternativeCastingLifecycle,
    LiveBridgeCapability::SpellCostPayment,
];
const BACKUP_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticKeywordInstallation,
    LiveBridgeCapability::TargetingLegality,
    LiveBridgeCapability::CounterLifecycle,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::ZoneChangeTriggerLifecycle,
    LiveBridgeCapability::TemporaryAbilityGrantLifecycle,
];
const UMBRA_ARMOR_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticEffectLifecycle,
    LiveBridgeCapability::DamagePrevention,
    LiveBridgeCapability::DestructionPrevention,
    LiveBridgeCapability::AttachmentLifecycle,
    LiveBridgeCapability::ZoneChangeReplacement,
];
const CIPHER_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::DamageTransaction,
    LiveBridgeCapability::LinkedAbilityState,
    LiveBridgeCapability::AlternativeCastingLifecycle,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::PublicZoneObjectTracking,
    LiveBridgeCapability::SpellCopyLifecycle,
    LiveBridgeCapability::CastWithoutManaCost,
];
const RENOWN_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::DamageTransaction,
    LiveBridgeCapability::LinkedAbilityState,
    LiveBridgeCapability::CounterLifecycle,
    LiveBridgeCapability::TriggeredAbilityResolution,
    LiveBridgeCapability::ZoneChangeTriggerLifecycle,
    LiveBridgeCapability::BattlefieldDesignationLifecycle,
];
const ASCEND_LIVE_BRIDGE_CAPABILITIES: &[LiveBridgeCapability] = &[
    LiveBridgeCapability::StaticEffectLifecycle,
    LiveBridgeCapability::StateBasedActionLifecycle,
    LiveBridgeCapability::LinkedAbilityState,
];

const ALLOWED_SINGLETON_KEYWORDS: &[OfficialKeyword] = &[
    OfficialKeyword::Protection,
    OfficialKeyword::Flying,
    OfficialKeyword::Investigate,
    OfficialKeyword::Kicker,
    OfficialKeyword::Flashback,
    OfficialKeyword::Morph,
    OfficialKeyword::Flash,
    OfficialKeyword::Menace,
    OfficialKeyword::Defender,
    OfficialKeyword::Reach,
    OfficialKeyword::Changeling,
    OfficialKeyword::Infect,
    OfficialKeyword::Fear,
    OfficialKeyword::Shadow,
    OfficialKeyword::Landwalk,
    OfficialKeyword::Affinity,
    OfficialKeyword::Cascade,
    OfficialKeyword::Delve,
    OfficialKeyword::Fuse,
    OfficialKeyword::Aftermath,
    OfficialKeyword::Rebound,
    OfficialKeyword::Exalted,
    OfficialKeyword::Bushido,
    OfficialKeyword::Wither,
    OfficialKeyword::Horsemanship,
    OfficialKeyword::Flanking,
    OfficialKeyword::Persist,
    OfficialKeyword::Undying,
    OfficialKeyword::Toxic,
    OfficialKeyword::Daybound,
    OfficialKeyword::Nightbound,
    OfficialKeyword::StartYourEngines,
    OfficialKeyword::ChooseABackground,
    OfficialKeyword::DoctorsCompanion,
    OfficialKeyword::Exploit,
    OfficialKeyword::Soulbond,
    OfficialKeyword::Evolve,
    OfficialKeyword::Improvise,
    OfficialKeyword::Intimidate,
    OfficialKeyword::Spree,
    OfficialKeyword::Bargain,
    OfficialKeyword::Mentor,
    OfficialKeyword::Extort,
    OfficialKeyword::LivingWeapon,
    OfficialKeyword::Myriad,
    OfficialKeyword::Retrace,
    OfficialKeyword::Backup,
    OfficialKeyword::UmbraArmor,
    OfficialKeyword::Cipher,
    OfficialKeyword::Renown,
    OfficialKeyword::Ascend,
    OfficialKeyword::Devoid,
    OfficialKeyword::Convoke,
    OfficialKeyword::Equip,
    OfficialKeyword::Enchant,
    OfficialKeyword::CumulativeUpkeep,
    OfficialKeyword::Haste,
    OfficialKeyword::Vigilance,
    OfficialKeyword::Trample,
    OfficialKeyword::Deathtouch,
    OfficialKeyword::Lifelink,
    OfficialKeyword::FirstStrike,
    OfficialKeyword::DoubleStrike,
    OfficialKeyword::Hexproof,
    OfficialKeyword::Shroud,
    OfficialKeyword::Indestructible,
    OfficialKeyword::Prowess,
    OfficialKeyword::Ward,
    OfficialKeyword::Cycling,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleClauseBackendInput<'a> {
    pub face_index: u16,
    pub clause_index: u16,
    pub source_name: &'a str,
    pub source_type_line: &'a str,
    pub oracle_clause: &'a str,
    /// Retained keyword metadata for the source card or face. Exact
    /// self-describing Oracle clauses are authoritative; this metadata can
    /// corroborate them but its absence, ordering, and unrelated entries do
    /// not change the compiled semantic identity.
    pub printed_keywords: &'a [&'a str],
}

/// Structural card context required by keyword families whose rules depend on
/// the relationship between a clause's source face and the physical card.
///
/// This contains no card identity or mutable metadata. The addressed face is
/// carried by [`OracleClauseBackendInput`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleClauseCardContext<'a> {
    pub layout: &'a str,
    pub face_count: usize,
}

/// Content-only semantic context for lifecycle keyword families that require
/// more than the addressed face's type line.
///
/// The lifecycle contexts contain exact or normalized Oracle face text and
/// rules-relevant face characteristics. They contain no card name, Oracle ID,
/// snapshot identity, row position, or clause address. Callers that cannot
/// provide complete context must use the narrower API, which remains fail
/// closed for context-dependent families.
#[derive(Debug, Clone, Copy)]
pub struct OracleClauseSemanticContext<'a> {
    pub card: OracleClauseCardContext<'a>,
    pub graveyard_transform: Option<&'a GraveyardTransformSourceSemanticContext>,
    pub level_progression: Option<&'a LevelProgressionProgram>,
    pub source_mana_value: Option<u32>,
    pub complete_face_oracle_text: Option<&'a str>,
}

impl OracleClauseBackendInput<'_> {
    fn bounded_input(&self) -> OracleClauseInput<'_> {
        OracleClauseInput {
            face_index: self.face_index,
            clause_index: self.clause_index,
            source_name: self.source_name,
            source_type_line: self.source_type_line,
            oracle_clause: self.oracle_clause,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleClauseBackendError {
    MalformedSyntax {
        syntax_error: OracleClauseSyntaxError,
    },
    Native {
        error: CompileError,
    },
    DelegatedKeyword {
        native_error: CompileError,
        keyword_error: KeywordCompileError,
    },
}

impl fmt::Display for OracleClauseBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedSyntax { syntax_error } => {
                write!(formatter, "lossless syntax parser: {syntax_error}")
            }
            Self::Native { error } => write!(formatter, "native bounded parser: {error}"),
            Self::DelegatedKeyword {
                native_error,
                keyword_error,
            } => write!(
                formatter,
                "native bounded parser: {native_error}; delegated keyword parser: {keyword_error}"
            ),
        }
    }
}

impl std::error::Error for OracleClauseBackendError {}

// Kept inline because this public clause contract is matched throughout production.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledOracleClause {
    Bounded(BoundedOracleClause),
    Delegated(DelegatedKeywordClause),
}

impl CompiledOracleClause {
    pub fn address(&self) -> ClauseAddress {
        match self {
            Self::Bounded(clause) => clause.address(),
            Self::Delegated(clause) => clause.address(),
        }
    }

    pub fn normalized_clause(&self) -> &str {
        match self {
            Self::Bounded(clause) => clause.normalized_clause(),
            Self::Delegated(clause) => clause.normalized_clause(),
        }
    }

    pub fn semantic_digest(&self) -> &str {
        match self {
            Self::Bounded(clause) => clause.semantic_digest(),
            Self::Delegated(clause) => clause.semantic_digest(),
        }
    }

    pub fn as_bounded(&self) -> Option<&BoundedOracleClause> {
        match self {
            Self::Bounded(clause) => Some(clause),
            Self::Delegated(_) => None,
        }
    }

    pub fn as_delegated(&self) -> Option<&DelegatedKeywordClause> {
        match self {
            Self::Bounded(_) => None,
            Self::Delegated(clause) => Some(clause),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleClauseRecognitionOutcome {
    ProgramRecognized(CompiledOracleClause),
    SyntaxOnly(SyntaxOnlyOracleClause),
}

impl OracleClauseRecognitionOutcome {
    pub fn as_compiled_program(&self) -> Option<&CompiledOracleClause> {
        match self {
            Self::ProgramRecognized(clause) => Some(clause),
            Self::SyntaxOnly(_) => None,
        }
    }

    pub fn as_syntax_only(&self) -> Option<&SyntaxOnlyOracleClause> {
        match self {
            Self::ProgramRecognized(_) => None,
            Self::SyntaxOnly(clause) => Some(clause),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelegatedKeywordFallbackDiagnostic {
    NoExactProgram,
    CompileError(KeywordCompileError),
}

impl fmt::Display for DelegatedKeywordFallbackDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoExactProgram => write!(formatter, "no exact delegated keyword program"),
            Self::CompileError(error) => write!(formatter, "{error}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleClauseRecognitionError {
    MalformedSyntax {
        syntax_error: OracleClauseSyntaxError,
    },
}

impl fmt::Display for OracleClauseRecognitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedSyntax { syntax_error } => {
                write!(formatter, "lossless syntax parser: {syntax_error}")
            }
        }
    }
}

impl std::error::Error for OracleClauseRecognitionError {}

/// A complete Oracle line whose syntax is retained losslessly after every
/// executable compiler path declined or rejected it.
///
/// This type intentionally exposes no executable contract, keyword program,
/// or live bridge capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxOnlyOracleClause {
    address: ClauseAddress,
    normalized_clause: String,
    syntax_digest: String,
    syntax: RecognizedOracleClauseSyntax,
    native_error: CompileError,
    delegated_diagnostic: DelegatedKeywordFallbackDiagnostic,
}

impl SyntaxOnlyOracleClause {
    pub fn address(&self) -> ClauseAddress {
        self.address
    }

    /// Existing card-aware compiler normalization retained for diagnostics
    /// and later program linking. It is not an input to the syntax digest.
    pub fn normalized_clause(&self) -> &str {
        &self.normalized_clause
    }

    pub fn syntax_digest(&self) -> &str {
        &self.syntax_digest
    }

    pub fn syntax(&self) -> &RecognizedOracleClauseSyntax {
        &self.syntax
    }

    pub fn native_error(&self) -> &CompileError {
        &self.native_error
    }

    pub fn delegated_diagnostic(&self) -> &DelegatedKeywordFallbackDiagnostic {
        &self.delegated_diagnostic
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegatedKeywordClause {
    runtime_version: &'static str,
    semantic_digest: String,
    address: ClauseAddress,
    normalized_clause: String,
    keyword_program: KeywordProgram,
    required_live_bridge_capabilities: &'static [LiveBridgeCapability],
}

impl DelegatedKeywordClause {
    pub fn runtime_version(&self) -> &'static str {
        self.runtime_version
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn address(&self) -> ClauseAddress {
        self.address
    }

    pub fn normalized_clause(&self) -> &str {
        &self.normalized_clause
    }

    pub fn keyword_program(&self) -> &KeywordProgram {
        &self.keyword_program
    }

    pub fn required_live_bridge_capabilities(&self) -> &'static [LiveBridgeCapability] {
        self.required_live_bridge_capabilities
    }
}

/// Compile one complete clause through the native bounded grammar first, then
/// the exact delegated keyword owner, exact residual standalone owners, the
/// complete typed ability compiler, and finally typed composition.
///
/// A delegated result is recognized source material only. The live consumer
/// must separately bind every capability returned by
/// [`DelegatedKeywordClause::required_live_bridge_capabilities`].
///
/// This API remains fail closed. Lossless syntax-only recognition is available
/// only through [`recognize_oracle_clause_backend`].
pub fn compile_oracle_clause_backend(
    input: OracleClauseBackendInput<'_>,
) -> Result<CompiledOracleClause, OracleClauseBackendError> {
    compile_oracle_clause_backend_with_optional_context(input, None, None, None, None, None)
}

/// Compile one complete clause with exact physical-card structure available
/// to contextual keyword families.
pub fn compile_oracle_clause_backend_with_context(
    input: OracleClauseBackendInput<'_>,
    card_context: OracleClauseCardContext<'_>,
) -> Result<CompiledOracleClause, OracleClauseBackendError> {
    compile_oracle_clause_backend_with_optional_context(
        input,
        Some(card_context),
        None,
        None,
        None,
        None,
    )
}

/// Compile one complete clause with all content-only semantic face context
/// needed by exact lifecycle keyword families.
pub fn compile_oracle_clause_backend_with_semantic_context(
    input: OracleClauseBackendInput<'_>,
    semantic_context: OracleClauseSemanticContext<'_>,
) -> Result<CompiledOracleClause, OracleClauseBackendError> {
    compile_oracle_clause_backend_with_optional_context(
        input,
        Some(semantic_context.card),
        semantic_context.graveyard_transform,
        semantic_context.level_progression,
        semantic_context.source_mana_value,
        semantic_context.complete_face_oracle_text,
    )
}

fn compile_oracle_clause_backend_with_optional_context(
    input: OracleClauseBackendInput<'_>,
    card_context: Option<OracleClauseCardContext<'_>>,
    graveyard_transform_context: Option<&GraveyardTransformSourceSemanticContext>,
    level_progression_context: Option<&LevelProgressionProgram>,
    source_mana_value: Option<u32>,
    complete_face_oracle_text: Option<&str>,
) -> Result<CompiledOracleClause, OracleClauseBackendError> {
    let validated = validate_oracle_clause_line(input.oracle_clause)
        .map_err(|syntax_error| OracleClauseBackendError::MalformedSyntax { syntax_error })?;
    compile_oracle_clause_program(
        input,
        validated,
        card_context,
        graveyard_transform_context,
        level_progression_context,
        source_mana_value,
        complete_face_oracle_text,
    )
}

fn compile_oracle_clause_program(
    input: OracleClauseBackendInput<'_>,
    validated: ValidatedOracleClauseLine<'_>,
    card_context: Option<OracleClauseCardContext<'_>>,
    graveyard_transform_context: Option<&GraveyardTransformSourceSemanticContext>,
    level_progression_context: Option<&LevelProgressionProgram>,
    source_mana_value: Option<u32>,
    complete_face_oracle_text: Option<&str>,
) -> Result<CompiledOracleClause, OracleClauseBackendError> {
    compile_oracle_clause_program_inner(
        input,
        validated,
        card_context,
        graveyard_transform_context,
        level_progression_context,
        source_mana_value,
        complete_face_oracle_text,
        true,
    )
}

// Kept explicit because each compiler input carries independent source evidence.
#[allow(clippy::too_many_arguments)]
fn compile_oracle_clause_program_inner(
    input: OracleClauseBackendInput<'_>,
    validated: ValidatedOracleClauseLine<'_>,
    card_context: Option<OracleClauseCardContext<'_>>,
    graveyard_transform_context: Option<&GraveyardTransformSourceSemanticContext>,
    level_progression_context: Option<&LevelProgressionProgram>,
    source_mana_value: Option<u32>,
    complete_face_oracle_text: Option<&str>,
    allow_composition: bool,
) -> Result<CompiledOracleClause, OracleClauseBackendError> {
    let validated_input = OracleClauseBackendInput {
        face_index: input.face_index,
        clause_index: input.clause_index,
        source_name: input.source_name,
        source_type_line: input.source_type_line,
        oracle_clause: validated.line(),
        printed_keywords: input.printed_keywords,
    };

    if let Some(program) = level_progression_context {
        let Some(context) = card_context else {
            return Err(OracleClauseBackendError::Native {
                error: level_progression_context_required_error(&validated_input),
            });
        };
        return retain_level_progression_program(
            validated_input.bounded_input(),
            context.layout,
            context.face_count,
            program.clone(),
        )
        .map(CompiledOracleClause::Bounded)
        .map_err(|error| OracleClauseBackendError::Native { error });
    }

    if card_context.is_some_and(|context| context.layout.eq_ignore_ascii_case("leveler"))
        || validated_input.oracle_clause.starts_with("Level up ")
        || validated_input.oracle_clause.starts_with("LEVEL ")
    {
        return Err(OracleClauseBackendError::Native {
            error: level_progression_context_required_error(&validated_input),
        });
    }

    if classify_linked_cast_cost_snapshot_candidate(
        validated_input.oracle_clause,
        validated_input.source_type_line,
    ) == Some(LinkedCastCostCandidateClass::SupportedResidual)
    {
        let Some(program) = compile_linked_cast_cost_keyword_program(
            validated_input.oracle_clause,
            validated_input.source_type_line,
        ) else {
            return Err(OracleClauseBackendError::Native {
                error: residual_program_context_error(&validated_input),
            });
        };
        return retain_linked_cast_cost_keyword_program(validated_input.bounded_input(), program)
            .map(CompiledOracleClause::Bounded)
            .map_err(|error| OracleClauseBackendError::Native { error });
    }

    let combat_trigger_normalized =
        reviewed_combat_trigger_normalized_source(validated_input.oracle_clause);
    if let CombatTriggerClauseClassification::Program(program) =
        classify_combat_trigger_keyword_clause(
            validated_input.oracle_clause,
            &combat_trigger_normalized,
        )
    {
        return retain_combat_trigger_keyword_program(validated_input.bounded_input(), program)
            .map(CompiledOracleClause::Bounded)
            .map_err(|error| OracleClauseBackendError::Native { error });
    }

    if classify_creature_counter_snapshot_candidate(
        validated_input.oracle_clause,
        validated_input.source_type_line,
    ) == Some(CreatureCounterCandidateClass::SupportedProgram)
    {
        let Some(program) = compile_creature_counter_keyword_program(
            validated_input.oracle_clause,
            validated_input.source_type_line,
        ) else {
            return Err(OracleClauseBackendError::Native {
                error: residual_program_context_error(&validated_input),
            });
        };
        return retain_creature_counter_keyword_program(validated_input.bounded_input(), program)
            .map(CompiledOracleClause::Bounded)
            .map_err(|error| OracleClauseBackendError::Native { error });
    }

    let zone_keyword_context = ZoneKeywordSourceSemanticContext {
        type_line: validated_input.source_type_line,
        mana_value: source_mana_value,
    };
    if classify_zone_keyword_snapshot_candidate(validated_input.oracle_clause, zone_keyword_context)
        == Some(ZoneKeywordCandidateClass::SupportedFamily)
    {
        let Some(program) =
            compile_zone_keyword_program(validated_input.oracle_clause, zone_keyword_context)
        else {
            return Err(OracleClauseBackendError::Native {
                error: residual_program_context_error(&validated_input),
            });
        };
        return retain_graveyard_hand_library_keyword_program(
            validated_input.bounded_input(),
            source_mana_value,
            program,
        )
        .map(CompiledOracleClause::Bounded)
        .map_err(|error| OracleClauseBackendError::Native { error });
    }

    if let (Some(card_context), Some(face_oracle_text)) = (card_context, complete_face_oracle_text)
        && let Some(source_context) = derive_cast_choice_source_context(
            validated_input.oracle_clause,
            validated_input.source_type_line,
            card_context.layout,
            validated_input.face_index,
            card_context.face_count,
            validated_input.clause_index,
            face_oracle_text,
        )
        && let CastChoiceClauseClassification::Program(program) =
            classify_cast_choice_keyword_clause(validated_input.oracle_clause, &source_context)
    {
        if compile_cast_choice_keyword_program(validated_input.oracle_clause, &source_context)
            .as_ref()
            != Some(&program)
        {
            return Err(OracleClauseBackendError::Native {
                error: residual_program_context_error(&validated_input),
            });
        }
        return retain_cast_choice_keyword_program(
            validated_input.bounded_input(),
            &source_context,
            program,
        )
        .map(CompiledOracleClause::Bounded)
        .map_err(|error| OracleClauseBackendError::Native { error });
    }

    let rejected_static_special_candidate =
        if is_static_special_keyword_candidate(validated_input.oracle_clause) {
            let source_context =
                StaticSpecialSourceContext::from_type_line(validated_input.source_type_line);
            let normalized_source =
                reviewed_static_special_normalized_source(validated_input.oracle_clause);
            match classify_static_special_keyword_clause(
                validated_input.oracle_clause,
                &normalized_source,
                source_context,
            ) {
                StaticSpecialClauseClassification::Program(program) => {
                    return retain_static_special_keyword_program(
                        validated_input.bounded_input(),
                        program,
                    )
                    .map(CompiledOracleClause::Bounded)
                    .map_err(|error| OracleClauseBackendError::Native { error });
                }
                StaticSpecialClauseClassification::Rejected => true,
            }
        } else {
            false
        };

    let rejected_common_action_candidate =
        if is_common_action_candidate(validated_input.oracle_clause) {
            let normalized_source =
                reviewed_common_action_normalized_source(validated_input.oracle_clause);
            match classify_common_action_clause(validated_input.oracle_clause, &normalized_source) {
                CommonActionClauseClassification::Program(program) => {
                    return retain_common_action_procedure_program(
                        validated_input.bounded_input(),
                        program,
                    )
                    .map(CompiledOracleClause::Bounded)
                    .map_err(|error| OracleClauseBackendError::Native { error });
                }
                CommonActionClauseClassification::EarlierOwner { .. } => false,
                CommonActionClauseClassification::Rejected => true,
            }
        } else {
            false
        };

    let rejected_regeneration_candidate =
        if contains_regeneration_lexeme(validated_input.oracle_clause) {
            let normalized_source =
                content_derived_whitespace_normalization(validated_input.oracle_clause);
            match classify_regeneration_action_clause(
                validated_input.oracle_clause,
                &normalized_source,
            ) {
                RegenerationClauseClassification::Program(program) => {
                    return retain_regeneration_action_program(
                        validated_input.bounded_input(),
                        program,
                    )
                    .map(CompiledOracleClause::Bounded)
                    .map_err(|error| OracleClauseBackendError::Native { error });
                }
                RegenerationClauseClassification::EarlierOwner { .. } => false,
                RegenerationClauseClassification::Rejected => true,
            }
        } else {
            false
        };

    let bounded = match card_context {
        Some(context) => compile_bounded_oracle_clause_after_syntax_validation_with_context(
            validated_input.bounded_input(),
            validated,
            BoundedOracleCardContext {
                layout: context.layout,
                face_count: context.face_count,
            },
        ),
        None => compile_bounded_oracle_clause_after_syntax_validation(
            validated_input.bounded_input(),
            validated,
        ),
    };
    match bounded {
        Ok(clause) => Ok(CompiledOracleClause::Bounded(clause)),
        Err(bounded_error) => {
            let delegated_error =
                match compile_delegated_keyword_clause_with_context(&validated_input, card_context)
                {
                    Ok(Some(clause)) => return Ok(CompiledOracleClause::Delegated(clause)),
                    Ok(None) => None,
                    Err(keyword_error) => Some(keyword_error),
                };

            match compile_residual_standalone_clause(&validated_input, card_context) {
                Ok(Some(clause)) => return Ok(CompiledOracleClause::Bounded(clause)),
                Ok(None) => {}
                Err(error) => return Err(OracleClauseBackendError::Native { error }),
            }

            match compile_residual_lifecycle_clause(
                &validated_input,
                card_context,
                graveyard_transform_context,
            ) {
                Ok(Some(clause)) => return Ok(CompiledOracleClause::Bounded(clause)),
                Ok(None) => {}
                Err(error) => return Err(OracleClauseBackendError::Native { error }),
            }

            if source_type_has_spell_resolution(validated_input.source_type_line) {
                let normalized_source =
                    reviewed_oracle_action_normalized_source(validated_input.oracle_clause);
                if let OracleActionClassification::Program(program) =
                    classify_oracle_action_instruction(OracleActionCompileInput {
                        exact_source: validated_input.oracle_clause,
                        normalized_source: &normalized_source,
                        semantic_context: OracleActionSemanticContext::ResolvingSpellInstruction,
                    })
                {
                    return retain_oracle_action_program(validated_input.bounded_input(), program)
                        .map(CompiledOracleClause::Bounded)
                        .map_err(|error| OracleClauseBackendError::Native { error });
                }
            }

            if rejected_static_special_candidate
                || rejected_common_action_candidate
                || rejected_regeneration_candidate
            {
                return match delegated_error {
                    Some(keyword_error) => Err(OracleClauseBackendError::DelegatedKeyword {
                        native_error: bounded_error,
                        keyword_error,
                    }),
                    None => Err(OracleClauseBackendError::Native {
                        error: bounded_error,
                    }),
                };
            }

            if delegated_error.is_none()
                && let Some(program) = compile_ability_clause_bridge(
                    validated_input.oracle_clause,
                    validated_input.source_name,
                    validated_input.source_type_line,
                )
                && let Ok(clause) =
                    retain_ability_clause_bridge_program(validated_input.bounded_input(), program)
            {
                return Ok(CompiledOracleClause::Bounded(clause));
            }

            if allow_composition
                && let Some(clause) = compile_typed_oracle_composition(
                    &validated_input,
                    card_context,
                    graveyard_transform_context,
                    level_progression_context,
                    source_mana_value,
                    complete_face_oracle_text,
                )
            {
                return Ok(CompiledOracleClause::Bounded(clause));
            }

            if allow_composition
                && let Some(face_oracle_text) = complete_face_oracle_text
                && let Some(clause) = compile_oracle_face_modal_line(
                    &validated_input,
                    card_context,
                    graveyard_transform_context,
                    level_progression_context,
                    source_mana_value,
                    face_oracle_text,
                )
            {
                return Ok(CompiledOracleClause::Bounded(clause));
            }

            match delegated_error {
                Some(keyword_error) => Err(OracleClauseBackendError::DelegatedKeyword {
                    native_error: bounded_error,
                    keyword_error,
                }),
                None => Err(OracleClauseBackendError::Native {
                    error: bounded_error,
                }),
            }
        }
    }
}

fn source_type_has_spell_resolution(source_type_line: &str) -> bool {
    source_type_line
        .split(|character: char| !character.is_alphanumeric())
        .any(|word| word.eq_ignore_ascii_case("instant") || word.eq_ignore_ascii_case("sorcery"))
}

struct BackendModalChildCompiler<'a> {
    face_index: u16,
    source_name: &'a str,
    source_type_line: &'a str,
    printed_keywords: &'a [&'a str],
    card_context: Option<OracleClauseCardContext<'a>>,
    graveyard_transform_context: Option<&'a GraveyardTransformSourceSemanticContext>,
    level_progression_context: Option<&'a LevelProgressionProgram>,
    source_mana_value: Option<u32>,
    complete_face_oracle_text: &'a str,
}

impl ClosedModalChildCompiler for BackendModalChildCompiler<'_> {
    type Program = OracleCompositionChildProgram;

    fn compile_closed_child(
        &mut self,
        source: ModalChildSource<'_>,
    ) -> ModalChildCompilation<Self::Program> {
        if source.complete_face_source != self.complete_face_oracle_text {
            return ModalChildCompilation::Incomplete {
                detail: "modal child does not belong to the exact face source".to_owned(),
            };
        }
        let Some(clause_index) =
            exact_face_clause_index(self.complete_face_oracle_text, source.source_span.start)
        else {
            return ModalChildCompilation::Incomplete {
                detail: "modal child has no exact face clause address".to_owned(),
            };
        };
        let Ok(validated) = validate_oracle_clause_line(source.exact_source) else {
            return ModalChildCompilation::Unsupported {
                detail: "modal child failed complete Oracle syntax validation".to_owned(),
            };
        };
        let input = OracleClauseBackendInput {
            face_index: self.face_index,
            clause_index,
            source_name: self.source_name,
            source_type_line: self.source_type_line,
            oracle_clause: source.exact_source,
            printed_keywords: self.printed_keywords,
        };
        let compiled = compile_oracle_clause_program_inner(
            input,
            validated,
            self.card_context,
            self.graveyard_transform_context,
            self.level_progression_context,
            self.source_mana_value,
            Some(self.complete_face_oracle_text),
            false,
        );
        let (program, semantic_digest) = match compiled {
            Ok(CompiledOracleClause::Bounded(program)) => {
                let semantic_digest = program.semantic_digest().to_owned();
                (
                    OracleCompositionChildProgram::bounded(program),
                    semantic_digest,
                )
            }
            Ok(CompiledOracleClause::Delegated(program)) => {
                let semantic_digest = program.semantic_digest().to_owned();
                (
                    OracleCompositionChildProgram::delegated_keyword(
                        source.exact_source.to_owned(),
                        semantic_digest.clone(),
                        program.normalized_clause().to_owned(),
                        program.keyword_program().clone(),
                    ),
                    semantic_digest,
                )
            }
            Err(error) => {
                return ModalChildCompilation::Unsupported {
                    detail: format!("modal child has no exact typed program: {error}"),
                };
            }
        };
        ModalChildCompilation::Closed(ClosedModalChildProgram {
            program,
            exact_source: source.exact_source.to_owned(),
            source_span: source.source_span,
            semantic_digest,
            complete: true,
        })
    }
}

fn compile_oracle_face_modal_line(
    input: &OracleClauseBackendInput<'_>,
    card_context: Option<OracleClauseCardContext<'_>>,
    graveyard_transform_context: Option<&GraveyardTransformSourceSemanticContext>,
    level_progression_context: Option<&LevelProgressionProgram>,
    source_mana_value: Option<u32>,
    complete_face_oracle_text: &str,
) -> Option<BoundedOracleClause> {
    let (line_start, line_end, exact_line) =
        exact_face_clause_span(complete_face_oracle_text, input.clause_index)?;
    if exact_line != input.oracle_clause {
        return None;
    }
    let mut child_compiler = BackendModalChildCompiler {
        face_index: input.face_index,
        source_name: input.source_name,
        source_type_line: input.source_type_line,
        printed_keywords: input.printed_keywords,
        card_context,
        graveyard_transform_context,
        level_progression_context,
        source_mana_value,
        complete_face_oracle_text,
    };
    let group = assemble_oracle_face_modal_program_containing_offset(
        OracleFaceProgramInput {
            exact_oracle_text: complete_face_oracle_text,
            provenance: OracleFaceProvenance {
                source_name: Some(input.source_name),
                face_index: Some(input.face_index),
                ..OracleFaceProvenance::default()
            },
        },
        line_start,
        &mut child_compiler,
    )
    .ok()??;
    let role = if group.header().source_span.start == line_start
        && group.header().source_span.end == line_end
    {
        OracleFaceModalLineRole::Header
    } else {
        let branch_index = group
            .branches()
            .iter()
            .position(|branch| {
                branch.marker_span.start == line_start && branch.body_span.end == line_end
            })
            .and_then(|index| u16::try_from(index).ok())?;
        OracleFaceModalLineRole::Branch { branch_index }
    };
    let program = OracleFaceModalLineProgram::compile(exact_line, role, group)?;
    retain_oracle_face_modal_line_program(input.bounded_input(), program).ok()
}

fn exact_face_clause_index(exact_face_oracle_text: &str, source_offset: usize) -> Option<u16> {
    exact_face_clause_spans(exact_face_oracle_text)
        .into_iter()
        .position(|(start, end, _)| start <= source_offset && source_offset < end)
        .and_then(|index| u16::try_from(index).ok())
}

fn exact_face_clause_span(
    exact_face_oracle_text: &str,
    clause_index: u16,
) -> Option<(usize, usize, &str)> {
    exact_face_clause_spans(exact_face_oracle_text)
        .into_iter()
        .nth(usize::from(clause_index))
}

fn exact_face_clause_spans(exact_face_oracle_text: &str) -> Vec<(usize, usize, &str)> {
    let mut spans = Vec::new();
    let mut offset = 0usize;
    for physical_line in exact_face_oracle_text.split_inclusive('\n') {
        let line_without_newline = physical_line
            .strip_suffix('\n')
            .unwrap_or(physical_line)
            .strip_suffix('\r')
            .unwrap_or_else(|| physical_line.strip_suffix('\n').unwrap_or(physical_line));
        let trimmed = line_without_newline.trim();
        if !trimmed.is_empty() && trimmed != "//" {
            let leading = line_without_newline.len() - line_without_newline.trim_start().len();
            let start = offset + leading;
            let end = start + trimmed.len();
            spans.push((start, end, &exact_face_oracle_text[start..end]));
        }
        offset += physical_line.len();
    }
    if !exact_face_oracle_text.ends_with('\n') && exact_face_oracle_text.is_empty() {
        return spans;
    }
    spans
}

fn compile_residual_standalone_clause(
    input: &OracleClauseBackendInput<'_>,
    card_context: Option<OracleClauseCardContext<'_>>,
) -> Result<Option<BoundedOracleClause>, CompileError> {
    if classify_alternate_zone_candidate(input.oracle_clause, input.source_type_line)
        == Some(AlternateZoneCandidateClass::SupportedFamily)
    {
        let Some(program) = compile_alternate_zone_cast_keyword_program(
            input.oracle_clause,
            input.source_type_line,
        ) else {
            return Ok(None);
        };
        return retain_alternate_zone_cast_keyword_program(input.bounded_input(), program)
            .map(Some);
    }

    if classify_cast_modifier_candidate(input.oracle_clause)
        == Some(CastModifierCandidateClass::SupportedResidual)
    {
        let Some(program) = compile_cast_modifier_keyword_program(input.oracle_clause) else {
            return Ok(None);
        };
        return retain_cast_modifier_keyword_program(input.bounded_input(), program).map(Some);
    }

    let source_layout = card_context.map_or("", |context| context.layout);
    let attachment_input = AttachmentFilterCompilerInput {
        exact_oracle_clause: input.oracle_clause,
        source_type_line: input.source_type_line,
        source_layout,
    };
    if prior_attachment_owner(attachment_input).is_none()
        && let Some(program) = compile_attachment_filter_program(attachment_input)
    {
        return retain_attachment_filter_program(input.bounded_input(), source_layout, program)
            .map(Some);
    }

    let normalized_clause = normalize_oracle_clause(
        input.oracle_clause,
        input.source_name,
        input.source_type_line,
    );
    if let Some(program) =
        compile_residual_cost_keyword_program(input.oracle_clause, &normalized_clause)
    {
        return retain_residual_cost_keyword_program(input.bounded_input(), program).map(Some);
    }

    if let DelayedCounterClauseClassification::Program(program) =
        classify_delayed_counter_keyword_clause(input.oracle_clause, &normalized_clause)
    {
        return retain_delayed_counter_keyword_program(input.bounded_input(), program).map(Some);
    }

    if classify_extended_cast_zone_snapshot_candidate(input.oracle_clause)
        == Some(ExtendedCastZoneCandidateClass::SupportedFamily)
    {
        let Some(program) = compile_extended_cast_zone_keyword_program(input.oracle_clause) else {
            return Ok(None);
        };
        return retain_extended_cast_zone_keyword_program(input.bounded_input(), program).map(Some);
    }

    Ok(None)
}

fn level_progression_context_required_error(input: &OracleClauseBackendInput<'_>) -> CompileError {
    residual_program_context_error(input)
}

fn residual_program_context_error(input: &OracleClauseBackendInput<'_>) -> CompileError {
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

fn is_static_special_keyword_candidate(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    [
        "job select",
        "for mirrodin",
        "living metal",
        "banding",
        "phasing",
        "training",
        "hidden agenda",
        "double agenda",
        "double team",
    ]
    .into_iter()
    .any(|keyword| contains_ascii_word(&lower, keyword))
        || lower.contains("enlist")
        || lower.contains("draft this card face up")
}

fn contains_ascii_word(source: &str, keyword: &str) -> bool {
    source.match_indices(keyword).any(|(start, _)| {
        let before = source[..start].chars().next_back();
        let after = source[start + keyword.len()..].chars().next();
        before.is_none_or(|character| !character.is_ascii_alphanumeric())
            && after.is_none_or(|character| !character.is_ascii_alphanumeric())
    })
}

fn is_common_action_candidate(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    [
        "energy counter",
        "{e}",
        "take the initiative",
        "explore",
        "learn",
        "investigate",
        "support ",
        "the ring tempts you",
        "venture into the dungeon",
        "become the monarch",
        "becomes the monarch",
        "proliferate",
        "amass ",
    ]
    .into_iter()
    .any(|candidate| lower.contains(candidate))
}

fn content_derived_whitespace_normalization(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compile_residual_lifecycle_clause(
    input: &OracleClauseBackendInput<'_>,
    card_context: Option<OracleClauseCardContext<'_>>,
    graveyard_transform_context: Option<&GraveyardTransformSourceSemanticContext>,
) -> Result<Option<BoundedOracleClause>, CompileError> {
    if let Some(card_context) = card_context
        && classify_face_down_merge_candidate(
            input.oracle_clause,
            input.source_type_line,
            card_context.layout,
        ) == Some(FaceDownMergeCandidateClass::SupportedResidual)
    {
        let Some(program) = compile_face_down_merge_keyword_program(
            input.oracle_clause,
            input.source_type_line,
            card_context.layout,
        ) else {
            return Ok(None);
        };
        return retain_face_down_merge_keyword_program(
            input.bounded_input(),
            card_context.layout,
            program,
        )
        .map(Some);
    }

    let combat_normalized = reviewed_combat_special_normalized_source(input.oracle_clause);
    if let CombatSpecialClauseClassification::Program(program) =
        classify_combat_special_keyword_clause(input.oracle_clause, &combat_normalized)
    {
        return retain_combat_special_keyword_program(input.bounded_input(), program).map(Some);
    }

    if let (Some(card_context), Some(source_context)) = (card_context, graveyard_transform_context)
        && graveyard_transform_context_matches_input(input, card_context, source_context)
        && classify_graveyard_transform_candidate(input.oracle_clause, source_context)
            == Some(GraveyardTransformCandidateClass::SupportedFamily)
    {
        let Some(program) =
            compile_graveyard_transform_keyword_program(input.oracle_clause, source_context)
        else {
            return Ok(None);
        };
        return retain_graveyard_transform_keyword_program(
            input.bounded_input(),
            source_context,
            program,
        )
        .map(Some);
    }

    Ok(None)
}

fn graveyard_transform_context_matches_input(
    input: &OracleClauseBackendInput<'_>,
    card_context: OracleClauseCardContext<'_>,
    source_context: &GraveyardTransformSourceSemanticContext,
) -> bool {
    let source_line_is_present = |oracle_text: &str| {
        oracle_text
            .lines()
            .map(str::trim)
            .any(|line| line == input.oracle_clause)
    };
    match source_context {
        GraveyardTransformSourceSemanticContext::SingleFace {
            layout,
            type_line,
            normalized_oracle_text,
        } => {
            *layout == GraveyardTransformCardLayout::Normal
                && card_context.layout == "normal"
                && card_context.face_count == 1
                && input.face_index == 0
                && type_line == input.source_type_line
                && source_line_is_present(normalized_oracle_text)
        }
        GraveyardTransformSourceSemanticContext::Transform(context) => {
            context.layout == GraveyardTransformCardLayout::Transform
                && context.keyword_face == GraveyardTransformFaceId::Front
                && card_context.layout == "transform"
                && card_context.face_count == 2
                && input.face_index == 0
                && context.front.type_line == input.source_type_line
                && source_line_is_present(&context.front.normalized_oracle_text)
        }
    }
}

#[derive(Debug, Clone)]
struct BackendCompositionChild {
    span: SourceSpan,
    capabilities: Vec<SemanticCapability>,
    timing: CompositionTimingClass,
    compiled: CompiledOracleClause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositionTimingClass {
    Casting,
    Resolution,
    Activated,
    Triggered,
    Static,
    Replacement,
    Modal,
    SpecialAction,
    DeckConstruction,
    Unknown,
}

impl TypedOracleChildProgram for BackendCompositionChild {
    fn exact_source(&self) -> &str {
        match &self.compiled {
            CompiledOracleClause::Bounded(clause) => clause.source_clause(),
            CompiledOracleClause::Delegated(clause) => clause
                .keyword_program()
                .source()
                .oracle_fragment
                .as_deref()
                .unwrap_or_default(),
        }
    }

    fn semantic_digest(&self) -> &str {
        self.compiled.semantic_digest()
    }

    fn capabilities(&self) -> &[SemanticCapability] {
        &self.capabilities
    }
}

fn compile_typed_oracle_composition(
    input: &OracleClauseBackendInput<'_>,
    card_context: Option<OracleClauseCardContext<'_>>,
    graveyard_transform_context: Option<&GraveyardTransformSourceSemanticContext>,
    level_progression_context: Option<&LevelProgressionProgram>,
    source_mana_value: Option<u32>,
    complete_face_oracle_text: Option<&str>,
) -> Option<BoundedOracleClause> {
    let composition = parse_oracle_clause_composition(OracleClauseCompositionInput::card_face(
        input.oracle_clause,
    ))
    .ok()?;
    if !composition.exclusions().is_empty()
        || matches!(composition.root(), OracleCompositionNode::Atom(_))
    {
        return None;
    }

    let mut spans = composition
        .requirements()
        .iter()
        .map(|requirement| requirement.span)
        .collect::<Vec<_>>();
    spans.sort();
    spans.dedup();

    let mut children = Vec::with_capacity(spans.len());
    for span in spans {
        let exact_source = span.slice(input.oracle_clause)?;
        let validated = validate_oracle_clause_line(exact_source).ok()?;
        let child_input = OracleClauseBackendInput {
            face_index: input.face_index,
            clause_index: input.clause_index,
            source_name: input.source_name,
            source_type_line: input.source_type_line,
            oracle_clause: exact_source,
            printed_keywords: input.printed_keywords,
        };
        let compiled = compile_oracle_clause_program_inner(
            child_input,
            validated,
            card_context,
            graveyard_transform_context,
            level_progression_context,
            source_mana_value,
            complete_face_oracle_text,
            false,
        )
        .ok()?;
        let requested = composition
            .requirements()
            .iter()
            .filter_map(|requirement| (requirement.span == span).then_some(requirement.capability))
            .collect::<Vec<_>>();
        let capabilities = proven_composition_capabilities(&compiled, &requested);
        if capabilities.is_empty() {
            return None;
        }
        let timing = composition_timing_class(&compiled);
        children.push(BackendCompositionChild {
            span,
            capabilities,
            timing,
            compiled,
        });
    }
    composition_node_timing_class(composition.root(), &children)?;

    let bindings = children
        .iter()
        .map(|child| TypedChildBinding {
            span: child.span,
            program: child,
        })
        .collect::<Vec<_>>();
    let typed = composition.compose_typed_children(&bindings).ok()?;
    let concrete_children = children
        .into_iter()
        .map(|child| match child.compiled {
            CompiledOracleClause::Bounded(program) => {
                OracleCompositionChildBinding::bounded(child.span, child.capabilities, program)
            }
            CompiledOracleClause::Delegated(program) => {
                OracleCompositionChildBinding::delegated_keyword(
                    child.span,
                    child.capabilities,
                    program
                        .keyword_program()
                        .source()
                        .oracle_fragment
                        .as_deref()
                        .unwrap_or_default()
                        .to_owned(),
                    program.semantic_digest().to_owned(),
                    program.normalized_clause().to_owned(),
                    program.keyword_program().clone(),
                )
            }
        })
        .collect();
    retain_oracle_clause_composition_program(input.bounded_input(), typed, concrete_children).ok()
}

fn composition_timing_class(compiled: &CompiledOracleClause) -> CompositionTimingClass {
    match compiled {
        CompiledOracleClause::Bounded(clause) => match clause.timing() {
            Timing::CastingAdditionalCost => CompositionTimingClass::Casting,
            Timing::SpellResolution => CompositionTimingClass::Resolution,
            Timing::Activated => CompositionTimingClass::Activated,
            Timing::Triggered(_) | Timing::TriggeredModalHeader { .. } => {
                CompositionTimingClass::Triggered
            }
            Timing::Static => CompositionTimingClass::Static,
            Timing::Replacement => CompositionTimingClass::Replacement,
            Timing::ModalHeader { .. } | Timing::ModalBranch { .. } => {
                CompositionTimingClass::Modal
            }
            Timing::SpecialAction(_) => CompositionTimingClass::SpecialAction,
            Timing::TypedStandaloneProgram => match clause.effects() {
                [Effect::StandaloneRuleProgram(StandaloneRuleProgram::AbilityClause(program))] => {
                    match program.timing() {
                        AbilityClauseTimingEnvelope::DeckConstruction => {
                            CompositionTimingClass::DeckConstruction
                        }
                        AbilityClauseTimingEnvelope::SpellResolution => {
                            CompositionTimingClass::Resolution
                        }
                        AbilityClauseTimingEnvelope::AuraSpellTargeting => {
                            CompositionTimingClass::Casting
                        }
                        AbilityClauseTimingEnvelope::Activated { .. } => {
                            CompositionTimingClass::Activated
                        }
                        AbilityClauseTimingEnvelope::Triggered { .. } => {
                            CompositionTimingClass::Triggered
                        }
                        AbilityClauseTimingEnvelope::StaticModifier => {
                            CompositionTimingClass::Static
                        }
                    }
                }
                _ => CompositionTimingClass::Unknown,
            },
        },
        CompiledOracleClause::Delegated(clause) => {
            let contract = clause.required_live_bridge_capabilities();
            if contract.contains(&LiveBridgeCapability::StaticKeywordInstallation)
                || contract.contains(&LiveBridgeCapability::StaticEffectLifecycle)
            {
                CompositionTimingClass::Static
            } else if contract.contains(&LiveBridgeCapability::CommanderDeckConstructionLifecycle)
                || contract.contains(&LiveBridgeCapability::CommanderPairRulesLifecycle)
            {
                CompositionTimingClass::DeckConstruction
            } else if contract.contains(&LiveBridgeCapability::SpecialActionExecution) {
                CompositionTimingClass::SpecialAction
            } else if contract.contains(&LiveBridgeCapability::ActivatedAbilityExecution) {
                CompositionTimingClass::Activated
            } else if contract.contains(&LiveBridgeCapability::TriggeredAbilityResolution) {
                CompositionTimingClass::Triggered
            } else if contract.contains(&LiveBridgeCapability::ZoneChangeReplacement)
                || contract.contains(&LiveBridgeCapability::RegenerationReplacementLifecycle)
            {
                CompositionTimingClass::Replacement
            } else {
                CompositionTimingClass::Resolution
            }
        }
    }
}

fn merge_composition_timing_classes(
    timings: impl IntoIterator<Item = CompositionTimingClass>,
) -> Option<CompositionTimingClass> {
    let mut timings = timings.into_iter();
    let first = timings.next()?;
    if first == CompositionTimingClass::Unknown {
        return None;
    }
    let compatible = match first {
        CompositionTimingClass::Triggered
        | CompositionTimingClass::Activated
        | CompositionTimingClass::Replacement
        | CompositionTimingClass::Casting
        | CompositionTimingClass::Modal => {
            timings.all(|timing| timing == first || timing == CompositionTimingClass::Resolution)
        }
        CompositionTimingClass::Resolution
        | CompositionTimingClass::Static
        | CompositionTimingClass::SpecialAction
        | CompositionTimingClass::DeckConstruction => timings.all(|timing| timing == first),
        CompositionTimingClass::Unknown => false,
    };
    compatible.then_some(first)
}

fn composition_node_timing_class(
    node: &OracleCompositionNode,
    children: &[BackendCompositionChild],
) -> Option<CompositionTimingClass> {
    match node {
        OracleCompositionNode::Atom(atom) => children
            .iter()
            .find_map(|child| (child.span == atom.span).then_some(child.timing)),
        OracleCompositionNode::Sequence { parts, .. }
        | OracleCompositionNode::Conjunction { parts, .. }
        | OracleCompositionNode::Alternative { parts, .. } => merge_composition_timing_classes(
            parts
                .iter()
                .map(|part| composition_node_timing_class(part, children))
                .collect::<Option<Vec<_>>>()?,
        ),
        OracleCompositionNode::Conditional {
            condition,
            consequence,
            otherwise_body,
            ..
        } => {
            composition_node_timing_class(condition, children)?;
            let mut branch_timings = vec![composition_node_timing_class(consequence, children)?];
            if let Some(otherwise_body) = otherwise_body {
                branch_timings.push(composition_node_timing_class(otherwise_body, children)?);
            }
            merge_composition_timing_classes(branch_timings)
        }
        OracleCompositionNode::OptionalChoice { body, .. } => {
            composition_node_timing_class(body, children)
        }
        OracleCompositionNode::ActivatedAbility {
            cost, instruction, ..
        } => {
            composition_node_timing_class(&OracleCompositionNode::Atom(cost.clone()), children)?;
            let instruction_timing = composition_node_timing_class(instruction, children)?;
            (instruction_timing == CompositionTimingClass::Resolution)
                .then_some(CompositionTimingClass::Activated)
        }
        OracleCompositionNode::DelayedInstruction {
            schedule,
            instruction,
            ..
        } => {
            let schedule_timing = composition_node_timing_class(
                &OracleCompositionNode::Atom(schedule.clone()),
                children,
            )?;
            let instruction_timing = composition_node_timing_class(instruction, children)?;
            (matches!(
                schedule_timing,
                CompositionTimingClass::Triggered | CompositionTimingClass::Resolution
            ) && instruction_timing == CompositionTimingClass::Resolution)
                .then_some(CompositionTimingClass::Triggered)
        }
        OracleCompositionNode::ModalGroup {
            header_span,
            branches,
            ..
        } => {
            let header_timing = children
                .iter()
                .find_map(|child| (child.span == *header_span).then_some(child.timing))?;
            if !matches!(
                header_timing,
                CompositionTimingClass::Modal | CompositionTimingClass::Resolution
            ) {
                return None;
            }
            branches
                .iter()
                .map(|branch| composition_node_timing_class(&branch.body, children))
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .all(|timing| timing == CompositionTimingClass::Resolution)
                .then_some(CompositionTimingClass::Modal)
        }
        OracleCompositionNode::DetachedModalBranch { .. } => None,
        OracleCompositionNode::EmbeddedAbilities {
            outer, abilities, ..
        } => {
            let outer_timing = composition_node_timing_class(outer, children)?;
            for ability in abilities {
                composition_node_timing_class(&ability.body, children)?;
            }
            Some(outer_timing)
        }
    }
}

fn proven_composition_capabilities(
    compiled: &CompiledOracleClause,
    requested: &[SemanticCapability],
) -> Vec<SemanticCapability> {
    let mut proven = match compiled {
        CompiledOracleClause::Bounded(clause) => {
            proven_bounded_composition_capabilities(clause, requested)
        }
        CompiledOracleClause::Delegated(clause) => {
            proven_delegated_composition_capabilities(clause)
        }
    };
    proven.retain(|capability| requested.contains(capability));
    proven.sort();
    proven.dedup();
    proven
}

fn proven_bounded_composition_capabilities(
    clause: &BoundedOracleClause,
    requested: &[SemanticCapability],
) -> Vec<SemanticCapability> {
    let mut proven = Vec::new();
    let effects = clause.effects();
    let has_standalone = effects_have_standalone_program(effects);
    let standalone_semantic_atom = matches!(
        effects,
        [Effect::StandaloneRuleProgram(StandaloneRuleProgram::AbilityClause(program))]
            if !program.ability().effects.is_empty()
    );
    if (!effects.is_empty() && !has_standalone) || standalone_semantic_atom {
        proven.push(SemanticCapability::SemanticAtom);
    }
    if !clause.targets().is_empty() {
        proven.push(SemanticCapability::TargetSelection);
    }
    if !clause.costs().is_empty() {
        proven.push(SemanticCapability::CostPayment);
    }
    if !matches!(
        clause.timing(),
        Timing::Static | Timing::TypedStandaloneProgram
    ) {
        proven.push(SemanticCapability::TimingWindow);
    }
    if !clause.conditions().is_empty() {
        proven.push(SemanticCapability::Condition);
    }
    if matches!(clause.timing(), Timing::Replacement) || effects.iter().any(effect_has_replacement)
    {
        proven.push(SemanticCapability::ReplacementEffect);
    }
    if effects.iter().any(effect_has_delayed_trigger) {
        proven.push(SemanticCapability::DelayedTrigger);
    }
    if effects.iter().any(effect_has_optional_choice) {
        proven.push(SemanticCapability::OptionalChoice);
    }
    if matches!(
        clause.timing(),
        Timing::ModalHeader { .. } | Timing::TriggeredModalHeader { .. }
    ) && effects
        .iter()
        .any(|effect| matches!(effect, Effect::ChooseMode { .. }))
    {
        proven.push(SemanticCapability::ModalSelection);
    }
    if requested.contains(&SemanticCapability::NestedGrantedAbility)
        && effects.iter().any(effect_has_complete_granted_ability)
    {
        proven.push(SemanticCapability::NestedGrantedAbility);
    }
    proven
}

fn proven_delegated_composition_capabilities(
    clause: &DelegatedKeywordClause,
) -> Vec<SemanticCapability> {
    let contract = clause.required_live_bridge_capabilities();
    let has = |capability| contract.contains(&capability);
    let mut proven = vec![SemanticCapability::SemanticAtom];
    if has(LiveBridgeCapability::TargetingLegality) {
        proven.push(SemanticCapability::TargetSelection);
    }
    if [
        LiveBridgeCapability::AdditionalCostPayment,
        LiveBridgeCapability::SpellCostPayment,
        LiveBridgeCapability::ResourceCostPayment,
        LiveBridgeCapability::ActivatedCostPayment,
        LiveBridgeCapability::CreatureTapCostPayment,
        LiveBridgeCapability::ArtifactTapCostPayment,
    ]
    .into_iter()
    .any(has)
    {
        proven.push(SemanticCapability::CostPayment);
    }
    if [
        LiveBridgeCapability::SpecialActionExecution,
        LiveBridgeCapability::ActivatedAbilityExecution,
        LiveBridgeCapability::TriggeredAbilityResolution,
    ]
    .into_iter()
    .any(has)
    {
        proven.push(SemanticCapability::TimingWindow);
    }
    if [
        LiveBridgeCapability::ZoneChangeReplacement,
        LiveBridgeCapability::RegenerationReplacementLifecycle,
    ]
    .into_iter()
    .any(has)
    {
        proven.push(SemanticCapability::ReplacementEffect);
    }
    if has(LiveBridgeCapability::DelayedTriggeredAbilityLifecycle) {
        proven.push(SemanticCapability::DelayedTrigger);
    }
    if has(LiveBridgeCapability::OptionalCreatureSacrificeChoice) {
        proven.push(SemanticCapability::OptionalChoice);
    }
    if has(LiveBridgeCapability::ModalSpellChoice)
        && has(LiveBridgeCapability::ModeAssociatedAdditionalCostBinding)
    {
        proven.push(SemanticCapability::ModalSelection);
    }
    if has(LiveBridgeCapability::TemporaryAbilityGrantLifecycle)
        || has(LiveBridgeCapability::TokenAbilityInstallation)
    {
        proven.push(SemanticCapability::NestedGrantedAbility);
    }
    proven
}

fn effects_have_standalone_program(effects: &[Effect]) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::StandaloneRuleProgram(_) => true,
        Effect::Optional(nested) => effects_have_standalone_program(nested),
        Effect::Conditional {
            if_true, if_false, ..
        } => effects_have_standalone_program(if_true) || effects_have_standalone_program(if_false),
        Effect::GrantAbility { ability, .. } => effects_have_standalone_program(&ability.effects),
        _ => false,
    })
}

fn effect_has_replacement(effect: &Effect) -> bool {
    match effect {
        Effect::Replacement(_) => true,
        Effect::Optional(nested) => nested.iter().any(effect_has_replacement),
        Effect::Conditional {
            if_true, if_false, ..
        } => {
            if_true.iter().any(effect_has_replacement)
                || if_false.iter().any(effect_has_replacement)
        }
        Effect::GrantAbility { ability, .. } => ability.effects.iter().any(effect_has_replacement),
        _ => false,
    }
}

fn effect_has_delayed_trigger(effect: &Effect) -> bool {
    match effect {
        Effect::CreateTokenWithDelayedMove { .. } | Effect::SchedulePaymentOrLose(_) => true,
        Effect::MoveZone(move_zone) => move_zone.delayed_until.is_some(),
        Effect::Draw { delayed_until, .. } => delayed_until.is_some(),
        Effect::ExileTop(exile) => exile.delayed_destination.is_some(),
        Effect::ExileCollection(exile) => exile.delayed_destination.is_some(),
        Effect::Optional(nested) => nested.iter().any(effect_has_delayed_trigger),
        Effect::Conditional {
            if_true, if_false, ..
        } => {
            if_true.iter().any(effect_has_delayed_trigger)
                || if_false.iter().any(effect_has_delayed_trigger)
        }
        Effect::GrantAbility { ability, .. } => {
            ability.effects.iter().any(effect_has_delayed_trigger)
        }
        _ => false,
    }
}

fn effect_has_optional_choice(effect: &Effect) -> bool {
    match effect {
        Effect::Optional(_) => true,
        Effect::Conditional {
            if_true, if_false, ..
        } => {
            if_true.iter().any(effect_has_optional_choice)
                || if_false.iter().any(effect_has_optional_choice)
        }
        Effect::GrantAbility { ability, .. } => {
            ability.effects.iter().any(effect_has_optional_choice)
        }
        _ => false,
    }
}

fn effect_has_complete_granted_ability(effect: &Effect) -> bool {
    match effect {
        Effect::GrantAbility { ability, .. } => !ability.effects.is_empty(),
        Effect::Optional(nested) => nested.iter().any(effect_has_complete_granted_ability),
        Effect::Conditional {
            if_true, if_false, ..
        } => {
            if_true.iter().any(effect_has_complete_granted_ability)
                || if_false.iter().any(effect_has_complete_granted_ability)
        }
        _ => false,
    }
}

/// Recognize a complete Oracle line without promoting syntax-only structure to
/// an executable compiler result.
pub fn recognize_oracle_clause_backend(
    input: OracleClauseBackendInput<'_>,
) -> Result<OracleClauseRecognitionOutcome, OracleClauseRecognitionError> {
    recognize_oracle_clause_backend_with_optional_context(input, None, None, None, None, None)
}

/// Recognize a complete Oracle line with exact physical-card structure
/// available to contextual keyword families.
pub fn recognize_oracle_clause_backend_with_context(
    input: OracleClauseBackendInput<'_>,
    card_context: OracleClauseCardContext<'_>,
) -> Result<OracleClauseRecognitionOutcome, OracleClauseRecognitionError> {
    recognize_oracle_clause_backend_with_optional_context(
        input,
        Some(card_context),
        None,
        None,
        None,
        None,
    )
}

/// Recognize one complete line with all content-only semantic face context
/// required by exact lifecycle keyword families.
pub fn recognize_oracle_clause_backend_with_semantic_context(
    input: OracleClauseBackendInput<'_>,
    semantic_context: OracleClauseSemanticContext<'_>,
) -> Result<OracleClauseRecognitionOutcome, OracleClauseRecognitionError> {
    recognize_oracle_clause_backend_with_optional_context(
        input,
        Some(semantic_context.card),
        semantic_context.graveyard_transform,
        semantic_context.level_progression,
        semantic_context.source_mana_value,
        semantic_context.complete_face_oracle_text,
    )
}

fn recognize_oracle_clause_backend_with_optional_context(
    input: OracleClauseBackendInput<'_>,
    card_context: Option<OracleClauseCardContext<'_>>,
    graveyard_transform_context: Option<&GraveyardTransformSourceSemanticContext>,
    level_progression_context: Option<&LevelProgressionProgram>,
    source_mana_value: Option<u32>,
    complete_face_oracle_text: Option<&str>,
) -> Result<OracleClauseRecognitionOutcome, OracleClauseRecognitionError> {
    let syntax = recognize_backend_input_syntax(&input)
        .map_err(|syntax_error| OracleClauseRecognitionError::MalformedSyntax { syntax_error })?;
    let compile_result = {
        let validated = syntax.validated_line();
        compile_oracle_clause_program(
            input.clone(),
            validated,
            card_context,
            graveyard_transform_context,
            level_progression_context,
            source_mana_value,
            complete_face_oracle_text,
        )
    };
    match compile_result {
        Ok(clause) => Ok(OracleClauseRecognitionOutcome::ProgramRecognized(clause)),
        Err(OracleClauseBackendError::Native { error }) => Ok(compile_syntax_only_clause(
            &input,
            syntax,
            error,
            DelegatedKeywordFallbackDiagnostic::NoExactProgram,
        )),
        Err(OracleClauseBackendError::DelegatedKeyword {
            native_error,
            keyword_error,
        }) => Ok(compile_syntax_only_clause(
            &input,
            syntax,
            native_error,
            DelegatedKeywordFallbackDiagnostic::CompileError(keyword_error),
        )),
        Err(OracleClauseBackendError::MalformedSyntax { .. }) => {
            unreachable!("program compilation runs only after syntax validation")
        }
    }
}

fn recognize_backend_input_syntax(
    input: &OracleClauseBackendInput<'_>,
) -> Result<RecognizedOracleClauseSyntax, OracleClauseSyntaxError> {
    recognize_oracle_clause_syntax(OracleClauseSyntaxInput {
        normalized_line: input.oracle_clause,
        semantic_context: OracleSyntaxSemanticContext::CardFace,
        provenance: OracleSyntaxProvenance {
            source_name: Some(input.source_name),
            face_index: Some(input.face_index),
            clause_index: Some(input.clause_index),
            ..OracleSyntaxProvenance::default()
        },
    })
}

fn compile_syntax_only_clause(
    input: &OracleClauseBackendInput<'_>,
    syntax: RecognizedOracleClauseSyntax,
    native_error: CompileError,
    delegated_diagnostic: DelegatedKeywordFallbackDiagnostic,
) -> OracleClauseRecognitionOutcome {
    let syntax_digest = syntax.syntax_digest().to_owned();
    let normalized_clause = normalize_oracle_clause(
        input.oracle_clause,
        input.source_name,
        input.source_type_line,
    );
    OracleClauseRecognitionOutcome::SyntaxOnly(SyntaxOnlyOracleClause {
        address: ClauseAddress {
            face_index: input.face_index,
            clause_index: input.clause_index,
        },
        normalized_clause,
        syntax_digest,
        syntax,
        native_error,
        delegated_diagnostic,
    })
}

fn compile_delegated_keyword_clause_with_context(
    input: &OracleClauseBackendInput<'_>,
    card_context: Option<OracleClauseCardContext<'_>>,
) -> Result<Option<DelegatedKeywordClause>, KeywordCompileError> {
    let source_clause = input.oracle_clause.trim();
    let Some(keyword) = exact_singleton_keyword(source_clause) else {
        return Ok(None);
    };
    let normalized_clause = normalize_oracle_clause(
        input.oracle_clause,
        input.source_name,
        input.source_type_line,
    );

    let printed_keyword = canonical_keyword_program_label(keyword, source_clause);
    let keyword_program = compile_keyword_program(KeywordProgramInput {
        face_index: input.face_index,
        clause_index: input.clause_index,
        printed_keyword,
        oracle_fragment: Some(source_clause),
    })?;
    if !delegated_program_has_exact_contract(
        &keyword_program,
        keyword,
        printed_keyword,
        source_clause,
    ) {
        return Ok(None);
    }

    let semantic_context = delegated_keyword_semantic_context(
        keyword,
        input.source_type_line,
        input.face_index,
        card_context,
    )?;
    let required_live_bridge_capabilities =
        required_live_bridge_capabilities_for_program(&keyword_program);
    let semantic_digest = delegated_keyword_semantic_digest(
        source_clause,
        &keyword_program,
        semantic_context,
        required_live_bridge_capabilities,
    );
    Ok(Some(DelegatedKeywordClause {
        runtime_version: ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION,
        semantic_digest,
        address: ClauseAddress {
            face_index: input.face_index,
            clause_index: input.clause_index,
        },
        normalized_clause,
        keyword_program,
        required_live_bridge_capabilities,
    }))
}

fn canonical_keyword_program_label(keyword: OfficialKeyword, source_clause: &str) -> &str {
    if keyword == OfficialKeyword::Landwalk {
        return exact_keyword_parts(source_clause)
            .map(|(core, _)| core)
            .unwrap_or(keyword.printed_label());
    }
    keyword.printed_label()
}

fn exact_singleton_keyword(normalized_clause: &str) -> Option<OfficialKeyword> {
    if is_exact_standalone_investigate_clause(normalized_clause) {
        return Some(OfficialKeyword::Investigate);
    }
    if let Some(keyword) = exact_standalone_keyword_action(normalized_clause) {
        return Some(keyword);
    }
    let (core, _reminder) = exact_keyword_parts(normalized_clause)?;
    ALLOWED_SINGLETON_KEYWORDS
        .iter()
        .copied()
        .find(|keyword| keyword_core_matches(*keyword, core))
}

fn is_exact_standalone_investigate_clause(normalized_clause: &str) -> bool {
    const CANONICAL_REMINDER: &str = "Investigate. (Create a Clue token. It's an artifact with \"{2}, Sacrifice this token: Draw a card.\")";
    let clause = normalized_clause.trim();
    clause.eq_ignore_ascii_case("Investigate")
        || clause.eq_ignore_ascii_case("Investigate.")
        || clause.eq_ignore_ascii_case(CANONICAL_REMINDER)
}

fn exact_standalone_keyword_action(clause: &str) -> Option<OfficialKeyword> {
    let (core, reminder) = exact_keyword_parts(clause)?;
    [
        (
            OfficialKeyword::Mill,
            is_exact_standalone_mill_action(core, reminder),
        ),
        (
            OfficialKeyword::Fight,
            is_exact_standalone_fight_action(core, reminder),
        ),
        (
            OfficialKeyword::Regenerate,
            is_exact_standalone_regenerate_action(core, reminder),
        ),
        (
            OfficialKeyword::Scry,
            is_exact_numbered_library_action("Scry", core, reminder),
        ),
        (
            OfficialKeyword::Surveil,
            is_exact_numbered_library_action("Surveil", core, reminder),
        ),
    ]
    .into_iter()
    .find_map(|(keyword, matched)| matched.then_some(keyword))
}

fn is_exact_standalone_mill_action(core: &str, reminder: Option<&str>) -> bool {
    if reminder.is_some() {
        return false;
    }
    let Some(sentence) = core.strip_suffix('.') else {
        return false;
    };
    let lower = sentence.to_ascii_lowercase();
    let Some(amount) = [
        "mill ",
        "target player mills ",
        "target opponent mills ",
        "each player mills ",
        "each opponent mills ",
    ]
    .into_iter()
    .find_map(|prefix| lower.strip_prefix(prefix)) else {
        return false;
    };
    is_exact_card_count(amount)
}

fn is_exact_card_count(amount: &str) -> bool {
    let Some((number, noun)) = amount.rsplit_once(' ') else {
        return false;
    };
    let value = match number {
        "a" | "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        "eleven" => Some(11),
        "twelve" => Some(12),
        "thirteen" => Some(13),
        "fourteen" => Some(14),
        "fifteen" => Some(15),
        "sixteen" => Some(16),
        "seventeen" => Some(17),
        "eighteen" => Some(18),
        "nineteen" => Some(19),
        "twenty" => Some(20),
        digits => digits
            .parse::<u32>()
            .ok()
            .filter(|value| value.to_string() == digits),
    };
    value.is_some_and(|value| (value == 1 && noun == "card") || (value != 1 && noun == "cards"))
}

fn is_exact_standalone_fight_action(core: &str, reminder: Option<&str>) -> bool {
    const CORES: &[&str] = &[
        "Target creature you control fights target creature you don't control.",
        "Target creature you control fights target creature an opponent controls.",
        "Target creature fights another target creature.",
    ];
    if !CORES.contains(&core) {
        return false;
    }
    reminder.is_none_or(|reminder| reminder == "Each deals damage equal to its power to the other.")
}

fn is_exact_standalone_regenerate_action(core: &str, reminder: Option<&str>) -> bool {
    const CORES: &[&str] = &[
        "Regenerate target creature.",
        "Regenerate target permanent.",
        "Regenerate each creature you control.",
    ];
    if !CORES.contains(&core) {
        return false;
    }
    reminder.is_none_or(|reminder| {
        core == "Regenerate target creature."
            && reminder
                == "The next time that creature would be destroyed this turn, instead tap it, remove it from combat, and heal all damage on it."
    })
}

fn is_exact_numbered_library_action(action: &str, core: &str, reminder: Option<&str>) -> bool {
    let Some(sentence) = core.strip_suffix('.') else {
        return false;
    };
    let Some(amount) = sentence
        .strip_prefix(action)
        .and_then(|suffix| suffix.strip_prefix(' '))
    else {
        return false;
    };
    let Ok(amount) = amount.parse::<u32>() else {
        return false;
    };
    if amount.to_string() != sentence[action.len() + 1..] {
        return false;
    }
    reminder.is_none_or(|reminder| exact_library_action_reminder(action, amount, reminder))
}

fn exact_library_action_reminder(action: &str, amount: u32, reminder: &str) -> bool {
    if amount == 1 {
        return match action {
            "Scry" => {
                reminder
                    == "Look at the top card of your library. You may put it on the bottom of your library."
            }
            "Surveil" => {
                reminder
                    == "Look at the top card of your library. You may put it into your graveyard."
            }
            _ => false,
        };
    }
    let Some(word) = canonical_small_number_word(amount) else {
        return false;
    };
    match action {
        "Scry" => {
            reminder
                == format!(
                    "Look at the top {word} cards of your library, then put any number of them on the bottom and the rest on top in any order."
                )
        }
        "Surveil" => {
            reminder
                == format!(
                    "Look at the top {word} cards of your library, then put any number of them into your graveyard and the rest on top of your library in any order."
                )
        }
        _ => false,
    }
}

fn canonical_small_number_word(value: u32) -> Option<&'static str> {
    match value {
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

/// Return a complete singleton ability core with, at most, one balanced
/// trailing reminder parenthetical removed. Text before or after that one
/// parenthetical remains part of the ability and therefore cannot be silently
/// discarded by the delegated compiler.
fn exact_keyword_parts(normalized_clause: &str) -> Option<(&str, Option<&str>)> {
    let clause = normalized_clause.trim();
    if clause.is_empty() || clause.contains('\n') || clause.contains('\r') {
        return None;
    }

    let mut depth = 0u32;
    let mut outer_open = None;
    let mut outer_close = None;
    for (index, character) in clause.char_indices() {
        match character {
            '(' => {
                if depth == 0 {
                    if outer_open.is_some() {
                        return None;
                    }
                    if index == 0
                        || !clause[..index]
                            .chars()
                            .next_back()
                            .is_some_and(char::is_whitespace)
                    {
                        return None;
                    }
                    outer_open = Some(index);
                }
                depth = depth.checked_add(1)?;
            }
            ')' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    outer_close = Some(index);
                }
            }
            _ => {
                if outer_close.is_some() {
                    return None;
                }
            }
        }
    }
    if depth != 0 {
        return None;
    }

    match (outer_open, outer_close) {
        (None, None) => Some((clause, None)),
        (Some(open), Some(close)) if close + ')'.len_utf8() == clause.len() => {
            let core = clause[..open].trim_end();
            let reminder = clause[open + '('.len_utf8()..close].trim();
            (!core.is_empty() && !reminder.is_empty()).then_some((core, Some(reminder)))
        }
        _ => None,
    }
}

fn keyword_core_matches(keyword: OfficialKeyword, core: &str) -> bool {
    let lower = core.to_ascii_lowercase();
    match keyword {
        OfficialKeyword::Protection => lower.starts_with("protection from "),
        OfficialKeyword::Flying => lower == "flying",
        OfficialKeyword::Investigate => false,
        OfficialKeyword::Kicker => {
            lower.starts_with("kicker ") || lower.starts_with("multikicker ")
        }
        OfficialKeyword::Flashback => lower.starts_with("flashback "),
        OfficialKeyword::Morph => lower.starts_with("morph "),
        OfficialKeyword::Flash => lower == "flash",
        OfficialKeyword::Menace => lower == "menace",
        OfficialKeyword::Defender => lower == "defender",
        OfficialKeyword::Reach => lower == "reach",
        OfficialKeyword::Changeling => lower == "changeling",
        OfficialKeyword::Infect => lower == "infect",
        OfficialKeyword::Fear => lower == "fear",
        OfficialKeyword::Shadow => lower == "shadow",
        OfficialKeyword::Landwalk => is_supported_landwalk_core(&lower),
        OfficialKeyword::Affinity => lower == "affinity for artifacts",
        OfficialKeyword::Cascade => lower == "cascade",
        OfficialKeyword::Delve => lower == "delve",
        OfficialKeyword::Fuse => lower == "fuse",
        OfficialKeyword::Aftermath => lower == "aftermath",
        OfficialKeyword::Rebound => lower == "rebound",
        OfficialKeyword::Exalted => lower == "exalted",
        OfficialKeyword::Bushido => lower
            .strip_prefix("bushido ")
            .is_some_and(is_canonical_positive_decimal),
        OfficialKeyword::Wither => lower == "wither",
        OfficialKeyword::Horsemanship => lower == "horsemanship",
        OfficialKeyword::Flanking => lower == "flanking",
        OfficialKeyword::Persist => lower == "persist",
        OfficialKeyword::Undying => lower == "undying",
        OfficialKeyword::Toxic => lower
            .strip_prefix("toxic ")
            .is_some_and(is_canonical_positive_decimal),
        OfficialKeyword::Daybound => lower == "daybound",
        OfficialKeyword::Nightbound => lower == "nightbound",
        OfficialKeyword::StartYourEngines => lower == "start your engines!",
        OfficialKeyword::ChooseABackground => lower == "choose a background",
        OfficialKeyword::DoctorsCompanion => lower == "doctor's companion",
        OfficialKeyword::Exploit => lower == "exploit",
        OfficialKeyword::Soulbond => lower == "soulbond",
        OfficialKeyword::Evolve => lower == "evolve",
        OfficialKeyword::Improvise => lower == "improvise",
        OfficialKeyword::Intimidate => lower == "intimidate",
        OfficialKeyword::Spree => lower == "spree",
        OfficialKeyword::Bargain => lower == "bargain",
        OfficialKeyword::Mentor => lower == "mentor",
        OfficialKeyword::Extort => lower == "extort",
        OfficialKeyword::LivingWeapon => lower == "living weapon",
        OfficialKeyword::Myriad => lower == "myriad",
        OfficialKeyword::Retrace => lower == "retrace",
        OfficialKeyword::Backup => lower
            .strip_prefix("backup ")
            .is_some_and(is_canonical_positive_decimal),
        OfficialKeyword::UmbraArmor => lower == "umbra armor",
        OfficialKeyword::Cipher => lower == "cipher",
        OfficialKeyword::Renown => lower
            .strip_prefix("renown ")
            .is_some_and(is_canonical_positive_decimal),
        OfficialKeyword::Ascend => lower == "ascend",
        OfficialKeyword::Devoid => lower == "devoid",
        OfficialKeyword::Convoke => lower == "convoke",
        OfficialKeyword::Equip => lower.starts_with("equip "),
        OfficialKeyword::Enchant => lower.starts_with("enchant "),
        OfficialKeyword::CumulativeUpkeep => lower.starts_with("cumulative upkeep"),
        OfficialKeyword::Haste => lower == "haste",
        OfficialKeyword::Vigilance => lower == "vigilance",
        OfficialKeyword::Trample => lower == "trample",
        OfficialKeyword::Deathtouch => lower == "deathtouch",
        OfficialKeyword::Lifelink => lower == "lifelink",
        OfficialKeyword::FirstStrike => lower == "first strike",
        OfficialKeyword::DoubleStrike => lower == "double strike",
        OfficialKeyword::Hexproof => lower == "hexproof" || lower.starts_with("hexproof from "),
        OfficialKeyword::Shroud => lower == "shroud",
        OfficialKeyword::Indestructible => lower == "indestructible",
        OfficialKeyword::Prowess => lower == "prowess",
        OfficialKeyword::Ward => lower.starts_with("ward "),
        OfficialKeyword::Cycling => lower.starts_with("cycling "),
        OfficialKeyword::Mill
        | OfficialKeyword::Regenerate
        | OfficialKeyword::Fight
        | OfficialKeyword::Saga
        | OfficialKeyword::Scry
        | OfficialKeyword::Surveil => false,
    }
}

fn is_canonical_positive_decimal(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('0')
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<i32>().is_ok_and(|amount| amount > 0)
}

fn is_supported_landwalk_core(core: &str) -> bool {
    matches!(
        core,
        "plainswalk"
            | "islandwalk"
            | "swampwalk"
            | "mountainwalk"
            | "forestwalk"
            | "desertwalk"
            | "legendary landwalk"
            | "nonbasic landwalk"
            | "snow landwalk"
    )
}

fn delegated_program_has_exact_contract(
    keyword_program: &KeywordProgram,
    expected_keyword: OfficialKeyword,
    printed_keyword: &str,
    source_clause: &str,
) -> bool {
    keyword_program.has_exact_contract()
        && keyword_program.keyword() == expected_keyword
        && keyword_kind_agrees(expected_keyword, keyword_program.kind())
        && keyword_program
            .source()
            .printed_keyword
            .trim()
            .eq_ignore_ascii_case(printed_keyword.trim())
        && keyword_program.source().oracle_fragment.as_deref() == Some(source_clause)
}

fn keyword_kind_agrees(keyword: OfficialKeyword, kind: &KeywordProgramKind) -> bool {
    matches!(
        (keyword, kind),
        (
            OfficialKeyword::Protection,
            KeywordProgramKind::Protection(_)
        ) | (OfficialKeyword::Flying, KeywordProgramKind::Flying)
            | (
                OfficialKeyword::Investigate,
                KeywordProgramKind::Investigate
            )
            | (OfficialKeyword::Kicker, KeywordProgramKind::Kicker(_))
            | (OfficialKeyword::Flashback, KeywordProgramKind::Flashback(_))
            | (OfficialKeyword::Morph, KeywordProgramKind::Morph(_))
            | (OfficialKeyword::Flash, KeywordProgramKind::Flash)
            | (OfficialKeyword::Menace, KeywordProgramKind::Menace)
            | (OfficialKeyword::Defender, KeywordProgramKind::Defender)
            | (OfficialKeyword::Reach, KeywordProgramKind::Reach)
            | (
                OfficialKeyword::Changeling,
                KeywordProgramKind::Changeling(_)
            )
            | (OfficialKeyword::Infect, KeywordProgramKind::Infect(_))
            | (OfficialKeyword::Fear, KeywordProgramKind::Fear(_))
            | (OfficialKeyword::Shadow, KeywordProgramKind::Shadow(_))
            | (OfficialKeyword::Landwalk, KeywordProgramKind::Landwalk(_))
            | (OfficialKeyword::Affinity, KeywordProgramKind::Affinity(_))
            | (OfficialKeyword::Cascade, KeywordProgramKind::Cascade(_))
            | (OfficialKeyword::Delve, KeywordProgramKind::Delve(_))
            | (OfficialKeyword::Fuse, KeywordProgramKind::Fuse(_))
            | (OfficialKeyword::Aftermath, KeywordProgramKind::Aftermath(_))
            | (OfficialKeyword::Rebound, KeywordProgramKind::Rebound(_))
            | (OfficialKeyword::Exalted, KeywordProgramKind::Exalted(_))
            | (OfficialKeyword::Bushido, KeywordProgramKind::Bushido(_))
            | (OfficialKeyword::Wither, KeywordProgramKind::Wither(_))
            | (
                OfficialKeyword::Horsemanship,
                KeywordProgramKind::Horsemanship(_)
            )
            | (OfficialKeyword::Flanking, KeywordProgramKind::Flanking(_))
            | (OfficialKeyword::Persist, KeywordProgramKind::Persist(_))
            | (OfficialKeyword::Undying, KeywordProgramKind::Undying(_))
            | (OfficialKeyword::Toxic, KeywordProgramKind::Toxic(_))
            | (OfficialKeyword::Daybound, KeywordProgramKind::Daybound(_))
            | (
                OfficialKeyword::Nightbound,
                KeywordProgramKind::Nightbound(_)
            )
            | (
                OfficialKeyword::StartYourEngines,
                KeywordProgramKind::StartYourEngines(_)
            )
            | (
                OfficialKeyword::ChooseABackground,
                KeywordProgramKind::ChooseABackground(_)
            )
            | (
                OfficialKeyword::DoctorsCompanion,
                KeywordProgramKind::DoctorsCompanion(_)
            )
            | (OfficialKeyword::Exploit, KeywordProgramKind::Exploit(_))
            | (OfficialKeyword::Soulbond, KeywordProgramKind::Soulbond(_))
            | (OfficialKeyword::Evolve, KeywordProgramKind::Evolve(_))
            | (OfficialKeyword::Improvise, KeywordProgramKind::Improvise(_))
            | (
                OfficialKeyword::Intimidate,
                KeywordProgramKind::Intimidate(_)
            )
            | (OfficialKeyword::Spree, KeywordProgramKind::Spree(_))
            | (OfficialKeyword::Bargain, KeywordProgramKind::Bargain(_))
            | (OfficialKeyword::Mentor, KeywordProgramKind::Mentor(_))
            | (OfficialKeyword::Extort, KeywordProgramKind::Extort(_))
            | (
                OfficialKeyword::LivingWeapon,
                KeywordProgramKind::LivingWeapon(_)
            )
            | (OfficialKeyword::Myriad, KeywordProgramKind::Myriad(_))
            | (OfficialKeyword::Retrace, KeywordProgramKind::Retrace(_))
            | (OfficialKeyword::Backup, KeywordProgramKind::Backup(_))
            | (
                OfficialKeyword::UmbraArmor,
                KeywordProgramKind::UmbraArmor(_)
            )
            | (OfficialKeyword::Cipher, KeywordProgramKind::Cipher(_))
            | (OfficialKeyword::Renown, KeywordProgramKind::Renown(_))
            | (OfficialKeyword::Ascend, KeywordProgramKind::Ascend(_))
            | (OfficialKeyword::Devoid, KeywordProgramKind::Devoid)
            | (OfficialKeyword::Convoke, KeywordProgramKind::Convoke(_))
            | (OfficialKeyword::Equip, KeywordProgramKind::Equip(_))
            | (OfficialKeyword::Enchant, KeywordProgramKind::Enchant(_))
            | (
                OfficialKeyword::CumulativeUpkeep,
                KeywordProgramKind::CumulativeUpkeep(_)
            )
            | (OfficialKeyword::Haste, KeywordProgramKind::Haste)
            | (OfficialKeyword::Vigilance, KeywordProgramKind::Vigilance)
            | (OfficialKeyword::Trample, KeywordProgramKind::Trample)
            | (OfficialKeyword::Deathtouch, KeywordProgramKind::Deathtouch)
            | (OfficialKeyword::Lifelink, KeywordProgramKind::Lifelink)
            | (
                OfficialKeyword::FirstStrike,
                KeywordProgramKind::FirstStrike
            )
            | (
                OfficialKeyword::DoubleStrike,
                KeywordProgramKind::DoubleStrike
            )
            | (OfficialKeyword::Hexproof, KeywordProgramKind::Hexproof(_))
            | (OfficialKeyword::Shroud, KeywordProgramKind::Shroud)
            | (
                OfficialKeyword::Indestructible,
                KeywordProgramKind::Indestructible
            )
            | (OfficialKeyword::Prowess, KeywordProgramKind::Prowess)
            | (OfficialKeyword::Ward, KeywordProgramKind::Ward(_))
            | (OfficialKeyword::Cycling, KeywordProgramKind::Cycling(_))
            | (OfficialKeyword::Mill, KeywordProgramKind::Mill)
            | (
                OfficialKeyword::Regenerate,
                KeywordProgramKind::Regenerate(_)
            )
            | (OfficialKeyword::Fight, KeywordProgramKind::Fight)
            | (OfficialKeyword::Scry, KeywordProgramKind::Scry)
            | (OfficialKeyword::Surveil, KeywordProgramKind::Surveil)
    )
}

fn delegated_keyword_semantic_context(
    keyword: OfficialKeyword,
    source_type_line: &str,
    face_index: u16,
    card_context: Option<OracleClauseCardContext<'_>>,
) -> Result<&'static [&'static str], KeywordCompileError> {
    const NO_CONTEXT: &[&str] = &[];
    const EQUIPMENT_CONTEXT: &[&str] = &["source-is-artifact-equipment/v1"];
    const AURA_CONTEXT: &[&str] = &["source-is-enchantment-aura/v1"];
    const INSTANT_OR_SORCERY_CONTEXT: &[&str] = &["source-is-instant-or-sorcery/v1"];
    const PERMANENT_CONTEXT: &[&str] = &["source-is-permanent/v1"];
    const ASCEND_SPELL_CONTEXT: &[&str] = &["ascend-source-is-instant-or-sorcery/v1"];
    const ASCEND_PERMANENT_CONTEXT: &[&str] = &["ascend-source-is-permanent/v1"];
    const LEGENDARY_CARD_CONTEXT: &[&str] = &["source-is-legendary-card/v1"];
    const LEGENDARY_CREATURE_CARD_CONTEXT: &[&str] = &["source-is-legendary-creature-card/v1"];
    const CREATURE_CONTEXT: &[&str] = &["source-is-creature/v1"];
    const SPELL_CARD_CONTEXT: &[&str] = &["source-can-exist-as-spell-on-stack/v1"];
    const BARGAIN_CONTEXT: &[&str] = &["bargain-source-is-nonland-spell-card/v1"];
    const MENTOR_CONTEXT: &[&str] = &["mentor-source-is-creature/v1"];
    const EXTORT_CONTEXT: &[&str] = &["extort-source-is-permanent/v1"];
    const LIVING_WEAPON_CONTEXT: &[&str] = &["living-weapon-source-is-artifact-equipment/v1"];
    const MYRIAD_CONTEXT: &[&str] = &["myriad-source-is-creature/v1"];
    const RETRACE_CONTEXT: &[&str] = &["retrace-source-is-nonland-spell-card/v1"];
    const BACKUP_CONTEXT: &[&str] = &["backup-source-is-creature/v1"];
    const UMBRA_ARMOR_CONTEXT: &[&str] = &["umbra-armor-source-is-enchantment-aura/v1"];
    const CIPHER_CONTEXT: &[&str] = &["cipher-source-is-instant-or-sorcery/v1"];
    const RENOWN_CONTEXT: &[&str] = &["renown-source-is-creature/v1"];
    const AFFINITY_CONTEXT: &[&str] = &["affinity-source-is-nonland-spell-card/v1"];
    const CASCADE_CONTEXT: &[&str] = &["cascade-source-is-nonland-spell-card/v1"];
    const DELVE_CONTEXT: &[&str] = &["delve-source-is-nonland-spell-card/v1"];
    const FUSE_CONTEXT: &[&str] = &[
        "fuse-two-half-split/v1",
        "fuse-source-is-instant-or-sorcery/v1",
    ];
    const REBOUND_CONTEXT: &[&str] = &["rebound-source-is-nonland-spell-card/v1"];
    const CONVOKE_CONTEXT: &[&str] = &["convoke-source-is-nonland-spell-card/v1"];
    const AFTERMATH_CONTEXT: &[&str] = &[
        "aftermath-two-half-split/v1",
        "aftermath-source-is-instant-or-sorcery/v1",
        "aftermath-selected-half-printed-mana-cost/v1",
    ];

    let type_words = source_type_line
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let contains = |word: &str| type_words.iter().any(|candidate| candidate == word);
    let is_instant_or_sorcery = contains("instant") || contains("sorcery");
    let is_permanent = [
        "artifact",
        "battle",
        "creature",
        "enchantment",
        "land",
        "planeswalker",
    ]
    .into_iter()
    .any(contains);
    let is_spell_card = [
        "artifact",
        "battle",
        "creature",
        "enchantment",
        "instant",
        "kindred",
        "planeswalker",
        "sorcery",
    ]
    .into_iter()
    .any(contains)
        && !contains("land");
    match keyword {
        OfficialKeyword::Equip if contains("artifact") && contains("equipment") => {
            Ok(EQUIPMENT_CONTEXT)
        }
        OfficialKeyword::Equip => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Equip requires the source face to be an Artifact Equipment".into(),
        }),
        OfficialKeyword::Enchant if contains("enchantment") && contains("aura") => Ok(AURA_CONTEXT),
        OfficialKeyword::Enchant => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Enchant requires the source face to be an Enchantment Aura".into(),
        }),
        OfficialKeyword::Affinity if is_spell_card => Ok(AFFINITY_CONTEXT),
        OfficialKeyword::Affinity => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Affinity requires a nonland source that can exist as a spell on the stack"
                .into(),
        }),
        OfficialKeyword::Cascade if is_spell_card => Ok(CASCADE_CONTEXT),
        OfficialKeyword::Cascade => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Cascade requires a nonland source that can exist as a spell on the stack"
                .into(),
        }),
        OfficialKeyword::Delve if is_spell_card => Ok(DELVE_CONTEXT),
        OfficialKeyword::Delve => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Delve requires a nonland source that can exist as a spell on the stack".into(),
        }),
        OfficialKeyword::Fuse => {
            let Some(card_context) = card_context else {
                return Err(KeywordCompileError::InsufficientSourceData {
                    keyword,
                    detail: "Fuse requires exact split-card face context".into(),
                });
            };
            if !card_context.layout.trim().eq_ignore_ascii_case("split") {
                return Err(KeywordCompileError::InsufficientSourceData {
                    keyword,
                    detail: "Fuse requires split layout".into(),
                });
            }
            if card_context.face_count != 2 {
                return Err(KeywordCompileError::InsufficientSourceData {
                    keyword,
                    detail: "Fuse requires exactly two real split-card faces".into(),
                });
            }
            if usize::from(face_index) >= card_context.face_count {
                return Err(KeywordCompileError::InsufficientSourceData {
                    keyword,
                    detail: "Fuse source face address is outside the split card".into(),
                });
            }
            if !is_instant_or_sorcery {
                return Err(KeywordCompileError::InsufficientSourceData {
                    keyword,
                    detail: "Fuse requires an Instant or Sorcery source face".into(),
                });
            }
            Ok(FUSE_CONTEXT)
        }
        OfficialKeyword::Rebound if is_spell_card => Ok(REBOUND_CONTEXT),
        OfficialKeyword::Rebound => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Rebound requires a nonland source that can exist as a spell on the stack"
                .into(),
        }),
        OfficialKeyword::Aftermath => {
            let Some(card_context) = card_context else {
                return Err(KeywordCompileError::InsufficientSourceData {
                    keyword,
                    detail: "Aftermath requires exact split-card face context".into(),
                });
            };
            if !card_context.layout.trim().eq_ignore_ascii_case("split") {
                return Err(KeywordCompileError::InsufficientSourceData {
                    keyword,
                    detail: "Aftermath requires split layout".into(),
                });
            }
            if card_context.face_count != 2 {
                return Err(KeywordCompileError::InsufficientSourceData {
                    keyword,
                    detail: "Aftermath requires exactly two real split-card faces".into(),
                });
            }
            if usize::from(face_index) >= card_context.face_count {
                return Err(KeywordCompileError::InsufficientSourceData {
                    keyword,
                    detail: "Aftermath source face address is outside the split card".into(),
                });
            }
            if !is_instant_or_sorcery {
                return Err(KeywordCompileError::InsufficientSourceData {
                    keyword,
                    detail: "Aftermath requires an Instant or Sorcery source face".into(),
                });
            }
            Ok(AFTERMATH_CONTEXT)
        }
        OfficialKeyword::Exalted if is_permanent => Ok(PERMANENT_CONTEXT),
        OfficialKeyword::Exalted => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Exalted requires a permanent source face".into(),
        }),
        OfficialKeyword::Ascend if is_instant_or_sorcery => Ok(ASCEND_SPELL_CONTEXT),
        OfficialKeyword::Ascend if is_permanent => Ok(ASCEND_PERMANENT_CONTEXT),
        OfficialKeyword::Ascend => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Ascend requires an Instant, Sorcery, or permanent source face".into(),
        }),
        OfficialKeyword::ChooseABackground if contains("legendary") => Ok(LEGENDARY_CARD_CONTEXT),
        OfficialKeyword::ChooseABackground => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Choose a Background requires a legendary source card".into(),
        }),
        OfficialKeyword::DoctorsCompanion if contains("legendary") && contains("creature") => {
            Ok(LEGENDARY_CREATURE_CARD_CONTEXT)
        }
        OfficialKeyword::DoctorsCompanion => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Doctor's companion requires a legendary creature source card".into(),
        }),
        OfficialKeyword::Exploit
        | OfficialKeyword::Soulbond
        | OfficialKeyword::Evolve
        | OfficialKeyword::Intimidate
            if contains("creature") =>
        {
            Ok(CREATURE_CONTEXT)
        }
        OfficialKeyword::Exploit
        | OfficialKeyword::Soulbond
        | OfficialKeyword::Evolve
        | OfficialKeyword::Intimidate => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: format!("{} requires a creature source", keyword.printed_label()),
        }),
        OfficialKeyword::Improvise if is_spell_card => Ok(SPELL_CARD_CONTEXT),
        OfficialKeyword::Improvise => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Improvise requires a source that can exist as a spell on the stack".into(),
        }),
        OfficialKeyword::Spree if is_instant_or_sorcery => Ok(INSTANT_OR_SORCERY_CONTEXT),
        OfficialKeyword::Spree => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Spree requires an Instant or Sorcery source face".into(),
        }),
        OfficialKeyword::Bargain if is_spell_card => Ok(BARGAIN_CONTEXT),
        OfficialKeyword::Bargain => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Bargain requires a nonland source that can exist as a spell on the stack"
                .into(),
        }),
        OfficialKeyword::Mentor if contains("creature") => Ok(MENTOR_CONTEXT),
        OfficialKeyword::Mentor => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Mentor requires a creature source".into(),
        }),
        OfficialKeyword::Extort if is_permanent => Ok(EXTORT_CONTEXT),
        OfficialKeyword::Extort => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Extort requires a permanent source".into(),
        }),
        OfficialKeyword::LivingWeapon if contains("artifact") && contains("equipment") => {
            Ok(LIVING_WEAPON_CONTEXT)
        }
        OfficialKeyword::LivingWeapon => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Living weapon requires an Artifact Equipment source".into(),
        }),
        OfficialKeyword::Myriad if contains("creature") => Ok(MYRIAD_CONTEXT),
        OfficialKeyword::Myriad => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Myriad requires a creature source".into(),
        }),
        OfficialKeyword::Retrace if is_spell_card => Ok(RETRACE_CONTEXT),
        OfficialKeyword::Retrace => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Retrace requires a nonland card that can be cast as a spell".into(),
        }),
        OfficialKeyword::Backup if contains("creature") => Ok(BACKUP_CONTEXT),
        OfficialKeyword::Backup => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Backup requires a creature source".into(),
        }),
        OfficialKeyword::UmbraArmor if contains("enchantment") && contains("aura") => {
            Ok(UMBRA_ARMOR_CONTEXT)
        }
        OfficialKeyword::UmbraArmor => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Umbra armor requires an Enchantment Aura source".into(),
        }),
        OfficialKeyword::Cipher if is_instant_or_sorcery => Ok(CIPHER_CONTEXT),
        OfficialKeyword::Cipher => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Cipher requires an Instant or Sorcery source".into(),
        }),
        OfficialKeyword::Renown if contains("creature") => Ok(RENOWN_CONTEXT),
        OfficialKeyword::Renown => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Renown requires a creature source".into(),
        }),
        OfficialKeyword::Convoke if is_spell_card => Ok(CONVOKE_CONTEXT),
        OfficialKeyword::Convoke => Err(KeywordCompileError::InsufficientSourceData {
            keyword,
            detail: "Convoke requires a nonland source that can exist as a spell on the stack"
                .into(),
        }),
        _ => Ok(NO_CONTEXT),
    }
}

fn required_live_bridge_capabilities(keyword: OfficialKeyword) -> &'static [LiveBridgeCapability] {
    match keyword {
        OfficialKeyword::Protection => PROTECTION_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Flying
        | OfficialKeyword::Menace
        | OfficialKeyword::Reach
        | OfficialKeyword::Fear
        | OfficialKeyword::Shadow
        | OfficialKeyword::Landwalk => STATIC_BLOCK_LEGALITY_CAPABILITIES,
        OfficialKeyword::Changeling => CHANGELING_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Infect => INFECT_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Affinity => AFFINITY_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Cascade => CASCADE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Delve => DELVE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Fuse => FUSE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Aftermath => AFTERMATH_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Rebound => REBOUND_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Exalted => EXALTED_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Bushido => BUSHIDO_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Wither => WITHER_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Horsemanship => HORSEMANSHIP_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Flanking => FLANKING_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Persist | OfficialKeyword::Undying => {
            DEATH_RETURN_LIVE_BRIDGE_CAPABILITIES
        }
        OfficialKeyword::Toxic => TOXIC_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Daybound | OfficialKeyword::Nightbound => {
            DAY_NIGHT_LIVE_BRIDGE_CAPABILITIES
        }
        OfficialKeyword::StartYourEngines => START_YOUR_ENGINES_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::ChooseABackground | OfficialKeyword::DoctorsCompanion => {
            COMMANDER_PARTNER_LIVE_BRIDGE_CAPABILITIES
        }
        OfficialKeyword::Exploit => EXPLOIT_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Soulbond => SOULBOND_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Evolve => EVOLVE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Improvise => IMPROVISE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Intimidate => INTIMIDATE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Spree => SPREE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Bargain => BARGAIN_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Mentor => MENTOR_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Extort => EXTORT_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::LivingWeapon => LIVING_WEAPON_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Myriad => MYRIAD_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Retrace => RETRACE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Backup => BACKUP_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::UmbraArmor => UMBRA_ARMOR_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Cipher => CIPHER_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Renown => RENOWN_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Ascend => ASCEND_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Investigate => INVESTIGATE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Kicker => KICKER_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Flashback => FLASHBACK_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Morph => MORPH_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Flash => STATIC_CAST_TIMING_CAPABILITIES,
        OfficialKeyword::Defender => STATIC_ATTACK_LEGALITY_CAPABILITIES,
        OfficialKeyword::Devoid => DEVOID_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Convoke => CONVOKE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Equip => EQUIP_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Enchant => ENCHANT_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::CumulativeUpkeep => CUMULATIVE_UPKEEP_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Haste => HASTE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Vigilance => VIGILANCE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Trample => TRAMPLE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Deathtouch => DEATHTOUCH_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Lifelink => LIFELINK_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::FirstStrike | OfficialKeyword::DoubleStrike => {
            COMBAT_DAMAGE_STEPS_CAPABILITIES
        }
        OfficialKeyword::Hexproof | OfficialKeyword::Shroud => STATIC_TARGETING_CAPABILITIES,
        OfficialKeyword::Indestructible => INDESTRUCTIBLE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Prowess => PROWESS_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Ward => WARD_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Cycling => CYCLING_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Mill | OfficialKeyword::Scry | OfficialKeyword::Surveil => {
            LIBRARY_ACTION_LIVE_BRIDGE_CAPABILITIES
        }
        OfficialKeyword::Fight => FIGHT_ACTION_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Regenerate => REGENERATE_SOURCE_LIVE_BRIDGE_CAPABILITIES,
        OfficialKeyword::Saga => &[],
    }
}

fn required_live_bridge_capabilities_for_program(
    program: &KeywordProgram,
) -> &'static [LiveBridgeCapability] {
    let KeywordProgramKind::Regenerate(regeneration) = program.kind() else {
        return required_live_bridge_capabilities(program.keyword());
    };
    match (regeneration.replacement, regeneration.recipients) {
        (
            RegenerationReplacement::NextDestructionThisTurn,
            RegenerationRecipientScope::SourcePermanent,
        ) => REGENERATE_SOURCE_LIVE_BRIDGE_CAPABILITIES,
        (
            RegenerationReplacement::NextDestructionThisTurn,
            RegenerationRecipientScope::SingleTarget { .. },
        ) => REGENERATE_TARGET_LIVE_BRIDGE_CAPABILITIES,
        (
            RegenerationReplacement::NextDestructionThisTurn,
            RegenerationRecipientScope::EachCreatureControlledByEffectController { .. },
        ) => REGENERATE_CONTROLLED_CREATURE_SET_LIVE_BRIDGE_CAPABILITIES,
        (
            RegenerationReplacement::EveryDestructionWhileStaticEffectApplies,
            RegenerationRecipientScope::SourcePermanent,
        ) => REGENERATE_STATIC_LIVE_BRIDGE_CAPABILITIES,
        (
            RegenerationReplacement::EveryDestructionWhileStaticEffectApplies,
            RegenerationRecipientScope::SingleTarget { .. }
            | RegenerationRecipientScope::EachCreatureControlledByEffectController { .. },
        ) => REGENERATE_STATIC_LIVE_BRIDGE_CAPABILITIES,
    }
}

fn delegated_keyword_semantic_digest(
    source_clause: &str,
    keyword_program: &KeywordProgram,
    semantic_context: &[&str],
    required_capabilities: &[LiveBridgeCapability],
) -> String {
    delegated_keyword_semantic_digest_with_versions(
        ORACLE_CLAUSE_BACKEND_COMPILER_VERSION,
        ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION,
        source_clause,
        keyword_program,
        semantic_context,
        required_capabilities,
    )
}

fn delegated_keyword_semantic_digest_with_versions(
    backend_compiler_version: &str,
    backend_runtime_version: &str,
    source_clause: &str,
    keyword_program: &KeywordProgram,
    semantic_context: &[&str],
    required_capabilities: &[LiveBridgeCapability],
) -> String {
    delegated_keyword_semantic_digest_with_all_versions(
        backend_compiler_version,
        backend_runtime_version,
        keyword_program.runtime_version(),
        source_clause,
        keyword_program,
        semantic_context,
        required_capabilities,
    )
}

fn delegated_keyword_semantic_digest_with_all_versions(
    backend_compiler_version: &str,
    backend_runtime_version: &str,
    keyword_runtime_version: &str,
    source_clause: &str,
    keyword_program: &KeywordProgram,
    semantic_context: &[&str],
    required_capabilities: &[LiveBridgeCapability],
) -> String {
    let keyword = keyword_program.keyword().printed_label();
    let mut components = vec![
        backend_compiler_version.to_owned(),
        backend_runtime_version.to_owned(),
        source_clause.to_owned(),
        keyword_runtime_version.to_owned(),
        keyword.to_owned(),
    ];
    components.extend(
        keyword_program
            .official_rules()
            .iter()
            .map(|rule| rule.id().to_owned()),
    );
    components.extend(keyword_program_semantic_components(keyword_program));
    components.extend(
        semantic_context
            .iter()
            .map(|component| (*component).to_owned()),
    );
    components.extend(
        required_capabilities
            .iter()
            .map(|capability| capability.stable_id().to_owned()),
    );

    let mut hasher = Sha256::new();
    for component in components {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn keyword_program_semantic_components(program: &KeywordProgram) -> Vec<String> {
    match program.kind() {
        KeywordProgramKind::Regenerate(regeneration) => {
            regeneration_program_semantic_components(regeneration)
                .into_iter()
                .map(str::to_owned)
                .collect()
        }
        KeywordProgramKind::Changeling(changeling) => vec![
            "changeling-program/v1".into(),
            format!(
                "is-characteristic-defining-ability/{}",
                changeling.is_characteristic_defining_ability
            ),
            match changeling.affected_characteristic {
                ChangelingCharacteristic::EveryCreatureType => {
                    "affected-characteristic-every-creature-type/v1".into()
                }
            },
            format!(
                "applies-to-object-with-changeling/{}",
                changeling.applies_to_the_object_with_changeling
            ),
            match changeling.function_scope {
                ChangelingFunctionScope::EverywhereIncludingOutsideTheGame => {
                    "function-scope-everywhere-including-outside-game/v1".into()
                }
            },
        ],
        KeywordProgramKind::Infect(infect) => vec![
            "infect-program/v1".into(),
            format!("is-static-ability/{}", infect.is_static_ability),
            format!(
                "applies-to-combat-and-noncombat-damage/{}",
                infect.applies_to_combat_and_noncombat_damage
            ),
            match infect.player_damage_result {
                InfectPlayerDamageResult::SourceControllerGivesPoisonCountersEqualToDamageInsteadOfLifeLoss => {
                    "player-damage-source-controller-gives-poison-counters-equal-to-damage-instead-of-life-loss/v1".into()
                }
            },
            match infect.creature_damage_result {
                InfectCreatureDamageResult::SourceControllerPutsMinusOneMinusOneCountersEqualToDamageInsteadOfMarkedDamage => {
                    "creature-damage-source-controller-puts-minus-one-minus-one-counters-equal-to-damage-instead-of-marked-damage/v1".into()
                }
            },
            format!(
                "uses-damage-after-replacement-and-prevention/{}",
                infect.uses_damage_after_replacement_and_prevention
            ),
            format!(
                "uses-last-known-information-when-source-left-expected-zone/{}",
                infect.uses_last_known_information_when_source_left_expected_zone
            ),
            format!(
                "functions-no-matter-which-zone-source-deals-damage-from/{}",
                infect.functions_no_matter_which_zone_source_deals_damage_from
            ),
            format!("instances-are-redundant/{}", infect.instances_are_redundant),
        ],
        KeywordProgramKind::Affinity(affinity) => vec![
            "affinity-for-artifacts-program/v1".into(),
            format!("is-static-ability/{}", affinity.is_static_ability),
            match affinity.function_scope {
                SpellStackFunctionScope::WhileThisSpellIsOnTheStack => {
                    "function-scope-while-this-spell-is-on-stack/v1".into()
                }
            },
            match affinity.counted_objects {
                AffinityCountedObjects::ArtifactPermanentsControlledBySpellController => {
                    "count-artifact-permanents-controlled-by-spell-controller/v1".into()
                }
            },
            format!(
                "generic-mana-reduction-per-counted-object/{}",
                affinity.generic_mana_reduction_per_counted_object
            ),
            format!(
                "count-uses-current-state-during-total-cost-determination/{}",
                affinity.count_uses_current_game_state_during_total_cost_determination
            ),
            format!(
                "cannot-reduce-colored-colorless-or-snow-requirements/{}",
                affinity.cannot_reduce_colored_colorless_or_snow_requirements
            ),
            format!(
                "cannot-reduce-generic-requirement-below-zero/{}",
                affinity.cannot_reduce_generic_requirement_below_zero
            ),
            format!(
                "multiple-instances-each-apply/{}",
                affinity.multiple_instances_each_apply
            ),
        ],
        KeywordProgramKind::Cascade(cascade) => vec![
            "cascade-program/v1".into(),
            format!("is-triggered-ability/{}", cascade.is_triggered_ability),
            match cascade.function_scope {
                SpellStackFunctionScope::WhileThisSpellIsOnTheStack => {
                    "function-scope-while-this-spell-is-on-stack/v1".into()
                }
            },
            match cascade.trigger_transition {
                CascadeTriggerTransition::ControllerCastsThisSpell => {
                    "trigger-controller-casts-this-spell/v1".into()
                }
            },
            match cascade.exile_procedure {
                CascadeExileProcedure::FromLibraryTopUntilFirstNonlandCardWithLesserManaValue => {
                    "exile-from-library-top-until-first-nonland-card-with-lesser-mana-value/v1"
                        .into()
                }
            },
            format!(
                "source-spell-mana-value-is-strict-upper-bound/{}",
                cascade.source_spell_mana_value_is_strict_upper_bound
            ),
            format!(
                "resulting-spell-mana-value-rechecked-after-cast-choices/{}",
                cascade.resulting_spell_mana_value_is_rechecked_after_cast_choices
            ),
            format!("eligible-card-cast-is-optional/{}", cascade.eligible_card_cast_is_optional),
            format!(
                "eligible-card-casts-without-paying-mana-cost/{}",
                cascade.eligible_card_casts_without_paying_mana_cost
            ),
            format!(
                "cast-occurs-during-resolution/{}",
                cascade.cast_occurs_during_resolution
            ),
            format!(
                "casting-restrictions-and-additional-costs-still-apply/{}",
                cascade.casting_restrictions_and_additional_costs_still_apply
            ),
            format!(
                "another-alternative-cost-cannot-be-used/{}",
                cascade.another_alternative_cost_cannot_be_used
            ),
            format!(
                "as-you-cascade-action-window-precedes-cast-choice/{}",
                cascade.as_you_cascade_action_window_precedes_cast_choice
            ),
            match cascade.uncast_card_destination {
                CascadeUncastCardDestination::LibraryBottomInRandomOrder => {
                    "uncast-card-destination-library-bottom-in-random-order/v1".into()
                }
            },
            format!("instances-trigger-separately/{}", cascade.instances_trigger_separately),
        ],
        KeywordProgramKind::Delve(delve) => vec![
            "delve-program/v1".into(),
            format!("is-static-ability/{}", delve.is_static_ability),
            match delve.function_scope {
                SpellStackFunctionScope::WhileThisSpellIsOnTheStack => {
                    "function-scope-while-this-spell-is-on-stack/v1".into()
                }
            },
            match delve.payment_exchange {
                DelvePaymentExchange::ExileOneCardFromSpellControllersGraveyardForOneGenericMana => {
                    "exile-one-card-from-spell-controller-graveyard-for-one-generic-mana/v1".into()
                }
            },
            format!(
                "applies-after-total-cost-is-determined/{}",
                delve.applies_after_total_cost_is_determined
            ),
            format!(
                "applies-only-to-generic-mana-in-total-cost/{}",
                delve.applies_only_to_generic_mana_in_total_cost
            ),
            format!(
                "is-not-additional-or-alternative-cost/{}",
                delve.is_not_an_additional_or_alternative_cost
            ),
            format!("is-not-cost-reduction/{}", delve.is_not_a_cost_reduction),
            format!(
                "each-graveyard-card-can-pay-at-most-once/{}",
                delve.each_graveyard_card_can_pay_at_most_once
            ),
            format!(
                "multiple-instances-are-redundant/{}",
                delve.multiple_instances_are_redundant
            ),
        ],
        KeywordProgramKind::Fuse(fuse) => vec![
            "fuse-program/v1".into(),
            format!("is-static-ability/{}", fuse.is_static_ability),
            match fuse.function_scope {
                FuseFunctionScope::CardInItsControllersHand => {
                    "function-scope-card-in-its-controller-hand/v1".into()
                }
            },
            format!(
                "requires-one-physical-split-card-with-exactly-two-halves/{}",
                fuse.requires_one_physical_split_card_with_exactly_two_halves
            ),
            format!("requires-cast-origin-hand/{}", fuse.requires_cast_origin_hand),
            match fuse.cast_choice {
                FuseCastChoice::OneHalfOrBothHalvesChosenBeforeCardIsPutOnStack => {
                    "cast-choice-one-half-or-both-before-card-put-on-stack/v1".into()
                }
            },
            format!("fused-result-is-one-spell/{}", fuse.fused_result_is_one_spell),
            format!(
                "fused-spell-has-combined-characteristics-of-both-halves/{}",
                fuse.fused_spell_has_combined_characteristics_of_both_halves
            ),
            format!(
                "total-cost-includes-each-half-mana-cost/{}",
                fuse.total_cost_includes_each_halfs_mana_cost
            ),
            match fuse.resolution_order {
                FuseResolutionOrder::LeftHalfThenRightHalf => {
                    "resolution-order-left-half-then-right-half/v1".into()
                }
            },
        ],
        KeywordProgramKind::Rebound(rebound) => vec![
            "rebound-program/v1".into(),
            format!("is-static-ability/{}", rebound.is_static_ability),
            match rebound.function_scope {
                SpellStackFunctionScope::WhileThisSpellIsOnTheStack => {
                    "function-scope-while-this-spell-is-on-stack/v1".into()
                }
            },
            match rebound.replacement_event {
                ReboundReplacementEvent::CardSpellCastFromHandWouldEnterOwnersGraveyardAsItResolves => {
                    "replace-card-spell-cast-from-hand-entering-owner-graveyard-as-it-resolves/v1"
                        .into()
                }
            },
            format!(
                "replacement-exiles-same-card/{}",
                rebound.replacement_exiles_the_same_card
            ),
            format!(
                "creates-delayed-trigger-only-when-replacement-exiles-card/{}",
                rebound.creates_delayed_trigger_only_when_replacement_exiles_card
            ),
            match rebound.delayed_trigger {
                ReboundDelayedTrigger::BeginningOfSpellControllersNextUpkeep => {
                    "delayed-trigger-beginning-of-spell-controller-next-upkeep/v1".into()
                }
            },
            format!(
                "delayed-cast-from-exile-is-optional/{}",
                rebound.delayed_cast_from_exile_is_optional
            ),
            format!(
                "delayed-cast-without-paying-mana-cost/{}",
                rebound.delayed_cast_without_paying_mana_cost
            ),
            format!(
                "casting-restrictions-and-additional-costs-still-apply/{}",
                rebound.casting_restrictions_and_additional_costs_still_apply
            ),
            format!(
                "another-alternative-cost-cannot-be-used/{}",
                rebound.another_alternative_cost_cannot_be_used
            ),
            format!(
                "no-effect-for-spell-copy-without-card-or-non-graveyard-destination/{}",
                rebound.no_effect_for_spell_copy_without_card_or_non_graveyard_destination
            ),
            format!("instances-are-redundant/{}", rebound.instances_are_redundant),
        ],
        KeywordProgramKind::Exalted(exalted) => vec![
            "exalted-program/v1".into(),
            format!("is-triggered-ability/{}", exalted.is_triggered_ability),
            match exalted.trigger_transition {
                ExaltedTriggerTransition::CreatureControlledByAbilityControllerIsDeclaredAsOnlyAttackerInCombat => {
                    "trigger-controlled-creature-declared-as-only-attacker-in-combat/v1".into()
                }
            },
            format!(
                "attacks-alone-counts-only-declared-attackers/{}",
                exalted.attacks_alone_counts_only_declared_attackers
            ),
            format!("trigger-uses-stack/{}", exalted.trigger_uses_stack),
            format!(
                "affected-creature-is-declared-attacker-that-caused-trigger/{}",
                exalted.affected_creature_is_the_declared_attacker_that_caused_trigger
            ),
            format!("uses-targeting/{}", exalted.uses_targeting),
            format!("power-delta/{}", exalted.power_delta),
            format!("toughness-delta/{}", exalted.toughness_delta),
            match exalted.duration {
                ExaltedEffectDuration::UntilEndOfTurn => "duration-until-end-of-turn/v1".into(),
            },
            format!(
                "source-need-not-remain-on-battlefield-for-resolution/{}",
                exalted.source_need_not_remain_on_battlefield_for_resolution
            ),
            format!(
                "later-creatures-entering-attacking-do-not-undo-trigger/{}",
                exalted.later_creatures_entering_attacking_do_not_undo_trigger
            ),
            format!("instances-trigger-separately/{}", exalted.instances_trigger_separately),
        ],
        KeywordProgramKind::Ascend(ascend) => vec![
            "ascend-program/v1".into(),
            match ascend.spell_application {
                AscendSpellApplication::SpellAbilityChecksDuringInstantOrSorceryResolution => {
                    "spell-ability-checks-during-instant-or-sorcery-resolution/v1".into()
                }
            },
            match ascend.permanent_application {
                AscendPermanentApplication::StaticAbilityContinuouslyChecksWhilePermanentIsOnBattlefield => {
                    "static-ability-continuously-checks-while-permanent-on-battlefield/v1".into()
                }
            },
            format!("permanent-threshold/{}", ascend.permanent_threshold),
            format!(
                "requires-player-not-already-have-citys-blessing/{}",
                ascend.requires_player_not_already_have_citys_blessing
            ),
            format!(
                "citys-blessing-persists-for-rest-of-game/{}",
                ascend.citys_blessing_persists_for_rest_of_game
            ),
            format!(
                "citys-blessing-has-no-inherent-rules-effect/{}",
                ascend.citys_blessing_has_no_inherent_rules_effect
            ),
            format!(
                "any-number-of-players-may-have-citys-blessing/{}",
                ascend.any_number_of_players_may_have_citys_blessing
            ),
            format!(
                "continuous-effects-reapply-before-trigger-condition-checks/{}",
                ascend.continuous_effects_reapply_before_trigger_condition_checks
            ),
        ],
        KeywordProgramKind::Convoke(convoke) => vec![
            "convoke-program/v1".into(),
            format!("is-static-ability/{}", convoke.is_static_ability),
            match convoke.function_scope {
                SpellStackFunctionScope::WhileThisSpellIsOnTheStack => {
                    "function-scope-while-this-spell-is-on-stack/v1".into()
                }
            },
            match convoke.payment_timing {
                ConvokePaymentTiming::AfterTotalCostIsDeterminedAndManaAbilitiesAreActivatedDuringPayment => {
                    "payment-after-total-cost-determined-and-mana-abilities-activated/v1".into()
                }
            },
            match convoke.payment_exchange {
                ConvokePaymentExchange::TapOneUntappedControlledCreatureForOneGenericOrOneMatchingColoredMana => {
                    "tap-one-untapped-controlled-creature-for-one-generic-or-matching-colored-mana/v1".into()
                }
            },
            format!(
                "cannot-pay-colorless-or-snow-requirements/{}",
                convoke.cannot_pay_colorless_or_snow_requirements
            ),
            format!(
                "is-not-additional-or-alternative-cost/{}",
                convoke.is_not_an_additional_or_alternative_cost
            ),
            format!("is-not-cost-reduction/{}", convoke.is_not_a_cost_reduction),
            format!(
                "summoning-sickness-does-not-prevent-payment/{}",
                convoke.summoning_sickness_does_not_prevent_payment
            ),
            format!(
                "each-creature-can-pay-at-most-once/{}",
                convoke.each_creature_can_pay_at_most_once
            ),
            format!(
                "tapped-creature-designated-as-convoking-spell/{}",
                convoke.tapped_creature_is_designated_as_having_convoked_spell
            ),
            format!(
                "multiple-instances-are-redundant/{}",
                convoke.multiple_instances_are_redundant
            ),
        ],
        KeywordProgramKind::Wither(wither) => vec![
            "wither-program/v1".into(),
            match wither.creature_damage {
                WitherCreatureDamageApplication::MinusOneMinusOneCountersEqualToDamage => {
                    "creature-damage-minus-one-minus-one-counters-equal-to-damage/v1".into()
                }
            },
            if wither.source_controller_places_counters {
                "source-controller-places-counters/true".into()
            } else {
                "source-controller-places-counters/false".into()
            },
            if wither.uses_last_known_information {
                "uses-last-known-information/true".into()
            } else {
                "uses-last-known-information/false".into()
            },
            if wither.functions_in_all_zones {
                "functions-in-all-zones/true".into()
            } else {
                "functions-in-all-zones/false".into()
            },
            if wither.instances_are_redundant {
                "instances-are-redundant/true".into()
            } else {
                "instances-are-redundant/false".into()
            },
        ],
        KeywordProgramKind::Horsemanship(horsemanship) => vec![
            "horsemanship-program/v1".into(),
            match horsemanship.block_restriction {
                HorsemanshipBlockRestriction::BlockerMustHaveHorsemanship => {
                    "blocker-must-have-horsemanship/v1".into()
                }
            },
            if horsemanship.creature_with_horsemanship_may_block_either_kind {
                "creature-with-horsemanship-may-block-either-kind/true".into()
            } else {
                "creature-with-horsemanship-may-block-either-kind/false".into()
            },
            if horsemanship.instances_are_redundant {
                "instances-are-redundant/true".into()
            } else {
                "instances-are-redundant/false".into()
            },
        ],
        KeywordProgramKind::Flanking(flanking) => vec![
            "flanking-program/v1".into(),
            match flanking.trigger_transition {
                FlankingTriggerTransition::SourceBecomesBlockedByCreature => {
                    "source-becomes-blocked-by-creature/v1".into()
                }
            },
            match flanking.blocker_predicate {
                FlankingBlockerPredicate::BlockingCreatureWithoutFlanking => {
                    "blocking-creature-without-flanking/v1".into()
                }
            },
            match flanking.trigger_multiplicity {
                FlankingTriggerMultiplicity::OncePerAbilityOccurrencePerQualifyingBlockingCreature => {
                    "once-per-ability-occurrence-per-qualifying-blocking-creature/v1".into()
                }
            },
            if flanking.instances_trigger_separately {
                "instances-trigger-separately/true".into()
            } else {
                "instances-trigger-separately/false".into()
            },
            match flanking.resolution_recipient {
                FlankingEffectRecipient::BlockingCreatureIncarnationThatCausedTrigger => {
                    "blocking-creature-incarnation-that-caused-trigger/v1".into()
                }
            },
            if flanking.uses_targeting_system {
                "uses-targeting-system/true".into()
            } else {
                "uses-targeting-system/false".into()
            },
            format!("power-delta/{}", flanking.power_delta),
            format!("toughness-delta/{}", flanking.toughness_delta),
            match flanking.duration {
                FlankingEffectDuration::UntilEndOfTurn => "duration-until-end-of-turn/v1".into(),
            },
        ],
        KeywordProgramKind::Persist(death_return)
        | KeywordProgramKind::Undying(death_return) => vec![
            "death-return-program/v1".into(),
            if death_return.is_triggered_ability {
                "is-triggered-ability/true".into()
            } else {
                "is-triggered-ability/false".into()
            },
            match death_return.trigger_transition {
                DeathReturnTriggerTransition::BattlefieldPermanentPutIntoGraveyard => {
                    "battlefield-permanent-put-into-graveyard/v1".into()
                }
            },
            match death_return.prohibited_counter {
                DeathReturnCounterKind::MinusOneMinusOne => {
                    "prohibited-counter-minus-one-minus-one/v1".into()
                }
                DeathReturnCounterKind::PlusOnePlusOne => {
                    "prohibited-counter-plus-one-plus-one/v1".into()
                }
            },
            match death_return.counter_condition {
                DeathReturnCounterCondition::NoCounterOfKindImmediatelyBeforeDeathUsingLastKnownInformation => {
                    "counter-condition-immediately-before-death-using-last-known-information/v1"
                        .into()
                }
            },
            match death_return.trigger_multiplicity {
                DeathReturnTriggerMultiplicity::OncePerAbilityOccurrencePerQualifyingDeath => {
                    "once-per-ability-occurrence-per-qualifying-death/v1".into()
                }
            },
            if death_return.trigger_uses_stack_at_next_priority {
                "trigger-uses-stack-at-next-priority/true".into()
            } else {
                "trigger-uses-stack-at-next-priority/false".into()
            },
            if death_return.instances_trigger_separately {
                "instances-trigger-separately/true".into()
            } else {
                "instances-trigger-separately/false".into()
            },
            match death_return.linked_card {
                DeathReturnCardIdentity::NewPublicGraveyardObjectFromTriggeringZoneChange => {
                    "new-public-graveyard-object-from-triggering-zone-change/v1".into()
                }
            },
            match death_return.resolution_requirement {
                DeathReturnResolutionRequirement::LinkedCardRemainsInFirstGraveyard => {
                    "linked-card-remains-in-first-graveyard/v1".into()
                }
            },
            match death_return.return_under {
                DeathReturnBattlefieldController::Owner => "return-under-owner-control/v1".into(),
            },
            match death_return.return_counter {
                DeathReturnCounterKind::MinusOneMinusOne => {
                    "return-counter-minus-one-minus-one/v1".into()
                }
                DeathReturnCounterKind::PlusOnePlusOne => {
                    "return-counter-plus-one-plus-one/v1".into()
                }
            },
            if death_return.return_creates_new_battlefield_object {
                "return-creates-new-battlefield-object/true".into()
            } else {
                "return-creates-new-battlefield-object/false".into()
            },
            match death_return.token_interaction {
                DeathReturnTokenInteraction::TriggerMayExistButTokenCeasesBeforeResolution => {
                    "token-trigger-may-exist-but-token-ceases-before-resolution/v1".into()
                }
            },
            match death_return.replacement_interaction {
                DeathReturnReplacementInteraction::ReplacedGraveyardMoveDoesNotTrigger => {
                    "replaced-graveyard-move-does-not-trigger/v1".into()
                }
            },
        ],
        KeywordProgramKind::Toxic(toxic) => vec![
            "toxic-program/v1".into(),
            format!("amount/{}", toxic.amount),
            if toxic.is_static_ability {
                "is-static-ability/true".into()
            } else {
                "is-static-ability/false".into()
            },
            match toxic.damage_event {
                ToxicDamageEvent::CombatDamageDealtToPlayerByCreature => {
                    "combat-damage-dealt-to-player-by-creature/v1".into()
                }
            },
            if toxic.actual_damage_required {
                "actual-damage-required/true".into()
            } else {
                "actual-damage-required/false".into()
            },
            match toxic.value_combination {
                ToxicValueCombination::SumAllToxicAbilityValues => {
                    "sum-all-toxic-ability-values/v1".into()
                }
            },
            match toxic.poison_application {
                ToxicPoisonApplication::SourceControllerGivesDamagedPlayerCountersInDamageTransaction => {
                    "source-controller-gives-damaged-player-counters-in-damage-transaction/v1"
                        .into()
                }
            },
            if toxic.poison_counters_equal_total_toxic_value {
                "poison-counters-equal-total-toxic-value/true".into()
            } else {
                "poison-counters-equal-total-toxic-value/false".into()
            },
            if toxic.poison_is_in_addition_to_other_damage_results {
                "poison-in-addition-to-other-damage-results/true".into()
            } else {
                "poison-in-addition-to-other-damage-results/false".into()
            },
        ],
        KeywordProgramKind::Daybound(day_night)
        | KeywordProgramKind::Nightbound(day_night) => vec![
            "day-night-program/v1".into(),
            if day_night.is_static_ability {
                "is-static-ability/true".into()
            } else {
                "is-static-ability/false".into()
            },
            match day_night.face_role {
                DayNightFaceRole::DayboundFrontFace => "daybound-front-face/v1".into(),
                DayNightFaceRole::NightboundBackFace => "nightbound-back-face/v1".into(),
            },
            match day_night.global_lifecycle {
                DayNightGlobalLifecycle::SinglePersistentMutuallyExclusiveGameDesignationInitiallyNeither => {
                    "single-persistent-mutually-exclusive-game-designation-initially-neither/v1"
                        .into()
                }
            },
            match day_night.initial_designation {
                DayNightInitialDesignation::DayWhenDayboundPermanentIsControlledWhileNeither => {
                    "initial-day-when-daybound-permanent-is-controlled-while-neither/v1".into()
                }
                DayNightInitialDesignation::NightWhenNightboundPermanentIsControlledWhileNeitherAndNoDayboundPermanentExists => {
                    "initial-night-when-nightbound-permanent-is-controlled-while-neither-and-no-daybound-permanent-exists/v1".into()
                }
            },
            if day_night
                .spell_count_transition
                .check_during_second_part_of_untap_step
            {
                "spell-count-check-during-second-part-of-untap-step/true".into()
            } else {
                "spell-count-check-during-second-part-of-untap-step/false".into()
            },
            if day_night
                .spell_count_transition
                .inspect_previous_active_player_turn
            {
                "inspect-previous-active-player-turn/true".into()
            } else {
                "inspect-previous-active-player-turn/false".into()
            },
            if day_night
                .spell_count_transition
                .day_to_night_when_zero_spells
            {
                "day-to-night-when-zero-spells/true".into()
            } else {
                "day-to-night-when-zero-spells/false".into()
            },
            format!(
                "night-to-day-minimum-spells/{}",
                day_night
                    .spell_count_transition
                    .night_to_day_minimum_spells
            ),
            if day_night
                .spell_count_transition
                .neither_designation_skips_check
            {
                "neither-designation-skips-check/true".into()
            } else {
                "neither-designation-skips-check/false".into()
            },
            match day_night.spell_count_transition.shared_team_turn_rule {
                DayNightSharedTeamSpellCountRule::DayToNightIfTeamCastNoneAndNightToDayIfAnyOneTeamPlayerCastAtLeastTwo => {
                    "shared-team-day-to-night-if-team-cast-none-and-night-to-day-if-any-one-team-player-cast-at-least-two/v1".into()
                }
            },
            match day_night.entry_behavior {
                DayNightEntryBehavior::EnterTransformedAtNightWhenRepresentedByDoubleFacedCard => {
                    "enter-transformed-at-night-when-represented-by-double-faced-card/v1".into()
                }
                DayNightEntryBehavior::NoEntryModification => "no-entry-modification/v1".into(),
            },
            match day_night.invalid_entry_destination {
                Some(
                    DayNightInvalidEntryDestination::InstantOrSorceryBackFaceKeepsNonstackCardInPriorZoneOrPutsResolvingSpellIntoOwnersGraveyard,
                ) => "instant-or-sorcery-back-face-keeps-nonstack-card-in-prior-zone-or-puts-resolving-spell-into-owners-graveyard/v1".into(),
                None => "no-invalid-entry-destination-rule/v1".into(),
            },
            match day_night.immediate_alignment {
                DayNightImmediateAlignment::TransformFrontFaceUpPermanentAtNight => {
                    "transform-front-face-up-permanent-at-night/v1".into()
                }
                DayNightImmediateAlignment::TransformBackFaceUpPermanentAtDay => {
                    "transform-back-face-up-permanent-at-day/v1".into()
                }
            },
            match day_night.designation_transform {
                DayNightDesignationTransform::FrontToBackAsItBecomesNight => {
                    "front-to-back-as-it-becomes-night/v1".into()
                }
                DayNightDesignationTransform::BackToFrontAsItBecomesDay => {
                    "back-to-front-as-it-becomes-day/v1".into()
                }
            },
            match day_night.zone_scope {
                DayNightZoneScope::EntryModificationWhileEnteringAndOtherAbilitiesOnBattlefield => {
                    "entry-modification-while-entering-and-other-abilities-on-battlefield/v1".into()
                }
                DayNightZoneScope::BattlefieldOnly => "battlefield-only/v1".into(),
            },
            match day_night.transform_batch {
                DayNightTransformBatch::AllEligibleBattlefieldPermanentsSimultaneously => {
                    "all-eligible-battlefield-permanents-simultaneously/v1".into()
                }
            },
            if day_night.transform_requires_double_faced_card_or_token {
                "transform-requires-double-faced-card-or-token/true".into()
            } else {
                "transform-requires-double-faced-card-or-token/false".into()
            },
            if day_night.transform_instruction_rejects_instant_or_sorcery_destination {
                "transform-instruction-rejects-instant-or-sorcery-destination/true".into()
            } else {
                "transform-instruction-rejects-instant-or-sorcery-destination/false".into()
            },
            if day_night.transform_preserves_object_identity {
                "transform-preserves-object-identity/true".into()
            } else {
                "transform-preserves-object-identity/false".into()
            },
            if day_night.other_transform_causes_are_prohibited {
                "other-transform-causes-are-prohibited/true".into()
            } else {
                "other-transform-causes-are-prohibited/false".into()
            },
            if day_night.instances_are_redundant {
                "instances-are-redundant/true".into()
            } else {
                "instances-are-redundant/false".into()
            },
        ],
        KeywordProgramKind::StartYourEngines(speed) => vec![
            "start-your-engines-program/v1".into(),
            if speed.is_static_ability {
                "is-static-ability/true".into()
            } else {
                "is-static-ability/false".into()
            },
            match speed.source_scope {
                SpeedSourceScope::ControlledPermanentOnBattlefield => {
                    "controlled-permanent-on-battlefield/v1".into()
                }
            },
            match speed.initialization {
                SpeedInitialization::NoSpeedToOneAsStateBasedAction => {
                    "no-speed-to-one-as-state-based-action/v1".into()
                }
            },
            format!("initial-speed/{}", speed.initial_speed),
            if speed.speed_is_absent_until_set {
                "speed-is-absent-until-set/true".into()
            } else {
                "speed-is-absent-until-set/false".into()
            },
            if speed.inherent_trigger_has_no_source {
                "inherent-trigger-has-no-source/true".into()
            } else {
                "inherent-trigger-has-no-source/false".into()
            },
            if speed.inherent_trigger_is_controlled_by_player {
                "inherent-trigger-is-controlled-by-player/true".into()
            } else {
                "inherent-trigger-is-controlled-by-player/false".into()
            },
            if speed.inherent_trigger_uses_stack_at_next_priority {
                "inherent-trigger-uses-stack-at-next-priority/true".into()
            } else {
                "inherent-trigger-uses-stack-at-next-priority/false".into()
            },
            match speed.increase_event {
                SpeedIncreaseEvent::OneOrMoreOpponentsLoseLifeDuringControllersTurn => {
                    "one-or-more-opponents-lose-life-during-controllers-turn/v1".into()
                }
            },
            match speed.increase_limit {
                SpeedIncreaseLimit::OncePerControllerTurn => {
                    "increase-once-per-controller-turn/v1".into()
                }
            },
            if speed.increase_requires_current_speed_below_maximum {
                "increase-requires-current-speed-below-maximum/true".into()
            } else {
                "increase-requires-current-speed-below-maximum/false".into()
            },
            format!("increase-amount/{}", speed.increase_amount),
            if speed.increase_instruction_from_no_speed_sets_to_requested_value {
                "increase-instruction-from-no-speed-sets-to-requested-value/true".into()
            } else {
                "increase-instruction-from-no-speed-sets-to-requested-value/false".into()
            },
            format!("maximum-speed/{}", speed.maximum_speed),
            match speed.persistence {
                SpeedPersistence::PlayerRetainsDesignationAfterSourceLeaves => {
                    "player-retains-designation-after-source-leaves/v1".into()
                }
            },
            if speed.no_speed_reads_as_zero_for_effects {
                "no-speed-reads-as-zero-for-effects/true".into()
            } else {
                "no-speed-reads-as-zero-for-effects/false".into()
            },
            if speed.instances_are_redundant {
                "instances-are-redundant/true".into()
            } else {
                "instances-are-redundant/false".into()
            },
        ],
        KeywordProgramKind::ChooseABackground(partner)
        | KeywordProgramKind::DoctorsCompanion(partner) => vec![
            "commander-partner-program/v1".into(),
            match partner.variant {
                CommanderPartnerVariant::ChooseABackground => {
                    "choose-a-background-variant/v1".into()
                }
                CommanderPartnerVariant::DoctorsCompanion => {
                    "doctors-companion-variant/v1".into()
                }
            },
            if partner.functions_only_before_game_for_deck_construction {
                "functions-only-before-game-for-deck-construction/true".into()
            } else {
                "functions-only-before-game-for-deck-construction/false".into()
            },
            match partner.source_requirement {
                CommanderPartnerSourceRequirement::DistinctLegendaryCardWithThisAbility => {
                    "distinct-legendary-card-with-this-ability/v1".into()
                }
                CommanderPartnerSourceRequirement::DistinctLegendaryCreatureCardWithThisAbility => {
                    "distinct-legendary-creature-card-with-this-ability/v1".into()
                }
            },
            match partner.counterpart_requirement {
                CommanderPartnerCounterpartRequirement::LegendaryBackgroundEnchantmentCard => {
                    "legendary-background-enchantment-card/v1".into()
                }
                CommanderPartnerCounterpartRequirement::LegendaryTimeLordDoctorCreatureCardWithNoOtherCreatureTypes => {
                    "legendary-time-lord-doctor-creature-card-with-no-other-creature-types/v1"
                        .into()
                }
            },
            if partner.counterpart_needs_same_partner_ability {
                "counterpart-needs-same-partner-ability/true".into()
            } else {
                "counterpart-needs-same-partner-ability/false".into()
            },
            format!(
                "commander-count-when-used/{}",
                partner.commander_count_when_used
            ),
            format!(
                "maximum-commanders-from-partner-abilities/{}",
                partner.maximum_commanders_from_partner_abilities
            ),
            format!(
                "deck-card-count-including-commanders/{}",
                partner.deck_card_count_including_commanders
            ),
            if partner.both_commanders_start_in_command_zone {
                "both-commanders-start-in-command-zone/true".into()
            } else {
                "both-commanders-start-in-command-zone/false".into()
            },
            if partner.commander_designation_persists_across_zones {
                "commander-designation-persists-across-zones/true".into()
            } else {
                "commander-designation-persists-across-zones/false".into()
            },
            if partner.combined_color_identity_for_deck_construction_and_references {
                "combined-color-identity-for-deck-construction-and-references/true".into()
            } else {
                "combined-color-identity-for-deck-construction-and-references/false".into()
            },
            match partner.independent_tracking {
                CommanderPartnerTracking::SeparateCastCountsTaxAndCombatDamagePerCommanderPerDamagedPlayer => {
                    "separate-cast-counts-tax-and-combat-damage-per-commander-per-damaged-player/v1"
                        .into()
                }
            },
            match partner.commander_reference {
                CommanderPartnerReference::EitherCommanderAndAffectedPlayerChoosesOneWhenBothCouldBeAffected => {
                    "either-commander-and-affected-player-chooses-one-when-both-could-be-affected/v1"
                        .into()
                }
            },
            if partner.different_partner_variants_cannot_combine {
                "different-partner-variants-cannot-combine/true".into()
            } else {
                "different-partner-variants-cannot-combine/false".into()
            },
            if partner.choose_only_one_when_source_has_multiple_partner_abilities {
                "choose-only-one-when-source-has-multiple-partner-abilities/true".into()
            } else {
                "choose-only-one-when-source-has-multiple-partner-abilities/false".into()
            },
        ],
        KeywordProgramKind::Exploit(exploit) => vec![
            "exploit-program/v1".into(),
            match exploit.trigger_transition {
                ExploitTriggerTransition::SourceCreatureEntersBattlefield => {
                    "source-creature-enters-battlefield/v1".into()
                }
            },
            bool_semantic_component("trigger-uses-stack", exploit.trigger_uses_stack),
            bool_semantic_component(
                "trigger-controller-is-source-controller-at-trigger-time",
                exploit.trigger_controller_is_source_controller_at_trigger_time,
            ),
            match exploit.sacrifice_choice {
                ExploitSacrificeChoice::OptionalOneCreatureControlledByAbilityControllerOnResolution => {
                    "optional-one-creature-controlled-by-ability-controller-on-resolution/v1".into()
                }
            },
            bool_semantic_component(
                "sacrifice-uses-targeting",
                exploit.sacrifice_uses_targeting,
            ),
            bool_semantic_component(
                "source-may-be-chosen-for-sacrifice",
                exploit.source_may_be_chosen_for_sacrifice,
            ),
            bool_semantic_component(
                "source-need-not-remain-on-battlefield-for-resolution",
                exploit.source_need_not_remain_on_battlefield_for_resolution,
            ),
            bool_semantic_component(
                "sacrifice-moves-controlled-permanent-from-battlefield-to-owners-graveyard",
                exploit.sacrifice_moves_controlled_permanent_from_battlefield_to_owners_graveyard,
            ),
            bool_semantic_component(
                "sacrifice-destination-is-subject-to-zone-change-replacement",
                exploit.sacrifice_destination_is_subject_to_zone_change_replacement,
            ),
            bool_semantic_component(
                "sacrifice-is-not-destruction",
                exploit.sacrifice_is_not_destruction,
            ),
            match exploit.exploit_event {
                ExploitEventDefinition::SourceExploitsChosenCreatureWhenControllerSacrificesItDuringThisResolution => {
                    "source-exploits-chosen-creature-when-controller-sacrifices-it-during-this-resolution/v1".into()
                }
            },
            bool_semantic_component(
                "exploit-event-requires-completed-sacrifice-action",
                exploit.exploit_event_requires_completed_sacrifice_action,
            ),
            bool_semantic_component(
                "instances-trigger-separately",
                exploit.instances_trigger_separately,
            ),
        ],
        KeywordProgramKind::Soulbond(soulbond) => vec![
            "soulbond-program/v1".into(),
            bool_semantic_component(
                "represents-two-triggered-abilities",
                soulbond.represents_two_triggered_abilities,
            ),
            match soulbond.trigger_set {
                SoulbondTriggerSet::SourceEntersOrAnotherCreatureControlledBySourceControllerEnters => {
                    "source-enters-or-another-creature-controlled-by-source-controller-enters/v1"
                        .into()
                }
            },
            bool_semantic_component("trigger-uses-stack", soulbond.trigger_uses_stack),
            bool_semantic_component(
                "trigger-controller-is-source-controller-at-trigger-time",
                soulbond.trigger_controller_is_source_controller_at_trigger_time,
            ),
            match soulbond.eligibility {
                SoulbondEligibility::BothObjectsAreUnpairedCreaturesOnBattlefieldControlledByAbilityControllerAtTriggerAndResolution => {
                    "both-objects-are-unpaired-creatures-on-battlefield-controlled-by-ability-controller-at-trigger-and-resolution/v1".into()
                }
            },
            bool_semantic_component(
                "source-entry-chooses-another-eligible-creature",
                soulbond.source_entry_chooses_another_eligible_creature,
            ),
            bool_semantic_component(
                "other-entry-is-bound-to-that-entering-creature",
                soulbond.other_entry_is_bound_to_that_entering_creature,
            ),
            bool_semantic_component(
                "simultaneous-other-entries-each-create-their-own-trigger",
                soulbond.simultaneous_other_entries_each_create_their_own_trigger,
            ),
            match soulbond.pair_choice {
                SoulbondPairChoice::OptionalNontargetedChoiceOnResolution => {
                    "optional-nontargeted-choice-on-resolution/v1".into()
                }
            },
            match soulbond.pair_lifecycle {
                SoulbondPairLifecycle::SymmetricExclusivePairWhileBothRemainCreaturesOnBattlefieldUnderSameController => {
                    "symmetric-exclusive-pair-while-both-remain-creatures-on-battlefield-under-same-controller/v1".into()
                }
            },
            format!(
                "maximum-partners-per-creature/{}",
                soulbond.maximum_partners_per_creature
            ),
            match soulbond.unpair_transition {
                SoulbondUnpairTransition::EitherLeavesBattlefieldStopsBeingCreatureOrChangesController => {
                    "either-leaves-battlefield-stops-being-creature-or-changes-controller/v1".into()
                }
            },
            bool_semantic_component(
                "teammate-or-opponent-creatures-are-ineligible",
                soulbond.teammate_or_opponent_creatures_are_ineligible,
            ),
            bool_semantic_component(
                "instances-trigger-separately",
                soulbond.instances_trigger_separately,
            ),
        ],
        KeywordProgramKind::Evolve(evolve) => vec![
            "evolve-program/v1".into(),
            match evolve.trigger_transition {
                EvolveTriggerTransition::CreatureControlledBySourceControllerEntersBattlefield => {
                    "creature-controlled-by-source-controller-enters-battlefield/v1".into()
                }
            },
            bool_semantic_component("trigger-uses-stack", evolve.trigger_uses_stack),
            bool_semantic_component(
                "trigger-controller-is-source-controller-at-trigger-time",
                evolve.trigger_controller_is_source_controller_at_trigger_time,
            ),
            bool_semantic_component(
                "uses-intervening-if-at-trigger-and-resolution",
                evolve.uses_intervening_if_at_trigger_and_resolution,
            ),
            match evolve.comparison {
                EvolveComparison::EnteringPowerGreaterOrEnteringToughnessGreaterThanSource => {
                    "entering-power-greater-or-entering-toughness-greater-than-source/v1".into()
                }
            },
            bool_semantic_component(
                "compares-effective-power-and-toughness",
                evolve.compares_effective_power_and_toughness,
            ),
            match evolve.information_rule {
                EvolveInformationRule::CurrentInformationOrLastKnownInformationForDepartedEnteringCreature => {
                    "current-information-or-last-known-information-for-departed-entering-creature/v1".into()
                }
            },
            bool_semantic_component(
                "comparison-is-false-against-noncreature-permanent",
                evolve.comparison_is_false_against_noncreature_permanent,
            ),
            bool_semantic_component(
                "counter-recipient-is-source-incarnation-on-battlefield",
                evolve.counter_recipient_is_source_incarnation_on_battlefield,
            ),
            format!(
                "plus-one-plus-one-counters-per-resolution/{}",
                evolve.plus_one_plus_one_counters_per_resolution
            ),
            match evolve.evolve_event {
                EvolveEventDefinition::OneOrMorePlusOnePlusOneCountersPlacedByResolvingEvolveAbility => {
                    "one-or-more-plus-one-plus-one-counters-placed-by-resolving-evolve-ability/v1"
                        .into()
                }
            },
            bool_semantic_component("uses-targeting", evolve.uses_targeting),
            bool_semantic_component(
                "simultaneous-entries-each-create-their-own-trigger",
                evolve.simultaneous_entries_each_create_their_own_trigger,
            ),
            bool_semantic_component(
                "instances-trigger-separately",
                evolve.instances_trigger_separately,
            ),
        ],
        KeywordProgramKind::Improvise(improvise) => vec![
            "improvise-program/v1".into(),
            bool_semantic_component("is-static-ability", improvise.is_static_ability),
            match improvise.function_zone {
                ImproviseFunctionZone::SpellStackOnly => "spell-stack-only/v1".into(),
            },
            match improvise.payment_timing {
                ImprovisePaymentTiming::AfterTotalCostLockedAndManaAbilitiesActivatedDuringCostPayment => {
                    "after-total-cost-locked-and-mana-abilities-activated-during-cost-payment/v1"
                        .into()
                }
            },
            match improvise.payment_exchange {
                ImprovisePaymentExchange::TapOneUntappedControlledArtifactForOneGenericMana => {
                    "tap-one-untapped-controlled-artifact-for-one-generic-mana/v1".into()
                }
            },
            bool_semantic_component(
                "applies-only-to-generic-mana-in-locked-total-cost",
                improvise.applies_only_to_generic_mana_in_locked_total_cost,
            ),
            bool_semantic_component(
                "is-not-additional-or-alternative-cost",
                improvise.is_not_additional_or_alternative_cost,
            ),
            bool_semantic_component("is-not-cost-reduction", improvise.is_not_cost_reduction),
            bool_semantic_component(
                "payment-is-optional-for-each-generic-mana",
                improvise.payment_is_optional_for_each_generic_mana,
            ),
            bool_semantic_component(
                "tapped-or-uncontrolled-artifacts-are-ineligible",
                improvise.tapped_or_uncontrolled_artifacts_are_ineligible,
            ),
            bool_semantic_component(
                "summoning-sickness-does-not-prevent-artifact-payment",
                improvise.summoning_sickness_does_not_prevent_artifact_payment,
            ),
            bool_semantic_component(
                "one-artifact-cannot-pay-more-than-once",
                improvise.one_artifact_cannot_pay_more_than_once,
            ),
            bool_semantic_component(
                "instances-are-redundant",
                improvise.instances_are_redundant,
            ),
        ],
        KeywordProgramKind::Intimidate(intimidate) => vec![
            "intimidate-program/v1".into(),
            bool_semantic_component(
                "is-static-evasion-ability",
                intimidate.is_static_evasion_ability,
            ),
            match intimidate.blocker_qualification {
                IntimidateBlockerQualification::ArtifactCreatureOrCreatureSharingAtLeastOneCurrentColorWithAttacker => {
                    "artifact-creature-or-creature-sharing-at-least-one-current-color-with-attacker/v1".into()
                }
            },
            bool_semantic_component(
                "every-declared-blocker-must-individually-qualify",
                intimidate.every_declared_blocker_must_individually_qualify,
            ),
            bool_semantic_component(
                "colorless-attacker-requires-artifact-blocker",
                intimidate.colorless_attacker_requires_artifact_blocker,
            ),
            bool_semantic_component(
                "checks-current-characteristics-during-block-declaration",
                intimidate.checks_current_characteristics_during_block_declaration,
            ),
            bool_semantic_component(
                "gain-or-loss-after-legal-declaration-does-not-change-block",
                intimidate.gain_or_loss_after_legal_declaration_does_not_change_block,
            ),
            bool_semantic_component(
                "later-attacker-or-blocker-characteristic-changes-do-not-change-block",
                intimidate.later_attacker_or_blocker_characteristic_changes_do_not_change_block,
            ),
            bool_semantic_component(
                "composes-with-other-block-restrictions",
                intimidate.composes_with_other_block_restrictions,
            ),
            bool_semantic_component(
                "instances-are-redundant",
                intimidate.instances_are_redundant,
            ),
        ],
        KeywordProgramKind::Spree(spree) => vec![
            "spree-program/v1".into(),
            bool_semantic_component("is-static-ability", spree.is_static_ability),
            match spree.function_zone {
                SpreeFunctionZone::ModalSpellOnStack => "modal-spell-on-stack/v1".into(),
            },
            match spree.mode_choice {
                SpreeModeChoice::ControllerChoosesOneOrMoreLegalModesWhileCasting => {
                    "controller-chooses-one-or-more-legal-modes-while-casting/v1".into()
                }
            },
            bool_semantic_component("choose-modes-before-targets", spree.choose_modes_before_targets),
            bool_semantic_component(
                "chosen-mode-must-have-legal-required-targets",
                spree.chosen_mode_must_have_legal_required_targets,
            ),
            bool_semantic_component(
                "same-mode-normally-cannot-be-chosen-more-than-once",
                spree.same_mode_normally_cannot_be_chosen_more_than_once,
            ),
            bool_semantic_component(
                "retargeting-does-not-change-modes",
                spree.retargeting_does_not_change_modes,
            ),
            bool_semantic_component(
                "spell-copy-retains-chosen-modes-without-new-choice",
                spree.spell_copy_retains_chosen_modes_without_new_choice,
            ),
            bool_semantic_component(
                "chosen-modes-resolve-in-printed-order",
                spree.chosen_modes_resolve_in_printed_order,
            ),
            match spree.mode_cost_binding {
                SpreeModeCostBinding::EveryChosenModeRequiresItsAssociatedPrintedAdditionalCost => {
                    "every-chosen-mode-requires-its-associated-printed-additional-cost/v1".into()
                }
            },
            bool_semantic_component(
                "all-chosen-mode-costs-are-additional-costs",
                spree.all_chosen_mode_costs_are_additional_costs,
            ),
            bool_semantic_component(
                "all-chosen-mode-costs-must-be-paid-without-partial-payment",
                spree.all_chosen_mode_costs_must_be_paid_without_partial_payment,
            ),
            bool_semantic_component(
                "mode-costs-do-not-change-mana-cost",
                spree.mode_costs_do_not_change_mana_cost,
            ),
            bool_semantic_component(
                "requires-exact-associated-mode-table-from-source",
                spree.requires_exact_associated_mode_table_from_source,
            ),
            bool_semantic_component(
                "plus-sign-icons-have-no-rules-meaning",
                spree.plus_sign_icons_have_no_rules_meaning,
            ),
        ],
        KeywordProgramKind::Bargain(bargain) => vec![
            "bargain-program/v1".into(),
            bool_semantic_component(
                "is-static-ability-on-spell-stack",
                bargain.is_static_ability_on_spell_stack,
            ),
            bool_semantic_component(
                "is-optional-additional-cost",
                bargain.is_optional_additional_cost,
            ),
            match bargain.sacrifice_choice {
                BargainSacrificeChoice::OneControlledArtifactEnchantmentOrTokenPermanent => {
                    "one-controlled-artifact-enchantment-or-token-permanent/v1".into()
                }
            },
            bool_semantic_component(
                "sacrifice-is-declared-before-targets",
                bargain.sacrifice_is_declared_before_targets,
            ),
            bool_semantic_component(
                "sacrifice-is-paid-with-total-cost",
                bargain.sacrifice_is_paid_with_total_cost,
            ),
            bool_semantic_component(
                "bargain-does-not-change-mana-cost",
                bargain.bargain_does_not_change_mana_cost,
            ),
            bool_semantic_component(
                "bargained-status-is-set-when-intention-is-declared",
                bargain.bargained_status_is_set_when_intention_is_declared,
            ),
            bool_semantic_component(
                "casting-must-later-pay-declared-cost-to-complete",
                bargain.casting_must_later_pay_declared_cost_to_complete,
            ),
            bool_semantic_component(
                "linked-effects-reference-only-this-printed-bargain-ability",
                bargain.linked_effects_reference_only_this_printed_bargain_ability,
            ),
            bool_semantic_component(
                "conditional-targets-are-chosen-only-when-bargained",
                bargain.conditional_targets_are_chosen_only_when_bargained,
            ),
            bool_semantic_component(
                "cost-can-be-paid-at-most-once",
                bargain.cost_can_be_paid_at_most_once,
            ),
        ],
        KeywordProgramKind::Mentor(mentor) => vec![
            "mentor-program/v1".into(),
            match mentor.trigger_transition {
                MentorTriggerTransition::SourceCreatureDeclaredAsAttacker => {
                    "source-creature-declared-as-attacker/v1".into()
                }
            },
            bool_semantic_component("trigger-uses-stack", mentor.trigger_uses_stack),
            match mentor.target_restriction {
                MentorTargetRestriction::AttackingCreatureWithCurrentPowerLessThanSourceCurrentPower => {
                    "attacking-creature-with-current-power-less-than-source-current-power/v1".into()
                }
            },
            bool_semantic_component(
                "restriction-checked-on-target-selection-and-resolution",
                mentor.restriction_checked_on_target_selection_and_resolution,
            ),
            bool_semantic_component(
                "source-and-target-use-current-power",
                mentor.source_and_target_use_current_power,
            ),
            format!(
                "plus-one-plus-one-counters/{}",
                mentor.plus_one_plus_one_counters
            ),
            bool_semantic_component(
                "counter-is-placed-on-legal-target-on-resolution",
                mentor.counter_is_placed_on_legal_target_on_resolution,
            ),
            bool_semantic_component(
                "mentor-event-occurs-when-ability-resolves",
                mentor.mentor_event_occurs_when_ability_resolves,
            ),
            bool_semantic_component(
                "instances-trigger-separately",
                mentor.instances_trigger_separately,
            ),
        ],
        KeywordProgramKind::Extort(extort) => vec![
            "extort-program/v1".into(),
            match extort.trigger_transition {
                ExtortTriggerTransition::ControllerCastsSpell => {
                    "controller-casts-spell/v1".into()
                }
            },
            bool_semantic_component("trigger-uses-stack", extort.trigger_uses_stack),
            bool_semantic_component(
                "optional-hybrid-white-black-payment-on-resolution",
                extort.optional_hybrid_white_black_payment_on_resolution,
            ),
            bool_semantic_component(
                "payment-may-be-made-at-most-once-per-trigger",
                extort.payment_may_be_made_at_most_once_per_trigger,
            ),
            format!(
                "each-opponent-loses-life-simultaneously/{}",
                extort.each_opponent_loses_life_simultaneously
            ),
            bool_semantic_component(
                "controller-gains-life-equal-to-total-life-actually-lost",
                extort.controller_gains_life_equal_to_total_life_actually_lost,
            ),
            bool_semantic_component("uses-targeting", extort.uses_targeting),
            bool_semantic_component(
                "instances-trigger-separately",
                extort.instances_trigger_separately,
            ),
        ],
        KeywordProgramKind::LivingWeapon(living_weapon) => vec![
            "living-weapon-program/v1".into(),
            bool_semantic_component(
                "is-enters-battlefield-trigger",
                living_weapon.is_enters_battlefield_trigger,
            ),
            bool_semantic_component("trigger-uses-stack", living_weapon.trigger_uses_stack),
            match living_weapon.token {
                LivingWeaponTokenDefinition::ZeroZeroBlackPhyrexianGermCreature => {
                    "zero-zero-black-phyrexian-germ-creature/v1".into()
                }
            },
            format!("token-count/{}", living_weapon.token_count),
            bool_semantic_component(
                "token-creation-precedes-attachment",
                living_weapon.token_creation_precedes_attachment,
            ),
            bool_semantic_component(
                "attach-source-equipment-to-created-token",
                living_weapon.attach_source_equipment_to_created_token,
            ),
            bool_semantic_component(
                "attachment-does-not-target",
                living_weapon.attachment_does_not_target,
            ),
            bool_semantic_component(
                "failed-or-illegal-attachment-leaves-equipment-unattached",
                living_weapon.failed_or_illegal_attachment_leaves_equipment_unattached,
            ),
        ],
        KeywordProgramKind::Myriad(myriad) => vec![
            "myriad-program/v1".into(),
            match myriad.trigger_transition {
                MyriadTriggerTransition::SourceCreatureDeclaredAsAttacker => {
                    "source-creature-declared-as-attacker/v1".into()
                }
            },
            bool_semantic_component("trigger-uses-stack", myriad.trigger_uses_stack),
            bool_semantic_component(
                "one-optional-copy-for-each-opponent-other-than-defending-player",
                myriad.one_optional_copy_for_each_opponent_other_than_defending_player,
            ),
            bool_semantic_component(
                "copy-is-token-with-source-copiable-values",
                myriad.copy_is_token_with_source_copiable_values,
            ),
            bool_semantic_component(
                "token-enters-tapped-and-attacking",
                myriad.token_enters_tapped_and_attacking,
            ),
            bool_semantic_component(
                "token-controller-chooses-that-opponent-or-their-planeswalker",
                myriad.token_controller_chooses_that_opponent_or_their_planeswalker,
            ),
            bool_semantic_component(
                "entering-attacking-does-not-trigger-declared-attacker-abilities",
                myriad.entering_attacking_does_not_trigger_declared_attacker_abilities,
            ),
            bool_semantic_component(
                "creates-delayed-end-of-combat-exile-trigger-when-any-token-was-created",
                myriad.creates_delayed_end_of_combat_exile_trigger_when_any_token_was_created,
            ),
            bool_semantic_component(
                "delayed-trigger-exiles-only-tokens-created-by-this-resolution",
                myriad.delayed_trigger_exiles_only_tokens_created_by_this_resolution,
            ),
            bool_semantic_component(
                "instances-trigger-separately",
                myriad.instances_trigger_separately,
            ),
        ],
        KeywordProgramKind::Retrace(retrace) => vec![
            "retrace-program/v1".into(),
            bool_semantic_component("is-static-ability", retrace.is_static_ability),
            match retrace.function_zone {
                RetraceFunctionZone::OwnersGraveyard => "owners-graveyard/v1".into(),
            },
            bool_semantic_component(
                "permits-casting-card-from-graveyard",
                retrace.permits_casting_card_from_graveyard,
            ),
            bool_semantic_component(
                "discard-one-land-card-is-additional-cost",
                retrace.discard_one_land_card_is_additional_cost,
            ),
            bool_semantic_component(
                "printed-and-other-costs-are-still-paid",
                retrace.printed_and_other_costs_are_still_paid,
            ),
            bool_semantic_component(
                "normal-casting-timing-and-restrictions-still-apply",
                retrace.normal_casting_timing_and_restrictions_still_apply,
            ),
            bool_semantic_component(
                "does-not-change-mana-cost",
                retrace.does_not_change_mana_cost,
            ),
        ],
        KeywordProgramKind::Backup(backup) => vec![
            "backup-program/v1".into(),
            format!("counter-count/{}", backup.counter_count),
            bool_semantic_component(
                "is-enters-battlefield-trigger",
                backup.is_enters_battlefield_trigger,
            ),
            bool_semantic_component("trigger-uses-stack", backup.trigger_uses_stack),
            bool_semantic_component("targets-one-creature", backup.targets_one_creature),
            bool_semantic_component(
                "places-plus-one-plus-one-counters-on-legal-target",
                backup.places_plus_one_plus_one_counters_on_legal_target,
            ),
            bool_semantic_component(
                "grants-abilities-only-if-target-is-another-creature",
                backup.grants_abilities_only_if_target_is_another_creature,
            ),
            match backup.granted_abilities {
                BackupGrantedAbilitySet::NonBackupAbilitiesPrintedBelowThisBackupAbility => {
                    "nonbackup-abilities-printed-below-this-backup-ability/v1".into()
                }
            },
            bool_semantic_component(
                "granted-abilities-last-until-end-of-turn",
                backup.granted_abilities_last_until_end_of_turn,
            ),
            bool_semantic_component(
                "printed-ability-order-is-copiable-and-preserved",
                backup.printed_ability_order_is_copiable_and_preserved,
            ),
            bool_semantic_component(
                "gained-or-created-abilities-are-not-granted",
                backup.gained_or_created_abilities_are_not_granted,
            ),
            bool_semantic_component(
                "granted-ability-set-is-fixed-when-trigger-enters-stack",
                backup.granted_ability_set_is_fixed_when_trigger_enters_stack,
            ),
        ],
        KeywordProgramKind::UmbraArmor(umbra) => vec![
            "umbra-armor-program/v1".into(),
            bool_semantic_component(
                "is-static-replacement-effect",
                umbra.is_static_replacement_effect,
            ),
            bool_semantic_component(
                "replaces-destruction-of-enchanted-permanent",
                umbra.replaces_destruction_of_enchanted_permanent,
            ),
            bool_semantic_component("replacement-is-mandatory", umbra.replacement_is_mandatory),
            bool_semantic_component(
                "removes-all-damage-marked-on-enchanted-permanent",
                umbra.removes_all_damage_marked_on_enchanted_permanent,
            ),
            bool_semantic_component("destroys-source-aura", umbra.destroys_source_aura),
            bool_semantic_component(
                "source-aura-is-destroyed-by-replacement-instruction",
                umbra.source_aura_is_destroyed_by_replacement_instruction,
            ),
            bool_semantic_component(
                "does-not-regenerate-enchanted-permanent",
                umbra.does_not_regenerate_enchanted_permanent,
            ),
            bool_semantic_component(
                "multiple-applicable-replacements-follow-replacement-choice-rules",
                umbra.multiple_applicable_replacements_follow_replacement_choice_rules,
            ),
        ],
        KeywordProgramKind::Cipher(cipher) => vec![
            "cipher-program/v1".into(),
            bool_semantic_component(
                "spell-ability-functions-on-stack",
                cipher.spell_ability_functions_on_stack,
            ),
            bool_semantic_component(
                "requires-spell-represented-by-card",
                cipher.requires_spell_represented_by_card,
            ),
            match cipher.encode_choice_on_resolution {
                CipherEncodeChoice::OptionalNontargetedCreatureControlledBySpellController => {
                    "optional-nontargeted-creature-controlled-by-spell-controller/v1".into()
                }
            },
            bool_semantic_component(
                "exiles-spell-card-encoded-on-chosen-creature",
                cipher.exiles_spell_card_encoded_on_chosen_creature,
            ),
            bool_semantic_component(
                "static-ability-functions-while-card-is-exiled",
                cipher.static_ability_functions_while_card_is_exiled,
            ),
            bool_semantic_component(
                "relationship-requires-card-in-exile-and-same-creature-object-on-battlefield",
                cipher.relationship_requires_card_in_exile_and_same_creature_object_on_battlefield,
            ),
            bool_semantic_component(
                "relationship-survives-creature-control-change-or-loss-of-creature-type",
                cipher.relationship_survives_creature_control_change_or_loss_of_creature_type,
            ),
            bool_semantic_component(
                "combat-damage-to-player-triggers-for-current-creature-controller",
                cipher.combat_damage_to_player_triggers_for_current_creature_controller,
            ),
            bool_semantic_component(
                "trigger-copies-encoded-card",
                cipher.trigger_copies_encoded_card,
            ),
            bool_semantic_component(
                "copied-card-may-be-cast-without-paying-mana-cost",
                cipher.copied_card_may_be_cast_without_paying_mana_cost,
            ),
            bool_semantic_component(
                "casting-copy-is-optional-and-obeys-other-casting-restrictions",
                cipher.casting_copy_is_optional_and_obeys_other_casting_restrictions,
            ),
            bool_semantic_component(
                "casting-copy-still-requires-additional-costs-and-cannot-use-another-alternative-cost",
                cipher
                    .casting_copy_still_requires_additional_costs_and_cannot_use_another_alternative_cost,
            ),
            bool_semantic_component(
                "spell-copy-without-a-card-cannot-be-encoded",
                cipher.spell_copy_without_a_card_cannot_be_encoded,
            ),
        ],
        KeywordProgramKind::Renown(renown) => vec![
            "renown-program/v1".into(),
            format!("counter-count/{}", renown.counter_count),
            bool_semantic_component(
                "triggers-on-combat-damage-to-player",
                renown.triggers_on_combat_damage_to_player,
            ),
            bool_semantic_component(
                "uses-intervening-if-not-renowned",
                renown.uses_intervening_if_not_renowned,
            ),
            bool_semantic_component("trigger-uses-stack", renown.trigger_uses_stack),
            bool_semantic_component(
                "puts-plus-one-plus-one-counters-on-source",
                renown.puts_plus_one_plus_one_counters_on_source,
            ),
            bool_semantic_component(
                "source-becomes-renowned-after-counter-instruction",
                renown.source_becomes_renowned_after_counter_instruction,
            ),
            bool_semantic_component(
                "renowned-is-persistent-battlefield-designation",
                renown.renowned_is_persistent_battlefield_designation,
            ),
            bool_semantic_component(
                "renowned-is-not-an-ability-or-copiable-value",
                renown.renowned_is_not_an_ability_or_copiable_value,
            ),
            bool_semantic_component(
                "designation-ends-when-permanent-leaves-battlefield",
                renown.designation_ends_when_permanent_leaves_battlefield,
            ),
            bool_semantic_component(
                "instances-trigger-separately-but-later-resolutions-do-nothing",
                renown.instances_trigger_separately_but_later_resolutions_do_nothing,
            ),
        ],
        _ => Vec::new(),
    }
}

fn bool_semantic_component(name: &str, value: bool) -> String {
    format!("{name}/{}", if value { "true" } else { "false" })
}

fn regeneration_program_semantic_components(
    regeneration: &RegenerationProgram,
) -> Vec<&'static str> {
    let mut components = Vec::with_capacity(12);
    components.push("regeneration-program/v1");
    components.push(match regeneration.replacement {
        RegenerationReplacement::NextDestructionThisTurn => {
            "replacement-next-destruction-this-turn/v1"
        }
        RegenerationReplacement::EveryDestructionWhileStaticEffectApplies => {
            "replacement-every-destruction-while-static-effect-applies/v1"
        }
    });
    match regeneration.recipients {
        RegenerationRecipientScope::SourcePermanent => {
            components.push("recipients-source-permanent/v1");
            components.push("cardinality-exactly-one/v1");
        }
        RegenerationRecipientScope::SingleTarget {
            filter,
            cardinality,
            selection_time,
        } => {
            components.push("recipients-single-target/v1");
            components.push(match filter {
                RegenerationTargetFilter::BattlefieldCreature => {
                    "target-filter-battlefield-creature/v1"
                }
                RegenerationTargetFilter::BattlefieldPermanent => {
                    "target-filter-battlefield-permanent/v1"
                }
            });
            components.push(match cardinality {
                RegenerationRecipientCardinality::ExactlyOne => "cardinality-exactly-one/v1",
                RegenerationRecipientCardinality::ZeroOrMore => "cardinality-zero-or-more/v1",
            });
            components.push(match selection_time {
                RegenerationRecipientSelectionTime::WhenSpellOrAbilityIsPutOnStack => {
                    "selection-when-spell-or-ability-is-put-on-stack/v1"
                }
                RegenerationRecipientSelectionTime::OnResolution => "selection-on-resolution/v1",
            });
        }
        RegenerationRecipientScope::EachCreatureControlledByEffectController {
            cardinality,
            selection_time,
        } => {
            components.push("recipients-each-creature-controlled-by-effect-controller/v1");
            components.push(match cardinality {
                RegenerationRecipientCardinality::ExactlyOne => "cardinality-exactly-one/v1",
                RegenerationRecipientCardinality::ZeroOrMore => "cardinality-zero-or-more/v1",
            });
            components.push(match selection_time {
                RegenerationRecipientSelectionTime::WhenSpellOrAbilityIsPutOnStack => {
                    "selection-when-spell-or-ability-is-put-on-stack/v1"
                }
                RegenerationRecipientSelectionTime::OnResolution => "selection-on-resolution/v1",
            });
        }
    }
    match regeneration.reminder {
        RegenerationReminderEvidence::Absent => components.push("reminder-absent/v1"),
        RegenerationReminderEvidence::CanonicalTargetCreature {
            referent,
            protection_window,
            removes_all_damage,
            controller_taps_recipient,
            removes_from_combat_if_attacking_or_blocking_creature,
        } => {
            components.push("reminder-canonical-target-creature/v1");
            components.push(match referent {
                RegenerationReminderReferent::SelectedTarget => {
                    "reminder-referent-selected-target/v1"
                }
            });
            components.push(match protection_window {
                RegenerationProtectionWindow::NextDestructionThisTurn => {
                    "reminder-window-next-destruction-this-turn/v1"
                }
            });
            components.push(if removes_all_damage {
                "reminder-removes-all-damage/true"
            } else {
                "reminder-removes-all-damage/false"
            });
            components.push(if controller_taps_recipient {
                "reminder-controller-taps-recipient/true"
            } else {
                "reminder-controller-taps-recipient/false"
            });
            components.push(if removes_from_combat_if_attacking_or_blocking_creature {
                "reminder-removes-from-combat-when-applicable/true"
            } else {
                "reminder-removes-from-combat-when-applicable/false"
            });
        }
    }
    components
}
