//! Content keyed action algebra for complete resolving Oracle instructions.
//!
//! This module deliberately owns neither trigger, activation-cost, casting,
//! timing, replacement, modal, nor static-ability envelopes. It accepts an
//! instruction only when the complete normalized source can be represented by
//! typed operands and by actions whose state transition is implemented below.
//! There is no opaque text node and no generic action fallback.
//!
//! Semantic identity is derived from the exact Oracle instruction, its typed
//! resolution context, the complete algebra, and versioned compiler, runtime,
//! and rules contracts. Card names, object identifiers, database rows, clause
//! addresses, snapshot hashes, timestamps, and source ordering are not inputs.
//! An unchanged Oracle instruction therefore keeps the same identity across a
//! card snapshot refresh.
//!
//! The production adapter remains disconnected. The staged adapter in this
//! file is a complete local contract for the action forms the parser accepts:
//! it commits the whole program or leaves the caller's state unchanged.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::OnceLock;

use regex::Regex;
use sha2::{Digest, Sha256};

pub const ORACLE_ACTION_ALGEBRA_COMPILER_VERSION: &str = "oracle-action-algebra-compiler-0.4";
pub const ORACLE_ACTION_ALGEBRA_RUNTIME_VERSION: &str = "oracle-action-algebra-runtime-0.4";
pub const ORACLE_ACTION_ALGEBRA_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:101-102,104,107,109,111,119-122,400-406,608.2c-d,609-611,613,701.3,701.6-9,701.13-15,701.17-20,701.25,701.32,701.35,701.45,707";

/// Recognition here cannot become production execution coverage until the
/// host engine binds every choice and its replacement-effect pipeline.
pub const fn oracle_action_algebra_production_adapter_connected() -> bool {
    false
}

pub type PlayerId = u16;
pub type ObjectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncarnationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum OracleActionSemanticContext {
    #[default]
    ResolvingSpellInstruction,
    ResolvingActivatedAbilityInstruction,
    ResolvingTriggeredAbilityInstruction,
    ResolvingSpecialActionInstruction,
    ResolvingDungeonRoomInstruction,
}

impl OracleActionSemanticContext {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::ResolvingSpellInstruction => "resolving-spell-instruction/v1",
            Self::ResolvingActivatedAbilityInstruction => {
                "resolving-activated-ability-instruction/v1"
            }
            Self::ResolvingTriggeredAbilityInstruction => {
                "resolving-triggered-ability-instruction/v1"
            }
            Self::ResolvingSpecialActionInstruction => "resolving-special-action-instruction/v1",
            Self::ResolvingDungeonRoomInstruction => "resolving-dungeon-room-instruction/v1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleActionCompileInput<'a> {
    pub exact_source: &'a str,
    pub normalized_source: &'a str,
    pub semantic_context: OracleActionSemanticContext,
}

