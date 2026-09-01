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
    BargainBaseManaCostChoice, BargainCommanderDestination, BushidoResolutionTransition,
    CombatKeyword, CumulativeUpkeepPayment, FuseHalfChoice, KeywordAction, KeywordEvidenceEvent,
    KeywordExecutionError, KeywordGameState, KeywordObject, KeywordPlayerState, KeywordProgram,
    KeywordProgramKind, KeywordReceipt, ManaColor, ManaCost, ManaPayment, MillMode,
    ObjectCharacteristics, ObjectId, OfficialKeyword, PlayerId, ProtectionInteraction,
    ProtectionTarget, RegenerationChoice, SourceProfile, SpreeModeChoiceInput, Zone,
    can_activate_tap_or_untap_symbol, can_attack, can_block_for_defending_player,
    can_cast_at_instant_timing, execute_keyword_action, protection_forbids, resolve_destruction,
    targeting_is_legal,
};

pub(crate) const DEVOID_PRODUCTION_BRIDGE_VERSION: &str = "devoid-production-bridge/v1";
pub(crate) const CHANGELING_PRODUCTION_BRIDGE_VERSION: &str = "changeling-production-bridge/v1";
pub(crate) const STATIC_KEYWORD_PRODUCTION_BRIDGE_VERSION: &str =
    "static-keyword-production-bridge/v3";
pub(crate) const COMBAT_EVASION_PRODUCTION_BRIDGE_VERSION: &str =
    "combat-evasion-production-bridge/v3";
pub(crate) const COMBAT_TRIGGER_PRODUCTION_BRIDGE_VERSION: &str =
    "combat-trigger-production-bridge/v2";
pub(crate) const WITHER_PRODUCTION_BRIDGE_VERSION: &str = "wither-production-bridge/v1";
pub(crate) const POISON_DAMAGE_PRODUCTION_BRIDGE_VERSION: &str =
    "poison-damage-production-bridge/v1";
pub(crate) const CONVOKE_PRODUCTION_BRIDGE_VERSION: &str = "convoke-production-bridge/v1";
pub(crate) const KICKER_PRODUCTION_BRIDGE_VERSION: &str = "kicker-production-bridge/v1";
pub(crate) const FLASHBACK_PRODUCTION_BRIDGE_VERSION: &str = "flashback-production-bridge/v1";
pub(crate) const MORPH_PRODUCTION_BRIDGE_VERSION: &str = "morph-production-bridge/v1";
pub(crate) const PROTECTION_PRODUCTION_BRIDGE_VERSION: &str = "protection-production-bridge/v1";
pub(crate) const AFFINITY_PRODUCTION_BRIDGE_VERSION: &str = "affinity-production-bridge/v1";
pub(crate) const DELVE_PRODUCTION_BRIDGE_VERSION: &str = "delve-production-bridge/v1";
pub(crate) const ASCEND_PRODUCTION_BRIDGE_VERSION: &str = "ascend-production-bridge/v1";
pub(crate) const AFTERMATH_PRODUCTION_BRIDGE_VERSION: &str = "aftermath-production-bridge/v1";
pub(crate) const DEATH_RETURN_PRODUCTION_BRIDGE_VERSION: &str = "death-return-production-bridge/v1";
pub(crate) const CREATURE_COUNTER_TRIGGER_PRODUCTION_BRIDGE_VERSION: &str =
    "creature-counter-trigger-production-bridge/v1";
pub(crate) const PLAYER_SPEED_PRODUCTION_BRIDGE_VERSION: &str = "player-speed-production-bridge/v1";
pub(crate) const IMPROVISE_PRODUCTION_BRIDGE_VERSION: &str = "improvise-production-bridge/v1";
pub(crate) const EXTORT_PRODUCTION_BRIDGE_VERSION: &str = "extort-production-bridge/v1";
pub(crate) const LIVING_WEAPON_PRODUCTION_BRIDGE_VERSION: &str =
    "living-weapon-production-bridge/v1";
pub(crate) const BARGAIN_PRODUCTION_BRIDGE_VERSION: &str = "bargain-production-bridge/v2";
pub(crate) const RETRACE_PRODUCTION_BRIDGE_VERSION: &str = "retrace-production-bridge/v1";
pub(crate) const EXPLOIT_PRODUCTION_BRIDGE_VERSION: &str = "exploit-production-bridge/v1";
pub(crate) const SOULBOND_PRODUCTION_BRIDGE_VERSION: &str = "soulbond-production-bridge/v1";
pub(crate) const UMBRA_ARMOR_PRODUCTION_BRIDGE_VERSION: &str = "umbra-armor-production-bridge/v1";
pub(crate) const BACKUP_PRODUCTION_BRIDGE_VERSION: &str = "backup-production-bridge/v1";
pub(crate) const MYRIAD_PRODUCTION_BRIDGE_VERSION: &str = "myriad-production-bridge/v1";
pub(crate) const CIPHER_PRODUCTION_BRIDGE_VERSION: &str = "cipher-production-bridge/v1";
pub(crate) const COMMANDER_PARTNER_PRODUCTION_BRIDGE_VERSION: &str =
    "commander-partner-production-bridge/v1";
