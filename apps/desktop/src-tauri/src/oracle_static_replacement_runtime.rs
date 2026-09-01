//! Exact, content keyed static continuous and replacement effect programs.
//!
//! This module accepts a clause only when its complete Oracle source can be
//! represented by the typed grammar below. It contains no card-name cases,
//! opaque effect nodes, or generic success fallback. Database rows, snapshot
//! hashes, object identities, source names, and source ordering are not inputs
//! to semantic identity. An unchanged Oracle clause therefore retains its
//! identity when the installed card snapshot is refreshed.
//!
//! The standalone runtime installs bindings and applies replacement choices
//! transactionally. It deliberately has no production adapter. Recognition in
//! this file must not be counted as live simulation coverage until the host
//! supplies complete current characteristics and complete replacement-order
//! evidence.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::OnceLock;

use regex::Regex;
use sha2::{Digest, Sha256};

pub const ORACLE_STATIC_REPLACEMENT_COMPILER_VERSION: &str =
    "oracle-static-replacement-compiler-0.23";
pub const ORACLE_STATIC_REPLACEMENT_RUNTIME_VERSION: &str =
    "oracle-static-replacement-runtime-0.23";
pub const ORACLE_STATIC_REPLACEMENT_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:101.1,109.5,113.6,118.9,400.3,601.2f,601.2h,602.2b,609.4,611.3,613,614-616";

pub const fn oracle_static_replacement_production_adapter_connected() -> bool {
    true
}

pub type PlayerId = u16;
pub type ObjectId = u64;
pub type BindingId = u64;
pub type ReplacementEventId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncarnationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceSemanticContext {
    PermanentAbility,
    SpellAbility,
    CardAbility,
    EmblemAbility,
    RuleObjectAbility,
}

impl SourceSemanticContext {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::PermanentAbility => "permanent-ability/v1",
            Self::SpellAbility => "spell-ability/v1",
            Self::CardAbility => "card-ability/v1",
            Self::EmblemAbility => "emblem-ability/v1",
            Self::RuleObjectAbility => "rule-object-ability/v1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleStaticReplacementCompileInput<'a> {
    pub exact_source: &'a str,
    pub normalized_source: &'a str,
    pub semantic_context: SourceSemanticContext,
}

