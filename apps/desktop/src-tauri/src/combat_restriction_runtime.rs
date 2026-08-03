//! Content keyed programs and a fail closed runtime for a narrow set of
//! static combat declaration restrictions.
//!
//! This module models only the restrictions compiled by
//! [`compile_combat_restriction_program`]. It does not claim that satisfying
//! these restrictions is sufficient to make an otherwise illegal attack or
//! block legal.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const COMBAT_RESTRICTION_COMPILER_VERSION: &str = "combat-restriction-compiler-0.4";
pub const COMBAT_RESTRICTION_RUNTIME_VERSION: &str = "combat-restriction-runtime-0.4";
pub const COMBAT_DECLARATION_RULES_CONTEXT: &str =
    "combat-declaration-context-0.1:current-characteristics;simultaneous-blocks;source-incarnation";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type BindingId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CombatKeyword {
    Flying,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PermanentSubtype {
    Island,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatRestrictionKind {
    BlockerMayBlockOnlyAttackerWithKeyword { required_keyword: CombatKeyword },
    AttackRequiresDefendingPlayerPermanentSubtype { required_subtype: PermanentSubtype },
    AttackerRequiresAnotherAttacker,
    AttackerCannotBeBlockedByPowerAtMost { maximum_power: i32 },
    AttackerCannotBeBlockedByLowerPower,
    AttackerCannotBeBlockedByFlying,
    AttackerMaximumBlockers { maximum_blockers: usize },
    BlockerAdditionalCapacity { additional_creatures: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatRestrictionProgram {
    exact_source: String,
    normalized_semantics: String,
    semantic_digest: String,
    kind: CombatRestrictionKind,
}

impl CombatRestrictionProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_semantics(&self) -> &str {
        &self.normalized_semantics
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &CombatRestrictionKind {
        &self.kind
    }
}

pub fn compile_combat_restriction_program(source: &str) -> Option<CombatRestrictionProgram> {
    if source.is_empty() || source.trim() != source {
        return None;
    }

    let kind = match source {
        "This creature can block only creatures with flying." => {
            CombatRestrictionKind::BlockerMayBlockOnlyAttackerWithKeyword {
                required_keyword: CombatKeyword::Flying,
            }
        }
        "This creature can't attack unless defending player controls an Island." => {
            CombatRestrictionKind::AttackRequiresDefendingPlayerPermanentSubtype {
                required_subtype: PermanentSubtype::Island,
            }
        }
        "This creature can't attack alone." => {
            CombatRestrictionKind::AttackerRequiresAnotherAttacker
        }
        "This creature can't be blocked by creatures with power 2 or less." => {
            CombatRestrictionKind::AttackerCannotBeBlockedByPowerAtMost { maximum_power: 2 }
        }
        "Creatures with power less than this creature's power can't block it." => {
            CombatRestrictionKind::AttackerCannotBeBlockedByLowerPower
        }
        "This creature can't be blocked by creatures with flying." => {
            CombatRestrictionKind::AttackerCannotBeBlockedByFlying
        }
        "This creature can't be blocked by more than one creature." => {
            CombatRestrictionKind::AttackerMaximumBlockers {
                maximum_blockers: 1,
            }
        }
        "This creature can block an additional creature each combat." => {
            CombatRestrictionKind::BlockerAdditionalCapacity {
                additional_creatures: 1,
            }
        }
        _ => return None,
    };

    let normalized_semantics = canonical_semantics(&kind);
    let semantic_digest = combat_restriction_semantic_digest(source, &normalized_semantics);

    Some(CombatRestrictionProgram {
        exact_source: source.to_owned(),
        normalized_semantics,
        semantic_digest,
        kind,
    })
}

fn canonical_semantics(kind: &CombatRestrictionKind) -> String {
    match kind {
        CombatRestrictionKind::BlockerMayBlockOnlyAttackerWithKeyword {
            required_keyword: CombatKeyword::Flying,
        } => "subject=self;declaration=block;attacker-requires-keyword=flying".to_owned(),
        CombatRestrictionKind::AttackRequiresDefendingPlayerPermanentSubtype {
            required_subtype: PermanentSubtype::Island,
        } => "subject=self;declaration=attack;defending-player-controls-subtype=island".to_owned(),
        CombatRestrictionKind::AttackerRequiresAnotherAttacker =>
            "subject=self;declaration=attack;minimum-distinct-attackers=2".to_owned(),
        CombatRestrictionKind::AttackerCannotBeBlockedByPowerAtMost { maximum_power } => format!(
            "subject=self;declaration=block;each-blocker-current-effective-power>{maximum_power}"
        ),
        CombatRestrictionKind::AttackerCannotBeBlockedByLowerPower =>
            "subject=self;declaration=block;each-blocker-current-effective-power>=attacker-current-effective-power".to_owned(),
        CombatRestrictionKind::AttackerCannotBeBlockedByFlying =>
            "subject=self;declaration=block;blocker-must-not-have-keyword=flying".to_owned(),
        CombatRestrictionKind::AttackerMaximumBlockers { maximum_blockers } => {
            format!("subject=self;declaration=block;maximum-distinct-blockers={maximum_blockers}")
        }
        CombatRestrictionKind::BlockerAdditionalCapacity {
            additional_creatures,
        } => format!(
            "subject=self;declaration=block;additional-distinct-attackers={additional_creatures}"
        ),
    }
}

fn combat_restriction_semantic_digest(source: &str, normalized_semantics: &str) -> String {
    let mut hasher = Sha256::new();
    for component in [
        "combat-restriction-content/v1",
        COMBAT_RESTRICTION_COMPILER_VERSION,
        COMBAT_RESTRICTION_RUNTIME_VERSION,
        COMBAT_DECLARATION_RULES_CONTEXT,
        source,
        normalized_semantics,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatSourceZone {
    Battlefield,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattlefieldPermanent {
    pub object_ref: ObjectRef,
    pub controller: PlayerId,
    pub is_creature: bool,
    pub has_flying: Option<bool>,
    pub effective_power_at_block_declaration: Option<i32>,
    pub subtypes: BTreeSet<PermanentSubtype>,
}

impl BattlefieldPermanent {
    pub fn creature(
        object_ref: ObjectRef,
        controller: PlayerId,
        has_flying: Option<bool>,
        effective_power_at_block_declaration: Option<i32>,
    ) -> Self {
        Self {
            object_ref,
            controller,
            is_creature: true,
            has_flying,
            effective_power_at_block_declaration,
            subtypes: BTreeSet::new(),
        }
    }

    pub fn permanent(
        object_ref: ObjectRef,
        controller: PlayerId,
        subtypes: impl IntoIterator<Item = PermanentSubtype>,
    ) -> Self {
        Self {
            object_ref,
            controller,
            is_creature: false,
            has_flying: None,
            effective_power_at_block_declaration: None,
            subtypes: subtypes.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattlefieldSnapshot {
    permanents: BTreeMap<ObjectId, BattlefieldPermanent>,
    complete_player_battlefields: BTreeSet<PlayerId>,
}

impl BattlefieldSnapshot {
    pub fn new() -> Self {
        Self {
            permanents: BTreeMap::new(),
            complete_player_battlefields: BTreeSet::new(),
        }
    }

    pub fn insert(
        &mut self,
        permanent: BattlefieldPermanent,
    ) -> Result<(), BattlefieldSnapshotError> {
        if self
            .permanents
            .contains_key(&permanent.object_ref.object_id)
        {
            return Err(BattlefieldSnapshotError::DuplicateObjectId(
                permanent.object_ref.object_id,
            ));
        }
        self.permanents
            .insert(permanent.object_ref.object_id, permanent);
        Ok(())
    }

    pub fn mark_player_battlefield_complete(&mut self, player: PlayerId) {
        self.complete_player_battlefields.insert(player);
    }

    pub fn permanent(&self, object_ref: ObjectRef) -> Option<&BattlefieldPermanent> {
        self.permanents
            .get(&object_ref.object_id)
            .filter(|permanent| permanent.object_ref == object_ref)
    }

    fn current_incarnation(&self, object_id: ObjectId) -> Option<IncarnationId> {
        self.permanents
            .get(&object_id)
            .map(|permanent| permanent.object_ref.incarnation_id)
    }

    fn defender_controls_subtype(
        &self,
        defending_player: PlayerId,
        subtype: PermanentSubtype,
    ) -> Option<bool> {
        if self.permanents.values().any(|permanent| {
            permanent.controller == defending_player && permanent.subtypes.contains(&subtype)
        }) {
            return Some(true);
        }
        self.complete_player_battlefields
            .contains(&defending_player)
            .then_some(false)
    }
}

impl Default for BattlefieldSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BattlefieldSnapshotError {
    DuplicateObjectId(ObjectId),
}

impl fmt::Display for BattlefieldSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateObjectId(object_id) => {
                write!(
                    formatter,
                    "battlefield object {object_id} was supplied twice"
                )
            }
        }
    }
}

impl std::error::Error for BattlefieldSnapshotError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCombatRestriction {
    binding_id: BindingId,
    source: ObjectRef,
    program: CombatRestrictionProgram,
}

impl BoundCombatRestriction {
    pub fn binding_id(&self) -> BindingId {
        self.binding_id
    }

    pub fn source(&self) -> ObjectRef {
        self.source
    }

    pub fn program(&self) -> &CombatRestrictionProgram {
        &self.program
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatRestrictionBindingError {
    SourceNotOnBattlefield,
    BindingIdExhausted,
}

impl fmt::Display for CombatRestrictionBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceNotOnBattlefield => {
                formatter.write_str("a static combat restriction must originate on the battlefield")
            }
            Self::BindingIdExhausted => formatter.write_str("combat binding id space is exhausted"),
        }
    }
}

impl std::error::Error for CombatRestrictionBindingError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AttackDeclaration {
    pub attacker: ObjectRef,
    pub defending_player: PlayerId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockAssignment {
    pub attacker: ObjectRef,
    pub blocker: ObjectRef,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CombatRestrictionViolation {
    DuplicateAttacker {
        attacker: ObjectRef,
    },
    DuplicateBlockAssignment {
        attacker: ObjectRef,
        blocker: ObjectRef,
    },
    ParticipantIsNotCreature {
        participant: ObjectRef,
    },
    AttackerTargetsOwnController {
        attacker: ObjectRef,
        controller: PlayerId,
    },
    AttacksAlone {
        attacker: ObjectRef,
    },
    DefendingPlayerLacksSubtype {
        attacker: ObjectRef,
        defending_player: PlayerId,
        required_subtype: PermanentSubtype,
    },
    BlockAgainstUndeclaredAttacker {
        attacker: ObjectRef,
        blocker: ObjectRef,
    },
    BlockerNotControlledByDefendingPlayer {
        blocker: ObjectRef,
        blocker_controller: PlayerId,
        defending_player: PlayerId,
    },
    AttackerLacksRequiredKeyword {
        attacker: ObjectRef,
        blocker: ObjectRef,
        required_keyword: CombatKeyword,
    },
    BlockerPowerAtOrBelowMaximum {
        attacker: ObjectRef,
        blocker: ObjectRef,
        blocker_power: i32,
        maximum_power: i32,
    },
    BlockerPowerBelowAttacker {
        attacker: ObjectRef,
        blocker: ObjectRef,
        attacker_power: i32,
        blocker_power: i32,
    },
    FlyingBlockerForbidden {
        attacker: ObjectRef,
        blocker: ObjectRef,
    },
    TooManyBlockers {
        attacker: ObjectRef,
        actual_blockers: usize,
        maximum_blockers: usize,
    },
    BlockerCapacityExceeded {
        blocker: ObjectRef,
        actual_attackers: usize,
        maximum_attackers: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CombatStateAmbiguity {
    MissingBattlefieldObject {
        participant: ObjectRef,
    },
    StaleBattlefieldIncarnation {
        requested: ObjectRef,
        current_incarnation: IncarnationId,
    },
    DefendingPlayerBattlefieldIncomplete {
        defending_player: PlayerId,
        required_subtype: PermanentSubtype,
    },
    AttackerKeywordUnknown {
        attacker: ObjectRef,
        required_keyword: CombatKeyword,
    },
    BlockerKeywordUnknown {
        blocker: ObjectRef,
        keyword: CombatKeyword,
    },
    BlockerPowerUnknown {
        blocker: ObjectRef,
    },
    AttackerPowerUnknown {
        attacker: ObjectRef,
    },
    CapacityOverflow {
        blocker: ObjectRef,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatLegalityStatus {
    Legal,
    Illegal,
    Indeterminate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatLegalityReport {
    violations: Vec<CombatRestrictionViolation>,
    ambiguities: Vec<CombatStateAmbiguity>,
}

impl CombatLegalityReport {
    pub fn status(&self) -> CombatLegalityStatus {
        if !self.violations.is_empty() {
            CombatLegalityStatus::Illegal
        } else if !self.ambiguities.is_empty() {
            CombatLegalityStatus::Indeterminate
        } else {
            CombatLegalityStatus::Legal
        }
    }

    pub fn is_legal(&self) -> bool {
        self.status() == CombatLegalityStatus::Legal
    }

    pub fn violations(&self) -> &[CombatRestrictionViolation] {
        &self.violations
    }

    pub fn ambiguities(&self) -> &[CombatStateAmbiguity] {
        &self.ambiguities
    }

    fn merge(&mut self, other: Self) {
        self.violations.extend(other.violations);
        self.ambiguities.extend(other.ambiguities);
        self.violations.sort();
        self.violations.dedup();
        self.ambiguities.sort();
        self.ambiguities.dedup();
    }
}

#[derive(Default)]
struct CombatLegalityBuilder {
    violations: BTreeSet<CombatRestrictionViolation>,
    ambiguities: BTreeSet<CombatStateAmbiguity>,
}

impl CombatLegalityBuilder {
    fn finish(self) -> CombatLegalityReport {
        CombatLegalityReport {
            violations: self.violations.into_iter().collect(),
            ambiguities: self.ambiguities.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatRestrictionRuntime {
    next_binding_id: BindingId,
    bindings: Vec<BoundCombatRestriction>,
}

impl CombatRestrictionRuntime {
    pub fn new() -> Self {
        Self {
            next_binding_id: 1,
            bindings: Vec::new(),
        }
    }

    pub fn bind(
        &mut self,
        program: CombatRestrictionProgram,
        source: ObjectRef,
        source_zone: CombatSourceZone,
    ) -> Result<BindingId, CombatRestrictionBindingError> {
        if source_zone != CombatSourceZone::Battlefield {
            return Err(CombatRestrictionBindingError::SourceNotOnBattlefield);
        }
        let binding_id = self.next_binding_id;
        self.next_binding_id = self
            .next_binding_id
            .checked_add(1)
            .ok_or(CombatRestrictionBindingError::BindingIdExhausted)?;
        self.bindings.push(BoundCombatRestriction {
            binding_id,
            source,
            program,
        });
        Ok(binding_id)
    }

    pub fn bindings(&self) -> &[BoundCombatRestriction] {
        &self.bindings
    }

    pub fn validate_attacks(
        &self,
        attacks: &[AttackDeclaration],
        battlefield: &BattlefieldSnapshot,
    ) -> CombatLegalityReport {
        let mut report = CombatLegalityBuilder::default();
        let mut declared_attackers = BTreeSet::new();
        let all_distinct_attackers = attacks
            .iter()
            .map(|attack| attack.attacker)
            .collect::<BTreeSet<_>>();

        for attack in attacks {
            if !declared_attackers.insert(attack.attacker) {
                report
                    .violations
                    .insert(CombatRestrictionViolation::DuplicateAttacker {
                        attacker: attack.attacker,
                    });
                continue;
            }

            let Some(attacker) = resolve_participant(battlefield, attack.attacker, &mut report)
            else {
                continue;
            };
            if !attacker.is_creature {
                report
                    .violations
                    .insert(CombatRestrictionViolation::ParticipantIsNotCreature {
                        participant: attack.attacker,
                    });
                continue;
            }
            if attacker.controller == attack.defending_player {
                report.violations.insert(
                    CombatRestrictionViolation::AttackerTargetsOwnController {
                        attacker: attack.attacker,
                        controller: attacker.controller,
                    },
                );
            }

            for binding in self.active_bindings(attack.attacker, battlefield) {
                match binding.program.kind() {
                    CombatRestrictionKind::AttackRequiresDefendingPlayerPermanentSubtype {
                        required_subtype,
                    } => match battlefield
                        .defender_controls_subtype(attack.defending_player, *required_subtype)
                    {
                        Some(true) => {}
                        Some(false) => {
                            report.violations.insert(
                                CombatRestrictionViolation::DefendingPlayerLacksSubtype {
                                    attacker: attack.attacker,
                                    defending_player: attack.defending_player,
                                    required_subtype: *required_subtype,
                                },
                            );
                        }
                        None => {
                            report.ambiguities.insert(
                                CombatStateAmbiguity::DefendingPlayerBattlefieldIncomplete {
                                    defending_player: attack.defending_player,
                                    required_subtype: *required_subtype,
                                },
                            );
                        }
                    },
                    CombatRestrictionKind::AttackerRequiresAnotherAttacker
                        if all_distinct_attackers.len() < 2 =>
                    {
                        report
                            .violations
                            .insert(CombatRestrictionViolation::AttacksAlone {
                                attacker: attack.attacker,
                            });
                    }
                    _ => {}
                }
            }
        }

        report.finish()
    }

    pub fn validate_blocks(
        &self,
        attacks: &[AttackDeclaration],
        blocks: &[BlockAssignment],
        battlefield: &BattlefieldSnapshot,
    ) -> CombatLegalityReport {
        let mut report = CombatLegalityBuilder::default();
        let mut attack_defenders = BTreeMap::new();

        for attack in attacks {
            match attack_defenders.entry(attack.attacker) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(attack.defending_player);
                }
                std::collections::btree_map::Entry::Occupied(_) => {
                    report
                        .violations
                        .insert(CombatRestrictionViolation::DuplicateAttacker {
                            attacker: attack.attacker,
                        });
                }
            }
        }

        let mut distinct_assignments = BTreeSet::new();
        let mut attackers_by_blocker: BTreeMap<ObjectRef, BTreeSet<ObjectRef>> = BTreeMap::new();
        let mut blockers_by_attacker: BTreeMap<ObjectRef, BTreeSet<ObjectRef>> = BTreeMap::new();

        for assignment in blocks {
            if !distinct_assignments.insert(*assignment) {
                report
                    .violations
                    .insert(CombatRestrictionViolation::DuplicateBlockAssignment {
                        attacker: assignment.attacker,
                        blocker: assignment.blocker,
                    });
                continue;
            }

            let Some(defending_player) = attack_defenders.get(&assignment.attacker).copied() else {
                report.violations.insert(
                    CombatRestrictionViolation::BlockAgainstUndeclaredAttacker {
                        attacker: assignment.attacker,
                        blocker: assignment.blocker,
                    },
                );
                continue;
            };

            let attacker = resolve_participant(battlefield, assignment.attacker, &mut report);
            let blocker = resolve_participant(battlefield, assignment.blocker, &mut report);
            let (Some(attacker), Some(blocker)) = (attacker, blocker) else {
                continue;
            };

            if !attacker.is_creature {
                report
                    .violations
                    .insert(CombatRestrictionViolation::ParticipantIsNotCreature {
                        participant: assignment.attacker,
                    });
            }
            if !blocker.is_creature {
                report
                    .violations
                    .insert(CombatRestrictionViolation::ParticipantIsNotCreature {
                        participant: assignment.blocker,
                    });
            }
            if !attacker.is_creature || !blocker.is_creature {
                continue;
            }
            if blocker.controller != defending_player {
                report.violations.insert(
                    CombatRestrictionViolation::BlockerNotControlledByDefendingPlayer {
                        blocker: assignment.blocker,
                        blocker_controller: blocker.controller,
                        defending_player,
                    },
                );
            }

            attackers_by_blocker
                .entry(assignment.blocker)
                .or_default()
                .insert(assignment.attacker);
            blockers_by_attacker
                .entry(assignment.attacker)
                .or_default()
                .insert(assignment.blocker);

            for binding in self.active_bindings(assignment.blocker, battlefield) {
                if let CombatRestrictionKind::BlockerMayBlockOnlyAttackerWithKeyword {
                    required_keyword: CombatKeyword::Flying,
                } = binding.program.kind()
                {
                    match attacker.has_flying {
                        Some(true) => {}
                        Some(false) => {
                            report.violations.insert(
                                CombatRestrictionViolation::AttackerLacksRequiredKeyword {
                                    attacker: assignment.attacker,
                                    blocker: assignment.blocker,
                                    required_keyword: CombatKeyword::Flying,
                                },
                            );
                        }
                        None => {
                            report.ambiguities.insert(
                                CombatStateAmbiguity::AttackerKeywordUnknown {
                                    attacker: assignment.attacker,
                                    required_keyword: CombatKeyword::Flying,
                                },
                            );
                        }
                    }
                }
            }

            for binding in self.active_bindings(assignment.attacker, battlefield) {
                match binding.program.kind() {
                    CombatRestrictionKind::AttackerCannotBeBlockedByPowerAtMost {
                        maximum_power,
                    } => match blocker.effective_power_at_block_declaration {
                        Some(blocker_power) if blocker_power <= *maximum_power => {
                            report.violations.insert(
                                CombatRestrictionViolation::BlockerPowerAtOrBelowMaximum {
                                    attacker: assignment.attacker,
                                    blocker: assignment.blocker,
                                    blocker_power,
                                    maximum_power: *maximum_power,
                                },
                            );
                        }
                        Some(_) => {}
                        None => {
                            report
                                .ambiguities
                                .insert(CombatStateAmbiguity::BlockerPowerUnknown {
                                    blocker: assignment.blocker,
                                });
                        }
                    },
                    CombatRestrictionKind::AttackerCannotBeBlockedByLowerPower => {
                        match (
                            attacker.effective_power_at_block_declaration,
                            blocker.effective_power_at_block_declaration,
                        ) {
                            (Some(attacker_power), Some(blocker_power))
                                if blocker_power < attacker_power =>
                            {
                                report.violations.insert(
                                    CombatRestrictionViolation::BlockerPowerBelowAttacker {
                                        attacker: assignment.attacker,
                                        blocker: assignment.blocker,
                                        attacker_power,
                                        blocker_power,
                                    },
                                );
                            }
                            (Some(_), Some(_)) => {}
                            (None, _) => {
                                report.ambiguities.insert(
                                    CombatStateAmbiguity::AttackerPowerUnknown {
                                        attacker: assignment.attacker,
                                    },
                                );
                            }
                            (_, None) => {
                                report.ambiguities.insert(
                                    CombatStateAmbiguity::BlockerPowerUnknown {
                                        blocker: assignment.blocker,
                                    },
                                );
                            }
                        }
                    }
                    CombatRestrictionKind::AttackerCannotBeBlockedByFlying => {
                        match blocker.has_flying {
                            Some(true) => {
                                report.violations.insert(
                                    CombatRestrictionViolation::FlyingBlockerForbidden {
                                        attacker: assignment.attacker,
                                        blocker: assignment.blocker,
                                    },
                                );
                            }
                            Some(false) => {}
                            None => {
                                report.ambiguities.insert(
                                    CombatStateAmbiguity::BlockerKeywordUnknown {
                                        blocker: assignment.blocker,
                                        keyword: CombatKeyword::Flying,
                                    },
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        for (attacker, blockers) in blockers_by_attacker {
            let maximum = self
                .active_bindings(attacker, battlefield)
                .into_iter()
                .filter_map(|binding| match binding.program.kind() {
                    CombatRestrictionKind::AttackerMaximumBlockers { maximum_blockers } => {
                        Some(*maximum_blockers)
                    }
                    _ => None,
                })
                .min();
            if let Some(maximum_blockers) = maximum
                && blockers.len() > maximum_blockers
            {
                report
                    .violations
                    .insert(CombatRestrictionViolation::TooManyBlockers {
                        attacker,
                        actual_blockers: blockers.len(),
                        maximum_blockers,
                    });
            }
        }

        for (blocker, attackers) in attackers_by_blocker {
            let mut maximum_attackers = 1usize;
            for additional in self
                .active_bindings(blocker, battlefield)
                .into_iter()
                .filter_map(|binding| match binding.program.kind() {
                    CombatRestrictionKind::BlockerAdditionalCapacity {
                        additional_creatures,
                    } => Some(*additional_creatures),
                    _ => None,
                })
            {
                let Some(updated_capacity) = maximum_attackers.checked_add(additional) else {
                    report
                        .ambiguities
                        .insert(CombatStateAmbiguity::CapacityOverflow { blocker });
                    maximum_attackers = usize::MAX;
                    break;
                };
                maximum_attackers = updated_capacity;
            }
            if attackers.len() > maximum_attackers {
                report
                    .violations
                    .insert(CombatRestrictionViolation::BlockerCapacityExceeded {
                        blocker,
                        actual_attackers: attackers.len(),
                        maximum_attackers,
                    });
            }
        }

        report.finish()
    }

    pub fn validate_combat(
        &self,
        attacks: &[AttackDeclaration],
        blocks: &[BlockAssignment],
        battlefield: &BattlefieldSnapshot,
    ) -> CombatLegalityReport {
        let mut report = self.validate_attacks(attacks, battlefield);
        report.merge(self.validate_blocks(attacks, blocks, battlefield));
        report
    }

    fn active_bindings<'a>(
        &'a self,
        source: ObjectRef,
        battlefield: &BattlefieldSnapshot,
    ) -> Vec<&'a BoundCombatRestriction> {
        if battlefield.permanent(source).is_none() {
            return Vec::new();
        }
        self.bindings
            .iter()
            .filter(|binding| binding.source == source)
            .collect()
    }
}

impl Default for CombatRestrictionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

fn resolve_participant<'a>(
    battlefield: &'a BattlefieldSnapshot,
    participant: ObjectRef,
    report: &mut CombatLegalityBuilder,
) -> Option<&'a BattlefieldPermanent> {
    if let Some(permanent) = battlefield.permanent(participant) {
        return Some(permanent);
    }

    match battlefield.current_incarnation(participant.object_id) {
        Some(current_incarnation) => {
            report
                .ambiguities
                .insert(CombatStateAmbiguity::StaleBattlefieldIncarnation {
                    requested: participant,
                    current_incarnation,
                });
        }
        None => {
            report
                .ambiguities
                .insert(CombatStateAmbiguity::MissingBattlefieldObject { participant });
        }
    }
    None
}
