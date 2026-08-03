//! Exact production lifecycle for the supported delegated Enchant envelope.
//!
//! This module does not register global execution coverage or emit a runtime
//! receipt. It keeps target declaration, cost payment, the response window,
//! resolution, noncast entry, and Aura state based checks as separate events.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::bounded_oracle_runtime::{
    BOUNDED_ORACLE_COMPILER_VERSION, BOUNDED_ORACLE_RUNTIME_VERSION, BoundedOracleClause,
    CardType as BoundedCardType, Effect as BoundedEffect, ObjectFilter as BoundedObjectFilter,
    Restriction as BoundedRestriction, Timing as BoundedTiming, normalize_oracle_clause,
};
use crate::keyword_rules_runtime::{
    AttachmentFilter, CardType, EnchantProgram, KEYWORD_RULES_RUNTIME_VERSION, KeywordProgramKind,
    ManaColor, ObjectCharacteristics, ObjectPredicate, PlayerId, ProtectionQualitySpec,
    RelativePlayer, Zone,
};
use crate::oracle_clause_backend::{
    CompiledOracleClause, ORACLE_CLAUSE_BACKEND_COMPILER_VERSION,
    ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION,
};

pub(crate) const ENCHANT_PRODUCTION_RUNTIME_VERSION: &str = "enchant-production-runtime/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AuraSpellId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AuraObjectIdentity {
    pub(crate) incarnation: u64,
    pub(crate) card_index: Option<usize>,
}

impl AuraObjectIdentity {
    pub(crate) const fn card(incarnation: u64, card_index: usize) -> Self {
        Self {
            incarnation,
            card_index: Some(card_index),
        }
    }

