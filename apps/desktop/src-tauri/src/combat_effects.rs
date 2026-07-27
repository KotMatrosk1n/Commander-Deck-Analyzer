//! Name-independent combat-effect evaluation primitives.
//!
//! This module is a deliberately bounded bridge between typed Oracle
//! compilation and the simulator's battlefield representation. It does not
//! parse card text and contains no deck, commander, or card-name switches.
//! Callers provide normalized permanent characteristics plus typed effects.
//!
//! The evaluator currently models only monotonic keyword grants and additive
//! power/toughness modifiers. It is not a replacement for Magic's full layer,
//! dependency, attachment-legality, blocking, or combat-damage rules.

#![allow(dead_code)]

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PlayerId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum PermanentType {
    Artifact,
    Battle,
    Creature,
    Enchantment,
    Land,
    Planeswalker,
    Kindred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum CombatKeyword {
    CantBeBlocked,
    Deathtouch,
    DoubleStrike,
    FirstStrike,
    Flying,
    Haste,
    Hexproof,
    Indestructible,
    Lifelink,
    Menace,
    Reach,
    Shroud,
    Trample,
    Vigilance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum AttachmentKind {
    Aura,
    Equipment,
    Fortification,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CreatureType(String);

impl CreatureType {
    pub(crate) fn new(value: impl AsRef<str>) -> Result<Self, CombatEffectError> {
        let normalized = value
            .as_ref()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if normalized.is_empty() {
            return Err(CombatEffectError::EmptyCreatureType);
        }
        if !normalized.chars().all(|character| {
            character.is_alphabetic() || character == ' ' || character == '-' || character == '\''
        }) {
            return Err(CombatEffectError::InvalidCreatureType {
                value: value.as_ref().to_owned(),
            });
        }
        Ok(Self(normalized))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PermanentSnapshot {
    pub id: ObjectId,
    pub controller: PlayerId,
    pub card_types: BTreeSet<PermanentType>,
    pub creature_types: BTreeSet<CreatureType>,
    pub has_all_creature_types: bool,
    pub is_token: bool,
    pub base_power: Option<i32>,
    pub base_toughness: Option<i32>,
    /// Counters and already-resolved one-shot adjustments supplied by the
    /// caller. Continuous effects belong in `CombatEffectSet`.
    pub power_adjustment: i32,
    pub toughness_adjustment: i32,
    pub printed_keywords: BTreeSet<CombatKeyword>,
}

impl PermanentSnapshot {
    pub(crate) fn new(
        id: ObjectId,
        controller: PlayerId,
        card_types: impl IntoIterator<Item = PermanentType>,
    ) -> Self {
        Self {
            id,
            controller,
            card_types: card_types.into_iter().collect(),
            creature_types: BTreeSet::new(),
            has_all_creature_types: false,
            is_token: false,
            base_power: None,
            base_toughness: None,
            power_adjustment: 0,
            toughness_adjustment: 0,
            printed_keywords: BTreeSet::new(),
        }
    }

    pub(crate) fn creature(id: ObjectId, controller: PlayerId, power: i32, toughness: i32) -> Self {
        let mut permanent = Self::new(id, controller, [PermanentType::Creature]);
        permanent.base_power = Some(power);
        permanent.base_toughness = Some(toughness);
        permanent
    }

    pub(crate) fn is_creature(&self) -> bool {
        self.card_types.contains(&PermanentType::Creature)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ControllerConstraint {
    #[default]
    Any,
    SameAsSource,
    DifferentFromSource,
    Exact(PlayerId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CreatureTypeConstraint {
    Exact(CreatureType),
    ChosenBySource,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum AttachmentConstraint {
    #[default]
    Any,
    Attached,
    Unattached,
    AttachedByAnyOf(BTreeSet<AttachmentKind>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PermanentFilter {
    pub controller: ControllerConstraint,
    /// Every listed type must be present.
    pub all_card_types: BTreeSet<PermanentType>,
    /// At least one listed type must be present. An empty set is unconstrained.
    pub any_card_types: BTreeSet<PermanentType>,
    pub required_keywords: BTreeSet<CombatKeyword>,
    pub creature_type: Option<CreatureTypeConstraint>,
    /// `Some(true)` requires a token; `Some(false)` requires a nontoken.
    pub token: Option<bool>,
    pub attachment: AttachmentConstraint,
    pub exclude_source: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AttachmentLink {
    pub source: ObjectId,
    pub target: ObjectId,
    pub kind: AttachmentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EffectTarget {
    Source,
    AttachedToSource,
    Filter(PermanentFilter),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CountedValue {
    MatchingPermanents {
        multiplier: i32,
        filter: PermanentFilter,
    },
    AttachmentsOnTarget {
        multiplier: i32,
        /// Empty means every attachment kind.
        kinds: BTreeSet<AttachmentKind>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DynamicValue {
    pub constant: i32,
    pub terms: Vec<CountedValue>,
}

impl DynamicValue {
    pub(crate) fn fixed(value: i32) -> Self {
        Self {
            constant: value,
            terms: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContinuousModifier {
    pub source: ObjectId,
    pub target: EffectTarget,
    pub power: DynamicValue,
    pub toughness: DynamicValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeywordGrant {
    pub source: ObjectId,
    pub target: EffectTarget,
    pub keywords: BTreeSet<CombatKeyword>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct CombatEffectSet {
    pub modifiers: Vec<ContinuousModifier>,
    pub keyword_grants: Vec<KeywordGrant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreatureCombatProfile {
    pub object: ObjectId,
    pub power: i32,
    /// Toughness remains unknown when the caller has not supplied an exact
    /// base value. Player-damage projection never invents one.
    pub toughness: Option<i32>,
    pub keywords: BTreeSet<CombatKeyword>,
    /// Double strike creates two player-combat-damage steps; first strike
    /// alone still creates one. This is not a blocked-combat resolution.
    pub unblocked_damage_steps: u8,
    pub projected_unblocked_damage: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttachmentChoice {
    pub target: ObjectId,
    pub profile: CreatureCombatProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct EffectId {
    pub source: ObjectId,
    pub clause_index: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriggerEventKind {
    PermanentEnteredBattlefield,
    CreatureAttacked,
    CreatureDealtCombatDamage,
    CreatureDied,
    BeginningOfEndStep,
    SpellCast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TriggerContext {
    pub event: TriggerEventKind,
    pub triggering_object: Option<ObjectId>,
    pub ability_source: ObjectId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TriggerMultiplier {
    pub id: EffectId,
    pub event: TriggerEventKind,
    /// `None` means the rule does not inspect an event object.
    pub triggering_object: Option<PermanentFilter>,
    pub ability_source: PermanentFilter,
    pub additional_triggers: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct CombatEffectState {
    permanents: BTreeMap<ObjectId, PermanentSnapshot>,
    attachments: BTreeMap<ObjectId, AttachmentLink>,
    chosen_creature_types: BTreeMap<ObjectId, CreatureType>,
}

impl CombatEffectState {
    pub(crate) fn new(
        permanents: impl IntoIterator<Item = PermanentSnapshot>,
    ) -> Result<Self, CombatEffectError> {
        let mut state = Self::default();
        for permanent in permanents {
            let id = permanent.id;
            if state.permanents.insert(id, permanent).is_some() {
                return Err(CombatEffectError::DuplicatePermanent { object: id });
            }
        }
        Ok(state)
    }

    pub(crate) fn permanent(&self, object: ObjectId) -> Option<&PermanentSnapshot> {
        self.permanents.get(&object)
    }

    pub(crate) fn permanents(&self) -> impl Iterator<Item = &PermanentSnapshot> {
        self.permanents.values()
    }

    /// Attach or move one object. Full Magic target legality remains a caller
    /// responsibility; this state helper enforces identity and one-target-per-
    /// attachment invariants.
    pub(crate) fn attach(
        &mut self,
        source: ObjectId,
        target: ObjectId,
        kind: AttachmentKind,
    ) -> Result<Option<AttachmentLink>, CombatEffectError> {
        self.require_permanent(source)?;
        self.require_permanent(target)?;
        if source == target {
            return Err(CombatEffectError::SelfAttachment { object: source });
        }
        Ok(self.attachments.insert(
            source,
            AttachmentLink {
                source,
                target,
                kind,
            },
        ))
    }

    pub(crate) fn detach(&mut self, source: ObjectId) -> Option<AttachmentLink> {
        self.attachments.remove(&source)
    }

    pub(crate) fn attachment_target(&self, source: ObjectId) -> Option<ObjectId> {
        self.attachments.get(&source).map(|link| link.target)
    }

    pub(crate) fn attachments_on(&self, target: ObjectId) -> impl Iterator<Item = &AttachmentLink> {
        self.attachments
            .values()
            .filter(move |link| link.target == target)
    }

    pub(crate) fn set_chosen_creature_type(
        &mut self,
        source: ObjectId,
        creature_type: CreatureType,
    ) -> Result<Option<CreatureType>, CombatEffectError> {
        self.require_permanent(source)?;
        Ok(self.chosen_creature_types.insert(source, creature_type))
    }

    pub(crate) fn chosen_creature_type(&self, source: ObjectId) -> Option<&CreatureType> {
        self.chosen_creature_types.get(&source)
    }

    pub(crate) fn permanent_matches(
        &self,
        target: ObjectId,
        source: ObjectId,
        filter: &PermanentFilter,
        effects: &CombatEffectSet,
    ) -> Result<bool, CombatEffectError> {
        self.validate_effects(effects)?;
        let keywords = self.effective_keywords(effects)?;
        self.filter_matches(target, source, filter, &keywords)
    }

    pub(crate) fn evaluate_creature(
        &self,
        object: ObjectId,
        effects: &CombatEffectSet,
    ) -> Result<CreatureCombatProfile, CombatEffectError> {
        self.validate_effects(effects)?;
        let keywords = self.effective_keywords(effects)?;
        self.evaluate_creature_with_keywords(object, effects, &keywords)
    }

    pub(crate) fn evaluate_creatures(
        &self,
        effects: &CombatEffectSet,
    ) -> Result<BTreeMap<ObjectId, CreatureCombatProfile>, CombatEffectError> {
        self.validate_effects(effects)?;
        let keywords = self.effective_keywords(effects)?;
        self.permanents
            .values()
            .filter(|permanent| permanent.is_creature())
            .map(|permanent| {
                self.evaluate_creature_with_keywords(permanent.id, effects, &keywords)
                    .map(|profile| (permanent.id, profile))
            })
            .collect()
    }

    /// Select a legal creature target by the post-attachment projected
    /// unblocked damage, then power, useful combat keywords, and finally the
    /// lowest stable object ID. Input/map order cannot change the result.
    pub(crate) fn choose_attachment_target(
        &self,
        attachment: ObjectId,
        kind: AttachmentKind,
        legal_target: &PermanentFilter,
        effects: &CombatEffectSet,
    ) -> Result<Option<AttachmentChoice>, CombatEffectError> {
        self.require_permanent(attachment)?;
        self.validate_effects(effects)?;
        let current_keywords = self.effective_keywords(effects)?;
        let mut best: Option<(u32, i32, u8, Reverse<ObjectId>, AttachmentChoice)> = None;

        for target in self.permanents.values() {
            if target.id == attachment
                || !target.is_creature()
                || !self.filter_matches(target.id, attachment, legal_target, &current_keywords)?
            {
                continue;
            }

            let mut candidate = self.clone();
            candidate.attach(attachment, target.id, kind)?;
            let profile = candidate.evaluate_creature(target.id, effects)?;
            let access_rank = combat_access_rank(&profile.keywords);
            let choice = AttachmentChoice {
                target: target.id,
                profile,
            };
            let score = (
                choice.profile.projected_unblocked_damage,
                choice.profile.power,
                access_rank,
                Reverse(target.id),
            );
            if best
                .as_ref()
                .is_none_or(|current| score > (current.0, current.1, current.2, current.3))
            {
                best = Some((score.0, score.1, score.2, score.3, choice));
            }
        }

        Ok(best.map(|(_, _, _, _, choice)| choice))
    }

    /// Expand an already-proven trigger occurrence. Matching multiplier rules
    /// stack additively: two “additional time” effects turn one occurrence
    /// into three, not four.
    pub(crate) fn expanded_trigger_count(
        &self,
        base_triggers: u32,
        context: TriggerContext,
        multipliers: &[TriggerMultiplier],
    ) -> Result<u32, CombatEffectError> {
        self.require_permanent(context.ability_source)?;
        if let Some(object) = context.triggering_object {
            self.require_permanent(object)?;
        }

        let keywords = self.printed_keyword_map();
        let mut seen = BTreeSet::new();
        let mut additional = 0u32;
        for multiplier in multipliers {
            if !seen.insert(multiplier.id) {
                return Err(CombatEffectError::DuplicateEffect {
                    effect: multiplier.id,
                });
            }
            self.require_permanent(multiplier.id.source)?;
            if multiplier.event != context.event
                || !self.filter_matches(
                    context.ability_source,
                    multiplier.id.source,
                    &multiplier.ability_source,
                    &keywords,
                )?
            {
                continue;
            }
            if let Some(filter) = &multiplier.triggering_object {
                let Some(object) = context.triggering_object else {
                    continue;
                };
                if !self.filter_matches(object, multiplier.id.source, filter, &keywords)? {
                    continue;
                }
            }
            additional = additional
                .checked_add(u32::from(multiplier.additional_triggers))
                .ok_or(CombatEffectError::NumericOverflow)?;
        }
        let factor = 1u32
            .checked_add(additional)
            .ok_or(CombatEffectError::NumericOverflow)?;
        base_triggers
            .checked_mul(factor)
            .ok_or(CombatEffectError::NumericOverflow)
    }

    fn require_permanent(&self, object: ObjectId) -> Result<&PermanentSnapshot, CombatEffectError> {
        self.permanents
            .get(&object)
            .ok_or(CombatEffectError::UnknownPermanent { object })
    }

    fn validate_effects(&self, effects: &CombatEffectSet) -> Result<(), CombatEffectError> {
        for modifier in &effects.modifiers {
            self.require_permanent(modifier.source)?;
        }
        for grant in &effects.keyword_grants {
            self.require_permanent(grant.source)?;
            if grant.keywords.is_empty() {
                return Err(CombatEffectError::EmptyKeywordGrant {
                    effect_source: grant.source,
                });
            }
        }
        Ok(())
    }

    fn printed_keyword_map(&self) -> BTreeMap<ObjectId, BTreeSet<CombatKeyword>> {
        self.permanents
            .iter()
            .map(|(id, permanent)| (*id, permanent.printed_keywords.clone()))
            .collect()
    }

    fn effective_keywords(
        &self,
        effects: &CombatEffectSet,
    ) -> Result<BTreeMap<ObjectId, BTreeSet<CombatKeyword>>, CombatEffectError> {
        let mut effective = self.printed_keyword_map();
        loop {
            let mut changed = false;
            for grant in &effects.keyword_grants {
                for target in self.permanents.keys().copied() {
                    if self.target_applies(target, grant.source, &grant.target, &effective)? {
                        let target_keywords = effective
                            .get_mut(&target)
                            .ok_or(CombatEffectError::UnknownPermanent { object: target })?;
                        for keyword in &grant.keywords {
                            changed |= target_keywords.insert(*keyword);
                        }
                    }
                }
            }
            if !changed {
                return Ok(effective);
            }
        }
    }

    fn evaluate_creature_with_keywords(
        &self,
        object: ObjectId,
        effects: &CombatEffectSet,
        effective_keywords: &BTreeMap<ObjectId, BTreeSet<CombatKeyword>>,
    ) -> Result<CreatureCombatProfile, CombatEffectError> {
        let permanent = self.require_permanent(object)?;
        if !permanent.is_creature() {
            return Err(CombatEffectError::NotCreature { object });
        }
        let mut power = permanent
            .base_power
            .ok_or(CombatEffectError::MissingBasePower { object })?
            .checked_add(permanent.power_adjustment)
            .ok_or(CombatEffectError::NumericOverflow)?;
        let mut toughness = permanent
            .base_toughness
            .map(|base| {
                base.checked_add(permanent.toughness_adjustment)
                    .ok_or(CombatEffectError::NumericOverflow)
            })
            .transpose()?;

        for modifier in &effects.modifiers {
            if !self.target_applies(
                object,
                modifier.source,
                &modifier.target,
                effective_keywords,
            )? {
                continue;
            }
            power = power
                .checked_add(self.evaluate_dynamic(
                    &modifier.power,
                    modifier.source,
                    object,
                    effective_keywords,
                )?)
                .ok_or(CombatEffectError::NumericOverflow)?;
            if let Some(current) = toughness {
                toughness = Some(
                    current
                        .checked_add(self.evaluate_dynamic(
                            &modifier.toughness,
                            modifier.source,
                            object,
                            effective_keywords,
                        )?)
                        .ok_or(CombatEffectError::NumericOverflow)?,
                );
            }
        }

        let keywords = effective_keywords
            .get(&object)
            .cloned()
            .ok_or(CombatEffectError::UnknownPermanent { object })?;
        let unblocked_damage_steps = if keywords.contains(&CombatKeyword::DoubleStrike) {
            2
        } else {
            1
        };
        let nonnegative_power =
            u32::try_from(power.max(0)).map_err(|_| CombatEffectError::NumericOverflow)?;
        let projected_unblocked_damage = nonnegative_power
            .checked_mul(u32::from(unblocked_damage_steps))
            .ok_or(CombatEffectError::NumericOverflow)?;

        Ok(CreatureCombatProfile {
            object,
            power,
            toughness,
            keywords,
            unblocked_damage_steps,
            projected_unblocked_damage,
        })
    }

    fn evaluate_dynamic(
        &self,
        value: &DynamicValue,
        source: ObjectId,
        target: ObjectId,
        effective_keywords: &BTreeMap<ObjectId, BTreeSet<CombatKeyword>>,
    ) -> Result<i32, CombatEffectError> {
        let mut total = value.constant;
        for term in &value.terms {
            let (count, multiplier) = match term {
                CountedValue::MatchingPermanents { multiplier, filter } => {
                    let mut count = 0i32;
                    for candidate in self.permanents.keys().copied() {
                        if self.filter_matches(candidate, source, filter, effective_keywords)? {
                            count = count
                                .checked_add(1)
                                .ok_or(CombatEffectError::NumericOverflow)?;
                        }
                    }
                    (count, *multiplier)
                }
                CountedValue::AttachmentsOnTarget { multiplier, kinds } => {
                    let count = self
                        .attachments_on(target)
                        .filter(|link| kinds.is_empty() || kinds.contains(&link.kind))
                        .count();
                    (
                        i32::try_from(count).map_err(|_| CombatEffectError::NumericOverflow)?,
                        *multiplier,
                    )
                }
            };
            total = total
                .checked_add(
                    count
                        .checked_mul(multiplier)
                        .ok_or(CombatEffectError::NumericOverflow)?,
                )
                .ok_or(CombatEffectError::NumericOverflow)?;
        }
        Ok(total)
    }

    fn target_applies(
        &self,
        target: ObjectId,
        source: ObjectId,
        selector: &EffectTarget,
        effective_keywords: &BTreeMap<ObjectId, BTreeSet<CombatKeyword>>,
    ) -> Result<bool, CombatEffectError> {
        self.require_permanent(source)?;
        self.require_permanent(target)?;
        match selector {
            EffectTarget::Source => Ok(source == target),
            EffectTarget::AttachedToSource => Ok(self.attachment_target(source) == Some(target)),
            EffectTarget::Filter(filter) => {
                self.filter_matches(target, source, filter, effective_keywords)
            }
        }
    }

    fn filter_matches(
        &self,
        target: ObjectId,
        source: ObjectId,
        filter: &PermanentFilter,
        effective_keywords: &BTreeMap<ObjectId, BTreeSet<CombatKeyword>>,
    ) -> Result<bool, CombatEffectError> {
        let target = self.require_permanent(target)?;
        let source = self.require_permanent(source)?;
        if filter.exclude_source && target.id == source.id {
            return Ok(false);
        }
        let controller_matches = match filter.controller {
            ControllerConstraint::Any => true,
            ControllerConstraint::SameAsSource => target.controller == source.controller,
            ControllerConstraint::DifferentFromSource => target.controller != source.controller,
            ControllerConstraint::Exact(player) => target.controller == player,
        };
        if !controller_matches
            || !filter.all_card_types.is_subset(&target.card_types)
            || !filter.any_card_types.is_empty()
                && filter.any_card_types.is_disjoint(&target.card_types)
            || filter
                .token
                .is_some_and(|required| required != target.is_token)
        {
            return Ok(false);
        }
        let target_keywords = effective_keywords
            .get(&target.id)
            .ok_or(CombatEffectError::UnknownPermanent { object: target.id })?;
        if !filter.required_keywords.is_subset(target_keywords) {
            return Ok(false);
        }
        if let Some(creature_type) = &filter.creature_type {
            if !target.is_creature() {
                return Ok(false);
            }
            let required = match creature_type {
                CreatureTypeConstraint::Exact(creature_type) => Some(creature_type),
                CreatureTypeConstraint::ChosenBySource => {
                    self.chosen_creature_types.get(&source.id)
                }
            };
            let Some(required) = required else {
                return Ok(false);
            };
            if !target.has_all_creature_types && !target.creature_types.contains(required) {
                return Ok(false);
            }
        }
        let attachment_matches = match &filter.attachment {
            AttachmentConstraint::Any => true,
            AttachmentConstraint::Attached => self.attachments_on(target.id).next().is_some(),
            AttachmentConstraint::Unattached => self.attachments_on(target.id).next().is_none(),
            AttachmentConstraint::AttachedByAnyOf(kinds) => {
                !kinds.is_empty()
                    && self
                        .attachments_on(target.id)
                        .any(|link| kinds.contains(&link.kind))
            }
        };
        Ok(attachment_matches)
    }
}

fn combat_access_rank(keywords: &BTreeSet<CombatKeyword>) -> u8 {
    [
        CombatKeyword::CantBeBlocked,
        CombatKeyword::Flying,
        CombatKeyword::Menace,
        CombatKeyword::Trample,
    ]
    .into_iter()
    .filter(|keyword| keywords.contains(keyword))
    .count() as u8
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum CombatEffectError {
    #[error("permanent {object:?} appears more than once")]
    DuplicatePermanent { object: ObjectId },
    #[error("permanent {object:?} is not present")]
    UnknownPermanent { object: ObjectId },
    #[error("permanent {object:?} cannot attach to itself")]
    SelfAttachment { object: ObjectId },
    #[error("creature type cannot be empty")]
    EmptyCreatureType,
    #[error("creature type {value:?} contains unsupported characters")]
    InvalidCreatureType { value: String },
    #[error("permanent {object:?} is not a creature")]
    NotCreature { object: ObjectId },
    #[error("creature {object:?} lacks exact base power")]
    MissingBasePower { object: ObjectId },
    #[error("keyword grant from {effect_source:?} grants no keywords")]
    EmptyKeywordGrant { effect_source: ObjectId },
    #[error("effect {effect:?} appears more than once")]
    DuplicateEffect { effect: EffectId },
    #[error("combat-effect arithmetic overflowed")]
    NumericOverflow,
}
