//! Exact, name-independent rules procedures for retained card layouts.
//!
//! The compiler accepts only complete, occurrence-addressed source evidence.
//! The consumer below owns the zone-facing characteristics, legal face
//! choices, and live layout transitions. Oracle abilities still decide when a
//! transform, flip, meld, unlock, or preparation event is instructed.

use sha2::{Digest, Sha256};

use crate::ability_program::EXECUTABLE_ABILITY_PROGRAM_VERSION;
use crate::runtime_receipts::{
    FACE_LAYOUT_RUNTIME_EXECUTOR_VERSION, RUNTIME_RECEIPT_SCHEMA_VERSION, RuntimeCapability,
    RuntimeExecutorBinding, RuntimeOracleClauseEvidence, RuntimeSourceEvidence,
};

pub(crate) const FACE_LAYOUT_EXECUTOR_ID: &str = "abstract-play.face-layout";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FaceLayoutKind {
    Transform,
    Adventure,
    ModalDoubleFaced,
    Flip,
    Reversible,
    DoubleFacedToken,
    Prepare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FaceRole {
    Single,
    SplitPart,
    TransformFront,
    TransformBack,
    AdventurePermanent,
    AdventureSpell,
    ModalFront,
    ModalBack,
    FlipNormal,
    FlipAlternative,
    ReversibleFront,
    ReversibleBack,
    TokenFront,
    TokenBack,
    MeldFront,
    PreparePermanent,
    PrepareSpell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaceLayoutFaceSource {
    pub face_index: u16,
    pub source_sha256: String,
    pub functional_sha256: String,
    pub profile: FaceRulesProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FaceRulesProfile {
    pub is_land: bool,
    pub is_permanent: bool,
    pub is_instant_or_sorcery: bool,
    pub is_room: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelatedLayoutSource {
    pub stable_id: String,
    pub component_kind: String,
    pub source_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaceLayoutRuntimeInput {
    pub layout: String,
    pub card_revision_sha256: String,
    pub faces: Vec<FaceLayoutFaceSource>,
    pub related_components: Vec<RelatedLayoutSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FaceLayoutProgram {
    SingleFace {
        layout: String,
        profile: FaceRulesProfile,
    },
    TwoFace {
        kind: FaceLayoutKind,
        roles: [FaceRole; 2],
        profiles: [FaceRulesProfile; 2],
    },
    Split {
        roles: Vec<FaceRole>,
        profiles: Vec<FaceRulesProfile>,
        shared_permanent: bool,
    },
    Meld {
        part_ids: [String; 2],
        result_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaceLayoutRuntimeReceipt {
    pub binding: RuntimeExecutorBinding,
    pub capabilities: Vec<RuntimeCapability>,
    pub source_evidence: RuntimeSourceEvidence,
    pub program: FaceLayoutProgram,
    pub face_source_sha256s: Vec<String>,
    pub face_functional_sha256s: Vec<String>,
    pub related_component_source_sha256s: Vec<String>,
    pub contract_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FaceZone {
    Library,
    Hand,
    Graveyard,
    Exile,
    Command,
    Stack,
    Battlefield,
    OutsideGame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActiveFaceView {
    Face(u16),
    Combined(Vec<u16>),
    Melded {
        part_ids: [String; 2],
        result_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaceLayoutState {
    pub zone: FaceZone,
    pub active_view: ActiveFaceView,
    pub prepared: bool,
    pub adventure_cast_permission: bool,
    pub unlocked_halves: [bool; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FaceLayoutEvent {
    CastFace(u16),
    PlayLandFace(u16),
    PutOntoBattlefield { face_index: Option<u16> },
    ResolveSpell,
    Transform,
    Flip,
    BecomePrepared,
    BecomeUnprepared,
    CastPreparedCopy,
    UnlockHalf(u16),
    LockHalf(u16),
    Meld { present_part_ids: [String; 2] },
    LeaveBattlefield(FaceZone),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FaceLayoutRuntimeError {
    UnsupportedLayout,
    IncompleteFaceEvidence,
    IncompleteRelatedComponentEvidence,
    IllegalTransition,
}

pub(crate) fn compile_face_layout_runtime(
    mut input: FaceLayoutRuntimeInput,
) -> Result<FaceLayoutRuntimeReceipt, FaceLayoutRuntimeError> {
    input.layout = input.layout.trim().to_ascii_lowercase();
    if !is_sha256_hex(&input.card_revision_sha256)
        || input.faces.iter().any(|face| {
            !is_sha256_hex(&face.source_sha256)
                || !is_sha256_hex(&face.functional_sha256)
                || face.face_index as usize >= input.faces.len()
        })
        || input
            .faces
            .windows(2)
            .any(|pair| pair[0].face_index >= pair[1].face_index)
    {
        return Err(FaceLayoutRuntimeError::IncompleteFaceEvidence);
    }
    if input.related_components.iter().any(|component| {
        component.stable_id.trim().is_empty()
            || component.component_kind.trim().is_empty()
            || !is_sha256_hex(&component.source_sha256)
    }) {
        return Err(FaceLayoutRuntimeError::IncompleteRelatedComponentEvidence);
    }

    let program = compile_program(&input)?;
    let face_source_sha256s = input
        .faces
        .iter()
        .map(|face| face.source_sha256.clone())
        .collect::<Vec<_>>();
    let related_component_source_sha256s = input
        .related_components
        .iter()
        .map(|component| component.source_sha256.clone())
        .collect::<Vec<_>>();
    let face_functional_sha256s = input
        .faces
        .iter()
        .map(|face| face.functional_sha256.clone())
        .collect::<Vec<_>>();
    let source_payload = evidence_payload(&input, &program);
    let source_evidence_sha256 = sha256_hex(source_payload.as_bytes());
    let mut clause_digests = face_source_sha256s.clone();
    clause_digests.extend(related_component_source_sha256s.iter().cloned());
    if clause_digests.is_empty() {
        return Err(FaceLayoutRuntimeError::IncompleteFaceEvidence);
    }
    let covered_oracle_clauses = clause_digests
        .iter()
        .enumerate()
        .map(|(index, digest)| RuntimeOracleClauseEvidence {
            face_index: 0,
            clause_index: index as u16,
            normalized_clause_sha256: digest.clone(),
        })
        .collect::<Vec<_>>();
    let binding = RuntimeExecutorBinding {
        receipt_schema_version: RUNTIME_RECEIPT_SCHEMA_VERSION,
        executor_id: FACE_LAYOUT_EXECUTOR_ID,
        executor_version: FACE_LAYOUT_RUNTIME_EXECUTOR_VERSION,
    };
    let capabilities = vec![RuntimeCapability::ExactFaceLayoutProgram];
    let source_evidence = RuntimeSourceEvidence {
        ability_program_version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
        normalized_oracle_sha256: input.card_revision_sha256.clone(),
        normalized_oracle_clause_sha256s: clause_digests,
        covered_oracle_clauses,
        type_line_sha256: input.card_revision_sha256,
        relevant_type_role_mask: 0,
        source_evidence_sha256,
    };
    let contract_sha256 = receipt_contract_sha256(
        &binding,
        &capabilities,
        &source_evidence,
        &program,
        &face_source_sha256s,
        &face_functional_sha256s,
        &related_component_source_sha256s,
    );
    let receipt = FaceLayoutRuntimeReceipt {
        binding,
        capabilities,
        source_evidence,
        program,
        face_source_sha256s,
        face_functional_sha256s,
        related_component_source_sha256s,
        contract_sha256,
    };
    receipt
        .has_exact_contract()
        .then_some(receipt)
        .ok_or(FaceLayoutRuntimeError::IncompleteFaceEvidence)
}

fn compile_program(
    input: &FaceLayoutRuntimeInput,
) -> Result<FaceLayoutProgram, FaceLayoutRuntimeError> {
    let two_face = |kind, roles| {
        if input.faces.len() != 2 {
            return Err(FaceLayoutRuntimeError::IncompleteFaceEvidence);
        }
        Ok(FaceLayoutProgram::TwoFace {
            kind,
            roles,
            profiles: [input.faces[0].profile, input.faces[1].profile],
        })
    };
    match input.layout.as_str() {
        "normal" | "leveler" | "class" | "case" | "saga" | "mutate" | "prototype" | "battle"
        | "planar" | "scheme" | "vanguard" | "token" | "emblem" | "augment" | "host"
        | "art_series" => {
            if input.faces.len() != 1 {
                return Err(FaceLayoutRuntimeError::IncompleteFaceEvidence);
            }
            Ok(FaceLayoutProgram::SingleFace {
                layout: input.layout.clone(),
                profile: input.faces[0].profile,
            })
        }
        "split" => {
            if input.faces.len() < 2 {
                return Err(FaceLayoutRuntimeError::IncompleteFaceEvidence);
            }
            Ok(FaceLayoutProgram::Split {
                roles: vec![FaceRole::SplitPart; input.faces.len()],
                profiles: input.faces.iter().map(|face| face.profile).collect(),
                shared_permanent: input.faces.iter().all(|face| face.profile.is_room),
            })
        }
        "transform" => two_face(
            FaceLayoutKind::Transform,
            [FaceRole::TransformFront, FaceRole::TransformBack],
        ),
        "adventure" => two_face(
            FaceLayoutKind::Adventure,
            [FaceRole::AdventurePermanent, FaceRole::AdventureSpell],
        ),
        "modal_dfc" => two_face(
            FaceLayoutKind::ModalDoubleFaced,
            [FaceRole::ModalFront, FaceRole::ModalBack],
        ),
        "flip" => two_face(
            FaceLayoutKind::Flip,
            [FaceRole::FlipNormal, FaceRole::FlipAlternative],
        ),
        "reversible_card" => {
            if input.faces.len() != 2
                || input.faces[0].functional_sha256 != input.faces[1].functional_sha256
            {
                return Err(FaceLayoutRuntimeError::IncompleteFaceEvidence);
            }
            two_face(
                FaceLayoutKind::Reversible,
                [FaceRole::ReversibleFront, FaceRole::ReversibleBack],
            )
        }
        "double_faced_token" => two_face(
            FaceLayoutKind::DoubleFacedToken,
            [FaceRole::TokenFront, FaceRole::TokenBack],
        ),
        "prepare" => two_face(
            FaceLayoutKind::Prepare,
            [FaceRole::PreparePermanent, FaceRole::PrepareSpell],
        ),
        "meld" => {
            if input.faces.len() != 1 {
                return Err(FaceLayoutRuntimeError::IncompleteRelatedComponentEvidence);
            }
            let mut part_ids = input
                .related_components
                .iter()
                .filter(|component| component.component_kind.eq_ignore_ascii_case("meld_part"))
                .map(|component| component.stable_id.clone())
                .collect::<Vec<_>>();
            let mut result_ids = input
                .related_components
                .iter()
                .filter(|component| component.component_kind.eq_ignore_ascii_case("meld_result"))
                .map(|component| component.stable_id.clone())
                .collect::<Vec<_>>();
            part_ids.sort();
            part_ids.dedup();
            result_ids.sort();
            result_ids.dedup();
            let related_role_count = input
                .related_components
                .iter()
                .filter(|component| {
                    matches!(
                        component
                            .component_kind
                            .trim()
                            .to_ascii_lowercase()
                            .as_str(),
                        "meld_part" | "meld_result"
                    )
                })
                .count();
            if related_role_count != 3 || part_ids.len() != 2 || result_ids.len() != 1 {
                return Err(FaceLayoutRuntimeError::IncompleteRelatedComponentEvidence);
            }
            Ok(FaceLayoutProgram::Meld {
                part_ids: [part_ids.remove(0), part_ids.remove(0)],
                result_id: result_ids.remove(0),
            })
        }
        _ => Err(FaceLayoutRuntimeError::UnsupportedLayout),
    }
}

impl FaceLayoutRuntimeReceipt {
    pub(crate) fn has_exact_contract(&self) -> bool {
        self.binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
            && self.binding.executor_id == FACE_LAYOUT_EXECUTOR_ID
            && self.binding.executor_version == FACE_LAYOUT_RUNTIME_EXECUTOR_VERSION
            && self.capabilities == [RuntimeCapability::ExactFaceLayoutProgram]
            && is_sha256_hex(&self.source_evidence.normalized_oracle_sha256)
            && self.source_evidence.has_exact_clause_contract()
            && is_sha256_hex(&self.source_evidence.type_line_sha256)
            && is_sha256_hex(&self.source_evidence.source_evidence_sha256)
            && self
                .face_source_sha256s
                .iter()
                .chain(self.face_functional_sha256s.iter())
                .chain(self.related_component_source_sha256s.iter())
                .all(|digest| is_sha256_hex(digest))
            && self.source_evidence.normalized_oracle_clause_sha256s
                == self
                    .face_source_sha256s
                    .iter()
                    .chain(self.related_component_source_sha256s.iter())
                    .cloned()
                    .collect::<Vec<_>>()
            && self.contract_sha256
                == receipt_contract_sha256(
                    &self.binding,
                    &self.capabilities,
                    &self.source_evidence,
                    &self.program,
                    &self.face_source_sha256s,
                    &self.face_functional_sha256s,
                    &self.related_component_source_sha256s,
                )
    }

    pub(crate) fn owns_face_source(&self, face_index: usize, source_sha256: &str) -> bool {
        self.has_exact_contract()
            && self.face_source_sha256s.get(face_index).map(String::as_str) == Some(source_sha256)
    }

    pub(crate) fn owns_related_component_source(&self, source_sha256: &str) -> bool {
        self.has_exact_contract()
            && self
                .related_component_source_sha256s
                .iter()
                .any(|digest| digest == source_sha256)
    }
}

pub(crate) fn initial_layout_state(
    program: &FaceLayoutProgram,
    zone: FaceZone,
    selected_face: Option<u16>,
) -> Result<FaceLayoutState, FaceLayoutRuntimeError> {
    let face_count = match program {
        FaceLayoutProgram::SingleFace { .. } | FaceLayoutProgram::Meld { .. } => 1,
        FaceLayoutProgram::TwoFace { .. } => 2,
        FaceLayoutProgram::Split { profiles, .. } => profiles.len(),
    };
    if selected_face.is_some_and(|face| usize::from(face) >= face_count) {
        return Err(FaceLayoutRuntimeError::IllegalTransition);
    }
    let active_view = match program {
        FaceLayoutProgram::SingleFace { .. } | FaceLayoutProgram::Meld { .. } => {
            if selected_face.is_some_and(|face| face != 0) {
                return Err(FaceLayoutRuntimeError::IllegalTransition);
            }
            ActiveFaceView::Face(0)
        }
        FaceLayoutProgram::Split { profiles, .. } => match zone {
            FaceZone::Stack => {
                ActiveFaceView::Face(valid_face_selection(selected_face, profiles.len())?)
            }
            _ => ActiveFaceView::Combined(
                (0..profiles.len())
                    .map(|index| {
                        u16::try_from(index).map_err(|_| FaceLayoutRuntimeError::IllegalTransition)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        },
        FaceLayoutProgram::TwoFace { kind, .. } => match (kind, zone) {
            (FaceLayoutKind::ModalDoubleFaced | FaceLayoutKind::Reversible, FaceZone::Stack) => {
                ActiveFaceView::Face(valid_face_selection(selected_face, 2)?)
            }
            (
                FaceLayoutKind::ModalDoubleFaced
                | FaceLayoutKind::Reversible
                | FaceLayoutKind::DoubleFacedToken,
                FaceZone::Battlefield,
            ) => ActiveFaceView::Face(selected_face.unwrap_or(0)),
            (FaceLayoutKind::Adventure, FaceZone::Stack) => {
                ActiveFaceView::Face(valid_face_selection(selected_face, 2)?)
            }
            (_, _) => ActiveFaceView::Face(0),
        },
    };
    Ok(FaceLayoutState {
        zone,
        active_view,
        prepared: false,
        adventure_cast_permission: false,
        unlocked_halves: [false; 2],
    })
}

pub(crate) fn apply_layout_event(
    program: &FaceLayoutProgram,
    state: &FaceLayoutState,
    event: FaceLayoutEvent,
) -> Result<FaceLayoutState, FaceLayoutRuntimeError> {
    let mut next = state.clone();
    match (program, event) {
        (FaceLayoutProgram::SingleFace { profile, .. }, FaceLayoutEvent::CastFace(0))
            if !profile.is_land =>
        {
            next.zone = FaceZone::Stack;
            next.active_view = ActiveFaceView::Face(0);
        }
        (FaceLayoutProgram::Split { profiles, .. }, FaceLayoutEvent::CastFace(face))
            if profiles
                .get(usize::from(face))
                .is_some_and(|profile| !profile.is_land) =>
        {
            next.zone = FaceZone::Stack;
            next.active_view = ActiveFaceView::Face(face);
        }
        (
            FaceLayoutProgram::TwoFace {
                kind: FaceLayoutKind::ModalDoubleFaced,
                profiles,
                ..
            },
            FaceLayoutEvent::CastFace(face),
        )
        | (
            FaceLayoutProgram::TwoFace {
                kind: FaceLayoutKind::Reversible,
                profiles,
                ..
            },
            FaceLayoutEvent::CastFace(face),
        ) if face < 2 && !profiles[face as usize].is_land => {
            next.zone = FaceZone::Stack;
            next.active_view = ActiveFaceView::Face(face);
        }
        (
            FaceLayoutProgram::TwoFace {
                kind: FaceLayoutKind::Adventure,
                profiles,
                ..
            },
            FaceLayoutEvent::CastFace(face),
        ) if face < 2
            && !profiles[face as usize].is_land
            && !(state.adventure_cast_permission && face == 1) =>
        {
            next.zone = FaceZone::Stack;
            next.active_view = ActiveFaceView::Face(face);
            next.adventure_cast_permission = false;
        }
        (
            FaceLayoutProgram::TwoFace {
                kind: FaceLayoutKind::Transform | FaceLayoutKind::Flip | FaceLayoutKind::Prepare,
                profiles,
                ..
            },
            FaceLayoutEvent::CastFace(0),
        ) if !profiles[0].is_land => {
            next.zone = FaceZone::Stack;
            next.active_view = ActiveFaceView::Face(0);
        }
        (FaceLayoutProgram::SingleFace { profile, .. }, FaceLayoutEvent::PlayLandFace(0))
            if profile.is_land =>
        {
            next.zone = FaceZone::Battlefield;
            next.active_view = ActiveFaceView::Face(0);
        }
        (
            FaceLayoutProgram::TwoFace {
                kind: FaceLayoutKind::ModalDoubleFaced | FaceLayoutKind::Reversible,
                profiles,
                ..
            },
            FaceLayoutEvent::PlayLandFace(face),
        ) if face < 2 && profiles[face as usize].is_land => {
            next.zone = FaceZone::Battlefield;
            next.active_view = ActiveFaceView::Face(face);
        }
        (
            FaceLayoutProgram::TwoFace {
                kind: FaceLayoutKind::Transform | FaceLayoutKind::DoubleFacedToken,
                profiles,
                ..
            },
            FaceLayoutEvent::Transform,
        ) if state.zone == FaceZone::Battlefield => {
            let ActiveFaceView::Face(face) = state.active_view else {
                return Err(FaceLayoutRuntimeError::IllegalTransition);
            };
            let destination = 1 - face;
            if profiles[destination as usize].is_instant_or_sorcery {
                return Err(FaceLayoutRuntimeError::IllegalTransition);
            }
            next.active_view = ActiveFaceView::Face(destination);
        }
        (
            FaceLayoutProgram::TwoFace {
                kind: FaceLayoutKind::Flip,
                ..
            },
            FaceLayoutEvent::Flip,
        ) if state.zone == FaceZone::Battlefield
            && state.active_view == ActiveFaceView::Face(0) =>
        {
            next.active_view = ActiveFaceView::Face(1);
        }
        (
            FaceLayoutProgram::TwoFace {
                kind: FaceLayoutKind::Prepare,
                ..
            },
            FaceLayoutEvent::BecomePrepared,
        ) if state.zone == FaceZone::Battlefield && !state.prepared => {
            next.prepared = true;
        }
        (
            FaceLayoutProgram::TwoFace {
                kind: FaceLayoutKind::Prepare,
                ..
            },
            FaceLayoutEvent::BecomeUnprepared,
        ) if state.zone == FaceZone::Battlefield && state.prepared => {
            next.prepared = false;
        }
        (
            FaceLayoutProgram::Split {
                shared_permanent: true,
                ..
            },
            FaceLayoutEvent::UnlockHalf(face),
        ) if state.zone == FaceZone::Battlefield
            && face < 2
            && !state.unlocked_halves[face as usize] =>
        {
            next.unlocked_halves[face as usize] = true;
        }
        (
            FaceLayoutProgram::Split {
                shared_permanent: true,
                ..
            },
            FaceLayoutEvent::LockHalf(face),
        ) if state.zone == FaceZone::Battlefield
            && face < 2
            && state.unlocked_halves[face as usize] =>
        {
            next.unlocked_halves[face as usize] = false;
        }
        (
            FaceLayoutProgram::TwoFace {
                kind: FaceLayoutKind::Prepare,
                ..
            },
            FaceLayoutEvent::CastPreparedCopy,
        ) if state.zone == FaceZone::Battlefield && state.prepared => {
            next.prepared = false;
        }
        (
            FaceLayoutProgram::TwoFace {
                kind: FaceLayoutKind::Adventure,
                ..
            },
            FaceLayoutEvent::ResolveSpell,
        ) if state.zone == FaceZone::Stack && state.active_view == ActiveFaceView::Face(1) => {
            next.zone = FaceZone::Exile;
            next.active_view = ActiveFaceView::Face(0);
            next.adventure_cast_permission = true;
        }
        (FaceLayoutProgram::SingleFace { profile, .. }, FaceLayoutEvent::ResolveSpell)
            if state.zone == FaceZone::Stack =>
        {
            next.zone = if profile.is_permanent {
                FaceZone::Battlefield
            } else {
                FaceZone::Graveyard
            };
        }
        (
            FaceLayoutProgram::Split {
                profiles,
                shared_permanent,
                ..
            },
            FaceLayoutEvent::ResolveSpell,
        ) if state.zone == FaceZone::Stack => {
            let ActiveFaceView::Face(face) = state.active_view else {
                return Err(FaceLayoutRuntimeError::IllegalTransition);
            };
            let Some(profile) = profiles.get(usize::from(face)) else {
                return Err(FaceLayoutRuntimeError::IllegalTransition);
            };
            let combined = ActiveFaceView::Combined(
                (0..profiles.len())
                    .map(|index| {
                        u16::try_from(index).map_err(|_| FaceLayoutRuntimeError::IllegalTransition)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            if profile.is_permanent {
                next.zone = FaceZone::Battlefield;
                if *shared_permanent {
                    next.active_view = combined;
                    let Some(unlocked) = next.unlocked_halves.get_mut(usize::from(face)) else {
                        return Err(FaceLayoutRuntimeError::IllegalTransition);
                    };
                    *unlocked = true;
                }
            } else {
                next.zone = FaceZone::Graveyard;
                next.active_view = combined;
            }
        }
        (FaceLayoutProgram::TwoFace { profiles, .. }, FaceLayoutEvent::ResolveSpell)
            if state.zone == FaceZone::Stack =>
        {
            let ActiveFaceView::Face(face) = state.active_view else {
                return Err(FaceLayoutRuntimeError::IllegalTransition);
            };
            if profiles[face as usize].is_permanent {
                next.zone = FaceZone::Battlefield;
            } else {
                next.zone = FaceZone::Graveyard;
                next.active_view = ActiveFaceView::Face(0);
            }
        }
        (
            FaceLayoutProgram::Meld {
                part_ids,
                result_id,
            },
            FaceLayoutEvent::Meld {
                mut present_part_ids,
            },
        ) if state.zone == FaceZone::Battlefield => {
            present_part_ids.sort();
            if present_part_ids != *part_ids {
                return Err(FaceLayoutRuntimeError::IllegalTransition);
            }
            next.active_view = ActiveFaceView::Melded {
                part_ids: part_ids.clone(),
                result_id: result_id.clone(),
            };
        }
        (_, FaceLayoutEvent::PutOntoBattlefield { face_index }) => {
            next = initial_layout_state(program, FaceZone::Battlefield, face_index)?;
            let selected = match next.active_view {
                ActiveFaceView::Face(index) => Some(index),
                _ => None,
            };
            let can_enter = match program {
                FaceLayoutProgram::SingleFace { profile, .. } => profile.is_permanent,
                FaceLayoutProgram::TwoFace { profiles, .. } => {
                    selected.is_none_or(|index| profiles[index as usize].is_permanent)
                }
                FaceLayoutProgram::Split {
                    profiles,
                    shared_permanent,
                    ..
                } => {
                    *shared_permanent
                        && profiles.len() == 2
                        && profiles.iter().all(|profile| profile.is_permanent)
                }
                FaceLayoutProgram::Meld { .. } => true,
            };
            if !can_enter {
                return Err(FaceLayoutRuntimeError::IllegalTransition);
            }
            if let FaceLayoutProgram::Split {
                shared_permanent: true,
                ..
            } = program
                && let Some(face) = face_index
            {
                next.unlocked_halves[face as usize] = true;
            }
        }
        (_, FaceLayoutEvent::LeaveBattlefield(zone))
            if state.zone == FaceZone::Battlefield && zone != FaceZone::Battlefield =>
        {
            next = initial_layout_state(program, zone, None)?;
        }
        _ => return Err(FaceLayoutRuntimeError::IllegalTransition),
    }
    Ok(next)
}

pub(crate) fn meld_cards_released_on_leave(
    program: &FaceLayoutProgram,
    state: &FaceLayoutState,
) -> Option<[String; 2]> {
    let FaceLayoutProgram::Meld {
        part_ids,
        result_id,
    } = program
    else {
        return None;
    };
    let ActiveFaceView::Melded {
        part_ids: active_parts,
        result_id: active_result,
    } = &state.active_view
    else {
        return None;
    };
    (state.zone == FaceZone::Battlefield && active_parts == part_ids && active_result == result_id)
        .then(|| part_ids.clone())
}

fn valid_face_selection(
    selected_face: Option<u16>,
    face_count: usize,
) -> Result<u16, FaceLayoutRuntimeError> {
    selected_face
        .filter(|face| usize::from(*face) < face_count)
        .ok_or(FaceLayoutRuntimeError::IllegalTransition)
}

fn evidence_payload(input: &FaceLayoutRuntimeInput, program: &FaceLayoutProgram) -> String {
    let faces = input
        .faces
        .iter()
        .map(|face| {
            format!(
                "{}:{}:{}:{:?}",
                face.face_index, face.source_sha256, face.functional_sha256, face.profile
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let related = input
        .related_components
        .iter()
        .map(|component| {
            format!(
                "{}:{}:{}",
                component.component_kind.trim().to_ascii_lowercase(),
                component.stable_id,
                component.source_sha256
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "layout={};revision={};faces={faces};related={related};program={program:?}",
        input.layout, input.card_revision_sha256
    )
}

fn receipt_contract_sha256(
    binding: &RuntimeExecutorBinding,
    capabilities: &[RuntimeCapability],
    source: &RuntimeSourceEvidence,
    program: &FaceLayoutProgram,
    faces: &[String],
    functional_faces: &[String],
    related: &[String],
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        binding.receipt_schema_version.as_bytes(),
        binding.executor_id.as_bytes(),
        binding.executor_version.as_bytes(),
        source.ability_program_version.as_bytes(),
        source.normalized_oracle_sha256.as_bytes(),
        source.source_evidence_sha256.as_bytes(),
        format!("{capabilities:?}").as_bytes(),
        format!("{program:?}").as_bytes(),
        faces.join(",").as_bytes(),
        functional_faces.join(",").as_bytes(),
        related.join(",").as_bytes(),
    ] {
        hash_framed(&mut hasher, part);
    }
    format!("{:x}", hasher.finalize())
}

fn hash_framed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn sha256_hex(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
