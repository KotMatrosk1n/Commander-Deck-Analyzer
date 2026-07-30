//! Stateful production transaction for exact delegated Equip programs.
//!
//! This module deliberately does not register execution coverage or create a
//! runtime receipt. It binds one occurrence-addressed delegated program to one
//! physical battlefield object, records activation separately from resolution,
//! and leaves the authoritative mana and attachment mutations to the
//! simulation adapter.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;

use crate::keyword_rules_runtime::{
    CardType, EquipProgram, KeywordProgramKind, ManaColor, ManaCost, ManaSymbol,
    ObjectCharacteristics, ObjectPredicate, PlayerId, ProtectionQualitySpec, RelativePlayer, Zone,
};
use crate::oracle_clause_backend::DelegatedKeywordClause;

pub(crate) const EQUIP_PRODUCTION_RUNTIME_VERSION: &str = "equip-production-runtime/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct EquipActivationId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct EquipObjectIdentity {
    pub(crate) sequence: u16,
    pub(crate) card_index: Option<usize>,
}

impl EquipObjectIdentity {
    pub(crate) const fn card(sequence: u16, card_index: usize) -> Self {
        Self {
            sequence,
            card_index: Some(card_index),
        }
    }

    pub(crate) const fn token(sequence: u16) -> Self {
        Self {
            sequence,
            card_index: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EquipTargetingStatus {
    Allowed,
    Forbidden,
    Unproven,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EquipObjectSnapshot {
    pub(crate) identity: EquipObjectIdentity,
    pub(crate) owner: PlayerId,
    pub(crate) controller: PlayerId,
    pub(crate) zone: Zone,
    pub(crate) characteristics: ObjectCharacteristics,
    pub(crate) is_commander: bool,
    pub(crate) has_reconfigure: bool,
    pub(crate) targeting_status: EquipTargetingStatus,
}

impl EquipObjectSnapshot {
    pub(crate) fn permits_targeting(mut self, status: EquipTargetingStatus) -> Self {
        self.targeting_status = status;
        self
    }

    fn is_artifact_equipment(&self) -> bool {
        self.characteristics
            .card_types
            .contains(&CardType::Artifact)
            && self
                .characteristics
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case("equipment"))
    }

    fn can_be_attached_as_equipment(&self) -> bool {
        self.is_artifact_equipment()
            && !self
                .characteristics
                .card_types
                .contains(&CardType::Creature)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum EquipTargetFamily {
    Creature,
    LegendaryCreature,
    CommanderCreature,
    Planeswalker,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EquipClauseIdentity {
    pub(crate) runtime_version: String,
    pub(crate) semantic_digest: String,
    pub(crate) face_index: u16,
    pub(crate) clause_index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundDelegatedEquipProgram {
    pub(crate) bridge_version: &'static str,
    pub(crate) source_object: EquipObjectIdentity,
    pub(crate) source_card_index: usize,
    pub(crate) clause: EquipClauseIdentity,
    pub(crate) program: EquipProgram,
    pub(crate) target_family: EquipTargetFamily,
}

impl BoundDelegatedEquipProgram {
    pub(crate) fn bind(
        source_object: EquipObjectIdentity,
        source_card_index: usize,
        expected_face_index: u16,
        clause: &DelegatedKeywordClause,
    ) -> Result<Self, EquipBindingError> {
        if source_object.card_index != Some(source_card_index) {
            return Err(EquipBindingError::SourceDefinitionMismatch);
        }
        let program = clause.keyword_program();
        if !program.has_exact_contract() {
            return Err(EquipBindingError::InexactProgramContract);
        }
        let KeywordProgramKind::Equip(equip) = program.kind() else {
            return Err(EquipBindingError::ProgramKindMismatch);
        };
        let address = clause.address();
        let source = program.source();
        if address.face_index != expected_face_index
            || source.face_index != expected_face_index
            || source.clause_index != address.clause_index
        {
            return Err(EquipBindingError::ProgramAddressMismatch);
        }
        if source.oracle_fragment.as_deref() != Some(clause.normalized_clause()) {
            return Err(EquipBindingError::OracleFragmentMismatch);
        }
        if clause.semantic_digest().trim().is_empty() {
            return Err(EquipBindingError::MissingSemanticDigest);
        }
        if equip.activation_cost.symbols.is_empty()
            || !equip.activation_cost.symbols.iter().all(|symbol| {
                matches!(
                    symbol,
                    ManaSymbol::Generic(_)
                        | ManaSymbol::Colored(
                            ManaColor::White
                                | ManaColor::Blue
                                | ManaColor::Black
                                | ManaColor::Red
                                | ManaColor::Green
                        )
                )
            })
        {
            return Err(EquipBindingError::UnsupportedActivationCost);
        }
        if !equip.sorcery_timing_only {
            return Err(EquipBindingError::UnsupportedTimingContract);
        }
        let target_family = exact_target_family(equip)?;
        Ok(Self {
            bridge_version: EQUIP_PRODUCTION_RUNTIME_VERSION,
            source_object,
            source_card_index,
            clause: EquipClauseIdentity {
                runtime_version: clause.runtime_version().to_owned(),
                semantic_digest: clause.semantic_digest().to_owned(),
                face_index: address.face_index,
                clause_index: address.clause_index,
            },
            program: equip.clone(),
            target_family,
        })
    }

    pub(crate) fn target_matches(
        &self,
        target: &EquipObjectSnapshot,
        activating_player: PlayerId,
    ) -> bool {
        target.zone == Zone::Battlefield
            && target.controller == activating_player
            && match self.target_family {
                EquipTargetFamily::Creature => target
                    .characteristics
                    .card_types
                    .contains(&CardType::Creature),
                EquipTargetFamily::LegendaryCreature => {
                    target
                        .characteristics
                        .card_types
                        .contains(&CardType::Creature)
                        && target
                            .characteristics
                            .supertypes
                            .iter()
                            .any(|supertype| supertype.eq_ignore_ascii_case("legendary"))
                }
                EquipTargetFamily::CommanderCreature => {
                    target.is_commander
                        && target
                            .characteristics
                            .card_types
                            .contains(&CardType::Creature)
                }
                EquipTargetFamily::Planeswalker => target
                    .characteristics
                    .card_types
                    .contains(&CardType::Planeswalker),
            }
    }
}

fn exact_target_family(program: &EquipProgram) -> Result<EquipTargetFamily, EquipBindingError> {
    let ObjectPredicate::All(predicates) = &program.target_filter else {
        return Err(EquipBindingError::UnsupportedTargetPredicate);
    };
    let mut card_type = None;
    let mut controller = None;
    let mut zone = None;
    let mut legendary = false;
    let mut commander = false;
    for predicate in predicates {
        match predicate {
            ObjectPredicate::CardType(candidate) if card_type.replace(*candidate).is_none() => {}
            ObjectPredicate::Controller(candidate) if controller.replace(*candidate).is_none() => {}
            ObjectPredicate::Zone(candidate) if zone.replace(*candidate).is_none() => {}
            ObjectPredicate::Supertype(supertype)
                if !legendary && supertype.eq_ignore_ascii_case("legendary") =>
            {
                legendary = true;
            }
            ObjectPredicate::Commander if !commander => commander = true,
            _ => return Err(EquipBindingError::UnsupportedTargetPredicate),
        }
    }
    if controller != Some(RelativePlayer::You) || zone != Some(Zone::Battlefield) {
        return Err(EquipBindingError::UnsupportedTargetPredicate);
    }
    match (
        card_type,
        legendary,
        commander,
        program.planeswalker_as_creature,
    ) {
        (Some(CardType::Creature), false, false, false) => Ok(EquipTargetFamily::Creature),
        (Some(CardType::Creature), true, false, false) => Ok(EquipTargetFamily::LegendaryCreature),
        (Some(CardType::Creature), false, true, false) => Ok(EquipTargetFamily::CommanderCreature),
        (Some(CardType::Planeswalker), false, false, true) => Ok(EquipTargetFamily::Planeswalker),
        _ => Err(EquipBindingError::UnsupportedTargetPredicate),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EquipTurnPhase {
    PrecombatMain,
    PostcombatMain,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EquipActivationContext {
    pub(crate) actor: PlayerId,
    pub(crate) active_player: PlayerId,
    pub(crate) priority_player: Option<PlayerId>,
    pub(crate) phase: EquipTurnPhase,
    pub(crate) stack_depth: usize,
}

impl EquipActivationContext {
    pub(crate) const fn active_precombat_main(actor: PlayerId) -> Self {
        Self {
            actor,
            active_player: actor,
            priority_player: Some(actor),
            phase: EquipTurnPhase::PrecombatMain,
            stack_depth: 0,
        }
    }

    fn permits_sorcery_timing(self) -> bool {
        self.actor == self.active_player
            && self.priority_player == Some(self.actor)
            && matches!(
                self.phase,
                EquipTurnPhase::PrecombatMain | EquipTurnPhase::PostcombatMain
            )
            && self.stack_depth == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EquipActivationRecord {
    pub(crate) activation_id: EquipActivationId,
    pub(crate) actor: PlayerId,
    pub(crate) source: EquipObjectIdentity,
    pub(crate) target: EquipObjectIdentity,
    pub(crate) binding: BoundDelegatedEquipProgram,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PendingEquipActivations {
    next_activation_id: u64,
    pending: BTreeMap<EquipActivationId, EquipActivationRecord>,
}

impl PendingEquipActivations {
    pub(crate) fn activate_with_payment<F>(
        &mut self,
        binding: &BoundDelegatedEquipProgram,
        context: EquipActivationContext,
        source: &EquipObjectSnapshot,
        target: &EquipObjectSnapshot,
        pay: F,
    ) -> Result<EquipActivationRecord, EquipActivationError>
    where
        F: FnOnce(&ManaCost) -> bool,
    {
        if binding.bridge_version != EQUIP_PRODUCTION_RUNTIME_VERSION {
            return Err(EquipActivationError::BindingVersionMismatch);
        }
        if !context.permits_sorcery_timing() || !binding.program.sorcery_timing_only {
            return Err(EquipActivationError::InvalidTiming);
        }
        if source.identity != binding.source_object
            || source.identity.card_index != Some(binding.source_card_index)
        {
            return Err(EquipActivationError::SourceIdentityMismatch);
        }
        if source.zone != Zone::Battlefield {
            return Err(EquipActivationError::SourceNotOnBattlefield);
        }
        if source.controller != context.actor {
            return Err(EquipActivationError::WrongSourceController);
        }
        if !source.can_be_attached_as_equipment() {
            return Err(EquipActivationError::IllegalEquipmentSource);
        }
        if source.identity == target.identity {
            return Err(EquipActivationError::SelfTarget);
        }
        if !binding.target_matches(target, context.actor) {
            return Err(EquipActivationError::IllegalTarget);
        }
        if target.targeting_status != EquipTargetingStatus::Allowed {
            return Err(EquipActivationError::TargetingForbiddenOrUnproven);
        }
        let next = self
            .next_activation_id
            .checked_add(1)
            .filter(|candidate| *candidate != 0)
            .ok_or(EquipActivationError::ActivationIdentityExhausted)?;
        if !pay(&binding.program.activation_cost) {
            return Err(EquipActivationError::CostNotPaid);
        }
        let activation_id = EquipActivationId(next);
        let record = EquipActivationRecord {
            activation_id,
            actor: context.actor,
            source: source.identity,
            target: target.identity,
            binding: binding.clone(),
        };
        if self.pending.insert(activation_id, record.clone()).is_some() {
            return Err(EquipActivationError::ActivationIdentityCollision);
        }
        self.next_activation_id = next;
        Ok(record)
    }

    pub(crate) fn copy_pending_activation(
        &mut self,
        original: EquipActivationId,
    ) -> Result<EquipActivationRecord, EquipActivationError> {
        let mut copied = self
            .pending
            .get(&original)
            .cloned()
            .ok_or(EquipActivationError::MissingActivationToCopy)?;
        let next = self
            .next_activation_id
            .checked_add(1)
            .filter(|candidate| *candidate != 0)
            .ok_or(EquipActivationError::ActivationIdentityExhausted)?;
        copied.activation_id = EquipActivationId(next);
        if self
            .pending
            .insert(copied.activation_id, copied.clone())
            .is_some()
        {
            return Err(EquipActivationError::ActivationIdentityCollision);
        }
        self.next_activation_id = next;
        Ok(copied)
    }

    pub(crate) fn resolve(
        &mut self,
        activation_id: EquipActivationId,
        source: Option<&EquipObjectSnapshot>,
        target: Option<&EquipObjectSnapshot>,
    ) -> Result<EquipResolutionRecord, EquipResolutionError> {
        let pending = self
            .pending
            .remove(&activation_id)
            .ok_or(EquipResolutionError::MissingPendingActivation)?;
        let outcome = match source {
            None => EquipResolutionOutcome::Failed(EquipResolutionFailure::SourceMissing),
            Some(source) if source.identity != pending.source => {
                EquipResolutionOutcome::Failed(EquipResolutionFailure::SourceIdentityChanged)
            }
            Some(source)
                if source.zone != Zone::Battlefield || !source.can_be_attached_as_equipment() =>
            {
                EquipResolutionOutcome::Failed(EquipResolutionFailure::SourceIllegal)
            }
            Some(_) => match target {
                None => EquipResolutionOutcome::Failed(EquipResolutionFailure::TargetMissing),
                Some(target) if target.identity != pending.target => {
                    EquipResolutionOutcome::Failed(EquipResolutionFailure::TargetIdentityChanged)
                }
                Some(target)
                    if !pending.binding.target_matches(target, pending.actor)
                        || target.targeting_status != EquipTargetingStatus::Allowed =>
                {
                    EquipResolutionOutcome::Failed(EquipResolutionFailure::TargetIllegal)
                }
                Some(target) => EquipResolutionOutcome::Attached {
                    source: pending.source,
                    target: target.identity,
                },
            },
        };
        Ok(EquipResolutionRecord {
            activation_id,
            clause: pending.binding.clause,
            outcome,
        })
    }

    pub(crate) fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub(crate) fn pending_for_source(&self, source: EquipObjectIdentity) -> usize {
        self.pending
            .values()
            .filter(|pending| pending.source == source)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EquipResolutionRecord {
    pub(crate) activation_id: EquipActivationId,
    pub(crate) clause: EquipClauseIdentity,
    pub(crate) outcome: EquipResolutionOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EquipResolutionOutcome {
    Attached {
        source: EquipObjectIdentity,
        target: EquipObjectIdentity,
    },
    Failed(EquipResolutionFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EquipResolutionFailure {
    SourceMissing,
    SourceIdentityChanged,
    SourceIllegal,
    TargetMissing,
    TargetIdentityChanged,
    TargetIllegal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EquipBindingError {
    SourceDefinitionMismatch,
    InexactProgramContract,
    ProgramKindMismatch,
    ProgramAddressMismatch,
    OracleFragmentMismatch,
    MissingSemanticDigest,
    UnsupportedActivationCost,
    UnsupportedTimingContract,
    UnsupportedTargetPredicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EquipActivationError {
    BindingVersionMismatch,
    InvalidTiming,
    SourceIdentityMismatch,
    SourceNotOnBattlefield,
    WrongSourceController,
    IllegalEquipmentSource,
    SelfTarget,
    IllegalTarget,
    TargetingForbiddenOrUnproven,
    ActivationIdentityExhausted,
    CostNotPaid,
    ActivationIdentityCollision,
    MissingActivationToCopy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EquipResolutionError {
    MissingPendingActivation,
}

impl fmt::Display for EquipBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl fmt::Display for EquipActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl fmt::Display for EquipResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EquipBindingError {}
impl std::error::Error for EquipActivationError {}
impl std::error::Error for EquipResolutionError {}

pub(crate) fn protection_forbids_source(
    qualities: &[ProtectionQualitySpec],
    source: &EquipObjectSnapshot,
) -> Result<bool, EquipProtectionError> {
    for quality in qualities {
        let matched = match quality {
            ProtectionQualitySpec::Everything => true,
            ProtectionQualitySpec::Color(color) => source.characteristics.colors.contains(color),
            ProtectionQualitySpec::EachColor => source
                .characteristics
                .colors
                .iter()
                .any(|color| !matches!(color, ManaColor::Colorless)),
            ProtectionQualitySpec::ChosenColor | ProtectionQualitySpec::ChosenPlayer => {
                return Err(EquipProtectionError::UntrackedProtectionChoice);
            }
            ProtectionQualitySpec::Colored => source
                .characteristics
                .colors
                .iter()
                .any(|color| !matches!(color, ManaColor::Colorless)),
            ProtectionQualitySpec::Colorless => source
                .characteristics
                .colors
                .iter()
                .all(|color| matches!(color, ManaColor::Colorless)),
            ProtectionQualitySpec::Monocolored => {
                source
                    .characteristics
                    .colors
                    .iter()
                    .filter(|color| !matches!(color, ManaColor::Colorless))
                    .count()
                    == 1
            }
            ProtectionQualitySpec::Multicolored => {
                source
                    .characteristics
                    .colors
                    .iter()
                    .filter(|color| !matches!(color, ManaColor::Colorless))
                    .count()
                    > 1
            }
            ProtectionQualitySpec::CardType(card_type) => source
                .characteristics
                .card_types
                .iter()
                .any(|candidate| card_type_name(*candidate).eq_ignore_ascii_case(card_type)),
            ProtectionQualitySpec::Subtype(subtype) => source
                .characteristics
                .subtypes
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(subtype)),
            ProtectionQualitySpec::Named(name) => source
                .characteristics
                .name
                .as_deref()
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name)),
            ProtectionQualitySpec::ManaValueAtMost(maximum) => {
                source.characteristics.mana_value <= *maximum
            }
            ProtectionQualitySpec::ManaValueAtLeast(minimum) => {
                source.characteristics.mana_value >= *minimum
            }
        };
        if matched {
            return Ok(true);
        }
    }
    Ok(false)
}

fn card_type_name(card_type: CardType) -> &'static str {
    match card_type {
        CardType::Artifact => "artifact",
        CardType::Battle => "battle",
        CardType::Creature => "creature",
        CardType::Enchantment => "enchantment",
        CardType::Instant => "instant",
        CardType::Kindred => "kindred",
        CardType::Land => "land",
        CardType::Planeswalker => "planeswalker",
        CardType::Sorcery => "sorcery",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EquipProtectionError {
    UntrackedProtectionChoice,
}