impl<'a> OracleActionCompileInput<'a> {
    pub fn resolving_spell(exact_source: &'a str) -> Self {
        Self {
            exact_source,
            normalized_source: exact_source,
            semantic_context: OracleActionSemanticContext::ResolvingSpellInstruction,
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
    const fn stable_id(self) -> &'static str {
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
    Trample,
    Vigilance,
    Ward,
    Wither,
}

impl KeywordAbility {
    fn parse(source: &str) -> Option<Self> {
        match source.trim().to_ascii_lowercase().as_str() {
            "deathtouch" => Some(Self::Deathtouch),
            "defender" => Some(Self::Defender),
            "double strike" => Some(Self::DoubleStrike),
            "first strike" => Some(Self::FirstStrike),
            "flash" => Some(Self::Flash),
            "flying" => Some(Self::Flying),
            "haste" => Some(Self::Haste),
            "hexproof" => Some(Self::Hexproof),
            "indestructible" => Some(Self::Indestructible),
            "infect" => Some(Self::Infect),
            "lifelink" => Some(Self::Lifelink),
            "menace" => Some(Self::Menace),
            "reach" => Some(Self::Reach),
            "trample" => Some(Self::Trample),
            "vigilance" => Some(Self::Vigilance),
            "ward" => Some(Self::Ward),
            "wither" => Some(Self::Wither),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VariableAmount {
    X,
    ThatMany,
    NumberOfSelectedObjects,
    SourcePower,
    SourceToughness,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Amount {
    Fixed(u32),
    Variable(VariableAmount),
}

impl Amount {
    fn fixed_positive(value: u32) -> Option<Self> {
        (value > 0).then_some(Self::Fixed(value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cardinality {
    ExactlyOne,
    Exactly(u32),
    UpTo(u32),
    AnyNumber,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlayerOperand {
    You,
    TargetPlayer,
    TargetOpponent,
    EachPlayer,
    EachOpponent,
    ThatPlayer,
    ChosenPlayer,
    OwnerOfSelectedObject,
    ControllerOfSelectedObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControllerConstraint {
    Any,
    You,
    Opponent,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectFilter {
    pub required_types: BTreeSet<CardType>,
    pub any_types: BTreeSet<CardType>,
    pub excluded_types: BTreeSet<CardType>,
    pub required_supertypes: BTreeSet<String>,
    pub required_colors: BTreeSet<Color>,
    pub excluded_colors: BTreeSet<Color>,
    pub required_subtypes: BTreeSet<String>,
    pub controller: ControllerConstraint,
    pub other_than_source: bool,
    pub token: Option<bool>,
    pub attacking: Option<bool>,
    pub blocking: Option<bool>,
    pub tapped: Option<bool>,
}

impl ObjectFilter {
    pub fn permanent() -> Self {
        Self {
            required_types: BTreeSet::new(),
            any_types: BTreeSet::new(),
            excluded_types: BTreeSet::new(),
            required_supertypes: BTreeSet::new(),
            required_colors: BTreeSet::new(),
            excluded_colors: BTreeSet::new(),
            required_subtypes: BTreeSet::new(),
            controller: ControllerConstraint::Any,
            other_than_source: false,
            token: None,
            attacking: None,
            blocking: None,
            tapped: None,
        }
    }

    pub fn creature() -> Self {
        let mut filter = Self::permanent();
        filter.required_types.insert(CardType::Creature);
        filter
    }

    pub fn card() -> Self {
        Self::permanent()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectOperand {
    Source,
    It,
    ThatObject,
    PreviousSelection,
    Target {
        slot: u8,
        cardinality: Cardinality,
        filter: ObjectFilter,
    },
    Set {
        cardinality: Cardinality,
        filter: ObjectFilter,
    },
    EnchantedObject,
    EquippedObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ZoneOwner {
    You,
    TargetPlayer,
    ThatPlayer,
    ObjectOwner,
    ObjectController,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZoneOperand {
    pub owner: ZoneOwner,
    pub zone: Zone,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CardSelection {
    pub zone: ZoneOperand,
    pub cardinality: Cardinality,
    pub filter: ObjectFilter,
    pub from_top: bool,
    pub random: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SacrificeSelection {
    Objects(ObjectOperand),
    Choice {
        cardinality: Cardinality,
        filter: ObjectFilter,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DamageRecipient {
    Player(PlayerOperand),
    Object(ObjectOperand),
    AnyTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DamageSource {
    SourceObject,
    ThisSpell,
    Object(ObjectOperand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Duration {
    Permanent,
    ThisTurn,
    UntilEndOfTurn,
    UntilYourNextTurn,
    UntilSourceLeavesBattlefield,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatePredicate {
    PreviousActionSucceeded,
    YouControl {
        filter: ObjectFilter,
        comparison: CountComparison,
    },
    YourHandIsEmpty,
    YourGraveyardHas {
        filter: ObjectFilter,
        comparison: CountComparison,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CountComparison {
    Exactly(u32),
    AtLeast(u32),
    AtMost(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SequenceSeparator {
    Then,
    Sentence,
    And,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CounterOperation {
    Put,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeywordOperation {
    Grant,
    Lose,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenTemplate {
    pub name: String,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
    pub colors: BTreeSet<Color>,
    pub card_types: BTreeSet<CardType>,
    pub subtypes: BTreeSet<String>,
    pub keywords: BTreeSet<KeywordAbility>,
    pub tapped: bool,
    pub attacking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LibraryPosition {
    Top,
    Bottom,
    Shuffled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardSelectionPolicy {
    Exact,
    AsMuchAsPossible,
    HiddenSearch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionKind {
    Draw {
        player: PlayerOperand,
        amount: Amount,
    },
    Discard {
        player: PlayerOperand,
        selection: CardSelection,
    },
    Sacrifice {
        player: PlayerOperand,
        selection: SacrificeSelection,
    },
    GainLife {
        player: PlayerOperand,
        amount: Amount,
    },
    LoseLife {
        player: PlayerOperand,
        amount: Amount,
    },
    DealDamage {
        source: DamageSource,
        recipient: DamageRecipient,
        amount: Amount,
    },
    Fight {
        first: ObjectOperand,
        second: ObjectOperand,
    },
    Tap {
        objects: ObjectOperand,
    },
    Untap {
        objects: ObjectOperand,
    },
    ChangeCounters {
        operation: CounterOperation,
        objects: ObjectOperand,
        counter: String,
        amount: Amount,
    },
    ModifyPowerToughness {
        objects: ObjectOperand,
        power: i32,
        toughness: i32,
        duration: Duration,
    },
    ChangeKeywords {
        operation: KeywordOperation,
        objects: ObjectOperand,
        keywords: BTreeSet<KeywordAbility>,
        duration: Duration,
    },
    CreateToken {
        player: PlayerOperand,
        amount: Amount,
        template: TokenTemplate,
    },
    CreateCopyToken {
        player: PlayerOperand,
        source: ObjectOperand,
        tapped: bool,
    },
    MoveZone {
        objects: ObjectOperand,
        from: Option<Zone>,
        destination: ZoneOperand,
        position: Option<LibraryPosition>,
        tapped: bool,
    },
    MoveCards {
        selection: CardSelection,
        destination: ZoneOperand,
        position: Option<LibraryPosition>,
        tapped: bool,
    },
    Reveal {
        player: PlayerOperand,
        selection: CardSelection,
    },
    Look {
        player: PlayerOperand,
        selection: CardSelection,
    },
    SearchLibrary {
        player: PlayerOperand,
        library_owner: ZoneOwner,
        selection: CardSelection,
        destination: ZoneOperand,
        position: Option<LibraryPosition>,
        reveal: bool,
        tapped: bool,
        shuffle_after: bool,
    },
    Mill {
        player: PlayerOperand,
        amount: Amount,
    },
    ShuffleLibrary {
        player: PlayerOperand,
    },
    Optional {
        actor: PlayerOperand,
        action: Box<ActionNode>,
    },
    Conditional {
        predicate: StatePredicate,
        if_true: Box<ActionNode>,
        if_false: Option<Box<ActionNode>>,
    },
    OrderedSequence {
        actions: Vec<ActionNode>,
        separators: Vec<SequenceSeparator>,
    },
}

impl ActionKind {
    pub const fn family(&self) -> OracleActionFamily {
        match self {
            Self::Draw { .. } => OracleActionFamily::Draw,
            Self::Discard { .. } => OracleActionFamily::Discard,
            Self::Sacrifice { .. } => OracleActionFamily::Sacrifice,
            Self::GainLife { .. } | Self::LoseLife { .. } => OracleActionFamily::Life,
            Self::DealDamage { .. } => OracleActionFamily::Damage,
            Self::Fight { .. } => OracleActionFamily::Fight,
            Self::Tap { .. } | Self::Untap { .. } => OracleActionFamily::TapUntap,
            Self::ChangeCounters { .. } => OracleActionFamily::Counters,
            Self::ModifyPowerToughness { .. } => OracleActionFamily::PowerToughness,
            Self::ChangeKeywords { .. } => OracleActionFamily::Keywords,
            Self::CreateToken { .. } => OracleActionFamily::Token,
            Self::CreateCopyToken { .. } => OracleActionFamily::Copy,
            Self::MoveZone { .. } | Self::MoveCards { .. } => OracleActionFamily::ZoneMovement,
            Self::Reveal { .. } => OracleActionFamily::Reveal,
            Self::Look { .. } => OracleActionFamily::Look,
            Self::SearchLibrary { .. } => OracleActionFamily::Search,
            Self::Mill { .. } => OracleActionFamily::Mill,
            Self::ShuffleLibrary { .. } => OracleActionFamily::Shuffle,
            Self::Optional { .. } => OracleActionFamily::Optional,
            Self::Conditional { .. } => OracleActionFamily::Conditional,
            Self::OrderedSequence { .. } => OracleActionFamily::Sequence,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OracleActionFamily {
    Draw,
    Discard,
    Sacrifice,
    Life,
    Damage,
    Fight,
    TapUntap,
    Counters,
    PowerToughness,
    Keywords,
    Token,
    Copy,
    ZoneMovement,
    Reveal,
    Look,
    Search,
    Mill,
    Shuffle,
    Optional,
    Conditional,
    Sequence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionNode {
    pub id: ActionId,
    pub kind: ActionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleActionProgram {
    exact_source: String,
    normalized_source: String,
    semantic_context: OracleActionSemanticContext,
    semantic_digest: String,
    root: ActionNode,
}

impl OracleActionProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub const fn semantic_context(&self) -> OracleActionSemanticContext {
        self.semantic_context
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn root(&self) -> &ActionNode {
        &self.root
    }

    pub const fn production_adapter_connected(&self) -> bool {
        oracle_action_algebra_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OracleActionRejection {
    EmptyOrMalformedSource,
    NormalizationMismatch,
    TriggerEnvelope,
    ActivationCostEnvelope,
    TimingRestrictionEnvelope,
    ReplacementEnvelope,
    ModalEnvelope,
    StaticAbility,
    AmbiguousComposition,
    UnsupportedOperand,
    UnsupportedAmount,
    UnsupportedAction,
    UnconsumedSource,
}

impl fmt::Display for OracleActionRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for OracleActionRejection {}

// Kept inline because this public classification contract is matched throughout production.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleActionClassification {
    Program(OracleActionProgram),
    Rejected(OracleActionRejection),
}

pub fn reviewed_oracle_action_normalized_source(exact_source: &str) -> String {
    collapse_whitespace(exact_source)
}

pub fn compile_oracle_action_program(
    input: OracleActionCompileInput<'_>,
) -> Result<OracleActionProgram, OracleActionRejection> {
    match classify_oracle_action_instruction(input) {
        OracleActionClassification::Program(program) => Ok(program),
        OracleActionClassification::Rejected(reason) => Err(reason),
    }
}

pub fn classify_oracle_action_instruction(
    input: OracleActionCompileInput<'_>,
) -> OracleActionClassification {
    if !is_complete_single_line(input.exact_source)
        || !is_complete_single_line(input.normalized_source)
    {
        return OracleActionClassification::Rejected(OracleActionRejection::EmptyOrMalformedSource);
    }
    if reviewed_oracle_action_normalized_source(input.exact_source) != input.normalized_source {
        return OracleActionClassification::Rejected(OracleActionRejection::NormalizationMismatch);
    }

    if let Some(reason) = reject_outer_envelope(input.normalized_source) {
        return OracleActionClassification::Rejected(reason);
    }

    let mut parser = ActionParser::default();
    let mut root = match parser.parse_complete(input.normalized_source) {
        Ok(root) => root,
        Err(reason) => return OracleActionClassification::Rejected(reason),
    };
    let mut next_id = 1u32;
    assign_action_ids(&mut root, &mut next_id);
    let semantic_digest = oracle_action_semantic_digest(
        input.exact_source,
        input.normalized_source,
        input.semantic_context,
        &root,
    );
    OracleActionClassification::Program(OracleActionProgram {
        exact_source: input.exact_source.to_owned(),
        normalized_source: input.normalized_source.to_owned(),
        semantic_context: input.semantic_context,
        semantic_digest,
        root,
    })
}

fn reject_outer_envelope(source: &str) -> Option<OracleActionRejection> {
    let lower = source.to_ascii_lowercase();
    if lower.starts_with("when ")
        || lower.starts_with("whenever ")
        || lower.starts_with("at the beginning ")
        || lower.starts_with("at the end ")
        || lower.starts_with("after ")
        || lower.starts_with("the next time ")
    {
        return Some(OracleActionRejection::TriggerEnvelope);
    }
    if contains_top_level_colon(source)
        || lower.starts_with("as an additional cost ")
        || lower.starts_with("as an additional cost, ")
        || lower.starts_with("to cast ")
    {
        return Some(OracleActionRejection::ActivationCostEnvelope);
    }
    if lower.contains(" activate only ")
        || lower.starts_with("activate only ")
        || lower.contains(" cast only ")
        || lower.starts_with("cast only ")
        || lower.starts_with("during ")
        || lower.starts_with("until ")
    {
        return Some(OracleActionRejection::TimingRestrictionEnvelope);
    }
    if lower.contains(" would ")
        || lower.contains(" instead")
        || lower.starts_with("as ") && lower.contains(" enters ")
    {
        return Some(OracleActionRejection::ReplacementEnvelope);
    }
    if source.starts_with('•')
        || lower.starts_with("choose one")
        || lower.starts_with("choose two")
        || lower.starts_with("choose any number")
        || lower.starts_with("choose one or both")
    {
        return Some(OracleActionRejection::ModalEnvelope);
    }
    None
}

#[derive(Default)]
struct ActionParser {
    depth: u8,
}

impl ActionParser {
    fn parse_complete(&mut self, source: &str) -> Result<ActionNode, OracleActionRejection> {
        let source = strip_one_terminal_period(source)
            .ok_or(OracleActionRejection::EmptyOrMalformedSource)?;
        if source.is_empty() {
            return Err(OracleActionRejection::EmptyOrMalformedSource);
        }
        self.parse_node(source)
    }

    fn parse_node(&mut self, source: &str) -> Result<ActionNode, OracleActionRejection> {
        if self.depth >= 24 {
            return Err(OracleActionRejection::AmbiguousComposition);
        }
        self.depth += 1;
        let result = self.parse_node_inner(source.trim());
        self.depth -= 1;
        result
    }

    fn parse_node_inner(&mut self, source: &str) -> Result<ActionNode, OracleActionRejection> {
        if source.is_empty() || source.trim() != source {
            return Err(OracleActionRejection::EmptyOrMalformedSource);
        }
        if let Some(reason) = reject_outer_envelope(source) {
            return Err(reason);
        }
        let sentence_cased;
        let source = if source
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_lowercase)
        {
            sentence_cased = sentence_case_action(source);
            sentence_cased.as_str()
        } else {
            source
        };

        if let Some(action) = self.parse_optional(source)? {
            return Ok(action);
        }
        if let Some(action) = self.parse_conditional(source)? {
            return Ok(action);
        }
        if let Some(action) = parse_atomic_action(source) {
            return Ok(action);
        }
        if let Some(sequence) = self.parse_ordered_sequence(source)? {
            return Ok(sequence);
        }

        let lower = source.to_ascii_lowercase();
        if lower.contains(" gets ")
            || lower.contains(" get ")
            || lower.contains(" gain ")
            || lower.contains(" have ")
            || lower.contains(" has ")
            || lower.contains(" can't ")
            || lower.contains(" cannot ")
        {
            return Err(OracleActionRejection::StaticAbility);
        }
        Err(OracleActionRejection::UnsupportedAction)
    }

    fn parse_optional(
        &mut self,
        source: &str,
    ) -> Result<Option<ActionNode>, OracleActionRejection> {
        let candidates = [
            ("You may ", PlayerOperand::You),
            ("Target player may ", PlayerOperand::TargetPlayer),
            ("Target opponent may ", PlayerOperand::TargetOpponent),
            ("That player may ", PlayerOperand::ThatPlayer),
        ];
        for (prefix, actor) in candidates {
            if let Some(body) = source.strip_prefix(prefix) {
                if body.is_empty() {
                    return Err(OracleActionRejection::UnconsumedSource);
                }
                let body = sentence_case_action(body);
                let mut action = self.parse_node(&body)?;
                if actor != PlayerOperand::You {
                    rebind_implicit_you(&mut action, actor);
                }
                return Ok(Some(node(ActionKind::Optional {
                    actor,
                    action: Box::new(action),
                })));
            }
        }
        Ok(None)
    }

    fn parse_conditional(
        &mut self,
        source: &str,
    ) -> Result<Option<ActionNode>, OracleActionRejection> {
        if !source.starts_with("If ") {
            return Ok(None);
        }
        let comma = find_top_level(source, ", ").ok_or(OracleActionRejection::UnconsumedSource)?;
        let condition_source = &source[3..comma];
        let consequence_source = &source[comma + 2..];
        let predicate = parse_state_predicate(condition_source)?;

        let (if_true_source, if_false_source) =
            if let Some(otherwise) = find_top_level(consequence_source, ". Otherwise, ") {
                (
                    &consequence_source[..otherwise],
                    Some(&consequence_source[otherwise + ". Otherwise, ".len()..]),
                )
            } else if let Some(otherwise) = find_top_level(consequence_source, " Otherwise, ") {
                (
                    &consequence_source[..otherwise],
                    Some(&consequence_source[otherwise + " Otherwise, ".len()..]),
                )
            } else if let Some(otherwise) = find_top_level(consequence_source, "; otherwise, ") {
                (
                    &consequence_source[..otherwise],
                    Some(&consequence_source[otherwise + "; otherwise, ".len()..]),
                )
            } else {
                (consequence_source, None)
            };
        let if_true = self.parse_node(&sentence_case_action(if_true_source))?;
        let if_false = if_false_source
            .map(|body| self.parse_node(&sentence_case_action(body)).map(Box::new))
            .transpose()?;
        Ok(Some(node(ActionKind::Conditional {
            predicate,
            if_true: Box::new(if_true),
            if_false,
        })))
    }

    fn parse_ordered_sequence(
        &mut self,
        source: &str,
    ) -> Result<Option<ActionNode>, OracleActionRejection> {
        for (literal, separator) in [
            (", then ", SequenceSeparator::Then),
            (". Then ", SequenceSeparator::Then),
            ("; then ", SequenceSeparator::Then),
            (". ", SequenceSeparator::Sentence),
        ] {
            let positions = find_all_top_level(source, literal);
            if positions.is_empty() {
                continue;
            }
            let parts = split_at_positions(source, literal, &positions);
            let mut parsed = Vec::with_capacity(parts.len());
            for (index, part) in parts.into_iter().enumerate() {
                let part = if index == 0 {
                    part.to_owned()
                } else {
                    sentence_case_action(part)
                };
                parsed.push(self.parse_node(&part)?);
            }
            return Ok(Some(flatten_sequence(parsed, separator)));
        }

        let and_positions = find_all_top_level(source, " and ");
        let mut successful = Vec::new();
        for position in and_positions {
            let left = &source[..position];
            let right = &source[position + " and ".len()..];
            let Ok(left_action) = self.parse_node(left) else {
                continue;
            };
            let right_action = self.parse_node(&sentence_case_action(right)).or_else(|_| {
                inherited_player_subject(source)
                    .ok_or(OracleActionRejection::UnsupportedAction)
                    .and_then(|subject| {
                        let mut action = self.parse_node(&format!("{subject} {right}"))?;
                        if matches!(subject, "Target player" | "Target opponent") {
                            rebind_inherited_target_player(&mut action);
                        }
                        Ok(action)
                    })
            });
            let Ok(right_action) = right_action else {
                continue;
            };
            successful.push(flatten_sequence(
                vec![left_action, right_action],
                SequenceSeparator::And,
            ));
        }
        match successful.len() {
            0 => Ok(None),
            1 => Ok(successful.pop()),
            _ => Err(OracleActionRejection::AmbiguousComposition),
        }
    }
}

fn node(kind: ActionKind) -> ActionNode {
    ActionNode {
        id: ActionId(0),
        kind,
    }
}

fn flatten_sequence(actions: Vec<ActionNode>, separator: SequenceSeparator) -> ActionNode {
    let mut flat_actions = Vec::new();
    let mut separators = Vec::new();
    for action in actions {
        match action.kind {
            ActionKind::OrderedSequence {
                actions: nested,
                separators: nested_separators,
            } if nested_separators.iter().all(|nested| *nested == separator) => {
                if !flat_actions.is_empty() && !nested.is_empty() {
                    separators.push(separator);
                }
                flat_actions.extend(nested);
                separators.extend(nested_separators);
            }
            _ => {
                if !flat_actions.is_empty() {
                    separators.push(separator);
                }
                flat_actions.push(action);
            }
        }
    }
    node(ActionKind::OrderedSequence {
        actions: flat_actions,
        separators,
    })
}

fn assign_action_ids(node: &mut ActionNode, next: &mut u32) {
    node.id = ActionId(*next);
    *next = next.saturating_add(1);
    assign_object_target_slots(&mut node.kind);
    match &mut node.kind {
        ActionKind::Optional { action, .. } => assign_action_ids(action, next),
        ActionKind::Conditional {
            if_true, if_false, ..
        } => {
            assign_action_ids(if_true, next);
            if let Some(if_false) = if_false {
                assign_action_ids(if_false, next);
            }
        }
        ActionKind::OrderedSequence { actions, .. } => {
            for action in actions {
                assign_action_ids(action, next);
            }
        }
        _ => {}
    }
}

fn assign_object_target_slots(kind: &mut ActionKind) {
    fn assign(operand: &mut ObjectOperand, next_slot: &mut u8) {
        if let ObjectOperand::Target { slot, .. } = operand {
            *slot = *next_slot;
            *next_slot = next_slot.saturating_add(1);
        }
    }

    let mut next_slot = 0u8;
    match kind {
        ActionKind::DealDamage {
            source, recipient, ..
        } => {
            if let DamageSource::Object(source) = source {
                assign(source, &mut next_slot);
            }
            if let DamageRecipient::Object(recipient) = recipient {
                assign(recipient, &mut next_slot);
            }
        }
        ActionKind::Fight { first, second } => {
            assign(first, &mut next_slot);
            assign(second, &mut next_slot);
        }
        ActionKind::Tap { objects }
        | ActionKind::Untap { objects }
        | ActionKind::ChangeCounters { objects, .. }
        | ActionKind::ModifyPowerToughness { objects, .. }
        | ActionKind::ChangeKeywords { objects, .. }
        | ActionKind::MoveZone { objects, .. } => assign(objects, &mut next_slot),
        ActionKind::Sacrifice {
            selection: SacrificeSelection::Objects(objects),
            ..
        } => assign(objects, &mut next_slot),
        ActionKind::CreateCopyToken { source, .. } => assign(source, &mut next_slot),
        _ => {}
    }
}

fn rebind_implicit_you(node: &mut ActionNode, actor: PlayerOperand) {
    fn rebind_player(player: &mut PlayerOperand, actor: PlayerOperand) {
        if *player == PlayerOperand::You {
            *player = actor;
        }
    }

    fn rebind_zone(zone: &mut ZoneOperand, actor: PlayerOperand) {
        if zone.owner == ZoneOwner::You {
            zone.owner = match actor {
                PlayerOperand::TargetPlayer | PlayerOperand::TargetOpponent => {
                    ZoneOwner::TargetPlayer
                }
                PlayerOperand::ThatPlayer => ZoneOwner::ThatPlayer,
                _ => ZoneOwner::You,
            };
        }
    }

    match &mut node.kind {
        ActionKind::Draw { player, .. }
        | ActionKind::GainLife { player, .. }
        | ActionKind::LoseLife { player, .. }
        | ActionKind::Mill { player, .. }
        | ActionKind::ShuffleLibrary { player } => rebind_player(player, actor),
        ActionKind::Sacrifice { player, .. } => rebind_player(player, actor),
        ActionKind::Discard { player, selection }
        | ActionKind::Reveal { player, selection }
        | ActionKind::Look { player, selection } => {
            rebind_player(player, actor);
            rebind_zone(&mut selection.zone, actor);
        }
        ActionKind::CreateToken { player, .. } | ActionKind::CreateCopyToken { player, .. } => {
            rebind_player(player, actor)
        }
        ActionKind::SearchLibrary {
            player,
            library_owner,
            selection,
            destination,
            ..
        } => {
            rebind_player(player, actor);
            if *library_owner == ZoneOwner::You {
                *library_owner = match actor {
                    PlayerOperand::TargetPlayer | PlayerOperand::TargetOpponent => {
                        ZoneOwner::TargetPlayer
                    }
                    PlayerOperand::ThatPlayer => ZoneOwner::ThatPlayer,
                    _ => ZoneOwner::You,
                };
            }
            rebind_zone(&mut selection.zone, actor);
            rebind_zone(destination, actor);
        }
        ActionKind::MoveCards {
            selection,
            destination,
            ..
        } => {
            rebind_zone(&mut selection.zone, actor);
            rebind_zone(destination, actor);
        }
        ActionKind::DealDamage {
            recipient: DamageRecipient::Player(player),
            ..
        } => {
            rebind_player(player, actor);
        }
        ActionKind::Optional {
            actor: nested_actor,
            action,
        } => {
            rebind_player(nested_actor, actor);
            rebind_implicit_you(action, actor);
        }
        ActionKind::Conditional {
            if_true, if_false, ..
        } => {
            rebind_implicit_you(if_true, actor);
            if let Some(if_false) = if_false {
                rebind_implicit_you(if_false, actor);
            }
        }
        ActionKind::OrderedSequence { actions, .. } => {
            for action in actions {
                rebind_implicit_you(action, actor);
            }
        }
        _ => {}
    }
}

fn parse_atomic_action(source: &str) -> Option<ActionNode> {
    parse_draw(source)
        .or_else(|| parse_discard(source))
        .or_else(|| parse_sacrifice(source))
        .or_else(|| parse_life(source))
        .or_else(|| parse_damage(source))
        .or_else(|| parse_fight(source))
        .or_else(|| parse_tap_untap(source))
        .or_else(|| parse_counters(source))
        .or_else(|| parse_power_toughness(source))
        .or_else(|| parse_keyword_change(source))
        .or_else(|| parse_copy_token(source))
        .or_else(|| parse_token(source))
        .or_else(|| parse_search(source))
        .or_else(|| parse_reveal(source))
        .or_else(|| parse_look(source))
        .or_else(|| parse_mill(source))
        .or_else(|| parse_shuffle(source))
        .or_else(|| parse_zone_movement(source))
        .map(node)
}

fn parse_draw(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?:(?P<subject>You|Target player|Target opponent|Each player|Each opponent|That player|Chosen player) )?(?P<verb>Draw|draw|draws) (?P<amount>[A-Za-z0-9]+(?: many)?) cards?$",
            )
            .expect("draw pattern")
        })
        .captures(source)?;
    let player = parse_player_and_verb(
        captures.name("subject").map(|value| value.as_str()),
        captures.name("verb")?.as_str(),
        "Draw",
        "draw",
        "draws",
    )?;
    let amount = parse_amount(captures.name("amount")?.as_str())?;
    Some(ActionKind::Draw { player, amount })
}

fn parse_discard(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?:(?P<subject>You|Target player|Target opponent|Each player|Each opponent|That player|Chosen player) )?(?P<verb>Discard|discard|discards) (?P<body>.+)$",
            )
            .expect("discard pattern")
        })
        .captures(source)?;
    let player = parse_player_and_verb(
        captures.name("subject").map(|value| value.as_str()),
        captures.name("verb")?.as_str(),
        "Discard",
        "discard",
        "discards",
    )?;
    let body = captures.name("body")?.as_str();
    let zone = ZoneOperand {
        owner: zone_owner_for_player(player)?,
        zone: Zone::Hand,
    };
    let (cardinality, random) = match body {
        "your hand" | "their hand" | "that player's hand" => (Cardinality::All, false),
        "a card" | "one card" => (Cardinality::ExactlyOne, false),
        "a card at random" | "one card at random" => (Cardinality::ExactlyOne, true),
        _ => {
            let (amount_source, suffix) = body.rsplit_once(' ')?;
            if suffix != "cards" {
                return None;
            }
            match parse_amount(amount_source)? {
                Amount::Fixed(amount) => (Cardinality::Exactly(amount), false),
                Amount::Variable(_) => return None,
            }
        }
    };
    Some(ActionKind::Discard {
        player,
        selection: CardSelection {
            zone,
            cardinality,
            filter: ObjectFilter::card(),
            from_top: false,
            random,
        },
    })
}

fn parse_sacrifice(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?:(?P<subject>You|Target player|Target opponent|Each player|Each opponent|That player|Chosen player) )?(?P<verb>Sacrifice|sacrifice|sacrifices) (?P<body>.+)$",
            )
            .expect("sacrifice pattern")
        })
        .captures(source)?;
    let player = parse_player_and_verb(
        captures.name("subject").map(|value| value.as_str()),
        captures.name("verb")?.as_str(),
        "Sacrifice",
        "sacrifice",
        "sacrifices",
    )?;
    let body = captures.name("body")?.as_str();
    if let Some(objects) = parse_object_operand(body)
        && matches!(objects, ObjectOperand::Source | ObjectOperand::Set { .. })
    {
        return Some(ActionKind::Sacrifice {
            player,
            selection: SacrificeSelection::Objects(objects),
        });
    }

    let choice_source = body
        .strip_suffix(" of their choice")
        .or_else(|| body.strip_suffix(" they control"))
        .or_else(|| body.strip_suffix(" you control"))
        .unwrap_or(body);
    let (cardinality, filter) = parse_search_selection(choice_source)?;
    Some(ActionKind::Sacrifice {
        player,
        selection: SacrificeSelection::Choice {
            cardinality,
            filter,
        },
    })
}

fn parse_life(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?:(?P<subject>You|Target player|Target opponent|Each player|Each opponent|That player|Chosen player) )?(?P<verb>Gain|gain|gains|Lose|lose|loses) (?P<amount>[A-Za-z0-9]+(?: many)?) life$",
            )
            .expect("life pattern")
        })
        .captures(source)?;
    let verb = captures.name("verb")?.as_str();
    let gain = matches!(verb, "Gain" | "gain" | "gains");
    let player = parse_player_and_verb(
        captures.name("subject").map(|value| value.as_str()),
        verb,
        if gain { "Gain" } else { "Lose" },
        if gain { "gain" } else { "lose" },
        if gain { "gains" } else { "loses" },
    )?;
    let amount = parse_amount(captures.name("amount")?.as_str())?;
    Some(if gain {
        ActionKind::GainLife { player, amount }
    } else {
        ActionKind::LoseLife { player, amount }
    })
}

fn parse_damage(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?P<source>This spell|This creature|This permanent|It|Target creature) deals (?P<amount>[A-Za-z0-9]+(?: many)?) damage to (?P<recipient>any target|target player|target opponent|target creature|target permanent|each opponent|each player|that player|that creature|it)$",
            )
            .expect("damage pattern")
        })
        .captures(source)?;
    let source = match captures.name("source")?.as_str() {
        "This spell" => DamageSource::ThisSpell,
        "This creature" | "This permanent" => DamageSource::SourceObject,
        "It" => DamageSource::Object(ObjectOperand::It),
        "Target creature" => DamageSource::Object(parse_object_operand("target creature")?),
        _ => return None,
    };
    let recipient = match captures.name("recipient")?.as_str() {
        "any target" => DamageRecipient::AnyTarget,
        "target player" => DamageRecipient::Player(PlayerOperand::TargetPlayer),
        "target opponent" => DamageRecipient::Player(PlayerOperand::TargetOpponent),
        "each opponent" => DamageRecipient::Player(PlayerOperand::EachOpponent),
        "each player" => DamageRecipient::Player(PlayerOperand::EachPlayer),
        "that player" => DamageRecipient::Player(PlayerOperand::ThatPlayer),
        "target creature" => DamageRecipient::Object(parse_object_operand("target creature")?),
        "target permanent" => DamageRecipient::Object(parse_object_operand("target permanent")?),
        "that creature" => DamageRecipient::Object(ObjectOperand::ThatObject),
        "it" => DamageRecipient::Object(ObjectOperand::It),
        _ => return None,
    };
    Some(ActionKind::DealDamage {
        source,
        recipient,
        amount: parse_amount(captures.name("amount")?.as_str())?,
    })
}

fn parse_fight(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?P<first>This creature|Target creature(?: you control)?) fights (?P<second>another target creature|target creature(?: an opponent controls)?|that creature)$",
            )
            .expect("fight pattern")
        })
        .captures(source)?;
    Some(ActionKind::Fight {
        first: parse_object_operand(captures.name("first")?.as_str())?,
        second: parse_object_operand(captures.name("second")?.as_str())?,
    })
}

fn parse_tap_untap(source: &str) -> Option<ActionKind> {
    let (operation, operand_source) = if let Some(operand) = source.strip_prefix("Tap ") {
        (true, operand)
    } else {
        let operand = source.strip_prefix("Untap ")?;
        (false, operand)
    };
    if operand_source.contains(" or ") {
        return None;
    }
    let objects = parse_object_operand(operand_source)?;
    Some(if operation {
        ActionKind::Tap { objects }
    } else {
        ActionKind::Untap { objects }
    })
}

fn parse_counters(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?P<operation>Put|Remove) (?P<amount>a|an|one|two|three|four|five|six|seven|eight|nine|ten|[1-9][0-9]*|X|that many) (?P<counter>[A-Za-z0-9+/' -]+?) counters? (?P<preposition>on|from) (?P<object>.+)$",
            )
            .expect("counter pattern")
        })
        .captures(source)?;
    let operation = match captures.name("operation")?.as_str() {
        "Put" => CounterOperation::Put,
        "Remove" => CounterOperation::Remove,
        _ => return None,
    };
    if (operation == CounterOperation::Put && captures.name("preposition")?.as_str() != "on")
        || (operation == CounterOperation::Remove
            && captures.name("preposition")?.as_str() != "from")
    {
        return None;
    }
    let counter = captures.name("counter")?.as_str().trim();
    if counter.is_empty()
        || counter.contains(" counter")
        || counter.contains(" and ")
        || !counter
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "+-/' ".contains(character))
    {
        return None;
    }
    Some(ActionKind::ChangeCounters {
        operation,
        objects: parse_object_operand(captures.name("object")?.as_str())?,
        counter: counter.to_ascii_lowercase(),
        amount: parse_amount(captures.name("amount")?.as_str())?,
    })
}

fn parse_power_toughness(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?P<object>.+?) gets (?P<power>[+-][0-9]+)/(?P<toughness>[+-][0-9]+) (?P<duration>until end of turn|this turn|until your next turn)$",
            )
            .expect("power toughness pattern")
        })
        .captures(source)?;
    Some(ActionKind::ModifyPowerToughness {
        objects: parse_object_operand(captures.name("object")?.as_str())?,
        power: captures.name("power")?.as_str().parse().ok()?,
        toughness: captures.name("toughness")?.as_str().parse().ok()?,
        duration: parse_duration(captures.name("duration")?.as_str())?,
    })
}

fn parse_keyword_change(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?P<object>.+?) (?P<operation>gains|loses) (?P<keywords>[A-Za-z ]+(?:, [A-Za-z ]+)*(?:,? and [A-Za-z ]+)?) (?P<duration>until end of turn|this turn|until your next turn)$",
            )
            .expect("keyword change pattern")
        })
        .captures(source)?;
    let keywords = parse_keyword_list(captures.name("keywords")?.as_str())?;
    Some(ActionKind::ChangeKeywords {
        operation: match captures.name("operation")?.as_str() {
            "gains" => KeywordOperation::Grant,
            "loses" => KeywordOperation::Lose,
            _ => return None,
        },
        objects: parse_object_operand(captures.name("object")?.as_str())?,
        keywords,
        duration: parse_duration(captures.name("duration")?.as_str())?,
    })
}

fn parse_copy_token(source: &str) -> Option<ActionKind> {
    let (player, body) = if let Some(body) = source.strip_prefix("Create ") {
        (PlayerOperand::You, body)
    } else if let Some(body) = source.strip_prefix("You create ") {
        (PlayerOperand::You, body)
    } else {
        let body = source.strip_prefix("Target player creates ")?;
        (PlayerOperand::TargetPlayer, body)
    };
    let (tapped, source_operand) = if let Some(source) = body
        .strip_prefix("a tapped token that's a copy of ")
        .or_else(|| body.strip_prefix("one tapped token that's a copy of "))
    {
        (true, source)
    } else {
        let source = body
            .strip_prefix("a token that's a copy of ")
            .or_else(|| body.strip_prefix("one token that's a copy of "))?;
        (false, source)
    };
    Some(ActionKind::CreateCopyToken {
        player,
        source: parse_object_operand(source_operand)?,
        tapped,
    })
}

fn parse_token(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?:(?P<subject>You|Target player|Each player) )?(?P<verb>Create|create|creates) (?P<amount>a|an|one|two|three|four|five|six|seven|eight|nine|ten|[1-9][0-9]*|X) (?P<body>.+?) tokens?$",
            )
            .expect("token pattern")
        })
        .captures(source)?;
    let player = parse_player_and_verb(
        captures.name("subject").map(|value| value.as_str()),
        captures.name("verb")?.as_str(),
        "Create",
        "create",
        "creates",
    )?;
    let amount = parse_amount(captures.name("amount")?.as_str())?;
    let template = parse_token_template(captures.name("body")?.as_str())?;
    Some(ActionKind::CreateToken {
        player,
        amount,
        template,
    })
}

fn parse_search(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?:(?P<subject>You|Target player) )?(?P<verb>Search|search|searches) (?P<owner>your|their|that player's) library for (?P<selection>.+?)(?:, reveal (?P<reveal>it|them|that card|those cards))?, put (?P<put>it|them|that card|those cards) (?:(?P<tapped_before>tapped) )?(?P<destination>into your hand|into their hand|onto the battlefield|on top of its owner's library|on the bottom of its owner's library)(?: (?P<tapped_after>tapped))?, then shuffle(?: that library| your library| their library)?$",
            )
            .expect("search pattern")
        })
        .captures(source)?;
    let player = parse_player_and_verb(
        captures.name("subject").map(|value| value.as_str()),
        captures.name("verb")?.as_str(),
        "Search",
        "search",
        "searches",
    )?;
    let library_owner = match captures.name("owner")?.as_str() {
        "your" => ZoneOwner::You,
        "their" => zone_owner_for_player(player)?,
        "that player's" => ZoneOwner::ThatPlayer,
        _ => return None,
    };
    let (cardinality, filter) = parse_search_selection(captures.name("selection")?.as_str())?;
    let singular_selection = matches!(
        cardinality,
        Cardinality::ExactlyOne | Cardinality::Exactly(1) | Cardinality::UpTo(1)
    );
    let pronoun_matches = |pronoun: &str| {
        if singular_selection {
            matches!(pronoun, "it" | "that card")
        } else {
            matches!(pronoun, "them" | "those cards")
        }
    };
    if !pronoun_matches(captures.name("put")?.as_str())
        || captures
            .name("reveal")
            .is_some_and(|pronoun| !pronoun_matches(pronoun.as_str()))
    {
        return None;
    }
    let (destination, position) =
        parse_destination(captures.name("destination")?.as_str(), player)?;
    let tapped =
        captures.name("tapped_before").is_some() || captures.name("tapped_after").is_some();
    if tapped && destination.zone != Zone::Battlefield {
        return None;
    }
    Some(ActionKind::SearchLibrary {
        player,
        library_owner,
        selection: CardSelection {
            zone: ZoneOperand {
                owner: library_owner,
                zone: Zone::Library,
            },
            cardinality,
            filter,
            from_top: false,
            random: false,
        },
        destination,
        position,
        reveal: captures.name("reveal").is_some(),
        tapped,
        shuffle_after: true,
    })
}