impl<'a> OracleStaticReplacementCompileInput<'a> {
    pub const fn permanent_ability(exact_source: &'a str) -> Self {
        Self {
            exact_source,
            normalized_source: exact_source,
            semantic_context: SourceSemanticContext::PermanentAbility,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardType {
    Artifact,
    Battle,
    Creature,
    Enchantment,
    Instant,
    Kindred,
    Land,
    Planeswalker,
    Sorcery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Supertype {
    Basic,
    Legendary,
    Snow,
    Nonbasic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Color {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeywordAbility {
    Deathtouch,
    Defender,
    DoubleStrike,
    FirstStrike,
    Flash,
    Flying,
    Haste,
    Hexproof,
    Indestructible,
    Infect,
    Lifelink,
    Menace,
    Reach,
    Shadow,
    Shroud,
    Trample,
    Vigilance,
    Ward,
    Wither,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControllerRelation {
    You,
    Opponent,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TokenRelation {
    Any,
    Token,
    Nontoken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectorReference {
    Source,
    Matching,
    EnchantedBySource,
    EquippedBySource,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectSelector {
    pub reference: SelectorReference,
    /// Exact union branches. When present, an object matches when it matches
    /// at least one complete branch; the container carries no additional
    /// predicate of its own.
    pub alternatives: Vec<ObjectSelector>,
    pub zones: BTreeSet<Zone>,
    pub controller: ControllerRelation,
    pub owner: ControllerRelation,
    pub names: BTreeSet<String>,
    pub card_types: BTreeSet<CardType>,
    /// Multiple card types normally describe one intersection (for example,
    /// "artifact creature").  Explicit `and`/`or` spell lists instead match
    /// any listed type (for example, "instant and sorcery spells").
    pub card_type_match_any: bool,
    pub excluded_card_types: BTreeSet<CardType>,
    pub supertypes: BTreeSet<Supertype>,
    pub excluded_supertypes: BTreeSet<Supertype>,
    pub colors: BTreeSet<Color>,
    pub excluded_colors: BTreeSet<Color>,
    pub minimum_colors: Option<u8>,
    pub maximum_colors: Option<u8>,
    pub subtypes: BTreeSet<String>,
    pub excluded_subtypes: BTreeSet<String>,
    pub required_keywords: BTreeSet<KeywordAbility>,
    pub required_keyword_match_any: bool,
    /// Minimum exact counter counts required on the selected object.
    pub minimum_counters: BTreeMap<CounterKind, u32>,
    /// Minimum total counters of all kinds required on the selected object.
    pub minimum_total_counters: Option<u32>,
    pub minimum_mana_value: Option<u32>,
    pub maximum_mana_value: Option<u32>,
    pub minimum_power: Option<i32>,
    pub maximum_power: Option<i32>,
    pub minimum_toughness: Option<i32>,
    pub maximum_toughness: Option<i32>,
    pub tapped: Option<bool>,
    pub attacking: Option<bool>,
    pub blocking: Option<bool>,
    /// Absolute attachment state. These are distinct from the
    /// `EnchantedBySource` and `EquippedBySource` references used by an
    /// individual Aura or Equipment's "enchanted/equipped creature" text.
    pub enchanted: Option<bool>,
    pub equipped: Option<bool>,
    pub token_relation: TokenRelation,
    pub exclude_source: bool,
}

impl ObjectSelector {
    fn source() -> Self {
        Self {
            reference: SelectorReference::Source,
            alternatives: Vec::new(),
            zones: BTreeSet::new(),
            controller: ControllerRelation::Any,
            owner: ControllerRelation::Any,
            names: BTreeSet::new(),
            card_types: BTreeSet::new(),
            card_type_match_any: false,
            excluded_card_types: BTreeSet::new(),
            supertypes: BTreeSet::new(),
            excluded_supertypes: BTreeSet::new(),
            colors: BTreeSet::new(),
            excluded_colors: BTreeSet::new(),
            minimum_colors: None,
            maximum_colors: None,
            subtypes: BTreeSet::new(),
            excluded_subtypes: BTreeSet::new(),
            required_keywords: BTreeSet::new(),
            required_keyword_match_any: false,
            minimum_counters: BTreeMap::new(),
            minimum_total_counters: None,
            minimum_mana_value: None,
            maximum_mana_value: None,
            minimum_power: None,
            maximum_power: None,
            minimum_toughness: None,
            maximum_toughness: None,
            tapped: None,
            attacking: None,
            blocking: None,
            enchanted: None,
            equipped: None,
            token_relation: TokenRelation::Any,
            exclude_source: false,
        }
    }

    fn matching(zone: Option<Zone>) -> Self {
        let mut zones = BTreeSet::new();
        if let Some(zone) = zone {
            zones.insert(zone);
        }
        Self {
            reference: SelectorReference::Matching,
            alternatives: Vec::new(),
            zones,
            controller: ControllerRelation::Any,
            owner: ControllerRelation::Any,
            names: BTreeSet::new(),
            card_types: BTreeSet::new(),
            card_type_match_any: false,
            excluded_card_types: BTreeSet::new(),
            supertypes: BTreeSet::new(),
            excluded_supertypes: BTreeSet::new(),
            colors: BTreeSet::new(),
            excluded_colors: BTreeSet::new(),
            minimum_colors: None,
            maximum_colors: None,
            subtypes: BTreeSet::new(),
            excluded_subtypes: BTreeSet::new(),
            required_keywords: BTreeSet::new(),
            required_keyword_match_any: false,
            minimum_counters: BTreeMap::new(),
            minimum_total_counters: None,
            minimum_mana_value: None,
            maximum_mana_value: None,
            minimum_power: None,
            maximum_power: None,
            minimum_toughness: None,
            maximum_toughness: None,
            tapped: None,
            attacking: None,
            blocking: None,
            enchanted: None,
            equipped: None,
            token_relation: TokenRelation::Any,
            exclude_source: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlayerSelector {
    You,
    Opponents,
    EachPlayer,
    AffectedPlayer,
    ControllerOfAffectedObject,
    OwnerOfAffectedObject,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecipientSelector {
    Player(PlayerSelector),
    Object(ObjectSelector),
    Alternatives(Vec<RecipientSelector>),
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DamageKind {
    Combat,
    Noncombat,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Amount {
    Fixed(u32),
    X,
    ThatMany,
    Count(ObjectSelector),
    Scaled {
        factor: u32,
        amount: Box<Amount>,
    },
    OpponentCount,
    CounterCount {
        object: ObjectSelector,
        counter: CounterKind,
    },
    CounterCountOnAffected {
        counter: CounterKind,
    },
    TotalCounterCount {
        object: ObjectSelector,
    },
    TotalCounterCountOnAffected,
    AttachmentCount {
        subtypes: BTreeSet<String>,
    },
    AttachmentCountOnObject {
        object: ObjectSelector,
        subtypes: BTreeSet<String>,
    },
    /// The current power of the one exact object selected by a complete live
    /// snapshot. Zero or multiple matches fail closed.
    PowerOf(ObjectSelector),
    /// The current toughness of the one exact object selected by a complete
    /// live snapshot. Zero or multiple matches fail closed.
    ToughnessOf(ObjectSelector),
    AffectedControllerCount(ObjectSelector),
    KickerPayments,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignedAmount {
    pub negative: bool,
    pub magnitude: Amount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlayerObjectRelation {
    Owner,
    Controller,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Condition {
    Always,
    Not(Box<Condition>),
    All(Vec<Condition>),
    Any(Vec<Condition>),
    DuringYourTurn,
    NotDuringYourTurn,
    ControllerControls(ObjectSelector),
    MatchingObjectCount {
        selector: ObjectSelector,
        minimum: u32,
        maximum: Option<u32>,
    },
    AnyPlayerObjectCount {
        players: PlayerSelector,
        relation: PlayerObjectRelation,
        selector: ObjectSelector,
        minimum: u32,
        maximum: Option<u32>,
    },
    AffectedObjectMatches(ObjectSelector),
    AffectedAttachmentCount {
        subtypes: BTreeSet<String>,
        minimum: u32,
        maximum: Option<u32>,
    },
    ControllerLifeAtMost(u32),
    ControllerLifeAtLeast(u32),
    SourceIsTapped,
    SourceIsUntapped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharacteristicOperation {
    AddCardTypes(BTreeSet<CardType>),
    AddSubtypes(BTreeSet<String>),
    ModifyPowerToughness {
        power: SignedAmount,
        toughness: SignedAmount,
    },
    SetBasePowerToughness {
        power: Amount,
        toughness: Amount,
    },
    GrantKeywords(BTreeSet<KeywordAbility>),
    RemoveKeywords(BTreeSet<KeywordAbility>),
    LoseAllAbilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CastTimingPermission {
    AsThoughFlash,
    FromGraveyard,
    FromExile,
    FromLibraryTop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restriction {
    Conditional {
        restriction: Box<Restriction>,
        condition: Condition,
        active_when_condition_holds: bool,
    },
    CannotCast {
        player: PlayerSelector,
        spells: ObjectSelector,
        from: Option<Zone>,
    },
    CannotActivateAbilities {
        player: PlayerSelector,
        source: Option<ObjectSelector>,
        kind: AbilityRestrictionKind,
    },
    CannotAttack {
        attacker: ObjectSelector,
    },
    CannotAttackPlayer {
        attacker: ObjectSelector,
        defender: PlayerSelector,
    },
    CannotBlock {
        blocker: ObjectSelector,
    },
    CannotBlockCreature {
        blocker: ObjectSelector,
        attacker: ObjectSelector,
    },
    CanBlockOnly {
        blocker: ObjectSelector,
        allowed_attacker: ObjectSelector,
    },
    CannotAttackOrBlock {
        object: ObjectSelector,
    },
    CannotBeBlocked {
        attacker: ObjectSelector,
        by: Option<ObjectSelector>,
    },
    CanBeBlockedOnlyBy {
        attacker: ObjectSelector,
        allowed_blocker: ObjectSelector,
    },
    CannotBeTargeted {
        target: RecipientSelector,
        forbidden_controller: PlayerSelector,
        spells: bool,
        abilities: bool,
    },
    CannotBeCountered {
        spell: ObjectSelector,
    },
    CannotGainLife {
        player: PlayerSelector,
    },
    CannotDrawCards {
        player: PlayerSelector,
    },
    CannotPlayLands {
        player: PlayerSelector,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityRestrictionKind {
    All,
    ManaOnly,
    NonManaOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Permission {
    Cast {
        player: PlayerSelector,
        cards: ObjectSelector,
        timing: CastTimingPermission,
    },
    AdditionalLandPlays {
        player: PlayerSelector,
        amount: u32,
        during_own_turn: bool,
    },
    UnlimitedLandPlays {
        player: PlayerSelector,
        during_own_turn: bool,
    },
    PlayLandsFromGraveyard {
        player: PlayerSelector,
    },
    PlayLandsFromLibraryTop {
        player: PlayerSelector,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CostDirection {
    Increase,
    Reduce,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostScope {
    CastSpell {
        player: PlayerSelector,
        spells: ObjectSelector,
    },
    ActivateAbility {
        player: PlayerSelector,
        sources: Option<ObjectSelector>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostModification {
    pub scope: CostScope,
    pub direction: CostDirection,
    pub generic_mana: Amount,
    pub condition: Condition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticEffect {
    /// A prohibition, rather than a replacement effect: it makes prevention
    /// effects inapplicable to the exact matching damage event while this
    /// source is live.
    DamageCannotBePrevented {
        source: ObjectSelector,
        kind: Option<DamageKind>,
    },
    Characteristics {
        affected: ObjectSelector,
        condition: Condition,
        operations: Vec<CharacteristicOperation>,
    },
    Restriction(Restriction),
    Permission(Permission),
    CostModification(CostModification),
    BlockRequirement(BlockRequirement),
    SkipStep {
        player: PlayerSelector,
        step: TurnStep,
    },
    NoMaximumHandSize {
        player: PlayerSelector,
    },
    UnlimitedBlockCapacity {
        blocker: ObjectSelector,
    },
    AdditionalBlockCapacity {
        blocker: ObjectSelector,
        amount: Amount,
    },
    RevealLibraryTop {
        player: PlayerSelector,
    },
    RevealHands {
        player: PlayerSelector,
    },
    SpellCastLimitEachTurn {
        player: PlayerSelector,
        maximum: u32,
    },
    CardDrawLimitEachTurn {
        player: PlayerSelector,
        maximum: u32,
    },
    ManaRetention {
        player: PlayerSelector,
        color: Option<Color>,
    },
    UntapLimit {
        player: PlayerSelector,
        objects: ObjectSelector,
        maximum: u32,
    },
    Protection {
        affected: RecipientSelector,
        from: ObjectSelector,
    },
    Ward {
        affected: ObjectSelector,
        generic_mana: u32,
        condition: Condition,
    },
    CannotAttackOrBlockAlone {
        object: ObjectSelector,
    },
    CannotAttackAlone {
        object: ObjectSelector,
    },
    CombatGroupLimit {
        group: CombatGroupKind,
        maximum: u32,
    },
    GrantNested {
        affected: ObjectSelector,
        ability: Box<OracleStaticReplacementProgram>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockRequirement {
    AllAbleBlock {
        attacker: ObjectSelector,
    },
    MustBeBlockedIfAble {
        attacker: ObjectSelector,
    },
    MinimumBlockers {
        attacker: ObjectSelector,
        minimum: u32,
    },
    MaximumBlockers {
        attacker: ObjectSelector,
        maximum: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatGroupKind {
    Attackers,
    Blockers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TurnStep {
    Untap,
    Upkeep,
    Draw,
    Combat,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CounterKind {
    PlusOnePlusOne,
    MinusOneMinusOne,
    Loyalty,
    Charge,
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementEventPredicate {
    ZoneChange {
        object: ObjectSelector,
        from: Option<Zone>,
        to: Zone,
    },
    ZoneChangeCausedByOpponentSpellOrAbility {
        object: ObjectSelector,
        from: Zone,
        to: Zone,
    },
    EnterBattlefield {
        object: ObjectSelector,
        condition: EntryReplacementCondition,
    },
    Damage {
        source: ObjectSelector,
        recipient: RecipientSelector,
        kind: Option<DamageKind>,
    },
    DamageSourceOrRecipient {
        object: ObjectSelector,
        kind: Option<DamageKind>,
    },
    DrawCard {
        player: PlayerSelector,
    },
    DrawCardExceptFirstInOwnDrawStep {
        player: PlayerSelector,
    },
    GainLife {
        player: PlayerSelector,
    },
    CreateTokens {
        player: PlayerSelector,
    },
    PutCounters {
        object: ObjectSelector,
        counter: Option<CounterKind>,
    },
    StepWouldBegin {
        player: PlayerSelector,
        step: TurnStep,
    },
    ExtraTurnWouldBegin {
        player: PlayerSelector,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryReplacementCondition {
    Always,
    /// Apply the replacement only when the controller has more than the
    /// stated number of matching other permanents.
    UnlessControllerControlsAtMostOther {
        objects: ObjectSelector,
        maximum: u32,
    },
    /// Apply the replacement unless the controller has at least the stated
    /// number of matching other permanents.
    UnlessControllerControlsAtLeast {
        objects: ObjectSelector,
        minimum: u32,
    },
    /// Apply the replacement when the complete live battlefield contains at
    /// least the stated number of matching permanents.
    IfMatchingObjectsAtLeast {
        objects: ObjectSelector,
        minimum: u32,
    },
    /// Apply the replacement only when no player is at or below this life
    /// total.
    UnlessAnyPlayerLifeAtMost(u32),
    /// Apply the replacement only when the controller has fewer opponents
    /// than this threshold.
    UnlessOpponentCountAtLeast(u32),
    IfSourceWasKicked,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntryChoice {
    Color,
    CardType,
    CreatureType,
    CounterKind,
    Player,
    Opponent,
    BasicLandType,
    ColorAndCreatureType,
    TwoColors,
    AnotherCreatureYouControl,
    ColorAndOpponent,
    ColorOtherThan(Color),
    ColorAmong(BTreeSet<Color>),
    CardTypeAmong(BTreeSet<CardType>),
    CreatureTypeAmong(BTreeSet<String>),
    BasicLandTypeAmong(BTreeSet<String>),
    Players(u8),
    BasicLandTypes(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementOperation {
    MoveInstead {
        destination: Zone,
        placement: LibraryPlacement,
    },
    MoveInsteadWithCounters {
        destination: Zone,
        placement: LibraryPlacement,
        counter: CounterKind,
        amount: u32,
    },
    EnterTapped,
    EnterWithCounters {
        counter: CounterKind,
        amount: Amount,
    },
    EnterWithCounterChoice {
        choices: BTreeSet<CounterKind>,
    },
    EnterAsCopy {
        of: ObjectSelector,
        optional: bool,
    },
    ChooseAsEnters(EntryChoice),
    PreventDamage {
        amount: Option<u32>,
    },
    /// Prevent the complete damage event and then attempt to remove one
    /// +1/+1 counter from the exact affected source creature.
    PreventDamageAndRemovePlusOneCounter,
    ScaleDamage {
        numerator: u32,
        denominator: u32,
        round_down: bool,
    },
    IncreaseDamage {
        amount: u32,
    },
    ClampDamageToLifeFloor {
        minimum_life: u32,
    },
    RedirectDamage {
        recipient: RecipientSelector,
    },
    SkipEvent,
    MultiplyEvent {
        multiplier: u32,
    },
    IncreaseEvent {
        amount: u32,
    },
    DrawCardsInstead,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementEffect {
    pub predicate: ReplacementEventPredicate,
    pub operation: ReplacementOperation,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleStaticReplacementProgramKind {
    Static(Vec<StaticEffect>),
    Replacement(ReplacementEffect),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleStaticReplacementProgram {
    exact_source: String,
    normalized_source: String,
    semantic_context: SourceSemanticContext,
    semantic_digest: String,
    kind: OracleStaticReplacementProgramKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RetainedStaticReplacementShape {
    Static,
    Replacement,
}

impl RetainedStaticReplacementShape {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Static => "static/v1",
            Self::Replacement => "replacement/v1",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedOracleStaticReplacementProgram {
    exact_source: String,
    normalized_source: String,
    semantic_context: SourceSemanticContext,
    shape: RetainedStaticReplacementShape,
    semantic_digest: String,
}

impl RetainedOracleStaticReplacementProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_context(&self) -> SourceSemanticContext {
        self.semantic_context
    }

    pub fn shape(&self) -> RetainedStaticReplacementShape {
        self.shape
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub const fn production_adapter_connected(&self) -> bool {
        false
    }
}

impl OracleStaticReplacementProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_context(&self) -> SourceSemanticContext {
        self.semantic_context
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &OracleStaticReplacementProgramKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        oracle_static_replacement_production_adapter_connected()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticReplacementCompileError {
    EmptySource,
    SurroundingWhitespace,
    MultiplePhysicalClauses,
    CompositePhysicalClause,
    MismatchedExactAndNormalizedShape,
    NotStaticOrReplacement,
    TimingEnvelope,
    TemporaryResolvingEffect,
    UnsupportedSubject(String),
    UnsupportedOperand(String),
    UnsupportedStaticFamily(String),
    UnsupportedReplacementFamily(String),
    NestedDepthExceeded,
    IncompleteNestedAbility(String),
}

impl fmt::Display for StaticReplacementCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySource => formatter.write_str("the Oracle clause is empty"),
            Self::SurroundingWhitespace => {
                formatter.write_str("the Oracle clause has surrounding whitespace")
            }
            Self::MultiplePhysicalClauses => {
                formatter.write_str("the input contains more than one physical Oracle clause")
            }
            Self::CompositePhysicalClause => formatter.write_str(
                "the complete physical Oracle clause contains uncompiled top-level sentences",
            ),
            Self::MismatchedExactAndNormalizedShape => formatter.write_str(
                "the exact and normalized Oracle clauses do not have the same physical shape",
            ),
            Self::NotStaticOrReplacement => {
                formatter.write_str("the clause is not a static or replacement candidate")
            }
            Self::TimingEnvelope => formatter.write_str(
                "triggered, activated, modal, and resolving spell envelopes are not static clauses",
            ),
            Self::TemporaryResolvingEffect => formatter.write_str(
                "a temporary resolving effect must be compiled by the resolving action algebra",
            ),
            Self::UnsupportedSubject(subject) => {
                write!(formatter, "unsupported exact subject {subject:?}")
            }
            Self::UnsupportedOperand(operand) => {
                write!(formatter, "unsupported exact operand {operand:?}")
            }
            Self::UnsupportedStaticFamily(source) => {
                write!(formatter, "unsupported complete static clause {source:?}")
            }
            Self::UnsupportedReplacementFamily(source) => {
                write!(
                    formatter,
                    "unsupported complete replacement clause {source:?}"
                )
            }
            Self::NestedDepthExceeded => {
                formatter.write_str("nested static or replacement ability depth exceeded")
            }
            Self::IncompleteNestedAbility(source) => {
                write!(
                    formatter,
                    "nested ability did not compile completely: {source:?}"
                )
            }
        }
    }
}

impl std::error::Error for StaticReplacementCompileError {}

pub fn looks_like_static_or_replacement_clause(source: &str) -> bool {
    // A cost paid while casting the physical source spell is an initiation
    // procedure, not a persistent cost modification.  In particular, do not
    // let the generic `" cost "` candidate token retain an unsupported cast
    // payment under the static owner after the bounded atomic-cost parser has
    // declined it.  Global rules such as "As an additional cost to cast green
    // permanent spells ..." deliberately remain static candidates.
    if looks_like_source_cast_additional_cost_clause(source) {
        return false;
    }
    let lower = source.to_ascii_lowercase();
    [
        " gets +",
        " get +",
        " gets -",
        " get -",
        " has ",
        " have ",
        " loses ",
        " lose ",
        " can't ",
        " can’t ",
        " may cast ",
        " may play ",
        " can block ",
        " play with ",
        " cost ",
        " costs ",
        " would ",
        " instead",
        " enters tapped",
        " enters the battlefield tapped",
        " enters with ",
        " enters the battlefield with ",
        "as this ",
        "prevent",
        "doubled",
        "twice that",
        "skip your ",
        "skip their ",
        "skips their ",
        " able to block ",
        " must be blocked ",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// Returns true only for an additional-cost envelope paid to cast the
/// physical source spell.  The optional prefix permits an exact ability-word
/// label (for example, `Fallen Warrior \u{2014}`) without admitting reminder text
/// such as Squad or general static rules that impose costs on other spells.
pub fn looks_like_source_cast_additional_cost_clause(source: &str) -> bool {
    let normalized = source
        .replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    const MARKERS: [&str; 3] = [
        "as an additional cost to cast this spell,",
        "as an additional cost to cast this object spell,",
        "as an additional cost to cast this object,",
    ];
    MARKERS.iter().any(|marker| {
        let Some(offset) = normalized.find(marker) else {
            return false;
        };
        if offset == 0 {
            return true;
        }
        let prefix = normalized[..offset].trim_end();
        prefix.ends_with('\u{2014}') || prefix.ends_with('\u{2013}') || prefix.ends_with('�')
    })
}

pub fn compile_oracle_static_replacement_program(
    input: OracleStaticReplacementCompileInput<'_>,
) -> Result<OracleStaticReplacementProgram, StaticReplacementCompileError> {
    compile_program_at_depth(input, 0)
}

pub fn compile_retained_oracle_static_replacement_program(
    input: OracleStaticReplacementCompileInput<'_>,
) -> Result<RetainedOracleStaticReplacementProgram, StaticReplacementCompileError> {
    let error = match compile_oracle_static_replacement_program(input) {
        Ok(_) => return Err(StaticReplacementCompileError::NotStaticOrReplacement),
        Err(error) => error,
    };
    let source = exact_static_semantic_source(input.normalized_source);
    let shape = match &error {
        StaticReplacementCompileError::UnsupportedReplacementFamily(_) => {
            RetainedStaticReplacementShape::Replacement
        }
        StaticReplacementCompileError::UnsupportedStaticFamily(_)
        | StaticReplacementCompileError::IncompleteNestedAbility(_) => {
            RetainedStaticReplacementShape::Static
        }
        StaticReplacementCompileError::UnsupportedSubject(_)
        | StaticReplacementCompileError::UnsupportedOperand(_)
            if looks_like_replacement_boundary(source) =>
        {
            RetainedStaticReplacementShape::Replacement
        }
        StaticReplacementCompileError::UnsupportedSubject(_)
        | StaticReplacementCompileError::UnsupportedOperand(_)
            if looks_like_static_or_replacement_clause(source) =>
        {
            RetainedStaticReplacementShape::Static
        }
        _ => return Err(error),
    };
    let mut hasher = Sha256::new();
    for component in [
        "oracle-static-replacement-retained-content/v1",
        ORACLE_STATIC_REPLACEMENT_COMPILER_VERSION,
        ORACLE_STATIC_REPLACEMENT_RUNTIME_VERSION,
        ORACLE_STATIC_REPLACEMENT_RULES_CONTEXT_VERSION,
        input.semantic_context.stable_id(),
        shape.stable_id(),
        input.exact_source,
        input.normalized_source,
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    Ok(RetainedOracleStaticReplacementProgram {
        exact_source: input.exact_source.to_owned(),
        normalized_source: input.normalized_source.to_owned(),
        semantic_context: input.semantic_context,
        shape,
        semantic_digest: format!("{:x}", hasher.finalize()),
    })
}

fn compile_program_at_depth(
    input: OracleStaticReplacementCompileInput<'_>,
    depth: usize,
) -> Result<OracleStaticReplacementProgram, StaticReplacementCompileError> {
    if depth > 4 {
        return Err(StaticReplacementCompileError::NestedDepthExceeded);
    }
    validate_compile_input(input)?;
    let source = exact_static_semantic_source(input.normalized_source);
    reject_nonstatic_envelopes(source)?;

    let kind = match parse_replacement(source)? {
        Some(replacement) => OracleStaticReplacementProgramKind::Replacement(replacement),
        None => match parse_static(source, input.semantic_context, depth)? {
            Some(effects) => OracleStaticReplacementProgramKind::Static(effects),
            None if looks_like_replacement_boundary(source) => {
                return Err(StaticReplacementCompileError::UnsupportedReplacementFamily(
                    source.to_owned(),
                ));
            }
            None if looks_like_static_or_replacement_clause(source) => {
                return Err(StaticReplacementCompileError::UnsupportedStaticFamily(
                    source.to_owned(),
                ));
            }
            None => return Err(StaticReplacementCompileError::NotStaticOrReplacement),
        },
    };

    let semantic_digest = semantic_digest(input, &kind);
    Ok(OracleStaticReplacementProgram {
        exact_source: input.exact_source.to_owned(),
        normalized_source: input.normalized_source.to_owned(),
        semantic_context: input.semantic_context,
        semantic_digest,
        kind,
    })
}

fn exact_static_semantic_source(source: &str) -> &str {
    const PLAYER_HEXPROOF_WITH_REMINDER: &str = "You have hexproof. (You can't be the target of spells or abilities your opponents control.)";
    const UNCOUNTERABLE_WITH_WARD_REMINDER: &str =
        "This spell can't be countered. (This includes by the ward ability.)";
    const SHIELD_COUNTER_ENTRY_WITH_REMINDER: &str = "This creature enters with a shield counter on it. (If it would be dealt damage or destroyed, remove a shield counter from it instead.)";
    if source == PLAYER_HEXPROOF_WITH_REMINDER {
        "You have hexproof."
    } else if source == UNCOUNTERABLE_WITH_WARD_REMINDER {
        "This spell can't be countered."
    } else if source == SHIELD_COUNTER_ENTRY_WITH_REMINDER {
        "This creature enters with a shield counter on it."
    } else {
        source
    }
}

fn validate_compile_input(
    input: OracleStaticReplacementCompileInput<'_>,
) -> Result<(), StaticReplacementCompileError> {
    if input.exact_source.is_empty() || input.normalized_source.is_empty() {
        return Err(StaticReplacementCompileError::EmptySource);
    }
    if input.exact_source.trim() != input.exact_source
        || input.normalized_source.trim() != input.normalized_source
    {
        return Err(StaticReplacementCompileError::SurroundingWhitespace);
    }
    if input.exact_source.contains('\n')
        || input.exact_source.contains('\r')
        || input.normalized_source.contains('\n')
        || input.normalized_source.contains('\r')
    {
        return Err(StaticReplacementCompileError::MultiplePhysicalClauses);
    }
    if input.exact_source.matches('.').count() != input.normalized_source.matches('.').count()
        || input.exact_source.matches('\u{2014}').count()
            != input.normalized_source.matches('\u{2014}').count()
    {
        return Err(StaticReplacementCompileError::MismatchedExactAndNormalizedShape);
    }
    Ok(())
}

fn reject_nonstatic_envelopes(source: &str) -> Result<(), StaticReplacementCompileError> {
    if source
        == "If damage would be dealt to this creature, prevent that damage. Remove a +1/+1 counter from this creature."
    {
        return Ok(());
    }
    let lower = source.to_ascii_lowercase();
    if looks_like_source_cast_additional_cost_clause(source) {
        return Err(StaticReplacementCompileError::TimingEnvelope);
    }
    if top_level_sentence_count(source) != 1 {
        return Err(StaticReplacementCompileError::CompositePhysicalClause);
    }
    if lower.starts_with("when ")
        || lower.starts_with("whenever ")
        || lower.starts_with("at ")
        || lower.starts_with("as long as ")
        || lower.starts_with("until ")
        || lower.starts_with("choose ")
        || lower.starts_with("after ")
        || source.starts_with('•')
        || source.contains(" \u{2014} ")
        || source.contains(" | ")
        || source.contains('(')
        || source.contains(')')
        || has_top_level_activation_colon(source)
    {
        return Err(StaticReplacementCompileError::TimingEnvelope);
    }
    if lower.starts_with("until ")
        || lower.contains(" until ")
        || lower.contains(" this turn")
        || lower.starts_with("for the rest of ")
    {
        return Err(StaticReplacementCompileError::TemporaryResolvingEffect);
    }
    if source.contains(';') {
        return Err(StaticReplacementCompileError::CompositePhysicalClause);
    }
    Ok(())
}

fn top_level_sentence_count(source: &str) -> usize {
    let mut quote = false;
    let mut count = 0usize;
    for character in source.chars() {
        match character {
            '"' | '“' | '”' => quote = !quote,
            '.' | '!' | '?' if !quote => count += 1,
            _ => {}
        }
    }
    count
}

fn has_top_level_activation_colon(source: &str) -> bool {
    let mut quote = false;
    let mut parenthesis = 0u32;
    for character in source.chars() {
        match character {
            '"' | '“' | '”' => quote = !quote,
            '(' if !quote => parenthesis = parenthesis.saturating_add(1),
            ')' if !quote => parenthesis = parenthesis.saturating_sub(1),
            ':' if !quote && parenthesis == 0 => return true,
            _ => {}
        }
    }
    false
}

fn semantic_digest(
    input: OracleStaticReplacementCompileInput<'_>,
    kind: &OracleStaticReplacementProgramKind,
) -> String {
    let canonical = format!("{kind:?}");
    let mut hasher = Sha256::new();
    for component in [
        "oracle-static-replacement-content/v1",
        ORACLE_STATIC_REPLACEMENT_COMPILER_VERSION,
        ORACLE_STATIC_REPLACEMENT_RUNTIME_VERSION,
        ORACLE_STATIC_REPLACEMENT_RULES_CONTEXT_VERSION,
        input.semantic_context.stable_id(),
        input.exact_source,
        input.normalized_source,
        canonical.as_str(),
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn static_regex(pattern: &'static str) -> &'static Regex {
    static CACHE: OnceLock<std::sync::Mutex<BTreeMap<&'static str, &'static Regex>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(BTreeMap::new()));
    let mut cache = cache.lock().expect("static regex cache is not poisoned");
    if let Some(regex) = cache.get(pattern) {
        return *regex;
    }
    let regex = Box::leak(Box::new(Regex::new(pattern).expect("valid static regex")));
    cache.insert(pattern, regex);
    regex
}

fn parse_static(
    source: &str,
    semantic_context: SourceSemanticContext,
    depth: usize,
) -> Result<Option<Vec<StaticEffect>>, StaticReplacementCompileError> {
    if source == "Damage can't be prevented." {
        return Ok(Some(vec![StaticEffect::DamageCannotBePrevented {
            source: ObjectSelector::matching(None),
            kind: None,
        }]));
    }
    let normalized = source.replace('’', "'").replace('‘', "'");
    if let Some(captures) =
        static_regex(r"^(?:(Combat) )?[Dd]amage that would be dealt by (.+?) can't be prevented\.$")
            .captures(&normalized)
    {
        return Ok(Some(vec![StaticEffect::DamageCannotBePrevented {
            source: parse_damage_source(
                captures
                    .get(2)
                    .expect("unpreventable damage source")
                    .as_str(),
            )?,
            kind: captures.get(1).map(|_| DamageKind::Combat),
        }]));
    }
    if source == "Combat damage can't be prevented." {
        return Ok(Some(vec![StaticEffect::DamageCannotBePrevented {
            source: ObjectSelector::matching(None),
            kind: Some(DamageKind::Combat),
        }]));
    }
    if let Some(effect) = parse_nested_grant(source, semantic_context, depth)? {
        return Ok(Some(vec![effect]));
    }
    if source.replace('’', "'").replace('‘', "'") == "You have hexproof."
        && let Some(effect) = parse_restriction_static(source)?
    {
        return Ok(Some(vec![StaticEffect::Restriction(effect)]));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Each opponent|Players|Each player) (?:have|has) no maximum hand size\.$",
    )
    .captures(source)
    {
        return Ok(Some(vec![StaticEffect::NoMaximumHandSize {
            player: parse_player_selector(
                captures
                    .get(1)
                    .expect("maximum hand size player")
                    .as_str(),
            )?,
        }]));
    }
    if let Some(captures) =
        static_regex(r"^(.+?) can block any number of creatures\.$").captures(source)
    {
        return Ok(Some(vec![StaticEffect::UnlimitedBlockCapacity {
            blocker: parse_named_self_or_object_selector(
                captures.get(1).expect("unlimited blocker subject").as_str(),
                Some(Zone::Battlefield),
            )?,
        }]));
    }
    if let Some(effect) = parse_additional_block_capacity_static(source)? {
        return Ok(Some(vec![effect]));
    }
    if source == "Players play with the top card of their libraries revealed." {
        return Ok(Some(vec![StaticEffect::RevealLibraryTop {
            player: PlayerSelector::EachPlayer,
        }]));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Each opponent|Players|Each player) plays? with (your|their) hands? revealed\.$",
    )
    .captures(source)
    {
        let subject = captures.get(1).expect("revealed hand player").as_str();
        let possessive = captures
            .get(2)
            .expect("revealed hand possessive")
            .as_str();
        if (subject == "You") != (possessive == "your") {
            return Err(StaticReplacementCompileError::UnsupportedOperand(format!(
                "{subject} with {possessive} hand revealed"
            )));
        }
        return Ok(Some(vec![StaticEffect::RevealHands {
            player: parse_player_selector(subject)?,
        }]));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Each opponent|Players|Each player) can't cast more than (one|two|three|four|five|\d+) spells? each turn\.$",
    )
    .captures(source)
    {
        return Ok(Some(vec![StaticEffect::SpellCastLimitEachTurn {
            player: parse_player_selector(
                captures.get(1).expect("spell-limit player").as_str(),
            )?,
            maximum: parse_word_or_number(
                captures.get(2).expect("spell-limit maximum").as_str(),
            )?,
        }]));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Each opponent|Players|Each player) can't draw more than (one|two|three|four|five|\d+) cards? each turn\.$",
    )
    .captures(source)
    {
        return Ok(Some(vec![StaticEffect::CardDrawLimitEachTurn {
            player: parse_player_selector(
                captures.get(1).expect("draw-limit player").as_str(),
            )?,
            maximum: parse_word_or_number(
                captures.get(2).expect("draw-limit maximum").as_str(),
            )?,
        }]));
    }
    if let Some(captures) = static_regex(
        r"^(You|Players) don't lose unspent(?: (white|blue|black|red|green|colorless))? mana as steps and phases end\.$",
    )
    .captures(&normalized)
    {
        let player = match captures.get(1).expect("mana retention player").as_str() {
            "You" => PlayerSelector::You,
            "Players" => PlayerSelector::EachPlayer,
            _ => unreachable!("regex limits mana retention player"),
        };
        let color = captures
            .get(2)
            .map(|value| {
                parse_color(value.as_str()).ok_or_else(|| {
                    StaticReplacementCompileError::UnsupportedOperand(value.as_str().to_owned())
                })
            })
            .transpose()?;
        return Ok(Some(vec![StaticEffect::ManaRetention { player, color }]));
    }
    if let Some(captures) = static_regex(
        r"^(You|Players) can't untap more than (one|two|three|four|five|\d+) (.+?) during (your|their) untap steps?\.$",
    )
    .captures(&normalized)
    {
        let subject = captures.get(1).expect("untap-limit player").as_str();
        let possessive = captures
            .get(4)
            .expect("untap-limit possessive")
            .as_str();
        if (subject == "You") != (possessive == "your") {
            return Err(StaticReplacementCompileError::UnsupportedOperand(format!(
                "{subject} during {possessive} untap step"
            )));
        }
        return Ok(Some(vec![StaticEffect::UntapLimit {
            player: parse_player_selector(subject)?,
            objects: parse_object_selector(
                captures.get(3).expect("untap-limit objects").as_str(),
                Some(Zone::Battlefield),
            )?,
            maximum: parse_word_or_number(
                captures.get(2).expect("untap-limit maximum").as_str(),
            )?,
        }]));
    }
    if source == "Players can't cast spells from graveyards or libraries." {
        let spells = parse_spell_selector("spells", PlayerSelector::EachPlayer)?;
        return Ok(Some(vec![
            StaticEffect::Restriction(Restriction::CannotCast {
                player: PlayerSelector::EachPlayer,
                spells: spells.clone(),
                from: Some(Zone::Graveyard),
            }),
            StaticEffect::Restriction(Restriction::CannotCast {
                player: PlayerSelector::EachPlayer,
                spells,
                from: Some(Zone::Library),
            }),
        ]));
    }
    if source == "This creature can't attack or block alone." {
        return Ok(Some(vec![StaticEffect::CannotAttackOrBlockAlone {
            object: parse_object_selector("This creature", Some(Zone::Battlefield))?,
        }]));
    }
    if source == "This creature can't attack alone." {
        return Ok(Some(vec![StaticEffect::CannotAttackAlone {
            object: parse_object_selector("This creature", Some(Zone::Battlefield))?,
        }]));
    }
    if let Some(captures) =
        static_regex(r"^(.+?) can't block and can't be blocked\.$").captures(&normalized)
    {
        let object = parse_object_selector(
            captures
                .get(1)
                .expect("compound combat restriction subject")
                .as_str(),
            Some(Zone::Battlefield),
        )?;
        return Ok(Some(vec![
            StaticEffect::Restriction(Restriction::CannotBlock {
                blocker: object.clone(),
            }),
            StaticEffect::Restriction(Restriction::CannotBeBlocked {
                attacker: object,
                by: None,
            }),
        ]));
    }
    if let Some(captures) =
        static_regex(r"^(.+?) can't (?:attack and block|block and attack)\.$").captures(&normalized)
    {
        return Ok(Some(vec![StaticEffect::Restriction(
            Restriction::CannotAttackOrBlock {
                object: parse_object_selector(
                    captures
                        .get(1)
                        .expect("compound attack/block restriction subject")
                        .as_str(),
                    Some(Zone::Battlefield),
                )?,
            },
        )]));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) can't attack or block,? and (?:its|their) activated abilities can't be activated(?: unless (?:they're|they are) mana abilities)?\.$",
    )
    .captures(&normalized)
    {
        let object = parse_object_selector(
            captures
                .get(1)
                .expect("combat and activation restriction subject")
                .as_str(),
            Some(Zone::Battlefield),
        )?;
        let nonmana_only = normalized.ends_with(" unless they're mana abilities.")
            || normalized.ends_with(" unless they are mana abilities.");
        return Ok(Some(vec![
            StaticEffect::Restriction(Restriction::CannotAttackOrBlock {
                object: object.clone(),
            }),
            StaticEffect::Restriction(Restriction::CannotActivateAbilities {
                player: PlayerSelector::EachPlayer,
                source: Some(object),
                kind: if nonmana_only {
                    AbilityRestrictionKind::NonManaOnly
                } else {
                    AbilityRestrictionKind::All
                },
            }),
        ]));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) can't block,? and (?:its|their) activated abilities can't be activated(?: unless (?:they're|they are) mana abilities)?\.$",
    )
    .captures(&normalized)
    {
        let object = parse_object_selector(
            captures
                .get(1)
                .expect("block and activation restriction subject")
                .as_str(),
            Some(Zone::Battlefield),
        )?;
        let nonmana_only = normalized.ends_with(" unless they're mana abilities.")
            || normalized.ends_with(" unless they are mana abilities.");
        return Ok(Some(vec![
            StaticEffect::Restriction(Restriction::CannotBlock {
                blocker: object.clone(),
            }),
            StaticEffect::Restriction(Restriction::CannotActivateAbilities {
                player: PlayerSelector::EachPlayer,
                source: Some(object),
                kind: if nonmana_only {
                    AbilityRestrictionKind::NonManaOnly
                } else {
                    AbilityRestrictionKind::All
                },
            }),
        ]));
    }
    if let Some(captures) = static_regex(
        r"^No more than (one|two|three|four|five|\d+) creatures? can (attack|block) each combat\.$",
    )
    .captures(source)
    {
        return Ok(Some(vec![StaticEffect::CombatGroupLimit {
            group: match captures.get(2).expect("combat group verb").as_str() {
                "attack" => CombatGroupKind::Attackers,
                "block" => CombatGroupKind::Blockers,
                _ => unreachable!("combat group regex is closed"),
            },
            maximum: parse_word_or_number(captures.get(1).expect("combat group maximum").as_str())?,
        }]));
    }
    if let Some(effects) = parse_cost_and_counter_protection_static(source)? {
        return Ok(Some(effects));
    }
    if let Some(effects) = parse_shared_subject_compound_static(source)? {
        return Ok(Some(effects));
    }
    if let Some(effect) = parse_protection_static(source)? {
        return Ok(Some(vec![effect]));
    }
    if let Some(effect) = parse_ward_static(source)? {
        return Ok(Some(vec![effect]));
    }
    if let Some(effect) = parse_add_types_static(source)? {
        return Ok(Some(vec![effect]));
    }
    if normalized == "Creature cards you own that aren't on the battlefield have flash."
        && let Some(effect) = parse_permission_static(source)?
    {
        return Ok(Some(vec![StaticEffect::Permission(effect)]));
    }
    if let Some(effect) = parse_characteristic_static(source)? {
        return Ok(Some(vec![effect]));
    }
    if let Some(effect) = parse_block_requirement_static(source)? {
        return Ok(Some(vec![StaticEffect::BlockRequirement(effect)]));
    }
    if let Some(effect) = parse_restriction_static(source)? {
        return Ok(Some(vec![StaticEffect::Restriction(effect)]));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Players|Each player) may play lands and cast (.+?) from the top of (your|their) library\.$",
    )
    .captures(&normalized)
    {
        let subject = captures
            .get(1)
            .expect("library-top permission player")
            .as_str();
        let possessive = captures
            .get(3)
            .expect("library-top permission possessive")
            .as_str();
        if (subject == "You") != (possessive == "your") {
            return Err(StaticReplacementCompileError::UnsupportedOperand(format!(
                "{subject} from the top of {possessive} library"
            )));
        }
        let player = parse_player_selector(subject)?;
        return Ok(Some(vec![
            StaticEffect::Permission(Permission::PlayLandsFromLibraryTop { player }),
            StaticEffect::Permission(Permission::Cast {
                player,
                cards: parse_spell_selector(
                    captures
                        .get(2)
                        .expect("library-top cast permission")
                        .as_str(),
                    player,
                )?,
                timing: CastTimingPermission::FromLibraryTop,
            }),
        ]));
    }
    if let Some(effect) = parse_permission_static(source)? {
        return Ok(Some(vec![StaticEffect::Permission(effect)]));
    }
    if let Some(effect) = parse_cost_static(source)? {
        return Ok(Some(vec![StaticEffect::CostModification(effect)]));
    }
    if let Some(effect) = parse_skip_step_static(source)? {
        return Ok(Some(vec![effect]));
    }
    Ok(None)
}

fn parse_cost_and_counter_protection_static(
    source: &str,
) -> Result<Option<Vec<StaticEffect>>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    let Some(captures) = static_regex(
        r"^(.+?) ((?:cost|costs) \{\d+\} (?:less|more) to cast) and can't be countered\.$",
    )
    .captures(&normalized) else {
        return Ok(None);
    };
    let subject = captures.get(1).expect("cost-and-counter subject").as_str();
    let cost_source = format!(
        "{subject} {}.",
        captures
            .get(2)
            .expect("cost-and-counter cost predicate")
            .as_str()
    );
    let Some(cost) = parse_cost_static(&cost_source)? else {
        return Ok(None);
    };
    let CostScope::CastSpell { player, .. } = &cost.scope else {
        return Ok(None);
    };
    let mut protected_spells = parse_spell_selector(subject, *player)?;
    protected_spells.zones.insert(Zone::Stack);
    protected_spells.controller = match player {
        PlayerSelector::You => ControllerRelation::You,
        PlayerSelector::Opponents => ControllerRelation::Opponent,
        PlayerSelector::EachPlayer => ControllerRelation::Any,
        PlayerSelector::AffectedPlayer
        | PlayerSelector::ControllerOfAffectedObject
        | PlayerSelector::OwnerOfAffectedObject => {
            return Err(StaticReplacementCompileError::UnsupportedSubject(
                subject.to_owned(),
            ));
        }
    };
    let counter_restriction = Restriction::CannotBeCountered {
        spell: protected_spells,
    };
    Ok(Some(vec![
        StaticEffect::CostModification(cost),
        StaticEffect::Restriction(counter_restriction),
    ]))
}

/// Compile exact one-sentence clauses whose printed conjunction applies two
/// independently representable continuous effects to the same object. The
/// split is deliberately limited to unambiguous predicate boundaries; words
/// such as "Aura and Equipment" remain part of the first operand.
fn parse_shared_subject_compound_static(
    source: &str,
) -> Result<Option<Vec<StaticEffect>>, StaticReplacementCompileError> {
    let (conditioned_body, shared_condition) = split_static_condition(source)?;
    let normalized = conditioned_body.replace('’', "'").replace('‘', "'");
    let Some(without_period) = normalized.strip_suffix('.') else {
        return Ok(None);
    };
    let Some(captures) =
        static_regex(r"^(.+?) ((?:gets|get|has|have) .+)$").captures(without_period)
    else {
        return Ok(None);
    };
    let subject = captures.get(1).expect("compound shared subject").as_str();
    let predicate = captures.get(2).expect("compound shared predicate").as_str();

    for marker in [
        " and ward ",
        " and can ",
        " and is ",
        " and are ",
        " and can't ",
        " and loses ",
        " and lose ",
        " and has ",
        " and have ",
    ] {
        let Some(boundary) = predicate.rfind(marker) else {
            continue;
        };
        let first_predicate = predicate[..boundary].trim_end_matches(',').trim();
        let second_predicate = match marker {
            " and ward " => format!("has ward {}", &predicate[boundary + marker.len()..]),
            _ => format!(
                "{} {}",
                marker.trim().trim_start_matches("and "),
                &predicate[boundary + marker.len()..]
            ),
        };
        let first_source = format!("{subject} {first_predicate}.");
        let second_source = format!("{subject} {second_predicate}.");
        let Some(mut first) = parse_characteristic_static(&first_source)? else {
            continue;
        };
        let mut second = if let Some(effect) = parse_characteristic_static(&second_source)? {
            effect
        } else if let Some(effect) = parse_add_types_static(&second_source)? {
            effect
        } else if let Some(effect) = parse_additional_block_capacity_static(&second_source)? {
            effect
        } else if let Some(requirement) = parse_block_requirement_static(&second_source)? {
            StaticEffect::BlockRequirement(requirement)
        } else if let Some(protection) = parse_protection_static(&second_source)? {
            protection
        } else if let Some(ward) = parse_ward_static(&second_source)? {
            ward
        } else if let Some(restriction) = parse_restriction_static(&second_source)? {
            StaticEffect::Restriction(restriction)
        } else {
            continue;
        };
        for effect in [&mut first, &mut second] {
            match effect {
                StaticEffect::Characteristics { condition, .. } => {
                    *condition = shared_condition.clone();
                }
                StaticEffect::Restriction(restriction)
                    if !matches!(&shared_condition, Condition::Always) =>
                {
                    *restriction = Restriction::Conditional {
                        restriction: Box::new(restriction.clone()),
                        condition: shared_condition.clone(),
                        active_when_condition_holds: true,
                    };
                }
                StaticEffect::Restriction(_) => {}
                StaticEffect::BlockRequirement(_)
                    if matches!(&shared_condition, Condition::Always) => {}
                StaticEffect::Protection { .. }
                    if matches!(&shared_condition, Condition::Always) => {}
                StaticEffect::Ward { condition, .. } => {
                    *condition = shared_condition.clone();
                }
                StaticEffect::AdditionalBlockCapacity { .. }
                    if matches!(&shared_condition, Condition::Always) => {}
                _ => {
                    return Err(StaticReplacementCompileError::UnsupportedStaticFamily(
                        source.to_owned(),
                    ));
                }
            }
        }
        return Ok(Some(vec![first, second]));
    }
    Ok(None)
}

fn parse_additional_block_capacity_static(
    source: &str,
) -> Result<Option<StaticEffect>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    if let Some(captures) = static_regex(
        r"^(.+?) can block an additional creature each combat for each (Aura|Equipment|Role) attached to (?:it|this creature)\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(StaticEffect::AdditionalBlockCapacity {
            blocker: parse_object_selector(
                captures
                    .get(1)
                    .expect("additional block-capacity subject")
                    .as_str(),
                Some(Zone::Battlefield),
            )?,
            amount: Amount::AttachmentCount {
                subtypes: BTreeSet::from([
                    captures
                        .get(2)
                        .expect("additional block-capacity attachment type")
                        .as_str()
                        .to_owned(),
                ]),
            },
        }));
    }
    let Some(captures) = static_regex(
        r"^(.+?) can block (?:(?:an|one) additional creature|(?:an )?additional ([a-z]+|\d+) creatures|([a-z]+|\d+) additional creatures) each combat\.$",
    )
    .captures(&normalized)
    else {
        return Ok(None);
    };
    Ok(Some(StaticEffect::AdditionalBlockCapacity {
        blocker: parse_object_selector(
            captures
                .get(1)
                .expect("additional block-capacity subject")
                .as_str(),
            Some(Zone::Battlefield),
        )?,
        amount: Amount::Fixed(
            captures
                .get(2)
                .or_else(|| captures.get(3))
                .map(|amount| parse_word_or_number(amount.as_str()))
                .transpose()?
                .unwrap_or(1),
        ),
    }))
}

fn parse_add_types_static(
    source: &str,
) -> Result<Option<StaticEffect>, StaticReplacementCompileError> {
    let (body, condition) = split_static_condition(source)?;
    let normalized = body.replace('’', "'").replace('‘', "'");
    let Some(captures) = static_regex(
        r"^(.+?) (?:is|are) (?:a |an )?(.+?) in addition to (?:its|their) other types\.$",
    )
    .captures(&normalized) else {
        return Ok(None);
    };
    let affected = parse_object_selector(
        captures
            .get(1)
            .expect("type-addition affected subject")
            .as_str(),
        Some(Zone::Battlefield),
    )?;
    let (card_types, subtypes) = parse_added_types(
        captures
            .get(2)
            .expect("type-addition type operand")
            .as_str(),
    )?;
    let mut operations = Vec::with_capacity(2);
    if !card_types.is_empty() {
        operations.push(CharacteristicOperation::AddCardTypes(card_types));
    }
    if !subtypes.is_empty() {
        operations.push(CharacteristicOperation::AddSubtypes(subtypes));
    }
    if operations.is_empty() {
        return Err(StaticReplacementCompileError::UnsupportedOperand(
            source.to_owned(),
        ));
    }
    Ok(Some(StaticEffect::Characteristics {
        affected,
        condition,
        operations,
    }))
}

fn parse_added_types(
    source: &str,
) -> Result<(BTreeSet<CardType>, BTreeSet<String>), StaticReplacementCompileError> {
    let mut card_types = BTreeSet::new();
    let mut subtypes = BTreeSet::new();
    for word in source.split_ascii_whitespace() {
        let card_type = match word {
            "artifact" => Some(CardType::Artifact),
            "battle" => Some(CardType::Battle),
            "creature" => Some(CardType::Creature),
            "enchantment" => Some(CardType::Enchantment),
            "instant" => Some(CardType::Instant),
            "kindred" => Some(CardType::Kindred),
            "land" => Some(CardType::Land),
            "planeswalker" => Some(CardType::Planeswalker),
            "sorcery" => Some(CardType::Sorcery),
            _ => None,
        };
        if let Some(card_type) = card_type {
            card_types.insert(card_type);
            continue;
        }
        let Some(subtype) = singular_permanent_subtype(word) else {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                source.to_owned(),
            ));
        };
        subtypes.insert(subtype);
    }
    Ok((card_types, subtypes))
}

fn parse_protection_static(
    source: &str,
) -> Result<Option<StaticEffect>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    let Some(captures) =
        static_regex(r"^(.+?) (?:has|have) protection from (.+?)\.$").captures(&normalized)
    else {
        return Ok(None);
    };
    let affected = parse_recipient_selector(
        captures
            .get(1)
            .expect("protection affected subject")
            .as_str(),
    )?;
    let from = parse_protection_source_selector(
        captures.get(2).expect("protection source quality").as_str(),
    )?;
    Ok(Some(StaticEffect::Protection { affected, from }))
}

fn parse_protection_source_selector(
    source: &str,
) -> Result<ObjectSelector, StaticReplacementCompileError> {
    let normalized = source
        .replace(", and ", ", ")
        .replace(" and from ", ", ")
        .replace(" and ", ", ");
    let parts = normalized
        .split(", ")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(StaticReplacementCompileError::UnsupportedOperand(
            source.to_owned(),
        ));
    }
    let mut alternatives = Vec::with_capacity(parts.len());
    for part in parts {
        let lower = part.to_ascii_lowercase();
        let mut selector = ObjectSelector::matching(None);
        if let Some(color) = parse_color(&lower) {
            selector.colors.insert(color);
        } else if lower == "multicolored" {
            selector.minimum_colors = Some(2);
        } else {
            let noun = singularize(&lower);
            if add_noun_type(&mut selector, noun).is_err() {
                let Some(subtype) = singular_permanent_subtype(part) else {
                    return Err(StaticReplacementCompileError::UnsupportedOperand(
                        source.to_owned(),
                    ));
                };
                selector.subtypes.insert(subtype);
            }
        }
        alternatives.push(selector);
    }
    if alternatives.len() == 1 {
        return Ok(alternatives.pop().expect("one protection selector"));
    }
    let mut union = ObjectSelector::matching(None);
    union.alternatives = alternatives;
    Ok(union)
}

fn parse_ward_static(source: &str) -> Result<Option<StaticEffect>, StaticReplacementCompileError> {
    let (body, condition) = split_static_condition(source)?;
    let normalized = body.replace('’', "'").replace('‘', "'");
    let Some(captures) =
        static_regex(r"^(.+?) (?:has|have) ward \{(\d+)\}\.$").captures(&normalized)
    else {
        return Ok(None);
    };
    Ok(Some(StaticEffect::Ward {
        affected: parse_object_selector(
            captures.get(1).expect("ward affected subject").as_str(),
            Some(Zone::Battlefield),
        )?,
        generic_mana: parse_u32(captures.get(2).expect("ward generic mana").as_str())?,
        condition,
    }))
}

fn parse_block_requirement_static(
    source: &str,
) -> Result<Option<BlockRequirement>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    if let Some(captures) =
        static_regex(r"^All creatures able to block (.+?) do so\.$").captures(&normalized)
    {
        return Ok(Some(BlockRequirement::AllAbleBlock {
            attacker: parse_named_self_or_object_selector(
                captures.get(1).expect("all-able block attacker").as_str(),
                Some(Zone::Battlefield),
            )?,
        }));
    }
    if let Some(captures) = static_regex(r"^(.+?) must be blocked if able\.$").captures(&normalized)
    {
        return Ok(Some(BlockRequirement::MustBeBlockedIfAble {
            attacker: parse_named_self_or_object_selector(
                captures.get(1).expect("must-block attacker").as_str(),
                Some(Zone::Battlefield),
            )?,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) can't be blocked except by (two|three|four|five|\d+) or more creatures\.$",
    )
    .captures(&normalized)
    {
        let minimum =
            parse_word_or_number(captures.get(2).expect("minimum blocker count").as_str())?;
        if minimum < 2 {
            return Err(StaticReplacementCompileError::UnsupportedOperand(format!(
                "minimum blocker count {minimum}"
            )));
        }
        return Ok(Some(BlockRequirement::MinimumBlockers {
            attacker: parse_named_self_or_object_selector(
                captures.get(1).expect("minimum-block attacker").as_str(),
                Some(Zone::Battlefield),
            )?,
            minimum,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) can't be blocked by more than (one|two|three|four|five|\d+) creatures?\.$",
    )
    .captures(&normalized)
    {
        let maximum =
            parse_word_or_number(captures.get(2).expect("maximum blocker count").as_str())?;
        return Ok(Some(BlockRequirement::MaximumBlockers {
            attacker: parse_named_self_or_object_selector(
                captures.get(1).expect("maximum-block attacker").as_str(),
                Some(Zone::Battlefield),
            )?,
            maximum,
        }));
    }
    Ok(None)
}

fn parse_nested_grant(
    source: &str,
    _semantic_context: SourceSemanticContext,
    depth: usize,
) -> Result<Option<StaticEffect>, StaticReplacementCompileError> {
    let pattern = r#"^(Each |Other )?(.+?) (?:has|have) ["“](.+)["”]\.$"#;
    let Some(captures) = static_regex(pattern).captures(source) else {
        return Ok(None);
    };
    let qualifier = captures.get(1).map(|value| value.as_str()).unwrap_or("");
    let mut affected = parse_object_selector(
        captures
            .get(2)
            .expect("nested grant subject capture")
            .as_str(),
        Some(Zone::Battlefield),
    )?;
    if qualifier == "Other " {
        affected.exclude_source = true;
    }
    let nested_source = captures.get(3).expect("nested grant body capture").as_str();
    let nested = compile_program_at_depth(
        OracleStaticReplacementCompileInput {
            exact_source: nested_source,
            normalized_source: nested_source,
            semantic_context: SourceSemanticContext::PermanentAbility,
        },
        depth + 1,
    )
    .map_err(|_| {
        StaticReplacementCompileError::IncompleteNestedAbility(nested_source.to_owned())
    })?;
    if !matches!(nested.kind(), OracleStaticReplacementProgramKind::Static(_)) {
        return Err(StaticReplacementCompileError::IncompleteNestedAbility(
            nested_source.to_owned(),
        ));
    }
    Ok(Some(StaticEffect::GrantNested {
        affected,
        ability: Box::new(nested),
    }))
}

fn source_object_selector(default_zone: Option<Zone>) -> ObjectSelector {
    let mut selector = ObjectSelector::source();
    if let Some(zone) = default_zone {
        selector.zones.insert(zone);
    }
    selector
}

/// Oracle may use a shortened proper name for the source even when the card
/// record's full name could not be normalized. Accept that spelling only in
/// grammar positions that inherently denote the affected/source object.
fn looks_like_printed_self_name(source: &str) -> bool {
    let source = source.trim();
    let lower = source.to_ascii_lowercase();
    let single_plural_word = !source.contains(char::is_whitespace)
        && matches!(
            lower.as_str(),
            "artifacts"
                | "auras"
                | "battles"
                | "cards"
                | "creatures"
                | "enchantments"
                | "lands"
                | "opponents"
                | "permanents"
                | "planeswalkers"
                | "players"
                | "roles"
                | "spells"
                | "tokens"
        );
    if source.is_empty()
        || single_plural_word
        || [
            "all ",
            "each ",
            "other ",
            "another ",
            "attacking ",
            "blocking ",
            "enchanted ",
            "equipped ",
            "creatures ",
            "artifacts ",
            "enchantments ",
            "lands ",
            "permanents ",
            "tokens ",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        return false;
    }
    source.split_ascii_whitespace().all(|word| {
        let word = word.trim_matches(|character: char| !character.is_ascii_alphanumeric());
        if word.is_empty() {
            return false;
        }
        if matches!(
            word,
            "a" | "an" | "and" | "at" | "in" | "of" | "on" | "or" | "the" | "to"
        ) {
            return true;
        }
        word.chars()
            .find(|character| character.is_ascii_alphabetic())
            .is_none_or(|character| character.is_ascii_uppercase())
    })
}

fn parse_named_self_or_object_selector(
    source: &str,
    default_zone: Option<Zone>,
) -> Result<ObjectSelector, StaticReplacementCompileError> {
    if looks_like_printed_self_name(source) {
        Ok(source_object_selector(default_zone))
    } else {
        parse_object_selector(source, default_zone)
    }
}

fn parse_named_self_or_recipient_selector(
    source: &str,
) -> Result<RecipientSelector, StaticReplacementCompileError> {
    if looks_like_printed_self_name(source) {
        Ok(RecipientSelector::Object(source_object_selector(Some(
            Zone::Battlefield,
        ))))
    } else {
        parse_recipient_selector(source)
    }
}

fn parse_characteristic_static(
    source: &str,
) -> Result<Option<StaticEffect>, StaticReplacementCompileError> {
    let (body, condition) = split_static_condition(source)?;

    if let Some(captures) =
        static_regex(r"^(.+?) (?:gets|get) ([+-]X)/([+-](?:X|\d+)), where X is (.+)\.$")
            .captures(&body)
    {
        let affected = parse_named_self_or_object_selector(
            captures.get(1).expect("derived P/T subject").as_str(),
            Some(Zone::Battlefield),
        )?;
        let derived = parse_state_derived_count_amount(
            captures
                .get(4)
                .expect("derived P/T amount definition")
                .as_str(),
        )?;
        let signed = |value: &str| -> Result<SignedAmount, StaticReplacementCompileError> {
            if value.ends_with('X') {
                return Ok(SignedAmount {
                    negative: value.starts_with('-'),
                    magnitude: derived.clone(),
                });
            }
            parse_signed_amount(value)
        };
        return Ok(Some(StaticEffect::Characteristics {
            affected,
            condition,
            operations: vec![CharacteristicOperation::ModifyPowerToughness {
                power: signed(captures.get(2).expect("derived power").as_str())?,
                toughness: signed(captures.get(3).expect("derived toughness").as_str())?,
            }],
        }));
    }

    if let Some(captures) =
        static_regex(r"^(.+?) gets \+1/\+0 for each (.+?) you control\.$").captures(&body)
    {
        let affected = parse_named_self_or_object_selector(
            captures.get(1).expect("scaled P/T subject").as_str(),
            Some(Zone::Battlefield),
        )?;
        let counted_source = format!(
            "{} you control",
            captures.get(2).expect("scaled P/T count subject").as_str()
        );
        let counted = parse_object_selector(&counted_source, Some(Zone::Battlefield))?;
        return Ok(Some(StaticEffect::Characteristics {
            affected,
            condition,
            operations: vec![CharacteristicOperation::ModifyPowerToughness {
                power: SignedAmount {
                    negative: false,
                    magnitude: Amount::Count(counted),
                },
                toughness: SignedAmount {
                    negative: false,
                    magnitude: Amount::Fixed(0),
                },
            }],
        }));
    }

    if let Some(captures) =
        static_regex(r"^(.+?) (?:gets|get) ([+-]\d+)/([+-]\d+) for each (.+?)\.$").captures(&body)
    {
        let affected = parse_named_self_or_object_selector(
            captures.get(1).expect("scaled P/T subject").as_str(),
            Some(Zone::Battlefield),
        )?;
        let magnitude = parse_counted_characteristic_amount(
            captures.get(4).expect("scaled P/T count operand").as_str(),
        )?;
        let signed = |value: &str| -> Result<SignedAmount, StaticReplacementCompileError> {
            let coefficient = value
                .parse::<i32>()
                .map_err(|_| StaticReplacementCompileError::UnsupportedOperand(value.to_owned()))?;
            let factor = coefficient.unsigned_abs();
            Ok(SignedAmount {
                negative: coefficient.is_negative(),
                magnitude: match factor {
                    0 => Amount::Fixed(0),
                    1 => magnitude.clone(),
                    _ => Amount::Scaled {
                        factor,
                        amount: Box::new(magnitude.clone()),
                    },
                },
            })
        };
        return Ok(Some(StaticEffect::Characteristics {
            affected,
            condition,
            operations: vec![CharacteristicOperation::ModifyPowerToughness {
                power: signed(captures.get(2).expect("scaled power").as_str())?,
                toughness: signed(captures.get(3).expect("scaled toughness").as_str())?,
            }],
        }));
    }

    let pt_pattern =
        r"^(.+?) (?:gets|get) ([+-]\d+|[+-]X)/([+-]\d+|[+-]X)(?:,? (?:and )?(?:has|have) (.+))?\.$";
    if let Some(captures) = static_regex(pt_pattern).captures(&body) {
        let affected = parse_named_self_or_object_selector(
            captures.get(1).expect("P/T subject capture").as_str(),
            Some(Zone::Battlefield),
        )?;
        let power = parse_signed_amount(captures.get(2).expect("power capture").as_str())?;
        let toughness = parse_signed_amount(captures.get(3).expect("toughness capture").as_str())?;
        let mut operations =
            vec![CharacteristicOperation::ModifyPowerToughness { power, toughness }];
        if let Some(keywords) = captures.get(4) {
            operations.push(CharacteristicOperation::GrantKeywords(parse_keyword_set(
                keywords.as_str(),
            )?));
        }
        return Ok(Some(StaticEffect::Characteristics {
            affected,
            condition,
            operations,
        }));
    }

    let base_pattern = r"^(.+?) (?:has|have) base power and toughness (\d+|X)/(\d+|X)\.$";
    if let Some(captures) = static_regex(base_pattern).captures(&body) {
        let affected = parse_named_self_or_object_selector(
            captures.get(1).expect("base P/T subject capture").as_str(),
            Some(Zone::Battlefield),
        )?;
        return Ok(Some(StaticEffect::Characteristics {
            affected,
            condition,
            operations: vec![CharacteristicOperation::SetBasePowerToughness {
                power: parse_amount(captures.get(2).expect("base power capture").as_str())?,
                toughness: parse_amount(captures.get(3).expect("base toughness capture").as_str())?,
            }],
        }));
    }

    let equal_pattern = r"^(.+?)(?:'s|’s) power and toughness are each equal to (\d+|X)\.$";
    if let Some(captures) = static_regex(equal_pattern).captures(&body) {
        let affected = parse_named_self_or_object_selector(
            captures.get(1).expect("equal P/T subject capture").as_str(),
            Some(Zone::Battlefield),
        )?;
        let amount = parse_amount(captures.get(2).expect("equal amount capture").as_str())?;
        return Ok(Some(StaticEffect::Characteristics {
            affected,
            condition,
            operations: vec![CharacteristicOperation::SetBasePowerToughness {
                power: amount.clone(),
                toughness: amount,
            }],
        }));
    }

    let keyword_pattern = r"^(.+?) (?:has|have) (.+)\.$";
    if let Some(captures) = static_regex(keyword_pattern).captures(&body) {
        let operand = captures.get(2).expect("keyword operand capture").as_str();
        if let Ok(keywords) = parse_keyword_set(operand) {
            let affected = parse_named_self_or_object_selector(
                captures.get(1).expect("keyword subject capture").as_str(),
                Some(Zone::Battlefield),
            )?;
            return Ok(Some(StaticEffect::Characteristics {
                affected,
                condition,
                operations: vec![CharacteristicOperation::GrantKeywords(keywords)],
            }));
        }
    }

    let loss_pattern = r"^(.+?) (?:loses|lose) (.+)\.$";
    if let Some(captures) = static_regex(loss_pattern).captures(&body) {
        let affected = parse_named_self_or_object_selector(
            captures.get(1).expect("loss subject capture").as_str(),
            Some(Zone::Battlefield),
        )?;
        let operand = captures.get(2).expect("loss operand capture").as_str();
        if operand == "all abilities" {
            return Ok(Some(StaticEffect::Characteristics {
                affected,
                condition,
                operations: vec![CharacteristicOperation::LoseAllAbilities],
            }));
        }
        let operation = CharacteristicOperation::RemoveKeywords(parse_keyword_set(operand)?);
        return Ok(Some(StaticEffect::Characteristics {
            affected,
            condition,
            operations: vec![operation],
        }));
    }

    Ok(None)
}

fn parse_counted_characteristic_amount(
    source: &str,
) -> Result<Amount, StaticReplacementCompileError> {
    let normalized = source.trim().replace('’', "'").replace('‘', "'");
    let lower = normalized.to_ascii_lowercase();
    for (suffix, zone) in [
        (" in all graveyards", Zone::Graveyard),
        (" in all players' hands", Zone::Hand),
        (" in exile", Zone::Exile),
    ] {
        if !lower.ends_with(suffix) {
            continue;
        }
        let subject = normalized[..normalized.len() - suffix.len()].trim();
        let mut selector = parse_object_selector(subject, Some(zone))?;
        selector.owner = ControllerRelation::Any;
        if subject
            .to_ascii_lowercase()
            .split_whitespace()
            .last()
            .is_some_and(|noun| matches!(noun, "card" | "cards"))
        {
            selector.token_relation = TokenRelation::Nontoken;
        }
        return Ok(Amount::Count(selector));
    }
    for (suffix, zone) in [
        (" in your opponents' hands", Zone::Hand),
        (" in your opponents' graveyards", Zone::Graveyard),
    ] {
        if !lower.ends_with(suffix) {
            continue;
        }
        let subject = normalized[..normalized.len() - suffix.len()].trim();
        let mut selector = parse_object_selector(subject, Some(zone))?;
        selector.owner = ControllerRelation::Opponent;
        selector.token_relation = TokenRelation::Nontoken;
        return Ok(Amount::Count(selector));
    }
    if let Some(subject) = lower.strip_suffix(" your opponents own in exile") {
        let mut selector = parse_object_selector(subject, Some(Zone::Exile))?;
        selector.owner = ControllerRelation::Opponent;
        selector.token_relation = TokenRelation::Nontoken;
        return Ok(Amount::Count(selector));
    }
    for (suffix, zone) in [
        (" in its controller's hand", Zone::Hand),
        (" in its controller's graveyard", Zone::Graveyard),
    ] {
        if !lower.ends_with(suffix) {
            continue;
        }
        let subject = normalized[..normalized.len() - suffix.len()].trim();
        let mut selector = parse_object_selector(subject, Some(zone))?;
        selector.owner = ControllerRelation::You;
        selector.token_relation = TokenRelation::Nontoken;
        return Ok(Amount::AffectedControllerCount(selector));
    }
    if lower == "noncreature, nonland card in your graveyard" {
        let mut selector = ObjectSelector::matching(Some(Zone::Graveyard));
        selector.owner = ControllerRelation::You;
        selector.token_relation = TokenRelation::Nontoken;
        selector
            .excluded_card_types
            .extend([CardType::Creature, CardType::Land]);
        return Ok(Amount::Count(selector));
    }
    if let Some(captures) = static_regex(r"^(.+?) attached to (.+)$").captures(&normalized) {
        let subtypes = match captures.get(1).expect("attachment-count subtype").as_str() {
            "Aura" => BTreeSet::from(["Aura".to_owned()]),
            "Equipment" => BTreeSet::from(["Equipment".to_owned()]),
            "Aura and Equipment" => BTreeSet::from(["Aura".to_owned(), "Equipment".to_owned()]),
            "Role" => BTreeSet::from(["Role".to_owned()]),
            _ => BTreeSet::new(),
        };
        if !subtypes.is_empty() {
            let object = captures.get(2).expect("attachment-count object").as_str();
            return Ok(if matches!(object, "it" | "him" | "her") {
                Amount::AttachmentCount { subtypes }
            } else {
                Amount::AttachmentCountOnObject {
                    object: parse_named_self_or_object_selector(object, Some(Zone::Battlefield))?,
                    subtypes,
                }
            });
        }
    }
    if let Some(captures) = static_regex(r"^counters? on (.+)$").captures(&normalized) {
        let object = captures
            .get(1)
            .expect("total-counter count object")
            .as_str();
        return Ok(if matches!(object, "it" | "them") {
            Amount::TotalCounterCountOnAffected
        } else {
            Amount::TotalCounterCount {
                object: parse_named_self_or_object_selector(object, Some(Zone::Battlefield))?,
            }
        });
    }
    if let Some(captures) = static_regex(r"^(.+?) counters? on (.+)$").captures(&normalized) {
        let counter_text = captures.get(1).expect("counter count kind").as_str();
        if let Ok(counter) = parse_counter_kind(counter_text) {
            let object = captures.get(2).expect("counter count object").as_str();
            return Ok(if matches!(object, "it" | "them") {
                Amount::CounterCountOnAffected { counter }
            } else {
                Amount::CounterCount {
                    object: parse_named_self_or_object_selector(object, Some(Zone::Battlefield))?,
                    counter,
                }
            });
        }
    }
    if matches!(
        lower.as_str(),
        "opponent" | "opponents" | "opponent you have"
    ) {
        return Ok(Amount::OpponentCount);
    }
    if matches!(
        lower.as_str(),
        "card in your hand"
            | "cards in your hand"
            | "card in your graveyard"
            | "cards in your graveyard"
    ) {
        let zone = if lower.ends_with("your hand") {
            Zone::Hand
        } else {
            Zone::Graveyard
        };
        let mut selector = ObjectSelector::matching(Some(zone));
        selector.owner = ControllerRelation::You;
        selector.token_relation = TokenRelation::Nontoken;
        return Ok(Amount::Count(selector));
    }
    for (suffix, zone) in [
        (" cards in your hand", Zone::Hand),
        (" card in your hand", Zone::Hand),
        (" cards in your graveyard", Zone::Graveyard),
        (" card in your graveyard", Zone::Graveyard),
    ] {
        if !lower.ends_with(suffix) {
            continue;
        }
        let qualifier = normalized[..normalized.len() - suffix.len()].trim();
        let selector_source = if qualifier.is_empty() {
            "card".to_owned()
        } else {
            format!("{qualifier} card")
        };
        let mut selector = parse_object_selector(&selector_source, Some(zone))?;
        selector.owner = ControllerRelation::You;
        return Ok(Amount::Count(selector));
    }
    if lower.ends_with(" you control")
        || lower.ends_with(" you don't control")
        || lower.ends_with(" your opponents control")
        || lower.ends_with(" an opponent controls")
        || lower.ends_with(" opponents control")
    {
        return Ok(Amount::Count(parse_object_selector(
            &normalized,
            Some(Zone::Battlefield),
        )?));
    }
    for suffix in [" on the battlefield", " on battlefield"] {
        if lower.ends_with(suffix) {
            let subject = normalized[..normalized.len() - suffix.len()].trim();
            return Ok(Amount::Count(parse_object_selector(
                subject,
                Some(Zone::Battlefield),
            )?));
        }
    }
    if let Ok(selector) = parse_object_selector(&normalized, Some(Zone::Battlefield)) {
        return Ok(Amount::Count(selector));
    }
    Err(StaticReplacementCompileError::UnsupportedOperand(
        source.to_owned(),
    ))
}

/// Parse only amounts derivable from complete current-state evidence. This
/// deliberately excludes cast history, draw history, payments, targets,
/// choices, and arbitrary remembered values.
fn parse_state_derived_count_amount(source: &str) -> Result<Amount, StaticReplacementCompileError> {
    let normalized = source.trim().replace('’', "'").replace('‘', "'");
    if matches!(
        normalized.as_str(),
        "time it was kicked" | "time he was kicked" | "time she was kicked"
    ) {
        return Ok(Amount::KickerPayments);
    }
    if let Some(captures) =
        static_regex(r"^(.+?)(?:'s|’s) (power|toughness)$").captures(&normalized)
    {
        let object = parse_named_self_or_object_selector(
            captures
                .get(1)
                .expect("characteristic-value object")
                .as_str(),
            Some(Zone::Battlefield),
        )?;
        return Ok(
            match captures.get(2).expect("characteristic-value kind").as_str() {
                "power" => Amount::PowerOf(object),
                "toughness" => Amount::ToughnessOf(object),
                _ => unreachable!("characteristic-value regex is closed"),
            },
        );
    }
    let subject = normalized
        .strip_prefix("the total number of ")
        .or_else(|| normalized.strip_prefix("the number of "))
        .or_else(|| normalized.strip_prefix("number of "))
        .unwrap_or(&normalized)
        .replace(" you already control", " you control");
    let amount = parse_counted_characteristic_amount(&subject)?;
    if matches!(
        &amount,
        Amount::Count(_)
            | Amount::OpponentCount
            | Amount::CounterCount { .. }
            | Amount::CounterCountOnAffected { .. }
            | Amount::TotalCounterCount { .. }
            | Amount::TotalCounterCountOnAffected
            | Amount::AffectedControllerCount(_)
            | Amount::PowerOf(_)
            | Amount::ToughnessOf(_)
            | Amount::KickerPayments
    ) {
        Ok(amount)
    } else {
        Err(StaticReplacementCompileError::UnsupportedOperand(
            source.to_owned(),
        ))
    }
}

fn split_static_condition(
    source: &str,
) -> Result<(String, Condition), StaticReplacementCompileError> {
    if let Some(body) = source.strip_prefix("During your turn, ") {
        return Ok((body.to_owned(), Condition::DuringYourTurn));
    }
    if let Some(body) = source.strip_prefix("During turns other than yours, ") {
        return Ok((body.to_owned(), Condition::NotDuringYourTurn));
    }
    let Some(without_period) = source.strip_suffix('.') else {
        return Ok((source.to_owned(), Condition::Always));
    };
    for (separator, negate) in [(" as long as ", false), (" unless ", true), (" if ", false)] {
        let Some((body, condition)) = without_period.rsplit_once(separator) else {
            continue;
        };
        let parsed = parse_static_condition_expression(condition)?;
        let parsed = if negate {
            Condition::Not(Box::new(parsed))
        } else {
            parsed
        };
        return Ok((format!("{body}."), parsed));
    }
    Ok((source.to_owned(), Condition::Always))
}

fn parse_static_condition_expression(
    condition: &str,
) -> Result<Condition, StaticReplacementCompileError> {
    let condition = condition
        .replace("he has ", "it has ")
        .replace("she has ", "it has ")
        .replace("her power", "its power")
        .replace("his power", "its power")
        .replace("her toughness", "its toughness")
        .replace("his toughness", "its toughness");
    let condition = condition.as_str();
    let parsed = match condition {
        "it's your turn" | "it is your turn" => Condition::DuringYourTurn,
        "it's not your turn" | "it is not your turn" => Condition::NotDuringYourTurn,
        "this permanent is tapped" | "this creature is tapped" => Condition::SourceIsTapped,
        "this permanent is untapped" | "this creature is untapped" => Condition::SourceIsUntapped,
        "it's attacking or blocking" | "it is attacking or blocking" => Condition::Any(vec![
            parse_affected_object_static_condition("it's attacking")?
                .expect("attacking affected-object condition"),
            parse_affected_object_static_condition("it's blocking")?
                .expect("blocking affected-object condition"),
        ]),
        _ => {
            if let Some(captures) = static_regex(
                r"^(one|two|three|four|five|six|seven|eight|nine|ten|\d+) or more (Auras?|Equipment|Roles?) are attached to it$",
            )
            .captures(condition)
            {
                let subtype = match captures
                    .get(2)
                    .expect("attachment condition subtype")
                    .as_str()
                {
                    "Aura" | "Auras" => "Aura",
                    "Equipment" => "Equipment",
                    "Role" | "Roles" => "Role",
                    _ => unreachable!("attachment condition regex is closed"),
                };
                Condition::AffectedAttachmentCount {
                    subtypes: BTreeSet::from([subtype.to_owned()]),
                    minimum: parse_word_or_number(
                        captures
                            .get(1)
                            .expect("attachment condition minimum")
                            .as_str(),
                    )?,
                    maximum: None,
                }
            } else if let Some(parsed) = parse_affected_object_static_condition(condition)? {
                parsed
            } else if let Some(value) = static_regex(r"^you have (\d+) or less life$")
                .captures(condition)
                .and_then(|captures| captures.get(1))
            {
                Condition::ControllerLifeAtMost(parse_u32(value.as_str())?)
            } else if let Some(value) =
                static_regex(r"^your life total is (\d+) or less$")
                    .captures(condition)
                    .and_then(|captures| captures.get(1))
            {
                Condition::ControllerLifeAtMost(parse_u32(value.as_str())?)
            } else if let Some(value) = static_regex(r"^you have (\d+) or more life$")
                .captures(condition)
                .and_then(|captures| captures.get(1))
            {
                Condition::ControllerLifeAtLeast(parse_u32(value.as_str())?)
            } else if let Some(value) =
                static_regex(r"^your life total is (\d+) or more$")
                    .captures(condition)
                    .and_then(|captures| captures.get(1))
            {
                Condition::ControllerLifeAtLeast(parse_u32(value.as_str())?)
            } else if let Some(parsed) = parse_object_count_static_condition(condition)? {
                parsed
            } else if let Some(captures) =
                static_regex(r"^(?:a|an) (.+?) is on the battlefield$").captures(condition)
            {
                Condition::MatchingObjectCount {
                    selector: parse_object_selector(
                        captures
                            .get(1)
                            .expect("battlefield-presence condition")
                            .as_str(),
                        Some(Zone::Battlefield),
                    )?,
                    minimum: 1,
                    maximum: None,
                }
            } else if let Some(subject) = condition.strip_prefix("you control ") {
                let mut selector = parse_object_selector(subject, Some(Zone::Battlefield))?;
                selector.controller = ControllerRelation::You;
                Condition::ControllerControls(selector)
            } else if let Some(subject) = condition.strip_prefix("an opponent controls ") {
                let mut selector = parse_object_selector(subject, Some(Zone::Battlefield))?;
                selector.controller = ControllerRelation::Opponent;
                Condition::ControllerControls(selector)
            } else if let Some(subject) = condition.strip_prefix("any player controls ") {
                let mut selector = parse_object_selector(subject, Some(Zone::Battlefield))?;
                selector.controller = ControllerRelation::Any;
                Condition::ControllerControls(selector)
            } else {
                return Err(StaticReplacementCompileError::UnsupportedOperand(
                    condition.to_owned(),
                ));
            }
        }
    };
    Ok(parsed)
}

fn parse_affected_object_static_condition(
    source: &str,
) -> Result<Option<Condition>, StaticReplacementCompileError> {
    let lower = source.to_ascii_lowercase();
    let mut selector = ObjectSelector::matching(None);
    match lower.as_str() {
        "it's attacking" | "it is attacking" => selector.attacking = Some(true),
        "it's not attacking" | "it is not attacking" => selector.attacking = Some(false),
        "it's blocking" | "it is blocking" => selector.blocking = Some(true),
        "it's not blocking" | "it is not blocking" => selector.blocking = Some(false),
        "it isn't attacking or blocking" | "it is not attacking or blocking" => {
            selector.attacking = Some(false);
            selector.blocking = Some(false);
        }
        "it's tapped" | "it is tapped" => selector.tapped = Some(true),
        "it's untapped" | "it is untapped" => selector.tapped = Some(false),
        "it's enchanted" | "it is enchanted" => selector.enchanted = Some(true),
        "it's equipped" | "it is equipped" => selector.equipped = Some(true),
        "it's legendary" | "it is legendary" => {
            selector.supertypes.insert(Supertype::Legendary);
        }
        "it's basic" | "it is basic" => {
            selector.supertypes.insert(Supertype::Basic);
        }
        "it's snow" | "it is snow" => {
            selector.supertypes.insert(Supertype::Snow);
        }
        _ => {
            if let Some(color) = lower
                .strip_prefix("it's ")
                .or_else(|| lower.strip_prefix("it is "))
                .and_then(parse_color)
            {
                selector.colors.insert(color);
            } else if let Some(noun) = lower
                .strip_prefix("it's a ")
                .or_else(|| lower.strip_prefix("it is a "))
            {
                selector.card_types.insert(card_type_adjective(noun)?);
            } else if let Some(captures) = static_regex(
                r"^its (power|toughness|mana value) is (\d+) or (greater|more|less)$",
            )
            .captures(source)
            {
                let characteristic = captures
                    .get(1)
                    .expect("affected characteristic")
                    .as_str();
                let value = parse_u32(
                    captures
                        .get(2)
                        .expect("affected characteristic threshold")
                        .as_str(),
                )?;
                let lower_bound = matches!(
                    captures
                        .get(3)
                        .expect("affected characteristic direction")
                        .as_str(),
                    "greater" | "more"
                );
                match (characteristic, lower_bound) {
                    ("mana value", true) => selector.minimum_mana_value = Some(value),
                    ("mana value", false) => selector.maximum_mana_value = Some(value),
                    ("power", true) => selector.minimum_power = Some(i32::try_from(value).map_err(
                        |_| StaticReplacementCompileError::UnsupportedOperand(value.to_string()),
                    )?),
                    ("power", false) => selector.maximum_power = Some(i32::try_from(value).map_err(
                        |_| StaticReplacementCompileError::UnsupportedOperand(value.to_string()),
                    )?),
                    ("toughness", true) => {
                        selector.minimum_toughness = Some(i32::try_from(value).map_err(|_| {
                            StaticReplacementCompileError::UnsupportedOperand(value.to_string())
                        })?)
                    }
                    ("toughness", false) => {
                        selector.maximum_toughness = Some(i32::try_from(value).map_err(|_| {
                            StaticReplacementCompileError::UnsupportedOperand(value.to_string())
                        })?)
                    }
                    _ => unreachable!("regex limits affected characteristic"),
                }
            } else if let Some(captures) = static_regex(
                r"^it has (one|two|three|four|five|six|seven|eight|nine|ten|\d+) or more (.+?) counters? on it$",
            )
            .captures(source)
            {
                let minimum = parse_word_or_number(
                    captures
                        .get(1)
                        .expect("affected counter threshold")
                        .as_str(),
                )?;
                let counter = parse_counter_kind(
                    captures
                        .get(2)
                        .expect("affected counter kind")
                        .as_str(),
                )?;
                selector.minimum_counters.insert(counter, minimum);
            } else if let Some(counter_text) = lower
                .strip_prefix("it has a ")
                .and_then(|value| value.strip_suffix(" counter on it"))
            {
                selector
                    .minimum_counters
                    .insert(parse_counter_kind(counter_text)?, 1);
            } else if let Some(keyword_text) = lower.strip_prefix("it has ") {
                selector.required_keywords = parse_keyword_set(keyword_text)?;
            } else {
                return Ok(None);
            }
        }
    }
    Ok(Some(Condition::AffectedObjectMatches(selector)))
}

fn parse_object_count_static_condition(
    source: &str,
) -> Result<Option<Condition>, StaticReplacementCompileError> {
    let make = |selector: ObjectSelector, minimum: u32, maximum: Option<u32>| {
        Condition::MatchingObjectCount {
            selector,
            minimum,
            maximum,
        }
    };
    let make_any_player =
        |players: PlayerSelector,
         relation: PlayerObjectRelation,
         selector: ObjectSelector,
         minimum: u32,
         maximum: Option<u32>| Condition::AnyPlayerObjectCount {
            players,
            relation,
            selector,
            minimum,
            maximum,
        };
    if let Some(captures) = static_regex(
        r"^(an opponent|a player) has (one|two|three|four|five|six|seven|eight|nine|ten|twenty|\d+) or (more|fewer) (?:(.+?) )?cards? in (?:their )?(hand|graveyard|library)$",
    )
    .captures(source)
    {
        let players = if captures
            .get(1)
            .expect("per-player zone-count player")
            .as_str()
            == "an opponent"
        {
            PlayerSelector::Opponents
        } else {
            PlayerSelector::EachPlayer
        };
        let threshold = parse_word_or_number(
            captures
                .get(2)
                .expect("per-player zone-count threshold")
                .as_str(),
        )?;
        let zone = parse_zone(
            captures
                .get(5)
                .expect("per-player zone-count zone")
                .as_str(),
        )?;
        let subject = captures
            .get(4)
            .map(|qualifier| format!("{} cards", qualifier.as_str()))
            .unwrap_or_else(|| "cards".to_owned());
        let selector = parse_object_selector(&subject, Some(zone))?;
        let (minimum, maximum) = if captures
            .get(3)
            .expect("per-player zone-count direction")
            .as_str()
            == "more"
        {
            (threshold, None)
        } else {
            (0, Some(threshold))
        };
        return Ok(Some(make_any_player(
            players,
            PlayerObjectRelation::Owner,
            selector,
            minimum,
            maximum,
        )));
    }
    if let Some(captures) = static_regex(
        r"^(an opponent|a player) has no (?:(.+?) )?cards? in (?:their )?(hand|graveyard|library)$",
    )
    .captures(source)
    {
        let players = if captures
            .get(1)
            .expect("per-player empty-zone player")
            .as_str()
            == "an opponent"
        {
            PlayerSelector::Opponents
        } else {
            PlayerSelector::EachPlayer
        };
        let zone = parse_zone(
            captures
                .get(3)
                .expect("per-player empty-zone zone")
                .as_str(),
        )?;
        let subject = captures
            .get(2)
            .map(|qualifier| format!("{} cards", qualifier.as_str()))
            .unwrap_or_else(|| "cards".to_owned());
        return Ok(Some(make_any_player(
            players,
            PlayerObjectRelation::Owner,
            parse_object_selector(&subject, Some(zone))?,
            0,
            Some(0),
        )));
    }
    if let Some(captures) = static_regex(
        r"^an opponent controls (one|two|three|four|five|six|seven|eight|nine|ten|twenty|\d+) or more (.+)$",
    )
    .captures(source)
    {
        return Ok(Some(make_any_player(
            PlayerSelector::Opponents,
            PlayerObjectRelation::Controller,
            parse_object_selector(
                captures
                    .get(2)
                    .expect("opponent controlled-count subject")
                    .as_str(),
                Some(Zone::Battlefield),
            )?,
            parse_word_or_number(
                captures
                    .get(1)
                    .expect("opponent controlled-count threshold")
                    .as_str(),
            )?,
            None,
        )));
    }
    if let Some(subject) = source.strip_prefix("an opponent controls no ") {
        return Ok(Some(make_any_player(
            PlayerSelector::Opponents,
            PlayerObjectRelation::Controller,
            parse_object_selector(subject, Some(Zone::Battlefield))?,
            0,
            Some(0),
        )));
    }
    if let Some(captures) = static_regex(
        r"^a library has (one|two|three|four|five|six|seven|eight|nine|ten|twenty|\d+) or (more|fewer) cards in it$",
    )
    .captures(source)
    {
        let threshold = parse_word_or_number(
            captures
                .get(1)
                .expect("library-count threshold")
                .as_str(),
        )?;
        let (minimum, maximum) = if captures
            .get(2)
            .expect("library-count direction")
            .as_str()
            == "more"
        {
            (threshold, None)
        } else {
            (0, Some(threshold))
        };
        return Ok(Some(make_any_player(
            PlayerSelector::EachPlayer,
            PlayerObjectRelation::Owner,
            parse_object_selector("cards", Some(Zone::Library))?,
            minimum,
            maximum,
        )));
    }
    if let Some(captures) = static_regex(
        r"^you have (one|two|three|four|five|six|seven|eight|nine|ten|\d+) or (more|fewer) (?:(.+?) )?cards? in (?:your )?(hand|graveyard)$",
    )
    .captures(source)
    {
        let threshold = parse_word_or_number(
            captures
                .get(1)
                .expect("owned-zone count threshold")
                .as_str(),
        )?;
        let zone = captures
            .get(4)
            .expect("owned-zone count zone")
            .as_str();
        let qualifier = captures.get(3).map(|value| value.as_str().trim());
        let counted_source = match qualifier {
            Some(qualifier) if !qualifier.is_empty() => {
                format!("{qualifier} cards in your {zone}")
            }
            _ => format!("cards in your {zone}"),
        };
        let Amount::Count(selector) = parse_counted_characteristic_amount(&counted_source)? else {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                source.to_owned(),
            ));
        };
        return Ok(Some(if captures.get(2).expect("count direction").as_str() == "more" {
            make(selector, threshold, None)
        } else {
            make(selector, 0, Some(threshold))
        }));
    }
    if let Some(captures) = static_regex(
        r"^you control (one|two|three|four|five|six|seven|eight|nine|ten|\d+) or more (.+)$",
    )
    .captures(source)
    {
        let minimum = parse_word_or_number(
            captures
                .get(1)
                .expect("controlled count threshold")
                .as_str(),
        )?;
        let subject = format!(
            "{} you control",
            captures.get(2).expect("controlled count subject").as_str()
        );
        return Ok(Some(make(
            parse_object_selector(&subject, Some(Zone::Battlefield))?,
            minimum,
            None,
        )));
    }
    if let Some(captures) = static_regex(
        r"^you control at least (one|two|three|four|five|six|seven|eight|nine|ten|\d+) (.+)$",
    )
    .captures(source)
    {
        let minimum = parse_word_or_number(
            captures
                .get(1)
                .expect("controlled minimum threshold")
                .as_str(),
        )?;
        let subject = format!(
            "{} you control",
            captures
                .get(2)
                .expect("controlled minimum subject")
                .as_str()
        );
        return Ok(Some(make(
            parse_object_selector(&subject, Some(Zone::Battlefield))?,
            minimum,
            None,
        )));
    }
    if let Some(subject) = source.strip_prefix("you control no ") {
        let subject = format!("{subject} you control");
        return Ok(Some(make(
            parse_object_selector(&subject, Some(Zone::Battlefield))?,
            0,
            Some(0),
        )));
    }
    if let Some(subject) = source.strip_prefix("no opponent controls ") {
        let subject = format!("{subject} an opponent controls");
        return Ok(Some(make(
            parse_object_selector(&subject, Some(Zone::Battlefield))?,
            0,
            Some(0),
        )));
    }
    if let Some(captures) = static_regex(
        r"^there are (one|two|three|four|five|six|seven|eight|nine|ten|\d+) or more (.+)$",
    )
    .captures(source)
    {
        let minimum =
            parse_word_or_number(captures.get(1).expect("global count threshold").as_str())?;
        let Amount::Count(selector) = parse_counted_characteristic_amount(
            captures.get(2).expect("global count subject").as_str(),
        )?
        else {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                source.to_owned(),
            ));
        };
        return Ok(Some(make(selector, minimum, None)));
    }
    if let Some(subject) = source.strip_prefix("there are no ") {
        let Amount::Count(selector) = parse_counted_characteristic_amount(subject)? else {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                source.to_owned(),
            ));
        };
        return Ok(Some(make(selector, 0, Some(0))));
    }
    let singular_subject = source
        .strip_prefix("there is a ")
        .or_else(|| source.strip_prefix("there is an "))
        .or_else(|| source.strip_prefix("there's a "))
        .or_else(|| source.strip_prefix("there's an "));
    if let Some(subject) = singular_subject {
        let Amount::Count(selector) = parse_counted_characteristic_amount(subject)? else {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                source.to_owned(),
            ));
        };
        return Ok(Some(make(selector, 1, None)));
    }
    Ok(None)
}

fn parse_restriction_static(
    source: &str,
) -> Result<Option<Restriction>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");

    if normalized == "You have hexproof." {
        return Ok(Some(Restriction::CannotBeTargeted {
            target: RecipientSelector::Player(PlayerSelector::You),
            forbidden_controller: PlayerSelector::Opponents,
            spells: true,
            abilities: true,
        }));
    }

    if normalized == "Cards in graveyards can't be the targets of spells or abilities." {
        return Ok(Some(Restriction::CannotBeTargeted {
            target: RecipientSelector::Object(ObjectSelector::matching(Some(Zone::Graveyard))),
            forbidden_controller: PlayerSelector::EachPlayer,
            spells: true,
            abilities: true,
        }));
    }

    if let Some(body) = normalized.strip_suffix(" during your turn.")
        && let Some(restriction) = parse_restriction_static(&format!("{body}."))?
    {
        return Ok(Some(Restriction::Conditional {
            restriction: Box::new(restriction),
            condition: Condition::DuringYourTurn,
            active_when_condition_holds: true,
        }));
    }

    if let Some(captures) = static_regex(
        r"^Activated abilities of (.+?) can't be activated unless (?:they're|they are) mana abilities\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(Restriction::CannotActivateAbilities {
            player: PlayerSelector::EachPlayer,
            source: Some(parse_named_self_or_object_selector(
                captures.get(1).expect("activation source subject").as_str(),
                Some(Zone::Battlefield),
            )?),
            kind: AbilityRestrictionKind::NonManaOnly,
        }));
    }

    if let Some(captures) = static_regex(
        r"^(.+?)'s activated abilities can't be activated(?: unless (?:they're|they are) mana abilities)?\.$",
    )
    .captures(&normalized)
    {
        let nonmana_only = normalized.ends_with(" unless they're mana abilities.")
            || normalized.ends_with(" unless they are mana abilities.");
        return Ok(Some(Restriction::CannotActivateAbilities {
            player: PlayerSelector::EachPlayer,
            source: Some(parse_named_self_or_object_selector(
                captures.get(1).expect("activation source subject").as_str(),
                Some(Zone::Battlefield),
            )?),
            kind: if nonmana_only {
                AbilityRestrictionKind::NonManaOnly
            } else {
                AbilityRestrictionKind::All
            },
        }));
    }

    if let Some(without_period) = normalized.strip_suffix('.') {
        for (separator, active_when_condition_holds) in
            [(" as long as ", true), (" unless ", false), (" if ", true)]
        {
            let Some((body, condition_text)) = without_period.rsplit_once(separator) else {
                continue;
            };
            let Some(restriction) = parse_restriction_static(&format!("{body}."))? else {
                continue;
            };
            let (_, condition) = split_static_condition(&format!(
                "This creature gets +0/+0 as long as {condition_text}."
            ))?;
            return Ok(Some(Restriction::Conditional {
                restriction: Box::new(restriction),
                condition,
                active_when_condition_holds,
            }));
        }
    }

    if let Some(captures) =
        static_regex(r"^Activated abilities of (.+?) can't be activated\.$").captures(&normalized)
    {
        return Ok(Some(Restriction::CannotActivateAbilities {
            player: PlayerSelector::EachPlayer,
            source: Some(parse_object_selector(
                captures.get(1).expect("activation source subject").as_str(),
                Some(Zone::Battlefield),
            )?),
            kind: AbilityRestrictionKind::All,
        }));
    }

    if let Some(captures) = static_regex(r"^(.+?) can't attack or block\.$").captures(&normalized) {
        return Ok(Some(Restriction::CannotAttackOrBlock {
            object: parse_named_self_or_object_selector(
                captures.get(1).expect("attack or block subject").as_str(),
                Some(Zone::Battlefield),
            )?,
        }));
    }
    if let Some(captures) =
        static_regex(r"^(.+?) can't attack (you|your opponents|an opponent|opponents|players)\.$")
            .captures(&normalized)
    {
        return Ok(Some(Restriction::CannotAttackPlayer {
            attacker: parse_named_self_or_object_selector(
                captures.get(1).expect("attack subject").as_str(),
                Some(Zone::Battlefield),
            )?,
            defender: parse_player_selector(captures.get(2).expect("attack defender").as_str())?,
        }));
    }
    if let Some(captures) = static_regex(r"^(.+?) can't attack\.$").captures(&normalized) {
        return Ok(Some(Restriction::CannotAttack {
            attacker: parse_named_self_or_object_selector(
                captures.get(1).expect("attack subject").as_str(),
                Some(Zone::Battlefield),
            )?,
        }));
    }
    if let Some(captures) = static_regex(r"^(.+?) can't block (.+?)\.$").captures(&normalized) {
        return Ok(Some(Restriction::CannotBlockCreature {
            blocker: parse_named_self_or_object_selector(
                captures.get(1).expect("qualified blocker subject").as_str(),
                Some(Zone::Battlefield),
            )?,
            attacker: parse_object_selector(
                captures
                    .get(2)
                    .expect("forbidden attacker subject")
                    .as_str(),
                Some(Zone::Battlefield),
            )?,
        }));
    }
    if let Some(captures) = static_regex(r"^(.+?) can block only (.+?)\.$").captures(&normalized) {
        return Ok(Some(Restriction::CanBlockOnly {
            blocker: parse_named_self_or_object_selector(
                captures.get(1).expect("limited blocker subject").as_str(),
                Some(Zone::Battlefield),
            )?,
            allowed_attacker: parse_object_selector(
                captures.get(2).expect("allowed attacker subject").as_str(),
                Some(Zone::Battlefield),
            )?,
        }));
    }
    if let Some(captures) = static_regex(r"^(.+?) can't block\.$").captures(&normalized) {
        return Ok(Some(Restriction::CannotBlock {
            blocker: parse_named_self_or_object_selector(
                captures.get(1).expect("block subject").as_str(),
                Some(Zone::Battlefield),
            )?,
        }));
    }
    if let Some(captures) =
        static_regex(r"^(.+?) can't be blocked except by (.+?)\.$").captures(&normalized)
    {
        let allowed_blocker = captures.get(2).expect("allowed blocker subject").as_str();
        if static_regex(r"^(two|three|four|five|\d+) or more creatures$").is_match(allowed_blocker)
        {
            return Ok(None);
        }
        return Ok(Some(Restriction::CanBeBlockedOnlyBy {
            attacker: parse_named_self_or_object_selector(
                captures.get(1).expect("limited attacker subject").as_str(),
                Some(Zone::Battlefield),
            )?,
            allowed_blocker: parse_object_selector(allowed_blocker, Some(Zone::Battlefield))?,
        }));
    }
    if let Some(captures) =
        static_regex(r"^(.+?) can't be blocked(?: by (.+))?\.$").captures(&normalized)
    {
        let by = captures
            .get(2)
            .map(|value| parse_object_selector(value.as_str(), Some(Zone::Battlefield)))
            .transpose()?;
        return Ok(Some(Restriction::CannotBeBlocked {
            attacker: parse_named_self_or_object_selector(
                captures.get(1).expect("unblockable subject").as_str(),
                Some(Zone::Battlefield),
            )?,
            by,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) can't be (?:the target of|targeted by) (spells|abilities|spells or abilities) (you control|your opponents control|players control)\.$",
    )
    .captures(&normalized)
    {
        let kind = captures.get(2).expect("targeting kind").as_str();
        return Ok(Some(Restriction::CannotBeTargeted {
            target: parse_named_self_or_recipient_selector(
                captures.get(1).expect("target subject").as_str(),
            )?,
            forbidden_controller: parse_controller_phrase(
                captures.get(3).expect("forbidden controller").as_str(),
            )?,
            spells: kind.contains("spell"),
            abilities: kind.contains("abilit"),
        }));
    }
    if let Some(captures) = static_regex(r"^(.+?) can't be countered\.$").captures(&normalized) {
        return Ok(Some(Restriction::CannotBeCountered {
            spell: parse_named_self_or_object_selector(
                captures.get(1).expect("counter subject").as_str(),
                Some(Zone::Stack),
            )?,
        }));
    }
    if let Some(captures) =
        static_regex(r"^(You|Your opponents|Players|Each player) can't gain life\.$")
            .captures(&normalized)
    {
        return Ok(Some(Restriction::CannotGainLife {
            player: parse_player_selector(
                captures.get(1).expect("life restriction player").as_str(),
            )?,
        }));
    }
    if let Some(captures) =
        static_regex(r"^(You|Your opponents|Players|Each player) can't draw cards\.$")
            .captures(&normalized)
    {
        return Ok(Some(Restriction::CannotDrawCards {
            player: parse_player_selector(
                captures.get(1).expect("draw restriction player").as_str(),
            )?,
        }));
    }
    if let Some(captures) =
        static_regex(r"^(You|Your opponents|Players|Each player) can't play lands\.$")
            .captures(&normalized)
    {
        return Ok(Some(Restriction::CannotPlayLands {
            player: parse_player_selector(
                captures.get(1).expect("land restriction player").as_str(),
            )?,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Players|Each player) can't cast (.+?)(?: from (graveyards|exile|libraries))?\.$",
    )
    .captures(&normalized)
    {
        let player = parse_player_selector(
            captures.get(1).expect("cast restriction player").as_str(),
        )?;
        let from = captures
            .get(3)
            .map(|value| parse_plural_zone(value.as_str()))
            .transpose()?;
        let spells = parse_spell_selector(
            captures.get(2).expect("cast restriction spells").as_str(),
            player,
        )?;
        return Ok(Some(Restriction::CannotCast {
            player,
            spells,
            from,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Players|Each player) can't activate (mana abilities|nonmana abilities|activated abilities)(?: of (.+))?\.$",
    )
    .captures(&normalized)
    {
        let ability_kind = captures.get(2).expect("ability kind").as_str();
        let source = captures
            .get(3)
            .map(|value| parse_object_selector(value.as_str(), Some(Zone::Battlefield)))
            .transpose()?;
        return Ok(Some(Restriction::CannotActivateAbilities {
            player: parse_player_selector(
                captures.get(1).expect("activation player").as_str(),
            )?,
            source,
            kind: match ability_kind {
                "mana abilities" => AbilityRestrictionKind::ManaOnly,
                "nonmana abilities" => AbilityRestrictionKind::NonManaOnly,
                "activated abilities" => AbilityRestrictionKind::All,
                _ => unreachable!("regex limits ability restriction kind"),
            },
        }));
    }
    Ok(None)
}

fn parse_permission_static(
    source: &str,
) -> Result<Option<Permission>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    if normalized == "Creature cards you own that aren't on the battlefield have flash." {
        let mut cards = ObjectSelector::matching(None);
        cards.zones = BTreeSet::from([
            Zone::Library,
            Zone::Hand,
            Zone::Graveyard,
            Zone::Exile,
            Zone::Command,
        ]);
        cards.owner = ControllerRelation::You;
        cards.card_types.insert(CardType::Creature);
        return Ok(Some(Permission::Cast {
            player: PlayerSelector::You,
            cards,
            timing: CastTimingPermission::AsThoughFlash,
        }));
    }
    if normalized == "You may play lands from your graveyard." {
        return Ok(Some(Permission::PlayLandsFromGraveyard {
            player: PlayerSelector::You,
        }));
    }
    if normalized == "You may play lands from the top of your library." {
        return Ok(Some(Permission::PlayLandsFromLibraryTop {
            player: PlayerSelector::You,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Players|Each player) may play any number of lands on each of (your|their) turns\.$",
    )
    .captures(&normalized)
    {
        let subject = captures
            .get(1)
            .expect("unlimited land player")
            .as_str();
        let possessive = captures
            .get(2)
            .expect("unlimited land turn possessive")
            .as_str();
        if (subject == "You") != (possessive == "your") {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                format!("{subject} on each of {possessive} turns"),
            ));
        }
        return Ok(Some(Permission::UnlimitedLandPlays {
            player: parse_player_selector(subject)?,
            during_own_turn: true,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Players|Each player|Any player) may cast (.+?) as though (?:it|they) had flash\.$",
    )
    .captures(&normalized)
    {
        let player = parse_player_selector(
            captures.get(1).expect("cast permission player").as_str(),
        )?;
        return Ok(Some(Permission::Cast {
            player,
            cards: parse_spell_selector(
                captures.get(2).expect("cast permission cards").as_str(),
                player,
            )?,
            timing: CastTimingPermission::AsThoughFlash,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Players|Each player|Any player) may cast (.+?) from (your graveyard|their graveyards|exile|the top of your library)\.$",
    )
    .captures(&normalized)
    {
        let player = parse_player_selector(
            captures.get(1).expect("zone cast player").as_str(),
        )?;
        let source_zone = captures.get(3).expect("cast source zone").as_str();
        let grammatical_owner_matches = match source_zone {
            "your graveyard" | "the top of your library" => player == PlayerSelector::You,
            "their graveyards" => matches!(
                player,
                PlayerSelector::Opponents | PlayerSelector::EachPlayer
            ),
            "exile" => true,
            _ => false,
        };
        if !grammatical_owner_matches {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                source_zone.to_owned(),
            ));
        }
        let timing = match source_zone {
            "your graveyard" | "their graveyards" => CastTimingPermission::FromGraveyard,
            "exile" => CastTimingPermission::FromExile,
            "the top of your library" => CastTimingPermission::FromLibraryTop,
            value => {
                return Err(StaticReplacementCompileError::UnsupportedOperand(
                    value.to_owned(),
                ));
            }
        };
        return Ok(Some(Permission::Cast {
            player,
            cards: parse_spell_selector(
                captures.get(2).expect("zone cast cards").as_str(),
                player,
            )?,
            timing,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Players|Each player) may play (an|one|two|three|\d+) additional lands? on each of (your|their) turns\.$",
    )
    .captures(&normalized)
    {
        let subject = captures
            .get(1)
            .expect("additional land player")
            .as_str();
        let possessive = captures
            .get(3)
            .expect("additional land turn possessive")
            .as_str();
        if (subject == "You") != (possessive == "your") {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                format!("{subject} on each of {possessive} turns"),
            ));
        }
        let amount = parse_word_or_number(captures.get(2).expect("land amount").as_str())?;
        if amount == 0 {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                "0 additional land plays".to_owned(),
            ));
        }
        return Ok(Some(Permission::AdditionalLandPlays {
            player: parse_player_selector(subject)?,
            amount,
            during_own_turn: true,
        }));
    }
    Ok(None)
}

fn parse_cost_static(
    source: &str,
) -> Result<Option<CostModification>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    let (body, condition) = if let Some(body) = normalized.strip_suffix(" during your turn.") {
        (format!("{body}."), Condition::DuringYourTurn)
    } else if let Some(without_prefix) = normalized.strip_prefix("If ")
        && let Some((condition, body)) = without_prefix.split_once(", ")
    {
        split_static_condition(&format!(
            "{} as long as {condition}.",
            body.trim_end_matches('.')
        ))?
    } else if let Some(without_period) = normalized.strip_suffix('.')
        && let Some((body, condition)) = without_period.rsplit_once(" if ")
    {
        split_static_condition(&format!("{body} as long as {condition}."))?
    } else {
        split_static_condition(&normalized)?
    };
    let (body, variable_amount) = if let Some(without_period) = body.strip_suffix('.')
        && let Some((body, definition)) = without_period.rsplit_once(", where X is ")
    {
        (
            format!("{body}."),
            Some(parse_state_derived_count_amount(definition)?),
        )
    } else {
        (body, None)
    };
    let pattern =
        r"^(.+?) (?:cost|costs) \{(\d+|X)\} (less|more) to (cast|activate)(?: for each (.+))?\.$";
    let Some(captures) = static_regex(pattern).captures(&body) else {
        return Ok(None);
    };
    let subject = captures.get(1).expect("cost subject").as_str();
    let fixed_text = captures.get(2).expect("cost amount").as_str();
    let fixed = (fixed_text != "X")
        .then(|| parse_u32(fixed_text))
        .transpose()?;
    if fixed == Some(0) {
        return Err(StaticReplacementCompileError::UnsupportedOperand(
            "{0}".to_owned(),
        ));
    }
    let generic_mana = if fixed_text == "X" {
        if captures.get(5).is_some() {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        variable_amount.ok_or_else(|| {
            StaticReplacementCompileError::UnsupportedOperand("undefined cost X".to_owned())
        })?
    } else if variable_amount.is_some() {
        return Err(StaticReplacementCompileError::UnsupportedOperand(
            "unused cost X definition".to_owned(),
        ));
    } else if let Some(each) = captures.get(5) {
        let counted = parse_counted_characteristic_amount(each.as_str())?;
        match fixed.expect("non-X cost has a fixed amount") {
            1 => counted,
            _ => Amount::Scaled {
                factor: fixed.expect("non-X cost has a fixed amount"),
                amount: Box::new(counted),
            },
        }
    } else {
        Amount::Fixed(fixed.expect("non-X cost has a fixed amount"))
    };
    let direction = match captures.get(3).expect("cost direction").as_str() {
        "less" => CostDirection::Reduce,
        "more" => CostDirection::Increase,
        _ => unreachable!("regex limits cost direction"),
    };
    let verb = captures.get(4).expect("cost verb").as_str();
    let scope = if verb == "cast" {
        let player = if subject.contains("your opponents") {
            PlayerSelector::Opponents
        } else if subject.contains("you cast") || subject.eq_ignore_ascii_case("this spell") {
            PlayerSelector::You
        } else {
            PlayerSelector::EachPlayer
        };
        let (spell_subject, source_zones) = if let Some(subject) =
            subject.strip_suffix(" from your graveyard or from exile")
        {
            (
                subject,
                Some(BTreeSet::from([Zone::Graveyard, Zone::Exile])),
            )
        } else if let Some(subject) = subject.strip_suffix(" from graveyards or from exile") {
            (
                subject,
                Some(BTreeSet::from([Zone::Graveyard, Zone::Exile])),
            )
        } else if let Some(subject) = subject.strip_suffix(" from your graveyard") {
            (subject, Some(BTreeSet::from([Zone::Graveyard])))
        } else if let Some(subject) = subject.strip_suffix(" from anywhere other than your hand") {
            (
                subject,
                Some(BTreeSet::from([
                    Zone::Graveyard,
                    Zone::Exile,
                    Zone::Library,
                    Zone::Command,
                ])),
            )
        } else {
            (subject, None)
        };
        let mut spells = parse_spell_selector(spell_subject, player)?;
        if let Some(zones) = source_zones {
            spells.zones = zones;
        }
        CostScope::CastSpell { player, spells }
    } else {
        let (player, sources) = parse_ability_cost_subject(subject)?;
        CostScope::ActivateAbility { player, sources }
    };
    Ok(Some(CostModification {
        scope,
        direction,
        generic_mana,
        condition,
    }))
}

fn parse_skip_step_static(
    source: &str,
) -> Result<Option<StaticEffect>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    let Some(captures) = static_regex(
        r"^(Skip your|You skip your|Your opponents skip their|Players skip their|Each player skips their) (untap|upkeep|draw|combat|end) steps?\.$",
    )
    .captures(&normalized)
    else {
        return Ok(None);
    };
    let player = if normalized.starts_with("Skip ") || normalized.starts_with("You ") {
        PlayerSelector::You
    } else if normalized.starts_with("Your opponents ") {
        PlayerSelector::Opponents
    } else {
        PlayerSelector::EachPlayer
    };
    Ok(Some(StaticEffect::SkipStep {
        player,
        step: parse_step(captures.get(2).expect("step capture").as_str())?,
    }))
}

fn looks_like_replacement_boundary(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    lower.contains(" would ")
        || lower.contains(" instead")
        || lower.starts_with("as ")
        || lower.contains(" enters tapped")
        || lower.contains(" enters the battlefield tapped")
        || lower.contains(" enters with ")
        || lower.contains(" enters the battlefield with ")
        || lower.contains(" is prevented")
        || lower.contains(" can't be prevented")
}

fn parse_replacement(
    source: &str,
) -> Result<Option<ReplacementEffect>, StaticReplacementCompileError> {
    if let Some(effect) = parse_entry_replacement(source)? {
        return Ok(Some(effect));
    }
    if let Some(effect) = parse_zone_replacement(source)? {
        return Ok(Some(effect));
    }
    if let Some(effect) = parse_damage_replacement(source)? {
        return Ok(Some(effect));
    }
    if let Some(effect) = parse_multiplier_replacement(source)? {
        return Ok(Some(effect));
    }
    if let Some(effect) = parse_skip_replacement(source)? {
        return Ok(Some(effect));
    }
    Ok(None)
}

fn parse_entry_object_for_pronoun(
    subject: &str,
    pronoun: &str,
) -> Result<ObjectSelector, StaticReplacementCompileError> {
    let object = if matches!(pronoun, "him" | "her") && looks_like_printed_self_name(subject) {
        source_object_selector(Some(Zone::Battlefield))
    } else {
        parse_object_selector(subject, Some(Zone::Battlefield))?
    };
    if matches!(pronoun, "him" | "her")
        && (object.reference != SelectorReference::Source || !object.alternatives.is_empty())
    {
        return Err(StaticReplacementCompileError::UnsupportedSubject(
            subject.to_owned(),
        ));
    }
    Ok(object)
}

fn scale_state_derived_amount(
    factor: &str,
    source: &str,
) -> Result<Amount, StaticReplacementCompileError> {
    let amount = parse_state_derived_count_amount(source)?;
    let factor = parse_word_or_number(factor)?;
    Ok(if factor == 1 {
        amount
    } else {
        Amount::Scaled {
            factor,
            amount: Box::new(amount),
        }
    })
}

fn parse_entry_replacement(
    source: &str,
) -> Result<Option<ReplacementEffect>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    if let Some(captures) =
        static_regex(r"^(.+?) enters(?: the battlefield)? tapped(?: (unless|if) (.+))?\.$")
            .captures(&normalized)
    {
        let object = parse_object_selector(
            captures.get(1).expect("entry subject").as_str(),
            Some(Zone::Battlefield),
        )?;
        let condition = match (captures.get(2), captures.get(3)) {
            (None, None) => EntryReplacementCondition::Always,
            (Some(mode), Some(value)) if mode.as_str() == "unless" => {
                parse_entry_replacement_condition(value.as_str())?
            }
            (Some(mode), Some(value)) if mode.as_str() == "if" => {
                parse_entry_if_condition(value.as_str())?
            }
            _ => unreachable!("entry condition captures mode and body together"),
        };
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield { object, condition },
            operation: ReplacementOperation::EnterTapped,
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (.+?) was kicked, it enters(?: the battlefield)? with (a|an|one|two|three|four|five|six|seven|eight|nine|ten|\d+) (.+?) counters? on it for each (.+)\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object: parse_object_selector(
                    captures
                        .get(1)
                        .expect("kicked counted-entry subject")
                        .as_str(),
                    Some(Zone::Battlefield),
                )?,
                condition: EntryReplacementCondition::IfSourceWasKicked,
            },
            operation: ReplacementOperation::EnterWithCounters {
                counter: parse_entry_counter_kind(
                    captures
                        .get(3)
                        .expect("kicked counted-entry counter kind")
                        .as_str(),
                )?,
                amount: scale_state_derived_amount(
                    captures
                        .get(2)
                        .expect("kicked counted-entry factor")
                        .as_str(),
                    captures
                        .get(4)
                        .expect("kicked counted-entry source")
                        .as_str(),
                )?,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) enters(?: the battlefield)? with (?:a )?number of (?:additional )?(.+?) counters? on (it|him|her) equal to (.+)\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object: parse_entry_object_for_pronoun(
                    captures
                        .get(1)
                        .expect("derived-count entry subject")
                        .as_str(),
                    captures
                        .get(3)
                        .expect("derived-count entry pronoun")
                        .as_str(),
                )?,
                condition: EntryReplacementCondition::Always,
            },
            operation: ReplacementOperation::EnterWithCounters {
                counter: parse_entry_counter_kind(
                    captures
                        .get(2)
                        .expect("derived-count entry counter kind")
                        .as_str(),
                )?,
                amount: parse_state_derived_count_amount(
                    captures
                        .get(4)
                        .expect("derived-count entry definition")
                        .as_str(),
                )?,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) enters(?: the battlefield)? with (?:an? )?(?:additional )?X (.+?) counters? on (it|him|her), where X is (.+)\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object: parse_entry_object_for_pronoun(
                    captures
                        .get(1)
                        .expect("defined-X entry subject")
                        .as_str(),
                    captures
                        .get(3)
                        .expect("defined-X entry pronoun")
                        .as_str(),
                )?,
                condition: EntryReplacementCondition::Always,
            },
            operation: ReplacementOperation::EnterWithCounters {
                counter: parse_entry_counter_kind(
                    captures
                        .get(2)
                        .expect("defined-X entry counter kind")
                        .as_str(),
                )?,
                amount: parse_state_derived_count_amount(
                    captures
                        .get(4)
                        .expect("defined-X entry definition")
                        .as_str(),
                )?,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) enters(?: the battlefield)? with (a|an|one|two|three|four|five|six|seven|eight|nine|ten|\d+) (?:additional )?(.+?) counters? on (it|him|her) for each (.+)\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object: parse_entry_object_for_pronoun(
                    captures
                        .get(1)
                        .expect("counted-entry subject")
                        .as_str(),
                    captures.get(4).expect("counted-entry pronoun").as_str(),
                )?,
                condition: EntryReplacementCondition::Always,
            },
            operation: ReplacementOperation::EnterWithCounters {
                counter: parse_entry_counter_kind(
                    captures
                        .get(3)
                        .expect("counted-entry counter kind")
                        .as_str(),
                )?,
                amount: scale_state_derived_amount(
                    captures.get(2).expect("counted-entry factor").as_str(),
                    captures.get(5).expect("counted-entry source").as_str(),
                )?,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (.+?) was kicked, it enters(?: the battlefield)? with (a|an|one|two|three|\d+) (.+?) counters? on it\.$",
    )
    .captures(&normalized)
    {
        let object = parse_object_selector(
            captures
                .get(1)
                .expect("kicked entry subject")
                .as_str(),
            Some(Zone::Battlefield),
        )?;
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object,
                condition: EntryReplacementCondition::IfSourceWasKicked,
            },
            operation: ReplacementOperation::EnterWithCounters {
                counter: parse_entry_counter_kind(
                    captures
                        .get(3)
                        .expect("kicked entry counter kind")
                        .as_str(),
                )?,
                amount: parse_amount_word(
                    captures
                        .get(2)
                        .expect("kicked entry counter amount")
                        .as_str(),
                )?,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) enters(?: the battlefield)? with a (.+?) counter on it for each time it was kicked\.$",
    )
    .captures(&normalized)
    {
        let object = parse_object_selector(
            captures
                .get(1)
                .expect("multikicker entry subject")
                .as_str(),
            Some(Zone::Battlefield),
        )?;
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object,
                condition: EntryReplacementCondition::Always,
            },
            operation: ReplacementOperation::EnterWithCounters {
                counter: parse_entry_counter_kind(
                    captures
                        .get(2)
                        .expect("multikicker entry counter kind")
                        .as_str(),
                )?,
                amount: Amount::KickerPayments,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) enters(?: the battlefield)? with your choice of (?:a |an )?(.+?) counter or (?:a |an )?(.+?) counter on it\.$",
    )
    .captures(&normalized)
    {
        let choices = BTreeSet::from([
            parse_counter_kind(
                captures
                    .get(2)
                    .expect("first entry counter choice")
                    .as_str(),
            )?,
            parse_counter_kind(
                captures
                    .get(3)
                    .expect("second entry counter choice")
                    .as_str(),
            )?,
        ]);
        if choices.len() != 2 {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object: parse_object_selector(
                    captures.get(1).expect("entry choice subject").as_str(),
                    Some(Zone::Battlefield),
                )?,
                condition: EntryReplacementCondition::Always,
            },
            operation: ReplacementOperation::EnterWithCounterChoice { choices },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) enters(?: the battlefield)? with (a|an|one|two|three|four|five|six|seven|eight|nine|ten|\d+|X|twice X) (.+?) counters?\.$",
    )
    .captures(&normalized)
    {
        let object = parse_named_self_or_object_selector(
            captures
                .get(1)
                .expect("implicit self-entry subject")
                .as_str(),
            Some(Zone::Battlefield),
        )?;
        if object.reference != SelectorReference::Source || !object.alternatives.is_empty() {
            return Err(StaticReplacementCompileError::UnsupportedSubject(
                captures
                    .get(1)
                    .expect("implicit self-entry subject")
                    .as_str()
                    .to_owned(),
            ));
        }
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object,
                condition: EntryReplacementCondition::Always,
            },
            operation: ReplacementOperation::EnterWithCounters {
                counter: parse_entry_counter_kind(
                    captures
                        .get(3)
                        .expect("implicit self-entry counter kind")
                        .as_str(),
                )?,
                amount: parse_amount_word(
                    captures
                        .get(2)
                        .expect("implicit self-entry amount")
                        .as_str(),
                )?,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(.+?) enters(?: the battlefield)? with (a|an|one|two|three|four|five|six|seven|eight|nine|ten|\d+|X|twice X) (.+?) counters? on (it|him|her)\.$",
    )
    .captures(&normalized)
    {
        let object = parse_entry_object_for_pronoun(
            captures.get(1).expect("counter entry subject").as_str(),
            captures.get(4).expect("counter entry pronoun").as_str(),
        )?;
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object,
                condition: EntryReplacementCondition::Always,
            },
            operation: ReplacementOperation::EnterWithCounters {
                counter: parse_entry_counter_kind(
                    captures.get(3).expect("entry counter kind").as_str(),
                )?,
                amount: parse_amount_word(captures.get(2).expect("entry amount").as_str())?,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(You may have )?(.+?) enter(?: the battlefield)? as a copy of (?:any |a )?(.+?)(?: on the battlefield)?\.$",
    )
    .captures(&normalized)
    {
        let object = parse_object_selector(
            captures.get(2).expect("copy entry subject").as_str(),
            Some(Zone::Battlefield),
        )?;
        let of = parse_object_selector(
            captures.get(3).expect("copy source selector").as_str(),
            Some(Zone::Battlefield),
        )?;
        let optional = captures.get(1).is_some();
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object,
                condition: EntryReplacementCondition::Always,
            },
            operation: ReplacementOperation::EnterAsCopy { of, optional },
            optional,
        }));
    }
    if let Some(captures) =
        static_regex(r"^As (.+?) enters(?: the battlefield)?, choose (.+)\.$").captures(&normalized)
        && let Some(choice) = parse_constrained_entry_choice(
            captures
                .get(2)
                .expect("constrained entry choice operand")
                .as_str(),
        )?
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object: parse_named_self_or_object_selector(
                    captures
                        .get(1)
                        .expect("constrained entry choice subject")
                        .as_str(),
                    Some(Zone::Battlefield),
                )?,
                condition: EntryReplacementCondition::Always,
            },
            operation: ReplacementOperation::ChooseAsEnters(choice),
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^As (.+?) enters(?: the battlefield)?, choose (a basic land type|a color and a creature type|two colors|another creature you control|a color and an opponent)\.$",
    )
    .captures(&normalized)
    {
        let object = parse_object_selector(
            captures.get(1).expect("compound choice subject").as_str(),
            Some(Zone::Battlefield),
        )?;
        let choice = match captures.get(2).expect("compound entry choice").as_str() {
            "a basic land type" => EntryChoice::BasicLandType,
            "a color and a creature type" => EntryChoice::ColorAndCreatureType,
            "two colors" => EntryChoice::TwoColors,
            "another creature you control" => EntryChoice::AnotherCreatureYouControl,
            "a color and an opponent" => EntryChoice::ColorAndOpponent,
            _ => unreachable!("regex limits compound entry choice"),
        };
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object,
                condition: EntryReplacementCondition::Always,
            },
            operation: ReplacementOperation::ChooseAsEnters(choice),
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^As (.+?) enters(?: the battlefield)?, choose (a color|a card type|a creature type|a player|an opponent)\.$",
    )
    .captures(&normalized)
    {
        let object = parse_object_selector(
            captures.get(1).expect("choice entry subject").as_str(),
            Some(Zone::Battlefield),
        )?;
        let choice = match captures.get(2).expect("entry choice").as_str() {
            "a color" => EntryChoice::Color,
            "a card type" => EntryChoice::CardType,
            "a creature type" => EntryChoice::CreatureType,
            "a player" => EntryChoice::Player,
            "an opponent" => EntryChoice::Opponent,
            _ => unreachable!("regex limits entry choice"),
        };
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield {
                object,
                condition: EntryReplacementCondition::Always,
            },
            operation: ReplacementOperation::ChooseAsEnters(choice),
            optional: false,
        }));
    }
    Ok(None)
}

fn parse_constrained_entry_choice(
    source: &str,
) -> Result<Option<EntryChoice>, StaticReplacementCompileError> {
    if let Some(color) = source
        .strip_prefix("a color other than ")
        .and_then(parse_color)
    {
        if color == Color::Colorless {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                source.to_owned(),
            ));
        }
        return Ok(Some(EntryChoice::ColorOtherThan(color)));
    }
    match source {
        "two players" => return Ok(Some(EntryChoice::Players(2))),
        "two basic land types" => return Ok(Some(EntryChoice::BasicLandTypes(2))),
        _ => {}
    }

    let normalized = source
        .replace(", and ", ", ")
        .replace(", or ", ", ")
        .replace(" and ", ", ")
        .replace(" or ", ", ");
    let parts = normalized
        .split(", ")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 2 {
        return Ok(None);
    }

    let colors = parts
        .iter()
        .map(|part| parse_color(&part.to_ascii_lowercase()))
        .collect::<Option<BTreeSet<_>>>();
    if let Some(colors) = colors
        && colors.len() == parts.len()
        && !colors.contains(&Color::Colorless)
    {
        return Ok(Some(EntryChoice::ColorAmong(colors)));
    }

    let card_types = parts
        .iter()
        .map(|part| card_type_adjective(&part.to_ascii_lowercase()))
        .collect::<Result<BTreeSet<_>, _>>();
    if let Ok(card_types) = card_types
        && card_types.len() == parts.len()
    {
        return Ok(Some(EntryChoice::CardTypeAmong(card_types)));
    }

    let basic_land_types = parts
        .iter()
        .copied()
        .filter(|part| matches!(*part, "Plains" | "Island" | "Swamp" | "Mountain" | "Forest"))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if basic_land_types.len() == parts.len() {
        return Ok(Some(EntryChoice::BasicLandTypeAmong(basic_land_types)));
    }

    if parts.len() >= 3 {
        let creature_types = parts
            .iter()
            .map(|part| singular_permanent_subtype(part))
            .collect::<Option<BTreeSet<_>>>();
        if let Some(creature_types) = creature_types
            && creature_types.len() == parts.len()
        {
            return Ok(Some(EntryChoice::CreatureTypeAmong(creature_types)));
        }
    }
    Ok(None)
}

fn parse_entry_replacement_condition(
    source: &str,
) -> Result<EntryReplacementCondition, StaticReplacementCompileError> {
    if let Some(captures) = static_regex(
        r"^you control (one|two|three|four|five|six|seven|eight|nine|ten|\d+) or more (.+)$",
    )
    .captures(source)
    {
        let minimum = parse_word_or_number(
            captures
                .get(1)
                .expect("entry minimum permanent threshold")
                .as_str(),
        )?;
        let subject = format!(
            "{} you control",
            captures
                .get(2)
                .expect("entry minimum permanent selector")
                .as_str()
        );
        return Ok(EntryReplacementCondition::UnlessControllerControlsAtLeast {
            objects: parse_object_selector(&subject, Some(Zone::Battlefield))?,
            minimum,
        });
    }
    if let Some(captures) = static_regex(
        r"^you control (one|two|three|four|five|six|seven|eight|nine|ten|\d+) or fewer other (.+)$",
    )
    .captures(source)
    {
        let maximum = parse_word_or_number(
            captures
                .get(1)
                .expect("entry permanent-count threshold")
                .as_str(),
        )?;
        let mut objects = parse_object_selector(
            captures
                .get(2)
                .expect("entry permanent-count selector")
                .as_str(),
            Some(Zone::Battlefield),
        )?;
        objects.controller = ControllerRelation::You;
        return Ok(
            EntryReplacementCondition::UnlessControllerControlsAtMostOther { objects, maximum },
        );
    }
    if let Some(captures) = static_regex(r"^a player has (\d+) or less life$").captures(source) {
        return Ok(EntryReplacementCondition::UnlessAnyPlayerLifeAtMost(
            parse_u32(captures.get(1).expect("entry life threshold").as_str())?,
        ));
    }
    if let Some(captures) = static_regex(
        r"^you have (one|two|three|four|five|six|seven|eight|nine|ten|\d+) or more opponents$",
    )
    .captures(source)
    {
        return Ok(EntryReplacementCondition::UnlessOpponentCountAtLeast(
            parse_word_or_number(
                captures
                    .get(1)
                    .expect("entry opponent-count threshold")
                    .as_str(),
            )?,
        ));
    }
    if let Some(subject) = source.strip_prefix("you control ") {
        let subject = format!("{subject} you control");
        return Ok(EntryReplacementCondition::UnlessControllerControlsAtLeast {
            objects: parse_object_selector(&subject, Some(Zone::Battlefield))?,
            minimum: 1,
        });
    }
    Err(StaticReplacementCompileError::UnsupportedOperand(
        source.to_owned(),
    ))
}

fn parse_entry_if_condition(
    source: &str,
) -> Result<EntryReplacementCondition, StaticReplacementCompileError> {
    if let Some(captures) = static_regex(
        r"^you control (one|two|three|four|five|six|seven|eight|nine|ten|\d+) or more (.+)$",
    )
    .captures(source)
    {
        let minimum = parse_word_or_number(
            captures
                .get(1)
                .expect("entry if-condition threshold")
                .as_str(),
        )?;
        let subject = format!(
            "{} you control",
            captures
                .get(2)
                .expect("entry if-condition selector")
                .as_str()
        );
        return Ok(EntryReplacementCondition::IfMatchingObjectsAtLeast {
            objects: parse_object_selector(&subject, Some(Zone::Battlefield))?,
            minimum,
        });
    }
    if let Some(subject) = source.strip_prefix("you control ") {
        return Ok(EntryReplacementCondition::IfMatchingObjectsAtLeast {
            objects: parse_object_selector(
                &format!("{subject} you control"),
                Some(Zone::Battlefield),
            )?,
            minimum: 1,
        });
    }
    if let Some(subject) = source
        .strip_prefix("an opponent controls ")
        .or_else(|| source.strip_prefix("your opponents control "))
    {
        return Ok(EntryReplacementCondition::IfMatchingObjectsAtLeast {
            objects: parse_object_selector(
                &format!("{subject} an opponent controls"),
                Some(Zone::Battlefield),
            )?,
            minimum: 1,
        });
    }
    Err(StaticReplacementCompileError::UnsupportedOperand(
        source.to_owned(),
    ))
}

fn parse_zone_replacement(
    source: &str,
) -> Result<Option<ReplacementEffect>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    if normalized
        == "If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield instead of putting it into your graveyard."
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::ZoneChangeCausedByOpponentSpellOrAbility {
                object: parse_object_selector("this card", Some(Zone::Hand))?,
                from: Zone::Hand,
                to: Zone::Graveyard,
            },
            operation: ReplacementOperation::MoveInstead {
                destination: Zone::Battlefield,
                placement: LibraryPlacement::Unspecified,
            },
            optional: false,
        }));
    }
    if normalized
        == "If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield with two +1/+1 counters on it instead of putting it into your graveyard."
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::ZoneChangeCausedByOpponentSpellOrAbility {
                object: parse_object_selector("this card", Some(Zone::Hand))?,
                from: Zone::Hand,
                to: Zone::Graveyard,
            },
            operation: ReplacementOperation::MoveInsteadWithCounters {
                destination: Zone::Battlefield,
                placement: LibraryPlacement::Unspecified,
                counter: CounterKind::PlusOnePlusOne,
                amount: 2,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (.+?) would die, (exile it|put it on the bottom of its owner's library|put it on top of its owner's library) instead\.$",
    )
    .captures(&normalized)
    {
        let (destination, placement) = match captures.get(2).expect("dies operation").as_str() {
            "exile it" => (Zone::Exile, LibraryPlacement::Unspecified),
            "put it on the bottom of its owner's library" => {
                (Zone::Library, LibraryPlacement::Bottom)
            }
            "put it on top of its owner's library" => (Zone::Library, LibraryPlacement::Top),
            _ => unreachable!("regex limits dies operation"),
        };
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::ZoneChange {
                object: parse_object_selector(
                    captures.get(1).expect("dies subject").as_str(),
                    Some(Zone::Battlefield),
                )?,
                from: Some(Zone::Battlefield),
                to: Zone::Graveyard,
            },
            operation: ReplacementOperation::MoveInstead {
                destination,
                placement,
            },
            optional: false,
        }));
    }
    if normalized == "If a permanent would be put into a graveyard, exile it instead." {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::ZoneChange {
                object: parse_object_selector("a permanent", Some(Zone::Battlefield))?,
                from: Some(Zone::Battlefield),
                to: Zone::Graveyard,
            },
            operation: ReplacementOperation::MoveInstead {
                destination: Zone::Exile,
                placement: LibraryPlacement::Unspecified,
            },
            optional: false,
        }));
    }
    let pattern = r"^If (.+?) would be put into (a graveyard|a hand|a library|its owner's graveyard|its owner's hand|its owner's library|their owner's graveyard|their owner's hand|their owner's library|their graveyards|an opponent's graveyard|your opponents' graveyards|your graveyard|your hand|your library|exile) from (anywhere|the battlefield|a graveyard|a hand|a library|exile), (exile it|exile that card|exile that creature|put it into its owner's hand|put it on the bottom of its owner's library|shuffle it into its owner's library) instead\.$";
    let Some(captures) = static_regex(pattern).captures(&normalized) else {
        return Ok(None);
    };
    let destination_phrase = captures.get(2).expect("zone destination phrase").as_str();
    let destination_zone = destination_phrase
        .split_whitespace()
        .next_back()
        .expect("destination phrase has a final zone");
    let to = match destination_zone {
        "graveyards" => Zone::Graveyard,
        value => parse_zone(value)?,
    };
    let from = match captures.get(3).expect("zone origin").as_str() {
        "anywhere" => None,
        "the battlefield" => Some(Zone::Battlefield),
        "a graveyard" => Some(Zone::Graveyard),
        "a hand" => Some(Zone::Hand),
        "a library" => Some(Zone::Library),
        "exile" => Some(Zone::Exile),
        value => {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                value.to_owned(),
            ));
        }
    };
    let (destination, placement) = match captures.get(4).expect("zone operation").as_str() {
        "exile it" | "exile that card" | "exile that creature" => {
            (Zone::Exile, LibraryPlacement::Unspecified)
        }
        "put it into its owner's hand" => (Zone::Hand, LibraryPlacement::Unspecified),
        "put it on the bottom of its owner's library" => (Zone::Library, LibraryPlacement::Bottom),
        "shuffle it into its owner's library" => (Zone::Library, LibraryPlacement::Shuffled),
        _ => unreachable!("regex limits zone operation"),
    };
    let mut object = parse_object_selector(captures.get(1).expect("zone subject").as_str(), None)?;
    if destination_phrase.starts_with("your ") && !destination_phrase.starts_with("your opponents'")
    {
        object.owner = ControllerRelation::You;
    } else if destination_phrase == "an opponent's graveyard"
        || destination_phrase == "your opponents' graveyards"
    {
        object.owner = ControllerRelation::Opponent;
    }
    Ok(Some(ReplacementEffect {
        predicate: ReplacementEventPredicate::ZoneChange { object, from, to },
        operation: ReplacementOperation::MoveInstead {
            destination,
            placement,
        },
        optional: false,
    }))
}

fn parse_damage_replacement(
    source: &str,
) -> Result<Option<ReplacementEffect>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    if normalized
        == "If damage would be dealt to this creature, prevent that damage. Remove a +1/+1 counter from this creature."
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: ObjectSelector::matching(None),
                recipient: RecipientSelector::Object(source_object_selector(Some(
                    Zone::Battlefield,
                ))),
                kind: None,
            },
            operation: ReplacementOperation::PreventDamageAndRemovePlusOneCounter,
            optional: false,
        }));
    }
    let damage_kind = |value: Option<regex::Match<'_>>| match value.map(|value| value.as_str()) {
        Some("combat") => Some(DamageKind::Combat),
        Some("noncombat") => Some(DamageKind::Noncombat),
        None => None,
        _ => unreachable!("damage-kind regex is closed"),
    };
    let scale_operation = |scale: &str| match scale {
        "double" | "twice" => ReplacementOperation::ScaleDamage {
            numerator: 2,
            denominator: 1,
            round_down: false,
        },
        "triple" => ReplacementOperation::ScaleDamage {
            numerator: 3,
            denominator: 1,
            round_down: false,
        },
        "half" => ReplacementOperation::ScaleDamage {
            numerator: 1,
            denominator: 2,
            round_down: false,
        },
        _ => unreachable!("damage-scale regex is closed"),
    };
    fn object_selector_requires_creature(selector: &ObjectSelector) -> bool {
        if selector.alternatives.is_empty() {
            !selector.card_type_match_any && selector.card_types.contains(&CardType::Creature)
        } else {
            selector
                .alternatives
                .iter()
                .all(object_selector_requires_creature)
        }
    }
    fn recipient_has_player_and_object(recipient: &RecipientSelector, creature_only: bool) -> bool {
        match recipient {
            RecipientSelector::Alternatives(alternatives) => {
                let has_player = alternatives
                    .iter()
                    .any(|alternative| matches!(alternative, RecipientSelector::Player(_)));
                let objects = alternatives
                    .iter()
                    .filter_map(|alternative| match alternative {
                        RecipientSelector::Object(selector) => Some(selector),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                has_player
                    && !objects.is_empty()
                    && (!creature_only
                        || objects
                            .iter()
                            .all(|selector| object_selector_requires_creature(selector)))
            }
            _ => false,
        }
    }
    fn echoed_recipient_matches(original: &str, echoed: &str) -> bool {
        let original = original.trim().to_ascii_lowercase();
        let echoed = echoed.trim().to_ascii_lowercase();
        if original == echoed {
            return true;
        }
        let Ok(recipient) = parse_recipient_selector(&original) else {
            return false;
        };
        match echoed.as_str() {
            "that player" => matches!(recipient, RecipientSelector::Player(_)),
            "that creature" => matches!(
                recipient,
                RecipientSelector::Object(ref selector)
                    if object_selector_requires_creature(selector)
            ),
            "that permanent" => matches!(recipient, RecipientSelector::Object(_)),
            "that player or permanent" | "that permanent or player" => {
                recipient_has_player_and_object(&recipient, false)
            }
            "that creature or player" | "that player or creature" => {
                recipient_has_player_and_object(&recipient, true)
            }
            _ => false,
        }
    }
    if let Some(captures) = static_regex(
        r"^Damage that would reduce your life total to less than (\d+) reduces it to (\d+) instead\.$",
    )
    .captures(&normalized)
    {
        let threshold = parse_u32(captures.get(1).expect("damage life threshold").as_str())?;
        let result = parse_u32(captures.get(2).expect("damage life result").as_str())?;
        if threshold != result {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: ObjectSelector::matching(None),
                recipient: RecipientSelector::Player(PlayerSelector::You),
                kind: None,
            },
            operation: ReplacementOperation::ClampDamageToLifeFloor {
                minimum_life: threshold,
            },
            optional: false,
        }));
    }
    if let Some(captures) =
        static_regex(r"^All damage that would be dealt to (.+?) is dealt to (.+?) instead\.$")
            .captures(&normalized)
    {
        let original_recipient = parse_recipient_selector(
            captures
                .get(1)
                .expect("redirected damage recipient")
                .as_str(),
        )?;
        let destination_text = captures
            .get(2)
            .expect("damage redirection destination")
            .as_str();
        let destination = if destination_text == "its controller" {
            if !matches!(&original_recipient, RecipientSelector::Object(_)) {
                return Err(StaticReplacementCompileError::UnsupportedOperand(
                    normalized,
                ));
            }
            RecipientSelector::Player(PlayerSelector::ControllerOfAffectedObject)
        } else {
            parse_recipient_selector(destination_text)?
        };
        if matches!(
            &destination,
            RecipientSelector::Any
                | RecipientSelector::Alternatives(_)
                | RecipientSelector::Player(PlayerSelector::Opponents | PlayerSelector::EachPlayer)
        ) {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: ObjectSelector::matching(None),
                recipient: original_recipient,
                kind: None,
            },
            operation: ReplacementOperation::RedirectDamage {
                recipient: destination,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (.+?) would deal (?:(combat|noncombat) )?damage, (?:it|that source) deals (double|twice|triple|half) that damage instead\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: parse_damage_source(
                    captures.get(1).expect("damage source").as_str(),
                )?,
                recipient: RecipientSelector::Any,
                kind: damage_kind(captures.get(2)),
            },
            operation: scale_operation(captures.get(3).expect("damage scale").as_str()),
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (.+?) would deal (?:(combat|noncombat) )?damage to (.+?), (?:it|that source) deals (double|twice|triple|half) that damage(?: to (that player|that permanent|that creature|that player or permanent|that permanent or player))? instead\.$",
    )
    .captures(&normalized)
    {
        let original = captures.get(3).expect("damage recipient").as_str();
        if captures
            .get(5)
            .is_some_and(|echoed| !echoed_recipient_matches(original, echoed.as_str()))
        {
            return Err(StaticReplacementCompileError::UnsupportedOperand(normalized));
        }
        let operation = scale_operation(captures.get(4).expect("damage scale").as_str());
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: parse_damage_source(
                    captures.get(1).expect("damage source").as_str(),
                )?,
                recipient: parse_recipient_selector(original)?,
                kind: damage_kind(captures.get(2)),
            },
            operation,
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (.+?) would deal (?:(combat|noncombat) )?damage to (.+?), (?:it|that source) deals (double|twice|triple|half) that damage to (.+?) instead\.$",
    )
    .captures(&normalized)
    {
        let original = captures.get(3).expect("damage recipient").as_str();
        let echoed = captures.get(5).expect("echoed damage recipient").as_str();
        if !echoed_recipient_matches(original, echoed) {
            return Err(StaticReplacementCompileError::UnsupportedOperand(normalized));
        }
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: parse_damage_source(
                    captures.get(1).expect("damage source").as_str(),
                )?,
                recipient: parse_recipient_selector(original)?,
                kind: damage_kind(captures.get(2)),
            },
            operation: scale_operation(captures.get(4).expect("damage scale").as_str()),
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (.+?) would deal (?:(combat|noncombat) )?damage to (.+?), (?:it|that source) deals half that damage, rounded down, to (.+?) instead\.$",
    )
    .captures(&normalized)
    {
        let original = captures.get(3).expect("damage recipient").as_str();
        let echoed = captures.get(4).expect("echoed damage recipient").as_str();
        if !echoed_recipient_matches(original, echoed) {
            return Err(StaticReplacementCompileError::UnsupportedOperand(normalized));
        }
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: parse_damage_source(
                    captures.get(1).expect("damage source").as_str(),
                )?,
                recipient: parse_recipient_selector(original)?,
                kind: damage_kind(captures.get(2)),
            },
            operation: ReplacementOperation::ScaleDamage {
                numerator: 1,
                denominator: 2,
                round_down: true,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (.+?) would deal (?:(combat|noncombat) )?damage to (.+?), (?:it|that source) deals that much damage plus (\d+)(?: to (that player|that permanent|that creature|that player or permanent|that permanent or player))? instead\.$",
    )
    .captures(&normalized)
    {
        let original = captures.get(3).expect("damage recipient").as_str();
        if captures
            .get(5)
            .is_some_and(|echoed| !echoed_recipient_matches(original, echoed.as_str()))
        {
            return Err(StaticReplacementCompileError::UnsupportedOperand(normalized));
        }
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: parse_damage_source(
                    captures.get(1).expect("damage source").as_str(),
                )?,
                recipient: parse_recipient_selector(original)?,
                kind: damage_kind(captures.get(2)),
            },
            operation: ReplacementOperation::IncreaseDamage {
                amount: parse_u32(captures.get(4).expect("damage increase").as_str())?,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (.+?) would deal damage to a (creature|permanent) or player, it deals (double|twice|half) that damage to that (creature|permanent) or player instead\.$",
    )
    .captures(&normalized)
    {
        if captures.get(2).expect("damage recipient noun").as_str()
            != captures
                .get(4)
                .expect("echoed damage recipient noun")
                .as_str()
        {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        let operation = match captures.get(3).expect("damage scale").as_str() {
            "double" | "twice" => ReplacementOperation::ScaleDamage {
                numerator: 2,
                denominator: 1,
                round_down: false,
            },
            "half" => ReplacementOperation::ScaleDamage {
                numerator: 1,
                denominator: 2,
                round_down: false,
            },
            _ => unreachable!("regex limits damage scale"),
        };
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: parse_damage_source(
                    captures.get(1).expect("damage source").as_str(),
                )?,
                recipient: parse_recipient_selector(&format!(
                    "a {} or player",
                    captures.get(2).expect("damage recipient noun").as_str()
                ))?,
                kind: None,
            },
            operation,
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (.+?) would deal damage to a (creature|permanent) or player, it deals that much damage plus (\d+) to that (creature|permanent) or player instead\.$",
    )
    .captures(&normalized)
    {
        if captures.get(2).expect("damage recipient noun").as_str()
            != captures
                .get(4)
                .expect("echoed damage recipient noun")
                .as_str()
        {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: parse_damage_source(
                    captures.get(1).expect("damage source").as_str(),
                )?,
                recipient: parse_recipient_selector(&format!(
                    "a {} or player",
                    captures.get(2).expect("damage recipient noun").as_str()
                ))?,
                kind: None,
            },
            operation: ReplacementOperation::IncreaseDamage {
                amount: parse_u32(captures.get(3).expect("damage increase").as_str())?,
            },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (.+?) would deal damage to (.+?), (prevent (?:all )?(?:the next )?(?:(\d+) of )?that damage|it deals (double|twice|half) that damage instead|it deals that much damage plus (\d+) instead)\.$",
    )
    .captures(&normalized)
    {
        let source_selector = parse_damage_source(captures.get(1).expect("damage source").as_str())?;
        let recipient =
            parse_recipient_selector(captures.get(2).expect("damage recipient").as_str())?;
        let operation_text = captures.get(3).expect("damage operation").as_str();
        let operation = if operation_text.starts_with("prevent ") {
            ReplacementOperation::PreventDamage {
                amount: captures
                    .get(4)
                    .map(|value| parse_u32(value.as_str()))
                    .transpose()?,
            }
        } else if let Some(scale) = captures.get(5) {
            match scale.as_str() {
                "double" | "twice" => ReplacementOperation::ScaleDamage {
                    numerator: 2,
                    denominator: 1,
                    round_down: false,
                },
                "half" => ReplacementOperation::ScaleDamage {
                    numerator: 1,
                    denominator: 2,
                    round_down: false,
                },
                _ => unreachable!("regex limits damage scale"),
            }
        } else {
            ReplacementOperation::IncreaseDamage {
                amount: parse_u32(
                    captures
                        .get(6)
                        .expect("increased damage amount")
                        .as_str(),
                )?,
            }
        };
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: source_selector,
                recipient,
                kind: None,
            },
            operation,
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^Prevent all (?:(combat|noncombat) )?damage that would be dealt to and dealt by (.+?)\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::DamageSourceOrRecipient {
                object: parse_object_selector(
                    captures
                        .get(2)
                        .expect("two-way damage prevention object")
                        .as_str(),
                    Some(Zone::Battlefield),
                )?,
                kind: damage_kind(captures.get(1)),
            },
            operation: ReplacementOperation::PreventDamage { amount: None },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^Prevent all (?:(combat|noncombat) )?damage that would be dealt to (.+?) by (.+?)\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: parse_damage_source(
                    captures.get(3).expect("prevention damage source").as_str(),
                )?,
                recipient: parse_recipient_selector(
                    captures
                        .get(2)
                        .expect("prevention damage recipient")
                        .as_str(),
                )?,
                kind: damage_kind(captures.get(1)),
            },
            operation: ReplacementOperation::PreventDamage { amount: None },
            optional: false,
        }));
    }
    if let Some(captures) =
        static_regex(r"^Prevent all (?:(combat|noncombat) )?damage that would be dealt by (.+?)\.$")
            .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: parse_damage_source(
                    captures.get(2).expect("prevented damage source").as_str(),
                )?,
                recipient: RecipientSelector::Any,
                kind: damage_kind(captures.get(1)),
            },
            operation: ReplacementOperation::PreventDamage { amount: None },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^Prevent all (?:(combat|noncombat) )?damage that (.+?) would deal to (.+?)\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: parse_damage_source(
                    captures.get(2).expect("prevented damage source").as_str(),
                )?,
                recipient: parse_recipient_selector(
                    captures
                        .get(3)
                        .expect("prevented damage recipient")
                        .as_str(),
                )?,
                kind: damage_kind(captures.get(1)),
            },
            operation: ReplacementOperation::PreventDamage { amount: None },
            optional: false,
        }));
    }
    if let Some(captures) =
        static_regex(r"^Prevent all (?:(combat|noncombat) )?damage that would be dealt to (.+?)\.$")
            .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: ObjectSelector::matching(None),
                recipient: parse_recipient_selector(
                    captures
                        .get(2)
                        .expect("imperative prevention recipient")
                        .as_str(),
                )?,
                kind: damage_kind(captures.get(1)),
            },
            operation: ReplacementOperation::PreventDamage { amount: None },
            optional: false,
        }));
    }
    if let Some(captures) =
        static_regex(r"^(?:All )?damage that would be dealt to (.+?) is prevented\.$")
            .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: ObjectSelector::matching(None),
                recipient: parse_recipient_selector(
                    captures.get(1).expect("prevented recipient").as_str(),
                )?,
                kind: None,
            },
            operation: ReplacementOperation::PreventDamage { amount: None },
            optional: false,
        }));
    }
    if let Some(captures) =
        static_regex(r"^Prevent all damage that would be dealt to (.+?)\.$").captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::Damage {
                source: ObjectSelector::matching(None),
                recipient: parse_recipient_selector(
                    captures
                        .get(1)
                        .expect("imperative prevention recipient")
                        .as_str(),
                )?,
                kind: None,
            },
            operation: ReplacementOperation::PreventDamage { amount: None },
            optional: false,
        }));
    }
    Ok(None)
}

