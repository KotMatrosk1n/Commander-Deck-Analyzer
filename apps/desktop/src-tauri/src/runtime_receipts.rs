//! Shared, fail-closed runtime capability receipts.
//!
//! A receipt is deterministic evidence that the live runtime accepted one
//! complete typed program. Card names never select behavior. They may appear
//! inside Oracle text only long enough for the ability compiler to normalize
//! self-reference before this module sees the program.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ability_program::{
    AbilityCompilation, AbilityCost, AbilityEffect, AbilityPrecondition, AbilityTiming,
    ActivationWindow, AlternativeSpellCostComponent, AtomicAdditionalCostTiming, AtomicBargainCost,
    AtomicCardNameReference, AtomicCastCostWaiver, AtomicCastPermissionCondition, AtomicCost,
    AtomicEffect, AtomicEffectDuration, AtomicGraveyardScope, AtomicInitiation,
    AtomicLibrarySearch, AtomicManaValueSubject, AtomicMovementCondition, AtomicSearchChooser,
    AtomicShuffleTiming, AtomicStateCondition, AtomicTrackedObject, AttachmentKind,
    BargainSacrificeKind, BargainSearchCastOrHandEffect, CardType as ProgramCardType,
    CommanderEligibility, ControllerRelation, ControllerStateCondition, CopyTargetChoice,
    CounterKind, CounterTarget, EXECUTABLE_ABILITY_PROGRAM_VERSION, EntryLinkedCardFilter,
    EntryLinkedManaOutput, ExecutableAbility, ExecutableGraveyardReclamation, FixedManaProfile,
    GrantedAbilityKind, GrantedSelfCost, GraveyardReclamationAllTargetsIllegalOutcome,
    GraveyardReclamationCastAction, GraveyardReclamationCopyCondition,
    GraveyardReclamationCopyTiming, GraveyardReclamationFlashbackCastStep,
    GraveyardReclamationObject, GraveyardReclamationPendingTriggerOrder,
    GraveyardReclamationResolutionInstruction, GraveyardReclamationResolutionOrder,
    GraveyardReclamationResolutionTargetLegality, GraveyardReclamationRetargetLegality,
    GraveyardReclamationRetargetTiming, GraveyardReclamationStackExitEvent,
    GraveyardReclamationStackPosition, GraveyardReclamationTarget,
    GraveyardReclamationTargetSelection, LinkedEntryObject, ManaCost as ProgramManaCost,
    ManaKind as ProgramManaKind, ObjectFilter as ProgramObjectFilter,
    OpponentChoiceSearchSplitEffect, OracleCardInput, PermanentEntryProcedure, PlayerSelector,
    RandomSelection, ReplacedSpellCost, ResourceKind, SpellCopyCount, SpellCostReductionCondition,
    SpellCostReductionScope, StaticCreatureModifierTarget, StaticModifierValue, TokenKind,
    TriggerEventKind, TutorEffect as ProgramTutorEffect, TutorFilter, Zone as ProgramZone,
    compile_executable_ability_program, normalize_oracle_clause_for_receipt,
};
use crate::alternative_cast_runtime::{AlternativeCastRuntimeProgram, CompiledAlternativeCast};
use crate::bounded_oracle_consumer::{
    BOUNDED_ORACLE_CONSUMER_VERSION, clause_has_executable_contract,
};
use crate::bounded_oracle_runtime::{
    BOUNDED_ORACLE_RUNTIME_VERSION, BoundedOracleClause, Effect as BoundedEffect,
    Restriction as BoundedRestriction, Timing as BoundedTiming,
    TokenSpecification as BoundedTokenSpecification, normalize_oracle_clause,
};
use crate::bounded_oracle_simulation::{
    BOUNDED_ORACLE_SIMULATION_BRIDGE_VERSION, clause_has_live_bridge_contract,
    printed_cost_has_live_bridge_contract,
};
use crate::characteristic_oracle_runtime::{
    AttractionLightsProcedure, CharacteristicOracleProgram, CompiledCharacteristicOracle,
    DefenseInitializationProcedure, ExactColorSetProcedure, ExactManaValueProcedure,
    ExactTypeLineProcedure, LoyaltyInitializationProcedure, PrintedStatProcedure,
    STRUCTURAL_CHARACTERISTIC_RUNTIME_VERSION, VanguardModifierProcedure,
    compile_exact_attraction_lights_procedure, compile_exact_color_set_procedure,
    compile_exact_defense_initialization_procedure, compile_exact_loyalty_initialization_procedure,
    compile_exact_mana_value_procedure, compile_exact_printed_stat_procedure,
    compile_exact_type_line_procedure, compile_exact_vanguard_modifier_procedure,
};
use crate::continuous_trigger_runtime::{
    CardSubtype as ContinuousCardSubtype, CompiledContinuousTrigger, ContinuousTriggerProgram,
    OracleOwnership as ContinuousOracleOwnership,
};
use crate::dynamic_characteristic_runtime::{
    DYNAMIC_CHARACTERISTIC_RUNTIME_VERSION, DynamicCharacteristicProcedure,
    DynamicCharacteristicSubject, compile_dynamic_loyalty_procedure,
    compile_dynamic_printed_stat_procedure,
};
use crate::effects::{
    CardTypeProfile, DevotionColor, DynamicCreatureCharacteristic, PrintedKeyword,
    compile_card_types, compile_dynamic_creature_characteristic, compile_printed_keyword_profile,
};
use crate::interaction_runtime::{
    InteractionAction, InteractionRuntimeProgram, compile_interaction_runtime_from_program,
};
use crate::keyword_rules_runtime::{
    KEYWORD_RULES_EVIDENCE_VERSION, KEYWORD_RULES_RUNTIME_VERSION, KEYWORD_TRANSACTION_CONTRACT,
    KeywordProgram, KeywordProgramInput, KeywordProgramKind, OfficialKeyword,
    TransactionalRollbackContract, compile_keyword_program,
};
use crate::land_runtime::{
    BasicLandSubtype, ExactLandClauseEvidence, ExactLandRuntimeBindingInput,
    ExactLandRuntimeSubject, LAND_RUNTIME_CLASSIFIER_VERSION, LandManaColor,
    classify_exact_land_program,
};
use crate::mana::parse_mana_cost;
use crate::mana_network_runtime::{
    COMMANDER_IDENTITY_MANA_EXECUTOR_ID, CONTROLLED_LAND_ANY_COLOR_GRANT_EXECUTOR_ID,
    CONTROLLED_LAND_CAPABILITY_MANA_EXECUTOR_ID, ExactManaNetworkProgram,
    GLOBAL_BASIC_LAND_SUBTYPE_GRANT_EXECUTOR_ID, MANA_NETWORK_RUNTIME_VERSION,
    SELF_BOUNCE_DUAL_LAND_EXECUTOR_ID,
};
use crate::mechanic_runtime::{
    MECHANIC_RUNTIME_VERSION, MarkerDisposition, MechanicProcedure, MechanicProgram,
    PrintedMechanic,
};
use crate::object_lifecycle_runtime::{
    CompiledObjectLifecycle, ObjectLifecycleProgram,
    OracleOwnership as ObjectLifecycleOracleOwnership, SourceZone as ObjectLifecycleSourceZone,
};
use crate::oracle_clause_backend::{DelegatedKeywordClause, ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION};
use crate::printed_cost_runtime::{
    PRINTED_COST_PAYMENT_BRIDGE_VERSION, PRINTED_COST_RUNTIME_VERSION, PrintedManaCost,
    parse_printed_mana_cost, printed_mana_cost_has_exact_payment_contract,
};
use crate::restriction_protection_runtime::{
    CompiledRestrictionProtection, OracleOwnership as RestrictionOracleOwnership,
    RestrictionProtectionProgram, compile_restriction_protection_from_program,
};
use crate::semantics::{CompiledCard, role};
use crate::tutor_runtime::{
    LifeLossTiming, SearchedCardPredicate, TutorDestinationZone, TutorRuntimeProgram,
    compile_tutor_runtime_from_program,
};
use crate::utility_modal_runtime::{
    CompiledUtilityModal, OracleOwnership as UtilityModalOracleOwnership,
    SourceZone as UtilityModalSourceZone, UtilityModalRuntimeProgram,
};

pub(crate) const RUNTIME_RECEIPT_SCHEMA_VERSION: &str = "commander-runtime-capability-receipt/v5";
pub(crate) const ATOMIC_TRANSACTION_EXECUTOR_VERSION: &str = "abstract-play-atomic-transaction/v2";
pub(crate) const SPELL_RESOLUTION_MANA_EXECUTOR_VERSION: &str =
    "abstract-play-spell-resolution-mana/v1";
pub(crate) const CONDITIONAL_MANA_SOURCE_EXECUTOR_VERSION: &str =
    "abstract-play-conditional-mana-source/v1";
pub(crate) const SACRIFICE_SELF_MANA_EXECUTOR_VERSION: &str =
    "abstract-play-sacrifice-self-mana/v2";
pub(crate) const GRAVEYARD_RECLAMATION_EXECUTOR_VERSION: &str =
    "abstract-play-graveyard-reclamation/v1";
pub(crate) const CHARACTERISTIC_EXECUTOR_VERSION: &str = "abstract-play-compiled-characteristic/v5";
pub(crate) const CHARACTERISTIC_ORACLE_EXECUTOR_VERSION: &str =
    "abstract-play-characteristic-oracle/v1";
pub(crate) const LIVE_ABILITY_EXECUTOR_VERSION: &str = "abstract-play-live-ability/v1";
pub(crate) const LAND_RUNTIME_EXECUTOR_VERSION: &str = LAND_RUNTIME_CLASSIFIER_VERSION;
pub(crate) const INTERACTION_RUNTIME_EXECUTOR_VERSION: &str =
    "abstract-play-interaction-runtime/v1";
pub(crate) const TUTOR_RUNTIME_EXECUTOR_VERSION: &str = "abstract-play-tutor-runtime/v1";
pub(crate) const RESTRICTION_PROTECTION_EXECUTOR_VERSION: &str =
    "abstract-play-restriction-protection/v1";
pub(crate) const ALTERNATIVE_CAST_EXECUTOR_VERSION: &str = "abstract-play-alternative-cast/v1";
pub(crate) const CONTINUOUS_TRIGGER_EXECUTOR_VERSION: &str = "abstract-play-continuous-trigger/v1";
pub(crate) const OBJECT_LIFECYCLE_EXECUTOR_VERSION: &str = "abstract-play-object-lifecycle/v1";
pub(crate) const UTILITY_MODAL_EXECUTOR_VERSION: &str = "abstract-play-utility-modal/v1";
pub(crate) const MANA_NETWORK_RUNTIME_EXECUTOR_VERSION: &str = MANA_NETWORK_RUNTIME_VERSION;
pub(crate) const BOUNDED_ORACLE_RUNTIME_EXECUTOR_VERSION: &str = "abstract-play-bounded-oracle/v5";
pub(crate) const FACE_LAYOUT_RUNTIME_EXECUTOR_VERSION: &str = "abstract-play-face-layout/v1";
pub(crate) const PRINTED_COST_RUNTIME_EXECUTOR_ID: &str = "abstract-play.printed-mana-cost";
pub(crate) const PRINTED_COST_RUNTIME_EXECUTOR_VERSION: &str = "abstract-play-printed-mana-cost/v1";
pub(crate) const KEYWORD_RULES_RUNTIME_EXECUTOR_ID: &str =
    "abstract-play.keyword-rules.transaction";
pub(crate) const KEYWORD_RULES_RUNTIME_EXECUTOR_VERSION: &str =
    "abstract-play-keyword-rules-transaction/v1";
pub(crate) const KEYWORD_RULES_EXECUTION_BRIDGE_VERSION: &str = "keyword-rules-execution-bridge/v1";

const RUNTIME_KIND_ROLE_MASK: u32 =
    role::LAND | role::CREATURE | role::ARTIFACT | role::ENCHANTMENT | role::INSTANT_SORCERY;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModeledLineCardKind {
    Permanent,
    Spell,
}

pub(crate) fn modeled_line_card_kind(card: &CompiledCard) -> Option<ModeledLineCardKind> {
    let is_permanent = card.has(role::LAND | role::CREATURE | role::ARTIFACT | role::ENCHANTMENT);
    let is_spell = card.has(role::INSTANT_SORCERY);
    match (is_permanent, is_spell) {
        (true, false) => Some(ModeledLineCardKind::Permanent),
        (false, true) => Some(ModeledLineCardKind::Spell),
        // Modal/ambiguous type combinations and unsupported permanent types
        // require a richer face/zone model. Excluding them is safer than
        // assigning whichever face would make a combo work.
        _ => None,
    }
}

fn exact_graveyard_reclamation_target(target: &GraveyardReclamationTarget) -> bool {
    target.chooser == ControllerRelation::You
        && target.count == 1
        && target.graveyard_owner == ControllerRelation::You
        && target.from == ProgramZone::Graveyard
        && target.card_type == ProgramCardType::Permanent
        && target.maximum_mana_value == 3
}

/// Revalidates every field consumed by the bounded Sevinne executor. The
/// compiler remains the source of the typed root, while this separate check
/// prevents a hand-authored or future extended IR from inheriting execution.
pub(crate) fn classify_graveyard_reclamation(
    card: &CompiledCard,
) -> Option<&ExecutableGraveyardReclamation> {
    let ability_program = &card.ability_program;
    if ability_program.version != EXECUTABLE_ABILITY_PROGRAM_VERSION
        || !ability_program.abilities.is_empty()
        || ability_program.atomic_transaction.is_some()
        || ability_program.necropotence_lifecycle.is_some()
        || ability_program.self_transfer_tutor_permanent.is_some()
        || ability_program.entry_linked_permanent.is_some()
        || !ability_program.face_programs.is_empty()
        || ability_program.unsupported_abilities().next().is_some()
        || !type_line_has_exact_type_envelope(&card.type_line, "sorcery", false)
    {
        return None;
    }
    let program = ability_program.executable_graveyard_reclamation()?;
    let source = &program.source_spell;
    let [
        GraveyardReclamationResolutionInstruction::ReturnTargetToBattlefield(return_target),
        GraveyardReclamationResolutionInstruction::CreateConditionalCopy(copy_procedure),
    ] = source.resolution.instructions.as_slice()
    else {
        return None;
    };
    let copy = &copy_procedure.copy;
    let flashback = &program.flashback;
    let flashback_profile = match &flashback.alternative_cost {
        ProgramManaCost::PrintedSymbols { oracle, profile } if oracle == "{4}{W}" => profile,
        _ => return None,
    };
    let exact_flashback_profile = flashback_profile.generic == 4
        && flashback_profile.white == 1
        && flashback_profile.blue == 0
        && flashback_profile.black == 0
        && flashback_profile.red == 0
        && flashback_profile.green == 0
        && flashback_profile.colorless == 0
        && flashback_profile.variable_x == 0;
    let exact_cast_sequence = flashback.cast_sequence
        == [
            GraveyardReclamationFlashbackCastStep::MovePhysicalSourceCardToStack,
            GraveyardReclamationFlashbackCastStep::ChooseFlashbackAlternativeCost,
            GraveyardReclamationFlashbackCastStep::ChooseRequiredLegalTarget,
            GraveyardReclamationFlashbackCastStep::DetermineTotalCost,
            GraveyardReclamationFlashbackCastStep::ActivateManaAbilities,
            GraveyardReclamationFlashbackCastStep::PayTotalCost,
        ];
    let exact_source = source.physical_card == GraveyardReclamationObject::PhysicalSourceCard
        && source.stack_object == GraveyardReclamationObject::SourceSpellOnStack
        && exact_graveyard_reclamation_target(&source.target)
        && source.target_selection
            == GraveyardReclamationTargetSelection::ChooseOneLegalTargetDuringCast
        && source.resolution.target_legality
            == GraveyardReclamationResolutionTargetLegality::CheckAllTargetsBeforeResolutionInstructions
        && source.resolution.all_targets_illegal
            == GraveyardReclamationAllTargetsIllegalOutcome::SpellDoesNotResolveAndSkipsAllInstructions;
    let exact_return = return_target.object == GraveyardReclamationObject::TargetOfThisSpellOrCopy
        && return_target.from == ProgramZone::Graveyard
        && return_target.to == ProgramZone::Battlefield
        && return_target.destination_controller == ControllerRelation::You;
    let exact_copy = copy_procedure.condition
        == GraveyardReclamationCopyCondition::ThisSpellWasCastFromGraveyard
        && copy_procedure.timing
            == GraveyardReclamationCopyTiming::SecondInstructionAfterTargetReturn
        && copy_procedure.source_object == GraveyardReclamationObject::SourceSpellOnStack
        && copy_procedure.optional
        && copy.object == GraveyardReclamationObject::CopyOfSourceSpellOnStack
        && copy.destination == ProgramZone::Stack
        && !copy.is_cast
        && copy.inherits_source_effect
        && copy.inherits_source_target
        && copy.retarget.optional
        && exact_graveyard_reclamation_target(&copy.retarget.target)
        && copy.retarget.timing == GraveyardReclamationRetargetTiming::AsCopyIsPutOntoStack
        && copy.retarget.legality
            == GraveyardReclamationRetargetLegality::MustSatisfySourceTargetDefinition
        && copy.stack_position == GraveyardReclamationStackPosition::AboveResolvingSourceSpell;
    let exact_flashback = flashback.action
        == GraveyardReclamationCastAction::CastPhysicalSourceCard
        && flashback.object == GraveyardReclamationObject::PhysicalSourceCard
        && flashback.owner == ControllerRelation::You
        && flashback.from == ProgramZone::Graveyard
        && flashback.to == ProgramZone::Stack
        && exact_flashback_profile
        && exact_cast_sequence
        && flashback.stack_exit_replacement.object
            == GraveyardReclamationObject::PhysicalSourceCard
        && flashback.stack_exit_replacement.event
            == GraveyardReclamationStackExitEvent::FlashbackSourceWouldLeaveStackForAnyReason
        && flashback.stack_exit_replacement.replacement_destination == ProgramZone::Exile;

    (exact_source
        && exact_return
        && exact_copy
        && exact_flashback
        && program.resolution_order
            == GraveyardReclamationResolutionOrder::SourceSpellFinishesAndLeavesStackBeforeCopyCanResolve
        && program.pending_trigger_order
            == GraveyardReclamationPendingTriggerOrder::SourceReturnTriggersArePutAboveCopyAfterSourceFinishes)
        .then_some(program)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeCapability {
    /// The executor owns every non-formatting Oracle clause in this root.
    CompleteOracleRoot,
    /// The executor owns only the exact, occurrence-addressed Oracle clauses
    /// listed in its source evidence.
    ExactOracleClauseSet,
    /// Costs and activation/cast initiation commit before resolution.
    AtomicInitiationBoundary,
    /// Every resolution effect executes in the retained typed order.
    OrderedResolution,
    /// A countered spell keeps committed costs and skips every resolution step.
    CounteredSpellResolutionBoundary,
    /// This is a hand-zone mana ability and does not cast a spell or use the stack.
    HandManaAbilityWithoutStack,
    /// A permanent's reviewed replacement or linked entry procedure moves the
    /// exact physical objects required by its mana ability.
    ExactPermanentEntryProcedure,
    /// The mana source is refreshed from the live battlefield condition each
    /// time the bounded executor considers spending it.
    LiveBattlefieldManaCondition,
    /// The activation chooses one color from the current commander's color identity.
    ExactCommanderColorIdentityMana,
    /// The activation derives its available mana types from lands currently controlled.
    ExactControlledLandManaCapabilities,
    /// The source continuously grants an exact any-color mana ability to controlled lands.
    ExactControlledLandManaGrant,
    /// The source continuously grants one exact basic land subtype and its intrinsic mana ability.
    ExactGlobalBasicLandSubtypeGrant,
    /// Entry tapped, return-one-land, and coupled two-color output remain one exact land lifecycle.
    ExactSelfBounceDualLandLifecycle,
    /// The exact untapped battlefield object is tapped, then sacrificed, as
    /// the complete activation cost for one mana of any color.
    SacrificeSelfManaAbility,
    /// One exact, face-bound printed characteristic was compiled offline from
    /// the same typed parser used by the runtime consumer.
    ExactCompiledCharacteristic,
    /// One exact, face-bound printed mana cost was accepted by the closed
    /// printed-symbol parser without collapsing any payment alternative.
    ExactPrintedManaCost,
    /// Printed-cost payment stages mana, life, and land-drop resources and
    /// commits them only after every declared choice can be paid.
    TransactionalPrintedManaPayment,
    /// The complete retained face envelope was compiled into the exact
    /// zone, casting, and live transition procedure for its layout.
    ExactFaceLayoutProgram,
    /// The complete root owns the exact printed Flashback keyword procedure.
    ExactFlashbackKeyword,
    /// The complete atomic transaction owns the exact printed Bargain keyword.
    ExactBargainKeyword,
    /// The entry-linked permanent transaction owns the exact Imprint ability word.
    ExactImprintAbilityWord,
    /// The live conditional mana source owns the exact Metalcraft ability word.
    ExactMetalcraftAbilityWord,
    /// The atomic replacement transaction owns the exact Threshold ability word.
    ExactThresholdAbilityWord,
    /// The reviewed spell-resolution procedure owns the exact Mill keyword action.
    ExactMillKeyword,
    /// The reviewed cast-trigger procedure owns the exact Storm copy procedure.
    ExactStormKeyword,
    /// The marker is an exact printed ability word whose following Oracle
    /// clause is independently owned by the same live bounded executor.
    ExactAbilityWordMarker,
    /// A complete occurrence-addressed clause is represented by the generic
    /// bounded Oracle IR and consumed by its transactional state executor.
    ExactBoundedOracleProgram,
    /// The exact source restriction is consulted when determining whether the
    /// source can legally be blocked in combat.
    ExactCannotBeBlockedRestriction,
    /// The exact static clause resolves the source's current legal Aura or
    /// Equipment attachment through the live state bridge.
    ExactAttachmentStaticEffect,
    /// One exact keyword occurrence compiled into the official keyword rules
    /// kernel. This capability proves the kernel program only. A coverage
    /// metric must also bind the receipt to its production state adapter.
    ExactKeywordRulesProgram,
    /// The keyword kernel consumer commits the complete action or restores the
    /// full pre-action state.
    TransactionalKeywordRulesExecution,
    /// One bounded resolution retains an extra-turn grant and its mandatory
    /// delayed game-loss consequence as one transactional lifecycle.
    ExactDelayedDrawbackLifecycle,
    ExactChannelKeyword,
    ExactCyclingKeyword,
    ExactTypecyclingKeyword,
    ExactTreasureKeyword,
    ExactFoodKeyword,
    ExactProwessKeyword,
    ExactWardKeyword,
    ExactSurveilKeyword,
    ExactCrewKeyword,
    ExactSplitSecondKeyword,
    ExactEvokeKeyword,
    ExactManifestKeyword,
    ExactPartnerKeyword,
    ExactTransformKeyword,
    ExactParadigmKeyword,
    ExactDoubleKeyword,
    ExactProtectionKeyword,
    ExactLandfallAbilityWord,
    ExactFerociousAbilityWord,
    ExactDashKeyword,
    ExactGiftKeyword,
    ExactMobilizeKeyword,
    /// The complete cast choice owns the exact printed Overload procedure.
    ExactOverloadKeyword,
    /// The complete cast choice owns the exact printed Escape procedure.
    ExactEscapeKeyword,
    /// The retained Aura program owns its exact Enchant target procedure.
    ExactEnchantKeyword,
    /// The retained face program owns its exact Equip activation procedure.
    ExactEquipKeyword,
    /// The retained resolution owns the exact Scry procedure.
    ExactScryKeyword,
    /// Graveyard choices retain physical object identity independently from a
    /// repeated card-definition index.
    ExactPhysicalZoneObjectIdentity,
    /// A resolving source may create one optional spell copy without casting it.
    OptionalUncastSpellCopy,
    /// The optional copy can retain its inherited target or choose one new
    /// target satisfying the complete source target definition.
    OptionalLegalCopyRetarget,
    /// The physical source finishes and leaves the stack before its copy resolves.
    SourceExitBeforeCopyResolution,
    /// Entry triggers from the source return resolve above the already-created copy.
    SourceReturnTriggersAboveCopy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeExecutorBinding {
    pub receipt_schema_version: &'static str,
    pub executor_id: &'static str,
    pub executor_version: &'static str,
}

/// Stable source identity for later coverage-leaf binding.
///
/// The composite digest includes every source input that can select this
/// classifier: the reviewed ability-program version, complete normalized
/// Oracle root, exact type line, relevant semantic type-role bits, and the
/// selected executor family. Names are deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeSourceEvidence {
    pub ability_program_version: &'static str,
    pub normalized_oracle_sha256: String,
    pub normalized_oracle_clause_sha256s: Vec<String>,
    pub covered_oracle_clauses: Vec<RuntimeOracleClauseEvidence>,
    pub type_line_sha256: String,
    pub relevant_type_role_mask: u32,
    pub source_evidence_sha256: String,
}

/// One exact occurrence in the normalized Oracle source owned by a runtime
/// receipt. The ordinal prevents one repeated line from authorizing every
/// identical occurrence on a card.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeOracleClauseEvidence {
    pub face_index: u16,
    pub clause_index: u16,
    pub normalized_clause_sha256: String,
}

impl RuntimeSourceEvidence {
    pub(crate) fn has_exact_clause_contract(&self) -> bool {
        !self.normalized_oracle_clause_sha256s.is_empty()
            && self
                .normalized_oracle_clause_sha256s
                .iter()
                .all(|digest| is_sha256_hex(digest))
            && !self.covered_oracle_clauses.is_empty()
            && self.covered_oracle_clauses.iter().all(|clause| {
                is_sha256_hex(&clause.normalized_clause_sha256)
                    && self
                        .normalized_oracle_clause_sha256s
                        .contains(&clause.normalized_clause_sha256)
            })
            && self
                .covered_oracle_clauses
                .windows(2)
                .all(|pair| pair[0] < pair[1])
    }
}

/// Exact retained metadata identity for one keyword in one face keyword profile.
///
/// Oracle-authoritative receipts leave the profile empty and bind the complete
/// self-describing Oracle clause instead. Snapshot keyword metadata can
/// corroborate that clause, but cannot invalidate it when the text is unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeywordRulesOccurrenceEvidence {
    pub face_index: u16,
    pub keyword_occurrence_index: u16,
    pub oracle_clause_index: u16,
    pub normalized_keyword: String,
    pub normalized_face_keywords: Vec<String>,
    pub complete_keyword_profile_sha256: String,
    pub printed_keyword_sha256: String,
    pub oracle_fragment_sha256: Option<String>,
    pub type_line_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeywordRulesAuthority {
    RetainedKeywordMetadata,
    SelfDescribingOracleClause,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct KeywordRulesReceiptInput<'a> {
    pub face_index: u16,
    pub keyword_occurrence_index: u16,
    pub oracle_clause_index: u16,
    pub face_keywords: &'a [String],
    pub type_line: &'a str,
    /// The exact keyword-bearing Oracle fragment when the keyword has a
    /// parameter or more than one rules shape. Fixed keyword metadata may omit
    /// this field.
    pub oracle_fragment: Option<&'a str>,
}

/// A kernel execution receipt for one exact retained keyword occurrence.
///
/// This receipt proves that the official keyword kernel compiled the
/// occurrence and exposes a transactional consumer for its action. It does not
/// by itself prove that a report metric calls that consumer. Coverage must
/// additionally bind the receipt to the production metric adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeywordRulesRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub authority: KeywordRulesAuthority,
    pub occurrence: KeywordRulesOccurrenceEvidence,
    pub keyword: OfficialKeyword,
    pub program: KeywordProgram,
    pub official_rule_ids: Vec<String>,
    pub kernel_runtime_version: &'static str,
    pub kernel_evidence_version: &'static str,
    pub execution_bridge_version: &'static str,
    pub rollback_contract: TransactionalRollbackContract,
    pub contract_sha256: String,
}