fn parse_reveal(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?:(?P<subject>You|Target player|Each player) )?(?P<verb>Reveal|reveal|reveals) (?P<body>your hand|their hand|the top card of your library|the top card of their library|the top [A-Za-z0-9]+ cards? of your library|the top [A-Za-z0-9]+ cards? of their library)$",
            )
            .expect("reveal pattern")
        })
        .captures(source)?;
    let player = parse_player_and_verb(
        captures.name("subject").map(|value| value.as_str()),
        captures.name("verb")?.as_str(),
        "Reveal",
        "reveal",
        "reveals",
    )?;
    let selection = parse_visibility_selection(captures.name("body")?.as_str(), player)?;
    Some(ActionKind::Reveal { player, selection })
}

fn parse_look(source: &str) -> Option<ActionKind> {
    if source == "Look at the top card of your library" {
        return Some(ActionKind::Look {
            player: PlayerOperand::You,
            selection: CardSelection {
                zone: ZoneOperand {
                    owner: ZoneOwner::You,
                    zone: Zone::Library,
                },
                cardinality: Cardinality::ExactlyOne,
                filter: ObjectFilter::card(),
                from_top: true,
                random: false,
            },
        });
    }
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?:(?P<subject>You|Target player) )?(?P<verb>Look|look|looks) at the top (?P<amount>[A-Za-z0-9]+) cards? of (?P<owner>your|their) library$",
            )
            .expect("look pattern")
        })
        .captures(source)?;
    let player = parse_player_and_verb(
        captures.name("subject").map(|value| value.as_str()),
        captures.name("verb")?.as_str(),
        "Look",
        "look",
        "looks",
    )?;
    let amount = match parse_amount(captures.name("amount")?.as_str())? {
        Amount::Fixed(amount) => amount,
        Amount::Variable(_) => return None,
    };
    let owner = match captures.name("owner")?.as_str() {
        "your" => ZoneOwner::You,
        "their" => zone_owner_for_player(player)?,
        _ => return None,
    };
    Some(ActionKind::Look {
        player,
        selection: CardSelection {
            zone: ZoneOperand {
                owner,
                zone: Zone::Library,
            },
            cardinality: Cardinality::Exactly(amount),
            filter: ObjectFilter::card(),
            from_top: true,
            random: false,
        },
    })
}

fn parse_mill(source: &str) -> Option<ActionKind> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let captures = PATTERN
        .get_or_init(|| {
            Regex::new(
                r"^(?:(?P<subject>You|Target player|Target opponent|Each player|Each opponent|That player) )?(?P<verb>Mill|mill|mills) (?P<amount>[A-Za-z0-9]+(?: many)?) cards?$",
            )
            .expect("mill pattern")
        })
        .captures(source)?;
    let player = parse_player_and_verb(
        captures.name("subject").map(|value| value.as_str()),
        captures.name("verb")?.as_str(),
        "Mill",
        "mill",
        "mills",
    )?;
    Some(ActionKind::Mill {
        player,
        amount: parse_amount(captures.name("amount")?.as_str())?,
    })
}

fn parse_shuffle(source: &str) -> Option<ActionKind> {
    let player = match source {
        "Shuffle your library" => PlayerOperand::You,
        "Target player shuffles their library" => PlayerOperand::TargetPlayer,
        "Target opponent shuffles their library" => PlayerOperand::TargetOpponent,
        "That player shuffles their library" => PlayerOperand::ThatPlayer,
        "Each player shuffles their library" => PlayerOperand::EachPlayer,
        _ => return None,
    };
    Some(ActionKind::ShuffleLibrary { player })
}

fn parse_zone_movement(source: &str) -> Option<ActionKind> {
    if let Some(body) = source.strip_prefix("Return ") {
        let (object_source, destination_source) = body.rsplit_once(" to ")?;
        if let Some(objects) = parse_object_operand(object_source) {
            let destination = match destination_source {
                "its owner's hand" | "their owners' hands" => ZoneOperand {
                    owner: ZoneOwner::ObjectOwner,
                    zone: Zone::Hand,
                },
                "its controller's hand" => ZoneOperand {
                    owner: ZoneOwner::ObjectController,
                    zone: Zone::Hand,
                },
                _ => return None,
            };
            return Some(ActionKind::MoveZone {
                objects,
                from: Some(Zone::Battlefield),
                destination,
                position: None,
                tapped: false,
            });
        }
        let (mut selection, from) = parse_zoned_card_selection(object_source)?;
        let (destination, position) = parse_destination(destination_source, PlayerOperand::You)?;
        selection.zone.zone = from;
        return Some(ActionKind::MoveCards {
            selection,
            destination,
            position,
            tapped: false,
        });
    }
    if let Some(object_source) = source.strip_prefix("Exile ") {
        return Some(ActionKind::MoveZone {
            objects: parse_object_operand(object_source)?,
            from: None,
            destination: ZoneOperand {
                owner: ZoneOwner::ObjectOwner,
                zone: Zone::Exile,
            },
            position: None,
            tapped: false,
        });
    }
    if let Some(rest) = source.strip_prefix("Put ") {
        for (suffix, destination, position) in [
            (
                " on top of its owner's library",
                ZoneOperand {
                    owner: ZoneOwner::ObjectOwner,
                    zone: Zone::Library,
                },
                Some(LibraryPosition::Top),
            ),
            (
                " on the bottom of its owner's library",
                ZoneOperand {
                    owner: ZoneOwner::ObjectOwner,
                    zone: Zone::Library,
                },
                Some(LibraryPosition::Bottom),
            ),
            (
                " into its owner's hand",
                ZoneOperand {
                    owner: ZoneOwner::ObjectOwner,
                    zone: Zone::Hand,
                },
                None,
            ),
            (
                " into its owner's graveyard",
                ZoneOperand {
                    owner: ZoneOwner::ObjectOwner,
                    zone: Zone::Graveyard,
                },
                None,
            ),
        ] {
            if let Some(object_source) = rest.strip_suffix(suffix) {
                return Some(ActionKind::MoveZone {
                    objects: parse_object_operand(object_source)?,
                    from: Some(Zone::Battlefield),
                    destination,
                    position,
                    tapped: false,
                });
            }
        }
        let (selection_source, destination_source) = if let Some(parts) = rest.split_once(" into ")
        {
            parts
        } else if let Some(parts) = rest.split_once(" onto ") {
            parts
        } else if let Some(parts) = rest.split_once(" on top of ") {
            parts
        } else {
            rest.split_once(" on the bottom of ")?
        };
        let (selection, from) = parse_zoned_card_selection(selection_source)?;
        let destination_phrase = if rest.contains(" on top of ") {
            format!("on top of {destination_source}")
        } else if rest.contains(" on the bottom of ") {
            format!("on the bottom of {destination_source}")
        } else if rest.contains(" onto ") {
            format!("onto {destination_source}")
        } else {
            format!("into {destination_source}")
        };
        let (destination, position) = parse_destination(&destination_phrase, PlayerOperand::You)?;
        let mut selection = selection;
        selection.zone.zone = from;
        return Some(ActionKind::MoveCards {
            selection,
            destination,
            position,
            tapped: destination_phrase.contains(" tapped"),
        });
    }
    None
}

fn parse_player_and_verb(
    subject: Option<&str>,
    verb: &str,
    imperative: &str,
    plural: &str,
    singular: &str,
) -> Option<PlayerOperand> {
    match subject {
        None if verb == imperative => Some(PlayerOperand::You),
        Some(subject) => {
            let player = parse_player_operand(subject)?;
            let needs_plural = player == PlayerOperand::You;
            ((needs_plural && verb == plural) || (!needs_plural && verb == singular))
                .then_some(player)
        }
        _ => None,
    }
}

fn parse_player_operand(source: &str) -> Option<PlayerOperand> {
    match source {
        "You" | "you" => Some(PlayerOperand::You),
        "Target player" | "target player" => Some(PlayerOperand::TargetPlayer),
        "Target opponent" | "target opponent" => Some(PlayerOperand::TargetOpponent),
        "Each player" | "each player" => Some(PlayerOperand::EachPlayer),
        "Each opponent" | "each opponent" => Some(PlayerOperand::EachOpponent),
        "That player" | "that player" => Some(PlayerOperand::ThatPlayer),
        "Chosen player" | "chosen player" => Some(PlayerOperand::ChosenPlayer),
        _ => None,
    }
}

fn zone_owner_for_player(player: PlayerOperand) -> Option<ZoneOwner> {
    match player {
        PlayerOperand::You => Some(ZoneOwner::You),
        PlayerOperand::TargetPlayer | PlayerOperand::TargetOpponent => {
            Some(ZoneOwner::TargetPlayer)
        }
        PlayerOperand::ThatPlayer => Some(ZoneOwner::ThatPlayer),
        _ => None,
    }
}

fn parse_amount(source: &str) -> Option<Amount> {
    let lower = source.trim().to_ascii_lowercase();
    match lower.as_str() {
        "a" | "an" | "one" => Some(Amount::Fixed(1)),
        "two" => Some(Amount::Fixed(2)),
        "three" => Some(Amount::Fixed(3)),
        "four" => Some(Amount::Fixed(4)),
        "five" => Some(Amount::Fixed(5)),
        "six" => Some(Amount::Fixed(6)),
        "seven" => Some(Amount::Fixed(7)),
        "eight" => Some(Amount::Fixed(8)),
        "nine" => Some(Amount::Fixed(9)),
        "ten" => Some(Amount::Fixed(10)),
        "eleven" => Some(Amount::Fixed(11)),
        "twelve" => Some(Amount::Fixed(12)),
        "thirteen" => Some(Amount::Fixed(13)),
        "fourteen" => Some(Amount::Fixed(14)),
        "fifteen" => Some(Amount::Fixed(15)),
        "sixteen" => Some(Amount::Fixed(16)),
        "seventeen" => Some(Amount::Fixed(17)),
        "eighteen" => Some(Amount::Fixed(18)),
        "nineteen" => Some(Amount::Fixed(19)),
        "twenty" => Some(Amount::Fixed(20)),
        "x" => Some(Amount::Variable(VariableAmount::X)),
        "that many" => Some(Amount::Variable(VariableAmount::ThatMany)),
        _ => lower.parse::<u32>().ok().and_then(Amount::fixed_positive),
    }
}

fn parse_duration(source: &str) -> Option<Duration> {
    match source {
        "until end of turn" => Some(Duration::UntilEndOfTurn),
        "this turn" => Some(Duration::ThisTurn),
        "until your next turn" => Some(Duration::UntilYourNextTurn),
        _ => None,
    }
}

fn parse_keyword_list(source: &str) -> Option<BTreeSet<KeywordAbility>> {
    let replaced = source.replace(", and ", ", ").replace(" and ", ", ");
    let keywords = replaced
        .split(", ")
        .map(KeywordAbility::parse)
        .collect::<Option<BTreeSet<_>>>()?;
    (!keywords.is_empty()).then_some(keywords)
}

fn parse_object_operand(source: &str) -> Option<ObjectOperand> {
    let source = source.trim();
    let lower = source.to_ascii_lowercase();
    match lower.as_str() {
        "this object" | "this permanent" | "this creature" | "this artifact" => {
            Some(ObjectOperand::Source)
        }
        "it" => Some(ObjectOperand::It),
        "that object" | "that permanent" | "that creature" => Some(ObjectOperand::ThatObject),
        "that card" => Some(ObjectOperand::PreviousSelection),
        "enchanted creature" | "enchanted permanent" => Some(ObjectOperand::EnchantedObject),
        "equipped creature" => Some(ObjectOperand::EquippedObject),
        _ => {
            let (cardinality, filter_source, is_target, other_than_source) =
                if let Some((cardinality, filter_source)) = parse_target_cardinality_prefix(source)
                {
                    (cardinality, filter_source, true, false)
                } else if lower.starts_with("another target ") {
                    (
                        Cardinality::ExactlyOne,
                        &source["another target ".len()..],
                        true,
                        true,
                    )
                } else if lower.starts_with("target ") {
                    (
                        Cardinality::ExactlyOne,
                        &source["target ".len()..],
                        true,
                        false,
                    )
                } else if lower.starts_with("each ") {
                    (Cardinality::All, &source["each ".len()..], false, false)
                } else if lower.starts_with("all ") {
                    (Cardinality::All, &source["all ".len()..], false, false)
                } else {
                    return None;
                };
            let mut filter = parse_object_filter(filter_source)?;
            filter.other_than_source = other_than_source;
            Some(if is_target {
                ObjectOperand::Target {
                    slot: 0,
                    cardinality,
                    filter,
                }
            } else {
                ObjectOperand::Set {
                    cardinality,
                    filter,
                }
            })
        }
    }
}

fn parse_object_filter(source: &str) -> Option<ObjectFilter> {
    let mut original = source.trim().to_owned();
    let mut source = original.to_ascii_lowercase();
    let mut filter = ObjectFilter::permanent();
    if let Some(rest) = source.strip_suffix(" you control") {
        filter.controller = ControllerConstraint::You;
        source = rest.to_owned();
        original.truncate(original.len() - " you control".len());
    } else if let Some(rest) = source.strip_suffix(" an opponent controls") {
        filter.controller = ControllerConstraint::Opponent;
        source = rest.to_owned();
        original.truncate(original.len() - " an opponent controls".len());
    }
    if let Some(rest) = source.strip_prefix("nontoken ") {
        filter.token = Some(false);
        source = rest.to_owned();
        original = original["nontoken ".len()..].to_owned();
    } else if let Some(rest) = source.strip_prefix("token ") {
        filter.token = Some(true);
        source = rest.to_owned();
        original = original["token ".len()..].to_owned();
    }
    if let Some(rest) = source.strip_prefix("nonland ") {
        filter.excluded_types.insert(CardType::Land);
        source = rest.to_owned();
        original = original["nonland ".len()..].to_owned();
    }
    if let Some(rest) = source.strip_prefix("noncreature ") {
        filter.excluded_types.insert(CardType::Creature);
        source = rest.to_owned();
        original = original["noncreature ".len()..].to_owned();
    }
    for (prefix, field) in [
        ("attacking ", 0u8),
        ("blocking ", 1u8),
        ("tapped ", 2u8),
        ("untapped ", 3u8),
    ] {
        if let Some(rest) = source.strip_prefix(prefix) {
            match field {
                0 => filter.attacking = Some(true),
                1 => filter.blocking = Some(true),
                2 => filter.tapped = Some(true),
                3 => filter.tapped = Some(false),
                _ => unreachable!(),
            }
            source = rest.to_owned();
            original = original[prefix.len()..].to_owned();
            break;
        }
    }
    for (prefix, color, excluded) in [
        ("white ", Color::White, false),
        ("blue ", Color::Blue, false),
        ("black ", Color::Black, false),
        ("red ", Color::Red, false),
        ("green ", Color::Green, false),
        ("nonwhite ", Color::White, true),
        ("nonblue ", Color::Blue, true),
        ("nonblack ", Color::Black, true),
        ("nonred ", Color::Red, true),
        ("nongreen ", Color::Green, true),
    ] {
        if let Some(rest) = source.strip_prefix(prefix) {
            if excluded {
                filter.excluded_colors.insert(color);
            } else {
                filter.required_colors.insert(color);
            }
            source = rest.to_owned();
            original = original[prefix.len()..].to_owned();
            break;
        }
    }

    match source.as_str() {
        "permanent" | "permanents" | "card" | "cards" => {}
        "creature" | "creatures" | "creature card" | "creature cards" => {
            filter.required_types.insert(CardType::Creature);
        }
        "artifact" | "artifacts" | "artifact card" | "artifact cards" => {
            filter.required_types.insert(CardType::Artifact);
        }
        "enchantment" | "enchantments" | "enchantment card" | "enchantment cards" => {
            filter.required_types.insert(CardType::Enchantment);
        }
        "land" | "lands" | "land card" | "land cards" => {
            filter.required_types.insert(CardType::Land);
        }
        "basic land card" | "basic land cards" => {
            filter.required_types.insert(CardType::Land);
            filter.required_supertypes.insert("basic".into());
        }
        "planeswalker" | "planeswalkers" | "planeswalker card" => {
            filter.required_types.insert(CardType::Planeswalker);
        }
        "artifact or enchantment"
        | "artifact or enchantment card"
        | "artifact or enchantment cards" => {
            filter.any_types = BTreeSet::from([CardType::Artifact, CardType::Enchantment]);
        }
        "artifact or creature" | "artifact or creature card" | "artifact or creature cards" => {
            filter.any_types = BTreeSet::from([CardType::Artifact, CardType::Creature]);
        }
        "instant or sorcery card" | "instant or sorcery cards" => {
            filter.any_types = BTreeSet::from([CardType::Instant, CardType::Sorcery]);
        }
        "permanent card" | "permanent cards" => {
            filter.excluded_types = BTreeSet::from([CardType::Instant, CardType::Sorcery]);
        }
        _ => {
            let (subtype, original_subtype) = [
                " creature cards",
                " creature card",
                " creatures",
                " creature",
            ]
            .into_iter()
            .find_map(|suffix| {
                source.strip_suffix(suffix).map(|subtype| {
                    (
                        subtype,
                        original
                            .get(..original.len().saturating_sub(suffix.len()))
                            .unwrap_or_default(),
                    )
                })
            })?;
            if subtype.is_empty()
                || !original_subtype.chars().next()?.is_uppercase()
                || !original_subtype
                    .chars()
                    .all(|character| character.is_ascii_alphabetic() || character == '-')
            {
                return None;
            }
            filter.required_types.insert(CardType::Creature);
            filter.required_subtypes.insert(subtype.to_owned());
        }
    }
    Some(filter)
}

