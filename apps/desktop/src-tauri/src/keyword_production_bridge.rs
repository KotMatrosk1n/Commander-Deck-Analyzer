//! Production adapter for keyword behavior used by simulation queries.
//!
//! This module does not register execution coverage. It provides a narrow,
//! versioned boundary that binds a production physical object to the keyword
//! rules kernel, executes Devoid through that kernel, and returns the resulting
//! effective characteristics.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fmt;

use crate::keyword_rules_runtime::{
    CombatKeyword, KeywordAction, KeywordEvidenceEvent, KeywordExecutionError, KeywordGameState,
    KeywordObject, KeywordPlayerState, KeywordProgram, KeywordProgramKind, KeywordReceipt,
    ManaColor, ObjectCharacteristics, ObjectId, OfficialKeyword, PlayerId, ProtectionTarget,
    SourceProfile, Zone, can_activate_tap_or_untap_symbol, can_attack,
    can_block_for_defending_player, can_cast_at_instant_timing, execute_keyword_action,
    targeting_is_legal,
};

pub(crate) const DEVOID_PRODUCTION_BRIDGE_VERSION: &str = "devoid-production-bridge/v1";
pub(crate) const STATIC_KEYWORD_PRODUCTION_BRIDGE_VERSION: &str =
    "static-keyword-production-bridge/v1";
pub(crate) const COMBAT_EVASION_PRODUCTION_BRIDGE_VERSION: &str =
    "combat-evasion-production-bridge/v1";

pub(crate) const STATIC_KEYWORD_PRODUCTION_KEYWORDS: &[OfficialKeyword] = &[
    OfficialKeyword::Flying,
    OfficialKeyword::Flash,
    OfficialKeyword::Menace,
    OfficialKeyword::Defender,
    OfficialKeyword::Reach,
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
];

pub(crate) const fn static_keyword_has_complete_production_contract(
    keyword: OfficialKeyword,
) -> bool {
    matches!(
        keyword,
        OfficialKeyword::Defender | OfficialKeyword::Vigilance
    )
}