impl KeywordRulesRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        let source = self.program.source();
        let expected_rule_ids = self
            .program
            .official_rules()
            .iter()
            .map(|rule| rule.id().to_owned())
            .collect::<Vec<_>>();
        let authority_has_exact_contract = match self.authority {
            KeywordRulesAuthority::RetainedKeywordMetadata => {
                let occurrence_index = usize::from(self.occurrence.keyword_occurrence_index);
                self.occurrence
                    .normalized_face_keywords
                    .get(occurrence_index)
                    == Some(&self.occurrence.normalized_keyword)
                    && keyword_profile_is_exact(&self.occurrence.normalized_face_keywords)
                    && self.occurrence.complete_keyword_profile_sha256
                        == keyword_profile_sha256(&self.occurrence.normalized_face_keywords)
            }
            KeywordRulesAuthority::SelfDescribingOracleClause => {
                self.occurrence.keyword_occurrence_index == 0
                    && self.occurrence.normalized_face_keywords.is_empty()
                    && self.occurrence.complete_keyword_profile_sha256
                        == keyword_profile_sha256(&[])
                    && self.occurrence.oracle_fragment_sha256.is_some()
            }
        };
        self.binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
            && self.binding.executor_id == KEYWORD_RULES_RUNTIME_EXECUTOR_ID
            && self.binding.executor_version == KEYWORD_RULES_RUNTIME_EXECUTOR_VERSION
            && self.capabilities
                == [
                    RuntimeCapability::ExactKeywordRulesProgram,
                    RuntimeCapability::TransactionalKeywordRulesExecution,
                ]
            && self.kernel_runtime_version == KEYWORD_RULES_RUNTIME_VERSION
            && self.kernel_evidence_version == KEYWORD_RULES_EVIDENCE_VERSION
            && self.execution_bridge_version == KEYWORD_RULES_EXECUTION_BRIDGE_VERSION
            && self.rollback_contract == KEYWORD_TRANSACTION_CONTRACT
            && self.program.runtime_version() == KEYWORD_RULES_RUNTIME_VERSION
            && self.program.has_exact_contract()
            && self.keyword == self.program.keyword()
            && !self.occurrence.normalized_keyword.is_empty()
            && authority_has_exact_contract
            && self.occurrence.normalized_keyword
                == normalize_keyword_label(self.keyword.printed_label())
            && source.face_index == self.occurrence.face_index
            && source.clause_index == self.occurrence.oracle_clause_index
            && keyword_program_source_label_is_exact(
                &self.program,
                &self.occurrence.normalized_keyword,
            )
            && self.occurrence.printed_keyword_sha256
                == sha256_hex(source.printed_keyword.trim().as_bytes())
            && self.occurrence.oracle_fragment_sha256
                == source
                    .oracle_fragment
                    .as_deref()
                    .map(str::trim)
                    .map(|fragment| sha256_hex(fragment.as_bytes()))
            && is_sha256_hex(&self.occurrence.complete_keyword_profile_sha256)
            && is_sha256_hex(&self.occurrence.printed_keyword_sha256)
            && self
                .occurrence
                .oracle_fragment_sha256
                .as_deref()
                .is_none_or(is_sha256_hex)
            && is_sha256_hex(&self.occurrence.type_line_sha256)
            && self.official_rule_ids == expected_rule_ids
            && self
                .official_rule_ids
                .iter()
                .all(|rule_id| !rule_id.trim().is_empty())
            && is_sha256_hex(&self.contract_sha256)
            && self.contract_sha256 == keyword_rules_contract_sha256(self)
    }

    pub(crate) fn matches_keyword_occurrence(
        &self,
        face_index: u16,
        keyword_occurrence_index: u16,
        normalized_keyword: &str,
    ) -> bool {
        self.authority == KeywordRulesAuthority::RetainedKeywordMetadata
            && self.has_exact_contract()
            && self.occurrence.face_index == face_index
            && self.occurrence.keyword_occurrence_index == keyword_occurrence_index
            && self.occurrence.normalized_keyword == normalize_keyword_label(normalized_keyword)
    }

    pub(crate) fn matches_face_keyword(&self, face_index: u16, normalized_keyword: &str) -> bool {
        let normalized_keyword = normalize_keyword_label(normalized_keyword);
        self.has_exact_contract()
            && self.occurrence.face_index == face_index
            && (self.occurrence.normalized_keyword == normalized_keyword
                || normalize_keyword_label(&self.program.source().printed_keyword)
                    == normalized_keyword)
    }
}

/// Exact coverage binding for one delegated keyword clause.
///
/// The inner receipt proves only the keyword kernel contract. This wrapper
/// additionally binds that program to one occurrence-addressed Oracle clause
/// and to the delegated backend contract. It still does not prove that any
/// production trajectory calls the keyword executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactKeywordRulesRuntimeReceipt {
    pub keyword_rules: KeywordRulesRuntimeReceipt,
    pub delegated_clause: DelegatedKeywordClause,
    pub source_evidence: RuntimeSourceEvidence,
    pub contract_sha256: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ExactKeywordRulesReceiptInput<'a> {
    pub face_index: u16,
    pub face_name: &'a str,
    pub type_line: &'a str,
    pub oracle_text: &'a str,
    pub oracle_clauses: &'a [String],
    pub delegated_clause: &'a DelegatedKeywordClause,
}

impl ExactKeywordRulesRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        let address = self.delegated_clause.address();
        let Some(selected_digest) = self
            .source_evidence
            .normalized_oracle_clause_sha256s
            .get(usize::from(address.clause_index))
            .cloned()
        else {
            return false;
        };
        let covered = self.source_evidence.covered_oracle_clauses.as_slice();
        self.keyword_rules.has_exact_contract()
            && self.keyword_rules.authority == KeywordRulesAuthority::SelfDescribingOracleClause
            && self.delegated_clause.runtime_version() == ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION
            && self.delegated_clause.keyword_program() == &self.keyword_rules.program
            && self.delegated_clause.keyword_program().has_exact_contract()
            && self
                .delegated_clause
                .required_live_bridge_capabilities()
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            && !self
                .delegated_clause
                .required_live_bridge_capabilities()
                .is_empty()
            && address.face_index == self.keyword_rules.occurrence.face_index
            && address.clause_index == self.keyword_rules.occurrence.oracle_clause_index
            && self.source_evidence.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
            && self.source_evidence.has_exact_clause_contract()
            && self.source_evidence.type_line_sha256
                == self.keyword_rules.occurrence.type_line_sha256
            && covered
                == [RuntimeOracleClauseEvidence {
                    face_index: address.face_index,
                    clause_index: address.clause_index,
                    normalized_clause_sha256: selected_digest,
                }]
            && self.source_evidence.source_evidence_sha256
                == exact_keyword_rules_source_evidence_sha256(
                    &self.source_evidence,
                    &self.delegated_clause,
                )
            && is_sha256_hex(&self.contract_sha256)
            && self.contract_sha256 == exact_keyword_rules_contract_sha256(self)
    }
}

pub(crate) fn compile_keyword_rules_runtime_receipt(
    input: KeywordRulesReceiptInput<'_>,
) -> Option<KeywordRulesRuntimeReceipt> {
    let type_line = input.type_line.trim();
    if type_line.is_empty() {
        return None;
    }
    let normalized_face_keywords = input
        .face_keywords
        .iter()
        .map(|keyword| normalize_keyword_label(keyword))
        .collect::<Vec<_>>();
    if !keyword_profile_is_exact(&normalized_face_keywords) {
        return None;
    }
    let occurrence_index = usize::from(input.keyword_occurrence_index);
    let normalized_keyword = normalized_face_keywords.get(occurrence_index)?.clone();
    let printed_keyword = input.face_keywords.get(occurrence_index)?.trim();
    let oracle_fragment = match input.oracle_fragment {
        Some(fragment) if fragment.trim().is_empty() => return None,
        Some(fragment) => Some(fragment.trim()),
        None => None,
    };
    let program = compile_keyword_program(KeywordProgramInput {
        face_index: input.face_index,
        clause_index: input.oracle_clause_index,
        printed_keyword,
        oracle_fragment,
    })
    .ok()?;
    if keyword_rules_receipt_requires_fragment(program.keyword()) && oracle_fragment.is_none() {
        return None;
    }
    if normalize_keyword_label(program.keyword().printed_label()) != normalized_keyword {
        return None;
    }
    let official_rule_ids = program
        .official_rules()
        .iter()
        .map(|rule| rule.id().to_owned())
        .collect::<Vec<_>>();
    let occurrence = KeywordRulesOccurrenceEvidence {
        face_index: input.face_index,
        keyword_occurrence_index: input.keyword_occurrence_index,
        oracle_clause_index: input.oracle_clause_index,
        normalized_keyword,
        complete_keyword_profile_sha256: keyword_profile_sha256(&normalized_face_keywords),
        normalized_face_keywords,
        printed_keyword_sha256: sha256_hex(printed_keyword.as_bytes()),
        oracle_fragment_sha256: oracle_fragment.map(|fragment| sha256_hex(fragment.as_bytes())),
        type_line_sha256: sha256_hex(type_line.as_bytes()),
    };
    let mut receipt = KeywordRulesRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id: KEYWORD_RULES_RUNTIME_EXECUTOR_ID,
            executor_version: KEYWORD_RULES_RUNTIME_EXECUTOR_VERSION,
        },
        capabilities: vec![
            RuntimeCapability::ExactKeywordRulesProgram,
            RuntimeCapability::TransactionalKeywordRulesExecution,
        ],
        authority: KeywordRulesAuthority::RetainedKeywordMetadata,
        occurrence,
        keyword: program.keyword(),
        program,
        official_rule_ids,
        kernel_runtime_version: KEYWORD_RULES_RUNTIME_VERSION,
        kernel_evidence_version: KEYWORD_RULES_EVIDENCE_VERSION,
        execution_bridge_version: KEYWORD_RULES_EXECUTION_BRIDGE_VERSION,
        rollback_contract: KEYWORD_TRANSACTION_CONTRACT,
        contract_sha256: String::new(),
    };
    receipt.contract_sha256 = keyword_rules_contract_sha256(&receipt);
    receipt.has_exact_contract().then_some(receipt)
}

fn compile_oracle_keyword_rules_runtime_receipt(
    face_index: u16,
    oracle_clause_index: u16,
    printed_keyword: &str,
    type_line: &str,
    oracle_fragment: &str,
) -> Option<KeywordRulesRuntimeReceipt> {
    let type_line = type_line.trim();
    let printed_keyword = printed_keyword.trim();
    let oracle_fragment = oracle_fragment.trim();
    if type_line.is_empty() || printed_keyword.is_empty() || oracle_fragment.is_empty() {
        return None;
    }
    let program = compile_keyword_program(KeywordProgramInput {
        face_index,
        clause_index: oracle_clause_index,
        printed_keyword,
        oracle_fragment: Some(oracle_fragment),
    })
    .ok()?;
    let normalized_keyword = normalize_keyword_label(program.keyword().printed_label());
    if !keyword_program_source_label_is_exact(&program, &normalized_keyword) {
        return None;
    }
    let official_rule_ids = program
        .official_rules()
        .iter()
        .map(|rule| rule.id().to_owned())
        .collect::<Vec<_>>();
    let occurrence = KeywordRulesOccurrenceEvidence {
        face_index,
        keyword_occurrence_index: 0,
        oracle_clause_index,
        normalized_keyword,
        normalized_face_keywords: Vec::new(),
        complete_keyword_profile_sha256: keyword_profile_sha256(&[]),
        printed_keyword_sha256: sha256_hex(printed_keyword.as_bytes()),
        oracle_fragment_sha256: Some(sha256_hex(oracle_fragment.as_bytes())),
        type_line_sha256: sha256_hex(type_line.as_bytes()),
    };
    let mut receipt = KeywordRulesRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id: KEYWORD_RULES_RUNTIME_EXECUTOR_ID,
            executor_version: KEYWORD_RULES_RUNTIME_EXECUTOR_VERSION,
        },
        capabilities: vec![
            RuntimeCapability::ExactKeywordRulesProgram,
            RuntimeCapability::TransactionalKeywordRulesExecution,
        ],
        authority: KeywordRulesAuthority::SelfDescribingOracleClause,
        occurrence,
        keyword: program.keyword(),
        program,
        official_rule_ids,
        kernel_runtime_version: KEYWORD_RULES_RUNTIME_VERSION,
        kernel_evidence_version: KEYWORD_RULES_EVIDENCE_VERSION,
        execution_bridge_version: KEYWORD_RULES_EXECUTION_BRIDGE_VERSION,
        rollback_contract: KEYWORD_TRANSACTION_CONTRACT,
        contract_sha256: String::new(),
    };
    receipt.contract_sha256 = keyword_rules_contract_sha256(&receipt);
    receipt.has_exact_contract().then_some(receipt)
}

fn keyword_program_source_label_is_exact(
    program: &KeywordProgram,
    normalized_official_keyword: &str,
) -> bool {
    let normalized_source = normalize_keyword_label(&program.source().printed_keyword);
    match program.kind() {
        KeywordProgramKind::Landwalk(landwalk) => {
            normalized_source == normalize_keyword_label(landwalk.quality.printed_label())
        }
        _ => normalized_source == normalized_official_keyword,
    }
}

pub(crate) fn compile_exact_keyword_rules_runtime_receipt(
    input: ExactKeywordRulesReceiptInput<'_>,
) -> Option<ExactKeywordRulesRuntimeReceipt> {
    let address = input.delegated_clause.address();
    if address.face_index != input.face_index
        || usize::from(address.clause_index) >= input.oracle_clauses.len()
    {
        return None;
    }
    let raw_clause = input
        .oracle_clauses
        .get(usize::from(address.clause_index))?
        .trim();
    if raw_clause.is_empty() {
        return None;
    }
    let delegated_normalized_clause =
        normalize_oracle_clause(raw_clause, input.face_name, input.type_line);
    if delegated_normalized_clause != input.delegated_clause.normalized_clause() {
        return None;
    }
    let keyword_rules = compile_oracle_keyword_rules_runtime_receipt(
        input.face_index,
        address.clause_index,
        &input
            .delegated_clause
            .keyword_program()
            .source()
            .printed_keyword,
        input.type_line,
        raw_clause,
    )?;
    if keyword_rules.program != *input.delegated_clause.keyword_program() {
        return None;
    }

    let normalized_oracle =
        normalize_oracle_clause_for_receipt(input.oracle_text, input.face_name, input.type_line);
    if normalized_oracle.trim().is_empty() {
        return None;
    }
    let normalized_oracle_clause_sha256s = input
        .oracle_clauses
        .iter()
        .map(|clause| normalize_oracle_clause_for_receipt(clause, input.face_name, input.type_line))
        .map(|clause| sha256_hex(clause.as_bytes()))
        .collect::<Vec<_>>();
    if normalized_oracle_clause_sha256s.is_empty()
        || normalized_oracle_clause_sha256s
            .iter()
            .any(|digest| !is_sha256_hex(digest))
    {
        return None;
    }
    let selected_digest = normalized_oracle_clause_sha256s
        .get(usize::from(address.clause_index))?
        .clone();
    let mut source_evidence = RuntimeSourceEvidence {
        ability_program_version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
        normalized_oracle_sha256: sha256_hex(normalized_oracle.as_bytes()),
        normalized_oracle_clause_sha256s,
        covered_oracle_clauses: vec![RuntimeOracleClauseEvidence {
            face_index: address.face_index,
            clause_index: address.clause_index,
            normalized_clause_sha256: selected_digest,
        }],
        type_line_sha256: sha256_hex(input.type_line.trim().as_bytes()),
        relevant_type_role_mask: runtime_kind_role_mask_for_type_line(input.type_line),
        source_evidence_sha256: String::new(),
    };
    source_evidence.source_evidence_sha256 =
        exact_keyword_rules_source_evidence_sha256(&source_evidence, input.delegated_clause);
    let mut receipt = ExactKeywordRulesRuntimeReceipt {
        keyword_rules,
        delegated_clause: input.delegated_clause.clone(),
        source_evidence,
        contract_sha256: String::new(),
    };
    receipt.contract_sha256 = exact_keyword_rules_contract_sha256(&receipt);
    receipt.has_exact_contract().then_some(receipt)
}

