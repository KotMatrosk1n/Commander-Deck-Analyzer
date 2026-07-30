//! Exact, content keyed programs for three self object state clauses.
//!
//! This module owns the rules transactions for an optional untap-step choice,
//! a self graveyard-to-exile replacement, and an unconditional tapped entry.
//! It deliberately has no production adapter. Syntax recognition and the
//! standalone transactions in this file must not be reported as live
//! simulation coverage until an adapter supplies complete game state and
//! replacement-order evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const OBJECT_STATE_CLAUSE_COMPILER_VERSION: &str = "object-state-clause-compiler-0.2";
pub const OBJECT_STATE_CLAUSE_RUNTIME_VERSION: &str = "object-state-clause-runtime-0.1";
pub const OBJECT_STATE_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:113.6h,400.6-7,403.4,502.3-4,\
     614.1,614.5-6,614.12,616.1";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type BindingId = u64;
pub type UntapStepId = u64;
pub type ZoneChangeEventId = u64;

const OPTIONAL_UNTAP_NORMALIZED: &str =
    "you may choose not to untap this object during your untap step.";
const SELF_GRAVEYARD_EXILE_NORMALIZED: &str =
    "if this object would be put into a graveyard from anywhere, exile it instead.";
const ENTERS_BATTLEFIELD_TAPPED_NORMALIZED: &str = "this object enters the battlefield tapped.";
const ENTERS_TAPPED_NORMALIZED: &str = "this object enters tapped.";

/// These programs remain nonlive until the main engine supplies a complete
/// battlefield, simultaneous untap-step commit, and full replacement census.
pub const fn object_state_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectStateClauseKind {
    OptionalUntapDuringYourUntapStep,
    SelfGraveyardMoveBecomesExile,
    EntersBattlefieldTapped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStateClauseProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    kind: ObjectStateClauseKind,
}

impl ObjectStateClauseProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> ObjectStateClauseKind {
        self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        object_state_production_adapter_connected()
    }
}

pub fn compile_object_state_clause_program(
    exact_source: &str,
    normalized_source: &str,
) -> Option<ObjectStateClauseProgram> {
    if exact_source.is_empty()
        || normalized_source.is_empty()
        || exact_source.trim() != exact_source
        || normalized_source.trim() != normalized_source
    {
        return None;
    }

    let kind = match normalized_source.to_ascii_lowercase().as_str() {
        OPTIONAL_UNTAP_NORMALIZED => ObjectStateClauseKind::OptionalUntapDuringYourUntapStep,
        SELF_GRAVEYARD_EXILE_NORMALIZED => ObjectStateClauseKind::SelfGraveyardMoveBecomesExile,
        ENTERS_BATTLEFIELD_TAPPED_NORMALIZED | ENTERS_TAPPED_NORMALIZED => {
            ObjectStateClauseKind::EntersBattlefieldTapped
        }
        _ => return None,
    };
    let semantic_digest = object_state_semantic_digest(exact_source, normalized_source, kind);
    Some(ObjectStateClauseProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        semantic_digest,
        kind,
    })
}