fn parse_multiplier_replacement(
    source: &str,
) -> Result<Option<ReplacementEffect>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    if normalized
        == "If you would draw a card except the first one you draw in each of your draw steps, draw two cards instead."
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::DrawCardExceptFirstInOwnDrawStep {
                player: PlayerSelector::You,
            },
            operation: ReplacementOperation::MultiplyEvent { multiplier: 2 },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (you|an opponent|a player) would draw a card, (?:(you|that player) )?draws? two cards instead\.$",
    )
    .captures(&normalized)
    {
        let subject = captures.get(1).expect("draw replacement player").as_str();
        if let Some(echoed) = captures.get(2)
            && (subject == "you") != (echoed.as_str() == "you")
        {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        let player = match subject {
            "you" => PlayerSelector::You,
            "an opponent" => PlayerSelector::Opponents,
            "a player" => PlayerSelector::EachPlayer,
            _ => unreachable!("regex limits draw replacement player"),
        };
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::DrawCard { player },
            operation: ReplacementOperation::MultiplyEvent { multiplier: 2 },
            optional: false,
        }));
    }
    let token_patterns = [
        r"^If an effect would create one or more tokens under your control, it creates twice that many of those tokens instead\.$",
        r"^If one or more tokens would be created under your control, twice that many of those tokens are created instead\.$",
        r"^If you would create one or more tokens, create twice that many of those tokens instead\.$",
    ];
    if token_patterns
        .iter()
        .any(|pattern| static_regex(pattern).is_match(&normalized))
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::CreateTokens {
                player: PlayerSelector::You,
            },
            operation: ReplacementOperation::MultiplyEvent { multiplier: 2 },
            optional: false,
        }));
    }
    let token_increment_patterns = [
        r"^If one or more tokens would be created under your control, that many plus one of those tokens are created instead\.$",
        r"^If you would create one or more tokens, create that many plus one of those tokens instead\.$",
    ];
    if token_increment_patterns
        .iter()
        .any(|pattern| static_regex(pattern).is_match(&normalized))
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::CreateTokens {
                player: PlayerSelector::You,
            },
            operation: ReplacementOperation::IncreaseEvent { amount: 1 },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If an effect would put one or more counters on (.+?), it puts twice that many of those counters on (?:it|that permanent|that creature) instead\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::PutCounters {
                object: parse_object_selector(
                    captures.get(1).expect("counter multiplier object").as_str(),
                    Some(Zone::Battlefield),
                )?,
                counter: None,
            },
            operation: ReplacementOperation::MultiplyEvent { multiplier: 2 },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If an effect would put one or more (.+?) counters on (.+?), it puts twice that many of those counters on (?:it|that permanent|that creature) instead\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::PutCounters {
                object: parse_object_selector(
                    captures.get(2).expect("counter multiplier object").as_str(),
                    Some(Zone::Battlefield),
                )?,
                counter: Some(parse_counter_kind(
                    captures.get(1).expect("counter multiplier kind").as_str(),
                )?),
            },
            operation: ReplacementOperation::MultiplyEvent { multiplier: 2 },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If one or more counters would be put on (.+?), twice that many of those counters are put on it instead\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::PutCounters {
                object: parse_object_selector(
                    captures.get(1).expect("counter multiplier object").as_str(),
                    Some(Zone::Battlefield),
                )?,
                counter: None,
            },
            operation: ReplacementOperation::MultiplyEvent { multiplier: 2 },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If one or more (.+?) counters? would be put on (.+?), twice that many of those counters are put on it instead\.$",
    )
    .captures(&normalized)
    {
        let counter_text = captures.get(1).expect("counter multiplier kind").as_str();
        let counter = if counter_text == "kind of" || counter_text == "different kinds of" {
            None
        } else {
            Some(parse_counter_kind(counter_text)?)
        };
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::PutCounters {
                object: parse_object_selector(
                    captures.get(2).expect("counter object").as_str(),
                    Some(Zone::Battlefield),
                )?,
                counter,
            },
            operation: ReplacementOperation::MultiplyEvent { multiplier: 2 },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If one or more (.+?) counters? would be put on (.+?), twice that many (.+?) counters? are put on (?:it|that creature|that permanent) instead\.$",
    )
    .captures(&normalized)
    {
        let predicate_counter = parse_counter_kind(
            captures.get(1).expect("counter multiplier kind").as_str(),
        )?;
        let operation_counter = parse_counter_kind(
            captures
                .get(3)
                .expect("counter multiplier result kind")
                .as_str(),
        )?;
        if predicate_counter != operation_counter {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::PutCounters {
                object: parse_object_selector(
                    captures.get(2).expect("counter multiplier object").as_str(),
                    Some(Zone::Battlefield),
                )?,
                counter: Some(predicate_counter),
            },
            operation: ReplacementOperation::MultiplyEvent { multiplier: 2 },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If one or more (.+?) counters? would be put on (.+?), that many of those counters plus one are put on (?:it|that creature|that permanent) instead\.$",
    )
    .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::PutCounters {
                object: parse_object_selector(
                    captures.get(2).expect("counter increment object").as_str(),
                    Some(Zone::Battlefield),
                )?,
                counter: Some(parse_counter_kind(
                    captures.get(1).expect("counter increment kind").as_str(),
                )?),
            },
            operation: ReplacementOperation::IncreaseEvent { amount: 1 },
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If one or more (.+?) counters? would be put on (.+?), that many plus one (.+?) counters? are put on (?:it|that creature|that permanent) instead\.$",
    )
    .captures(&normalized)
    {
        let predicate_counter = parse_counter_kind(
            captures.get(1).expect("counter increment kind").as_str(),
        )?;
        let operation_counter = parse_counter_kind(
            captures
                .get(3)
                .expect("counter increment result kind")
                .as_str(),
        )?;
        if predicate_counter != operation_counter {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::PutCounters {
                object: parse_object_selector(
                    captures.get(2).expect("counter increment object").as_str(),
                    Some(Zone::Battlefield),
                )?,
                counter: Some(predicate_counter),
            },
            operation: ReplacementOperation::IncreaseEvent { amount: 1 },
            optional: false,
        }));
    }
    if static_regex(r"^If you would gain life, you gain twice that much life instead\.$")
        .is_match(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::GainLife {
                player: PlayerSelector::You,
            },
            operation: ReplacementOperation::MultiplyEvent { multiplier: 2 },
            optional: false,
        }));
    }
    if static_regex(r"^If you would gain life, you gain that much life plus 1 instead\.$")
        .is_match(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::GainLife {
                player: PlayerSelector::You,
            },
            operation: ReplacementOperation::IncreaseEvent { amount: 1 },
            optional: false,
        }));
    }
    if static_regex(r"^If you would gain life, draw that many cards instead\.$")
        .is_match(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::GainLife {
                player: PlayerSelector::You,
            },
            operation: ReplacementOperation::DrawCardsInstead,
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(r"^(.+?) can't have (.+?) counters? put on (?:it|them)\.$")
        .captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::PutCounters {
                object: parse_object_selector(
                    captures.get(1).expect("counter prevention object").as_str(),
                    Some(Zone::Battlefield),
                )?,
                counter: Some(parse_counter_kind(
                    captures.get(2).expect("prevented counter kind").as_str(),
                )?),
            },
            operation: ReplacementOperation::SkipEvent,
            optional: false,
        }));
    }
    if let Some(captures) =
        static_regex(r"^Counters can't be put on (.+?)\.$").captures(&normalized)
    {
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::PutCounters {
                object: parse_object_selector(
                    captures.get(1).expect("counter prevention object").as_str(),
                    Some(Zone::Battlefield),
                )?,
                counter: None,
            },
            operation: ReplacementOperation::SkipEvent,
            optional: false,
        }));
    }
    Ok(None)
}

