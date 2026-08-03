//! Exact whole-clause triggered and activated ability envelopes.
//!
//! This module owns the event, activation-cost, and timing boundary around an
//! executable action body. It does not accept a clause merely because its
//! opening words look like a trigger or because it contains a colon. Every
//! event predicate, intervening condition, activation cost, restriction, and
//! body must have a typed representation before a program can be produced.
//!
//! Semantic identity is content based. The exact Oracle clause, its complete
//! typed envelope, its child action digest, and the versioned compiler,
//! runtime, and rules contracts are hashed. Card names, database rows, clause
//! addresses, snapshot hashes, timestamps, and snapshot order are never hash
//! inputs. An unchanged Oracle clause therefore keeps its identity across card
//! snapshot refreshes.
//!
//! Recognition remains disconnected from production coverage until the host
//! binds the staged trigger and activation contracts below.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

use crate::oracle_action_algebra_runtime::{
    CardType, ControllerConstraint, CountComparison, GameObject, IncarnationId, ObjectFilter,
    ObjectRef, OracleActionBindings, OracleActionCompileInput, OracleActionProgram,
    OracleActionProgramReceipt, OracleActionRuntimeError, OracleActionSemanticContext,
    OracleActionStateAdapter, PlayerId, VariableAmount, Zone as ActionZone,
    compile_oracle_action_program, execute_oracle_action_program_transactionally,
    object_matches_filter, parse_object_filter, reviewed_oracle_action_normalized_source,
};

pub const ORACLE_ABILITY_ENVELOPE_COMPILER_VERSION: &str = "oracle-ability-envelope-compiler-0.4";
pub const ORACLE_ABILITY_ENVELOPE_RUNTIME_VERSION: &str = "oracle-ability-envelope-runtime-0.4";
pub const ORACLE_ABILITY_ENVELOPE_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:113.7a,117.1b,117.3,117.5,506,508-511,603,605,606,701.14-17";

pub const fn oracle_ability_envelope_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbilityEnvelopeKind {
    Triggered,
    Activated,
}

impl AbilityEnvelopeKind {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Triggered => "triggered/v1",
            Self::Activated => "activated/v1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbilityObjectBinding {
    Source,
    ThisPermanent,
    ThisCreature,
    ThisArtifact,
    ThisEnchantment,
    ThisLand,
    ThisPlaneswalker,
    ThisBattle,
    AnotherPermanentYouControl,
    AnotherCreatureYouControl,
    ACreatureYouControl,
    OneOrMoreCreaturesYouControl,
    AnyCreature,
    AnyPermanent,
    ACard,
    ASpell,
    ThisSpell,
    CopiedSpell,
    TargetOfAbility,
    EnchantedObject,
    EquippedObject,
}

impl AbilityObjectBinding {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::ThisPermanent => "this-permanent",
            Self::ThisCreature => "this-creature",
            Self::ThisArtifact => "this-artifact",
            Self::ThisEnchantment => "this-enchantment",
            Self::ThisLand => "this-land",
            Self::ThisPlaneswalker => "this-planeswalker",
            Self::ThisBattle => "this-battle",
            Self::AnotherPermanentYouControl => "another-permanent-you-control",
            Self::AnotherCreatureYouControl => "another-creature-you-control",
            Self::ACreatureYouControl => "a-creature-you-control",
            Self::OneOrMoreCreaturesYouControl => "one-or-more-creatures-you-control",
            Self::AnyCreature => "any-creature",
            Self::AnyPermanent => "any-permanent",
            Self::ACard => "a-card",
            Self::ASpell => "a-spell",
            Self::ThisSpell => "this-spell",
            Self::CopiedSpell => "copied-spell",
            Self::TargetOfAbility => "target-of-ability",
            Self::EnchantedObject => "enchanted-object",
            Self::EquippedObject => "equipped-object",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbilityPlayerBinding {
    You,
    AnOpponent,
    AnyPlayer,
    ActivePlayer,
    SourceController,
    ObjectController,
    ObjectOwner,
    SpellCaster,
}

impl AbilityPlayerBinding {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::You => "you",
            Self::AnOpponent => "an-opponent",
            Self::AnyPlayer => "any-player",
            Self::ActivePlayer => "active-player",
            Self::SourceController => "source-controller",
            Self::ObjectController => "object-controller",
            Self::ObjectOwner => "object-owner",
            Self::SpellCaster => "spell-caster",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Command,
    Stack,
}

impl Zone {
    fn stable_id_for_envelope(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Hand => "hand",
            Self::Battlefield => "battlefield",
            Self::Graveyard => "graveyard",
            Self::Exile => "exile",
            Self::Command => "command",
            Self::Stack => "stack",
        }
    }

    const fn to_action_zone(self) -> ActionZone {
        match self {
            Self::Library => ActionZone::Library,
            Self::Hand => ActionZone::Hand,
            Self::Battlefield => ActionZone::Battlefield,
            Self::Graveyard => ActionZone::Graveyard,
            Self::Exile => ActionZone::Exile,
            Self::Command => ActionZone::Command,
            Self::Stack => ActionZone::Stack,
        }
    }