fn parse_state_predicate(source: &str) -> Result<StatePredicate, OracleActionRejection> {
    let lower = source.trim().to_ascii_lowercase();
    if matches!(lower.as_str(), "you do" | "you did") {
        return Ok(StatePredicate::PreviousActionSucceeded);
    }
    if lower == "you have no cards in hand" || lower == "your hand is empty" {
        return Ok(StatePredicate::YourHandIsEmpty);
    }
    if let Some(object) = lower.strip_prefix("you control a ") {
        return Ok(StatePredicate::YouControl {
            filter: parse_object_filter(object).ok_or(OracleActionRejection::UnsupportedOperand)?,
            comparison: CountComparison::AtLeast(1),
        });
    }
    if let Some(object) = lower.strip_prefix("you control no ") {
        return Ok(StatePredicate::YouControl {
            filter: parse_object_filter(object).ok_or(OracleActionRejection::UnsupportedOperand)?,
            comparison: CountComparison::Exactly(0),
        });
    }
    Err(OracleActionRejection::UnsupportedOperand)
}

fn parse_token_template(source: &str) -> Option<TokenTemplate> {
    let mut source = source.trim();
    let mut tapped = false;
    let mut attacking = false;
    if let Some(rest) = source.strip_prefix("tapped and attacking ") {
        tapped = true;
        attacking = true;
        source = rest;
    } else if let Some(rest) = source.strip_prefix("tapped ") {
        tapped = true;
        source = rest;
    }

    let (core, keywords) = if let Some((core, keyword_source)) = source.rsplit_once(" with ") {
        (core, parse_keyword_list(keyword_source)?)
    } else {
        (source, BTreeSet::new())
    };

    static POWER_TOUGHNESS: OnceLock<Regex> = OnceLock::new();
    let (power, toughness, core) = if let Some(captures) = POWER_TOUGHNESS
        .get_or_init(|| {
            Regex::new(r"^(?P<power>-?[0-9]+)/(?P<toughness>-?[0-9]+) (?P<rest>.+)$")
                .expect("token power toughness pattern")
        })
        .captures(core)
    {
        (
            Some(captures.name("power")?.as_str().parse().ok()?),
            Some(captures.name("toughness")?.as_str().parse().ok()?),
            captures.name("rest")?.as_str(),
        )
    } else {
        (None, None, core)
    };

    let mut colors = BTreeSet::new();
    let mut saw_color_descriptor = false;
    let mut descriptor_words = core.split_whitespace().collect::<Vec<_>>();
    while let Some(word) = descriptor_words.first().copied() {
        let color = match word.to_ascii_lowercase().as_str() {
            "white" => Some(Color::White),
            "blue" => Some(Color::Blue),
            "black" => Some(Color::Black),
            "red" => Some(Color::Red),
            "green" => Some(Color::Green),
            "colorless" => Some(Color::Colorless),
            "and" => {
                descriptor_words.remove(0);
                continue;
            }
            _ => None,
        };
        let Some(color) = color else {
            break;
        };
        saw_color_descriptor = true;
        if color != Color::Colorless {
            colors.insert(color);
        }
        descriptor_words.remove(0);
    }
    if !saw_color_descriptor {
        return None;
    }

    let mut card_types = BTreeSet::new();
    let mut subtypes = BTreeSet::new();
    let mut subtype_words = Vec::new();
    for word in descriptor_words {
        let normalized = word.trim_matches(',').to_ascii_lowercase();
        let card_type = match normalized.as_str() {
            "artifact" => Some(CardType::Artifact),
            "battle" => Some(CardType::Battle),
            "creature" => Some(CardType::Creature),
            "enchantment" => Some(CardType::Enchantment),
            "land" => Some(CardType::Land),
            _ => None,
        };
        if let Some(card_type) = card_type {
            card_types.insert(card_type);
        } else if !normalized.is_empty()
            && normalized
                .chars()
                .all(|character| character.is_ascii_alphabetic() || character == '-')
        {
            subtype_words.push(normalized.clone());
            subtypes.insert(normalized);
        } else {
            return None;
        }
    }
    if card_types.is_empty()
        || card_types.contains(&CardType::Creature) != power.is_some()
        || power.is_some() != toughness.is_some()
    {
        return None;
    }
    Some(TokenTemplate {
        name: if subtype_words.is_empty() {
            "Token".to_owned()
        } else {
            subtype_words.join(" ")
        },
        power,
        toughness,
        colors,
        card_types,
        subtypes,
        keywords,
        tapped,
        attacking,
    })
}

fn parse_search_selection(source: &str) -> Option<(Cardinality, ObjectFilter)> {
    let source = source.trim();
    let (cardinality, filter_source) =
        if let Some((cardinality, filter_source)) = parse_target_cardinality_prefix(source) {
            (cardinality, filter_source)
        } else if let Some(rest) = source.strip_prefix("target ") {
            (Cardinality::ExactlyOne, rest)
        } else if let Some(rest) = source
            .strip_prefix("any number of target ")
            .or_else(|| source.strip_prefix("any number of "))
        {
            (Cardinality::AnyNumber, rest)
        } else if let Some(rest) = source.strip_prefix("all ") {
            (Cardinality::All, rest)
        } else if let Some(rest) = source.strip_prefix("up to ") {
            let (amount, filter) = rest.split_once(' ')?;
            let amount = match parse_amount(amount)? {
                Amount::Fixed(amount) => amount,
                Amount::Variable(_) => return None,
            };
            (Cardinality::UpTo(amount), filter)
        } else if let Some(rest) = source
            .strip_prefix("a ")
            .or_else(|| source.strip_prefix("an "))
            .or_else(|| source.strip_prefix("one "))
        {
            (Cardinality::ExactlyOne, rest)
        } else {
            let (amount, filter) = source.split_once(' ')?;
            let amount = match parse_amount(amount)? {
                Amount::Fixed(amount) => amount,
                Amount::Variable(_) => return None,
            };
            (Cardinality::Exactly(amount), filter)
        };
    Some((cardinality, parse_object_filter(filter_source)?))
}

fn parse_target_cardinality_prefix(source: &str) -> Option<(Cardinality, &str)> {
    let source = source.trim();
    if let Some(rest) = source.strip_prefix("any number of target ") {
        return Some((Cardinality::AnyNumber, rest));
    }
    if let Some(rest) = source.strip_prefix("up to ") {
        let (amount_source, target_and_filter) = rest.split_once(' ')?;
        let filter_source = target_and_filter.strip_prefix("target ")?;
        let amount = match parse_amount(amount_source)? {
            Amount::Fixed(amount) => amount,
            Amount::Variable(_) => return None,
        };
        return Some((Cardinality::UpTo(amount), filter_source));
    }
    let (amount_source, target_and_filter) = source.split_once(' ')?;
    let filter_source = target_and_filter.strip_prefix("target ")?;
    let amount = match parse_amount(amount_source)? {
        Amount::Fixed(amount) => amount,
        Amount::Variable(_) => return None,
    };
    Some((Cardinality::Exactly(amount), filter_source))
}

fn inherited_player_subject(source: &str) -> Option<&'static str> {
    [
        "Target opponent",
        "Target player",
        "Each opponent",
        "Each player",
        "That player",
        "Chosen player",
        "You",
    ]
    .into_iter()
    .find(|subject| source.starts_with(&format!("{subject} ")))
}

fn rebind_inherited_target_player(node: &mut ActionNode) {
    fn rebind_player(player: &mut PlayerOperand) {
        if matches!(
            *player,
            PlayerOperand::TargetPlayer | PlayerOperand::TargetOpponent
        ) {
            *player = PlayerOperand::ThatPlayer;
        }
    }

    fn rebind_zone(zone: &mut ZoneOperand) {
        if zone.owner == ZoneOwner::TargetPlayer {
            zone.owner = ZoneOwner::ThatPlayer;
        }
    }

    match &mut node.kind {
        ActionKind::Draw { player, .. }
        | ActionKind::GainLife { player, .. }
        | ActionKind::LoseLife { player, .. }
        | ActionKind::Mill { player, .. }
        | ActionKind::ShuffleLibrary { player }
        | ActionKind::Sacrifice { player, .. } => rebind_player(player),
        ActionKind::Discard { player, selection }
        | ActionKind::Reveal { player, selection }
        | ActionKind::Look { player, selection } => {
            rebind_player(player);
            rebind_zone(&mut selection.zone);
        }
        ActionKind::CreateToken { player, .. } | ActionKind::CreateCopyToken { player, .. } => {
            rebind_player(player);
        }
        ActionKind::SearchLibrary {
            player,
            library_owner,
            selection,
            destination,
            ..
        } => {
            rebind_player(player);
            if *library_owner == ZoneOwner::TargetPlayer {
                *library_owner = ZoneOwner::ThatPlayer;
            }
            rebind_zone(&mut selection.zone);
            rebind_zone(destination);
        }
        ActionKind::MoveCards {
            selection,
            destination,
            ..
        } => {
            rebind_zone(&mut selection.zone);
            rebind_zone(destination);
        }
        ActionKind::DealDamage {
            recipient: DamageRecipient::Player(player),
            ..
        } => {
            rebind_player(player);
        }
        ActionKind::Optional { actor, action } => {
            rebind_player(actor);
            rebind_inherited_target_player(action);
        }
        ActionKind::Conditional {
            if_true, if_false, ..
        } => {
            rebind_inherited_target_player(if_true);
            if let Some(if_false) = if_false {
                rebind_inherited_target_player(if_false);
            }
        }
        ActionKind::OrderedSequence { actions, .. } => {
            for action in actions {
                rebind_inherited_target_player(action);
            }
        }
        _ => {}
    }
}

fn parse_visibility_selection(source: &str, player: PlayerOperand) -> Option<CardSelection> {
    match source {
        "your hand" | "their hand" => Some(CardSelection {
            zone: ZoneOperand {
                owner: zone_owner_for_player(player)?,
                zone: Zone::Hand,
            },
            cardinality: Cardinality::All,
            filter: ObjectFilter::card(),
            from_top: false,
            random: false,
        }),
        _ => {
            if source == "the top card of your library" || source == "the top card of their library"
            {
                return Some(CardSelection {
                    zone: ZoneOperand {
                        owner: if source.contains("your library") {
                            ZoneOwner::You
                        } else {
                            zone_owner_for_player(player)?
                        },
                        zone: Zone::Library,
                    },
                    cardinality: Cardinality::ExactlyOne,
                    filter: ObjectFilter::card(),
                    from_top: true,
                    random: false,
                });
            }
            let (amount_source, owner) = source
                .strip_prefix("the top ")?
                .strip_suffix(" library")?
                .rsplit_once(" cards of ")
                .or_else(|| {
                    source
                        .strip_prefix("the top ")?
                        .strip_suffix(" library")?
                        .rsplit_once(" card of ")
                })?;
            let amount = match parse_amount(amount_source)? {
                Amount::Fixed(amount) => amount,
                Amount::Variable(_) => return None,
            };
            Some(CardSelection {
                zone: ZoneOperand {
                    owner: match owner {
                        "your" => ZoneOwner::You,
                        "their" => zone_owner_for_player(player)?,
                        _ => return None,
                    },
                    zone: Zone::Library,
                },
                cardinality: Cardinality::Exactly(amount),
                filter: ObjectFilter::card(),
                from_top: true,
                random: false,
            })
        }
    }
}

fn parse_zoned_card_selection(source: &str) -> Option<(CardSelection, Zone)> {
    let (card_source, zone_owner, zone) = if let Some(card) = source.strip_suffix(" from your hand")
    {
        (card, ZoneOwner::You, Zone::Hand)
    } else if let Some(card) = source.strip_suffix(" from your graveyard") {
        (card, ZoneOwner::You, Zone::Graveyard)
    } else {
        let card = source.strip_suffix(" from exile")?;
        (card, ZoneOwner::You, Zone::Exile)
    };
    let (cardinality, filter) = parse_search_selection(card_source)?;
    Some((
        CardSelection {
            zone: ZoneOperand {
                owner: zone_owner,
                zone,
            },
            cardinality,
            filter,
            from_top: false,
            random: false,
        },
        zone,
    ))
}

fn parse_destination(
    source: &str,
    player: PlayerOperand,
) -> Option<(ZoneOperand, Option<LibraryPosition>)> {
    let normalized = source.trim();
    let destination = match normalized {
        "into your hand" | "your hand" => (
            ZoneOperand {
                owner: ZoneOwner::You,
                zone: Zone::Hand,
            },
            None,
        ),
        "into their hand" | "their hand" => (
            ZoneOperand {
                owner: zone_owner_for_player(player)?,
                zone: Zone::Hand,
            },
            None,
        ),
        "onto the battlefield"
        | "onto the battlefield tapped"
        | "the battlefield"
        | "the battlefield tapped" => (
            ZoneOperand {
                owner: ZoneOwner::ObjectController,
                zone: Zone::Battlefield,
            },
            None,
        ),
        "on top of its owner's library" | "top of its owner's library" => (
            ZoneOperand {
                owner: ZoneOwner::ObjectOwner,
                zone: Zone::Library,
            },
            Some(LibraryPosition::Top),
        ),
        "on the bottom of its owner's library" | "the bottom of its owner's library" => (
            ZoneOperand {
                owner: ZoneOwner::ObjectOwner,
                zone: Zone::Library,
            },
            Some(LibraryPosition::Bottom),
        ),
        _ => return None,
    };
    Some(destination)
}

fn collapse_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source.trim() == source
        && !source.contains(['\r', '\n'])
        && collapse_whitespace(source) == source
}

fn strip_one_terminal_period(source: &str) -> Option<&str> {
    let source = source.strip_suffix('.')?;
    (!source.ends_with('.')).then_some(source)
}

fn sentence_case_action(source: &str) -> String {
    let mut characters = source.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    first.to_uppercase().chain(characters).collect()
}

fn contains_top_level_colon(source: &str) -> bool {
    find_top_level(source, ":").is_some()
}

fn find_top_level(source: &str, needle: &str) -> Option<usize> {
    find_all_top_level(source, needle).into_iter().next()
}

fn find_all_top_level(source: &str, needle: &str) -> Vec<usize> {
    if needle.is_empty() {
        return Vec::new();
    }
    let mut positions = Vec::new();
    let mut parentheses = 0u32;
    let mut braces = 0u32;
    let mut quoted = false;
    let mut offset = 0usize;
    while offset < source.len() {
        if parentheses == 0 && braces == 0 && !quoted && source[offset..].starts_with(needle) {
            positions.push(offset);
            offset += needle.len();
            continue;
        }
        let Some(character) = source[offset..].chars().next() else {
            break;
        };
        match character {
            '(' if !quoted => parentheses = parentheses.saturating_add(1),
            ')' if !quoted => parentheses = parentheses.saturating_sub(1),
            '{' if !quoted => braces = braces.saturating_add(1),
            '}' if !quoted => braces = braces.saturating_sub(1),
            '"' => quoted = !quoted,
            _ => {}
        }
        offset += character.len_utf8();
    }
    positions
}

fn split_at_positions<'a>(source: &'a str, literal: &str, positions: &[usize]) -> Vec<&'a str> {
    let mut parts = Vec::with_capacity(positions.len() + 1);
    let mut start = 0usize;
    for position in positions {
        parts.push(&source[start..*position]);
        start = *position + literal.len();
    }
    parts.push(&source[start..]);
    parts
}

fn likely_action_verb(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    [
        "draw",
        "discard",
        "sacrifice",
        "gain",
        "lose",
        "deals",
        "fight",
        "tap",
        "untap",
        "put",
        "remove",
        "gets",
        "gains",
        "loses",
        "create",
        "return",
        "exile",
        "reveal",
        "look",
        "search",
        "mill",
        "shuffle",
    ]
    .iter()
    .any(|verb| {
        lower.starts_with(verb)
            || lower.contains(&format!(" {verb} "))
            || lower.contains(&format!(" {verb}s "))
    })
}

