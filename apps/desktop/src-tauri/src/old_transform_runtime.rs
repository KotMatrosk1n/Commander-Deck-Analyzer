//! Content keyed programs for the original Innistrad transform triggers.
//!
//! These triggers are different from daybound and nightbound. They inspect
//! spells cast during the immediately preceding turn, use an intervening if
//! condition, and transform only the same permanent incarnation that created
//! the trigger.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const OLD_TRANSFORM_COMPILER_VERSION: &str = "old-transform-compiler-0.1";
pub const OLD_TRANSFORM_RUNTIME_VERSION: &str = "old-transform-runtime-0.1";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type TriggerId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OldTransformCondition {
    NoSpellsCastLastTurn,
    OnePlayerCastAtLeastTwoSpellsLastTurn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldTransformProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    condition: OldTransformCondition,
}

impl OldTransformProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn condition(&self) -> OldTransformCondition {
        self.condition
    }
}

pub fn compile_old_transform_program(
    exact_source: &str,
    normalized_source: &str,
) -> Option<OldTransformProgram> {
    if exact_source.trim() != exact_source
        || normalized_source.trim() != normalized_source
        || exact_source.is_empty()
        || normalized_source.is_empty()
    {
        return None;
    }
    let condition = match normalized_source.to_ascii_lowercase().as_str() {
        "at the beginning of each upkeep, if no spells were cast last turn, transform this object." => {
            OldTransformCondition::NoSpellsCastLastTurn
        }
        "at the beginning of each upkeep, if a player cast two or more spells last turn, transform this object." => {
            OldTransformCondition::OnePlayerCastAtLeastTwoSpellsLastTurn
        }
        _ => return None,
    };
    let semantic_digest = old_transform_semantic_digest(exact_source, normalized_source, condition);
    Some(OldTransformProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        semantic_digest,
        condition,
    })
}

