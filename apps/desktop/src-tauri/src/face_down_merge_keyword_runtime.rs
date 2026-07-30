//! Content-addressed rules for Morph, Megamorph, Disguise, and Mutate.
//!
//! This module deliberately owns only complete, reviewed singleton Oracle
//! clauses. It models the hidden face-down cast and reveal lifecycle and the
//! merged-permanent lifecycle without claiming that the production simulator
//! consumes either program yet.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const FACE_DOWN_MERGE_KEYWORD_COMPILER_VERSION: &str = "face-down-merge-keyword-compiler-0.1";
pub const FACE_DOWN_MERGE_KEYWORD_RUNTIME_VERSION: &str = "face-down-merge-keyword-runtime-0.1";
pub const FACE_DOWN_MERGE_KEYWORD_RULES_CONTEXT_VERSION: &str =
    "magic-comprehensive-rules-2026-06-19:108.2,118,601,608.3,702.37,702.168,702.140,708,727,903.3";

const FACE_DOWN_CAST_COST: &str = "{3}";
const DISGUISE_WARD_COST: &str = "{2}";
const MORPH_REMINDER: &str = "You may cast this card face down as a 2/2 creature for {3}. Turn it face up any time for its morph cost.";
const MEGAMORPH_REMINDER: &str = "You may cast this card face down as a 2/2 creature for {3}. Turn it face up any time for its megamorph cost and put a +1/+1 counter on it.";
const DISGUISE_REMINDER: &str = "You may cast this card face down for {3} as a 2/2 creature with ward {2}. Turn it face up any time for its disguise cost.";
const MUTATE_REMINDER: &str = "If you cast this spell for its mutate cost, put it over or under target non-Human creature you own. They mutate into the creature on top plus all abilities from under it.";

pub type AbilityInstanceId = u64;
pub type AttachmentId = u64;
pub type CardComponentId = u64;
pub type CastEventId = u64;
pub type CommanderId = u64;
pub type EventId = u64;
pub type IncarnationId = u64;
pub type ObjectId = u64;
pub type PlayerId = u8;
pub type StackObjectId = u64;
pub type TriggerId = u64;