fn oracle_action_semantic_digest(
    exact_source: &str,
    normalized_source: &str,
    semantic_context: OracleActionSemanticContext,
    root: &ActionNode,
) -> String {
    let typed_algebra = format!("{root:?}");
    let mut digest = Sha256::new();
    for component in [
        "oracle-action-algebra-content/v1",
        ORACLE_ACTION_ALGEBRA_COMPILER_VERSION,
        ORACLE_ACTION_ALGEBRA_RUNTIME_VERSION,
        ORACLE_ACTION_ALGEBRA_RULES_CONTEXT_VERSION,
        exact_source,
        normalized_source,
        semantic_context.stable_id(),
        &typed_algebra,
    ] {
        digest.update((component.len() as u64).to_le_bytes());
        digest.update(component.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerActionState {
    pub life: i64,
    pub poison_counters: u32,
    pub draws_from_empty_library: u32,
}

impl Default for PlayerActionState {
    fn default() -> Self {
        Self {
            life: 40,
            poison_counters: 0,
            draws_from_empty_library: 0,
        }
    }
}

/// Values copied by a copy effect. Status, counters, marked damage, controller,
/// and continuous effects are deliberately outside this record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiableValues {
    pub name: String,
    pub mana_cost: Option<String>,
    pub card_types: BTreeSet<CardType>,
    pub supertypes: BTreeSet<String>,
    pub colors: BTreeSet<Color>,
    pub subtypes: BTreeSet<String>,
    pub base_power: Option<i32>,
    pub base_toughness: Option<i32>,
    pub base_loyalty: Option<u32>,
    pub base_defense: Option<u32>,
    pub intrinsic_keywords: BTreeSet<KeywordAbility>,
    /// Versioned semantic identities for every other copiable rules ability.
    /// The host must supply the complete set before this object can be copied.
    pub ability_semantic_ids: BTreeSet<String>,
    /// Choices made as the source entered or was turned face up that rules
    /// make copiable.
    pub copiable_choices: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameObject {
    pub reference: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub card_types: BTreeSet<CardType>,
    pub supertypes: BTreeSet<String>,
    pub colors: BTreeSet<Color>,
    pub subtypes: BTreeSet<String>,
    pub base_power: Option<i32>,
    pub base_toughness: Option<i32>,
    pub base_loyalty: Option<u32>,
    pub base_defense: Option<u32>,
    pub tapped: bool,
    pub attacking: bool,
    pub blocking: bool,
    pub marked_damage: u32,
    pub deathtouch_damage: bool,
    pub counters: BTreeMap<String, u32>,
    pub intrinsic_keywords: BTreeSet<KeywordAbility>,
    pub is_token: bool,
    /// None means the host has not supplied complete copiable-value evidence.
    /// Copy actions fail closed for such an object.
    pub copiable_values: Option<CopiableValues>,
}

impl GameObject {
    pub fn is_permanent(&self) -> bool {
        self.zone == Zone::Battlefield
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuousActionEffect {
    PowerToughness {
        objects: Vec<ObjectRef>,
        power: i32,
        toughness: i32,
        duration: Duration,
        source: Option<ObjectRef>,
        program_digest: String,
    },
    Keywords {
        objects: Vec<ObjectRef>,
        operation: KeywordOperation,
        keywords: BTreeSet<KeywordAbility>,
        duration: Duration,
        source: Option<ObjectRef>,
        program_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageEvent {
    pub action_id: ActionId,
    pub source: Option<ObjectRef>,
    pub recipient: ResolvedDamageRecipient,
    pub amount: u32,
    pub source_had_deathtouch: bool,
    pub source_had_lifelink: bool,
    pub source_had_infect: bool,
    pub source_had_wither: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedDamageRecipient {
    Player(PlayerId),
    Object(ObjectRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibilityEvent {
    pub action_id: ActionId,
    pub viewer: Option<PlayerId>,
    pub objects: Vec<ObjectRef>,
    pub public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneMoveEvent {
    pub action_id: ActionId,
    pub old_reference: ObjectRef,
    pub new_reference: ObjectRef,
    pub from: Zone,
    pub to: Zone,
    pub old_owner: PlayerId,
    pub old_controller: PlayerId,
    pub last_known_copiable_values: Option<CopiableValues>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleActionWorldState {
    pub players: BTreeMap<PlayerId, PlayerActionState>,
    /// Exact opponent relations for every player whose actions are resolved.
    pub opponents: BTreeMap<PlayerId, BTreeSet<PlayerId>>,
    /// Top card is the final element.
    pub libraries: BTreeMap<PlayerId, Vec<ObjectRef>>,
    pub objects: BTreeMap<ObjectRef, GameObject>,
    pub continuous_effects: Vec<ContinuousActionEffect>,
    pub damage_events: Vec<DamageEvent>,
    pub visibility_events: Vec<VisibilityEvent>,
    pub zone_moves: Vec<ZoneMoveEvent>,
    pub next_object_id: ObjectId,
    pub battlefield_evidence_complete: bool,
    pub hidden_zone_evidence_complete: bool,
    /// True only after the host proves that no replacement or prevention
    /// effect applies to the state changes staged by this program.
    pub no_applicable_replacement_effects: bool,
}

impl Default for OracleActionWorldState {
    fn default() -> Self {
        Self {
            players: BTreeMap::new(),
            opponents: BTreeMap::new(),
            libraries: BTreeMap::new(),
            objects: BTreeMap::new(),
            continuous_effects: Vec::new(),
            damage_events: Vec::new(),
            visibility_events: Vec::new(),
            zone_moves: Vec::new(),
            next_object_id: 1,
            battlefield_evidence_complete: false,
            hidden_zone_evidence_complete: false,
            no_applicable_replacement_effects: false,
        }
    }
}

/// Adapter boundary for a host state that can stage this algebra. A host gets a
/// full clone, and only a successful resolution replaces the original.
pub trait OracleActionStateAdapter: Clone {
    fn action_world(&self) -> &OracleActionWorldState;
    fn action_world_mut(&mut self) -> &mut OracleActionWorldState;
}

impl OracleActionStateAdapter for OracleActionWorldState {
    fn action_world(&self) -> &OracleActionWorldState {
        self
    }

    fn action_world_mut(&mut self) -> &mut OracleActionWorldState {
        self
    }
}

fn validate_world_state(state: &OracleActionWorldState) -> Result<(), OracleActionRuntimeError> {
    for (player, opponents) in &state.opponents {
        if !state.players.contains_key(player)
            || opponents.contains(player)
            || opponents
                .iter()
                .any(|opponent| !state.players.contains_key(opponent))
        {
            return Err(OracleActionRuntimeError::StateInvariantViolation);
        }
        for opponent in opponents {
            if !state
                .opponents
                .get(opponent)
                .is_some_and(|their_opponents| their_opponents.contains(player))
            {
                return Err(OracleActionRuntimeError::StateInvariantViolation);
            }
        }
    }
    if state.hidden_zone_evidence_complete
        && state
            .players
            .keys()
            .any(|player| !state.libraries.contains_key(player))
    {
        return Err(OracleActionRuntimeError::StateInvariantViolation);
    }

    let mut library_references = BTreeSet::new();
    for (player, library) in &state.libraries {
        if !state.players.contains_key(player) {
            return Err(OracleActionRuntimeError::StateInvariantViolation);
        }
        for reference in library {
            if !library_references.insert(*reference) {
                return Err(OracleActionRuntimeError::StateInvariantViolation);
            }
            let object = state
                .objects
                .get(reference)
                .ok_or(OracleActionRuntimeError::StateInvariantViolation)?;
            if object.zone != Zone::Library || object.owner != *player {
                return Err(OracleActionRuntimeError::StateInvariantViolation);
            }
        }
    }

    let mut object_ids = BTreeSet::new();
    for (reference, object) in &state.objects {
        if *reference != object.reference
            || !object_ids.insert(reference.object_id)
            || !state.players.contains_key(&object.owner)
            || !state.players.contains_key(&object.controller)
            || (!matches!(object.zone, Zone::Battlefield | Zone::Stack)
                && object.controller != object.owner)
            || (object.zone == Zone::Library) != library_references.contains(reference)
        {
            return Err(OracleActionRuntimeError::StateInvariantViolation);
        }
        if let Some(values) = &object.copiable_values
            && (values.card_types != object.card_types
                || values.supertypes != object.supertypes
                || values.colors != object.colors
                || values.subtypes != object.subtypes
                || values.base_power != object.base_power
                || values.base_toughness != object.base_toughness
                || values.base_loyalty != object.base_loyalty
                || values.base_defense != object.base_defense
                || values.intrinsic_keywords != object.intrinsic_keywords)
        {
            return Err(OracleActionRuntimeError::StateInvariantViolation);
        }
    }
    if state
        .objects
        .keys()
        .any(|reference| reference.object_id >= state.next_object_id)
    {
        return Err(OracleActionRuntimeError::StateInvariantViolation);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResolvedAnyTarget {
    Player(PlayerId),
    Object(ObjectRef),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OracleActionBindings {
    pub controller: PlayerId,
    pub source: Option<ObjectRef>,
    pub that_player: Option<PlayerId>,
    pub chosen_player: Option<PlayerId>,
    pub that_object: Option<ObjectRef>,
    pub enchanted_object: Option<ObjectRef>,
    pub equipped_object: Option<ObjectRef>,
    pub player_targets: BTreeMap<ActionId, Vec<PlayerId>>,
    pub object_targets: BTreeMap<(ActionId, u8), Vec<ObjectRef>>,
    pub object_choices: BTreeMap<(ActionId, PlayerId), Vec<ObjectRef>>,
    pub card_choices: BTreeMap<(ActionId, PlayerId), Vec<ObjectRef>>,
    /// Externally sampled outcomes for instructions that explicitly say
    /// "at random". A deliberate choice cannot stand in for a random result.
    pub random_card_outcomes: BTreeMap<(ActionId, PlayerId), Vec<ObjectRef>>,
    /// Values such as X are scoped to the complete resolving instruction, not
    /// to an individual action node. Repeated uses therefore cannot diverge.
    pub variable_amounts: BTreeMap<VariableAmount, u32>,
    pub optional_choices: BTreeMap<ActionId, bool>,
    pub any_targets: BTreeMap<ActionId, ResolvedAnyTarget>,
    pub shuffle_orders: BTreeMap<(ActionId, PlayerId), Vec<ObjectRef>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleActionRuntimeError {
    ProductionAdapterDisconnected,
    IncompleteBattlefieldEvidence,
    IncompleteHiddenZoneEvidence,
    IncompleteReplacementEffectEvidence,
    IncompletePlayerRelationEvidence(PlayerId),
    MissingPlayer(PlayerId),
    MissingSource,
    MissingThatPlayer,
    AmbiguousThatPlayer(usize),
    MissingChosenPlayer,
    MissingThatObject,
    AmbiguousThatObject(usize),
    MissingAttachment,
    MissingTarget(ActionId),
    WrongTargetCardinality {
        action: ActionId,
        expected: Cardinality,
        actual: usize,
    },
    DuplicateTarget(ObjectRef),
    IllegalPlayerTarget {
        action: ActionId,
        player: PlayerId,
    },
    MissingObject(ObjectRef),
    StaleObject(ObjectRef),
    IllegalObjectTarget {
        action: ActionId,
        object: ObjectRef,
    },
    MissingChoice(ActionId),
    MissingRandomOutcome(ActionId),
    IllegalCardChoice {
        action: ActionId,
        object: ObjectRef,
    },
    IllegalObjectChoice {
        action: ActionId,
        object: ObjectRef,
    },
    MissingVariableAmount {
        action: ActionId,
        variable: VariableAmount,
    },
    AmountOverflow,
    LifeOverflow,
    CounterOverflow,
    InsufficientCounters {
        object: ObjectRef,
        counter: String,
        required: u32,
        available: u32,
    },
    MissingPower(ObjectRef),
    IncompleteCopiableValues(ObjectRef),
    ObjectNotOnBattlefield(ObjectRef),
    ObjectIdOverflow,
    IncarnationOverflow(ObjectRef),
    MissingAnyTarget(ActionId),
    IllegalAnyTarget(ActionId),
    InvalidShuffleOrder {
        action: ActionId,
        player: PlayerId,
    },
    UnsupportedDestinationOwner,
    StateInvariantViolation,
}

impl fmt::Display for OracleActionRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for OracleActionRuntimeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionReceiptKind {
    Drew {
        players: Vec<PlayerId>,
        cards: Vec<ObjectRef>,
        failed_draws: u32,
    },
    Discarded {
        cards: Vec<ObjectRef>,
    },
    Sacrificed {
        players: Vec<PlayerId>,
        events: Vec<ZoneMoveEvent>,
    },
    LifeChanged {
        players: Vec<PlayerId>,
        deltas: Vec<i64>,
    },
    Damage {
        events: Vec<DamageEvent>,
    },
    Fought {
        first: ObjectRef,
        second: ObjectRef,
        events: Vec<DamageEvent>,
    },
    Tapped {
        objects: Vec<ObjectRef>,
        tapped: bool,
    },
    CountersChanged {
        objects: Vec<ObjectRef>,
        counter: String,
        requested_amount: u32,
        actual_amounts: Vec<u32>,
        operation: CounterOperation,
    },
    ContinuousEffect {
        objects: Vec<ObjectRef>,
    },
    TokensCreated {
        objects: Vec<ObjectRef>,
    },
    ZoneMoved {
        events: Vec<ZoneMoveEvent>,
    },
    RevealedOrLooked {
        objects: Vec<ObjectRef>,
        public: bool,
    },
    Searched {
        objects: Vec<ObjectRef>,
    },
    Milled {
        cards: Vec<ObjectRef>,
    },
    Shuffled {
        players: Vec<PlayerId>,
    },
    OptionalDeclined,
    ConditionalBranch {
        condition: bool,
    },
    Sequence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionReceipt {
    pub action_id: ActionId,
    pub kind: ActionReceiptKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleActionProgramReceipt {
    pub program_digest: String,
    pub receipts: Vec<ActionReceipt>,
    pub committed: bool,
}

#[derive(Debug, Clone, Default)]
struct ExecutionMemory {
    last_objects: Vec<ObjectRef>,
    last_players: Vec<PlayerId>,
    last_amount: Option<u32>,
    last_known_objects: BTreeMap<ObjectRef, LastKnownObjectInformation>,
    previous_action_succeeded: bool,
    scoped_actor: Option<PlayerId>,
    optional_scope_depth: u32,
}

#[derive(Debug, Clone)]
struct LastKnownObjectInformation {
    controller: PlayerId,
    copiable_values: Option<CopiableValues>,
}

/// Resolve into a cloned adapter and commit only after every nested action has
/// completed. Any error, including an error in the final sequence member,
/// drops the staged state and leaves `state` byte-for-byte equivalent.
pub fn execute_oracle_action_program_transactionally<S: OracleActionStateAdapter>(
    program: &OracleActionProgram,
    bindings: &OracleActionBindings,
    state: &mut S,
) -> Result<OracleActionProgramReceipt, OracleActionRuntimeError> {
    let mut staged = state.clone();
    {
        let world = staged.action_world_mut();
        validate_world_state(world)?;
        if !world.no_applicable_replacement_effects {
            return Err(OracleActionRuntimeError::IncompleteReplacementEffectEvidence);
        }
        if !world.players.contains_key(&bindings.controller) {
            return Err(OracleActionRuntimeError::MissingPlayer(bindings.controller));
        }
        if let Some(source) = bindings.source
            && !world.objects.contains_key(&source)
        {
            return Err(OracleActionRuntimeError::StaleObject(source));
        }
    }

    let mut receipts = Vec::new();
    let mut memory = ExecutionMemory::default();
    execute_action_node(
        &program.root,
        program.semantic_digest(),
        bindings,
        staged.action_world_mut(),
        &mut memory,
        &mut receipts,
    )?;
    validate_world_state(staged.action_world())?;
    *state = staged;
    Ok(OracleActionProgramReceipt {
        program_digest: program.semantic_digest.clone(),
        receipts,
        committed: true,
    })
}

fn execute_action_node(
    node: &ActionNode,
    program_digest: &str,
    bindings: &OracleActionBindings,
    state: &mut OracleActionWorldState,
    memory: &mut ExecutionMemory,
    receipts: &mut Vec<ActionReceipt>,
) -> Result<(), OracleActionRuntimeError> {
    match &node.kind {
        ActionKind::Draw { player, amount } => {
            require_hidden_zones(state)?;
            let players = resolve_players(*player, node.id, bindings, state, memory)?;
            let amount = resolve_amount(amount, node.id, bindings, state, memory)?;
            let mut cards = Vec::new();
            let mut failed_draws = 0u32;
            for player in &players {
                for _ in 0..amount {
                    let top = state
                        .libraries
                        .get_mut(player)
                        .ok_or(OracleActionRuntimeError::MissingPlayer(*player))?
                        .pop();
                    if let Some(card) = top {
                        let moved = move_object_to_zone(
                            node.id,
                            card,
                            ZoneOperand {
                                owner: ZoneOwner::ObjectOwner,
                                zone: Zone::Hand,
                            },
                            None,
                            false,
                            bindings,
                            state,
                        )?;
                        cards.push(moved.new_reference);
                    } else {
                        let player_state = state
                            .players
                            .get_mut(player)
                            .ok_or(OracleActionRuntimeError::MissingPlayer(*player))?;
                        player_state.draws_from_empty_library = player_state
                            .draws_from_empty_library
                            .checked_add(1)
                            .ok_or(OracleActionRuntimeError::AmountOverflow)?;
                        failed_draws = failed_draws
                            .checked_add(1)
                            .ok_or(OracleActionRuntimeError::AmountOverflow)?;
                    }
                }
            }
            memory.last_objects = cards.clone();
            memory.last_players = players.clone();
            memory.last_amount = Some(amount);
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::Drew {
                    players,
                    cards,
                    failed_draws,
                },
            });
        }
        ActionKind::Discard { player, selection } => {
            require_hidden_zones(state)?;
            let players = resolve_players(*player, node.id, bindings, state, memory)?;
            let mut moved_cards = Vec::new();
            for player in &players {
                let policy = if memory.optional_scope_depth > 0 {
                    CardSelectionPolicy::Exact
                } else {
                    CardSelectionPolicy::AsMuchAsPossible
                };
                let chosen = resolve_card_selection_with_policy(
                    selection, node.id, *player, bindings, state, policy,
                )?;
                for card in chosen {
                    let moved = move_object_to_zone(
                        node.id,
                        card,
                        ZoneOperand {
                            owner: ZoneOwner::ObjectOwner,
                            zone: Zone::Graveyard,
                        },
                        None,
                        false,
                        bindings,
                        state,
                    )?;
                    moved_cards.push(moved.new_reference);
                }
            }
            memory.last_objects = moved_cards.clone();
            memory.last_players = players;
            memory.last_amount = u32::try_from(moved_cards.len()).ok();
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::Discarded { cards: moved_cards },
            });
        }
        ActionKind::Sacrifice { player, selection } => {
            require_battlefield(state)?;
            let players = resolve_players(*player, node.id, bindings, state, memory)?;
            let mut events = Vec::new();
            let mut distinct = BTreeSet::new();
            for player in &players {
                let objects = match selection {
                    SacrificeSelection::Objects(objects) => {
                        if players.len() != 1 {
                            return Err(OracleActionRuntimeError::StateInvariantViolation);
                        }
                        resolve_objects(objects, node.id, bindings, state, memory)?
                    }
                    SacrificeSelection::Choice {
                        cardinality,
                        filter,
                    } => {
                        let objects = bindings
                            .object_choices
                            .get(&(node.id, *player))
                            .cloned()
                            .or_else(|| {
                                matches!(
                                    cardinality,
                                    Cardinality::UpTo(_)
                                        | Cardinality::AnyNumber
                                        | Cardinality::Exactly(0)
                                )
                                .then(Vec::new)
                            })
                            .ok_or(OracleActionRuntimeError::MissingChoice(node.id))?;
                        validate_cardinality(*cardinality, objects.len(), node.id)?;
                        for object in &objects {
                            let state_object = state
                                .objects
                                .get(object)
                                .ok_or(OracleActionRuntimeError::MissingObject(*object))?;
                            if state_object.controller != *player
                                || !object_matches_filter(
                                    state_object,
                                    filter,
                                    *player,
                                    bindings.source,
                                    state,
                                )?
                            {
                                return Err(OracleActionRuntimeError::IllegalObjectChoice {
                                    action: node.id,
                                    object: *object,
                                });
                            }
                        }
                        objects
                    }
                };
                for object in objects {
                    if !distinct.insert(object) {
                        return Err(OracleActionRuntimeError::DuplicateTarget(object));
                    }
                    let state_object = state
                        .objects
                        .get(&object)
                        .ok_or(OracleActionRuntimeError::MissingObject(object))?;
                    if state_object.zone != Zone::Battlefield || state_object.controller != *player
                    {
                        return Err(OracleActionRuntimeError::IllegalObjectChoice {
                            action: node.id,
                            object,
                        });
                    }
                    events.push(move_object_to_zone(
                        node.id,
                        object,
                        ZoneOperand {
                            owner: ZoneOwner::ObjectOwner,
                            zone: Zone::Graveyard,
                        },
                        None,
                        false,
                        bindings,
                        state,
                    )?);
                }
            }
            remember_zone_moves(&events, memory);
            memory.last_amount = u32::try_from(events.len()).ok();
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::Sacrificed { players, events },
            });
        }
        ActionKind::GainLife { player, amount } | ActionKind::LoseLife { player, amount } => {
            let gain = matches!(&node.kind, ActionKind::GainLife { .. });
            let players = resolve_players(*player, node.id, bindings, state, memory)?;
            let amount = resolve_amount(amount, node.id, bindings, state, memory)?;
            let signed = i64::from(amount);
            let delta = if gain { signed } else { -signed };
            let mut deltas = Vec::new();
            for player in &players {
                let player_state = state
                    .players
                    .get_mut(player)
                    .ok_or(OracleActionRuntimeError::MissingPlayer(*player))?;
                player_state.life = player_state
                    .life
                    .checked_add(delta)
                    .ok_or(OracleActionRuntimeError::LifeOverflow)?;
                deltas.push(delta);
            }
            memory.last_objects.clear();
            memory.last_players = players.clone();
            memory.last_amount = Some(amount);
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::LifeChanged { players, deltas },
            });
        }
        ActionKind::DealDamage {
            source,
            recipient,
            amount,
        } => {
            require_battlefield(state)?;
            let source = resolve_damage_source(source, node.id, bindings, state, memory)?;
            let recipients =
                resolve_damage_recipients(recipient, node.id, bindings, state, memory)?;
            let amount = resolve_amount(amount, node.id, bindings, state, memory)?;
            let events = recipients
                .into_iter()
                .map(|recipient| {
                    build_damage_event(node.id, source, recipient, amount, state, program_digest)
                })
                .collect::<Result<Vec<_>, _>>()?;
            apply_damage_batch(&events, state)?;
            memory.last_objects = events
                .iter()
                .filter_map(|event| match event.recipient {
                    ResolvedDamageRecipient::Object(object) => Some(object),
                    ResolvedDamageRecipient::Player(_) => None,
                })
                .collect();
            memory.last_players = events
                .iter()
                .filter_map(|event| match event.recipient {
                    ResolvedDamageRecipient::Player(player) => Some(player),
                    ResolvedDamageRecipient::Object(_) => None,
                })
                .collect();
            memory.last_amount = Some(amount);
            memory.previous_action_succeeded = true;
            state.damage_events.extend(events.clone());
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::Damage { events },
            });
        }
        ActionKind::Fight { first, second } => {
            require_battlefield(state)?;
            let first = resolve_objects(first, node.id, bindings, state, memory)?
                .into_iter()
                .next()
                .ok_or(OracleActionRuntimeError::MissingTarget(node.id))?;
            let second = resolve_objects(second, node.id, bindings, state, memory)?
                .into_iter()
                .find(|object| *object != first)
                .ok_or(OracleActionRuntimeError::MissingTarget(node.id))?;
            let first_power = current_power(first, state)?;
            let second_power = current_power(second, state)?;
            let events = vec![
                build_damage_event(
                    node.id,
                    Some(first),
                    ResolvedDamageRecipient::Object(second),
                    u32::try_from(first_power.max(0))
                        .map_err(|_| OracleActionRuntimeError::AmountOverflow)?,
                    state,
                    program_digest,
                )?,
                build_damage_event(
                    node.id,
                    Some(second),
                    ResolvedDamageRecipient::Object(first),
                    u32::try_from(second_power.max(0))
                        .map_err(|_| OracleActionRuntimeError::AmountOverflow)?,
                    state,
                    program_digest,
                )?,
            ];
            apply_damage_batch(&events, state)?;
            state.damage_events.extend(events.clone());
            memory.last_objects = vec![first, second];
            memory.last_players.clear();
            memory.last_amount = None;
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::Fought {
                    first,
                    second,
                    events,
                },
            });
        }
        ActionKind::Tap { objects } | ActionKind::Untap { objects } => {
            require_battlefield(state)?;
            let tapped = matches!(&node.kind, ActionKind::Tap { .. });
            let objects = resolve_objects(objects, node.id, bindings, state, memory)?;
            for object in &objects {
                let state_object = state
                    .objects
                    .get_mut(object)
                    .ok_or(OracleActionRuntimeError::MissingObject(*object))?;
                if !state_object.is_permanent() {
                    return Err(OracleActionRuntimeError::ObjectNotOnBattlefield(*object));
                }
                state_object.tapped = tapped;
            }
            memory.last_objects = objects.clone();
            memory.last_players.clear();
            memory.last_amount = u32::try_from(objects.len()).ok();
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::Tapped { objects, tapped },
            });
        }
        ActionKind::ChangeCounters {
            operation,
            objects,
            counter,
            amount,
        } => {
            require_battlefield(state)?;
            let objects = resolve_objects(objects, node.id, bindings, state, memory)?;
            let amount = resolve_amount(amount, node.id, bindings, state, memory)?;
            let mut actual_amounts = Vec::with_capacity(objects.len());
            for object in &objects {
                let state_object = state
                    .objects
                    .get_mut(object)
                    .ok_or(OracleActionRuntimeError::MissingObject(*object))?;
                if !state_object.is_permanent() {
                    return Err(OracleActionRuntimeError::ObjectNotOnBattlefield(*object));
                }
                let current = state_object.counters.get(counter).copied().unwrap_or(0);
                let actual = match operation {
                    CounterOperation::Put => amount,
                    CounterOperation::Remove
                        if memory.optional_scope_depth > 0 && current < amount =>
                    {
                        return Err(OracleActionRuntimeError::InsufficientCounters {
                            object: *object,
                            counter: counter.clone(),
                            required: amount,
                            available: current,
                        });
                    }
                    CounterOperation::Remove => current.min(amount),
                };
                let next = match operation {
                    CounterOperation::Put => current
                        .checked_add(actual)
                        .ok_or(OracleActionRuntimeError::CounterOverflow)?,
                    CounterOperation::Remove => current - actual,
                };
                if next == 0 {
                    state_object.counters.remove(counter);
                } else {
                    state_object.counters.insert(counter.clone(), next);
                }
                actual_amounts.push(actual);
            }
            memory.last_objects = objects.clone();
            memory.last_players.clear();
            memory.last_amount = Some(amount);
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::CountersChanged {
                    objects,
                    counter: counter.clone(),
                    requested_amount: amount,
                    actual_amounts,
                    operation: *operation,
                },
            });
        }
        ActionKind::ModifyPowerToughness {
            objects,
            power,
            toughness,
            duration,
        } => {
            require_battlefield(state)?;
            let objects = resolve_objects(objects, node.id, bindings, state, memory)?;
            require_objects_on_battlefield(&objects, state)?;
            state
                .continuous_effects
                .push(ContinuousActionEffect::PowerToughness {
                    objects: objects.clone(),
                    power: *power,
                    toughness: *toughness,
                    duration: *duration,
                    source: bindings.source,
                    program_digest: program_digest.to_owned(),
                });
            memory.last_objects = objects.clone();
            memory.last_players.clear();
            memory.last_amount = None;
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::ContinuousEffect { objects },
            });
        }
        ActionKind::ChangeKeywords {
            operation,
            objects,
            keywords,
            duration,
        } => {
            require_battlefield(state)?;
            let objects = resolve_objects(objects, node.id, bindings, state, memory)?;
            require_objects_on_battlefield(&objects, state)?;
            state
                .continuous_effects
                .push(ContinuousActionEffect::Keywords {
                    objects: objects.clone(),
                    operation: *operation,
                    keywords: keywords.clone(),
                    duration: *duration,
                    source: bindings.source,
                    program_digest: program_digest.to_owned(),
                });
            memory.last_objects = objects.clone();
            memory.last_players.clear();
            memory.last_amount = None;
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::ContinuousEffect { objects },
            });
        }
        ActionKind::CreateToken {
            player,
            amount,
            template,
        } => {
            require_battlefield(state)?;
            let players = resolve_players(*player, node.id, bindings, state, memory)?;
            let amount = resolve_amount(amount, node.id, bindings, state, memory)?;
            let mut created = Vec::new();
            for player in &players {
                for _ in 0..amount {
                    created.push(create_token(*player, template, state)?);
                }
            }
            memory.last_objects = created.clone();
            memory.last_players = players;
            memory.last_amount = Some(amount);
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::TokensCreated { objects: created },
            });
        }
        ActionKind::CreateCopyToken {
            player,
            source,
            tapped,
        } => {
            require_battlefield(state)?;
            let players = resolve_players(*player, node.id, bindings, state, memory)?;
            let source = resolve_objects(source, node.id, bindings, state, memory)?
                .into_iter()
                .next()
                .ok_or(OracleActionRuntimeError::MissingTarget(node.id))?;
            let copiable_values = memory
                .last_known_objects
                .get(&source)
                .and_then(|information| information.copiable_values.clone())
                .or_else(|| {
                    state
                        .objects
                        .get(&source)
                        .and_then(|object| object.copiable_values.clone())
                })
                .ok_or(OracleActionRuntimeError::IncompleteCopiableValues(source))?;
            let mut created = Vec::new();
            for player in &players {
                created.push(create_copy_token(
                    *player,
                    &copiable_values,
                    *tapped,
                    state,
                )?);
            }
            memory.last_objects = created.clone();
            memory.last_players = players;
            memory.last_amount = Some(1);
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::TokensCreated { objects: created },
            });
        }
        ActionKind::MoveZone {
            objects,
            from,
            destination,
            position,
            tapped,
        } => {
            require_battlefield_if_needed(*from, destination.zone, state)?;
            let objects = resolve_objects(objects, node.id, bindings, state, memory)?;
            let mut events = Vec::new();
            for object in objects {
                if from.is_some_and(|zone| {
                    state
                        .objects
                        .get(&object)
                        .is_none_or(|state_object| state_object.zone != zone)
                }) {
                    return Err(OracleActionRuntimeError::IllegalObjectTarget {
                        action: node.id,
                        object,
                    });
                }
                events.push(move_object_to_zone(
                    node.id,
                    object,
                    *destination,
                    *position,
                    *tapped,
                    bindings,
                    state,
                )?);
            }
            remember_zone_moves(&events, memory);
            memory.last_amount = u32::try_from(events.len()).ok();
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::ZoneMoved { events },
            });
        }
        ActionKind::MoveCards {
            selection,
            destination,
            position,
            tapped,
        } => {
            require_hidden_zones(state)?;
            let acting_player = memory.scoped_actor.unwrap_or(bindings.controller);
            let that_player = if selection.zone.owner == ZoneOwner::ThatPlayer {
                Some(
                    bindings
                        .that_player
                        .map(Ok)
                        .unwrap_or_else(|| single_last_player(memory))?,
                )
            } else {
                Some(acting_player)
            };
            let player = resolve_zone_owner_player(
                selection.zone.owner,
                acting_player,
                that_player,
                None,
                state,
            )?;
            let cards = resolve_card_selection(selection, node.id, player, bindings, state)?;
            let mut events = Vec::new();
            for card in cards {
                events.push(move_object_to_zone(
                    node.id,
                    card,
                    *destination,
                    *position,
                    *tapped,
                    bindings,
                    state,
                )?);
            }
            remember_zone_moves(&events, memory);
            memory.last_amount = u32::try_from(events.len()).ok();
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::ZoneMoved { events },
            });
        }
        ActionKind::Reveal { player, selection } | ActionKind::Look { player, selection } => {
            require_hidden_zones(state)?;
            let public = matches!(&node.kind, ActionKind::Reveal { .. });
            let players = resolve_players(*player, node.id, bindings, state, memory)?;
            let mut visible = Vec::new();
            for player in &players {
                let objects = resolve_visibility_selection(selection, *player, state)?;
                state.visibility_events.push(VisibilityEvent {
                    action_id: node.id,
                    viewer: (!public).then_some(*player),
                    objects: objects.clone(),
                    public,
                });
                visible.extend(objects);
            }
            memory.last_objects = visible.clone();
            memory.last_players = players;
            memory.last_amount = u32::try_from(visible.len()).ok();
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::RevealedOrLooked {
                    objects: visible,
                    public,
                },
            });
        }
        ActionKind::SearchLibrary {
            player,
            selection,
            destination,
            position,
            reveal,
            tapped,
            shuffle_after,
            ..
        } => {
            require_hidden_zones(state)?;
            let players = resolve_players(*player, node.id, bindings, state, memory)?;
            let mut found = Vec::new();
            for player in &players {
                let cards = resolve_card_selection_with_policy(
                    selection,
                    node.id,
                    *player,
                    bindings,
                    state,
                    CardSelectionPolicy::HiddenSearch,
                )?;
                for card in cards {
                    if *reveal {
                        state.visibility_events.push(VisibilityEvent {
                            action_id: node.id,
                            viewer: None,
                            objects: vec![card],
                            public: true,
                        });
                    }
                    let moved = move_object_to_zone(
                        node.id,
                        card,
                        *destination,
                        *position,
                        *tapped,
                        bindings,
                        state,
                    )?;
                    found.push(moved.new_reference);
                }
                if *shuffle_after {
                    apply_shuffle(node.id, *player, bindings, state)?;
                }
            }
            memory.last_objects = found.clone();
            memory.last_players = players;
            memory.last_amount = u32::try_from(found.len()).ok();
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::Searched { objects: found },
            });
        }
        ActionKind::Mill { player, amount } => {
            require_hidden_zones(state)?;
            let players = resolve_players(*player, node.id, bindings, state, memory)?;
            let amount = resolve_amount(amount, node.id, bindings, state, memory)?;
            let mut milled = Vec::new();
            for player in &players {
                for _ in 0..amount {
                    let Some(card) = state
                        .libraries
                        .get_mut(player)
                        .ok_or(OracleActionRuntimeError::MissingPlayer(*player))?
                        .pop()
                    else {
                        break;
                    };
                    let moved = move_object_to_zone(
                        node.id,
                        card,
                        ZoneOperand {
                            owner: ZoneOwner::ObjectOwner,
                            zone: Zone::Graveyard,
                        },
                        None,
                        false,
                        bindings,
                        state,
                    )?;
                    milled.push(moved.new_reference);
                }
            }
            memory.last_objects = milled.clone();
            memory.last_players = players;
            memory.last_amount = u32::try_from(milled.len()).ok();
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::Milled { cards: milled },
            });
        }
        ActionKind::ShuffleLibrary { player } => {
            require_hidden_zones(state)?;
            let players = resolve_players(*player, node.id, bindings, state, memory)?;
            for player in &players {
                apply_shuffle(node.id, *player, bindings, state)?;
            }
            memory.last_objects.clear();
            memory.last_players = players.clone();
            memory.last_amount = None;
            memory.previous_action_succeeded = true;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::Shuffled { players },
            });
        }
        ActionKind::Optional { actor, action } => {
            let choice = bindings
                .optional_choices
                .get(&node.id)
                .copied()
                .ok_or(OracleActionRuntimeError::MissingChoice(node.id))?;
            let actors = resolve_players(*actor, node.id, bindings, state, memory)?;
            if actors.len() != 1 {
                return Err(OracleActionRuntimeError::WrongTargetCardinality {
                    action: node.id,
                    expected: Cardinality::ExactlyOne,
                    actual: actors.len(),
                });
            }
            if choice {
                let prior_actor = memory.scoped_actor.replace(actors[0]);
                memory.optional_scope_depth = memory
                    .optional_scope_depth
                    .checked_add(1)
                    .ok_or(OracleActionRuntimeError::AmountOverflow)?;
                let resolution =
                    execute_action_node(action, program_digest, bindings, state, memory, receipts);
                memory.optional_scope_depth -= 1;
                memory.scoped_actor = prior_actor;
                resolution?;
            } else {
                memory.last_objects.clear();
                memory.last_players.clear();
                memory.last_amount = Some(0);
                memory.previous_action_succeeded = false;
                receipts.push(ActionReceipt {
                    action_id: node.id,
                    kind: ActionReceiptKind::OptionalDeclined,
                });
            }
        }
        ActionKind::Conditional {
            predicate,
            if_true,
            if_false,
        } => {
            let condition = evaluate_predicate(predicate, bindings, state, memory)?;
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::ConditionalBranch { condition },
            });
            if condition {
                execute_action_node(if_true, program_digest, bindings, state, memory, receipts)?;
            } else if let Some(if_false) = if_false {
                execute_action_node(if_false, program_digest, bindings, state, memory, receipts)?;
            } else {
                memory.previous_action_succeeded = false;
            }
        }
        ActionKind::OrderedSequence {
            actions,
            separators,
        } => {
            if actions.is_empty() || separators.len() + 1 != actions.len() {
                return Err(OracleActionRuntimeError::StateInvariantViolation);
            }
            for action in actions {
                execute_action_node(action, program_digest, bindings, state, memory, receipts)?;
            }
            receipts.push(ActionReceipt {
                action_id: node.id,
                kind: ActionReceiptKind::Sequence,
            });
        }
    }
    Ok(())
}