fn old_transform_semantic_digest(
    exact_source: &str,
    normalized_source: &str,
    condition: OldTransformCondition,
) -> String {
    let mut hasher = Sha256::new();
    for component in [
        "old-transform-content/v1",
        OLD_TRANSFORM_COMPILER_VERSION,
        OLD_TRANSFORM_RUNTIME_VERSION,
        exact_source,
        normalized_source,
        match condition {
            OldTransformCondition::NoSpellsCastLastTurn => "no-spells-last-turn",
            OldTransformCondition::OnePlayerCastAtLeastTwoSpellsLastTurn => {
                "one-player-two-spells-last-turn"
            }
        },
        "trigger:beginning-of-each-upkeep",
        "condition:intervening-if-trigger-and-resolution",
        "action:transform-same-permanent-incarnation",
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OldTransformZone {
    Battlefield,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformableObject {
    pub id: ObjectId,
    pub incarnation: IncarnationId,
    pub zone: OldTransformZone,
    pub transforming_double_faced: bool,
    pub face_index: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingOldTransformTrigger {
    pub id: TriggerId,
    pub source: ObjectId,
    pub source_incarnation: IncarnationId,
    pub condition: OldTransformCondition,
    pub program_semantic_digest: String,
    pub created_turn_sequence: u64,
    pub created_upkeep_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OldTransformNoEffectReason {
    InterveningConditionFalse,
    SourceNoLongerExists,
    SourceIsNewIncarnation,
    SourceNotOnBattlefield,
    SourceIsNotTransformingDoubleFaced,
    InvalidCurrentFace(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OldTransformResolution {
    Transformed {
        source: ObjectId,
        incarnation: IncarnationId,
        from_face: u8,
        to_face: u8,
    },
    NoEffect(OldTransformNoEffectReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OldTransformError {
    DuplicateObject(ObjectId),
    MissingObject(ObjectId),
    IncarnationMismatch {
        object: ObjectId,
        expected: IncarnationId,
        actual: IncarnationId,
    },
    InvalidFaceIndex(u8),
    DuplicateFaceProgram {
        object: ObjectId,
        incarnation: IncarnationId,
        face_index: u8,
        semantic_digest: String,
    },
    UnknownTrigger(TriggerId),
    TurnSequenceExhausted,
    UpkeepSequenceExhausted,
    TriggerIdExhausted,
}

impl fmt::Display for OldTransformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for OldTransformError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledFaceProgram {
    source_incarnation: IncarnationId,
    face_index: u8,
    program: OldTransformProgram,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OldTransformRuntime {
    objects: BTreeMap<ObjectId, TransformableObject>,
    programs: BTreeMap<ObjectId, Vec<InstalledFaceProgram>>,
    spells_cast_this_turn: BTreeMap<PlayerId, u32>,
    spells_cast_last_turn: BTreeMap<PlayerId, u32>,
    turn_sequence: u64,
    upkeep_sequence: u64,
    next_trigger_id: TriggerId,
    pending: BTreeMap<TriggerId, PendingOldTransformTrigger>,
}

impl OldTransformRuntime {
    pub fn insert_object(&mut self, object: TransformableObject) -> Result<(), OldTransformError> {
        if object.face_index > 1 {
            return Err(OldTransformError::InvalidFaceIndex(object.face_index));
        }
        if self.objects.contains_key(&object.id) {
            return Err(OldTransformError::DuplicateObject(object.id));
        }
        self.objects.insert(object.id, object);
        Ok(())
    }

    pub fn replace_object_incarnation(
        &mut self,
        object: TransformableObject,
    ) -> Result<(), OldTransformError> {
        if object.face_index > 1 {
            return Err(OldTransformError::InvalidFaceIndex(object.face_index));
        }
        self.objects.insert(object.id, object);
        Ok(())
    }

    pub fn move_object(
        &mut self,
        object: ObjectId,
        incarnation: IncarnationId,
        zone: OldTransformZone,
    ) -> Result<(), OldTransformError> {
        let state = self
            .objects
            .get_mut(&object)
            .ok_or(OldTransformError::MissingObject(object))?;
        if state.incarnation != incarnation {
            return Err(OldTransformError::IncarnationMismatch {
                object,
                expected: incarnation,
                actual: state.incarnation,
            });
        }
        state.zone = zone;
        Ok(())
    }

    pub fn object(&self, object: ObjectId) -> Option<&TransformableObject> {
        self.objects.get(&object)
    }

    pub fn install_face_program(
        &mut self,
        object: ObjectId,
        incarnation: IncarnationId,
        face_index: u8,
        program: OldTransformProgram,
    ) -> Result<(), OldTransformError> {
        if face_index > 1 {
            return Err(OldTransformError::InvalidFaceIndex(face_index));
        }
        let object_state = self
            .objects
            .get(&object)
            .ok_or(OldTransformError::MissingObject(object))?;
        if object_state.incarnation != incarnation {
            return Err(OldTransformError::IncarnationMismatch {
                object,
                expected: incarnation,
                actual: object_state.incarnation,
            });
        }
        let programs = self.programs.entry(object).or_default();
        if programs.iter().any(|installed| {
            installed.source_incarnation == incarnation
                && installed.face_index == face_index
                && installed.program.semantic_digest() == program.semantic_digest()
        }) {
            return Err(OldTransformError::DuplicateFaceProgram {
                object,
                incarnation,
                face_index,
                semantic_digest: program.semantic_digest().to_owned(),
            });
        }
        programs.push(InstalledFaceProgram {
            source_incarnation: incarnation,
            face_index,
            program,
        });
        programs.sort_by(|left, right| {
            left.face_index.cmp(&right.face_index).then_with(|| {
                left.program
                    .semantic_digest()
                    .cmp(right.program.semantic_digest())
            })
        });
        Ok(())
    }

    pub fn begin_turn(&mut self) -> Result<(), OldTransformError> {
        self.turn_sequence = self
            .turn_sequence
            .checked_add(1)
            .ok_or(OldTransformError::TurnSequenceExhausted)?;
        self.spells_cast_last_turn = std::mem::take(&mut self.spells_cast_this_turn);
        Ok(())
    }

    pub fn record_spell_cast(&mut self, player: PlayerId) {
        let count = self.spells_cast_this_turn.entry(player).or_default();
        *count = count.saturating_add(1);
    }

    pub fn spells_cast_last_turn(&self, player: PlayerId) -> u32 {
        self.spells_cast_last_turn
            .get(&player)
            .copied()
            .unwrap_or(0)
    }

    pub fn begin_upkeep(&mut self) -> Result<Vec<PendingOldTransformTrigger>, OldTransformError> {
        self.upkeep_sequence = self
            .upkeep_sequence
            .checked_add(1)
            .ok_or(OldTransformError::UpkeepSequenceExhausted)?;
        let mut candidates = Vec::new();
        for (object_id, object) in &self.objects {
            if object.zone != OldTransformZone::Battlefield {
                continue;
            }
            let Some(programs) = self.programs.get(object_id) else {
                continue;
            };
            for installed in programs {
                if installed.source_incarnation != object.incarnation
                    || installed.face_index != object.face_index
                    || !self.condition_is_true(installed.program.condition())
                {
                    continue;
                }
                candidates.push((
                    *object_id,
                    object.incarnation,
                    installed.program.condition(),
                    installed.program.semantic_digest().to_owned(),
                ));
            }
        }
        candidates.sort();

        let trigger_count = TriggerId::try_from(candidates.len())
            .map_err(|_| OldTransformError::TriggerIdExhausted)?;
        let next_trigger_id = self
            .next_trigger_id
            .checked_add(trigger_count)
            .ok_or(OldTransformError::TriggerIdExhausted)?;
        let mut created = Vec::with_capacity(candidates.len());
        for (offset, (source, source_incarnation, condition, program_semantic_digest)) in
            candidates.into_iter().enumerate()
        {
            let id = self.next_trigger_id + offset as TriggerId;
            let pending = PendingOldTransformTrigger {
                id,
                source,
                source_incarnation,
                condition,
                program_semantic_digest,
                created_turn_sequence: self.turn_sequence,
                created_upkeep_sequence: self.upkeep_sequence,
            };
            self.pending.insert(id, pending.clone());
            created.push(pending);
        }
        self.next_trigger_id = next_trigger_id;
        Ok(created)
    }

    pub fn pending_trigger(&self, id: TriggerId) -> Option<&PendingOldTransformTrigger> {
        self.pending.get(&id)
    }

    pub fn resolve(&mut self, id: TriggerId) -> Result<OldTransformResolution, OldTransformError> {
        let pending = self
            .pending
            .remove(&id)
            .ok_or(OldTransformError::UnknownTrigger(id))?;
        if !self.condition_is_true(pending.condition) {
            return Ok(OldTransformResolution::NoEffect(
                OldTransformNoEffectReason::InterveningConditionFalse,
            ));
        }
        let Some(object) = self.objects.get_mut(&pending.source) else {
            return Ok(OldTransformResolution::NoEffect(
                OldTransformNoEffectReason::SourceNoLongerExists,
            ));
        };
        if object.incarnation != pending.source_incarnation {
            return Ok(OldTransformResolution::NoEffect(
                OldTransformNoEffectReason::SourceIsNewIncarnation,
            ));
        }
        if object.zone != OldTransformZone::Battlefield {
            return Ok(OldTransformResolution::NoEffect(
                OldTransformNoEffectReason::SourceNotOnBattlefield,
            ));
        }
        if !object.transforming_double_faced {
            return Ok(OldTransformResolution::NoEffect(
                OldTransformNoEffectReason::SourceIsNotTransformingDoubleFaced,
            ));
        }
        let from_face = object.face_index;
        let to_face = match from_face {
            0 => 1,
            1 => 0,
            other => {
                return Ok(OldTransformResolution::NoEffect(
                    OldTransformNoEffectReason::InvalidCurrentFace(other),
                ));
            }
        };
        object.face_index = to_face;
        Ok(OldTransformResolution::Transformed {
            source: pending.source,
            incarnation: pending.source_incarnation,
            from_face,
            to_face,
        })
    }

    fn condition_is_true(&self, condition: OldTransformCondition) -> bool {
        match condition {
            OldTransformCondition::NoSpellsCastLastTurn => {
                self.spells_cast_last_turn.values().all(|count| *count == 0)
            }
            OldTransformCondition::OnePlayerCastAtLeastTwoSpellsLastTurn => {
                self.spells_cast_last_turn.values().any(|count| *count >= 2)
            }
        }
    }

    pub fn audited_players_last_turn(&self) -> BTreeSet<PlayerId> {
        self.spells_cast_last_turn.keys().copied().collect()
    }
}
