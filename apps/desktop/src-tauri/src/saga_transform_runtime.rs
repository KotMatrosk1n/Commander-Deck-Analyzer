//! Content keyed execution for the exact final chapter used by transforming Sagas.
//!
//! The program is intentionally not connected to production coverage yet. It
//! models the complete chapter trigger and resolution transaction so a later
//! bridge can bind it without treating syntax recognition as live execution.

use std::collections::BTreeMap;
use std::fmt;

use sha2::{Digest, Sha256};

pub const SAGA_TRANSFORM_COMPILER_VERSION: &str = "saga-transform-compiler-0.1";
pub const SAGA_TRANSFORM_RUNTIME_VERSION: &str = "saga-transform-runtime-0.1";
pub const SAGA_TRANSFORM_RULES_CONTEXT_VERSION: &str = "saga-transform-rules-context-0.1";

const EXACT_FINAL_CHAPTER: &str = concat!(
    "III \u{2014} Exile this Saga, then return it to the battlefield transformed ",
    "under your control."
);
const NORMALIZED_FINAL_CHAPTER: &str = concat!(
    "III \u{2014} Exile this Saga, then return it to the battlefield transformed ",
    "under your control."
);

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type TriggerId = u64;
pub type ReplacementEffectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SagaTransformLayoutKind {
    TransformingDoubleFaced,
    ModalDoubleFaced,
    Reversible,
    SingleFaced,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SagaTransformFaceRole {
    Front,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SagaTransformSourceContext {
    pub layout: SagaTransformLayoutKind,
    pub face_count: u8,
    pub source_face: SagaTransformFaceRole,
    pub source_face_is_enchantment: bool,
    pub source_face_is_saga: bool,
    pub source_face_is_permanent: bool,
    pub transformed_face: SagaTransformFaceRole,
    pub transformed_face_is_permanent: bool,
    pub transformed_face_is_instant_or_sorcery: bool,
}

impl SagaTransformSourceContext {
    pub const fn exact_transforming_saga() -> Self {
        Self {
            layout: SagaTransformLayoutKind::TransformingDoubleFaced,
            face_count: 2,
            source_face: SagaTransformFaceRole::Front,
            source_face_is_enchantment: true,
            source_face_is_saga: true,
            source_face_is_permanent: true,
            transformed_face: SagaTransformFaceRole::Back,
            transformed_face_is_permanent: true,
            transformed_face_is_instant_or_sorcery: false,
        }
    }

    pub const fn is_complete(self) -> bool {
        matches!(
            self.layout,
            SagaTransformLayoutKind::TransformingDoubleFaced
        ) && self.face_count == 2
            && matches!(self.source_face, SagaTransformFaceRole::Front)
            && self.source_face_is_enchantment
            && self.source_face_is_saga
            && self.source_face_is_permanent
            && matches!(self.transformed_face, SagaTransformFaceRole::Back)
            && self.transformed_face_is_permanent
            && !self.transformed_face_is_instant_or_sorcery
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaTransformProgram {
    exact_source: String,
    normalized_source: String,
    source_context: SagaTransformSourceContext,
    source_context_digest: String,
    semantic_digest: String,
}

impl SagaTransformProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub const fn source_context(&self) -> SagaTransformSourceContext {
        self.source_context
    }

    pub fn source_context_digest(&self) -> &str {
        &self.source_context_digest
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }
}

pub fn compile_saga_transform_program(
    exact_source: &str,
    normalized_source: &str,
    source_context: SagaTransformSourceContext,
) -> Option<SagaTransformProgram> {
    if exact_source != EXACT_FINAL_CHAPTER
        || normalized_source != NORMALIZED_FINAL_CHAPTER
        || !source_context.is_complete()
    {
        return None;
    }
    let source_context_digest = saga_transform_source_context_digest(source_context);
    let semantic_digest = saga_transform_semantic_digest(
        exact_source,
        normalized_source,
        source_context,
        &source_context_digest,
    );
    Some(SagaTransformProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        source_context,
        source_context_digest,
        semantic_digest,
    })
}

fn saga_transform_source_context_digest(context: SagaTransformSourceContext) -> String {
    content_digest(&[
        "saga-transform-source-context/v1",
        SAGA_TRANSFORM_RULES_CONTEXT_VERSION,
        match context.layout {
            SagaTransformLayoutKind::TransformingDoubleFaced => "layout:transform",
            SagaTransformLayoutKind::ModalDoubleFaced => "layout:modal-double-faced",
            SagaTransformLayoutKind::Reversible => "layout:reversible",
            SagaTransformLayoutKind::SingleFaced => "layout:single-faced",
            SagaTransformLayoutKind::Other => "layout:other",
        },
        match context.source_face {
            SagaTransformFaceRole::Front => "source-face:front",
            SagaTransformFaceRole::Back => "source-face:back",
        },
        if context.source_face_is_enchantment {
            "source-enchantment:true"
        } else {
            "source-enchantment:false"
        },
        if context.source_face_is_saga {
            "source-saga:true"
        } else {
            "source-saga:false"
        },
        if context.source_face_is_permanent {
            "source-permanent:true"
        } else {
            "source-permanent:false"
        },
        match context.transformed_face {
            SagaTransformFaceRole::Front => "transformed-face:front",
            SagaTransformFaceRole::Back => "transformed-face:back",
        },
        if context.transformed_face_is_permanent {
            "transformed-permanent:true"
        } else {
            "transformed-permanent:false"
        },
        if context.transformed_face_is_instant_or_sorcery {
            "transformed-instant-or-sorcery:true"
        } else {
            "transformed-instant-or-sorcery:false"
        },
        match context.face_count {
            2 => "face-count:2",
            _ => "face-count:other",
        },
    ])
}

fn saga_transform_semantic_digest(
    exact_source: &str,
    normalized_source: &str,
    context: SagaTransformSourceContext,
    source_context_digest: &str,
) -> String {
    content_digest(&[
        "saga-transform-program/v1",
        SAGA_TRANSFORM_COMPILER_VERSION,
        SAGA_TRANSFORM_RUNTIME_VERSION,
        SAGA_TRANSFORM_RULES_CONTEXT_VERSION,
        exact_source,
        normalized_source,
        source_context_digest,
        match context.layout {
            SagaTransformLayoutKind::TransformingDoubleFaced => "layout:transform",
            SagaTransformLayoutKind::ModalDoubleFaced => "layout:modal-double-faced",
            SagaTransformLayoutKind::Reversible => "layout:reversible",
            SagaTransformLayoutKind::SingleFaced => "layout:single-faced",
            SagaTransformLayoutKind::Other => "layout:other",
        },
        "chapter:3",
        "trigger:one-or-more-lore-counters-cross-chapter-three",
        "targeting:none",
        "reference:same-source-object-and-incarnation",
        "resolution:exile-then-return-expected-exile-object",
        "zone-change:new-incarnation-per-move",
        "return:back-face-permanent",
        "return-controller:resolving-trigger-controller",
        "return:event-enters-the-battlefield",
        "token:cannot-return-after-leaving-battlefield",
        "replacement:return-requires-the-expected-exile-object",
    ])
}

fn content_digest(components: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for component in components {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SagaTransformZone {
    Battlefield,
    Exile,
    Command,
    Graveyard,
    Hand,
    Library,
    OutsideGame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaTransformObject {
    pub id: ObjectId,
    pub incarnation: IncarnationId,
    pub owner: PlayerId,
    pub controller: Option<PlayerId>,
    pub zone: SagaTransformZone,
    pub active_face: SagaTransformFaceRole,
    pub is_token: bool,
    pub lore_counters: u32,
    pub source_context: SagaTransformSourceContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledSagaTransformProgram {
    source_incarnation: IncarnationId,
    program: SagaTransformProgram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoreChapterTriggerEvidence {
    pub source: ObjectId,
    pub source_incarnation: IncarnationId,
    pub chapter: u8,
    pub lore_counters_before: u32,
    pub lore_counters_after: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SagaTransformTargeting {
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSagaTransformTrigger {
    pub id: TriggerId,
    pub controller: PlayerId,
    pub source: ObjectId,
    pub source_incarnation: IncarnationId,
    pub program_semantic_digest: String,
    pub source_context_digest: String,
    pub chapter_evidence: LoreChapterTriggerEvidence,
    pub targeting: SagaTransformTargeting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SagaTransformMovementReplacement {
    None,
    Prevented {
        replacement_effect: ReplacementEffectId,
    },
    DestinationChanged {
        replacement_effect: ReplacementEffectId,
        destination: SagaTransformZone,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SagaTransformNoEffectReason {
    SourceNoLongerExists,
    SourceIsNewIncarnation,
    SourceNotOnBattlefield,
    SourceNotOnFrontFace,
    SourceLayoutNoLongerMatchesProgram,
    ExileMovementPrevented,
    ExpectedExileObjectMissing { actual_zone: SagaTransformZone },
    TokenCannotReturn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SagaTransformOutcome {
    ReturnedTransformed {
        object: ObjectId,
        battlefield_incarnation: IncarnationId,
    },
    NoEffect(SagaTransformNoEffectReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SagaTransformResolutionPhase {
    TriggerResolution,
    StateBasedActions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaTransformReceipt {
    pub order: u16,
    pub phase: SagaTransformResolutionPhase,
    pub event: SagaTransformReceiptEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SagaTransformReceiptEvent {
    TriggerResolutionBegan {
        trigger: TriggerId,
        source: ObjectId,
        source_incarnation: IncarnationId,
        controller: PlayerId,
    },
    SourceReferenceFailed {
        reason: SagaTransformNoEffectReason,
    },
    ExileMovementReplaced {
        replacement_effect: ReplacementEffectId,
        actual_destination: Option<SagaTransformZone>,
    },
    ZoneChanged {
        object: ObjectId,
        from: SagaTransformZone,
        to: SagaTransformZone,
        old_incarnation: IncarnationId,
        new_incarnation: IncarnationId,
    },
    ExpectedExileObjectMissing {
        actual_zone: SagaTransformZone,
        actual_incarnation: IncarnationId,
    },
    TokenReturnProhibited {
        object: ObjectId,
        exile_incarnation: IncarnationId,
    },
    ReturnedTransformed {
        object: ObjectId,
        exile_incarnation: IncarnationId,
        battlefield_incarnation: IncarnationId,
        face: SagaTransformFaceRole,
    },
    ControllerSetByResolvingEffect {
        object: ObjectId,
        incarnation: IncarnationId,
        controller: PlayerId,
    },
    EnteredBattlefield {
        object: ObjectId,
        incarnation: IncarnationId,
        controller: PlayerId,
        face: SagaTransformFaceRole,
    },
    TriggerResolutionCompleted {
        outcome: SagaTransformOutcome,
    },
    TokenCeasedToExist {
        object: ObjectId,
        last_incarnation: IncarnationId,
        last_zone: SagaTransformZone,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaTransformResolution {
    pub trigger: PendingSagaTransformTrigger,
    pub outcome: SagaTransformOutcome,
    pub receipts: Vec<SagaTransformReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SagaTransformError {
    DuplicateObject(ObjectId),
    MissingObject(ObjectId),
    IncarnationMismatch {
        object: ObjectId,
        expected: IncarnationId,
        actual: IncarnationId,
    },
    InvalidSourceLayout,
    InvalidSourceZone,
    InvalidSourceFace,
    MissingController,
    ProgramNotInstalled,
    DuplicateProgram {
        object: ObjectId,
        incarnation: IncarnationId,
    },
    InvalidLoreCounterAddition,
    LoreCounterOverflow,
    UnknownTrigger(TriggerId),
    InvalidMovementReplacement,
    IncarnationIdExhausted,
    TriggerIdExhausted,
    ReceiptOrderExhausted,
}

impl fmt::Display for SagaTransformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SagaTransformError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaTransformRuntime {
    objects: BTreeMap<ObjectId, SagaTransformObject>,
    programs: BTreeMap<ObjectId, InstalledSagaTransformProgram>,
    pending: BTreeMap<TriggerId, PendingSagaTransformTrigger>,
    next_incarnation_id: IncarnationId,
    next_trigger_id: TriggerId,
}

impl Default for SagaTransformRuntime {
    fn default() -> Self {
        Self {
            objects: BTreeMap::new(),
            programs: BTreeMap::new(),
            pending: BTreeMap::new(),
            next_incarnation_id: 1,
            next_trigger_id: 1,
        }
    }
}

impl SagaTransformRuntime {
    pub fn insert_object(&mut self, object: SagaTransformObject) -> Result<(), SagaTransformError> {
        validate_object(&object)?;
        if self.objects.contains_key(&object.id) {
            return Err(SagaTransformError::DuplicateObject(object.id));
        }
        self.next_incarnation_id = self.next_incarnation_id.max(
            object
                .incarnation
                .checked_add(1)
                .unwrap_or(IncarnationId::MAX),
        );
        self.objects.insert(object.id, object);
        Ok(())
    }

    pub fn object(&self, id: ObjectId) -> Option<&SagaTransformObject> {
        self.objects.get(&id)
    }

    pub fn pending_trigger(&self, id: TriggerId) -> Option<&PendingSagaTransformTrigger> {
        self.pending.get(&id)
    }

    pub fn change_controller(
        &mut self,
        object: ObjectId,
        incarnation: IncarnationId,
        controller: PlayerId,
    ) -> Result<(), SagaTransformError> {
        let state = self
            .objects
            .get_mut(&object)
            .ok_or(SagaTransformError::MissingObject(object))?;
        require_incarnation(state, incarnation)?;
        if state.zone != SagaTransformZone::Battlefield {
            return Err(SagaTransformError::InvalidSourceZone);
        }
        state.controller = Some(controller);
        Ok(())
    }

    pub fn install_program(
        &mut self,
        object: ObjectId,
        incarnation: IncarnationId,
        program: SagaTransformProgram,
    ) -> Result<(), SagaTransformError> {
        let state = self
            .objects
            .get(&object)
            .ok_or(SagaTransformError::MissingObject(object))?;
        require_incarnation(state, incarnation)?;
        if state.source_context != program.source_context()
            || !state.source_context.is_complete()
            || state.active_face != SagaTransformFaceRole::Front
        {
            return Err(SagaTransformError::InvalidSourceLayout);
        }
        if self.programs.get(&object).is_some_and(|installed| {
            installed.source_incarnation == incarnation
                && installed.program.semantic_digest() == program.semantic_digest()
        }) {
            return Err(SagaTransformError::DuplicateProgram {
                object,
                incarnation,
            });
        }
        self.programs.insert(
            object,
            InstalledSagaTransformProgram {
                source_incarnation: incarnation,
                program,
            },
        );
        Ok(())
    }

    pub fn put_lore_counters(
        &mut self,
        object: ObjectId,
        incarnation: IncarnationId,
        amount: u32,
    ) -> Result<Option<PendingSagaTransformTrigger>, SagaTransformError> {
        if amount == 0 {
            return Err(SagaTransformError::InvalidLoreCounterAddition);
        }
        let mut next = self.clone();
        let trigger = next.put_lore_counters_in_place(object, incarnation, amount)?;
        *self = next;
        Ok(trigger)
    }

    fn put_lore_counters_in_place(
        &mut self,
        object: ObjectId,
        incarnation: IncarnationId,
        amount: u32,
    ) -> Result<Option<PendingSagaTransformTrigger>, SagaTransformError> {
        let installed = self
            .programs
            .get(&object)
            .filter(|installed| installed.source_incarnation == incarnation)
            .cloned()
            .ok_or(SagaTransformError::ProgramNotInstalled)?;
        let state = self
            .objects
            .get_mut(&object)
            .ok_or(SagaTransformError::MissingObject(object))?;
        require_incarnation(state, incarnation)?;
        if state.zone != SagaTransformZone::Battlefield {
            return Err(SagaTransformError::InvalidSourceZone);
        }
        if state.active_face != SagaTransformFaceRole::Front {
            return Err(SagaTransformError::InvalidSourceFace);
        }
        if state.source_context != installed.program.source_context()
            || !state.source_context.is_complete()
        {
            return Err(SagaTransformError::InvalidSourceLayout);
        }
        let controller = state
            .controller
            .ok_or(SagaTransformError::MissingController)?;
        let before = state.lore_counters;
        let after = before
            .checked_add(amount)
            .ok_or(SagaTransformError::LoreCounterOverflow)?;
        state.lore_counters = after;
        if before >= 3 || after < 3 {
            return Ok(None);
        }
        let id = self.next_trigger_id;
        self.next_trigger_id = self
            .next_trigger_id
            .checked_add(1)
            .ok_or(SagaTransformError::TriggerIdExhausted)?;
        let trigger = PendingSagaTransformTrigger {
            id,
            controller,
            source: object,
            source_incarnation: incarnation,
            program_semantic_digest: installed.program.semantic_digest().to_owned(),
            source_context_digest: installed.program.source_context_digest().to_owned(),
            chapter_evidence: LoreChapterTriggerEvidence {
                source: object,
                source_incarnation: incarnation,
                chapter: 3,
                lore_counters_before: before,
                lore_counters_after: after,
            },
            targeting: SagaTransformTargeting::None,
        };
        self.pending.insert(id, trigger.clone());
        Ok(Some(trigger))
    }

    pub fn move_object_for_external_effect(
        &mut self,
        object: ObjectId,
        incarnation: IncarnationId,
        destination: SagaTransformZone,
        controller: Option<PlayerId>,
    ) -> Result<IncarnationId, SagaTransformError> {
        let mut next = self.clone();
        let new_incarnation = next.move_object_for_external_effect_in_place(
            object,
            incarnation,
            destination,
            controller,
        )?;
        *self = next;
        Ok(new_incarnation)
    }

    fn move_object_for_external_effect_in_place(
        &mut self,
        object: ObjectId,
        incarnation: IncarnationId,
        destination: SagaTransformZone,
        controller: Option<PlayerId>,
    ) -> Result<IncarnationId, SagaTransformError> {
        let current = self
            .objects
            .get(&object)
            .ok_or(SagaTransformError::MissingObject(object))?;
        require_incarnation(current, incarnation)?;
        if current.zone == destination {
            return Ok(incarnation);
        }
        let new_incarnation = self.take_incarnation_id()?;
        let state = self
            .objects
            .get_mut(&object)
            .ok_or(SagaTransformError::MissingObject(object))?;
        state.incarnation = new_incarnation;
        state.zone = destination;
        state.active_face = SagaTransformFaceRole::Front;
        state.controller = (destination == SagaTransformZone::Battlefield)
            .then_some(controller)
            .flatten();
        state.lore_counters = 0;
        Ok(new_incarnation)
    }

    pub fn resolve(
        &mut self,
        trigger: TriggerId,
        replacement: SagaTransformMovementReplacement,
    ) -> Result<SagaTransformResolution, SagaTransformError> {
        validate_replacement(replacement)?;
        let mut next = self.clone();
        let resolution = next.resolve_in_place(trigger, replacement)?;
        *self = next;
        Ok(resolution)
    }

    fn resolve_in_place(
        &mut self,
        trigger_id: TriggerId,
        replacement: SagaTransformMovementReplacement,
    ) -> Result<SagaTransformResolution, SagaTransformError> {
        let trigger = self
            .pending
            .remove(&trigger_id)
            .ok_or(SagaTransformError::UnknownTrigger(trigger_id))?;
        let mut receipts = Vec::new();
        push_receipt(
            &mut receipts,
            SagaTransformResolutionPhase::TriggerResolution,
            SagaTransformReceiptEvent::TriggerResolutionBegan {
                trigger: trigger.id,
                source: trigger.source,
                source_incarnation: trigger.source_incarnation,
                controller: trigger.controller,
            },
        )?;

        let source_check = self.check_resolving_source(&trigger);
        if let Err(reason) = source_check {
            push_receipt(
                &mut receipts,
                SagaTransformResolutionPhase::TriggerResolution,
                SagaTransformReceiptEvent::SourceReferenceFailed { reason },
            )?;
            let outcome = SagaTransformOutcome::NoEffect(reason);
            push_resolution_completed(&mut receipts, outcome)?;
            return Ok(SagaTransformResolution {
                trigger,
                outcome,
                receipts,
            });
        }

        let source_before = self
            .objects
            .get(&trigger.source)
            .cloned()
            .ok_or(SagaTransformError::MissingObject(trigger.source))?;
        let exile_incarnation = match replacement {
            SagaTransformMovementReplacement::None => {
                let new_incarnation = self.take_incarnation_id()?;
                self.move_same_object_to_new_zone(
                    trigger.source,
                    trigger.source_incarnation,
                    new_incarnation,
                    SagaTransformZone::Exile,
                )?;
                push_zone_change(
                    &mut receipts,
                    trigger.source,
                    SagaTransformZone::Battlefield,
                    SagaTransformZone::Exile,
                    trigger.source_incarnation,
                    new_incarnation,
                )?;
                Some(new_incarnation)
            }
            SagaTransformMovementReplacement::Prevented { replacement_effect } => {
                push_receipt(
                    &mut receipts,
                    SagaTransformResolutionPhase::TriggerResolution,
                    SagaTransformReceiptEvent::ExileMovementReplaced {
                        replacement_effect,
                        actual_destination: None,
                    },
                )?;
                None
            }
            SagaTransformMovementReplacement::DestinationChanged {
                replacement_effect,
                destination,
            } => {
                push_receipt(
                    &mut receipts,
                    SagaTransformResolutionPhase::TriggerResolution,
                    SagaTransformReceiptEvent::ExileMovementReplaced {
                        replacement_effect,
                        actual_destination: Some(destination),
                    },
                )?;
                let new_incarnation = self.take_incarnation_id()?;
                self.move_same_object_to_new_zone(
                    trigger.source,
                    trigger.source_incarnation,
                    new_incarnation,
                    destination,
                )?;
                push_zone_change(
                    &mut receipts,
                    trigger.source,
                    SagaTransformZone::Battlefield,
                    destination,
                    trigger.source_incarnation,
                    new_incarnation,
                )?;
                None
            }
        };

        let outcome = match replacement {
            SagaTransformMovementReplacement::Prevented { .. } => {
                SagaTransformOutcome::NoEffect(SagaTransformNoEffectReason::ExileMovementPrevented)
            }
            SagaTransformMovementReplacement::DestinationChanged { destination, .. } => {
                let actual = self
                    .objects
                    .get(&trigger.source)
                    .ok_or(SagaTransformError::MissingObject(trigger.source))?;
                push_receipt(
                    &mut receipts,
                    SagaTransformResolutionPhase::TriggerResolution,
                    SagaTransformReceiptEvent::ExpectedExileObjectMissing {
                        actual_zone: actual.zone,
                        actual_incarnation: actual.incarnation,
                    },
                )?;
                SagaTransformOutcome::NoEffect(
                    SagaTransformNoEffectReason::ExpectedExileObjectMissing {
                        actual_zone: destination,
                    },
                )
            }
            SagaTransformMovementReplacement::None if source_before.is_token => {
                let exile_incarnation =
                    exile_incarnation.ok_or(SagaTransformError::IncarnationIdExhausted)?;
                push_receipt(
                    &mut receipts,
                    SagaTransformResolutionPhase::TriggerResolution,
                    SagaTransformReceiptEvent::TokenReturnProhibited {
                        object: trigger.source,
                        exile_incarnation,
                    },
                )?;
                SagaTransformOutcome::NoEffect(SagaTransformNoEffectReason::TokenCannotReturn)
            }
            SagaTransformMovementReplacement::None => {
                let exile_incarnation =
                    exile_incarnation.ok_or(SagaTransformError::IncarnationIdExhausted)?;
                let battlefield_incarnation = self.take_incarnation_id()?;
                self.return_transformed(&trigger, exile_incarnation, battlefield_incarnation)?;
                push_zone_change(
                    &mut receipts,
                    trigger.source,
                    SagaTransformZone::Exile,
                    SagaTransformZone::Battlefield,
                    exile_incarnation,
                    battlefield_incarnation,
                )?;
                push_receipt(
                    &mut receipts,
                    SagaTransformResolutionPhase::TriggerResolution,
                    SagaTransformReceiptEvent::ReturnedTransformed {
                        object: trigger.source,
                        exile_incarnation,
                        battlefield_incarnation,
                        face: SagaTransformFaceRole::Back,
                    },
                )?;
                push_receipt(
                    &mut receipts,
                    SagaTransformResolutionPhase::TriggerResolution,
                    SagaTransformReceiptEvent::ControllerSetByResolvingEffect {
                        object: trigger.source,
                        incarnation: battlefield_incarnation,
                        controller: trigger.controller,
                    },
                )?;
                push_receipt(
                    &mut receipts,
                    SagaTransformResolutionPhase::TriggerResolution,
                    SagaTransformReceiptEvent::EnteredBattlefield {
                        object: trigger.source,
                        incarnation: battlefield_incarnation,
                        controller: trigger.controller,
                        face: SagaTransformFaceRole::Back,
                    },
                )?;
                SagaTransformOutcome::ReturnedTransformed {
                    object: trigger.source,
                    battlefield_incarnation,
                }
            }
        };
        push_resolution_completed(&mut receipts, outcome)?;

        if self
            .objects
            .get(&trigger.source)
            .is_some_and(|object| object.is_token && object.zone != SagaTransformZone::Battlefield)
        {
            let token = self
                .objects
                .remove(&trigger.source)
                .ok_or(SagaTransformError::MissingObject(trigger.source))?;
            push_receipt(
                &mut receipts,
                SagaTransformResolutionPhase::StateBasedActions,
                SagaTransformReceiptEvent::TokenCeasedToExist {
                    object: token.id,
                    last_incarnation: token.incarnation,
                    last_zone: token.zone,
                },
            )?;
        }

        Ok(SagaTransformResolution {
            trigger,
            outcome,
            receipts,
        })
    }

    fn check_resolving_source(
        &self,
        trigger: &PendingSagaTransformTrigger,
    ) -> Result<(), SagaTransformNoEffectReason> {
        let Some(object) = self.objects.get(&trigger.source) else {
            return Err(SagaTransformNoEffectReason::SourceNoLongerExists);
        };
        if object.incarnation != trigger.source_incarnation {
            return Err(SagaTransformNoEffectReason::SourceIsNewIncarnation);
        }
        if object.zone != SagaTransformZone::Battlefield {
            return Err(SagaTransformNoEffectReason::SourceNotOnBattlefield);
        }
        if object.active_face != SagaTransformFaceRole::Front {
            return Err(SagaTransformNoEffectReason::SourceNotOnFrontFace);
        }
        let Some(installed) = self.programs.get(&trigger.source) else {
            return Err(SagaTransformNoEffectReason::SourceLayoutNoLongerMatchesProgram);
        };
        if installed.source_incarnation != trigger.source_incarnation
            || installed.program.semantic_digest() != trigger.program_semantic_digest
            || installed.program.source_context_digest() != trigger.source_context_digest
            || object.source_context != installed.program.source_context()
            || !object.source_context.is_complete()
        {
            return Err(SagaTransformNoEffectReason::SourceLayoutNoLongerMatchesProgram);
        }
        Ok(())
    }

    fn move_same_object_to_new_zone(
        &mut self,
        object: ObjectId,
        old_incarnation: IncarnationId,
        new_incarnation: IncarnationId,
        destination: SagaTransformZone,
    ) -> Result<(), SagaTransformError> {
        let state = self
            .objects
            .get_mut(&object)
            .ok_or(SagaTransformError::MissingObject(object))?;
        require_incarnation(state, old_incarnation)?;
        state.incarnation = new_incarnation;
        state.zone = destination;
        state.active_face = SagaTransformFaceRole::Front;
        state.controller = None;
        state.lore_counters = 0;
        Ok(())
    }

    fn return_transformed(
        &mut self,
        trigger: &PendingSagaTransformTrigger,
        exile_incarnation: IncarnationId,
        battlefield_incarnation: IncarnationId,
    ) -> Result<(), SagaTransformError> {
        let state = self
            .objects
            .get_mut(&trigger.source)
            .ok_or(SagaTransformError::MissingObject(trigger.source))?;
        require_incarnation(state, exile_incarnation)?;
        if state.zone != SagaTransformZone::Exile
            || state.is_token
            || !state.source_context.is_complete()
        {
            return Err(SagaTransformError::InvalidSourceLayout);
        }
        state.incarnation = battlefield_incarnation;
        state.zone = SagaTransformZone::Battlefield;
        state.active_face = SagaTransformFaceRole::Back;
        state.controller = Some(trigger.controller);
        state.lore_counters = 0;
        Ok(())
    }

    fn take_incarnation_id(&mut self) -> Result<IncarnationId, SagaTransformError> {
        let id = self.next_incarnation_id;
        self.next_incarnation_id = self
            .next_incarnation_id
            .checked_add(1)
            .ok_or(SagaTransformError::IncarnationIdExhausted)?;
        Ok(id)
    }
}

fn validate_object(object: &SagaTransformObject) -> Result<(), SagaTransformError> {
    if !object.source_context.is_complete() {
        return Err(SagaTransformError::InvalidSourceLayout);
    }
    match object.zone {
        SagaTransformZone::Battlefield => {
            if object.controller.is_none() {
                return Err(SagaTransformError::MissingController);
            }
        }
        _ if object.controller.is_some() => return Err(SagaTransformError::InvalidSourceZone),
        _ => {}
    }
    if object.active_face == SagaTransformFaceRole::Back
        && object.zone != SagaTransformZone::Battlefield
    {
        return Err(SagaTransformError::InvalidSourceFace);
    }
    Ok(())
}

fn require_incarnation(
    object: &SagaTransformObject,
    expected: IncarnationId,
) -> Result<(), SagaTransformError> {
    if object.incarnation != expected {
        return Err(SagaTransformError::IncarnationMismatch {
            object: object.id,
            expected,
            actual: object.incarnation,
        });
    }
    Ok(())
}

fn validate_replacement(
    replacement: SagaTransformMovementReplacement,
) -> Result<(), SagaTransformError> {
    match replacement {
        SagaTransformMovementReplacement::None
        | SagaTransformMovementReplacement::Prevented { .. } => Ok(()),
        SagaTransformMovementReplacement::DestinationChanged {
            destination:
                SagaTransformZone::Command
                | SagaTransformZone::Graveyard
                | SagaTransformZone::Hand
                | SagaTransformZone::Library
                | SagaTransformZone::OutsideGame,
            ..
        } => Ok(()),
        SagaTransformMovementReplacement::DestinationChanged { .. } => {
            Err(SagaTransformError::InvalidMovementReplacement)
        }
    }
}

fn push_zone_change(
    receipts: &mut Vec<SagaTransformReceipt>,
    object: ObjectId,
    from: SagaTransformZone,
    to: SagaTransformZone,
    old_incarnation: IncarnationId,
    new_incarnation: IncarnationId,
) -> Result<(), SagaTransformError> {
    push_receipt(
        receipts,
        SagaTransformResolutionPhase::TriggerResolution,
        SagaTransformReceiptEvent::ZoneChanged {
            object,
            from,
            to,
            old_incarnation,
            new_incarnation,
        },
    )
}

fn push_resolution_completed(
    receipts: &mut Vec<SagaTransformReceipt>,
    outcome: SagaTransformOutcome,
) -> Result<(), SagaTransformError> {
    push_receipt(
        receipts,
        SagaTransformResolutionPhase::TriggerResolution,
        SagaTransformReceiptEvent::TriggerResolutionCompleted { outcome },
    )
}

fn push_receipt(
    receipts: &mut Vec<SagaTransformReceipt>,
    phase: SagaTransformResolutionPhase,
    event: SagaTransformReceiptEvent,
) -> Result<(), SagaTransformError> {
    let order =
        u16::try_from(receipts.len()).map_err(|_| SagaTransformError::ReceiptOrderExhausted)?;
    receipts.push(SagaTransformReceipt {
        order,
        phase,
        event,
    });
    Ok(())
}