pub(crate) const REBOUND_PRODUCTION_BRIDGE_VERSION: &str = "rebound-production-bridge/v1";
pub(crate) const CASCADE_PRODUCTION_BRIDGE_VERSION: &str = "cascade-production-bridge/v1";
pub(crate) const SPREE_PRODUCTION_BRIDGE_VERSION: &str = "spree-production-bridge/v1";
pub(crate) const DAY_NIGHT_PRODUCTION_BRIDGE_VERSION: &str = "day-night-production-bridge/v1";
pub(crate) const FUSE_PRODUCTION_BRIDGE_VERSION: &str = "fuse-production-bridge/v1";
pub(crate) const CUMULATIVE_UPKEEP_PRODUCTION_BRIDGE_VERSION: &str =
    "cumulative-upkeep-production-bridge/v1";
pub(crate) const MILL_PRODUCTION_BRIDGE_VERSION: &str = "mill-production-bridge/v1";
pub(crate) const REGENERATE_PRODUCTION_BRIDGE_VERSION: &str = "regenerate-production-bridge/v1";

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
        OfficialKeyword::Flying
            | OfficialKeyword::Flash
            | OfficialKeyword::Menace
            | OfficialKeyword::Defender
            | OfficialKeyword::Reach
            | OfficialKeyword::Haste
            | OfficialKeyword::Vigilance
            | OfficialKeyword::Trample
            | OfficialKeyword::Deathtouch
            | OfficialKeyword::Lifelink
            | OfficialKeyword::FirstStrike
            | OfficialKeyword::DoubleStrike
            | OfficialKeyword::Hexproof
            | OfficialKeyword::Shroud
            | OfficialKeyword::Indestructible
    )
}

pub(crate) fn changeling_program_has_complete_production_contract(
    program: &KeywordProgram,
) -> bool {
    matches!(
        program.kind(),
        KeywordProgramKind::Changeling(changeling)
            if program.keyword() == OfficialKeyword::Changeling
                && program.has_exact_contract()
                && changeling.is_characteristic_defining_ability
                && changeling.applies_to_the_object_with_changeling
    )
}

pub(crate) const COMBAT_EVASION_PRODUCTION_KEYWORDS: &[OfficialKeyword] = &[
    OfficialKeyword::Fear,
    OfficialKeyword::Intimidate,
    OfficialKeyword::Skulk,
    OfficialKeyword::Shadow,
    OfficialKeyword::Landwalk,
    OfficialKeyword::Horsemanship,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CombatTriggerKeywordEvaluation {
    bridge_version: &'static str,
    binding: StaticKeywordObjectBinding,
    state: KeywordGameState,
    receipt: KeywordReceipt,
}

impl CombatTriggerKeywordEvaluation {
    pub(crate) const fn bridge_version(&self) -> &'static str {
        self.bridge_version
    }

    pub(crate) const fn binding(&self) -> StaticKeywordObjectBinding {
        self.binding
    }

    pub(crate) fn object(&self) -> &KeywordObject {
        self.state
            .object(self.binding.object_id)
            .expect("validated combat trigger binding retains its object")
    }

    pub(crate) fn receipt(&self) -> &KeywordReceipt {
        &self.receipt
    }
}

pub(crate) fn resolve_exalted_keyword(
    program: &KeywordProgram,
    ability_controller: PlayerId,
    attacker_binding: StaticKeywordObjectBinding,
    printed: ObjectCharacteristics,
    declared_attackers: &[ObjectId],
) -> Result<CombatTriggerKeywordEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Exalted(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let mut state = bind_static_keyword_object(attacker_binding, printed)?;
    state.object_mut(attacker_binding.object_id)?.attacking = true;
    let receipt = execute_keyword_action(
        &mut state,
        program,
        KeywordAction::ResolveExaltedTrigger {
            ability_controller,
            attacker: attacker_binding.object_id,
            declared_attackers: declared_attackers.to_vec(),
        },
    )?;
    Ok(CombatTriggerKeywordEvaluation {
        bridge_version: COMBAT_TRIGGER_PRODUCTION_BRIDGE_VERSION,
        binding: attacker_binding,
        state,
        receipt,
    })
}