/// No production path may treat these programs as executable until the main
/// engine supplies the complete hidden-information and merged-object state
/// required by the transactions below.
pub const fn face_down_merge_keyword_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManaSymbol {
    Generic(u32),
    Colored(ManaColor),
    Colorless,
    VariableX,
    Hybrid(ManaColor, ManaColor),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaCost {
    exact: String,
    symbols: Vec<ManaSymbol>,
}

impl ManaCost {
    pub fn exact(&self) -> &str {
        &self.exact
    }

    pub fn symbols(&self) -> &[ManaSymbol] {
        &self.symbols
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardFilter {
    Any,
    Color(ManaColor),
    Subtype(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermanentFilter {
    AnotherCreature,
    Subtype(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnFaceUpCost {
    Mana(ManaCost),
    DiscardFromHand { count: u32, filter: CardFilter },
    PayLife(u32),
    ReturnControlledToOwnersHand { count: u32, filter: PermanentFilter },
    RevealFromHand { count: u32, filter: CardFilter },
    SacrificeControlled { count: u32, filter: PermanentFilter },
}

impl TurnFaceUpCost {
    fn stable_contract(&self) -> String {
        match self {
            Self::Mana(cost) => format!("mana:{}", cost.exact),
            Self::DiscardFromHand { count, filter } => {
                format!("discard:{count}:{}", card_filter_contract(filter))
            }
            Self::PayLife(amount) => format!("pay-life:{amount}"),
            Self::ReturnControlledToOwnersHand { count, filter } => {
                format!(
                    "return-controlled-to-owner-hand:{count}:{}",
                    permanent_filter_contract(filter)
                )
            }
            Self::RevealFromHand { count, filter } => {
                format!("reveal-from-hand:{count}:{}", card_filter_contract(filter))
            }
            Self::SacrificeControlled { count, filter } => {
                format!(
                    "sacrifice-controlled:{count}:{}",
                    permanent_filter_contract(filter)
                )
            }
        }
    }
}

fn card_filter_contract(filter: &CardFilter) -> String {
    match filter {
        CardFilter::Any => "any-card".to_owned(),
        CardFilter::Color(color) => format!("color:{}", color_contract(*color)),
        CardFilter::Subtype(subtype) => format!("subtype:{subtype}"),
    }
}

fn permanent_filter_contract(filter: &PermanentFilter) -> String {
    match filter {
        PermanentFilter::AnotherCreature => "another-creature".to_owned(),
        PermanentFilter::Subtype(subtype) => format!("subtype:{subtype}"),
    }
}

fn color_contract(color: ManaColor) -> &'static str {
    match color {
        ManaColor::White => "white",
        ManaColor::Blue => "blue",
        ManaColor::Black => "black",
        ManaColor::Red => "red",
        ManaColor::Green => "green",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceLayout {
    Normal,
    Mutate,
}

impl SourceLayout {
    fn exact(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Mutate => "mutate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSemanticContext {
    exact_type_line: String,
    layout: SourceLayout,
}

impl SourceSemanticContext {
    pub fn exact_type_line(&self) -> &str {
        &self.exact_type_line
    }

    pub const fn layout(&self) -> SourceLayout {
        self.layout
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceDownMergeKeywordKind {
    Morph {
        face_up_cost: TurnFaceUpCost,
    },
    Megamorph {
        face_up_cost: ManaCost,
    },
    Disguise {
        face_up_cost: ManaCost,
        ward_cost: ManaCost,
    },
    Mutate {
        alternative_cost: ManaCost,
    },
}

impl FaceDownMergeKeywordKind {
    fn stable_contract(&self) -> String {
        match self {
            Self::Morph { face_up_cost } => format!(
                "morph/v1;cast-face-down=mana:{FACE_DOWN_CAST_COST};face-down=2/2-colorless-creature-no-name-no-mana-cost-no-subtypes-no-text;turn-face-up=special-action-no-stack;cost={};reveal=708.9",
                face_up_cost.stable_contract()
            ),
            Self::Megamorph { face_up_cost } => format!(
                "megamorph/v1;cast-face-down=mana:{FACE_DOWN_CAST_COST};face-down=2/2-colorless-creature-no-name-no-mana-cost-no-subtypes-no-text;turn-face-up=special-action-no-stack;cost={};post-turn-face-up=put-one-plus-one-counter;reveal=708.9",
                face_up_cost.exact
            ),
            Self::Disguise {
                face_up_cost,
                ward_cost,
            } => format!(
                "disguise/v1;cast-face-down=mana:{FACE_DOWN_CAST_COST};face-down=2/2-colorless-creature-no-name-no-mana-cost-no-subtypes-with-ward:{};turn-face-up=special-action-no-stack;cost={};reveal=708.9",
                ward_cost.exact, face_up_cost.exact
            ),
            Self::Mutate { alternative_cost } => format!(
                "mutate/v1;alternative-cost={};target=non-human-creature-same-owner;revalidate-on-resolution=true;merge-choice=top-or-bottom;object-identity=target;top-characteristics-plus-all-component-abilities=true;retain-target-status=true;all-components-move-on-zone-change=true",
                alternative_cost.exact
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceDownMergeKeywordProgram {
    exact_source: String,
    normalized_source: String,
    source_context: SourceSemanticContext,
    semantic_digest: String,
    kind: FaceDownMergeKeywordKind,
}

impl FaceDownMergeKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn source_context(&self) -> &SourceSemanticContext {
        &self.source_context
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &FaceDownMergeKeywordKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        face_down_merge_keyword_production_adapter_connected()
    }
}

/// Compiles one complete singleton Oracle line. Card names, Oracle IDs,
/// snapshot hashes, row order, and clause addresses are intentionally absent.
pub fn compile_face_down_merge_keyword_program(
    exact_source: &str,
    exact_source_type_line: &str,
    exact_layout: &str,
) -> Option<FaceDownMergeKeywordProgram> {
    if !complete_single_line(exact_source)
        || !complete_single_line(exact_source_type_line)
        || exact_layout.trim() != exact_layout
    {
        return None;
    }
    let layout = match exact_layout {
        "normal" => SourceLayout::Normal,
        "mutate" => SourceLayout::Mutate,
        _ => return None,
    };

    let (core, reminder) = split_exact_trailing_reminder(exact_source)?;
    let kind = if let Some(cost) = core.strip_prefix("Morph ") {
        (reminder == MORPH_REMINDER && layout == SourceLayout::Normal).then(|| {
            parse_mana_cost(cost).map(|face_up_cost| FaceDownMergeKeywordKind::Morph {
                face_up_cost: TurnFaceUpCost::Mana(face_up_cost),
            })
        })??
    } else if let Some(cost) = core
        .strip_prefix("Morph\u{fffd}")
        .or_else(|| core.strip_prefix("Morph\u{2014}"))
    {
        if reminder != MORPH_REMINDER || layout != SourceLayout::Normal {
            return None;
        }
        FaceDownMergeKeywordKind::Morph {
            face_up_cost: parse_reviewed_nonmana_morph_cost(cost)?,
        }
    } else if let Some(cost) = core.strip_prefix("Megamorph ") {
        if reminder != MEGAMORPH_REMINDER || layout != SourceLayout::Normal {
            return None;
        }
        FaceDownMergeKeywordKind::Megamorph {
            face_up_cost: parse_mana_cost(cost)?,
        }
    } else if let Some(cost) = core.strip_prefix("Disguise ") {
        if reminder != DISGUISE_REMINDER || layout != SourceLayout::Normal {
            return None;
        }
        FaceDownMergeKeywordKind::Disguise {
            face_up_cost: parse_mana_cost(cost)?,
            ward_cost: parse_mana_cost(DISGUISE_WARD_COST)?,
        }
    } else if let Some(cost) = core.strip_prefix("Mutate ") {
        if reminder != MUTATE_REMINDER
            || layout != SourceLayout::Mutate
            || !type_line_has_card_type(exact_source_type_line, "Creature")
        {
            return None;
        }
        FaceDownMergeKeywordKind::Mutate {
            alternative_cost: parse_mana_cost(cost)?,
        }
    } else {
        return None;
    };

    if !source_context_supports(&kind, exact_source_type_line, layout) {
        return None;
    }
    let source_context = SourceSemanticContext {
        exact_type_line: exact_source_type_line.to_owned(),
        layout,
    };
    let normalized_source = normalize_reviewed_clause(exact_source);
    let semantic_digest =
        semantic_digest_with_versions(exact_source, &normalized_source, &source_context, &kind);
    Some(FaceDownMergeKeywordProgram {
        exact_source: exact_source.to_owned(),
        normalized_source,
        source_context,
        semantic_digest,
        kind,
    })
}

fn complete_single_line(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !value.contains(['\r', '\n'])
        && collapse_whitespace(value) == value
}

fn split_exact_trailing_reminder(source: &str) -> Option<(&str, &str)> {
    let open = source.find(" (")?;
    let core = &source[..open];
    let reminder = source.get(open + 2..source.len().checked_sub(1)?)?;
    if core.is_empty()
        || reminder.is_empty()
        || !source.ends_with(')')
        || core.contains(['(', ')'])
        || reminder.contains(['(', ')'])
    {
        return None;
    }
    Some((core, reminder))
}

fn source_context_supports(
    kind: &FaceDownMergeKeywordKind,
    type_line: &str,
    layout: SourceLayout,
) -> bool {
    match kind {
        FaceDownMergeKeywordKind::Mutate { .. } => {
            layout == SourceLayout::Mutate && type_line_has_card_type(type_line, "Creature")
        }
        _ => {
            layout == SourceLayout::Normal
                && [
                    "Artifact",
                    "Battle",
                    "Creature",
                    "Enchantment",
                    "Land",
                    "Planeswalker",
                ]
                .iter()
                .any(|card_type| type_line_has_card_type(type_line, card_type))
        }
    }
}

fn type_line_has_card_type(type_line: &str, wanted: &str) -> bool {
    type_line
        .split(|character: char| {
            character.is_whitespace() || matches!(character, '\u{2014}' | '\u{fffd}' | '-' | '/')
        })
        .any(|word| word == wanted)
}

fn parse_reviewed_nonmana_morph_cost(source: &str) -> Option<TurnFaceUpCost> {
    match source {
        "Discard a card." => Some(TurnFaceUpCost::DiscardFromHand {
            count: 1,
            filter: CardFilter::Any,
        }),
        "Discard a Zombie card." => Some(TurnFaceUpCost::DiscardFromHand {
            count: 1,
            filter: CardFilter::Subtype("Zombie".to_owned()),
        }),
        "Pay 5 life." => Some(TurnFaceUpCost::PayLife(5)),
        "Return a Bird you control to its owner's hand." => {
            Some(TurnFaceUpCost::ReturnControlledToOwnersHand {
                count: 1,
                filter: PermanentFilter::Subtype("Bird".to_owned()),
            })
        }
        "Return two Islands you control to their owner's hand." => {
            Some(TurnFaceUpCost::ReturnControlledToOwnersHand {
                count: 2,
                filter: PermanentFilter::Subtype("Island".to_owned()),
            })
        }
        "Sacrifice another creature." => Some(TurnFaceUpCost::SacrificeControlled {
            count: 1,
            filter: PermanentFilter::AnotherCreature,
        }),
        "Sacrifice two Mountains." => Some(TurnFaceUpCost::SacrificeControlled {
            count: 2,
            filter: PermanentFilter::Subtype("Mountain".to_owned()),
        }),
        "Reveal a white card in your hand." => Some(TurnFaceUpCost::RevealFromHand {
            count: 1,
            filter: CardFilter::Color(ManaColor::White),
        }),
        "Reveal a blue card in your hand." => Some(TurnFaceUpCost::RevealFromHand {
            count: 1,
            filter: CardFilter::Color(ManaColor::Blue),
        }),
        "Reveal a black card in your hand." => Some(TurnFaceUpCost::RevealFromHand {
            count: 1,
            filter: CardFilter::Color(ManaColor::Black),
        }),
        "Reveal a red card in your hand." => Some(TurnFaceUpCost::RevealFromHand {
            count: 1,
            filter: CardFilter::Color(ManaColor::Red),
        }),
        "Reveal a green card in your hand." => Some(TurnFaceUpCost::RevealFromHand {
            count: 1,
            filter: CardFilter::Color(ManaColor::Green),
        }),
        _ => None,
    }
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    if source.is_empty() || source.trim() != source {
        return None;
    }
    let mut cursor = 0usize;
    let mut symbols = Vec::new();
    while cursor < source.len() {
        if source.as_bytes().get(cursor).copied() != Some(b'{') {
            return None;
        }
        let close = source[cursor + 1..].find('}')? + cursor + 1;
        let symbol = &source[cursor + 1..close];
        let parsed = match symbol {
            "W" => ManaSymbol::Colored(ManaColor::White),
            "U" => ManaSymbol::Colored(ManaColor::Blue),
            "B" => ManaSymbol::Colored(ManaColor::Black),
            "R" => ManaSymbol::Colored(ManaColor::Red),
            "G" => ManaSymbol::Colored(ManaColor::Green),
            "C" => ManaSymbol::Colorless,
            "X" => ManaSymbol::VariableX,
            _ if symbol.bytes().all(|byte| byte.is_ascii_digit()) => {
                if symbol.len() > 1 && symbol.starts_with('0') {
                    return None;
                }
                ManaSymbol::Generic(symbol.parse::<u32>().ok()?)
            }
            _ => {
                let (left, right) = symbol.split_once('/')?;
                let left = parse_hybrid_color(left)?;
                let right = parse_hybrid_color(right)?;
                if left == right {
                    return None;
                }
                ManaSymbol::Hybrid(left, right)
            }
        };
        symbols.push(parsed);
        cursor = close + 1;
    }
    (!symbols.is_empty()).then(|| ManaCost {
        exact: source.to_owned(),
        symbols,
    })
}

fn parse_hybrid_color(source: &str) -> Option<ManaColor> {
    match source {
        "W" => Some(ManaColor::White),
        "U" => Some(ManaColor::Blue),
        "B" => Some(ManaColor::Black),
        "R" => Some(ManaColor::Red),
        "G" => Some(ManaColor::Green),
        _ => None,
    }
}

fn normalize_reviewed_clause(source: &str) -> String {
    collapse_whitespace(
        &source
            .replace('\u{2019}', "'")
            .replace('\u{2014}', "\u{fffd}"),
    )
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn semantic_digest_with_versions(
    exact_source: &str,
    normalized_source: &str,
    source_context: &SourceSemanticContext,
    kind: &FaceDownMergeKeywordKind,
) -> String {
    semantic_digest_using_versions(
        exact_source,
        normalized_source,
        source_context,
        kind,
        FACE_DOWN_MERGE_KEYWORD_COMPILER_VERSION,
        FACE_DOWN_MERGE_KEYWORD_RUNTIME_VERSION,
        FACE_DOWN_MERGE_KEYWORD_RULES_CONTEXT_VERSION,
    )
}

fn semantic_digest_using_versions(
    exact_source: &str,
    normalized_source: &str,
    source_context: &SourceSemanticContext,
    kind: &FaceDownMergeKeywordKind,
    compiler_version: &str,
    runtime_version: &str,
    rules_context_version: &str,
) -> String {
    let kind_contract = kind.stable_contract();
    let mut hasher = Sha256::new();
    for component in [
        "face-down-merge-keyword-content/v1",
        compiler_version,
        runtime_version,
        rules_context_version,
        exact_source,
        normalized_source,
        source_context.exact_type_line.as_str(),
        source_context.layout.exact(),
        kind_contract.as_str(),
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotCandidateClass {
    SupportedResidual,
    ExistingMorphKeywordOwner,
    UnsupportedCompoundModifierOrReminderless,
}

pub fn classify_snapshot_candidate(
    exact_source: &str,
    exact_source_type_line: &str,
    exact_layout: &str,
) -> Option<SnapshotCandidateClass> {
    snapshot_family(exact_source)?;
    let Some(program) =
        compile_face_down_merge_keyword_program(exact_source, exact_source_type_line, exact_layout)
    else {
        return Some(SnapshotCandidateClass::UnsupportedCompoundModifierOrReminderless);
    };
    if matches!(
        program.kind,
        FaceDownMergeKeywordKind::Morph {
            face_up_cost: TurnFaceUpCost::Mana(_)
        }
    ) {
        Some(SnapshotCandidateClass::ExistingMorphKeywordOwner)
    } else {
        Some(SnapshotCandidateClass::SupportedResidual)
    }
}

fn snapshot_family(source: &str) -> Option<&'static str> {
    if source.starts_with("Morph ")
        || source.starts_with("Morph\u{fffd}")
        || source.starts_with("Morph\u{2014}")
    {
        Some("Morph")
    } else if source.starts_with("Megamorph ") {
        Some("Megamorph")
    } else if source.starts_with("Disguise ") {
        Some("Disguise")
    } else if source.starts_with("Mutate ") {
        Some("Mutate")
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Library,
    Hand,
    Command,
    Graveyard,
    Exile,
    Stack,
    Battlefield,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiableCharacteristics {
    pub name: Option<String>,
    pub mana_cost: Option<ManaCost>,
    pub colors: BTreeSet<ManaColor>,
    pub supertypes: BTreeSet<String>,
    pub card_types: BTreeSet<String>,
    pub subtypes: BTreeSet<String>,
    pub ability_semantic_digests: Vec<String>,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
}

impl CopiableCharacteristics {
    pub fn is_creature(&self) -> bool {
        self.card_types.contains("Creature")
    }

    pub fn is_human(&self) -> bool {
        self.subtypes.contains("Human")
    }
}

pub fn canonical_face_down_characteristics(with_disguise_ward: bool) -> CopiableCharacteristics {
    CopiableCharacteristics {
        name: None,
        mana_cost: None,
        colors: BTreeSet::new(),
        supertypes: BTreeSet::new(),
        card_types: BTreeSet::from(["Creature".to_owned()]),
        subtypes: BTreeSet::new(),
        ability_semantic_digests: if with_disguise_ward {
            vec!["ward:{2}:face-down-disguise/v1".to_owned()]
        } else {
            Vec::new()
        },
        power: Some(2),
        toughness: Some(2),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentKind {
    PhysicalCard,
    Token,
    SpellCopyToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardComponent {
    pub component_id: CardComponentId,
    /// Stable physical card identity. Tokens and spell-copy components have
    /// no physical card identity.
    pub physical_card_object_id: Option<ObjectId>,
    /// Most recent incarnation of that physical card while it is a component.
    pub physical_card_incarnation_id: Option<IncarnationId>,
    pub current_object: ObjectRef,
    pub owner: PlayerId,
    pub kind: ComponentKind,
    pub printed: CopiableCharacteristics,
    pub source_type_line: String,
    pub source_layout: SourceLayout,
    pub commander_id: Option<CommanderId>,
    pub turn_face_up_trigger_digests: Vec<String>,
    pub mutates_trigger_digests: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceDownCastMode {
    Morph,
    Megamorph,
    Disguise,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceDownSpell {
    pub stack_id: StackObjectId,
    pub cast_event_id: CastEventId,
    pub object: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub hidden_component: Option<CardComponent>,
    pub characteristics: CopiableCharacteristics,
    pub mode: FaceDownCastMode,
    pub program_semantic_digest: String,
    pub is_copy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceDownPermanent {
    pub object: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub hidden_component: Option<CardComponent>,
    pub characteristics: CopiableCharacteristics,
    pub mode: FaceDownCastMode,
    pub program_semantic_digest: String,
    pub is_copy: bool,
    pub tapped: bool,
    pub damage_marked: u32,
    pub counters: BTreeMap<String, u32>,
    pub attachments: BTreeSet<AttachmentId>,
    pub attacking: bool,
    pub blocking: bool,
    pub control_since_turn: u64,
}

impl FaceDownPermanent {
    pub fn can_turn_face_up_with(&self, program: &FaceDownMergeKeywordProgram) -> bool {
        self.hidden_component.is_some()
            && !self.is_copy
            && self.program_semantic_digest == program.semantic_digest
            && matches!(
                (self.mode, &program.kind),
                (
                    FaceDownCastMode::Morph,
                    FaceDownMergeKeywordKind::Morph { .. }
                ) | (
                    FaceDownCastMode::Megamorph,
                    FaceDownMergeKeywordKind::Megamorph { .. }
                ) | (
                    FaceDownCastMode::Disguise,
                    FaceDownMergeKeywordKind::Disguise { .. }
                )
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostPaymentRole {
    FaceDownAlternativeCast,
    TurnFaceUpSpecialAction,
    MutateAlternativeCast,
    Ward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPaymentEvidence {
    pub payer: PlayerId,
    pub role: CostPaymentRole,
    pub exact_cost: ManaCost,
    pub mana_source_objects: Vec<ObjectRef>,
    pub chosen_x: Option<u32>,
    pub payment_complete: bool,
    pub resources_debited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostObjectEvidence {
    pub object_before: ObjectRef,
    pub object_after: Option<ObjectRef>,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone_before: Zone,
    pub zone_after: Zone,
    pub colors: BTreeSet<ManaColor>,
    pub card_types: BTreeSet<String>,
    pub subtypes: BTreeSet<String>,
    pub revealed_to_all_players: bool,
    pub transition_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnFaceUpPaymentEvidence {
    Mana(ManaPaymentEvidence),
    Discard {
        payer: PlayerId,
        cards: Vec<CostObjectEvidence>,
        complete: bool,
    },
    PayLife {
        payer: PlayerId,
        life_before: u32,
        life_after: u32,
        payment_complete: bool,
    },
    ReturnToOwnersHand {
        payer: PlayerId,
        permanents: Vec<CostObjectEvidence>,
        complete: bool,
    },
    RevealFromHand {
        payer: PlayerId,
        cards: Vec<CostObjectEvidence>,
        complete: bool,
    },
    Sacrifice {
        payer: PlayerId,
        permanents: Vec<CostObjectEvidence>,
        complete: bool,
    },
}

fn validate_mana_payment(
    evidence: &ManaPaymentEvidence,
    payer: PlayerId,
    role: CostPaymentRole,
    expected: &ManaCost,
) -> Result<(), FaceDownMergeRuntimeError> {
    if evidence.payer != payer {
        return Err(FaceDownMergeRuntimeError::WrongPayer {
            expected: payer,
            actual: evidence.payer,
        });
    }
    if evidence.role != role || evidence.exact_cost != *expected {
        return Err(FaceDownMergeRuntimeError::PaymentContractMismatch);
    }
    if !evidence.payment_complete || !evidence.resources_debited {
        return Err(FaceDownMergeRuntimeError::IncompletePaymentEvidence);
    }
    let mut sources = BTreeSet::new();
    for source in &evidence.mana_source_objects {
        if !sources.insert(*source) {
            return Err(FaceDownMergeRuntimeError::DuplicatePaymentObject(*source));
        }
    }
    if expected.symbols.contains(&ManaSymbol::VariableX) && evidence.chosen_x.is_none() {
        return Err(FaceDownMergeRuntimeError::MissingXChoice);
    }
    if !expected.symbols.contains(&ManaSymbol::VariableX) && evidence.chosen_x.is_some() {
        return Err(FaceDownMergeRuntimeError::UnexpectedXChoice);
    }
    Ok(())
}

fn validate_turn_face_up_payment(
    expected: &TurnFaceUpCost,
    evidence: &TurnFaceUpPaymentEvidence,
    payer: PlayerId,
    source: ObjectRef,
) -> Result<(), FaceDownMergeRuntimeError> {
    match (expected, evidence) {
        (TurnFaceUpCost::Mana(expected), TurnFaceUpPaymentEvidence::Mana(actual)) => {
            validate_mana_payment(
                actual,
                payer,
                CostPaymentRole::TurnFaceUpSpecialAction,
                expected,
            )
        }
        (
            TurnFaceUpCost::PayLife(amount),
            TurnFaceUpPaymentEvidence::PayLife {
                payer: actual_payer,
                life_before,
                life_after,
                payment_complete,
            },
        ) => {
            validate_payer(*actual_payer, payer)?;
            if !payment_complete || life_before.checked_sub(*amount) != Some(*life_after) {
                return Err(FaceDownMergeRuntimeError::IncompletePaymentEvidence);
            }
            Ok(())
        }
        (
            TurnFaceUpCost::DiscardFromHand { count, filter },
            TurnFaceUpPaymentEvidence::Discard {
                payer: actual_payer,
                cards,
                complete,
            },
        ) => {
            validate_payer(*actual_payer, payer)?;
            validate_cost_objects(
                cards,
                *count,
                Zone::Hand,
                Zone::Graveyard,
                payer,
                None,
                Some(filter),
                source,
                *complete,
            )
        }
        (
            TurnFaceUpCost::ReturnControlledToOwnersHand { count, filter },
            TurnFaceUpPaymentEvidence::ReturnToOwnersHand {
                payer: actual_payer,
                permanents,
                complete,
            },
        ) => {
            validate_payer(*actual_payer, payer)?;
            validate_cost_objects(
                permanents,
                *count,
                Zone::Battlefield,
                Zone::Hand,
                payer,
                Some(filter),
                None,
                source,
                *complete,
            )
        }
        (
            TurnFaceUpCost::RevealFromHand { count, filter },
            TurnFaceUpPaymentEvidence::RevealFromHand {
                payer: actual_payer,
                cards,
                complete,
            },
        ) => {
            validate_payer(*actual_payer, payer)?;
            validate_cost_objects(
                cards,
                *count,
                Zone::Hand,
                Zone::Hand,
                payer,
                None,
                Some(filter),
                source,
                *complete,
            )?;
            if cards.iter().any(|card| !card.revealed_to_all_players) {
                return Err(FaceDownMergeRuntimeError::CardWasNotRevealed);
            }
            Ok(())
        }
        (
            TurnFaceUpCost::SacrificeControlled { count, filter },
            TurnFaceUpPaymentEvidence::Sacrifice {
                payer: actual_payer,
                permanents,
                complete,
            },
        ) => {
            validate_payer(*actual_payer, payer)?;
            validate_cost_objects(
                permanents,
                *count,
                Zone::Battlefield,
                Zone::Graveyard,
                payer,
                Some(filter),
                None,
                source,
                *complete,
            )
        }
        _ => Err(FaceDownMergeRuntimeError::PaymentContractMismatch),
    }
}

fn validate_payer(actual: PlayerId, expected: PlayerId) -> Result<(), FaceDownMergeRuntimeError> {
    if actual != expected {
        Err(FaceDownMergeRuntimeError::WrongPayer { expected, actual })
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_cost_objects(
    objects: &[CostObjectEvidence],
    count: u32,
    from: Zone,
    to: Zone,
    payer: PlayerId,
    permanent_filter: Option<&PermanentFilter>,
    card_filter: Option<&CardFilter>,
    source: ObjectRef,
    complete: bool,
) -> Result<(), FaceDownMergeRuntimeError> {
    if !complete || objects.len() != count as usize {
        return Err(FaceDownMergeRuntimeError::IncompletePaymentEvidence);
    }
    let mut identities = BTreeSet::new();
    for object in objects {
        if !object.transition_complete
            || object.zone_before != from
            || object.zone_after != to
            || !identities.insert(object.object_before)
        {
            return Err(FaceDownMergeRuntimeError::InvalidCostObjectTransition);
        }
        match (from, to) {
            (Zone::Hand, Zone::Hand) => {
                if object.object_after != Some(object.object_before) {
                    return Err(FaceDownMergeRuntimeError::InvalidCostObjectTransition);
                }
            }
            _ => {
                let Some(after) = object.object_after else {
                    return Err(FaceDownMergeRuntimeError::InvalidCostObjectTransition);
                };
                if after.object_id != object.object_before.object_id
                    || after.incarnation_id == object.object_before.incarnation_id
                {
                    return Err(FaceDownMergeRuntimeError::InvalidCostObjectTransition);
                }
            }
        }
        if let Some(filter) = permanent_filter
            && (object.controller != payer || !permanent_matches(object, filter, source))
        {
            return Err(FaceDownMergeRuntimeError::CostObjectDoesNotMatch);
        }
        if let Some(filter) = card_filter
            && (object.owner != payer || !card_matches(object, filter))
        {
            return Err(FaceDownMergeRuntimeError::CostObjectDoesNotMatch);
        }
    }
    Ok(())
}

fn permanent_matches(
    object: &CostObjectEvidence,
    filter: &PermanentFilter,
    source: ObjectRef,
) -> bool {
    match filter {
        PermanentFilter::AnotherCreature => {
            object.object_before != source && object.card_types.contains("Creature")
        }
        PermanentFilter::Subtype(subtype) => object.subtypes.contains(subtype),
    }
}

fn card_matches(object: &CostObjectEvidence, filter: &CardFilter) -> bool {
    match filter {
        CardFilter::Any => true,
        CardFilter::Color(color) => object.colors.contains(color),
        CardFilter::Subtype(subtype) => object.subtypes.contains(subtype),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceDownCastInput {
    pub cast_event_id: CastEventId,
    pub stack_id: StackObjectId,
    pub caster: PlayerId,
    pub source_before_cast: ObjectRef,
    pub stack_incarnation_id: IncarnationId,
    pub source_zone: Zone,
    pub component: CardComponent,
    pub player_has_priority: bool,
    pub casting_permission_complete: bool,
    pub all_costs_enumerated: bool,
    pub mandatory_additional_costs_paid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingFaceDownCast {
    program: FaceDownMergeKeywordProgram,
    input: FaceDownCastInput,
    mode: FaceDownCastMode,
    cast_cost: ManaCost,
}

pub fn begin_face_down_cast(
    program: &FaceDownMergeKeywordProgram,
    input: FaceDownCastInput,
) -> Result<PendingFaceDownCast, FaceDownMergeRuntimeError> {
    let mode = match program.kind {
        FaceDownMergeKeywordKind::Morph { .. } => FaceDownCastMode::Morph,
        FaceDownMergeKeywordKind::Megamorph { .. } => FaceDownCastMode::Megamorph,
        FaceDownMergeKeywordKind::Disguise { .. } => FaceDownCastMode::Disguise,
        FaceDownMergeKeywordKind::Mutate { .. } => {
            return Err(FaceDownMergeRuntimeError::WrongProgramKind);
        }
    };
    if !input.player_has_priority
        || !input.casting_permission_complete
        || !input.all_costs_enumerated
        || !input.mandatory_additional_costs_paid
    {
        return Err(FaceDownMergeRuntimeError::IncompleteCastEvidence);
    }
    if matches!(input.source_zone, Zone::Battlefield | Zone::Stack)
        || input.component.current_object != input.source_before_cast
        || input.component.physical_card_object_id != Some(input.source_before_cast.object_id)
        || input.component.physical_card_incarnation_id
            != Some(input.source_before_cast.incarnation_id)
        || input.component.source_layout != SourceLayout::Normal
        || input.component.source_type_line != program.source_context.exact_type_line
        || input.stack_incarnation_id == input.source_before_cast.incarnation_id
    {
        return Err(FaceDownMergeRuntimeError::InvalidCastSource);
    }
    Ok(PendingFaceDownCast {
        program: program.clone(),
        input,
        mode,
        cast_cost: parse_mana_cost(FACE_DOWN_CAST_COST)
            .expect("the reviewed face-down cost is valid"),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceDownCastReceipt {
    pub cast_event_id: CastEventId,
    pub source_before_cast: ObjectRef,
    pub stack_spell: ObjectRef,
    pub mode: FaceDownCastMode,
    pub semantic_digest: String,
    pub face_down_characteristics: CopiableCharacteristics,
}

impl PendingFaceDownCast {
    pub fn commit(
        self,
        payment: ManaPaymentEvidence,
    ) -> Result<(FaceDownSpell, FaceDownCastReceipt), FaceDownMergeRuntimeError> {
        validate_mana_payment(
            &payment,
            self.input.caster,
            CostPaymentRole::FaceDownAlternativeCast,
            &self.cast_cost,
        )?;
        let stack_spell = ObjectRef {
            object_id: self.input.source_before_cast.object_id,
            incarnation_id: self.input.stack_incarnation_id,
        };
        let mut component = self.input.component;
        component.current_object = stack_spell;
        component.physical_card_incarnation_id = Some(stack_spell.incarnation_id);
        let characteristics =
            canonical_face_down_characteristics(self.mode == FaceDownCastMode::Disguise);
        let spell = FaceDownSpell {
            stack_id: self.input.stack_id,
            cast_event_id: self.input.cast_event_id,
            object: stack_spell,
            owner: component.owner,
            controller: self.input.caster,
            hidden_component: Some(component),
            characteristics: characteristics.clone(),
            mode: self.mode,
            program_semantic_digest: self.program.semantic_digest.clone(),
            is_copy: false,
        };
        let receipt = FaceDownCastReceipt {
            cast_event_id: self.input.cast_event_id,
            source_before_cast: self.input.source_before_cast,
            stack_spell,
            mode: self.mode,
            semantic_digest: self.program.semantic_digest,
            face_down_characteristics: characteristics,
        };
        Ok((spell, receipt))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceDownResolutionReceipt {
    pub stack_spell: ObjectRef,
    pub battlefield_object: ObjectRef,
    pub entered_battlefield_face_down: bool,
    pub enters_triggers_may_observe_event: bool,
    pub hidden_face_has_no_active_abilities: bool,
}

pub fn resolve_face_down_spell(
    spell: FaceDownSpell,
    battlefield_incarnation_id: IncarnationId,
) -> Result<(FaceDownPermanent, FaceDownResolutionReceipt), FaceDownMergeRuntimeError> {
    if battlefield_incarnation_id == spell.object.incarnation_id
        || spell.hidden_component.is_none()
        || spell.is_copy
    {
        return Err(FaceDownMergeRuntimeError::InvalidResolutionTransition);
    }
    let battlefield_object = ObjectRef {
        object_id: spell.object.object_id,
        incarnation_id: battlefield_incarnation_id,
    };
    let mut component = spell.hidden_component.expect("checked");
    component.current_object = battlefield_object;
    component.physical_card_incarnation_id = Some(battlefield_object.incarnation_id);
    let permanent = FaceDownPermanent {
        object: battlefield_object,
        owner: spell.owner,
        controller: spell.controller,
        hidden_component: Some(component),
        characteristics: spell.characteristics,
        mode: spell.mode,
        program_semantic_digest: spell.program_semantic_digest,
        is_copy: false,
        tapped: false,
        damage_marked: 0,
        counters: BTreeMap::new(),
        attachments: BTreeSet::new(),
        attacking: false,
        blocking: false,
        control_since_turn: 0,
    };
    Ok((
        permanent,
        FaceDownResolutionReceipt {
            stack_spell: spell.object,
            battlefield_object,
            entered_battlefield_face_down: true,
            enters_triggers_may_observe_event: true,
            hidden_face_has_no_active_abilities: true,
        },
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceDownCopy {
    pub object: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub characteristics: CopiableCharacteristics,
    pub is_token: bool,
    pub turn_face_up_authority: bool,
}

pub fn copy_face_down_spell_or_permanent(
    source_characteristics: &CopiableCharacteristics,
    object: ObjectRef,
    owner: PlayerId,
    controller: PlayerId,
    zone: Zone,
) -> Result<FaceDownCopy, FaceDownMergeRuntimeError> {
    if !source_characteristics.is_creature()
        || source_characteristics.name.is_some()
        || source_characteristics.mana_cost.is_some()
        || source_characteristics.power != Some(2)
        || source_characteristics.toughness != Some(2)
    {
        return Err(FaceDownMergeRuntimeError::NotCanonicalFaceDownObject);
    }
    if !matches!(zone, Zone::Stack | Zone::Battlefield) {
        return Err(FaceDownMergeRuntimeError::WrongZone);
    }
    Ok(FaceDownCopy {
        object,
        owner,
        controller,
        zone,
        characteristics: source_characteristics.clone(),
        is_token: true,
        // The hidden face and the special-action authority are not copiable.
        turn_face_up_authority: false,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceDownRevealReason {
    BattlefieldToAnotherZone,
    StackToNonBattlefieldZone,
    OwnerLeavesGame,
    GameEnds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceDownRevealReceipt {
    pub object: ObjectRef,
    pub reason: FaceDownRevealReason,
    pub revealed_component_id: Option<CardComponentId>,
    pub revealed_to_all_players: bool,
    pub before_zone_change_or_game_exit: bool,
}

pub fn reveal_face_down_object(
    object: ObjectRef,
    zone: Zone,
    destination: Option<Zone>,
    hidden_component: Option<&CardComponent>,
    reason: FaceDownRevealReason,
    revealed_to_all_players: bool,
    before_transition: bool,
) -> Result<FaceDownRevealReceipt, FaceDownMergeRuntimeError> {
    let reason_matches = match reason {
        FaceDownRevealReason::BattlefieldToAnotherZone => {
            zone == Zone::Battlefield && destination.is_some_and(|to| to != Zone::Battlefield)
        }
        FaceDownRevealReason::StackToNonBattlefieldZone => {
            zone == Zone::Stack && destination.is_some_and(|to| to != Zone::Battlefield)
        }
        FaceDownRevealReason::OwnerLeavesGame | FaceDownRevealReason::GameEnds => {
            matches!(zone, Zone::Stack | Zone::Battlefield) && destination.is_none()
        }
    };
    if !reason_matches || !revealed_to_all_players || !before_transition {
        return Err(FaceDownMergeRuntimeError::IncompleteRevealEvidence);
    }
    Ok(FaceDownRevealReceipt {
        object,
        reason,
        revealed_component_id: hidden_component.map(|component| component.component_id),
        revealed_to_all_players,
        before_zone_change_or_game_exit: before_transition,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterPlacementEvidence {
    pub object: ObjectRef,
    pub counter_name: String,
    pub requested: u32,
    pub placed: u32,
    pub replacement_effects_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnFaceUpSpecialActionInput {
    pub event_id: EventId,
    pub actor: PlayerId,
    pub actor_has_priority: bool,
    pub special_actions_complete: bool,
    pub payment: TurnFaceUpPaymentEvidence,
    pub megamorph_counter_placement: Option<CounterPlacementEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTurnedFaceUpTrigger {
    pub trigger_id: TriggerId,
    pub controller: PlayerId,
    pub source: ObjectRef,
    pub ability_semantic_digest: String,
    pub waits_for_state_based_actions: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnFaceUpReceipt {
    pub event_id: EventId,
    pub permanent: ObjectRef,
    pub semantic_digest: String,
    pub used_stack: bool,
    pub changed_zone: bool,
    pub caused_enters_event: bool,
    pub restored_characteristics: CopiableCharacteristics,
    pub megamorph_counter_requested: u32,
    pub megamorph_counter_placed: u32,
    pub pending_triggers: Vec<PendingTurnedFaceUpTrigger>,
    pub state_based_actions_due_before_priority: bool,
}

pub fn turn_face_up_special_action(
    program: &FaceDownMergeKeywordProgram,
    permanent: &mut FaceDownPermanent,
    input: TurnFaceUpSpecialActionInput,
) -> Result<TurnFaceUpReceipt, FaceDownMergeRuntimeError> {
    if !input.actor_has_priority || !input.special_actions_complete {
        return Err(FaceDownMergeRuntimeError::SpecialActionTimingUnavailable);
    }
    if input.actor != permanent.controller {
        return Err(FaceDownMergeRuntimeError::WrongController {
            expected: permanent.controller,
            actual: input.actor,
        });
    }
    if !permanent.can_turn_face_up_with(program) {
        return Err(FaceDownMergeRuntimeError::NoTurnFaceUpAuthority);
    }

    let (cost, megamorph) = match &program.kind {
        FaceDownMergeKeywordKind::Morph { face_up_cost } => (face_up_cost.clone(), false),
        FaceDownMergeKeywordKind::Megamorph { face_up_cost } => {
            (TurnFaceUpCost::Mana(face_up_cost.clone()), true)
        }
        FaceDownMergeKeywordKind::Disguise { face_up_cost, .. } => {
            (TurnFaceUpCost::Mana(face_up_cost.clone()), false)
        }
        FaceDownMergeKeywordKind::Mutate { .. } => {
            return Err(FaceDownMergeRuntimeError::WrongProgramKind);
        }
    };
    validate_turn_face_up_payment(&cost, &input.payment, input.actor, permanent.object)?;

    let (counter_requested, counter_placed) = if megamorph {
        let placement = input
            .megamorph_counter_placement
            .as_ref()
            .ok_or(FaceDownMergeRuntimeError::MissingCounterPlacementEvidence)?;
        if placement.object != permanent.object
            || placement.counter_name != "+1/+1"
            || placement.requested != 1
            || !placement.replacement_effects_complete
        {
            return Err(FaceDownMergeRuntimeError::InvalidCounterPlacementEvidence);
        }
        (placement.requested, placement.placed)
    } else {
        if input.megamorph_counter_placement.is_some() {
            return Err(FaceDownMergeRuntimeError::UnexpectedCounterPlacementEvidence);
        }
        (0, 0)
    };

    let component = permanent
        .hidden_component
        .as_ref()
        .ok_or(FaceDownMergeRuntimeError::NoTurnFaceUpAuthority)?;
    if component.current_object != permanent.object
        || component.source_type_line != program.source_context.exact_type_line
        || component.source_layout != program.source_context.layout
    {
        return Err(FaceDownMergeRuntimeError::HiddenFaceContextMismatch);
    }
    let restored = component.printed.clone();
    let pending_triggers = component
        .turn_face_up_trigger_digests
        .iter()
        .enumerate()
        .map(|(index, digest)| PendingTurnedFaceUpTrigger {
            trigger_id: input
                .event_id
                .wrapping_mul(1_000_003)
                .wrapping_add(index as u64 + 1),
            controller: permanent.controller,
            source: permanent.object,
            ability_semantic_digest: digest.clone(),
            waits_for_state_based_actions: true,
        })
        .collect::<Vec<_>>();
    permanent.characteristics = restored.clone();
    if counter_placed > 0 {
        let total = permanent.counters.entry("+1/+1".to_owned()).or_default();
        *total = total
            .checked_add(counter_placed)
            .ok_or(FaceDownMergeRuntimeError::CounterOverflow)?;
    }

    Ok(TurnFaceUpReceipt {
        event_id: input.event_id,
        permanent: permanent.object,
        semantic_digest: program.semantic_digest.clone(),
        used_stack: false,
        changed_zone: false,
        caused_enters_event: false,
        restored_characteristics: restored,
        megamorph_counter_requested: counter_requested,
        megamorph_counter_placed: counter_placed,
        pending_triggers,
        state_based_actions_due_before_priority: true,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackObjectKind {
    Spell,
    Ability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisguiseWardTargetEvidence {
    pub event_id: EventId,
    pub stack_object: StackObjectId,
    pub stack_object_kind: StackObjectKind,
    pub stack_controller: PlayerId,
    pub target: ObjectRef,
    pub target_controller: PlayerId,
    pub target_was_newly_chosen: bool,
    pub all_targets_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDisguiseWardTrigger {
    pub event_id: EventId,
    pub stack_object: StackObjectId,
    pub stack_object_kind: StackObjectKind,
    pub ward_controller: PlayerId,
    pub ward_cost: ManaCost,
    pub source: ObjectRef,
}

pub fn begin_disguise_ward_trigger(
    program: &FaceDownMergeKeywordProgram,
    permanent: &FaceDownPermanent,
    evidence: DisguiseWardTargetEvidence,
) -> Result<Option<PendingDisguiseWardTrigger>, FaceDownMergeRuntimeError> {
    let FaceDownMergeKeywordKind::Disguise { ward_cost, .. } = &program.kind else {
        return Err(FaceDownMergeRuntimeError::WrongProgramKind);
    };
    if permanent.mode != FaceDownCastMode::Disguise
        || permanent.program_semantic_digest != program.semantic_digest
        || permanent.object != evidence.target
        || permanent.controller != evidence.target_controller
        || !evidence.all_targets_complete
    {
        return Err(FaceDownMergeRuntimeError::InvalidWardTargetEvidence);
    }
    if !evidence.target_was_newly_chosen || evidence.stack_controller == permanent.controller {
        return Ok(None);
    }
    Ok(Some(PendingDisguiseWardTrigger {
        event_id: evidence.event_id,
        stack_object: evidence.stack_object,
        stack_object_kind: evidence.stack_object_kind,
        ward_controller: permanent.controller,
        ward_cost: ward_cost.clone(),
        source: permanent.object,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WardChoice {
    Pay,
    Decline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisguiseWardResolutionReceipt {
    pub event_id: EventId,
    pub source: ObjectRef,
    pub stack_object: StackObjectId,
    pub paid: bool,
    pub counter_attempted: bool,
    pub stack_object_countered: bool,
}

impl PendingDisguiseWardTrigger {
    pub fn resolve(
        self,
        targeted_object_controller: PlayerId,
        choice: WardChoice,
        payment: Option<ManaPaymentEvidence>,
        stack_object_can_be_countered: bool,
        counterability_evidence_complete: bool,
    ) -> Result<DisguiseWardResolutionReceipt, FaceDownMergeRuntimeError> {
        if !counterability_evidence_complete {
            return Err(FaceDownMergeRuntimeError::IncompleteCounterabilityEvidence);
        }
        let (paid, attempted, countered) = match choice {
            WardChoice::Pay => {
                let payment =
                    payment.ok_or(FaceDownMergeRuntimeError::IncompletePaymentEvidence)?;
                validate_mana_payment(
                    &payment,
                    targeted_object_controller,
                    CostPaymentRole::Ward,
                    &self.ward_cost,
                )?;
                (true, false, false)
            }
            WardChoice::Decline => {
                if payment.is_some() {
                    return Err(FaceDownMergeRuntimeError::UnexpectedPaymentEvidence);
                }
                (false, true, stack_object_can_be_countered)
            }
        };
        Ok(DisguiseWardResolutionReceipt {
            event_id: self.event_id,
            source: self.source,
            stack_object: self.stack_object,
            paid,
            counter_attempted: attempted,
            stack_object_countered: countered,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermanentState {
    pub object: ObjectRef,
    pub controller: PlayerId,
    /// Top component first.
    pub components: Vec<CardComponent>,
    pub characteristics: CopiableCharacteristics,
    pub counters: BTreeMap<String, u32>,
    pub attachments: BTreeSet<AttachmentId>,
    pub tapped: bool,
    pub damage_marked: u32,
    pub attacking: bool,
    pub blocking: bool,
    pub control_since_turn: u64,
}

impl PermanentState {
    pub fn owner(&self) -> Result<PlayerId, FaceDownMergeRuntimeError> {
        let Some(first) = self.components.first() else {
            return Err(FaceDownMergeRuntimeError::MissingPermanentComponents);
        };
        if self
            .components
            .iter()
            .any(|component| component.owner != first.owner)
        {
            return Err(FaceDownMergeRuntimeError::ComponentOwnerMismatch);
        }
        Ok(first.owner)
    }

    pub fn is_token(&self) -> bool {
        self.components.first().is_some_and(|component| {
            matches!(
                component.kind,
                ComponentKind::Token | ComponentKind::SpellCopyToken
            )
        })
    }

    pub fn is_merged(&self) -> bool {
        self.components.len() > 1
    }

    pub fn commander_components(&self) -> Vec<CommanderId> {
        self.components
            .iter()
            .filter_map(|component| component.commander_id)
            .collect()
    }

    fn validate_component_state(&self) -> Result<(), FaceDownMergeRuntimeError> {
        if self.components.is_empty()
            || self.components.iter().any(|component| {
                component.current_object != self.object
                    || match component.kind {
                        ComponentKind::PhysicalCard => {
                            component.physical_card_object_id.is_none()
                                || component.physical_card_incarnation_id.is_none()
                        }
                        ComponentKind::Token | ComponentKind::SpellCopyToken => {
                            component.physical_card_object_id.is_some()
                                || component.physical_card_incarnation_id.is_some()
                        }
                    }
            })
        {
            return Err(FaceDownMergeRuntimeError::InvalidMergedComponentState);
        }
        let expected = merged_copiable_characteristics(&self.components)?;
        if expected != self.characteristics {
            return Err(FaceDownMergeRuntimeError::MergedCharacteristicsMismatch);
        }
        self.owner()?;
        Ok(())
    }
}

fn merged_copiable_characteristics(
    components: &[CardComponent],
) -> Result<CopiableCharacteristics, FaceDownMergeRuntimeError> {
    let Some(top) = components.first() else {
        return Err(FaceDownMergeRuntimeError::MissingPermanentComponents);
    };
    let mut result = top.printed.clone();
    result.ability_semantic_digests.clear();
    for component in components {
        result
            .ability_semantic_digests
            .extend(component.printed.ability_semantic_digests.iter().cloned());
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutateTargetEvidence {
    pub target: ObjectRef,
    pub target_owner: PlayerId,
    pub target_controller: PlayerId,
    pub characteristics: CopiableCharacteristics,
    pub component_owners_complete: bool,
    pub target_characteristics_complete: bool,
    pub target_legality_checked: bool,
}

fn validate_mutate_target_snapshot(
    spell_owner: PlayerId,
    target: &PermanentState,
    evidence: &MutateTargetEvidence,
) -> Result<(), FaceDownMergeRuntimeError> {
    if evidence.target != target.object
        || evidence.target_controller != target.controller
        || !evidence.component_owners_complete
        || !evidence.target_characteristics_complete
        || !evidence.target_legality_checked
        || evidence.characteristics != target.characteristics
    {
        return Err(FaceDownMergeRuntimeError::IncompleteMutateTargetEvidence);
    }
    let actual_owner = target.owner()?;
    if actual_owner != evidence.target_owner {
        return Err(FaceDownMergeRuntimeError::IncompleteMutateTargetEvidence);
    }
    if actual_owner != spell_owner
        || !target.characteristics.is_creature()
        || target.characteristics.is_human()
    {
        return Err(FaceDownMergeRuntimeError::IllegalMutateTarget);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutateCastInput {
    pub cast_event_id: CastEventId,
    pub stack_id: StackObjectId,
    pub caster: PlayerId,
    pub source_before_cast: ObjectRef,
    pub stack_incarnation_id: IncarnationId,
    pub source_zone: Zone,
    pub component: CardComponent,
    pub player_has_priority: bool,
    pub casting_permission_complete: bool,
    pub all_targets_complete: bool,
    pub all_costs_enumerated: bool,
    pub mandatory_additional_costs_paid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutatingCreatureSpell {
    pub stack_id: StackObjectId,
    pub cast_event_id: CastEventId,
    pub object: ObjectRef,
    pub controller: PlayerId,
    pub component: CardComponent,
    pub target: ObjectRef,
    pub characteristics: CopiableCharacteristics,
    pub program_semantic_digest: String,
    pub is_copy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutateCastReceipt {
    pub cast_event_id: CastEventId,
    pub stack_spell: ObjectRef,
    pub target: ObjectRef,
    pub alternative_cost: ManaCost,
    pub semantic_digest: String,
}

pub fn cast_with_mutate(
    program: &FaceDownMergeKeywordProgram,
    input: MutateCastInput,
    target: &PermanentState,
    target_evidence: MutateTargetEvidence,
    payment: ManaPaymentEvidence,
) -> Result<(MutatingCreatureSpell, MutateCastReceipt), FaceDownMergeRuntimeError> {
    let FaceDownMergeKeywordKind::Mutate { alternative_cost } = &program.kind else {
        return Err(FaceDownMergeRuntimeError::WrongProgramKind);
    };
    if !input.player_has_priority
        || !input.casting_permission_complete
        || !input.all_targets_complete
        || !input.all_costs_enumerated
        || !input.mandatory_additional_costs_paid
    {
        return Err(FaceDownMergeRuntimeError::IncompleteCastEvidence);
    }
    if matches!(input.source_zone, Zone::Battlefield | Zone::Stack)
        || input.component.current_object != input.source_before_cast
        || input.component.physical_card_object_id != Some(input.source_before_cast.object_id)
        || input.component.physical_card_incarnation_id
            != Some(input.source_before_cast.incarnation_id)
        || input.component.source_layout != SourceLayout::Mutate
        || input.component.source_type_line != program.source_context.exact_type_line
        || !input.component.printed.is_creature()
        || input.stack_incarnation_id == input.source_before_cast.incarnation_id
    {
        return Err(FaceDownMergeRuntimeError::InvalidCastSource);
    }
    validate_mutate_target_snapshot(input.component.owner, target, &target_evidence)?;
    validate_mana_payment(
        &payment,
        input.caster,
        CostPaymentRole::MutateAlternativeCast,
        alternative_cost,
    )?;

    let stack_spell = ObjectRef {
        object_id: input.source_before_cast.object_id,
        incarnation_id: input.stack_incarnation_id,
    };
    let mut component = input.component;
    component.current_object = stack_spell;
    component.physical_card_incarnation_id = Some(stack_spell.incarnation_id);
    let spell = MutatingCreatureSpell {
        stack_id: input.stack_id,
        cast_event_id: input.cast_event_id,
        object: stack_spell,
        controller: input.caster,
        characteristics: component.printed.clone(),
        component,
        target: target.object,
        program_semantic_digest: program.semantic_digest.clone(),
        is_copy: false,
    };
    let receipt = MutateCastReceipt {
        cast_event_id: input.cast_event_id,
        stack_spell,
        target: target.object,
        alternative_cost: alternative_cost.clone(),
        semantic_digest: program.semantic_digest.clone(),
    };
    Ok((spell, receipt))
}

pub fn copy_mutating_creature_spell(
    spell: &MutatingCreatureSpell,
    stack_id: StackObjectId,
    object: ObjectRef,
    copy_controller: PlayerId,
    target: &PermanentState,
    target_evidence: MutateTargetEvidence,
) -> Result<MutatingCreatureSpell, FaceDownMergeRuntimeError> {
    if object == spell.object {
        return Err(FaceDownMergeRuntimeError::ReusedIncarnation);
    }
    validate_mutate_target_snapshot(copy_controller, target, &target_evidence)?;
    let mut component = spell.component.clone();
    component.component_id = component.component_id.wrapping_mul(1_000_003) ^ stack_id;
    component.current_object = object;
    component.owner = copy_controller;
    component.kind = ComponentKind::SpellCopyToken;
    component.physical_card_object_id = None;
    component.physical_card_incarnation_id = None;
    component.commander_id = None;
    Ok(MutatingCreatureSpell {
        stack_id,
        cast_event_id: spell.cast_event_id,
        object,
        controller: copy_controller,
        component,
        target: target.object,
        characteristics: spell.characteristics.clone(),
        program_semantic_digest: spell.program_semantic_digest.clone(),
        is_copy: true,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergePlacement {
    OnTop,
    Under,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutateResolutionEvidence {
    pub event_id: EventId,
    pub targets_revalidated: bool,
    pub spell_still_on_stack: bool,
    pub component_transition_complete: bool,
    pub battlefield_component_incarnation_id: IncarnationId,
    pub state_based_action_inputs_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingMutatesTrigger {
    pub trigger_id: TriggerId,
    pub controller: PlayerId,
    pub source: ObjectRef,
    pub granting_component_id: CardComponentId,
    pub ability_semantic_digest: String,
    pub waits_for_state_based_actions: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutateResolutionKind {
    Merged {
        placement: MergePlacement,
        resulting_object: ObjectRef,
        top_component: CardComponentId,
        component_order: Vec<CardComponentId>,
        is_token: bool,
        retained_counters: BTreeMap<String, u32>,
        retained_attachments: BTreeSet<AttachmentId>,
        retained_tapped: bool,
        retained_damage: u32,
        retained_attacking: bool,
        retained_blocking: bool,
        retained_controller: PlayerId,
        retained_control_since_turn: u64,
        commander_components: Vec<CommanderId>,
        pending_mutates_triggers: Vec<PendingMutatesTrigger>,
    },
    ResolvedAsNormalCreature {
        battlefield_object: ObjectRef,
        entered_battlefield: bool,
        is_token: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutateResolutionReceipt {
    pub event_id: EventId,
    pub stack_spell: ObjectRef,
    pub original_target: ObjectRef,
    pub target_was_legal_on_resolution: bool,
    pub caused_enters_event: bool,
    pub state_based_actions_due_before_priority: bool,
    pub kind: MutateResolutionKind,
}

pub fn resolve_mutating_creature_spell(
    program: &FaceDownMergeKeywordProgram,
    mut spell: MutatingCreatureSpell,
    target: Option<&mut PermanentState>,
    target_evidence: Option<MutateTargetEvidence>,
    placement: Option<MergePlacement>,
    evidence: MutateResolutionEvidence,
) -> Result<(Option<PermanentState>, MutateResolutionReceipt), FaceDownMergeRuntimeError> {
    if !matches!(program.kind, FaceDownMergeKeywordKind::Mutate { .. })
        || spell.program_semantic_digest != program.semantic_digest
    {
        return Err(FaceDownMergeRuntimeError::WrongProgramKind);
    }
    if !evidence.targets_revalidated
        || !evidence.spell_still_on_stack
        || !evidence.component_transition_complete
        || !evidence.state_based_action_inputs_complete
        || evidence.battlefield_component_incarnation_id == spell.object.incarnation_id
    {
        return Err(FaceDownMergeRuntimeError::IncompleteMutateResolutionEvidence);
    }

    let legal_target = match (target.as_deref(), target_evidence.as_ref()) {
        (Some(target), Some(target_evidence)) if target.object == spell.target => {
            match validate_mutate_target_snapshot(spell.component.owner, target, target_evidence) {
                Ok(()) => true,
                Err(FaceDownMergeRuntimeError::IllegalMutateTarget) => false,
                Err(error) => return Err(error),
            }
        }
        (None, None) => false,
        (Some(target), Some(_)) if target.object != spell.target => false,
        _ => return Err(FaceDownMergeRuntimeError::IncompleteMutateTargetEvidence),
    };

    if !legal_target {
        if placement.is_some() {
            return Err(FaceDownMergeRuntimeError::UnexpectedMergePlacement);
        }
        let battlefield_object = ObjectRef {
            object_id: spell.object.object_id,
            incarnation_id: evidence.battlefield_component_incarnation_id,
        };
        spell.component.current_object = battlefield_object;
        spell.component.physical_card_incarnation_id =
            Some(evidence.battlefield_component_incarnation_id);
        let permanent = PermanentState {
            object: battlefield_object,
            controller: spell.controller,
            components: vec![spell.component],
            characteristics: spell.characteristics,
            counters: BTreeMap::new(),
            attachments: BTreeSet::new(),
            tapped: false,
            damage_marked: 0,
            attacking: false,
            blocking: false,
            control_since_turn: 0,
        };
        permanent.validate_component_state()?;
        let receipt = MutateResolutionReceipt {
            event_id: evidence.event_id,
            stack_spell: spell.object,
            original_target: spell.target,
            target_was_legal_on_resolution: false,
            caused_enters_event: true,
            state_based_actions_due_before_priority: true,
            kind: MutateResolutionKind::ResolvedAsNormalCreature {
                battlefield_object,
                entered_battlefield: true,
                is_token: permanent.is_token(),
            },
        };
        return Ok((Some(permanent), receipt));
    }

    let placement = placement.ok_or(FaceDownMergeRuntimeError::MissingMergePlacement)?;
    let target = target.ok_or(FaceDownMergeRuntimeError::IncompleteMutateTargetEvidence)?;
    target.validate_component_state()?;
    let retained_counters = target.counters.clone();
    let retained_attachments = target.attachments.clone();
    let retained_tapped = target.tapped;
    let retained_damage = target.damage_marked;
    let retained_attacking = target.attacking;
    let retained_blocking = target.blocking;
    let retained_controller = target.controller;
    let retained_control_since_turn = target.control_since_turn;

    spell.component.current_object = target.object;
    if spell.component.kind == ComponentKind::PhysicalCard {
        spell.component.physical_card_incarnation_id =
            Some(evidence.battlefield_component_incarnation_id);
    }
    match placement {
        MergePlacement::OnTop => target.components.insert(0, spell.component),
        MergePlacement::Under => target.components.push(spell.component),
    }
    target.characteristics = merged_copiable_characteristics(&target.components)?;
    target.validate_component_state()?;

    if target.counters != retained_counters
        || target.attachments != retained_attachments
        || target.tapped != retained_tapped
        || target.damage_marked != retained_damage
        || target.attacking != retained_attacking
        || target.blocking != retained_blocking
        || target.controller != retained_controller
        || target.control_since_turn != retained_control_since_turn
    {
        return Err(FaceDownMergeRuntimeError::MutateDidNotRetainPermanentState);
    }

    let mut trigger_ordinal = 0u64;
    let mut pending_mutates_triggers = Vec::new();
    for component in &target.components {
        for trigger in &component.mutates_trigger_digests {
            trigger_ordinal = trigger_ordinal.wrapping_add(1);
            pending_mutates_triggers.push(PendingMutatesTrigger {
                trigger_id: evidence
                    .event_id
                    .wrapping_mul(1_000_003)
                    .wrapping_add(trigger_ordinal),
                controller: target.controller,
                source: target.object,
                granting_component_id: component.component_id,
                ability_semantic_digest: trigger.clone(),
                waits_for_state_based_actions: true,
            });
        }
    }

    let component_order = target
        .components
        .iter()
        .map(|component| component.component_id)
        .collect::<Vec<_>>();
    let top_component = component_order[0];
    let receipt = MutateResolutionReceipt {
        event_id: evidence.event_id,
        stack_spell: spell.object,
        original_target: spell.target,
        target_was_legal_on_resolution: true,
        caused_enters_event: false,
        state_based_actions_due_before_priority: true,
        kind: MutateResolutionKind::Merged {
            placement,
            resulting_object: target.object,
            top_component,
            component_order,
            is_token: target.is_token(),
            retained_counters,
            retained_attachments,
            retained_tapped,
            retained_damage,
            retained_attacking,
            retained_blocking,
            retained_controller,
            retained_control_since_turn,
            commander_components: target.commander_components(),
            pending_mutates_triggers,
        },
    };
    Ok((None, receipt))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedPermanentCopyReceipt {
    pub source: ObjectRef,
    pub copy: ObjectRef,
    pub copied_top_characteristics: CopiableCharacteristics,
    pub copied_ability_count: usize,
    pub component_count_copied: usize,
    pub commander_designation_copied: bool,
    pub counters_copied: bool,
    pub tapped_status_copied: bool,
}

pub fn create_token_copy_of_permanent(
    source: &PermanentState,
    copy: ObjectRef,
    owner: PlayerId,
    controller: PlayerId,
    component_id: CardComponentId,
) -> Result<(PermanentState, MergedPermanentCopyReceipt), FaceDownMergeRuntimeError> {
    source.validate_component_state()?;
    if copy == source.object {
        return Err(FaceDownMergeRuntimeError::ReusedIncarnation);
    }
    let copied_mutates_triggers = source
        .components
        .iter()
        .flat_map(|component| component.mutates_trigger_digests.iter().cloned())
        .collect::<Vec<_>>();
    let component = CardComponent {
        component_id,
        physical_card_object_id: None,
        physical_card_incarnation_id: None,
        current_object: copy,
        owner,
        kind: ComponentKind::Token,
        printed: source.characteristics.clone(),
        source_type_line: "Token copy".to_owned(),
        source_layout: SourceLayout::Normal,
        commander_id: None,
        turn_face_up_trigger_digests: Vec::new(),
        mutates_trigger_digests: copied_mutates_triggers,
    };
    let permanent = PermanentState {
        object: copy,
        controller,
        components: vec![component],
        characteristics: source.characteristics.clone(),
        counters: BTreeMap::new(),
        attachments: BTreeSet::new(),
        tapped: false,
        damage_marked: 0,
        attacking: false,
        blocking: false,
        control_since_turn: 0,
    };
    permanent.validate_component_state()?;
    let receipt = MergedPermanentCopyReceipt {
        source: source.object,
        copy,
        copied_top_characteristics: source.characteristics.clone(),
        copied_ability_count: source.characteristics.ability_semantic_digests.len(),
        component_count_copied: 1,
        commander_designation_copied: false,
        counters_copied: false,
        tapped_status_copied: false,
    };
    Ok((permanent, receipt))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommanderDestinationHandling {
    NotCommander,
    StayedWithBaseMove,
    ReplacedHandOrLibraryMoveWithCommandZone,
    EligibleForCommandZoneStateBasedActionAfterGraveyardOrExileMove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentMoveDecision {
    pub component_id: CardComponentId,
    pub actual_destination: Option<Zone>,
    pub new_object: Option<ObjectRef>,
    pub commander_hand_or_library_replacement_chosen: bool,
    pub decision_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedZoneChangeInput {
    pub event_id: EventId,
    pub base_destination: Zone,
    pub decisions: Vec<ComponentMoveDecision>,
    /// Required only for physical cards moving to the same library.
    pub library_relative_order_top_first: Vec<CardComponentId>,
    pub replacement_effects_complete: bool,
    pub owner_order_choice_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentMoveReceipt {
    pub component_id: CardComponentId,
    pub component_kind: ComponentKind,
    pub owner: PlayerId,
    pub from_object: ObjectRef,
    pub base_destination: Zone,
    pub actual_destination: Option<Zone>,
    pub new_object: Option<ObjectRef>,
    pub token_ceases_to_exist: bool,
    pub commander_handling: CommanderDestinationHandling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedZoneChangeReceipt {
    pub event_id: EventId,
    pub permanent: ObjectRef,
    pub component_moves: Vec<ComponentMoveReceipt>,
    pub all_components_moved: bool,
    pub counters_ceased: BTreeMap<String, u32>,
    pub damage_cleared: u32,
    pub attachments_left_on_battlefield_unattached: BTreeSet<AttachmentId>,
    pub state_based_actions_due_before_priority: bool,
}

pub fn move_merged_permanent_from_battlefield(
    permanent: PermanentState,
    input: MergedZoneChangeInput,
) -> Result<MergedZoneChangeReceipt, FaceDownMergeRuntimeError> {
    permanent.validate_component_state()?;
    if input.base_destination == Zone::Battlefield
        || !input.replacement_effects_complete
        || !input.owner_order_choice_complete
    {
        return Err(FaceDownMergeRuntimeError::IncompleteZoneChangeEvidence);
    }
    let decisions = input
        .decisions
        .iter()
        .map(|decision| (decision.component_id, decision))
        .collect::<BTreeMap<_, _>>();
    if decisions.len() != input.decisions.len()
        || decisions.len() != permanent.components.len()
        || input
            .decisions
            .iter()
            .any(|decision| !decision.decision_complete)
    {
        return Err(FaceDownMergeRuntimeError::IncompleteZoneChangeEvidence);
    }

    let physical_to_library = permanent
        .components
        .iter()
        .filter(|component| component.kind == ComponentKind::PhysicalCard)
        .filter(|component| {
            decisions
                .get(&component.component_id)
                .is_some_and(|decision| decision.actual_destination == Some(Zone::Library))
        })
        .map(|component| component.component_id)
        .collect::<BTreeSet<_>>();
    let supplied_library_order = input
        .library_relative_order_top_first
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if physical_to_library != supplied_library_order
        || supplied_library_order.len() != input.library_relative_order_top_first.len()
    {
        return Err(FaceDownMergeRuntimeError::InvalidLibraryComponentOrder);
    }

    let mut component_moves = Vec::with_capacity(permanent.components.len());
    for component in &permanent.components {
        let decision = decisions
            .get(&component.component_id)
            .ok_or(FaceDownMergeRuntimeError::IncompleteZoneChangeEvidence)?;
        let token = component.kind != ComponentKind::PhysicalCard;
        let commander_hand_or_library = component.commander_id.is_some()
            && matches!(input.base_destination, Zone::Hand | Zone::Library);
        let expected_destination =
            if commander_hand_or_library && decision.commander_hand_or_library_replacement_chosen {
                Some(Zone::Command)
            } else {
                Some(input.base_destination)
            };
        if decision.actual_destination != expected_destination {
            return Err(FaceDownMergeRuntimeError::InvalidComponentDestination);
        }
        if decision.commander_hand_or_library_replacement_chosen && !commander_hand_or_library {
            return Err(FaceDownMergeRuntimeError::InvalidCommanderDestinationChoice);
        }

        if token {
            if decision.new_object.is_some() {
                return Err(FaceDownMergeRuntimeError::InvalidComponentIncarnation);
            }
        } else {
            let next = decision
                .new_object
                .ok_or(FaceDownMergeRuntimeError::InvalidComponentIncarnation)?;
            if Some(next.object_id) != component.physical_card_object_id
                || Some(next.incarnation_id) == component.physical_card_incarnation_id
            {
                return Err(FaceDownMergeRuntimeError::InvalidComponentIncarnation);
            }
        }
        let commander_handling = match (
            component.commander_id,
            input.base_destination,
            decision.commander_hand_or_library_replacement_chosen,
        ) {
            (None, _, _) => CommanderDestinationHandling::NotCommander,
            (Some(_), Zone::Hand | Zone::Library, true) => {
                CommanderDestinationHandling::ReplacedHandOrLibraryMoveWithCommandZone
            }
            (Some(_), Zone::Graveyard | Zone::Exile, false) => {
                CommanderDestinationHandling::EligibleForCommandZoneStateBasedActionAfterGraveyardOrExileMove
            }
            (Some(_), _, false) => CommanderDestinationHandling::StayedWithBaseMove,
            (Some(_), _, true) => {
                return Err(FaceDownMergeRuntimeError::InvalidCommanderDestinationChoice)
            }
        };
        component_moves.push(ComponentMoveReceipt {
            component_id: component.component_id,
            component_kind: component.kind,
            owner: component.owner,
            from_object: permanent.object,
            base_destination: input.base_destination,
            actual_destination: decision.actual_destination,
            new_object: decision.new_object,
            token_ceases_to_exist: token,
            commander_handling,
        });
    }
    Ok(MergedZoneChangeReceipt {
        event_id: input.event_id,
        permanent: permanent.object,
        component_moves,
        all_components_moved: true,
        counters_ceased: permanent.counters,
        damage_cleared: permanent.damage_marked,
        attachments_left_on_battlefield_unattached: permanent.attachments,
        state_based_actions_due_before_priority: true,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommanderDamageAttribution {
    pub commander_id: CommanderId,
    pub damaged_player: PlayerId,
    pub combat_damage_dealt: u32,
    pub source_permanent: ObjectRef,
}

pub fn attribute_merged_commander_combat_damage(
    permanent: &PermanentState,
    damaged_player: PlayerId,
    actual_damage_dealt: u32,
    was_combat_damage: bool,
    damage_event_complete: bool,
) -> Result<Vec<CommanderDamageAttribution>, FaceDownMergeRuntimeError> {
    permanent.validate_component_state()?;
    if !damage_event_complete {
        return Err(FaceDownMergeRuntimeError::IncompleteDamageEvidence);
    }
    if !was_combat_damage || actual_damage_dealt == 0 {
        return Ok(Vec::new());
    }
    Ok(permanent
        .commander_components()
        .into_iter()
        .map(|commander_id| CommanderDamageAttribution {
            commander_id,
            damaged_player,
            combat_damage_dealt: actual_damage_dealt,
            source_permanent: permanent.object,
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceDownMergeRuntimeError {
    WrongProgramKind,
    WrongZone,
    WrongPayer {
        expected: PlayerId,
        actual: PlayerId,
    },
    WrongController {
        expected: PlayerId,
        actual: PlayerId,
    },
    PaymentContractMismatch,
    IncompletePaymentEvidence,
    DuplicatePaymentObject(ObjectRef),
    MissingXChoice,
    UnexpectedXChoice,
    UnexpectedPaymentEvidence,
    InvalidCostObjectTransition,
    CostObjectDoesNotMatch,
    CardWasNotRevealed,
    IncompleteCastEvidence,
    InvalidCastSource,
    InvalidResolutionTransition,
    NotCanonicalFaceDownObject,
    IncompleteRevealEvidence,
    SpecialActionTimingUnavailable,
    NoTurnFaceUpAuthority,
    HiddenFaceContextMismatch,
    MissingCounterPlacementEvidence,
    UnexpectedCounterPlacementEvidence,
    InvalidCounterPlacementEvidence,
    CounterOverflow,
    InvalidWardTargetEvidence,
    IncompleteCounterabilityEvidence,
    MissingPermanentComponents,
    ComponentOwnerMismatch,
    InvalidMergedComponentState,
    MergedCharacteristicsMismatch,
    IncompleteMutateTargetEvidence,
    IllegalMutateTarget,
    ReusedIncarnation,
    IncompleteMutateResolutionEvidence,
    UnexpectedMergePlacement,
    MissingMergePlacement,
    MutateDidNotRetainPermanentState,
    IncompleteZoneChangeEvidence,
    InvalidLibraryComponentOrder,
    InvalidComponentDestination,
    InvalidCommanderDestinationChoice,
    InvalidComponentIncarnation,
    IncompleteDamageEvidence,
}

impl fmt::Display for FaceDownMergeRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProgramKind => {
                formatter.write_str("the keyword program kind does not match")
            }
            Self::WrongZone => formatter.write_str("the object is in the wrong zone"),
            Self::WrongPayer { expected, actual } => {
                write!(formatter, "expected payer {expected}, got {actual}")
            }
            Self::WrongController { expected, actual } => {
                write!(formatter, "expected controller {expected}, got {actual}")
            }
            Self::DuplicatePaymentObject(object) => {
                write!(
                    formatter,
                    "payment object {object:?} was used more than once"
                )
            }
            other => write!(formatter, "{other:?}"),
        }
    }
}

impl std::error::Error for FaceDownMergeRuntimeError {}
