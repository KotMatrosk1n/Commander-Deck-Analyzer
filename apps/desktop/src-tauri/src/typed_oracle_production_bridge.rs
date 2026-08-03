//! Transactional production bridge primitives for retained typed Oracle programs.
//!
//! Semantic identity in this module is derived only from exact Oracle content,
//! its normalized executable form, and the versioned compiler/runtime contract.
//! Occurrence coordinates and snapshot observations are retained separately so
//! relocating unchanged Oracle text cannot invalidate its executable program.
//!
//! The bridge never treats compilation as execution. Every entry point checks
//! independent source evidence, validates dynamic inputs, executes against a
//! cloned state projection, verifies a complete typed receipt, and commits the
//! clone only after the entire transaction succeeds.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

use crate::ability_clause_bridge::{
    ABILITY_CLAUSE_BRIDGE_COMPILER_VERSION, ABILITY_CLAUSE_BRIDGE_RUNTIME_VERSION,
    AbilityClauseActivationWindow, AbilityClauseBridgeProgram, AbilityClauseCardType,
    AbilityClauseControllerRelation, AbilityClauseObjectFilter, AbilityClauseSpecificCardType,
    AbilityClauseTimingEnvelope, AbilityClauseTriggerEventKind,
};
use crate::ability_program::{
    AbilityCost, AbilityEffect, AbilityPrecondition, ExecutableAbility, Zone as AbilityZone,
};
use crate::damage_clause_compiler::{
    CompiledDamageClause, DAMAGE_CLAUSE_COMPILER_VERSION, DamageAmountTemplate,
    DamageClauseBindingError, DamageClauseBindings, DamageClauseEnvelope, DamageRecipientTemplate,
};
use crate::damage_transaction_runtime::{
    DAMAGE_TRANSACTION_RUNTIME_VERSION, DamageRuntimeState, DamageSourceIdentity,
    DamageTransactionError, DamageTransactionReceipt, execute_damage_transaction,
};
use crate::object_state_clause_runtime::{
    ExternalReplacementOutcome, OBJECT_STATE_CLAUSE_COMPILER_VERSION,
    OBJECT_STATE_CLAUSE_RUNTIME_VERSION, ObjectRef, ObjectStateClauseKind,
    ObjectStateClauseProgram, ObjectStateClauseRuntime, ObjectStateRuntimeError, ObjectZone,
    PendingZoneChange, ReplacementCandidateEvidence, ReplacementEffectIdentity,
    ReplacementOrderEvidence, ReplacementPriority, ReplacementStepResolution, UntapChoice,
    UntapStepResolution, ZoneChangeCommit,
};

pub(crate) const TYPED_ORACLE_PRODUCTION_BRIDGE_VERSION: &str =
    "typed-oracle-production-bridge-0.1";
pub(crate) const TYPED_ORACLE_RECEIPT_VERSION: &str = "typed-oracle-production-receipt-0.1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OracleSemanticEvidence {
    pub exact_oracle: String,
    pub normalized_oracle: String,
    pub semantic_digest: String,
}

impl OracleSemanticEvidence {
    pub(crate) fn new(
        exact_oracle: impl Into<String>,
        normalized_oracle: impl Into<String>,
        semantic_digest: impl Into<String>,
    ) -> Result<Self, TypedOracleBridgeError> {
        let evidence = Self {
            exact_oracle: exact_oracle.into(),
            normalized_oracle: normalized_oracle.into(),
            semantic_digest: semantic_digest.into(),
        };
        evidence.validate_shape()?;
        Ok(evidence)
    }

    pub(crate) fn for_object_state(program: &ObjectStateClauseProgram) -> Self {
        Self {
            exact_oracle: program.exact_source().to_owned(),
            normalized_oracle: program.normalized_source().to_owned(),
            semantic_digest: program.semantic_digest().to_owned(),
        }
    }

    pub(crate) fn for_damage(program: &CompiledDamageClause) -> Self {
        Self {
            exact_oracle: program.source_clause().to_owned(),
            normalized_oracle: program.normalized_clause().to_owned(),
            semantic_digest: program.semantic_digest().to_owned(),
        }
    }

    pub(crate) fn for_ability(program: &AbilityClauseBridgeProgram) -> Self {
        Self {
            exact_oracle: program.exact_source().to_owned(),
            normalized_oracle: program.normalized_source().to_owned(),
            semantic_digest: program.semantic_digest().to_owned(),
        }
    }

    fn validate_shape(&self) -> Result<(), TypedOracleBridgeError> {
        if self.exact_oracle.is_empty() || self.exact_oracle.trim() != self.exact_oracle {
            return Err(TypedOracleBridgeError::InvalidSemanticEvidence(
                "exact Oracle source must be nonempty and already trimmed",
            ));
        }
        if self.normalized_oracle.is_empty()
            || self.normalized_oracle.trim() != self.normalized_oracle
        {
            return Err(TypedOracleBridgeError::InvalidSemanticEvidence(
                "normalized Oracle source must be nonempty and already trimmed",
            ));
        }
        if !is_sha256_hex(&self.semantic_digest) {
            return Err(TypedOracleBridgeError::InvalidSemanticEvidence(
                "semantic digest must be lowercase SHA-256",
            ));
        }
        Ok(())
    }
}

/// Dynamic source occurrence information. None of these fields participates in
/// `OracleSemanticEvidence` or a compiled program's semantic digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct OracleOccurrenceProvenance {
    pub source_object: u64,
    pub source_incarnation: u64,
    pub face_index: u16,
    pub clause_index: u16,
    /// Optional local observation metadata. It is copied into receipts for
    /// audit only and must never select or invalidate executable semantics.
    pub snapshot_observation: Option<String>,
}