pub(crate) fn resolve_bushido_keyword(
    program: &KeywordProgram,
    creature_binding: StaticKeywordObjectBinding,
    printed: ObjectCharacteristics,
    transition: BushidoResolutionTransition,
) -> Result<CombatTriggerKeywordEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Bushido(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let mut state = bind_static_keyword_object(creature_binding, printed)?;
    let object = state.object_mut(creature_binding.object_id)?;
    match transition {
        BushidoResolutionTransition::DeclaredAsBlocker => object.blocking = true,
        BushidoResolutionTransition::AttackerBecameBlocked => object.attacking = true,
    }
    let receipt = execute_keyword_action(
        &mut state,
        program,
        KeywordAction::ResolveBushidoTrigger {
            creature: creature_binding.object_id,
            transition,
        },
    )?;
    Ok(CombatTriggerKeywordEvaluation {
        bridge_version: COMBAT_TRIGGER_PRODUCTION_BRIDGE_VERSION,
        binding: creature_binding,
        state,
        receipt,
    })
}

pub(crate) fn resolve_flanking_keyword(
    program: &KeywordProgram,
    attacker_binding: StaticKeywordObjectBinding,
    attacker_printed: ObjectCharacteristics,
    blocker_binding: StaticKeywordObjectBinding,
    blocker_printed: ObjectCharacteristics,
    blocker_had_flanking_at_trigger: bool,
) -> Result<CombatTriggerKeywordEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Flanking(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let mut state = bind_static_keyword_object(attacker_binding, attacker_printed)?;
    state.object_mut(attacker_binding.object_id)?.attacking = true;
    for player in [blocker_binding.owner, blocker_binding.controller] {
        if !state.players.contains_key(&player) {
            state.add_player(KeywordPlayerState::new(player, 40))?;
        }
    }
    let mut blocker = KeywordObject::new(
        blocker_binding.object_id,
        blocker_binding.owner,
        blocker_binding.controller,
        blocker_binding.zone,
        blocker_printed,
    );
    blocker.controlled_since_turn_began = blocker_binding.controlled_since_turn_began;
    blocker.tapped = blocker_binding.tapped;
    blocker.blocking = true;
    state.insert_object(blocker)?;
    let receipt = execute_keyword_action(
        &mut state,
        program,
        KeywordAction::ResolveFlankingTrigger {
            attacker: attacker_binding.object_id,
            blocker: blocker_binding.object_id,
            blocker_had_flanking_at_trigger,
        },
    )?;
    Ok(CombatTriggerKeywordEvaluation {
        bridge_version: COMBAT_TRIGGER_PRODUCTION_BRIDGE_VERSION,
        binding: blocker_binding,
        state,
        receipt,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WitherDamageEvaluation {
    bridge_version: &'static str,
    source: ObjectId,
    creature: ObjectId,
    state: KeywordGameState,
    receipts: Vec<KeywordReceipt>,
}

impl WitherDamageEvaluation {
    pub(crate) const fn bridge_version(&self) -> &'static str {
        self.bridge_version
    }

    pub(crate) fn source(&self) -> &KeywordObject {
        self.state
            .object(self.source)
            .expect("validated Wither source remains bound")
    }

    pub(crate) fn creature(&self) -> &KeywordObject {
        self.state
            .object(self.creature)
            .expect("validated Wither recipient remains bound")
    }

    pub(crate) fn receipts(&self) -> &[KeywordReceipt] {
        &self.receipts
    }
}

pub(crate) fn apply_wither_creature_damage(
    program: &KeywordProgram,
    source_binding: StaticKeywordObjectBinding,
    source_printed: ObjectCharacteristics,
    source_controller_at_damage: PlayerId,
    creature_binding: StaticKeywordObjectBinding,
    creature_printed: ObjectCharacteristics,
    damage_dealt: u32,
) -> Result<WitherDamageEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Wither(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let mut state = bind_static_keyword_object(source_binding, source_printed)?;
    let install = execute_keyword_action(
        &mut state,
        program,
        KeywordAction::InstallStaticKeyword {
            object: source_binding.object_id,
        },
    )?;
    for player in [creature_binding.owner, creature_binding.controller] {
        if !state.players.contains_key(&player) {
            state.add_player(KeywordPlayerState::new(player, 40))?;
        }
    }
    let mut creature = KeywordObject::new(
        creature_binding.object_id,
        creature_binding.owner,
        creature_binding.controller,
        creature_binding.zone,
        creature_printed,
    );
    creature.controlled_since_turn_began = creature_binding.controlled_since_turn_began;
    creature.tapped = creature_binding.tapped;
    state.insert_object(creature)?;
    let damage = execute_keyword_action(
        &mut state,
        program,
        KeywordAction::ApplyWitherCreatureDamage {
            source: source_binding.object_id,
            source_controller_at_damage,
            creature: creature_binding.object_id,
            damage_dealt,
        },
    )?;
    Ok(WitherDamageEvaluation {
        bridge_version: WITHER_PRODUCTION_BRIDGE_VERSION,
        source: source_binding.object_id,
        creature: creature_binding.object_id,
        state,
        receipts: vec![install, damage],
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InfectDamageRecipient {
    Creature {
        binding: StaticKeywordObjectBinding,
        printed: ObjectCharacteristics,
    },
    Player(PlayerId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PoisonDamageEvaluation {
    bridge_version: &'static str,
    state: KeywordGameState,
    receipts: Vec<KeywordReceipt>,
}

impl PoisonDamageEvaluation {
    pub(crate) const fn bridge_version(&self) -> &'static str {
        self.bridge_version
    }

    pub(crate) fn state(&self) -> &KeywordGameState {
        &self.state
    }

    pub(crate) fn receipts(&self) -> &[KeywordReceipt] {
        &self.receipts
    }
}

pub(crate) fn apply_infect_damage(
    program: &KeywordProgram,
    source_binding: StaticKeywordObjectBinding,
    source_printed: ObjectCharacteristics,
    source_controller_at_damage: PlayerId,
    recipient: InfectDamageRecipient,
    damage_dealt: u32,
) -> Result<PoisonDamageEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Infect(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let mut state = bind_static_keyword_object(source_binding, source_printed)?;
    let install = execute_keyword_action(
        &mut state,
        program,
        KeywordAction::InstallStaticKeyword {
            object: source_binding.object_id,
        },
    )?;
    let damage = match recipient {
        InfectDamageRecipient::Creature { binding, printed } => {
            for player in [binding.owner, binding.controller] {
                if !state.players.contains_key(&player) {
                    state.add_player(KeywordPlayerState::new(player, 40))?;
                }
            }
            let creature = KeywordObject::new(
                binding.object_id,
                binding.owner,
                binding.controller,
                binding.zone,
                printed,
            );
            state.insert_object(creature)?;
            execute_keyword_action(
                &mut state,
                program,
                KeywordAction::ApplyInfectCreatureDamage {
                    source: source_binding.object_id,
                    source_controller_at_damage,
                    creature: binding.object_id,
                    damage_dealt,
                },
            )?
        }
        InfectDamageRecipient::Player(player) => {
            if !state.players.contains_key(&player) {
                state.add_player(KeywordPlayerState::new(player, 40))?;
            }
            execute_keyword_action(
                &mut state,
                program,
                KeywordAction::ApplyInfectPlayerDamage {
                    source: source_binding.object_id,
                    source_controller_at_damage,
                    player,
                    damage_dealt,
                },
            )?
        }
    };
    Ok(PoisonDamageEvaluation {
        bridge_version: POISON_DAMAGE_PRODUCTION_BRIDGE_VERSION,
        state,
        receipts: vec![install, damage],
    })
}

pub(crate) fn apply_toxic_combat_damage(
    program: &KeywordProgram,
    source_binding: StaticKeywordObjectBinding,
    source_printed: ObjectCharacteristics,
    source_controller_at_damage: PlayerId,
    player: PlayerId,
    damage_dealt: u32,
    step: crate::keyword_rules_runtime::CombatDamageStep,
) -> Result<PoisonDamageEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Toxic(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let mut state = bind_static_keyword_object(source_binding, source_printed)?;
    state.object_mut(source_binding.object_id)?.attacking = true;
    let install = execute_keyword_action(
        &mut state,
        program,
        KeywordAction::InstallStaticKeyword {
            object: source_binding.object_id,
        },
    )?;
    if !state.players.contains_key(&player) {
        state.add_player(KeywordPlayerState::new(player, 40))?;
    }
    let damage = execute_keyword_action(
        &mut state,
        program,
        KeywordAction::ApplyToxicCombatDamage {
            source: source_binding.object_id,
            source_controller_at_damage,
            player,
            damage_dealt,
            step,
        },
    )?;
    Ok(PoisonDamageEvaluation {
        bridge_version: POISON_DAMAGE_PRODUCTION_BRIDGE_VERSION,
        state,
        receipts: vec![install, damage],
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConvokePaymentEvaluation {
    bridge_version: &'static str,
    receipt: KeywordReceipt,
}

impl ConvokePaymentEvaluation {
    pub(crate) const fn bridge_version(&self) -> &'static str {
        self.bridge_version
    }

    pub(crate) fn receipt(&self) -> &KeywordReceipt {
        &self.receipt
    }
}

pub(crate) fn execute_convoke_payment(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    spell: ObjectId,
    total_cost: ManaCost,
    convoking_creatures: std::collections::BTreeMap<usize, Vec<ObjectId>>,
    mana_payment: ManaPayment,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Convoke(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::PayConvokeCost {
            player,
            spell,
            total_cost,
            convoking_creatures,
            mana_payment,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: CONVOKE_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn execute_kicker_payment(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    spell: ObjectId,
    cost_index: usize,
    payment: ManaPayment,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Kicker(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::PayKicker {
            player,
            spell,
            cost_index,
            payment,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: KICKER_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

fn validate_flashback_program(
    program: &KeywordProgram,
) -> Result<(), StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Flashback(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    Ok(())
}

pub(crate) fn cast_with_flashback(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    card: ObjectId,
    payment: ManaPayment,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_flashback_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::CastWithFlashback {
            player,
            card,
            payment,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: FLASHBACK_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn leave_stack_after_flashback(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    card: ObjectId,
    requested_destination: Zone,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_flashback_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::LeaveStackAfterFlashback {
            card,
            requested_destination,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: FLASHBACK_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

fn validate_aftermath_program(
    program: &KeywordProgram,
) -> Result<(), StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Aftermath(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    Ok(())
}

pub(crate) fn cast_with_aftermath(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    card: ObjectId,
    selected_half_mana_cost: ManaCost,
    payment: ManaPayment,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_aftermath_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::CastWithAftermath {
            player,
            card,
            selected_half_mana_cost,
            payment,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: AFTERMATH_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn leave_stack_after_aftermath(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    card: ObjectId,
    requested_destination: Zone,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_aftermath_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::LeaveStackAfterAftermath {
            card,
            requested_destination,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: AFTERMATH_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

fn validate_morph_program(
    program: &KeywordProgram,
) -> Result<(), StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Morph(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    Ok(())
}

pub(crate) fn cast_face_down_with_morph(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    card: ObjectId,
    can_cast_from_current_zone: bool,
    payment: ManaPayment,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_morph_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::CastFaceDownWithMorph {
            player,
            card,
            can_cast_from_current_zone,
            payment,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: MORPH_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_face_down_morph_spell(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    card: ObjectId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_morph_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveFaceDownMorphSpell { card },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: MORPH_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn turn_morph_face_up(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    permanent: ObjectId,
    payment: ManaPayment,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_morph_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::TurnMorphFaceUp {
            player,
            permanent,
            payment,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: MORPH_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProtectionEvaluation {
    bridge_version: &'static str,
    binding: StaticKeywordObjectBinding,
    state: KeywordGameState,
    receipt: KeywordReceipt,
}

impl ProtectionEvaluation {
    pub(crate) const fn bridge_version(&self) -> &'static str {
        self.bridge_version
    }

    pub(crate) fn receipt(&self) -> &KeywordReceipt {
        &self.receipt
    }

    pub(crate) fn forbids(
        &self,
        source: &SourceProfile,
        interaction: ProtectionInteraction,
    ) -> Result<bool, KeywordExecutionError> {
        protection_forbids(
            &self.state,
            ProtectionTarget::Object(self.binding.object_id),
            source,
            interaction,
        )
    }
}

pub(crate) fn evaluate_protection(
    program: &KeywordProgram,
    binding: StaticKeywordObjectBinding,
    printed: ObjectCharacteristics,
    chosen_color: Option<ManaColor>,
    chosen_player: Option<PlayerId>,
) -> Result<ProtectionEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Protection(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let mut state = bind_static_keyword_object(binding, printed)?;
    let receipt = execute_keyword_action(
        &mut state,
        program,
        KeywordAction::InstallProtection {
            target: ProtectionTarget::Object(binding.object_id),
            chosen_color,
            chosen_player,
        },
    )?;
    Ok(ProtectionEvaluation {
        bridge_version: PROTECTION_PRODUCTION_BRIDGE_VERSION,
        binding,
        state,
        receipt,
    })
}

pub(crate) fn execute_affinity_for_artifacts_payment(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    spell: ObjectId,
    total_cost: ManaCost,
    affinity_instances: u32,
    mana_payment: ManaPayment,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Affinity(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::PayAffinityForArtifactsCost {
            player,
            spell,
            total_cost,
            affinity_instances,
            mana_payment,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: AFFINITY_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn execute_delve_payment(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    spell: ObjectId,
    total_cost: ManaCost,
    exiled_cards: Vec<ObjectId>,
    mana_payment: ManaPayment,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Delve(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::PayDelveCost {
            player,
            spell,
            total_cost,
            exiled_cards,
            mana_payment,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: DELVE_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn check_ascend(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    source: ObjectId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Ascend(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::CheckAscend { player, source },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: ASCEND_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_death_return_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    card: ObjectId,
    had_prohibited_counter_immediately_before_death: bool,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(
        program.kind(),
        KeywordProgramKind::Persist(_) | KeywordProgramKind::Undying(_)
    ) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveDeathReturnTrigger {
            card,
            had_prohibited_counter_immediately_before_death,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: DEATH_RETURN_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_evolve_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    source: ObjectId,
    entering_creature: ObjectId,
    comparison_was_true_at_trigger: bool,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Evolve(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveEvolveTrigger {
            source,
            entering_creature,
            comparison_was_true_at_trigger,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: CREATURE_COUNTER_TRIGGER_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_mentor_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    source: ObjectId,
    target: ObjectId,
    restriction_was_legal_at_trigger: bool,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Mentor(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveMentorTrigger {
            source,
            target,
            restriction_was_legal_at_trigger,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: CREATURE_COUNTER_TRIGGER_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_renown_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    source: ObjectId,
    dealt_combat_damage_to_player: bool,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Renown(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveRenownTrigger {
            source,
            dealt_combat_damage_to_player,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: CREATURE_COUNTER_TRIGGER_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

fn validate_start_your_engines_program(
    program: &KeywordProgram,
) -> Result<(), StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::StartYourEngines(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    Ok(())
}

pub(crate) fn check_start_your_engines(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    source: ObjectId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_start_your_engines_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::CheckStartYourEngines { player, source },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: PLAYER_SPEED_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn begin_speed_controller_turn(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_start_your_engines_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::BeginSpeedControllerTurn { player },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: PLAYER_SPEED_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_speed_increase_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    losing_opponent: PlayerId,
    is_players_turn: bool,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_start_your_engines_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveSpeedIncreaseTrigger {
            player,
            losing_opponent,
            is_players_turn,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: PLAYER_SPEED_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn execute_improvise_payment(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    spell: ObjectId,
    total_cost: ManaCost,
    tapped_artifacts: Vec<ObjectId>,
    mana_payment: ManaPayment,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Improvise(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::PayImproviseCost {
            player,
            spell,
            total_cost,
            tapped_artifacts,
            mana_payment,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: IMPROVISE_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_extort_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    payment: Option<ManaPayment>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Extort(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveExtortTrigger { player, payment },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: EXTORT_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_living_weapon_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    equipment: ObjectId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::LivingWeapon(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveLivingWeaponTrigger { equipment },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: LIVING_WEAPON_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

fn validate_bargain_program(
    program: &KeywordProgram,
) -> Result<(), StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Bargain(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    Ok(())
}

pub(crate) fn cast_with_bargain(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    spell: ObjectId,
    base_mana_cost: BargainBaseManaCostChoice,
    sacrificed_permanent: Option<ObjectId>,
    sacrificed_commander_destination: Option<BargainCommanderDestination>,
    bargain_conditional_targets: Vec<ProtectionTarget>,
    mana_payment: ManaPayment,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_bargain_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::CastWithBargain {
            player,
            spell,
            base_mana_cost,
            sacrificed_permanent,
            sacrificed_commander_destination,
            bargain_conditional_targets,
            mana_payment,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: BARGAIN_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn cast_with_retrace(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    card: ObjectId,
    discarded_land: ObjectId,
    printed_mana_cost: ManaCost,
    timing_and_restrictions_legal: bool,
    payment: ManaPayment,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Retrace(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::CastWithRetrace {
            player,
            card,
            discarded_land,
            printed_mana_cost,
            timing_and_restrictions_legal,
            payment,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: RETRACE_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_exploit_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    source: ObjectId,
    ability_controller: PlayerId,
    sacrifice: Option<ObjectId>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Exploit(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveExploitTrigger {
            source,
            ability_controller,
            sacrifice,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: EXPLOIT_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

fn validate_soulbond_program(
    program: &KeywordProgram,
) -> Result<(), StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Soulbond(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    Ok(())
}

pub(crate) fn resolve_soulbond_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    source: ObjectId,
    ability_controller: PlayerId,
    partner: Option<ObjectId>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_soulbond_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveSoulbondTrigger {
            source,
            ability_controller,
            partner,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: SOULBOND_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn check_soulbond_pair_state(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    object: ObjectId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    validate_soulbond_program(program)?;
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::CheckSoulbondPairState { object },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: SOULBOND_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn replace_destruction_with_umbra_armor(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    aura: ObjectId,
    enchanted_permanent: ObjectId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::UmbraArmor(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ReplaceDestructionWithUmbraArmor {
            aura,
            enchanted_permanent,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: UMBRA_ARMOR_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_backup_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    source: ObjectId,
    target: ObjectId,
    granted_abilities: Vec<String>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Backup(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveBackupTrigger {
            source,
            target,
            granted_abilities,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: BACKUP_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_myriad_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    source: ObjectId,
    defending_player: PlayerId,
    attack_targets: Vec<ProtectionTarget>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Myriad(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    if !program.has_exact_contract() {
        return Err(StaticKeywordProductionBridgeError::InexactProgramContract);
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveMyriadTrigger {
            source,
            defending_player,
            attack_targets,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: MYRIAD_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_myriad_end_of_combat(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    delayed_trigger_id: u64,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Myriad(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveMyriadEndOfCombat { delayed_trigger_id },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: MYRIAD_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn cleanup_backup_abilities(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    object: ObjectId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Backup(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::CleanupBackupAbilities { object },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: BACKUP_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn encode_cipher(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    spell: ObjectId,
    creature: Option<ObjectId>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Cipher(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::EncodeCipher { spell, creature },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: CIPHER_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_cipher_combat_damage_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    encoded_card: ObjectId,
    creature: ObjectId,
    damaged_player: PlayerId,
    cast_copy: bool,
    timing_and_restrictions_legal: bool,
    additional_costs_paid: bool,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Cipher(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveCipherCombatDamageTrigger {
            encoded_card,
            creature,
            damaged_player,
            cast_copy,
            timing_and_restrictions_legal,
            additional_costs_paid,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: CIPHER_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_commander_partner_pair(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    source: ObjectId,
    counterpart: ObjectId,
    deck_card_count: u32,
    source_color_identity: BTreeSet<ManaColor>,
    counterpart_color_identity: BTreeSet<ManaColor>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(
        program.kind(),
        KeywordProgramKind::ChooseABackground(_) | KeywordProgramKind::DoctorsCompanion(_)
    ) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ValidateCommanderPartnerPair {
            source,
            counterpart,
            deck_card_count,
            source_color_identity,
            counterpart_color_identity,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: COMMANDER_PARTNER_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn replace_rebound_resolution(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    card: ObjectId,
    requested_destination: Zone,
    resolved: bool,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Rebound(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ReplaceReboundResolution {
            card,
            requested_destination,
            resolved,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: REBOUND_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_rebound_upkeep_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    delayed_trigger_id: u64,
    active_player: PlayerId,
    cast_card: bool,
    timing_and_restrictions_legal: bool,
    additional_costs_paid: bool,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Rebound(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveReboundUpkeepTrigger {
            delayed_trigger_id,
            active_player,
            cast_card,
            timing_and_restrictions_legal,
            additional_costs_paid,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: REBOUND_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_cascade_trigger(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    source_spell: ObjectId,
    cast_eligible_card: bool,
    resulting_spell_mana_value: Option<u32>,
    timing_and_restrictions_legal: bool,
    additional_costs_paid: bool,
    as_you_cascade_action_complete: bool,
    library_bottom_order: Vec<ObjectId>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Cascade(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveCascadeTrigger {
            source_spell,
            cast_eligible_card,
            resulting_spell_mana_value,
            timing_and_restrictions_legal,
            additional_costs_paid,
            as_you_cascade_action_complete,
            library_bottom_order,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: CASCADE_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn pay_spree_mode_costs(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    spell: ObjectId,
    mode_costs: Vec<ManaCost>,
    choices: Vec<SpreeModeChoiceInput>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Spree(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::PaySpreeModeCosts {
            player,
            spell,
            mode_costs,
            choices,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: SPREE_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn copy_spree_spell(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    spell: ObjectId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Spree(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(state, program, KeywordAction::CopySpreeSpell { spell })?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: SPREE_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn register_day_night_permanent(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    object: ObjectId,
    daybound_front: ObjectCharacteristics,
    nightbound_back: ObjectCharacteristics,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(
        program.kind(),
        KeywordProgramKind::Daybound(_) | KeywordProgramKind::Nightbound(_)
    ) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::RegisterDayNightPermanent {
            object,
            daybound_front,
            nightbound_back,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: DAY_NIGHT_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn record_day_night_spell_cast(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(
        program.kind(),
        KeywordProgramKind::Daybound(_) | KeywordProgramKind::Nightbound(_)
    ) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::RecordDayNightSpellCast { player },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: DAY_NIGHT_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn evaluate_day_night_untap_step(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    previous_turn_players: Vec<PlayerId>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(
        program.kind(),
        KeywordProgramKind::Daybound(_) | KeywordProgramKind::Nightbound(_)
    ) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::EvaluateDayNightUntapStep {
            previous_turn_players,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: DAY_NIGHT_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn cast_with_fuse(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    card: ObjectId,
    left: ObjectCharacteristics,
    right: ObjectCharacteristics,
    choice: FuseHalfChoice,
    left_mana_cost: ManaCost,
    right_mana_cost: ManaCost,
    left_payment: Option<ManaPayment>,
    right_payment: Option<ManaPayment>,
    timing_and_restrictions_legal: bool,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Fuse(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::CastWithFuse {
            player,
            card,
            left,
            right,
            choice,
            left_mana_cost,
            right_mana_cost,
            left_payment,
            right_payment,
            timing_and_restrictions_legal,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: FUSE_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_cumulative_upkeep(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    permanent: ObjectId,
    player: PlayerId,
    payments: Option<Vec<CumulativeUpkeepPayment>>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::CumulativeUpkeep(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::ResolveCumulativeUpkeep {
            permanent,
            player,
            payments,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: CUMULATIVE_UPKEEP_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn install_targeting_keyword(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    target: ProtectionTarget,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(
        program.kind(),
        KeywordProgramKind::Hexproof(_) | KeywordProgramKind::Shroud
    ) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::InstallTargetingRestriction {
            target,
            chosen_color: None,
            chosen_player: None,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: PROTECTION_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn execute_mill(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    player: PlayerId,
    amount: u32,
    mode: MillMode,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Mill) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::Mill {
            player,
            amount,
            mode,
        },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: MILL_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn create_regeneration_replacement(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    permanent: ObjectId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Regenerate(_)) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::CreateRegenerationReplacement { permanent },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: REGENERATE_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn resolve_regeneration_destruction(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    permanent: ObjectId,
    choice: Option<RegenerationChoice>,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    let receipt = resolve_destruction(state, program, permanent, choice)?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: REGENERATE_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
}

pub(crate) fn attempt_indestructible_destruction(
    program: &KeywordProgram,
    state: &mut KeywordGameState,
    permanent: ObjectId,
) -> Result<ConvokePaymentEvaluation, StaticKeywordProductionBridgeError> {
    if !matches!(program.kind(), KeywordProgramKind::Indestructible) {
        return Err(StaticKeywordProductionBridgeError::UnsupportedProgram(
            program.keyword(),
        ));
    }
    let receipt = execute_keyword_action(
        state,
        program,
        KeywordAction::AttemptIndestructibleDestruction { permanent },
    )?;
    Ok(ConvokePaymentEvaluation {
        bridge_version: STATIC_KEYWORD_PRODUCTION_BRIDGE_VERSION,
        receipt,
    })
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
            }) | KeywordProgramKind::Intimidate(
                crate::keyword_rules_runtime::IntimidateProgram {
                    is_static_evasion_ability: true,
                    blocker_qualification:
                        crate::keyword_rules_runtime::IntimidateBlockerQualification::ArtifactCreatureOrCreatureSharingAtLeastOneCurrentColorWithAttacker,
                    every_declared_blocker_must_individually_qualify: true,
                    colorless_attacker_requires_artifact_blocker: true,
                    checks_current_characteristics_during_block_declaration: true,
                    gain_or_loss_after_legal_declaration_does_not_change_block: true,
                    later_attacker_or_blocker_characteristic_changes_do_not_change_block: true,
                    composes_with_other_block_restrictions: true,
                    instances_are_redundant: true,
                }
            ) | KeywordProgramKind::Skulk(
                crate::keyword_rules_runtime::SkulkProgram {
                    is_static_evasion_ability: true,
                    blocker_qualification: crate::keyword_rules_runtime::SkulkBlockerQualification::CreatureWithCurrentPowerNotGreaterThanAttacker,
                    every_declared_blocker_must_individually_qualify: true,
                    checks_current_power_during_block_declaration: true,
                    gain_or_loss_after_legal_declaration_does_not_change_block: true,
                    later_attacker_or_blocker_power_changes_do_not_change_block: true,
                    composes_with_other_block_restrictions: true,
                    instances_are_redundant: true,
                }
            ) | KeywordProgramKind::Shadow(crate::keyword_rules_runtime::ShadowProgram {
                requires_matching_shadow_status: true,
            }) | KeywordProgramKind::Landwalk(crate::keyword_rules_runtime::LandwalkProgram {
                checks_defending_player: true,
                same_kind_instances_are_redundant: true,
                ..
            }) | KeywordProgramKind::Horsemanship(
                crate::keyword_rules_runtime::HorsemanshipProgram {
                    block_restriction: crate::keyword_rules_runtime::HorsemanshipBlockRestriction::BlockerMustHaveHorsemanship,
                    creature_with_horsemanship_may_block_either_kind: true,
                    instances_are_redundant: true,
                }
            )
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
            KeywordProgramKind::Intimidate(_) => {
                expected_combat_keywords.insert(CombatKeyword::Intimidate);
            }
            KeywordProgramKind::Skulk(_) => {
                expected_combat_keywords.insert(CombatKeyword::Skulk);
            }
            KeywordProgramKind::Shadow(_) => {
                expected_combat_keywords.insert(CombatKeyword::Shadow);
            }
            KeywordProgramKind::Horsemanship(_) => {
                expected_combat_keywords.insert(CombatKeyword::Horsemanship);
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