fn remember_zone_moves(events: &[ZoneMoveEvent], memory: &mut ExecutionMemory) {
    memory.last_objects = events.iter().map(|event| event.new_reference).collect();
    memory.last_players = events
        .iter()
        .map(|event| event.old_controller)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    memory.last_known_objects = events
        .iter()
        .map(|event| {
            (
                event.new_reference,
                LastKnownObjectInformation {
                    controller: event.old_controller,
                    copiable_values: event.last_known_copiable_values.clone(),
                },
            )
        })
        .collect();
}

fn require_battlefield(state: &OracleActionWorldState) -> Result<(), OracleActionRuntimeError> {
    if state.battlefield_evidence_complete {
        Ok(())
    } else {
        Err(OracleActionRuntimeError::IncompleteBattlefieldEvidence)
    }
}

fn require_objects_on_battlefield(
    objects: &[ObjectRef],
    state: &OracleActionWorldState,
) -> Result<(), OracleActionRuntimeError> {
    for object in objects {
        if !state
            .objects
            .get(object)
            .ok_or(OracleActionRuntimeError::MissingObject(*object))?
            .is_permanent()
        {
            return Err(OracleActionRuntimeError::ObjectNotOnBattlefield(*object));
        }
    }
    Ok(())
}

fn require_hidden_zones(state: &OracleActionWorldState) -> Result<(), OracleActionRuntimeError> {
    if state.hidden_zone_evidence_complete {
        Ok(())
    } else {
        Err(OracleActionRuntimeError::IncompleteHiddenZoneEvidence)
    }
}

fn require_battlefield_if_needed(
    from: Option<Zone>,
    destination: Zone,
    state: &OracleActionWorldState,
) -> Result<(), OracleActionRuntimeError> {
    if from == Some(Zone::Battlefield) || destination == Zone::Battlefield {
        require_battlefield(state)
    } else {
        Ok(())
    }
}

fn resolve_players(
    operand: PlayerOperand,
    action: ActionId,
    bindings: &OracleActionBindings,
    state: &OracleActionWorldState,
    memory: &ExecutionMemory,
) -> Result<Vec<PlayerId>, OracleActionRuntimeError> {
    let players = match operand {
        PlayerOperand::You => vec![bindings.controller],
        PlayerOperand::TargetPlayer | PlayerOperand::TargetOpponent => bindings
            .player_targets
            .get(&action)
            .cloned()
            .or_else(|| memory.scoped_actor.map(|player| vec![player]))
            .ok_or(OracleActionRuntimeError::MissingTarget(action))?,
        PlayerOperand::EachPlayer => state.players.keys().copied().collect(),
        PlayerOperand::EachOpponent => state
            .opponents
            .get(&bindings.controller)
            .cloned()
            .ok_or(OracleActionRuntimeError::IncompletePlayerRelationEvidence(
                bindings.controller,
            ))?
            .into_iter()
            .collect(),
        PlayerOperand::ThatPlayer => vec![
            bindings
                .that_player
                .map(Ok)
                .unwrap_or_else(|| single_last_player(memory))?,
        ],
        PlayerOperand::ChosenPlayer => vec![
            bindings
                .chosen_player
                .ok_or(OracleActionRuntimeError::MissingChosenPlayer)?,
        ],
        PlayerOperand::OwnerOfSelectedObject => {
            let object = bindings
                .that_object
                .map(Ok)
                .unwrap_or_else(|| single_last_object(memory))?;
            vec![
                state
                    .objects
                    .get(&object)
                    .ok_or(OracleActionRuntimeError::MissingObject(object))?
                    .owner,
            ]
        }
        PlayerOperand::ControllerOfSelectedObject => {
            let object = bindings
                .that_object
                .map(Ok)
                .unwrap_or_else(|| single_last_object(memory))?;
            vec![
                state
                    .objects
                    .get(&object)
                    .ok_or(OracleActionRuntimeError::MissingObject(object))?
                    .controller,
            ]
        }
    };
    let expected = match operand {
        PlayerOperand::TargetPlayer | PlayerOperand::TargetOpponent => {
            Some(Cardinality::ExactlyOne)
        }
        _ => None,
    };
    if let Some(expected) = expected {
        validate_cardinality(expected, players.len(), action)?;
    }
    let mut distinct = BTreeSet::new();
    for player in &players {
        let target_is_opponent = operand != PlayerOperand::TargetOpponent
            || state
                .opponents
                .get(&bindings.controller)
                .ok_or(OracleActionRuntimeError::IncompletePlayerRelationEvidence(
                    bindings.controller,
                ))?
                .contains(player);
        if !state.players.contains_key(player) || !distinct.insert(*player) || !target_is_opponent {
            return Err(OracleActionRuntimeError::IllegalPlayerTarget {
                action,
                player: *player,
            });
        }
    }
    Ok(players)
}