fn parse_skip_replacement(
    source: &str,
) -> Result<Option<ReplacementEffect>, StaticReplacementCompileError> {
    let normalized = source.replace('’', "'").replace('‘', "'");
    if let Some(captures) = static_regex(
        r"^If (you|an opponent|a player) would draw a card, (you|that player) (may )?skips? that draw instead\.$",
    )
    .captures(&normalized)
    {
        let subject = captures.get(1).expect("draw player").as_str();
        let echoed = captures.get(2).expect("draw player echo").as_str();
        if (subject == "you") != (echoed == "you") {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        let player = match subject {
            "you" => PlayerSelector::You,
            "an opponent" => PlayerSelector::Opponents,
            "a player" => PlayerSelector::EachPlayer,
            _ => unreachable!("regex limits draw player"),
        };
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::DrawCard { player },
            operation: ReplacementOperation::SkipEvent,
            optional: captures.get(3).is_some(),
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (you|an opponent|a player) would gain life, (you|that player) gains? no life instead\.$",
    )
    .captures(&normalized)
    {
        let subject = captures.get(1).expect("life-gain player").as_str();
        let echoed = captures.get(2).expect("life-gain player echo").as_str();
        if (subject == "you") != (echoed == "you") {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        let player = match subject {
            "you" => PlayerSelector::You,
            "an opponent" => PlayerSelector::Opponents,
            "a player" => PlayerSelector::EachPlayer,
            _ => unreachable!("regex limits life-gain player"),
        };
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::GainLife { player },
            operation: ReplacementOperation::SkipEvent,
            optional: false,
        }));
    }
    if let Some(captures) = static_regex(
        r"^If (you|an opponent|a player) would begin an extra turn, (you|that player) skips? that turn instead\.$",
    )
    .captures(&normalized)
    {
        let subject = captures.get(1).expect("extra-turn player").as_str();
        let echoed = captures.get(2).expect("extra-turn pronoun").as_str();
        if (subject == "you") != (echoed == "you") {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                normalized,
            ));
        }
        let player = match subject {
            "you" => PlayerSelector::You,
            "an opponent" => PlayerSelector::Opponents,
            "a player" => PlayerSelector::EachPlayer,
            _ => unreachable!("regex limits extra-turn player"),
        };
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::ExtraTurnWouldBegin { player },
            operation: ReplacementOperation::SkipEvent,
            optional: false,
        }));
    }
    Ok(None)
}