impl OracleOccurrenceProvenance {
    fn validate(&self) -> Result<(), TypedOracleBridgeError> {
        if self
            .snapshot_observation
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(TypedOracleBridgeError::InvalidOccurrenceProvenance(
                "snapshot observation must be omitted or nonempty",
            ));
        }
        Ok(())
    }

    fn object_ref(&self) -> ObjectRef {
        ObjectRef {
            object_id: self.source_object,
            incarnation_id: self.source_incarnation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypedOracleExecutionBinding {
    pub semantic: OracleSemanticEvidence,
    pub occurrence: OracleOccurrenceProvenance,
}

impl TypedOracleExecutionBinding {
    pub(crate) fn new(
        semantic: OracleSemanticEvidence,
        occurrence: OracleOccurrenceProvenance,
    ) -> Result<Self, TypedOracleBridgeError> {
        semantic.validate_shape()?;
        occurrence.validate()?;
        Ok(Self {
            semantic,
            occurrence,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypedOracleBridgeCapability {
    ExactObjectStateClause {
        kind: ObjectStateClauseKind,
    },
    TransactionalUntapStep,
    TransactionalZoneReplacement,
    ExactDamageClause {
        envelope: DamageClauseEnvelope,
        amount: DamageAmountTemplate,
        recipient: DamageRecipientTemplate,
    },
    TransactionalDamageReplacementAndPrevention,
    ExactAbilityClause {
        timing: AbilityClauseTimingEnvelope,
        preconditions: u16,
        costs: u16,
        effects: u16,
    },
    TransactionalAbilityCostsAndResolution,
    CompleteExternalInputConsumption,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ObjectStateTransactionInput {
    UntapStep {
        active_player: u8,
        battlefield_evidence_complete: bool,
        choices: BTreeMap<ObjectRef, UntapChoice>,
    },
    ZoneChange {
        source: ObjectRef,
        destination: ObjectZone,
        battlefield_controller: Option<u8>,
        replacement_steps: Vec<ObjectStateReplacementStepInput>,
        new_incarnation: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectStateReplacementStepInput {
    pub chooser: u8,
    pub external_applicable_complete: bool,
    pub external_applicable: Vec<ExternalReplacementCandidateInput>,
    pub chosen: Option<ObjectStateReplacementChoiceInput>,
    pub external_outcome: Option<ObjectStateExternalOutcomeInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalReplacementCandidateInput {
    pub identity: String,
    pub priority: ReplacementPriority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ObjectStateReplacementChoiceInput {
    BoundProgram,
    Intrinsic(ReplacementEffectIdentity),
    External(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ObjectStateExternalOutcomeInput {
    pub destination: Option<Option<ObjectZone>>,
    pub battlefield_controller: Option<Option<u8>>,
    pub enters_tapped: Option<bool>,
    pub intrinsic_bindings_no_longer_applicable: BTreeSet<u64>,
    pub disable_bound_program: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ObjectStateTransactionOutcome {
    Untap(UntapStepResolution),
    ZoneChange {
        replacement_steps: Vec<ReplacementStepResolution>,
        commit: ZoneChangeCommit,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectStateProductionReceipt {
    pub receipt_version: &'static str,
    pub bridge_version: &'static str,
    pub compiler_version: &'static str,
    pub runtime_version: &'static str,
    pub semantic: OracleSemanticEvidence,
    pub occurrence: OracleOccurrenceProvenance,
    pub capabilities: Vec<TypedOracleBridgeCapability>,
    pub state_before_sha256: String,
    pub state_after_sha256: String,
    pub transaction_input: ObjectStateTransactionInput,
    pub outcome: ObjectStateTransactionOutcome,
}

impl ObjectStateProductionReceipt {
    pub(crate) fn has_exact_contract(&self, program: &ObjectStateClauseProgram) -> bool {
        self.receipt_version == TYPED_ORACLE_RECEIPT_VERSION
            && self.bridge_version == TYPED_ORACLE_PRODUCTION_BRIDGE_VERSION
            && self.compiler_version == OBJECT_STATE_CLAUSE_COMPILER_VERSION
            && self.runtime_version == OBJECT_STATE_CLAUSE_RUNTIME_VERSION
            && self.semantic == OracleSemanticEvidence::for_object_state(program)
            && self.capabilities == object_state_capabilities(program.kind())
            && is_sha256_hex(&self.state_before_sha256)
            && is_sha256_hex(&self.state_after_sha256)
            && object_state_receipt_matches_occurrence(
                program.kind(),
                &self.occurrence,
                &self.transaction_input,
                &self.outcome,
            )
    }
}

pub(crate) fn execute_object_state_clause_transaction(
    state: &mut ObjectStateClauseRuntime,
    program: &ObjectStateClauseProgram,
    binding: &TypedOracleExecutionBinding,
    input: ObjectStateTransactionInput,
) -> Result<ObjectStateProductionReceipt, TypedOracleBridgeError> {
    validate_object_state_binding(program, binding)?;
    let transaction_input = input.clone();
    let state_before_sha256 = debug_state_digest("object-state/before", state);
    let mut staged = state.clone();
    let bound_source = binding.occurrence.object_ref();
    let binding_id = staged
        .install_program(bound_source, program.clone())
        .map_err(TypedOracleBridgeError::ObjectState)?;

    let outcome = match (program.kind(), input) {
        (
            ObjectStateClauseKind::OptionalUntapDuringYourUntapStep,
            ObjectStateTransactionInput::UntapStep {
                active_player,
                battlefield_evidence_complete,
                choices,
            },
        ) => {
            let pending = staged
                .begin_untap_step(active_player, battlefield_evidence_complete)
                .map_err(TypedOracleBridgeError::ObjectState)?;
            let expected = pending
                .choices
                .iter()
                .map(|choice| choice.object)
                .collect::<BTreeSet<_>>();
            let actual = choices.keys().copied().collect::<BTreeSet<_>>();
            if expected != actual {
                return Err(TypedOracleBridgeError::UntapChoiceSetMismatch {
                    expected: expected.into_iter().collect(),
                    actual: actual.into_iter().collect(),
                });
            }
            for (object, choice) in choices {
                staged
                    .choose_untap(pending.id, active_player, object, choice)
                    .map_err(TypedOracleBridgeError::ObjectState)?;
            }
            let resolution = staged
                .resolve_untap_step(pending.id)
                .map_err(TypedOracleBridgeError::ObjectState)?;
            staged.remove_binding(binding_id);
            ObjectStateTransactionOutcome::Untap(resolution)
        }
        (
            ObjectStateClauseKind::SelfGraveyardMoveBecomesExile,
            ObjectStateTransactionInput::ZoneChange {
                source,
                destination,
                battlefield_controller,
                replacement_steps,
                new_incarnation,
            },
        ) => {
            if source != bound_source {
                return Err(TypedOracleBridgeError::OccurrenceSourceMismatch);
            }
            if destination != ObjectZone::Graveyard {
                return Err(TypedOracleBridgeError::ObjectStateInputMismatch(
                    "graveyard replacement must begin with a would-be graveyard move",
                ));
            }
            execute_bound_zone_change(
                &mut staged,
                binding_id,
                source,
                destination,
                battlefield_controller,
                replacement_steps,
                new_incarnation,
            )?
        }
        (
            ObjectStateClauseKind::EntersBattlefieldTapped,
            ObjectStateTransactionInput::ZoneChange {
                source,
                destination,
                battlefield_controller,
                replacement_steps,
                new_incarnation,
            },
        ) => {
            if source != bound_source {
                return Err(TypedOracleBridgeError::OccurrenceSourceMismatch);
            }
            if destination != ObjectZone::Battlefield {
                return Err(TypedOracleBridgeError::ObjectStateInputMismatch(
                    "tapped-entry replacement must begin with a battlefield move",
                ));
            }
            execute_bound_zone_change(
                &mut staged,
                binding_id,
                source,
                destination,
                battlefield_controller,
                replacement_steps,
                new_incarnation,
            )?
        }
        _ => {
            return Err(TypedOracleBridgeError::ObjectStateInputMismatch(
                "transaction input does not match the compiled object-state kind",
            ));
        }
    };

    let state_after_sha256 = debug_state_digest("object-state/after", &staged);
    let receipt = ObjectStateProductionReceipt {
        receipt_version: TYPED_ORACLE_RECEIPT_VERSION,
        bridge_version: TYPED_ORACLE_PRODUCTION_BRIDGE_VERSION,
        compiler_version: OBJECT_STATE_CLAUSE_COMPILER_VERSION,
        runtime_version: OBJECT_STATE_CLAUSE_RUNTIME_VERSION,
        semantic: binding.semantic.clone(),
        occurrence: binding.occurrence.clone(),
        capabilities: object_state_capabilities(program.kind()),
        state_before_sha256,
        state_after_sha256,
        transaction_input,
        outcome,
    };
    if !receipt.has_exact_contract(program) {
        return Err(TypedOracleBridgeError::InvalidGeneratedReceipt(
            "object-state receipt failed its exact contract",
        ));
    }
    *state = staged;
    Ok(receipt)
}

fn object_state_receipt_matches_occurrence(
    kind: ObjectStateClauseKind,
    occurrence: &OracleOccurrenceProvenance,
    input: &ObjectStateTransactionInput,
    outcome: &ObjectStateTransactionOutcome,
) -> bool {
    if occurrence.validate().is_err() {
        return false;
    }
    let source = occurrence.object_ref();
    match (kind, input, outcome) {
        (
            ObjectStateClauseKind::OptionalUntapDuringYourUntapStep,
            ObjectStateTransactionInput::UntapStep { .. },
            ObjectStateTransactionOutcome::Untap(_),
        ) => true,
        (
            ObjectStateClauseKind::SelfGraveyardMoveBecomesExile,
            ObjectStateTransactionInput::ZoneChange {
                source: input_source,
                destination: ObjectZone::Graveyard,
                ..
            },
            ObjectStateTransactionOutcome::ZoneChange { commit, .. },
        )
        | (
            ObjectStateClauseKind::EntersBattlefieldTapped,
            ObjectStateTransactionInput::ZoneChange {
                source: input_source,
                destination: ObjectZone::Battlefield,
                ..
            },
            ObjectStateTransactionOutcome::ZoneChange { commit, .. },
        ) => {
            input_source == &source
                && match commit {
                    ZoneChangeCommit::Moved { old_object, .. } => old_object == &source,
                    ZoneChangeCommit::ReplacedWithNoZoneChange { object, .. } => object == &source,
                }
        }
        _ => false,
    }
}

fn execute_bound_zone_change(
    staged: &mut ObjectStateClauseRuntime,
    binding_id: u64,
    source: ObjectRef,
    destination: ObjectZone,
    battlefield_controller: Option<u8>,
    replacement_steps: Vec<ObjectStateReplacementStepInput>,
    new_incarnation: Option<u64>,
) -> Result<ObjectStateTransactionOutcome, TypedOracleBridgeError> {
    if replacement_steps.is_empty() {
        return Err(TypedOracleBridgeError::IncompleteReplacementSequence);
    }
    let mut pending = staged
        .begin_zone_change(source, destination, battlefield_controller)
        .map_err(TypedOracleBridgeError::ObjectState)?;
    let mut resolutions = Vec::with_capacity(replacement_steps.len());
    for (index, step) in replacement_steps.into_iter().enumerate() {
        if pending.replacement_window_complete() {
            return Err(TypedOracleBridgeError::TrailingReplacementStep { index });
        }
        resolutions.push(apply_bound_replacement_step(
            staged,
            &mut pending,
            binding_id,
            step,
        )?);
    }
    if !pending.replacement_window_complete() {
        return Err(TypedOracleBridgeError::IncompleteReplacementSequence);
    }
    let commit = staged
        .commit_zone_change(pending, new_incarnation)
        .map_err(TypedOracleBridgeError::ObjectState)?;
    staged.remove_binding(binding_id);
    Ok(ObjectStateTransactionOutcome::ZoneChange {
        replacement_steps: resolutions,
        commit,
    })
}

fn apply_bound_replacement_step(
    staged: &ObjectStateClauseRuntime,
    pending: &mut PendingZoneChange,
    binding_id: u64,
    step: ObjectStateReplacementStepInput,
) -> Result<ReplacementStepResolution, TypedOracleBridgeError> {
    if !step.external_applicable_complete {
        return Err(TypedOracleBridgeError::IncompleteExternalReplacementEvidence);
    }
    let mut applicable = staged
        .intrinsic_replacement_candidates(pending)
        .map_err(TypedOracleBridgeError::ObjectState)?;
    let mut external_ids = BTreeSet::new();
    for candidate in step.external_applicable {
        let identity = candidate.identity.trim().to_owned();
        if identity.is_empty() {
            return Err(TypedOracleBridgeError::InvalidExternalReplacementIdentity);
        }
        if !external_ids.insert(identity.clone()) {
            return Err(TypedOracleBridgeError::DuplicateExternalReplacementIdentity(identity));
        }
        applicable.push(ReplacementCandidateEvidence {
            identity: ReplacementEffectIdentity::External(identity),
            priority: candidate.priority,
        });
    }
    applicable.sort_by(|left, right| left.identity.cmp(&right.identity));

    let chosen = match step.chosen {
        Some(ObjectStateReplacementChoiceInput::BoundProgram) => {
            let matching = applicable
                .iter()
                .filter_map(|candidate| match &candidate.identity {
                    ReplacementEffectIdentity::Intrinsic {
                        binding_id: actual, ..
                    } if *actual == binding_id => Some(candidate.identity.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [identity] = matching.as_slice() else {
                return Err(TypedOracleBridgeError::BoundReplacementNotApplicable);
            };
            Some(identity.clone())
        }
        Some(ObjectStateReplacementChoiceInput::Intrinsic(identity)) => Some(identity),
        Some(ObjectStateReplacementChoiceInput::External(identity)) => {
            let identity = identity.trim().to_owned();
            if identity.is_empty() {
                return Err(TypedOracleBridgeError::InvalidExternalReplacementIdentity);
            }
            Some(ReplacementEffectIdentity::External(identity))
        }
        None => None,
    };
    let external_outcome = step.external_outcome.map(|outcome| {
        let mut disabled = outcome.intrinsic_bindings_no_longer_applicable;
        if outcome.disable_bound_program {
            disabled.insert(binding_id);
        }
        ExternalReplacementOutcome {
            destination: outcome.destination,
            battlefield_controller: outcome.battlefield_controller,
            enters_tapped: outcome.enters_tapped,
            intrinsic_bindings_no_longer_applicable: disabled,
        }
    });
    staged
        .apply_replacement_step(
            pending,
            ReplacementOrderEvidence {
                chooser: step.chooser,
                applicable_effects_complete: true,
                applicable,
                chosen,
            },
            external_outcome,
        )
        .map_err(TypedOracleBridgeError::ObjectState)
}

fn validate_object_state_binding(
    program: &ObjectStateClauseProgram,
    binding: &TypedOracleExecutionBinding,
) -> Result<(), TypedOracleBridgeError> {
    validate_binding(
        binding,
        program.exact_source(),
        program.normalized_source(),
        program.semantic_digest(),
    )
}

fn object_state_capabilities(kind: ObjectStateClauseKind) -> Vec<TypedOracleBridgeCapability> {
    let transaction = match kind {
        ObjectStateClauseKind::OptionalUntapDuringYourUntapStep => {
            TypedOracleBridgeCapability::TransactionalUntapStep
        }
        ObjectStateClauseKind::SelfGraveyardMoveBecomesExile
        | ObjectStateClauseKind::EntersBattlefieldTapped => {
            TypedOracleBridgeCapability::TransactionalZoneReplacement
        }
    };
    vec![
        TypedOracleBridgeCapability::ExactObjectStateClause { kind },
        transaction,
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DamageProductionReceipt {
    pub receipt_version: &'static str,
    pub bridge_version: &'static str,
    pub compiler_version: &'static str,
    pub runtime_version: &'static str,
    pub semantic: OracleSemanticEvidence,
    pub occurrence: OracleOccurrenceProvenance,
    pub capabilities: Vec<TypedOracleBridgeCapability>,
    pub state_before_sha256: String,
    pub state_after_sha256: String,
    pub transaction: DamageTransactionReceipt,
}

/// Damage runtime state plus the physical incarnation evidence that the
/// standalone damage kernel deliberately does not model. Stack objects may
/// appear in `object_incarnations` even when they are not damage recipients.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DamageProductionState {
    pub damage: DamageRuntimeState,
    pub object_incarnations: BTreeMap<u64, u64>,
}

impl DamageProductionReceipt {
    pub(crate) fn has_exact_contract(&self, program: &CompiledDamageClause) -> bool {
        self.receipt_version == TYPED_ORACLE_RECEIPT_VERSION
            && self.bridge_version == TYPED_ORACLE_PRODUCTION_BRIDGE_VERSION
            && self.compiler_version == DAMAGE_CLAUSE_COMPILER_VERSION
            && self.runtime_version == DAMAGE_TRANSACTION_RUNTIME_VERSION
            && self.semantic == OracleSemanticEvidence::for_damage(program)
            && self.transaction.runtime_version == DAMAGE_TRANSACTION_RUNTIME_VERSION
            && self.transaction.semantic.semantic_digest() == program.semantic_digest()
            && matches!(
                self.transaction.source.identity,
                DamageSourceIdentity::Object(object)
                    if object == self.occurrence.source_object
            )
            && self.occurrence.validate().is_ok()
            && self.capabilities == damage_capabilities(program)
            && is_sha256_hex(&self.state_before_sha256)
            && is_sha256_hex(&self.state_after_sha256)
    }
}

pub(crate) fn execute_damage_clause_transaction(
    state: &mut DamageProductionState,
    program: &CompiledDamageClause,
    binding: &TypedOracleExecutionBinding,
    bindings: DamageClauseBindings,
) -> Result<DamageProductionReceipt, TypedOracleBridgeError> {
    validate_binding(
        binding,
        program.source_clause(),
        program.normalized_clause(),
        program.semantic_digest(),
    )?;
    match bindings.source.identity {
        DamageSourceIdentity::Object(object)
            if object == binding.occurrence.source_object
                && state.object_incarnations.get(&object)
                    == Some(&binding.occurrence.source_incarnation) => {}
        _ => return Err(TypedOracleBridgeError::OccurrenceSourceMismatch),
    }

    let request = program
        .bind(bindings)
        .map_err(TypedOracleBridgeError::DamageBinding)?;
    if request.semantic.semantic_digest() != binding.semantic.semantic_digest
        || request.semantic.exact_oracle() != binding.semantic.normalized_oracle
        || request.semantic.normalized_oracle()
            != binding.semantic.normalized_oracle.to_ascii_lowercase()
    {
        return Err(TypedOracleBridgeError::DamageRequestSemanticMismatch);
    }

    let state_before_sha256 = debug_state_digest("damage/before", state);
    let mut staged = state.clone();
    let transaction = execute_damage_transaction(&mut staged.damage, &request)
        .map_err(TypedOracleBridgeError::DamageTransaction)?;
    if transaction.semantic.semantic_digest() != program.semantic_digest() {
        return Err(TypedOracleBridgeError::DamageRequestSemanticMismatch);
    }
    let state_after_sha256 = debug_state_digest("damage/after", &staged);
    let receipt = DamageProductionReceipt {
        receipt_version: TYPED_ORACLE_RECEIPT_VERSION,
        bridge_version: TYPED_ORACLE_PRODUCTION_BRIDGE_VERSION,
        compiler_version: DAMAGE_CLAUSE_COMPILER_VERSION,
        runtime_version: DAMAGE_TRANSACTION_RUNTIME_VERSION,
        semantic: binding.semantic.clone(),
        occurrence: binding.occurrence.clone(),
        capabilities: damage_capabilities(program),
        state_before_sha256,
        state_after_sha256,
        transaction,
    };
    if !receipt.has_exact_contract(program) {
        return Err(TypedOracleBridgeError::InvalidGeneratedReceipt(
            "damage receipt failed its exact contract",
        ));
    }
    *state = staged;
    Ok(receipt)
}

fn damage_capabilities(program: &CompiledDamageClause) -> Vec<TypedOracleBridgeCapability> {
    vec![
        TypedOracleBridgeCapability::ExactDamageClause {
            envelope: program.envelope(),
            amount: program.amount(),
            recipient: program.recipient(),
        },
        TypedOracleBridgeCapability::TransactionalDamageReplacementAndPrevention,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BridgeObjectRef {
    pub object: u64,
    pub incarnation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AbilityInvocationTimingInput {
    DeckConstruction,
    SpellResolution { resolving_source: BridgeObjectRef },
    AuraSpellTargeting { source_spell: BridgeObjectRef },
    Activated(AbilityActivationEvidence),
    Triggered(AbilityTriggerEvidence),
    StaticContinuous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityActivationEvidence {
    pub controller_had_priority: bool,
    pub controller_main_phase: bool,
    pub stack_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityTriggerEvidence {
    pub event_id: u64,
    pub kind: AbilityClauseTriggerEventKind,
    pub actor: u8,
    pub object: Option<AbilityEventObjectEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityEventObjectEvidence {
    pub object: BridgeObjectRef,
    pub controller: u8,
    pub card_types: BTreeSet<AbilityClauseCardType>,
    pub specific_card_types: BTreeSet<AbilityClauseSpecificCardType>,
    pub subtypes: BTreeSet<String>,
    pub facts_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityTargetInput {
    pub id: u16,
    pub target: AbilityTarget,
    pub legality_witness_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AbilityTarget {
    Player(u8),
    Object(BridgeObjectRef),
    Spell(BridgeObjectRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityChoiceInput {
    pub id: u16,
    pub value: AbilityChoiceValue,
    pub legality_witness_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AbilityChoiceValue {
    Boolean(bool),
    Number(u32),
    Color(AbilityColor),
    CreatureType(String),
    Objects(Vec<BridgeObjectRef>),
    Ordering(Vec<u64>),
    OpaqueRulesValueSha256(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbilityColor {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityPaymentInput {
    pub id: u16,
    pub payment: AbilityPayment,
    pub legality_witness_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AbilityPayment {
    Mana(AbilityManaPayment),
    Life(u16),
    Objects(Vec<BridgeObjectRef>),
    Cards(Vec<u64>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct AbilityManaPayment {
    pub generic: u16,
    pub white: u16,
    pub blue: u16,
    pub black: u16,
    pub red: u16,
    pub green: u16,
    pub colorless: u16,
}

impl AbilityManaPayment {
    fn total(self) -> u32 {
        [
            self.generic,
            self.white,
            self.blue,
            self.black,
            self.red,
            self.green,
            self.colorless,
        ]
        .into_iter()
        .map(u32::from)
        .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityClauseExecutionContext {
    pub source: BridgeObjectRef,
    pub controller: u8,
    pub timing: AbilityInvocationTimingInput,
    pub targets: Vec<AbilityTargetInput>,
    pub choices: Vec<AbilityChoiceInput>,
    pub payments: Vec<AbilityPaymentInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AbilityExternalInputId {
    Target(u16),
    Choice(u16),
    Payment(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityProjectionFact {
    pub predicate_sha256: String,
    pub observed_value_sha256: String,
}

impl AbilityProjectionFact {
    pub(crate) fn from_canonical(predicate: &str, observed_value: &str) -> Self {
        Self {
            predicate_sha256: sha256_framed(&["ability-fact/predicate", predicate]),
            observed_value_sha256: sha256_framed(&["ability-fact/value", observed_value]),
        }
    }

    fn is_valid(&self) -> bool {
        is_sha256_hex(&self.predicate_sha256) && is_sha256_hex(&self.observed_value_sha256)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AbilityMutationClass {
    Mana,
    Life,
    ObjectState,
    ZoneContents,
    ZoneOrder,
    ContinuousRule,
    Choice,
    Counter,
    Attachment,
    Monarch,
    Stack,
    TokenObjects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AbilityStateChange {
    ManaPool {
        player: u8,
        before_sha256: String,
        after_sha256: String,
    },
    PlayerLife {
        player: u8,
        before: i64,
        after: i64,
    },
    ObjectState {
        object: BridgeObjectRef,
        before_sha256: String,
        after_sha256: String,
    },
    ZoneContents {
        player: u8,
        zone: AbilityZone,
        before_sha256: String,
        after_sha256: String,
    },
    ZoneOrder {
        player: u8,
        zone: AbilityZone,
        before_sha256: String,
        after_sha256: String,
    },
    ContinuousRule {
        rule_sha256: String,
        installed: bool,
    },
    ChoiceRecorded {
        input: AbilityExternalInputId,
        decision_sha256: String,
    },
    Counter {
        object: BridgeObjectRef,
        counter_sha256: String,
        before: u32,
        after: u32,
    },
    Attachment {
        source: BridgeObjectRef,
        before: Option<BridgeObjectRef>,
        after: Option<BridgeObjectRef>,
    },
    Monarch {
        before: Option<u8>,
        after: Option<u8>,
    },
    StackObject {
        object: BridgeObjectRef,
        program_sha256: String,
        created: bool,
    },
    TokenObjects {
        controller: u8,
        objects: Vec<BridgeObjectRef>,
        token_definition_sha256: String,
    },
    /// A rules action was fully evaluated but produced no game-state mutation.
    /// This is allowed only for effects, never costs, and keeps the exact
    /// reason plus observed-state evidence in the receipt.
    NoMutation {
        class: AbilityMutationClass,
        reason: AbilityNoMutationReason,
        evidence: AbilityProjectionFact,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbilityNoMutationReason {
    OptionalChoiceDeclined,
    EmptySourceZone,
    TargetIllegalOnResolution,
    AlreadyInRequestedState,
    RuleAlreadyInstalled,
    ReplacementProducedNoChange,
}

impl AbilityStateChange {
    fn class(&self) -> AbilityMutationClass {
        match self {
            Self::ManaPool { .. } => AbilityMutationClass::Mana,
            Self::PlayerLife { .. } => AbilityMutationClass::Life,
            Self::ObjectState { .. } => AbilityMutationClass::ObjectState,
            Self::ZoneContents { .. } => AbilityMutationClass::ZoneContents,
            Self::ZoneOrder { .. } => AbilityMutationClass::ZoneOrder,
            Self::ContinuousRule { .. } => AbilityMutationClass::ContinuousRule,
            Self::ChoiceRecorded { .. } => AbilityMutationClass::Choice,
            Self::Counter { .. } => AbilityMutationClass::Counter,
            Self::Attachment { .. } => AbilityMutationClass::Attachment,
            Self::Monarch { .. } => AbilityMutationClass::Monarch,
            Self::StackObject { .. } => AbilityMutationClass::Stack,
            Self::TokenObjects { .. } => AbilityMutationClass::TokenObjects,
            Self::NoMutation { class, .. } => *class,
        }
    }

    fn validate(&self) -> bool {
        match self {
            Self::ManaPool {
                before_sha256,
                after_sha256,
                ..
            }
            | Self::ObjectState {
                before_sha256,
                after_sha256,
                ..
            }
            | Self::ZoneContents {
                before_sha256,
                after_sha256,
                ..
            }
            | Self::ZoneOrder {
                before_sha256,
                after_sha256,
                ..
            } => {
                is_sha256_hex(before_sha256)
                    && is_sha256_hex(after_sha256)
                    && before_sha256 != after_sha256
            }
            Self::PlayerLife { before, after, .. } => before != after,
            Self::ContinuousRule { rule_sha256, .. } => is_sha256_hex(rule_sha256),
            Self::ChoiceRecorded {
                decision_sha256, ..
            } => is_sha256_hex(decision_sha256),
            Self::Counter {
                counter_sha256,
                before,
                after,
                ..
            } => is_sha256_hex(counter_sha256) && before != after,
            Self::Attachment { before, after, .. } => before != after,
            Self::Monarch { before, after } => before != after,
            Self::StackObject { program_sha256, .. } => is_sha256_hex(program_sha256),
            Self::TokenObjects {
                objects,
                token_definition_sha256,
                ..
            } => {
                !objects.is_empty()
                    && objects.iter().copied().collect::<BTreeSet<_>>().len() == objects.len()
                    && is_sha256_hex(token_definition_sha256)
            }
            Self::NoMutation { evidence, .. } => evidence.is_valid(),
        }
    }

    fn is_no_mutation(&self) -> bool {
        matches!(self, Self::NoMutation { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityNodeProjection {
    pub facts: Vec<AbilityProjectionFact>,
    pub consumed_inputs: Vec<AbilityExternalInputId>,
    pub changes: Vec<AbilityStateChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityProjectionFailure {
    pub code: AbilityProjectionFailureCode,
    pub detail: String,
}

impl AbilityProjectionFailure {
    pub(crate) fn new(code: AbilityProjectionFailureCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbilityProjectionFailureCode {
    TimingNotSatisfied,
    PreconditionNotSatisfied,
    CostCannotBePaid,
    MissingTarget,
    IllegalTarget,
    MissingChoice,
    IllegalChoice,
    MissingPayment,
    IllegalPayment,
    EffectCannotResolve,
    StateProjectionIncomplete,
}

/// Production adapters implement this trait over a complete simulator state
/// projection. The bridge clones the projection before invoking any mutating
/// method, so a later error cannot retain paid costs or partial effects.
///
/// Each cost and effect must return explicit before/after state changes.
/// Returning only success, a score delta, or a heuristic estimate cannot
/// satisfy the bridge's receipt validation.
pub(crate) trait AbilityClauseStateProjection: Clone {
    /// Canonical, strictly sorted state components. Snapshot coordinates,
    /// display names, and transient report metadata must be omitted.
    fn canonical_state_components(&self) -> Vec<String>;

    fn validate_timing(
        &self,
        ability: &ExecutableAbility,
        context: &AbilityClauseExecutionContext,
    ) -> Result<Vec<AbilityProjectionFact>, AbilityProjectionFailure>;

    fn validate_precondition(
        &self,
        precondition: &AbilityPrecondition,
        context: &AbilityClauseExecutionContext,
    ) -> Result<Vec<AbilityProjectionFact>, AbilityProjectionFailure>;

    fn pay_cost(
        &mut self,
        cost: &AbilityCost,
        context: &AbilityClauseExecutionContext,
    ) -> Result<AbilityNodeProjection, AbilityProjectionFailure>;

    fn apply_effect(
        &mut self,
        effect: &AbilityEffect,
        context: &AbilityClauseExecutionContext,
    ) -> Result<AbilityNodeProjection, AbilityProjectionFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbilityNodePhase {
    Cost,
    Effect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityNodeProductionReceipt {
    pub phase: AbilityNodePhase,
    pub index: u16,
    pub node_semantic_sha256: String,
    pub facts: Vec<AbilityProjectionFact>,
    pub consumed_inputs: Vec<AbilityExternalInputId>,
    pub changes: Vec<AbilityStateChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbilityProductionReceipt {
    pub receipt_version: &'static str,
    pub bridge_version: &'static str,
    pub compiler_version: &'static str,
    pub runtime_version: &'static str,
    pub semantic: OracleSemanticEvidence,
    pub occurrence: OracleOccurrenceProvenance,
    pub capabilities: Vec<TypedOracleBridgeCapability>,
    pub invocation: AbilityClauseExecutionContext,
    pub invocation_sha256: String,
    pub state_before_sha256: String,
    pub state_after_sha256: String,
    pub timing_facts: Vec<AbilityProjectionFact>,
    pub precondition_facts: Vec<Vec<AbilityProjectionFact>>,
    pub nodes: Vec<AbilityNodeProductionReceipt>,
    pub consumed_inputs: Vec<AbilityExternalInputId>,
}

impl AbilityProductionReceipt {
    pub(crate) fn has_exact_contract(&self, program: &AbilityClauseBridgeProgram) -> bool {
        let ability = program.ability();
        let expected_nodes = ability
            .costs
            .iter()
            .enumerate()
            .map(|(index, node)| {
                (
                    AbilityNodePhase::Cost,
                    u16::try_from(index).ok(),
                    ability_node_digest(
                        program.semantic_digest(),
                        AbilityNodePhase::Cost,
                        index,
                        node,
                    ),
                    cost_required_mutation_groups(node),
                    false,
                )
            })
            .chain(ability.effects.iter().enumerate().map(|(index, node)| {
                (
                    AbilityNodePhase::Effect,
                    u16::try_from(index).ok(),
                    ability_node_digest(
                        program.semantic_digest(),
                        AbilityNodePhase::Effect,
                        index,
                        node,
                    ),
                    effect_required_mutation_groups(node),
                    true,
                )
            }))
            .collect::<Vec<_>>();
        let expected_inputs = validate_external_inputs(&self.invocation).ok();
        let node_consumed = self
            .nodes
            .iter()
            .flat_map(|node| node.consumed_inputs.iter().copied())
            .collect::<BTreeSet<_>>();
        let node_consumed_count = self
            .nodes
            .iter()
            .map(|node| node.consumed_inputs.len())
            .sum::<usize>();
        self.receipt_version == TYPED_ORACLE_RECEIPT_VERSION
            && self.bridge_version == TYPED_ORACLE_PRODUCTION_BRIDGE_VERSION
            && self.compiler_version == ABILITY_CLAUSE_BRIDGE_COMPILER_VERSION
            && self.runtime_version == ABILITY_CLAUSE_BRIDGE_RUNTIME_VERSION
            && self.semantic == OracleSemanticEvidence::for_ability(program)
            && self.capabilities == ability_capabilities(program)
            && self.occurrence.validate().is_ok()
            && self.invocation.source.object == self.occurrence.source_object
            && self.invocation.source.incarnation == self.occurrence.source_incarnation
            && validate_ability_invocation(program, &self.invocation).is_ok()
            && is_sha256_hex(&self.invocation_sha256)
            && self.invocation_sha256
                == ability_invocation_digest(program.semantic_digest(), &self.invocation)
            && is_sha256_hex(&self.state_before_sha256)
            && is_sha256_hex(&self.state_after_sha256)
            && !self.timing_facts.is_empty()
            && self
                .timing_facts
                .iter()
                .all(AbilityProjectionFact::is_valid)
            && self.precondition_facts.len() == ability.preconditions.len()
            && self
                .precondition_facts
                .iter()
                .all(|facts| !facts.is_empty() && facts.iter().all(AbilityProjectionFact::is_valid))
            && self.nodes.len() == expected_nodes.len()
            && self.nodes.iter().zip(expected_nodes).all(
                |(actual, (phase, index, digest, required_groups, allow_no_mutation))| {
                    index == Some(actual.index)
                        && actual.phase == phase
                        && actual.node_semantic_sha256 == digest
                        && is_sha256_hex(&actual.node_semantic_sha256)
                        && actual.facts.iter().all(AbilityProjectionFact::is_valid)
                        && !actual.changes.is_empty()
                        && actual.changes.iter().all(AbilityStateChange::validate)
                        && (allow_no_mutation
                            || !actual
                                .changes
                                .iter()
                                .any(AbilityStateChange::is_no_mutation))
                        && mutation_groups_satisfied(&actual.changes, &required_groups)
                },
            )
            && self
                .consumed_inputs
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            && expected_inputs.as_ref().is_some_and(|expected| {
                expected
                    .iter()
                    .copied()
                    .eq(self.consumed_inputs.iter().copied())
            })
            && node_consumed
                .iter()
                .copied()
                .eq(self.consumed_inputs.iter().copied())
            && node_consumed_count == node_consumed.len()
    }
}

pub(crate) fn execute_ability_clause_transaction<S: AbilityClauseStateProjection>(
    state: &mut S,
    program: &AbilityClauseBridgeProgram,
    binding: &TypedOracleExecutionBinding,
    context: &AbilityClauseExecutionContext,
) -> Result<AbilityProductionReceipt, TypedOracleBridgeError> {
    validate_binding(
        binding,
        program.exact_source(),
        program.normalized_source(),
        program.semantic_digest(),
    )?;
    if context.source.object != binding.occurrence.source_object
        || context.source.incarnation != binding.occurrence.source_incarnation
    {
        return Err(TypedOracleBridgeError::OccurrenceSourceMismatch);
    }
    validate_ability_invocation(program, context)?;
    let all_inputs = validate_external_inputs(context)?;
    let before_components = validate_canonical_state(state.canonical_state_components())?;
    let state_before_sha256 = canonical_state_digest("ability/state", &before_components);
    let mut staged = state.clone();
    let ability = program.ability();

    let timing_facts = staged
        .validate_timing(ability, context)
        .map_err(TypedOracleBridgeError::AbilityProjection)?;
    validate_facts(&timing_facts, "timing validation")?;

    let mut precondition_facts = Vec::with_capacity(ability.preconditions.len());
    for precondition in &ability.preconditions {
        let facts = staged
            .validate_precondition(precondition, context)
            .map_err(TypedOracleBridgeError::AbilityProjection)?;
        validate_facts(&facts, "precondition validation")?;
        precondition_facts.push(facts);
    }

    let mut nodes = Vec::with_capacity(ability.costs.len() + ability.effects.len());
    let mut consumed = BTreeSet::new();
    for (index, cost) in ability.costs.iter().enumerate() {
        let projected = staged
            .pay_cost(cost, context)
            .map_err(TypedOracleBridgeError::AbilityProjection)?;
        validate_node_projection(
            &projected,
            &cost_required_mutation_groups(cost),
            &all_inputs,
            &mut consumed,
            false,
        )?;
        nodes.push(AbilityNodeProductionReceipt {
            phase: AbilityNodePhase::Cost,
            index: u16::try_from(index)
                .map_err(|_| TypedOracleBridgeError::AbilityNodeIndexOverflow)?,
            node_semantic_sha256: ability_node_digest(
                program.semantic_digest(),
                AbilityNodePhase::Cost,
                index,
                cost,
            ),
            facts: projected.facts,
            consumed_inputs: projected.consumed_inputs,
            changes: projected.changes,
        });
    }
    for (index, effect) in ability.effects.iter().enumerate() {
        let projected = staged
            .apply_effect(effect, context)
            .map_err(TypedOracleBridgeError::AbilityProjection)?;
        validate_node_projection(
            &projected,
            &effect_required_mutation_groups(effect),
            &all_inputs,
            &mut consumed,
            true,
        )?;
        nodes.push(AbilityNodeProductionReceipt {
            phase: AbilityNodePhase::Effect,
            index: u16::try_from(index)
                .map_err(|_| TypedOracleBridgeError::AbilityNodeIndexOverflow)?,
            node_semantic_sha256: ability_node_digest(
                program.semantic_digest(),
                AbilityNodePhase::Effect,
                index,
                effect,
            ),
            facts: projected.facts,
            consumed_inputs: projected.consumed_inputs,
            changes: projected.changes,
        });
    }
    if consumed != all_inputs {
        return Err(TypedOracleBridgeError::ExternalInputConsumptionMismatch {
            expected: all_inputs.into_iter().collect(),
            actual: consumed.into_iter().collect(),
        });
    }

    let after_components = validate_canonical_state(staged.canonical_state_components())?;
    let state_after_sha256 = canonical_state_digest("ability/state", &after_components);
    let consumed_inputs = consumed.into_iter().collect::<Vec<_>>();
    let receipt = AbilityProductionReceipt {
        receipt_version: TYPED_ORACLE_RECEIPT_VERSION,
        bridge_version: TYPED_ORACLE_PRODUCTION_BRIDGE_VERSION,
        compiler_version: ABILITY_CLAUSE_BRIDGE_COMPILER_VERSION,
        runtime_version: ABILITY_CLAUSE_BRIDGE_RUNTIME_VERSION,
        semantic: binding.semantic.clone(),
        occurrence: binding.occurrence.clone(),
        capabilities: ability_capabilities(program),
        invocation: context.clone(),
        invocation_sha256: ability_invocation_digest(program.semantic_digest(), context),
        state_before_sha256,
        state_after_sha256,
        timing_facts,
        precondition_facts,
        nodes,
        consumed_inputs,
    };
    if !receipt.has_exact_contract(program) {
        return Err(TypedOracleBridgeError::InvalidGeneratedReceipt(
            "ability receipt failed its exact contract",
        ));
    }
    *state = staged;
    Ok(receipt)
}

fn validate_ability_invocation(
    program: &AbilityClauseBridgeProgram,
    context: &AbilityClauseExecutionContext,
) -> Result<(), TypedOracleBridgeError> {
    match (program.timing(), &context.timing) {
        (
            AbilityClauseTimingEnvelope::DeckConstruction,
            AbilityInvocationTimingInput::DeckConstruction,
        )
        | (
            AbilityClauseTimingEnvelope::StaticModifier,
            AbilityInvocationTimingInput::StaticContinuous,
        ) => {}
        (
            AbilityClauseTimingEnvelope::SpellResolution,
            AbilityInvocationTimingInput::SpellResolution { resolving_source },
        ) if resolving_source == &context.source => {}
        (
            AbilityClauseTimingEnvelope::AuraSpellTargeting,
            AbilityInvocationTimingInput::AuraSpellTargeting { source_spell },
        ) if source_spell == &context.source => {}
        (
            AbilityClauseTimingEnvelope::Activated { window },
            AbilityInvocationTimingInput::Activated(evidence),
        ) => {
            if !evidence.controller_had_priority {
                return Err(TypedOracleBridgeError::AbilityTimingMismatch(
                    "activation requires controller priority",
                ));
            }
            if *window == AbilityClauseActivationWindow::SorcerySpeedOnly
                && (!evidence.controller_main_phase || !evidence.stack_empty)
            {
                return Err(TypedOracleBridgeError::AbilityTimingMismatch(
                    "sorcery-speed activation requires the controller's main phase and an empty stack",
                ));
            }
        }
        (
            AbilityClauseTimingEnvelope::Triggered { event },
            AbilityInvocationTimingInput::Triggered(actual),
        ) => validate_trigger_evidence(context, event, actual)?,
        _ => {
            return Err(TypedOracleBridgeError::AbilityTimingMismatch(
                "invocation timing does not match the compiled timing envelope",
            ));
        }
    }

    if program
        .ability()
        .preconditions
        .iter()
        .any(|condition| matches!(condition, AbilityPrecondition::EventObjectIsSource))
    {
        let AbilityInvocationTimingInput::Triggered(trigger) = &context.timing else {
            return Err(TypedOracleBridgeError::AbilityEventMismatch(
                "source-event precondition requires triggered event evidence",
            ));
        };
        if trigger.object.as_ref().map(|object| object.object) != Some(context.source) {
            return Err(TypedOracleBridgeError::AbilityEventMismatch(
                "event object is not the source incarnation",
            ));
        }
    }
    Ok(())
}

fn validate_trigger_evidence(
    context: &AbilityClauseExecutionContext,
    expected: &crate::ability_clause_bridge::AbilityClauseTriggerEvent,
    actual: &AbilityTriggerEvidence,
) -> Result<(), TypedOracleBridgeError> {
    if expected.kind != actual.kind {
        return Err(TypedOracleBridgeError::AbilityEventMismatch(
            "trigger event kind does not match",
        ));
    }
    if !relation_matches(expected.actor, context.controller, actual.actor) {
        return Err(TypedOracleBridgeError::AbilityEventMismatch(
            "trigger actor does not match the retained controller relation",
        ));
    }
    let requires_object = trigger_kind_requires_object(expected.kind)
        || object_filter_has_constraints(&expected.object_filter);
    if requires_object && actual.object.is_none() {
        return Err(TypedOracleBridgeError::AbilityEventMismatch(
            "trigger event requires complete object evidence",
        ));
    }
    if let Some(object) = &actual.object {
        if !object.facts_complete {
            return Err(TypedOracleBridgeError::AbilityEventMismatch(
                "trigger object facts are incomplete",
            ));
        }
        if !object_matches_filter(object, &expected.object_filter, context.controller) {
            return Err(TypedOracleBridgeError::AbilityEventMismatch(
                "trigger object does not match the retained filter",
            ));
        }
    }
    Ok(())
}

fn trigger_kind_requires_object(kind: AbilityClauseTriggerEventKind) -> bool {
    !matches!(
        kind,
        AbilityClauseTriggerEventKind::BeginningOfUpkeep
            | AbilityClauseTriggerEventKind::BeginningOfEndStep
            | AbilityClauseTriggerEventKind::CardDraw
    )
}

fn object_filter_has_constraints(filter: &AbilityClauseObjectFilter) -> bool {
    filter.card_type.is_some()
        || !filter.any_of_card_types.is_empty()
        || filter.excluded_card_type.is_some()
        || filter.subtype.is_some()
        || filter.excluded_subtype.is_some()
        || filter.nonland
        || filter.controller.is_some()
}

fn object_matches_filter(
    object: &AbilityEventObjectEvidence,
    filter: &AbilityClauseObjectFilter,
    source_controller: u8,
) -> bool {
    if filter
        .card_type
        .is_some_and(|required| !object.card_types.contains(&required))
    {
        return false;
    }
    if !filter.any_of_card_types.is_empty()
        && !filter
            .any_of_card_types
            .iter()
            .any(|required| object.specific_card_types.contains(required))
    {
        return false;
    }
    if filter
        .excluded_card_type
        .is_some_and(|excluded| object.card_types.contains(&excluded))
    {
        return false;
    }
    if filter.subtype.as_ref().is_some_and(|required| {
        !object
            .subtypes
            .iter()
            .any(|actual| actual.eq_ignore_ascii_case(required))
    }) {
        return false;
    }
    if filter.excluded_subtype.as_ref().is_some_and(|excluded| {
        object
            .subtypes
            .iter()
            .any(|actual| actual.eq_ignore_ascii_case(excluded))
    }) {
        return false;
    }
    if filter.nonland && object.card_types.contains(&AbilityClauseCardType::Land) {
        return false;
    }
    filter
        .controller
        .is_none_or(|relation| relation_matches(relation, source_controller, object.controller))
}

fn relation_matches(
    relation: AbilityClauseControllerRelation,
    source_controller: u8,
    actual: u8,
) -> bool {
    match relation {
        AbilityClauseControllerRelation::You => actual == source_controller,
        AbilityClauseControllerRelation::Opponent => actual != source_controller,
        AbilityClauseControllerRelation::Any => true,
    }
}

fn validate_external_inputs(
    context: &AbilityClauseExecutionContext,
) -> Result<BTreeSet<AbilityExternalInputId>, TypedOracleBridgeError> {
    let mut all = BTreeSet::new();
    for target in &context.targets {
        if !is_sha256_hex(&target.legality_witness_sha256) {
            return Err(TypedOracleBridgeError::InvalidExternalInput(
                "target legality witness must be lowercase SHA-256",
            ));
        }
        if !all.insert(AbilityExternalInputId::Target(target.id)) {
            return Err(TypedOracleBridgeError::DuplicateExternalInput(
                AbilityExternalInputId::Target(target.id),
            ));
        }
    }
    for choice in &context.choices {
        if !is_sha256_hex(&choice.legality_witness_sha256) || !valid_choice_value(&choice.value) {
            return Err(TypedOracleBridgeError::InvalidExternalInput(
                "choice value or legality witness is invalid",
            ));
        }
        if !all.insert(AbilityExternalInputId::Choice(choice.id)) {
            return Err(TypedOracleBridgeError::DuplicateExternalInput(
                AbilityExternalInputId::Choice(choice.id),
            ));
        }
    }
    for payment in &context.payments {
        if !is_sha256_hex(&payment.legality_witness_sha256) || !valid_payment(&payment.payment) {
            return Err(TypedOracleBridgeError::InvalidExternalInput(
                "payment value or legality witness is invalid",
            ));
        }
        if !all.insert(AbilityExternalInputId::Payment(payment.id)) {
            return Err(TypedOracleBridgeError::DuplicateExternalInput(
                AbilityExternalInputId::Payment(payment.id),
            ));
        }
    }
    Ok(all)
}

fn valid_choice_value(value: &AbilityChoiceValue) -> bool {
    match value {
        AbilityChoiceValue::Boolean(_)
        | AbilityChoiceValue::Number(_)
        | AbilityChoiceValue::Color(_) => true,
        AbilityChoiceValue::CreatureType(value) => !value.trim().is_empty(),
        AbilityChoiceValue::Objects(objects) => {
            !objects.is_empty()
                && objects.iter().copied().collect::<BTreeSet<_>>().len() == objects.len()
        }
        AbilityChoiceValue::Ordering(objects) => {
            !objects.is_empty()
                && objects.iter().copied().collect::<BTreeSet<_>>().len() == objects.len()
        }
        AbilityChoiceValue::OpaqueRulesValueSha256(value) => is_sha256_hex(value),
    }
}

fn valid_payment(payment: &AbilityPayment) -> bool {
    match payment {
        AbilityPayment::Mana(mana) => mana.total() > 0,
        AbilityPayment::Life(life) => *life > 0,
        AbilityPayment::Objects(objects) => {
            !objects.is_empty()
                && objects.iter().copied().collect::<BTreeSet<_>>().len() == objects.len()
        }
        AbilityPayment::Cards(cards) => {
            !cards.is_empty() && cards.iter().copied().collect::<BTreeSet<_>>().len() == cards.len()
        }
    }
}

fn validate_node_projection(
    projected: &AbilityNodeProjection,
    required_groups: &[Vec<AbilityMutationClass>],
    all_inputs: &BTreeSet<AbilityExternalInputId>,
    consumed: &mut BTreeSet<AbilityExternalInputId>,
    allow_no_mutation: bool,
) -> Result<(), TypedOracleBridgeError> {
    if projected.changes.is_empty() || !projected.changes.iter().all(AbilityStateChange::validate) {
        return Err(TypedOracleBridgeError::IncompleteAbilityNodeReceipt);
    }
    if !allow_no_mutation
        && projected
            .changes
            .iter()
            .any(AbilityStateChange::is_no_mutation)
    {
        return Err(TypedOracleBridgeError::NoMutationCannotPayCost);
    }
    if !projected.facts.iter().all(AbilityProjectionFact::is_valid) {
        return Err(TypedOracleBridgeError::InvalidProjectionFact);
    }
    let actual_classes = projected
        .changes
        .iter()
        .map(AbilityStateChange::class)
        .collect::<BTreeSet<_>>();
    if let Some(group) = required_groups.iter().find(|group| {
        !group
            .iter()
            .any(|required| actual_classes.contains(required))
    }) {
        return Err(TypedOracleBridgeError::MissingRequiredMutation {
            accepted: group.clone(),
            actual: actual_classes.iter().copied().collect(),
        });
    }
    let mut local = BTreeSet::new();
    for input in &projected.consumed_inputs {
        if !all_inputs.contains(input) {
            return Err(TypedOracleBridgeError::UnknownConsumedInput(*input));
        }
        if !local.insert(*input) || !consumed.insert(*input) {
            return Err(TypedOracleBridgeError::ExternalInputConsumedTwice(*input));
        }
    }
    Ok(())
}

fn mutation_groups_satisfied(
    changes: &[AbilityStateChange],
    required_groups: &[Vec<AbilityMutationClass>],
) -> bool {
    let actual = changes
        .iter()
        .map(AbilityStateChange::class)
        .collect::<BTreeSet<_>>();
    required_groups
        .iter()
        .all(|group| group.iter().any(|required| actual.contains(required)))
}

fn validate_facts(
    facts: &[AbilityProjectionFact],
    context: &'static str,
) -> Result<(), TypedOracleBridgeError> {
    if facts.is_empty() || !facts.iter().all(AbilityProjectionFact::is_valid) {
        return Err(TypedOracleBridgeError::IncompleteProjectionFacts(context));
    }
    Ok(())
}

fn cost_required_mutation_groups(cost: &AbilityCost) -> Vec<Vec<AbilityMutationClass>> {
    use AbilityMutationClass as Class;
    match cost {
        AbilityCost::Mana(_) => vec![vec![Class::Mana]],
        AbilityCost::TapSelf | AbilityCost::TapPermanents { .. } => {
            vec![vec![Class::ObjectState]]
        }
        AbilityCost::SacrificeSelf | AbilityCost::SacrificeResource { .. } => {
            vec![vec![Class::ObjectState, Class::ZoneContents]]
        }
        AbilityCost::Discard(_) | AbilityCost::ExileFromGraveyard { .. } => {
            vec![vec![Class::ZoneContents]]
        }
        AbilityCost::PayLife(_) => vec![vec![Class::Life]],
    }
}

fn effect_required_mutation_groups(effect: &AbilityEffect) -> Vec<Vec<AbilityMutationClass>> {
    use AbilityMutationClass as Class;
    match effect {
        AbilityEffect::PartnerCommanderPairing(_)
        | AbilityEffect::GrantCastPermission(_)
        | AbilityEffect::GrantCastTimingPermission(_)
        | AbilityEffect::Ward(_)
        | AbilityEffect::ModifyNonlandMana(_)
        | AbilityEffect::ApplyStaticCreatureModifier(_)
        | AbilityEffect::ReduceSpellCost(_)
        | AbilityEffect::AlternativeSpellCost(_)
        | AbilityEffect::SourceHasChosenCreatureType(_)
        | AbilityEffect::MultiplyTriggeredAbility(_)
        | AbilityEffect::GrantAllCreatureTypes(_)
        | AbilityEffect::DoesNotUntapDuringUntapStep(_) => {
            vec![vec![Class::ContinuousRule]]
        }
        AbilityEffect::AddMana(_) => vec![vec![Class::Mana]],
        AbilityEffect::AddManaWithRetention(_) => {
            vec![vec![Class::Mana], vec![Class::ContinuousRule]]
        }
        AbilityEffect::Draw(_)
        | AbilityEffect::LookAtTopAndSelect(_)
        | AbilityEffect::ExhaustiveTopCardAccess(_)
        | AbilityEffect::Tutor(_)
        | AbilityEffect::VariableCreatureTutor(_)
        | AbilityEffect::Mill(_)
        | AbilityEffect::MoveZone(_)
        | AbilityEffect::WholeHandDiscardThenDraw(_) => {
            vec![vec![Class::ZoneContents, Class::ZoneOrder]]
        }
        AbilityEffect::CumulativeUpkeep(_) => vec![
            vec![Class::Counter],
            vec![Class::Mana, Class::ObjectState, Class::ZoneContents],
        ],
        AbilityEffect::LoseLife(_) => vec![vec![Class::Life]],
        AbilityEffect::UnlessEventPlayerPays(_) | AbilityEffect::Conditional(_) => {
            vec![vec![Class::Choice]]
        }
        AbilityEffect::RepeatableTopCardReveal(_) => vec![
            vec![Class::Choice],
            vec![Class::ZoneContents, Class::ZoneOrder],
            vec![Class::Life],
        ],
        AbilityEffect::LinkedDelayedCardAccess(_) => {
            vec![
                vec![Class::ContinuousRule],
                vec![Class::ZoneContents, Class::ZoneOrder],
            ]
        }
        AbilityEffect::VariableCreatureOverrun(_)
        | AbilityEffect::ModifyPowerToughnessUntilEndOfTurn(_) => {
            vec![vec![Class::ContinuousRule, Class::ObjectState]]
        }
        AbilityEffect::CopyThisSpell(_) => vec![vec![Class::Stack]],
        AbilityEffect::Tap(_) | AbilityEffect::Untap(_) => vec![vec![Class::ObjectState]],
        AbilityEffect::OptionalUntap(_) => vec![vec![Class::Choice]],
        AbilityEffect::CreateToken(_) => vec![vec![Class::TokenObjects]],
        AbilityEffect::Scry(_) => vec![vec![Class::ZoneOrder]],
        AbilityEffect::AddManaAndSourceDamage(_) => {
            vec![vec![Class::Mana], vec![Class::Life, Class::ObjectState]]
        }
        AbilityEffect::AttachSourceToTarget(_) => vec![vec![Class::Attachment]],
        AbilityEffect::ChooseCreatureType(_) => {
            vec![vec![Class::Choice], vec![Class::ContinuousRule]]
        }
        AbilityEffect::BecomeMonarch(_) => vec![vec![Class::Monarch]],
        AbilityEffect::AddCounters(_) => vec![vec![Class::Counter]],
        AbilityEffect::SacrificeSelf => {
            vec![vec![Class::ObjectState, Class::ZoneContents]]
        }
    }
}

fn ability_capabilities(program: &AbilityClauseBridgeProgram) -> Vec<TypedOracleBridgeCapability> {
    let ability = program.ability();
    vec![
        TypedOracleBridgeCapability::ExactAbilityClause {
            timing: program.timing().clone(),
            preconditions: u16::try_from(ability.preconditions.len()).unwrap_or(u16::MAX),
            costs: u16::try_from(ability.costs.len()).unwrap_or(u16::MAX),
            effects: u16::try_from(ability.effects.len()).unwrap_or(u16::MAX),
        },
        TypedOracleBridgeCapability::TransactionalAbilityCostsAndResolution,
        TypedOracleBridgeCapability::CompleteExternalInputConsumption,
    ]
}

fn ability_node_digest<T: fmt::Debug>(
    program_digest: &str,
    phase: AbilityNodePhase,
    index: usize,
    node: &T,
) -> String {
    sha256_framed(&[
        "ability-node-content/v1",
        TYPED_ORACLE_PRODUCTION_BRIDGE_VERSION,
        program_digest,
        &format!("{phase:?}"),
        &index.to_string(),
        &format!("{node:?}"),
    ])
}

fn ability_invocation_digest(
    program_digest: &str,
    context: &AbilityClauseExecutionContext,
) -> String {
    sha256_framed(&[
        "ability-invocation/v1",
        TYPED_ORACLE_PRODUCTION_BRIDGE_VERSION,
        program_digest,
        &format!("{context:?}"),
    ])
}

fn validate_canonical_state(
    components: Vec<String>,
) -> Result<Vec<String>, TypedOracleBridgeError> {
    if components.is_empty()
        || components
            .iter()
            .any(|component| component.is_empty() || component.trim() != component)
        || !components.windows(2).all(|pair| pair[0] < pair[1])
    {
        return Err(TypedOracleBridgeError::NonCanonicalStateProjection);
    }
    Ok(components)
}

fn canonical_state_digest(label: &str, components: &[String]) -> String {
    let mut framed = vec![
        "typed-oracle-state/v1".to_owned(),
        TYPED_ORACLE_PRODUCTION_BRIDGE_VERSION.to_owned(),
        label.to_owned(),
    ];
    framed.extend(components.iter().cloned());
    sha256_owned_framed(&framed)
}

fn validate_binding(
    binding: &TypedOracleExecutionBinding,
    exact_oracle: &str,
    normalized_oracle: &str,
    semantic_digest: &str,
) -> Result<(), TypedOracleBridgeError> {
    binding.semantic.validate_shape()?;
    binding.occurrence.validate()?;
    if binding.semantic.exact_oracle != exact_oracle {
        return Err(TypedOracleBridgeError::SemanticEvidenceMismatch(
            "exact Oracle source",
        ));
    }
    if binding.semantic.normalized_oracle != normalized_oracle {
        return Err(TypedOracleBridgeError::SemanticEvidenceMismatch(
            "normalized Oracle source",
        ));
    }
    if binding.semantic.semantic_digest != semantic_digest {
        return Err(TypedOracleBridgeError::SemanticEvidenceMismatch(
            "semantic digest",
        ));
    }
    Ok(())
}

fn debug_state_digest<T: fmt::Debug>(label: &str, state: &T) -> String {
    sha256_framed(&[
        "typed-oracle-debug-state/v1",
        TYPED_ORACLE_PRODUCTION_BRIDGE_VERSION,
        label,
        &format!("{state:#?}"),
    ])
}

fn sha256_framed(components: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for component in components {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn sha256_owned_framed(components: &[String]) -> String {
    let mut hasher = Sha256::new();
    for component in components {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypedOracleBridgeError {
    InvalidSemanticEvidence(&'static str),
    InvalidOccurrenceProvenance(&'static str),
    SemanticEvidenceMismatch(&'static str),
    OccurrenceSourceMismatch,
    ObjectStateInputMismatch(&'static str),
    UntapChoiceSetMismatch {
        expected: Vec<ObjectRef>,
        actual: Vec<ObjectRef>,
    },
    IncompleteReplacementSequence,
    IncompleteExternalReplacementEvidence,
    TrailingReplacementStep {
        index: usize,
    },
    InvalidExternalReplacementIdentity,
    DuplicateExternalReplacementIdentity(String),
    BoundReplacementNotApplicable,
    ObjectState(ObjectStateRuntimeError),
    DamageBinding(DamageClauseBindingError),
    DamageTransaction(DamageTransactionError),
    DamageRequestSemanticMismatch,
    AbilityTimingMismatch(&'static str),
    AbilityEventMismatch(&'static str),
    InvalidExternalInput(&'static str),
    DuplicateExternalInput(AbilityExternalInputId),
    UnknownConsumedInput(AbilityExternalInputId),
    ExternalInputConsumedTwice(AbilityExternalInputId),
    ExternalInputConsumptionMismatch {
        expected: Vec<AbilityExternalInputId>,
        actual: Vec<AbilityExternalInputId>,
    },
    InvalidProjectionFact,
    IncompleteProjectionFacts(&'static str),
    IncompleteAbilityNodeReceipt,
    MissingRequiredMutation {
        accepted: Vec<AbilityMutationClass>,
        actual: Vec<AbilityMutationClass>,
    },
    NonCanonicalStateProjection,
    NoMutationCannotPayCost,
    AbilityNodeIndexOverflow,
    AbilityProjection(AbilityProjectionFailure),
    InvalidGeneratedReceipt(&'static str),
}

impl fmt::Display for TypedOracleBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TypedOracleBridgeError {}