fn object_state_semantic_digest(
    exact_source: &str,
    normalized_source: &str,
    kind: ObjectStateClauseKind,
) -> String {
    let kind_contract = match kind {
        ObjectStateClauseKind::OptionalUntapDuringYourUntapStep => {
            "untap-step/v1;chooser=active-controller;choice=untap-or-remain-tapped;\
             simultaneous=true;stack=false"
        }
        ObjectStateClauseKind::SelfGraveyardMoveBecomesExile => {
            "zone-replacement/v1;subject=same-object-incarnation;from=any-zone;\
             would-destination=graveyard;instead-destination=exile;apply-once=true"
        }
        ObjectStateClauseKind::EntersBattlefieldTapped => {
            "entry-replacement/v1;subject=same-object-incarnation;\
             destination=battlefield;entry-status=tapped;apply-once=true"
        }
    };
    let mut hasher = Sha256::new();
    for component in [
        "object-state-clause-content/v1",
        OBJECT_STATE_CLAUSE_COMPILER_VERSION,
        OBJECT_STATE_CLAUSE_RUNTIME_VERSION,
        OBJECT_STATE_RULES_CONTEXT_VERSION,
        exact_source,
        normalized_source,
        kind_contract,
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectZone {
    Battlefield,
    Command,
    Exile,
    Graveyard,
    Hand,
    Library,
    Stack,
    OutsideGame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedObject {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub controller: Option<PlayerId>,
    pub zone: ObjectZone,
    /// `Some` is required on the battlefield and forbidden elsewhere.
    pub tapped: Option<bool>,
}

impl TrackedObject {
    pub fn battlefield(
        object_ref: ObjectRef,
        owner: PlayerId,
        controller: PlayerId,
        tapped: bool,
    ) -> Self {
        Self {
            object_ref,
            owner,
            controller: Some(controller),
            zone: ObjectZone::Battlefield,
            tapped: Some(tapped),
        }
    }

    pub fn card(object_ref: ObjectRef, owner: PlayerId, zone: ObjectZone) -> Self {
        Self {
            object_ref,
            owner,
            controller: (zone == ObjectZone::Stack).then_some(owner),
            zone,
            tapped: None,
        }
    }

    fn validate(&self) -> Result<(), ObjectStateRuntimeError> {
        match self.zone {
            ObjectZone::Battlefield => {
                if self.controller.is_none() {
                    return Err(ObjectStateRuntimeError::MissingController(self.object_ref));
                }
                if self.tapped.is_none() {
                    return Err(ObjectStateRuntimeError::MissingTappedState(self.object_ref));
                }
            }
            ObjectZone::Stack => {
                if self.controller.is_none() {
                    return Err(ObjectStateRuntimeError::MissingController(self.object_ref));
                }
                if self.tapped.is_some() {
                    return Err(ObjectStateRuntimeError::TappedStateOutsideBattlefield(
                        self.object_ref,
                    ));
                }
            }
            _ => {
                if self.controller.is_some() {
                    return Err(ObjectStateRuntimeError::ControllerOutsideControlledZone(
                        self.object_ref,
                    ));
                }
                if self.tapped.is_some() {
                    return Err(ObjectStateRuntimeError::TappedStateOutsideBattlefield(
                        self.object_ref,
                    ));
                }
            }
        }
        Ok(())
    }

    fn replacement_chooser(&self) -> PlayerId {
        self.controller.unwrap_or(self.owner)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledObjectStateProgram {
    binding_id: BindingId,
    source: ObjectRef,
    program: ObjectStateClauseProgram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UntapChoice {
    Untap,
    KeepTapped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingUntapChoice {
    pub object: ObjectRef,
    pub binding_ids: Vec<BindingId>,
    pub semantic_digests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingUntapStep {
    pub id: UntapStepId,
    pub active_player: PlayerId,
    pub choices: Vec<PendingUntapChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UntapStepSnapshot {
    public: PendingUntapStep,
    controlled_permanents: BTreeMap<ObjectRef, bool>,
    choices: BTreeMap<ObjectRef, Option<UntapChoice>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntapStepResolution {
    pub id: UntapStepId,
    pub active_player: PlayerId,
    pub untapped: Vec<ObjectRef>,
    pub kept_tapped: Vec<ObjectRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReplacementPriority {
    SelfReplacement,
    ControlChangingEntry,
    CopyEntry,
    BackFaceEntry,
    General,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReplacementEffectIdentity {
    Intrinsic {
        binding_id: BindingId,
        semantic_digest: String,
    },
    External(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementCandidateEvidence {
    pub identity: ReplacementEffectIdentity,
    pub priority: ReplacementPriority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementOrderEvidence {
    pub chooser: PlayerId,
    pub applicable_effects_complete: bool,
    pub applicable: Vec<ReplacementCandidateEvidence>,
    /// `None` is valid only when the complete applicable set is empty.
    pub chosen: Option<ReplacementEffectIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalReplacementOutcome {
    /// `Some(None)` replaces the move with no zone change.
    pub destination: Option<Option<ObjectZone>>,
    pub battlefield_controller: Option<Option<PlayerId>>,
    pub enters_tapped: Option<bool>,
    /// This is required when an earlier copy or characteristic replacement
    /// removes one of the entering object's intrinsic replacement abilities.
    pub intrinsic_bindings_no_longer_applicable: BTreeSet<BindingId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementStepResolution {
    AppliedIntrinsic {
        effect: ReplacementEffectIdentity,
        destination: Option<ObjectZone>,
        enters_tapped: bool,
    },
    AppliedExternal {
        effect: ReplacementEffectIdentity,
        destination: Option<ObjectZone>,
        enters_tapped: bool,
    },
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingZoneChange {
    id: ZoneChangeEventId,
    initial_object: TrackedObject,
    destination: Option<ObjectZone>,
    battlefield_controller: Option<PlayerId>,
    enters_tapped: bool,
    applied_effects: BTreeSet<ReplacementEffectIdentity>,
    disabled_intrinsic_bindings: BTreeSet<BindingId>,
    replacement_window_complete: bool,
}

impl PendingZoneChange {
    pub fn id(&self) -> ZoneChangeEventId {
        self.id
    }

    pub fn object(&self) -> ObjectRef {
        self.initial_object.object_ref
    }

    pub fn from(&self) -> ObjectZone {
        self.initial_object.zone
    }

    pub fn destination(&self) -> Option<ObjectZone> {
        self.destination
    }

    pub fn affected_player(&self) -> PlayerId {
        self.initial_object.replacement_chooser()
    }

    pub fn battlefield_controller(&self) -> Option<PlayerId> {
        self.battlefield_controller
    }

    pub fn enters_tapped(&self) -> bool {
        self.enters_tapped
    }

    pub fn replacement_window_complete(&self) -> bool {
        self.replacement_window_complete
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneChangeCommit {
    Moved {
        event_id: ZoneChangeEventId,
        old_object: ObjectRef,
        new_object: ObjectRef,
        from: ObjectZone,
        to: ObjectZone,
        entered_tapped: Option<bool>,
    },
    ReplacedWithNoZoneChange {
        event_id: ZoneChangeEventId,
        object: ObjectRef,
        zone: ObjectZone,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectStateRuntimeError {
    EmptyExternalReplacementIdentity,
    DuplicateObject(ObjectId),
    MissingObject(ObjectId),
    MissingController(ObjectRef),
    MissingTappedState(ObjectRef),
    ControllerOutsideControlledZone(ObjectRef),
    TappedStateOutsideBattlefield(ObjectRef),
    IncarnationMismatch {
        object: ObjectId,
        expected: IncarnationId,
        actual: IncarnationId,
    },
    BindingIdExhausted,
    UntapStepIdExhausted,
    ZoneChangeEventIdExhausted,
    IncompleteBattlefieldEvidence,
    UntapStepAlreadyPending(UntapStepId),
    NoUntapStepPending,
    WrongUntapStep {
        expected: UntapStepId,
        actual: UntapStepId,
    },
    WrongUntapChooser {
        expected: PlayerId,
        actual: PlayerId,
    },
    ObjectHasNoUntapChoice(ObjectRef),
    DuplicateUntapChoice(ObjectRef),
    MissingUntapChoice(ObjectRef),
    UntapStepSnapshotChanged,
    ZoneChangeDuringUntapStep,
    NoZoneChange {
        object: ObjectRef,
        zone: ObjectZone,
    },
    MissingBattlefieldController,
    UnexpectedBattlefieldController,
    UnknownZoneChangeEvent(ZoneChangeEventId),
    IncompleteReplacementEvidence,
    WrongReplacementChooser {
        expected: PlayerId,
        actual: PlayerId,
    },
    DuplicateReplacementCandidate(ReplacementEffectIdentity),
    AppliedReplacementListedAgain(ReplacementEffectIdentity),
    MissingIntrinsicReplacementCandidate(ReplacementEffectIdentity),
    UnknownIntrinsicReplacementCandidate(ReplacementEffectIdentity),
    ReplacementChoiceMissing,
    ReplacementChoiceProvidedForEmptySet,
    ReplacementChoiceNotApplicable(ReplacementEffectIdentity),
    ReplacementPriorityViolation {
        chosen: ReplacementPriority,
        required: ReplacementPriority,
    },
    MissingExternalReplacementOutcome,
    UnexpectedExternalReplacementOutcome,
    UnknownDisabledIntrinsicBinding(BindingId),
    InvalidExternalTappedOutcome,
    ReplacementWindowAlreadyComplete,
    ReplacementWindowNotComplete,
    ZoneChangeSnapshotChanged,
    MissingNewIncarnation,
    UnexpectedNewIncarnation,
    ReusedIncarnation(IncarnationId),
}

impl fmt::Display for ObjectStateRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ObjectStateRuntimeError {}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectStateClauseRuntime {
    objects: BTreeMap<ObjectId, TrackedObject>,
    bindings: BTreeMap<BindingId, InstalledObjectStateProgram>,
    next_binding_id: BindingId,
    next_untap_step_id: UntapStepId,
    next_zone_change_event_id: ZoneChangeEventId,
    pending_untap_step: Option<UntapStepSnapshot>,
    pending_zone_changes: BTreeSet<ZoneChangeEventId>,
}

impl ObjectStateClauseRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_object(&mut self, object: TrackedObject) -> Result<(), ObjectStateRuntimeError> {
        object.validate()?;
        if self.objects.contains_key(&object.object_ref.object_id) {
            return Err(ObjectStateRuntimeError::DuplicateObject(
                object.object_ref.object_id,
            ));
        }
        self.objects.insert(object.object_ref.object_id, object);
        Ok(())
    }

    pub fn replace_object_state(
        &mut self,
        object: TrackedObject,
    ) -> Result<(), ObjectStateRuntimeError> {
        object.validate()?;
        if !self.objects.contains_key(&object.object_ref.object_id) {
            return Err(ObjectStateRuntimeError::MissingObject(
                object.object_ref.object_id,
            ));
        }
        self.objects.insert(object.object_ref.object_id, object);
        Ok(())
    }

    pub fn object(&self, object_id: ObjectId) -> Option<&TrackedObject> {
        self.objects.get(&object_id)
    }

    pub fn install_program(
        &mut self,
        source: ObjectRef,
        program: ObjectStateClauseProgram,
    ) -> Result<BindingId, ObjectStateRuntimeError> {
        self.current_object(source)?;
        let binding_id = self.next_binding_id;
        self.next_binding_id = self
            .next_binding_id
            .checked_add(1)
            .ok_or(ObjectStateRuntimeError::BindingIdExhausted)?;
        self.bindings.insert(
            binding_id,
            InstalledObjectStateProgram {
                binding_id,
                source,
                program,
            },
        );
        Ok(binding_id)
    }

    pub fn remove_binding(&mut self, binding_id: BindingId) {
        self.bindings.remove(&binding_id);
    }

    pub fn begin_untap_step(
        &mut self,
        active_player: PlayerId,
        battlefield_evidence_complete: bool,
    ) -> Result<PendingUntapStep, ObjectStateRuntimeError> {
        if !battlefield_evidence_complete {
            return Err(ObjectStateRuntimeError::IncompleteBattlefieldEvidence);
        }
        if let Some(pending) = &self.pending_untap_step {
            return Err(ObjectStateRuntimeError::UntapStepAlreadyPending(
                pending.public.id,
            ));
        }
        if !self.pending_zone_changes.is_empty() {
            return Err(ObjectStateRuntimeError::ZoneChangeDuringUntapStep);
        }

        let id = self.next_untap_step_id;
        self.next_untap_step_id = self
            .next_untap_step_id
            .checked_add(1)
            .ok_or(ObjectStateRuntimeError::UntapStepIdExhausted)?;

        let controlled_permanents = self
            .objects
            .values()
            .filter(|object| {
                object.zone == ObjectZone::Battlefield && object.controller == Some(active_player)
            })
            .map(|object| {
                object
                    .tapped
                    .map(|tapped| (object.object_ref, tapped))
                    .ok_or(ObjectStateRuntimeError::MissingTappedState(
                        object.object_ref,
                    ))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        let mut choices = BTreeMap::new();
        let mut public_choices = Vec::new();
        for (object, tapped) in &controlled_permanents {
            if !tapped {
                continue;
            }
            let mut matching = self
                .bindings
                .values()
                .filter(|binding| {
                    binding.source == *object
                        && binding.program.kind()
                            == ObjectStateClauseKind::OptionalUntapDuringYourUntapStep
                })
                .collect::<Vec<_>>();
            if matching.is_empty() {
                continue;
            }
            matching.sort_by_key(|binding| binding.binding_id);
            choices.insert(*object, None);
            public_choices.push(PendingUntapChoice {
                object: *object,
                binding_ids: matching.iter().map(|binding| binding.binding_id).collect(),
                semantic_digests: matching
                    .iter()
                    .map(|binding| binding.program.semantic_digest().to_owned())
                    .collect(),
            });
        }
        public_choices.sort_by_key(|choice| choice.object);
        let public = PendingUntapStep {
            id,
            active_player,
            choices: public_choices,
        };
        self.pending_untap_step = Some(UntapStepSnapshot {
            public: public.clone(),
            controlled_permanents,
            choices,
        });
        Ok(public)
    }

    pub fn choose_untap(
        &mut self,
        step_id: UntapStepId,
        chooser: PlayerId,
        object: ObjectRef,
        choice: UntapChoice,
    ) -> Result<(), ObjectStateRuntimeError> {
        let pending = self
            .pending_untap_step
            .as_mut()
            .ok_or(ObjectStateRuntimeError::NoUntapStepPending)?;
        if pending.public.id != step_id {
            return Err(ObjectStateRuntimeError::WrongUntapStep {
                expected: pending.public.id,
                actual: step_id,
            });
        }
        if pending.public.active_player != chooser {
            return Err(ObjectStateRuntimeError::WrongUntapChooser {
                expected: pending.public.active_player,
                actual: chooser,
            });
        }
        let decision = pending
            .choices
            .get_mut(&object)
            .ok_or(ObjectStateRuntimeError::ObjectHasNoUntapChoice(object))?;
        if decision.is_some() {
            return Err(ObjectStateRuntimeError::DuplicateUntapChoice(object));
        }
        *decision = Some(choice);
        Ok(())
    }

    pub fn resolve_untap_step(
        &mut self,
        step_id: UntapStepId,
    ) -> Result<UntapStepResolution, ObjectStateRuntimeError> {
        let pending = self
            .pending_untap_step
            .take()
            .ok_or(ObjectStateRuntimeError::NoUntapStepPending)?;
        if pending.public.id != step_id {
            self.pending_untap_step = Some(pending);
            return Err(ObjectStateRuntimeError::WrongUntapStep {
                expected: self
                    .pending_untap_step
                    .as_ref()
                    .expect("restored pending untap step")
                    .public
                    .id,
                actual: step_id,
            });
        }
        if let Some(object) = pending
            .choices
            .iter()
            .find_map(|(object, choice)| choice.is_none().then_some(*object))
        {
            self.pending_untap_step = Some(pending);
            return Err(ObjectStateRuntimeError::MissingUntapChoice(object));
        }

        let current = self
            .objects
            .values()
            .filter(|object| {
                object.zone == ObjectZone::Battlefield
                    && object.controller == Some(pending.public.active_player)
            })
            .map(|object| (object.object_ref, object.tapped))
            .collect::<Vec<_>>();
        if current.iter().any(|(_, tapped)| tapped.is_none())
            || current
                .iter()
                .map(|(object, tapped)| (*object, tapped.unwrap_or(false)))
                .collect::<BTreeMap<_, _>>()
                != pending.controlled_permanents
        {
            self.pending_untap_step = Some(pending);
            return Err(ObjectStateRuntimeError::UntapStepSnapshotChanged);
        }

        let mut untapped = Vec::new();
        let mut kept_tapped = Vec::new();
        for (object_ref, was_tapped) in &pending.controlled_permanents {
            if !was_tapped {
                continue;
            }
            let choice = pending.choices.get(object_ref).copied().flatten();
            match choice {
                Some(UntapChoice::KeepTapped) => kept_tapped.push(*object_ref),
                Some(UntapChoice::Untap) | None => untapped.push(*object_ref),
            }
        }
        for object_ref in &untapped {
            let object = self
                .objects
                .get_mut(&object_ref.object_id)
                .expect("untap snapshot object remains present");
            object.tapped = Some(false);
        }
        Ok(UntapStepResolution {
            id: pending.public.id,
            active_player: pending.public.active_player,
            untapped,
            kept_tapped,
        })
    }

    pub fn begin_zone_change(
        &mut self,
        object: ObjectRef,
        destination: ObjectZone,
        battlefield_controller: Option<PlayerId>,
    ) -> Result<PendingZoneChange, ObjectStateRuntimeError> {
        if self.pending_untap_step.is_some() {
            return Err(ObjectStateRuntimeError::ZoneChangeDuringUntapStep);
        }
        let initial_object = self.current_object(object)?.clone();
        if initial_object.zone == destination {
            return Err(ObjectStateRuntimeError::NoZoneChange {
                object,
                zone: destination,
            });
        }
        if destination == ObjectZone::Battlefield && battlefield_controller.is_none() {
            return Err(ObjectStateRuntimeError::MissingBattlefieldController);
        }
        if destination != ObjectZone::Battlefield && battlefield_controller.is_some() {
            return Err(ObjectStateRuntimeError::UnexpectedBattlefieldController);
        }

        let id = self.next_zone_change_event_id;
        self.next_zone_change_event_id = self
            .next_zone_change_event_id
            .checked_add(1)
            .ok_or(ObjectStateRuntimeError::ZoneChangeEventIdExhausted)?;
        self.pending_zone_changes.insert(id);
        Ok(PendingZoneChange {
            id,
            initial_object,
            destination: Some(destination),
            battlefield_controller,
            enters_tapped: false,
            applied_effects: BTreeSet::new(),
            disabled_intrinsic_bindings: BTreeSet::new(),
            replacement_window_complete: false,
        })
    }

    pub fn intrinsic_replacement_candidates(
        &self,
        event: &PendingZoneChange,
    ) -> Result<Vec<ReplacementCandidateEvidence>, ObjectStateRuntimeError> {
        self.validate_pending_event(event)?;
        let mut candidates = self
            .bindings
            .values()
            .filter(|binding| {
                binding.source == event.initial_object.object_ref
                    && !event
                        .disabled_intrinsic_bindings
                        .contains(&binding.binding_id)
            })
            .filter_map(|binding| {
                let applicable = match binding.program.kind() {
                    ObjectStateClauseKind::OptionalUntapDuringYourUntapStep => false,
                    ObjectStateClauseKind::SelfGraveyardMoveBecomesExile => {
                        event.destination == Some(ObjectZone::Graveyard)
                    }
                    ObjectStateClauseKind::EntersBattlefieldTapped => {
                        event.destination == Some(ObjectZone::Battlefield)
                    }
                };
                let identity = ReplacementEffectIdentity::Intrinsic {
                    binding_id: binding.binding_id,
                    semantic_digest: binding.program.semantic_digest().to_owned(),
                };
                (applicable && !event.applied_effects.contains(&identity)).then_some(
                    ReplacementCandidateEvidence {
                        identity,
                        priority: ReplacementPriority::General,
                    },
                )
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.identity.cmp(&right.identity));
        Ok(candidates)
    }

    pub fn apply_replacement_step(
        &self,
        event: &mut PendingZoneChange,
        evidence: ReplacementOrderEvidence,
        external_outcome: Option<ExternalReplacementOutcome>,
    ) -> Result<ReplacementStepResolution, ObjectStateRuntimeError> {
        self.validate_pending_event(event)?;
        if event.replacement_window_complete {
            return Err(ObjectStateRuntimeError::ReplacementWindowAlreadyComplete);
        }
        if !evidence.applicable_effects_complete {
            return Err(ObjectStateRuntimeError::IncompleteReplacementEvidence);
        }
        let expected_chooser = event.affected_player();
        if evidence.chooser != expected_chooser {
            return Err(ObjectStateRuntimeError::WrongReplacementChooser {
                expected: expected_chooser,
                actual: evidence.chooser,
            });
        }

        let mut candidates = BTreeMap::new();
        for candidate in &evidence.applicable {
            if candidates
                .insert(candidate.identity.clone(), candidate.priority)
                .is_some()
            {
                return Err(ObjectStateRuntimeError::DuplicateReplacementCandidate(
                    candidate.identity.clone(),
                ));
            }
            if event.applied_effects.contains(&candidate.identity) {
                return Err(ObjectStateRuntimeError::AppliedReplacementListedAgain(
                    candidate.identity.clone(),
                ));
            }
        }

        let intrinsic_candidates = self.intrinsic_replacement_candidates(event)?;
        let intrinsic_by_identity = intrinsic_candidates
            .iter()
            .map(|candidate| (candidate.identity.clone(), candidate.priority))
            .collect::<BTreeMap<_, _>>();
        for (identity, priority) in &intrinsic_by_identity {
            if candidates.get(identity) != Some(priority) {
                return Err(
                    ObjectStateRuntimeError::MissingIntrinsicReplacementCandidate(identity.clone()),
                );
            }
        }
        for identity in candidates.keys() {
            if let ReplacementEffectIdentity::Intrinsic { .. } = identity
                && !intrinsic_by_identity.contains_key(identity)
            {
                return Err(
                    ObjectStateRuntimeError::UnknownIntrinsicReplacementCandidate(identity.clone()),
                );
            }
        }

        if candidates.is_empty() {
            if evidence.chosen.is_some() {
                return Err(ObjectStateRuntimeError::ReplacementChoiceProvidedForEmptySet);
            }
            if external_outcome.is_some() {
                return Err(ObjectStateRuntimeError::UnexpectedExternalReplacementOutcome);
            }
            event.replacement_window_complete = true;
            return Ok(ReplacementStepResolution::Complete);
        }

        let chosen = evidence
            .chosen
            .ok_or(ObjectStateRuntimeError::ReplacementChoiceMissing)?;
        let chosen_priority = candidates.get(&chosen).copied().ok_or_else(|| {
            ObjectStateRuntimeError::ReplacementChoiceNotApplicable(chosen.clone())
        })?;
        let required_priority = candidates
            .values()
            .copied()
            .min()
            .expect("nonempty replacement candidate set");
        if chosen_priority != required_priority {
            return Err(ObjectStateRuntimeError::ReplacementPriorityViolation {
                chosen: chosen_priority,
                required: required_priority,
            });
        }

        match &chosen {
            ReplacementEffectIdentity::Intrinsic { binding_id, .. } => {
                if external_outcome.is_some() {
                    return Err(ObjectStateRuntimeError::UnexpectedExternalReplacementOutcome);
                }
                let binding = self
                    .bindings
                    .get(binding_id)
                    .expect("validated intrinsic replacement binding exists");
                match binding.program.kind() {
                    ObjectStateClauseKind::OptionalUntapDuringYourUntapStep => {
                        unreachable!("untap choices never enter replacement candidates")
                    }
                    ObjectStateClauseKind::SelfGraveyardMoveBecomesExile => {
                        event.destination = Some(ObjectZone::Exile);
                    }
                    ObjectStateClauseKind::EntersBattlefieldTapped => {
                        event.enters_tapped = true;
                    }
                }
                event.applied_effects.insert(chosen.clone());
                Ok(ReplacementStepResolution::AppliedIntrinsic {
                    effect: chosen,
                    destination: event.destination,
                    enters_tapped: event.enters_tapped,
                })
            }
            ReplacementEffectIdentity::External(external_id) => {
                if external_id.trim().is_empty() {
                    return Err(ObjectStateRuntimeError::EmptyExternalReplacementIdentity);
                }
                let outcome = external_outcome
                    .ok_or(ObjectStateRuntimeError::MissingExternalReplacementOutcome)?;
                for binding_id in &outcome.intrinsic_bindings_no_longer_applicable {
                    let Some(binding) = self.bindings.get(binding_id) else {
                        return Err(ObjectStateRuntimeError::UnknownDisabledIntrinsicBinding(
                            *binding_id,
                        ));
                    };
                    if binding.source != event.initial_object.object_ref {
                        return Err(ObjectStateRuntimeError::UnknownDisabledIntrinsicBinding(
                            *binding_id,
                        ));
                    }
                }
                if let Some(destination) = outcome.destination {
                    event.destination = destination;
                }
                if let Some(controller) = outcome.battlefield_controller {
                    event.battlefield_controller = controller;
                }
                if let Some(enters_tapped) = outcome.enters_tapped {
                    if event.destination != Some(ObjectZone::Battlefield) && enters_tapped {
                        return Err(ObjectStateRuntimeError::InvalidExternalTappedOutcome);
                    }
                    event.enters_tapped = enters_tapped;
                }
                event
                    .disabled_intrinsic_bindings
                    .extend(outcome.intrinsic_bindings_no_longer_applicable);
                event.applied_effects.insert(chosen.clone());
                Ok(ReplacementStepResolution::AppliedExternal {
                    effect: chosen,
                    destination: event.destination,
                    enters_tapped: event.enters_tapped,
                })
            }
        }
    }

    pub fn commit_zone_change(
        &mut self,
        event: PendingZoneChange,
        new_incarnation: Option<IncarnationId>,
    ) -> Result<ZoneChangeCommit, ObjectStateRuntimeError> {
        self.validate_pending_event(&event)?;
        if !event.replacement_window_complete {
            return Err(ObjectStateRuntimeError::ReplacementWindowNotComplete);
        }
        let current = self.current_object(event.initial_object.object_ref)?;
        if current != &event.initial_object {
            return Err(ObjectStateRuntimeError::ZoneChangeSnapshotChanged);
        }

        let result = match event.destination {
            None => {
                if new_incarnation.is_some() {
                    return Err(ObjectStateRuntimeError::UnexpectedNewIncarnation);
                }
                ZoneChangeCommit::ReplacedWithNoZoneChange {
                    event_id: event.id,
                    object: event.initial_object.object_ref,
                    zone: event.initial_object.zone,
                }
            }
            Some(destination) => {
                let next_incarnation =
                    new_incarnation.ok_or(ObjectStateRuntimeError::MissingNewIncarnation)?;
                if next_incarnation == event.initial_object.object_ref.incarnation_id {
                    return Err(ObjectStateRuntimeError::ReusedIncarnation(next_incarnation));
                }
                if destination == ObjectZone::Battlefield && event.battlefield_controller.is_none()
                {
                    return Err(ObjectStateRuntimeError::MissingBattlefieldController);
                }
                if destination != ObjectZone::Battlefield && event.battlefield_controller.is_some()
                {
                    return Err(ObjectStateRuntimeError::UnexpectedBattlefieldController);
                }
                let new_ref = ObjectRef {
                    object_id: event.initial_object.object_ref.object_id,
                    incarnation_id: next_incarnation,
                };
                let entered_tapped =
                    (destination == ObjectZone::Battlefield).then_some(event.enters_tapped);
                let replacement = TrackedObject {
                    object_ref: new_ref,
                    owner: event.initial_object.owner,
                    controller: event.battlefield_controller,
                    zone: destination,
                    tapped: entered_tapped,
                };
                replacement.validate()?;
                self.objects.insert(new_ref.object_id, replacement);
                ZoneChangeCommit::Moved {
                    event_id: event.id,
                    old_object: event.initial_object.object_ref,
                    new_object: new_ref,
                    from: event.initial_object.zone,
                    to: destination,
                    entered_tapped,
                }
            }
        };
        self.pending_zone_changes.remove(&event.id);
        Ok(result)
    }

    fn current_object(&self, object: ObjectRef) -> Result<&TrackedObject, ObjectStateRuntimeError> {
        let state = self
            .objects
            .get(&object.object_id)
            .ok_or(ObjectStateRuntimeError::MissingObject(object.object_id))?;
        if state.object_ref.incarnation_id != object.incarnation_id {
            return Err(ObjectStateRuntimeError::IncarnationMismatch {
                object: object.object_id,
                expected: object.incarnation_id,
                actual: state.object_ref.incarnation_id,
            });
        }
        Ok(state)
    }

    fn validate_pending_event(
        &self,
        event: &PendingZoneChange,
    ) -> Result<(), ObjectStateRuntimeError> {
        if !self.pending_zone_changes.contains(&event.id) {
            return Err(ObjectStateRuntimeError::UnknownZoneChangeEvent(event.id));
        }
        let current = self.current_object(event.initial_object.object_ref)?;
        if current != &event.initial_object {
            return Err(ObjectStateRuntimeError::ZoneChangeSnapshotChanged);
        }
        Ok(())
    }
}