fn single_last_player(memory: &ExecutionMemory) -> Result<PlayerId, OracleActionRuntimeError> {
    match memory.last_players.as_slice() {
        [player] => Ok(*player),
        [] => Err(OracleActionRuntimeError::MissingThatPlayer),
        players => Err(OracleActionRuntimeError::AmbiguousThatPlayer(players.len())),
    }
}

fn single_last_object(memory: &ExecutionMemory) -> Result<ObjectRef, OracleActionRuntimeError> {
    match memory.last_objects.as_slice() {
        [object] => Ok(*object),
        [] => Err(OracleActionRuntimeError::MissingThatObject),
        objects => Err(OracleActionRuntimeError::AmbiguousThatObject(objects.len())),
    }
}

fn resolve_objects(
    operand: &ObjectOperand,
    action: ActionId,
    bindings: &OracleActionBindings,
    state: &OracleActionWorldState,
    memory: &ExecutionMemory,
) -> Result<Vec<ObjectRef>, OracleActionRuntimeError> {
    let (objects, filter, cardinality) = match operand {
        ObjectOperand::Source => (
            vec![
                bindings
                    .source
                    .ok_or(OracleActionRuntimeError::MissingSource)?,
            ],
            None,
            Cardinality::ExactlyOne,
        ),
        ObjectOperand::It | ObjectOperand::ThatObject => (
            vec![
                bindings
                    .that_object
                    .map(Ok)
                    .unwrap_or_else(|| single_last_object(memory))?,
            ],
            None,
            Cardinality::ExactlyOne,
        ),
        ObjectOperand::PreviousSelection => {
            (memory.last_objects.clone(), None, Cardinality::AnyNumber)
        }
        ObjectOperand::Target {
            slot,
            cardinality,
            filter,
        } => {
            let objects = bindings
                .object_targets
                .get(&(action, *slot))
                .cloned()
                .or_else(|| {
                    matches!(
                        cardinality,
                        Cardinality::UpTo(_) | Cardinality::AnyNumber | Cardinality::Exactly(0)
                    )
                    .then(Vec::new)
                })
                .ok_or(OracleActionRuntimeError::MissingTarget(action))?;
            (objects, Some(filter), *cardinality)
        }
        ObjectOperand::Set {
            cardinality,
            filter,
        } => {
            let mut objects = Vec::new();
            for object in state.objects.values() {
                if object_matches_filter(
                    object,
                    filter,
                    bindings.controller,
                    bindings.source,
                    state,
                )? {
                    objects.push(object.reference);
                }
            }
            (objects, Some(filter), *cardinality)
        }
        ObjectOperand::EnchantedObject => (
            vec![
                bindings
                    .enchanted_object
                    .ok_or(OracleActionRuntimeError::MissingAttachment)?,
            ],
            None,
            Cardinality::ExactlyOne,
        ),
        ObjectOperand::EquippedObject => (
            vec![
                bindings
                    .equipped_object
                    .ok_or(OracleActionRuntimeError::MissingAttachment)?,
            ],
            None,
            Cardinality::ExactlyOne,
        ),
    };
    validate_cardinality(cardinality, objects.len(), action)?;
    let mut distinct = BTreeSet::new();
    for object in &objects {
        if !distinct.insert(*object) {
            return Err(OracleActionRuntimeError::DuplicateTarget(*object));
        }
        let state_object = state
            .objects
            .get(object)
            .ok_or(OracleActionRuntimeError::MissingObject(*object))?;
        if let Some(filter) = filter
            && !object_matches_filter(
                state_object,
                filter,
                bindings.controller,
                bindings.source,
                state,
            )?
        {
            return Err(OracleActionRuntimeError::IllegalObjectTarget {
                action,
                object: *object,
            });
        }
    }
    Ok(objects)
}

fn object_matches_filter(
    object: &GameObject,
    filter: &ObjectFilter,
    controller: PlayerId,
    source: Option<ObjectRef>,
    state: &OracleActionWorldState,
) -> Result<bool, OracleActionRuntimeError> {
    let controller_matches = match filter.controller {
        ControllerConstraint::Any => true,
        ControllerConstraint::You => object.controller == controller,
        ControllerConstraint::Opponent => state
            .opponents
            .get(&controller)
            .ok_or(OracleActionRuntimeError::IncompletePlayerRelationEvidence(
                controller,
            ))?
            .contains(&object.controller),
    };
    Ok(object.zone == Zone::Battlefield
        && filter
            .required_types
            .iter()
            .all(|required| object.card_types.contains(required))
        && (filter.any_types.is_empty()
            || filter
                .any_types
                .iter()
                .any(|required| object.card_types.contains(required)))
        && filter
            .excluded_types
            .iter()
            .all(|excluded| !object.card_types.contains(excluded))
        && filter.required_supertypes.iter().all(|supertype| {
            object
                .supertypes
                .iter()
                .any(|actual| actual.eq_ignore_ascii_case(supertype))
        })
        && filter
            .required_colors
            .iter()
            .all(|color| object.colors.contains(color))
        && filter
            .excluded_colors
            .iter()
            .all(|color| !object.colors.contains(color))
        && filter.required_subtypes.iter().all(|subtype| {
            object
                .subtypes
                .iter()
                .any(|actual| actual.eq_ignore_ascii_case(subtype))
        })
        && controller_matches
        && (!filter.other_than_source || Some(object.reference) != source)
        && filter.token.is_none_or(|token| object.is_token == token)
        && filter
            .attacking
            .is_none_or(|attacking| object.attacking == attacking)
        && filter
            .blocking
            .is_none_or(|blocking| object.blocking == blocking)
        && filter.tapped.is_none_or(|tapped| object.tapped == tapped))
}

fn card_matches_filter(object: &GameObject, filter: &ObjectFilter) -> bool {
    filter
        .required_types
        .iter()
        .all(|required| object.card_types.contains(required))
        && (filter.any_types.is_empty()
            || filter
                .any_types
                .iter()
                .any(|required| object.card_types.contains(required)))
        && filter
            .excluded_types
            .iter()
            .all(|excluded| !object.card_types.contains(excluded))
        && filter.required_supertypes.iter().all(|supertype| {
            object
                .supertypes
                .iter()
                .any(|actual| actual.eq_ignore_ascii_case(supertype))
        })
        && filter
            .required_colors
            .iter()
            .all(|color| object.colors.contains(color))
        && filter
            .excluded_colors
            .iter()
            .all(|color| !object.colors.contains(color))
        && filter.required_subtypes.iter().all(|subtype| {
            object
                .subtypes
                .iter()
                .any(|actual| actual.eq_ignore_ascii_case(subtype))
        })
        && filter.token.is_none_or(|token| object.is_token == token)
        && filter.attacking.is_none()
        && filter.blocking.is_none()
        && filter.tapped.is_none()
}

fn validate_cardinality(
    cardinality: Cardinality,
    actual: usize,
    action: ActionId,
) -> Result<(), OracleActionRuntimeError> {
    let valid = match cardinality {
        Cardinality::ExactlyOne => actual == 1,
        Cardinality::Exactly(amount) => usize::try_from(amount).ok() == Some(actual),
        Cardinality::UpTo(amount) => usize::try_from(amount).is_ok_and(|amount| actual <= amount),
        Cardinality::AnyNumber | Cardinality::All => true,
    };
    if valid {
        Ok(())
    } else {
        Err(OracleActionRuntimeError::WrongTargetCardinality {
            action,
            expected: cardinality,
            actual,
        })
    }
}

fn resolve_card_selection(
    selection: &CardSelection,
    action: ActionId,
    acting_player: PlayerId,
    bindings: &OracleActionBindings,
    state: &OracleActionWorldState,
) -> Result<Vec<ObjectRef>, OracleActionRuntimeError> {
    resolve_card_selection_with_policy(
        selection,
        action,
        acting_player,
        bindings,
        state,
        CardSelectionPolicy::Exact,
    )
}

fn resolve_card_selection_with_policy(
    selection: &CardSelection,
    action: ActionId,
    acting_player: PlayerId,
    bindings: &OracleActionBindings,
    state: &OracleActionWorldState,
    policy: CardSelectionPolicy,
) -> Result<Vec<ObjectRef>, OracleActionRuntimeError> {
    if selection.random && selection.from_top {
        return Err(OracleActionRuntimeError::StateInvariantViolation);
    }
    let zone_player = resolve_zone_owner_player(
        selection.zone.owner,
        acting_player,
        Some(acting_player),
        None,
        state,
    )?;
    let available = objects_in_zone(state, zone_player, selection.zone.zone);
    if selection.cardinality == Cardinality::All {
        return Ok(available
            .into_iter()
            .filter(|object| {
                state
                    .objects
                    .get(object)
                    .is_some_and(|object| card_matches_filter(object, &selection.filter))
            })
            .collect());
    }
    if selection.from_top {
        let amount = match selection.cardinality {
            Cardinality::ExactlyOne => 1usize,
            Cardinality::Exactly(amount) | Cardinality::UpTo(amount) => {
                usize::try_from(amount).map_err(|_| OracleActionRuntimeError::AmountOverflow)?
            }
            Cardinality::AnyNumber | Cardinality::All => {
                return Err(OracleActionRuntimeError::StateInvariantViolation);
            }
        };
        let library = state
            .libraries
            .get(&zone_player)
            .ok_or(OracleActionRuntimeError::MissingPlayer(zone_player))?;
        let start = library.len().saturating_sub(amount);
        let chosen = library[start..].iter().rev().copied().collect::<Vec<_>>();
        if matches!(
            selection.cardinality,
            Cardinality::Exactly(_) | Cardinality::ExactlyOne
        ) && chosen.len() != amount
        {
            return Err(OracleActionRuntimeError::WrongTargetCardinality {
                action,
                expected: selection.cardinality,
                actual: chosen.len(),
            });
        }
        return Ok(chosen);
    }
    let matching_available = available
        .iter()
        .filter(|object| {
            state
                .objects
                .get(object)
                .is_some_and(|object| card_matches_filter(object, &selection.filter))
        })
        .count();
    let effective_cardinality = match policy {
        CardSelectionPolicy::Exact => selection.cardinality,
        CardSelectionPolicy::HiddenSearch => match selection.cardinality {
            Cardinality::ExactlyOne => Cardinality::UpTo(1),
            Cardinality::Exactly(amount) => Cardinality::UpTo(amount),
            other => other,
        },
        CardSelectionPolicy::AsMuchAsPossible => match selection.cardinality {
            Cardinality::ExactlyOne => Cardinality::Exactly(
                u32::try_from(matching_available.min(1))
                    .map_err(|_| OracleActionRuntimeError::AmountOverflow)?,
            ),
            Cardinality::Exactly(amount) => Cardinality::Exactly(
                u32::try_from(
                    matching_available.min(
                        usize::try_from(amount)
                            .map_err(|_| OracleActionRuntimeError::AmountOverflow)?,
                    ),
                )
                .map_err(|_| OracleActionRuntimeError::AmountOverflow)?,
            ),
            other => other,
        },
    };
    let chosen = if effective_cardinality == Cardinality::Exactly(0) {
        Vec::new()
    } else if selection.random {
        bindings
            .random_card_outcomes
            .get(&(action, acting_player))
            .cloned()
            .ok_or(OracleActionRuntimeError::MissingRandomOutcome(action))?
    } else {
        bindings
            .card_choices
            .get(&(action, acting_player))
            .cloned()
            .or_else(|| {
                matches!(
                    effective_cardinality,
                    Cardinality::UpTo(_) | Cardinality::AnyNumber | Cardinality::Exactly(0)
                )
                .then(Vec::new)
            })
            .ok_or(OracleActionRuntimeError::MissingChoice(action))?
    };
    validate_cardinality(effective_cardinality, chosen.len(), action)?;
    let available_set = available.into_iter().collect::<BTreeSet<_>>();
    let mut distinct = BTreeSet::new();
    for object in &chosen {
        let state_object = state
            .objects
            .get(object)
            .ok_or(OracleActionRuntimeError::MissingObject(*object))?;
        if !distinct.insert(*object)
            || !available_set.contains(object)
            || !card_matches_filter(state_object, &selection.filter)
        {
            return Err(OracleActionRuntimeError::IllegalCardChoice {
                action,
                object: *object,
            });
        }
    }
    Ok(chosen)
}

fn resolve_visibility_selection(
    selection: &CardSelection,
    player: PlayerId,
    state: &OracleActionWorldState,
) -> Result<Vec<ObjectRef>, OracleActionRuntimeError> {
    let zone_player = resolve_zone_owner_player(selection.zone.owner, player, None, None, state)?;
    let available = objects_in_zone(state, zone_player, selection.zone.zone);
    if selection.cardinality == Cardinality::All {
        return Ok(available);
    }
    if !selection.from_top {
        return Err(OracleActionRuntimeError::StateInvariantViolation);
    }
    let amount = match selection.cardinality {
        Cardinality::ExactlyOne => 1,
        Cardinality::Exactly(amount) | Cardinality::UpTo(amount) => {
            usize::try_from(amount).map_err(|_| OracleActionRuntimeError::AmountOverflow)?
        }
        _ => return Err(OracleActionRuntimeError::StateInvariantViolation),
    };
    let library = state
        .libraries
        .get(&zone_player)
        .ok_or(OracleActionRuntimeError::MissingPlayer(zone_player))?;
    Ok(library.iter().rev().take(amount).copied().collect())
}

fn objects_in_zone(state: &OracleActionWorldState, player: PlayerId, zone: Zone) -> Vec<ObjectRef> {
    if zone == Zone::Library {
        return state.libraries.get(&player).cloned().unwrap_or_default();
    }
    state
        .objects
        .values()
        .filter(|object| object.owner == player && object.zone == zone)
        .map(|object| object.reference)
        .collect()
}

fn resolve_amount(
    amount: &Amount,
    action: ActionId,
    bindings: &OracleActionBindings,
    state: &OracleActionWorldState,
    memory: &ExecutionMemory,
) -> Result<u32, OracleActionRuntimeError> {
    match amount {
        Amount::Fixed(amount) => Ok(*amount),
        Amount::Variable(VariableAmount::ThatMany) => {
            memory
                .last_amount
                .ok_or(OracleActionRuntimeError::MissingVariableAmount {
                    action,
                    variable: VariableAmount::ThatMany,
                })
        }
        Amount::Variable(VariableAmount::NumberOfSelectedObjects) => {
            u32::try_from(memory.last_objects.len())
                .map_err(|_| OracleActionRuntimeError::AmountOverflow)
        }
        Amount::Variable(VariableAmount::SourcePower) => {
            let source = bindings
                .source
                .ok_or(OracleActionRuntimeError::MissingSource)?;
            u32::try_from(current_power(source, state)?.max(0))
                .map_err(|_| OracleActionRuntimeError::AmountOverflow)
        }
        Amount::Variable(VariableAmount::SourceToughness) => {
            let source = bindings
                .source
                .ok_or(OracleActionRuntimeError::MissingSource)?;
            let toughness = current_toughness(source, state)?;
            u32::try_from(toughness.max(0)).map_err(|_| OracleActionRuntimeError::AmountOverflow)
        }
        Amount::Variable(variable) => bindings.variable_amounts.get(variable).copied().ok_or(
            OracleActionRuntimeError::MissingVariableAmount {
                action,
                variable: *variable,
            },
        ),
    }
}

fn resolve_damage_source(
    source: &DamageSource,
    action: ActionId,
    bindings: &OracleActionBindings,
    state: &OracleActionWorldState,
    memory: &ExecutionMemory,
) -> Result<Option<ObjectRef>, OracleActionRuntimeError> {
    match source {
        DamageSource::SourceObject | DamageSource::ThisSpell => bindings
            .source
            .map(Some)
            .ok_or(OracleActionRuntimeError::MissingSource),
        DamageSource::Object(operand) => resolve_objects(operand, action, bindings, state, memory)?
            .into_iter()
            .next()
            .map(Some)
            .ok_or(OracleActionRuntimeError::MissingTarget(action)),
    }
}

fn resolve_damage_recipients(
    recipient: &DamageRecipient,
    action: ActionId,
    bindings: &OracleActionBindings,
    state: &OracleActionWorldState,
    memory: &ExecutionMemory,
) -> Result<Vec<ResolvedDamageRecipient>, OracleActionRuntimeError> {
    match recipient {
        DamageRecipient::Player(player) => {
            resolve_players(*player, action, bindings, state, memory).map(|players| {
                players
                    .into_iter()
                    .map(ResolvedDamageRecipient::Player)
                    .collect()
            })
        }
        DamageRecipient::Object(object) => resolve_objects(object, action, bindings, state, memory)
            .map(|objects| {
                objects
                    .into_iter()
                    .map(ResolvedDamageRecipient::Object)
                    .collect()
            }),
        DamageRecipient::AnyTarget => {
            let target = bindings
                .any_targets
                .get(&action)
                .copied()
                .ok_or(OracleActionRuntimeError::MissingAnyTarget(action))?;
            match target {
                ResolvedAnyTarget::Player(player) if state.players.contains_key(&player) => {
                    Ok(vec![ResolvedDamageRecipient::Player(player)])
                }
                ResolvedAnyTarget::Object(object)
                    if state.objects.get(&object).is_some_and(|object| {
                        object.is_permanent()
                            && object.card_types.iter().any(|card_type| {
                                matches!(
                                    card_type,
                                    CardType::Creature | CardType::Planeswalker | CardType::Battle
                                )
                            })
                    }) =>
                {
                    Ok(vec![ResolvedDamageRecipient::Object(object)])
                }
                _ => Err(OracleActionRuntimeError::IllegalAnyTarget(action)),
            }
        }
    }
}

fn build_damage_event(
    action_id: ActionId,
    source: Option<ObjectRef>,
    recipient: ResolvedDamageRecipient,
    amount: u32,
    state: &OracleActionWorldState,
    _program_digest: &str,
) -> Result<DamageEvent, OracleActionRuntimeError> {
    let keywords = source
        .map(|source| current_keywords(source, state))
        .transpose()?
        .unwrap_or_default();
    Ok(DamageEvent {
        action_id,
        source,
        recipient,
        amount,
        source_had_deathtouch: keywords.contains(&KeywordAbility::Deathtouch),
        source_had_lifelink: keywords.contains(&KeywordAbility::Lifelink),
        source_had_infect: keywords.contains(&KeywordAbility::Infect),
        source_had_wither: keywords.contains(&KeywordAbility::Wither),
    })
}

