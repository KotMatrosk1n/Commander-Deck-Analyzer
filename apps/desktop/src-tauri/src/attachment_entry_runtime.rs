//! Exact, content keyed programs for two attachment entry abilities.
//!
//! This module is intentionally not connected to production coverage. It owns
//! a standalone trigger and resolution model so the compiler can retain exact
//! meaning without claiming that the main simulator executes it yet.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const ATTACHMENT_ENTRY_COMPILER_VERSION: &str = "attachment-entry-compiler-0.1";
pub const ATTACHMENT_ENTRY_RUNTIME_VERSION: &str = "attachment-entry-runtime-0.1";

pub const AURA_TAP_ENTRY_ORACLE: &str = "When this Aura enters, tap enchanted creature.";
pub const EQUIPMENT_ATTACH_ENTRY_ORACLE: &str =
    "When this Equipment enters, attach it to target creature you control.";

const AURA_TAP_NORMALIZED: &str = "when this aura enters, tap enchanted creature.";
const EQUIPMENT_ATTACH_NORMALIZED: &str =
    "when this equipment enters, attach it to target creature you control.";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type EntryEventId = u64;
pub type TriggerId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttachmentEntryProgramKind {
    TapEnchantedCreature,
    AttachToTargetCreatureYouControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttachmentSourceContext {
    AuraPermanent,
    EquipmentPermanent,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttachmentEntrySemanticIdentity {
    exact_oracle: String,
    normalized_oracle: String,
    source_context: AttachmentSourceContext,
    compiler_version: &'static str,
    runtime_version: &'static str,
    rules_context: &'static str,
}

impl AttachmentEntrySemanticIdentity {
    pub fn exact_oracle(&self) -> &str {
        &self.exact_oracle
    }

    pub fn normalized_oracle(&self) -> &str {
        &self.normalized_oracle
    }

    pub fn source_context(&self) -> AttachmentSourceContext {
        self.source_context
    }

    pub fn compiler_version(&self) -> &'static str {
        self.compiler_version
    }

    pub fn runtime_version(&self) -> &'static str {
        self.runtime_version
    }

    pub fn rules_context(&self) -> &'static str {
        self.rules_context
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentEntryProgram {
    kind: AttachmentEntryProgramKind,
    identity: AttachmentEntrySemanticIdentity,
    semantic_digest: String,
}

impl AttachmentEntryProgram {
    pub fn kind(&self) -> AttachmentEntryProgramKind {
        self.kind
    }

    pub fn identity(&self) -> &AttachmentEntrySemanticIdentity {
        &self.identity
    }

    pub fn exact_source(&self) -> &str {
        self.identity.exact_oracle()
    }

    pub fn normalized_source(&self) -> &str {
        self.identity.normalized_oracle()
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }
}

pub fn compile_attachment_entry_program(
    exact_oracle: &str,
    source_type_line: &str,
) -> Option<AttachmentEntryProgram> {
    if exact_oracle.trim() != exact_oracle
        || exact_oracle.is_empty()
        || source_type_line.trim().is_empty()
    {
        return None;
    }

    let (kind, source_context, normalized_oracle, rules_context) = match exact_oracle {
        AURA_TAP_ENTRY_ORACLE if source_is_aura(source_type_line) => (
            AttachmentEntryProgramKind::TapEnchantedCreature,
            AttachmentSourceContext::AuraPermanent,
            AURA_TAP_NORMALIZED,
            "rules:113.7a,303.4b,303.4c,303.4m,603.3,608.2h,701.26",
        ),
        EQUIPMENT_ATTACH_ENTRY_ORACLE if source_is_equipment(source_type_line) => (
            AttachmentEntryProgramKind::AttachToTargetCreatureYouControl,
            AttachmentSourceContext::EquipmentPermanent,
            EQUIPMENT_ATTACH_NORMALIZED,
            "rules:113.7a,115.1d,115.6,301.5b,301.5c,603.3,608.2b,701.3",
        ),
        _ => return None,
    };

    let identity = AttachmentEntrySemanticIdentity {
        exact_oracle: exact_oracle.to_owned(),
        normalized_oracle: normalized_oracle.to_owned(),
        source_context,
        compiler_version: ATTACHMENT_ENTRY_COMPILER_VERSION,
        runtime_version: ATTACHMENT_ENTRY_RUNTIME_VERSION,
        rules_context,
    };
    let semantic_digest = semantic_digest(&identity, kind);
    Some(AttachmentEntryProgram {
        kind,
        identity,
        semantic_digest,
    })
}

fn source_is_aura(type_line: &str) -> bool {
    type_words(type_line).any(|word| word == "aura")
        && type_words(type_line).any(|word| word == "enchantment")
}

fn source_is_equipment(type_line: &str) -> bool {
    type_words(type_line).any(|word| word == "equipment")
        && type_words(type_line).any(|word| word == "artifact")
}

fn type_words(type_line: &str) -> impl Iterator<Item = String> + '_ {
    type_line
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
}