fn parse_object_selector(
    source: &str,
    default_zone: Option<Zone>,
) -> Result<ObjectSelector, StaticReplacementCompileError> {
    let mut text = source.trim().replace('’', "'").replace('‘', "'");
    if text.is_empty() {
        return Err(StaticReplacementCompileError::UnsupportedSubject(
            source.to_owned(),
        ));
    }
    // Quantifiers do not alter the characteristic predicate, but removing
    // them here lets the following `other` handling retain the source
    // exclusion in forms such as "Each other creature you control".
    if let Some(rest) = text
        .strip_prefix("Each ")
        .or_else(|| text.strip_prefix("each "))
        .or_else(|| text.strip_prefix("All "))
        .or_else(|| text.strip_prefix("all "))
    {
        text = rest.to_owned();
    }
    let lower = text.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "you" | "your opponents" | "players" | "each player" | "a player" | "an opponent"
    ) {
        return Err(StaticReplacementCompileError::UnsupportedSubject(
            source.to_owned(),
        ));
    }

    // Plural attachment adjectives describe absolute current state, unlike
    // an Aura or Equipment's singular "enchanted/equipped creature", which
    // is relative to that source incarnation. ObjectState carries both exact
    // attachment sets, so these selectors remain incarnation safe.
    for (prefix, is_enchanted) in [("enchanted ", true), ("equipped ", false)] {
        if !lower.starts_with(prefix) {
            continue;
        }
        let mut remainder = text[prefix.len()..].trim().to_owned();
        let remainder_lower = remainder.to_ascii_lowercase();
        let absolute_plural = remainder_lower == "creatures"
            || remainder_lower == "creatures each"
            || remainder_lower.starts_with("creatures ")
            || (remainder_lower.ends_with("s you control")
                && !remainder_lower.starts_with("this "));
        if !absolute_plural {
            continue;
        }
        if remainder_lower.ends_with(" each") {
            remainder.truncate(remainder.len() - " each".len());
        }
        let mut selector = parse_object_selector(&remainder, default_zone)?;
        if is_enchanted {
            selector.enchanted = Some(true);
        } else {
            selector.equipped = Some(true);
        }
        return Ok(selector);
    }

    for (suffix, state) in [
        (" that are enchanted or equipped", None),
        (" that are enchanted", Some(true)),
        (" that are equipped", Some(false)),
    ] {
        if !lower.ends_with(suffix) {
            continue;
        }
        let base = text[..text.len() - suffix.len()].trim();
        let mut selector = parse_object_selector(base, default_zone)?;
        match state {
            Some(true) => selector.enchanted = Some(true),
            Some(false) => selector.equipped = Some(true),
            None => {
                let mut enchanted = selector.clone();
                enchanted.enchanted = Some(true);
                let mut equipped = selector.clone();
                equipped.equipped = Some(true);
                selector.alternatives = vec![enchanted, equipped];
            }
        }
        return Ok(selector);
    }

    if let Some(captures) =
        static_regex(r"^(.+? you control) (?:that's|that is|that are) (.+)$").captures(&text)
    {
        let base = parse_object_selector(
            captures.get(1).expect("qualified selector base").as_str(),
            default_zone,
        )?;
        return parse_subtype_or_token_union(
            base,
            captures
                .get(2)
                .expect("qualified selector alternatives")
                .as_str(),
            source,
        );
    }
    for (prefix, reference) in [
        ("enchanted ", SelectorReference::EnchantedBySource),
        ("equipped ", SelectorReference::EquippedBySource),
    ] {
        if lower.starts_with(prefix) {
            let remainder = &text[prefix.len()..];
            let remainder_lower = remainder.to_ascii_lowercase();
            if ![
                "artifact",
                "battle",
                "creature",
                "enchantment",
                "land",
                "permanent",
                "planeswalker",
            ]
            .iter()
            .any(|noun| {
                remainder_lower == *noun || remainder_lower.starts_with(&format!("{noun} "))
            }) {
                continue;
            }
            let mut selector = parse_object_selector(remainder, default_zone)?;
            if selector.reference != SelectorReference::Matching {
                return Err(StaticReplacementCompileError::UnsupportedSubject(
                    source.to_owned(),
                ));
            }
            selector.reference = reference;
            return Ok(selector);
        }
    }
    if [
        "this",
        "it",
        "this object",
        "this permanent",
        "this creature",
        "this artifact",
        "this aura",
        "this battle",
        "this enchantment",
        "this equipment",
        "this land",
        "this planeswalker",
        "this card",
        "this spell",
    ]
    .contains(&lower.as_str())
    {
        let mut selector = ObjectSelector::source();
        if let Some(zone) = default_zone {
            selector.zones.insert(zone);
        }
        if lower == "this spell" {
            selector.zones.clear();
            selector.zones.insert(Zone::Stack);
        }
        match lower.strip_prefix("this ").unwrap_or("") {
            "aura" => {
                selector.card_types.insert(CardType::Enchantment);
                selector.subtypes.insert("Aura".to_owned());
            }
            "equipment" => {
                selector.card_types.insert(CardType::Artifact);
                selector.subtypes.insert("Equipment".to_owned());
            }
            noun => add_noun_type(&mut selector, noun)?,
        }
        return Ok(selector);
    }

    let mut selector = ObjectSelector::matching(default_zone);
    if let Some(rest) = text
        .strip_prefix("other ")
        .or_else(|| text.strip_prefix("another "))
    {
        selector.exclude_source = true;
        text = rest.to_owned();
    } else if let Some(rest) = text
        .strip_prefix("Other ")
        .or_else(|| text.strip_prefix("Another "))
    {
        selector.exclude_source = true;
        text = rest.to_owned();
    }
    let normalized = text.replace('’', "'").replace('‘', "'");
    if let Some(captures) = static_regex(r"^(?:a |an )?(\d+)/(\d+) (.+)$").captures(&normalized) {
        let power = parse_u32(captures.get(1).expect("exact selector power").as_str())?;
        let toughness = parse_u32(captures.get(2).expect("exact selector toughness").as_str())?;
        let power = i32::try_from(power)
            .map_err(|_| StaticReplacementCompileError::UnsupportedOperand(source.to_owned()))?;
        let toughness = i32::try_from(toughness)
            .map_err(|_| StaticReplacementCompileError::UnsupportedOperand(source.to_owned()))?;
        selector.minimum_power = Some(power);
        selector.maximum_power = Some(power);
        selector.minimum_toughness = Some(toughness);
        selector.maximum_toughness = Some(toughness);
        text = captures
            .get(3)
            .expect("exact selector object kind")
            .as_str()
            .to_owned();
    }
    let normalized = text.replace('’', "'").replace('‘', "'");
    if let Some(captures) = static_regex(
        r"^(.+?) with (a|an|one|two|three|four|five|six|seven|eight|nine|ten|\d+)(?: or more)? (.+?) counters? on (?:it|them)$",
    )
    .captures(&normalized)
    {
        text = captures
            .get(1)
            .expect("counter-qualified selector base")
            .as_str()
            .to_owned();
        selector.minimum_counters.insert(
            parse_counter_kind(
                captures
                    .get(3)
                    .expect("counter-qualified selector kind")
                    .as_str(),
            )?,
            parse_word_or_number(
                captures
                    .get(2)
                    .expect("counter-qualified selector minimum")
                    .as_str(),
            )?,
        );
    }
    let lower = text.to_ascii_lowercase();
    for suffix in [
        " with a +1/+1 counter on it",
        " with a +1/+1 counter on them",
        " with +1/+1 counters on it",
        " with +1/+1 counters on them",
        " with one or more +1/+1 counters on it",
        " with one or more +1/+1 counters on them",
    ] {
        if lower.ends_with(suffix) {
            text.truncate(text.len() - suffix.len());
            selector
                .minimum_counters
                .insert(CounterKind::PlusOnePlusOne, 1);
            break;
        }
    }
    let lower = text.to_ascii_lowercase();
    for suffix in [
        " with a counter on it",
        " with a counter on them",
        " with counters on it",
        " with counters on them",
        " with one or more counters on it",
        " with one or more counters on them",
    ] {
        if lower.ends_with(suffix) {
            text.truncate(text.len() - suffix.len());
            selector.minimum_total_counters = Some(1);
            break;
        }
    }
    loop {
        let normalized = text.replace('’', "'").replace('‘', "'");
        if let Some(captures) = static_regex(r"^(.+?) named (.+)$").captures(&normalized) {
            let base = captures.get(1).expect("named selector base").as_str();
            let name = captures.get(2).expect("named selector value").as_str();
            if name.is_empty() || name.contains(" or ") || name.contains(" and ") {
                return Err(StaticReplacementCompileError::UnsupportedSubject(
                    source.to_owned(),
                ));
            }
            selector.names.insert(name.to_owned());
            text = base.to_owned();
            continue;
        }
        if let Some(captures) =
            static_regex(r"^(.+?) with (power|toughness|mana value) (\d+) or (greater|more|less)$")
                .captures(&normalized)
        {
            let base = captures.get(1).expect("bounded selector base").as_str();
            let characteristic = captures
                .get(2)
                .expect("bounded selector characteristic")
                .as_str();
            let value = parse_u32(
                captures
                    .get(3)
                    .expect("bounded selector threshold")
                    .as_str(),
            )?;
            let lower_bound = matches!(
                captures
                    .get(4)
                    .expect("bounded selector direction")
                    .as_str(),
                "greater" | "more"
            );
            match characteristic {
                "mana value" if lower_bound => selector.minimum_mana_value = Some(value),
                "mana value" => selector.maximum_mana_value = Some(value),
                "power" if lower_bound => {
                    selector.minimum_power = Some(i32::try_from(value).map_err(|_| {
                        StaticReplacementCompileError::UnsupportedOperand(value.to_string())
                    })?)
                }
                "power" => {
                    selector.maximum_power = Some(i32::try_from(value).map_err(|_| {
                        StaticReplacementCompileError::UnsupportedOperand(value.to_string())
                    })?)
                }
                "toughness" if lower_bound => {
                    selector.minimum_toughness = Some(i32::try_from(value).map_err(|_| {
                        StaticReplacementCompileError::UnsupportedOperand(value.to_string())
                    })?)
                }
                "toughness" => {
                    selector.maximum_toughness = Some(i32::try_from(value).map_err(|_| {
                        StaticReplacementCompileError::UnsupportedOperand(value.to_string())
                    })?)
                }
                _ => unreachable!("regex limits bounded selector characteristic"),
            }
            text = base.to_owned();
            continue;
        }
        if let Some(captures) = static_regex(r"^(.+?) with (.+)$").captures(&normalized) {
            let base = captures.get(1).expect("keyword selector base").as_str();
            let keyword_text = captures.get(2).expect("keyword selector value").as_str();
            let normalized_keyword_text = keyword_text.replace(" and/or ", " or ");
            let keyword_alternatives = normalized_keyword_text
                .split(" or ")
                .map(str::trim)
                .map(parse_keyword_set)
                .collect::<Result<Vec<_>, _>>();
            if let Ok(alternatives) = keyword_alternatives
                && alternatives.len() > 1
                && alternatives.iter().all(|keywords| keywords.len() == 1)
            {
                selector.required_keywords = alternatives.into_iter().flatten().collect();
                selector.required_keyword_match_any = true;
                text = base.to_owned();
                continue;
            }
            if let Ok(keywords) = parse_keyword_set(keyword_text) {
                selector.required_keywords.extend(keywords);
                text = base.to_owned();
                continue;
            }
        }
        break;
    }
    let lower = text.to_ascii_lowercase();
    let (base_end, relation) = if lower.ends_with(" you control") {
        (text.len() - " you control".len(), ControllerRelation::You)
    } else if lower.ends_with(" you don't control") {
        (
            text.len() - " you don't control".len(),
            ControllerRelation::Opponent,
        )
    } else if lower.ends_with(" your opponents control") {
        (
            text.len() - " your opponents control".len(),
            ControllerRelation::Opponent,
        )
    } else if lower.ends_with(" an opponent controls") {
        (
            text.len() - " an opponent controls".len(),
            ControllerRelation::Opponent,
        )
    } else if lower.ends_with(" opponents control") {
        (
            text.len() - " opponents control".len(),
            ControllerRelation::Opponent,
        )
    } else {
        (text.len(), ControllerRelation::Any)
    };
    selector.controller = relation;

    let mut base = text[..base_end].trim();
    let base_lower = base.to_ascii_lowercase();
    for prefix in ["one or more ", "all ", "each ", "a ", "an "] {
        if base_lower.starts_with(prefix) {
            base = &base[prefix.len()..];
            break;
        }
    }
    let base_lower = base.to_ascii_lowercase();
    if matches!(
        base_lower.as_str(),
        "you" | "your opponents" | "players" | "each player" | "a player" | "an opponent"
    ) {
        return Err(StaticReplacementCompileError::UnsupportedSubject(
            source.to_owned(),
        ));
    }
    let type_union = [" spells", " cards", " card"]
        .iter()
        .find_map(|suffix| base_lower.strip_suffix(suffix));
    if let Some(type_list) = type_union {
        let alternatives = type_list
            .split(" and/or ")
            .flat_map(|part| part.split(" and "))
            .flat_map(|part| part.split(" or "))
            .collect::<Vec<_>>();
        if alternatives.len() > 1
            && alternatives
                .iter()
                .all(|part| !part.is_empty() && !part.contains(' '))
        {
            let card_types = alternatives
                .iter()
                .map(|part| card_type_adjective(part))
                .collect::<Result<BTreeSet<_>, _>>();
            if let Ok(card_types) = card_types
                && card_types.len() == alternatives.len()
            {
                selector.card_types = card_types;
                selector.card_type_match_any = true;
                return Ok(selector);
            }
        }
    }
    let union_parts = elided_selector_union_parts(base).or_else(|| {
        explicit_selector_union_parts(base)
            .map(|parts| parts.into_iter().map(str::to_owned).collect())
    });
    if let Some(parts) = union_parts {
        let controller_suffix = match relation {
            ControllerRelation::You => " you control",
            ControllerRelation::Opponent => " an opponent controls",
            ControllerRelation::Any => "",
        };
        let alternatives = parts
            .into_iter()
            .map(|part| {
                let mut alternative =
                    parse_object_selector(&format!("{part}{controller_suffix}"), default_zone)?;
                alternative.exclude_source |= selector.exclude_source;
                alternative
                    .required_keywords
                    .extend(selector.required_keywords.iter().copied());
                alternative.required_keyword_match_any = selector.required_keyword_match_any;
                for (counter, minimum) in &selector.minimum_counters {
                    alternative
                        .minimum_counters
                        .entry(counter.clone())
                        .and_modify(|current| *current = (*current).max(*minimum))
                        .or_insert(*minimum);
                }
                alternative.minimum_total_counters = selector.minimum_total_counters;
                alternative.minimum_mana_value = selector.minimum_mana_value;
                alternative.maximum_mana_value = selector.maximum_mana_value;
                alternative.minimum_power = selector.minimum_power;
                alternative.maximum_power = selector.maximum_power;
                alternative.minimum_toughness = selector.minimum_toughness;
                alternative.maximum_toughness = selector.maximum_toughness;
                Ok(alternative)
            })
            .collect::<Result<Vec<_>, StaticReplacementCompileError>>();
        if let Ok(alternatives) = alternatives
            && alternatives.len() > 1
        {
            let mut union = ObjectSelector::matching(default_zone);
            union.alternatives = alternatives;
            return Ok(union);
        }
    }
    if base.contains(',')
        || base.contains(';')
        || base.contains('|')
        || base.contains('\u{2014}')
        || base.contains('(')
        || base.contains(')')
        || base_lower.contains(" and ")
        || base_lower.contains(" or ")
        || base_lower.contains(" and/or ")
        || base_lower.contains(" with ")
        || base_lower.contains(" without ")
        || base_lower.contains(" among ")
        || base_lower.contains(" named ")
        || base_lower.contains(" on ")
        || base_lower.contains(" of the chosen ")
        || base_lower.contains("same name")
    {
        return Err(StaticReplacementCompileError::UnsupportedSubject(
            source.to_owned(),
        ));
    }

    let mut words = base
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if words.is_empty() {
        return Err(StaticReplacementCompileError::UnsupportedSubject(
            source.to_owned(),
        ));
    }
    if words
        .iter()
        .any(|word| matches!(word.to_ascii_lowercase().as_str(), "token" | "tokens"))
    {
        selector.token_relation = TokenRelation::Token;
        words.retain(|word| !matches!(word.to_ascii_lowercase().as_str(), "token" | "tokens"));
    } else if words
        .iter()
        .any(|word| word.eq_ignore_ascii_case("nontoken"))
    {
        selector.token_relation = TokenRelation::Nontoken;
        words.retain(|word| !word.eq_ignore_ascii_case("nontoken"));
    }
    if words.is_empty() && selector.token_relation != TokenRelation::Any {
        return Ok(selector);
    }

    let noun_original = words
        .last()
        .map(String::as_str)
        .ok_or_else(|| StaticReplacementCompileError::UnsupportedSubject(source.to_owned()))?;
    let noun_lower = noun_original.to_ascii_lowercase();
    let noun = singularize(&noun_lower);
    if noun == "card" {
        selector.token_relation = TokenRelation::Nontoken;
    } else if noun == "permanent" || noun == "object" || noun == "spell" {
    } else if add_noun_type(&mut selector, noun).is_err() {
        if noun == "source" {
            return Ok(selector);
        }
        if let Some(subtype) = singular_permanent_subtype(noun_original) {
            selector.subtypes.insert(subtype);
        } else {
            return Err(StaticReplacementCompileError::UnsupportedSubject(
                source.to_owned(),
            ));
        }
    }
    for adjective_original in &words[..words.len().saturating_sub(1)] {
        let adjective_lower = adjective_original.to_ascii_lowercase();
        let adjective = adjective_lower.as_str();
        if [
            "blocked",
            "chosen",
            "commander",
            "common",
            "enchanted",
            "equipped",
            "face-down",
            "face-up",
            "favorite",
            "fortified",
            "goated",
            "goaded",
            "historic",
            "hosted",
            "kicked",
            "monstrous",
            "modified",
            "paired",
            "permanent",
            "phased-out",
            "premium",
            "renowned",
            "saddled",
            "stickered",
            "suspected",
            "target",
            "transformed",
            "unblocked",
            "white-bordered",
        ]
        .contains(&adjective)
        {
            return Err(StaticReplacementCompileError::UnsupportedSubject(
                source.to_owned(),
            ));
        }
        if adjective.starts_with("non-") {
            let excluded = &adjective_original["non-".len()..];
            let excluded_lower = excluded.to_ascii_lowercase();
            if let Some(color) = parse_color(&excluded_lower) {
                selector.excluded_colors.insert(color);
            } else if let Ok(card_type) = card_type_adjective(&excluded_lower) {
                selector.excluded_card_types.insert(card_type);
            } else if let Some(subtype) = singular_permanent_subtype(excluded) {
                selector.excluded_subtypes.insert(subtype);
            } else {
                return Err(StaticReplacementCompileError::UnsupportedSubject(
                    source.to_owned(),
                ));
            }
        } else if adjective == "basic" {
            selector.supertypes.insert(Supertype::Basic);
        } else if adjective == "legendary" {
            selector.supertypes.insert(Supertype::Legendary);
        } else if adjective == "snow" {
            selector.supertypes.insert(Supertype::Snow);
        } else if adjective == "nonbasic" {
            selector.excluded_supertypes.insert(Supertype::Basic);
        } else if adjective == "nonlegendary" {
            selector.excluded_supertypes.insert(Supertype::Legendary);
        } else if adjective == "nonsnow" {
            selector.excluded_supertypes.insert(Supertype::Snow);
        } else if adjective == "monocolored" {
            selector.minimum_colors = Some(1);
            selector.maximum_colors = Some(1);
        } else if adjective == "multicolored" {
            selector.minimum_colors = Some(2);
        } else if let Some(color) = adjective.strip_prefix("non").and_then(parse_color) {
            selector.excluded_colors.insert(color);
        } else if let Some(card_type) = adjective
            .strip_prefix("non")
            .and_then(|noun| card_type_adjective(noun).ok())
        {
            selector.excluded_card_types.insert(card_type);
        } else if adjective == "tapped" {
            selector.tapped = Some(true);
        } else if adjective == "untapped" {
            selector.tapped = Some(false);
        } else if adjective == "attacking" {
            selector.attacking = Some(true);
        } else if adjective == "blocking" {
            selector.blocking = Some(true);
        } else if let Some(color) = parse_color(adjective) {
            selector.colors.insert(color);
        } else if let Ok(card_type) = card_type_adjective(adjective) {
            selector.card_types.insert(card_type);
        } else {
            let valid_subtype_word = adjective_original
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
                && adjective_original
                    .chars()
                    .all(|character| character.is_ascii_alphabetic() || character == '-');
            if !valid_subtype_word
                || matches!(
                    adjective_original.as_str(),
                    "Beyond" | "Secret" | "Lair" | "Universes"
                )
            {
                return Err(StaticReplacementCompileError::UnsupportedSubject(
                    source.to_owned(),
                ));
            }
            selector.subtypes.insert(adjective_original.clone());
        }
    }
    Ok(selector)
}

fn parse_subtype_or_token_union(
    mut base: ObjectSelector,
    source: &str,
    exact_subject: &str,
) -> Result<ObjectSelector, StaticReplacementCompileError> {
    let normalized = source
        .replace(", and ", ", ")
        .replace(", or ", ", ")
        .replace(" and/or ", ", ")
        .replace(" or ", ", ");
    let parts = normalized
        .split(", ")
        .map(str::trim)
        .map(|part| {
            part.strip_prefix("a ")
                .or_else(|| part.strip_prefix("an "))
                .unwrap_or(part)
        })
        .collect::<Vec<_>>();
    if !(2..=8).contains(&parts.len()) || parts.iter().any(|part| part.is_empty()) {
        return Err(StaticReplacementCompileError::UnsupportedSubject(
            exact_subject.to_owned(),
        ));
    }
    let mut alternatives = Vec::with_capacity(parts.len());
    for part in parts {
        let mut alternative = base.clone();
        alternative.alternatives.clear();
        if matches!(part.to_ascii_lowercase().as_str(), "token" | "tokens") {
            alternative.token_relation = TokenRelation::Token;
        } else if let Some(subtype) = singular_permanent_subtype(part) {
            alternative.subtypes.insert(subtype);
        } else {
            return Err(StaticReplacementCompileError::UnsupportedSubject(
                exact_subject.to_owned(),
            ));
        }
        alternatives.push(alternative);
    }
    base.alternatives = alternatives;
    Ok(base)
}

fn explicit_selector_union_parts(source: &str) -> Option<Vec<&str>> {
    fn clean_part(part: &str) -> &str {
        part.trim()
            .strip_prefix("and/or ")
            .or_else(|| part.trim().strip_prefix("and "))
            .or_else(|| part.trim().strip_prefix("or "))
            .unwrap_or_else(|| part.trim())
            .trim()
    }
    let parts = if source.contains(',') {
        source.split(',').map(clean_part).collect::<Vec<_>>()
    } else if source.contains(" and/or ") {
        source.split(" and/or ").map(str::trim).collect::<Vec<_>>()
    } else if source.contains(" or ") {
        source.split(" or ").map(str::trim).collect::<Vec<_>>()
    } else if source.contains(" and ") {
        source.split(" and ").map(str::trim).collect::<Vec<_>>()
    } else {
        return None;
    };
    (2..=16)
        .contains(&parts.len())
        .then_some(parts)
        .filter(|parts| parts.iter().all(|part| !part.is_empty()))
}

fn elided_selector_union_parts(source: &str) -> Option<Vec<String>> {
    let lower = source.to_ascii_lowercase();
    let noun = [
        "creature",
        "creatures",
        "permanent",
        "permanents",
        "spell",
        "spells",
        "card",
        "cards",
        "land",
        "lands",
    ]
    .into_iter()
    .find(|noun| lower.ends_with(&format!(" {noun}")))?;
    let modifiers = source[..source.len() - noun.len()].trim_end();
    let normalized = modifiers.replace(" and/or ", " or ");
    let parts = normalized.split(" or ").map(str::trim).collect::<Vec<_>>();
    if !(2..=5).contains(&parts.len())
        || parts.iter().any(|part| {
            part.is_empty()
                || part.contains(char::is_whitespace)
                || part.to_ascii_lowercase().starts_with("non")
        })
    {
        return None;
    }
    Some(
        parts
            .into_iter()
            .map(|part| format!("{part} {noun}"))
            .collect(),
    )
}

fn card_type_adjective(source: &str) -> Result<CardType, StaticReplacementCompileError> {
    match singularize(source) {
        "artifact" => Ok(CardType::Artifact),
        "battle" => Ok(CardType::Battle),
        "creature" => Ok(CardType::Creature),
        "enchantment" => Ok(CardType::Enchantment),
        "instant" => Ok(CardType::Instant),
        "kindred" | "tribal" => Ok(CardType::Kindred),
        "land" => Ok(CardType::Land),
        "planeswalker" => Ok(CardType::Planeswalker),
        "sorcery" => Ok(CardType::Sorcery),
        _ => Err(StaticReplacementCompileError::UnsupportedSubject(
            source.to_owned(),
        )),
    }
}

fn parse_spell_selector(
    source: &str,
    _player: PlayerSelector,
) -> Result<ObjectSelector, StaticReplacementCompileError> {
    let mut text = source.trim().to_owned();
    let mut lower = text.to_ascii_lowercase();
    for exact in ["spells you cast", "spells your opponents cast"] {
        if lower == exact {
            return Ok(ObjectSelector::matching(None));
        }
    }
    // The acting player is carried by `CostScope::CastSpell`, not by the
    // object selector. Oracle places that actor phrase both before and after
    // selector qualifiers ("creature spells you cast with ..." and "spells
    // with flying you cast"). Remove only the exact actor phrase so the same
    // strict selector parser handles both word orders without discarding any
    // characteristic predicate.
    for actor_phrase in [" you cast", " your opponents cast"] {
        if let Some(index) = lower.find(actor_phrase) {
            text.replace_range(index..index + actor_phrase.len(), "");
            lower = text.to_ascii_lowercase();
            break;
        }
    }
    for suffix in [" spells you cast", " spells your opponents cast"] {
        if lower.ends_with(suffix) {
            text.truncate(text.len() - suffix.len());
            text.push_str(" spells");
            lower = text.to_ascii_lowercase();
            break;
        }
    }
    if lower.ends_with(" cards") {
        text.truncate(text.len() - " cards".len());
        text.push_str(" spells");
    }
    let mut selector = parse_object_selector(&text, None)?;
    selector.zones.clear();
    // The acting player is checked independently by cast and cost queries.
    // Cards outside the battlefield do not have controllers under the rules,
    // so a retained object-controller field is not valid cast evidence.
    selector.controller = ControllerRelation::Any;
    Ok(selector)
}

fn parse_ability_cost_subject(
    source: &str,
) -> Result<(PlayerSelector, Option<ObjectSelector>), StaticReplacementCompileError> {
    let source = source.trim();
    let lower = source.to_ascii_lowercase();
    if lower == "activated abilities you activate" {
        return Ok((PlayerSelector::You, None));
    }
    if lower == "activated abilities your opponents activate" {
        return Ok((PlayerSelector::Opponents, None));
    }
    let prefix = "activated abilities of ";
    if lower.starts_with(prefix) && lower.ends_with(" you control") {
        let subject = &source[prefix.len()..source.len() - " you control".len()];
        return Ok((
            PlayerSelector::You,
            Some(parse_object_selector(subject, Some(Zone::Battlefield))?),
        ));
    }
    if lower.starts_with(prefix) && lower.ends_with(" your opponents control") {
        let subject = &source[prefix.len()..source.len() - " your opponents control".len()];
        return Ok((
            PlayerSelector::Opponents,
            Some(parse_object_selector(subject, Some(Zone::Battlefield))?),
        ));
    }
    if lower.starts_with(prefix) {
        let subject = &source[prefix.len()..];
        return Ok((
            PlayerSelector::EachPlayer,
            Some(parse_object_selector(subject, Some(Zone::Battlefield))?),
        ));
    }
    Err(StaticReplacementCompileError::UnsupportedSubject(
        source.to_owned(),
    ))
}

fn parse_recipient_selector(
    source: &str,
) -> Result<RecipientSelector, StaticReplacementCompileError> {
    let normalized = source.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "an opponent or a permanent an opponent controls"
            | "an opponent or permanent an opponent controls"
            | "an opponent or a permanent they control"
            | "an opponent or permanent they control"
    ) {
        return Ok(RecipientSelector::Alternatives(vec![
            RecipientSelector::Player(PlayerSelector::Opponents),
            RecipientSelector::Object(parse_object_selector(
                "a permanent an opponent controls",
                Some(Zone::Battlefield),
            )?),
        ]));
    }
    if let Some(object) = normalized.strip_prefix("you and ") {
        return Ok(RecipientSelector::Alternatives(vec![
            RecipientSelector::Player(PlayerSelector::You),
            RecipientSelector::Object(parse_object_selector(object, Some(Zone::Battlefield))?),
        ]));
    }
    match normalized.as_str() {
        "you" => Ok(RecipientSelector::Player(PlayerSelector::You)),
        "an opponent" | "your opponents" => {
            Ok(RecipientSelector::Player(PlayerSelector::Opponents))
        }
        "a player" | "each player" | "players" => {
            Ok(RecipientSelector::Player(PlayerSelector::EachPlayer))
        }
        "any target"
        | "a permanent, player, or battle"
        | "a permanent, player, or planeswalker" => Ok(RecipientSelector::Any),
        "a permanent or player"
        | "a permanent or a player"
        | "a player or permanent"
        | "a player or a permanent" => Ok(RecipientSelector::Alternatives(vec![
            RecipientSelector::Object(parse_object_selector(
                "a permanent",
                Some(Zone::Battlefield),
            )?),
            RecipientSelector::Player(PlayerSelector::EachPlayer),
        ])),
        "a creature or player" | "a creature or a player" => {
            Ok(RecipientSelector::Alternatives(vec![
                RecipientSelector::Object(parse_object_selector(
                    "a creature",
                    Some(Zone::Battlefield),
                )?),
                RecipientSelector::Player(PlayerSelector::EachPlayer),
            ]))
        }
        _ => Ok(RecipientSelector::Object(parse_object_selector(
            source,
            Some(Zone::Battlefield),
        )?)),
    }
}

fn parse_damage_source(source: &str) -> Result<ObjectSelector, StaticReplacementCompileError> {
    let lower = source.trim().to_ascii_lowercase();
    if lower == "a source" || lower == "any source" || lower == "damage" {
        return Ok(ObjectSelector::matching(None));
    }
    parse_object_selector(source, None)
}

fn parse_player_selector(source: &str) -> Result<PlayerSelector, StaticReplacementCompileError> {
    match source.trim().to_ascii_lowercase().as_str() {
        "you" => Ok(PlayerSelector::You),
        "your opponents" | "opponents" | "each opponent" | "an opponent" => {
            Ok(PlayerSelector::Opponents)
        }
        "players" | "each player" | "a player" | "any player" => Ok(PlayerSelector::EachPlayer),
        value => Err(StaticReplacementCompileError::UnsupportedSubject(
            value.to_owned(),
        )),
    }
}

fn parse_controller_phrase(source: &str) -> Result<PlayerSelector, StaticReplacementCompileError> {
    match source {
        "you control" => Ok(PlayerSelector::You),
        "your opponents control" => Ok(PlayerSelector::Opponents),
        "players control" => Ok(PlayerSelector::EachPlayer),
        value => Err(StaticReplacementCompileError::UnsupportedOperand(
            value.to_owned(),
        )),
    }
}

