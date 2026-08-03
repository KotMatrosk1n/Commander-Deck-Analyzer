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
    "oracle-static-replacement-compiler-0.8";
pub const ORACLE_STATIC_REPLACEMENT_RUNTIME_VERSION: &str = "oracle-static-replacement-runtime-0.8";
pub const ORACLE_STATIC_REPLACEMENT_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:101.1,109.5,113.6,118.9,400.3,601.2f,601.2h,602.2b,609.4,611.3,613,614-616";

pub const fn oracle_static_replacement_production_adapter_connected() -> bool {
    false
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
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectSelector {
    pub reference: SelectorReference,
    pub zones: BTreeSet<Zone>,
    pub controller: ControllerRelation,
    pub owner: ControllerRelation,
    pub card_types: BTreeSet<CardType>,
    /// Multiple card types normally describe one intersection (for example,
    /// "artifact creature").  Explicit `and`/`or` spell lists instead match
    /// any listed type (for example, "instant and sorcery spells").
    pub card_type_match_any: bool,
    pub colors: BTreeSet<Color>,
    pub subtypes: BTreeSet<String>,
    /// Minimum exact counter counts required on the selected object.
    pub minimum_counters: BTreeMap<CounterKind, u32>,
    pub attacking: Option<bool>,
    pub blocking: Option<bool>,
    pub token_relation: TokenRelation,
    pub exclude_source: bool,
}

impl ObjectSelector {
    fn source() -> Self {
        Self {
            reference: SelectorReference::Source,
            zones: BTreeSet::new(),
            controller: ControllerRelation::Any,
            owner: ControllerRelation::Any,
            card_types: BTreeSet::new(),
            card_type_match_any: false,
            colors: BTreeSet::new(),
            subtypes: BTreeSet::new(),
            minimum_counters: BTreeMap::new(),
            attacking: None,
            blocking: None,
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
            zones,
            controller: ControllerRelation::Any,
            owner: ControllerRelation::Any,
            card_types: BTreeSet::new(),
            card_type_match_any: false,
            colors: BTreeSet::new(),
            subtypes: BTreeSet::new(),
            minimum_counters: BTreeMap::new(),
            attacking: None,
            blocking: None,
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
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Amount {
    Fixed(u32),
    X,
    ThatMany,
    Count(ObjectSelector),
    CounterCount {
        object: ObjectSelector,
        counter: CounterKind,
    },
    KickerPayments,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignedAmount {
    pub negative: bool,
    pub magnitude: Amount,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Condition {
    Always,
    DuringYourTurn,
    NotDuringYourTurn,
    ControllerControls(ObjectSelector),
    ControllerLifeAtMost(u32),
    SourceIsTapped,
    SourceIsUntapped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharacteristicOperation {
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
    CannotBlock {
        blocker: ObjectSelector,
    },
    CannotAttackOrBlock {
        object: ObjectSelector,
    },
    CannotBeBlocked {
        attacker: ObjectSelector,
        by: Option<ObjectSelector>,
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
    PlayLandsFromGraveyard {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticEffect {
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
    CannotAttackOrBlockAlone {
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
    EnterBattlefield {
        object: ObjectSelector,
        condition: EntryReplacementCondition,
    },
    Damage {
        source: ObjectSelector,
        recipient: RecipientSelector,
    },
    DrawCard {
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
    /// Apply the replacement only when no player is at or below this life
    /// total.
    UnlessAnyPlayerLifeAtMost(u32),
    /// Apply the replacement only when the controller has fewer opponents
    /// than this threshold.
    UnlessOpponentCountAtLeast(u32),
    IfSourceWasKicked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntryChoice {
    Color,
    CardType,
    CreatureType,
    Player,
    Opponent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementOperation {
    MoveInstead {
        destination: Zone,
        bottom: bool,
        shuffle_into_library: bool,
    },
    EnterTapped,
    EnterWithCounters {
        counter: CounterKind,
        amount: Amount,
    },
    EnterAsCopy {
        of: ObjectSelector,
        optional: bool,
    },
    ChooseAsEnters(EntryChoice),
    PreventDamage {
        amount: Option<u32>,
    },
    ScaleDamage {
        numerator: u32,
        denominator: u32,
        round_down: bool,
    },
    IncreaseDamage {
        amount: u32,
    },
    SkipEvent,
    MultiplyEvent {
        multiplier: u32,
    },
    IncreaseEvent {
        amount: u32,
    },
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

pub fn compile_oracle_static_replacement_program(
    input: OracleStaticReplacementCompileInput<'_>,
) -> Result<OracleStaticReplacementProgram, StaticReplacementCompileError> {
    compile_program_at_depth(input, 0)
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
    let lower = source.to_ascii_lowercase();
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
        return regex;
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
    if let Some(effect) = parse_nested_grant(source, semantic_context, depth)? {
        return Ok(Some(vec![effect]));
    }
    if source.replace(['’', '‘'], "'") == "You have hexproof."
        && let Some(effect) = parse_restriction_static(source)?
    {
        return Ok(Some(vec![StaticEffect::Restriction(effect)]));
    }
    if source == "Players have no maximum hand size." {
        return Ok(Some(vec![StaticEffect::NoMaximumHandSize {
            player: PlayerSelector::EachPlayer,
        }]));
    }
    if source == "This creature can block any number of creatures." {
        return Ok(Some(vec![StaticEffect::UnlimitedBlockCapacity {
            blocker: parse_object_selector("This creature", Some(Zone::Battlefield))?,
        }]));
    }
    if source == "Players play with the top card of their libraries revealed." {
        return Ok(Some(vec![StaticEffect::RevealLibraryTop {
            player: PlayerSelector::EachPlayer,
        }]));
    }
    if source == "Players play with their hands revealed." {
        return Ok(Some(vec![StaticEffect::RevealHands {
            player: PlayerSelector::EachPlayer,
        }]));
    }
    if source == "Each player can't cast more than one spell each turn." {
        return Ok(Some(vec![StaticEffect::SpellCastLimitEachTurn {
            player: PlayerSelector::EachPlayer,
            maximum: 1,
        }]));
    }
    if source == "You can't cast more than one spell each turn." {
        return Ok(Some(vec![StaticEffect::SpellCastLimitEachTurn {
            player: PlayerSelector::You,
            maximum: 1,
        }]));
    }
    if source == "This creature can't attack or block alone." {
        return Ok(Some(vec![StaticEffect::CannotAttackOrBlockAlone {
            object: parse_object_selector("This creature", Some(Zone::Battlefield))?,
        }]));
    }
    if source == "No more than one creature can attack each combat." {
        return Ok(Some(vec![StaticEffect::CombatGroupLimit {
            group: CombatGroupKind::Attackers,
            maximum: 1,
        }]));
    }
    if source == "No more than one creature can block each combat." {
        return Ok(Some(vec![StaticEffect::CombatGroupLimit {
            group: CombatGroupKind::Blockers,
            maximum: 1,
        }]));
    }
    if let Some(effect) = parse_characteristic_static(source)? {
        return Ok(Some(vec![effect]));
    }
    if let Some(effect) = parse_restriction_static(source)? {
        return Ok(Some(vec![StaticEffect::Restriction(effect)]));
    }
    if let Some(effect) = parse_block_requirement_static(source)? {
        return Ok(Some(vec![StaticEffect::BlockRequirement(effect)]));
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

fn parse_block_requirement_static(
    source: &str,
) -> Result<Option<BlockRequirement>, StaticReplacementCompileError> {
    let normalized = source.replace(['’', '‘'], "'");
    if let Some(captures) =
        static_regex(r"^All creatures able to block (.+?) do so\.$").captures(&normalized)
    {
        return Ok(Some(BlockRequirement::AllAbleBlock {
            attacker: parse_object_selector(
                captures.get(1).expect("all-able block attacker").as_str(),
                Some(Zone::Battlefield),
            )?,
        }));
    }
    if let Some(captures) = static_regex(r"^(.+?) must be blocked if able\.$").captures(&normalized)
    {
        return Ok(Some(BlockRequirement::MustBeBlockedIfAble {
            attacker: parse_object_selector(
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
            attacker: parse_object_selector(
                captures.get(1).expect("minimum-block attacker").as_str(),
                Some(Zone::Battlefield),
            )?,
            minimum,
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

fn parse_characteristic_static(
    source: &str,
) -> Result<Option<StaticEffect>, StaticReplacementCompileError> {
    let (body, condition) = split_static_condition(source)?;

    if let Some(captures) =
        static_regex(r"^(.+?) gets \+1/\+0 for each (.+?) you control\.$").captures(&body)
    {
        let affected = parse_object_selector(
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
        static_regex(r"^(.+?) (?:gets|get) ([+-]1)/([+-]1) for each (.+?)\.$").captures(&body)
    {
        let affected = parse_object_selector(
            captures.get(1).expect("scaled P/T subject").as_str(),
            Some(Zone::Battlefield),
        )?;
        let magnitude = parse_counted_characteristic_amount(
            captures.get(4).expect("scaled P/T count operand").as_str(),
        )?;
        let signed = |value: &str| SignedAmount {
            negative: value.starts_with('-'),
            magnitude: magnitude.clone(),
        };
        return Ok(Some(StaticEffect::Characteristics {
            affected,
            condition,
            operations: vec![CharacteristicOperation::ModifyPowerToughness {
                power: signed(captures.get(2).expect("scaled power").as_str()),
                toughness: signed(captures.get(3).expect("scaled toughness").as_str()),
            }],
        }));
    }

    let pt_pattern =
        r"^(.+?) (?:gets|get) ([+-]\d+|[+-]X)/([+-]\d+|[+-]X)(?: and (?:has|have) (.+))?\.$";
    if let Some(captures) = static_regex(pt_pattern).captures(&body) {
        let affected = parse_object_selector(
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
        let affected = parse_object_selector(
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
        let affected = parse_object_selector(
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
            let affected = parse_object_selector(
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
        let affected = parse_object_selector(
            captures.get(1).expect("loss subject capture").as_str(),
            Some(Zone::Battlefield),
        )?;
        let operand = captures.get(2).expect("loss operand capture").as_str();
        if operand == "all abilities" {
            return Err(StaticReplacementCompileError::UnsupportedOperand(
                operand.to_owned(),
            ));
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
    let normalized = source.trim().replace(['’', '‘'], "'");
    let lower = normalized.to_ascii_lowercase();
    if let Some(counter_text) = lower
        .strip_suffix(" counters on it")
        .or_else(|| lower.strip_suffix(" counter on it"))
    {
        return Ok(Amount::CounterCount {
            object: ObjectSelector::source(),
            counter: parse_counter_kind(counter_text)?,
        });
    }
    if lower == "card in your hand" || lower == "cards in your hand" {
        let mut selector = ObjectSelector::matching(Some(Zone::Hand));
        selector.owner = ControllerRelation::You;
        return Ok(Amount::Count(selector));
    }
    if let Some(card_text) = lower
        .strip_suffix(" cards in your graveyard")
        .or_else(|| lower.strip_suffix(" card in your graveyard"))
    {
        let mut selector = ObjectSelector::matching(Some(Zone::Graveyard));
        selector.owner = ControllerRelation::You;
        if card_text != "card" && !card_text.is_empty() {
            add_noun_type(&mut selector, singularize(card_text))?;
        }
        return Ok(Amount::Count(selector));
    }
    Err(StaticReplacementCompileError::UnsupportedOperand(
        source.to_owned(),
    ))
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
    let Some((body, condition)) = source.rsplit_once(" as long as ") else {
        return Ok((source.to_owned(), Condition::Always));
    };
    let condition = condition
        .strip_suffix('.')
        .ok_or_else(|| StaticReplacementCompileError::UnsupportedOperand(condition.to_owned()))?;
    let parsed = match condition {
        "it's your turn" | "it is your turn" => Condition::DuringYourTurn,
        "it's not your turn" | "it is not your turn" => Condition::NotDuringYourTurn,
        "this permanent is tapped" | "this creature is tapped" => Condition::SourceIsTapped,
        "this permanent is untapped" | "this creature is untapped" => Condition::SourceIsUntapped,
        _ => {
            if let Some(value) = static_regex(r"^you have (\d+) or less life$")
                .captures(condition)
                .and_then(|captures| captures.get(1))
            {
                Condition::ControllerLifeAtMost(parse_u32(value.as_str())?)
            } else if let Some(subject) = condition.strip_prefix("you control ") {
                let mut selector = parse_object_selector(subject, Some(Zone::Battlefield))?;
                selector.controller = ControllerRelation::You;
                Condition::ControllerControls(selector)
            } else {
                return Err(StaticReplacementCompileError::UnsupportedOperand(
                    condition.to_owned(),
                ));
            }
        }
    };
    let mut rebuilt = body.to_owned();
    rebuilt.push('.');
    Ok((rebuilt, parsed))
}

fn parse_restriction_static(
    source: &str,
) -> Result<Option<Restriction>, StaticReplacementCompileError> {
    let normalized = source.replace(['’', '‘'], "'");

    if normalized == "You have hexproof." {
        return Ok(Some(Restriction::CannotBeTargeted {
            target: RecipientSelector::Player(PlayerSelector::You),
            forbidden_controller: PlayerSelector::Opponents,
            spells: true,
            abilities: true,
        }));
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
            object: parse_object_selector(
                captures.get(1).expect("attack or block subject").as_str(),
                Some(Zone::Battlefield),
            )?,
        }));
    }
    if let Some(captures) = static_regex(r"^(.+?) can't attack\.$").captures(&normalized) {
        return Ok(Some(Restriction::CannotAttack {
            attacker: parse_object_selector(
                captures.get(1).expect("attack subject").as_str(),
                Some(Zone::Battlefield),
            )?,
        }));
    }
    if let Some(captures) = static_regex(r"^(.+?) can't block\.$").captures(&normalized) {
        return Ok(Some(Restriction::CannotBlock {
            blocker: parse_object_selector(
                captures.get(1).expect("block subject").as_str(),
                Some(Zone::Battlefield),
            )?,
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
            attacker: parse_object_selector(
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
            target: parse_recipient_selector(
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
            spell: parse_object_selector(
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
    let normalized = source.replace(['’', '‘'], "'");
    if normalized == "You may play lands from your graveyard." {
        return Ok(Some(Permission::PlayLandsFromGraveyard {
            player: PlayerSelector::You,
        }));
    }
    if let Some(captures) = static_regex(
        r"^(You|Your opponents|Players|Each player) may cast (.+?) as though (?:it|they) had flash\.$",
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
        r"^(You|Your opponents|Players|Each player) may cast (.+?) from (your graveyard|their graveyards|exile|the top of your library)\.$",
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
    let normalized = source.replace(['’', '‘'], "'");
    let pattern =
        r"^(.+?) (?:cost|costs) \{(\d+)\} (less|more) to (cast|activate)(?: for each (.+))?\.$";
    let Some(captures) = static_regex(pattern).captures(&normalized) else {
        return Ok(None);
    };
    let subject = captures.get(1).expect("cost subject").as_str();
    let fixed = parse_u32(captures.get(2).expect("cost amount").as_str())?;
    if fixed == 0 {
        return Err(StaticReplacementCompileError::UnsupportedOperand(
            "{0}".to_owned(),
        ));
    }
    let generic_mana = if let Some(each) = captures.get(5) {
        let selector = parse_object_selector(each.as_str(), Some(Zone::Battlefield))?;
        if fixed == 1 {
            Amount::Count(selector)
        } else {
            return Err(StaticReplacementCompileError::UnsupportedOperand(format!(
                "{{{fixed}}} for each {}",
                each.as_str()
            )));
        }
    } else {
        Amount::Fixed(fixed)
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
        CostScope::CastSpell {
            player,
            spells: parse_spell_selector(subject, player)?,
        }
    } else {
        let (player, sources) = parse_ability_cost_subject(subject)?;
        CostScope::ActivateAbility { player, sources }
    };
    Ok(Some(CostModification {
        scope,
        direction,
        generic_mana,
    }))
}

fn parse_skip_step_static(
    source: &str,
) -> Result<Option<StaticEffect>, StaticReplacementCompileError> {
    let normalized = source.replace(['’', '‘'], "'");
    let Some(captures) = static_regex(
        r"^(?:You skip your|Your opponents skip their|Players skip their|Each player skips their) (untap|upkeep|draw|combat|end) steps?\.$",
    )
    .captures(&normalized)
    else {
        return Ok(None);
    };
    let player = if normalized.starts_with("You ") {
        PlayerSelector::You
    } else if normalized.starts_with("Your opponents ") {
        PlayerSelector::Opponents
    } else {
        PlayerSelector::EachPlayer
    };
    Ok(Some(StaticEffect::SkipStep {
        player,
        step: parse_step(captures.get(1).expect("step capture").as_str())?,
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

fn parse_entry_replacement(
    source: &str,
) -> Result<Option<ReplacementEffect>, StaticReplacementCompileError> {
    let normalized = source.replace(['’', '‘'], "'");
    if let Some(captures) =
        static_regex(r"^(.+?) enters(?: the battlefield)? tapped(?: unless (.+))?\.$")
            .captures(&normalized)
    {
        let object = parse_object_selector(
            captures.get(1).expect("entry subject").as_str(),
            Some(Zone::Battlefield),
        )?;
        let condition = captures
            .get(2)
            .map(|value| parse_entry_replacement_condition(value.as_str()))
            .transpose()?
            .unwrap_or(EntryReplacementCondition::Always);
        return Ok(Some(ReplacementEffect {
            predicate: ReplacementEventPredicate::EnterBattlefield { object, condition },
            operation: ReplacementOperation::EnterTapped,
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
        r"^(.+?) enters(?: the battlefield)? with (a|an|one|two|three|\d+|X) (.+?) counters? on it\.$",
    )
    .captures(&normalized)
    {
        let object = parse_object_selector(
            captures.get(1).expect("counter entry subject").as_str(),
            Some(Zone::Battlefield),
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

fn parse_entry_replacement_condition(
    source: &str,
) -> Result<EntryReplacementCondition, StaticReplacementCompileError> {
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
    Err(StaticReplacementCompileError::UnsupportedOperand(
        source.to_owned(),
    ))
}

fn parse_zone_replacement(
    source: &str,
) -> Result<Option<ReplacementEffect>, StaticReplacementCompileError> {
    let normalized = source.replace(['’', '‘'], "'");
    if let Some(captures) =
        static_regex(r"^If (.+?) would die, exile it instead\.$").captures(&normalized)
    {
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
                destination: Zone::Exile,
                bottom: false,
                shuffle_into_library: false,
            },
            optional: false,
        }));
    }
    let pattern = r"^If (.+?) would be put into (a graveyard|a hand|a library|its owner's graveyard|its owner's hand|its owner's library|their owner's graveyard|their owner's hand|their owner's library|your graveyard|your hand|your library|exile) from (anywhere|the battlefield|a graveyard|a hand|a library|exile), (exile it|put it into its owner's hand|put it on the bottom of its owner's library|shuffle it into its owner's library) instead\.$";
    let Some(captures) = static_regex(pattern).captures(&normalized) else {
        return Ok(None);
    };
    let destination_phrase = captures.get(2).expect("zone destination phrase").as_str();
    let destination_zone = destination_phrase
        .split_whitespace()
        .next_back()
        .expect("destination phrase has a final zone");
    let to = parse_zone(destination_zone)?;
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
    let (destination, bottom, shuffle_into_library) =
        match captures.get(4).expect("zone operation").as_str() {
            "exile it" => (Zone::Exile, false, false),
            "put it into its owner's hand" => (Zone::Hand, false, false),
            "put it on the bottom of its owner's library" => (Zone::Library, true, false),
            "shuffle it into its owner's library" => (Zone::Library, false, true),
            _ => unreachable!("regex limits zone operation"),
        };
    let mut object = parse_object_selector(captures.get(1).expect("zone subject").as_str(), None)?;
    if destination_phrase.starts_with("your ") {
        object.owner = ControllerRelation::You;
    }
    Ok(Some(ReplacementEffect {
        predicate: ReplacementEventPredicate::ZoneChange { object, from, to },
        operation: ReplacementOperation::MoveInstead {
            destination,
            bottom,
            shuffle_into_library,
        },
        optional: false,
    }))
}

fn parse_damage_replacement(
    source: &str,
) -> Result<Option<ReplacementEffect>, StaticReplacementCompileError> {
    let normalized = source.replace(['’', '‘'], "'");
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
                    round_down: true,
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
            },
            operation,
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
    let normalized = source.replace(['’', '‘'], "'");
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
    Ok(None)
}

fn parse_skip_replacement(
    source: &str,
) -> Result<Option<ReplacementEffect>, StaticReplacementCompileError> {
    let normalized = source.replace(['’', '‘'], "'");
    let Some(captures) = static_regex(
        r"^If (you|an opponent|a player) would draw a card, (?:you|that player) skips? that draw instead\.$",
    )
    .captures(&normalized)
    else {
        return Ok(None);
    };
    let player = match captures.get(1).expect("draw player").as_str() {
        "you" => PlayerSelector::You,
        "an opponent" => PlayerSelector::Opponents,
        "a player" => PlayerSelector::EachPlayer,
        _ => unreachable!("regex limits draw player"),
    };
    Ok(Some(ReplacementEffect {
        predicate: ReplacementEventPredicate::DrawCard { player },
        operation: ReplacementOperation::SkipEvent,
        optional: false,
    }))
}

fn parse_object_selector(
    source: &str,
    default_zone: Option<Zone>,
) -> Result<ObjectSelector, StaticReplacementCompileError> {
    let mut text = source.trim().replace(['’', '‘'], "'");
    if text.is_empty() {
        return Err(StaticReplacementCompileError::UnsupportedSubject(
            source.to_owned(),
        ));
    }
    let lower = text.to_ascii_lowercase();
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
    if let Some(rest) = text.strip_prefix("other ") {
        selector.exclude_source = true;
        text = rest.to_owned();
    } else if let Some(rest) = text.strip_prefix("Other ") {
        selector.exclude_source = true;
        text = rest.to_owned();
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
    let (base_end, relation) = if lower.ends_with(" you control") {
        (text.len() - " you control".len(), ControllerRelation::You)
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
    if let Some(type_list) = base_lower.strip_suffix(" spells") {
        let alternatives = type_list
            .split(" and ")
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
                .collect::<Result<BTreeSet<_>, _>>()?;
            if card_types.len() == alternatives.len() {
                selector.card_types = card_types;
                selector.card_type_match_any = true;
                return Ok(selector);
            }
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

    let noun_original = words
        .last()
        .map(String::as_str)
        .ok_or_else(|| StaticReplacementCompileError::UnsupportedSubject(source.to_owned()))?;
    let noun_lower = noun_original.to_ascii_lowercase();
    let noun = singularize(&noun_lower);
    if noun == "permanent" || noun == "card" || noun == "object" || noun == "spell" {
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
            "another",
            "basic",
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
            "legendary",
            "monocolored",
            "monstrous",
            "modified",
            "multicolored",
            "nonartifact",
            "noncreature",
            "nonland",
            "nonlegendary",
            "nonsnow",
            "paired",
            "permanent",
            "phased-out",
            "premium",
            "renowned",
            "saddled",
            "snow",
            "stickered",
            "suspected",
            "tapped",
            "target",
            "transformed",
            "unblocked",
            "untapped",
            "white-bordered",
        ]
        .contains(&adjective)
            || adjective.starts_with("non")
        {
            return Err(StaticReplacementCompileError::UnsupportedSubject(
                source.to_owned(),
            ));
        }
        if adjective == "attacking" {
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
    let lower = source.to_ascii_lowercase();
    if lower == "activated abilities you activate" {
        return Ok((PlayerSelector::You, None));
    }
    if lower == "activated abilities your opponents activate" {
        return Ok((PlayerSelector::Opponents, None));
    }
    if let Some(subject) = lower
        .strip_prefix("activated abilities of ")
        .and_then(|value| value.strip_suffix(" you control"))
    {
        return Ok((
            PlayerSelector::You,
            Some(parse_object_selector(subject, Some(Zone::Battlefield))?),
        ));
    }
    if let Some(subject) = lower
        .strip_prefix("activated abilities of ")
        .and_then(|value| value.strip_suffix(" your opponents control"))
    {
        return Ok((
            PlayerSelector::Opponents,
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
    match source.trim().to_ascii_lowercase().as_str() {
        "you" => Ok(RecipientSelector::Player(PlayerSelector::You)),
        "an opponent" | "your opponents" => {
            Ok(RecipientSelector::Player(PlayerSelector::Opponents))
        }
        "a player" | "each player" | "players" => {
            Ok(RecipientSelector::Player(PlayerSelector::EachPlayer))
        }
        "any target" => Ok(RecipientSelector::Any),
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
        "your opponents" | "opponents" => Ok(PlayerSelector::Opponents),
        "players" | "each player" | "a player" => Ok(PlayerSelector::EachPlayer),
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
        // Magic uses the ordinary English irregular plural for these subtype
        // words. Keep the conversion exact instead of guessing at arbitrary
        // lowercase nouns that happen to end in `ves`.
        "Elves" => "Elf".to_owned(),
        "Dwarves" => "Dwarf".to_owned(),
        "Wolves" => "Wolf".to_owned(),
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
    Ok(Amount::Fixed(parse_word_or_number(source)?))
}

fn parse_word_or_number(source: &str) -> Result<u32, StaticReplacementCompileError> {
    match source {
        "a" | "an" | "one" => Ok(1),
        "two" => Ok(2),
        "three" => Ok(3),
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
    pub card_types: BTreeSet<CardType>,
    pub colors: BTreeSet<Color>,
    pub subtypes: BTreeSet<String>,
    /// Exact counters currently on this object.
    pub counters: BTreeMap<CounterKind, u32>,
    pub keywords: BTreeSet<KeywordAbility>,
    pub token: bool,
    pub tapped: bool,
    pub attacking: bool,
    pub blocking: bool,
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
    EntryCastEvidenceMissing(ObjectRef),
    ReplacementEvidenceMismatch,
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
    IllegalBlockerEvidence(ObjectRef),
    DeclaredBlockerNotAble(ObjectRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCharacteristics {
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
        prior_land_plays_this_turn: u32,
    },
    ActivateAbility {
        player: PlayerId,
        source: ObjectRef,
        is_mana_ability: bool,
    },
    DeclareAttack {
        attacker: ObjectRef,
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
    MaximumExceeded {
        binding_id: BindingId,
        group: CombatGroupKind,
        maximum: u32,
        declared: u32,
    },
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeRecipient {
    Player(PlayerId),
    Object(ObjectRef, PlayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryPlacement {
    Unspecified,
    Bottom,
    Shuffled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryChoiceValue {
    Color(Color),
    CardType(CardType),
    CreatureType(String),
    Player(PlayerId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingReplacementEvent {
    pub event_id: ReplacementEventId,
    pub original: RuntimeEvent,
    pub current: RuntimeEvent,
    pub handled_bindings: BTreeSet<BindingId>,
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

#[derive(Debug, Clone)]
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
            ) || !condition_holds(condition, &entry.binding, snapshot)?
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
            apply_characteristic_operation(&mut result, operation, binding, snapshot)?;
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
                | BlockRequirement::MinimumBlockers { attacker, .. } => attacker,
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
        for entry in self.expanded_static_effects(snapshot) {
            let StaticEffect::Permission(permission) = &entry.effect else {
                continue;
            };
            if let Permission::AdditionalLandPlays { amount, .. } = permission
                && additional_land_permission_matches_action(
                    action,
                    permission,
                    &entry.binding,
                    snapshot,
                )?
            {
                additional_land_plays = additional_land_plays
                    .checked_add(*amount)
                    .ok_or(RuntimeEvaluationError::NumericOverflow)?;
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
            if prior_land_plays_this_turn >= 1 && prior_land_plays_this_turn < maximum_land_plays {
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

    pub fn begin_replacement_event(
        &mut self,
        event: RuntimeEvent,
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
            handled_bindings: BTreeSet::new(),
        })
    }

    pub fn applicable_replacements(
        &self,
        snapshot: &RuntimeSnapshot,
        event: &PendingReplacementEvent,
    ) -> Result<Vec<BindingId>, RuntimeEvaluationError> {
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
            if replacement_matches(replacement, binding, &event.current, snapshot)? {
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
                apply_replacement_operation(
                    &mut staged.current,
                    &replacement.operation,
                    binding,
                    snapshot,
                    entry_choice,
                    copy_object,
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
            },
        ) => Ok(selector_matches(
            attacker,
            binding,
            object(actual_attacker)?,
            snapshot.perspective_player,
        )),
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
        (Permission::AdditionalLandPlays { .. }, StaticAction::PlayLand { .. }) => Ok(false),
        _ => Ok(false),
    }
}

fn additional_land_permission_matches_action(
    action: StaticAction,
    permission: &Permission,
    binding: &BoundProgram,
    snapshot: &RuntimeSnapshot,
) -> Result<bool, RuntimeEvaluationError> {
    let (
        Permission::AdditionalLandPlays {
            player,
            during_own_turn,
            ..
        },
        StaticAction::PlayLand {
            player: actual_player,
            land,
            from,
            ..
        },
    ) = (permission, action)
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
                && selector_matches(spells, binding, state, snapshot.perspective_player))
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
                }))
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
    if selector.reference == SelectorReference::Source && object.object_ref != binding.source {
        return false;
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
    if !card_types_match
        || !selector.colors.is_subset(&object.colors)
        || !selector.subtypes.is_subset(&object.subtypes)
        || selector
            .minimum_counters
            .iter()
            .any(|(kind, minimum)| object.counters.get(kind).copied().unwrap_or(0) < *minimum)
        || selector
            .attacking
            .is_some_and(|expected| object.attacking != expected)
        || selector
            .blocking
            .is_some_and(|expected| object.blocking != expected)
    {
        return false;
    }
    match selector.token_relation {
        TokenRelation::Any => true,
        TokenRelation::Token => object.token,
        TokenRelation::Nontoken => !object.token,
    }
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
    snapshot: &RuntimeSnapshot,
) -> Result<bool, RuntimeEvaluationError> {
    match condition {
        Condition::Always => Ok(true),
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
        Condition::ControllerLifeAtMost(maximum) => {
            if !snapshot.complete_players.contains(&binding.controller) {
                return Err(RuntimeEvaluationError::PlayerStateIncomplete(
                    binding.controller,
                ));
            }
            snapshot
                .life_totals
                .get(&binding.controller)
                .map(|life| *life <= i32::try_from(*maximum).unwrap_or(i32::MAX))
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
    snapshot: &RuntimeSnapshot,
) -> Result<(), RuntimeEvaluationError> {
    match operation {
        CharacteristicOperation::ModifyPowerToughness { power, toughness } => {
            let power_delta = evaluate_signed_amount(power, binding, snapshot)?;
            let toughness_delta = evaluate_signed_amount(toughness, binding, snapshot)?;
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
            result.power = Some(evaluate_amount(power, binding, snapshot)?);
            result.toughness = Some(evaluate_amount(toughness, binding, snapshot)?);
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
    snapshot: &RuntimeSnapshot,
) -> Result<i32, RuntimeEvaluationError> {
    let magnitude = evaluate_amount(&amount.magnitude, binding, snapshot)?;
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
            ReplacementEventPredicate::Damage { source, recipient },
            RuntimeEvent::Damage {
                source: actual_source,
                recipient: actual_recipient,
                preventable,
                ..
            },
        ) => Ok((!matches!(
            replacement.operation,
            ReplacementOperation::PreventDamage { .. }
        ) || *preventable)
            && selector_matches(source, binding, actual_source, snapshot.perspective_player)
            && recipient_matches(recipient, binding, *actual_recipient, snapshot)?),
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
        | RuntimeEvent::Step { .. } => return Ok(()),
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
        | RuntimeEvent::Step { player, .. } => *player,
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
) -> Result<(), RuntimeEvaluationError> {
    match operation {
        ReplacementOperation::EnterAsCopy { .. } if entry_choice.is_some() => {
            return Err(RuntimeEvaluationError::UnexpectedReplacementChoiceEvidence);
        }
        ReplacementOperation::ChooseAsEnters(_) if copy_object.is_some() => {
            return Err(RuntimeEvaluationError::UnexpectedReplacementChoiceEvidence);
        }
        ReplacementOperation::EnterAsCopy { .. } | ReplacementOperation::ChooseAsEnters(_) => {}
        _ if entry_choice.is_some() || copy_object.is_some() => {
            return Err(RuntimeEvaluationError::UnexpectedReplacementChoiceEvidence);
        }
        _ => {}
    }
    match (operation, event) {
        (
            ReplacementOperation::MoveInstead {
                destination,
                bottom,
                shuffle_into_library,
            },
            RuntimeEvent::ZoneChange {
                to,
                library_placement,
                ..
            },
        ) => {
            *to = *destination;
            *library_placement = if *bottom {
                LibraryPlacement::Bottom
            } else if *shuffle_into_library {
                LibraryPlacement::Shuffled
            } else {
                LibraryPlacement::Unspecified
            };
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
                entry_choice: event_choice,
                ..
            },
        ) => {
            let choice =
                entry_choice.ok_or(RuntimeEvaluationError::EntryChoiceRequired(*expected))?;
            let correct = matches!(
                (expected, &choice),
                (EntryChoice::Color, EntryChoiceValue::Color(_))
                    | (EntryChoice::CardType, EntryChoiceValue::CardType(_))
                    | (EntryChoice::CreatureType, EntryChoiceValue::CreatureType(_))
                    | (EntryChoice::Player, EntryChoiceValue::Player(_))
                    | (EntryChoice::Opponent, EntryChoiceValue::Player(_))
            );
            if !correct {
                return Err(RuntimeEvaluationError::EntryChoiceRequired(*expected));
            }
            let legal = match &choice {
                EntryChoiceValue::Color(_) | EntryChoiceValue::CardType(_) => true,
                EntryChoiceValue::CreatureType(creature_type) => {
                    snapshot.legal_creature_types.contains(creature_type)
                }
                EntryChoiceValue::Player(player) => {
                    snapshot.complete_players.contains(player)
                        && (!matches!(expected, EntryChoice::Opponent)
                            || *player != binding.controller)
                }
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
        (ReplacementOperation::SkipEvent, RuntimeEvent::DrawCards { amount, .. }) => *amount = 0,
        (ReplacementOperation::SkipEvent, RuntimeEvent::Step { skipped, .. }) => *skipped = true,
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
        _ => return Err(RuntimeEvaluationError::EventKindMismatch),
    }
    Ok(())
}