fn semantic_digest(
    identity: &AttachmentEntrySemanticIdentity,
    kind: AttachmentEntryProgramKind,
) -> String {
    let source_context = match identity.source_context {
        AttachmentSourceContext::AuraPermanent => "source:aura-permanent",
        AttachmentSourceContext::EquipmentPermanent => "source:equipment-permanent",
    };
    let kind = match kind {
        AttachmentEntryProgramKind::TapEnchantedCreature => "effect:tap-current-enchanted-creature",
        AttachmentEntryProgramKind::AttachToTargetCreatureYouControl => {
            "effect:attach-source-to-target-creature-you-control"
        }
    };
    let mut hasher = Sha256::new();
    for component in [
        "attachment-entry-content/v1",
        identity.compiler_version,
        identity.runtime_version,
        identity.exact_oracle.as_str(),
        identity.normalized_oracle.as_str(),
        source_context,
        kind,
        identity.rules_context,
        "identity-excludes:card-name,snapshot,row,face,address,occurrence,unrelated-metadata",
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectIdentity {
    pub object: ObjectId,
    pub incarnation: IncarnationId,
}

impl ObjectIdentity {
    pub const fn new(object: ObjectId, incarnation: IncarnationId) -> Self {
        Self {
            object,
            incarnation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentZone {
    Battlefield,
    Stack,
    Hand,
    Library,
    Graveyard,
    Exile,
    Command,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachmentCharacteristics {
    pub is_creature: bool,
    pub is_aura: bool,
    pub is_equipment: bool,
    pub has_reconfigure: bool,
}

impl AttachmentCharacteristics {
    pub const fn creature() -> Self {
        Self {
            is_creature: true,
            is_aura: false,
            is_equipment: false,
            has_reconfigure: false,
        }
    }

    pub const fn aura() -> Self {
        Self {
            is_creature: false,
            is_aura: true,
            is_equipment: false,
            has_reconfigure: false,
        }
    }

    pub const fn equipment() -> Self {
        Self {
            is_creature: false,
            is_aura: false,
            is_equipment: true,
            has_reconfigure: false,
        }
    }

    pub const fn equipment_creature(has_reconfigure: bool) -> Self {
        Self {
            is_creature: true,
            is_aura: false,
            is_equipment: true,
            has_reconfigure,
        }
    }

    fn can_be_attached_as_equipment(self) -> bool {
        self.is_equipment && (!self.is_creature || self.has_reconfigure)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentPermanent {
    pub identity: ObjectIdentity,
    pub controller: PlayerId,
    pub zone: AttachmentZone,
    pub characteristics: AttachmentCharacteristics,
    pub tapped: bool,
    pub attached_to: Option<ObjectIdentity>,
}

impl AttachmentPermanent {
    pub fn battlefield(
        identity: ObjectIdentity,
        controller: PlayerId,
        characteristics: AttachmentCharacteristics,
    ) -> Self {
        Self {
            identity,
            controller,
            zone: AttachmentZone::Battlefield,
            characteristics,
            tapped: false,
            attached_to: None,
        }
    }

    pub fn attached_to(mut self, target: ObjectIdentity) -> Self {
        self.attached_to = Some(target);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalityStatus {
    Allowed,
    Forbidden,
    Unproven,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObjectRecord {
    permanent: AttachmentPermanent,
    revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PairLegalityEvidence {
    source: ObjectIdentity,
    affected: ObjectIdentity,
    source_revision: u64,
    affected_revision: u64,
    rules_revision: u64,
    targeting: LegalityStatus,
    attaching: LegalityStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryTriggerEvidence {
    pub event_id: EntryEventId,
    pub source: ObjectIdentity,
    pub source_controller: PlayerId,
    pub source_context: AttachmentSourceContext,
    pub source_revision: u64,
    pub attached_to_at_entry: Option<ObjectIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLastKnownInformation {
    pub source: ObjectIdentity,
    pub source_controller: PlayerId,
    pub characteristics: AttachmentCharacteristics,
    pub attached_to: Option<ObjectIdentity>,
    pub source_revision: u64,
    pub attachment_legality_at_departure: LegalityStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerSourceEvidence {
    pub entry: EntryTriggerEvidence,
    pub program_semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingAttachmentEntryTrigger {
    AuraTap {
        id: TriggerId,
        source: TriggerSourceEvidence,
        enchanted_at_entry: ObjectIdentity,
    },
    EquipmentAttach {
        id: TriggerId,
        source: TriggerSourceEvidence,
        target: ObjectIdentity,
        required_controller: PlayerId,
    },
}

impl PendingAttachmentEntryTrigger {
    pub fn id(&self) -> TriggerId {
        match self {
            Self::AuraTap { id, .. } | Self::EquipmentAttach { id, .. } => *id,
        }
    }

    pub fn source(&self) -> &TriggerSourceEvidence {
        match self {
            Self::AuraTap { source, .. } | Self::EquipmentAttach { source, .. } => source,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuraReferenceEvidence {
    CurrentAttachment,
    SourceLastKnownInformation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuraNoEffectReason {
    SourceHasNoEnchantedObject,
    EnchantedObjectUnavailable,
    EnchantedObjectIsNotCreature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentCounterReason {
    TargetUnavailable,
    TargetIsNotCreature,
    TargetNotControlledByTriggerController,
    TargetCannotBeTargeted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentNoEffectReason {
    SourceUnavailable,
    SourceIsNotEquipment,
    SourceCannotEquip,
    SourceCannotAttachToItself,
    AttachmentForbidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentEntryResolution {
    AuraTapped {
        target: ObjectIdentity,
        was_already_tapped: bool,
        reference_evidence: AuraReferenceEvidence,
    },
    AuraNoEffect(AuraNoEffectReason),
    EquipmentAttached {
        equipment: ObjectIdentity,
        target: ObjectIdentity,
        previously_attached_to: Option<ObjectIdentity>,
    },
    EquipmentCountered(EquipmentCounterReason),
    EquipmentNoEffect(EquipmentNoEffectReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentEntryError {
    DuplicateObject(ObjectId),
    MissingObject(ObjectId),
    ObjectIncarnationMismatch {
        object: ObjectId,
        expected: IncarnationId,
        actual: IncarnationId,
    },
    EntrySourceNotOnBattlefield,
    EntrySourceContextMismatch,
    EntryEventIdExhausted,
    TriggerIdExhausted,
    UnknownEntryEvent(EntryEventId),
    EntryProgramContextMismatch,
    EntryProgramAlreadyUsed {
        event: EntryEventId,
        semantic_digest: String,
    },
    MissingAuraAttachmentAtEntry,
    AuraAttachmentTargetUnavailable,
    AuraAttachmentTargetIsNotCreature,
    MissingTargetSelection,
    UnexpectedTargetSelection,
    IllegalTargetSelection,
    MissingLegalityEvidence {
        source: ObjectIdentity,
        affected: ObjectIdentity,
    },
    StaleLegalityEvidence {
        source: ObjectIdentity,
        affected: ObjectIdentity,
    },
    UnprovenTargetingLegality {
        source: ObjectIdentity,
        affected: ObjectIdentity,
    },
    UnprovenAttachmentLegality {
        source: ObjectIdentity,
        affected: ObjectIdentity,
    },
    MissingSourceLastKnownInformation(ObjectIdentity),
    UnknownTrigger(TriggerId),
    WrongTriggerKind,
    IncarnationDidNotChange(ObjectId),
    ReplacementObjectIdMismatch,
    RevisionExhausted,
    RulesRevisionExhausted,
}

impl fmt::Display for AttachmentEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AttachmentEntryError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentEntryRuntime {
    objects: BTreeMap<ObjectId, ObjectRecord>,
    legality: BTreeMap<(ObjectIdentity, ObjectIdentity), PairLegalityEvidence>,
    entry_events: BTreeMap<EntryEventId, EntryTriggerEvidence>,
    used_entry_programs: BTreeSet<(EntryEventId, String)>,
    last_known_sources: BTreeMap<ObjectIdentity, SourceLastKnownInformation>,
    pending: BTreeMap<TriggerId, PendingAttachmentEntryTrigger>,
    rules_revision: u64,
    next_object_revision: u64,
    next_entry_event: EntryEventId,
    next_trigger: TriggerId,
}

impl Default for AttachmentEntryRuntime {
    fn default() -> Self {
        Self {
            objects: BTreeMap::new(),
            legality: BTreeMap::new(),
            entry_events: BTreeMap::new(),
            used_entry_programs: BTreeSet::new(),
            last_known_sources: BTreeMap::new(),
            pending: BTreeMap::new(),
            rules_revision: 0,
            next_object_revision: 1,
            next_entry_event: 1,
            next_trigger: 1,
        }
    }
}

impl AttachmentEntryRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_existing(
        &mut self,
        permanent: AttachmentPermanent,
    ) -> Result<(), AttachmentEntryError> {
        if self.objects.contains_key(&permanent.identity.object) {
            return Err(AttachmentEntryError::DuplicateObject(
                permanent.identity.object,
            ));
        }
        let revision = self.take_object_revision()?;
        self.objects.insert(
            permanent.identity.object,
            ObjectRecord {
                permanent,
                revision,
            },
        );
        Ok(())
    }

    pub fn record_entry(
        &mut self,
        permanent: AttachmentPermanent,
    ) -> Result<EntryTriggerEvidence, AttachmentEntryError> {
        if permanent.zone != AttachmentZone::Battlefield {
            return Err(AttachmentEntryError::EntrySourceNotOnBattlefield);
        }
        if let Some(existing) = self.objects.get(&permanent.identity.object) {
            if existing.permanent.identity == permanent.identity {
                return Err(AttachmentEntryError::DuplicateObject(
                    permanent.identity.object,
                ));
            }
            if existing.permanent.identity.incarnation == permanent.identity.incarnation {
                return Err(AttachmentEntryError::IncarnationDidNotChange(
                    permanent.identity.object,
                ));
            }
        }

        let source_context = if permanent.characteristics.is_aura {
            AttachmentSourceContext::AuraPermanent
        } else if permanent.characteristics.is_equipment {
            AttachmentSourceContext::EquipmentPermanent
        } else {
            return Err(AttachmentEntryError::EntrySourceContextMismatch);
        };
        let revision = self.take_object_revision()?;
        let event_id = self.next_entry_event;
        self.next_entry_event = self
            .next_entry_event
            .checked_add(1)
            .ok_or(AttachmentEntryError::EntryEventIdExhausted)?;
        let evidence = EntryTriggerEvidence {
            event_id,
            source: permanent.identity,
            source_controller: permanent.controller,
            source_context,
            source_revision: revision,
            attached_to_at_entry: permanent.attached_to,
        };
        self.objects.insert(
            permanent.identity.object,
            ObjectRecord {
                permanent,
                revision,
            },
        );
        self.entry_events.insert(event_id, evidence.clone());
        Ok(evidence)
    }

    pub fn record_pair_legality(
        &mut self,
        source: ObjectIdentity,
        affected: ObjectIdentity,
        targeting: LegalityStatus,
        attaching: LegalityStatus,
    ) -> Result<(), AttachmentEntryError> {
        let source_revision = self.exact_record(source)?.revision;
        let affected_revision = self.exact_record(affected)?.revision;
        self.legality.insert(
            (source, affected),
            PairLegalityEvidence {
                source,
                affected,
                source_revision,
                affected_revision,
                rules_revision: self.rules_revision,
                targeting,
                attaching,
            },
        );
        Ok(())
    }

    pub fn invalidate_legality_evidence(&mut self) -> Result<(), AttachmentEntryError> {
        self.rules_revision = self
            .rules_revision
            .checked_add(1)
            .ok_or(AttachmentEntryError::RulesRevisionExhausted)?;
        Ok(())
    }

    pub fn put_trigger_on_stack(
        &mut self,
        program: &AttachmentEntryProgram,
        event_id: EntryEventId,
        chosen_target: Option<ObjectIdentity>,
    ) -> Result<TriggerId, AttachmentEntryError> {
        let entry = self
            .entry_events
            .get(&event_id)
            .cloned()
            .ok_or(AttachmentEntryError::UnknownEntryEvent(event_id))?;
        let expected_context = match program.kind {
            AttachmentEntryProgramKind::TapEnchantedCreature => {
                AttachmentSourceContext::AuraPermanent
            }
            AttachmentEntryProgramKind::AttachToTargetCreatureYouControl => {
                AttachmentSourceContext::EquipmentPermanent
            }
        };
        if entry.source_context != expected_context
            || program.identity.source_context != expected_context
        {
            return Err(AttachmentEntryError::EntryProgramContextMismatch);
        }
        let use_key = (event_id, program.semantic_digest.clone());
        if self.used_entry_programs.contains(&use_key) {
            return Err(AttachmentEntryError::EntryProgramAlreadyUsed {
                event: event_id,
                semantic_digest: program.semantic_digest.clone(),
            });
        }

        let source = self.exact_record(entry.source)?;
        if source.permanent.zone != AttachmentZone::Battlefield
            || source.revision != entry.source_revision
        {
            return Err(AttachmentEntryError::StaleLegalityEvidence {
                source: entry.source,
                affected: entry.source,
            });
        }

        let trigger_id = self.next_trigger;
        let pending = match program.kind {
            AttachmentEntryProgramKind::TapEnchantedCreature => {
                if chosen_target.is_some() {
                    return Err(AttachmentEntryError::UnexpectedTargetSelection);
                }
                let enchanted = source
                    .permanent
                    .attached_to
                    .ok_or(AttachmentEntryError::MissingAuraAttachmentAtEntry)?;
                if entry.attached_to_at_entry != Some(enchanted) {
                    return Err(AttachmentEntryError::StaleLegalityEvidence {
                        source: entry.source,
                        affected: enchanted,
                    });
                }
                let enchanted_record =
                    self.exact_record(enchanted).map_err(|error| match error {
                        AttachmentEntryError::MissingObject(_)
                        | AttachmentEntryError::ObjectIncarnationMismatch { .. } => {
                            AttachmentEntryError::AuraAttachmentTargetUnavailable
                        }
                        other => other,
                    })?;
                if enchanted_record.permanent.zone != AttachmentZone::Battlefield {
                    return Err(AttachmentEntryError::AuraAttachmentTargetUnavailable);
                }
                if !enchanted_record.permanent.characteristics.is_creature {
                    return Err(AttachmentEntryError::AuraAttachmentTargetIsNotCreature);
                }
                let legality = self.current_legality(entry.source, enchanted)?;
                match legality.attaching {
                    LegalityStatus::Allowed => {}
                    LegalityStatus::Forbidden => {
                        return Err(AttachmentEntryError::IllegalTargetSelection);
                    }
                    LegalityStatus::Unproven => {
                        return Err(AttachmentEntryError::UnprovenAttachmentLegality {
                            source: entry.source,
                            affected: enchanted,
                        });
                    }
                }
                PendingAttachmentEntryTrigger::AuraTap {
                    id: trigger_id,
                    source: TriggerSourceEvidence {
                        entry,
                        program_semantic_digest: program.semantic_digest.clone(),
                    },
                    enchanted_at_entry: enchanted,
                }
            }
            AttachmentEntryProgramKind::AttachToTargetCreatureYouControl => {
                let target = chosen_target.ok_or(AttachmentEntryError::MissingTargetSelection)?;
                if target == entry.source {
                    return Err(AttachmentEntryError::IllegalTargetSelection);
                }
                let target_record = self
                    .exact_record(target)
                    .map_err(|_| AttachmentEntryError::IllegalTargetSelection)?;
                if target_record.permanent.zone != AttachmentZone::Battlefield
                    || !target_record.permanent.characteristics.is_creature
                    || target_record.permanent.controller != entry.source_controller
                {
                    return Err(AttachmentEntryError::IllegalTargetSelection);
                }
                let legality = self.current_legality(entry.source, target)?;
                match legality.targeting {
                    LegalityStatus::Allowed => {}
                    LegalityStatus::Forbidden => {
                        return Err(AttachmentEntryError::IllegalTargetSelection);
                    }
                    LegalityStatus::Unproven => {
                        return Err(AttachmentEntryError::UnprovenTargetingLegality {
                            source: entry.source,
                            affected: target,
                        });
                    }
                }
                PendingAttachmentEntryTrigger::EquipmentAttach {
                    id: trigger_id,
                    source: TriggerSourceEvidence {
                        entry: entry.clone(),
                        program_semantic_digest: program.semantic_digest.clone(),
                    },
                    target,
                    required_controller: entry.source_controller,
                }
            }
        };

        self.next_trigger = self
            .next_trigger
            .checked_add(1)
            .ok_or(AttachmentEntryError::TriggerIdExhausted)?;
        self.used_entry_programs.insert(use_key);
        self.pending.insert(trigger_id, pending);
        Ok(trigger_id)
    }

    pub fn retarget_equipment_trigger(
        &mut self,
        trigger_id: TriggerId,
        new_target: ObjectIdentity,
    ) -> Result<(), AttachmentEntryError> {
        let pending = self
            .pending
            .get(&trigger_id)
            .cloned()
            .ok_or(AttachmentEntryError::UnknownTrigger(trigger_id))?;
        let PendingAttachmentEntryTrigger::EquipmentAttach {
            source,
            required_controller,
            ..
        } = pending
        else {
            return Err(AttachmentEntryError::WrongTriggerKind);
        };
        if new_target == source.entry.source {
            return Err(AttachmentEntryError::IllegalTargetSelection);
        }
        let target = self
            .exact_record(new_target)
            .map_err(|_| AttachmentEntryError::IllegalTargetSelection)?;
        if target.permanent.zone != AttachmentZone::Battlefield
            || !target.permanent.characteristics.is_creature
            || target.permanent.controller != required_controller
        {
            return Err(AttachmentEntryError::IllegalTargetSelection);
        }
        let legality = self.current_legality(source.entry.source, new_target)?;
        match legality.targeting {
            LegalityStatus::Allowed => {}
            LegalityStatus::Forbidden => {
                return Err(AttachmentEntryError::IllegalTargetSelection);
            }
            LegalityStatus::Unproven => {
                return Err(AttachmentEntryError::UnprovenTargetingLegality {
                    source: source.entry.source,
                    affected: new_target,
                });
            }
        }
        let Some(PendingAttachmentEntryTrigger::EquipmentAttach { target, .. }) =
            self.pending.get_mut(&trigger_id)
        else {
            return Err(AttachmentEntryError::WrongTriggerKind);
        };
        *target = new_target;
        Ok(())
    }

    pub fn resolve(
        &mut self,
        trigger_id: TriggerId,
    ) -> Result<AttachmentEntryResolution, AttachmentEntryError> {
        let pending = self
            .pending
            .get(&trigger_id)
            .cloned()
            .ok_or(AttachmentEntryError::UnknownTrigger(trigger_id))?;
        let resolution = match pending {
            PendingAttachmentEntryTrigger::AuraTap { source, .. } => {
                self.preview_aura_resolution(&source)?
            }
            PendingAttachmentEntryTrigger::EquipmentAttach {
                source,
                target,
                required_controller,
                ..
            } => self.preview_equipment_resolution(&source, target, required_controller)?,
        };

        match &resolution {
            AttachmentEntryResolution::AuraTapped { target, .. } => {
                self.mutate_exact(*target, |permanent| permanent.tapped = true)?;
            }
            AttachmentEntryResolution::EquipmentAttached {
                equipment, target, ..
            } => {
                self.mutate_exact(*equipment, |permanent| {
                    permanent.attached_to = Some(*target)
                })?;
            }
            AttachmentEntryResolution::AuraNoEffect(_)
            | AttachmentEntryResolution::EquipmentCountered(_)
            | AttachmentEntryResolution::EquipmentNoEffect(_) => {}
        }
        self.pending.remove(&trigger_id);
        Ok(resolution)
    }

    fn preview_aura_resolution(
        &self,
        source: &TriggerSourceEvidence,
    ) -> Result<AttachmentEntryResolution, AttachmentEntryError> {
        let source_identity = source.entry.source;
        let current_source = self.objects.get(&source_identity.object);
        let (enchanted, reference_evidence) = match current_source {
            Some(record)
                if record.permanent.identity == source_identity
                    && record.permanent.zone == AttachmentZone::Battlefield =>
            {
                let Some(enchanted) = record.permanent.attached_to else {
                    return Ok(AttachmentEntryResolution::AuraNoEffect(
                        AuraNoEffectReason::SourceHasNoEnchantedObject,
                    ));
                };
                let current_enchanted = match self.objects.get(&enchanted.object) {
                    Some(target)
                        if target.permanent.identity == enchanted
                            && target.permanent.zone == AttachmentZone::Battlefield =>
                    {
                        target
                    }
                    _ => {
                        return Ok(AttachmentEntryResolution::AuraNoEffect(
                            AuraNoEffectReason::EnchantedObjectUnavailable,
                        ));
                    }
                };
                if !current_enchanted.permanent.characteristics.is_creature {
                    return Ok(AttachmentEntryResolution::AuraNoEffect(
                        AuraNoEffectReason::EnchantedObjectIsNotCreature,
                    ));
                }
                let legality = self.current_legality(source_identity, enchanted)?;
                match legality.attaching {
                    LegalityStatus::Allowed => {}
                    LegalityStatus::Forbidden => {
                        return Ok(AttachmentEntryResolution::AuraNoEffect(
                            AuraNoEffectReason::SourceHasNoEnchantedObject,
                        ));
                    }
                    LegalityStatus::Unproven => {
                        return Err(AttachmentEntryError::UnprovenAttachmentLegality {
                            source: source_identity,
                            affected: enchanted,
                        });
                    }
                }
                (enchanted, AuraReferenceEvidence::CurrentAttachment)
            }
            _ => {
                let last_known = self.last_known_sources.get(&source_identity).ok_or(
                    AttachmentEntryError::MissingSourceLastKnownInformation(source_identity),
                )?;
                match last_known.attachment_legality_at_departure {
                    LegalityStatus::Allowed => {}
                    LegalityStatus::Forbidden => {
                        return Ok(AttachmentEntryResolution::AuraNoEffect(
                            AuraNoEffectReason::SourceHasNoEnchantedObject,
                        ));
                    }
                    LegalityStatus::Unproven => {
                        let affected = last_known.attached_to.unwrap_or(source.entry.source);
                        return Err(AttachmentEntryError::UnprovenAttachmentLegality {
                            source: source_identity,
                            affected,
                        });
                    }
                }
                let Some(enchanted) = last_known.attached_to else {
                    return Ok(AttachmentEntryResolution::AuraNoEffect(
                        AuraNoEffectReason::SourceHasNoEnchantedObject,
                    ));
                };
                (enchanted, AuraReferenceEvidence::SourceLastKnownInformation)
            }
        };

        let target = match self.objects.get(&enchanted.object) {
            Some(record)
                if record.permanent.identity == enchanted
                    && record.permanent.zone == AttachmentZone::Battlefield =>
            {
                record
            }
            _ => {
                return Ok(AttachmentEntryResolution::AuraNoEffect(
                    AuraNoEffectReason::EnchantedObjectUnavailable,
                ));
            }
        };
        if !target.permanent.characteristics.is_creature {
            return Ok(AttachmentEntryResolution::AuraNoEffect(
                AuraNoEffectReason::EnchantedObjectIsNotCreature,
            ));
        }
        Ok(AttachmentEntryResolution::AuraTapped {
            target: enchanted,
            was_already_tapped: target.permanent.tapped,
            reference_evidence,
        })
    }

    fn preview_equipment_resolution(
        &self,
        source: &TriggerSourceEvidence,
        target: ObjectIdentity,
        required_controller: PlayerId,
    ) -> Result<AttachmentEntryResolution, AttachmentEntryError> {
        let equipment = source.entry.source;
        let source_record = match self.objects.get(&equipment.object) {
            Some(record)
                if record.permanent.identity == equipment
                    && record.permanent.zone == AttachmentZone::Battlefield =>
            {
                record
            }
            _ => {
                return Ok(AttachmentEntryResolution::EquipmentNoEffect(
                    EquipmentNoEffectReason::SourceUnavailable,
                ));
            }
        };
        if !source_record.permanent.characteristics.is_equipment {
            return Ok(AttachmentEntryResolution::EquipmentNoEffect(
                EquipmentNoEffectReason::SourceIsNotEquipment,
            ));
        }
        if equipment == target {
            return Ok(AttachmentEntryResolution::EquipmentNoEffect(
                EquipmentNoEffectReason::SourceCannotAttachToItself,
            ));
        }

        let target_record = match self.objects.get(&target.object) {
            Some(record)
                if record.permanent.identity == target
                    && record.permanent.zone == AttachmentZone::Battlefield =>
            {
                record
            }
            _ => {
                return Ok(AttachmentEntryResolution::EquipmentCountered(
                    EquipmentCounterReason::TargetUnavailable,
                ));
            }
        };
        if !target_record.permanent.characteristics.is_creature {
            return Ok(AttachmentEntryResolution::EquipmentCountered(
                EquipmentCounterReason::TargetIsNotCreature,
            ));
        }
        if target_record.permanent.controller != required_controller {
            return Ok(AttachmentEntryResolution::EquipmentCountered(
                EquipmentCounterReason::TargetNotControlledByTriggerController,
            ));
        }

        let legality = self.current_legality(equipment, target)?;
        match legality.targeting {
            LegalityStatus::Allowed => {}
            LegalityStatus::Forbidden => {
                return Ok(AttachmentEntryResolution::EquipmentCountered(
                    EquipmentCounterReason::TargetCannotBeTargeted,
                ));
            }
            LegalityStatus::Unproven => {
                return Err(AttachmentEntryError::UnprovenTargetingLegality {
                    source: equipment,
                    affected: target,
                });
            }
        }
        if !source_record
            .permanent
            .characteristics
            .can_be_attached_as_equipment()
        {
            return Ok(AttachmentEntryResolution::EquipmentNoEffect(
                EquipmentNoEffectReason::SourceCannotEquip,
            ));
        }
        match legality.attaching {
            LegalityStatus::Allowed => {}
            LegalityStatus::Forbidden => {
                return Ok(AttachmentEntryResolution::EquipmentNoEffect(
                    EquipmentNoEffectReason::AttachmentForbidden,
                ));
            }
            LegalityStatus::Unproven => {
                return Err(AttachmentEntryError::UnprovenAttachmentLegality {
                    source: equipment,
                    affected: target,
                });
            }
        }
        Ok(AttachmentEntryResolution::EquipmentAttached {
            equipment,
            target,
            previously_attached_to: source_record.permanent.attached_to,
        })
    }

    pub fn set_attachment(
        &mut self,
        source: ObjectIdentity,
        target: Option<ObjectIdentity>,
    ) -> Result<(), AttachmentEntryError> {
        if let Some(target) = target {
            let target_record = self.exact_record(target)?;
            if target_record.permanent.zone != AttachmentZone::Battlefield {
                return Err(AttachmentEntryError::AuraAttachmentTargetUnavailable);
            }
        }
        self.mutate_exact(source, |permanent| permanent.attached_to = target)
    }

    pub fn change_controller(
        &mut self,
        identity: ObjectIdentity,
        controller: PlayerId,
    ) -> Result<(), AttachmentEntryError> {
        self.mutate_exact(identity, |permanent| permanent.controller = controller)
    }

    pub fn change_characteristics(
        &mut self,
        identity: ObjectIdentity,
        characteristics: AttachmentCharacteristics,
    ) -> Result<(), AttachmentEntryError> {
        self.mutate_exact(identity, |permanent| {
            permanent.characteristics = characteristics
        })
    }

    pub fn move_object(
        &mut self,
        expected: ObjectIdentity,
        replacement: AttachmentPermanent,
    ) -> Result<SourceLastKnownInformation, AttachmentEntryError> {
        if replacement.identity.object != expected.object {
            return Err(AttachmentEntryError::ReplacementObjectIdMismatch);
        }
        if replacement.identity.incarnation == expected.incarnation {
            return Err(AttachmentEntryError::IncarnationDidNotChange(
                expected.object,
            ));
        }
        let record = self.exact_record(expected)?.clone();
        let attachment_legality_at_departure = match record.permanent.attached_to {
            Some(affected) => self
                .current_legality(expected, affected)
                .map(|evidence| evidence.attaching)
                .unwrap_or(LegalityStatus::Unproven),
            None => LegalityStatus::Allowed,
        };
        let last_known = SourceLastKnownInformation {
            source: expected,
            source_controller: record.permanent.controller,
            characteristics: record.permanent.characteristics,
            attached_to: record.permanent.attached_to,
            source_revision: record.revision,
            attachment_legality_at_departure,
        };
        let revision = self.take_object_revision()?;
        self.last_known_sources.insert(expected, last_known.clone());
        self.objects.insert(
            expected.object,
            ObjectRecord {
                permanent: replacement,
                revision,
            },
        );
        Ok(last_known)
    }

    pub fn object(&self, identity: ObjectIdentity) -> Option<&AttachmentPermanent> {
        self.objects
            .get(&identity.object)
            .filter(|record| record.permanent.identity == identity)
            .map(|record| &record.permanent)
    }

    pub fn pending(&self, trigger_id: TriggerId) -> Option<&PendingAttachmentEntryTrigger> {
        self.pending.get(&trigger_id)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn last_known_source(&self, source: ObjectIdentity) -> Option<&SourceLastKnownInformation> {
        self.last_known_sources.get(&source)
    }

    fn current_legality(
        &self,
        source: ObjectIdentity,
        affected: ObjectIdentity,
    ) -> Result<&PairLegalityEvidence, AttachmentEntryError> {
        let evidence = self
            .legality
            .get(&(source, affected))
            .ok_or(AttachmentEntryError::MissingLegalityEvidence { source, affected })?;
        let source_revision = self.exact_record(source)?.revision;
        let affected_revision = self.exact_record(affected)?.revision;
        if evidence.source_revision != source_revision
            || evidence.affected_revision != affected_revision
            || evidence.rules_revision != self.rules_revision
        {
            return Err(AttachmentEntryError::StaleLegalityEvidence { source, affected });
        }
        Ok(evidence)
    }

    fn exact_record(
        &self,
        identity: ObjectIdentity,
    ) -> Result<&ObjectRecord, AttachmentEntryError> {
        let record = self
            .objects
            .get(&identity.object)
            .ok_or(AttachmentEntryError::MissingObject(identity.object))?;
        if record.permanent.identity != identity {
            return Err(AttachmentEntryError::ObjectIncarnationMismatch {
                object: identity.object,
                expected: identity.incarnation,
                actual: record.permanent.identity.incarnation,
            });
        }
        Ok(record)
    }

    fn mutate_exact(
        &mut self,
        identity: ObjectIdentity,
        mutate: impl FnOnce(&mut AttachmentPermanent),
    ) -> Result<(), AttachmentEntryError> {
        self.exact_record(identity)?;
        let revision = self.take_object_revision()?;
        let record = self
            .objects
            .get_mut(&identity.object)
            .expect("exact record was checked");
        mutate(&mut record.permanent);
        record.revision = revision;
        Ok(())
    }

    fn take_object_revision(&mut self) -> Result<u64, AttachmentEntryError> {
        let revision = self.next_object_revision;
        self.next_object_revision = self
            .next_object_revision
            .checked_add(1)
            .ok_or(AttachmentEntryError::RevisionExhausted)?;
        Ok(revision)
    }
}