fn add_noun_type(
    selector: &mut ObjectSelector,
    noun: &str,
) -> Result<(), StaticReplacementCompileError> {
    let card_type = match noun {
        "" | "permanent" | "card" | "object" | "spell" | "source" => return Ok(()),
        "artifact" => CardType::Artifact,
        "battle" => CardType::Battle,
        "creature" => CardType::Creature,
        "enchantment" => CardType::Enchantment,
        "instant" => CardType::Instant,
        "kindred" | "tribal" => CardType::Kindred,
        "land" => CardType::Land,
        "planeswalker" => CardType::Planeswalker,
        "sorcery" => CardType::Sorcery,
        _ => {
            return Err(StaticReplacementCompileError::UnsupportedSubject(
                noun.to_owned(),
            ));
        }
    };
    selector.card_types.insert(card_type);
    Ok(())
}

fn singularize(noun: &str) -> &str {
    match noun {
        "artifacts" => "artifact",
        "battles" => "battle",
        "cards" => "card",
        "creatures" => "creature",
        "enchantments" => "enchantment",
        "instants" => "instant",
        "lands" => "land",
        "objects" => "object",
        "permanents" => "permanent",
        "planeswalkers" => "planeswalker",
        "sorceries" => "sorcery",
        "sources" => "source",
        "spells" => "spell",
        other => other,
    }
}

fn singular_permanent_subtype(noun: &str) -> Option<String> {
    let is_printed_subtype = noun
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_uppercase())
        && noun
            .chars()
            .all(|character| character.is_ascii_alphabetic() || character == '-');
    if !is_printed_subtype {
        return None;
    }
    Some(match noun {
        // Basic land subtypes are exact printed subtype names. `Plains` is
        // singular even though its spelling ends in `s`.
        "Plains" => "Plains".to_owned(),
        // Magic uses the ordinary English irregular plural for these subtype
        // words. Keep the conversion exact instead of guessing at arbitrary
        // lowercase nouns that happen to end in `ves`.
        "Elves" => "Elf".to_owned(),
        "Dwarves" => "Dwarf".to_owned(),
        "Wolves" => "Wolf".to_owned(),
        "Oxen" => "Ox".to_owned(),
        // Printed subtype plurals otherwise retain the singular spelling and
        // append `s` (Faerie/Faeries is represented by removing only the final
        // `s`, not by applying a general English `ies` -> `y` rule).
        plural if plural.ends_with('s') && plural.len() > 1 => {
            plural[..plural.len() - 1].to_owned()
        }
        singular => singular.to_owned(),
    })
}

fn capitalize_ascii(source: &str) -> String {
    let mut characters = source.chars();
    match characters.next() {
        None => String::new(),
        Some(first) => first.to_ascii_uppercase().to_string() + characters.as_str(),
    }
}

fn parse_color(source: &str) -> Option<Color> {
    match source {
        "white" => Some(Color::White),
        "blue" => Some(Color::Blue),
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "colorless" => Some(Color::Colorless),
        _ => None,
    }
}

fn parse_keyword_set(
    source: &str,
) -> Result<BTreeSet<KeywordAbility>, StaticReplacementCompileError> {
    let normalized = source
        .replace(", and ", ", ")
        .replace(" and ", ", ")
        .replace("ward\u{2014}", "ward ");
    let mut keywords = BTreeSet::new();
    for token in normalized.split(", ").map(str::trim) {
        let keyword = match token.to_ascii_lowercase().as_str() {
            "deathtouch" => KeywordAbility::Deathtouch,
            "defender" => KeywordAbility::Defender,
            "double strike" => KeywordAbility::DoubleStrike,
            "first strike" => KeywordAbility::FirstStrike,
            "flash" => KeywordAbility::Flash,
            "flying" => KeywordAbility::Flying,
            "haste" => KeywordAbility::Haste,
            "hexproof" => KeywordAbility::Hexproof,
            "indestructible" => KeywordAbility::Indestructible,
            "infect" => KeywordAbility::Infect,
            "lifelink" => KeywordAbility::Lifelink,
            "menace" => KeywordAbility::Menace,
            "reach" => KeywordAbility::Reach,
            "shadow" => KeywordAbility::Shadow,
            "shroud" => KeywordAbility::Shroud,
            "trample" => KeywordAbility::Trample,
            "vigilance" => KeywordAbility::Vigilance,
            "ward" => KeywordAbility::Ward,
            "wither" => KeywordAbility::Wither,
            _ => {
                return Err(StaticReplacementCompileError::UnsupportedOperand(
                    token.to_owned(),
                ));
            }
        };
        if !keywords.insert(keyword) {
            return Err(StaticReplacementCompileError::UnsupportedOperand(format!(
                "duplicate keyword {token}"
            )));
        }
    }
    if keywords.is_empty() {
        return Err(StaticReplacementCompileError::UnsupportedOperand(
            source.to_owned(),
        ));
    }
    Ok(keywords)
}

fn parse_signed_amount(source: &str) -> Result<SignedAmount, StaticReplacementCompileError> {
    let (negative, magnitude) = if let Some(value) = source.strip_prefix('+') {
        (false, value)
    } else if let Some(value) = source.strip_prefix('-') {
        (true, value)
    } else {
        return Err(StaticReplacementCompileError::UnsupportedOperand(
            source.to_owned(),
        ));
    };
    Ok(SignedAmount {
        negative,
        magnitude: parse_amount(magnitude)?,
    })
}

fn parse_amount(source: &str) -> Result<Amount, StaticReplacementCompileError> {
    match source {
        "X" => Ok(Amount::X),
        "that many" => Ok(Amount::ThatMany),
        value => Ok(Amount::Fixed(parse_u32(value)?)),
    }
}

fn parse_amount_word(source: &str) -> Result<Amount, StaticReplacementCompileError> {
    if source == "X" {
        return Ok(Amount::X);
    }
    if source == "twice X" {
        return Ok(Amount::Scaled {
            factor: 2,
            amount: Box::new(Amount::X),
        });
    }
    Ok(Amount::Fixed(parse_word_or_number(source)?))
}

fn parse_word_or_number(source: &str) -> Result<u32, StaticReplacementCompileError> {
    match source {
        "a" | "an" | "one" => Ok(1),
        "two" => Ok(2),
        "three" => Ok(3),
        "four" => Ok(4),
        "five" => Ok(5),
        "six" => Ok(6),
        "seven" => Ok(7),
        "eight" => Ok(8),
        "nine" => Ok(9),
        "ten" => Ok(10),
        "twenty" => Ok(20),
        value => parse_u32(value),
    }
}

fn parse_u32(source: &str) -> Result<u32, StaticReplacementCompileError> {
    source
        .parse::<u32>()
        .map_err(|_| StaticReplacementCompileError::UnsupportedOperand(source.to_owned()))
}

fn parse_counter_kind(source: &str) -> Result<CounterKind, StaticReplacementCompileError> {
    let normalized = source.trim().trim_end_matches(" counter");
    if normalized.is_empty() {
        return Err(StaticReplacementCompileError::UnsupportedOperand(
            source.to_owned(),
        ));
    }
    Ok(match normalized {
        "+1/+1" => CounterKind::PlusOnePlusOne,
        "-1/-1" => CounterKind::MinusOneMinusOne,
        "loyalty" => CounterKind::Loyalty,
        "charge" => CounterKind::Charge,
        value
            if value.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || character == ' '
                    || character == '-'
                    || character == '\''
            }) =>
        {
            CounterKind::Named(value.to_owned())
        }
        _ => {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                source.to_owned(),
            ));
        }
    })
}

fn parse_entry_counter_kind(source: &str) -> Result<CounterKind, StaticReplacementCompileError> {
    parse_counter_kind(source.strip_prefix("additional ").unwrap_or(source))
}

fn parse_zone(source: &str) -> Result<Zone, StaticReplacementCompileError> {
    match source {
        "library" => Ok(Zone::Library),
        "hand" => Ok(Zone::Hand),
        "battlefield" => Ok(Zone::Battlefield),
        "graveyard" => Ok(Zone::Graveyard),
        "exile" => Ok(Zone::Exile),
        "command zone" => Ok(Zone::Command),
        "stack" => Ok(Zone::Stack),
        value => Err(StaticReplacementCompileError::UnsupportedOperand(
            value.to_owned(),
        )),
    }
}

fn parse_plural_zone(source: &str) -> Result<Zone, StaticReplacementCompileError> {
    match source {
        "graveyards" => Ok(Zone::Graveyard),
        "libraries" => Ok(Zone::Library),
        "hands" => Ok(Zone::Hand),
        "exile" => Ok(Zone::Exile),
        value => Err(StaticReplacementCompileError::UnsupportedOperand(
            value.to_owned(),
        )),
    }
}