fn exact_keyword_rules_source_evidence_sha256(
    source_evidence: &RuntimeSourceEvidence,
    delegated_clause: &DelegatedKeywordClause,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        RUNTIME_RECEIPT_SCHEMA_VERSION.as_bytes(),
        KEYWORD_RULES_RUNTIME_EXECUTOR_ID.as_bytes(),
        KEYWORD_RULES_RUNTIME_EXECUTOR_VERSION.as_bytes(),
        source_evidence.ability_program_version.as_bytes(),
        ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION.as_bytes(),
        delegated_clause.semantic_digest().as_bytes(),
        source_evidence.normalized_oracle_sha256.as_bytes(),
        source_evidence.type_line_sha256.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    hash_framed(
        &mut hasher,
        &source_evidence.relevant_type_role_mask.to_be_bytes(),
    );
    for digest in &source_evidence.normalized_oracle_clause_sha256s {
        hash_framed(&mut hasher, digest.as_bytes());
    }
    for clause in &source_evidence.covered_oracle_clauses {
        hash_framed(&mut hasher, &clause.face_index.to_be_bytes());
        hash_framed(&mut hasher, &clause.clause_index.to_be_bytes());
        hash_framed(&mut hasher, clause.normalized_clause_sha256.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn exact_keyword_rules_contract_sha256(receipt: &ExactKeywordRulesRuntimeReceipt) -> String {
    let mut hasher = Sha256::new();
    for part in [
        receipt.keyword_rules.contract_sha256.as_bytes(),
        receipt.delegated_clause.runtime_version().as_bytes(),
        receipt.delegated_clause.semantic_digest().as_bytes(),
        receipt.source_evidence.source_evidence_sha256.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    for capability in receipt.delegated_clause.required_live_bridge_capabilities() {
        hash_framed(&mut hasher, capability.stable_id().as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn keyword_rules_receipt_requires_fragment(keyword: OfficialKeyword) -> bool {
    matches!(
        keyword,
        OfficialKeyword::Regenerate
            | OfficialKeyword::Protection
            | OfficialKeyword::Kicker
            | OfficialKeyword::Flashback
            | OfficialKeyword::Morph
            | OfficialKeyword::Equip
            | OfficialKeyword::Enchant
            | OfficialKeyword::Saga
            | OfficialKeyword::CumulativeUpkeep
            | OfficialKeyword::Hexproof
            | OfficialKeyword::Ward
            | OfficialKeyword::Cycling
    )
}

fn normalize_keyword_label(keyword: &str) -> String {
    keyword
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn keyword_profile_is_exact(profile: &[String]) -> bool {
    !profile.is_empty()
        && profile.iter().all(|keyword| !keyword.is_empty())
        && profile
            .iter()
            .enumerate()
            .all(|(index, keyword)| !profile[..index].contains(keyword))
}

fn keyword_profile_sha256(profile: &[String]) -> String {
    let mut hasher = Sha256::new();
    hash_framed(&mut hasher, &(profile.len() as u64).to_be_bytes());
    for keyword in profile {
        hash_framed(&mut hasher, keyword.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn keyword_rules_contract_sha256(receipt: &KeywordRulesRuntimeReceipt) -> String {
    let mut hasher = Sha256::new();
    for part in [
        receipt.binding.receipt_schema_version.as_bytes(),
        receipt.binding.executor_id.as_bytes(),
        receipt.binding.executor_version.as_bytes(),
        receipt.kernel_runtime_version.as_bytes(),
        receipt.kernel_evidence_version.as_bytes(),
        receipt.execution_bridge_version.as_bytes(),
        receipt.occurrence.normalized_keyword.as_bytes(),
        receipt
            .occurrence
            .complete_keyword_profile_sha256
            .as_bytes(),
        receipt.occurrence.printed_keyword_sha256.as_bytes(),
        receipt.occurrence.type_line_sha256.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    hash_framed(&mut hasher, &receipt.occurrence.face_index.to_be_bytes());
    hash_framed(&mut hasher, format!("{:?}", receipt.authority).as_bytes());
    hash_framed(
        &mut hasher,
        &receipt.occurrence.keyword_occurrence_index.to_be_bytes(),
    );
    hash_framed(
        &mut hasher,
        &receipt.occurrence.oracle_clause_index.to_be_bytes(),
    );
    for keyword in &receipt.occurrence.normalized_face_keywords {
        hash_framed(&mut hasher, keyword.as_bytes());
    }
    if let Some(oracle_fragment_sha256) = &receipt.occurrence.oracle_fragment_sha256 {
        hash_framed(&mut hasher, oracle_fragment_sha256.as_bytes());
    }
    for capability in &receipt.capabilities {
        hash_framed(&mut hasher, format!("{capability:?}").as_bytes());
    }
    for rule_id in &receipt.official_rule_ids {
        hash_framed(&mut hasher, rule_id.as_bytes());
    }
    hash_framed(&mut hasher, format!("{:?}", receipt.keyword).as_bytes());
    hash_framed(&mut hasher, format!("{:?}", receipt.program).as_bytes());
    hash_framed(
        &mut hasher,
        format!("{:?}", receipt.rollback_contract).as_bytes(),
    );
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrintedCostRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub face_index: u16,
    pub type_line: String,
    pub program: PrintedManaCost,
    pub printed_cost_runtime_version: &'static str,
    pub payment_bridge_version: &'static str,
    pub contract_sha256: String,
}

impl PrintedCostRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        let [face] = self.program.faces.as_slice() else {
            return false;
        };
        let printed_sha256 = sha256_hex(self.program.raw.as_bytes());
        let type_line_sha256 = sha256_hex(self.type_line.as_bytes());
        let expected_contract_sha256 = printed_cost_contract_sha256(
            self.face_index,
            &self.type_line,
            &self.program,
            self.printed_cost_runtime_version,
            self.payment_bridge_version,
        );
        self.binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
            && self.binding.executor_id == PRINTED_COST_RUNTIME_EXECUTOR_ID
            && self.binding.executor_version == PRINTED_COST_RUNTIME_EXECUTOR_VERSION
            && self.capabilities
                == [
                    RuntimeCapability::ExactPrintedManaCost,
                    RuntimeCapability::TransactionalPrintedManaPayment,
                ]
            && self.printed_cost_runtime_version == PRINTED_COST_RUNTIME_VERSION
            && self.payment_bridge_version == PRINTED_COST_PAYMENT_BRIDGE_VERSION
            && face.has_mana_cost
            && face.raw == self.program.raw
            && printed_mana_cost_has_exact_payment_contract(&self.program)
            && printed_cost_has_live_bridge_contract(&self.program)
            && self.source_evidence.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
            && self.source_evidence.normalized_oracle_sha256 == printed_sha256
            && self.source_evidence.normalized_oracle_clause_sha256s == [printed_sha256.clone()]
            && self.source_evidence.covered_oracle_clauses
                == [RuntimeOracleClauseEvidence {
                    face_index: self.face_index,
                    clause_index: 0,
                    normalized_clause_sha256: printed_sha256,
                }]
            && self.source_evidence.type_line_sha256 == type_line_sha256
            && self.source_evidence.relevant_type_role_mask == 0
            && self.source_evidence.source_evidence_sha256 == expected_contract_sha256
            && self.source_evidence.has_exact_clause_contract()
            && self.contract_sha256 == expected_contract_sha256
            && is_sha256_hex(&self.contract_sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AtomicRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub transaction: TypedAtomicTransaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TypedSpellResolutionMana {
    pub output: FixedManaProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpellResolutionManaRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub program: TypedSpellResolutionMana,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypedConditionalManaSource {
    ImprintLinkedCardColors,
    DiscardLandOrFailEntry,
    ControlledLegendaryColors,
    MetalcraftAnyColor,
    FixedWithSourceDamage {
        output: FixedManaProfile,
        damage: u16,
    },
    ColorlessOrAnyColorWithSourceDamage {
        damage: u16,
    },
}

impl TypedConditionalManaSource {
    pub(crate) fn is_entry_linked(self) -> bool {
        matches!(
            self,
            Self::ImprintLinkedCardColors | Self::DiscardLandOrFailEntry
        )
    }

    pub(crate) fn is_receipted_artifact_family(self) -> bool {
        matches!(
            self,
            Self::ImprintLinkedCardColors
                | Self::DiscardLandOrFailEntry
                | Self::ControlledLegendaryColors
                | Self::MetalcraftAnyColor
        )
    }

    fn executor_id(self) -> Option<&'static str> {
        match self {
            Self::ImprintLinkedCardColors => {
                Some("abstract-play.conditional-mana.imprint-linked-card-colors")
            }
            Self::DiscardLandOrFailEntry => {
                Some("abstract-play.conditional-mana.discard-land-or-fail-entry")
            }
            Self::ControlledLegendaryColors => {
                Some("abstract-play.conditional-mana.controlled-legendary-colors")
            }
            Self::MetalcraftAnyColor => Some("abstract-play.conditional-mana.metalcraft-any-color"),
            Self::FixedWithSourceDamage { .. }
            | Self::ColorlessOrAnyColorWithSourceDamage { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConditionalManaSourceRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub source: TypedConditionalManaSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TypedSacrificeSelfMana {
    pub amount: u8,
    pub requires_tap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SacrificeSelfManaRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub program: TypedSacrificeSelfMana,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraveyardReclamationRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub program: ExecutableGraveyardReclamation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InteractionRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub program: InteractionRuntimeProgram,
}

impl InteractionRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        self.binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
            && self.binding.executor_version == INTERACTION_RUNTIME_EXECUTOR_VERSION
            && self.binding.executor_id == interaction_executor_id(&self.program)
            && self.capabilities
                == [
                    RuntimeCapability::CompleteOracleRoot,
                    RuntimeCapability::OrderedResolution,
                    RuntimeCapability::CounteredSpellResolutionBoundary,
                ]
            && self.source_evidence.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
            && self.source_evidence.has_exact_clause_contract()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TutorRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub program: TutorRuntimeProgram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestrictionProtectionRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub compiled: CompiledRestrictionProtection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundedOracleRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub clause: BoundedOracleClause,
    /// Snapshot-stable program identity supplied by the bounded Oracle
    /// compiler. Occurrence address and source evidence remain separate
    /// provenance and must never be folded into this digest.
    pub clause_semantic_sha256: String,
    pub consumer_version: &'static str,
    pub simulation_bridge_version: &'static str,
    pub mechanic_programs: Vec<MechanicProgram>,
    pub mechanic_contract_sha256: String,
}

impl BoundedOracleRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        let address = self.clause.address();
        self.binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
            && self.binding.executor_version == BOUNDED_ORACLE_RUNTIME_EXECUTOR_VERSION
            && self.binding.executor_id == bounded_oracle_executor_id(&self.clause)
            && self.consumer_version == BOUNDED_ORACLE_CONSUMER_VERSION
            && self.simulation_bridge_version == BOUNDED_ORACLE_SIMULATION_BRIDGE_VERSION
            && clause_has_executable_contract(&self.clause)
            && clause_has_live_bridge_contract(&self.clause)
            && bounded_mechanic_programs_have_exact_contract(&self.clause, &self.mechanic_programs)
            && self.capabilities
                == bounded_oracle_capabilities(
                    &self.clause,
                    &self.mechanic_programs,
                    &self.source_evidence,
                )
            && is_sha256_hex(&self.mechanic_contract_sha256)
            && self.mechanic_contract_sha256
                == bounded_mechanic_contract_sha256(&self.mechanic_programs)
            && self.clause.runtime_version() == BOUNDED_ORACLE_RUNTIME_VERSION
            && is_sha256_hex(&self.clause_semantic_sha256)
            && self.clause_semantic_sha256 == self.clause.semantic_digest()
            && self.source_evidence.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
            && self.source_evidence.has_exact_clause_contract()
            && self.source_evidence.covered_oracle_clauses.len() == 1
            && self.source_evidence.covered_oracle_clauses[0].face_index == address.face_index
            && self.source_evidence.covered_oracle_clauses[0].clause_index == address.clause_index
    }

    pub(crate) fn owns_exact_ability_word(&self, marker: &str) -> bool {
        self.has_exact_contract()
            && self.mechanic_programs.iter().any(|program| {
                matches!(
                    program.procedure(),
                    MechanicProcedure::AbilityWord(procedure)
                        if procedure.marker == PrintedMechanic::AbilityWord
                            && procedure.printed_label.eq_ignore_ascii_case(marker.trim())
                )
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewedRuntimeProgram {
    AlternativeCast(CompiledAlternativeCast),
    CharacteristicOracle(CompiledCharacteristicOracle),
    ContinuousTrigger(CompiledContinuousTrigger),
    ManaNetwork(ExactManaNetworkProgram),
    ObjectLifecycle(CompiledObjectLifecycle),
    UtilityModal(CompiledUtilityModal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewedRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub program: ReviewedRuntimeProgram,
}

impl ReviewedRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        self.binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
            && self.binding.executor_version == reviewed_runtime_executor_version(&self.program)
            && self.binding.executor_id == reviewed_runtime_executor_id(&self.program)
            && self.capabilities == reviewed_runtime_capabilities(&self.program)
            && self.source_evidence.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
            && self.source_evidence.has_exact_clause_contract()
            && match &self.program {
                ReviewedRuntimeProgram::ManaNetwork(program) => program.has_exact_contract(),
                _ => true,
            }
    }
}

impl RestrictionProtectionRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        self.binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
            && self.binding.executor_version == RESTRICTION_PROTECTION_EXECUTOR_VERSION
            && self.binding.executor_id
                == restriction_protection_executor_id(&self.compiled.program)
            && self.capabilities == restriction_protection_capabilities(&self.compiled)
            && self.source_evidence.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
            && self.source_evidence.has_exact_clause_contract()
    }
}

impl TutorRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        self.binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
            && self.binding.executor_version == TUTOR_RUNTIME_EXECUTOR_VERSION
            && self.binding.executor_id == tutor_executor_id(&self.program)
            && self.capabilities == tutor_runtime_capabilities(&self.program)
            && self.source_evidence.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
            && self.source_evidence.has_exact_clause_contract()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LiveAbilityShape {
    FlashPermission,
    Ward,
    AuraSpellTargeting,
    ScryResolution,
    StaticCreatureModifier,
    ControllerTokenTrigger,
    DrawTrigger,
    UpkeepTokenLife,
    QuestCounterLifecycle,
    CreatureTypeChoice,
    ChosenCreatureTypeMarker,
    TriggerMultiplier,
    SpellCostReduction,
    AlternativeSpellCost,
    EquipmentAttach,
    CumulativeUpkeep,
    FixedManaActivation,
    FixedDrawResolution,
    TemporaryPowerToughnessTrigger,
    BecomeMonarchTrigger,
    AllCreatureTypes,
    TableResourceTrigger,
    MillResolution,
    StormCopyTrigger,
}

impl LiveAbilityShape {
    pub(crate) fn executor_id(self) -> &'static str {
        match self {
            Self::FlashPermission => "abstract-play.ability.static.flash-permission",
            Self::Ward => "abstract-play.ability.trigger.ward",
            Self::AuraSpellTargeting => "abstract-play.ability.spell.aura-targeting",
            Self::ScryResolution => "abstract-play.ability.resolution.scry",
            Self::StaticCreatureModifier => "abstract-play.ability.static-creature-modifier",
            Self::ControllerTokenTrigger => "abstract-play.ability.trigger.controller-token",
            Self::DrawTrigger => "abstract-play.ability.trigger.draw",
            Self::UpkeepTokenLife => "abstract-play.ability.trigger.upkeep-token-life",
            Self::QuestCounterLifecycle => "abstract-play.ability.lifecycle.quest-counter-token",
            Self::CreatureTypeChoice => "abstract-play.ability.static.creature-type-choice",
            Self::ChosenCreatureTypeMarker => "abstract-play.ability.static.chosen-creature-type",
            Self::TriggerMultiplier => "abstract-play.ability.static.trigger-multiplier",
            Self::SpellCostReduction => "abstract-play.ability.static.spell-cost-reduction",
            Self::AlternativeSpellCost => "abstract-play.ability.static.alternative-spell-cost",
            Self::EquipmentAttach => "abstract-play.ability.activated.equip",
            Self::CumulativeUpkeep => "abstract-play.ability.trigger.cumulative-upkeep",
            Self::FixedManaActivation => "abstract-play.ability.activated.fixed-mana",
            Self::FixedDrawResolution => "abstract-play.ability.resolution.fixed-draw",
            Self::TemporaryPowerToughnessTrigger => {
                "abstract-play.ability.trigger.temporary-power-toughness"
            }
            Self::BecomeMonarchTrigger => "abstract-play.ability.trigger.become-monarch",
            Self::AllCreatureTypes => "abstract-play.ability.static.all-creature-types",
            Self::TableResourceTrigger => "abstract-play.ability.trigger.table-resource",
            Self::MillResolution => "abstract-play.ability.resolution.mill",
            Self::StormCopyTrigger => "abstract-play.ability.trigger.storm-copy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveAbilityRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub shape: LiveAbilityShape,
    pub abilities: Vec<ExecutableAbility>,
}

impl LiveAbilityRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        self.binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
            && self.binding.executor_version == LIVE_ABILITY_EXECUTOR_VERSION
            && self.binding.executor_id == self.shape.executor_id()
            && self.capabilities == live_ability_capabilities(self.shape)
            && self.source_evidence.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
            && self.source_evidence.has_exact_clause_contract()
            && self.abilities.len() == self.source_evidence.covered_oracle_clauses.len()
            && live_receipt_shape_matches(self.shape, &self.abilities)
    }

    pub(crate) fn owns_exact_treasure_token_keyword(&self) -> bool {
        self.has_exact_contract()
            && self
                .abilities
                .iter()
                .flat_map(|ability| ability.effects.iter())
                .any(effect_creates_exact_treasure)
    }
}

fn live_ability_capabilities(shape: LiveAbilityShape) -> Vec<RuntimeCapability> {
    let mut capabilities = vec![RuntimeCapability::ExactOracleClauseSet];
    match shape {
        LiveAbilityShape::EquipmentAttach => {
            capabilities.push(RuntimeCapability::ExactEquipKeyword);
        }
        LiveAbilityShape::MillResolution => {
            capabilities.push(RuntimeCapability::ExactMillKeyword);
        }
        LiveAbilityShape::StormCopyTrigger => {
            capabilities.push(RuntimeCapability::ExactStormKeyword);
        }
        _ => {}
    }
    capabilities
}

fn effect_creates_exact_treasure(effect: &AbilityEffect) -> bool {
    match effect {
        AbilityEffect::CreateToken(token) => token.count > 0 && token.kind == TokenKind::Treasure,
        AbilityEffect::UnlessEventPlayerPays(unless) => {
            unless.if_not_paid.iter().any(effect_creates_exact_treasure)
        }
        AbilityEffect::Conditional(conditional) => {
            conditional
                .if_true
                .iter()
                .any(effect_creates_exact_treasure)
                || conditional
                    .if_false
                    .iter()
                    .any(effect_creates_exact_treasure)
        }
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LandRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub subject: ExactLandRuntimeSubject,
    pub contract_sha256: String,
}

impl LandRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        let Some(subject_payload) = exact_land_subject_payload(&self.subject) else {
            return false;
        };
        let expected_capability = land_subject_capability(&self.subject);
        let exact_clause_shape = match &self.subject {
            ExactLandRuntimeSubject::BasicTypeMana { .. } => {
                self.source_evidence.covered_oracle_clauses.len() == 1
            }
            ExactLandRuntimeSubject::FixedPrintedMana { .. } => {
                !self.source_evidence.covered_oracle_clauses.is_empty()
            }
            ExactLandRuntimeSubject::AlwaysTappedEntry
            | ExactLandRuntimeSubject::PayTwoLifeOrTappedEntry { .. }
            | ExactLandRuntimeSubject::UntappedWithAtLeastOpponents { .. }
            | ExactLandRuntimeSubject::FetchTwoBasicLandTypes { .. } => {
                self.source_evidence.covered_oracle_clauses.len() == 1
            }
        };
        let exact_source_shape =
            if matches!(self.subject, ExactLandRuntimeSubject::BasicTypeMana { .. }) {
                let payload_sha256 = sha256_hex(subject_payload.as_bytes());
                self.source_evidence.normalized_oracle_sha256 == payload_sha256
                    && self.source_evidence.normalized_oracle_clause_sha256s
                        == [payload_sha256.clone()]
                    && self.source_evidence.covered_oracle_clauses
                        == [RuntimeOracleClauseEvidence {
                            face_index: 0,
                            clause_index: 0,
                            normalized_clause_sha256: payload_sha256,
                        }]
            } else {
                true
            };
        self.binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
            && self.binding.executor_version == LAND_RUNTIME_EXECUTOR_VERSION
            && self.binding.executor_id == self.subject.executor_id()
            && self.capabilities == [expected_capability]
            && self.source_evidence.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
            && is_sha256_hex(&self.source_evidence.normalized_oracle_sha256)
            && self.source_evidence.has_exact_clause_contract()
            && is_sha256_hex(&self.source_evidence.type_line_sha256)
            && is_sha256_hex(&self.source_evidence.source_evidence_sha256)
            && self.source_evidence.relevant_type_role_mask & role::LAND != 0
            && exact_clause_shape
            && exact_source_shape
            && is_sha256_hex(&self.contract_sha256)
            && self.contract_sha256
                == land_receipt_contract_sha256(
                    &self.binding,
                    &self.capabilities,
                    &self.source_evidence,
                    &subject_payload,
                )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CharacteristicSubject {
    FaceRelationship,
    ManaCost,
    ManaValue,
    Colors,
    ColorIndicator,
    Power,
    Toughness,
    Loyalty,
    Defense,
    HandModifier,
    LifeModifier,
    AttractionLights,
    CardTypeProfile,
    PrintedCombatKeyword(String),
}

impl CharacteristicSubject {
    pub(crate) fn executor_id(&self) -> &'static str {
        match self {
            Self::FaceRelationship => "abstract-play.characteristic.double-faced-relationship",
            Self::ManaCost => "abstract-play.characteristic.mana-cost",
            Self::ManaValue => "abstract-play.characteristic.mana-value",
            Self::Colors => "abstract-play.characteristic.colors",
            Self::ColorIndicator => "abstract-play.characteristic.color-indicator",
            Self::Power => "abstract-play.characteristic.power",
            Self::Toughness => "abstract-play.characteristic.toughness",
            Self::Loyalty => "abstract-play.characteristic.loyalty",
            Self::Defense => "abstract-play.characteristic.defense",
            Self::HandModifier => "abstract-play.characteristic.hand-modifier",
            Self::LifeModifier => "abstract-play.characteristic.life-modifier",
            Self::AttractionLights => "abstract-play.characteristic.attraction-lights",
            Self::CardTypeProfile => "abstract-play.characteristic.card-type-profile",
            Self::PrintedCombatKeyword(keyword) => match keyword.as_str() {
                "deathtouch" => "abstract-play.characteristic.printed-combat-keyword.deathtouch",
                "double strike" => {
                    "abstract-play.characteristic.printed-combat-keyword.double-strike"
                }
                "first strike" => {
                    "abstract-play.characteristic.printed-combat-keyword.first-strike"
                }
                "flying" => "abstract-play.characteristic.printed-combat-keyword.flying",
                "haste" => "abstract-play.characteristic.printed-combat-keyword.haste",
                "hexproof" => "abstract-play.characteristic.printed-combat-keyword.hexproof",
                "indestructible" => {
                    "abstract-play.characteristic.printed-combat-keyword.indestructible"
                }
                "lifelink" => "abstract-play.characteristic.printed-combat-keyword.lifelink",
                "menace" => "abstract-play.characteristic.printed-combat-keyword.menace",
                "reach" => "abstract-play.characteristic.printed-combat-keyword.reach",
                "shroud" => "abstract-play.characteristic.printed-combat-keyword.shroud",
                "trample" => "abstract-play.characteristic.printed-combat-keyword.trample",
                "vigilance" => "abstract-play.characteristic.printed-combat-keyword.vigilance",
                "defender" => "abstract-play.characteristic.printed-combat-keyword.defender",
                "equip" => "abstract-play.characteristic.printed-keyword.equip",
                "cumulative upkeep" => {
                    "abstract-play.characteristic.printed-keyword.cumulative-upkeep"
                }
                "devoid" => "abstract-play.characteristic.printed-keyword.devoid",
                _ => "abstract-play.characteristic.printed-combat-keyword.unsupported",
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExactManaCostShape {
    Absent,
    Fixed {
        symbols: Vec<String>,
        generic_value: u16,
        pip_appearances: [u16; 6],
    },
    /// A fully parsed single-face cost that contains variable or hybrid
    /// symbols. The mana model consumes the same ordered symbols when it
    /// enumerates legal payments, so this is an executable characteristic,
    /// not a flattened mana-value approximation.
    Structured {
        symbols: Vec<String>,
        generic_value: u16,
        pip_appearances: [u16; 6],
        variable_symbols: u16,
        hybrid_symbols: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypedCharacteristic {
    FaceRelationship(CharacteristicFaceBinding),
    ManaCost(ExactManaCostShape),
    ManaValue(ExactManaValueProcedure),
    Colors(ExactColorSetProcedure),
    ColorIndicator(ExactColorSetProcedure),
    Power {
        procedure: PrintedStatProcedure,
        dynamic: Option<DynamicCharacteristicProcedure>,
    },
    Toughness {
        procedure: PrintedStatProcedure,
        dynamic: Option<DynamicCharacteristicProcedure>,
    },
    ToughnessEqualsDevotion {
        color: DevotionColor,
    },
    Loyalty {
        procedure: LoyaltyInitializationProcedure,
        dynamic: Option<DynamicCharacteristicProcedure>,
    },
    Defense(DefenseInitializationProcedure),
    HandModifier(VanguardModifierProcedure),
    LifeModifier(VanguardModifierProcedure),
    AttractionLights(AttractionLightsProcedure),
    CardTypeProfile {
        procedure: ExactTypeLineProcedure,
    },
    PrintedCombatKeyword {
        keyword: PrintedKeyword,
        complete_profile_sha256: String,
        runtime_clause_sha256: Option<String>,
    },
}

impl TypedCharacteristic {
    fn matches_subject(&self, subject: &CharacteristicSubject) -> bool {
        matches!(
            (self, subject),
            (
                Self::FaceRelationship(_),
                CharacteristicSubject::FaceRelationship
            ) | (Self::ManaCost(_), CharacteristicSubject::ManaCost)
                | (Self::ManaValue(_), CharacteristicSubject::ManaValue)
                | (Self::Colors(_), CharacteristicSubject::Colors)
                | (
                    Self::ColorIndicator(_),
                    CharacteristicSubject::ColorIndicator
                )
                | (Self::Power { .. }, CharacteristicSubject::Power)
                | (Self::Toughness { .. }, CharacteristicSubject::Toughness)
                | (
                    Self::ToughnessEqualsDevotion { .. },
                    CharacteristicSubject::Toughness
                )
                | (Self::Loyalty { .. }, CharacteristicSubject::Loyalty)
                | (Self::Defense(_), CharacteristicSubject::Defense)
                | (Self::HandModifier(_), CharacteristicSubject::HandModifier)
                | (Self::LifeModifier(_), CharacteristicSubject::LifeModifier)
                | (
                    Self::AttractionLights(_),
                    CharacteristicSubject::AttractionLights
                )
                | (
                    Self::CardTypeProfile { .. },
                    CharacteristicSubject::CardTypeProfile
                )
                | (
                    Self::PrintedCombatKeyword { .. },
                    CharacteristicSubject::PrintedCombatKeyword(_)
                )
        )
    }

    fn evidence_payload(&self) -> String {
        match self {
            Self::FaceRelationship(binding) => {
                format!("face-relationship:{}", binding.evidence_tag())
            }
            Self::ManaCost(ExactManaCostShape::Absent) => "mana-cost:absent".into(),
            Self::ManaCost(ExactManaCostShape::Fixed {
                symbols,
                generic_value,
                pip_appearances,
            }) => format!(
                "mana-cost:fixed:{}:{generic_value}:{pip_appearances:?}",
                symbols.join(",")
            ),
            Self::ManaCost(ExactManaCostShape::Structured {
                symbols,
                generic_value,
                pip_appearances,
                variable_symbols,
                hybrid_symbols,
            }) => format!(
                "mana-cost:structured:{}:{generic_value}:{pip_appearances:?}:x{variable_symbols}:hybrid{hybrid_symbols}",
                symbols.join(",")
            ),
            Self::ManaValue(procedure) => procedure.canonical_evidence_payload(),
            Self::Colors(procedure) => procedure.canonical_evidence_payload("colors"),
            Self::ColorIndicator(procedure) => {
                procedure.canonical_evidence_payload("color-indicator")
            }
            Self::Power { procedure, dynamic } => format!(
                "{}:{}",
                procedure.canonical_evidence_payload("power"),
                dynamic.as_ref().map_or_else(
                    || "dynamic=none".into(),
                    |value| value.canonical_evidence_payload()
                )
            ),
            Self::Toughness { procedure, dynamic } => format!(
                "{}:{}",
                procedure.canonical_evidence_payload("toughness"),
                dynamic.as_ref().map_or_else(
                    || "dynamic=none".into(),
                    |value| value.canonical_evidence_payload()
                )
            ),
            Self::ToughnessEqualsDevotion { color } => {
                format!("toughness:devotion:{}", color.as_name())
            }
            Self::Loyalty { procedure, dynamic } => format!(
                "{}:{}",
                procedure.canonical_evidence_payload(),
                dynamic.as_ref().map_or_else(
                    || "dynamic=none".into(),
                    |value| value.canonical_evidence_payload()
                )
            ),
            Self::Defense(procedure) => procedure.canonical_evidence_payload(),
            Self::HandModifier(procedure) => procedure.canonical_evidence_payload("hand-modifier"),
            Self::LifeModifier(procedure) => procedure.canonical_evidence_payload("life-modifier"),
            Self::AttractionLights(procedure) => procedure.canonical_evidence_payload(),
            Self::CardTypeProfile { procedure } => procedure.canonical_evidence_payload(),
            Self::PrintedCombatKeyword {
                keyword,
                complete_profile_sha256,
                runtime_clause_sha256,
            } => format!(
                "printed-keyword:{keyword:?}:{complete_profile_sha256}:{}",
                runtime_clause_sha256.as_deref().unwrap_or("characteristic")
            ),
        }
    }

    fn has_live_consumer_without_additional_input(&self) -> bool {
        match self {
            Self::Power {
                procedure: PrintedStatProcedure::Fixed(_) | PrintedStatProcedure::Infinite,
                ..
            }
            | Self::Toughness {
                procedure: PrintedStatProcedure::Fixed(_) | PrintedStatProcedure::Infinite,
                ..
            } => true,
            Self::Power {
                dynamic: Some(_), ..
            }
            | Self::Toughness {
                dynamic: Some(_), ..
            } => true,
            Self::Power { .. } | Self::Toughness { .. } => false,
            Self::Loyalty { procedure, dynamic } => {
                !procedure.requires_live_input() || dynamic.is_some()
            }
            Self::FaceRelationship(_)
            | Self::ManaCost(_)
            | Self::ManaValue(_)
            | Self::Colors(_)
            | Self::ColorIndicator(_)
            | Self::ToughnessEqualsDevotion { .. }
            | Self::Defense(_)
            | Self::HandModifier(_)
            | Self::LifeModifier(_)
            | Self::AttractionLights(_)
            | Self::CardTypeProfile { .. } => true,
            // This receipt proves retained characteristic metadata only.
            // Functional keyword execution requires a separate
            // `KeywordRulesRuntimeReceipt` plus a production metric adapter.
            Self::PrintedCombatKeyword { .. } => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CharacteristicRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub face_index: u16,
    pub face_binding: CharacteristicFaceBinding,
    pub subject: CharacteristicSubject,
    pub characteristic: TypedCharacteristic,
}

impl CharacteristicRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        self.binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
            && self.binding.executor_version == CHARACTERISTIC_EXECUTOR_VERSION
            && self.binding.executor_id == self.subject.executor_id()
            && self.capabilities == [RuntimeCapability::ExactCompiledCharacteristic]
            && self.face_binding.matches_face_index(self.face_index)
            && self.characteristic.matches_subject(&self.subject)
            && characteristic_face_binding_matches(&self.characteristic, self.face_binding)
            && characteristic_keyword_matches_subject(&self.characteristic, &self.subject)
            && characteristic_keyword_has_exact_runtime_contract(&self.characteristic)
            && self.source_evidence.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
            && is_sha256_hex(&self.source_evidence.normalized_oracle_sha256)
            && self.source_evidence.normalized_oracle_clause_sha256s.len() == 1
            && self.source_evidence.has_exact_clause_contract()
            && self.source_evidence.covered_oracle_clauses.len() == 1
            && self.source_evidence.covered_oracle_clauses[0].face_index == self.face_index
            && self.source_evidence.covered_oracle_clauses[0].clause_index == 0
            && self.source_evidence.covered_oracle_clauses[0].normalized_clause_sha256
                == self.source_evidence.normalized_oracle_clause_sha256s[0]
            && is_sha256_hex(&self.source_evidence.type_line_sha256)
            && is_sha256_hex(&self.source_evidence.source_evidence_sha256)
    }

    pub(crate) fn has_live_consumer_without_additional_input(&self) -> bool {
        self.has_exact_contract()
            && self
                .characteristic
                .has_live_consumer_without_additional_input()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CharacteristicFaceBinding {
    NormalSingleFace,
    ModalDoubleFacedFront,
    ModalDoubleFacedBack,
    TransformDoubleFacedFront,
    TransformDoubleFacedBack,
    ExactFace(u16),
}

impl CharacteristicFaceBinding {
    fn matches_face_index(self, face_index: u16) -> bool {
        matches!(
            (self, face_index),
            (
                Self::NormalSingleFace
                    | Self::ModalDoubleFacedFront
                    | Self::TransformDoubleFacedFront,
                0
            ) | (
                Self::ModalDoubleFacedBack | Self::TransformDoubleFacedBack,
                1
            ) | (Self::ExactFace(_), _)
        ) && !matches!(self, Self::ExactFace(expected) if expected != face_index)
    }

    fn evidence_tag(self) -> String {
        match self {
            Self::NormalSingleFace => "normal-single-face".into(),
            Self::ModalDoubleFacedFront => "modal-double-faced-front".into(),
            Self::ModalDoubleFacedBack => "modal-double-faced-back".into(),
            Self::TransformDoubleFacedFront => "transform-double-faced-front".into(),
            Self::TransformDoubleFacedBack => "transform-double-faced-back".into(),
            Self::ExactFace(face_index) => format!("exact-face-{face_index}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CharacteristicFaceInput<'a> {
    pub face_index: u16,
    pub face_binding: Option<CharacteristicFaceBinding>,
    pub layout: &'a str,
    pub name: &'a str,
    pub oracle_text: &'a str,
    pub mana_cost: Option<&'a str>,
    pub mana_value: Option<f32>,
    pub colors: &'a [String],
    pub color_indicator: &'a [String],
    pub power: Option<&'a str>,
    pub toughness: Option<&'a str>,
    pub loyalty: Option<&'a str>,
    pub defense: Option<&'a str>,
    pub hand_modifier: Option<&'a str>,
    pub life_modifier: Option<&'a str>,
    pub attraction_lights: &'a [u8],
    pub type_line: Option<&'a str>,
    pub keywords: &'a [String],
    pub root_alignment: CharacteristicRootAlignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CharacteristicRootAlignment {
    pub mana_cost: bool,
    pub mana_value: bool,
    pub colors: bool,
    pub color_indicator: bool,
    pub power: bool,
    pub toughness: bool,
    pub loyalty: bool,
    pub defense: bool,
    pub hand_modifier: bool,
    pub life_modifier: bool,
    pub attraction_lights: bool,
    pub type_line: bool,
    pub keywords: bool,
}

impl CharacteristicRootAlignment {
    pub(crate) const EXACT: Self = Self {
        mana_cost: true,
        mana_value: true,
        colors: true,
        color_indicator: true,
        power: true,
        toughness: true,
        loyalty: true,
        defense: true,
        hand_modifier: true,
        life_modifier: true,
        attraction_lights: true,
        type_line: true,
        keywords: true,
    };
}

pub(crate) fn compile_characteristic_runtime_receipts(
    input: CharacteristicFaceInput<'_>,
) -> Vec<CharacteristicRuntimeReceipt> {
    let Some(face_binding) = input
        .face_binding
        .filter(|binding| binding.matches_face_index(input.face_index))
    else {
        return Vec::new();
    };
    let Some(type_line) = input
        .type_line
        .map(str::trim)
        .filter(|type_line| !type_line.is_empty())
    else {
        return Vec::new();
    };
    let compiled_type_profile = compile_card_types(type_line);

    let mut characteristics = Vec::<(CharacteristicSubject, TypedCharacteristic)>::new();
    if !matches!(face_binding, CharacteristicFaceBinding::NormalSingleFace) {
        characteristics.push((
            CharacteristicSubject::FaceRelationship,
            TypedCharacteristic::FaceRelationship(face_binding),
        ));
    }
    if input.root_alignment.mana_cost
        && let Some(cost) = exact_mana_cost_shape(input.mana_cost)
    {
        characteristics.push((
            CharacteristicSubject::ManaCost,
            TypedCharacteristic::ManaCost(cost),
        ));
    }
    if input.root_alignment.mana_value
        && let Some(mana_value) = input
            .mana_value
            .and_then(compile_exact_mana_value_procedure)
    {
        characteristics.push((
            CharacteristicSubject::ManaValue,
            TypedCharacteristic::ManaValue(mana_value),
        ));
    }
    if input.root_alignment.colors
        && let Some(colors) = compile_exact_color_set_procedure(input.colors)
    {
        characteristics.push((
            CharacteristicSubject::Colors,
            TypedCharacteristic::Colors(colors),
        ));
    }
    if input.root_alignment.color_indicator
        && let Some(color_indicator) = compile_exact_color_set_procedure(input.color_indicator)
    {
        characteristics.push((
            CharacteristicSubject::ColorIndicator,
            TypedCharacteristic::ColorIndicator(color_indicator),
        ));
    }
    if input.root_alignment.type_line
        && let Some(procedure) = compile_exact_type_line_procedure(type_line)
    {
        characteristics.push((
            CharacteristicSubject::CardTypeProfile,
            TypedCharacteristic::CardTypeProfile { procedure },
        ));
    }

    if input.root_alignment.power
        && let Some(procedure) = input
            .power
            .and_then(|value| compile_exact_printed_stat_procedure(input.layout, value))
    {
        let dynamic = compile_dynamic_printed_stat_procedure(
            input.layout,
            input.oracle_text,
            procedure,
            DynamicCharacteristicSubject::Power,
        );
        characteristics.push((
            CharacteristicSubject::Power,
            TypedCharacteristic::Power { procedure, dynamic },
        ));
    }
    if input.root_alignment.toughness {
        if let Some(DynamicCreatureCharacteristic::ToughnessEqualsDevotion(color)) =
            compile_dynamic_creature_characteristic(
                input.name,
                type_line,
                input.oracle_text,
                input.toughness,
            )
        {
            characteristics.push((
                CharacteristicSubject::Toughness,
                TypedCharacteristic::ToughnessEqualsDevotion { color },
            ));
        } else if let Some(procedure) = input
            .toughness
            .and_then(|value| compile_exact_printed_stat_procedure(input.layout, value))
        {
            let dynamic = compile_dynamic_printed_stat_procedure(
                input.layout,
                input.oracle_text,
                procedure,
                DynamicCharacteristicSubject::Toughness,
            );
            characteristics.push((
                CharacteristicSubject::Toughness,
                TypedCharacteristic::Toughness { procedure, dynamic },
            ));
        }
    }
    if input.root_alignment.loyalty
        && let Some(procedure) = input
            .loyalty
            .and_then(compile_exact_loyalty_initialization_procedure)
    {
        let dynamic = compile_dynamic_loyalty_procedure(input.oracle_text, procedure);
        characteristics.push((
            CharacteristicSubject::Loyalty,
            TypedCharacteristic::Loyalty { procedure, dynamic },
        ));
    }
    if input.root_alignment.defense
        && let Some(procedure) = input
            .defense
            .and_then(compile_exact_defense_initialization_procedure)
    {
        characteristics.push((
            CharacteristicSubject::Defense,
            TypedCharacteristic::Defense(procedure),
        ));
    }
    if input.root_alignment.hand_modifier
        && let Some(procedure) = input
            .hand_modifier
            .and_then(compile_exact_vanguard_modifier_procedure)
    {
        characteristics.push((
            CharacteristicSubject::HandModifier,
            TypedCharacteristic::HandModifier(procedure),
        ));
    }
    if input.root_alignment.life_modifier
        && let Some(procedure) = input
            .life_modifier
            .and_then(compile_exact_vanguard_modifier_procedure)
    {
        characteristics.push((
            CharacteristicSubject::LifeModifier,
            TypedCharacteristic::LifeModifier(procedure),
        ));
    }
    if input.root_alignment.attraction_lights
        && let Some(procedure) = compile_exact_attraction_lights_procedure(input.attraction_lights)
    {
        characteristics.push((
            CharacteristicSubject::AttractionLights,
            TypedCharacteristic::AttractionLights(procedure),
        ));
    }

    let canonical_keywords = if input.root_alignment.keywords {
        canonical_keyword_names(input.keywords)
    } else {
        Vec::new()
    };
    let complete_profile_sha256 = sha256_hex(canonical_keywords.join("\n").as_bytes());
    let printed_profile = compile_printed_keyword_profile(&canonical_keywords);
    for keyword_name in canonical_keywords {
        let Some(keyword) = PrintedKeyword::from_name(&keyword_name) else {
            continue;
        };
        let runtime_clause_sha256 = exact_printed_keyword_runtime_clause(input, keyword);
        if (!is_exact_printed_combat_keyword(keyword) && runtime_clause_sha256.is_none())
            || !printed_profile.contains(keyword)
        {
            continue;
        }
        characteristics.push((
            CharacteristicSubject::PrintedCombatKeyword(keyword_name.to_ascii_lowercase()),
            TypedCharacteristic::PrintedCombatKeyword {
                keyword,
                complete_profile_sha256: complete_profile_sha256.clone(),
                runtime_clause_sha256,
            },
        ));
    }

    let relevant_type_role_mask = role_mask_for_type_profile(compiled_type_profile);
    let mut receipts = characteristics
        .into_iter()
        .map(|(subject, characteristic)| {
            characteristic_receipt(
                input.face_index,
                face_binding,
                type_line,
                relevant_type_role_mask,
                subject,
                characteristic,
            )
        })
        .collect::<Vec<_>>();
    receipts.sort_by(|left, right| {
        left.subject
            .cmp(&right.subject)
            .then_with(|| left.binding.executor_id.cmp(right.binding.executor_id))
    });
    receipts
}

fn characteristic_receipt(
    face_index: u16,
    face_binding: CharacteristicFaceBinding,
    type_line: &str,
    relevant_type_role_mask: u32,
    subject: CharacteristicSubject,
    characteristic: TypedCharacteristic,
) -> CharacteristicRuntimeReceipt {
    let executor_id = subject.executor_id();
    let payload = characteristic.evidence_payload();
    let payload_sha256 = sha256_hex(payload.as_bytes());
    let mut hasher = Sha256::new();
    for part in [
        RUNTIME_RECEIPT_SCHEMA_VERSION.as_bytes(),
        CHARACTERISTIC_EXECUTOR_VERSION.as_bytes(),
        STRUCTURAL_CHARACTERISTIC_RUNTIME_VERSION.as_bytes(),
        DYNAMIC_CHARACTERISTIC_RUNTIME_VERSION.as_bytes(),
        EXECUTABLE_ABILITY_PROGRAM_VERSION.as_bytes(),
        executor_id.as_bytes(),
        type_line.as_bytes(),
        payload.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    hash_framed(&mut hasher, &face_index.to_be_bytes());
    hash_framed(&mut hasher, face_binding.evidence_tag().as_bytes());
    hash_framed(&mut hasher, &relevant_type_role_mask.to_be_bytes());
    hash_framed(&mut hasher, &0u16.to_be_bytes());
    hash_framed(&mut hasher, payload_sha256.as_bytes());
    CharacteristicRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id,
            executor_version: CHARACTERISTIC_EXECUTOR_VERSION,
        },
        capabilities: vec![RuntimeCapability::ExactCompiledCharacteristic],
        source_evidence: RuntimeSourceEvidence {
            ability_program_version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
            normalized_oracle_sha256: payload_sha256.clone(),
            normalized_oracle_clause_sha256s: vec![payload_sha256.clone()],
            covered_oracle_clauses: vec![RuntimeOracleClauseEvidence {
                face_index,
                clause_index: 0,
                normalized_clause_sha256: payload_sha256,
            }],
            type_line_sha256: sha256_hex(type_line.as_bytes()),
            relevant_type_role_mask,
            source_evidence_sha256: format!("{:x}", hasher.finalize()),
        },
        face_index,
        face_binding,
        subject,
        characteristic,
    }
}

pub(crate) fn compile_printed_cost_runtime_receipt(
    face_index: u16,
    type_line: &str,
    printed_cost: &str,
) -> Option<PrintedCostRuntimeReceipt> {
    let program = parse_printed_mana_cost(printed_cost).ok()?;
    let [face] = program.faces.as_slice() else {
        return None;
    };
    if !face.has_mana_cost
        || face.raw != program.raw
        || !printed_mana_cost_has_exact_payment_contract(&program)
    {
        return None;
    }
    let printed_sha256 = sha256_hex(program.raw.as_bytes());
    let contract_sha256 = printed_cost_contract_sha256(
        face_index,
        type_line,
        &program,
        PRINTED_COST_RUNTIME_VERSION,
        PRINTED_COST_PAYMENT_BRIDGE_VERSION,
    );
    let receipt = PrintedCostRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id: PRINTED_COST_RUNTIME_EXECUTOR_ID,
            executor_version: PRINTED_COST_RUNTIME_EXECUTOR_VERSION,
        },
        capabilities: vec![
            RuntimeCapability::ExactPrintedManaCost,
            RuntimeCapability::TransactionalPrintedManaPayment,
        ],
        source_evidence: RuntimeSourceEvidence {
            ability_program_version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
            normalized_oracle_sha256: printed_sha256.clone(),
            normalized_oracle_clause_sha256s: vec![printed_sha256.clone()],
            covered_oracle_clauses: vec![RuntimeOracleClauseEvidence {
                face_index,
                clause_index: 0,
                normalized_clause_sha256: printed_sha256,
            }],
            type_line_sha256: sha256_hex(type_line.as_bytes()),
            relevant_type_role_mask: 0,
            source_evidence_sha256: contract_sha256.clone(),
        },
        face_index,
        type_line: type_line.to_owned(),
        program,
        printed_cost_runtime_version: PRINTED_COST_RUNTIME_VERSION,
        payment_bridge_version: PRINTED_COST_PAYMENT_BRIDGE_VERSION,
        contract_sha256,
    };
    receipt.has_exact_contract().then_some(receipt)
}

fn printed_cost_contract_sha256(
    face_index: u16,
    type_line: &str,
    program: &PrintedManaCost,
    printed_cost_runtime_version: &str,
    payment_bridge_version: &str,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        RUNTIME_RECEIPT_SCHEMA_VERSION.as_bytes(),
        PRINTED_COST_RUNTIME_EXECUTOR_ID.as_bytes(),
        PRINTED_COST_RUNTIME_EXECUTOR_VERSION.as_bytes(),
        EXECUTABLE_ABILITY_PROGRAM_VERSION.as_bytes(),
        printed_cost_runtime_version.as_bytes(),
        payment_bridge_version.as_bytes(),
        type_line.as_bytes(),
        program.raw.as_bytes(),
        format!("{program:?}").as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    hash_framed(&mut hasher, &face_index.to_be_bytes());
    format!("{:x}", hasher.finalize())
}

fn exact_mana_cost_shape(mana_cost: Option<&str>) -> Option<ExactManaCostShape> {
    let Some(mana_cost) = mana_cost else {
        return Some(ExactManaCostShape::Absent);
    };
    let trimmed = mana_cost.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = parse_mana_cost(Some(trimmed));
    let [face] = parsed.faces.as_slice() else {
        return None;
    };
    if parsed.confidence != 1.0
        || face.confidence != 1.0
        || face.pips.is_empty()
        || face
            .pips
            .iter()
            .any(|pip| pip.is_phyrexian || pip.is_snow || pip.is_unknown)
    {
        return None;
    }
    let symbols = face
        .pips
        .iter()
        .map(|pip| {
            let symbol = pip.raw.trim().to_ascii_uppercase();
            let canonical = if pip.is_variable && symbol == "X" {
                symbol.clone()
            } else if pip.is_hybrid {
                let parts = symbol.split('/').collect::<Vec<_>>();
                if parts.len() < 2
                    || parts.iter().any(|part| {
                        !matches!(*part, "W" | "U" | "B" | "R" | "G" | "C")
                            && part.parse::<u16>().is_err()
                    })
                {
                    return None;
                }
                parts.join("/")
            } else if let Ok(generic) = symbol.parse::<u16>() {
                generic.to_string()
            } else if matches!(symbol.as_str(), "W" | "U" | "B" | "R" | "G" | "C") {
                symbol.clone()
            } else {
                return None;
            };
            (canonical == symbol).then_some(canonical)
        })
        .collect::<Option<Vec<_>>>()?;
    let compact_source = trimmed
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect::<String>();
    let reconstructed = symbols
        .iter()
        .map(|symbol| format!("{{{symbol}}}"))
        .collect::<String>();
    if compact_source != reconstructed {
        return None;
    }
    let variable_symbols =
        u16::try_from(face.pips.iter().filter(|pip| pip.is_variable).count()).ok()?;
    let hybrid_symbols =
        u16::try_from(face.pips.iter().filter(|pip| pip.is_hybrid).count()).ok()?;
    if variable_symbols > 0 || hybrid_symbols > 0 {
        Some(ExactManaCostShape::Structured {
            symbols,
            generic_value: face.generic_value,
            pip_appearances: face.pip_appearances,
            variable_symbols,
            hybrid_symbols,
        })
    } else {
        Some(ExactManaCostShape::Fixed {
            symbols,
            generic_value: face.generic_value,
            pip_appearances: face.pip_appearances,
        })
    }
}

fn canonical_keyword_names(keywords: &[String]) -> Vec<String> {
    let mut canonical = keywords
        .iter()
        .map(|keyword| keyword.trim().to_string())
        .filter(|keyword| !keyword.is_empty())
        .collect::<Vec<_>>();
    canonical.sort_by_key(|keyword| keyword.to_ascii_lowercase());
    canonical.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    canonical
}

fn is_exact_printed_combat_keyword(keyword: PrintedKeyword) -> bool {
    matches!(
        keyword,
        PrintedKeyword::Deathtouch
            | PrintedKeyword::DoubleStrike
            | PrintedKeyword::FirstStrike
            | PrintedKeyword::Flying
            | PrintedKeyword::Haste
            | PrintedKeyword::Hexproof
            | PrintedKeyword::Indestructible
            | PrintedKeyword::Lifelink
            | PrintedKeyword::Menace
            | PrintedKeyword::Reach
            | PrintedKeyword::Shroud
            | PrintedKeyword::Trample
            | PrintedKeyword::Vigilance
            | PrintedKeyword::Defender
    )
}

fn exact_printed_keyword_runtime_clause(
    input: CharacteristicFaceInput<'_>,
    keyword: PrintedKeyword,
) -> Option<String> {
    if keyword == PrintedKeyword::Devoid {
        if !input.colors.is_empty() {
            return None;
        }
        let mut matching = input.oracle_text.lines().filter_map(|clause| {
            let normalized =
                normalize_oracle_clause_for_receipt(clause, input.name, input.type_line?);
            matches!(
                normalized.trim().to_ascii_lowercase().as_str(),
                "devoid"
                    | "devoid (this card has no color.)"
                    | "devoid (this object has no color.)"
            )
            .then_some(normalized)
        });
        let clause = matching.next()?;
        return matching
            .next()
            .is_none()
            .then(|| sha256_hex(clause.as_bytes()));
    }
    if !matches!(
        keyword,
        PrintedKeyword::Equip | PrintedKeyword::CumulativeUpkeep
    ) {
        return None;
    }
    let type_line = input.type_line?;
    let program = compile_executable_ability_program(OracleCardInput {
        name: input.name,
        layout: "normal",
        type_line,
        oracle_text: input.oracle_text,
        has_face_records: false,
    });
    if program.version != EXECUTABLE_ABILITY_PROGRAM_VERSION || !program.face_programs.is_empty() {
        return None;
    }
    let mut matching = program.abilities.iter().filter(|compilation| {
        let normalized_oracle = match compilation {
            AbilityCompilation::Executable(ability) => &ability.normalized_oracle,
            AbilityCompilation::Unsupported(ability) => &ability.normalized_oracle,
        };
        let lower = normalized_oracle.trim().to_ascii_lowercase();
        match keyword {
            PrintedKeyword::Equip => lower.starts_with("equip "),
            PrintedKeyword::CumulativeUpkeep => lower.starts_with("cumulative upkeep "),
            _ => false,
        }
    });
    let AbilityCompilation::Executable(ability) = matching.next()? else {
        return None;
    };
    if matching.next().is_some() {
        return None;
    }
    match keyword {
        PrintedKeyword::Equip if exact_runtime_equip_ability(ability) => {
            Some(sha256_hex(ability.normalized_oracle.as_bytes()))
        }
        PrintedKeyword::CumulativeUpkeep if exact_runtime_cumulative_upkeep_ability(ability) => {
            Some(sha256_hex(ability.normalized_oracle.as_bytes()))
        }
        _ => None,
    }
}

fn exact_runtime_equip_ability(ability: &crate::ability_program::ExecutableAbility) -> bool {
    if !ability
        .normalized_oracle
        .trim()
        .to_ascii_lowercase()
        .starts_with("equip ")
        || ability.timing
            != (AbilityTiming::Activated {
                window: ActivationWindow::SorcerySpeedOnly,
            })
        || ability.preconditions != [AbilityPrecondition::SourceZone(ProgramZone::Battlefield)]
    {
        return false;
    }
    let [AbilityCost::Mana(_)] = ability.costs.as_slice() else {
        return false;
    };
    let [AbilityEffect::AttachSourceToTarget(attachment)] = ability.effects.as_slice() else {
        return false;
    };
    attachment.attachment_kind == AttachmentKind::Equipment
        && attachment.target
            == ProgramObjectFilter {
                card_type: Some(ProgramCardType::Creature),
                controller: Some(ControllerRelation::You),
                ..ProgramObjectFilter::default()
            }
}

fn exact_runtime_cumulative_upkeep_ability(
    ability: &crate::ability_program::ExecutableAbility,
) -> bool {
    let AbilityTiming::Triggered { event } = &ability.timing else {
        return false;
    };
    let expected_filter = ProgramObjectFilter {
        controller: Some(ControllerRelation::You),
        ..ProgramObjectFilter::default()
    };
    if !ability
        .normalized_oracle
        .trim()
        .to_ascii_lowercase()
        .starts_with("cumulative upkeep ")
        || event.kind != TriggerEventKind::BeginningOfUpkeep
        || event.actor != ControllerRelation::You
        || event.object_filter != expected_filter
        || !ability.costs.is_empty()
        || ability.preconditions
            != [
                AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                AbilityPrecondition::EventObjectMatches(expected_filter),
            ]
    {
        return false;
    }
    let [AbilityEffect::CumulativeUpkeep(upkeep)] = ability.effects.as_slice() else {
        return false;
    };
    upkeep.counter == CounterKind::Age
        && upkeep.counters_added == 1
        && upkeep.if_not_paid == [AbilityEffect::SacrificeSelf]
        && fixed_generic_program_payment(&upkeep.payment_per_counter).is_some()
}

fn fixed_generic_program_payment(cost: &ProgramManaCost) -> Option<u16> {
    let ProgramManaCost::PrintedSymbols { profile, .. } = cost else {
        return None;
    };
    (profile.white == 0
        && profile.blue == 0
        && profile.black == 0
        && profile.red == 0
        && profile.green == 0
        && profile.colorless == 0
        && profile.variable_x == 0)
        .then_some(profile.generic)
}

fn characteristic_keyword_matches_subject(
    characteristic: &TypedCharacteristic,
    subject: &CharacteristicSubject,
) -> bool {
    match (characteristic, subject) {
        (
            TypedCharacteristic::PrintedCombatKeyword { keyword, .. },
            CharacteristicSubject::PrintedCombatKeyword(subject),
        ) => printed_keyword_name(*keyword) == subject,
        (TypedCharacteristic::PrintedCombatKeyword { .. }, _)
        | (_, CharacteristicSubject::PrintedCombatKeyword(_)) => false,
        _ => true,
    }
}

fn characteristic_face_binding_matches(
    characteristic: &TypedCharacteristic,
    face_binding: CharacteristicFaceBinding,
) -> bool {
    match characteristic {
        TypedCharacteristic::FaceRelationship(binding) => *binding == face_binding,
        _ => true,
    }
}

fn characteristic_keyword_has_exact_runtime_contract(characteristic: &TypedCharacteristic) -> bool {
    let TypedCharacteristic::PrintedCombatKeyword {
        keyword,
        complete_profile_sha256,
        runtime_clause_sha256,
    } = characteristic
    else {
        return true;
    };
    if !is_sha256_hex(complete_profile_sha256) {
        return false;
    }
    if is_exact_printed_combat_keyword(*keyword) {
        runtime_clause_sha256.is_none()
    } else if matches!(
        keyword,
        PrintedKeyword::Equip | PrintedKeyword::CumulativeUpkeep | PrintedKeyword::Devoid
    ) {
        runtime_clause_sha256.as_deref().is_some_and(is_sha256_hex)
    } else {
        false
    }
}

fn printed_keyword_name(keyword: PrintedKeyword) -> &'static str {
    match keyword {
        PrintedKeyword::Deathtouch => "deathtouch",
        PrintedKeyword::DoubleStrike => "double strike",
        PrintedKeyword::FirstStrike => "first strike",
        PrintedKeyword::Flying => "flying",
        PrintedKeyword::Haste => "haste",
        PrintedKeyword::Hexproof => "hexproof",
        PrintedKeyword::Indestructible => "indestructible",
        PrintedKeyword::Lifelink => "lifelink",
        PrintedKeyword::Menace => "menace",
        PrintedKeyword::Reach => "reach",
        PrintedKeyword::Shroud => "shroud",
        PrintedKeyword::Trample => "trample",
        PrintedKeyword::Vigilance => "vigilance",
        PrintedKeyword::Defender => "defender",
        PrintedKeyword::Partner => "partner",
        PrintedKeyword::FriendsForever => "friends forever",
        PrintedKeyword::Bargain => "bargain",
        PrintedKeyword::Imprint => "imprint",
        PrintedKeyword::Metalcraft => "metalcraft",
        PrintedKeyword::Threshold => "threshold",
        PrintedKeyword::Storm => "storm",
        PrintedKeyword::Flashback => "flashback",
        PrintedKeyword::Flash => "flash",
        PrintedKeyword::Ward => "ward",
        PrintedKeyword::Protection => "protection",
        PrintedKeyword::Equip => "equip",
        PrintedKeyword::CumulativeUpkeep => "cumulative upkeep",
        PrintedKeyword::Affinity => "affinity",
        PrintedKeyword::Kicker => "kicker",
        PrintedKeyword::Prowess => "prowess",
        PrintedKeyword::Boast => "boast",
        PrintedKeyword::Escape => "escape",
        PrintedKeyword::Convoke => "convoke",
        PrintedKeyword::Delve => "delve",
        PrintedKeyword::Devoid => "devoid",
        PrintedKeyword::Changeling => "changeling",
    }
}

fn role_mask_for_type_profile(profile: CardTypeProfile) -> u32 {
    let mut roles = 0u32;
    if profile.is_land {
        roles |= role::LAND;
    }
    if profile.is_creature {
        roles |= role::CREATURE;
    }
    if profile.is_artifact {
        roles |= role::ARTIFACT;
    }
    if profile.is_enchantment {
        roles |= role::ENCHANTMENT;
    }
    if profile.is_instant || profile.is_sorcery {
        roles |= role::INSTANT_SORCERY;
    }
    roles
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Strict authority for an unconditional fixed-mana spell resolution.
///
/// This classifier is intentionally independent from `EffectDescriptor`.
/// Descriptor summaries may rank cards, but they cannot grant runtime
/// execution when the complete ability program does not prove the exact root.
pub(crate) fn classify_spell_resolution_mana(
    card: &CompiledCard,
) -> Option<TypedSpellResolutionMana> {
    let program = &card.ability_program;
    if program.version != EXECUTABLE_ABILITY_PROGRAM_VERSION
        || program.atomic_transaction.is_some()
        || program.necropotence_lifecycle.is_some()
        || program.self_transfer_tutor_permanent.is_some()
        || program.entry_linked_permanent.is_some()
        || program.graveyard_reclamation.is_some()
        || !program.face_programs.is_empty()
        || program.abilities.len() != 1
        || program.unsupported_abilities().next().is_some()
        || !(type_line_has_exact_type_envelope(&card.type_line, "instant", false)
            || type_line_has_exact_type_envelope(&card.type_line, "sorcery", false))
    {
        return None;
    }

    let AbilityCompilation::Executable(ability) = &program.abilities[0] else {
        return None;
    };
    let [AbilityEffect::AddMana(mana)] = ability.effects.as_slice() else {
        return None;
    };
    let ProgramManaKind::Fixed(output) = mana.kind else {
        return None;
    };
    let total = fixed_mana_total(output)?;
    if ability.clause_index != 0
        || ability.timing != AbilityTiming::SpellResolution
        || !ability.costs.is_empty()
        || ability.preconditions != [AbilityPrecondition::SourceZone(ProgramZone::Stack)]
        || total == 0
        || total > u16::from(u8::MAX)
        || mana.amount != total
    {
        return None;
    }

    Some(TypedSpellResolutionMana { output })
}

/// Cheap, name-independent revalidation for conditional battlefield mana
/// sources. Runtime and planner hot paths call this classifier directly.
pub(crate) fn classify_conditional_mana_source(
    card: &CompiledCard,
) -> Option<TypedConditionalManaSource> {
    let program = &card.ability_program;
    if program.version != EXECUTABLE_ABILITY_PROGRAM_VERSION
        || program.atomic_transaction.is_some()
        || program.necropotence_lifecycle.is_some()
        || program.self_transfer_tutor_permanent.is_some()
        || program.graveyard_reclamation.is_some()
        || !program.face_programs.is_empty()
    {
        return None;
    }

    if let Some(permanent) = program.executable_entry_linked_permanent() {
        if program.unsupported_entry_linked_permanent().is_some()
            || !program.abilities.is_empty()
            || !type_line_has_exact_type_envelope(&card.type_line, "artifact", false)
            || permanent.mana_ability.costs != [AbilityCost::TapSelf]
            || permanent.mana_ability.preconditions
                != [
                    AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                    AbilityPrecondition::SourceUntapped,
                ]
        {
            return None;
        }
        return match (&permanent.entry, &permanent.mana_ability.output) {
            (
                PermanentEntryProcedure::OptionalImprint {
                    filter: EntryLinkedCardFilter::NonartifactNonlandCard,
                    from: ProgramZone::Hand,
                    to: ProgramZone::Exile,
                    link: LinkedEntryObject::CardExiledByThisEntry,
                },
                EntryLinkedManaOutput::AnyColorOfLinkedCard {
                    linked: LinkedEntryObject::CardExiledByThisEntry,
                },
            ) => Some(TypedConditionalManaSource::ImprintLinkedCardColors),
            (
                PermanentEntryProcedure::DiscardOrFailToEnter {
                    filter: EntryLinkedCardFilter::LandCard,
                    discard_from: ProgramZone::Hand,
                    discard_to: ProgramZone::Graveyard,
                    success_destination: ProgramZone::Battlefield,
                    failure_destination: ProgramZone::Graveyard,
                },
                EntryLinkedManaOutput::AnyOneColor,
            ) => Some(TypedConditionalManaSource::DiscardLandOrFailEntry),
            _ => None,
        };
    }
    if program.entry_linked_permanent.is_some() || program.unsupported_abilities().next().is_some()
    {
        return None;
    }

    let base_preconditions = [
        AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
        AbilityPrecondition::SourceUntapped,
    ];
    if program.abilities.len() == 2
        && type_line_has_exact_type_envelope(&card.type_line, "land", false)
    {
        let mut has_colorless_mode = false;
        let mut damaging_any_color_mode = None;
        for compilation in &program.abilities {
            let AbilityCompilation::Executable(ability) = compilation else {
                return None;
            };
            if ability.timing
                != (AbilityTiming::Activated {
                    window: ActivationWindow::NormalPriority,
                })
                || ability.costs != [AbilityCost::TapSelf]
                || ability.preconditions != base_preconditions
            {
                return None;
            }
            match ability.effects.as_slice() {
                [AbilityEffect::AddMana(mana)]
                    if mana.amount == 1
                        && mana.kind
                            == ProgramManaKind::Fixed(FixedManaProfile {
                                colorless: 1,
                                ..FixedManaProfile::default()
                            })
                        && !has_colorless_mode =>
                {
                    has_colorless_mode = true;
                }
                [AbilityEffect::AddManaAndSourceDamage(linked)]
                    if linked.mana.amount == 1
                        && linked.mana.kind == ProgramManaKind::AnyOneColor
                        && linked.damage.amount == 3
                        && linked.damage.recipient == ControllerRelation::You
                        && damaging_any_color_mode.is_none() =>
                {
                    damaging_any_color_mode = Some(linked.damage.amount);
                }
                _ => return None,
            }
        }
        if has_colorless_mode {
            return damaging_any_color_mode.map(|damage| {
                TypedConditionalManaSource::ColorlessOrAnyColorWithSourceDamage { damage }
            });
        }
        return None;
    }
    if program.abilities.len() != 1 {
        return None;
    }
    let ability = program.executable_abilities().next()?;
    if ability.clause_index != 0
        || ability.timing
            != (AbilityTiming::Activated {
                window: ActivationWindow::NormalPriority,
            })
        || ability.costs != [AbilityCost::TapSelf]
    {
        return None;
    }
    match ability.effects.as_slice() {
        [AbilityEffect::AddMana(mana)]
            if ability.preconditions == base_preconditions
                && mana.amount == 1
                && mana.kind
                    == ProgramManaKind::AnyColorAmongLegendaryCreaturesAndPlaneswalkersYouControl
                && type_line_has_exact_type_envelope(&card.type_line, "artifact", true) =>
        {
            Some(TypedConditionalManaSource::ControlledLegendaryColors)
        }
        [AbilityEffect::AddMana(mana)]
            if ability.preconditions
                == [
                    AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                    AbilityPrecondition::SourceUntapped,
                    AbilityPrecondition::ResourceAtLeast {
                        resource: ResourceKind::Artifact,
                        count: 3,
                    },
                ]
                && mana.amount == 1
                && mana.kind == ProgramManaKind::AnyOneColor
                && type_line_has_exact_type_envelope(&card.type_line, "artifact", true) =>
        {
            Some(TypedConditionalManaSource::MetalcraftAnyColor)
        }
        [AbilityEffect::AddManaAndSourceDamage(linked)]
            if ability.preconditions == base_preconditions
                && linked.mana.amount == 2
                && linked.mana.kind
                    == ProgramManaKind::Fixed(FixedManaProfile {
                        colorless: 2,
                        ..FixedManaProfile::default()
                    })
                && linked.damage.amount == 2
                && linked.damage.recipient == ControllerRelation::You
                && type_line_has_exact_type_envelope(&card.type_line, "land", false) =>
        {
            Some(TypedConditionalManaSource::FixedWithSourceDamage {
                output: FixedManaProfile {
                    colorless: 2,
                    ..FixedManaProfile::default()
                },
                damage: 2,
            })
        }
        _ => None,
    }
}

/// Exact Lotus Petal shaped mana authority. Restricted outputs, extra costs,
/// additional effects, and nonartifact roots remain report only.
pub(crate) fn classify_sacrifice_self_any_color_mana(
    card: &CompiledCard,
) -> Option<TypedSacrificeSelfMana> {
    let program = &card.ability_program;
    if program.version != EXECUTABLE_ABILITY_PROGRAM_VERSION
        || program.atomic_transaction.is_some()
        || program.necropotence_lifecycle.is_some()
        || program.self_transfer_tutor_permanent.is_some()
        || program.entry_linked_permanent.is_some()
        || program.graveyard_reclamation.is_some()
        || !program.face_programs.is_empty()
        || program.abilities.len() != 1
        || program.unsupported_abilities().next().is_some()
        || !type_line_has_exact_type_envelope(&card.type_line, "artifact", false)
    {
        return None;
    }
    let AbilityCompilation::Executable(ability) = &program.abilities[0] else {
        return None;
    };
    let [AbilityEffect::AddMana(mana)] = ability.effects.as_slice() else {
        return None;
    };
    if ability.clause_index != 0
        || ability.timing
            != (AbilityTiming::Activated {
                window: ActivationWindow::NormalPriority,
            })
        || ability.costs != [AbilityCost::TapSelf, AbilityCost::SacrificeSelf]
        || ability.preconditions
            != [
                AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                AbilityPrecondition::SourceUntapped,
            ]
        || mana.amount != 1
        || mana.kind != ProgramManaKind::AnyOneColor
    {
        return None;
    }
    Some(TypedSacrificeSelfMana {
        amount: 1,
        requires_tap: true,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypedAtomicTransaction {
    HandMana {
        output: FixedManaProfile,
    },
    SacrificeRitual {
        output: FixedManaProfile,
    },
    NameLinkedGraveyardRitual {
        base: FixedManaProfile,
        per_match: FixedManaProfile,
        /// Opponent graveyard objects are not present in the bounded local
        /// state. Zero is therefore an explicit conservative floor, not an
        /// assertion that opponents have no matching cards.
        opponent_matching_card_floor: u16,
    },
    SacrificeTutor {
        tutor: ProgramTutorEffect,
    },
    ThresholdRitual {
        default: FixedManaProfile,
        replacement: FixedManaProfile,
        threshold: u16,
    },
    SearchRandomDiscardShuffle {
        tutor: ProgramTutorEffect,
    },
    TemporaryLandSacrificeManaGrant {
        output: FixedManaProfile,
    },
    BargainSearchCastOrHand {
        maximum_mana_value: u16,
    },
    OpponentChoiceSearchSplit,
}

impl TypedAtomicTransaction {
    pub(crate) fn initiation(&self) -> AtomicInitiation {
        match self {
            Self::HandMana { .. } => AtomicInitiation::HandManaAbility,
            Self::SacrificeRitual { .. }
            | Self::NameLinkedGraveyardRitual { .. }
            | Self::SacrificeTutor { .. }
            | Self::ThresholdRitual { .. }
            | Self::SearchRandomDiscardShuffle { .. }
            | Self::TemporaryLandSacrificeManaGrant { .. }
            | Self::BargainSearchCastOrHand { .. }
            | Self::OpponentChoiceSearchSplit => AtomicInitiation::CastSpell,
        }
    }

    pub(crate) fn is_mana_development(&self) -> bool {
        matches!(
            self,
            Self::HandMana { .. }
                | Self::SacrificeRitual { .. }
                | Self::NameLinkedGraveyardRitual { .. }
                | Self::ThresholdRitual { .. }
                | Self::TemporaryLandSacrificeManaGrant { .. }
        )
    }

    pub(crate) fn is_tutor(&self) -> bool {
        matches!(
            self,
            Self::SacrificeTutor { .. }
                | Self::SearchRandomDiscardShuffle { .. }
                | Self::BargainSearchCastOrHand { .. }
                | Self::OpponentChoiceSearchSplit
        )
    }

    fn executor_id(&self) -> &'static str {
        match self {
            Self::HandMana { .. } => "abstract-play.atomic.hand-mana",
            Self::SacrificeRitual { .. } => "abstract-play.atomic.sacrifice-ritual",
            Self::NameLinkedGraveyardRitual { .. } => {
                "abstract-play.atomic.name-linked-graveyard-ritual"
            }
            Self::SacrificeTutor { .. } => "abstract-play.atomic.sacrifice-tutor",
            Self::ThresholdRitual { .. } => "abstract-play.atomic.threshold-ritual",
            Self::SearchRandomDiscardShuffle { .. } => {
                "abstract-play.atomic.search-random-discard-shuffle"
            }
            Self::TemporaryLandSacrificeManaGrant { .. } => {
                "abstract-play.atomic.temporary-land-sacrifice-mana-grant"
            }
            Self::BargainSearchCastOrHand { .. } => {
                "abstract-play.atomic.bargain-search-cast-or-hand"
            }
            Self::OpponentChoiceSearchSplit => "abstract-play.atomic.opponent-choice-search-split",
        }
    }
}

/// Runtime revalidation is deliberately as narrow as the Oracle compiler.
/// This prevents a hand-authored or future extended IR shape from inheriting
/// execution merely because one nearby cost or effect is familiar.
pub(crate) fn classify_atomic_runtime_transaction(
    card: &CompiledCard,
) -> Option<TypedAtomicTransaction> {
    let program = &card.ability_program;
    if program.version != EXECUTABLE_ABILITY_PROGRAM_VERSION {
        return None;
    }
    let source_transaction = program.executable_atomic_transaction()?;
    if program.unsupported_atomic_transaction().is_some()
        || !program.abilities.is_empty()
        || program.necropotence_lifecycle.is_some()
        || program.self_transfer_tutor_permanent.is_some()
        || program.entry_linked_permanent.is_some()
        || program.graveyard_reclamation.is_some()
        || !program.face_programs.is_empty()
        || program.unsupported_abilities().next().is_some()
    {
        return None;
    }

    let transaction = match (
        source_transaction.initiation,
        source_transaction.source_zone,
        source_transaction.initiation_costs.as_slice(),
        source_transaction.resolution.as_slice(),
    ) {
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost],
            [AtomicEffect::TemporaryLandSacrificeManaGrant(effect)],
        ) if effect.player == ControllerRelation::You
            && effect.affected_zone == ProgramZone::Battlefield
            && effect.affected_filter
                == (ProgramObjectFilter {
                    card_type: Some(ProgramCardType::Land),
                    controller: Some(ControllerRelation::You),
                    ..ProgramObjectFilter::default()
                })
            && effect.applies_to_future_matching_objects
            && effect.duration == AtomicEffectDuration::UntilEndOfTurn
            && effect.granted_ability.kind == GrantedAbilityKind::ManaAbility
            && effect.granted_ability.source_zone == ProgramZone::Battlefield
            && effect.granted_ability.controller == ControllerRelation::You
            && effect.granted_ability.cost == GrantedSelfCost::SacrificeSelf
            && effect.granted_ability.output
                == (FixedManaProfile {
                    black: 1,
                    ..FixedManaProfile::default()
                })
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            TypedAtomicTransaction::TemporaryLandSacrificeManaGrant {
                output: effect.granted_ability.output,
            }
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost, AtomicCost::Bargain(bargain)],
            [AtomicEffect::BargainSearchCastOrHand(effect)],
        ) if exact_bargain_cost(bargain)
            && exact_bargain_search_cast_or_hand(effect)
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            TypedAtomicTransaction::BargainSearchCastOrHand {
                maximum_mana_value: effect.conditional_cast.mana_value.maximum,
            }
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost],
            [AtomicEffect::OpponentChoiceSearchSplit(effect)],
        ) if exact_opponent_choice_search_split(effect)
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            TypedAtomicTransaction::OpponentChoiceSearchSplit
        }
        (
            AtomicInitiation::HandManaAbility,
            ProgramZone::Hand,
            [
                AtomicCost::ExileSelf {
                    from: ProgramZone::Hand,
                },
            ],
            [AtomicEffect::AddFixedMana(output)],
        ) if fixed_mana_is_exact_hand_output(*output) => {
            TypedAtomicTransaction::HandMana { output: *output }
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [
                AtomicCost::PrintedManaCost,
                AtomicCost::SacrificePermanents {
                    filter,
                    count: 1,
                    commander_eligibility: CommanderEligibility::Exclude,
                },
            ],
            [AtomicEffect::AddFixedMana(output)],
        ) if exact_noncommander_creature_filter(filter)
            && *output
                == (FixedManaProfile {
                    black: 4,
                    ..FixedManaProfile::default()
                })
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            TypedAtomicTransaction::SacrificeRitual { output: *output }
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [
                AtomicCost::PrintedManaCost,
                AtomicCost::SacrificePermanents {
                    filter,
                    count: 1,
                    commander_eligibility: CommanderEligibility::Exclude,
                },
            ],
            [AtomicEffect::SearchToHand(tutor)],
        ) if exact_noncommander_creature_filter(filter)
            && exact_any_card_tutor(tutor, true)
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            TypedAtomicTransaction::SacrificeTutor {
                tutor: tutor.clone(),
            }
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost],
            [
                AtomicEffect::AddFixedMana(base),
                AtomicEffect::AddManaPerNamedCardInGraveyards(dynamic),
            ],
        ) if *base
            == (FixedManaProfile {
                red: 2,
                ..FixedManaProfile::default()
            })
            && dynamic.mana_per_card
                == (FixedManaProfile {
                    red: 1,
                    ..FixedManaProfile::default()
                })
            && dynamic.card_name == AtomicCardNameReference::SourceCardName
            && dynamic.graveyards == AtomicGraveyardScope::EachPlayerGraveyard
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            TypedAtomicTransaction::NameLinkedGraveyardRitual {
                base: *base,
                per_match: dynamic.mana_per_card,
                opponent_matching_card_floor: 0,
            }
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost],
            [AtomicEffect::ConditionalManaReplacement(effect)],
        ) if effect.default
            == (FixedManaProfile {
                black: 3,
                ..FixedManaProfile::default()
            })
            && effect.replacement
                == (FixedManaProfile {
                    black: 5,
                    ..FixedManaProfile::default()
                })
            && effect.condition
                == (AtomicStateCondition::CardsInZoneAtLeast {
                    player: ControllerRelation::You,
                    zone: ProgramZone::Graveyard,
                    count: 7,
                })
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            TypedAtomicTransaction::ThresholdRitual {
                default: effect.default,
                replacement: effect.replacement,
                threshold: 7,
            }
        }
        (
            AtomicInitiation::CastSpell,
            ProgramZone::Hand,
            [AtomicCost::PrintedManaCost],
            [
                AtomicEffect::SearchToHand(tutor),
                AtomicEffect::RandomDiscard(discard),
                AtomicEffect::ShuffleLibrary(shuffle),
            ],
        ) if exact_any_card_tutor(tutor, false)
            && discard.player == ControllerRelation::You
            && discard.count == 1
            && discard.from == ProgramZone::Hand
            && discard.to == ProgramZone::Graveyard
            && discard.selection == RandomSelection::UniformAmongObjectsInZone
            && shuffle.player == ControllerRelation::You
            && matches!(
                modeled_line_card_kind(card),
                Some(ModeledLineCardKind::Spell)
            ) =>
        {
            TypedAtomicTransaction::SearchRandomDiscardShuffle {
                tutor: tutor.clone(),
            }
        }
        _ => return None,
    };

    Some(transaction)
}

pub(crate) fn compile_live_ability_runtime_receipts(
    card: &CompiledCard,
) -> Vec<LiveAbilityRuntimeReceipt> {
    let program = &card.ability_program;
    if program.version != EXECUTABLE_ABILITY_PROGRAM_VERSION
        || program.necropotence_lifecycle.is_some()
        || program.self_transfer_tutor_permanent.is_some()
        || program.entry_linked_permanent.is_some()
        || program.atomic_transaction.is_some()
        || program.graveyard_reclamation.is_some()
    {
        return Vec::new();
    }
    if !program.face_programs.is_empty() {
        if program.face_programs.iter().any(|face| {
            face.necropotence_lifecycle.is_some()
                || face.self_transfer_tutor_permanent.is_some()
                || face.entry_linked_permanent.is_some()
                || face.atomic_transaction.is_some()
                || face.graveyard_reclamation.is_some()
        }) {
            return Vec::new();
        }
        let mut receipts = program
            .face_programs
            .iter()
            .flat_map(|face| {
                face.abilities
                    .iter()
                    .filter_map(|compilation| match compilation {
                        AbilityCompilation::Executable(ability) => Some(ability),
                        AbilityCompilation::Unsupported(_) => None,
                    })
                    .filter_map(|ability| {
                        let shape = classify_live_ability_shape(ability)?;
                        if !live_static_attachment_source_is_legal(ability, &face.type_line) {
                            return None;
                        }
                        build_face_live_ability_receipt(
                            card,
                            u16::try_from(face.face_index).ok()?,
                            shape,
                            vec![ability.clone()],
                        )
                    })
            })
            .collect::<Vec<_>>();
        receipts.sort_by(|left, right| {
            left.binding
                .executor_id
                .cmp(right.binding.executor_id)
                .then_with(|| {
                    left.source_evidence
                        .covered_oracle_clauses
                        .cmp(&right.source_evidence.covered_oracle_clauses)
                })
        });
        return receipts;
    }

    let executable = program.executable_abilities().collect::<Vec<_>>();
    let aura_targeting = executable
        .iter()
        .copied()
        .filter(|ability| is_exact_aura_spell_targeting(ability))
        .collect::<Vec<_>>();
    let aura_payload = executable
        .iter()
        .copied()
        .filter(|ability| is_live_beneficial_aura_payload(ability))
        .collect::<Vec<_>>();
    let quest_counter = executable
        .iter()
        .copied()
        .filter(|ability| is_exact_quest_counter_trigger(ability))
        .collect::<Vec<_>>();
    let quest_activation = executable
        .iter()
        .copied()
        .filter(|ability| is_exact_quest_token_activation(ability))
        .collect::<Vec<_>>();
    let mut claimed_clause_indices = Vec::new();
    let mut receipts = Vec::new();
    if type_line_is_enchantment_aura(&card.type_line)
        && let ([targeting], [payload]) = (aura_targeting.as_slice(), aura_payload.as_slice())
    {
        let mut abilities = vec![(*targeting).clone(), (*payload).clone()];
        abilities.sort_by_key(|ability| ability.clause_index);
        if let Some(receipt) =
            build_live_ability_receipt(card, LiveAbilityShape::AuraSpellTargeting, abilities)
        {
            claimed_clause_indices
                .extend(receipt.abilities.iter().map(|ability| ability.clause_index));
            receipts.push(receipt);
        }
    }
    if let ([counter], [activation]) = (quest_counter.as_slice(), quest_activation.as_slice()) {
        let mut abilities = vec![(*counter).clone(), (*activation).clone()];
        abilities.sort_by_key(|ability| ability.clause_index);
        if let Some(receipt) =
            build_live_ability_receipt(card, LiveAbilityShape::QuestCounterLifecycle, abilities)
        {
            claimed_clause_indices
                .extend(receipt.abilities.iter().map(|ability| ability.clause_index));
            receipts.push(receipt);
        }
    }

    for ability in executable {
        if claimed_clause_indices.contains(&ability.clause_index) {
            continue;
        }
        let Some(shape) = classify_live_ability_shape(ability) else {
            continue;
        };
        if !live_static_attachment_source_is_legal(ability, &card.type_line) {
            continue;
        }
        if let Some(receipt) = build_live_ability_receipt(card, shape, vec![ability.clone()]) {
            receipts.push(receipt);
        }
    }
    receipts.sort_by(|left, right| {
        left.binding
            .executor_id
            .cmp(right.binding.executor_id)
            .then_with(|| {
                left.source_evidence
                    .covered_oracle_clauses
                    .cmp(&right.source_evidence.covered_oracle_clauses)
            })
    });
    receipts
}

fn build_live_ability_receipt(
    card: &CompiledCard,
    shape: LiveAbilityShape,
    abilities: Vec<ExecutableAbility>,
) -> Option<LiveAbilityRuntimeReceipt> {
    if !live_receipt_shape_matches(shape, &abilities) {
        return None;
    }
    let executor_id = shape.executor_id();
    let selected_clause_indices = abilities
        .iter()
        .map(|ability| ability.clause_index)
        .collect::<Vec<_>>();
    let source_evidence =
        live_ability_source_evidence(card, &selected_clause_indices, executor_id)?;
    Some(LiveAbilityRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id,
            executor_version: LIVE_ABILITY_EXECUTOR_VERSION,
        },
        capabilities: live_ability_capabilities(shape),
        source_evidence,
        shape,
        abilities,
    })
}

fn build_face_live_ability_receipt(
    card: &CompiledCard,
    face_index: u16,
    shape: LiveAbilityShape,
    abilities: Vec<ExecutableAbility>,
) -> Option<LiveAbilityRuntimeReceipt> {
    if !live_receipt_shape_matches(shape, &abilities) {
        return None;
    }
    let executor_id = shape.executor_id();
    let selected_clause_indices = abilities
        .iter()
        .map(|ability| u16::try_from(ability.clause_index).ok())
        .collect::<Option<Vec<_>>>()?;
    let source_evidence = selected_program_face_clause_source_evidence(
        card,
        face_index,
        &selected_clause_indices,
        executor_id,
        LIVE_ABILITY_EXECUTOR_VERSION,
    )?;
    Some(LiveAbilityRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id,
            executor_version: LIVE_ABILITY_EXECUTOR_VERSION,
        },
        capabilities: live_ability_capabilities(shape),
        source_evidence,
        shape,
        abilities,
    })
}

fn live_receipt_shape_matches(shape: LiveAbilityShape, abilities: &[ExecutableAbility]) -> bool {
    match shape {
        LiveAbilityShape::AuraSpellTargeting => {
            matches!(abilities, [targeting, payload]
                if targeting.clause_index < payload.clause_index
                    && is_exact_aura_spell_targeting(targeting)
                    && is_live_beneficial_aura_payload(payload))
        }
        LiveAbilityShape::QuestCounterLifecycle => {
            matches!(abilities, [counter, activation]
                if counter.clause_index < activation.clause_index
                    && is_exact_quest_counter_trigger(counter)
                    && is_exact_quest_token_activation(activation))
        }
        _ => matches!(abilities, [ability] if classify_live_ability_shape(ability) == Some(shape)),
    }
}

fn classify_live_ability_shape(ability: &ExecutableAbility) -> Option<LiveAbilityShape> {
    match (&ability.timing, ability.effects.as_slice()) {
        (AbilityTiming::StaticModifier, [AbilityEffect::GrantCastTimingPermission(permission)])
            if ability.costs.is_empty()
                && ability.preconditions
                    == [AbilityPrecondition::SourceZone(ProgramZone::Hand)]
                && permission.from == ProgramZone::Hand
                && permission.window == ActivationWindow::InstantSpeedOnly =>
        {
            Some(LiveAbilityShape::FlashPermission)
        }
        (AbilityTiming::Triggered { event }, [AbilityEffect::Ward(ward)])
            if exact_generic_ward_ability(ability, event, ward) =>
        {
            Some(LiveAbilityShape::Ward)
        }
        (AbilityTiming::SpellResolution, effects) if is_exact_scry_resolution(ability, effects) => {
            Some(LiveAbilityShape::ScryResolution)
        }
        (AbilityTiming::SpellResolution, [AbilityEffect::Mill(mill)])
            if ability.costs.is_empty()
                && ability.preconditions
                    == [AbilityPrecondition::SourceZone(ProgramZone::Stack)]
                && mill.player == PlayerSelector::TargetPlayer
                && mill.count > 0 =>
        {
            Some(LiveAbilityShape::MillResolution)
        }
        (AbilityTiming::Triggered { event }, [AbilityEffect::CopyThisSpell(copy)])
            if ability.costs.is_empty()
                && event.kind == TriggerEventKind::ThisSpellCast
                && event.actor == ControllerRelation::You
                && event.object_filter.card_type == Some(ProgramCardType::Spell)
                && event.object_filter.controller == Some(ControllerRelation::You)
                && ability.preconditions
                    == [
                        AbilityPrecondition::SourceZone(ProgramZone::Stack),
                        AbilityPrecondition::EventObjectMatches(event.object_filter.clone()),
                    ]
                && copy.count == SpellCopyCount::EachSpellCastBeforeThisSpellThisTurn
                && copy.target_choice == CopyTargetChoice::MayChooseNewTargets =>
        {
            Some(LiveAbilityShape::StormCopyTrigger)
        }
        (AbilityTiming::StaticModifier, [AbilityEffect::ApplyStaticCreatureModifier(_)])
            if ability.costs.is_empty() && has_battlefield_source_only(ability) =>
        {
            Some(LiveAbilityShape::StaticCreatureModifier)
        }
        (AbilityTiming::StaticModifier, [AbilityEffect::ChooseCreatureType(_)])
            if ability.costs.is_empty() && has_battlefield_source_only(ability) =>
        {
            Some(LiveAbilityShape::CreatureTypeChoice)
        }
        (AbilityTiming::StaticModifier, [AbilityEffect::SourceHasChosenCreatureType(_)])
            if ability.costs.is_empty() && has_battlefield_source_only(ability) =>
        {
            Some(LiveAbilityShape::ChosenCreatureTypeMarker)
        }
        (AbilityTiming::StaticModifier, [AbilityEffect::MultiplyTriggeredAbility(multiplier)])
            if ability.costs.is_empty()
                && has_battlefield_source_only(ability)
                && multiplier.additional_times > 0 =>
        {
            Some(LiveAbilityShape::TriggerMultiplier)
        }
        (AbilityTiming::StaticModifier, [AbilityEffect::GrantAllCreatureTypes(grant)])
            if ability.costs.is_empty()
                && has_battlefield_source_only(ability)
                && grant.creatures_you_control
                && grant.creature_spells_you_control
                && grant.nonbattlefield_creature_cards_you_own =>
        {
            Some(LiveAbilityShape::AllCreatureTypes)
        }
        (AbilityTiming::StaticModifier, [AbilityEffect::ReduceSpellCost(reduction)])
            if ability.costs.is_empty()
                && reduction.generic_mana_reduction > 0
                && spell_cost_reduction_preconditions_match(ability, reduction) =>
        {
            Some(LiveAbilityShape::SpellCostReduction)
        }
        (AbilityTiming::StaticModifier, [AbilityEffect::AlternativeSpellCost(alternative)])
            if ability.costs.is_empty()
                && ability.preconditions.is_empty()
                && alternative.replaces == ReplacedSpellCost::PrintedManaCost
                && matches!(
                    alternative.payment.as_slice(),
                    [
                        AlternativeSpellCostComponent::Mana(_),
                        AlternativeSpellCostComponent::TapUntappedPermanents {
                            count,
                            filter
                        }
                    ] if *count > 0
                        && filter.controller == ControllerRelation::You
                        && filter.card_type == ProgramCardType::Creature
                ) =>
        {
            Some(LiveAbilityShape::AlternativeSpellCost)
        }
        (
            AbilityTiming::Activated {
                window: ActivationWindow::SorcerySpeedOnly,
            },
            [AbilityEffect::AttachSourceToTarget(attachment)],
        ) if matches!(ability.costs.as_slice(), [AbilityCost::Mana(_)])
            && has_battlefield_source_only(ability)
            && attachment.attachment_kind == AttachmentKind::Equipment
            && attachment.target.card_type == Some(ProgramCardType::Creature)
            && attachment.target.controller == Some(ControllerRelation::You) =>
        {
            Some(LiveAbilityShape::EquipmentAttach)
        }
        (
            AbilityTiming::Activated {
                window: ActivationWindow::NormalPriority,
            },
            [AbilityEffect::AddMana(mana)],
        ) if ability.costs == [AbilityCost::TapSelf]
            && ability.preconditions
                == [
                    AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                    AbilityPrecondition::SourceUntapped,
                ]
            && mana.amount > 0
            && matches!(
                mana.kind,
                ProgramManaKind::Fixed(_) | ProgramManaKind::AnyOneColor
            ) =>
        {
            Some(LiveAbilityShape::FixedManaActivation)
        }
        (AbilityTiming::SpellResolution, [AbilityEffect::Draw(draw)])
            if ability.costs.is_empty()
                && ability.preconditions
                    == [AbilityPrecondition::SourceZone(ProgramZone::Stack)]
                && draw.count > 0
                && !draw.optional
                && draw.unless_event_player_pays.is_none() =>
        {
            Some(LiveAbilityShape::FixedDrawResolution)
        }
        (AbilityTiming::Triggered { event }, [AbilityEffect::CumulativeUpkeep(upkeep)])
            if ability.costs.is_empty()
                && exact_event_preconditions(ability, event)
                && event.kind == TriggerEventKind::BeginningOfUpkeep
                && event.actor == ControllerRelation::You
                && upkeep.counter == CounterKind::Age
                && upkeep.counters_added == 1
                && upkeep.if_not_paid == [AbilityEffect::SacrificeSelf] =>
        {
            Some(LiveAbilityShape::CumulativeUpkeep)
        }
        (AbilityTiming::Triggered { event }, [AbilityEffect::BecomeMonarch(monarch)])
            if ability.costs.is_empty()
                && event.kind == TriggerEventKind::PermanentEntersBattlefield
                && event.actor == ControllerRelation::You
                && monarch.player == ControllerRelation::You
                && ability.preconditions
                    == [
                        AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                        AbilityPrecondition::EventObjectIsSource,
                        AbilityPrecondition::EventObjectMatches(event.object_filter.clone()),
                    ] =>
        {
            Some(LiveAbilityShape::BecomeMonarchTrigger)
        }
        (
            AbilityTiming::Triggered { event },
            [AbilityEffect::ModifyPowerToughnessUntilEndOfTurn(modifier)],
        ) if ability.costs.is_empty()
            && exact_event_preconditions(ability, event)
            && event.kind == TriggerEventKind::OtherFlyingCreatureEntersBattlefield
            && event.actor == ControllerRelation::You
            && (modifier.power_delta != 0 || modifier.toughness_delta != 0) =>
        {
            Some(LiveAbilityShape::TemporaryPowerToughnessTrigger)
        }
        (AbilityTiming::Triggered { event }, effects)
            if ability.costs.is_empty()
                && exact_event_preconditions(ability, event)
                && matches!(
                    event.kind,
                    TriggerEventKind::SpellCast | TriggerEventKind::PermanentEntersBattlefield
                )
                && matches!(
                    event.actor,
                    ControllerRelation::You | ControllerRelation::Any
                )
                && effects.iter().all(is_nonzero_token_effect)
                && !effects.is_empty() =>
        {
            Some(LiveAbilityShape::ControllerTokenTrigger)
        }
        (AbilityTiming::Triggered { event }, effects)
            if ability.costs.is_empty()
                && exact_event_preconditions(ability, event)
                && event.kind == TriggerEventKind::BeginningOfUpkeep
                && event.actor == ControllerRelation::You
                && effects.iter().all(is_live_upkeep_effect)
                && !effects.is_empty() =>
        {
            Some(LiveAbilityShape::UpkeepTokenLife)
        }
        (AbilityTiming::Triggered { event }, effects)
            if ability.costs.is_empty()
                && exact_event_preconditions(ability, event)
                && is_live_draw_trigger_event(event.kind)
                && effects.iter().all(is_live_draw_trigger_effect)
                && !effects.is_empty() =>
        {
            Some(LiveAbilityShape::DrawTrigger)
        }
        (AbilityTiming::Triggered { event }, effects)
            if ability.costs.is_empty()
                && exact_event_preconditions(ability, event)
                && event.kind == TriggerEventKind::SecondSpellCastEachTurn
                && matches!(
                    event.actor,
                    ControllerRelation::You
                        | ControllerRelation::Opponent
                        | ControllerRelation::Any
                )
                && !effects.is_empty()
                && effects.iter().all(is_exact_table_resource_effect) =>
        {
            Some(LiveAbilityShape::TableResourceTrigger)
        }
        _ => None,
    }
}

fn is_exact_table_resource_effect(effect: &AbilityEffect) -> bool {
    match effect {
        AbilityEffect::LoseLife(loss) => loss.player == ControllerRelation::You && loss.amount > 0,
        AbilityEffect::CreateToken(token) => token.count > 0 && token.kind == TokenKind::Treasure,
        _ => false,
    }
}

fn type_line_is_enchantment_aura(type_line: &str) -> bool {
    let profile = compile_card_types(type_line);
    profile.is_enchantment
        && type_line
            .split(|character: char| !character.is_alphabetic())
            .any(|word| word.eq_ignore_ascii_case("aura"))
}

fn type_line_is_artifact_equipment(type_line: &str) -> bool {
    let profile = compile_card_types(type_line);
    profile.is_artifact
        && type_line
            .split(|character: char| !character.is_alphabetic())
            .any(|word| word.eq_ignore_ascii_case("equipment"))
}

fn live_static_attachment_source_is_legal(ability: &ExecutableAbility, type_line: &str) -> bool {
    let [AbilityEffect::ApplyStaticCreatureModifier(modifier)] = ability.effects.as_slice() else {
        return true;
    };
    match modifier.target {
        StaticCreatureModifierTarget::CreatureEnchantedBySource => {
            type_line_is_enchantment_aura(type_line) && modifier.granted_keywords.is_empty()
        }
        StaticCreatureModifierTarget::CreatureEquippedBySource => {
            type_line_is_artifact_equipment(type_line) && modifier.granted_keywords.is_empty()
        }
        _ => true,
    }
}

fn is_exact_aura_spell_targeting(ability: &ExecutableAbility) -> bool {
    let [AbilityEffect::AttachSourceToTarget(attachment)] = ability.effects.as_slice() else {
        return false;
    };
    let exact_target = attachment.target
        == (ProgramObjectFilter {
            card_type: Some(ProgramCardType::Creature),
            ..ProgramObjectFilter::default()
        })
        || attachment.target
            == (ProgramObjectFilter {
                card_type: Some(ProgramCardType::Creature),
                controller: Some(ControllerRelation::You),
                ..ProgramObjectFilter::default()
            });
    ability.timing == AbilityTiming::AuraSpellTargeting
        && ability.costs.is_empty()
        && ability.preconditions == [AbilityPrecondition::SourceZone(ProgramZone::Stack)]
        && attachment.attachment_kind == AttachmentKind::Aura
        && exact_target
}

fn live_static_value_is_nonnegative(value: &StaticModifierValue) -> bool {
    match value {
        StaticModifierValue::Fixed(value) => *value >= 0,
        StaticModifierValue::PermanentsYouControl { multiplier, .. } => *multiplier >= 0,
    }
}

fn live_static_value_can_help(value: &StaticModifierValue) -> bool {
    match value {
        StaticModifierValue::Fixed(value) => *value > 0,
        StaticModifierValue::PermanentsYouControl { multiplier, .. } => *multiplier > 0,
    }
}

fn is_live_beneficial_aura_payload(ability: &ExecutableAbility) -> bool {
    match (&ability.timing, ability.effects.as_slice()) {
        (AbilityTiming::StaticModifier, [AbilityEffect::ApplyStaticCreatureModifier(modifier)]) => {
            ability.costs.is_empty()
                && has_battlefield_source_only(ability)
                && modifier.target == StaticCreatureModifierTarget::CreatureEnchantedBySource
                && modifier.granted_keywords.is_empty()
                && live_static_value_is_nonnegative(&modifier.power_delta)
                && live_static_value_is_nonnegative(&modifier.toughness_delta)
                && (live_static_value_can_help(&modifier.power_delta)
                    || live_static_value_can_help(&modifier.toughness_delta)
                    || !modifier.granted_keywords.is_empty())
        }
        (AbilityTiming::Triggered { event }, effects) => {
            ability.costs.is_empty()
                && exact_event_preconditions(ability, event)
                && event.kind == TriggerEventKind::EnchantedCreatureDealsDamageToOpponent
                && event.actor == ControllerRelation::You
                && !effects.is_empty()
                && effects
                    .iter()
                    .all(|effect| matches!(effect, AbilityEffect::Draw(draw) if draw.count > 0))
        }
        _ => false,
    }
}

fn exact_generic_ward_ability(
    ability: &ExecutableAbility,
    event: &crate::ability_program::TriggerEvent,
    ward: &crate::ability_program::WardEffect,
) -> bool {
    let [AbilityCost::Mana(ProgramManaCost::PrintedSymbols { oracle, profile })] =
        ward.payment.as_slice()
    else {
        return false;
    };
    let exact_generic = profile.generic > 0
        && profile.white == 0
        && profile.blue == 0
        && profile.black == 0
        && profile.red == 0
        && profile.green == 0
        && profile.colorless == 0
        && profile.variable_x == 0
        && *oracle == format!("{{{}}}", profile.generic);
    ability.costs.is_empty()
        && ability.preconditions
            == [
                AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                AbilityPrecondition::EventObjectIsSource,
                AbilityPrecondition::EventObjectMatches(event.object_filter.clone()),
            ]
        && event.kind == TriggerEventKind::SourceBecomesTargetByOpponentSpellOrAbility
        && event.actor == ControllerRelation::Opponent
        && event.object_filter
            == (ProgramObjectFilter {
                card_type: Some(ProgramCardType::Permanent),
                controller: Some(ControllerRelation::You),
                ..ProgramObjectFilter::default()
            })
        && ward.target == crate::ability_program::TargetSelector::SelfPermanent
        && ward.triggering_object_controller == ControllerRelation::Opponent
        && ward.counter_triggering_spell_or_ability_unless_paid
        && exact_generic
}

fn is_exact_scry_resolution(ability: &ExecutableAbility, effects: &[AbilityEffect]) -> bool {
    let exact_scry = |effect: &AbilityEffect| {
        matches!(
            effect,
            AbilityEffect::Scry(scry)
                if scry.player == ControllerRelation::You
                    && scry.count > 0
                    && scry.from == ProgramZone::Library
                    && scry.may_put_any_number_on_bottom
                    && scry.preserve_kept_relative_order
                    && scry.may_order_bottom_cards
        )
    };
    let exact_draw = |effect: &AbilityEffect| {
        matches!(
            effect,
            AbilityEffect::Draw(draw)
                if draw.count > 0
                    && !draw.optional
                    && draw.unless_event_player_pays.is_none()
        )
    };
    let exact_effects = match effects {
        [scry] => exact_scry(scry),
        [scry, draw] => exact_scry(scry) && exact_draw(draw),
        _ => false,
    };
    ability.costs.is_empty()
        && ability.preconditions == [AbilityPrecondition::SourceZone(ProgramZone::Stack)]
        && exact_effects
}

fn has_battlefield_source_only(ability: &ExecutableAbility) -> bool {
    ability.preconditions == [AbilityPrecondition::SourceZone(ProgramZone::Battlefield)]
}

fn exact_event_preconditions(
    ability: &ExecutableAbility,
    event: &crate::ability_program::TriggerEvent,
) -> bool {
    ability.preconditions
        == [
            AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
            AbilityPrecondition::EventObjectMatches(event.object_filter.clone()),
        ]
}

fn spell_cost_reduction_preconditions_match(
    ability: &ExecutableAbility,
    reduction: &crate::ability_program::SpellCostReductionEffect,
) -> bool {
    match reduction.affected_spell {
        SpellCostReductionScope::CreatureSpellYouCastWithKeyword(_) => {
            reduction.condition.is_none() && has_battlefield_source_only(ability)
        }
        SpellCostReductionScope::SourceSpell => {
            ability.preconditions.is_empty()
                && matches!(
                    reduction.condition,
                    None | Some(SpellCostReductionCondition::YouControlCreatureWithKeyword(
                        _
                    ))
                )
        }
    }
}

fn is_nonzero_token_effect(effect: &AbilityEffect) -> bool {
    matches!(effect, AbilityEffect::CreateToken(token) if token.count > 0)
}

fn is_live_upkeep_effect(effect: &AbilityEffect) -> bool {
    match effect {
        AbilityEffect::LoseLife(loss) => loss.player == ControllerRelation::You && loss.amount > 0,
        AbilityEffect::CreateToken(token) => token.count > 0,
        AbilityEffect::Conditional(conditional) => {
            conditional.controller == ControllerRelation::You
                && conditional.condition == ControllerStateCondition::IsMonarch
                && !conditional.if_true.is_empty()
                && !conditional.if_false.is_empty()
                && conditional.if_true.iter().all(is_nonzero_token_effect)
                && conditional.if_false.iter().all(is_nonzero_token_effect)
        }
        _ => false,
    }
}

fn is_live_draw_trigger_event(kind: TriggerEventKind) -> bool {
    matches!(
        kind,
        TriggerEventKind::SpellCast
            | TriggerEventKind::FirstFilteredSpellCastEachTurn
            | TriggerEventKind::SecondSpellCastEachTurn
            | TriggerEventKind::CardDraw
            | TriggerEventKind::EnchantedCreatureDealsDamageToOpponent
            | TriggerEventKind::EquippedCreatureDies
            | TriggerEventKind::CreatureDealsCombatDamageToPlayer
            | TriggerEventKind::OneOrMoreCreaturesDealCombatDamageToPlayer
            | TriggerEventKind::ChosenTypeCreatureEntersOrAttacks
    )
}

fn is_live_draw_trigger_effect(effect: &AbilityEffect) -> bool {
    match effect {
        AbilityEffect::Draw(draw) => draw.count > 0,
        AbilityEffect::UnlessEventPlayerPays(unless) => {
            !unless.if_not_paid.is_empty()
                && unless.if_not_paid.iter().all(|effect| match effect {
                    AbilityEffect::Draw(draw) => draw.count > 0,
                    AbilityEffect::LoseLife(loss) => {
                        loss.player == ControllerRelation::You && loss.amount > 0
                    }
                    AbilityEffect::CreateToken(token) => {
                        token.count > 0 && token.kind == TokenKind::Treasure
                    }
                    _ => false,
                })
        }
        _ => false,
    }
}

fn is_exact_quest_counter_trigger(ability: &ExecutableAbility) -> bool {
    let AbilityTiming::Triggered { event } = &ability.timing else {
        return false;
    };
    let [AbilityEffect::AddCounters(counters)] = ability.effects.as_slice() else {
        return false;
    };
    ability.costs.is_empty()
        && event.kind == TriggerEventKind::BeginningOfEndStep
        && event.actor == ControllerRelation::Opponent
        && counters.target == CounterTarget::SourcePermanent
        && counters.counter == CounterKind::Quest
        && counters.count > 0
        && counters.optional
        && ability.preconditions
            == [
                AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                AbilityPrecondition::EventObjectMatches(event.object_filter.clone()),
                AbilityPrecondition::ControllerCondition(
                    crate::ability_program::ControllerConditionPrecondition {
                        controller: ControllerRelation::You,
                        condition: ControllerStateCondition::LostNoLifeThisTurn,
                        check_when_triggering_and_resolving: true,
                    },
                ),
            ]
}

fn is_exact_quest_token_activation(ability: &ExecutableAbility) -> bool {
    let [AbilityEffect::CreateToken(token)] = ability.effects.as_slice() else {
        return false;
    };
    matches!(
        ability.timing,
        AbilityTiming::Activated {
            window: ActivationWindow::NormalPriority
        }
    ) && matches!(ability.costs.as_slice(), [AbilityCost::Mana(_)])
        && matches!(
            ability.preconditions.as_slice(),
            [
                AbilityPrecondition::SourceZone(ProgramZone::Battlefield),
                AbilityPrecondition::SourceCounterAtLeast {
                    counter: CounterKind::Quest,
                    count
                }
            ] if *count > 0
        )
        && token.count > 0
        && matches!(token.kind, TokenKind::Creature { .. })
}

fn live_ability_source_evidence(
    card: &CompiledCard,
    selected_clause_indices: &[usize],
    executor_id: &'static str,
) -> Option<RuntimeSourceEvidence> {
    let mut clauses = card
        .ability_program
        .abilities
        .iter()
        .map(|compilation| match compilation {
            AbilityCompilation::Executable(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
            AbilityCompilation::Unsupported(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
        })
        .collect::<Vec<_>>();
    clauses.sort_by_key(|(clause_index, _)| *clause_index);
    if clauses.is_empty()
        || clauses
            .iter()
            .enumerate()
            .any(|(expected, (actual, _))| expected != *actual)
    {
        return None;
    }
    let normalized_oracle = clauses
        .iter()
        .map(|(_, clause)| *clause)
        .collect::<Vec<_>>()
        .join(" ");
    let normalized_oracle_sha256 = sha256_hex(normalized_oracle.as_bytes());
    let normalized_oracle_clause_sha256s = clauses
        .iter()
        .map(|(_, clause)| sha256_hex(clause.as_bytes()))
        .collect::<Vec<_>>();
    let mut selected = selected_clause_indices.to_vec();
    selected.sort_unstable();
    selected.dedup();
    if selected.len() != selected_clause_indices.len() {
        return None;
    }
    let covered_oracle_clauses = selected
        .into_iter()
        .map(|clause_index| {
            let (_, clause) = clauses.get(clause_index)?;
            Some(RuntimeOracleClauseEvidence {
                face_index: 0,
                clause_index: u16::try_from(clause_index).ok()?,
                normalized_clause_sha256: sha256_hex(clause.as_bytes()),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let type_line_sha256 = sha256_hex(card.type_line.as_bytes());
    let relevant_type_role_mask = card.roles & RUNTIME_KIND_ROLE_MASK;
    let mut hasher = Sha256::new();
    for part in [
        RUNTIME_RECEIPT_SCHEMA_VERSION.as_bytes(),
        LIVE_ABILITY_EXECUTOR_VERSION.as_bytes(),
        EXECUTABLE_ABILITY_PROGRAM_VERSION.as_bytes(),
        executor_id.as_bytes(),
        normalized_oracle.as_bytes(),
        card.type_line.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    hash_framed(&mut hasher, &relevant_type_role_mask.to_be_bytes());
    for clause in &covered_oracle_clauses {
        hash_framed(&mut hasher, &clause.face_index.to_be_bytes());
        hash_framed(&mut hasher, &clause.clause_index.to_be_bytes());
        hash_framed(&mut hasher, clause.normalized_clause_sha256.as_bytes());
    }
    Some(RuntimeSourceEvidence {
        ability_program_version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
        normalized_oracle_sha256,
        normalized_oracle_clause_sha256s,
        covered_oracle_clauses,
        type_line_sha256,
        relevant_type_role_mask,
        source_evidence_sha256: format!("{:x}", hasher.finalize()),
    })
}

pub(crate) fn compile_land_runtime_receipts(card: &CompiledCard) -> Vec<LandRuntimeReceipt> {
    let Some(program) = classify_exact_land_program(&card.type_line, &card.ability_program) else {
        return Vec::new();
    };
    let fetch_lifecycle_is_consumed = program.fetchland.is_some();
    let trajectory_is_consumed =
        !fetch_lifecycle_is_consumed && program.has_exact_trajectory_source();
    if !fetch_lifecycle_is_consumed && !trajectory_is_consumed {
        return Vec::new();
    }

    let mut receipts = program
        .binding_inputs()
        .into_iter()
        .filter(|input| {
            if fetch_lifecycle_is_consumed {
                matches!(
                    input.subject,
                    ExactLandRuntimeSubject::FetchTwoBasicLandTypes { .. }
                )
            } else {
                !matches!(
                    input.subject,
                    ExactLandRuntimeSubject::FetchTwoBasicLandTypes { .. }
                )
            }
        })
        .filter_map(|input| land_runtime_receipt(card, input))
        .collect::<Vec<_>>();
    receipts.sort_by(|left, right| {
        left.binding
            .executor_id
            .cmp(right.binding.executor_id)
            .then_with(|| left.contract_sha256.cmp(&right.contract_sha256))
    });
    receipts
}

fn land_runtime_receipt(
    card: &CompiledCard,
    input: ExactLandRuntimeBindingInput,
) -> Option<LandRuntimeReceipt> {
    let executor_id = input.subject.executor_id();
    let subject_payload = exact_land_subject_payload(&input.subject)?;
    let capability = land_subject_capability(&input.subject);
    let source_evidence = if matches!(input.subject, ExactLandRuntimeSubject::BasicTypeMana { .. })
    {
        if !input.covered_oracle_clauses.is_empty() {
            return None;
        }
        land_basic_type_source_evidence(card, executor_id, &subject_payload)
    } else {
        land_oracle_source_evidence(
            card,
            executor_id,
            &subject_payload,
            &input.covered_oracle_clauses,
        )?
    };
    let binding = RuntimeExecutorBinding {
        receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
        executor_id,
        executor_version: LAND_RUNTIME_EXECUTOR_VERSION,
    };
    let capabilities = vec![capability];
    let contract_sha256 =
        land_receipt_contract_sha256(&binding, &capabilities, &source_evidence, &subject_payload);
    Some(LandRuntimeReceipt {
        binding,
        capabilities,
        source_evidence,
        subject: input.subject,
        contract_sha256,
    })
}

fn land_subject_capability(subject: &ExactLandRuntimeSubject) -> RuntimeCapability {
    if matches!(subject, ExactLandRuntimeSubject::BasicTypeMana { .. }) {
        RuntimeCapability::ExactCompiledCharacteristic
    } else {
        RuntimeCapability::ExactOracleClauseSet
    }
}

fn exact_land_subject_payload(subject: &ExactLandRuntimeSubject) -> Option<String> {
    match subject {
        ExactLandRuntimeSubject::BasicTypeMana { subtypes, colors } => {
            if subtypes.is_empty()
                || !is_strictly_sorted(subtypes)
                || !is_strictly_sorted(colors)
                || colors
                    != &subtypes
                        .iter()
                        .copied()
                        .map(BasicLandSubtype::mana_color)
                        .collect::<Vec<_>>()
            {
                return None;
            }
            Some(format!(
                "basic-type-mana:{}:{}",
                subtypes
                    .iter()
                    .copied()
                    .map(basic_land_subtype_tag)
                    .collect::<Vec<_>>()
                    .join(","),
                colors
                    .iter()
                    .copied()
                    .map(land_mana_color_tag)
                    .collect::<Vec<_>>()
                    .join(",")
            ))
        }
        ExactLandRuntimeSubject::FixedPrintedMana { colors } => {
            if colors.is_empty() || !is_strictly_sorted(colors) {
                return None;
            }
            Some(format!(
                "fixed-printed-mana:{}",
                colors
                    .iter()
                    .copied()
                    .map(land_mana_color_tag)
                    .collect::<Vec<_>>()
                    .join(",")
            ))
        }
        ExactLandRuntimeSubject::AlwaysTappedEntry => Some("entry:always-tapped".into()),
        ExactLandRuntimeSubject::PayTwoLifeOrTappedEntry { life } if *life == 2 => {
            Some("entry:pay-two-life-or-tapped:2".into())
        }
        ExactLandRuntimeSubject::UntappedWithAtLeastOpponents { minimum_opponents }
            if *minimum_opponents == 2 =>
        {
            Some("entry:untapped-with-opponents:2".into())
        }
        ExactLandRuntimeSubject::FetchTwoBasicLandTypes {
            first_subtype,
            second_subtype,
        } if first_subtype != second_subtype => Some(format!(
            "fetch-two-basic-land-types:{}:{}",
            basic_land_subtype_tag(*first_subtype),
            basic_land_subtype_tag(*second_subtype)
        )),
        _ => None,
    }
}

fn basic_land_subtype_tag(subtype: BasicLandSubtype) -> &'static str {
    match subtype {
        BasicLandSubtype::Plains => "plains",
        BasicLandSubtype::Island => "island",
        BasicLandSubtype::Swamp => "swamp",
        BasicLandSubtype::Mountain => "mountain",
        BasicLandSubtype::Forest => "forest",
    }
}

fn land_mana_color_tag(color: LandManaColor) -> &'static str {
    match color {
        LandManaColor::White => "w",
        LandManaColor::Blue => "u",
        LandManaColor::Black => "b",
        LandManaColor::Red => "r",
        LandManaColor::Green => "g",
        LandManaColor::Colorless => "c",
    }
}

fn is_strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn land_basic_type_source_evidence(
    card: &CompiledCard,
    executor_id: &'static str,
    subject_payload: &str,
) -> RuntimeSourceEvidence {
    let payload_sha256 = sha256_hex(subject_payload.as_bytes());
    let covered_oracle_clauses = vec![RuntimeOracleClauseEvidence {
        face_index: 0,
        clause_index: 0,
        normalized_clause_sha256: payload_sha256.clone(),
    }];
    let type_line_sha256 = sha256_hex(card.type_line.as_bytes());
    let relevant_type_role_mask = card.roles & RUNTIME_KIND_ROLE_MASK;
    let mut hasher = Sha256::new();
    for part in [
        RUNTIME_RECEIPT_SCHEMA_VERSION.as_bytes(),
        LAND_RUNTIME_EXECUTOR_VERSION.as_bytes(),
        EXECUTABLE_ABILITY_PROGRAM_VERSION.as_bytes(),
        executor_id.as_bytes(),
        subject_payload.as_bytes(),
        card.type_line.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    hash_framed(&mut hasher, &relevant_type_role_mask.to_be_bytes());
    hash_framed(&mut hasher, &0u16.to_be_bytes());
    hash_framed(&mut hasher, payload_sha256.as_bytes());
    RuntimeSourceEvidence {
        ability_program_version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
        normalized_oracle_sha256: payload_sha256.clone(),
        normalized_oracle_clause_sha256s: vec![payload_sha256.clone()],
        covered_oracle_clauses,
        type_line_sha256,
        relevant_type_role_mask,
        source_evidence_sha256: format!("{:x}", hasher.finalize()),
    }
}

fn land_oracle_source_evidence(
    card: &CompiledCard,
    executor_id: &'static str,
    subject_payload: &str,
    selected_clauses: &[ExactLandClauseEvidence],
) -> Option<RuntimeSourceEvidence> {
    if selected_clauses.is_empty() {
        return None;
    }
    let mut clauses = card
        .ability_program
        .abilities
        .iter()
        .map(|compilation| match compilation {
            AbilityCompilation::Executable(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
            AbilityCompilation::Unsupported(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
        })
        .collect::<Vec<_>>();
    clauses.sort_by_key(|(clause_index, _)| *clause_index);
    if clauses.is_empty()
        || clauses
            .iter()
            .enumerate()
            .any(|(expected, (actual, _))| expected != *actual)
    {
        return None;
    }
    let normalized_oracle = clauses
        .iter()
        .map(|(_, clause)| *clause)
        .collect::<Vec<_>>()
        .join(" ");
    let normalized_oracle_sha256 = sha256_hex(normalized_oracle.as_bytes());
    let normalized_oracle_clause_sha256s = clauses
        .iter()
        .map(|(_, clause)| sha256_hex(clause.as_bytes()))
        .collect::<Vec<_>>();

    let mut selected = selected_clauses.to_vec();
    selected.sort();
    selected.dedup();
    if selected.len() != selected_clauses.len() {
        return None;
    }
    let covered_oracle_clauses = selected
        .into_iter()
        .map(|evidence| {
            let (_, clause) = clauses.get(usize::from(evidence.clause_index))?;
            if clause.trim().to_ascii_lowercase() != evidence.normalized_clause {
                return None;
            }
            Some(RuntimeOracleClauseEvidence {
                face_index: 0,
                clause_index: evidence.clause_index,
                normalized_clause_sha256: sha256_hex(clause.as_bytes()),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let type_line_sha256 = sha256_hex(card.type_line.as_bytes());
    let relevant_type_role_mask = card.roles & RUNTIME_KIND_ROLE_MASK;
    let mut hasher = Sha256::new();
    for part in [
        RUNTIME_RECEIPT_SCHEMA_VERSION.as_bytes(),
        LAND_RUNTIME_EXECUTOR_VERSION.as_bytes(),
        EXECUTABLE_ABILITY_PROGRAM_VERSION.as_bytes(),
        executor_id.as_bytes(),
        subject_payload.as_bytes(),
        normalized_oracle.as_bytes(),
        card.type_line.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    hash_framed(&mut hasher, &relevant_type_role_mask.to_be_bytes());
    for clause in &covered_oracle_clauses {
        hash_framed(&mut hasher, &clause.face_index.to_be_bytes());
        hash_framed(&mut hasher, &clause.clause_index.to_be_bytes());
        hash_framed(&mut hasher, clause.normalized_clause_sha256.as_bytes());
    }
    Some(RuntimeSourceEvidence {
        ability_program_version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
        normalized_oracle_sha256,
        normalized_oracle_clause_sha256s,
        covered_oracle_clauses,
        type_line_sha256,
        relevant_type_role_mask,
        source_evidence_sha256: format!("{:x}", hasher.finalize()),
    })
}

fn land_receipt_contract_sha256(
    binding: &RuntimeExecutorBinding,
    capabilities: &[RuntimeCapability],
    source_evidence: &RuntimeSourceEvidence,
    subject_payload: &str,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        binding.receipt_schema_version.as_bytes(),
        binding.executor_id.as_bytes(),
        binding.executor_version.as_bytes(),
        subject_payload.as_bytes(),
        source_evidence.ability_program_version.as_bytes(),
        source_evidence.normalized_oracle_sha256.as_bytes(),
        source_evidence.type_line_sha256.as_bytes(),
        source_evidence.source_evidence_sha256.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    for capability in capabilities {
        let tag = match capability {
            RuntimeCapability::ExactOracleClauseSet => "exact-oracle-clause-set",
            RuntimeCapability::ExactCompiledCharacteristic => "exact-compiled-characteristic",
            _ => "unsupported-land-capability",
        };
        hash_framed(&mut hasher, tag.as_bytes());
    }
    for digest in &source_evidence.normalized_oracle_clause_sha256s {
        hash_framed(&mut hasher, digest.as_bytes());
    }
    for clause in &source_evidence.covered_oracle_clauses {
        hash_framed(&mut hasher, &clause.face_index.to_be_bytes());
        hash_framed(&mut hasher, &clause.clause_index.to_be_bytes());
        hash_framed(&mut hasher, clause.normalized_clause_sha256.as_bytes());
    }
    hash_framed(
        &mut hasher,
        &source_evidence.relevant_type_role_mask.to_be_bytes(),
    );
    format!("{:x}", hasher.finalize())
}

/// Add stable coverage evidence to the exact transaction selected by the
/// cheap runtime classifier. Planner/runtime hot paths should call
/// `classify_atomic_runtime_transaction` directly and avoid hashing.
pub(crate) fn compile_atomic_runtime_receipt(card: &CompiledCard) -> Option<AtomicRuntimeReceipt> {
    let transaction = classify_atomic_runtime_transaction(card)?;
    let source_transaction = card.ability_program.executable_atomic_transaction()?;
    let executor_id = transaction.executor_id();
    let source_evidence = runtime_source_evidence(
        card,
        source_transaction.normalized_oracle.as_str(),
        executor_id,
        ATOMIC_TRANSACTION_EXECUTOR_VERSION,
    );
    let mut capabilities = vec![
        RuntimeCapability::CompleteOracleRoot,
        RuntimeCapability::AtomicInitiationBoundary,
        RuntimeCapability::OrderedResolution,
    ];
    match transaction.initiation() {
        AtomicInitiation::CastSpell => {
            capabilities.push(RuntimeCapability::CounteredSpellResolutionBoundary);
        }
        AtomicInitiation::HandManaAbility => {
            capabilities.push(RuntimeCapability::HandManaAbilityWithoutStack);
        }
    }
    if matches!(
        transaction,
        TypedAtomicTransaction::BargainSearchCastOrHand { .. }
    ) {
        capabilities.push(RuntimeCapability::ExactBargainKeyword);
    }
    if matches!(transaction, TypedAtomicTransaction::ThresholdRitual { .. }) {
        capabilities.push(RuntimeCapability::ExactThresholdAbilityWord);
    }

    Some(AtomicRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id,
            executor_version: ATOMIC_TRANSACTION_EXECUTOR_VERSION,
        },
        capabilities,
        source_evidence,
        transaction,
    })
}

pub(crate) fn compile_graveyard_reclamation_runtime_receipt(
    card: &CompiledCard,
) -> Option<GraveyardReclamationRuntimeReceipt> {
    let program = classify_graveyard_reclamation(card)?.clone();
    let executor_id = "abstract-play.graveyard-reclamation.sevinne";
    let source_evidence = runtime_source_evidence(
        card,
        program.normalized_oracle.as_str(),
        executor_id,
        GRAVEYARD_RECLAMATION_EXECUTOR_VERSION,
    );
    Some(GraveyardReclamationRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id,
            executor_version: GRAVEYARD_RECLAMATION_EXECUTOR_VERSION,
        },
        capabilities: vec![
            RuntimeCapability::CompleteOracleRoot,
            RuntimeCapability::OrderedResolution,
            RuntimeCapability::CounteredSpellResolutionBoundary,
            RuntimeCapability::ExactFlashbackKeyword,
            RuntimeCapability::ExactPhysicalZoneObjectIdentity,
            RuntimeCapability::OptionalUncastSpellCopy,
            RuntimeCapability::OptionalLegalCopyRetarget,
            RuntimeCapability::SourceExitBeforeCopyResolution,
            RuntimeCapability::SourceReturnTriggersAboveCopy,
        ],
        source_evidence,
        program,
    })
}

pub(crate) fn compile_spell_resolution_mana_runtime_receipt(
    card: &CompiledCard,
) -> Option<SpellResolutionManaRuntimeReceipt> {
    let program = classify_spell_resolution_mana(card)?;
    let ability = card.ability_program.executable_abilities().next()?;
    let executor_id = "abstract-play.spell-resolution.fixed-mana";
    let source_evidence = runtime_source_evidence(
        card,
        ability.normalized_oracle.as_str(),
        executor_id,
        SPELL_RESOLUTION_MANA_EXECUTOR_VERSION,
    );

    Some(SpellResolutionManaRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id,
            executor_version: SPELL_RESOLUTION_MANA_EXECUTOR_VERSION,
        },
        capabilities: vec![
            RuntimeCapability::CompleteOracleRoot,
            RuntimeCapability::OrderedResolution,
            RuntimeCapability::CounteredSpellResolutionBoundary,
        ],
        source_evidence,
        program,
    })
}

pub(crate) fn compile_conditional_mana_source_runtime_receipt(
    card: &CompiledCard,
) -> Option<ConditionalManaSourceRuntimeReceipt> {
    let source = classify_conditional_mana_source(card)?;
    let executor_id = source.executor_id()?;
    let normalized_oracle = if source.is_entry_linked() {
        card.ability_program
            .executable_entry_linked_permanent()?
            .normalized_oracle
            .as_str()
    } else {
        card.ability_program
            .executable_abilities()
            .next()?
            .normalized_oracle
            .as_str()
    };
    let source_evidence = runtime_source_evidence(
        card,
        normalized_oracle,
        executor_id,
        CONDITIONAL_MANA_SOURCE_EXECUTOR_VERSION,
    );
    let capability = if source.is_entry_linked() {
        RuntimeCapability::ExactPermanentEntryProcedure
    } else {
        RuntimeCapability::LiveBattlefieldManaCondition
    };

    let mut capabilities = vec![RuntimeCapability::CompleteOracleRoot, capability];
    match source {
        TypedConditionalManaSource::ImprintLinkedCardColors => {
            capabilities.push(RuntimeCapability::ExactImprintAbilityWord);
        }
        TypedConditionalManaSource::MetalcraftAnyColor => {
            capabilities.push(RuntimeCapability::ExactMetalcraftAbilityWord);
        }
        _ => {}
    }

    Some(ConditionalManaSourceRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id,
            executor_version: CONDITIONAL_MANA_SOURCE_EXECUTOR_VERSION,
        },
        capabilities,
        source_evidence,
        source,
    })
}

pub(crate) fn compile_sacrifice_self_mana_runtime_receipt(
    card: &CompiledCard,
) -> Option<SacrificeSelfManaRuntimeReceipt> {
    let program = classify_sacrifice_self_any_color_mana(card)?;
    let ability = card.ability_program.executable_abilities().next()?;
    let executor_id = "abstract-play.activated.sacrifice-self-any-color-mana";
    let source_evidence = runtime_source_evidence(
        card,
        ability.normalized_oracle.as_str(),
        executor_id,
        SACRIFICE_SELF_MANA_EXECUTOR_VERSION,
    );
    Some(SacrificeSelfManaRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id,
            executor_version: SACRIFICE_SELF_MANA_EXECUTOR_VERSION,
        },
        capabilities: vec![
            RuntimeCapability::CompleteOracleRoot,
            RuntimeCapability::SacrificeSelfManaAbility,
        ],
        source_evidence,
        program,
    })
}

fn interaction_executor_id(program: &InteractionRuntimeProgram) -> &'static str {
    match program {
        InteractionRuntimeProgram::TargetedPermanent {
            action: InteractionAction::Destroy,
            ..
        } => "abstract-play.interaction.targeted-destroy",
        InteractionRuntimeProgram::TargetedPermanent {
            action: InteractionAction::Exile,
            ..
        } => "abstract-play.interaction.targeted-exile",
        InteractionRuntimeProgram::TargetedPermanent {
            action: InteractionAction::ReturnToOwnersHand,
            ..
        } => "abstract-play.interaction.targeted-bounce",
        InteractionRuntimeProgram::Counterspell {
            unless_controller_pays_generic: None,
            ..
        } => "abstract-play.interaction.counterspell",
        InteractionRuntimeProgram::Counterspell {
            unless_controller_pays_generic: Some(_),
            ..
        } => "abstract-play.interaction.counter-unless-payment",
        InteractionRuntimeProgram::DestroyAll { .. } => "abstract-play.interaction.destroy-all",
    }
}

fn tutor_executor_id(program: &TutorRuntimeProgram) -> &'static str {
    match program {
        TutorRuntimeProgram::TriggeredPermanent(program)
            if program.search.searched_card == SearchedCardPredicate::BasicLand
                && program.search.destination_zone() == Some(TutorDestinationZone::Hand) =>
        {
            "abstract-play.tutor.upkeep-basic-lands"
        }
        TutorRuntimeProgram::Spell(program)
            if program.search.searched_card == SearchedCardPredicate::ArtifactOrEnchantment
                && program.search.destination_zone()
                    == Some(TutorDestinationZone::TopOfLibrary)
                && program.life_loss.is_none() =>
        {
            "abstract-play.tutor.artifact-enchantment-top"
        }
        TutorRuntimeProgram::Spell(program)
            if program.search.searched_card == SearchedCardPredicate::Enchantment
                && program.search.destination_zone() == Some(TutorDestinationZone::Hand)
                && program.life_loss.is_none() =>
        {
            "abstract-play.tutor.enchantment-hand"
        }
        TutorRuntimeProgram::Spell(program)
            if program.search.searched_card == SearchedCardPredicate::AnyCard
                && program.search.destination_zone() == Some(TutorDestinationZone::Hand)
                && matches!(
                    program.life_loss,
                    Some(crate::tutor_runtime::LifeLoss {
                        amount: 3,
                        timing: LifeLossTiming::AfterSearchTransaction,
                    })
                ) =>
        {
            "abstract-play.tutor.any-card-hand-lose-three"
        }
        _ => "abstract-play.tutor.invalid",
    }
}

fn tutor_runtime_capabilities(program: &TutorRuntimeProgram) -> Vec<RuntimeCapability> {
    let mut capabilities = vec![
        RuntimeCapability::CompleteOracleRoot,
        RuntimeCapability::OrderedResolution,
    ];
    if matches!(program, TutorRuntimeProgram::Spell(_)) {
        capabilities.push(RuntimeCapability::CounteredSpellResolutionBoundary);
    }
    capabilities
}

fn restriction_protection_executor_id(program: &RestrictionProtectionProgram) -> &'static str {
    match program {
        RestrictionProtectionProgram::AttackTax(_) => "abstract-play.restriction.attack-tax",
        RestrictionProtectionProgram::OpponentTurnRestriction(_) => {
            "abstract-play.restriction.opponent-turn-actions"
        }
        RestrictionProtectionProgram::KeywordGrant(_) => "abstract-play.protection.keyword-grant",
        RestrictionProtectionProgram::AuraRestriction(_) => "abstract-play.restriction.aura",
        RestrictionProtectionProgram::KeywordRemoval(_) => {
            "abstract-play.protection.keyword-removal"
        }
        RestrictionProtectionProgram::CompleteProtection(_) => {
            "abstract-play.protection.complete-turn"
        }
    }
}

fn restriction_protection_capabilities(
    compiled: &CompiledRestrictionProtection,
) -> Vec<RuntimeCapability> {
    let mut capabilities = vec![match compiled.ownership {
        RestrictionOracleOwnership::CompleteRoot { .. } => RuntimeCapability::CompleteOracleRoot,
        RestrictionOracleOwnership::ExactClauseSet { .. } => {
            RuntimeCapability::ExactOracleClauseSet
        }
    }];
    if matches!(
        compiled.program,
        RestrictionProtectionProgram::CompleteProtection(_)
    ) {
        capabilities.push(RuntimeCapability::OrderedResolution);
        capabilities.push(RuntimeCapability::CounteredSpellResolutionBoundary);
        capabilities.push(RuntimeCapability::ExactProtectionKeyword);
    }
    if matches!(
        compiled.program,
        RestrictionProtectionProgram::AuraRestriction(_)
    ) {
        capabilities.push(RuntimeCapability::ExactEnchantKeyword);
    }
    capabilities
}

fn reviewed_runtime_executor_version(program: &ReviewedRuntimeProgram) -> &'static str {
    match program {
        ReviewedRuntimeProgram::AlternativeCast(_) => ALTERNATIVE_CAST_EXECUTOR_VERSION,
        ReviewedRuntimeProgram::CharacteristicOracle(_) => CHARACTERISTIC_ORACLE_EXECUTOR_VERSION,
        ReviewedRuntimeProgram::ContinuousTrigger(_) => CONTINUOUS_TRIGGER_EXECUTOR_VERSION,
        ReviewedRuntimeProgram::ManaNetwork(_) => MANA_NETWORK_RUNTIME_EXECUTOR_VERSION,
        ReviewedRuntimeProgram::ObjectLifecycle(_) => OBJECT_LIFECYCLE_EXECUTOR_VERSION,
        ReviewedRuntimeProgram::UtilityModal(_) => UTILITY_MODAL_EXECUTOR_VERSION,
    }
}

fn reviewed_runtime_executor_id(program: &ReviewedRuntimeProgram) -> &'static str {
    match program {
        ReviewedRuntimeProgram::AlternativeCast(compiled) => match &compiled.program {
            AlternativeCastRuntimeProgram::Bounce(_) => {
                "abstract-play.alternative-cast.overload-bounce"
            }
            AlternativeCastRuntimeProgram::ExileWithControllerCompensation(_) => {
                "abstract-play.alternative-cast.overload-exile-compensation"
            }
            AlternativeCastRuntimeProgram::ConditionalFreeCounter(_) => {
                "abstract-play.alternative-cast.commander-free-counter"
            }
            AlternativeCastRuntimeProgram::EscapeAura(_) => {
                "abstract-play.alternative-cast.escape-aura"
            }
        },
        ReviewedRuntimeProgram::CharacteristicOracle(compiled) => match &compiled.program {
            CharacteristicOracleProgram::PureCombatKeyword(_) => {
                "abstract-play.characteristic-oracle.combat-keyword"
            }
            CharacteristicOracleProgram::DevotionToughness(_) => {
                "abstract-play.characteristic-oracle.devotion-toughness"
            }
        },
        ReviewedRuntimeProgram::ContinuousTrigger(compiled) => match &compiled.program {
            ContinuousTriggerProgram::ContinuousCreatureModifier(_) => {
                "abstract-play.continuous.creature-modifier"
            }
            ContinuousTriggerProgram::TokenCreationTrigger(_) => {
                "abstract-play.trigger.token-creation"
            }
            ContinuousTriggerProgram::LifeGainTrigger(_) => "abstract-play.trigger.life-gain",
            ContinuousTriggerProgram::AttachmentMoveTrigger(_) => {
                "abstract-play.trigger.attachment-move"
            }
            ContinuousTriggerProgram::EquippedDeathReturn(_) => {
                "abstract-play.trigger.equipped-death-return"
            }
            ContinuousTriggerProgram::SpellCostReduction(_) => {
                "abstract-play.continuous.spell-cost-reduction"
            }
            ContinuousTriggerProgram::TemporarySelfModifierTrigger(_) => {
                "abstract-play.trigger.temporary-self-modifier"
            }
        },
        ReviewedRuntimeProgram::ManaNetwork(program) => match program {
            ExactManaNetworkProgram::CommanderIdentityMana(_) => {
                COMMANDER_IDENTITY_MANA_EXECUTOR_ID
            }
            ExactManaNetworkProgram::ControlledLandCapabilityMana(_) => {
                CONTROLLED_LAND_CAPABILITY_MANA_EXECUTOR_ID
            }
            ExactManaNetworkProgram::ControlledLandAnyColorGrant(_) => {
                CONTROLLED_LAND_ANY_COLOR_GRANT_EXECUTOR_ID
            }
            ExactManaNetworkProgram::GlobalBasicLandSubtypeGrant(_) => {
                GLOBAL_BASIC_LAND_SUBTYPE_GRANT_EXECUTOR_ID
            }
            ExactManaNetworkProgram::SelfBounceDualLand(_) => SELF_BOUNCE_DUAL_LAND_EXECUTOR_ID,
        },
        ReviewedRuntimeProgram::ObjectLifecycle(compiled) => match &compiled.program {
            ObjectLifecycleProgram::LinkedExile(_) => "abstract-play.lifecycle.linked-exile",
            ObjectLifecycleProgram::DelayedExileReturn(_) => {
                "abstract-play.lifecycle.delayed-exile-return"
            }
            ObjectLifecycleProgram::ConditionalSelfReturn(_) => {
                "abstract-play.lifecycle.conditional-self-return"
            }
            ObjectLifecycleProgram::CreatureTokenReplacement(_) => {
                "abstract-play.lifecycle.token-replacement"
            }
            ObjectLifecycleProgram::CreatureEntryCounters(_) => {
                "abstract-play.lifecycle.creature-entry-counters"
            }
            ObjectLifecycleProgram::ModalGraveyardReturn(_) => {
                "abstract-play.lifecycle.modal-graveyard-return"
            }
            ObjectLifecycleProgram::AuraChoiceLifecycle(_) => {
                "abstract-play.lifecycle.aura-land-type-choice"
            }
        },
        ReviewedRuntimeProgram::UtilityModal(compiled) => match &compiled.program {
            UtilityModalRuntimeProgram::TopLibrary(_) => "abstract-play.utility.top-library",
            UtilityModalRuntimeProgram::SpellScryDraw(_) => "abstract-play.utility.spell-scry-draw",
            UtilityModalRuntimeProgram::EntryScry(_) => "abstract-play.utility.entry-scry",
            UtilityModalRuntimeProgram::TargetedDamagePrevention(_) => {
                "abstract-play.utility.damage-prevention"
            }
            UtilityModalRuntimeProgram::ActivatedWipe(_) => "abstract-play.utility.activated-wipe",
            UtilityModalRuntimeProgram::RetaliatoryDestroy(_) => {
                "abstract-play.utility.retaliatory-destroy"
            }
            UtilityModalRuntimeProgram::ModalCreatureInteraction(_) => {
                "abstract-play.utility.modal-creature-interaction"
            }
            UtilityModalRuntimeProgram::FaerieThresholdCounter(_) => {
                "abstract-play.utility.faerie-threshold-counter"
            }
        },
    }
}

fn reviewed_runtime_capabilities(program: &ReviewedRuntimeProgram) -> Vec<RuntimeCapability> {
    match program {
        ReviewedRuntimeProgram::AlternativeCast(compiled) => {
            let mut capabilities = vec![
                RuntimeCapability::CompleteOracleRoot,
                RuntimeCapability::AtomicInitiationBoundary,
                RuntimeCapability::OrderedResolution,
                RuntimeCapability::CounteredSpellResolutionBoundary,
            ];
            match &compiled.program {
                AlternativeCastRuntimeProgram::Bounce(_)
                | AlternativeCastRuntimeProgram::ExileWithControllerCompensation(_) => {
                    capabilities.push(RuntimeCapability::ExactOverloadKeyword);
                }
                AlternativeCastRuntimeProgram::ConditionalFreeCounter(_) => {}
                AlternativeCastRuntimeProgram::EscapeAura(_) => {
                    capabilities.push(RuntimeCapability::ExactEscapeKeyword);
                    capabilities.push(RuntimeCapability::ExactEnchantKeyword);
                    capabilities.push(RuntimeCapability::ExactPhysicalZoneObjectIdentity);
                }
            }
            capabilities
        }
        ReviewedRuntimeProgram::CharacteristicOracle(_) => vec![
            RuntimeCapability::ExactOracleClauseSet,
            RuntimeCapability::ExactCompiledCharacteristic,
        ],
        ReviewedRuntimeProgram::ContinuousTrigger(compiled) => {
            let mut capabilities = vec![match compiled.ownership.oracle {
                ContinuousOracleOwnership::CompleteFaceRoot { .. } => {
                    RuntimeCapability::CompleteOracleRoot
                }
                ContinuousOracleOwnership::ExactClauseSet { .. } => {
                    RuntimeCapability::ExactOracleClauseSet
                }
            }];
            if matches!(
                &compiled.program,
                ContinuousTriggerProgram::ContinuousCreatureModifier(modifier)
                    if modifier
                        .source
                        .required_subtypes
                        .contains(&ContinuousCardSubtype::Aura)
            ) && matches!(
                compiled.ownership.oracle,
                ContinuousOracleOwnership::CompleteFaceRoot { .. }
            ) {
                capabilities.push(RuntimeCapability::ExactEnchantKeyword);
            }
            capabilities
        }
        ReviewedRuntimeProgram::ManaNetwork(program) => {
            let exact_capability = match program {
                ExactManaNetworkProgram::CommanderIdentityMana(_) => {
                    RuntimeCapability::ExactCommanderColorIdentityMana
                }
                ExactManaNetworkProgram::ControlledLandCapabilityMana(_) => {
                    RuntimeCapability::ExactControlledLandManaCapabilities
                }
                ExactManaNetworkProgram::ControlledLandAnyColorGrant(_) => {
                    RuntimeCapability::ExactControlledLandManaGrant
                }
                ExactManaNetworkProgram::GlobalBasicLandSubtypeGrant(_) => {
                    RuntimeCapability::ExactGlobalBasicLandSubtypeGrant
                }
                ExactManaNetworkProgram::SelfBounceDualLand(_) => {
                    RuntimeCapability::ExactSelfBounceDualLandLifecycle
                }
            };
            let mut capabilities = vec![
                RuntimeCapability::CompleteOracleRoot,
                RuntimeCapability::LiveBattlefieldManaCondition,
                exact_capability,
            ];
            if matches!(program, ExactManaNetworkProgram::SelfBounceDualLand(_)) {
                capabilities.push(RuntimeCapability::ExactPermanentEntryProcedure);
            }
            capabilities
        }
        ReviewedRuntimeProgram::ObjectLifecycle(compiled) => {
            let mut capabilities = vec![match compiled.ownership {
                ObjectLifecycleOracleOwnership::CompleteRoot { .. } => {
                    RuntimeCapability::CompleteOracleRoot
                }
                ObjectLifecycleOracleOwnership::ExactClauseSet { .. } => {
                    RuntimeCapability::ExactOracleClauseSet
                }
            }];
            capabilities.push(RuntimeCapability::OrderedResolution);
            let source_zone = match &compiled.program {
                ObjectLifecycleProgram::LinkedExile(program) => program.source.zone,
                ObjectLifecycleProgram::DelayedExileReturn(program) => program.source.zone,
                ObjectLifecycleProgram::ConditionalSelfReturn(program) => program.source.zone,
                ObjectLifecycleProgram::CreatureTokenReplacement(program) => program.source.zone,
                ObjectLifecycleProgram::CreatureEntryCounters(program) => program.source.zone,
                ObjectLifecycleProgram::ModalGraveyardReturn(program) => program.source.zone,
                ObjectLifecycleProgram::AuraChoiceLifecycle(program) => program.source.zone,
            };
            if source_zone == ObjectLifecycleSourceZone::Stack {
                capabilities.push(RuntimeCapability::CounteredSpellResolutionBoundary);
            }
            if matches!(
                &compiled.program,
                ObjectLifecycleProgram::LinkedExile(_)
                    | ObjectLifecycleProgram::DelayedExileReturn(_)
                    | ObjectLifecycleProgram::ConditionalSelfReturn(_)
            ) {
                capabilities.push(RuntimeCapability::ExactPhysicalZoneObjectIdentity);
            }
            if matches!(
                &compiled.program,
                ObjectLifecycleProgram::AuraChoiceLifecycle(_)
            ) {
                capabilities.push(RuntimeCapability::ExactEnchantKeyword);
            }
            capabilities
        }
        ReviewedRuntimeProgram::UtilityModal(compiled) => {
            let mut capabilities = vec![match compiled.ownership {
                UtilityModalOracleOwnership::CompleteRoot { .. } => {
                    RuntimeCapability::CompleteOracleRoot
                }
                UtilityModalOracleOwnership::ExactClauseSet { .. } => {
                    RuntimeCapability::ExactOracleClauseSet
                }
            }];
            capabilities.push(RuntimeCapability::OrderedResolution);
            let source_zone = match &compiled.program {
                UtilityModalRuntimeProgram::TopLibrary(program) => program.source.zone,
                UtilityModalRuntimeProgram::SpellScryDraw(program) => program.source.zone,
                UtilityModalRuntimeProgram::EntryScry(program) => program.source.zone,
                UtilityModalRuntimeProgram::TargetedDamagePrevention(program) => {
                    program.source.zone
                }
                UtilityModalRuntimeProgram::ActivatedWipe(program) => program.source.zone,
                UtilityModalRuntimeProgram::RetaliatoryDestroy(program) => program.source.zone,
                UtilityModalRuntimeProgram::ModalCreatureInteraction(program) => {
                    program.source.zone
                }
                UtilityModalRuntimeProgram::FaerieThresholdCounter(program) => program.source.zone,
            };
            if source_zone == UtilityModalSourceZone::Stack {
                capabilities.push(RuntimeCapability::CounteredSpellResolutionBoundary);
            }
            if matches!(
                &compiled.program,
                UtilityModalRuntimeProgram::SpellScryDraw(_)
                    | UtilityModalRuntimeProgram::EntryScry(_)
            ) {
                capabilities.push(RuntimeCapability::ExactScryKeyword);
            }
            if matches!(&compiled.program, UtilityModalRuntimeProgram::TopLibrary(_)) {
                capabilities.push(RuntimeCapability::ExactPhysicalZoneObjectIdentity);
            }
            capabilities
        }
    }
}

fn complete_normalized_program_root(card: &CompiledCard) -> Option<String> {
    let mut clauses = card
        .ability_program
        .abilities
        .iter()
        .map(|compilation| match compilation {
            AbilityCompilation::Executable(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
            AbilityCompilation::Unsupported(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
        })
        .collect::<Vec<_>>();
    clauses.sort_by_key(|(clause_index, _)| *clause_index);
    if clauses.is_empty()
        || clauses
            .iter()
            .enumerate()
            .any(|(expected, (actual, _))| expected != *actual)
    {
        return None;
    }
    Some(
        clauses
            .iter()
            .map(|(_, clause)| *clause)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn selected_program_clause_source_evidence(
    card: &CompiledCard,
    selected_clause_indices: &[u16],
    executor_id: &'static str,
    executor_version: &'static str,
) -> Option<RuntimeSourceEvidence> {
    let mut clauses = card
        .ability_program
        .abilities
        .iter()
        .map(|compilation| match compilation {
            AbilityCompilation::Executable(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
            AbilityCompilation::Unsupported(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
        })
        .collect::<Vec<_>>();
    clauses.sort_by_key(|(clause_index, _)| *clause_index);
    if clauses.is_empty()
        || clauses
            .iter()
            .enumerate()
            .any(|(expected, (actual, _))| expected != *actual)
    {
        return None;
    }
    let mut selected = selected_clause_indices.to_vec();
    selected.sort_unstable();
    selected.dedup();
    if selected.is_empty() || selected.len() != selected_clause_indices.len() {
        return None;
    }
    let normalized_oracle = clauses
        .iter()
        .map(|(_, clause)| *clause)
        .collect::<Vec<_>>()
        .join(" ");
    let normalized_oracle_sha256 = sha256_hex(normalized_oracle.as_bytes());
    let normalized_oracle_clause_sha256s = clauses
        .iter()
        .map(|(_, clause)| sha256_hex(clause.as_bytes()))
        .collect::<Vec<_>>();
    let covered_oracle_clauses = selected
        .into_iter()
        .map(|clause_index| {
            let (_, clause) = clauses.get(usize::from(clause_index))?;
            Some(RuntimeOracleClauseEvidence {
                face_index: 0,
                clause_index,
                normalized_clause_sha256: sha256_hex(clause.as_bytes()),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let type_line_sha256 = sha256_hex(card.type_line.as_bytes());
    let relevant_type_role_mask = card.roles & RUNTIME_KIND_ROLE_MASK;
    let mut hasher = Sha256::new();
    for part in [
        RUNTIME_RECEIPT_SCHEMA_VERSION.as_bytes(),
        executor_version.as_bytes(),
        EXECUTABLE_ABILITY_PROGRAM_VERSION.as_bytes(),
        executor_id.as_bytes(),
        normalized_oracle.as_bytes(),
        card.type_line.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    hash_framed(&mut hasher, &relevant_type_role_mask.to_be_bytes());
    for clause in &covered_oracle_clauses {
        hash_framed(&mut hasher, &clause.face_index.to_be_bytes());
        hash_framed(&mut hasher, &clause.clause_index.to_be_bytes());
        hash_framed(&mut hasher, clause.normalized_clause_sha256.as_bytes());
    }
    Some(RuntimeSourceEvidence {
        ability_program_version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
        normalized_oracle_sha256,
        normalized_oracle_clause_sha256s,
        covered_oracle_clauses,
        type_line_sha256,
        relevant_type_role_mask,
        source_evidence_sha256: format!("{:x}", hasher.finalize()),
    })
}

fn selected_program_face_clause_source_evidence(
    card: &CompiledCard,
    face_index: u16,
    selected_clause_indices: &[u16],
    executor_id: &'static str,
    executor_version: &'static str,
) -> Option<RuntimeSourceEvidence> {
    let (type_line, compilations) = if card.ability_program.face_programs.is_empty() {
        (face_index == 0).then_some((
            card.type_line.as_str(),
            card.ability_program.abilities.as_slice(),
        ))?
    } else {
        let face = card
            .ability_program
            .face_programs
            .iter()
            .find(|face| u16::try_from(face.face_index).ok() == Some(face_index))?;
        (face.type_line.as_str(), face.abilities.as_slice())
    };
    let mut clauses = compilations
        .iter()
        .map(|compilation| match compilation {
            AbilityCompilation::Executable(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
            AbilityCompilation::Unsupported(ability) => {
                (ability.clause_index, ability.normalized_oracle.as_str())
            }
        })
        .collect::<Vec<_>>();
    clauses.sort_by_key(|(clause_index, _)| *clause_index);
    if clauses.is_empty()
        || clauses
            .iter()
            .enumerate()
            .any(|(expected, (actual, _))| expected != *actual)
    {
        return None;
    }
    let mut selected = selected_clause_indices.to_vec();
    selected.sort_unstable();
    selected.dedup();
    if selected.is_empty() || selected.len() != selected_clause_indices.len() {
        return None;
    }
    let normalized_oracle = clauses
        .iter()
        .map(|(_, clause)| *clause)
        .collect::<Vec<_>>()
        .join(" ");
    let normalized_oracle_sha256 = sha256_hex(normalized_oracle.as_bytes());
    let normalized_oracle_clause_sha256s = clauses
        .iter()
        .map(|(_, clause)| sha256_hex(clause.as_bytes()))
        .collect::<Vec<_>>();
    let covered_oracle_clauses = selected
        .into_iter()
        .map(|clause_index| {
            let (_, clause) = clauses.get(usize::from(clause_index))?;
            Some(RuntimeOracleClauseEvidence {
                face_index,
                clause_index,
                normalized_clause_sha256: sha256_hex(clause.as_bytes()),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let type_line_sha256 = sha256_hex(type_line.as_bytes());
    let relevant_type_role_mask = runtime_kind_role_mask_for_type_line(type_line);
    let mut hasher = Sha256::new();
    for part in [
        RUNTIME_RECEIPT_SCHEMA_VERSION.as_bytes(),
        executor_version.as_bytes(),
        EXECUTABLE_ABILITY_PROGRAM_VERSION.as_bytes(),
        executor_id.as_bytes(),
        normalized_oracle.as_bytes(),
        type_line.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    hash_framed(&mut hasher, &relevant_type_role_mask.to_be_bytes());
    for clause in &covered_oracle_clauses {
        hash_framed(&mut hasher, &clause.face_index.to_be_bytes());
        hash_framed(&mut hasher, &clause.clause_index.to_be_bytes());
        hash_framed(&mut hasher, clause.normalized_clause_sha256.as_bytes());
    }
    Some(RuntimeSourceEvidence {
        ability_program_version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
        normalized_oracle_sha256,
        normalized_oracle_clause_sha256s,
        covered_oracle_clauses,
        type_line_sha256,
        relevant_type_role_mask,
        source_evidence_sha256: format!("{:x}", hasher.finalize()),
    })
}

fn selected_bounded_oracle_clause_source_evidence(
    card: &CompiledCard,
    address: crate::bounded_oracle_runtime::ClauseAddress,
    executor_id: &'static str,
    executor_version: &'static str,
) -> Option<RuntimeSourceEvidence> {
    let root = card
        .effects
        .bounded_oracle_source_roots
        .iter()
        .find(|root| root.face_index == address.face_index)?;
    if root.normalized_clauses.is_empty()
        || usize::from(address.clause_index) >= root.normalized_clauses.len()
    {
        return None;
    }
    let normalized_oracle = root.normalized_clauses.join(" ");
    let normalized_oracle_sha256 = sha256_hex(normalized_oracle.as_bytes());
    let normalized_oracle_clause_sha256s = root
        .normalized_clauses
        .iter()
        .map(|clause| sha256_hex(clause.as_bytes()))
        .collect::<Vec<_>>();
    let covered_oracle_clauses = vec![RuntimeOracleClauseEvidence {
        face_index: address.face_index,
        clause_index: address.clause_index,
        normalized_clause_sha256: normalized_oracle_clause_sha256s
            [usize::from(address.clause_index)]
        .clone(),
    }];
    let type_line_sha256 = sha256_hex(root.type_line.as_bytes());
    let relevant_type_role_mask = runtime_kind_role_mask_for_type_line(&root.type_line);
    let mut hasher = Sha256::new();
    for part in [
        RUNTIME_RECEIPT_SCHEMA_VERSION.as_bytes(),
        executor_version.as_bytes(),
        EXECUTABLE_ABILITY_PROGRAM_VERSION.as_bytes(),
        executor_id.as_bytes(),
        normalized_oracle.as_bytes(),
        root.type_line.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    hash_framed(&mut hasher, &relevant_type_role_mask.to_be_bytes());
    for clause in &covered_oracle_clauses {
        hash_framed(&mut hasher, &clause.face_index.to_be_bytes());
        hash_framed(&mut hasher, &clause.clause_index.to_be_bytes());
        hash_framed(&mut hasher, clause.normalized_clause_sha256.as_bytes());
    }
    Some(RuntimeSourceEvidence {
        ability_program_version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
        normalized_oracle_sha256,
        normalized_oracle_clause_sha256s,
        covered_oracle_clauses,
        type_line_sha256,
        relevant_type_role_mask,
        source_evidence_sha256: format!("{:x}", hasher.finalize()),
    })
}

fn runtime_kind_role_mask_for_type_line(type_line: &str) -> u32 {
    let types = compile_card_types(type_line);
    let mut roles = 0u32;
    if types.is_land {
        roles |= role::LAND;
    }
    if types.is_creature {
        roles |= role::CREATURE;
    }
    if types.is_artifact {
        roles |= role::ARTIFACT;
    }
    if types.is_enchantment {
        roles |= role::ENCHANTMENT;
    }
    if types.is_instant || types.is_sorcery {
        roles |= role::INSTANT_SORCERY;
    }
    roles
}

pub(crate) fn compile_bounded_oracle_runtime_receipts(
    card: &CompiledCard,
) -> Vec<BoundedOracleRuntimeReceipt> {
    let mut receipts = card
        .effects
        .bounded_oracle
        .iter()
        .filter_map(|clause| {
            let address = clause.address();
            let mechanic_programs = card
                .effects
                .mechanic_programs
                .iter()
                .filter(|program| program.primary_address() == address)
                .cloned()
                .collect::<Vec<_>>();
            let executor_id = bounded_oracle_executor_id(clause);
            let source_evidence = selected_bounded_oracle_clause_source_evidence(
                card,
                address,
                executor_id,
                BOUNDED_ORACLE_RUNTIME_EXECUTOR_VERSION,
            )?;
            let receipt = BoundedOracleRuntimeReceipt {
                binding: RuntimeExecutorBinding {
                    receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
                    executor_id,
                    executor_version: BOUNDED_ORACLE_RUNTIME_EXECUTOR_VERSION,
                },
                capabilities: bounded_oracle_capabilities(
                    clause,
                    &mechanic_programs,
                    &source_evidence,
                ),
                source_evidence,
                clause: clause.clone(),
                clause_semantic_sha256: clause.semantic_digest().to_owned(),
                consumer_version: BOUNDED_ORACLE_CONSUMER_VERSION,
                simulation_bridge_version: BOUNDED_ORACLE_SIMULATION_BRIDGE_VERSION,
                mechanic_contract_sha256: bounded_mechanic_contract_sha256(&mechanic_programs),
                mechanic_programs,
            };
            receipt.has_exact_contract().then_some(receipt)
        })
        .collect::<Vec<_>>();
    receipts.sort_by(|left, right| {
        left.clause
            .address()
            .cmp(&right.clause.address())
            .then_with(|| left.binding.executor_id.cmp(right.binding.executor_id))
    });
    receipts
}

fn bounded_oracle_executor_id(clause: &BoundedOracleClause) -> &'static str {
    match clause.timing() {
        BoundedTiming::CastingAdditionalCost => {
            "abstract-play.bounded-oracle.casting-additional-cost"
        }
        BoundedTiming::SpellResolution => "abstract-play.bounded-oracle.resolution",
        BoundedTiming::Activated => "abstract-play.bounded-oracle.activated",
        BoundedTiming::Triggered(_) => "abstract-play.bounded-oracle.triggered",
        BoundedTiming::TriggeredModalHeader { .. } => {
            "abstract-play.bounded-oracle.triggered-modal"
        }
        BoundedTiming::Static => "abstract-play.bounded-oracle.static",
        BoundedTiming::Replacement => "abstract-play.bounded-oracle.replacement",
        BoundedTiming::ModalHeader { .. } | BoundedTiming::ModalBranch { .. } => {
            "abstract-play.bounded-oracle.modal"
        }
        BoundedTiming::TypedStandaloneProgram => {
            "abstract-play.bounded-oracle.typed-standalone-unbound"
        }
        BoundedTiming::SpecialAction(_) => "abstract-play.bounded-oracle.special-action",
    }
}

fn bounded_mechanic_programs_have_exact_contract(
    clause: &BoundedOracleClause,
    programs: &[MechanicProgram],
) -> bool {
    let address = clause.address();
    programs
        .windows(2)
        .all(|pair| pair[0].mechanic() < pair[1].mechanic())
        && programs.iter().all(|program| {
            program.runtime_version() == MECHANIC_RUNTIME_VERSION
                && program.has_exact_contract()
                && program.primary_address() == address
                && program.executable_clauses().first() == Some(clause)
                && program
                    .executable_clauses()
                    .iter()
                    .all(|compiled| compiled.runtime_version() == BOUNDED_ORACLE_RUNTIME_VERSION)
                && mechanic_procedure_matches(program.mechanic(), program.procedure())
                && match program.marker_disposition() {
                    MarkerDisposition::Executable => true,
                    MarkerDisposition::StructurallyNonoperative {
                        owned_executable_clause,
                    } => {
                        *owned_executable_clause == address
                            && program
                                .executable_clauses()
                                .iter()
                                .any(|compiled| compiled.address() == address)
                    }
                }
        })
}

fn mechanic_procedure_matches(mechanic: PrintedMechanic, procedure: &MechanicProcedure) -> bool {
    matches!(
        (mechanic, procedure),
        (
            PrintedMechanic::AbilityWord,
            MechanicProcedure::AbilityWord(_)
        ) | (PrintedMechanic::Cycling, MechanicProcedure::Cycling(_))
            | (
                PrintedMechanic::Typecycling,
                MechanicProcedure::Typecycling(_)
            )
            | (PrintedMechanic::Enchant, MechanicProcedure::Enchant(_))
            | (PrintedMechanic::Food, MechanicProcedure::Food(_))
            | (PrintedMechanic::Prowess, MechanicProcedure::Prowess(_))
            | (PrintedMechanic::Channel, MechanicProcedure::Channel(_))
            | (PrintedMechanic::Treasure, MechanicProcedure::Treasure(_))
            | (PrintedMechanic::Scry, MechanicProcedure::Scry(_))
            | (PrintedMechanic::Landfall, MechanicProcedure::Landfall(_))
            | (PrintedMechanic::Double, MechanicProcedure::Double(_))
            | (PrintedMechanic::Paradigm, MechanicProcedure::Paradigm(_))
            | (PrintedMechanic::Transform, MechanicProcedure::Transform(_))
            | (PrintedMechanic::Surveil, MechanicProcedure::Surveil(_))
            | (PrintedMechanic::Crew, MechanicProcedure::Crew(_))
            | (PrintedMechanic::Ward, MechanicProcedure::Ward(_))
            | (
                PrintedMechanic::SplitSecond,
                MechanicProcedure::SplitSecond(_)
            )
            | (PrintedMechanic::Evoke, MechanicProcedure::Evoke(_))
            | (PrintedMechanic::Manifest, MechanicProcedure::Manifest(_))
            | (PrintedMechanic::Partner, MechanicProcedure::Partner(_))
            | (PrintedMechanic::Ferocious, MechanicProcedure::Ferocious(_))
            | (PrintedMechanic::Dash, MechanicProcedure::Dash(_))
            | (PrintedMechanic::Gift, MechanicProcedure::Gift(_))
            | (PrintedMechanic::Mobilize, MechanicProcedure::Mobilize(_))
    )
}

fn bounded_mechanic_contract_sha256(programs: &[MechanicProgram]) -> String {
    bounded_mechanic_contract_sha256_for_version(MECHANIC_RUNTIME_VERSION, programs)
}

fn bounded_mechanic_contract_sha256_for_version(
    mechanic_runtime_version: &str,
    programs: &[MechanicProgram],
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        RUNTIME_RECEIPT_SCHEMA_VERSION.as_bytes(),
        BOUNDED_ORACLE_RUNTIME_EXECUTOR_VERSION.as_bytes(),
        BOUNDED_ORACLE_CONSUMER_VERSION.as_bytes(),
        BOUNDED_ORACLE_SIMULATION_BRIDGE_VERSION.as_bytes(),
        mechanic_runtime_version.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    for program in programs {
        hash_framed(
            &mut hasher,
            &program.primary_address().face_index.to_be_bytes(),
        );
        hash_framed(
            &mut hasher,
            &program.primary_address().clause_index.to_be_bytes(),
        );
        hash_framed(&mut hasher, program.mechanic().printed_label().as_bytes());
        hash_framed(&mut hasher, format!("{program:?}").as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn bounded_oracle_capabilities(
    clause: &BoundedOracleClause,
    mechanic_programs: &[MechanicProgram],
    source_evidence: &RuntimeSourceEvidence,
) -> Vec<RuntimeCapability> {
    let source_scope = if source_evidence.normalized_oracle_clause_sha256s.len() == 1
        && source_evidence.covered_oracle_clauses.len() == 1
        && source_evidence.covered_oracle_clauses[0].normalized_clause_sha256
            == source_evidence.normalized_oracle_clause_sha256s[0]
    {
        RuntimeCapability::CompleteOracleRoot
    } else {
        RuntimeCapability::ExactOracleClauseSet
    };
    let mut capabilities = vec![
        source_scope,
        RuntimeCapability::ExactBoundedOracleProgram,
        RuntimeCapability::OrderedResolution,
    ];
    if !clause.costs().is_empty() {
        capabilities.push(RuntimeCapability::AtomicInitiationBoundary);
    }
    if matches!(clause.timing(), BoundedTiming::SpellResolution) {
        capabilities.push(RuntimeCapability::CounteredSpellResolutionBoundary);
    }
    if bounded_clause_retains_physical_identity(clause) {
        capabilities.push(RuntimeCapability::ExactPhysicalZoneObjectIdentity);
    }
    if bounded_clause_any(clause, |effect| {
        matches!(
            effect,
            BoundedEffect::TakeExtraTurn(effect) if effect.lose_at_end_step
        )
    }) {
        capabilities.push(RuntimeCapability::ExactDelayedDrawbackLifecycle);
    }
    if bounded_clause_any(clause, |effect| {
        matches!(
            effect,
            BoundedEffect::Restriction(BoundedRestriction::CannotBeBlocked { .. })
        )
    }) {
        capabilities.push(RuntimeCapability::ExactCannotBeBlockedRestriction);
    }
    if bounded_clause_has_exact_live_attachment_static(clause) {
        capabilities.push(RuntimeCapability::ExactAttachmentStaticEffect);
    }
    for program in mechanic_programs {
        let capability = match program.mechanic() {
            PrintedMechanic::AbilityWord => RuntimeCapability::ExactAbilityWordMarker,
            PrintedMechanic::Cycling => RuntimeCapability::ExactCyclingKeyword,
            PrintedMechanic::Typecycling => RuntimeCapability::ExactTypecyclingKeyword,
            PrintedMechanic::Enchant => RuntimeCapability::ExactEnchantKeyword,
            PrintedMechanic::Food => RuntimeCapability::ExactFoodKeyword,
            PrintedMechanic::Prowess => RuntimeCapability::ExactProwessKeyword,
            PrintedMechanic::Channel => RuntimeCapability::ExactChannelKeyword,
            PrintedMechanic::Treasure => RuntimeCapability::ExactTreasureKeyword,
            PrintedMechanic::Scry => RuntimeCapability::ExactScryKeyword,
            PrintedMechanic::Landfall => RuntimeCapability::ExactLandfallAbilityWord,
            PrintedMechanic::Double => RuntimeCapability::ExactDoubleKeyword,
            PrintedMechanic::Paradigm => RuntimeCapability::ExactParadigmKeyword,
            PrintedMechanic::Transform => RuntimeCapability::ExactTransformKeyword,
            PrintedMechanic::Surveil => RuntimeCapability::ExactSurveilKeyword,
            PrintedMechanic::Crew => RuntimeCapability::ExactCrewKeyword,
            PrintedMechanic::Ward => RuntimeCapability::ExactWardKeyword,
            PrintedMechanic::SplitSecond => RuntimeCapability::ExactSplitSecondKeyword,
            PrintedMechanic::Evoke => RuntimeCapability::ExactEvokeKeyword,
            PrintedMechanic::Manifest => RuntimeCapability::ExactManifestKeyword,
            PrintedMechanic::Partner => RuntimeCapability::ExactPartnerKeyword,
            PrintedMechanic::Ferocious => RuntimeCapability::ExactFerociousAbilityWord,
            PrintedMechanic::Dash => RuntimeCapability::ExactDashKeyword,
            PrintedMechanic::Gift => RuntimeCapability::ExactGiftKeyword,
            PrintedMechanic::Mobilize => RuntimeCapability::ExactMobilizeKeyword,
        };
        capabilities.push(capability);
    }
    capabilities
}

fn bounded_clause_has_exact_live_attachment_static(clause: &BoundedOracleClause) -> bool {
    !clause.effects().is_empty()
        && matches!(clause.timing(), BoundedTiming::Static)
        && clause.effects().iter().all(|effect| match effect {
            BoundedEffect::ModifyPowerToughness(
                crate::bounded_oracle_runtime::PowerToughnessChange {
                    objects: crate::bounded_oracle_runtime::ObjectRef::AttachmentTarget { .. },
                    operation:
                        crate::bounded_oracle_runtime::PowerToughnessOperation::Add
                        | crate::bounded_oracle_runtime::PowerToughnessOperation::Subtract
                        | crate::bounded_oracle_runtime::PowerToughnessOperation::AddPowerSubtractToughness
                        | crate::bounded_oracle_runtime::PowerToughnessOperation::SubtractPowerAddToughness,
                    power: crate::bounded_oracle_runtime::Amount::Constant(_),
                    toughness: crate::bounded_oracle_runtime::Amount::Constant(_),
                    duration: crate::bounded_oracle_runtime::Duration::WhileSourceOnBattlefield,
                },
            ) => true,
            BoundedEffect::Restriction(BoundedRestriction::DoesNotUntapDuring {
                object: crate::bounded_oracle_runtime::ObjectRef::AttachmentTarget { .. },
                step: crate::bounded_oracle_runtime::Step::UntapStep,
            }) => true,
            BoundedEffect::Restriction(
                BoundedRestriction::ActivatedAbilitiesCannotBeActivated {
                    object: crate::bounded_oracle_runtime::ObjectRef::AttachmentTarget { .. },
                    duration:
                        crate::bounded_oracle_runtime::Duration::WhileSourceOnBattlefield,
                }
                | BoundedRestriction::MustAttackEachCombatIfAble {
                    object: crate::bounded_oracle_runtime::ObjectRef::AttachmentTarget { .. },
                    duration:
                        crate::bounded_oracle_runtime::Duration::WhileSourceOnBattlefield,
                }
                | BoundedRestriction::CannotAttack {
                    object: crate::bounded_oracle_runtime::ObjectRef::AttachmentTarget { .. },
                    duration:
                        crate::bounded_oracle_runtime::Duration::WhileSourceOnBattlefield,
                }
                | BoundedRestriction::CannotBlock {
                    object: crate::bounded_oracle_runtime::ObjectRef::AttachmentTarget { .. },
                    duration:
                        crate::bounded_oracle_runtime::Duration::WhileSourceOnBattlefield,
                }
                | BoundedRestriction::CannotBeBlocked {
                    object: crate::bounded_oracle_runtime::ObjectRef::AttachmentTarget { .. },
                    duration:
                        crate::bounded_oracle_runtime::Duration::WhileSourceOnBattlefield,
                },
            ) => true,
            _ => false,
        })
}

fn bounded_effect_any(effect: &BoundedEffect, predicate: fn(&BoundedEffect) -> bool) -> bool {
    if predicate(effect) {
        return true;
    }
    match effect {
        BoundedEffect::Conditional {
            if_true, if_false, ..
        } => if_true
            .iter()
            .chain(if_false)
            .any(|effect| bounded_effect_any(effect, predicate)),
        BoundedEffect::GrantAbility { ability, .. } => ability
            .effects
            .iter()
            .any(|effect| bounded_effect_any(effect, predicate)),
        BoundedEffect::CreateToken(token) => {
            let BoundedTokenSpecification::Defined(definition) = &token.specification else {
                return false;
            };
            definition.abilities.iter().any(|ability| {
                ability
                    .effects
                    .iter()
                    .any(|effect| bounded_effect_any(effect, predicate))
            })
        }
        BoundedEffect::Replacement(effect) => {
            let crate::bounded_oracle_runtime::ReplacementEffect::ConditionalTokenSubstitution {
                ordinary,
                replacement,
                ..
            } = effect.as_ref()
            else {
                return false;
            };
            [ordinary, replacement.as_ref()].into_iter().any(|token| {
                let BoundedTokenSpecification::Defined(definition) = &token.specification else {
                    return false;
                };
                definition.abilities.iter().any(|ability| {
                    ability
                        .effects
                        .iter()
                        .any(|effect| bounded_effect_any(effect, predicate))
                })
            })
        }
        _ => false,
    }
}

fn bounded_clause_any(clause: &BoundedOracleClause, predicate: fn(&BoundedEffect) -> bool) -> bool {
    clause
        .effects()
        .iter()
        .any(|effect| bounded_effect_any(effect, predicate))
}

fn bounded_clause_retains_physical_identity(clause: &BoundedOracleClause) -> bool {
    bounded_clause_any(clause, |effect| {
        matches!(
            effect,
            BoundedEffect::MoveZone(_)
                | BoundedEffect::MoveSelected(_)
                | BoundedEffect::SetSelectedTapped { .. }
                | BoundedEffect::SearchLibrary(_)
                | BoundedEffect::Manifest { .. }
                | BoundedEffect::Copy(_)
                | BoundedEffect::Transform { .. }
        )
    })
}
pub(crate) fn compile_interaction_runtime_receipt(
    card: &CompiledCard,
) -> Option<InteractionRuntimeReceipt> {
    let program = compile_interaction_runtime_from_program(&card.type_line, &card.ability_program)?;
    let executor_id = interaction_executor_id(&program);
    let normalized_oracle = complete_normalized_program_root(card)?;
    let source_evidence = runtime_source_evidence(
        card,
        &normalized_oracle,
        executor_id,
        INTERACTION_RUNTIME_EXECUTOR_VERSION,
    );
    Some(InteractionRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id,
            executor_version: INTERACTION_RUNTIME_EXECUTOR_VERSION,
        },
        capabilities: vec![
            RuntimeCapability::CompleteOracleRoot,
            RuntimeCapability::OrderedResolution,
            RuntimeCapability::CounteredSpellResolutionBoundary,
        ],
        source_evidence,
        program,
    })
}

pub(crate) fn compile_tutor_runtime_receipt(card: &CompiledCard) -> Option<TutorRuntimeReceipt> {
    let program = compile_tutor_runtime_from_program(&card.type_line, &card.ability_program)?;
    let executor_id = tutor_executor_id(&program);
    if executor_id == "abstract-play.tutor.invalid" {
        return None;
    }
    let normalized_oracle = complete_normalized_program_root(card)?;
    let source_evidence = runtime_source_evidence(
        card,
        &normalized_oracle,
        executor_id,
        TUTOR_RUNTIME_EXECUTOR_VERSION,
    );
    let capabilities = tutor_runtime_capabilities(&program);
    Some(TutorRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id,
            executor_version: TUTOR_RUNTIME_EXECUTOR_VERSION,
        },
        capabilities,
        source_evidence,
        program,
    })
}

pub(crate) fn compile_restriction_protection_runtime_receipt(
    card: &CompiledCard,
) -> Option<RestrictionProtectionRuntimeReceipt> {
    let compiled =
        compile_restriction_protection_from_program(&card.type_line, &card.ability_program)?;
    let selected_clause_indices = match &compiled.ownership {
        RestrictionOracleOwnership::CompleteRoot { clause_count } => {
            (0..*clause_count).collect::<Vec<_>>()
        }
        RestrictionOracleOwnership::ExactClauseSet { clause_indices } => clause_indices.clone(),
    };
    let executor_id = restriction_protection_executor_id(&compiled.program);
    let source_evidence = selected_program_clause_source_evidence(
        card,
        &selected_clause_indices,
        executor_id,
        RESTRICTION_PROTECTION_EXECUTOR_VERSION,
    )?;
    let capabilities = restriction_protection_capabilities(&compiled);
    Some(RestrictionProtectionRuntimeReceipt {
        binding: RuntimeExecutorBinding {
            receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
            executor_id,
            executor_version: RESTRICTION_PROTECTION_EXECUTOR_VERSION,
        },
        capabilities,
        source_evidence,
        compiled,
    })
}

pub(crate) fn compile_reviewed_runtime_receipts(
    card: &CompiledCard,
) -> Vec<ReviewedRuntimeReceipt> {
    let mut programs = Vec::new();
    if let Some(compiled) = card.effects.alternative_cast.clone() {
        if !compiled.ownership.complete_root_required
            || compiled.ownership.clause_count == 0
            || compiled.ownership.clauses.len() != usize::from(compiled.ownership.clause_count)
            || compiled
                .ownership
                .clauses
                .iter()
                .enumerate()
                .any(|(expected, owned)| usize::from(owned.clause_index) != expected)
        {
            return Vec::new();
        }
        programs.push(ReviewedRuntimeProgram::AlternativeCast(compiled));
    }
    programs.extend(
        card.effects
            .characteristic_oracle
            .iter()
            .cloned()
            .map(ReviewedRuntimeProgram::CharacteristicOracle),
    );
    programs.extend(
        card.effects
            .continuous_triggers
            .iter()
            .cloned()
            .map(ReviewedRuntimeProgram::ContinuousTrigger),
    );
    if let Some(program) = card.effects.mana_network.clone() {
        programs.push(ReviewedRuntimeProgram::ManaNetwork(program));
    }
    if let Some(compiled) = card.effects.object_lifecycle.clone() {
        programs.push(ReviewedRuntimeProgram::ObjectLifecycle(compiled));
    }
    if let Some(compiled) = card.effects.utility_modal.clone() {
        programs.push(ReviewedRuntimeProgram::UtilityModal(compiled));
    }

    programs
        .into_iter()
        .filter_map(|program| {
            let (face_index, selected_clause_indices) = match &program {
                ReviewedRuntimeProgram::AlternativeCast(compiled) => {
                    (0, (0..compiled.ownership.clause_count).collect::<Vec<_>>())
                }
                ReviewedRuntimeProgram::CharacteristicOracle(compiled) => (
                    compiled.ownership.face_index,
                    vec![compiled.ownership.clause_index],
                ),
                ReviewedRuntimeProgram::ContinuousTrigger(compiled) => (
                    compiled.ownership.face_index,
                    match &compiled.ownership.oracle {
                        ContinuousOracleOwnership::CompleteFaceRoot { clause_count } => {
                            (0..*clause_count).collect::<Vec<_>>()
                        }
                        ContinuousOracleOwnership::ExactClauseSet { clause_indices } => {
                            clause_indices.clone()
                        }
                    },
                ),
                ReviewedRuntimeProgram::ManaNetwork(program) => (
                    0,
                    program
                        .covered_clauses()
                        .into_iter()
                        .map(|clause| clause.clause_index)
                        .collect(),
                ),
                ReviewedRuntimeProgram::ObjectLifecycle(compiled) => (
                    0,
                    match &compiled.ownership {
                        ObjectLifecycleOracleOwnership::CompleteRoot { clause_count } => {
                            (0..*clause_count).collect::<Vec<_>>()
                        }
                        ObjectLifecycleOracleOwnership::ExactClauseSet { clause_indices } => {
                            clause_indices.clone()
                        }
                    },
                ),
                ReviewedRuntimeProgram::UtilityModal(compiled) => {
                    (0, compiled.ownership.owned_clause_indices())
                }
            };
            let executor_id = reviewed_runtime_executor_id(&program);
            let executor_version = reviewed_runtime_executor_version(&program);
            let source_evidence = selected_program_face_clause_source_evidence(
                card,
                face_index,
                &selected_clause_indices,
                executor_id,
                executor_version,
            )?;
            let capabilities = reviewed_runtime_capabilities(&program);
            Some(ReviewedRuntimeReceipt {
                binding: RuntimeExecutorBinding {
                    receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
                    executor_id,
                    executor_version,
                },
                capabilities,
                source_evidence,
                program,
            })
        })
        .collect()
}

fn runtime_source_evidence(
    card: &CompiledCard,
    normalized_oracle: &str,
    executor_id: &'static str,
    executor_version: &'static str,
) -> RuntimeSourceEvidence {
    let normalized_oracle =
        normalize_oracle_clause_for_receipt(normalized_oracle, &card.name, &card.type_line);
    let normalized_oracle_sha256 = sha256_hex(normalized_oracle.as_bytes());
    let normalized_oracle_clause_sha256s = normalized_oracle
        .lines()
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .map(|clause| sha256_hex(clause.as_bytes()))
        .collect::<Vec<_>>();
    let covered_oracle_clauses = normalized_oracle_clause_sha256s
        .iter()
        .enumerate()
        .map(
            |(clause_index, normalized_clause_sha256)| RuntimeOracleClauseEvidence {
                face_index: 0,
                clause_index: u16::try_from(clause_index).unwrap_or(u16::MAX),
                normalized_clause_sha256: normalized_clause_sha256.clone(),
            },
        )
        .collect::<Vec<_>>();
    let type_line_sha256 = sha256_hex(card.type_line.as_bytes());
    let relevant_type_role_mask = card.roles & RUNTIME_KIND_ROLE_MASK;

    let mut hasher = Sha256::new();
    for part in [
        RUNTIME_RECEIPT_SCHEMA_VERSION.as_bytes(),
        executor_version.as_bytes(),
        EXECUTABLE_ABILITY_PROGRAM_VERSION.as_bytes(),
        executor_id.as_bytes(),
        normalized_oracle.as_bytes(),
        card.type_line.as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    hash_framed(&mut hasher, &relevant_type_role_mask.to_be_bytes());
    for clause in &covered_oracle_clauses {
        hash_framed(&mut hasher, &clause.face_index.to_be_bytes());
        hash_framed(&mut hasher, &clause.clause_index.to_be_bytes());
        hash_framed(&mut hasher, clause.normalized_clause_sha256.as_bytes());
    }

    RuntimeSourceEvidence {
        ability_program_version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
        normalized_oracle_sha256,
        normalized_oracle_clause_sha256s,
        covered_oracle_clauses,
        type_line_sha256,
        relevant_type_role_mask,
        source_evidence_sha256: format!("{:x}", hasher.finalize()),
    }
}

fn hash_framed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn sha256_hex(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn fixed_mana_total(output: FixedManaProfile) -> Option<u16> {
    [
        output.white,
        output.blue,
        output.black,
        output.red,
        output.green,
        output.colorless,
    ]
    .into_iter()
    .try_fold(0u16, u16::checked_add)
}

fn type_line_has_exact_type_envelope(
    type_line: &str,
    expected: &str,
    allow_legendary: bool,
) -> bool {
    let card_type_segment = type_line
        .split_once('\u{2014}')
        .map(|(types, _)| types)
        .or_else(|| type_line.split_once(" - ").map(|(types, _)| types))
        .unwrap_or(type_line);
    let mut type_words = card_type_segment.split_whitespace();
    match (type_words.next(), type_words.next(), type_words.next()) {
        (Some(card_type), None, None) => card_type.eq_ignore_ascii_case(expected),
        (Some(supertype), Some(card_type), None) if allow_legendary => {
            supertype.eq_ignore_ascii_case("legendary") && card_type.eq_ignore_ascii_case(expected)
        }
        _ => false,
    }
}

fn fixed_mana_is_exact_hand_output(output: FixedManaProfile) -> bool {
    output
        == (FixedManaProfile {
            red: 1,
            ..FixedManaProfile::default()
        })
        || output
            == (FixedManaProfile {
                green: 1,
                ..FixedManaProfile::default()
            })
}

fn exact_noncommander_creature_filter(filter: &ProgramObjectFilter) -> bool {
    filter
        == &(ProgramObjectFilter {
            card_type: Some(ProgramCardType::Creature),
            controller: Some(ControllerRelation::You),
            ..ProgramObjectFilter::default()
        })
}

pub(crate) fn exact_any_card_tutor(tutor: &ProgramTutorEffect, shuffle_after: bool) -> bool {
    tutor.from == ProgramZone::Library
        && tutor.destination == ProgramZone::Hand
        && tutor.shuffle_after == shuffle_after
        && tutor.filter
            == TutorFilter::AnyOf(vec![ProgramObjectFilter {
                card_type: Some(ProgramCardType::Card),
                ..ProgramObjectFilter::default()
            }])
}

fn exact_atomic_any_card_search(search: &AtomicLibrarySearch, quantity: u16, reveal: bool) -> bool {
    search.player == ControllerRelation::You
        && search.from == ProgramZone::Library
        && search.minimum == quantity
        && search.maximum == quantity
        && search.reveal == reveal
        && search.filter
            == TutorFilter::AnyOf(vec![ProgramObjectFilter {
                card_type: Some(ProgramCardType::Card),
                ..ProgramObjectFilter::default()
            }])
}

fn exact_bargain_cost(cost: &AtomicBargainCost) -> bool {
    cost.player == ControllerRelation::You
        && cost.timing == AtomicAdditionalCostTiming::AsThisSpellIsCast
        && cost.optional
        && cost.from == ProgramZone::Battlefield
        && cost.count == 1
        && cost.eligible_kinds
            == [
                BargainSacrificeKind::Artifact,
                BargainSacrificeKind::Enchantment,
                BargainSacrificeKind::Token,
            ]
        && cost.commander_eligibility == CommanderEligibility::Include
}

fn exact_bargain_search_cast_or_hand(effect: &BargainSearchCastOrHandEffect) -> bool {
    exact_atomic_any_card_search(&effect.search, 1, false)
        && effect.searched_card == AtomicTrackedObject::OnlyCardFoundByThisSearch
        && effect.initial_destination == ProgramZone::Exile
        && effect.face_down
        && effect.shuffle.player == ControllerRelation::You
        && effect.shuffle.timing
            == AtomicShuffleTiming::AfterInitialSearchMovementBeforeConditionalCast
        && effect.conditional_cast.condition == AtomicCastPermissionCondition::ThisSpellWasBargained
        && effect.conditional_cast.card == AtomicTrackedObject::OnlyCardFoundByThisSearch
        && effect.conditional_cast.from == ProgramZone::Exile
        && effect.conditional_cast.optional
        && effect.conditional_cast.mana_value.subject == AtomicManaValueSubject::SpellAsCast
        && effect.conditional_cast.mana_value.maximum == 4
        && effect.conditional_cast.cost_waiver == AtomicCastCostWaiver::ManaCostOnly
        && effect.if_not_cast.condition == AtomicMovementCondition::NotCastByThisEffect
        && effect.if_not_cast.object == AtomicTrackedObject::OnlyCardFoundByThisSearch
        && effect.if_not_cast.from == ProgramZone::Exile
        && effect.if_not_cast.to == ProgramZone::Hand
        && effect.if_not_cast.recipient == ControllerRelation::You
}

fn exact_opponent_choice_search_split(effect: &OpponentChoiceSearchSplitEffect) -> bool {
    exact_atomic_any_card_search(&effect.search, 3, true)
        && effect.chooser == AtomicSearchChooser::TargetOpponent
        && effect.chosen_count == 1
        && effect.chosen_destination == ProgramZone::Hand
        && effect.chosen_recipient == ControllerRelation::You
        && effect.remainder_count == 2
        && effect.remainder_destination == ProgramZone::Graveyard
        && effect.remainder_recipient == ControllerRelation::You
        && effect.shuffle.player == ControllerRelation::You
        && effect.shuffle.timing == AtomicShuffleTiming::AfterOpponentChoiceMovements
}