    pub(crate) const fn token(incarnation: u64) -> Self {
        Self {
            incarnation,
            card_index: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuraLegalityStatus {
    Allowed,
    Forbidden,
    Unproven,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuraObjectSnapshot {
    pub(crate) identity: AuraObjectIdentity,
    pub(crate) owner: PlayerId,
    pub(crate) controller: PlayerId,
    pub(crate) zone: Zone,
    pub(crate) characteristics: ObjectCharacteristics,
    pub(crate) tapped: bool,
    /// Spell targeting legality. Shroud and Hexproof are represented here.
    pub(crate) targeting_status: AuraLegalityStatus,
    /// Continuous legality of being enchanted. Protection is represented here.
    pub(crate) enchanting_status: AuraLegalityStatus,
}

impl AuraObjectSnapshot {
    pub(crate) fn with_legality(
        mut self,
        targeting_status: AuraLegalityStatus,
        enchanting_status: AuraLegalityStatus,
    ) -> Self {
        self.targeting_status = targeting_status;
        self.enchanting_status = enchanting_status;
        self
    }

    fn is_noncreature_aura(&self) -> bool {
        self.characteristics
            .card_types
            .contains(&CardType::Enchantment)
            && self
                .characteristics
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case("aura"))
            && !self
                .characteristics
                .card_types
                .contains(&CardType::Creature)
    }

    fn is_permanent(&self) -> bool {
        self.characteristics.card_types.iter().any(|card_type| {
            matches!(
                card_type,
                CardType::Artifact
                    | CardType::Battle
                    | CardType::Creature
                    | CardType::Enchantment
                    | CardType::Land
                    | CardType::Planeswalker
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum EnchantTargetFamily {
    Artifact,
    ArtifactOrCreature,
    ArtifactOrCreatureYouControl,
    ArtifactYouControl,
    Creature,
    CreatureYouControl,
    Enchantment,
    Land,
    LandYouControl,
    NonlandPermanent,
    Permanent,
    Planeswalker,
    TappedCreature,
}

impl EnchantTargetFamily {
    fn requires_controller(self) -> Option<RelativePlayer> {
        match self {
            Self::ArtifactOrCreatureYouControl
            | Self::ArtifactYouControl
            | Self::CreatureYouControl
            | Self::LandYouControl => Some(RelativePlayer::You),
            _ => None,
        }
    }
}

/// Stable semantic identity for one Enchant clause.
///
/// Face and clause addresses are deliberately absent. They route the current
/// snapshot occurrence but cannot affect identity when unchanged Oracle text
/// moves to another row, face offset, or line offset.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AuraClauseIdentity {
    pub(crate) production_runtime_version: &'static str,
    pub(crate) backend_compiler_version: &'static str,
    pub(crate) backend_runtime_version: &'static str,
    pub(crate) semantic_compiler_version: &'static str,
    pub(crate) semantic_runtime_version: &'static str,
    pub(crate) backend_route: EnchantBackendRoute,
    pub(crate) backend_semantic_digest: String,
    pub(crate) source_type_signature: String,
    pub(crate) complete_oracle_clause: String,
    pub(crate) target_family: EnchantTargetFamily,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum EnchantBackendRoute {
    NativeBounded,
    DelegatedKeyword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuraClauseAddress {
    pub(crate) face_index: u16,
    pub(crate) clause_index: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundEnchantProgram {
    pub(crate) source_card_index: usize,
    pub(crate) clause_identity: AuraClauseIdentity,
    pub(crate) current_address: AuraClauseAddress,
    pub(crate) delegated_program: Option<EnchantProgram>,
    pub(crate) target_family: EnchantTargetFamily,
}

impl BoundEnchantProgram {
    pub(crate) fn bind(
        source_card_index: usize,
        source_type_line: &str,
        expected_face_index: u16,
        clause: &CompiledOracleClause,
    ) -> Result<Self, EnchantBindingError> {
        let source_type_signature = normalized_source_type_signature(source_type_line);
        if !source_type_is_noncreature_aura(&source_type_signature) {
            return Err(EnchantBindingError::IllegalAuraSourceType);
        }
        let address = clause.address();
        if address.face_index != expected_face_index {
            return Err(EnchantBindingError::ProgramAddressMismatch);
        }
        if clause.semantic_digest().trim().is_empty() {
            return Err(EnchantBindingError::MissingSemanticDigest);
        }
        let (
            backend_route,
            semantic_compiler_version,
            semantic_runtime_version,
            program,
            target_family,
        ) = match clause {
            CompiledOracleClause::Bounded(native) => {
                let target_family = exact_native_target_family(native)?;
                (
                    EnchantBackendRoute::NativeBounded,
                    BOUNDED_ORACLE_COMPILER_VERSION,
                    BOUNDED_ORACLE_RUNTIME_VERSION,
                    None,
                    target_family,
                )
            }
            CompiledOracleClause::Delegated(delegated) => {
                let keyword_program = delegated.keyword_program();
                if !keyword_program.has_exact_contract() {
                    return Err(EnchantBindingError::InexactProgramContract);
                }
                let KeywordProgramKind::Enchant(program) = keyword_program.kind() else {
                    return Err(EnchantBindingError::ProgramKindMismatch);
                };
                let source = keyword_program.source();
                if source.face_index != expected_face_index
                    || source.clause_index != address.clause_index
                {
                    return Err(EnchantBindingError::ProgramAddressMismatch);
                }
                let source_fragment_matches =
                    source.oracle_fragment.as_deref().is_some_and(|fragment| {
                        normalize_oracle_clause(fragment, "", source_type_line)
                            == clause.normalized_clause()
                    });
                if !source_fragment_matches {
                    return Err(EnchantBindingError::OracleFragmentMismatch);
                }
                if !program.aura_spell_targets || !program.all_enchant_abilities_must_match {
                    return Err(EnchantBindingError::IncompleteEnchantContract);
                }
                (
                    EnchantBackendRoute::DelegatedKeyword,
                    ORACLE_CLAUSE_BACKEND_COMPILER_VERSION,
                    KEYWORD_RULES_RUNTIME_VERSION,
                    Some(program.clone()),
                    exact_target_family(program)?,
                )
            }
        };
        Ok(Self {
            source_card_index,
            clause_identity: AuraClauseIdentity {
                production_runtime_version: ENCHANT_PRODUCTION_RUNTIME_VERSION,
                backend_compiler_version: ORACLE_CLAUSE_BACKEND_COMPILER_VERSION,
                backend_runtime_version: ORACLE_CLAUSE_BACKEND_RUNTIME_VERSION,
                semantic_compiler_version,
                semantic_runtime_version,
                backend_route,
                backend_semantic_digest: clause.semantic_digest().to_owned(),
                source_type_signature,
                complete_oracle_clause: clause.normalized_clause().to_owned(),
                target_family,
            },
            current_address: AuraClauseAddress {
                face_index: address.face_index,
                clause_index: address.clause_index,
            },
            delegated_program: program,
            target_family,
        })
    }

    pub(crate) fn target_matches(
        &self,
        target: &AuraObjectSnapshot,
        aura_controller: PlayerId,
    ) -> bool {
        if target.zone != Zone::Battlefield {
            return false;
        }
        if self
            .target_family
            .requires_controller()
            .is_some_and(|relation| {
                relation == RelativePlayer::You && target.controller != aura_controller
            })
        {
            return false;
        }
        let has = |card_type| target.characteristics.card_types.contains(&card_type);
        match self.target_family {
            EnchantTargetFamily::Artifact | EnchantTargetFamily::ArtifactYouControl => {
                has(CardType::Artifact)
            }
            EnchantTargetFamily::ArtifactOrCreature
            | EnchantTargetFamily::ArtifactOrCreatureYouControl => {
                has(CardType::Artifact) || has(CardType::Creature)
            }
            EnchantTargetFamily::Creature | EnchantTargetFamily::CreatureYouControl => {
                has(CardType::Creature)
            }
            EnchantTargetFamily::Enchantment => has(CardType::Enchantment),
            EnchantTargetFamily::Land | EnchantTargetFamily::LandYouControl => has(CardType::Land),
            EnchantTargetFamily::NonlandPermanent => target.is_permanent() && !has(CardType::Land),
            EnchantTargetFamily::Permanent => target.is_permanent(),
            EnchantTargetFamily::Planeswalker => has(CardType::Planeswalker),
            EnchantTargetFamily::TappedCreature => target.tapped && has(CardType::Creature),
        }
    }
}

fn normalized_source_type_signature(source_type_line: &str) -> String {
    source_type_line
        .chars()
        .map(|character| {
            if matches!(character, '-' | '\u{2013}' | '\u{2014}') {
                ' '
            } else {
                character.to_ascii_lowercase()
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn source_type_is_noncreature_aura(signature: &str) -> bool {
    let words = signature.split_whitespace().collect::<BTreeSet<_>>();
    words.contains("enchantment") && words.contains("aura") && !words.contains("creature")
}

fn exact_native_target_family(
    clause: &BoundedOracleClause,
) -> Result<EnchantTargetFamily, EnchantBindingError> {
    let mut expected_filter = BoundedObjectFilter::default();
    expected_filter.card_types.push(BoundedCardType::Creature);
    let exact_effect = matches!(
        clause.effects(),
        [BoundedEffect::Restriction(BoundedRestriction::EnchantRestriction { filter })]
            if *filter == expected_filter
    );
    if clause.timing() != &BoundedTiming::Static
        || !clause.conditions().is_empty()
        || !clause.costs().is_empty()
        || !clause.targets().is_empty()
        || clause.activation_restriction().is_some()
        || clause.reminder().is_some()
        || clause.saga_lore_procedure().is_some()
        || !exact_effect
    {
        return Err(EnchantBindingError::UnsupportedNativeEnchantContract);
    }
    Ok(EnchantTargetFamily::Creature)
}

fn exact_target_family(
    program: &EnchantProgram,
) -> Result<EnchantTargetFamily, EnchantBindingError> {
    let AttachmentFilter::Object(ObjectPredicate::All(predicates)) = &program.target_filter else {
        return Err(EnchantBindingError::UnsupportedTargetPredicate);
    };
    let mut zone = None;
    let mut controller = None;
    let mut tapped = false;
    let mut base = None;
    for predicate in predicates {
        match predicate {
            ObjectPredicate::Zone(candidate) if zone.replace(*candidate).is_none() => {}
            ObjectPredicate::Controller(candidate) if controller.replace(*candidate).is_none() => {}
            ObjectPredicate::Tapped if !tapped => tapped = true,
            candidate if base.replace(candidate).is_none() => {}
            _ => return Err(EnchantBindingError::UnsupportedTargetPredicate),
        }
    }
    if zone != Some(Zone::Battlefield) || !matches!(controller, None | Some(RelativePlayer::You)) {
        return Err(EnchantBindingError::UnsupportedTargetPredicate);
    }
    let Some(base) = base else {
        return Err(EnchantBindingError::UnsupportedTargetPredicate);
    };
    let family = match (base, controller, tapped) {
        (ObjectPredicate::CardType(CardType::Artifact), None, false) => {
            EnchantTargetFamily::Artifact
        }
        (ObjectPredicate::CardType(CardType::Artifact), Some(RelativePlayer::You), false) => {
            EnchantTargetFamily::ArtifactYouControl
        }
        (ObjectPredicate::CardType(CardType::Creature), None, false) => {
            EnchantTargetFamily::Creature
        }
        (ObjectPredicate::CardType(CardType::Creature), Some(RelativePlayer::You), false) => {
            EnchantTargetFamily::CreatureYouControl
        }
        (ObjectPredicate::CardType(CardType::Creature), None, true) => {
            EnchantTargetFamily::TappedCreature
        }
        (ObjectPredicate::CardType(CardType::Enchantment), None, false) => {
            EnchantTargetFamily::Enchantment
        }
        (ObjectPredicate::CardType(CardType::Land), None, false) => EnchantTargetFamily::Land,
        (ObjectPredicate::CardType(CardType::Land), Some(RelativePlayer::You), false) => {
            EnchantTargetFamily::LandYouControl
        }
        (ObjectPredicate::CardType(CardType::Planeswalker), None, false) => {
            EnchantTargetFamily::Planeswalker
        }
        (ObjectPredicate::Permanent, None, false) => EnchantTargetFamily::Permanent,
        (ObjectPredicate::Any(any), None, false) if is_artifact_or_creature(any) => {
            EnchantTargetFamily::ArtifactOrCreature
        }
        (ObjectPredicate::Any(any), Some(RelativePlayer::You), false)
            if is_artifact_or_creature(any) =>
        {
            EnchantTargetFamily::ArtifactOrCreatureYouControl
        }
        (ObjectPredicate::All(all), None, false) if is_nonland_permanent(all) => {
            EnchantTargetFamily::NonlandPermanent
        }
        _ => return Err(EnchantBindingError::UnsupportedTargetPredicate),
    };
    Ok(family)
}

fn is_artifact_or_creature(predicates: &[ObjectPredicate]) -> bool {
    predicates.len() == 2
        && predicates.contains(&ObjectPredicate::CardType(CardType::Artifact))
        && predicates.contains(&ObjectPredicate::CardType(CardType::Creature))
}

fn is_nonland_permanent(predicates: &[ObjectPredicate]) -> bool {
    predicates.len() == 2
        && predicates.contains(&ObjectPredicate::Permanent)
        && predicates.iter().any(|predicate| {
            *predicate == ObjectPredicate::Not(Box::new(ObjectPredicate::CardType(CardType::Land)))
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuraCastSource {
    pub(crate) identity: AuraObjectIdentity,
    pub(crate) card_index: usize,
    pub(crate) owner: PlayerId,
    pub(crate) controller: PlayerId,
    pub(crate) origin_zone: Zone,
    pub(crate) characteristics: ObjectCharacteristics,
}

impl AuraCastSource {
    fn is_noncreature_aura(&self) -> bool {
        AuraObjectSnapshot {
            identity: self.identity,
            owner: self.owner,
            controller: self.controller,
            zone: self.origin_zone,
            characteristics: self.characteristics.clone(),
            tapped: false,
            targeting_status: AuraLegalityStatus::Allowed,
            enchanting_status: AuraLegalityStatus::Allowed,
        }
        .is_noncreature_aura()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingAuraSpell {
    pub(crate) spell_id: AuraSpellId,
    pub(crate) source: AuraObjectIdentity,
    pub(crate) source_card_index: usize,
    pub(crate) owner: PlayerId,
    pub(crate) casting_controller: PlayerId,
    pub(crate) declared_target: AuraObjectIdentity,
    pub(crate) binding: BoundEnchantProgram,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PendingAuraSpells {
    next_spell_id: u64,
    pending: BTreeMap<AuraSpellId, PendingAuraSpell>,
}

impl PendingAuraSpells {
    /// Validates and declares the target before invoking the source-card cost
    /// callback. The Enchant clause supplies no casting cost.
    pub(crate) fn cast_with_source_cost<F>(
        &mut self,
        binding: &BoundEnchantProgram,
        source: &AuraCastSource,
        target: &AuraObjectSnapshot,
        pay_source_card_cost: F,
    ) -> Result<PendingAuraSpell, AuraCastError>
    where
        F: FnOnce() -> bool,
    {
        if binding.clause_identity.production_runtime_version != ENCHANT_PRODUCTION_RUNTIME_VERSION
        {
            return Err(AuraCastError::BindingVersionMismatch);
        }
        if source.card_index != binding.source_card_index
            || source.identity.card_index != Some(source.card_index)
        {
            return Err(AuraCastError::SourceDefinitionMismatch);
        }
        // Ordinary Enchant grants no permission to cast from another zone.
        // A future non-Hand origin must supply its own proven permission token.
        if source.origin_zone != Zone::Hand {
            return Err(AuraCastError::IllegalCastOrigin);
        }
        if !source.is_noncreature_aura() {
            return Err(AuraCastError::IllegalAuraSource);
        }
        if !binding.target_matches(target, source.controller) {
            return Err(AuraCastError::IllegalDeclaredTarget);
        }
        if target.targeting_status != AuraLegalityStatus::Allowed
            || target.enchanting_status != AuraLegalityStatus::Allowed
        {
            return Err(AuraCastError::TargetingForbiddenOrUnproven);
        }
        let next = self
            .next_spell_id
            .checked_add(1)
            .filter(|candidate| *candidate != 0)
            .ok_or(AuraCastError::SpellIdentityExhausted)?;
        if source.identity == target.identity {
            return Err(AuraCastError::SelfTarget);
        }
        if self
            .pending
            .values()
            .any(|pending| pending.source == source.identity)
        {
            return Err(AuraCastError::SourceAlreadyOnStack);
        }
        if !pay_source_card_cost() {
            return Err(AuraCastError::CostNotPaid);
        }
        let spell_id = AuraSpellId(next);
        let pending = PendingAuraSpell {
            spell_id,
            source: source.identity,
            source_card_index: source.card_index,
            owner: source.owner,
            casting_controller: source.controller,
            declared_target: target.identity,
            binding: binding.clone(),
        };
        if self.pending.insert(spell_id, pending.clone()).is_some() {
            return Err(AuraCastError::SpellIdentityCollision);
        }
        self.next_spell_id = next;
        Ok(pending)
    }

    pub(crate) fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub(crate) fn resolve(
        &mut self,
        spell_id: AuraSpellId,
        source: Option<&AuraObjectSnapshot>,
        target: Option<&AuraObjectSnapshot>,
    ) -> Result<AuraResolutionRecord, AuraResolutionError> {
        let pending = self
            .pending
            .remove(&spell_id)
            .ok_or(AuraResolutionError::MissingPendingSpell)?;
        let outcome = match source {
            None => AuraResolutionOutcome::SourceGone {
                source: pending.source,
            },
            Some(source) if source.identity != pending.source => {
                AuraResolutionOutcome::SourceIdentityChanged {
                    expected: pending.source,
                    actual: source.identity,
                }
            }
            Some(source)
                if source.zone != Zone::Stack
                    || source.identity.card_index != Some(pending.source_card_index)
                    || source.owner != pending.owner
                    || !source.is_noncreature_aura() =>
            {
                AuraResolutionOutcome::SourceIllegal {
                    source: pending.source,
                }
            }
            Some(source) => {
                let failure = match target {
                    None => Some(AuraTargetFailure::Missing),
                    Some(target) if target.identity != pending.declared_target => {
                        Some(AuraTargetFailure::IdentityChanged)
                    }
                    Some(target)
                        if !pending.binding.target_matches(target, source.controller)
                            || target.targeting_status != AuraLegalityStatus::Allowed
                            || target.enchanting_status != AuraLegalityStatus::Allowed =>
                    {
                        Some(AuraTargetFailure::Illegal)
                    }
                    Some(_) => None,
                };
                if let Some(reason) = failure {
                    AuraResolutionOutcome::FizzledToOwnerGraveyard {
                        source: pending.source,
                        card_index: pending.source_card_index,
                        owner: pending.owner,
                        reason,
                    }
                } else {
                    AuraResolutionOutcome::Attached {
                        source: pending.source,
                        card_index: pending.source_card_index,
                        owner: pending.owner,
                        controller: source.controller,
                        target: pending.declared_target,
                    }
                }
            }
        };
        Ok(AuraResolutionRecord {
            spell_id,
            clause_identity: pending.binding.clause_identity,
            outcome,
        })
    }

    pub(crate) fn counter(
        &mut self,
        spell_id: AuraSpellId,
        source: Option<&AuraObjectSnapshot>,
    ) -> Result<AuraResolutionRecord, AuraResolutionError> {
        let pending = self
            .pending
            .remove(&spell_id)
            .ok_or(AuraResolutionError::MissingPendingSpell)?;
        let outcome = match source {
            None => AuraResolutionOutcome::SourceGone {
                source: pending.source,
            },
            Some(source)
                if source.identity == pending.source
                    && source.zone == Zone::Stack
                    && source.identity.card_index == Some(pending.source_card_index)
                    && source.owner == pending.owner =>
            {
                AuraResolutionOutcome::CounteredToOwnerGraveyard {
                    source: pending.source,
                    card_index: pending.source_card_index,
                    owner: pending.owner,
                }
            }
            Some(source) => AuraResolutionOutcome::SourceIdentityChanged {
                expected: pending.source,
                actual: source.identity,
            },
        };
        Ok(AuraResolutionRecord {
            spell_id,
            clause_identity: pending.binding.clause_identity,
            outcome,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuraResolutionRecord {
    pub(crate) spell_id: AuraSpellId,
    pub(crate) clause_identity: AuraClauseIdentity,
    pub(crate) outcome: AuraResolutionOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuraTargetFailure {
    Missing,
    IdentityChanged,
    Illegal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuraResolutionOutcome {
    Attached {
        source: AuraObjectIdentity,
        card_index: usize,
        owner: PlayerId,
        controller: PlayerId,
        target: AuraObjectIdentity,
    },
    CounteredToOwnerGraveyard {
        source: AuraObjectIdentity,
        card_index: usize,
        owner: PlayerId,
    },
    FizzledToOwnerGraveyard {
        source: AuraObjectIdentity,
        card_index: usize,
        owner: PlayerId,
        reason: AuraTargetFailure,
    },
    SourceGone {
        source: AuraObjectIdentity,
    },
    SourceIdentityChanged {
        expected: AuraObjectIdentity,
        actual: AuraObjectIdentity,
    },
    SourceIllegal {
        source: AuraObjectIdentity,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoncastAuraEntryOutcome {
    Attached {
        source: AuraObjectIdentity,
        card_index: usize,
        owner: PlayerId,
        controller: PlayerId,
        target: AuraObjectIdentity,
    },
    RemainsInPreviousZone {
        source: AuraObjectIdentity,
        previous_zone: Zone,
    },
}

pub(crate) fn attempt_noncast_aura_entry(
    binding: &BoundEnchantProgram,
    source: &AuraObjectSnapshot,
    target: Option<&AuraObjectSnapshot>,
) -> Result<NoncastAuraEntryOutcome, NoncastAuraEntryError> {
    if source.identity.card_index != Some(binding.source_card_index) {
        return Err(NoncastAuraEntryError::SourceDefinitionMismatch);
    }
    if source.zone == Zone::Battlefield {
        return Err(NoncastAuraEntryError::SourceAlreadyOnBattlefield);
    }
    if !source.is_noncreature_aura() {
        return Err(NoncastAuraEntryError::IllegalAuraSource);
    }
    let legal = target.is_some_and(|target| {
        target.identity != source.identity
            && binding.target_matches(target, source.controller)
            && target.enchanting_status == AuraLegalityStatus::Allowed
    });
    Ok(if legal {
        let target = target.expect("the checked target is present");
        NoncastAuraEntryOutcome::Attached {
            source: source.identity,
            card_index: binding.source_card_index,
            owner: source.owner,
            controller: source.controller,
            target: target.identity,
        }
    } else {
        NoncastAuraEntryOutcome::RemainsInPreviousZone {
            source: source.identity,
            previous_zone: source.zone,
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuraStateBasedOutcome {
    Legal,
    MoveToOwnerGraveyard {
        source: AuraObjectIdentity,
        card_index: usize,
        owner: PlayerId,
        reason: AuraStateBasedFailure,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuraStateBasedFailure {
    IllegalAuraSource,
    Unattached,
    TargetIdentityChanged,
    IllegalAttachment,
}

pub(crate) fn check_aura_state_based_attachment(
    binding: &BoundEnchantProgram,
    aura: &AuraObjectSnapshot,
    recorded_target: Option<AuraObjectIdentity>,
    current_target: Option<&AuraObjectSnapshot>,
) -> Result<AuraStateBasedOutcome, AuraStateBasedError> {
    if aura.identity.card_index != Some(binding.source_card_index) {
        return Err(AuraStateBasedError::SourceDefinitionMismatch);
    }
    if aura.zone != Zone::Battlefield {
        return Err(AuraStateBasedError::SourceNotOnBattlefield);
    }
    let failure = if !aura.is_noncreature_aura() {
        Some(AuraStateBasedFailure::IllegalAuraSource)
    } else {
        match (recorded_target, current_target) {
            (None, _) | (Some(_), None) => Some(AuraStateBasedFailure::Unattached),
            (Some(recorded), Some(target)) if recorded != target.identity => {
                Some(AuraStateBasedFailure::TargetIdentityChanged)
            }
            (Some(_), Some(target))
                if !binding.target_matches(target, aura.controller)
                    || target.enchanting_status != AuraLegalityStatus::Allowed =>
            {
                Some(AuraStateBasedFailure::IllegalAttachment)
            }
            (Some(_), Some(_)) => None,
        }
    };
    Ok(match failure {
        None => AuraStateBasedOutcome::Legal,
        Some(reason) => AuraStateBasedOutcome::MoveToOwnerGraveyard {
            source: aura.identity,
            card_index: binding.source_card_index,
            owner: aura.owner,
            reason,
        },
    })
}

pub(crate) fn protection_forbids_aura_source(
    qualities: &[ProtectionQualitySpec],
    source: &AuraObjectSnapshot,
) -> Result<bool, AuraProtectionError> {
    for quality in qualities {
        let matched = match quality {
            ProtectionQualitySpec::Everything => true,
            ProtectionQualitySpec::Color(color) => source.characteristics.colors.contains(color),
            ProtectionQualitySpec::EachColor => source
                .characteristics
                .colors
                .iter()
                .any(|color| !matches!(color, ManaColor::Colorless)),
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
                .any(|candidate| card_type.eq_ignore_ascii_case(card_type_name(*candidate))),
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
            ProtectionQualitySpec::ChosenColor | ProtectionQualitySpec::ChosenPlayer => {
                return Err(AuraProtectionError::UntrackedProtectionChoice);
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
pub(crate) enum EnchantBindingError {
    IllegalAuraSourceType,
    InexactProgramContract,
    ProgramKindMismatch,
    ProgramAddressMismatch,
    OracleFragmentMismatch,
    MissingSemanticDigest,
    IncompleteEnchantContract,
    UnsupportedNativeEnchantContract,
    UnsupportedTargetPredicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuraCastError {
    BindingVersionMismatch,
    SourceDefinitionMismatch,
    IllegalCastOrigin,
    IllegalAuraSource,
    IllegalDeclaredTarget,
    TargetingForbiddenOrUnproven,
    SelfTarget,
    SpellIdentityExhausted,
    SourceAlreadyOnStack,
    CostNotPaid,
    SpellIdentityCollision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuraResolutionError {
    MissingPendingSpell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NoncastAuraEntryError {
    SourceDefinitionMismatch,
    SourceAlreadyOnBattlefield,
    IllegalAuraSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuraStateBasedError {
    SourceDefinitionMismatch,
    SourceNotOnBattlefield,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuraProtectionError {
    UntrackedProtectionChoice,
}

macro_rules! debug_display_error {
    ($($error:ty),+ $(,)?) => {
        $(
            impl fmt::Display for $error {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(formatter, "{self:?}")
                }
            }

            impl std::error::Error for $error {}
        )+
    };
}

debug_display_error!(
    EnchantBindingError,
    AuraCastError,
    AuraResolutionError,
    NoncastAuraEntryError,
    AuraStateBasedError,
    AuraProtectionError,
);