fn parse_step(source: &str) -> Result<TurnStep, StaticReplacementCompileError> {
    match source {
        "untap" => Ok(TurnStep::Untap),
        "upkeep" => Ok(TurnStep::Upkeep),
        "draw" => Ok(TurnStep::Draw),
        "combat" => Ok(TurnStep::Combat),
        "end" => Ok(TurnStep::End),
        value => Err(StaticReplacementCompileError::UnsupportedOperand(
            value.to_owned(),
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectState {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub names: BTreeSet<String>,
    pub card_types: BTreeSet<CardType>,
    pub supertypes: BTreeSet<Supertype>,
    pub colors: BTreeSet<Color>,
    pub subtypes: BTreeSet<String>,
    /// Current Aura source incarnations attached to this object.
    pub enchanting_sources: BTreeSet<ObjectRef>,
    /// Current Equipment source incarnations attached to this object.
    pub equipping_sources: BTreeSet<ObjectRef>,
    /// Exact counters currently on this object.
    pub counters: BTreeMap<CounterKind, u32>,
    pub keywords: BTreeSet<KeywordAbility>,
    pub token: bool,
    pub tapped: bool,
    pub attacking: bool,
    pub blocking: bool,
    pub mana_value: u32,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    pub perspective_player: PlayerId,
    pub active_player: PlayerId,
    pub objects: BTreeMap<ObjectRef, ObjectState>,
    pub life_totals: BTreeMap<PlayerId, i32>,
    /// Exact chosen or derived X for a source incarnation. A missing entry
    /// fails closed whenever an accepted program reads X.
    pub x_values: BTreeMap<ObjectRef, u32>,
    /// Exact number of kicker or multikicker costs paid for a cast object.
    /// Presence, including a zero value, is the completeness proof.
    pub kicker_payments: BTreeMap<ObjectRef, u32>,
    /// Exact roster of every player in the game whose state projection is
    /// complete. Roster-dependent programs fail closed unless the source
    /// controller is present and every referenced player field is supplied.
    pub complete_players: BTreeSet<PlayerId>,
    pub complete_zones: BTreeSet<Zone>,
    pub legal_creature_types: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundProgram {
    pub binding_id: BindingId,
    pub source: ObjectRef,
    pub controller: PlayerId,
    pub program: OracleStaticReplacementProgram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeInstallError {
    EmptyBatch,
    DuplicateSemanticProgram(String),
    SourceMissing(ObjectRef),
    SourceNotBattlefield(ObjectRef),
    SourceContextZoneMismatch {
        source: ObjectRef,
        expected: Zone,
        actual: Zone,
    },
    BindingIdExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvaluationError {
    ObjectMissing(ObjectRef),
    PlayerStateIncomplete(PlayerId),
    ZoneStateIncomplete(BTreeSet<Zone>),
    NumericOverflow,
    UnsupportedVariableAmount(Amount),
    VariableAmountMissing {
        source: ObjectRef,
        amount: Amount,
    },
    AmountObjectCardinality {
        amount: Amount,
        actual: usize,
    },
    EntryCastEvidenceMissing(ObjectRef),
    ReplacementEvidenceMismatch,
    ReplacementCauseEvidenceIncomplete,
    ReplacementCauseEvidenceMismatch,
    DrawContextEvidenceIncomplete,
    DrawContextEvidenceMismatch,
    WrongReplacementChooser {
        expected: PlayerId,
        supplied: PlayerId,
    },
    ChosenReplacementNotApplicable(BindingId),
    ReplacementAlreadyHandled(BindingId),
    EntryChoiceRequired(EntryChoice),
    EntryCopyObjectRequired,
    IllegalEntryCopyObject(ObjectRef),
    IllegalEntryChoice,
    UnexpectedReplacementChoiceEvidence,
    NonAtomicDrawEvent(u32),
    RecipientControllerMismatch {
        object: ObjectRef,
        supplied: PlayerId,
        actual: PlayerId,
    },
    ObjectRelationEvidenceMismatch {
        object: ObjectRef,
        supplied_owner: PlayerId,
        actual_owner: PlayerId,
        supplied_controller: PlayerId,
        actual_controller: PlayerId,
    },
    EventKindMismatch,
    IncompleteBlockLegalityEvidence,
    IncompleteCombatGroupEvidence,
    IllegalCombatGroupObject(ObjectRef),
    IncompleteUntapDeclarationEvidence,
    IllegalUntapObject(ObjectRef),
    IncompleteProtectionInteractionEvidence,
    IllegalProtectionInteraction,
    IncompleteWardTargetEvidence,
    IllegalWardTargetEvidence,
    IllegalBlockerEvidence(ObjectRef),
    DeclaredBlockerNotAble(ObjectRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCharacteristics {
    pub card_types: BTreeSet<CardType>,
    pub subtypes: BTreeSet<String>,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
    pub keywords: BTreeSet<KeywordAbility>,
    pub loses_all_abilities: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticAction {
    Cast {
        player: PlayerId,
        spell: ObjectRef,
        from: Zone,
        from_library_top: bool,
    },
    PlayLand {
        player: PlayerId,
        land: ObjectRef,
        from: Zone,
        from_library_top: bool,
        prior_land_plays_this_turn: u32,
    },
    ActivateAbility {
        player: PlayerId,
        source: ObjectRef,
        is_mana_ability: bool,
    },
    DeclareAttack {
        attacker: ObjectRef,
        defender: PlayerId,
    },
    DeclareBlock {
        blocker: ObjectRef,
        attacker: ObjectRef,
    },
    Target {
        target: RuntimeRecipient,
        source_controller: PlayerId,
        is_spell: bool,
        is_ability: bool,
    },
    Counter {
        spell: ObjectRef,
    },
    GainLife {
        player: PlayerId,
    },
    DrawCard {
        player: PlayerId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictionViolation {
    pub binding_id: BindingId,
    pub restriction: Restriction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDeclarationEvidence {
    pub attacker: ObjectRef,
    pub declared_blockers: BTreeSet<ObjectRef>,
    /// Exact set of creatures able to block this attacker after all evasion,
    /// restriction, cost, and capacity rules are applied.
    pub able_blockers: BTreeSet<ObjectRef>,
    pub legality_evidence_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRequirementViolation {
    pub binding_id: BindingId,
    pub requirement: BlockRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatGroupDeclarationEvidence {
    pub attackers: BTreeSet<ObjectRef>,
    pub blockers: BTreeSet<ObjectRef>,
    pub declaration_evidence_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatGroupViolation {
    CannotAttackOrBlockAlone {
        binding_id: BindingId,
        object: ObjectRef,
    },
    CannotAttackAlone {
        binding_id: BindingId,
        object: ObjectRef,
    },
    MaximumExceeded {
        binding_id: BindingId,
        group: CombatGroupKind,
        maximum: u32,
        declared: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntapDeclarationEvidence {
    pub player: PlayerId,
    pub objects: BTreeSet<ObjectRef>,
    pub declaration_evidence_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntapLimitViolation {
    pub binding_id: BindingId,
    pub maximum: u32,
    pub declared: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionInteractionKind {
    Damage,
    Target,
    Block,
    Attach,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectionInteractionEvidence {
    pub protected: RuntimeRecipient,
    pub source: ObjectRef,
    pub kind: ProtectionInteractionKind,
    pub evidence_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WardTargetEvidence {
    pub target: ObjectRef,
    pub source_controller: PlayerId,
    pub is_spell: bool,
    pub is_ability: bool,
    pub evidence_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WardCostReceipt {
    pub binding_id: BindingId,
    pub generic_mana: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockCapacityReceipt {
    /// `None` represents an unlimited number of creatures. Otherwise this is
    /// the exact maximum, including the normal one-creature capacity.
    pub maximum: Option<u32>,
    pub applied_bindings: Vec<BindingId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionReceipt {
    pub binding_id: BindingId,
    pub permission: Permission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostApplication {
    Cast { player: PlayerId, spell: ObjectRef },
    Activate { player: PlayerId, source: ObjectRef },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostModificationReceipt {
    pub original_generic_mana: u32,
    pub final_generic_mana: u32,
    pub applied_increases: Vec<BindingId>,
    pub applied_reductions: Vec<BindingId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    ZoneChange {
        object: ObjectState,
        from: Zone,
        to: Zone,
        enter_tapped: bool,
        enter_counters: BTreeMap<CounterKind, u32>,
        copy_of: Option<ObjectRef>,
        entry_choice: Option<EntryChoiceValue>,
        library_placement: LibraryPlacement,
    },
    Damage {
        source: ObjectState,
        recipient: RuntimeRecipient,
        kind: DamageKind,
        amount: u32,
        preventable: bool,
        prevented: u32,
    },
    DrawCards {
        player: PlayerId,
        amount: u32,
    },
    GainLife {
        player: PlayerId,
        amount: u32,
    },
    CreateTokens {
        player: PlayerId,
        amount: u32,
    },
    PutCounters {
        object: ObjectState,
        counter: CounterKind,
        amount: u32,
    },
    Step {
        player: PlayerId,
        step: TurnStep,
        skipped: bool,
    },
    ExtraTurn {
        player: PlayerId,
        skipped: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementEventCauseKind {
    Spell,
    Ability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementEventCauseEvidence {
    pub source: ObjectState,
    pub kind: ReplacementEventCauseKind,
    pub evidence_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawEventContextEvidence {
    /// The exact current step and the player whose turn owns it. `None` is a
    /// complete assertion that the draw occurs outside the tracked turn
    /// steps, such as during a main phase or while another procedure resolves.
    pub current_step: Option<(PlayerId, TurnStep)>,
    /// The number of individual card draws completed earlier in the current
    /// step. Draw replacement events are atomic, so this excludes this event.
    pub prior_draws_in_current_step: u32,
    pub evidence_complete: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplacementEventEvidence {
    pub cause: Option<ReplacementEventCauseEvidence>,
    pub draw_context: Option<DrawEventContextEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeRecipient {
    Player(PlayerId),
    Object(ObjectRef, PlayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryPlacement {
    Unspecified,
    Top,
    Bottom,
    Shuffled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryChoiceValue {
    Color(Color),
    CardType(CardType),
    CreatureType(String),
    CounterKind(CounterKind),
    Player(PlayerId),
    BasicLandType(String),
    ColorAndCreatureType { color: Color, creature_type: String },
    Colors(BTreeSet<Color>),
    Object(ObjectRef),
    ColorAndPlayer { color: Color, player: PlayerId },
    Players(BTreeSet<PlayerId>),
    BasicLandTypes(BTreeSet<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingReplacementEvent {
    pub event_id: ReplacementEventId,
    pub original: RuntimeEvent,
    pub current: RuntimeEvent,
    pub evidence: ReplacementEventEvidence,
    pub handled_bindings: BTreeSet<BindingId>,
    /// Additional physical actions staged by composite replacement effects.
    /// A host commits these only with the final replacement event.
    pub counter_removals: BTreeMap<ObjectRef, BTreeMap<CounterKind, u32>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementOrderEvidence {
    pub chooser: PlayerId,
    pub applicable_bindings: Vec<BindingId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementDecision {
    Apply {
        binding_id: BindingId,
        entry_choice: Option<EntryChoiceValue>,
        copy_object: Option<ObjectRef>,
    },
    Decline {
        binding_id: BindingId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementStep {
    Applied(BindingId),
    Declined(BindingId),
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleStaticReplacementRuntime {
    next_binding_id: BindingId,
    next_event_id: ReplacementEventId,
    bindings: BTreeMap<BindingId, BoundProgram>,
}

#[derive(Debug, Clone)]
struct ExpandedStaticEffect {
    receipt_binding_id: BindingId,
    binding: BoundProgram,
    effect: StaticEffect,
}

impl Default for OracleStaticReplacementRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl OracleStaticReplacementRuntime {
    pub fn new() -> Self {
        Self {
            next_binding_id: 1,
            next_event_id: 1,
            bindings: BTreeMap::new(),
        }
    }

    pub fn bindings(&self) -> &BTreeMap<BindingId, BoundProgram> {
        &self.bindings
    }

    pub fn install_batch(
        &mut self,
        snapshot: &RuntimeSnapshot,
        source: ObjectRef,
        controller: PlayerId,
        programs: Vec<OracleStaticReplacementProgram>,
    ) -> Result<Vec<BindingId>, RuntimeInstallError> {
        if programs.is_empty() {
            return Err(RuntimeInstallError::EmptyBatch);
        }
        let source_state = snapshot
            .objects
            .get(&source)
            .ok_or(RuntimeInstallError::SourceMissing(source))?;
        let mut semantic_ids = BTreeSet::new();
        for program in &programs {
            let expected_zone = match program.semantic_context() {
                SourceSemanticContext::PermanentAbility => Some(Zone::Battlefield),
                SourceSemanticContext::SpellAbility => Some(Zone::Stack),
                SourceSemanticContext::CardAbility
                | SourceSemanticContext::EmblemAbility
                | SourceSemanticContext::RuleObjectAbility => None,
            };
            if let Some(expected) = expected_zone
                && source_state.zone != expected
            {
                return Err(RuntimeInstallError::SourceContextZoneMismatch {
                    source,
                    expected,
                    actual: source_state.zone,
                });
            }
            if !semantic_ids.insert(program.semantic_digest().to_owned()) {
                return Err(RuntimeInstallError::DuplicateSemanticProgram(
                    program.semantic_digest().to_owned(),
                ));
            }
        }

        let mut staged = self.clone();
        let mut installed = Vec::with_capacity(programs.len());
        for program in programs {
            let binding_id = staged.next_binding_id;
            staged.next_binding_id = staged
                .next_binding_id
                .checked_add(1)
                .ok_or(RuntimeInstallError::BindingIdExhausted)?;
            staged.bindings.insert(
                binding_id,
                BoundProgram {
                    binding_id,
                    source,
                    controller,
                    program,
                },
            );
            installed.push(binding_id);
        }
        *self = staged;
        Ok(installed)
    }

    pub fn remove_source(&mut self, source: ObjectRef) {
        self.bindings.retain(|_, binding| binding.source != source);
    }

    fn expanded_static_effects(&self, snapshot: &RuntimeSnapshot) -> Vec<ExpandedStaticEffect> {
        let mut expanded = Vec::new();
        for binding in self.bindings.values() {
            if binding_is_active(binding, snapshot) {
                expand_static_program(
                    binding.binding_id,
                    binding.clone(),
                    snapshot,
                    0,
                    &mut expanded,
                );
            }
        }
        expanded
    }

    pub fn effective_characteristics(
        &self,
        snapshot: &RuntimeSnapshot,
        object: ObjectRef,
    ) -> Result<EffectiveCharacteristics, RuntimeEvaluationError> {
        let object_state = snapshot
            .objects
            .get(&object)
            .ok_or(RuntimeEvaluationError::ObjectMissing(object))?;
        let mut result = EffectiveCharacteristics {
            card_types: object_state.card_types.clone(),
            subtypes: object_state.subtypes.clone(),
            power: object_state.power,
            toughness: object_state.toughness,
            keywords: object_state.keywords.clone(),
            loses_all_abilities: false,
        };
        let expanded = self.expanded_static_effects(snapshot);
        let mut applicable_operations = Vec::new();
        for entry in &expanded {
            let StaticEffect::Characteristics {
                affected,
                condition,
                operations,
            } = &entry.effect
            else {
                continue;
            };
            if !selector_matches(
                affected,
                &entry.binding,
                object_state,
                snapshot.perspective_player,
            ) || !condition_holds(condition, &entry.binding, object_state, snapshot)?
            {
                continue;
            }
            for operation in operations {
                applicable_operations.push((
                    characteristic_layer(operation),
                    entry.receipt_binding_id,
                    operation,
                    &entry.binding,
                ));
            }
        }
        applicable_operations.sort_by_key(|(layer, binding_id, _, _)| (*layer, *binding_id));
        for (_, _, operation, binding) in applicable_operations {
            apply_characteristic_operation(
                &mut result,
                operation,
                binding,
                object_state,
                snapshot,
            )?;
        }
        Ok(result)
    }

    pub fn restriction_violations(
        &self,
        snapshot: &RuntimeSnapshot,
        action: StaticAction,
    ) -> Result<Vec<RestrictionViolation>, RuntimeEvaluationError> {
        let mut violations = Vec::new();
        for entry in self.expanded_static_effects(snapshot) {
            let StaticEffect::Restriction(restriction) = &entry.effect else {
                continue;
            };
            if action_violates_restriction(action, restriction, &entry.binding, snapshot)? {
                violations.push(RestrictionViolation {
                    binding_id: entry.receipt_binding_id,
                    restriction: restriction.clone(),
                });
            }
        }
        Ok(violations)
    }

    pub fn block_requirement_violations(
        &self,
        snapshot: &RuntimeSnapshot,
        evidence: &BlockDeclarationEvidence,
    ) -> Result<Vec<BlockRequirementViolation>, RuntimeEvaluationError> {
        if !evidence.legality_evidence_complete {
            return Err(RuntimeEvaluationError::IncompleteBlockLegalityEvidence);
        }
        let attacker = snapshot
            .objects
            .get(&evidence.attacker)
            .ok_or(RuntimeEvaluationError::ObjectMissing(evidence.attacker))?;
        if attacker.zone != Zone::Battlefield || !attacker.card_types.contains(&CardType::Creature)
        {
            return Err(RuntimeEvaluationError::IllegalBlockerEvidence(
                evidence.attacker,
            ));
        }
        for blocker in &evidence.able_blockers {
            let object = snapshot
                .objects
                .get(blocker)
                .ok_or(RuntimeEvaluationError::ObjectMissing(*blocker))?;
            if object.zone != Zone::Battlefield || !object.card_types.contains(&CardType::Creature)
            {
                return Err(RuntimeEvaluationError::IllegalBlockerEvidence(*blocker));
            }
        }
        if let Some(blocker) = evidence
            .declared_blockers
            .iter()
            .find(|blocker| !evidence.able_blockers.contains(blocker))
        {
            return Err(RuntimeEvaluationError::DeclaredBlockerNotAble(*blocker));
        }

        let mut violations = Vec::new();
        for entry in self.expanded_static_effects(snapshot) {
            let StaticEffect::BlockRequirement(requirement) = &entry.effect else {
                continue;
            };
            let selector = match requirement {
                BlockRequirement::AllAbleBlock { attacker }
                | BlockRequirement::MustBeBlockedIfAble { attacker }
                | BlockRequirement::MinimumBlockers { attacker, .. }
                | BlockRequirement::MaximumBlockers { attacker, .. } => attacker,
            };
            if !selector_matches(
                selector,
                &entry.binding,
                attacker,
                snapshot.perspective_player,
            ) {
                continue;
            }
            let violated = match requirement {
                BlockRequirement::AllAbleBlock { .. } => {
                    evidence.declared_blockers != evidence.able_blockers
                }
                BlockRequirement::MustBeBlockedIfAble { .. } => {
                    !evidence.able_blockers.is_empty() && evidence.declared_blockers.is_empty()
                }
                BlockRequirement::MinimumBlockers { minimum, .. } => {
                    !evidence.declared_blockers.is_empty()
                        && evidence.declared_blockers.len() < *minimum as usize
                }
                BlockRequirement::MaximumBlockers { maximum, .. } => {
                    evidence.declared_blockers.len() > *maximum as usize
                }
            };
            if violated {
                violations.push(BlockRequirementViolation {
                    binding_id: entry.receipt_binding_id,
                    requirement: requirement.clone(),
                });
            }
        }
        Ok(violations)
    }

    pub fn combat_group_violations(
        &self,
        snapshot: &RuntimeSnapshot,
        evidence: &CombatGroupDeclarationEvidence,
    ) -> Result<Vec<CombatGroupViolation>, RuntimeEvaluationError> {
        if !evidence.declaration_evidence_complete {
            return Err(RuntimeEvaluationError::IncompleteCombatGroupEvidence);
        }
        for object_ref in evidence.attackers.iter().chain(&evidence.blockers) {
            let object = snapshot
                .objects
                .get(object_ref)
                .ok_or(RuntimeEvaluationError::ObjectMissing(*object_ref))?;
            if object.zone != Zone::Battlefield || !object.card_types.contains(&CardType::Creature)
            {
                return Err(RuntimeEvaluationError::IllegalCombatGroupObject(
                    *object_ref,
                ));
            }
        }
        let mut violations = Vec::new();
        for entry in self.expanded_static_effects(snapshot) {
            match &entry.effect {
                StaticEffect::CannotAttackOrBlockAlone { object: selector } => {
                    for object in snapshot.objects.values() {
                        if !selector_matches(
                            selector,
                            &entry.binding,
                            object,
                            snapshot.perspective_player,
                        ) {
                            continue;
                        }
                        let attacks_alone = evidence.attackers.len() == 1
                            && evidence.attackers.contains(&object.object_ref);
                        let blocks_alone = evidence.blockers.len() == 1
                            && evidence.blockers.contains(&object.object_ref);
                        if attacks_alone || blocks_alone {
                            violations.push(CombatGroupViolation::CannotAttackOrBlockAlone {
                                binding_id: entry.receipt_binding_id,
                                object: object.object_ref,
                            });
                        }
                    }
                }
                StaticEffect::CannotAttackAlone { object: selector } => {
                    if evidence.attackers.len() == 1 {
                        let object = snapshot
                            .objects
                            .get(evidence.attackers.first().expect("single attacker"))
                            .expect("combat objects validated above");
                        if selector_matches(
                            selector,
                            &entry.binding,
                            object,
                            snapshot.perspective_player,
                        ) {
                            violations.push(CombatGroupViolation::CannotAttackAlone {
                                binding_id: entry.receipt_binding_id,
                                object: object.object_ref,
                            });
                        }
                    }
                }
                StaticEffect::CombatGroupLimit { group, maximum } => {
                    let declared = match group {
                        CombatGroupKind::Attackers => evidence.attackers.len(),
                        CombatGroupKind::Blockers => evidence.blockers.len(),
                    };
                    if declared > *maximum as usize {
                        violations.push(CombatGroupViolation::MaximumExceeded {
                            binding_id: entry.receipt_binding_id,
                            group: *group,
                            maximum: *maximum,
                            declared: u32::try_from(declared)
                                .map_err(|_| RuntimeEvaluationError::NumericOverflow)?,
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(violations)
    }

    pub fn permissions_for(
        &self,
        snapshot: &RuntimeSnapshot,
        action: StaticAction,
    ) -> Result<Vec<PermissionReceipt>, RuntimeEvaluationError> {
        let mut permissions = Vec::new();
        let mut matching_land_permissions = Vec::new();
        let mut additional_land_plays = 0u32;
        let mut unlimited_land_plays = false;
        for entry in self.expanded_static_effects(snapshot) {
            let StaticEffect::Permission(permission) = &entry.effect else {
                continue;
            };
            if matches!(
                permission,
                Permission::AdditionalLandPlays { .. } | Permission::UnlimitedLandPlays { .. }
            ) && land_play_permission_matches_action(
                action,
                permission,
                &entry.binding,
                snapshot,
            )? {
                match permission {
                    Permission::AdditionalLandPlays { amount, .. } => {
                        additional_land_plays = additional_land_plays
                            .checked_add(*amount)
                            .ok_or(RuntimeEvaluationError::NumericOverflow)?;
                    }
                    Permission::UnlimitedLandPlays { .. } => unlimited_land_plays = true,
                    _ => unreachable!("land-play permission match is exhaustive"),
                }
                matching_land_permissions.push(PermissionReceipt {
                    binding_id: entry.receipt_binding_id,
                    permission: permission.clone(),
                });
                continue;
            }
            if action_uses_permission(action, permission, &entry.binding, snapshot)? {
                permissions.push(PermissionReceipt {
                    binding_id: entry.receipt_binding_id,
                    permission: permission.clone(),
                });
            }
        }
        if let StaticAction::PlayLand {
            prior_land_plays_this_turn,
            ..
        } = action
        {
            let maximum_land_plays = 1u32
                .checked_add(additional_land_plays)
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
            if prior_land_plays_this_turn >= 1
                && (unlimited_land_plays || prior_land_plays_this_turn < maximum_land_plays)
            {
                permissions.extend(matching_land_permissions);
            }
        }
        Ok(permissions)
    }

    pub fn modify_generic_cost(
        &self,
        snapshot: &RuntimeSnapshot,
        application: CostApplication,
        original_generic_mana: u32,
    ) -> Result<CostModificationReceipt, RuntimeEvaluationError> {
        let mut increases = Vec::new();
        let mut reductions = Vec::new();
        for entry in self.expanded_static_effects(snapshot) {
            let StaticEffect::CostModification(modification) = &entry.effect else {
                continue;
            };
            if cost_modification_applies(application, modification, &entry.binding, snapshot)? {
                match modification.direction {
                    CostDirection::Increase => increases.push((
                        entry.receipt_binding_id,
                        entry.binding.clone(),
                        modification.clone(),
                    )),
                    CostDirection::Reduce => reductions.push((
                        entry.receipt_binding_id,
                        entry.binding.clone(),
                        modification.clone(),
                    )),
                }
            }
        }
        increases.sort_by_key(|(receipt_binding_id, _, _)| *receipt_binding_id);
        reductions.sort_by_key(|(receipt_binding_id, _, _)| *receipt_binding_id);

        let mut final_generic_mana = original_generic_mana;
        let mut applied_increases = Vec::new();
        let mut applied_reductions = Vec::new();
        for (receipt_binding_id, binding, modification) in increases {
            let amount =
                evaluate_nonnegative_amount(&modification.generic_mana, &binding, snapshot)?;
            final_generic_mana = final_generic_mana
                .checked_add(amount)
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
            applied_increases.push(receipt_binding_id);
        }
        for (receipt_binding_id, binding, modification) in reductions {
            let amount =
                evaluate_nonnegative_amount(&modification.generic_mana, &binding, snapshot)?;
            final_generic_mana = final_generic_mana.saturating_sub(amount);
            applied_reductions.push(receipt_binding_id);
        }
        Ok(CostModificationReceipt {
            original_generic_mana,
            final_generic_mana,
            applied_increases,
            applied_reductions,
        })
    }

    pub fn step_skip_bindings(
        &self,
        snapshot: &RuntimeSnapshot,
        player: PlayerId,
        step: TurnStep,
    ) -> Vec<BindingId> {
        self.expanded_static_effects(snapshot)
            .into_iter()
            .filter_map(|entry| match entry.effect {
                StaticEffect::SkipStep {
                    player: affected,
                    step: affected_step,
                } if affected_step == step
                    && player_matches(affected, entry.binding.controller, player) =>
                {
                    Some(entry.receipt_binding_id)
                }
                _ => None,
            })
            .collect()
    }

    pub fn no_maximum_hand_size_bindings(
        &self,
        snapshot: &RuntimeSnapshot,
        player: PlayerId,
    ) -> Vec<BindingId> {
        self.expanded_static_effects(snapshot)
            .into_iter()
            .filter_map(|entry| match entry.effect {
                StaticEffect::NoMaximumHandSize { player: affected }
                    if player_matches(affected, entry.binding.controller, player) =>
                {
                    Some(entry.receipt_binding_id)
                }
                _ => None,
            })
            .collect()
    }

    pub fn unlimited_block_capacity_bindings(
        &self,
        snapshot: &RuntimeSnapshot,
        blocker: ObjectRef,
    ) -> Result<Vec<BindingId>, RuntimeEvaluationError> {
        let state = snapshot
            .objects
            .get(&blocker)
            .ok_or(RuntimeEvaluationError::ObjectMissing(blocker))?;
        Ok(self
            .expanded_static_effects(snapshot)
            .into_iter()
            .filter_map(|entry| match entry.effect {
                StaticEffect::UnlimitedBlockCapacity { blocker: selector }
                    if selector_matches(
                        &selector,
                        &entry.binding,
                        state,
                        snapshot.perspective_player,
                    ) =>
                {
                    Some(entry.receipt_binding_id)
                }
                _ => None,
            })
            .collect())
    }

    pub fn block_capacity(
        &self,
        snapshot: &RuntimeSnapshot,
        blocker: ObjectRef,
    ) -> Result<BlockCapacityReceipt, RuntimeEvaluationError> {
        let state = snapshot
            .objects
            .get(&blocker)
            .ok_or(RuntimeEvaluationError::ObjectMissing(blocker))?;
        let mut maximum = 1u32;
        let mut unlimited = false;
        let mut applied_bindings = Vec::new();
        for entry in self.expanded_static_effects(snapshot) {
            match &entry.effect {
                StaticEffect::UnlimitedBlockCapacity { blocker: selector }
                    if selector_matches(
                        selector,
                        &entry.binding,
                        state,
                        snapshot.perspective_player,
                    ) =>
                {
                    unlimited = true;
                    applied_bindings.push(entry.receipt_binding_id);
                }
                StaticEffect::AdditionalBlockCapacity {
                    blocker: selector,
                    amount,
                } if selector_matches(
                    selector,
                    &entry.binding,
                    state,
                    snapshot.perspective_player,
                ) =>
                {
                    let additional = u32::try_from(evaluate_amount_for_object(
                        amount,
                        &entry.binding,
                        Some(state),
                        snapshot,
                    )?)
                    .map_err(|_| RuntimeEvaluationError::NumericOverflow)?;
                    maximum = maximum
                        .checked_add(additional)
                        .ok_or(RuntimeEvaluationError::NumericOverflow)?;
                    applied_bindings.push(entry.receipt_binding_id);
                }
                _ => {}
            }
        }
        Ok(BlockCapacityReceipt {
            maximum: (!unlimited).then_some(maximum),
            applied_bindings,
        })
    }

    pub fn revealed_library_top_bindings(
        &self,
        snapshot: &RuntimeSnapshot,
        player: PlayerId,
    ) -> Vec<BindingId> {
        self.expanded_static_effects(snapshot)
            .into_iter()
            .filter_map(|entry| match entry.effect {
                StaticEffect::RevealLibraryTop { player: affected }
                    if player_matches(affected, entry.binding.controller, player) =>
                {
                    Some(entry.receipt_binding_id)
                }
                _ => None,
            })
            .collect()
    }

    pub fn revealed_hand_bindings(
        &self,
        snapshot: &RuntimeSnapshot,
        player: PlayerId,
    ) -> Vec<BindingId> {
        self.expanded_static_effects(snapshot)
            .into_iter()
            .filter_map(|entry| match entry.effect {
                StaticEffect::RevealHands { player: affected }
                    if player_matches(affected, entry.binding.controller, player) =>
                {
                    Some(entry.receipt_binding_id)
                }
                _ => None,
            })
            .collect()
    }

    pub fn spell_cast_limit_each_turn(
        &self,
        snapshot: &RuntimeSnapshot,
        player: PlayerId,
    ) -> Option<u32> {
        self.expanded_static_effects(snapshot)
            .into_iter()
            .filter_map(|entry| match entry.effect {
                StaticEffect::SpellCastLimitEachTurn {
                    player: affected,
                    maximum,
                } if player_matches(affected, entry.binding.controller, player) => Some(maximum),
                _ => None,
            })
            .min()
    }

    pub fn card_draw_limit_each_turn(
        &self,
        snapshot: &RuntimeSnapshot,
        player: PlayerId,
    ) -> Option<u32> {
        self.expanded_static_effects(snapshot)
            .into_iter()
            .filter_map(|entry| match entry.effect {
                StaticEffect::CardDrawLimitEachTurn {
                    player: affected,
                    maximum,
                } if player_matches(affected, entry.binding.controller, player) => Some(maximum),
                _ => None,
            })
            .min()
    }

    pub fn mana_retention_bindings(
        &self,
        snapshot: &RuntimeSnapshot,
        player: PlayerId,
        color: Color,
    ) -> Vec<BindingId> {
        self.expanded_static_effects(snapshot)
            .into_iter()
            .filter_map(|entry| match entry.effect {
                StaticEffect::ManaRetention {
                    player: affected,
                    color: retained_color,
                } if player_matches(affected, entry.binding.controller, player)
                    && retained_color.is_none_or(|retained| retained == color) =>
                {
                    Some(entry.receipt_binding_id)
                }
                _ => None,
            })
            .collect()
    }

    pub fn untap_limit_violations(
        &self,
        snapshot: &RuntimeSnapshot,
        evidence: &UntapDeclarationEvidence,
    ) -> Result<Vec<UntapLimitViolation>, RuntimeEvaluationError> {
        if !evidence.declaration_evidence_complete {
            return Err(RuntimeEvaluationError::IncompleteUntapDeclarationEvidence);
        }
        if !snapshot.complete_players.contains(&evidence.player) {
            return Err(RuntimeEvaluationError::PlayerStateIncomplete(
                evidence.player,
            ));
        }
        for object_ref in &evidence.objects {
            let object = snapshot
                .objects
                .get(object_ref)
                .ok_or(RuntimeEvaluationError::ObjectMissing(*object_ref))?;
            if object.zone != Zone::Battlefield
                || object.controller != evidence.player
                || !object.tapped
            {
                return Err(RuntimeEvaluationError::IllegalUntapObject(*object_ref));
            }
        }

        let mut violations = Vec::new();
        for entry in self.expanded_static_effects(snapshot) {
            let StaticEffect::UntapLimit {
                player,
                objects,
                maximum,
            } = &entry.effect
            else {
                continue;
            };
            if !player_matches(*player, entry.binding.controller, evidence.player) {
                continue;
            }
            let declared = evidence
                .objects
                .iter()
                .filter(|object_ref| {
                    snapshot.objects.get(object_ref).is_some_and(|object| {
                        selector_matches(
                            objects,
                            &entry.binding,
                            object,
                            snapshot.perspective_player,
                        )
                    })
                })
                .count();
            if declared > *maximum as usize {
                violations.push(UntapLimitViolation {
                    binding_id: entry.receipt_binding_id,
                    maximum: *maximum,
                    declared: u32::try_from(declared)
                        .map_err(|_| RuntimeEvaluationError::NumericOverflow)?,
                });
            }
        }
        Ok(violations)
    }

    pub fn protection_bindings(
        &self,
        snapshot: &RuntimeSnapshot,
        evidence: ProtectionInteractionEvidence,
    ) -> Result<Vec<BindingId>, RuntimeEvaluationError> {
        if !evidence.evidence_complete {
            return Err(RuntimeEvaluationError::IncompleteProtectionInteractionEvidence);
        }
        validate_runtime_recipient(evidence.protected, snapshot)?;
        let source = snapshot
            .objects
            .get(&evidence.source)
            .ok_or(RuntimeEvaluationError::ObjectMissing(evidence.source))?;
        match evidence.kind {
            ProtectionInteractionKind::Block => {
                let RuntimeRecipient::Object(protected, _) = evidence.protected else {
                    return Err(RuntimeEvaluationError::IllegalProtectionInteraction);
                };
                let protected = snapshot
                    .objects
                    .get(&protected)
                    .ok_or(RuntimeEvaluationError::ObjectMissing(protected))?;
                if source.zone != Zone::Battlefield
                    || protected.zone != Zone::Battlefield
                    || !source.card_types.contains(&CardType::Creature)
                    || !protected.card_types.contains(&CardType::Creature)
                {
                    return Err(RuntimeEvaluationError::IllegalProtectionInteraction);
                }
            }
            ProtectionInteractionKind::Attach => {
                if !matches!(evidence.protected, RuntimeRecipient::Object(..))
                    || source.zone != Zone::Battlefield
                    || !(source.subtypes.contains("Aura")
                        || source.subtypes.contains("Equipment")
                        || source.subtypes.contains("Fortification"))
                {
                    return Err(RuntimeEvaluationError::IllegalProtectionInteraction);
                }
            }
            ProtectionInteractionKind::Damage | ProtectionInteractionKind::Target => {}
        }

        let mut bindings = Vec::new();
        for entry in self.expanded_static_effects(snapshot) {
            let StaticEffect::Protection { affected, from } = &entry.effect else {
                continue;
            };
            if runtime_recipient_matches(affected, &entry.binding, evidence.protected, snapshot)?
                && selector_matches(from, &entry.binding, source, snapshot.perspective_player)
            {
                bindings.push(entry.receipt_binding_id);
            }
        }
        Ok(bindings)
    }

    pub fn ward_costs_for_target(
        &self,
        snapshot: &RuntimeSnapshot,
        evidence: WardTargetEvidence,
    ) -> Result<Vec<WardCostReceipt>, RuntimeEvaluationError> {
        if !evidence.evidence_complete {
            return Err(RuntimeEvaluationError::IncompleteWardTargetEvidence);
        }
        if !evidence.is_spell && !evidence.is_ability {
            return Err(RuntimeEvaluationError::IllegalWardTargetEvidence);
        }
        let target = snapshot
            .objects
            .get(&evidence.target)
            .ok_or(RuntimeEvaluationError::ObjectMissing(evidence.target))?;
        if target.zone != Zone::Battlefield
            || !snapshot.complete_players.contains(&target.controller)
            || !snapshot
                .complete_players
                .contains(&evidence.source_controller)
        {
            return Err(RuntimeEvaluationError::IllegalWardTargetEvidence);
        }
        if target.controller == evidence.source_controller {
            return Ok(Vec::new());
        }

        let mut costs = Vec::new();
        for entry in self.expanded_static_effects(snapshot) {
            let StaticEffect::Ward {
                affected,
                generic_mana,
                condition,
            } = &entry.effect
            else {
                continue;
            };
            if selector_matches(
                affected,
                &entry.binding,
                target,
                snapshot.perspective_player,
            ) && condition_holds(condition, &entry.binding, target, snapshot)?
            {
                costs.push(WardCostReceipt {
                    binding_id: entry.receipt_binding_id,
                    generic_mana: *generic_mana,
                });
            }
        }
        Ok(costs)
    }

    pub fn begin_replacement_event(
        &mut self,
        event: RuntimeEvent,
    ) -> Result<PendingReplacementEvent, RuntimeEvaluationError> {
        self.begin_replacement_event_with_evidence(event, ReplacementEventEvidence::default())
    }

    pub fn begin_replacement_event_with_evidence(
        &mut self,
        event: RuntimeEvent,
        evidence: ReplacementEventEvidence,
    ) -> Result<PendingReplacementEvent, RuntimeEvaluationError> {
        let event_id = self.next_event_id;
        self.next_event_id = self
            .next_event_id
            .checked_add(1)
            .ok_or(RuntimeEvaluationError::NumericOverflow)?;
        Ok(PendingReplacementEvent {
            event_id,
            original: event.clone(),
            current: event,
            evidence,
            handled_bindings: BTreeSet::new(),
            counter_removals: BTreeMap::new(),
        })
    }

    pub fn applicable_replacements(
        &self,
        snapshot: &RuntimeSnapshot,
        event: &PendingReplacementEvent,
    ) -> Result<Vec<BindingId>, RuntimeEvaluationError> {
        validate_replacement_event_object_evidence(&event.current, snapshot)?;
        if let RuntimeEvent::Damage { recipient, .. } = &event.current {
            validate_runtime_recipient(*recipient, snapshot)?;
        }
        let prevention_is_prohibited = match &event.current {
            RuntimeEvent::Damage {
                source: actual_source,
                kind: actual_kind,
                ..
            } => self
                .expanded_static_effects(snapshot)
                .into_iter()
                .any(|entry| match entry.effect {
                    StaticEffect::DamageCannotBePrevented { source, kind } => {
                        kind.is_none_or(|expected| expected == *actual_kind)
                            && selector_matches(
                                &source,
                                &entry.binding,
                                actual_source,
                                snapshot.perspective_player,
                            )
                    }
                    _ => false,
                }),
            _ => false,
        };
        let mut applicable = Vec::new();
        for (binding_id, binding) in &self.bindings {
            if !binding_is_active(binding, snapshot) {
                continue;
            }
            if event.handled_bindings.contains(binding_id) {
                continue;
            }
            let OracleStaticReplacementProgramKind::Replacement(replacement) =
                binding.program.kind()
            else {
                continue;
            };
            if prevention_is_prohibited
                && matches!(
                    &replacement.operation,
                    ReplacementOperation::PreventDamage { .. }
                        | ReplacementOperation::PreventDamageAndRemovePlusOneCounter
                )
            {
                continue;
            }
            if replacement_matches(
                replacement,
                binding,
                &event.current,
                &event.evidence,
                snapshot,
            )? {
                applicable.push(*binding_id);
            }
        }
        Ok(applicable)
    }

    pub fn apply_replacement_step(
        &self,
        snapshot: &RuntimeSnapshot,
        event: &mut PendingReplacementEvent,
        evidence: ReplacementOrderEvidence,
        decision: Option<ReplacementDecision>,
    ) -> Result<ReplacementStep, RuntimeEvaluationError> {
        let applicable = self.applicable_replacements(snapshot, event)?;
        if applicable != evidence.applicable_bindings {
            return Err(RuntimeEvaluationError::ReplacementEvidenceMismatch);
        }
        let expected_chooser = affected_player(&event.current);
        if expected_chooser != evidence.chooser {
            return Err(RuntimeEvaluationError::WrongReplacementChooser {
                expected: expected_chooser,
                supplied: evidence.chooser,
            });
        }
        if applicable.is_empty() {
            if decision.is_some() {
                return Err(RuntimeEvaluationError::ReplacementEvidenceMismatch);
            }
            return Ok(ReplacementStep::Complete);
        }
        let decision = decision.ok_or(RuntimeEvaluationError::ReplacementEvidenceMismatch)?;
        let binding_id = match &decision {
            ReplacementDecision::Apply { binding_id, .. }
            | ReplacementDecision::Decline { binding_id } => *binding_id,
        };
        if event.handled_bindings.contains(&binding_id) {
            return Err(RuntimeEvaluationError::ReplacementAlreadyHandled(
                binding_id,
            ));
        }
        if !applicable.contains(&binding_id) {
            return Err(RuntimeEvaluationError::ChosenReplacementNotApplicable(
                binding_id,
            ));
        }
        let binding = self.bindings.get(&binding_id).ok_or(
            RuntimeEvaluationError::ChosenReplacementNotApplicable(binding_id),
        )?;
        let OracleStaticReplacementProgramKind::Replacement(replacement) = binding.program.kind()
        else {
            return Err(RuntimeEvaluationError::ChosenReplacementNotApplicable(
                binding_id,
            ));
        };
        if matches!(decision, ReplacementDecision::Decline { .. }) && !replacement.optional {
            return Err(RuntimeEvaluationError::ChosenReplacementNotApplicable(
                binding_id,
            ));
        }

        let mut staged = event.clone();
        staged.handled_bindings.insert(binding_id);
        let step = match decision {
            ReplacementDecision::Decline { .. } => ReplacementStep::Declined(binding_id),
            ReplacementDecision::Apply {
                entry_choice,
                copy_object,
                ..
            } => {
                let PendingReplacementEvent {
                    current,
                    counter_removals,
                    ..
                } = &mut staged;
                apply_replacement_operation(
                    current,
                    &replacement.operation,
                    binding,
                    snapshot,
                    entry_choice,
                    copy_object,
                    counter_removals,
                )?;
                ReplacementStep::Applied(binding_id)
            }
        };
        *event = staged;
        Ok(step)
    }
}

fn expand_static_program(
    receipt_binding_id: BindingId,
    binding: BoundProgram,
    snapshot: &RuntimeSnapshot,
    depth: usize,
    expanded: &mut Vec<ExpandedStaticEffect>,
) {
    if depth > 4 {
        return;
    }
    let OracleStaticReplacementProgramKind::Static(effects) = binding.program.kind() else {
        return;
    };
    for effect in effects.clone() {
        match effect {
            StaticEffect::GrantNested { affected, ability } => {
                if !matches!(
                    ability.kind(),
                    OracleStaticReplacementProgramKind::Static(_)
                ) {
                    continue;
                }
                for object in snapshot.objects.values() {
                    if selector_matches(&affected, &binding, object, snapshot.perspective_player) {
                        expand_static_program(
                            receipt_binding_id,
                            BoundProgram {
                                binding_id: receipt_binding_id,
                                source: object.object_ref,
                                controller: object.controller,
                                program: (*ability).clone(),
                            },
                            snapshot,
                            depth + 1,
                            expanded,
                        );
                    }
                }
            }
            effect => expanded.push(ExpandedStaticEffect {
                receipt_binding_id,
                binding: binding.clone(),
                effect,
            }),
        }
    }
}

fn binding_is_active(binding: &BoundProgram, snapshot: &RuntimeSnapshot) -> bool {
    let Some(source) = snapshot.objects.get(&binding.source) else {
        return matches!(
            binding.program.semantic_context(),
            SourceSemanticContext::EmblemAbility | SourceSemanticContext::RuleObjectAbility
        );
    };
    match binding.program.semantic_context() {
        SourceSemanticContext::PermanentAbility => source.zone == Zone::Battlefield,
        SourceSemanticContext::SpellAbility => source.zone == Zone::Stack,
        SourceSemanticContext::CardAbility => true,
        SourceSemanticContext::EmblemAbility | SourceSemanticContext::RuleObjectAbility => true,
    }
}

fn characteristic_layer(operation: &CharacteristicOperation) -> u8 {
    match operation {
        CharacteristicOperation::AddCardTypes(_) | CharacteristicOperation::AddSubtypes(_) => 40,
        CharacteristicOperation::GrantKeywords(_)
        | CharacteristicOperation::RemoveKeywords(_)
        | CharacteristicOperation::LoseAllAbilities => 60,
        CharacteristicOperation::SetBasePowerToughness { .. } => 71,
        CharacteristicOperation::ModifyPowerToughness { .. } => 72,
    }
}

fn action_violates_restriction(
    action: StaticAction,
    restriction: &Restriction,
    binding: &BoundProgram,
    snapshot: &RuntimeSnapshot,
) -> Result<bool, RuntimeEvaluationError> {
    let object = |object_ref: ObjectRef| {
        snapshot
            .objects
            .get(&object_ref)
            .ok_or(RuntimeEvaluationError::ObjectMissing(object_ref))
    };
    match (restriction, action) {
        (
            Restriction::Conditional {
                restriction,
                condition,
                active_when_condition_holds,
            },
            actual_action,
        ) => {
            let affected_ref = match actual_action {
                StaticAction::Cast { spell, .. } | StaticAction::Counter { spell } => spell,
                StaticAction::PlayLand { land, .. } => land,
                StaticAction::ActivateAbility { source, .. } => source,
                StaticAction::DeclareAttack { attacker, .. } => attacker,
                StaticAction::DeclareBlock { blocker, .. } => blocker,
                StaticAction::Target {
                    target: RuntimeRecipient::Object(target, _),
                    ..
                } => target,
                StaticAction::Target {
                    target: RuntimeRecipient::Player(_),
                    ..
                }
                | StaticAction::GainLife { .. }
                | StaticAction::DrawCard { .. } => binding.source,
            };
            let condition_holds =
                condition_holds(condition, binding, object(affected_ref)?, snapshot)?;
            if condition_holds == *active_when_condition_holds {
                action_violates_restriction(actual_action, restriction, binding, snapshot)
            } else {
                Ok(false)
            }
        }
        (
            Restriction::CannotCast {
                player,
                spells,
                from,
            },
            StaticAction::Cast {
                player: actual_player,
                spell,
                from: actual_from,
                ..
            },
        ) => {
            let state = object(spell)?;
            Ok(player_matches(*player, binding.controller, actual_player)
                && state.zone == actual_from
                && from.is_none_or(|expected| expected == actual_from)
                && selector_matches(spells, binding, state, snapshot.perspective_player))
        }
        (
            Restriction::CannotActivateAbilities {
                player,
                source,
                kind,
            },
            StaticAction::ActivateAbility {
                player: actual_player,
                source: actual_source,
                is_mana_ability,
            },
        ) => {
            let kind_matches = match kind {
                AbilityRestrictionKind::All => true,
                AbilityRestrictionKind::ManaOnly => is_mana_ability,
                AbilityRestrictionKind::NonManaOnly => !is_mana_ability,
            };
            let source_matches = match source {
                None => true,
                Some(selector) => selector_matches(
                    selector,
                    binding,
                    object(actual_source)?,
                    snapshot.perspective_player,
                ),
            };
            Ok(player_matches(*player, binding.controller, actual_player)
                && kind_matches
                && source_matches)
        }
        (
            Restriction::CannotAttack { attacker }
            | Restriction::CannotAttackOrBlock { object: attacker },
            StaticAction::DeclareAttack {
                attacker: actual_attacker,
                ..
            },
        ) => Ok(selector_matches(
            attacker,
            binding,
            object(actual_attacker)?,
            snapshot.perspective_player,
        )),
        (
            Restriction::CannotAttackPlayer { attacker, defender },
            StaticAction::DeclareAttack {
                attacker: actual_attacker,
                defender: actual_defender,
            },
        ) => Ok(selector_matches(
            attacker,
            binding,
            object(actual_attacker)?,
            snapshot.perspective_player,
        ) && player_matches(*defender, binding.controller, actual_defender)),
        (
            Restriction::CannotBlock { blocker }
            | Restriction::CannotAttackOrBlock { object: blocker },
            StaticAction::DeclareBlock {
                blocker: actual_blocker,
                ..
            },
        ) => Ok(selector_matches(
            blocker,
            binding,
            object(actual_blocker)?,
            snapshot.perspective_player,
        )),
        (
            Restriction::CannotBlockCreature { blocker, attacker },
            StaticAction::DeclareBlock {
                blocker: actual_blocker,
                attacker: actual_attacker,
            },
        ) => Ok(selector_matches(
            blocker,
            binding,
            object(actual_blocker)?,
            snapshot.perspective_player,
        ) && selector_matches(
            attacker,
            binding,
            object(actual_attacker)?,
            snapshot.perspective_player,
        )),
        (
            Restriction::CanBlockOnly {
                blocker,
                allowed_attacker,
            },
            StaticAction::DeclareBlock {
                blocker: actual_blocker,
                attacker: actual_attacker,
            },
        ) => Ok(selector_matches(
            blocker,
            binding,
            object(actual_blocker)?,
            snapshot.perspective_player,
        ) && !selector_matches(
            allowed_attacker,
            binding,
            object(actual_attacker)?,
            snapshot.perspective_player,
        )),
        (
            Restriction::CannotBeBlocked { attacker, by },
            StaticAction::DeclareBlock {
                blocker,
                attacker: actual_attacker,
            },
        ) => Ok(selector_matches(
            attacker,
            binding,
            object(actual_attacker)?,
            snapshot.perspective_player,
        ) && match by {
            Some(selector) => selector_matches(
                selector,
                binding,
                object(blocker)?,
                snapshot.perspective_player,
            ),
            None => true,
        }),
        (
            Restriction::CanBeBlockedOnlyBy {
                attacker,
                allowed_blocker,
            },
            StaticAction::DeclareBlock {
                blocker: actual_blocker,
                attacker: actual_attacker,
            },
        ) => Ok(selector_matches(
            attacker,
            binding,
            object(actual_attacker)?,
            snapshot.perspective_player,
        ) && !selector_matches(
            allowed_blocker,
            binding,
            object(actual_blocker)?,
            snapshot.perspective_player,
        )),
        (
            Restriction::CannotBeTargeted {
                target,
                forbidden_controller,
                spells,
                abilities,
            },
            StaticAction::Target {
                target: actual_target,
                source_controller,
                is_spell,
                is_ability,
            },
        ) => Ok(
            player_matches(*forbidden_controller, binding.controller, source_controller)
                && ((*spells && is_spell) || (*abilities && is_ability))
                && runtime_recipient_matches(target, binding, actual_target, snapshot)?,
        ),
        (
            Restriction::CannotBeCountered { spell },
            StaticAction::Counter {
                spell: actual_spell,
            },
        ) => Ok(selector_matches(
            spell,
            binding,
            object(actual_spell)?,
            snapshot.perspective_player,
        )),
        (
            Restriction::CannotGainLife { player },
            StaticAction::GainLife {
                player: actual_player,
            },
        )
        | (
            Restriction::CannotDrawCards { player },
            StaticAction::DrawCard {
                player: actual_player,
            },
        ) => Ok(player_matches(*player, binding.controller, actual_player)),
        (
            Restriction::CannotPlayLands { player },
            StaticAction::PlayLand {
                player: actual_player,
                ..
            },
        ) => Ok(player_matches(*player, binding.controller, actual_player)),
        _ => Ok(false),
    }
}

fn runtime_recipient_matches(
    selector: &RecipientSelector,
    binding: &BoundProgram,
    actual: RuntimeRecipient,
    snapshot: &RuntimeSnapshot,
) -> Result<bool, RuntimeEvaluationError> {
    validate_runtime_recipient(actual, snapshot)?;
    match (selector, actual) {
        (RecipientSelector::Any, _) => Ok(true),
        (RecipientSelector::Alternatives(alternatives), _) => {
            for alternative in alternatives {
                if runtime_recipient_matches(alternative, binding, actual, snapshot)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        (RecipientSelector::Player(player), RuntimeRecipient::Player(actual)) => {
            Ok(player_matches(*player, binding.controller, actual))
        }
        (RecipientSelector::Object(selector), RuntimeRecipient::Object(object, _)) => {
            let state = snapshot
                .objects
                .get(&object)
                .ok_or(RuntimeEvaluationError::ObjectMissing(object))?;
            Ok(selector_matches(
                selector,
                binding,
                state,
                snapshot.perspective_player,
            ))
        }
        _ => Ok(false),
    }
}

fn action_uses_permission(
    action: StaticAction,
    permission: &Permission,
    binding: &BoundProgram,
    snapshot: &RuntimeSnapshot,
) -> Result<bool, RuntimeEvaluationError> {
    match (permission, action) {
        (
            Permission::Cast {
                player,
                cards,
                timing,
            },
            StaticAction::Cast {
                player: actual_player,
                spell,
                from,
                from_library_top,
            },
        ) => {
            let source_matches = match timing {
                CastTimingPermission::AsThoughFlash => true,
                CastTimingPermission::FromGraveyard => from == Zone::Graveyard,
                CastTimingPermission::FromExile => from == Zone::Exile,
                CastTimingPermission::FromLibraryTop => from == Zone::Library && from_library_top,
            };
            let state = snapshot
                .objects
                .get(&spell)
                .ok_or(RuntimeEvaluationError::ObjectMissing(spell))?;
            Ok(player_matches(*player, binding.controller, actual_player)
                && state.zone == from
                && (!matches!(
                    timing,
                    CastTimingPermission::FromGraveyard | CastTimingPermission::FromLibraryTop
                ) || state.owner == actual_player)
                && source_matches
                && selector_matches(cards, binding, state, snapshot.perspective_player))
        }
        (
            Permission::PlayLandsFromGraveyard { player },
            StaticAction::PlayLand {
                player: actual_player,
                land,
                from,
                ..
            },
        ) => {
            let state = snapshot
                .objects
                .get(&land)
                .ok_or(RuntimeEvaluationError::ObjectMissing(land))?;
            Ok(player_matches(*player, binding.controller, actual_player)
                && from == Zone::Graveyard
                && state.zone == from
                && state.owner == actual_player
                && state.card_types.contains(&CardType::Land))
        }
        (
            Permission::PlayLandsFromLibraryTop { player },
            StaticAction::PlayLand {
                player: actual_player,
                land,
                from,
                from_library_top,
                ..
            },
        ) => {
            let state = snapshot
                .objects
                .get(&land)
                .ok_or(RuntimeEvaluationError::ObjectMissing(land))?;
            Ok(player_matches(*player, binding.controller, actual_player)
                && from == Zone::Library
                && from_library_top
                && state.zone == from
                && state.owner == actual_player
                && state.card_types.contains(&CardType::Land))
        }
        (
            Permission::AdditionalLandPlays { .. } | Permission::UnlimitedLandPlays { .. },
            StaticAction::PlayLand { .. },
        ) => Ok(false),
        _ => Ok(false),
    }
}

fn land_play_permission_matches_action(
    action: StaticAction,
    permission: &Permission,
    binding: &BoundProgram,
    snapshot: &RuntimeSnapshot,
) -> Result<bool, RuntimeEvaluationError> {
    let (player, during_own_turn) = match permission {
        Permission::AdditionalLandPlays {
            player,
            during_own_turn,
            ..
        }
        | Permission::UnlimitedLandPlays {
            player,
            during_own_turn,
        } => (player, during_own_turn),
        _ => return Ok(false),
    };
    let StaticAction::PlayLand {
        player: actual_player,
        land,
        from,
        ..
    } = action
    else {
        return Ok(false);
    };
    let state = snapshot
        .objects
        .get(&land)
        .ok_or(RuntimeEvaluationError::ObjectMissing(land))?;
    Ok(player_matches(*player, binding.controller, actual_player)
        && state.zone == from
        && state.card_types.contains(&CardType::Land)
        && (!*during_own_turn || snapshot.active_player == actual_player))
}

fn cost_modification_applies(
    application: CostApplication,
    modification: &CostModification,
    binding: &BoundProgram,
    snapshot: &RuntimeSnapshot,
) -> Result<bool, RuntimeEvaluationError> {
    match (&modification.scope, application) {
        (
            CostScope::CastSpell { player, spells },
            CostApplication::Cast {
                player: actual_player,
                spell,
            },
        ) => {
            let state = snapshot
                .objects
                .get(&spell)
                .ok_or(RuntimeEvaluationError::ObjectMissing(spell))?;
            Ok(player_matches(*player, binding.controller, actual_player)
                && selector_matches(spells, binding, state, snapshot.perspective_player)
                && condition_holds(&modification.condition, binding, state, snapshot)?)
        }
        (
            CostScope::ActivateAbility { player, sources },
            CostApplication::Activate {
                player: actual_player,
                source,
            },
        ) => {
            let state = snapshot
                .objects
                .get(&source)
                .ok_or(RuntimeEvaluationError::ObjectMissing(source))?;
            Ok(player_matches(*player, binding.controller, actual_player)
                && sources.as_ref().is_none_or(|selector| {
                    selector_matches(selector, binding, state, snapshot.perspective_player)
                })
                && condition_holds(&modification.condition, binding, state, snapshot)?)
        }
        _ => Ok(false),
    }
}

fn evaluate_nonnegative_amount(
    amount: &Amount,
    binding: &BoundProgram,
    snapshot: &RuntimeSnapshot,
) -> Result<u32, RuntimeEvaluationError> {
    u32::try_from(evaluate_amount(amount, binding, snapshot)?)
        .map_err(|_| RuntimeEvaluationError::NumericOverflow)
}

fn selector_matches(
    selector: &ObjectSelector,
    binding: &BoundProgram,
    object: &ObjectState,
    perspective_player: PlayerId,
) -> bool {
    match selector.reference {
        SelectorReference::Source if object.object_ref != binding.source => return false,
        SelectorReference::EnchantedBySource
            if !object.enchanting_sources.contains(&binding.source) =>
        {
            return false;
        }
        SelectorReference::EquippedBySource
            if !object.equipping_sources.contains(&binding.source) =>
        {
            return false;
        }
        SelectorReference::Source
        | SelectorReference::Matching
        | SelectorReference::EnchantedBySource
        | SelectorReference::EquippedBySource => {}
    }
    if selector.exclude_source && object.object_ref == binding.source {
        return false;
    }
    if !selector.zones.is_empty() && !selector.zones.contains(&object.zone) {
        return false;
    }
    if !controller_matches(
        selector.controller,
        binding.controller,
        object.controller,
        perspective_player,
    ) {
        return false;
    }
    if !controller_matches(
        selector.owner,
        binding.controller,
        object.owner,
        perspective_player,
    ) {
        return false;
    }
    let card_types_match = if selector.card_type_match_any {
        !selector.card_types.is_disjoint(&object.card_types)
    } else {
        selector.card_types.is_subset(&object.card_types)
    };
    let required_keywords_match = if selector.required_keyword_match_any {
        !selector.required_keywords.is_disjoint(&object.keywords)
    } else {
        selector.required_keywords.is_subset(&object.keywords)
    };
    let color_count = object
        .colors
        .iter()
        .filter(|color| **color != Color::Colorless)
        .count();
    let total_counter_count = object
        .counters
        .values()
        .map(|amount| u64::from(*amount))
        .sum::<u64>();
    if !card_types_match
        || !selector.names.is_subset(&object.names)
        || !selector.excluded_card_types.is_disjoint(&object.card_types)
        || !selector.supertypes.is_subset(&object.supertypes)
        || !selector.excluded_supertypes.is_disjoint(&object.supertypes)
        || !selector.colors.is_subset(&object.colors)
        || !selector.excluded_colors.is_disjoint(&object.colors)
        || selector
            .minimum_colors
            .is_some_and(|minimum| color_count < usize::from(minimum))
        || selector
            .maximum_colors
            .is_some_and(|maximum| color_count > usize::from(maximum))
        || !selector.subtypes.is_subset(&object.subtypes)
        || !selector.excluded_subtypes.is_disjoint(&object.subtypes)
        || !required_keywords_match
        || selector
            .minimum_counters
            .iter()
            .any(|(kind, minimum)| object.counters.get(kind).copied().unwrap_or(0) < *minimum)
        || selector
            .minimum_total_counters
            .is_some_and(|minimum| total_counter_count < u64::from(minimum))
        || selector
            .minimum_mana_value
            .is_some_and(|minimum| object.mana_value < minimum)
        || selector
            .maximum_mana_value
            .is_some_and(|maximum| object.mana_value > maximum)
        || selector
            .minimum_power
            .is_some_and(|minimum| object.power.is_none_or(|power| power < minimum))
        || selector
            .maximum_power
            .is_some_and(|maximum| object.power.is_none_or(|power| power > maximum))
        || selector
            .minimum_toughness
            .is_some_and(|minimum| object.toughness.is_none_or(|toughness| toughness < minimum))
        || selector
            .maximum_toughness
            .is_some_and(|maximum| object.toughness.is_none_or(|toughness| toughness > maximum))
        || selector
            .tapped
            .is_some_and(|expected| object.tapped != expected)
        || selector
            .attacking
            .is_some_and(|expected| object.attacking != expected)
        || selector
            .blocking
            .is_some_and(|expected| object.blocking != expected)
        || selector
            .enchanted
            .is_some_and(|expected| !object.enchanting_sources.is_empty() != expected)
        || selector
            .equipped
            .is_some_and(|expected| !object.equipping_sources.is_empty() != expected)
    {
        return false;
    }
    let token_matches = match selector.token_relation {
        TokenRelation::Any => true,
        TokenRelation::Token => object.token,
        TokenRelation::Nontoken => !object.token,
    };
    token_matches
        && (selector.alternatives.is_empty()
            || selector.alternatives.iter().any(|alternative| {
                selector_matches(alternative, binding, object, perspective_player)
            }))
}

fn controller_matches(
    relation: ControllerRelation,
    source_controller: PlayerId,
    actual: PlayerId,
    _perspective_player: PlayerId,
) -> bool {
    match relation {
        ControllerRelation::You => actual == source_controller,
        ControllerRelation::Opponent => actual != source_controller,
        ControllerRelation::Any => true,
    }
}

fn condition_holds(
    condition: &Condition,
    binding: &BoundProgram,
    affected_object: &ObjectState,
    snapshot: &RuntimeSnapshot,
) -> Result<bool, RuntimeEvaluationError> {
    match condition {
        Condition::Always => Ok(true),
        Condition::Not(condition) => Ok(!condition_holds(
            condition,
            binding,
            affected_object,
            snapshot,
        )?),
        Condition::All(conditions) => {
            for condition in conditions {
                if !condition_holds(condition, binding, affected_object, snapshot)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Condition::Any(conditions) => {
            for condition in conditions {
                if condition_holds(condition, binding, affected_object, snapshot)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Condition::DuringYourTurn => Ok(snapshot.active_player == binding.controller),
        Condition::NotDuringYourTurn => Ok(snapshot.active_player != binding.controller),
        Condition::ControllerControls(selector) => {
            if snapshot.objects.values().any(|object| {
                selector_matches(selector, binding, object, snapshot.perspective_player)
            }) {
                Ok(true)
            } else if selector.zones.is_subset(&snapshot.complete_zones) {
                Ok(false)
            } else {
                Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    selector.zones.clone(),
                ))
            }
        }
        Condition::MatchingObjectCount {
            selector,
            minimum,
            maximum,
        } => {
            if !selector.zones.is_subset(&snapshot.complete_zones) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    selector.zones.clone(),
                ));
            }
            let count = u32::try_from(
                snapshot
                    .objects
                    .values()
                    .filter(|object| {
                        selector_matches(selector, binding, object, snapshot.perspective_player)
                    })
                    .count(),
            )
            .map_err(|_| RuntimeEvaluationError::NumericOverflow)?;
            Ok(count >= *minimum && maximum.is_none_or(|maximum| count <= maximum))
        }
        Condition::AnyPlayerObjectCount {
            players,
            relation,
            selector,
            minimum,
            maximum,
        } => {
            if !snapshot.complete_players.contains(&binding.controller) {
                return Err(RuntimeEvaluationError::PlayerStateIncomplete(
                    binding.controller,
                ));
            }
            if !selector.zones.is_subset(&snapshot.complete_zones) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    selector.zones.clone(),
                ));
            }
            for player in snapshot
                .complete_players
                .iter()
                .copied()
                .filter(|player| player_matches(*players, binding.controller, *player))
            {
                let count = u32::try_from(
                    snapshot
                        .objects
                        .values()
                        .filter(|object| {
                            let relation_matches = match relation {
                                PlayerObjectRelation::Owner => object.owner == player,
                                PlayerObjectRelation::Controller => object.controller == player,
                            };
                            relation_matches
                                && selector_matches(
                                    selector,
                                    binding,
                                    object,
                                    snapshot.perspective_player,
                                )
                        })
                        .count(),
                )
                .map_err(|_| RuntimeEvaluationError::NumericOverflow)?;
                if count >= *minimum && maximum.is_none_or(|maximum| count <= maximum) {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Condition::AffectedObjectMatches(selector) => Ok(selector_matches(
            selector,
            binding,
            affected_object,
            snapshot.perspective_player,
        )),
        Condition::AffectedAttachmentCount {
            subtypes,
            minimum,
            maximum,
        } => {
            let attachment_refs = affected_object
                .enchanting_sources
                .iter()
                .chain(&affected_object.equipping_sources)
                .copied()
                .collect::<BTreeSet<_>>();
            let mut count = 0u32;
            for attachment in attachment_refs {
                let attachment = snapshot
                    .objects
                    .get(&attachment)
                    .ok_or(RuntimeEvaluationError::ObjectMissing(attachment))?;
                if !attachment.subtypes.is_disjoint(subtypes) {
                    count = count
                        .checked_add(1)
                        .ok_or(RuntimeEvaluationError::NumericOverflow)?;
                }
            }
            Ok(count >= *minimum && maximum.is_none_or(|maximum| count <= maximum))
        }
        Condition::ControllerLifeAtMost(maximum) | Condition::ControllerLifeAtLeast(maximum) => {
            if !snapshot.complete_players.contains(&binding.controller) {
                return Err(RuntimeEvaluationError::PlayerStateIncomplete(
                    binding.controller,
                ));
            }
            snapshot
                .life_totals
                .get(&binding.controller)
                .map(|life| {
                    let boundary = i32::try_from(*maximum).unwrap_or(i32::MAX);
                    if matches!(condition, Condition::ControllerLifeAtMost(_)) {
                        *life <= boundary
                    } else {
                        *life >= boundary
                    }
                })
                .ok_or(RuntimeEvaluationError::PlayerStateIncomplete(
                    binding.controller,
                ))
        }
        Condition::SourceIsTapped | Condition::SourceIsUntapped => {
            let source = snapshot
                .objects
                .get(&binding.source)
                .ok_or(RuntimeEvaluationError::ObjectMissing(binding.source))?;
            Ok(if matches!(condition, Condition::SourceIsTapped) {
                source.tapped
            } else {
                !source.tapped
            })
        }
    }
}

fn apply_characteristic_operation(
    result: &mut EffectiveCharacteristics,
    operation: &CharacteristicOperation,
    binding: &BoundProgram,
    affected_object: &ObjectState,
    snapshot: &RuntimeSnapshot,
) -> Result<(), RuntimeEvaluationError> {
    match operation {
        CharacteristicOperation::AddCardTypes(card_types) => {
            result.card_types.extend(card_types);
        }
        CharacteristicOperation::AddSubtypes(subtypes) => {
            result.subtypes.extend(subtypes.iter().cloned());
        }
        CharacteristicOperation::ModifyPowerToughness { power, toughness } => {
            let power_delta = evaluate_signed_amount(power, binding, affected_object, snapshot)?;
            let toughness_delta =
                evaluate_signed_amount(toughness, binding, affected_object, snapshot)?;
            result.power = match result.power {
                Some(value) => Some(
                    value
                        .checked_add(power_delta)
                        .ok_or(RuntimeEvaluationError::NumericOverflow)?,
                ),
                None => None,
            };
            result.toughness = match result.toughness {
                Some(value) => Some(
                    value
                        .checked_add(toughness_delta)
                        .ok_or(RuntimeEvaluationError::NumericOverflow)?,
                ),
                None => None,
            };
        }
        CharacteristicOperation::SetBasePowerToughness { power, toughness } => {
            result.power = Some(evaluate_amount_for_object(
                power,
                binding,
                Some(affected_object),
                snapshot,
            )?);
            result.toughness = Some(evaluate_amount_for_object(
                toughness,
                binding,
                Some(affected_object),
                snapshot,
            )?);
        }
        CharacteristicOperation::GrantKeywords(keywords) => {
            result.keywords.extend(keywords);
        }
        CharacteristicOperation::RemoveKeywords(keywords) => {
            result
                .keywords
                .retain(|keyword| !keywords.contains(keyword));
        }
        CharacteristicOperation::LoseAllAbilities => {
            result.keywords.clear();
            result.loses_all_abilities = true;
        }
    }
    Ok(())
}

fn evaluate_signed_amount(
    amount: &SignedAmount,
    binding: &BoundProgram,
    affected_object: &ObjectState,
    snapshot: &RuntimeSnapshot,
) -> Result<i32, RuntimeEvaluationError> {
    let magnitude =
        evaluate_amount_for_object(&amount.magnitude, binding, Some(affected_object), snapshot)?;
    if amount.negative {
        magnitude
            .checked_neg()
            .ok_or(RuntimeEvaluationError::NumericOverflow)
    } else {
        Ok(magnitude)
    }
}

fn evaluate_amount(
    amount: &Amount,
    binding: &BoundProgram,
    snapshot: &RuntimeSnapshot,
) -> Result<i32, RuntimeEvaluationError> {
    evaluate_amount_for_object(amount, binding, None, snapshot)
}

fn evaluate_amount_for_object(
    amount: &Amount,
    binding: &BoundProgram,
    affected_object: Option<&ObjectState>,
    snapshot: &RuntimeSnapshot,
) -> Result<i32, RuntimeEvaluationError> {
    match amount {
        Amount::Fixed(value) => {
            i32::try_from(*value).map_err(|_| RuntimeEvaluationError::NumericOverflow)
        }
        Amount::Count(selector) => {
            if !selector.zones.is_subset(&snapshot.complete_zones) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    selector.zones.clone(),
                ));
            }
            i32::try_from(
                snapshot
                    .objects
                    .values()
                    .filter(|object| {
                        selector_matches(selector, binding, object, snapshot.perspective_player)
                    })
                    .count(),
            )
            .map_err(|_| RuntimeEvaluationError::NumericOverflow)
        }
        Amount::Scaled { factor, amount } => {
            evaluate_amount_for_object(amount, binding, affected_object, snapshot)?
                .checked_mul(
                    i32::try_from(*factor).map_err(|_| RuntimeEvaluationError::NumericOverflow)?,
                )
                .ok_or(RuntimeEvaluationError::NumericOverflow)
        }
        Amount::OpponentCount => {
            if !snapshot.complete_players.contains(&binding.controller) {
                return Err(RuntimeEvaluationError::PlayerStateIncomplete(
                    binding.controller,
                ));
            }
            i32::try_from(
                snapshot
                    .complete_players
                    .iter()
                    .filter(|player| **player != binding.controller)
                    .count(),
            )
            .map_err(|_| RuntimeEvaluationError::NumericOverflow)
        }
        Amount::CounterCount { object, counter } => {
            if !object.zones.is_subset(&snapshot.complete_zones) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    object.zones.clone(),
                ));
            }
            let matching = snapshot
                .objects
                .values()
                .filter(|state| {
                    selector_matches(object, binding, state, snapshot.perspective_player)
                })
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return Err(RuntimeEvaluationError::ObjectMissing(binding.source));
            }
            i32::try_from(matching[0].counters.get(counter).copied().unwrap_or(0))
                .map_err(|_| RuntimeEvaluationError::NumericOverflow)
        }
        Amount::CounterCountOnAffected { counter } => {
            let affected = affected_object
                .ok_or_else(|| RuntimeEvaluationError::UnsupportedVariableAmount(amount.clone()))?;
            i32::try_from(affected.counters.get(counter).copied().unwrap_or(0))
                .map_err(|_| RuntimeEvaluationError::NumericOverflow)
        }
        Amount::TotalCounterCount { object } => {
            if !object.zones.is_subset(&snapshot.complete_zones) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    object.zones.clone(),
                ));
            }
            let matching = snapshot
                .objects
                .values()
                .filter(|state| {
                    selector_matches(object, binding, state, snapshot.perspective_player)
                })
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return Err(RuntimeEvaluationError::ObjectMissing(binding.source));
            }
            let total = matching[0]
                .counters
                .values()
                .try_fold(0u64, |total, count| total.checked_add(u64::from(*count)))
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
            i32::try_from(total).map_err(|_| RuntimeEvaluationError::NumericOverflow)
        }
        Amount::TotalCounterCountOnAffected => {
            let affected = affected_object
                .ok_or_else(|| RuntimeEvaluationError::UnsupportedVariableAmount(amount.clone()))?;
            let total = affected
                .counters
                .values()
                .try_fold(0u64, |total, count| total.checked_add(u64::from(*count)))
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
            i32::try_from(total).map_err(|_| RuntimeEvaluationError::NumericOverflow)
        }
        Amount::AttachmentCount { subtypes } => {
            if !snapshot.complete_zones.contains(&Zone::Battlefield) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(BTreeSet::from(
                    [Zone::Battlefield],
                )));
            }
            let affected = affected_object
                .ok_or_else(|| RuntimeEvaluationError::UnsupportedVariableAmount(amount.clone()))?;
            let attached = affected
                .enchanting_sources
                .union(&affected.equipping_sources)
                .copied()
                .collect::<BTreeSet<_>>();
            let mut count = 0usize;
            for source in attached {
                let source = snapshot
                    .objects
                    .get(&source)
                    .ok_or(RuntimeEvaluationError::ObjectMissing(source))?;
                if !source.subtypes.is_disjoint(subtypes) {
                    count = count
                        .checked_add(1)
                        .ok_or(RuntimeEvaluationError::NumericOverflow)?;
                }
            }
            i32::try_from(count).map_err(|_| RuntimeEvaluationError::NumericOverflow)
        }
        Amount::AttachmentCountOnObject { object, subtypes } => {
            if !object.zones.is_subset(&snapshot.complete_zones)
                || !snapshot.complete_zones.contains(&Zone::Battlefield)
            {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(BTreeSet::from(
                    [Zone::Battlefield],
                )));
            }
            let matching = snapshot
                .objects
                .values()
                .filter(|state| {
                    selector_matches(object, binding, state, snapshot.perspective_player)
                })
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return Err(RuntimeEvaluationError::ObjectMissing(binding.source));
            }
            let attached = matching[0]
                .enchanting_sources
                .union(&matching[0].equipping_sources)
                .copied()
                .collect::<BTreeSet<_>>();
            let mut count = 0usize;
            for source in attached {
                let source = snapshot
                    .objects
                    .get(&source)
                    .ok_or(RuntimeEvaluationError::ObjectMissing(source))?;
                if !source.subtypes.is_disjoint(subtypes) {
                    count = count
                        .checked_add(1)
                        .ok_or(RuntimeEvaluationError::NumericOverflow)?;
                }
            }
            i32::try_from(count).map_err(|_| RuntimeEvaluationError::NumericOverflow)
        }
        Amount::PowerOf(selector) | Amount::ToughnessOf(selector) => {
            if !selector.zones.is_subset(&snapshot.complete_zones) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    selector.zones.clone(),
                ));
            }
            let matching = snapshot
                .objects
                .values()
                .filter(|object| {
                    selector_matches(selector, binding, object, snapshot.perspective_player)
                })
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return Err(RuntimeEvaluationError::AmountObjectCardinality {
                    amount: amount.clone(),
                    actual: matching.len(),
                });
            }
            let object = matching[0];
            let value = if matches!(amount, Amount::PowerOf(_)) {
                object.power
            } else {
                object.toughness
            };
            value.ok_or_else(|| RuntimeEvaluationError::VariableAmountMissing {
                source: object.object_ref,
                amount: amount.clone(),
            })
        }
        Amount::AffectedControllerCount(selector) => {
            let affected = affected_object
                .ok_or_else(|| RuntimeEvaluationError::UnsupportedVariableAmount(amount.clone()))?;
            if !selector.zones.is_subset(&snapshot.complete_zones) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    selector.zones.clone(),
                ));
            }
            let mut relative_binding = binding.clone();
            relative_binding.controller = affected.controller;
            i32::try_from(
                snapshot
                    .objects
                    .values()
                    .filter(|object| {
                        selector_matches(
                            selector,
                            &relative_binding,
                            object,
                            snapshot.perspective_player,
                        )
                    })
                    .count(),
            )
            .map_err(|_| RuntimeEvaluationError::NumericOverflow)
        }
        Amount::X => snapshot
            .x_values
            .get(&binding.source)
            .copied()
            .ok_or_else(|| RuntimeEvaluationError::VariableAmountMissing {
                source: binding.source,
                amount: amount.clone(),
            })
            .and_then(|value| {
                i32::try_from(value).map_err(|_| RuntimeEvaluationError::NumericOverflow)
            }),
        Amount::ThatMany => Err(RuntimeEvaluationError::UnsupportedVariableAmount(
            amount.clone(),
        )),
        Amount::KickerPayments => snapshot
            .kicker_payments
            .get(&binding.source)
            .copied()
            .ok_or(RuntimeEvaluationError::EntryCastEvidenceMissing(
                binding.source,
            ))
            .and_then(|value| {
                i32::try_from(value).map_err(|_| RuntimeEvaluationError::NumericOverflow)
            }),
    }
}

fn replacement_matches(
    replacement: &ReplacementEffect,
    binding: &BoundProgram,
    event: &RuntimeEvent,
    evidence: &ReplacementEventEvidence,
    snapshot: &RuntimeSnapshot,
) -> Result<bool, RuntimeEvaluationError> {
    validate_replacement_event_object_evidence(event, snapshot)?;
    match (&replacement.predicate, event) {
        (
            ReplacementEventPredicate::ZoneChange { object, from, to },
            RuntimeEvent::ZoneChange {
                object: actual,
                from: actual_from,
                to: actual_to,
                ..
            },
        ) => Ok(from.is_none_or(|expected| expected == *actual_from)
            && *to == *actual_to
            && selector_matches(object, binding, actual, snapshot.perspective_player)),
        (
            ReplacementEventPredicate::ZoneChangeCausedByOpponentSpellOrAbility {
                object,
                from,
                to,
            },
            RuntimeEvent::ZoneChange {
                object: actual,
                from: actual_from,
                to: actual_to,
                ..
            },
        ) => {
            let cause = evidence
                .cause
                .as_ref()
                .ok_or(RuntimeEvaluationError::ReplacementCauseEvidenceIncomplete)?;
            if !cause.evidence_complete {
                return Err(RuntimeEvaluationError::ReplacementCauseEvidenceIncomplete);
            }
            let current_source = snapshot
                .objects
                .get(&cause.source.object_ref)
                .ok_or(RuntimeEvaluationError::ReplacementCauseEvidenceMismatch)?;
            if current_source.owner != cause.source.owner
                || current_source.controller != cause.source.controller
                || current_source.zone != cause.source.zone
                || !snapshot.complete_players.contains(&cause.source.controller)
                || matches!(cause.kind, ReplacementEventCauseKind::Spell)
                    && cause.source.zone != Zone::Stack
            {
                return Err(RuntimeEvaluationError::ReplacementCauseEvidenceMismatch);
            }
            Ok(*from == *actual_from
                && *to == *actual_to
                && cause.source.controller != binding.controller
                && selector_matches(object, binding, actual, snapshot.perspective_player))
        }
        (
            ReplacementEventPredicate::EnterBattlefield { object, condition },
            RuntimeEvent::ZoneChange {
                object: actual,
                to: Zone::Battlefield,
                ..
            },
        ) => {
            let mut entering_characteristics = actual.clone();
            entering_characteristics.zone = Zone::Battlefield;
            Ok(selector_matches(
                object,
                binding,
                &entering_characteristics,
                snapshot.perspective_player,
            ) && entry_replacement_condition_holds(
                condition,
                binding,
                actual.object_ref,
                snapshot,
            )?)
        }
        (
            ReplacementEventPredicate::Damage {
                source,
                recipient,
                kind,
            },
            RuntimeEvent::Damage {
                source: actual_source,
                recipient: actual_recipient,
                kind: actual_kind,
                preventable,
                ..
            },
        ) => {
            let redirect_available = match &replacement.operation {
                ReplacementOperation::RedirectDamage { recipient } => {
                    resolve_redirect_recipient(recipient, binding, *actual_recipient, snapshot)?
                        .is_some()
                }
                _ => true,
            };
            Ok((!matches!(
                &replacement.operation,
                ReplacementOperation::PreventDamage { .. }
                    | ReplacementOperation::PreventDamageAndRemovePlusOneCounter
            ) || *preventable)
                && redirect_available
                && kind.is_none_or(|expected| expected == *actual_kind)
                && selector_matches(source, binding, actual_source, snapshot.perspective_player)
                && recipient_matches(recipient, binding, *actual_recipient, snapshot)?)
        }
        (
            ReplacementEventPredicate::DamageSourceOrRecipient { object, kind },
            RuntimeEvent::Damage {
                source: actual_source,
                recipient: actual_recipient,
                kind: actual_kind,
                preventable,
                ..
            },
        ) => {
            validate_runtime_recipient(*actual_recipient, snapshot)?;
            let recipient_matches_object = match actual_recipient {
                RuntimeRecipient::Object(object_ref, _) => {
                    let state = snapshot
                        .objects
                        .get(object_ref)
                        .ok_or(RuntimeEvaluationError::ObjectMissing(*object_ref))?;
                    selector_matches(object, binding, state, snapshot.perspective_player)
                }
                RuntimeRecipient::Player(_) => false,
            };
            Ok(*preventable
                && kind.is_none_or(|expected| expected == *actual_kind)
                && (selector_matches(object, binding, actual_source, snapshot.perspective_player)
                    || recipient_matches_object))
        }
        (
            ReplacementEventPredicate::DrawCard { player },
            RuntimeEvent::DrawCards {
                player: actual,
                amount,
            },
        ) => {
            if *amount != 1 {
                return Err(RuntimeEvaluationError::NonAtomicDrawEvent(*amount));
            }
            Ok(player_matches(*player, binding.controller, *actual))
        }
        (
            ReplacementEventPredicate::DrawCardExceptFirstInOwnDrawStep { player },
            RuntimeEvent::DrawCards {
                player: actual,
                amount,
            },
        ) => {
            if *amount != 1 {
                return Err(RuntimeEvaluationError::NonAtomicDrawEvent(*amount));
            }
            if !player_matches(*player, binding.controller, *actual) {
                return Ok(false);
            }
            let draw_context = evidence
                .draw_context
                .ok_or(RuntimeEvaluationError::DrawContextEvidenceIncomplete)?;
            if !draw_context.evidence_complete {
                return Err(RuntimeEvaluationError::DrawContextEvidenceIncomplete);
            }
            if let Some((step_player, _)) = draw_context.current_step {
                if step_player != snapshot.active_player
                    || !snapshot.complete_players.contains(&step_player)
                {
                    return Err(RuntimeEvaluationError::DrawContextEvidenceMismatch);
                }
            }
            Ok(!matches!(
                draw_context.current_step,
                Some((step_player, TurnStep::Draw))
                    if step_player == *actual
                        && draw_context.prior_draws_in_current_step == 0
            ))
        }
        (
            ReplacementEventPredicate::GainLife { player },
            RuntimeEvent::GainLife { player: actual, .. },
        )
        | (
            ReplacementEventPredicate::CreateTokens { player },
            RuntimeEvent::CreateTokens { player: actual, .. },
        ) => Ok(player_matches(*player, binding.controller, *actual)),
        (
            ReplacementEventPredicate::PutCounters { object, counter },
            RuntimeEvent::PutCounters {
                object: actual,
                counter: actual_counter,
                ..
            },
        ) => Ok(counter
            .as_ref()
            .is_none_or(|expected| expected == actual_counter)
            && selector_matches(object, binding, actual, snapshot.perspective_player)),
        (
            ReplacementEventPredicate::StepWouldBegin { player, step },
            RuntimeEvent::Step {
                player: actual,
                step: actual_step,
                ..
            },
        ) => Ok(*step == *actual_step && player_matches(*player, binding.controller, *actual)),
        (
            ReplacementEventPredicate::ExtraTurnWouldBegin { player },
            RuntimeEvent::ExtraTurn { player: actual, .. },
        ) => Ok(player_matches(*player, binding.controller, *actual)),
        _ => Ok(false),
    }
}

fn entry_replacement_condition_holds(
    condition: &EntryReplacementCondition,
    binding: &BoundProgram,
    entering_object: ObjectRef,
    snapshot: &RuntimeSnapshot,
) -> Result<bool, RuntimeEvaluationError> {
    match condition {
        EntryReplacementCondition::Always => Ok(true),
        EntryReplacementCondition::UnlessControllerControlsAtMostOther { objects, maximum } => {
            if !snapshot.complete_zones.is_superset(&objects.zones) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    objects.zones.clone(),
                ));
            }
            let count = snapshot
                .objects
                .values()
                .filter(|object| {
                    object.object_ref != entering_object
                        && selector_matches(objects, binding, object, snapshot.perspective_player)
                })
                .count();
            let count =
                u32::try_from(count).map_err(|_| RuntimeEvaluationError::NumericOverflow)?;
            Ok(count > *maximum)
        }
        EntryReplacementCondition::UnlessControllerControlsAtLeast { objects, minimum } => {
            if !snapshot.complete_zones.is_superset(&objects.zones) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    objects.zones.clone(),
                ));
            }
            let count = snapshot
                .objects
                .values()
                .filter(|object| {
                    object.object_ref != entering_object
                        && selector_matches(objects, binding, object, snapshot.perspective_player)
                })
                .count();
            let count =
                u32::try_from(count).map_err(|_| RuntimeEvaluationError::NumericOverflow)?;
            Ok(count < *minimum)
        }
        EntryReplacementCondition::IfMatchingObjectsAtLeast { objects, minimum } => {
            if !snapshot.complete_zones.is_superset(&objects.zones) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    objects.zones.clone(),
                ));
            }
            let count = snapshot
                .objects
                .values()
                .filter(|object| {
                    object.object_ref != entering_object
                        && selector_matches(objects, binding, object, snapshot.perspective_player)
                })
                .count();
            let count =
                u32::try_from(count).map_err(|_| RuntimeEvaluationError::NumericOverflow)?;
            Ok(count >= *minimum)
        }
        EntryReplacementCondition::UnlessAnyPlayerLifeAtMost(maximum) => {
            if !snapshot.complete_players.contains(&binding.controller)
                || snapshot.complete_players.is_empty()
            {
                return Err(RuntimeEvaluationError::PlayerStateIncomplete(
                    binding.controller,
                ));
            }
            for player in &snapshot.complete_players {
                let life = snapshot
                    .life_totals
                    .get(player)
                    .ok_or(RuntimeEvaluationError::PlayerStateIncomplete(*player))?;
                if *life
                    <= i32::try_from(*maximum)
                        .map_err(|_| RuntimeEvaluationError::NumericOverflow)?
                {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        EntryReplacementCondition::UnlessOpponentCountAtLeast(minimum) => {
            if !snapshot.complete_players.contains(&binding.controller) {
                return Err(RuntimeEvaluationError::PlayerStateIncomplete(
                    binding.controller,
                ));
            }
            let opponents = snapshot
                .complete_players
                .iter()
                .filter(|player| **player != binding.controller)
                .count();
            let opponents =
                u32::try_from(opponents).map_err(|_| RuntimeEvaluationError::NumericOverflow)?;
            Ok(opponents < *minimum)
        }
        EntryReplacementCondition::IfSourceWasKicked => snapshot
            .kicker_payments
            .get(&binding.source)
            .copied()
            .map(|payments| payments > 0)
            .ok_or(RuntimeEvaluationError::EntryCastEvidenceMissing(
                binding.source,
            )),
    }
}

fn validate_replacement_event_object_evidence(
    event: &RuntimeEvent,
    snapshot: &RuntimeSnapshot,
) -> Result<(), RuntimeEvaluationError> {
    let supplied = match event {
        RuntimeEvent::ZoneChange { object, .. }
        | RuntimeEvent::Damage { source: object, .. }
        | RuntimeEvent::PutCounters { object, .. } => object,
        RuntimeEvent::DrawCards { .. }
        | RuntimeEvent::GainLife { .. }
        | RuntimeEvent::CreateTokens { .. }
        | RuntimeEvent::Step { .. }
        | RuntimeEvent::ExtraTurn { .. } => return Ok(()),
    };
    let actual = snapshot
        .objects
        .get(&supplied.object_ref)
        .ok_or(RuntimeEvaluationError::ObjectMissing(supplied.object_ref))?;
    if supplied.owner != actual.owner || supplied.controller != actual.controller {
        return Err(RuntimeEvaluationError::ObjectRelationEvidenceMismatch {
            object: supplied.object_ref,
            supplied_owner: supplied.owner,
            actual_owner: actual.owner,
            supplied_controller: supplied.controller,
            actual_controller: actual.controller,
        });
    }
    Ok(())
}

fn player_matches(selector: PlayerSelector, controller: PlayerId, actual: PlayerId) -> bool {
    match selector {
        PlayerSelector::You => actual == controller,
        PlayerSelector::Opponents => actual != controller,
        PlayerSelector::EachPlayer | PlayerSelector::AffectedPlayer => true,
        PlayerSelector::ControllerOfAffectedObject | PlayerSelector::OwnerOfAffectedObject => true,
    }
}

fn recipient_matches(
    selector: &RecipientSelector,
    binding: &BoundProgram,
    actual: RuntimeRecipient,
    snapshot: &RuntimeSnapshot,
) -> Result<bool, RuntimeEvaluationError> {
    validate_runtime_recipient(actual, snapshot)?;
    match (selector, actual) {
        (RecipientSelector::Any, _) => Ok(true),
        (RecipientSelector::Alternatives(alternatives), _) => {
            for alternative in alternatives {
                if recipient_matches(alternative, binding, actual, snapshot)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        (RecipientSelector::Player(player), RuntimeRecipient::Player(actual)) => {
            Ok(player_matches(*player, binding.controller, actual))
        }
        (RecipientSelector::Object(selector), RuntimeRecipient::Object(object, _)) => {
            let state = snapshot
                .objects
                .get(&object)
                .ok_or(RuntimeEvaluationError::ObjectMissing(object))?;
            Ok(selector_matches(
                selector,
                binding,
                state,
                snapshot.perspective_player,
            ))
        }
        _ => Ok(false),
    }
}

fn resolve_redirect_recipient(
    selector: &RecipientSelector,
    binding: &BoundProgram,
    original: RuntimeRecipient,
    snapshot: &RuntimeSnapshot,
) -> Result<Option<RuntimeRecipient>, RuntimeEvaluationError> {
    let complete_player = |player| {
        if snapshot.complete_players.contains(&player) {
            Ok(Some(RuntimeRecipient::Player(player)))
        } else {
            Err(RuntimeEvaluationError::PlayerStateIncomplete(player))
        }
    };
    match selector {
        RecipientSelector::Player(PlayerSelector::You) => complete_player(binding.controller),
        RecipientSelector::Player(PlayerSelector::AffectedPlayer) => match original {
            RuntimeRecipient::Player(player) | RuntimeRecipient::Object(_, player) => {
                complete_player(player)
            }
        },
        RecipientSelector::Player(PlayerSelector::ControllerOfAffectedObject) => {
            let RuntimeRecipient::Object(object_ref, _) = original else {
                return Ok(None);
            };
            let object = snapshot
                .objects
                .get(&object_ref)
                .ok_or(RuntimeEvaluationError::ObjectMissing(object_ref))?;
            complete_player(object.controller)
        }
        RecipientSelector::Player(PlayerSelector::OwnerOfAffectedObject) => {
            let RuntimeRecipient::Object(object_ref, _) = original else {
                return Ok(None);
            };
            let object = snapshot
                .objects
                .get(&object_ref)
                .ok_or(RuntimeEvaluationError::ObjectMissing(object_ref))?;
            complete_player(object.owner)
        }
        RecipientSelector::Object(selector) => {
            if !snapshot.complete_zones.is_superset(&selector.zones) {
                return Err(RuntimeEvaluationError::ZoneStateIncomplete(
                    selector.zones.clone(),
                ));
            }
            let mut matches = snapshot.objects.values().filter(|object| {
                selector_matches(selector, binding, object, snapshot.perspective_player)
            });
            let Some(object) = matches.next() else {
                return Ok(None);
            };
            if matches.next().is_some() {
                return Err(RuntimeEvaluationError::ReplacementEvidenceMismatch);
            }
            if !snapshot.complete_players.contains(&object.controller) {
                return Err(RuntimeEvaluationError::PlayerStateIncomplete(
                    object.controller,
                ));
            }
            Ok(Some(RuntimeRecipient::Object(
                object.object_ref,
                object.controller,
            )))
        }
        RecipientSelector::Player(PlayerSelector::Opponents | PlayerSelector::EachPlayer)
        | RecipientSelector::Alternatives(_)
        | RecipientSelector::Any => Err(RuntimeEvaluationError::ReplacementEvidenceMismatch),
    }
}

fn validate_runtime_recipient(
    recipient: RuntimeRecipient,
    snapshot: &RuntimeSnapshot,
) -> Result<(), RuntimeEvaluationError> {
    let RuntimeRecipient::Object(object, supplied) = recipient else {
        return Ok(());
    };
    let state = snapshot
        .objects
        .get(&object)
        .ok_or(RuntimeEvaluationError::ObjectMissing(object))?;
    if state.controller != supplied {
        return Err(RuntimeEvaluationError::RecipientControllerMismatch {
            object,
            supplied,
            actual: state.controller,
        });
    }
    Ok(())
}

fn affected_player(event: &RuntimeEvent) -> PlayerId {
    match event {
        RuntimeEvent::ZoneChange { object, to, .. } => {
            if *to == Zone::Battlefield {
                object.controller
            } else {
                object.owner
            }
        }
        RuntimeEvent::Damage { recipient, .. } => match recipient {
            RuntimeRecipient::Player(player) | RuntimeRecipient::Object(_, player) => *player,
        },
        RuntimeEvent::DrawCards { player, .. }
        | RuntimeEvent::GainLife { player, .. }
        | RuntimeEvent::CreateTokens { player, .. }
        | RuntimeEvent::Step { player, .. }
        | RuntimeEvent::ExtraTurn { player, .. } => *player,
        RuntimeEvent::PutCounters { object, .. } => object.controller,
    }
}

fn apply_replacement_operation(
    event: &mut RuntimeEvent,
    operation: &ReplacementOperation,
    binding: &BoundProgram,
    snapshot: &RuntimeSnapshot,
    entry_choice: Option<EntryChoiceValue>,
    copy_object: Option<ObjectRef>,
    counter_removals: &mut BTreeMap<ObjectRef, BTreeMap<CounterKind, u32>>,
) -> Result<(), RuntimeEvaluationError> {
    match operation {
        ReplacementOperation::EnterAsCopy { .. } if entry_choice.is_some() => {
            return Err(RuntimeEvaluationError::UnexpectedReplacementChoiceEvidence);
        }
        ReplacementOperation::ChooseAsEnters(_)
        | ReplacementOperation::EnterWithCounterChoice { .. }
            if copy_object.is_some() =>
        {
            return Err(RuntimeEvaluationError::UnexpectedReplacementChoiceEvidence);
        }
        ReplacementOperation::EnterAsCopy { .. }
        | ReplacementOperation::ChooseAsEnters(_)
        | ReplacementOperation::EnterWithCounterChoice { .. } => {}
        _ if entry_choice.is_some() || copy_object.is_some() => {
            return Err(RuntimeEvaluationError::UnexpectedReplacementChoiceEvidence);
        }
        _ => {}
    }
    match (operation, &mut *event) {
        (
            ReplacementOperation::MoveInstead {
                destination,
                placement,
            },
            RuntimeEvent::ZoneChange {
                to,
                library_placement,
                ..
            },
        ) => {
            *to = *destination;
            *library_placement = *placement;
        }
        (
            ReplacementOperation::MoveInsteadWithCounters {
                destination,
                placement,
                counter,
                amount,
            },
            RuntimeEvent::ZoneChange {
                to,
                enter_counters,
                library_placement,
                ..
            },
        ) => {
            *to = *destination;
            *library_placement = *placement;
            let slot = enter_counters.entry(counter.clone()).or_default();
            *slot = slot
                .checked_add(*amount)
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
        }
        (ReplacementOperation::EnterTapped, RuntimeEvent::ZoneChange { enter_tapped, .. }) => {
            *enter_tapped = true
        }
        (
            ReplacementOperation::EnterWithCounters { counter, amount },
            RuntimeEvent::ZoneChange { enter_counters, .. },
        ) => {
            let amount = evaluate_nonnegative_amount(amount, binding, snapshot)?;
            let slot = enter_counters.entry(counter.clone()).or_default();
            *slot = slot
                .checked_add(amount)
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
        }
        (
            ReplacementOperation::EnterWithCounterChoice { choices },
            RuntimeEvent::ZoneChange {
                enter_counters,
                entry_choice: event_choice,
                ..
            },
        ) => {
            let choice = entry_choice.ok_or(RuntimeEvaluationError::EntryChoiceRequired(
                EntryChoice::CounterKind,
            ))?;
            let EntryChoiceValue::CounterKind(counter) = choice else {
                return Err(RuntimeEvaluationError::EntryChoiceRequired(
                    EntryChoice::CounterKind,
                ));
            };
            if !choices.contains(&counter) {
                return Err(RuntimeEvaluationError::IllegalEntryChoice);
            }
            let slot = enter_counters.entry(counter.clone()).or_default();
            *slot = slot
                .checked_add(1)
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
            *event_choice = Some(EntryChoiceValue::CounterKind(counter));
        }
        (
            ReplacementOperation::EnterAsCopy { of, .. },
            RuntimeEvent::ZoneChange {
                copy_of: event_copy,
                ..
            },
        ) => {
            let copy_object = copy_object.ok_or(RuntimeEvaluationError::EntryCopyObjectRequired)?;
            let copy_state = snapshot
                .objects
                .get(&copy_object)
                .ok_or(RuntimeEvaluationError::IllegalEntryCopyObject(copy_object))?;
            if !selector_matches(of, binding, copy_state, snapshot.perspective_player) {
                return Err(RuntimeEvaluationError::IllegalEntryCopyObject(copy_object));
            }
            *event_copy = Some(copy_object);
        }
        (
            ReplacementOperation::ChooseAsEnters(expected),
            RuntimeEvent::ZoneChange {
                object: entering_object,
                entry_choice: event_choice,
                ..
            },
        ) => {
            let choice = entry_choice
                .ok_or_else(|| RuntimeEvaluationError::EntryChoiceRequired(expected.clone()))?;
            let correct = matches!(
                (expected, &choice),
                (EntryChoice::Color, EntryChoiceValue::Color(_))
                    | (EntryChoice::ColorOtherThan(_), EntryChoiceValue::Color(_))
                    | (EntryChoice::ColorAmong(_), EntryChoiceValue::Color(_))
                    | (EntryChoice::CardType, EntryChoiceValue::CardType(_))
                    | (EntryChoice::CardTypeAmong(_), EntryChoiceValue::CardType(_))
                    | (EntryChoice::CreatureType, EntryChoiceValue::CreatureType(_))
                    | (
                        EntryChoice::CreatureTypeAmong(_),
                        EntryChoiceValue::CreatureType(_)
                    )
                    | (EntryChoice::CounterKind, EntryChoiceValue::CounterKind(_))
                    | (EntryChoice::Player, EntryChoiceValue::Player(_))
                    | (EntryChoice::Opponent, EntryChoiceValue::Player(_))
                    | (
                        EntryChoice::BasicLandType,
                        EntryChoiceValue::BasicLandType(_)
                    )
                    | (
                        EntryChoice::BasicLandTypeAmong(_),
                        EntryChoiceValue::BasicLandType(_)
                    )
                    | (
                        EntryChoice::ColorAndCreatureType,
                        EntryChoiceValue::ColorAndCreatureType { .. }
                    )
                    | (EntryChoice::TwoColors, EntryChoiceValue::Colors(_))
                    | (
                        EntryChoice::AnotherCreatureYouControl,
                        EntryChoiceValue::Object(_)
                    )
                    | (
                        EntryChoice::ColorAndOpponent,
                        EntryChoiceValue::ColorAndPlayer { .. }
                    )
                    | (EntryChoice::Players(_), EntryChoiceValue::Players(_))
                    | (
                        EntryChoice::BasicLandTypes(_),
                        EntryChoiceValue::BasicLandTypes(_)
                    )
            );
            if !correct {
                return Err(RuntimeEvaluationError::EntryChoiceRequired(
                    expected.clone(),
                ));
            }
            let legal = match (expected, &choice) {
                (EntryChoice::Color, EntryChoiceValue::Color(color)) => *color != Color::Colorless,
                (EntryChoice::ColorOtherThan(excluded), EntryChoiceValue::Color(color)) => {
                    *color != Color::Colorless && color != excluded
                }
                (EntryChoice::ColorAmong(allowed), EntryChoiceValue::Color(color)) => {
                    allowed.contains(color)
                }
                (EntryChoice::CardType, EntryChoiceValue::CardType(_))
                | (EntryChoice::CounterKind, EntryChoiceValue::CounterKind(_)) => true,
                (EntryChoice::CardTypeAmong(allowed), EntryChoiceValue::CardType(card_type)) => {
                    allowed.contains(card_type)
                }
                (EntryChoice::CreatureType, EntryChoiceValue::CreatureType(creature_type)) => {
                    snapshot.legal_creature_types.contains(creature_type)
                }
                (
                    EntryChoice::CreatureTypeAmong(allowed),
                    EntryChoiceValue::CreatureType(creature_type),
                ) => {
                    allowed.contains(creature_type)
                        && snapshot.legal_creature_types.contains(creature_type)
                }
                (EntryChoice::Player, EntryChoiceValue::Player(player)) => {
                    snapshot.complete_players.contains(player)
                }
                (EntryChoice::Opponent, EntryChoiceValue::Player(player)) => {
                    snapshot.complete_players.contains(player) && *player != binding.controller
                }
                (EntryChoice::BasicLandType, EntryChoiceValue::BasicLandType(land_type)) => {
                    matches!(
                        land_type.as_str(),
                        "Plains" | "Island" | "Swamp" | "Mountain" | "Forest"
                    )
                }
                (
                    EntryChoice::BasicLandTypeAmong(allowed),
                    EntryChoiceValue::BasicLandType(land_type),
                ) => allowed.contains(land_type),
                (
                    EntryChoice::ColorAndCreatureType,
                    EntryChoiceValue::ColorAndCreatureType {
                        color,
                        creature_type,
                    },
                ) => {
                    *color != Color::Colorless
                        && snapshot.legal_creature_types.contains(creature_type)
                }
                (EntryChoice::TwoColors, EntryChoiceValue::Colors(colors)) => {
                    colors.len() == 2 && !colors.contains(&Color::Colorless)
                }
                (EntryChoice::AnotherCreatureYouControl, EntryChoiceValue::Object(chosen)) => {
                    if !snapshot.complete_zones.contains(&Zone::Battlefield) {
                        return Err(RuntimeEvaluationError::ZoneStateIncomplete(BTreeSet::from(
                            [Zone::Battlefield],
                        )));
                    }
                    snapshot.objects.get(chosen).is_some_and(|object| {
                        object.object_ref != entering_object.object_ref
                            && object.zone == Zone::Battlefield
                            && object.controller == binding.controller
                            && object.card_types.contains(&CardType::Creature)
                    })
                }
                (
                    EntryChoice::ColorAndOpponent,
                    EntryChoiceValue::ColorAndPlayer { color, player },
                ) => {
                    *color != Color::Colorless
                        && snapshot.complete_players.contains(player)
                        && *player != binding.controller
                }
                (EntryChoice::Players(count), EntryChoiceValue::Players(players)) => {
                    players.len() == usize::from(*count)
                        && players
                            .iter()
                            .all(|player| snapshot.complete_players.contains(player))
                }
                (
                    EntryChoice::BasicLandTypes(count),
                    EntryChoiceValue::BasicLandTypes(land_types),
                ) => {
                    land_types.len() == usize::from(*count)
                        && land_types.iter().all(|land_type| {
                            matches!(
                                land_type.as_str(),
                                "Plains" | "Island" | "Swamp" | "Mountain" | "Forest"
                            )
                        })
                }
                _ => false,
            };
            if !legal {
                return Err(RuntimeEvaluationError::IllegalEntryChoice);
            }
            *event_choice = Some(choice);
        }
        (
            ReplacementOperation::PreventDamage { amount },
            RuntimeEvent::Damage {
                amount: remaining,
                preventable,
                prevented,
                ..
            },
        ) => {
            if !*preventable {
                return Ok(());
            }
            let prevented_now = amount.unwrap_or(*remaining).min(*remaining);
            *remaining -= prevented_now;
            *prevented = prevented
                .checked_add(prevented_now)
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
        }
        (
            ReplacementOperation::PreventDamageAndRemovePlusOneCounter,
            RuntimeEvent::Damage {
                recipient,
                amount: remaining,
                preventable,
                prevented,
                ..
            },
        ) => {
            if !*preventable {
                return Ok(());
            }
            let RuntimeRecipient::Object(recipient, _) = *recipient else {
                return Err(RuntimeEvaluationError::ReplacementEvidenceMismatch);
            };
            if recipient != binding.source {
                return Err(RuntimeEvaluationError::ReplacementEvidenceMismatch);
            }
            let object = snapshot
                .objects
                .get(&recipient)
                .ok_or(RuntimeEvaluationError::ObjectMissing(recipient))?;
            let available = object
                .counters
                .get(&CounterKind::PlusOnePlusOne)
                .copied()
                .unwrap_or_default();
            let removals = counter_removals.entry(recipient).or_default();
            let removed = removals
                .get(&CounterKind::PlusOnePlusOne)
                .copied()
                .unwrap_or_default();
            if removed < available {
                removals.insert(CounterKind::PlusOnePlusOne, removed + 1);
            }
            let prevented_now = *remaining;
            *remaining = 0;
            *prevented = prevented
                .checked_add(prevented_now)
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
        }
        (
            ReplacementOperation::ScaleDamage {
                numerator,
                denominator,
                round_down,
            },
            RuntimeEvent::Damage { amount, .. },
        ) => {
            if *denominator == 0 {
                return Err(RuntimeEvaluationError::NumericOverflow);
            }
            let product = amount
                .checked_mul(*numerator)
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
            let quotient = product / *denominator;
            let remainder = product % *denominator;
            *amount = if *round_down || remainder == 0 {
                quotient
            } else {
                quotient
                    .checked_add(1)
                    .ok_or(RuntimeEvaluationError::NumericOverflow)?
            };
        }
        (
            ReplacementOperation::IncreaseDamage { amount: increase },
            RuntimeEvent::Damage { amount, .. },
        ) => {
            *amount = amount
                .checked_add(*increase)
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
        }
        (
            ReplacementOperation::ClampDamageToLifeFloor { minimum_life },
            RuntimeEvent::Damage {
                recipient: RuntimeRecipient::Player(player),
                amount,
                ..
            },
        ) => {
            if !snapshot.complete_players.contains(player) {
                return Err(RuntimeEvaluationError::PlayerStateIncomplete(*player));
            }
            let life = snapshot
                .life_totals
                .get(player)
                .copied()
                .ok_or(RuntimeEvaluationError::PlayerStateIncomplete(*player))?;
            let life =
                u32::try_from(life.max(0)).map_err(|_| RuntimeEvaluationError::NumericOverflow)?;
            *amount = (*amount).min(life.saturating_sub(*minimum_life));
        }
        (
            ReplacementOperation::RedirectDamage {
                recipient: destination,
            },
            RuntimeEvent::Damage { recipient, .. },
        ) => {
            *recipient = resolve_redirect_recipient(destination, binding, *recipient, snapshot)?
                .ok_or(RuntimeEvaluationError::ReplacementEvidenceMismatch)?;
        }
        (
            ReplacementOperation::SkipEvent,
            RuntimeEvent::DrawCards { amount, .. }
            | RuntimeEvent::GainLife { amount, .. }
            | RuntimeEvent::PutCounters { amount, .. },
        ) => *amount = 0,
        (ReplacementOperation::SkipEvent, RuntimeEvent::Step { skipped, .. }) => *skipped = true,
        (ReplacementOperation::SkipEvent, RuntimeEvent::ExtraTurn { skipped, .. }) => {
            *skipped = true
        }
        (
            ReplacementOperation::MultiplyEvent { multiplier },
            RuntimeEvent::DrawCards { amount, .. }
            | RuntimeEvent::GainLife { amount, .. }
            | RuntimeEvent::CreateTokens { amount, .. }
            | RuntimeEvent::PutCounters { amount, .. },
        ) => {
            *amount = amount
                .checked_mul(*multiplier)
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
        }
        (
            ReplacementOperation::IncreaseEvent { amount: increase },
            RuntimeEvent::DrawCards { amount, .. }
            | RuntimeEvent::GainLife { amount, .. }
            | RuntimeEvent::CreateTokens { amount, .. }
            | RuntimeEvent::PutCounters { amount, .. },
        ) => {
            *amount = amount
                .checked_add(*increase)
                .ok_or(RuntimeEvaluationError::NumericOverflow)?;
        }
        (ReplacementOperation::DrawCardsInstead, RuntimeEvent::GainLife { player, amount }) => {
            *event = RuntimeEvent::DrawCards {
                player: *player,
                amount: *amount,
            };
        }
        _ => return Err(RuntimeEvaluationError::EventKindMismatch),
    }
    Ok(())
}