pub(crate) const COMBAT_EVASION_PRODUCTION_KEYWORDS: &[OfficialKeyword] = &[
    OfficialKeyword::Fear,
    OfficialKeyword::Shadow,
    OfficialKeyword::Landwalk,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StaticKeywordObjectBinding {
    object_id: ObjectId,
    owner: PlayerId,
    controller: PlayerId,
    zone: Zone,
    controlled_since_turn_began: bool,
    tapped: bool,
}

impl StaticKeywordObjectBinding {
    pub(crate) const fn new(
        object_id: ObjectId,
        owner: PlayerId,
        controller: PlayerId,
        zone: Zone,
        controlled_since_turn_began: bool,
        tapped: bool,
    ) -> Self {
        Self {
            object_id,
            owner,
            controller,
            zone,
            controlled_since_turn_began,
            tapped,
        }
    }

    pub(crate) const fn object_id(self) -> ObjectId {
        self.object_id
    }

    pub(crate) const fn owner(self) -> PlayerId {
        self.owner
    }

    pub(crate) const fn controller(self) -> PlayerId {
        self.controller
    }

    pub(crate) const fn zone(self) -> Zone {
        self.zone
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaticKeywordEvaluation {
    bridge_version: &'static str,
    binding: StaticKeywordObjectBinding,
    keyword: OfficialKeyword,
    state: KeywordGameState,
    receipt: KeywordReceipt,
}

impl StaticKeywordEvaluation {
    pub(crate) const fn bridge_version(&self) -> &'static str {
        self.bridge_version
    }

    pub(crate) const fn binding(&self) -> StaticKeywordObjectBinding {
        self.binding
    }

    pub(crate) const fn keyword(&self) -> OfficialKeyword {
        self.keyword
    }

    pub(crate) fn object(&self) -> &KeywordObject {
        self.state
            .object(self.binding.object_id)
            .expect("validated static keyword binding retains its object")
    }

    pub(crate) fn receipt(&self) -> &KeywordReceipt {
        &self.receipt
    }

    pub(crate) fn permits_instant_timing(
        &self,
        can_play_from_current_zone: bool,
    ) -> Result<bool, KeywordExecutionError> {
        can_cast_at_instant_timing(
            &self.state,
            self.binding.object_id,
            can_play_from_current_zone,
        )
    }

    pub(crate) fn permits_attack(&self) -> Result<bool, KeywordExecutionError> {
        can_attack(&self.state, self.binding.object_id)
    }

    pub(crate) fn permits_tap_or_untap_symbol(&self) -> Result<bool, KeywordExecutionError> {
        can_activate_tap_or_untap_symbol(&self.state, self.binding.object_id)
    }

    pub(crate) fn permits_target_from(
        &self,
        source_controller: PlayerId,
    ) -> Result<bool, KeywordExecutionError> {
        let source = SourceProfile {
            owner: source_controller,
            controller: source_controller,
            name: None,
            card_types: BTreeSet::new(),
            subtypes: BTreeSet::new(),
            colors: BTreeSet::new(),
            mana_value: 0,
        };
        self.permits_target_from_source(&source)
    }

    pub(crate) fn permits_target_from_source(
        &self,
        source: &SourceProfile,
    ) -> Result<bool, KeywordExecutionError> {
        targeting_is_legal(
            &self.state,
            ProtectionTarget::Object(self.binding.object_id),
            source,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StaticKeywordProductionBridgeError {
    UnsupportedProgram(OfficialKeyword),
    InexactProgramContract,
    Kernel(KeywordExecutionError),
    ReceiptContractMismatch,
    BoundObjectIdentityChanged,
    BoundObjectContextChanged,
    PrintedCharacteristicsChanged,
    KeywordWasNotInstalled,
    KeywordStateMismatch,
}

impl fmt::Display for StaticKeywordProductionBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for StaticKeywordProductionBridgeError {}

impl From<KeywordExecutionError> for StaticKeywordProductionBridgeError {
    fn from(error: KeywordExecutionError) -> Self {
        Self::Kernel(error)
    }
}

pub(crate) fn evaluate_static_keyword(
    program: &KeywordProgram,
    binding: StaticKeywordObjectBinding,
    printed: ObjectCharacteristics,
) -> Result<StaticKeywordEvaluation, StaticKeywordProductionBridgeError> {
    if !STATIC_KEYWORD_PRODUCTION_KEYWORDS.contains(&program.keyword()) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }

    let mut state = bind_static_keyword_object(binding, printed.clone())?;
    let action = match program.kind() {
        KeywordProgramKind::Flying => KeywordAction::InstallFlying {
            creature: binding.object_id,
        },
        KeywordProgramKind::Hexproof(_) | KeywordProgramKind::Shroud => {
            KeywordAction::InstallTargetingRestriction {
                target: ProtectionTarget::Object(binding.object_id),
                chosen_color: None,
                chosen_player: None,
            }
        }
        KeywordProgramKind::Flash
        | KeywordProgramKind::Menace
        | KeywordProgramKind::Defender
        | KeywordProgramKind::Reach
        | KeywordProgramKind::Haste
        | KeywordProgramKind::Vigilance
        | KeywordProgramKind::Trample
        | KeywordProgramKind::Deathtouch
        | KeywordProgramKind::Lifelink
        | KeywordProgramKind::FirstStrike
        | KeywordProgramKind::DoubleStrike
        | KeywordProgramKind::Indestructible => KeywordAction::InstallStaticKeyword {
            object: binding.object_id,
        },
        _ => {
            return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
                program.keyword(),
            ));
        }
    };
    let receipt = execute_keyword_action(&mut state, program, action)?;
    validate_static_keyword_receipt(&receipt, program, binding, &state)?;
    validate_static_keyword_object(&state, binding, &printed, program)?;

    Ok(StaticKeywordEvaluation {
        bridge_version: STATIC_KEYWORD_PRODUCTION_BRIDGE_VERSION,
        binding,
        keyword: program.keyword(),
        state,
        receipt,
    })
}

fn bind_static_keyword_object(
    binding: StaticKeywordObjectBinding,
    printed: ObjectCharacteristics,
) -> Result<KeywordGameState, StaticKeywordProductionBridgeError> {
    let mut state = KeywordGameState::default();
    state.add_player(KeywordPlayerState::new(binding.owner, 40))?;
    if binding.controller != binding.owner {
        state.add_player(KeywordPlayerState::new(binding.controller, 40))?;
    }
    let mut object = KeywordObject::new(
        binding.object_id,
        binding.owner,
        binding.controller,
        binding.zone,
        printed,
    );
    object.controlled_since_turn_began = binding.controlled_since_turn_began;
    object.tapped = binding.tapped;
    state.insert_object(object)?;
    Ok(state)
}

fn validate_static_keyword_receipt(
    receipt: &KeywordReceipt,
    program: &KeywordProgram,
    binding: StaticKeywordObjectBinding,
    state: &KeywordGameState,
) -> Result<(), StaticKeywordProductionBridgeError> {
    if receipt.keyword != program.keyword()
        || receipt.runtime_version != program.runtime_version()
        || receipt.source != *program.source()
        || receipt.official_rules.as_slice() != program.official_rules()
    {
        return Err(StaticKeywordProductionBridgeError::ReceiptContractMismatch);
    }
    let receipt_matches = match program.kind() {
        KeywordProgramKind::Flying => {
            receipt.events.as_slice()
                == [KeywordEvidenceEvent::FlyingInstalled {
                    creature: binding.object_id,
                }]
        }
        KeywordProgramKind::Hexproof(_) => {
            let [
                KeywordEvidenceEvent::TargetingRestrictionInstalled {
                    target,
                    keyword,
                    qualities,
                },
            ] = receipt.events.as_slice()
            else {
                return Err(StaticKeywordProductionBridgeError::ReceiptContractMismatch);
            };
            let object = state.object(binding.object_id)?;
            *target == ProtectionTarget::Object(binding.object_id)
                && *keyword == OfficialKeyword::Hexproof
                && if object.has_hexproof {
                    qualities.is_empty() && object.hexproof_qualities.is_empty()
                } else {
                    !qualities.is_empty() && qualities == &object.hexproof_qualities
                }
        }
        KeywordProgramKind::Shroud => {
            receipt.events.as_slice()
                == [KeywordEvidenceEvent::TargetingRestrictionInstalled {
                    target: ProtectionTarget::Object(binding.object_id),
                    keyword: OfficialKeyword::Shroud,
                    qualities: Vec::new(),
                }]
        }
        _ => {
            receipt.events.as_slice()
                == [KeywordEvidenceEvent::StaticKeywordInstalled {
                    object: binding.object_id,
                    keyword: program.keyword(),
                }]
        }
    };
    if !receipt_matches {
        return Err(StaticKeywordProductionBridgeError::ReceiptContractMismatch);
    }
    Ok(())
}

fn validate_static_keyword_object(
    state: &KeywordGameState,
    binding: StaticKeywordObjectBinding,
    expected_printed: &ObjectCharacteristics,
    program: &KeywordProgram,
) -> Result<(), StaticKeywordProductionBridgeError> {
    let keyword = program.keyword();
    let object = state.object(binding.object_id)?;
    if object.id != binding.object_id {
        return Err(StaticKeywordProductionBridgeError::BoundObjectIdentityChanged);
    }
    if object.owner != binding.owner
        || object.controller != binding.controller
        || object.zone != binding.zone
        || object.controlled_since_turn_began != binding.controlled_since_turn_began
        || object.tapped != binding.tapped
    {
        return Err(StaticKeywordProductionBridgeError::BoundObjectContextChanged);
    }
    if &object.printed != expected_printed {
        return Err(StaticKeywordProductionBridgeError::PrintedCharacteristicsChanged);
    }
    if !object.rules_keywords.contains(&keyword)
        || object
            .keyword_instances
            .get(&keyword)
            .copied()
            .unwrap_or_default()
            != 1
    {
        return Err(StaticKeywordProductionBridgeError::KeywordWasNotInstalled);
    }
    let expected_combat_keyword = static_combat_keyword(keyword);
    if expected_combat_keyword.is_some_and(|expected| !object.combat_keywords.contains(&expected)) {
        return Err(StaticKeywordProductionBridgeError::KeywordStateMismatch);
    }
    match program.kind() {
        KeywordProgramKind::Hexproof(hexproof) => match &hexproof.qualities {
            None if object.has_hexproof
                && object.hexproof_qualities.is_empty()
                && !object.has_shroud => {}
            Some(qualities)
                if !qualities.is_empty()
                    && !object.has_hexproof
                    && !object.hexproof_qualities.is_empty()
                    && !object.has_shroud => {}
            _ => return Err(StaticKeywordProductionBridgeError::KeywordStateMismatch),
        },
        KeywordProgramKind::Shroud
            if object.has_shroud
                && !object.has_hexproof
                && object.hexproof_qualities.is_empty() => {}
        KeywordProgramKind::Shroud => {
            return Err(StaticKeywordProductionBridgeError::KeywordStateMismatch);
        }
        _ => {}
    }
    Ok(())
}

fn static_combat_keyword(keyword: OfficialKeyword) -> Option<CombatKeyword> {
    match keyword {
        OfficialKeyword::Flying => Some(CombatKeyword::Flying),
        OfficialKeyword::Menace => Some(CombatKeyword::Menace),
        OfficialKeyword::Defender => Some(CombatKeyword::Defender),
        OfficialKeyword::Reach => Some(CombatKeyword::Reach),
        OfficialKeyword::Haste => Some(CombatKeyword::Haste),
        OfficialKeyword::Vigilance => Some(CombatKeyword::Vigilance),
        OfficialKeyword::Trample => Some(CombatKeyword::Trample),
        OfficialKeyword::Deathtouch => Some(CombatKeyword::Deathtouch),
        OfficialKeyword::Lifelink => Some(CombatKeyword::Lifelink),
        OfficialKeyword::FirstStrike => Some(CombatKeyword::FirstStrike),
        OfficialKeyword::DoubleStrike => Some(CombatKeyword::DoubleStrike),
        OfficialKeyword::Indestructible => Some(CombatKeyword::Indestructible),
        OfficialKeyword::Flash | OfficialKeyword::Hexproof | OfficialKeyword::Shroud => None,
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CombatEvasionEvaluation {
    bridge_version: &'static str,
    binding: StaticKeywordObjectBinding,
    state: KeywordGameState,
    programs: Vec<KeywordProgram>,
    receipts: Vec<KeywordReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CombatEvasionBlockEvaluation {
    permitted: bool,
    blocker_receipts: Vec<KeywordReceipt>,
}

impl CombatEvasionBlockEvaluation {
    pub(crate) const fn permitted(&self) -> bool {
        self.permitted
    }

    pub(crate) fn blocker_receipts(&self) -> &[KeywordReceipt] {
        &self.blocker_receipts
    }
}

impl CombatEvasionEvaluation {
    pub(crate) const fn bridge_version(&self) -> &'static str {
        self.bridge_version
    }

    pub(crate) const fn binding(&self) -> StaticKeywordObjectBinding {
        self.binding
    }

    pub(crate) fn object(&self) -> &KeywordObject {
        self.state
            .object(self.binding.object_id)
            .expect("validated combat evasion binding retains its object")
    }

    pub(crate) fn receipts(&self) -> &[KeywordReceipt] {
        &self.receipts
    }

    pub(crate) fn programs(&self) -> &[KeywordProgram] {
        &self.programs
    }

    pub(crate) fn permits_block_by(
        &self,
        blocker_binding: StaticKeywordObjectBinding,
        blocker_printed: ObjectCharacteristics,
        blocker_programs: &[&KeywordProgram],
        defending_permanents: &[(StaticKeywordObjectBinding, ObjectCharacteristics)],
    ) -> Result<bool, CombatEvasionProductionBridgeError> {
        self.evaluate_block_by(
            blocker_binding,
            blocker_printed,
            blocker_programs,
            defending_permanents,
        )
        .map(|evaluation| evaluation.permitted)
    }

    pub(crate) fn evaluate_block_by(
        &self,
        blocker_binding: StaticKeywordObjectBinding,
        blocker_printed: ObjectCharacteristics,
        blocker_programs: &[&KeywordProgram],
        defending_permanents: &[(StaticKeywordObjectBinding, ObjectCharacteristics)],
    ) -> Result<CombatEvasionBlockEvaluation, CombatEvasionProductionBridgeError> {
        let mut state = self.state.clone();
        insert_bound_object(&mut state, blocker_binding, blocker_printed.clone())?;
        let blocker_receipts = if blocker_programs.is_empty() {
            Vec::new()
        } else {
            let receipts =
                install_combat_evasion_programs(&mut state, blocker_binding, blocker_programs)?;
            validate_combat_evasion_object(
                &state,
                blocker_binding,
                &blocker_printed,
                blocker_programs,
                &receipts,
            )?;
            receipts
        };
        for (binding, printed) in defending_permanents {
            insert_bound_object(&mut state, *binding, printed.clone())?;
        }
        let permitted = can_block_for_defending_player(
            &state,
            self.binding.object_id,
            blocker_binding.object_id,
            blocker_binding.controller,
        )
        .map_err(CombatEvasionProductionBridgeError::Kernel)?;
        Ok(CombatEvasionBlockEvaluation {
            permitted,
            blocker_receipts,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CombatEvasionProductionBridgeError {
    EmptyProgramSet,
    UnsupportedProgram(OfficialKeyword),
    InexactProgramContract,
    InexactProgramSemantics,
    MixedSourceFaces,
    DuplicateClauseAddress,
    Kernel(KeywordExecutionError),
    ReceiptContractMismatch,
    BoundObjectIdentityChanged,
    BoundObjectContextChanged,
    PrintedCharacteristicsChanged,
    KeywordStateMismatch,
}

impl fmt::Display for CombatEvasionProductionBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CombatEvasionProductionBridgeError {}

impl From<KeywordExecutionError> for CombatEvasionProductionBridgeError {
    fn from(error: KeywordExecutionError) -> Self {
        Self::Kernel(error)
    }
}

pub(crate) fn evaluate_combat_evasion_keywords(
    programs: &[&KeywordProgram],
    binding: StaticKeywordObjectBinding,
    printed: ObjectCharacteristics,
) -> Result<CombatEvasionEvaluation, CombatEvasionProductionBridgeError> {
    validate_combat_evasion_program_set(programs)?;
    let mut state =
        bind_static_keyword_object(binding, printed.clone()).map_err(|error| match error {
            StaticKeywordProductionBridgeError::Kernel(error) => {
                CombatEvasionProductionBridgeError::Kernel(error)
            }
            _ => CombatEvasionProductionBridgeError::BoundObjectContextChanged,
        })?;
    let receipts = install_combat_evasion_programs(&mut state, binding, programs)?;
    validate_combat_evasion_object(&state, binding, &printed, programs, &receipts)?;
    let mut installed_programs = programs
        .iter()
        .map(|program| (*program).clone())
        .collect::<Vec<_>>();
    installed_programs.sort_by_key(|program| {
        (
            program.source().face_index,
            program.source().clause_index,
            program.keyword(),
        )
    });
    Ok(CombatEvasionEvaluation {
        bridge_version: COMBAT_EVASION_PRODUCTION_BRIDGE_VERSION,
        binding,
        state,
        programs: installed_programs,
        receipts,
    })
}

pub(crate) fn validate_combat_evasion_program_set(
    programs: &[&KeywordProgram],
) -> Result<(), CombatEvasionProductionBridgeError> {
    let Some(first) = programs.first() else {
        return Err(CombatEvasionProductionBridgeError::EmptyProgramSet);
    };
    let face_index = first.source().face_index;
    let mut addresses = BTreeSet::new();
    for program in programs {
        if !COMBAT_EVASION_PRODUCTION_KEYWORDS.contains(&program.keyword()) {
            return Err(CombatEvasionProductionBridgeError::UnsupportedProgram(
                program.keyword(),
            ));
        }
        if !program.has_exact_contract() {
            return Err(CombatEvasionProductionBridgeError::InexactProgramContract);
        }
        let semantics_are_exact = matches!(
            program.kind(),
            KeywordProgramKind::Fear(crate::keyword_rules_runtime::FearProgram {
                artifact_or_black_blockers_only: true,
            }) | KeywordProgramKind::Shadow(crate::keyword_rules_runtime::ShadowProgram {
                requires_matching_shadow_status: true,
            }) | KeywordProgramKind::Landwalk(crate::keyword_rules_runtime::LandwalkProgram {
                checks_defending_player: true,
                same_kind_instances_are_redundant: true,
                ..
            })
        );
        if !semantics_are_exact {
            return Err(CombatEvasionProductionBridgeError::InexactProgramSemantics);
        }
        if program.source().face_index != face_index {
            return Err(CombatEvasionProductionBridgeError::MixedSourceFaces);
        }
        if !addresses.insert((program.source().face_index, program.source().clause_index)) {
            return Err(CombatEvasionProductionBridgeError::DuplicateClauseAddress);
        }
    }
    Ok(())
}

fn install_combat_evasion_programs(
    state: &mut KeywordGameState,
    binding: StaticKeywordObjectBinding,
    programs: &[&KeywordProgram],
) -> Result<Vec<KeywordReceipt>, CombatEvasionProductionBridgeError> {
    validate_combat_evasion_program_set(programs)?;
    let mut ordered = programs.to_vec();
    ordered.sort_by_key(|program| {
        (
            program.source().face_index,
            program.source().clause_index,
            program.keyword(),
        )
    });
    ordered
        .into_iter()
        .map(|program| {
            let receipt = execute_keyword_action(
                state,
                program,
                KeywordAction::InstallStaticKeyword {
                    object: binding.object_id,
                },
            )?;
            validate_static_keyword_receipt(&receipt, program, binding, state)
                .map_err(|_| CombatEvasionProductionBridgeError::ReceiptContractMismatch)?;
            Ok(receipt)
        })
        .collect()
}

fn validate_combat_evasion_object(
    state: &KeywordGameState,
    binding: StaticKeywordObjectBinding,
    expected_printed: &ObjectCharacteristics,
    programs: &[&KeywordProgram],
    receipts: &[KeywordReceipt],
) -> Result<(), CombatEvasionProductionBridgeError> {
    if receipts.len() != programs.len() {
        return Err(CombatEvasionProductionBridgeError::ReceiptContractMismatch);
    }
    let object = state.object(binding.object_id)?;
    if object.id != binding.object_id {
        return Err(CombatEvasionProductionBridgeError::BoundObjectIdentityChanged);
    }
    if object.owner != binding.owner
        || object.controller != binding.controller
        || object.zone != binding.zone
        || object.controlled_since_turn_began != binding.controlled_since_turn_began
        || object.tapped != binding.tapped
    {
        return Err(CombatEvasionProductionBridgeError::BoundObjectContextChanged);
    }
    if &object.printed != expected_printed {
        return Err(CombatEvasionProductionBridgeError::PrintedCharacteristicsChanged);
    }

    let mut expected_keyword_instances = std::collections::BTreeMap::new();
    let mut expected_landwalk_instances = std::collections::BTreeMap::new();
    let mut expected_combat_keywords = BTreeSet::new();
    for program in programs {
        let instances = expected_keyword_instances
            .entry(program.keyword())
            .or_insert(0u16);
        *instances = instances.saturating_add(1);
        match program.kind() {
            KeywordProgramKind::Fear(_) => {
                expected_combat_keywords.insert(CombatKeyword::Fear);
            }
            KeywordProgramKind::Shadow(_) => {
                expected_combat_keywords.insert(CombatKeyword::Shadow);
            }
            KeywordProgramKind::Landwalk(program) => {
                let instances = expected_landwalk_instances
                    .entry(program.quality)
                    .or_insert(0u16);
                *instances = instances.saturating_add(1);
            }
            _ => {
                return Err(CombatEvasionProductionBridgeError::UnsupportedProgram(
                    program.keyword(),
                ));
            }
        }
    }
    if object.keyword_instances != expected_keyword_instances
        || object.landwalk_instances != expected_landwalk_instances
        || object.combat_keywords != expected_combat_keywords
        || object.rules_keywords
            != expected_keyword_instances
                .keys()
                .copied()
                .collect::<BTreeSet<_>>()
    {
        return Err(CombatEvasionProductionBridgeError::KeywordStateMismatch);
    }
    Ok(())
}

fn insert_bound_object(
    state: &mut KeywordGameState,
    binding: StaticKeywordObjectBinding,
    printed: ObjectCharacteristics,
) -> Result<(), CombatEvasionProductionBridgeError> {
    for player in [binding.owner, binding.controller] {
        if !state.players.contains_key(&player) {
            state.add_player(KeywordPlayerState::new(player, 40))?;
        }
    }
    let mut object = KeywordObject::new(
        binding.object_id,
        binding.owner,
        binding.controller,
        binding.zone,
        printed,
    );
    object.controlled_since_turn_began = binding.controlled_since_turn_began;
    object.tapped = binding.tapped;
    state.insert_object(object)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DevoidObjectBinding {
    object_id: ObjectId,
    owner: PlayerId,
    controller: PlayerId,
    zone: Zone,
}

impl DevoidObjectBinding {
    pub(crate) const fn new(
        object_id: ObjectId,
        owner: PlayerId,
        controller: PlayerId,
        zone: Zone,
    ) -> Self {
        Self {
            object_id,
            owner,
            controller,
            zone,
        }
    }

    pub(crate) const fn object_id(self) -> ObjectId {
        self.object_id
    }

    pub(crate) const fn owner(self) -> PlayerId {
        self.owner
    }

    pub(crate) const fn controller(self) -> PlayerId {
        self.controller
    }

    pub(crate) const fn zone(self) -> Zone {
        self.zone
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DevoidCharacteristicEvaluation {
    bridge_version: &'static str,
    binding: DevoidObjectBinding,
    printed: ObjectCharacteristics,
    effective: ObjectCharacteristics,
    receipt: KeywordReceipt,
}

impl DevoidCharacteristicEvaluation {
    pub(crate) const fn bridge_version(&self) -> &'static str {
        self.bridge_version
    }

    pub(crate) const fn binding(&self) -> DevoidObjectBinding {
        self.binding
    }

    pub(crate) fn printed_characteristics(&self) -> &ObjectCharacteristics {
        &self.printed
    }

    pub(crate) fn effective_characteristics(&self) -> &ObjectCharacteristics {
        &self.effective
    }

    pub(crate) fn effective_colors(&self) -> &BTreeSet<ManaColor> {
        &self.effective.colors
    }

    pub(crate) fn receipt(&self) -> &KeywordReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DevoidProductionBridgeError {
    NonDevoidProgram(OfficialKeyword),
    InexactProgramContract,
    Kernel(KeywordExecutionError),
    ReceiptContractMismatch,
    BoundObjectIdentityChanged,
    BoundObjectContextChanged,
    PrintedCharacteristicsChanged,
    DevoidWasNotInstalled,
}

impl fmt::Display for DevoidProductionBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DevoidProductionBridgeError {}

impl From<KeywordExecutionError> for DevoidProductionBridgeError {
    fn from(error: KeywordExecutionError) -> Self {
        Self::Kernel(error)
    }
}

/// Installs Devoid through the official keyword executor and returns the
/// kernel-derived effective characteristics for one stable physical object.
///
/// The caller supplies the physical object identity and its current owner,
/// controller, zone, and printed characteristics. Printed characteristics are
/// retained unchanged. Any compile, execution, or validation failure returns
/// an error and exposes no partial result.
pub(crate) fn evaluate_devoid_characteristics(
    program: &KeywordProgram,
    binding: DevoidObjectBinding,
    printed: ObjectCharacteristics,
) -> Result<DevoidCharacteristicEvaluation, DevoidProductionBridgeError> {
    let mut state = bind_physical_object(binding, printed.clone())?;
    let receipt = install_and_validate_devoid(&mut state, program, binding, &printed)?;
    let object = state.object(binding.object_id)?;

    Ok(DevoidCharacteristicEvaluation {
        bridge_version: DEVOID_PRODUCTION_BRIDGE_VERSION,
        binding,
        printed: object.printed.clone(),
        effective: object.effective_characteristics(),
        receipt,
    })
}

fn bind_physical_object(
    binding: DevoidObjectBinding,
    printed: ObjectCharacteristics,
) -> Result<KeywordGameState, DevoidProductionBridgeError> {
    let mut state = KeywordGameState::default();
    state.add_player(KeywordPlayerState::new(binding.owner, 0))?;
    if binding.controller != binding.owner {
        state.add_player(KeywordPlayerState::new(binding.controller, 0))?;
    }
    state.insert_object(KeywordObject::new(
        binding.object_id,
        binding.owner,
        binding.controller,
        binding.zone,
        printed,
    ))?;
    Ok(state)
}

fn install_and_validate_devoid(
    state: &mut KeywordGameState,
    program: &KeywordProgram,
    binding: DevoidObjectBinding,
    expected_printed: &ObjectCharacteristics,
) -> Result<KeywordReceipt, DevoidProductionBridgeError> {
    let before = state.clone();
    let result = (|| {
        if !matches!(program.kind(), KeywordProgramKind::Devoid) {
            return Err(DevoidProductionBridgeError::NonDevoidProgram(
                program.keyword(),
            ));
        }
        if !program.has_exact_contract() {
            return Err(DevoidProductionBridgeError::InexactProgramContract);
        }

        let receipt = execute_keyword_action(
            state,
            program,
            KeywordAction::InstallStaticKeyword {
                object: binding.object_id,
            },
        )?;
        validate_receipt(&receipt, program, binding)?;
        validate_bound_object(state, binding, expected_printed)?;
        Ok(receipt)
    })();

    if result.is_err() {
        *state = before;
    }
    result
}

fn validate_receipt(
    receipt: &KeywordReceipt,
    program: &KeywordProgram,
    binding: DevoidObjectBinding,
) -> Result<(), DevoidProductionBridgeError> {
    let expected_event = KeywordEvidenceEvent::StaticKeywordInstalled {
        object: binding.object_id,
        keyword: OfficialKeyword::Devoid,
    };
    if receipt.keyword != OfficialKeyword::Devoid
        || receipt.runtime_version != program.runtime_version()
        || receipt.source != *program.source()
        || receipt.official_rules.as_slice() != program.official_rules()
        || receipt.events.as_slice() != [expected_event]
    {
        return Err(DevoidProductionBridgeError::ReceiptContractMismatch);
    }
    Ok(())
}

fn validate_bound_object(
    state: &KeywordGameState,
    binding: DevoidObjectBinding,
    expected_printed: &ObjectCharacteristics,
) -> Result<(), DevoidProductionBridgeError> {
    let object = state.object(binding.object_id)?;
    if object.id != binding.object_id {
        return Err(DevoidProductionBridgeError::BoundObjectIdentityChanged);
    }
    if object.owner != binding.owner
        || object.controller != binding.controller
        || object.zone != binding.zone
    {
        return Err(DevoidProductionBridgeError::BoundObjectContextChanged);
    }
    if &object.printed != expected_printed {
        return Err(DevoidProductionBridgeError::PrintedCharacteristicsChanged);
    }
    if !object.rules_keywords.contains(&OfficialKeyword::Devoid)
        || object
            .keyword_instances
            .get(&OfficialKeyword::Devoid)
            .copied()
            .unwrap_or_default()
            == 0
    {
        return Err(DevoidProductionBridgeError::DevoidWasNotInstalled);
    }
    Ok(())
}