fn apply_damage_batch(
    events: &[DamageEvent],
    state: &mut OracleActionWorldState,
) -> Result<(), OracleActionRuntimeError> {
    let mut life_deltas = BTreeMap::<PlayerId, i64>::new();
    let mut poison_deltas = BTreeMap::<PlayerId, u32>::new();
    let mut marked_damage = BTreeMap::<ObjectRef, u32>::new();
    let mut minus_one_counters = BTreeMap::<ObjectRef, u32>::new();
    let mut deathtouch_damage = BTreeSet::<ObjectRef>::new();
    let mut loyalty_damage = BTreeMap::<ObjectRef, u32>::new();
    let mut defense_damage = BTreeMap::<ObjectRef, u32>::new();
    for event in events {
        match event.recipient {
            ResolvedDamageRecipient::Player(player) => {
                if !state.players.contains_key(&player) {
                    return Err(OracleActionRuntimeError::MissingPlayer(player));
                }
                if event.source_had_infect {
                    let entry = poison_deltas.entry(player).or_default();
                    *entry = entry
                        .checked_add(event.amount)
                        .ok_or(OracleActionRuntimeError::CounterOverflow)?;
                } else {
                    let delta = -i64::from(event.amount);
                    let entry = life_deltas.entry(player).or_default();
                    *entry = (*entry)
                        .checked_add(delta)
                        .ok_or(OracleActionRuntimeError::LifeOverflow)?;
                }
            }
            ResolvedDamageRecipient::Object(object) => {
                let state_object = state
                    .objects
                    .get(&object)
                    .ok_or(OracleActionRuntimeError::MissingObject(object))?;
                if !state_object.is_permanent() {
                    return Err(OracleActionRuntimeError::ObjectNotOnBattlefield(object));
                }
                if state_object.card_types.contains(&CardType::Creature) {
                    if event.source_had_infect || event.source_had_wither {
                        let entry = minus_one_counters.entry(object).or_default();
                        *entry = entry
                            .checked_add(event.amount)
                            .ok_or(OracleActionRuntimeError::CounterOverflow)?;
                    } else {
                        let entry = marked_damage.entry(object).or_default();
                        *entry = entry
                            .checked_add(event.amount)
                            .ok_or(OracleActionRuntimeError::AmountOverflow)?;
                    }
                    if event.source_had_deathtouch && event.amount > 0 {
                        deathtouch_damage.insert(object);
                    }
                }
                if state_object.card_types.contains(&CardType::Planeswalker) {
                    let entry = loyalty_damage.entry(object).or_default();
                    *entry = entry
                        .checked_add(event.amount)
                        .ok_or(OracleActionRuntimeError::AmountOverflow)?;
                }
                if state_object.card_types.contains(&CardType::Battle) {
                    let entry = defense_damage.entry(object).or_default();
                    *entry = entry
                        .checked_add(event.amount)
                        .ok_or(OracleActionRuntimeError::AmountOverflow)?;
                }
            }
        }
        if event.source_had_lifelink && event.amount > 0 {
            let source = event
                .source
                .ok_or(OracleActionRuntimeError::MissingSource)?;
            let controller = state
                .objects
                .get(&source)
                .ok_or(OracleActionRuntimeError::MissingObject(source))?
                .controller;
            let entry = life_deltas.entry(controller).or_default();
            *entry = (*entry)
                .checked_add(i64::from(event.amount))
                .ok_or(OracleActionRuntimeError::LifeOverflow)?;
        }
    }
    for (player, delta) in life_deltas {
        let player_state = state
            .players
            .get_mut(&player)
            .ok_or(OracleActionRuntimeError::MissingPlayer(player))?;
        player_state.life = player_state
            .life
            .checked_add(delta)
            .ok_or(OracleActionRuntimeError::LifeOverflow)?;
    }
    for (player, amount) in poison_deltas {
        let player_state = state
            .players
            .get_mut(&player)
            .ok_or(OracleActionRuntimeError::MissingPlayer(player))?;
        player_state.poison_counters = player_state
            .poison_counters
            .checked_add(amount)
            .ok_or(OracleActionRuntimeError::CounterOverflow)?;
    }
    for (object, damage) in marked_damage {
        let state_object = state
            .objects
            .get_mut(&object)
            .ok_or(OracleActionRuntimeError::MissingObject(object))?;
        state_object.marked_damage = state_object
            .marked_damage
            .checked_add(damage)
            .ok_or(OracleActionRuntimeError::AmountOverflow)?;
    }
    for (object, amount) in minus_one_counters {
        let state_object = state
            .objects
            .get_mut(&object)
            .ok_or(OracleActionRuntimeError::MissingObject(object))?;
        let current = state_object.counters.get("-1/-1").copied().unwrap_or(0);
        state_object.counters.insert(
            "-1/-1".to_owned(),
            current
                .checked_add(amount)
                .ok_or(OracleActionRuntimeError::CounterOverflow)?,
        );
    }
    for object in deathtouch_damage {
        state
            .objects
            .get_mut(&object)
            .ok_or(OracleActionRuntimeError::MissingObject(object))?
            .deathtouch_damage = true;
    }
    for (object, amount) in loyalty_damage {
        remove_damage_counters(object, "loyalty", amount, state)?;
    }
    for (object, amount) in defense_damage {
        remove_damage_counters(object, "defense", amount, state)?;
    }
    Ok(())
}

fn remove_damage_counters(
    object: ObjectRef,
    counter: &str,
    amount: u32,
    state: &mut OracleActionWorldState,
) -> Result<(), OracleActionRuntimeError> {
    let state_object = state
        .objects
        .get_mut(&object)
        .ok_or(OracleActionRuntimeError::MissingObject(object))?;
    let current = state_object.counters.get(counter).copied().unwrap_or(0);
    let remaining = current.saturating_sub(amount);
    if remaining == 0 {
        state_object.counters.remove(counter);
    } else {
        state_object.counters.insert(counter.to_owned(), remaining);
    }
    Ok(())
}

fn current_power(
    object: ObjectRef,
    state: &OracleActionWorldState,
) -> Result<i32, OracleActionRuntimeError> {
    let state_object = state
        .objects
        .get(&object)
        .ok_or(OracleActionRuntimeError::MissingObject(object))?;
    let mut power = state_object
        .base_power
        .ok_or(OracleActionRuntimeError::MissingPower(object))?;
    for (counter, amount) in &state_object.counters {
        if let Some((power_modifier, _)) = parse_power_toughness_counter(counter) {
            let count =
                i32::try_from(*amount).map_err(|_| OracleActionRuntimeError::AmountOverflow)?;
            power = power
                .checked_add(
                    power_modifier
                        .checked_mul(count)
                        .ok_or(OracleActionRuntimeError::AmountOverflow)?,
                )
                .ok_or(OracleActionRuntimeError::AmountOverflow)?;
        }
    }
    for effect in &state.continuous_effects {
        if let ContinuousActionEffect::PowerToughness {
            objects,
            power: modifier,
            ..
        } = effect
            && objects.contains(&object)
        {
            power = power
                .checked_add(*modifier)
                .ok_or(OracleActionRuntimeError::AmountOverflow)?;
        }
    }
    Ok(power)
}

fn current_toughness(
    object: ObjectRef,
    state: &OracleActionWorldState,
) -> Result<i32, OracleActionRuntimeError> {
    let state_object = state
        .objects
        .get(&object)
        .ok_or(OracleActionRuntimeError::MissingObject(object))?;
    let mut toughness = state_object
        .base_toughness
        .ok_or(OracleActionRuntimeError::MissingPower(object))?;
    for (counter, amount) in &state_object.counters {
        if let Some((_, toughness_modifier)) = parse_power_toughness_counter(counter) {
            let count =
                i32::try_from(*amount).map_err(|_| OracleActionRuntimeError::AmountOverflow)?;
            toughness = toughness
                .checked_add(
                    toughness_modifier
                        .checked_mul(count)
                        .ok_or(OracleActionRuntimeError::AmountOverflow)?,
                )
                .ok_or(OracleActionRuntimeError::AmountOverflow)?;
        }
    }
    for effect in &state.continuous_effects {
        if let ContinuousActionEffect::PowerToughness {
            objects,
            toughness: modifier,
            ..
        } = effect
            && objects.contains(&object)
        {
            toughness = toughness
                .checked_add(*modifier)
                .ok_or(OracleActionRuntimeError::AmountOverflow)?;
        }
    }
    Ok(toughness)
}

fn current_keywords(
    object: ObjectRef,
    state: &OracleActionWorldState,
) -> Result<BTreeSet<KeywordAbility>, OracleActionRuntimeError> {
    let state_object = state
        .objects
        .get(&object)
        .ok_or(OracleActionRuntimeError::MissingObject(object))?;
    let mut keywords = state_object.intrinsic_keywords.clone();
    for (counter, amount) in &state_object.counters {
        if *amount > 0
            && let Some(keyword) = keyword_counter_ability(counter)
        {
            keywords.insert(keyword);
        }
    }
    for effect in &state.continuous_effects {
        if let ContinuousActionEffect::Keywords {
            objects,
            operation,
            keywords: changed,
            ..
        } = effect
            && objects.contains(&object)
        {
            match operation {
                KeywordOperation::Grant => keywords.extend(changed),
                KeywordOperation::Lose => {
                    keywords.retain(|keyword| !changed.contains(keyword));
                }
            }
        }
    }
    Ok(keywords)
}

fn parse_power_toughness_counter(counter: &str) -> Option<(i32, i32)> {
    let (power, toughness) = counter.split_once('/')?;
    if !matches!(power.as_bytes().first(), Some(b'+') | Some(b'-'))
        || !matches!(toughness.as_bytes().first(), Some(b'+') | Some(b'-'))
    {
        return None;
    }
    Some((power.parse().ok()?, toughness.parse().ok()?))
}

fn keyword_counter_ability(counter: &str) -> Option<KeywordAbility> {
    match counter {
        "deathtouch" => Some(KeywordAbility::Deathtouch),
        "defender" => Some(KeywordAbility::Defender),
        "double strike" => Some(KeywordAbility::DoubleStrike),
        "first strike" => Some(KeywordAbility::FirstStrike),
        "flying" => Some(KeywordAbility::Flying),
        "haste" => Some(KeywordAbility::Haste),
        "hexproof" => Some(KeywordAbility::Hexproof),
        "indestructible" => Some(KeywordAbility::Indestructible),
        "infect" => Some(KeywordAbility::Infect),
        "lifelink" => Some(KeywordAbility::Lifelink),
        "menace" => Some(KeywordAbility::Menace),
        "reach" => Some(KeywordAbility::Reach),
        "trample" => Some(KeywordAbility::Trample),
        "vigilance" => Some(KeywordAbility::Vigilance),
        "wither" => Some(KeywordAbility::Wither),
        _ => None,
    }
}

fn create_token(
    controller: PlayerId,
    template: &TokenTemplate,
    state: &mut OracleActionWorldState,
) -> Result<ObjectRef, OracleActionRuntimeError> {
    if !state.players.contains_key(&controller) {
        return Err(OracleActionRuntimeError::MissingPlayer(controller));
    }
    let object_id = allocate_object_id(state)?;
    let reference = ObjectRef {
        object_id,
        incarnation_id: IncarnationId(1),
    };
    let copiable_values = CopiableValues {
        name: template.name.clone(),
        mana_cost: None,
        card_types: template.card_types.clone(),
        supertypes: BTreeSet::new(),
        colors: template.colors.clone(),
        subtypes: template.subtypes.clone(),
        base_power: template.power,
        base_toughness: template.toughness,
        base_loyalty: None,
        base_defense: None,
        intrinsic_keywords: template.keywords.clone(),
        ability_semantic_ids: BTreeSet::new(),
        copiable_choices: BTreeMap::new(),
    };
    state.objects.insert(
        reference,
        GameObject {
            reference,
            owner: controller,
            controller,
            zone: Zone::Battlefield,
            card_types: template.card_types.clone(),
            supertypes: BTreeSet::new(),
            colors: template.colors.clone(),
            subtypes: template.subtypes.clone(),
            base_power: template.power,
            base_toughness: template.toughness,
            base_loyalty: None,
            base_defense: None,
            tapped: template.tapped,
            attacking: template.attacking,
            blocking: false,
            marked_damage: 0,
            deathtouch_damage: false,
            counters: BTreeMap::new(),
            intrinsic_keywords: template.keywords.clone(),
            is_token: true,
            copiable_values: Some(copiable_values),
        },
    );
    Ok(reference)
}

fn create_copy_token(
    controller: PlayerId,
    source: &CopiableValues,
    tapped: bool,
    state: &mut OracleActionWorldState,
) -> Result<ObjectRef, OracleActionRuntimeError> {
    if !state.players.contains_key(&controller) {
        return Err(OracleActionRuntimeError::MissingPlayer(controller));
    }
    let object_id = allocate_object_id(state)?;
    let reference = ObjectRef {
        object_id,
        incarnation_id: IncarnationId(1),
    };
    state.objects.insert(
        reference,
        GameObject {
            reference,
            owner: controller,
            controller,
            zone: Zone::Battlefield,
            card_types: source.card_types.clone(),
            supertypes: source.supertypes.clone(),
            colors: source.colors.clone(),
            subtypes: source.subtypes.clone(),
            base_power: source.base_power,
            base_toughness: source.base_toughness,
            base_loyalty: source.base_loyalty,
            base_defense: source.base_defense,
            tapped,
            attacking: false,
            blocking: false,
            marked_damage: 0,
            deathtouch_damage: false,
            counters: BTreeMap::new(),
            intrinsic_keywords: source.intrinsic_keywords.clone(),
            is_token: true,
            copiable_values: Some(source.clone()),
        },
    );
    Ok(reference)
}

fn allocate_object_id(
    state: &mut OracleActionWorldState,
) -> Result<ObjectId, OracleActionRuntimeError> {
    let object_id = state.next_object_id;
    state.next_object_id = state
        .next_object_id
        .checked_add(1)
        .ok_or(OracleActionRuntimeError::ObjectIdOverflow)?;
    Ok(object_id)
}

fn move_object_to_zone(
    action: ActionId,
    object: ObjectRef,
    destination: ZoneOperand,
    position: Option<LibraryPosition>,
    tapped: bool,
    bindings: &OracleActionBindings,
    state: &mut OracleActionWorldState,
) -> Result<ZoneMoveEvent, OracleActionRuntimeError> {
    let mut state_object = state
        .objects
        .remove(&object)
        .ok_or(OracleActionRuntimeError::MissingObject(object))?;
    if state_object.reference != object {
        return Err(OracleActionRuntimeError::StateInvariantViolation);
    }
    if state_object.zone == Zone::Library {
        let library = state
            .libraries
            .get_mut(&state_object.owner)
            .ok_or(OracleActionRuntimeError::MissingPlayer(state_object.owner))?;
        if let Some(index) = library.iter().position(|candidate| *candidate == object) {
            library.remove(index);
        }
    }
    let requested_destination_player = resolve_zone_owner_player(
        destination.owner,
        bindings.controller,
        bindings.that_player,
        Some(&state_object),
        state,
    )?;
    // A card can only enter its owner's hand, graveyard, or library. The rules
    // redirect an instruction naming another player's corresponding zone.
    let destination_player = if matches!(
        destination.zone,
        Zone::Hand | Zone::Graveyard | Zone::Library
    ) {
        state_object.owner
    } else {
        requested_destination_player
    };

    let next_incarnation = state_object
        .reference
        .incarnation_id
        .0
        .checked_add(1)
        .ok_or(OracleActionRuntimeError::IncarnationOverflow(object))?;
    let new_reference = ObjectRef {
        object_id: object.object_id,
        incarnation_id: IncarnationId(next_incarnation),
    };
    let from = state_object.zone;
    let old_owner = state_object.owner;
    let old_controller = state_object.controller;
    let last_known_copiable_values = state_object.copiable_values.clone();
    state_object.reference = new_reference;
    state_object.zone = destination.zone;
    state_object.tapped = destination.zone == Zone::Battlefield && tapped;
    state_object.attacking = false;
    state_object.blocking = false;
    state_object.marked_damage = 0;
    state_object.deathtouch_damage = false;
    state_object.counters.clear();
    if destination.zone == Zone::Battlefield {
        state_object.controller = destination_player;
    } else {
        state_object.controller = state_object.owner;
    }
    state.objects.insert(new_reference, state_object);

    if destination.zone == Zone::Library {
        let library = state
            .libraries
            .get_mut(&destination_player)
            .ok_or(OracleActionRuntimeError::MissingPlayer(destination_player))?;
        match position.unwrap_or(LibraryPosition::Top) {
            LibraryPosition::Top => library.push(new_reference),
            LibraryPosition::Bottom => library.insert(0, new_reference),
            LibraryPosition::Shuffled => {
                return Err(OracleActionRuntimeError::StateInvariantViolation);
            }
        }
    }
    let event = ZoneMoveEvent {
        action_id: action,
        old_reference: object,
        new_reference,
        from,
        to: destination.zone,
        old_owner,
        old_controller,
        last_known_copiable_values,
    };
    state.zone_moves.push(event.clone());
    Ok(event)
}

fn resolve_zone_owner_player(
    owner: ZoneOwner,
    acting_player: PlayerId,
    that_player: Option<PlayerId>,
    object: Option<&GameObject>,
    state: &OracleActionWorldState,
) -> Result<PlayerId, OracleActionRuntimeError> {
    let player = match owner {
        ZoneOwner::You => acting_player,
        ZoneOwner::TargetPlayer => acting_player,
        ZoneOwner::ThatPlayer => that_player.ok_or(OracleActionRuntimeError::MissingThatPlayer)?,
        ZoneOwner::ObjectOwner => object
            .map(|object| object.owner)
            .ok_or(OracleActionRuntimeError::MissingThatObject)?,
        ZoneOwner::ObjectController => object
            .map(|object| object.controller)
            .unwrap_or(acting_player),
    };
    state
        .players
        .contains_key(&player)
        .then_some(player)
        .ok_or(OracleActionRuntimeError::MissingPlayer(player))
}

fn apply_shuffle(
    action: ActionId,
    player: PlayerId,
    bindings: &OracleActionBindings,
    state: &mut OracleActionWorldState,
) -> Result<(), OracleActionRuntimeError> {
    let supplied = bindings
        .shuffle_orders
        .get(&(action, player))
        .cloned()
        .ok_or(OracleActionRuntimeError::MissingChoice(action))?;
    let library = state
        .libraries
        .get_mut(&player)
        .ok_or(OracleActionRuntimeError::MissingPlayer(player))?;
    let current_set = library.iter().copied().collect::<BTreeSet<_>>();
    let supplied_set = supplied.iter().copied().collect::<BTreeSet<_>>();
    if supplied.len() != library.len()
        || supplied_set.len() != supplied.len()
        || supplied_set != current_set
    {
        return Err(OracleActionRuntimeError::InvalidShuffleOrder { action, player });
    }
    *library = supplied;
    Ok(())
}

fn evaluate_predicate(
    predicate: &StatePredicate,
    bindings: &OracleActionBindings,
    state: &OracleActionWorldState,
    memory: &ExecutionMemory,
) -> Result<bool, OracleActionRuntimeError> {
    match predicate {
        StatePredicate::PreviousActionSucceeded => Ok(memory.previous_action_succeeded),
        StatePredicate::YouControl { filter, comparison } => {
            require_battlefield(state)?;
            let mut count = 0usize;
            for object in state.objects.values() {
                if object_matches_filter(
                    object,
                    filter,
                    bindings.controller,
                    bindings.source,
                    state,
                )? {
                    count += 1;
                }
            }
            compare_count(count, comparison)
        }
        StatePredicate::YourHandIsEmpty => {
            require_hidden_zones(state)?;
            Ok(objects_in_zone(state, bindings.controller, Zone::Hand).is_empty())
        }
        StatePredicate::YourGraveyardHas { filter, comparison } => {
            require_hidden_zones(state)?;
            let count = objects_in_zone(state, bindings.controller, Zone::Graveyard)
                .into_iter()
                .filter(|object| {
                    state
                        .objects
                        .get(object)
                        .is_some_and(|object| card_matches_filter(object, filter))
                })
                .count();
            compare_count(count, comparison)
        }
    }
}

fn compare_count(
    actual: usize,
    comparison: &CountComparison,
) -> Result<bool, OracleActionRuntimeError> {
    let actual = u32::try_from(actual).map_err(|_| OracleActionRuntimeError::AmountOverflow)?;
    Ok(match comparison {
        CountComparison::Exactly(expected) => actual == *expected,
        CountComparison::AtLeast(expected) => actual >= *expected,
        CountComparison::AtMost(expected) => actual <= *expected,
    })
}

/// Expire temporary effects at the matching rules boundary. Permanent effects
/// remain, and effects tied to a stale source incarnation are removed.
pub fn expire_oracle_action_effects(boundary: Duration, state: &mut OracleActionWorldState) {
    let existing_objects = state.objects.keys().copied().collect::<BTreeSet<_>>();
    state.continuous_effects.retain(|effect| {
        let (duration, source) = match effect {
            ContinuousActionEffect::PowerToughness {
                duration, source, ..
            }
            | ContinuousActionEffect::Keywords {
                duration, source, ..
            } => (*duration, *source),
        };
        let source_still_exists = source.is_none_or(|source| existing_objects.contains(&source));
        duration != boundary
            && (duration != Duration::UntilSourceLeavesBattlefield || source_still_exists)
    });
}