    const fn from_action_zone(zone: ActionZone) -> Self {
        match zone {
            ActionZone::Library => Self::Library,
            ActionZone::Hand => Self::Hand,
            ActionZone::Battlefield => Self::Battlefield,
            ActionZone::Graveyard => Self::Graveyard,
            ActionZone::Exile => Self::Exile,
            ActionZone::Command => Self::Command,
            ActionZone::Stack => Self::Stack,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TurnPhase {
    Beginning,
    PrecombatMain,
    Combat,
    PostcombatMain,
    Ending,
}

impl TurnPhase {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Beginning => "beginning",
            Self::PrecombatMain => "precombat-main",
            Self::Combat => "combat",
            Self::PostcombatMain => "postcombat-main",
            Self::Ending => "ending",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TurnStep {
    Untap,
    Upkeep,
    Draw,
    BeginningOfCombat,
    DeclareAttackers,
    DeclareBlockers,
    CombatDamage,
    EndOfCombat,
    End,
    Cleanup,
}

impl TurnStep {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Untap => "untap",
            Self::Upkeep => "upkeep",
            Self::Draw => "draw",
            Self::BeginningOfCombat => "beginning-of-combat",
            Self::DeclareAttackers => "declare-attackers",
            Self::DeclareBlockers => "declare-blockers",
            Self::CombatDamage => "combat-damage",
            Self::EndOfCombat => "end-of-combat",
            Self::End => "end",
            Self::Cleanup => "cleanup",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TurnOwner {
    Yours,
    Opponents,
    EachPlayers,
    Any,
}

impl TurnOwner {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Yours => "yours",
            Self::Opponents => "opponents",
            Self::EachPlayers => "each-players",
            Self::Any => "any",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CounterChange {
    Put,
    Removed,
    Changed,
}

impl CounterChange {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Put => "put",
            Self::Removed => "removed",
            Self::Changed => "changed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TriggerPredicate {
    EntersBattlefield {
        object: AbilityObjectBinding,
        tapped: Option<bool>,
    },
    LeavesBattlefield {
        object: AbilityObjectBinding,
        destination: Option<Zone>,
    },
    Dies {
        object: AbilityObjectBinding,
    },
    Attacks {
        object: AbilityObjectBinding,
        alone: bool,
        recipient: AttackRecipientRequirement,
    },
    Blocks {
        object: AbilityObjectBinding,
        became_blocked: bool,
    },
    DealsCombatDamage {
        source: AbilityObjectBinding,
        recipient: CombatDamageRecipient,
    },
    DealsDamage {
        source: AbilityObjectBinding,
    },
    StepOrPhase {
        boundary: StepBoundary,
        phase: Option<TurnPhase>,
        step: Option<TurnStep>,
        turn_owner: TurnOwner,
    },
    Cast {
        player: AbilityPlayerBinding,
        spell: AbilityObjectBinding,
        from_zone: Option<Zone>,
        mode: SpellEventMode,
    },
    BecomesTarget {
        object: AbilityObjectBinding,
        actor: Option<AbilityPlayerBinding>,
        cause: TargetingCauseRequirement,
    },
    TappedOrUntapped {
        object: AbilityObjectBinding,
        tapped: bool,
    },
    CounterChanged {
        object: AbilityObjectBinding,
        operation: CounterChange,
        counter_name: String,
        one_or_more: bool,
    },
    ControllerControls {
        filter: ObjectFilter,
        comparison: CountComparison,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TargetingCauseRequirement {
    Spell,
    Ability,
    SpellOrAbility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttackRecipientRequirement {
    Any,
    Player,
    Opponent,
    PlayerOrBattle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpellEventMode {
    Cast,
    Copy,
    CastOrCopy,
}

impl SpellEventMode {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Cast => "cast",
            Self::Copy => "copy",
            Self::CastOrCopy => "cast-or-copy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CombatDamageRecipient {
    Player,
    Opponent,
    Planeswalker,
    Battle,
    PlayerOrPlaneswalker,
    PlayerOrBattle,
    Any,
}

impl CombatDamageRecipient {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Opponent => "opponent",
            Self::Planeswalker => "planeswalker",
            Self::Battle => "battle",
            Self::PlayerOrPlaneswalker => "player-or-planeswalker",
            Self::PlayerOrBattle => "player-or-battle",
            Self::Any => "any",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StepBoundary {
    Beginning,
    End,
}

impl StepBoundary {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Beginning => "beginning",
            Self::End => "end",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterveningCondition {
    SourceIsTapped(bool),
    SourceIsAttacking(bool),
    SourceHasCounter {
        counter_name: String,
        at_least: u32,
    },
    YouControlObject(AbilityObjectBinding),
    YourTurn(bool),
    LifeComparison {
        player: AbilityPlayerBinding,
        comparison: NumericComparison,
        amount: u32,
    },
    CardsInHandComparison {
        player: AbilityPlayerBinding,
        comparison: NumericComparison,
        amount: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NumericComparison {
    Exactly,
    AtLeast,
    AtMost,
    GreaterThan,
    LessThan,
}

impl NumericComparison {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Exactly => "exactly",
            Self::AtLeast => "at-least",
            Self::AtMost => "at-most",
            Self::GreaterThan => "greater-than",
            Self::LessThan => "less-than",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TriggerEnvelope {
    pub predicate: TriggerPredicate,
    pub intervening_if: Option<InterveningCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActivationCost {
    Mana(ManaCost),
    TapSource,
    UntapSource,
    Sacrifice(ObjectCost),
    Discard(CardCost),
    Exile(CardCost),
    PayLife(u32),
    RemoveCounters {
        object: AbilityObjectBinding,
        counter_name: String,
        amount: CostAmount,
    },
    TapObjects {
        objects: ObjectCost,
        amount: CostAmount,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaCost {
    pub symbols: Vec<ManaSymbol>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaSymbol {
    Generic(u32),
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
    Snow,
    X,
    Hybrid(String, String),
    Phyrexian(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CostAmount {
    Fixed(u32),
    X,
    AnyNumber,
    OneOrMore,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectCost {
    pub amount: CostAmount,
    pub controller: CostController,
    pub filter: CostObjectFilter,
    pub source_only: bool,
    pub other_than_source: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CostController {
    You,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CostObjectFilter {
    pub card_types: BTreeSet<CostCardType>,
    pub subtypes: BTreeSet<String>,
    pub tapped: Option<bool>,
    pub nontoken: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CostCardType {
    Artifact,
    Battle,
    Creature,
    Enchantment,
    Land,
    Planeswalker,
    Permanent,
}

impl CostCardType {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Artifact => "artifact",
            Self::Battle => "battle",
            Self::Creature => "creature",
            Self::Enchantment => "enchantment",
            Self::Land => "land",
            Self::Planeswalker => "planeswalker",
            Self::Permanent => "permanent",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CardCost {
    pub amount: CostAmount,
    pub zone: Zone,
    pub random: bool,
    pub filter: CardCostFilter,
    pub source_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CardCostFilter {
    pub card_types: BTreeSet<CostCardType>,
    pub named_characteristic: Option<String>,
    pub other_than_source: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActivationRestriction {
    AnyTime,
    SorceryTiming,
    DuringYourTurn,
    DuringAnOpponentsTurn,
    DuringCombat,
    BeforeAttackersAreDeclared,
    OnlyOnceEachTurn,
    OnlyOnce,
    SourceWasNotCastThisTurn,
    SourceEnteredThisTurn,
    Combined(Vec<ActivationRestriction>),
}

impl ActivationRestriction {
    fn stable_id(&self) -> String {
        match self {
            Self::AnyTime => "any-time".to_owned(),
            Self::SorceryTiming => "sorcery-timing".to_owned(),
            Self::DuringYourTurn => "during-your-turn".to_owned(),
            Self::DuringAnOpponentsTurn => "during-an-opponents-turn".to_owned(),
            Self::DuringCombat => "during-combat".to_owned(),
            Self::BeforeAttackersAreDeclared => "before-attackers-are-declared".to_owned(),
            Self::OnlyOnceEachTurn => "only-once-each-turn".to_owned(),
            Self::OnlyOnce => "only-once".to_owned(),
            Self::SourceWasNotCastThisTurn => "source-was-not-cast-this-turn".to_owned(),
            Self::SourceEnteredThisTurn => "source-entered-this-turn".to_owned(),
            Self::Combined(restrictions) => format!(
                "combined({})",
                restrictions
                    .iter()
                    .map(Self::stable_id)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActivatedEnvelope {
    pub costs: Vec<ActivationCost>,
    pub restriction: ActivationRestriction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedAbilityEnvelope {
    Triggered(TriggerEnvelope),
    Activated(ActivatedEnvelope),
}

impl ParsedAbilityEnvelope {
    pub const fn kind(&self) -> AbilityEnvelopeKind {
        match self {
            Self::Triggered(_) => AbilityEnvelopeKind::Triggered,
            Self::Activated(_) => AbilityEnvelopeKind::Activated,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityEnvelopeShape {
    exact_source: String,
    normalized_source: String,
    exact_body: String,
    envelope: ParsedAbilityEnvelope,
    shape_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleAbilityEnvelopeProgram {
    shape: AbilityEnvelopeShape,
    body: OracleActionProgram,
    semantic_digest: String,
}

impl OracleAbilityEnvelopeProgram {
    pub fn exact_source(&self) -> &str {
        self.shape.exact_source()
    }

    pub fn normalized_source(&self) -> &str {
        self.shape.normalized_source()
    }

    pub fn exact_body(&self) -> &str {
        self.shape.exact_body()
    }

    pub fn envelope(&self) -> &ParsedAbilityEnvelope {
        self.shape.envelope()
    }

    pub const fn kind(&self) -> AbilityEnvelopeKind {
        self.shape.envelope.kind()
    }

    pub fn body(&self) -> &OracleActionProgram {
        &self.body
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub const fn production_adapter_connected(&self) -> bool {
        oracle_ability_envelope_production_adapter_connected()
    }
}

impl AbilityEnvelopeShape {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn exact_body(&self) -> &str {
        &self.exact_body
    }

    pub fn envelope(&self) -> &ParsedAbilityEnvelope {
        &self.envelope
    }

    pub fn shape_digest(&self) -> &str {
        &self.shape_digest
    }

    pub const fn production_adapter_connected(&self) -> bool {
        oracle_ability_envelope_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityEnvelopeCompileInput<'a> {
    pub exact_source: &'a str,
    pub normalized_source: &'a str,
}

impl<'a> AbilityEnvelopeCompileInput<'a> {
    pub fn exact(exact_source: &'a str) -> Self {
        Self {
            exact_source,
            normalized_source: exact_source,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbilityEnvelopeRejection {
    EmptyOrMalformedSource,
    NormalizationMismatch,
    NotAbilityEnvelope,
    UnsupportedTriggerPredicate,
    UnsupportedInterveningCondition,
    UnsupportedActivationCost,
    UnsupportedTimingRestriction,
    UnsupportedTargetBinding,
    UnsupportedActionBody,
    AmbiguousComposition,
    UnconsumedSource,
}

impl fmt::Display for AbilityEnvelopeRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AbilityEnvelopeRejection {}

pub fn reviewed_ability_envelope_normalized_source(exact_source: &str) -> String {
    collapse_whitespace(exact_source)
}

pub fn compile_oracle_ability_envelope(
    input: AbilityEnvelopeCompileInput<'_>,
) -> Result<OracleAbilityEnvelopeProgram, AbilityEnvelopeRejection> {
    let shape = parse_ability_envelope_shape(input)?;
    let semantic_context = match shape.envelope() {
        ParsedAbilityEnvelope::Triggered(_) => {
            OracleActionSemanticContext::ResolvingTriggeredAbilityInstruction
        }
        ParsedAbilityEnvelope::Activated(_) => {
            OracleActionSemanticContext::ResolvingActivatedAbilityInstruction
        }
    };
    let normalized_body = reviewed_oracle_action_normalized_source(shape.exact_body());
    let body = compile_oracle_action_program(OracleActionCompileInput {
        exact_source: shape.exact_body(),
        normalized_source: &normalized_body,
        semantic_context,
    })
    .map_err(|_| AbilityEnvelopeRejection::UnsupportedActionBody)?;
    let semantic_digest = ability_program_semantic_digest(&shape, &body);
    Ok(OracleAbilityEnvelopeProgram {
        shape,
        body,
        semantic_digest,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaResource {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbilityManaUnit {
    pub id: u64,
    pub resource: ManaResource,
    pub snow: bool,
    /// True only after the host proves that every restriction on this mana
    /// permits spending it to activate this exact ability.
    pub spend_restrictions_satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerObjectSnapshot {
    pub reference: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone_before: Zone,
    pub zone_after: Zone,
    pub card_types: BTreeSet<CardType>,
    pub tapped: bool,
    pub attachments: BTreeSet<TriggerAttachmentEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TriggerAttachmentEvidence {
    pub source: ObjectRef,
    pub kind: TriggerAttachmentKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TriggerAttachmentKind {
    Aura,
    Equipment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResolvedCombatDamageRecipient {
    Player(PlayerId),
    Planeswalker(ObjectRef),
    Battle(ObjectRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResolvedDamageRecipient {
    Player(PlayerId),
    Object(ObjectRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttackDefender {
    Player(PlayerId),
    Planeswalker(ObjectRef),
    Battle(ObjectRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TargetingCauseKind {
    Spell,
    Ability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetingCause {
    pub kind: TargetingCauseKind,
    pub controller: PlayerId,
    pub source: Option<ObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbilityTriggerEvent {
    ZoneChanged {
        object: TriggerObjectSnapshot,
    },
    Attacked {
        object: TriggerObjectSnapshot,
        alone: bool,
        defender: Option<AttackDefender>,
    },
    Blocked {
        object: TriggerObjectSnapshot,
        became_blocked: bool,
    },
    CombatDamage {
        source: TriggerObjectSnapshot,
        recipient: ResolvedCombatDamageRecipient,
    },
    Damage {
        source: TriggerObjectSnapshot,
        recipient: ResolvedDamageRecipient,
        amount: u32,
        combat: bool,
    },
    StepOrPhase {
        boundary: StepBoundary,
        phase: Option<TurnPhase>,
        step: Option<TurnStep>,
        active_player: PlayerId,
    },
    Spell {
        spell: TriggerObjectSnapshot,
        player: PlayerId,
        mode: SpellEventMode,
        cast_from: Option<Zone>,
    },
    BecameTarget {
        object: TriggerObjectSnapshot,
        cause: TargetingCause,
    },
    OrientationChanged {
        object: TriggerObjectSnapshot,
        tapped: bool,
    },
    CounterChanged {
        object: TriggerObjectSnapshot,
        operation: CounterChange,
        counter_name: String,
        amount: u32,
    },
    BattlefieldConditionChanged {
        player: PlayerId,
        prior_matching_count: u32,
        current_matching_count: u32,
        battlefield_evidence_complete: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AbilityActivationPayment {
    pub mana_units: Vec<u64>,
    pub phyrexian_life_symbol_indices: BTreeSet<usize>,
    pub object_selections: BTreeMap<usize, Vec<ObjectRef>>,
    pub card_selections: BTreeMap<usize, Vec<ObjectRef>>,
    pub x_value: Option<u32>,
    pub random_selection_proven: BTreeSet<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaidActivationCostReceipt {
    pub cost_index: usize,
    pub mana_units: Vec<u64>,
    pub life_paid: u32,
    pub tapped: Vec<ObjectRef>,
    pub untapped: Vec<ObjectRef>,
    pub sacrificed: Vec<(ObjectRef, ObjectRef)>,
    pub discarded: Vec<(ObjectRef, ObjectRef)>,
    pub exiled: Vec<(ObjectRef, ObjectRef)>,
    pub counters_removed: BTreeMap<(ObjectRef, String), u32>,
}

impl PaidActivationCostReceipt {
    fn empty(cost_index: usize) -> Self {
        Self {
            cost_index,
            mana_units: Vec::new(),
            life_paid: 0,
            tapped: Vec::new(),
            untapped: Vec::new(),
            sacrificed: Vec::new(),
            discarded: Vec::new(),
            exiled: Vec::new(),
            counters_removed: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingAbilityId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAbility {
    pub id: PendingAbilityId,
    pub program_digest: String,
    pub kind: AbilityEnvelopeKind,
    pub controller: PlayerId,
    pub source: ObjectRef,
    pub event_object: Option<ObjectRef>,
    pub event_player: Option<PlayerId>,
    event_amount: Option<u32>,
    activation_x_value: Option<u32>,
    intervening_if: Option<InterveningCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityTriggerReceipt {
    pub pending: PendingAbility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityActivationReceipt {
    pub pending: PendingAbility,
    pub paid_costs: Vec<PaidActivationCostReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbilityResolutionOutcome {
    ActionsCommitted(OracleActionProgramReceipt),
    InterveningConditionFalse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityResolutionReceipt {
    pub pending_id: PendingAbilityId,
    pub program_digest: String,
    pub outcome: AbilityResolutionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbilityEnvelopeRuntimeError {
    WrongEnvelopeKind,
    ProgramVersionMismatch,
    MissingSource(ObjectRef),
    StaleSource(ObjectRef),
    SourceInWrongZone {
        source: ObjectRef,
        expected: Zone,
        actual: Zone,
    },
    WrongController,
    TriggerDidNotMatch,
    IncompleteBattlefieldEvidence,
    IncompleteHiddenZoneEvidence,
    IncompletePlayerRelationEvidence(PlayerId),
    UnknownPlayer(PlayerId),
    InterveningConditionUnavailable,
    TimingRestrictionNotMet,
    ActivationLimitReached,
    IncompleteTapCostEvidence,
    CreatureCannotPayTapSymbol(ObjectRef),
    MissingManaUnit(u64),
    DuplicateManaUnit(u64),
    UnexpectedManaPayment,
    InvalidManaPayment,
    ManaSpendRestrictionNotSatisfied(u64),
    MissingXValue,
    UnexpectedXValue,
    ConflictingXBinding {
        pending: u32,
        supplied: u32,
    },
    ConflictingEventAmountBinding {
        pending: u32,
        supplied: u32,
    },
    UnexpectedPaymentSelection(usize),
    MissingPaymentSelection(usize),
    WrongPaymentCardinality {
        cost_index: usize,
        expected: CostAmount,
        actual: usize,
    },
    DuplicatePaymentObject(ObjectRef),
    IllegalPaymentObject {
        cost_index: usize,
        object: ObjectRef,
    },
    RandomSelectionEvidenceMissing(usize),
    InsufficientLife,
    LifeOverflow,
    ObjectIdOverflow(ObjectRef),
    InsufficientCounters {
        object: ObjectRef,
        counter: String,
        required: u32,
        available: u32,
    },
    MissingPending(PendingAbilityId),
    Action(OracleActionRuntimeError),
}

impl fmt::Display for AbilityEnvelopeRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AbilityEnvelopeRuntimeError {}

impl From<OracleActionRuntimeError> for AbilityEnvelopeRuntimeError {
    fn from(error: OracleActionRuntimeError) -> Self {
        Self::Action(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityEnvelopeRuntimeState<S: OracleActionStateAdapter> {
    pub action_state: S,
    pub mana_pools: BTreeMap<PlayerId, BTreeMap<u64, AbilityManaUnit>>,
    pub active_player: PlayerId,
    pub priority_player: PlayerId,
    pub phase: TurnPhase,
    pub step: Option<TurnStep>,
    pub turn_number: u64,
    pub stack_empty: bool,
    pub attackers_declared: bool,
    /// Complete evidence that no rule or effect prohibits the requested tap
    /// or untap cost.
    pub tap_cost_legality_complete: bool,
    /// Creature sources in this set either have been continuously controlled
    /// since the turn began or currently have haste.
    pub tap_symbol_eligible_creatures: BTreeSet<ObjectRef>,
    pub source_cast_turn: BTreeMap<ObjectRef, u64>,
    pub source_entered_turn: BTreeMap<ObjectRef, u64>,
    pending: BTreeMap<PendingAbilityId, PendingAbility>,
    activation_count_by_turn: BTreeMap<(String, ObjectRef, u64), u32>,
    activated_once: BTreeSet<(String, ObjectRef)>,
    next_pending_id: u64,
}

impl<S: OracleActionStateAdapter> AbilityEnvelopeRuntimeState<S> {
    pub fn new(action_state: S) -> Self {
        Self {
            action_state,
            mana_pools: BTreeMap::new(),
            active_player: 0,
            priority_player: 0,
            phase: TurnPhase::Beginning,
            step: Some(TurnStep::Untap),
            turn_number: 0,
            stack_empty: true,
            attackers_declared: false,
            tap_cost_legality_complete: false,
            tap_symbol_eligible_creatures: BTreeSet::new(),
            source_cast_turn: BTreeMap::new(),
            source_entered_turn: BTreeMap::new(),
            pending: BTreeMap::new(),
            activation_count_by_turn: BTreeMap::new(),
            activated_once: BTreeSet::new(),
            next_pending_id: 1,
        }
    }

    pub fn pending(&self, id: PendingAbilityId) -> Option<&PendingAbility> {
        self.pending.get(&id)
    }

    fn next_pending_id(&mut self) -> Result<PendingAbilityId, AbilityEnvelopeRuntimeError> {
        let id = PendingAbilityId(self.next_pending_id);
        self.next_pending_id = self.next_pending_id.checked_add(1).ok_or(
            AbilityEnvelopeRuntimeError::ObjectIdOverflow(ObjectRef {
                object_id: u64::MAX,
                incarnation_id: IncarnationId(u64::MAX),
            }),
        )?;
        Ok(id)
    }
}

pub fn begin_ability_trigger<S: OracleActionStateAdapter>(
    state: &mut AbilityEnvelopeRuntimeState<S>,
    program: &OracleAbilityEnvelopeProgram,
    controller: PlayerId,
    source: ObjectRef,
    event: &AbilityTriggerEvent,
) -> Result<AbilityTriggerReceipt, AbilityEnvelopeRuntimeError> {
    let ParsedAbilityEnvelope::Triggered(envelope) = program.envelope() else {
        return Err(AbilityEnvelopeRuntimeError::WrongEnvelopeKind);
    };
    verify_program_version(program)?;
    validate_trigger_source(state, controller, source, event)?;
    let (event_object, event_player) =
        validate_trigger_event(state, &envelope.predicate, controller, source, event)?;
    if let Some(condition) = &envelope.intervening_if
        && !evaluate_intervening_condition(state, condition, controller, source)?
    {
        return Err(AbilityEnvelopeRuntimeError::TriggerDidNotMatch);
    }
    let id = state.next_pending_id()?;
    let pending = PendingAbility {
        id,
        program_digest: program.semantic_digest().to_owned(),
        kind: AbilityEnvelopeKind::Triggered,
        controller,
        source,
        event_object,
        event_player,
        event_amount: trigger_event_amount(event),
        activation_x_value: None,
        intervening_if: envelope.intervening_if.clone(),
    };
    state.pending.insert(id, pending.clone());
    Ok(AbilityTriggerReceipt { pending })
}

pub fn begin_ability_activation<S: OracleActionStateAdapter>(
    state: &mut AbilityEnvelopeRuntimeState<S>,
    program: &OracleAbilityEnvelopeProgram,
    controller: PlayerId,
    source: ObjectRef,
    payment: &AbilityActivationPayment,
) -> Result<AbilityActivationReceipt, AbilityEnvelopeRuntimeError> {
    let ParsedAbilityEnvelope::Activated(envelope) = program.envelope() else {
        return Err(AbilityEnvelopeRuntimeError::WrongEnvelopeKind);
    };
    verify_program_version(program)?;
    let mut staged = state.clone();
    let result = (|| {
        validate_source_identity(&staged, controller, source, false)?;
        validate_activation_source_zone(&staged, envelope, source)?;
        if staged.priority_player != controller {
            return Err(AbilityEnvelopeRuntimeError::TimingRestrictionNotMet);
        }
        validate_activation_restriction(
            &staged,
            &envelope.restriction,
            program.semantic_digest(),
            controller,
            source,
        )?;
        validate_payment_shape(&envelope.costs, payment)?;
        let mut paid_costs = Vec::with_capacity(envelope.costs.len());
        let mut payment_order = (0..envelope.costs.len()).collect::<Vec<_>>();
        payment_order.sort_by_key(|index| activation_cost_payment_rank(&envelope.costs[*index]));
        for cost_index in payment_order {
            let cost = &envelope.costs[cost_index];
            let receipt =
                pay_activation_cost(&mut staged, controller, source, cost_index, cost, payment)?;
            paid_costs.push(receipt);
        }
        paid_costs.sort_by_key(|receipt| receipt.cost_index);
        note_activation(
            &mut staged,
            &envelope.restriction,
            program.semantic_digest(),
            source,
        );
        let id = staged.next_pending_id()?;
        let pending = PendingAbility {
            id,
            program_digest: program.semantic_digest().to_owned(),
            kind: AbilityEnvelopeKind::Activated,
            controller,
            source,
            event_object: None,
            event_player: None,
            event_amount: None,
            activation_x_value: payment.x_value,
            intervening_if: None,
        };
        staged.pending.insert(id, pending.clone());
        Ok(AbilityActivationReceipt {
            pending,
            paid_costs,
        })
    })();
    match result {
        Ok(receipt) => {
            *state = staged;
            Ok(receipt)
        }
        Err(error) => Err(error),
    }
}

fn validate_activation_source_zone<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    envelope: &ActivatedEnvelope,
    source: ObjectRef,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    let expected = required_activation_source_zone(envelope)?;
    let actual = state
        .action_state
        .action_world()
        .objects
        .get(&source)
        .map(|object| Zone::from_action_zone(object.zone))
        .ok_or(AbilityEnvelopeRuntimeError::StaleSource(source))?;
    if actual == expected {
        Ok(())
    } else {
        Err(AbilityEnvelopeRuntimeError::SourceInWrongZone {
            source,
            expected,
            actual,
        })
    }
}

fn required_activation_source_zone(
    envelope: &ActivatedEnvelope,
) -> Result<Zone, AbilityEnvelopeRuntimeError> {
    let mut source_zone = None;
    for cost in &envelope.costs {
        let candidate = match cost {
            ActivationCost::Discard(CardCost {
                zone,
                source_only: true,
                ..
            })
            | ActivationCost::Exile(CardCost {
                zone,
                source_only: true,
                ..
            }) => Some(*zone),
            ActivationCost::Sacrifice(ObjectCost {
                source_only: true, ..
            })
            | ActivationCost::TapSource
            | ActivationCost::UntapSource
            | ActivationCost::RemoveCounters {
                object:
                    AbilityObjectBinding::Source
                    | AbilityObjectBinding::ThisPermanent
                    | AbilityObjectBinding::ThisCreature
                    | AbilityObjectBinding::ThisArtifact
                    | AbilityObjectBinding::ThisEnchantment
                    | AbilityObjectBinding::ThisLand
                    | AbilityObjectBinding::ThisPlaneswalker
                    | AbilityObjectBinding::ThisBattle,
                ..
            } => Some(Zone::Battlefield),
            _ => None,
        };
        if let Some(candidate) = candidate {
            if source_zone.is_some_and(|zone| zone != candidate) {
                return Err(AbilityEnvelopeRuntimeError::TimingRestrictionNotMet);
            }
            source_zone = Some(candidate);
        }
    }
    Ok(source_zone.unwrap_or(Zone::Battlefield))
}

pub fn resolve_pending_ability<S: OracleActionStateAdapter>(
    state: &mut AbilityEnvelopeRuntimeState<S>,
    program: &OracleAbilityEnvelopeProgram,
    pending_id: PendingAbilityId,
    bindings: &OracleActionBindings,
) -> Result<AbilityResolutionReceipt, AbilityEnvelopeRuntimeError> {
    verify_program_version(program)?;
    let mut staged = state.clone();
    let result = (|| {
        let pending = staged
            .pending
            .remove(&pending_id)
            .ok_or(AbilityEnvelopeRuntimeError::MissingPending(pending_id))?;
        if pending.program_digest != program.semantic_digest() || pending.kind != program.kind() {
            return Err(AbilityEnvelopeRuntimeError::ProgramVersionMismatch);
        }
        if let Some(condition) = &pending.intervening_if
            && !evaluate_intervening_condition(
                &staged,
                condition,
                pending.controller,
                pending.source,
            )?
        {
            return Ok(AbilityResolutionReceipt {
                pending_id,
                program_digest: pending.program_digest,
                outcome: AbilityResolutionOutcome::InterveningConditionFalse,
            });
        }
        let mut resolved_bindings = bindings.clone();
        resolved_bindings.controller = pending.controller;
        resolved_bindings.source = staged
            .action_state
            .action_world()
            .objects
            .contains_key(&pending.source)
            .then_some(pending.source);
        if resolved_bindings.that_object.is_none() {
            resolved_bindings.that_object = pending.event_object;
        }
        if resolved_bindings.that_player.is_none() {
            resolved_bindings.that_player = pending.event_player;
        }
        if let Some(x_value) = pending.activation_x_value {
            if let Some(supplied) = resolved_bindings.variable_amounts.get(&VariableAmount::X) {
                if *supplied != x_value {
                    return Err(AbilityEnvelopeRuntimeError::ConflictingXBinding {
                        pending: x_value,
                        supplied: *supplied,
                    });
                }
            } else {
                resolved_bindings
                    .variable_amounts
                    .insert(VariableAmount::X, x_value);
            }
        }
        if let Some(event_amount) = pending.event_amount {
            if let Some(supplied) = resolved_bindings
                .variable_amounts
                .get(&VariableAmount::ThatMany)
            {
                if *supplied != event_amount {
                    return Err(AbilityEnvelopeRuntimeError::ConflictingEventAmountBinding {
                        pending: event_amount,
                        supplied: *supplied,
                    });
                }
            } else {
                resolved_bindings
                    .variable_amounts
                    .insert(VariableAmount::ThatMany, event_amount);
            }
        }
        let body_receipt = execute_oracle_action_program_transactionally(
            program.body(),
            &resolved_bindings,
            &mut staged.action_state,
        )?;
        Ok(AbilityResolutionReceipt {
            pending_id,
            program_digest: pending.program_digest,
            outcome: AbilityResolutionOutcome::ActionsCommitted(body_receipt),
        })
    })();
    match result {
        Ok(receipt) => {
            *state = staged;
            Ok(receipt)
        }
        Err(error) => Err(error),
    }
}

fn verify_program_version(
    program: &OracleAbilityEnvelopeProgram,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    let expected = ability_program_semantic_digest(&program.shape, &program.body);
    (expected == program.semantic_digest)
        .then_some(())
        .ok_or(AbilityEnvelopeRuntimeError::ProgramVersionMismatch)
}

fn validate_source_identity<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    controller: PlayerId,
    source: ObjectRef,
    require_battlefield: bool,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    let Some(object) = state.action_state.action_world().objects.get(&source) else {
        return Err(AbilityEnvelopeRuntimeError::StaleSource(source));
    };
    if object.reference != source {
        return Err(AbilityEnvelopeRuntimeError::StaleSource(source));
    }
    if object.controller != controller {
        return Err(AbilityEnvelopeRuntimeError::WrongController);
    }
    if require_battlefield && object.zone != ActionZone::Battlefield {
        return Err(AbilityEnvelopeRuntimeError::MissingSource(source));
    }
    Ok(())
}

fn validate_trigger_source<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    controller: PlayerId,
    source: ObjectRef,
    event: &AbilityTriggerEvent,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    if let Some(object) = state.action_state.action_world().objects.get(&source) {
        return (object.controller == controller)
            .then_some(())
            .ok_or(AbilityEnvelopeRuntimeError::WrongController);
    }
    let event_source = match event {
        AbilityTriggerEvent::ZoneChanged { object }
        | AbilityTriggerEvent::Attacked { object, .. }
        | AbilityTriggerEvent::Blocked { object, .. }
        | AbilityTriggerEvent::BecameTarget { object, .. }
        | AbilityTriggerEvent::OrientationChanged { object, .. }
        | AbilityTriggerEvent::CounterChanged { object, .. } => Some(object),
        AbilityTriggerEvent::CombatDamage { source, .. }
        | AbilityTriggerEvent::Damage { source, .. } => Some(source),
        AbilityTriggerEvent::Spell { spell, .. } => Some(spell),
        AbilityTriggerEvent::StepOrPhase { .. }
        | AbilityTriggerEvent::BattlefieldConditionChanged { .. } => None,
    };
    if event_source
        .is_some_and(|object| object.reference == source && object.controller == controller)
    {
        Ok(())
    } else {
        Err(AbilityEnvelopeRuntimeError::StaleSource(source))
    }
}

fn validate_trigger_event<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    predicate: &TriggerPredicate,
    controller: PlayerId,
    source: ObjectRef,
    event: &AbilityTriggerEvent,
) -> Result<(Option<ObjectRef>, Option<PlayerId>), AbilityEnvelopeRuntimeError> {
    validate_trigger_player_evidence(state, predicate, controller, event)?;
    let matched = match (predicate, event) {
        (
            TriggerPredicate::EntersBattlefield { object, tapped },
            AbilityTriggerEvent::ZoneChanged {
                object: event_object,
            },
        ) => {
            event_object.zone_after == Zone::Battlefield
                && tapped.is_none_or(|expected| event_object.tapped == expected)
                && event_object_binding_matches(*object, event_object, controller, source)
        }
        (
            TriggerPredicate::LeavesBattlefield {
                object,
                destination,
            },
            AbilityTriggerEvent::ZoneChanged {
                object: event_object,
            },
        ) => {
            event_object.zone_before == Zone::Battlefield
                && event_object.zone_after != Zone::Battlefield
                && destination.is_none_or(|zone| event_object.zone_after == zone)
                && event_object_binding_matches(*object, event_object, controller, source)
        }
        (
            TriggerPredicate::Dies { object },
            AbilityTriggerEvent::ZoneChanged {
                object: event_object,
            },
        ) => {
            event_object.zone_before == Zone::Battlefield
                && event_object.zone_after == Zone::Graveyard
                && event_object.card_types.contains(&CardType::Creature)
                && event_object_binding_matches(*object, event_object, controller, source)
        }
        (
            TriggerPredicate::Attacks {
                object,
                alone,
                recipient,
            },
            AbilityTriggerEvent::Attacked {
                object: event_object,
                alone: event_alone,
                defender,
            },
        ) => {
            (!*alone || *event_alone)
                && attack_recipient_matches(*recipient, *defender, controller, state)
                && event_object_binding_matches(*object, event_object, controller, source)
        }
        (
            TriggerPredicate::Blocks {
                object,
                became_blocked,
            },
            AbilityTriggerEvent::Blocked {
                object: event_object,
                became_blocked: event_became_blocked,
            },
        ) => {
            became_blocked == event_became_blocked
                && event_object_binding_matches(*object, event_object, controller, source)
        }
        (
            TriggerPredicate::DealsCombatDamage {
                source: object,
                recipient,
            },
            AbilityTriggerEvent::CombatDamage {
                source: event_object,
                recipient: event_recipient,
            },
        ) => {
            combat_recipient_matches(*recipient, *event_recipient, controller, state)
                && event_object_binding_matches(*object, event_object, controller, source)
        }
        (
            TriggerPredicate::DealsDamage { source: object },
            AbilityTriggerEvent::Damage {
                source: event_object,
                amount,
                ..
            },
        ) => *amount > 0 && event_object_binding_matches(*object, event_object, controller, source),
        (
            TriggerPredicate::StepOrPhase {
                boundary,
                phase,
                step,
                turn_owner,
            },
            AbilityTriggerEvent::StepOrPhase {
                boundary: event_boundary,
                phase: event_phase,
                step: event_step,
                active_player,
            },
        ) => {
            boundary == event_boundary
                && phase == event_phase
                && step == event_step
                && turn_owner_matches(*turn_owner, controller, *active_player, state)
        }
        (
            TriggerPredicate::Cast {
                player,
                spell,
                from_zone,
                mode,
            },
            AbilityTriggerEvent::Spell {
                spell: event_spell,
                player: event_player,
                mode: event_mode,
                cast_from,
            },
        ) => {
            player_binding_matches(*player, controller, *event_player, state)
                && spell_mode_matches(*mode, *event_mode)
                && from_zone.is_none_or(|zone| *cast_from == Some(zone))
                && event_object_binding_matches(*spell, event_spell, controller, source)
        }
        (
            TriggerPredicate::BecomesTarget {
                object,
                actor,
                cause,
            },
            AbilityTriggerEvent::BecameTarget {
                object: event_object,
                cause: event_cause,
            },
        ) => {
            actor.is_none_or(|binding| {
                player_binding_matches(binding, controller, event_cause.controller, state)
            }) && targeting_cause_matches(*cause, event_cause.kind)
                && event_object_binding_matches(*object, event_object, controller, source)
        }
        (
            TriggerPredicate::TappedOrUntapped { object, tapped },
            AbilityTriggerEvent::OrientationChanged {
                object: event_object,
                tapped: event_tapped,
            },
        ) => {
            tapped == event_tapped
                && event_object_binding_matches(*object, event_object, controller, source)
        }
        (
            TriggerPredicate::CounterChanged {
                object,
                operation,
                counter_name,
                one_or_more,
            },
            AbilityTriggerEvent::CounterChanged {
                object: event_object,
                operation: event_operation,
                counter_name: event_counter,
                amount,
            },
        ) => {
            operation == event_operation
                && counter_name == event_counter
                && (!*one_or_more || *amount > 0)
                && event_object_binding_matches(*object, event_object, controller, source)
        }
        (
            TriggerPredicate::ControllerControls { filter, comparison },
            AbilityTriggerEvent::BattlefieldConditionChanged {
                player,
                prior_matching_count,
                current_matching_count,
                battlefield_evidence_complete,
            },
        ) => {
            if *player != controller
                || !*battlefield_evidence_complete
                || !state
                    .action_state
                    .action_world()
                    .battlefield_evidence_complete
            {
                return Err(AbilityEnvelopeRuntimeError::TriggerDidNotMatch);
            }
            let world = state.action_state.action_world();
            let actual_count = world
                .objects
                .values()
                .map(|object| {
                    object_matches_filter(object, filter, controller, Some(source), world)
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|matched| *matched)
                .count();
            let actual_count = u32::try_from(actual_count).map_err(|_| {
                AbilityEnvelopeRuntimeError::Action(OracleActionRuntimeError::AmountOverflow)
            })?;
            actual_count == *current_matching_count
                && !count_comparison_matches(comparison, *prior_matching_count)
                && count_comparison_matches(comparison, *current_matching_count)
        }
        _ => false,
    };
    if !matched {
        return Err(AbilityEnvelopeRuntimeError::TriggerDidNotMatch);
    }
    Ok(trigger_event_bindings(event))
}

fn validate_trigger_player_evidence<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    predicate: &TriggerPredicate,
    controller: PlayerId,
    event: &AbilityTriggerEvent,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    let world = state.action_state.action_world();
    let event_player = match event {
        AbilityTriggerEvent::StepOrPhase { active_player, .. } => Some(*active_player),
        AbilityTriggerEvent::BattlefieldConditionChanged { player, .. } => Some(*player),
        AbilityTriggerEvent::Spell { player, .. } => Some(*player),
        AbilityTriggerEvent::BecameTarget { cause, .. } => Some(cause.controller),
        AbilityTriggerEvent::Attacked {
            defender: Some(AttackDefender::Player(player)),
            ..
        } => Some(*player),
        AbilityTriggerEvent::CombatDamage {
            recipient: ResolvedCombatDamageRecipient::Player(player),
            ..
        } => Some(*player),
        AbilityTriggerEvent::Damage {
            recipient: ResolvedDamageRecipient::Player(player),
            ..
        } => Some(*player),
        AbilityTriggerEvent::ZoneChanged { .. }
        | AbilityTriggerEvent::Attacked { .. }
        | AbilityTriggerEvent::Blocked { .. }
        | AbilityTriggerEvent::CombatDamage { .. }
        | AbilityTriggerEvent::Damage { .. }
        | AbilityTriggerEvent::OrientationChanged { .. }
        | AbilityTriggerEvent::CounterChanged { .. } => None,
    };
    if let Some(player) = event_player
        && !world.players.contains_key(&player)
    {
        return Err(AbilityEnvelopeRuntimeError::UnknownPlayer(player));
    }
    let requires_opponent_relation = matches!(
        predicate,
        TriggerPredicate::StepOrPhase {
            turn_owner: TurnOwner::Opponents,
            ..
        } | TriggerPredicate::Attacks {
            recipient: AttackRecipientRequirement::Opponent,
            ..
        } | TriggerPredicate::Cast {
            player: AbilityPlayerBinding::AnOpponent,
            ..
        } | TriggerPredicate::BecomesTarget {
            actor: Some(AbilityPlayerBinding::AnOpponent),
            ..
        } | TriggerPredicate::DealsCombatDamage {
            recipient: CombatDamageRecipient::Opponent,
            ..
        }
    );
    if requires_opponent_relation && !world.opponents.contains_key(&controller) {
        return Err(AbilityEnvelopeRuntimeError::IncompletePlayerRelationEvidence(controller));
    }
    Ok(())
}

fn trigger_event_bindings(event: &AbilityTriggerEvent) -> (Option<ObjectRef>, Option<PlayerId>) {
    match event {
        AbilityTriggerEvent::ZoneChanged { object }
        | AbilityTriggerEvent::Blocked { object, .. }
        | AbilityTriggerEvent::OrientationChanged { object, .. }
        | AbilityTriggerEvent::CounterChanged { object, .. } => {
            (Some(object.reference), Some(object.controller))
        }
        AbilityTriggerEvent::Attacked {
            object, defender, ..
        } => (
            Some(object.reference),
            match defender {
                Some(AttackDefender::Player(player)) => Some(*player),
                Some(AttackDefender::Planeswalker(_) | AttackDefender::Battle(_)) | None => None,
            },
        ),
        AbilityTriggerEvent::BecameTarget { object, cause } => {
            (Some(object.reference), Some(cause.controller))
        }
        AbilityTriggerEvent::CombatDamage { source, recipient } => (
            Some(source.reference),
            match recipient {
                ResolvedCombatDamageRecipient::Player(player) => Some(*player),
                ResolvedCombatDamageRecipient::Planeswalker(_)
                | ResolvedCombatDamageRecipient::Battle(_) => None,
            },
        ),
        AbilityTriggerEvent::Damage {
            source, recipient, ..
        } => (
            Some(source.reference),
            match recipient {
                ResolvedDamageRecipient::Player(player) => Some(*player),
                ResolvedDamageRecipient::Object(_) => None,
            },
        ),
        AbilityTriggerEvent::Spell { spell, player, .. } => (Some(spell.reference), Some(*player)),
        AbilityTriggerEvent::StepOrPhase { active_player, .. } => (None, Some(*active_player)),
        AbilityTriggerEvent::BattlefieldConditionChanged { player, .. } => (None, Some(*player)),
    }
}

fn trigger_event_amount(event: &AbilityTriggerEvent) -> Option<u32> {
    match event {
        AbilityTriggerEvent::Damage { amount, .. } => Some(*amount),
        _ => None,
    }
}

fn count_comparison_matches(comparison: &CountComparison, count: u32) -> bool {
    match comparison {
        CountComparison::Exactly(expected) => count == *expected,
        CountComparison::AtLeast(minimum) => count >= *minimum,
        CountComparison::AtMost(maximum) => count <= *maximum,
    }
}

fn event_object_binding_matches(
    binding: AbilityObjectBinding,
    object: &TriggerObjectSnapshot,
    controller: PlayerId,
    source: ObjectRef,
) -> bool {
    match binding {
        AbilityObjectBinding::Source | AbilityObjectBinding::ThisPermanent => {
            object.reference == source
        }
        AbilityObjectBinding::ThisCreature => {
            object.reference == source && object.card_types.contains(&CardType::Creature)
        }
        AbilityObjectBinding::ThisArtifact => {
            object.reference == source && object.card_types.contains(&CardType::Artifact)
        }
        AbilityObjectBinding::ThisEnchantment => {
            object.reference == source && object.card_types.contains(&CardType::Enchantment)
        }
        AbilityObjectBinding::ThisLand => {
            object.reference == source && object.card_types.contains(&CardType::Land)
        }
        AbilityObjectBinding::ThisPlaneswalker => {
            object.reference == source && object.card_types.contains(&CardType::Planeswalker)
        }
        AbilityObjectBinding::ThisBattle => {
            object.reference == source && object.card_types.contains(&CardType::Battle)
        }
        AbilityObjectBinding::AnotherPermanentYouControl => {
            object.reference != source
                && object.controller == controller
                && snapshot_is_permanent(object)
        }
        AbilityObjectBinding::AnotherCreatureYouControl => {
            object.reference != source
                && object.controller == controller
                && object.card_types.contains(&CardType::Creature)
        }
        AbilityObjectBinding::ACreatureYouControl
        | AbilityObjectBinding::OneOrMoreCreaturesYouControl => {
            object.controller == controller && object.card_types.contains(&CardType::Creature)
        }
        AbilityObjectBinding::AnyCreature => object.card_types.contains(&CardType::Creature),
        AbilityObjectBinding::AnyPermanent => snapshot_is_permanent(object),
        AbilityObjectBinding::ACard => object.zone_after != Zone::Battlefield,
        AbilityObjectBinding::ASpell
        | AbilityObjectBinding::ThisSpell
        | AbilityObjectBinding::CopiedSpell => object.zone_after == Zone::Stack,
        AbilityObjectBinding::TargetOfAbility => false,
        AbilityObjectBinding::EnchantedObject => {
            object.attachments.contains(&TriggerAttachmentEvidence {
                source,
                kind: TriggerAttachmentKind::Aura,
            })
        }
        AbilityObjectBinding::EquippedObject => {
            object.attachments.contains(&TriggerAttachmentEvidence {
                source,
                kind: TriggerAttachmentKind::Equipment,
            })
        }
    }
}

fn snapshot_is_permanent(object: &TriggerObjectSnapshot) -> bool {
    object.card_types.iter().any(|card_type| {
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

fn attack_recipient_matches<S: OracleActionStateAdapter>(
    expected: AttackRecipientRequirement,
    actual: Option<AttackDefender>,
    controller: PlayerId,
    state: &AbilityEnvelopeRuntimeState<S>,
) -> bool {
    match expected {
        AttackRecipientRequirement::Any => actual.is_some(),
        AttackRecipientRequirement::Player => matches!(actual, Some(AttackDefender::Player(_))),
        AttackRecipientRequirement::Opponent => matches!(
            actual,
            Some(AttackDefender::Player(player))
                if state
                    .action_state
                    .action_world()
                    .opponents
                    .get(&controller)
                    .is_some_and(|opponents| opponents.contains(&player))
        ),
        AttackRecipientRequirement::PlayerOrBattle => matches!(
            actual,
            Some(AttackDefender::Player(_) | AttackDefender::Battle(_))
        ),
    }
}

fn combat_recipient_matches<S: OracleActionStateAdapter>(
    expected: CombatDamageRecipient,
    actual: ResolvedCombatDamageRecipient,
    controller: PlayerId,
    state: &AbilityEnvelopeRuntimeState<S>,
) -> bool {
    match expected {
        CombatDamageRecipient::Player => {
            matches!(actual, ResolvedCombatDamageRecipient::Player(_))
        }
        CombatDamageRecipient::Opponent => {
            matches!(
                actual,
                ResolvedCombatDamageRecipient::Player(player)
                    if state
                        .action_state
                        .action_world()
                        .opponents
                        .get(&controller)
                        .is_some_and(|opponents| opponents.contains(&player))
            )
        }
        CombatDamageRecipient::Planeswalker => {
            matches!(actual, ResolvedCombatDamageRecipient::Planeswalker(_))
        }
        CombatDamageRecipient::Battle => {
            matches!(actual, ResolvedCombatDamageRecipient::Battle(_))
        }
        CombatDamageRecipient::PlayerOrPlaneswalker => matches!(
            actual,
            ResolvedCombatDamageRecipient::Player(_)
                | ResolvedCombatDamageRecipient::Planeswalker(_)
        ),
        CombatDamageRecipient::PlayerOrBattle => matches!(
            actual,
            ResolvedCombatDamageRecipient::Player(_) | ResolvedCombatDamageRecipient::Battle(_)
        ),
        CombatDamageRecipient::Any => true,
    }
}

fn turn_owner_matches<S: OracleActionStateAdapter>(
    expected: TurnOwner,
    controller: PlayerId,
    active_player: PlayerId,
    state: &AbilityEnvelopeRuntimeState<S>,
) -> bool {
    match expected {
        TurnOwner::Yours => active_player == controller,
        TurnOwner::Opponents => state
            .action_state
            .action_world()
            .opponents
            .get(&controller)
            .is_some_and(|opponents| opponents.contains(&active_player)),
        TurnOwner::EachPlayers | TurnOwner::Any => true,
    }
}

fn player_binding_matches<S: OracleActionStateAdapter>(
    expected: AbilityPlayerBinding,
    controller: PlayerId,
    actual: PlayerId,
    state: &AbilityEnvelopeRuntimeState<S>,
) -> bool {
    match expected {
        AbilityPlayerBinding::You | AbilityPlayerBinding::SourceController => actual == controller,
        AbilityPlayerBinding::AnOpponent => state
            .action_state
            .action_world()
            .opponents
            .get(&controller)
            .is_some_and(|opponents| opponents.contains(&actual)),
        AbilityPlayerBinding::AnyPlayer
        | AbilityPlayerBinding::ActivePlayer
        | AbilityPlayerBinding::ObjectController
        | AbilityPlayerBinding::ObjectOwner
        | AbilityPlayerBinding::SpellCaster => true,
    }
}

fn spell_mode_matches(expected: SpellEventMode, actual: SpellEventMode) -> bool {
    expected == actual
        || matches!(
            (expected, actual),
            (
                SpellEventMode::CastOrCopy,
                SpellEventMode::Cast | SpellEventMode::Copy
            )
        )
}

fn targeting_cause_matches(
    expected: TargetingCauseRequirement,
    actual: TargetingCauseKind,
) -> bool {
    matches!(
        (expected, actual),
        (
            TargetingCauseRequirement::SpellOrAbility,
            TargetingCauseKind::Spell | TargetingCauseKind::Ability
        ) | (TargetingCauseRequirement::Spell, TargetingCauseKind::Spell)
            | (
                TargetingCauseRequirement::Ability,
                TargetingCauseKind::Ability
            )
    )
}

fn evaluate_intervening_condition<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    condition: &InterveningCondition,
    controller: PlayerId,
    source: ObjectRef,
) -> Result<bool, AbilityEnvelopeRuntimeError> {
    let world = state.action_state.action_world();
    match condition {
        InterveningCondition::SourceIsTapped(expected) => world
            .objects
            .get(&source)
            .map(|object| object.tapped == *expected)
            .ok_or(AbilityEnvelopeRuntimeError::StaleSource(source)),
        InterveningCondition::SourceIsAttacking(expected) => world
            .objects
            .get(&source)
            .map(|object| object.attacking == *expected)
            .ok_or(AbilityEnvelopeRuntimeError::StaleSource(source)),
        InterveningCondition::SourceHasCounter {
            counter_name,
            at_least,
        } => world
            .objects
            .get(&source)
            .map(|object| object.counters.get(counter_name).copied().unwrap_or(0) >= *at_least)
            .ok_or(AbilityEnvelopeRuntimeError::StaleSource(source)),
        InterveningCondition::YouControlObject(binding) => {
            if !world.battlefield_evidence_complete {
                return Err(AbilityEnvelopeRuntimeError::IncompleteBattlefieldEvidence);
            }
            Ok(world.objects.values().any(|object| {
                let snapshot = snapshot_from_game_object(object);
                event_object_binding_matches(*binding, &snapshot, controller, source)
                    && object.zone == ActionZone::Battlefield
            }))
        }
        InterveningCondition::YourTurn(expected) => {
            Ok((state.active_player == controller) == *expected)
        }
        InterveningCondition::LifeComparison {
            player,
            comparison,
            amount,
        } => {
            let player = resolve_condition_player(*player, controller, state)?;
            let life = world
                .players
                .get(&player)
                .ok_or(AbilityEnvelopeRuntimeError::InterveningConditionUnavailable)?
                .life;
            Ok(compare_i64(life, *comparison, i64::from(*amount)))
        }
        InterveningCondition::CardsInHandComparison {
            player,
            comparison,
            amount,
        } => {
            if !world.hidden_zone_evidence_complete {
                return Err(AbilityEnvelopeRuntimeError::IncompleteHiddenZoneEvidence);
            }
            let player = resolve_condition_player(*player, controller, state)?;
            let count = world
                .objects
                .values()
                .filter(|object| object.owner == player && object.zone == ActionZone::Hand)
                .count() as i64;
            Ok(compare_i64(count, *comparison, i64::from(*amount)))
        }
    }
}

fn resolve_condition_player<S: OracleActionStateAdapter>(
    binding: AbilityPlayerBinding,
    controller: PlayerId,
    state: &AbilityEnvelopeRuntimeState<S>,
) -> Result<PlayerId, AbilityEnvelopeRuntimeError> {
    match binding {
        AbilityPlayerBinding::You | AbilityPlayerBinding::SourceController => Ok(controller),
        AbilityPlayerBinding::ActivePlayer => Ok(state.active_player),
        _ => Err(AbilityEnvelopeRuntimeError::InterveningConditionUnavailable),
    }
}

fn compare_i64(left: i64, comparison: NumericComparison, right: i64) -> bool {
    match comparison {
        NumericComparison::Exactly => left == right,
        NumericComparison::AtLeast => left >= right,
        NumericComparison::AtMost => left <= right,
        NumericComparison::GreaterThan => left > right,
        NumericComparison::LessThan => left < right,
    }
}

fn snapshot_from_game_object(object: &GameObject) -> TriggerObjectSnapshot {
    TriggerObjectSnapshot {
        reference: object.reference,
        owner: object.owner,
        controller: object.controller,
        zone_before: Zone::from_action_zone(object.zone),
        zone_after: Zone::from_action_zone(object.zone),
        card_types: object.card_types.clone(),
        tapped: object.tapped,
        attachments: BTreeSet::new(),
    }
}

fn validate_activation_restriction<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    restriction: &ActivationRestriction,
    program_digest: &str,
    controller: PlayerId,
    source: ObjectRef,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    let satisfied = match restriction {
        ActivationRestriction::AnyTime => true,
        ActivationRestriction::SorceryTiming => {
            state.active_player == controller
                && state.priority_player == controller
                && state.stack_empty
                && matches!(
                    state.phase,
                    TurnPhase::PrecombatMain | TurnPhase::PostcombatMain
                )
        }
        ActivationRestriction::DuringYourTurn => state.active_player == controller,
        ActivationRestriction::DuringAnOpponentsTurn => state
            .action_state
            .action_world()
            .opponents
            .get(&controller)
            .is_some_and(|opponents| opponents.contains(&state.active_player)),
        ActivationRestriction::DuringCombat => state.phase == TurnPhase::Combat,
        ActivationRestriction::BeforeAttackersAreDeclared => {
            state.phase == TurnPhase::Combat && !state.attackers_declared
        }
        ActivationRestriction::OnlyOnceEachTurn => {
            state
                .activation_count_by_turn
                .get(&(program_digest.to_owned(), source, state.turn_number))
                .copied()
                .unwrap_or(0)
                == 0
        }
        ActivationRestriction::OnlyOnce => !state
            .activated_once
            .contains(&(program_digest.to_owned(), source)),
        ActivationRestriction::SourceWasNotCastThisTurn => {
            state.source_cast_turn.get(&source).copied() != Some(state.turn_number)
        }
        ActivationRestriction::SourceEnteredThisTurn => {
            state.source_entered_turn.get(&source).copied() == Some(state.turn_number)
        }
        ActivationRestriction::Combined(restrictions) => restrictions.iter().all(|restriction| {
            validate_activation_restriction(state, restriction, program_digest, controller, source)
                .is_ok()
        }),
    };
    if satisfied {
        Ok(())
    } else if matches!(
        restriction,
        ActivationRestriction::OnlyOnceEachTurn | ActivationRestriction::OnlyOnce
    ) {
        Err(AbilityEnvelopeRuntimeError::ActivationLimitReached)
    } else {
        Err(AbilityEnvelopeRuntimeError::TimingRestrictionNotMet)
    }
}

fn note_activation<S: OracleActionStateAdapter>(
    state: &mut AbilityEnvelopeRuntimeState<S>,
    restriction: &ActivationRestriction,
    program_digest: &str,
    source: ObjectRef,
) {
    match restriction {
        ActivationRestriction::OnlyOnceEachTurn => {
            *state
                .activation_count_by_turn
                .entry((program_digest.to_owned(), source, state.turn_number))
                .or_default() += 1;
        }
        ActivationRestriction::OnlyOnce => {
            state
                .activated_once
                .insert((program_digest.to_owned(), source));
        }
        ActivationRestriction::Combined(restrictions) => {
            for restriction in restrictions {
                note_activation(state, restriction, program_digest, source);
            }
        }
        _ => {}
    }
}

fn validate_payment_shape(
    costs: &[ActivationCost],
    payment: &AbilityActivationPayment,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    for index in payment.object_selections.keys() {
        if !matches!(
            costs.get(*index),
            Some(ActivationCost::Sacrifice(_) | ActivationCost::TapObjects { .. })
        ) {
            return Err(AbilityEnvelopeRuntimeError::UnexpectedPaymentSelection(
                *index,
            ));
        }
    }
    for index in payment.card_selections.keys() {
        if !matches!(
            costs.get(*index),
            Some(ActivationCost::Discard(_) | ActivationCost::Exile(_))
        ) {
            return Err(AbilityEnvelopeRuntimeError::UnexpectedPaymentSelection(
                *index,
            ));
        }
    }
    for index in &payment.random_selection_proven {
        if !matches!(
            costs.get(*index),
            Some(ActivationCost::Discard(CardCost { random: true, .. }))
        ) {
            return Err(AbilityEnvelopeRuntimeError::UnexpectedPaymentSelection(
                *index,
            ));
        }
    }
    let has_mana_cost = costs
        .iter()
        .any(|cost| matches!(cost, ActivationCost::Mana(_)));
    if !has_mana_cost
        && (!payment.mana_units.is_empty() || !payment.phyrexian_life_symbol_indices.is_empty())
    {
        return Err(AbilityEnvelopeRuntimeError::UnexpectedManaPayment);
    }
    let costs_use_x = costs.iter().any(activation_cost_uses_x);
    if costs_use_x && payment.x_value.is_none() {
        return Err(AbilityEnvelopeRuntimeError::MissingXValue);
    }
    if !costs_use_x && payment.x_value.is_some() {
        return Err(AbilityEnvelopeRuntimeError::UnexpectedXValue);
    }
    Ok(())
}

fn activation_cost_uses_x(cost: &ActivationCost) -> bool {
    match cost {
        ActivationCost::Mana(cost) => cost
            .symbols
            .iter()
            .any(|symbol| matches!(symbol, ManaSymbol::X)),
        ActivationCost::Sacrifice(cost) => cost.amount == CostAmount::X,
        ActivationCost::Discard(cost) | ActivationCost::Exile(cost) => cost.amount == CostAmount::X,
        ActivationCost::RemoveCounters { amount, .. }
        | ActivationCost::TapObjects { amount, .. } => *amount == CostAmount::X,
        ActivationCost::TapSource | ActivationCost::UntapSource | ActivationCost::PayLife(_) => {
            false
        }
    }
}

fn pay_activation_cost<S: OracleActionStateAdapter>(
    state: &mut AbilityEnvelopeRuntimeState<S>,
    controller: PlayerId,
    source: ObjectRef,
    cost_index: usize,
    cost: &ActivationCost,
    payment: &AbilityActivationPayment,
) -> Result<PaidActivationCostReceipt, AbilityEnvelopeRuntimeError> {
    let mut receipt = PaidActivationCostReceipt::empty(cost_index);
    match cost {
        ActivationCost::Mana(cost) => {
            let (spent, life_paid) = pay_mana_cost(state, controller, cost, payment, cost_index)?;
            receipt.mana_units = spent;
            receipt.life_paid = life_paid;
        }
        ActivationCost::TapSource => {
            validate_source_identity(state, controller, source, true)?;
            validate_tap_symbol_cost(state, source)?;
            let object = state
                .action_state
                .action_world_mut()
                .objects
                .get_mut(&source)
                .ok_or(AbilityEnvelopeRuntimeError::StaleSource(source))?;
            if object.tapped {
                return Err(AbilityEnvelopeRuntimeError::IllegalPaymentObject {
                    cost_index,
                    object: source,
                });
            }
            object.tapped = true;
            receipt.tapped.push(source);
        }
        ActivationCost::UntapSource => {
            validate_source_identity(state, controller, source, true)?;
            validate_tap_symbol_cost(state, source)?;
            let object = state
                .action_state
                .action_world_mut()
                .objects
                .get_mut(&source)
                .ok_or(AbilityEnvelopeRuntimeError::StaleSource(source))?;
            if !object.tapped {
                return Err(AbilityEnvelopeRuntimeError::IllegalPaymentObject {
                    cost_index,
                    object: source,
                });
            }
            object.tapped = false;
            receipt.untapped.push(source);
        }
        ActivationCost::Sacrifice(objects) => {
            let selected = resolve_object_cost_selection(
                state,
                controller,
                source,
                cost_index,
                objects,
                payment.object_selections.get(&cost_index),
                payment.x_value,
            )?;
            for object in selected {
                let next = move_cost_object(state, object, Zone::Graveyard)?;
                receipt.sacrificed.push((object, next));
            }
        }
        ActivationCost::Discard(cards) => {
            let selected =
                resolve_card_cost_selection(state, controller, source, cost_index, cards, payment)?;
            for object in selected {
                let next = move_cost_object(state, object, Zone::Graveyard)?;
                receipt.discarded.push((object, next));
            }
        }
        ActivationCost::Exile(cards) => {
            let selected =
                resolve_card_cost_selection(state, controller, source, cost_index, cards, payment)?;
            for object in selected {
                let next = move_cost_object(state, object, Zone::Exile)?;
                receipt.exiled.push((object, next));
            }
        }
        ActivationCost::PayLife(amount) => {
            pay_life(state, controller, *amount)?;
            receipt.life_paid = *amount;
        }
        ActivationCost::RemoveCounters {
            object,
            counter_name,
            amount,
        } => {
            let amount = resolve_cost_amount_value(*amount, payment.x_value, 0, None, cost_index)?;
            let selected = resolve_bound_cost_objects(state, *object, controller, source)?;
            if selected.len() != 1 {
                return Err(AbilityEnvelopeRuntimeError::WrongPaymentCardinality {
                    cost_index,
                    expected: CostAmount::Fixed(1),
                    actual: selected.len(),
                });
            }
            let reference = selected[0];
            let current = state
                .action_state
                .action_world()
                .objects
                .get(&reference)
                .and_then(|value| value.counters.get(counter_name))
                .copied()
                .unwrap_or(0);
            if current < amount {
                return Err(AbilityEnvelopeRuntimeError::InsufficientCounters {
                    object: reference,
                    counter: counter_name.clone(),
                    required: amount,
                    available: current,
                });
            }
            let object = state
                .action_state
                .action_world_mut()
                .objects
                .get_mut(&reference)
                .ok_or(AbilityEnvelopeRuntimeError::StaleSource(reference))?;
            if amount == current {
                object.counters.remove(counter_name);
            } else {
                object
                    .counters
                    .insert(counter_name.clone(), current - amount);
            }
            receipt
                .counters_removed
                .insert((reference, counter_name.clone()), amount);
        }
        ActivationCost::TapObjects { objects, .. } => {
            if !state.tap_cost_legality_complete {
                return Err(AbilityEnvelopeRuntimeError::IncompleteTapCostEvidence);
            }
            let selected = resolve_object_cost_selection(
                state,
                controller,
                source,
                cost_index,
                objects,
                payment.object_selections.get(&cost_index),
                payment.x_value,
            )?;
            for reference in selected {
                let object = state
                    .action_state
                    .action_world_mut()
                    .objects
                    .get_mut(&reference)
                    .ok_or(AbilityEnvelopeRuntimeError::StaleSource(reference))?;
                if object.tapped {
                    return Err(AbilityEnvelopeRuntimeError::IllegalPaymentObject {
                        cost_index,
                        object: reference,
                    });
                }
                object.tapped = true;
                receipt.tapped.push(reference);
            }
        }
    }
    Ok(receipt)
}

fn validate_tap_symbol_cost<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    source: ObjectRef,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    if !state.tap_cost_legality_complete {
        return Err(AbilityEnvelopeRuntimeError::IncompleteTapCostEvidence);
    }
    let object = state
        .action_state
        .action_world()
        .objects
        .get(&source)
        .ok_or(AbilityEnvelopeRuntimeError::StaleSource(source))?;
    if object.card_types.contains(&CardType::Creature)
        && !state.tap_symbol_eligible_creatures.contains(&source)
    {
        return Err(AbilityEnvelopeRuntimeError::CreatureCannotPayTapSymbol(
            source,
        ));
    }
    Ok(())
}

fn activation_cost_payment_rank(cost: &ActivationCost) -> u8 {
    match cost {
        ActivationCost::Mana(_)
        | ActivationCost::PayLife(_)
        | ActivationCost::RemoveCounters { .. } => 0,
        ActivationCost::TapSource
        | ActivationCost::UntapSource
        | ActivationCost::TapObjects { .. } => 1,
        ActivationCost::Discard(_) | ActivationCost::Sacrifice(_) => 2,
        ActivationCost::Exile(_) => 3,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManaRequirement {
    Any,
    Exact(ManaResource),
    Snow,
    Hybrid(ManaResource, ManaResource),
}

fn pay_mana_cost<S: OracleActionStateAdapter>(
    state: &mut AbilityEnvelopeRuntimeState<S>,
    controller: PlayerId,
    cost: &ManaCost,
    payment: &AbilityActivationPayment,
    _cost_index: usize,
) -> Result<(Vec<u64>, u32), AbilityEnvelopeRuntimeError> {
    let variants = mana_requirement_variants(cost, payment)?;
    let mut chosen = Vec::with_capacity(payment.mana_units.len());
    let mut seen = BTreeSet::new();
    {
        let pool = state.mana_pools.entry(controller).or_default();
        for id in &payment.mana_units {
            if !seen.insert(*id) {
                return Err(AbilityEnvelopeRuntimeError::DuplicateManaUnit(*id));
            }
            let unit = pool
                .get(id)
                .copied()
                .ok_or(AbilityEnvelopeRuntimeError::MissingManaUnit(*id))?;
            if !unit.spend_restrictions_satisfied {
                return Err(AbilityEnvelopeRuntimeError::ManaSpendRestrictionNotSatisfied(*id));
            }
            chosen.push(unit);
        }
    }
    let matching_variant = variants
        .into_iter()
        .find(|requirements| mana_units_satisfy(requirements, &chosen))
        .ok_or(AbilityEnvelopeRuntimeError::InvalidManaPayment)?;
    if matching_variant.len() != chosen.len() {
        return Err(AbilityEnvelopeRuntimeError::UnexpectedManaPayment);
    }
    let life_paid = u32::try_from(payment.phyrexian_life_symbol_indices.len())
        .ok()
        .and_then(|count| count.checked_mul(2))
        .ok_or(AbilityEnvelopeRuntimeError::LifeOverflow)?;
    if life_paid > 0 {
        pay_life(state, controller, life_paid)?;
    }
    let pool = state.mana_pools.entry(controller).or_default();
    for unit in &chosen {
        pool.remove(&unit.id);
    }
    Ok((payment.mana_units.clone(), life_paid))
}

fn mana_requirement_variants(
    cost: &ManaCost,
    payment: &AbilityActivationPayment,
) -> Result<Vec<Vec<ManaRequirement>>, AbilityEnvelopeRuntimeError> {
    let mut variants = vec![Vec::new()];
    let mut saw_x = false;
    for (index, symbol) in cost.symbols.iter().enumerate() {
        let branches: Vec<Vec<ManaRequirement>> = match symbol {
            ManaSymbol::Generic(amount) => {
                vec![vec![ManaRequirement::Any; *amount as usize]]
            }
            ManaSymbol::White => vec![vec![ManaRequirement::Exact(ManaResource::White)]],
            ManaSymbol::Blue => vec![vec![ManaRequirement::Exact(ManaResource::Blue)]],
            ManaSymbol::Black => vec![vec![ManaRequirement::Exact(ManaResource::Black)]],
            ManaSymbol::Red => vec![vec![ManaRequirement::Exact(ManaResource::Red)]],
            ManaSymbol::Green => vec![vec![ManaRequirement::Exact(ManaResource::Green)]],
            ManaSymbol::Colorless => {
                vec![vec![ManaRequirement::Exact(ManaResource::Colorless)]]
            }
            ManaSymbol::Snow => vec![vec![ManaRequirement::Snow]],
            ManaSymbol::X => {
                saw_x = true;
                let value = payment
                    .x_value
                    .ok_or(AbilityEnvelopeRuntimeError::MissingXValue)?;
                vec![vec![ManaRequirement::Any; value as usize]]
            }
            ManaSymbol::Phyrexian(color) => {
                let resource = parse_mana_resource(color)
                    .ok_or(AbilityEnvelopeRuntimeError::InvalidManaPayment)?;
                if payment.phyrexian_life_symbol_indices.contains(&index) {
                    vec![Vec::new()]
                } else {
                    vec![vec![ManaRequirement::Exact(resource)]]
                }
            }
            ManaSymbol::Hybrid(first, second) => {
                let first_resource = parse_mana_resource(first);
                let second_resource = parse_mana_resource(second);
                match (
                    first_resource,
                    second_resource,
                    first.as_str(),
                    second.as_str(),
                ) {
                    (Some(first), Some(second), _, _) => {
                        vec![vec![ManaRequirement::Hybrid(first, second)]]
                    }
                    (None, Some(second), "2", _) => vec![
                        vec![ManaRequirement::Any, ManaRequirement::Any],
                        vec![ManaRequirement::Exact(second)],
                    ],
                    (Some(first), None, _, "2") => vec![
                        vec![ManaRequirement::Exact(first)],
                        vec![ManaRequirement::Any, ManaRequirement::Any],
                    ],
                    _ => return Err(AbilityEnvelopeRuntimeError::InvalidManaPayment),
                }
            }
        };
        let mut next = Vec::new();
        for variant in variants {
            for branch in &branches {
                let mut combined = variant.clone();
                combined.extend(branch.iter().copied());
                next.push(combined);
            }
        }
        variants = next;
    }
    if !saw_x && payment.x_value.is_some() {
        return Err(AbilityEnvelopeRuntimeError::UnexpectedXValue);
    }
    for index in &payment.phyrexian_life_symbol_indices {
        if !matches!(cost.symbols.get(*index), Some(ManaSymbol::Phyrexian(_))) {
            return Err(AbilityEnvelopeRuntimeError::InvalidManaPayment);
        }
    }
    Ok(variants)
}

fn parse_mana_resource(source: &str) -> Option<ManaResource> {
    match source {
        "W" => Some(ManaResource::White),
        "U" => Some(ManaResource::Blue),
        "B" => Some(ManaResource::Black),
        "R" => Some(ManaResource::Red),
        "G" => Some(ManaResource::Green),
        "C" => Some(ManaResource::Colorless),
        _ => None,
    }
}

fn mana_units_satisfy(requirements: &[ManaRequirement], units: &[AbilityManaUnit]) -> bool {
    if requirements.len() != units.len() {
        return false;
    }
    let mut ordered = requirements.to_vec();
    ordered.sort_by_key(|requirement| match requirement {
        ManaRequirement::Exact(_) => 0,
        ManaRequirement::Snow => 1,
        ManaRequirement::Hybrid(_, _) => 2,
        ManaRequirement::Any => 3,
    });
    let mut used = vec![false; units.len()];
    assign_mana_requirements(&ordered, units, &mut used, 0)
}

fn assign_mana_requirements(
    requirements: &[ManaRequirement],
    units: &[AbilityManaUnit],
    used: &mut [bool],
    index: usize,
) -> bool {
    if index == requirements.len() {
        return true;
    }
    for unit_index in 0..units.len() {
        if used[unit_index] || !mana_unit_matches(requirements[index], units[unit_index]) {
            continue;
        }
        used[unit_index] = true;
        if assign_mana_requirements(requirements, units, used, index + 1) {
            return true;
        }
        used[unit_index] = false;
    }
    false
}

fn mana_unit_matches(requirement: ManaRequirement, unit: AbilityManaUnit) -> bool {
    match requirement {
        ManaRequirement::Any => true,
        ManaRequirement::Exact(resource) => unit.resource == resource,
        ManaRequirement::Snow => unit.snow,
        ManaRequirement::Hybrid(first, second) => unit.resource == first || unit.resource == second,
    }
}

fn pay_life<S: OracleActionStateAdapter>(
    state: &mut AbilityEnvelopeRuntimeState<S>,
    player: PlayerId,
    amount: u32,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    let player_state = state
        .action_state
        .action_world_mut()
        .players
        .get_mut(&player)
        .ok_or(AbilityEnvelopeRuntimeError::InsufficientLife)?;
    if player_state.life < i64::from(amount) {
        return Err(AbilityEnvelopeRuntimeError::InsufficientLife);
    }
    player_state.life = player_state
        .life
        .checked_sub(i64::from(amount))
        .ok_or(AbilityEnvelopeRuntimeError::LifeOverflow)?;
    Ok(())
}

fn resolve_object_cost_selection<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    controller: PlayerId,
    source: ObjectRef,
    cost_index: usize,
    cost: &ObjectCost,
    selected: Option<&Vec<ObjectRef>>,
    x_value: Option<u32>,
) -> Result<Vec<ObjectRef>, AbilityEnvelopeRuntimeError> {
    if cost.source_only {
        if selected.is_some_and(|values| !values.is_empty()) {
            return Err(AbilityEnvelopeRuntimeError::UnexpectedPaymentSelection(
                cost_index,
            ));
        }
        validate_object_cost_candidate(state, controller, source, source, cost_index, cost)?;
        return Ok(vec![source]);
    }
    let selected = selected.ok_or(AbilityEnvelopeRuntimeError::MissingPaymentSelection(
        cost_index,
    ))?;
    let eligible_count = state
        .action_state
        .action_world()
        .objects
        .values()
        .filter(|object| {
            validate_object_cost_candidate(
                state,
                controller,
                source,
                object.reference,
                cost_index,
                cost,
            )
            .is_ok()
        })
        .count();
    validate_cost_cardinality(
        cost_index,
        cost.amount,
        selected.len(),
        eligible_count,
        x_value,
    )?;
    let mut unique = BTreeSet::new();
    for reference in selected {
        if !unique.insert(*reference) {
            return Err(AbilityEnvelopeRuntimeError::DuplicatePaymentObject(
                *reference,
            ));
        }
        validate_object_cost_candidate(state, controller, source, *reference, cost_index, cost)?;
    }
    Ok(selected.clone())
}

fn validate_object_cost_candidate<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    controller: PlayerId,
    source: ObjectRef,
    reference: ObjectRef,
    cost_index: usize,
    cost: &ObjectCost,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    let Some(object) = state.action_state.action_world().objects.get(&reference) else {
        return Err(AbilityEnvelopeRuntimeError::IllegalPaymentObject {
            cost_index,
            object: reference,
        });
    };
    if object.zone != ActionZone::Battlefield
        || cost.other_than_source && reference == source
        || cost.controller == CostController::You && object.controller != controller
        || cost
            .filter
            .tapped
            .is_some_and(|tapped| object.tapped != tapped)
        || cost
            .filter
            .nontoken
            .is_some_and(|nontoken| nontoken == object.is_token)
        || !cost_object_types_match(&cost.filter.card_types, &object.card_types)
    {
        return Err(AbilityEnvelopeRuntimeError::IllegalPaymentObject {
            cost_index,
            object: reference,
        });
    }
    Ok(())
}

fn cost_object_types_match(required: &BTreeSet<CostCardType>, actual: &BTreeSet<CardType>) -> bool {
    required.iter().all(|required| match required {
        CostCardType::Artifact => actual.contains(&CardType::Artifact),
        CostCardType::Battle => actual.contains(&CardType::Battle),
        CostCardType::Creature => actual.contains(&CardType::Creature),
        CostCardType::Enchantment => actual.contains(&CardType::Enchantment),
        CostCardType::Land => actual.contains(&CardType::Land),
        CostCardType::Planeswalker => actual.contains(&CardType::Planeswalker),
        CostCardType::Permanent => actual.iter().any(|card_type| {
            matches!(
                card_type,
                CardType::Artifact
                    | CardType::Battle
                    | CardType::Creature
                    | CardType::Enchantment
                    | CardType::Land
                    | CardType::Planeswalker
            )
        }),
    })
}

fn resolve_card_cost_selection<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    controller: PlayerId,
    source: ObjectRef,
    cost_index: usize,
    cost: &CardCost,
    payment: &AbilityActivationPayment,
) -> Result<Vec<ObjectRef>, AbilityEnvelopeRuntimeError> {
    if !state
        .action_state
        .action_world()
        .hidden_zone_evidence_complete
    {
        return Err(AbilityEnvelopeRuntimeError::IncompleteHiddenZoneEvidence);
    }
    if cost.source_only {
        if payment
            .card_selections
            .get(&cost_index)
            .is_some_and(|values| !values.is_empty())
        {
            return Err(AbilityEnvelopeRuntimeError::UnexpectedPaymentSelection(
                cost_index,
            ));
        }
        validate_card_cost_candidate(state, controller, source, source, cost_index, cost)?;
        return Ok(vec![source]);
    }
    let selected = payment.card_selections.get(&cost_index).ok_or(
        AbilityEnvelopeRuntimeError::MissingPaymentSelection(cost_index),
    )?;
    if cost.random && !payment.random_selection_proven.contains(&cost_index) {
        return Err(AbilityEnvelopeRuntimeError::RandomSelectionEvidenceMissing(
            cost_index,
        ));
    }
    let eligible_count = state
        .action_state
        .action_world()
        .objects
        .values()
        .filter(|object| {
            validate_card_cost_candidate(
                state,
                controller,
                source,
                object.reference,
                cost_index,
                cost,
            )
            .is_ok()
        })
        .count();
    validate_cost_cardinality(
        cost_index,
        cost.amount,
        selected.len(),
        eligible_count,
        payment.x_value,
    )?;
    let mut unique = BTreeSet::new();
    for reference in selected {
        if !unique.insert(*reference) {
            return Err(AbilityEnvelopeRuntimeError::DuplicatePaymentObject(
                *reference,
            ));
        }
        validate_card_cost_candidate(state, controller, source, *reference, cost_index, cost)?;
    }
    Ok(selected.clone())
}

fn validate_card_cost_candidate<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    controller: PlayerId,
    source: ObjectRef,
    reference: ObjectRef,
    cost_index: usize,
    cost: &CardCost,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    let Some(object) = state.action_state.action_world().objects.get(&reference) else {
        return Err(AbilityEnvelopeRuntimeError::IllegalPaymentObject {
            cost_index,
            object: reference,
        });
    };
    if object.zone != cost.zone.to_action_zone()
        || object.owner != controller
        || cost.filter.other_than_source && reference == source
        || !cost_object_types_match(&cost.filter.card_types, &object.card_types)
    {
        return Err(AbilityEnvelopeRuntimeError::IllegalPaymentObject {
            cost_index,
            object: reference,
        });
    }
    Ok(())
}

fn validate_cost_cardinality(
    cost_index: usize,
    expected: CostAmount,
    actual: usize,
    eligible: usize,
    x_value: Option<u32>,
) -> Result<(), AbilityEnvelopeRuntimeError> {
    let valid = match expected {
        CostAmount::Fixed(amount) => actual == amount as usize,
        CostAmount::X => {
            actual == x_value.ok_or(AbilityEnvelopeRuntimeError::MissingXValue)? as usize
        }
        CostAmount::AnyNumber => actual <= eligible,
        CostAmount::OneOrMore => actual >= 1 && actual <= eligible,
        CostAmount::All => actual == eligible,
    };
    valid
        .then_some(())
        .ok_or(AbilityEnvelopeRuntimeError::WrongPaymentCardinality {
            cost_index,
            expected,
            actual,
        })
}

fn resolve_cost_amount_value(
    amount: CostAmount,
    x_value: Option<u32>,
    selected: usize,
    eligible: Option<usize>,
    cost_index: usize,
) -> Result<u32, AbilityEnvelopeRuntimeError> {
    match amount {
        CostAmount::Fixed(amount) => Ok(amount),
        CostAmount::X => x_value.ok_or(AbilityEnvelopeRuntimeError::MissingXValue),
        CostAmount::AnyNumber => u32::try_from(selected).map_err(|_| {
            AbilityEnvelopeRuntimeError::WrongPaymentCardinality {
                cost_index,
                expected: amount,
                actual: selected,
            }
        }),
        CostAmount::OneOrMore if selected > 0 => u32::try_from(selected).map_err(|_| {
            AbilityEnvelopeRuntimeError::WrongPaymentCardinality {
                cost_index,
                expected: amount,
                actual: selected,
            }
        }),
        CostAmount::All => u32::try_from(eligible.unwrap_or(selected)).map_err(|_| {
            AbilityEnvelopeRuntimeError::WrongPaymentCardinality {
                cost_index,
                expected: amount,
                actual: selected,
            }
        }),
        _ => Err(AbilityEnvelopeRuntimeError::WrongPaymentCardinality {
            cost_index,
            expected: amount,
            actual: selected,
        }),
    }
}

fn resolve_bound_cost_objects<S: OracleActionStateAdapter>(
    state: &AbilityEnvelopeRuntimeState<S>,
    binding: AbilityObjectBinding,
    controller: PlayerId,
    source: ObjectRef,
) -> Result<Vec<ObjectRef>, AbilityEnvelopeRuntimeError> {
    match binding {
        AbilityObjectBinding::Source
        | AbilityObjectBinding::ThisPermanent
        | AbilityObjectBinding::ThisCreature
        | AbilityObjectBinding::ThisArtifact
        | AbilityObjectBinding::ThisEnchantment
        | AbilityObjectBinding::ThisLand
        | AbilityObjectBinding::ThisPlaneswalker
        | AbilityObjectBinding::ThisBattle => {
            let object = state
                .action_state
                .action_world()
                .objects
                .get(&source)
                .ok_or(AbilityEnvelopeRuntimeError::StaleSource(source))?;
            let snapshot = snapshot_from_game_object(object);
            event_object_binding_matches(binding, &snapshot, controller, source)
                .then_some(vec![source])
                .ok_or(AbilityEnvelopeRuntimeError::IllegalPaymentObject {
                    cost_index: usize::MAX,
                    object: source,
                })
        }
        _ => Err(AbilityEnvelopeRuntimeError::IllegalPaymentObject {
            cost_index: usize::MAX,
            object: source,
        }),
    }
}

fn move_cost_object<S: OracleActionStateAdapter>(
    state: &mut AbilityEnvelopeRuntimeState<S>,
    reference: ObjectRef,
    destination: Zone,
) -> Result<ObjectRef, AbilityEnvelopeRuntimeError> {
    let world = state.action_state.action_world_mut();
    let mut object = world
        .objects
        .remove(&reference)
        .ok_or(AbilityEnvelopeRuntimeError::StaleSource(reference))?;
    let next_incarnation = object
        .reference
        .incarnation_id
        .0
        .checked_add(1)
        .ok_or(AbilityEnvelopeRuntimeError::ObjectIdOverflow(reference))?;
    let next = ObjectRef {
        object_id: reference.object_id,
        incarnation_id: IncarnationId(next_incarnation),
    };
    object.reference = next;
    object.zone = destination.to_action_zone();
    object.tapped = false;
    object.attacking = false;
    object.blocking = false;
    object.marked_damage = 0;
    object.deathtouch_damage = false;
    world.objects.insert(next, object);
    Ok(next)
}
/// Parse only the exact outer envelope. The action body remains source text
/// until the child action compiler proves that it is fully executable.
pub fn parse_ability_envelope_shape(
    input: AbilityEnvelopeCompileInput<'_>,
) -> Result<AbilityEnvelopeShape, AbilityEnvelopeRejection> {
    if !is_complete_single_line(input.exact_source)
        || !is_complete_single_line(input.normalized_source)
    {
        return Err(AbilityEnvelopeRejection::EmptyOrMalformedSource);
    }
    if reviewed_ability_envelope_normalized_source(input.exact_source) != input.normalized_source {
        return Err(AbilityEnvelopeRejection::NormalizationMismatch);
    }

    let source = input.normalized_source;
    if !source.ends_with('.') || source.ends_with("..") {
        return Err(AbilityEnvelopeRejection::EmptyOrMalformedSource);
    }
    let (envelope, exact_body) = if starts_trigger_envelope(source) {
        parse_triggered_shape(source)?
    } else if contains_top_level_colon(source) {
        parse_activated_shape(source)?
    } else {
        return Err(AbilityEnvelopeRejection::NotAbilityEnvelope);
    };
    if exact_body.trim().is_empty() || exact_body.trim() != exact_body {
        return Err(AbilityEnvelopeRejection::UnsupportedActionBody);
    }
    let shape_digest = ability_shape_semantic_digest(
        input.exact_source,
        input.normalized_source,
        &envelope,
        exact_body,
    );
    Ok(AbilityEnvelopeShape {
        exact_source: input.exact_source.to_owned(),
        normalized_source: input.normalized_source.to_owned(),
        exact_body: exact_body.to_owned(),
        envelope,
        shape_digest,
    })
}

fn parse_triggered_shape(
    source: &str,
) -> Result<(ParsedAbilityEnvelope, &str), AbilityEnvelopeRejection> {
    let comma_indices = top_level_delimiter_indices(source, ',');
    for comma in comma_indices {
        let (header, body_with_comma) = source.split_at(comma);
        let Some(mut body) = body_with_comma
            .strip_prefix(',')
            .and_then(|value| value.strip_prefix(' '))
        else {
            continue;
        };
        let Ok(predicate) = parse_trigger_predicate(header) else {
            continue;
        };
        let mut intervening_if = None;
        if let Some(condition_and_body) = body.strip_prefix("if ")
            && let Some(condition_comma) = find_top_level_delimiter(condition_and_body, ',')
        {
            let (condition, following) = condition_and_body.split_at(condition_comma);
            let Ok(condition) = parse_intervening_condition(condition) else {
                continue;
            };
            let Some(following) = following
                .strip_prefix(',')
                .and_then(|value| value.strip_prefix(' '))
            else {
                continue;
            };
            intervening_if = Some(condition);
            body = following;
        }
        return Ok((
            ParsedAbilityEnvelope::Triggered(TriggerEnvelope {
                predicate,
                intervening_if,
            }),
            body,
        ));
    }
    Err(AbilityEnvelopeRejection::UnsupportedTriggerPredicate)
}

fn parse_activated_shape(
    source: &str,
) -> Result<(ParsedAbilityEnvelope, &str), AbilityEnvelopeRejection> {
    let colon = find_top_level_delimiter(source, ':')
        .ok_or(AbilityEnvelopeRejection::AmbiguousComposition)?;
    let (cost_source, body_with_colon) = source.split_at(colon);
    let body_and_restriction = body_with_colon
        .strip_prefix(':')
        .and_then(|value| value.strip_prefix(' '))
        .ok_or(AbilityEnvelopeRejection::UnconsumedSource)?;
    let costs = parse_activation_costs(cost_source)?;
    let (body, restriction) = split_activation_restriction(body_and_restriction)?;
    Ok((
        ParsedAbilityEnvelope::Activated(ActivatedEnvelope { costs, restriction }),
        body,
    ))
}

fn parse_trigger_predicate(source: &str) -> Result<TriggerPredicate, AbilityEnvelopeRejection> {
    let source = source.trim();
    let remainder = source
        .strip_prefix("When ")
        .or_else(|| source.strip_prefix("Whenever "))
        .or_else(|| source.strip_prefix("At "))
        .ok_or(AbilityEnvelopeRejection::UnsupportedTriggerPredicate)?;
    parse_trigger_predicate_remainder(remainder)
}

fn parse_trigger_predicate_remainder(
    source: &str,
) -> Result<TriggerPredicate, AbilityEnvelopeRejection> {
    if let Some(predicate) = parse_step_or_phase_trigger(source) {
        return Ok(predicate);
    }
    if let Some(subject) = source.strip_prefix("you control no ")
        && let Some(mut filter) = parse_object_filter(subject)
    {
        filter.controller = ControllerConstraint::You;
        return Ok(TriggerPredicate::ControllerControls {
            filter,
            comparison: CountComparison::Exactly(0),
        });
    }
    if let Some((subject, suffix)) = split_subject_before_suffix(
        source,
        &[
            " enters the battlefield tapped",
            " enters the battlefield",
            " enters tapped",
            " enters",
        ],
    ) && let Some(object) = parse_ability_object_binding(subject)
    {
        return Ok(TriggerPredicate::EntersBattlefield {
            object,
            tapped: suffix.ends_with(" tapped").then_some(true),
        });
    }
    if let Some((subject, suffix)) = split_subject_before_suffix(
        source,
        &[
            " leaves the battlefield for exile",
            " leaves the battlefield",
            " is put into exile from the battlefield",
            " is put into a graveyard from the battlefield",
        ],
    ) && let Some(object) = parse_ability_object_binding(subject)
    {
        let destination = if suffix.contains("exile") {
            Some(Zone::Exile)
        } else if suffix.contains("graveyard") {
            Some(Zone::Graveyard)
        } else {
            None
        };
        return Ok(TriggerPredicate::LeavesBattlefield {
            object,
            destination,
        });
    }
    if let Some(subject) = source.strip_suffix(" dies")
        && let Some(object) = parse_ability_object_binding(subject)
    {
        return Ok(TriggerPredicate::Dies { object });
    }
    if let Some(subject) = source.strip_suffix(" attacks alone")
        && let Some(object) = parse_ability_object_binding(subject)
    {
        return Ok(TriggerPredicate::Attacks {
            object,
            alone: true,
            recipient: AttackRecipientRequirement::Any,
        });
    }
    for (suffix, recipient) in [
        (
            " attacks a player or battle",
            AttackRecipientRequirement::PlayerOrBattle,
        ),
        (" attacks a player", AttackRecipientRequirement::Player),
        (" attacks an opponent", AttackRecipientRequirement::Opponent),
        (" attacks", AttackRecipientRequirement::Any),
    ] {
        if let Some(subject) = source.strip_suffix(suffix)
            && let Some(object) = parse_ability_object_binding(subject.trim_end())
        {
            return Ok(TriggerPredicate::Attacks {
                object,
                alone: false,
                recipient,
            });
        }
    }
    if let Some(subject) = source.strip_suffix(" deals damage")
        && let Some(object) = parse_ability_object_binding(subject)
    {
        return Ok(TriggerPredicate::DealsDamage { source: object });
    }
    if let Some(subject) = source.strip_suffix(" becomes blocked")
        && let Some(object) = parse_ability_object_binding(subject)
    {
        return Ok(TriggerPredicate::Blocks {
            object,
            became_blocked: true,
        });
    }
    if let Some(subject) = source.strip_suffix(" blocks")
        && let Some(object) = parse_ability_object_binding(subject)
    {
        return Ok(TriggerPredicate::Blocks {
            object,
            became_blocked: false,
        });
    }
    for (suffix, recipient) in [
        (
            " deals combat damage to a player or planeswalker",
            CombatDamageRecipient::PlayerOrPlaneswalker,
        ),
        (
            " deals combat damage to a player or battle",
            CombatDamageRecipient::PlayerOrBattle,
        ),
        (
            " deals combat damage to an opponent",
            CombatDamageRecipient::Opponent,
        ),
        (
            " deals combat damage to a player",
            CombatDamageRecipient::Player,
        ),
        (
            " deals combat damage to a planeswalker",
            CombatDamageRecipient::Planeswalker,
        ),
        (
            " deals combat damage to a battle",
            CombatDamageRecipient::Battle,
        ),
    ] {
        if let Some(subject) = source.strip_suffix(suffix)
            && let Some(object) = parse_ability_object_binding(subject)
        {
            return Ok(TriggerPredicate::DealsCombatDamage {
                source: object,
                recipient,
            });
        }
    }
    if let Some(predicate) = parse_cast_or_copy_trigger(source) {
        return Ok(predicate);
    }
    for (suffix, actor, cause) in [
        (
            " becomes the target of a spell or ability an opponent controls",
            Some(AbilityPlayerBinding::AnOpponent),
            TargetingCauseRequirement::SpellOrAbility,
        ),
        (
            " becomes the target of a spell or ability",
            None,
            TargetingCauseRequirement::SpellOrAbility,
        ),
        (
            " becomes the target of a spell",
            None,
            TargetingCauseRequirement::Spell,
        ),
        (
            " becomes the target of an ability",
            None,
            TargetingCauseRequirement::Ability,
        ),
    ] {
        if let Some(subject) = source.strip_suffix(suffix)
            && let Some(object) = parse_ability_object_binding(subject)
        {
            return Ok(TriggerPredicate::BecomesTarget {
                object,
                actor,
                cause,
            });
        }
    }
    for (suffix, tapped) in [
        (" becomes tapped", true),
        (" becomes untapped", false),
        (" is tapped", true),
        (" is untapped", false),
    ] {
        if let Some(subject) = source.strip_suffix(suffix)
            && let Some(object) = parse_ability_object_binding(subject)
        {
            return Ok(TriggerPredicate::TappedOrUntapped { object, tapped });
        }
    }
    if let Some(predicate) = parse_counter_change_trigger(source) {
        return Ok(predicate);
    }
    Err(AbilityEnvelopeRejection::UnsupportedTriggerPredicate)
}

fn parse_intervening_condition(
    source: &str,
) -> Result<InterveningCondition, AbilityEnvelopeRejection> {
    match source {
        "this permanent is tapped" | "this creature is tapped" => {
            return Ok(InterveningCondition::SourceIsTapped(true));
        }
        "this permanent is untapped" | "this creature is untapped" => {
            return Ok(InterveningCondition::SourceIsTapped(false));
        }
        "this creature is attacking" | "this permanent is attacking" => {
            return Ok(InterveningCondition::SourceIsAttacking(true));
        }
        "it's your turn" | "it is your turn" => {
            return Ok(InterveningCondition::YourTurn(true));
        }
        "it's not your turn" | "it is not your turn" => {
            return Ok(InterveningCondition::YourTurn(false));
        }
        _ => {}
    }
    if let Some(counter_source) = source
        .strip_prefix("this permanent has at least ")
        .or_else(|| source.strip_prefix("this creature has at least "))
        && let Some((amount, counter)) = parse_counted_counter(counter_source)
    {
        return Ok(InterveningCondition::SourceHasCounter {
            counter_name: counter,
            at_least: amount,
        });
    }
    if let Some(object_source) = source.strip_prefix("you control ")
        && let Some(object) = parse_ability_object_binding(object_source)
    {
        return Ok(InterveningCondition::YouControlObject(object));
    }
    for (prefix, player, comparison) in [
        (
            "your life total is exactly ",
            AbilityPlayerBinding::You,
            NumericComparison::Exactly,
        ),
        (
            "your life total is at least ",
            AbilityPlayerBinding::You,
            NumericComparison::AtLeast,
        ),
        (
            "your life total is ",
            AbilityPlayerBinding::You,
            NumericComparison::Exactly,
        ),
    ] {
        if let Some(amount) = source.strip_prefix(prefix).and_then(parse_unsigned_decimal) {
            return Ok(InterveningCondition::LifeComparison {
                player,
                comparison,
                amount,
            });
        }
    }
    for (prefix, player, comparison) in [
        (
            "you have exactly ",
            AbilityPlayerBinding::You,
            NumericComparison::Exactly,
        ),
        (
            "you have at least ",
            AbilityPlayerBinding::You,
            NumericComparison::AtLeast,
        ),
        (
            "you have no ",
            AbilityPlayerBinding::You,
            NumericComparison::Exactly,
        ),
    ] {
        if let Some(remainder) = source.strip_prefix(prefix)
            && let Some(amount) = if prefix.ends_with("no ") {
                remainder.strip_suffix("cards in hand").map(|_| 0)
            } else {
                remainder
                    .strip_suffix(" cards in hand")
                    .and_then(parse_unsigned_decimal)
            }
        {
            return Ok(InterveningCondition::CardsInHandComparison {
                player,
                comparison,
                amount,
            });
        }
    }
    Err(AbilityEnvelopeRejection::UnsupportedInterveningCondition)
}

fn parse_activation_costs(source: &str) -> Result<Vec<ActivationCost>, AbilityEnvelopeRejection> {
    let components = split_top_level_cost_components(source)?;
    let mut costs = Vec::with_capacity(components.len());
    for component in components {
        costs.push(parse_activation_cost_component(component)?);
    }
    (!costs.is_empty())
        .then_some(costs)
        .ok_or(AbilityEnvelopeRejection::UnsupportedActivationCost)
}

fn split_activation_restriction(
    source: &str,
) -> Result<(&str, ActivationRestriction), AbilityEnvelopeRejection> {
    let restrictions = [
        (
            " Activate only as a sorcery.",
            ActivationRestriction::SorceryTiming,
        ),
        (
            " Activate only during your turn.",
            ActivationRestriction::DuringYourTurn,
        ),
        (
            " Activate only during an opponent's turn.",
            ActivationRestriction::DuringAnOpponentsTurn,
        ),
        (
            " Activate only during combat.",
            ActivationRestriction::DuringCombat,
        ),
        (
            " Activate only before attackers are declared.",
            ActivationRestriction::BeforeAttackersAreDeclared,
        ),
        (
            " Activate only once each turn.",
            ActivationRestriction::OnlyOnceEachTurn,
        ),
        (" Activate only once.", ActivationRestriction::OnlyOnce),
        (
            " Activate only if this permanent wasn't cast this turn.",
            ActivationRestriction::SourceWasNotCastThisTurn,
        ),
        (
            " Activate only if this permanent entered the battlefield this turn.",
            ActivationRestriction::SourceEnteredThisTurn,
        ),
    ];
    for (suffix, restriction) in restrictions {
        if let Some(body) = source.strip_suffix(suffix) {
            if !body.ends_with('.') {
                return Err(AbilityEnvelopeRejection::UnconsumedSource);
            }
            return Ok((body, restriction));
        }
    }
    if source.contains(" Activate only ")
        || source.contains(" Activate this ability only ")
        || source.contains(" Use this ability only ")
    {
        return Err(AbilityEnvelopeRejection::UnsupportedTimingRestriction);
    }
    if !source.ends_with('.') {
        return Err(AbilityEnvelopeRejection::UnconsumedSource);
    }
    Ok((source, ActivationRestriction::AnyTime))
}

fn parse_step_or_phase_trigger(source: &str) -> Option<TriggerPredicate> {
    let (boundary, remainder) = source
        .strip_prefix("the beginning of ")
        .map(|value| (StepBoundary::Beginning, value))
        .or_else(|| {
            source
                .strip_prefix("the end of ")
                .map(|value| (StepBoundary::End, value))
        })?;
    let (turn_owner, boundary_name) = if let Some(value) = remainder.strip_prefix("your ") {
        (TurnOwner::Yours, value)
    } else if let Some(value) = remainder.strip_prefix("each opponent's ") {
        (TurnOwner::Opponents, value)
    } else if let Some(value) = remainder.strip_prefix("each player's ") {
        (TurnOwner::EachPlayers, value)
    } else if let Some(value) = remainder.strip_prefix("each ") {
        (TurnOwner::Any, value)
    } else {
        (TurnOwner::Any, remainder)
    };
    let (phase, step) = match boundary_name {
        "beginning phase" => (Some(TurnPhase::Beginning), None),
        "precombat main phase" => (Some(TurnPhase::PrecombatMain), None),
        "combat phase" => (Some(TurnPhase::Combat), None),
        "combat" if boundary == StepBoundary::Beginning => {
            (None, Some(TurnStep::BeginningOfCombat))
        }
        "combat" if boundary == StepBoundary::End => (None, Some(TurnStep::EndOfCombat)),
        "postcombat main phase" => (Some(TurnPhase::PostcombatMain), None),
        "ending phase" => (Some(TurnPhase::Ending), None),
        "untap step" => (None, Some(TurnStep::Untap)),
        "upkeep" | "upkeep step" => (None, Some(TurnStep::Upkeep)),
        "draw step" => (None, Some(TurnStep::Draw)),
        "beginning of combat" | "beginning of combat step" => {
            (None, Some(TurnStep::BeginningOfCombat))
        }
        "declare attackers step" => (None, Some(TurnStep::DeclareAttackers)),
        "declare blockers step" => (None, Some(TurnStep::DeclareBlockers)),
        "combat damage step" => (None, Some(TurnStep::CombatDamage)),
        "end of combat" | "end of combat step" => (None, Some(TurnStep::EndOfCombat)),
        "end step" => (None, Some(TurnStep::End)),
        "cleanup step" => (None, Some(TurnStep::Cleanup)),
        _ => return None,
    };
    Some(TriggerPredicate::StepOrPhase {
        boundary,
        phase,
        step,
        turn_owner,
    })
}

fn parse_cast_or_copy_trigger(source: &str) -> Option<TriggerPredicate> {
    let candidates = [
        (
            "you cast or copy ",
            AbilityPlayerBinding::You,
            SpellEventMode::CastOrCopy,
        ),
        (
            "an opponent casts or copies ",
            AbilityPlayerBinding::AnOpponent,
            SpellEventMode::CastOrCopy,
        ),
        (
            "a player casts or copies ",
            AbilityPlayerBinding::AnyPlayer,
            SpellEventMode::CastOrCopy,
        ),
        ("you cast ", AbilityPlayerBinding::You, SpellEventMode::Cast),
        (
            "an opponent casts ",
            AbilityPlayerBinding::AnOpponent,
            SpellEventMode::Cast,
        ),
        (
            "a player casts ",
            AbilityPlayerBinding::AnyPlayer,
            SpellEventMode::Cast,
        ),
        ("you copy ", AbilityPlayerBinding::You, SpellEventMode::Copy),
        (
            "an opponent copies ",
            AbilityPlayerBinding::AnOpponent,
            SpellEventMode::Copy,
        ),
        (
            "a player copies ",
            AbilityPlayerBinding::AnyPlayer,
            SpellEventMode::Copy,
        ),
    ];
    for (prefix, player, mode) in candidates {
        let Some(spell_source) = source.strip_prefix(prefix) else {
            continue;
        };
        let (spell_source, from_zone) =
            if let Some(value) = spell_source.strip_suffix(" from your graveyard") {
                (value, Some(Zone::Graveyard))
            } else if let Some(value) = spell_source.strip_suffix(" from exile") {
                (value, Some(Zone::Exile))
            } else {
                (spell_source, None)
            };
        let spell = match spell_source {
            "a spell" | "one or more spells" => AbilityObjectBinding::ASpell,
            "this spell" => AbilityObjectBinding::ThisSpell,
            "a copy of a spell" => AbilityObjectBinding::CopiedSpell,
            _ => return None,
        };
        return Some(TriggerPredicate::Cast {
            player,
            spell,
            from_zone,
            mode,
        });
    }
    None
}

fn parse_counter_change_trigger(source: &str) -> Option<TriggerPredicate> {
    let patterns = [
        (" is put on ", CounterChange::Put, false),
        (" are put on ", CounterChange::Put, true),
        (" is removed from ", CounterChange::Removed, false),
        (" are removed from ", CounterChange::Removed, true),
    ];
    for (separator, operation, plural) in patterns {
        let Some(index) = source.find(separator) else {
            continue;
        };
        let (counter_source, object_with_separator) = source.split_at(index);
        let object_source = object_with_separator.strip_prefix(separator)?;
        let object = parse_ability_object_binding(object_source)?;
        let (one_or_more, counter_name) =
            if let Some(value) = counter_source.strip_prefix("one or more ") {
                (true, parse_counter_name(value)?)
            } else if let Some(value) = counter_source.strip_prefix("a ") {
                (false, parse_counter_name(value)?)
            } else if let Some(value) = counter_source.strip_prefix("an ") {
                (false, parse_counter_name(value)?)
            } else {
                (plural, parse_counter_name(counter_source)?)
            };
        return Some(TriggerPredicate::CounterChanged {
            object,
            operation,
            counter_name,
            one_or_more,
        });
    }
    None
}

fn parse_ability_object_binding(source: &str) -> Option<AbilityObjectBinding> {
    match source.trim() {
        "this permanent" | "it" => Some(AbilityObjectBinding::ThisPermanent),
        "this creature" => Some(AbilityObjectBinding::ThisCreature),
        "this artifact" => Some(AbilityObjectBinding::ThisArtifact),
        "this enchantment" => Some(AbilityObjectBinding::ThisEnchantment),
        "this land" => Some(AbilityObjectBinding::ThisLand),
        "this planeswalker" => Some(AbilityObjectBinding::ThisPlaneswalker),
        "this battle" => Some(AbilityObjectBinding::ThisBattle),
        "another permanent you control" => Some(AbilityObjectBinding::AnotherPermanentYouControl),
        "another creature you control" => Some(AbilityObjectBinding::AnotherCreatureYouControl),
        "a creature you control" => Some(AbilityObjectBinding::ACreatureYouControl),
        "one or more creatures you control" => {
            Some(AbilityObjectBinding::OneOrMoreCreaturesYouControl)
        }
        "a creature" | "one or more creatures" => Some(AbilityObjectBinding::AnyCreature),
        "a permanent" | "one or more permanents" => Some(AbilityObjectBinding::AnyPermanent),
        "a card" | "one or more cards" => Some(AbilityObjectBinding::ACard),
        "a spell" | "one or more spells" => Some(AbilityObjectBinding::ASpell),
        "this spell" => Some(AbilityObjectBinding::ThisSpell),
        "a copy of a spell" => Some(AbilityObjectBinding::CopiedSpell),
        "enchanted creature" => Some(AbilityObjectBinding::EnchantedObject),
        "equipped creature" => Some(AbilityObjectBinding::EquippedObject),
        _ => None,
    }
}

fn split_subject_before_suffix<'a>(
    source: &'a str,
    suffixes: &[&'a str],
) -> Option<(&'a str, &'a str)> {
    for suffix in suffixes {
        if let Some(subject) = source.strip_suffix(suffix) {
            return Some((subject, suffix));
        }
    }
    None
}

fn parse_counted_counter(source: &str) -> Option<(u32, String)> {
    let (amount_source, counter_source) = source.split_once(' ')?;
    let amount = parse_number_word_or_decimal(amount_source)?;
    let counter_name = parse_counter_name(counter_source)?;
    Some((amount, counter_name))
}

fn parse_counter_name(source: &str) -> Option<String> {
    let source = source
        .strip_suffix(" counters")
        .or_else(|| source.strip_suffix(" counter"))?;
    let name = source.trim();
    (!name.is_empty()
        && name.len() <= 80
        && !name.contains(',')
        && !name.contains('.')
        && !name.contains(':'))
    .then(|| name.to_owned())
}

fn parse_unsigned_decimal(source: &str) -> Option<u32> {
    (!source.is_empty() && source.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| source.parse().ok())
        .flatten()
}

fn parse_number_word_or_decimal(source: &str) -> Option<u32> {
    parse_unsigned_decimal(source).or_else(|| {
        Some(match source {
            "a" | "an" | "one" => 1,
            "two" => 2,
            "three" => 3,
            "four" => 4,
            "five" => 5,
            "six" => 6,
            "seven" => 7,
            "eight" => 8,
            "nine" => 9,
            "ten" => 10,
            "eleven" => 11,
            "twelve" => 12,
            _ => return None,
        })
    })
}

fn split_top_level_cost_components(source: &str) -> Result<Vec<&str>, AbilityEnvelopeRejection> {
    if source.trim() != source || source.is_empty() {
        return Err(AbilityEnvelopeRejection::UnsupportedActivationCost);
    }
    let mut components = Vec::new();
    let mut start = 0usize;
    let mut parenthesis_depth = 0u16;
    for (index, character) in source.char_indices() {
        match character {
            '(' => parenthesis_depth = parenthesis_depth.saturating_add(1),
            ')' => parenthesis_depth = parenthesis_depth.saturating_sub(1),
            ',' if parenthesis_depth == 0 => {
                let component = source[start..index].trim();
                if component.is_empty() {
                    return Err(AbilityEnvelopeRejection::UnsupportedActivationCost);
                }
                components.push(component);
                start = index + 1;
            }
            _ => {}
        }
    }
    let final_component = source[start..].trim();
    if final_component.is_empty() {
        return Err(AbilityEnvelopeRejection::UnsupportedActivationCost);
    }
    components.push(final_component);
    Ok(components)
}

fn parse_activation_cost_component(
    source: &str,
) -> Result<ActivationCost, AbilityEnvelopeRejection> {
    if source == "{T}" {
        return Ok(ActivationCost::TapSource);
    }
    if source == "{Q}" {
        return Ok(ActivationCost::UntapSource);
    }
    if source.starts_with('{') {
        return parse_mana_cost(source)
            .map(ActivationCost::Mana)
            .ok_or(AbilityEnvelopeRejection::UnsupportedActivationCost);
    }
    if let Some(object_source) = source.strip_prefix("Sacrifice ") {
        return parse_object_cost(object_source, false)
            .map(ActivationCost::Sacrifice)
            .ok_or(AbilityEnvelopeRejection::UnsupportedActivationCost);
    }
    if let Some(card_source) = source.strip_prefix("Discard ") {
        return parse_card_cost(card_source, Zone::Hand, false)
            .map(ActivationCost::Discard)
            .ok_or(AbilityEnvelopeRejection::UnsupportedActivationCost);
    }
    if let Some(card_source) = source.strip_prefix("Exile ") {
        let (selection, zone) =
            if let Some(value) = card_source.strip_suffix(" from your graveyard") {
                (value, Zone::Graveyard)
            } else if let Some(value) = card_source.strip_suffix(" from your hand") {
                (value, Zone::Hand)
            } else {
                return Err(AbilityEnvelopeRejection::UnsupportedActivationCost);
            };
        return parse_card_cost(selection, zone, false)
            .map(ActivationCost::Exile)
            .ok_or(AbilityEnvelopeRejection::UnsupportedActivationCost);
    }
    if let Some(life_source) = source
        .strip_prefix("Pay ")
        .and_then(|value| value.strip_suffix(" life"))
        && let Some(amount) = parse_number_word_or_decimal(life_source)
    {
        return Ok(ActivationCost::PayLife(amount));
    }
    if let Some(counter_source) = source.strip_prefix("Remove ")
        && let Some((counter_amount_source, object_source)) = counter_source.split_once(" from ")
        && let Some((amount, counter_name)) = parse_counter_cost_amount(counter_amount_source)
        && let Some(object) = parse_ability_object_binding(object_source)
    {
        return Ok(ActivationCost::RemoveCounters {
            object,
            counter_name,
            amount,
        });
    }
    if let Some(object_source) = source.strip_prefix("Tap ") {
        return parse_object_cost(object_source, true)
            .map(|objects| ActivationCost::TapObjects {
                amount: objects.amount,
                objects,
            })
            .ok_or(AbilityEnvelopeRejection::UnsupportedActivationCost);
    }
    Err(AbilityEnvelopeRejection::UnsupportedActivationCost)
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    let mut symbols = Vec::new();
    let mut remaining = source;
    while let Some(after_open) = remaining.strip_prefix('{') {
        let close = after_open.find('}')?;
        let symbol_source = &after_open[..close];
        if symbol_source.is_empty() {
            return None;
        }
        symbols.push(parse_mana_symbol(symbol_source)?);
        remaining = &after_open[close + 1..];
    }
    (!symbols.is_empty() && remaining.is_empty()).then_some(ManaCost { symbols })
}

fn parse_mana_symbol(source: &str) -> Option<ManaSymbol> {
    match source {
        "W" => Some(ManaSymbol::White),
        "U" => Some(ManaSymbol::Blue),
        "B" => Some(ManaSymbol::Black),
        "R" => Some(ManaSymbol::Red),
        "G" => Some(ManaSymbol::Green),
        "C" => Some(ManaSymbol::Colorless),
        "S" => Some(ManaSymbol::Snow),
        "X" => Some(ManaSymbol::X),
        value if value.bytes().all(|byte| byte.is_ascii_digit()) => {
            value.parse().ok().map(ManaSymbol::Generic)
        }
        value if value.ends_with("/P") => {
            let color = value.strip_suffix("/P")?;
            matches!(color, "W" | "U" | "B" | "R" | "G")
                .then(|| ManaSymbol::Phyrexian(color.to_owned()))
        }
        value if value.contains('/') => {
            let (first, second) = value.split_once('/')?;
            (!first.is_empty()
                && !second.is_empty()
                && !second.contains('/')
                && [first, second]
                    .iter()
                    .all(|part| matches!(*part, "W" | "U" | "B" | "R" | "G" | "2")))
            .then(|| ManaSymbol::Hybrid(first.to_owned(), second.to_owned()))
        }
        _ => None,
    }
}

fn parse_object_cost(source: &str, requires_untapped: bool) -> Option<ObjectCost> {
    let source = source.trim();
    let source_only = matches!(
        source,
        "this permanent"
            | "this creature"
            | "this artifact"
            | "this enchantment"
            | "this land"
            | "this planeswalker"
            | "this battle"
    );
    let mut other_than_source = false;
    let mut filter = CostObjectFilter::default();
    if source_only {
        filter.card_types.insert(match source {
            "this creature" => CostCardType::Creature,
            "this artifact" => CostCardType::Artifact,
            "this enchantment" => CostCardType::Enchantment,
            "this land" => CostCardType::Land,
            "this planeswalker" => CostCardType::Planeswalker,
            "this battle" => CostCardType::Battle,
            "this permanent" => CostCardType::Permanent,
            _ => return None,
        });
        return Some(ObjectCost {
            amount: CostAmount::Fixed(1),
            controller: CostController::You,
            filter,
            source_only: true,
            other_than_source: false,
        });
    }
    let (amount, mut remainder) = parse_cost_amount_prefix(source)?;
    if let Some(value) = remainder.strip_prefix("another ") {
        other_than_source = true;
        remainder = value;
    }
    if let Some(value) = remainder.strip_prefix("untapped ") {
        filter.tapped = Some(false);
        remainder = value;
    } else if let Some(value) = remainder.strip_prefix("tapped ") {
        filter.tapped = Some(true);
        remainder = value;
    } else if requires_untapped {
        return None;
    }
    if let Some(value) = remainder.strip_prefix("nontoken ") {
        filter.nontoken = Some(true);
        remainder = value;
    }
    let controller = if let Some(value) = remainder.strip_suffix(" you control") {
        remainder = value;
        CostController::You
    } else if !requires_untapped {
        CostController::You
    } else {
        return None;
    };
    let card_type = parse_cost_card_type(remainder)?;
    filter.card_types.insert(card_type);
    Some(ObjectCost {
        amount,
        controller,
        filter,
        source_only: false,
        other_than_source,
    })
}

fn parse_card_cost(source: &str, zone: Zone, default_random: bool) -> Option<CardCost> {
    let mut source = source.trim();
    let random = if let Some(value) = source.strip_suffix(" at random") {
        source = value;
        true
    } else {
        default_random
    };
    if source == "this card" {
        return Some(CardCost {
            amount: CostAmount::Fixed(1),
            zone,
            random,
            filter: CardCostFilter {
                other_than_source: false,
                ..CardCostFilter::default()
            },
            source_only: true,
        });
    }
    if source == "your hand" {
        return Some(CardCost {
            amount: CostAmount::All,
            zone,
            random,
            filter: CardCostFilter::default(),
            source_only: false,
        });
    }
    let (amount, mut remainder) = parse_cost_amount_prefix(source)?;
    let mut filter = CardCostFilter::default();
    if let Some(value) = remainder.strip_prefix("another ") {
        filter.other_than_source = true;
        remainder = value;
    }
    if let Some(type_source) = remainder.strip_suffix(" card") {
        filter.card_types.insert(parse_cost_card_type(type_source)?);
    } else if !matches!(remainder, "card" | "cards") {
        return None;
    }
    Some(CardCost {
        amount,
        zone,
        random,
        filter,
        source_only: false,
    })
}

fn parse_cost_amount_prefix(source: &str) -> Option<(CostAmount, &str)> {
    for (prefix, amount) in [
        ("any number of ", CostAmount::AnyNumber),
        ("one or more ", CostAmount::OneOrMore),
        ("X ", CostAmount::X),
        ("a ", CostAmount::Fixed(1)),
        ("an ", CostAmount::Fixed(1)),
        ("one ", CostAmount::Fixed(1)),
        ("two ", CostAmount::Fixed(2)),
        ("three ", CostAmount::Fixed(3)),
        ("four ", CostAmount::Fixed(4)),
        ("five ", CostAmount::Fixed(5)),
        ("six ", CostAmount::Fixed(6)),
    ] {
        if let Some(remainder) = source.strip_prefix(prefix) {
            return Some((amount, remainder));
        }
    }
    let digit_end = source
        .char_indices()
        .take_while(|(_, character)| character.is_ascii_digit())
        .map(|(index, character)| index + character.len_utf8())
        .last()?;
    let amount = parse_unsigned_decimal(&source[..digit_end])?;
    let remainder = source[digit_end..].strip_prefix(' ')?;
    Some((CostAmount::Fixed(amount), remainder))
}

fn parse_counter_cost_amount(source: &str) -> Option<(CostAmount, String)> {
    let (amount, counter_source) = if let Some(value) = source.strip_prefix("a ") {
        (CostAmount::Fixed(1), value)
    } else if let Some(value) = source.strip_prefix("an ") {
        (CostAmount::Fixed(1), value)
    } else if let Some(value) = source.strip_prefix("X ") {
        (CostAmount::X, value)
    } else {
        let (number, value) = source.split_once(' ')?;
        (
            CostAmount::Fixed(parse_number_word_or_decimal(number)?),
            value,
        )
    };
    Some((amount, parse_counter_name(counter_source)?))
}

fn parse_cost_card_type(source: &str) -> Option<CostCardType> {
    match source.trim_end_matches('s') {
        "artifact" => Some(CostCardType::Artifact),
        "battle" => Some(CostCardType::Battle),
        "creature" => Some(CostCardType::Creature),
        "enchantment" => Some(CostCardType::Enchantment),
        "land" => Some(CostCardType::Land),
        "planeswalker" => Some(CostCardType::Planeswalker),
        "permanent" => Some(CostCardType::Permanent),
        _ => None,
    }
}

fn starts_trigger_envelope(source: &str) -> bool {
    source.starts_with("When ") || source.starts_with("Whenever ") || source.starts_with("At ")
}

fn is_complete_single_line(source: &str) -> bool {
    !source.trim().is_empty()
        && source.trim() == source
        && !source.contains('\n')
        && !source.contains('\r')
        && !source.contains('\0')
}

fn strip_one_terminal_period(source: &str) -> Option<&str> {
    let source = source.strip_suffix('.')?;
    (!source.ends_with('.')).then_some(source)
}

fn contains_top_level_colon(source: &str) -> bool {
    find_top_level_delimiter(source, ':').is_some()
}

fn find_top_level_delimiter(source: &str, delimiter: char) -> Option<usize> {
    let mut parenthesis_depth = 0u16;
    for (index, character) in source.char_indices() {
        match character {
            '(' => parenthesis_depth = parenthesis_depth.saturating_add(1),
            ')' => parenthesis_depth = parenthesis_depth.saturating_sub(1),
            value if value == delimiter && parenthesis_depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

fn top_level_delimiter_indices(source: &str, delimiter: char) -> Vec<usize> {
    let mut indices = Vec::new();
    let mut parenthesis_depth = 0u16;
    for (index, character) in source.char_indices() {
        match character {
            '(' => parenthesis_depth = parenthesis_depth.saturating_add(1),
            ')' => parenthesis_depth = parenthesis_depth.saturating_sub(1),
            value if value == delimiter && parenthesis_depth == 0 => indices.push(index),
            _ => {}
        }
    }
    indices
}

fn find_top_level_substring(source: &str, needle: &str) -> Option<usize> {
    let mut parenthesis_depth = 0u16;
    for (index, character) in source.char_indices() {
        match character {
            '(' => parenthesis_depth = parenthesis_depth.saturating_add(1),
            ')' => parenthesis_depth = parenthesis_depth.saturating_sub(1),
            _ => {}
        }
        if parenthesis_depth == 0 && source[index..].starts_with(needle) {
            return Some(index);
        }
    }
    None
}

fn collapse_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn ability_shape_semantic_digest(
    exact_source: &str,
    normalized_source: &str,
    envelope: &ParsedAbilityEnvelope,
    exact_body: &str,
) -> String {
    let mut digest = Sha256::new();
    for component in [
        "oracle-ability-envelope-shape/v1",
        ORACLE_ABILITY_ENVELOPE_COMPILER_VERSION,
        ORACLE_ABILITY_ENVELOPE_RUNTIME_VERSION,
        ORACLE_ABILITY_ENVELOPE_RULES_CONTEXT_VERSION,
        exact_source,
        normalized_source,
        envelope.kind().stable_id(),
        &format!("{envelope:?}"),
        exact_body,
    ] {
        digest.update((component.len() as u64).to_le_bytes());
        digest.update(component.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn ability_program_semantic_digest(
    shape: &AbilityEnvelopeShape,
    body: &OracleActionProgram,
) -> String {
    let mut digest = Sha256::new();
    for component in [
        "oracle-ability-envelope-program/v1",
        ORACLE_ABILITY_ENVELOPE_COMPILER_VERSION,
        ORACLE_ABILITY_ENVELOPE_RUNTIME_VERSION,
        ORACLE_ABILITY_ENVELOPE_RULES_CONTEXT_VERSION,
        shape.exact_source(),
        shape.normalized_source(),
        shape.shape_digest(),
        body.semantic_digest(),
    ] {
        digest.update((component.len() as u64).to_le_bytes());
        digest.update(component.as_bytes());
    }
    format!("{:x}", digest.finalize())
}
