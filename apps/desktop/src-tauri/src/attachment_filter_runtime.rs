//! Content keyed attachment filters and transactions that are not owned by the
//! existing narrow Enchant and Equip production bridges.
//!
//! This module is intentionally disconnected from production execution. It
//! compiles only complete, reviewed Oracle lines and keeps spell targeting,
//! cost payment, resolution, continuous attachment legality, and state based
//! actions as distinct transactions.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const ATTACHMENT_FILTER_COMPILER_VERSION: &str = "attachment-filter-compiler-0.1";
pub const ATTACHMENT_FILTER_RUNTIME_VERSION: &str = "attachment-filter-runtime-0.1";
pub const ATTACHMENT_FILTER_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:107.1,109.2,115,118.3,118.7,301.5,303.4,601.2,608.2b,704.5m,702.5,702.6,702.103";

const BESTOW_REMINDER_SUFFIX: &str = " (If you cast this card for its bestow cost, it's an Aura spell with enchant creature. It becomes a creature again if it's not attached.)";
const ENCHANT_CARD_IN_HAND_EXACT: &str = "Enchant card in your hand (This Aura remains on the battlefield. If you play enchanted card or it otherwise leaves your hand, put this Aura into the graveyard.)";
const ENCHANT_MODIFIED_CREATURE_EXACT: &str = "Enchant modified creature (Equipment, Auras its controller controls, and counters are modifications.)";
const ENCHANT_ZONE_EXACT: &str = "Enchant zone (Battlefield, command, exile, and stack are shared zones. Each player has their own graveyard, hand, and library zones.)";
const EQUIP_PHYREXIAN_EXACT: &str =
    "Equip {B/P}{B/P} ({B/P} can be paid with either {B} or 2 life.)";
const EQUIP_WORTHY_EXACT: &str = "Equip worthy {1} (A creature is worthy if it's a legendary non-Villain that's red and/or white.)";
const EQUIP_HALFLING_EXACT: &str =
    "Equip Halfling {1} ({1}: Attach to target Halfling you control. Equip only as a sorcery.)";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type SpellId = u64;
pub type ActivationId = u64;
pub type ExternalPaymentId = u64;

/// No clause in this file can produce a production receipt until a main engine
/// adapter supplies authoritative objects, targets, costs, and state based
/// action ordering.
pub const fn attachment_filter_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttachmentSourceContext {
    AuraEnchantment,
    ArtifactEquipment,
    BestowEnchantmentCreature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttachmentLayoutContext {
    PermanentFace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
}

impl ManaColor {
    const fn contract_label(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Blue => "blue",
            Self::Black => "black",
            Self::Red => "red",
            Self::Green => "green",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaSymbol {
    Generic(u16),
    Colored(ManaColor),
    Phyrexian(ManaColor),
    VariableX,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaAlternative {
    symbols: Vec<ManaSymbol>,
}

impl ManaAlternative {
    pub fn symbols(&self) -> &[ManaSymbol] {
        &self.symbols
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaCost {
    alternatives: Vec<ManaAlternative>,
}

impl ManaCost {
    pub fn alternatives(&self) -> &[ManaAlternative] {
        &self.alternatives
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardType {
    Artifact,
    Battle,
    Creature,
    Enchantment,
    Instant,
    Land,
    Planeswalker,
    Sorcery,
}

impl CardType {
    const fn contract_label(self) -> &'static str {
        match self {
            Self::Artifact => "artifact",
            Self::Battle => "battle",
            Self::Creature => "creature",
            Self::Enchantment => "enchantment",
            Self::Instant => "instant",
            Self::Land => "land",
            Self::Planeswalker => "planeswalker",
            Self::Sorcery => "sorcery",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    Battlefield,
    Command,
    Exile,
    Graveyard,
    Hand,
    Library,
    Stack,
}

impl Zone {
    const fn contract_label(self) -> &'static str {
        match self {
            Self::Battlefield => "battlefield",
            Self::Command => "command",
            Self::Exile => "exile",
            Self::Graveyard => "graveyard",
            Self::Hand => "hand",
            Self::Library => "library",
            Self::Stack => "stack",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelativePlayer {
    You,
    Opponent,
    Any,
}

impl RelativePlayer {
    const fn contract_label(self) -> &'static str {
        match self {
            Self::You => "you",
            Self::Opponent => "opponent",
            Self::Any => "any",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectPredicate {
    Permanent,
    CardType(CardType),
    Subtype(String),
    Supertype(String),
    Color(ManaColor),
    Token,
    Commander,
    Tapped,
    HasFlying,
    Modified,
    HasAnotherAuraAttached,
    PowerAtMost(i32),
    ManaValueAtMost(u32),
    Controller(RelativePlayer),
    Owner(RelativePlayer),
    ZonePlayer(RelativePlayer),
    Zone(Zone),
    Not(Box<ObjectPredicate>),
    All(Vec<ObjectPredicate>),
    Any(Vec<ObjectPredicate>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ZoneOwner {
    Shared,
    Player(RelativePlayer),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttachmentFilter {
    Object(ObjectPredicate),
    Player(RelativePlayer),
    Zone {
        zone: Option<Zone>,
        owner: Option<ZoneOwner>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttachmentFilterProgramKind {
    Enchant {
        filter: AttachmentFilter,
    },
    Equip {
        activation_cost: ManaCost,
        target_filter: ObjectPredicate,
        legal_attachment_filter: ObjectPredicate,
    },
    Bestow {
        alternate_cost: ManaCost,
        enchant_filter: AttachmentFilter,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttachmentFilterSemanticIdentity {
    exact_oracle_clause: String,
    normalized_oracle_clause: String,
    source_context: AttachmentSourceContext,
    layout_context: AttachmentLayoutContext,
    compiler_version: &'static str,
    runtime_version: &'static str,
    rules_context_version: &'static str,
    typed_contract: String,
    semantic_digest: String,
}

impl AttachmentFilterSemanticIdentity {
    pub fn exact_oracle_clause(&self) -> &str {
        &self.exact_oracle_clause
    }

    pub fn normalized_oracle_clause(&self) -> &str {
        &self.normalized_oracle_clause
    }

    pub const fn source_context(&self) -> AttachmentSourceContext {
        self.source_context
    }

    pub const fn layout_context(&self) -> AttachmentLayoutContext {
        self.layout_context
    }

    pub fn typed_contract(&self) -> &str {
        &self.typed_contract
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentFilterProgram {
    identity: AttachmentFilterSemanticIdentity,
    kind: AttachmentFilterProgramKind,
}

impl AttachmentFilterProgram {
    pub fn identity(&self) -> &AttachmentFilterSemanticIdentity {
        &self.identity
    }

    pub fn kind(&self) -> &AttachmentFilterProgramKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        attachment_filter_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AttachmentFilterCompilerInput<'a> {
    pub exact_oracle_clause: &'a str,
    pub source_type_line: &'a str,
    pub source_layout: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorAttachmentOwner {
    EnchantProductionRuntime,
    EquipProductionRuntime,
}

pub fn prior_attachment_owner(
    input: AttachmentFilterCompilerInput<'_>,
) -> Option<PriorAttachmentOwner> {
    let exact = input.exact_oracle_clause;
    if exact.starts_with("Enchant ")
        && source_context(input.source_type_line, exact)
            == Some(AttachmentSourceContext::AuraEnchantment)
        && earlier_owned_enchant_clause(exact)
    {
        return Some(PriorAttachmentOwner::EnchantProductionRuntime);
    }
    if exact.starts_with("Equip ")
        && source_context(input.source_type_line, exact)
            == Some(AttachmentSourceContext::ArtifactEquipment)
        && earlier_owned_equip_clause(exact)
    {
        return Some(PriorAttachmentOwner::EquipProductionRuntime);
    }
    None
}

/// Compiles only a complete line. Card names, snapshot identifiers, database
/// rows, face indexes, and clause indexes are not accepted as inputs.
pub fn compile_attachment_filter_program(
    input: AttachmentFilterCompilerInput<'_>,
) -> Option<AttachmentFilterProgram> {
    let exact = input.exact_oracle_clause;
    if !is_canonical_complete_line(exact) || prior_attachment_owner(input).is_some() {
        return None;
    }
    let source_context = source_context(input.source_type_line, exact)?;
    let layout_context = layout_context(input.source_layout)?;
    let kind = if exact.starts_with("Enchant ") {
        AttachmentFilterProgramKind::Enchant {
            filter: parse_residual_enchant_filter(exact)?,
        }
    } else if exact.starts_with("Equip ") {
        let (activation_cost, target_filter, legal_attachment_filter) =
            parse_residual_equip(exact)?;
        AttachmentFilterProgramKind::Equip {
            activation_cost,
            target_filter,
            legal_attachment_filter,
        }
    } else if exact.starts_with("Bestow ") {
        let alternate_cost = parse_complete_bestow(exact)?;
        AttachmentFilterProgramKind::Bestow {
            alternate_cost,
            enchant_filter: AttachmentFilter::Object(all([
                ObjectPredicate::Zone(Zone::Battlefield),
                ObjectPredicate::CardType(CardType::Creature),
            ])),
        }
    } else {
        return None;
    };
    let normalized_oracle_clause = normalized_oracle_clause(exact);
    let typed_contract = typed_program_contract(&kind);
    let semantic_digest = semantic_digest(
        exact,
        &normalized_oracle_clause,
        source_context,
        layout_context,
        &typed_contract,
    );
    Some(AttachmentFilterProgram {
        identity: AttachmentFilterSemanticIdentity {
            exact_oracle_clause: exact.to_owned(),
            normalized_oracle_clause,
            source_context,
            layout_context,
            compiler_version: ATTACHMENT_FILTER_COMPILER_VERSION,
            runtime_version: ATTACHMENT_FILTER_RUNTIME_VERSION,
            rules_context_version: ATTACHMENT_FILTER_RULES_CONTEXT_VERSION,
            typed_contract,
            semantic_digest,
        },
        kind,
    })
}

fn is_canonical_complete_line(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !value.contains(['\r', '\n'])
        && collapse_whitespace(value) == value
}

fn source_context(type_line: &str, exact: &str) -> Option<AttachmentSourceContext> {
    let words = normalized_type_words(type_line);
    if exact.starts_with("Enchant ") && words.contains("enchantment") && words.contains("aura") {
        Some(AttachmentSourceContext::AuraEnchantment)
    } else if exact.starts_with("Equip ")
        && words.contains("artifact")
        && words.contains("equipment")
    {
        Some(AttachmentSourceContext::ArtifactEquipment)
    } else if exact.starts_with("Bestow ")
        && words.contains("enchantment")
        && words.contains("creature")
    {
        Some(AttachmentSourceContext::BestowEnchantmentCreature)
    } else {
        None
    }
}

fn normalized_type_words(type_line: &str) -> BTreeSet<&str> {
    type_line
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            if part.eq_ignore_ascii_case("enchantment") {
                "enchantment"
            } else if part.eq_ignore_ascii_case("artifact") {
                "artifact"
            } else if part.eq_ignore_ascii_case("equipment") {
                "equipment"
            } else if part.eq_ignore_ascii_case("creature") {
                "creature"
            } else if part.eq_ignore_ascii_case("aura") {
                "aura"
            } else {
                ""
            }
        })
        .filter(|part| !part.is_empty())
        .collect()
}

fn layout_context(layout: &str) -> Option<AttachmentLayoutContext> {
    let normalized = layout.trim().to_ascii_lowercase();
    // Whether this permanent face is the root of a normal card or one face of
    // a transforming card does not alter Enchant, Equip, or Bestow rules.
    if normalized.is_empty() || matches!(normalized.as_str(), "normal" | "transform") {
        Some(AttachmentLayoutContext::PermanentFace)
    } else {
        None
    }
}

fn earlier_owned_enchant_clause(exact: &str) -> bool {
    matches!(
        exact,
        "Enchant artifact"
            | "Enchant artifact or creature"
            | "Enchant artifact or creature you control"
            | "Enchant artifact you control"
            | "Enchant creature"
            | "Enchant creature you control"
            | "Enchant enchantment"
            | "Enchant land"
            | "Enchant land you control"
            | "Enchant nonland permanent"
            | "Enchant permanent"
            | "Enchant planeswalker"
            | "Enchant tapped creature"
            | "Enchant creature (Target a creature as you cast this. This card enters attached to that creature.)"
            | "Enchant land (Target a land as you cast this. This card enters attached to that land.)"
    )
}

fn earlier_owned_equip_clause(exact: &str) -> bool {
    let Some(body) = exact.strip_prefix("Equip ") else {
        return false;
    };
    let (head, reminder) = split_parenthetical_suffix(body);
    if head.contains('.') {
        return false;
    }
    let Some(cost_start) = head.find('{') else {
        return false;
    };
    let quality = head[..cost_start].trim();
    if !matches!(
        quality,
        "" | "legendary creature" | "commander" | "planeswalker"
    ) {
        return false;
    }
    let Some(cost) = parse_mana_cost(&head[cost_start..]) else {
        return false;
    };
    if cost.alternatives.len() != 1
        || !cost.alternatives[0]
            .symbols
            .iter()
            .all(|symbol| matches!(symbol, ManaSymbol::Generic(_) | ManaSymbol::Colored(_)))
    {
        return false;
    }
    let Some(reminder) = reminder else {
        return true;
    };
    let target = match quality {
        "" => "creature",
        "legendary creature" => "legendary creature",
        "commander" => "commander",
        "planeswalker" => "planeswalker",
        _ => return false,
    };
    reminder
        == format!(
            "{}: Attach to target {target} you control. Equip only as a sorcery.",
            &head[cost_start..]
        )
}

fn parse_residual_enchant_filter(exact: &str) -> Option<AttachmentFilter> {
    if exact == "Enchant two creatures" {
        return None;
    }
    if exact == ENCHANT_CARD_IN_HAND_EXACT {
        return Some(AttachmentFilter::Object(all([
            ObjectPredicate::Zone(Zone::Hand),
            ObjectPredicate::ZonePlayer(RelativePlayer::You),
        ])));
    }
    if exact == ENCHANT_MODIFIED_CREATURE_EXACT {
        return Some(AttachmentFilter::Object(all([
            ObjectPredicate::Zone(Zone::Battlefield),
            ObjectPredicate::CardType(CardType::Creature),
            ObjectPredicate::Modified,
        ])));
    }
    if exact == ENCHANT_ZONE_EXACT {
        return Some(AttachmentFilter::Zone {
            zone: None,
            owner: None,
        });
    }
    let mut filter = exact.strip_prefix("Enchant ")?;
    if filter == "nonland permanent." {
        filter = "nonland permanent";
    } else if filter.ends_with('.') || filter.contains(['(', ')']) {
        return None;
    }
    match filter {
        "player" => return Some(AttachmentFilter::Player(RelativePlayer::Any)),
        "opponent" => return Some(AttachmentFilter::Player(RelativePlayer::Opponent)),
        "your graveyard" => {
            return Some(AttachmentFilter::Zone {
                zone: Some(Zone::Graveyard),
                owner: Some(ZoneOwner::Player(RelativePlayer::You)),
            });
        }
        "your library" => {
            return Some(AttachmentFilter::Zone {
                zone: Some(Zone::Library),
                owner: Some(ZoneOwner::Player(RelativePlayer::You)),
            });
        }
        "creature card in a graveyard" => {
            return Some(AttachmentFilter::Object(all([
                ObjectPredicate::Zone(Zone::Graveyard),
                ObjectPredicate::CardType(CardType::Creature),
            ])));
        }
        "instant card in a graveyard" => {
            return Some(AttachmentFilter::Object(all([
                ObjectPredicate::Zone(Zone::Graveyard),
                ObjectPredicate::CardType(CardType::Instant),
            ])));
        }
        "instant or sorcery spell on the stack" => {
            return Some(AttachmentFilter::Object(all([
                ObjectPredicate::Zone(Zone::Stack),
                any([
                    ObjectPredicate::CardType(CardType::Instant),
                    ObjectPredicate::CardType(CardType::Sorcery),
                ]),
            ])));
        }
        _ => {}
    }

    let (base, controller) = if let Some(base) = filter.strip_suffix(" you control") {
        (base, Some(RelativePlayer::You))
    } else if let Some(base) = filter.strip_suffix(" an opponent controls") {
        (base, Some(RelativePlayer::Opponent))
    } else if let Some(base) = filter.strip_suffix(" you don't control") {
        (base, Some(RelativePlayer::Opponent))
    } else {
        (filter, None)
    };
    let mut predicates = vec![ObjectPredicate::Zone(Zone::Battlefield)];
    if let Some(controller) = controller {
        predicates.push(ObjectPredicate::Controller(controller));
    }
    predicates.push(parse_battlefield_enchant_quality(base)?);
    Some(AttachmentFilter::Object(ObjectPredicate::All(predicates)))
}

fn parse_battlefield_enchant_quality(filter: &str) -> Option<ObjectPredicate> {
    let card_type = |card_type| ObjectPredicate::CardType(card_type);
    let subtype = |subtype: &str| ObjectPredicate::Subtype(subtype.to_owned());
    let creature = || card_type(CardType::Creature);
    let permanent_union = |members: Vec<ObjectPredicate>| any(members);
    Some(match filter {
        "artifact an opponent controls" | "creature an opponent controls" => return None,
        "artifact" => card_type(CardType::Artifact),
        "creature" => creature(),
        "enchantment" => card_type(CardType::Enchantment),
        "land" => card_type(CardType::Land),
        "planeswalker" => card_type(CardType::Planeswalker),
        "permanent" => ObjectPredicate::Permanent,
        "nonland permanent" => all([
            ObjectPredicate::Permanent,
            ObjectPredicate::Not(Box::new(card_type(CardType::Land))),
        ]),
        "artifact creature" => all([card_type(CardType::Artifact), creature()]),
        "artifact or enchantment" => permanent_union(vec![
            card_type(CardType::Artifact),
            card_type(CardType::Enchantment),
        ]),
        "artifact or land" => permanent_union(vec![
            card_type(CardType::Artifact),
            card_type(CardType::Land),
        ]),
        "creature or Vehicle" => permanent_union(vec![creature(), subtype("Vehicle")]),
        "creature or Food" => permanent_union(vec![creature(), subtype("Food")]),
        "creature or enchantment" => {
            permanent_union(vec![creature(), card_type(CardType::Enchantment)])
        }
        "creature or land" => permanent_union(vec![creature(), card_type(CardType::Land)]),
        "creature or planeswalker" => {
            permanent_union(vec![creature(), card_type(CardType::Planeswalker)])
        }
        "creature or Spacecraft" => permanent_union(vec![creature(), subtype("Spacecraft")]),
        "artifact, creature, or planeswalker" => permanent_union(vec![
            card_type(CardType::Artifact),
            creature(),
            card_type(CardType::Planeswalker),
        ]),
        "creature, planeswalker, or Clue" => permanent_union(vec![
            creature(),
            card_type(CardType::Planeswalker),
            subtype("Clue"),
        ]),
        "creature, land, or planeswalker" => permanent_union(vec![
            creature(),
            card_type(CardType::Land),
            card_type(CardType::Planeswalker),
        ]),
        "Forest or Plains" => permanent_union(vec![subtype("Forest"), subtype("Plains")]),
        "Forest" | "Mountain" | "Plains" | "Swamp" | "Island" | "Equipment" | "Giant" | "Wall" => {
            subtype(filter)
        }
        "basic land" => all([
            ObjectPredicate::Supertype("Basic".to_owned()),
            card_type(CardType::Land),
        ]),
        "snow land" => all([
            ObjectPredicate::Supertype("Snow".to_owned()),
            card_type(CardType::Land),
        ]),
        "nonbasic land" => all([
            card_type(CardType::Land),
            ObjectPredicate::Not(Box::new(ObjectPredicate::Supertype("Basic".to_owned()))),
        ]),
        "legendary creature" => all([
            creature(),
            ObjectPredicate::Supertype("Legendary".to_owned()),
        ]),
        "non-Wall creature" => all([creature(), ObjectPredicate::Not(Box::new(subtype("Wall")))]),
        "nonblack creature" => all([
            creature(),
            ObjectPredicate::Not(Box::new(ObjectPredicate::Color(ManaColor::Black))),
        ]),
        "noncommander creature" => all([
            creature(),
            ObjectPredicate::Not(Box::new(ObjectPredicate::Commander)),
        ]),
        "black creature" => all([creature(), ObjectPredicate::Color(ManaColor::Black)]),
        "green creature" => all([creature(), ObjectPredicate::Color(ManaColor::Green)]),
        "red or green creature" => all([
            creature(),
            any([
                ObjectPredicate::Color(ManaColor::Red),
                ObjectPredicate::Color(ManaColor::Green),
            ]),
        ]),
        "green or white creature" => all([
            creature(),
            any([
                ObjectPredicate::Color(ManaColor::Green),
                ObjectPredicate::Color(ManaColor::White),
            ]),
        ]),
        "creature without flying" => all([
            creature(),
            ObjectPredicate::Not(Box::new(ObjectPredicate::HasFlying)),
        ]),
        "creature with another Aura attached to it" => {
            all([creature(), ObjectPredicate::HasAnotherAuraAttached])
        }
        "creature with power 3 or less" => all([creature(), ObjectPredicate::PowerAtMost(3)]),
        "creature with mana value 2 or less" => {
            all([creature(), ObjectPredicate::ManaValueAtMost(2)])
        }
        _ => return None,
    })
}

fn parse_residual_equip(exact: &str) -> Option<(ManaCost, ObjectPredicate, ObjectPredicate)> {
    let creature = || ObjectPredicate::CardType(CardType::Creature);
    let controller = || ObjectPredicate::Controller(RelativePlayer::You);
    let battlefield = || ObjectPredicate::Zone(Zone::Battlefield);
    let creature_legality = || creature();

    if exact == EQUIP_PHYREXIAN_EXACT {
        return Some((
            parse_mana_cost("{B/P}{B/P}")?,
            all([battlefield(), controller(), creature()]),
            creature_legality(),
        ));
    }
    if exact == EQUIP_WORTHY_EXACT {
        let worthy = all([
            creature(),
            ObjectPredicate::Supertype("Legendary".to_owned()),
            ObjectPredicate::Not(Box::new(ObjectPredicate::Subtype("Villain".to_owned()))),
            any([
                ObjectPredicate::Color(ManaColor::Red),
                ObjectPredicate::Color(ManaColor::White),
            ]),
        ]);
        return Some((
            parse_mana_cost("{1}")?,
            all([battlefield(), controller(), worthy]),
            creature_legality(),
        ));
    }
    if exact == EQUIP_HALFLING_EXACT {
        return Some((
            parse_mana_cost("{1}")?,
            all([
                battlefield(),
                controller(),
                creature(),
                ObjectPredicate::Subtype("Halfling".to_owned()),
            ]),
            creature_legality(),
        ));
    }

    let body = exact.strip_prefix("Equip ")?;
    if body.contains(['.', '(', ')']) {
        return None;
    }
    let cost_start = body.find('{')?;
    let quality = body[..cost_start].trim();
    let cost = parse_mana_cost(&body[cost_start..])?;
    let quality_filter = match quality {
        "" if cost.alternatives.len() == 2 => creature(),
        "creature token" => all([creature(), ObjectPredicate::Token]),
        "Wizard" | "Human" | "Citizen" | "Elf" | "Detective" | "Hero" | "Pirate" | "Knight"
        | "Soldier" => all([creature(), ObjectPredicate::Subtype(quality.to_owned())]),
        "Shaman, Warlock, or Wizard" => all([
            creature(),
            any([
                ObjectPredicate::Subtype("Shaman".to_owned()),
                ObjectPredicate::Subtype("Warlock".to_owned()),
                ObjectPredicate::Subtype("Wizard".to_owned()),
            ]),
        ]),
        "creature or planeswalker" => any([
            creature(),
            ObjectPredicate::CardType(CardType::Planeswalker),
        ]),
        _ => return None,
    };
    let legal_attachment_filter = if quality == "creature or planeswalker" {
        any([
            creature(),
            ObjectPredicate::CardType(CardType::Planeswalker),
        ])
    } else {
        creature_legality()
    };
    Some((
        cost,
        all([battlefield(), controller(), quality_filter]),
        legal_attachment_filter,
    ))
}

fn parse_complete_bestow(exact: &str) -> Option<ManaCost> {
    let body = exact
        .strip_prefix("Bestow ")?
        .strip_suffix(BESTOW_REMINDER_SUFFIX)?;
    parse_mana_cost(body)
}

fn split_parenthetical_suffix(value: &str) -> (&str, Option<&str>) {
    let Some((head, tail)) = value.split_once(" (") else {
        return (value, None);
    };
    let Some(reminder) = tail.strip_suffix(')') else {
        return (value, None);
    };
    (head, Some(reminder))
}

fn parse_mana_cost(value: &str) -> Option<ManaCost> {
    let alternatives = value
        .split(" or ")
        .map(parse_mana_alternative)
        .collect::<Option<Vec<_>>>()?;
    if alternatives.is_empty() || alternatives.len() > 2 {
        return None;
    }
    Some(ManaCost { alternatives })
}

fn parse_mana_alternative(value: &str) -> Option<ManaAlternative> {
    if value.is_empty() {
        return None;
    }
    let bytes = value.as_bytes();
    let mut cursor = 0usize;
    let mut symbols = Vec::new();
    while cursor < bytes.len() {
        if bytes[cursor] != b'{' {
            return None;
        }
        let relative_end = value[cursor + 1..].find('}')?;
        let end = cursor + 1 + relative_end;
        let token = &value[cursor + 1..end];
        let symbol = match token {
            "W" => ManaSymbol::Colored(ManaColor::White),
            "U" => ManaSymbol::Colored(ManaColor::Blue),
            "B" => ManaSymbol::Colored(ManaColor::Black),
            "R" => ManaSymbol::Colored(ManaColor::Red),
            "G" => ManaSymbol::Colored(ManaColor::Green),
            "W/P" => ManaSymbol::Phyrexian(ManaColor::White),
            "U/P" => ManaSymbol::Phyrexian(ManaColor::Blue),
            "B/P" => ManaSymbol::Phyrexian(ManaColor::Black),
            "R/P" => ManaSymbol::Phyrexian(ManaColor::Red),
            "G/P" => ManaSymbol::Phyrexian(ManaColor::Green),
            "X" => ManaSymbol::VariableX,
            _ if token.bytes().all(|byte| byte.is_ascii_digit())
                && (token.len() == 1 || !token.starts_with('0')) =>
            {
                ManaSymbol::Generic(token.parse().ok()?)
            }
            _ => return None,
        };
        symbols.push(symbol);
        cursor = end + 1;
    }
    if symbols.is_empty() {
        None
    } else {
        Some(ManaAlternative { symbols })
    }
}

fn all(predicates: impl IntoIterator<Item = ObjectPredicate>) -> ObjectPredicate {
    ObjectPredicate::All(predicates.into_iter().collect())
}

fn any(predicates: impl IntoIterator<Item = ObjectPredicate>) -> ObjectPredicate {
    ObjectPredicate::Any(predicates.into_iter().collect())
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalized_oracle_clause(value: &str) -> String {
    value.replace('\u{2019}', "'").to_ascii_lowercase()
}

fn typed_program_contract(kind: &AttachmentFilterProgramKind) -> String {
    match kind {
        AttachmentFilterProgramKind::Enchant { filter } => {
            format!(
                "enchant/v1;spell-target=true;continuous-legality=true;sba-illegal-or-unattached=graveyard;filter={}",
                filter_contract(filter)
            )
        }
        AttachmentFilterProgramKind::Equip {
            activation_cost,
            target_filter,
            legal_attachment_filter,
        } => format!(
            "equip/v1;cost={};timing=active-main-empty-stack-priority;target={};resolution-revalidation=true;move-attachment=true;continuous-legality={}",
            mana_cost_contract(activation_cost),
            predicate_contract(target_filter),
            predicate_contract(legal_attachment_filter)
        ),
        AttachmentFilterProgramKind::Bestow {
            alternate_cost,
            enchant_filter,
        } => format!(
            "bestow/v1;alternate-cost={};stack-type=enchantment-aura-not-creature;filter={};illegal-resolution=creature-permanent;unattached=end-bestow-effect",
            mana_cost_contract(alternate_cost),
            filter_contract(enchant_filter)
        ),
    }
}

fn mana_cost_contract(cost: &ManaCost) -> String {
    cost.alternatives
        .iter()
        .map(|alternative| {
            alternative
                .symbols
                .iter()
                .map(|symbol| match symbol {
                    ManaSymbol::Generic(amount) => format!("generic:{amount}"),
                    ManaSymbol::Colored(color) => {
                        format!("colored:{}", color.contract_label())
                    }
                    ManaSymbol::Phyrexian(color) => {
                        format!("phyrexian:{}", color.contract_label())
                    }
                    ManaSymbol::VariableX => "variable:x".to_owned(),
                })
                .collect::<Vec<_>>()
                .join("+")
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn filter_contract(filter: &AttachmentFilter) -> String {
    match filter {
        AttachmentFilter::Object(predicate) => {
            format!("object:{}", predicate_contract(predicate))
        }
        AttachmentFilter::Player(relative) => {
            format!("player:{}", relative.contract_label())
        }
        AttachmentFilter::Zone { zone, owner } => format!(
            "zone:{}:{}",
            zone.map(Zone::contract_label).unwrap_or("any"),
            match owner {
                None => "any".to_owned(),
                Some(ZoneOwner::Shared) => "shared".to_owned(),
                Some(ZoneOwner::Player(relative)) =>
                    format!("player:{}", relative.contract_label()),
            }
        ),
    }
}

fn predicate_contract(predicate: &ObjectPredicate) -> String {
    match predicate {
        ObjectPredicate::Permanent => "permanent".to_owned(),
        ObjectPredicate::CardType(card_type) => {
            format!("type:{}", card_type.contract_label())
        }
        ObjectPredicate::Subtype(subtype) => format!("subtype:{}", subtype.to_ascii_lowercase()),
        ObjectPredicate::Supertype(supertype) => {
            format!("supertype:{}", supertype.to_ascii_lowercase())
        }
        ObjectPredicate::Color(color) => format!("color:{}", color.contract_label()),
        ObjectPredicate::Token => "token".to_owned(),
        ObjectPredicate::Commander => "commander".to_owned(),
        ObjectPredicate::Tapped => "tapped".to_owned(),
        ObjectPredicate::HasFlying => "has-flying".to_owned(),
        ObjectPredicate::Modified => "modified".to_owned(),
        ObjectPredicate::HasAnotherAuraAttached => "has-another-aura-attached".to_owned(),
        ObjectPredicate::PowerAtMost(power) => format!("power-at-most:{power}"),
        ObjectPredicate::ManaValueAtMost(value) => format!("mana-value-at-most:{value}"),
        ObjectPredicate::Controller(relative) => {
            format!("controller:{}", relative.contract_label())
        }
        ObjectPredicate::Owner(relative) => format!("owner:{}", relative.contract_label()),
        ObjectPredicate::ZonePlayer(relative) => {
            format!("zone-player:{}", relative.contract_label())
        }
        ObjectPredicate::Zone(zone) => format!("zone:{}", zone.contract_label()),
        ObjectPredicate::Not(inner) => format!("not({})", predicate_contract(inner)),
        ObjectPredicate::All(predicates) => format!(
            "all({})",
            predicates
                .iter()
                .map(predicate_contract)
                .collect::<Vec<_>>()
                .join(",")
        ),
        ObjectPredicate::Any(predicates) => format!(
            "any({})",
            predicates
                .iter()
                .map(predicate_contract)
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn semantic_digest(
    exact: &str,
    normalized: &str,
    source_context: AttachmentSourceContext,
    layout_context: AttachmentLayoutContext,
    typed_contract: &str,
) -> String {
    let source = match source_context {
        AttachmentSourceContext::AuraEnchantment => "source:aura-enchantment",
        AttachmentSourceContext::ArtifactEquipment => "source:artifact-equipment",
        AttachmentSourceContext::BestowEnchantmentCreature => "source:bestow-enchantment-creature",
    };
    let layout = match layout_context {
        AttachmentLayoutContext::PermanentFace => "layout:permanent-face",
    };
    let mut hasher = Sha256::new();
    for component in [
        "attachment-filter-content/v1",
        ATTACHMENT_FILTER_COMPILER_VERSION,
        ATTACHMENT_FILTER_RUNTIME_VERSION,
        ATTACHMENT_FILTER_RULES_CONTEXT_VERSION,
        exact,
        normalized,
        source,
        layout,
        typed_contract,
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation: IncarnationId,
}

impl ObjectRef {
    pub const fn new(object_id: ObjectId, incarnation: IncarnationId) -> Self {
        Self {
            object_id,
            incarnation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZoneRef {
    pub zone: Zone,
    /// Shared zones use `None`. A player's hand, library, and graveyard use
    /// `Some(player)`.
    pub owner: Option<PlayerId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttachmentEntityRef {
    Object(ObjectRef),
    Player(PlayerId),
    Zone(ZoneRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalityStatus {
    Allowed,
    Forbidden,
    Unproven,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectCharacteristics {
    pub card_types: BTreeSet<CardType>,
    pub supertypes: BTreeSet<String>,
    pub subtypes: BTreeSet<String>,
    pub colors: BTreeSet<ManaColor>,
    pub mana_value: u32,
    pub power: i32,
    pub has_flying: bool,
    pub is_token: bool,
    pub is_commander: bool,
}

impl ObjectCharacteristics {
    fn is_permanent(&self) -> bool {
        self.card_types.iter().any(|card_type| {
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

    fn is_aura_enchantment(&self) -> bool {
        self.card_types.contains(&CardType::Enchantment)
            && contains_ascii_case_insensitive(&self.subtypes, "Aura")
    }

    fn is_artifact_equipment(&self) -> bool {
        self.card_types.contains(&CardType::Artifact)
            && contains_ascii_case_insensitive(&self.subtypes, "Equipment")
    }

    fn is_enchantment_creature(&self) -> bool {
        self.card_types.contains(&CardType::Enchantment)
            && self.card_types.contains(&CardType::Creature)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectState {
    pub identity: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    /// The player whose private zone contains this object. Shared zones and
    /// the battlefield use `None`.
    pub zone_player: Option<PlayerId>,
    /// Copy effects change these copiable values. Bestow's type change is
    /// applied on top and never overwrites them.
    pub copiable_characteristics: ObjectCharacteristics,
    pub characteristics: ObjectCharacteristics,
    pub tapped: bool,
    pub counter_total: u32,
    pub targeting_status: LegalityStatus,
    pub attachment_status: LegalityStatus,
    /// Exact source incarnations permitted to remain attached despite a
    /// general attachment prohibition.
    pub attachment_exceptions: BTreeSet<ObjectRef>,
    pub bestow_overlay_active: bool,
}

impl ObjectState {
    pub fn new(
        identity: ObjectRef,
        owner: PlayerId,
        controller: PlayerId,
        zone: Zone,
        characteristics: ObjectCharacteristics,
    ) -> Self {
        Self {
            identity,
            owner,
            controller,
            zone,
            zone_player: private_zone_player(zone, owner),
            copiable_characteristics: characteristics.clone(),
            characteristics,
            tapped: false,
            counter_total: 0,
            targeting_status: LegalityStatus::Allowed,
            attachment_status: LegalityStatus::Allowed,
            attachment_exceptions: BTreeSet::new(),
            bestow_overlay_active: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityLegality {
    pub targeting_status: LegalityStatus,
    pub attachment_status: LegalityStatus,
}

impl EntityLegality {
    pub const fn allowed() -> Self {
        Self {
            targeting_status: LegalityStatus::Allowed,
            attachment_status: LegalityStatus::Allowed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaUnit {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

impl ManaUnit {
    const fn for_color(color: ManaColor) -> Self {
        match color {
            ManaColor::White => Self::White,
            ManaColor::Blue => Self::Blue,
            ManaColor::Black => Self::Black,
            ManaColor::Red => Self::Red,
            ManaColor::Green => Self::Green,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerResources {
    pub mana: BTreeMap<ManaUnit, u16>,
    pub life: i32,
}

impl PlayerResources {
    pub fn new(life: i32) -> Self {
        Self {
            mana: BTreeMap::new(),
            life,
        }
    }

    pub fn add_mana(&mut self, unit: ManaUnit, amount: u16) {
        *self.mana.entry(unit).or_default() = self
            .mana
            .get(&unit)
            .copied()
            .unwrap_or(0)
            .saturating_add(amount);
    }

    pub fn mana_count(&self) -> u32 {
        self.mana.values().map(|amount| u32::from(*amount)).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostPaymentPlan {
    pub alternative_index: usize,
    pub x_value: u16,
    /// Indexes refer to symbol positions in the selected alternative.
    pub phyrexian_paid_with_life: BTreeSet<usize>,
}

impl CostPaymentPlan {
    pub fn ordinary() -> Self {
        Self {
            alternative_index: 0,
            x_value: 0,
            phyrexian_paid_with_life: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnPhase {
    PrecombatMain,
    PostcombatMain,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquipActivationContext {
    pub actor: PlayerId,
    pub active_player: PlayerId,
    pub priority_player: Option<PlayerId>,
    pub phase: TurnPhase,
    pub stack_depth: usize,
}

impl EquipActivationContext {
    pub const fn active_main(actor: PlayerId) -> Self {
        Self {
            actor,
            active_player: actor,
            priority_player: Some(actor),
            phase: TurnPhase::PrecombatMain,
            stack_depth: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellCastPermission {
    pub actor: PlayerId,
    pub priority_player: Option<PlayerId>,
    /// Bestow does not change when the underlying card may be cast. This flag
    /// is an authoritative permission supplied by the spell timing engine.
    pub underlying_spell_timing_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalSpellCostReceipt {
    payment_id: ExternalPaymentId,
    source: ObjectRef,
    payer: PlayerId,
}

impl ExternalSpellCostReceipt {
    pub const fn payment_id(self) -> ExternalPaymentId {
        self.payment_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Aura,
    Equipment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentRecord {
    pub source: ObjectRef,
    pub target: Option<AttachmentEntityRef>,
    pub kind: AttachmentKind,
    pub filter: AttachmentFilter,
    pub bestow: bool,
    pub program_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingSpellKind {
    Aura { filter: AttachmentFilter },
    Bestow { filter: AttachmentFilter },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingAttachmentSpell {
    id: SpellId,
    source: ObjectRef,
    owner: PlayerId,
    controller: PlayerId,
    target: AttachmentEntityRef,
    program_digest: String,
    kind: PendingSpellKind,
    is_copy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingEquipActivation {
    id: ActivationId,
    source: ObjectRef,
    controller: PlayerId,
    target: ObjectRef,
    target_filter: ObjectPredicate,
    legal_attachment_filter: ObjectPredicate,
    program_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuraResolutionOutcome {
    Attached {
        spell_source: ObjectRef,
        permanent: ObjectRef,
        target: AttachmentEntityRef,
    },
    CounteredByIllegalTarget {
        spell_source: ObjectRef,
        graveyard_object: ObjectRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BestowResolutionOutcome {
    AttachedAura {
        spell_source: ObjectRef,
        permanent: ObjectRef,
        target: AttachmentEntityRef,
    },
    ResolvedAsCreature {
        spell_source: ObjectRef,
        permanent: ObjectRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EquipResolutionOutcome {
    Attached {
        source: ObjectRef,
        target: ObjectRef,
    },
    FailedTargetRevalidation,
    SourceCannotAttach,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StateBasedAttachmentResult {
    pub auras_moved_to_graveyard: Vec<(ObjectRef, ObjectRef)>,
    pub bestow_effects_ended: Vec<ObjectRef>,
    pub equipment_detached: Vec<ObjectRef>,
    pub non_aura_objects_detached: Vec<ObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentRuntimeError {
    ProgramKindMismatch,
    ProductionAdapterStillDisconnected,
    DuplicateObject(ObjectRef),
    StaleOrMissingObject(ObjectRef),
    NonincreasingIncarnation {
        current: ObjectRef,
        attempted: ObjectRef,
    },
    IncarnationExhausted(ObjectRef),
    WrongSourceCharacteristics,
    SourceNotInCastableZone,
    SourceNotOnBattlefield,
    SourceControllerMismatch,
    MissingEntityLegality(AttachmentEntityRef),
    IllegalOrUnprovenTarget,
    InvalidSpellTiming,
    InvalidEquipTiming,
    MissingResources(PlayerId),
    InvalidCostAlternative,
    InvalidPhyrexianChoice,
    InsufficientMana,
    InsufficientLife,
    ExternalPaymentAlreadyConsumed(ExternalPaymentId),
    ExternalPaymentMismatch,
    UnknownSpell(SpellId),
    UnknownActivation(ActivationId),
    IdentifierExhausted,
    CopySourceIsNotPendingSpell,
}

impl fmt::Display for AttachmentRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AttachmentRuntimeError {}

#[derive(Debug, Clone)]
pub struct AttachmentFilterRuntime {
    objects: BTreeMap<ObjectRef, ObjectState>,
    latest_incarnation: BTreeMap<ObjectId, IncarnationId>,
    players: BTreeMap<PlayerId, EntityLegality>,
    zones: BTreeMap<ZoneRef, EntityLegality>,
    resources: BTreeMap<PlayerId, PlayerResources>,
    external_payments: BTreeMap<ExternalPaymentId, (ObjectRef, PlayerId, bool)>,
    pending_spells: BTreeMap<SpellId, PendingAttachmentSpell>,
    pending_equip: BTreeMap<ActivationId, PendingEquipActivation>,
    attachments: BTreeMap<ObjectRef, AttachmentRecord>,
    next_external_payment_id: ExternalPaymentId,
    next_spell_id: SpellId,
    next_activation_id: ActivationId,
}

impl Default for AttachmentFilterRuntime {
    fn default() -> Self {
        Self {
            objects: BTreeMap::new(),
            latest_incarnation: BTreeMap::new(),
            players: BTreeMap::new(),
            zones: BTreeMap::new(),
            resources: BTreeMap::new(),
            external_payments: BTreeMap::new(),
            pending_spells: BTreeMap::new(),
            pending_equip: BTreeMap::new(),
            attachments: BTreeMap::new(),
            next_external_payment_id: 1,
            next_spell_id: 1,
            next_activation_id: 1,
        }
    }
}

impl AttachmentFilterRuntime {
    pub fn insert_object(&mut self, object: ObjectState) -> Result<(), AttachmentRuntimeError> {
        if self.objects.contains_key(&object.identity) {
            return Err(AttachmentRuntimeError::DuplicateObject(object.identity));
        }
        if self
            .latest_incarnation
            .get(&object.identity.object_id)
            .is_some_and(|current| *current >= object.identity.incarnation)
        {
            return Err(AttachmentRuntimeError::NonincreasingIncarnation {
                current: ObjectRef::new(
                    object.identity.object_id,
                    self.latest_incarnation[&object.identity.object_id],
                ),
                attempted: object.identity,
            });
        }
        self.latest_incarnation
            .insert(object.identity.object_id, object.identity.incarnation);
        self.objects.insert(object.identity, object);
        Ok(())
    }

    pub fn register_player(&mut self, player: PlayerId, legality: EntityLegality) {
        self.players.insert(player, legality);
    }

    pub fn register_zone(&mut self, zone: ZoneRef, legality: EntityLegality) {
        self.zones.insert(zone, legality);
    }

    pub fn set_resources(&mut self, player: PlayerId, resources: PlayerResources) {
        self.resources.insert(player, resources);
    }

    pub fn resources(&self, player: PlayerId) -> Option<&PlayerResources> {
        self.resources.get(&player)
    }

    pub fn object(&self, identity: ObjectRef) -> Option<&ObjectState> {
        self.objects.get(&identity)
    }

    pub fn object_mut(&mut self, identity: ObjectRef) -> Option<&mut ObjectState> {
        self.objects.get_mut(&identity)
    }

    pub fn attachment(&self, source: ObjectRef) -> Option<&AttachmentRecord> {
        self.attachments.get(&source)
    }

    pub fn record_external_spell_cost_payment(
        &mut self,
        source: ObjectRef,
        payer: PlayerId,
    ) -> Result<ExternalSpellCostReceipt, AttachmentRuntimeError> {
        if !self.objects.contains_key(&source) {
            return Err(AttachmentRuntimeError::StaleOrMissingObject(source));
        }
        let payment_id = self.next_external_payment_id;
        self.next_external_payment_id = payment_id
            .checked_add(1)
            .ok_or(AttachmentRuntimeError::IdentifierExhausted)?;
        self.external_payments
            .insert(payment_id, (source, payer, false));
        Ok(ExternalSpellCostReceipt {
            payment_id,
            source,
            payer,
        })
    }

    pub fn apply_copy_effect(
        &mut self,
        object: ObjectRef,
        copied_characteristics: ObjectCharacteristics,
    ) -> Result<(), AttachmentRuntimeError> {
        let state = self
            .objects
            .get_mut(&object)
            .ok_or(AttachmentRuntimeError::StaleOrMissingObject(object))?;
        state.copiable_characteristics = copied_characteristics.clone();
        state.characteristics = if state.bestow_overlay_active {
            bestow_characteristics(&copied_characteristics)
        } else {
            copied_characteristics
        };
        Ok(())
    }

    pub fn move_object_to_zone(
        &mut self,
        object: ObjectRef,
        zone: Zone,
        controller: PlayerId,
    ) -> Result<ObjectRef, AttachmentRuntimeError> {
        self.move_object(object, zone, controller, false)
    }

    fn move_object(
        &mut self,
        object: ObjectRef,
        zone: Zone,
        controller: PlayerId,
        preserve_bestow_overlay: bool,
    ) -> Result<ObjectRef, AttachmentRuntimeError> {
        let mut state = self
            .objects
            .remove(&object)
            .ok_or(AttachmentRuntimeError::StaleOrMissingObject(object))?;
        let incarnation = object
            .incarnation
            .checked_add(1)
            .ok_or(AttachmentRuntimeError::IncarnationExhausted(object))?;
        let next = ObjectRef::new(object.object_id, incarnation);
        if self
            .latest_incarnation
            .get(&object.object_id)
            .is_some_and(|current| *current >= incarnation)
        {
            return Err(AttachmentRuntimeError::NonincreasingIncarnation {
                current: ObjectRef::new(
                    object.object_id,
                    self.latest_incarnation[&object.object_id],
                ),
                attempted: next,
            });
        }
        self.latest_incarnation
            .insert(object.object_id, incarnation);
        state.identity = next;
        state.zone = zone;
        state.zone_player = private_zone_player(zone, state.owner);
        state.controller = controller;
        state.bestow_overlay_active &= preserve_bestow_overlay;
        state.characteristics = if state.bestow_overlay_active {
            bestow_characteristics(&state.copiable_characteristics)
        } else {
            state.copiable_characteristics.clone()
        };
        self.objects.insert(next, state);
        self.attachments.remove(&object);
        Ok(next)
    }

    pub fn cast_aura(
        &mut self,
        program: &AttachmentFilterProgram,
        source: ObjectRef,
        target: AttachmentEntityRef,
        permission: SpellCastPermission,
        payment: ExternalSpellCostReceipt,
    ) -> Result<(SpellId, ObjectRef), AttachmentRuntimeError> {
        let AttachmentFilterProgramKind::Enchant { filter } = program.kind() else {
            return Err(AttachmentRuntimeError::ProgramKindMismatch);
        };
        if permission.priority_player != Some(permission.actor)
            || !permission.underlying_spell_timing_allowed
        {
            return Err(AttachmentRuntimeError::InvalidSpellTiming);
        }
        let source_state = self
            .objects
            .get(&source)
            .ok_or(AttachmentRuntimeError::StaleOrMissingObject(source))?;
        if !source_state.copiable_characteristics.is_aura_enchantment() {
            return Err(AttachmentRuntimeError::WrongSourceCharacteristics);
        }
        if matches!(source_state.zone, Zone::Battlefield | Zone::Stack) {
            return Err(AttachmentRuntimeError::SourceNotInCastableZone);
        }
        self.validate_target(filter, target, permission.actor, true)?;
        let owner = source_state.owner;
        self.consume_external_payment(payment, source, permission.actor)?;
        let stack_source = self.move_object(source, Zone::Stack, permission.actor, false)?;
        let spell_id = self.next_spell_id;
        self.next_spell_id = spell_id
            .checked_add(1)
            .ok_or(AttachmentRuntimeError::IdentifierExhausted)?;
        self.pending_spells.insert(
            spell_id,
            PendingAttachmentSpell {
                id: spell_id,
                source: stack_source,
                owner,
                controller: permission.actor,
                target,
                program_digest: program.identity.semantic_digest.clone(),
                kind: PendingSpellKind::Aura {
                    filter: filter.clone(),
                },
                is_copy: false,
            },
        );
        Ok((spell_id, stack_source))
    }

    pub fn cast_bestow(
        &mut self,
        program: &AttachmentFilterProgram,
        source: ObjectRef,
        target: AttachmentEntityRef,
        permission: SpellCastPermission,
        payment_plan: &CostPaymentPlan,
    ) -> Result<(SpellId, ObjectRef), AttachmentRuntimeError> {
        let AttachmentFilterProgramKind::Bestow {
            alternate_cost,
            enchant_filter,
        } = program.kind()
        else {
            return Err(AttachmentRuntimeError::ProgramKindMismatch);
        };
        if permission.priority_player != Some(permission.actor)
            || !permission.underlying_spell_timing_allowed
        {
            return Err(AttachmentRuntimeError::InvalidSpellTiming);
        }
        let source_state = self
            .objects
            .get(&source)
            .ok_or(AttachmentRuntimeError::StaleOrMissingObject(source))?;
        if !source_state
            .copiable_characteristics
            .is_enchantment_creature()
        {
            return Err(AttachmentRuntimeError::WrongSourceCharacteristics);
        }
        if matches!(source_state.zone, Zone::Battlefield | Zone::Stack) {
            return Err(AttachmentRuntimeError::SourceNotInCastableZone);
        }
        self.validate_target(enchant_filter, target, permission.actor, true)?;
        let owner = source_state.owner;
        self.pay_cost(permission.actor, alternate_cost, payment_plan)?;
        let stack_source = self.move_object(source, Zone::Stack, permission.actor, true)?;
        let stack_state = self
            .objects
            .get_mut(&stack_source)
            .ok_or(AttachmentRuntimeError::StaleOrMissingObject(stack_source))?;
        stack_state.bestow_overlay_active = true;
        stack_state.characteristics = bestow_characteristics(&stack_state.copiable_characteristics);
        let spell_id = self.next_spell_id;
        self.next_spell_id = spell_id
            .checked_add(1)
            .ok_or(AttachmentRuntimeError::IdentifierExhausted)?;
        self.pending_spells.insert(
            spell_id,
            PendingAttachmentSpell {
                id: spell_id,
                source: stack_source,
                owner,
                controller: permission.actor,
                target,
                program_digest: program.identity.semantic_digest.clone(),
                kind: PendingSpellKind::Bestow {
                    filter: enchant_filter.clone(),
                },
                is_copy: false,
            },
        );
        Ok((spell_id, stack_source))
    }

    /// Copies the current spell characteristics and its declared target. The
    /// caller supplies a fresh object identity for the spell copy. A copied
    /// permanent spell becomes a token permanent if it resolves.
    pub fn copy_pending_spell(
        &mut self,
        original_spell: SpellId,
        copy_identity: ObjectRef,
        controller: PlayerId,
    ) -> Result<SpellId, AttachmentRuntimeError> {
        let pending = self
            .pending_spells
            .get(&original_spell)
            .cloned()
            .ok_or(AttachmentRuntimeError::CopySourceIsNotPendingSpell)?;
        let original = self
            .objects
            .get(&pending.source)
            .cloned()
            .ok_or(AttachmentRuntimeError::StaleOrMissingObject(pending.source))?;
        let mut copy_characteristics = original.copiable_characteristics.clone();
        copy_characteristics.is_token = true;
        let mut copy = ObjectState::new(
            copy_identity,
            controller,
            controller,
            Zone::Stack,
            copy_characteristics,
        );
        copy.bestow_overlay_active = matches!(&pending.kind, PendingSpellKind::Bestow { .. });
        if copy.bestow_overlay_active {
            copy.characteristics = bestow_characteristics(&copy.copiable_characteristics);
        }
        self.insert_object(copy)?;
        let spell_id = self.next_spell_id;
        self.next_spell_id = spell_id
            .checked_add(1)
            .ok_or(AttachmentRuntimeError::IdentifierExhausted)?;
        self.pending_spells.insert(
            spell_id,
            PendingAttachmentSpell {
                id: spell_id,
                source: copy_identity,
                owner: controller,
                controller,
                target: pending.target,
                program_digest: pending.program_digest,
                kind: pending.kind,
                is_copy: true,
            },
        );
        Ok(spell_id)
    }

    pub fn resolve_aura(
        &mut self,
        spell_id: SpellId,
    ) -> Result<AuraResolutionOutcome, AttachmentRuntimeError> {
        let pending = self
            .pending_spells
            .remove(&spell_id)
            .ok_or(AttachmentRuntimeError::UnknownSpell(spell_id))?;
        self.require_pending_stack_source(&pending)?;
        let PendingSpellKind::Aura { filter } = pending.kind.clone() else {
            self.pending_spells.insert(spell_id, pending);
            return Err(AttachmentRuntimeError::ProgramKindMismatch);
        };
        let legal = self
            .validate_target(&filter, pending.target, pending.controller, true)
            .is_ok();
        if !legal {
            let graveyard =
                self.move_object(pending.source, Zone::Graveyard, pending.owner, false)?;
            return Ok(AuraResolutionOutcome::CounteredByIllegalTarget {
                spell_source: pending.source,
                graveyard_object: graveyard,
            });
        }
        let permanent =
            self.move_object(pending.source, Zone::Battlefield, pending.controller, false)?;
        self.attachments.insert(
            permanent,
            AttachmentRecord {
                source: permanent,
                target: Some(pending.target),
                kind: AttachmentKind::Aura,
                filter,
                bestow: false,
                program_digest: pending.program_digest,
            },
        );
        Ok(AuraResolutionOutcome::Attached {
            spell_source: pending.source,
            permanent,
            target: pending.target,
        })
    }

    pub fn resolve_bestow(
        &mut self,
        spell_id: SpellId,
    ) -> Result<BestowResolutionOutcome, AttachmentRuntimeError> {
        let pending = self
            .pending_spells
            .remove(&spell_id)
            .ok_or(AttachmentRuntimeError::UnknownSpell(spell_id))?;
        self.require_pending_stack_source(&pending)?;
        let PendingSpellKind::Bestow { filter } = pending.kind.clone() else {
            self.pending_spells.insert(spell_id, pending);
            return Err(AttachmentRuntimeError::ProgramKindMismatch);
        };
        let legal = self
            .validate_target(&filter, pending.target, pending.controller, true)
            .is_ok();
        if !legal {
            let permanent =
                self.move_object(pending.source, Zone::Battlefield, pending.controller, false)?;
            if let Some(state) = self.objects.get_mut(&permanent) {
                state.bestow_overlay_active = false;
                state.characteristics = state.copiable_characteristics.clone();
            }
            return Ok(BestowResolutionOutcome::ResolvedAsCreature {
                spell_source: pending.source,
                permanent,
            });
        }
        let permanent =
            self.move_object(pending.source, Zone::Battlefield, pending.controller, true)?;
        if let Some(state) = self.objects.get_mut(&permanent) {
            state.bestow_overlay_active = true;
            state.characteristics = bestow_characteristics(&state.copiable_characteristics);
        }
        self.attachments.insert(
            permanent,
            AttachmentRecord {
                source: permanent,
                target: Some(pending.target),
                kind: AttachmentKind::Aura,
                filter,
                bestow: true,
                program_digest: pending.program_digest,
            },
        );
        Ok(BestowResolutionOutcome::AttachedAura {
            spell_source: pending.source,
            permanent,
            target: pending.target,
        })
    }

    pub fn activate_equip(
        &mut self,
        program: &AttachmentFilterProgram,
        source: ObjectRef,
        target: ObjectRef,
        context: EquipActivationContext,
        payment_plan: &CostPaymentPlan,
    ) -> Result<ActivationId, AttachmentRuntimeError> {
        let AttachmentFilterProgramKind::Equip {
            activation_cost,
            target_filter,
            legal_attachment_filter,
        } = program.kind()
        else {
            return Err(AttachmentRuntimeError::ProgramKindMismatch);
        };
        if context.actor != context.active_player
            || context.priority_player != Some(context.actor)
            || !matches!(
                context.phase,
                TurnPhase::PrecombatMain | TurnPhase::PostcombatMain
            )
            || context.stack_depth != 0
        {
            return Err(AttachmentRuntimeError::InvalidEquipTiming);
        }
        let source_state = self
            .objects
            .get(&source)
            .ok_or(AttachmentRuntimeError::StaleOrMissingObject(source))?;
        if source_state.zone != Zone::Battlefield {
            return Err(AttachmentRuntimeError::SourceNotOnBattlefield);
        }
        if source_state.controller != context.actor {
            return Err(AttachmentRuntimeError::SourceControllerMismatch);
        }
        if !source_state.characteristics.is_artifact_equipment() {
            return Err(AttachmentRuntimeError::WrongSourceCharacteristics);
        }
        let target_state = self
            .objects
            .get(&target)
            .ok_or(AttachmentRuntimeError::StaleOrMissingObject(target))?;
        if target_state.targeting_status != LegalityStatus::Allowed
            || target_state.attachment_status != LegalityStatus::Allowed
            || !self.object_matches(target_filter, target_state, context.actor)
        {
            return Err(AttachmentRuntimeError::IllegalOrUnprovenTarget);
        }
        self.pay_cost(context.actor, activation_cost, payment_plan)?;
        let activation_id = self.next_activation_id;
        self.next_activation_id = activation_id
            .checked_add(1)
            .ok_or(AttachmentRuntimeError::IdentifierExhausted)?;
        self.pending_equip.insert(
            activation_id,
            PendingEquipActivation {
                id: activation_id,
                source,
                controller: context.actor,
                target,
                target_filter: target_filter.clone(),
                legal_attachment_filter: legal_attachment_filter.clone(),
                program_digest: program.identity.semantic_digest.clone(),
            },
        );
        Ok(activation_id)
    }

    pub fn resolve_equip(
        &mut self,
        activation_id: ActivationId,
    ) -> Result<EquipResolutionOutcome, AttachmentRuntimeError> {
        let pending = self
            .pending_equip
            .remove(&activation_id)
            .ok_or(AttachmentRuntimeError::UnknownActivation(activation_id))?;
        let Some(source) = self.objects.get(&pending.source) else {
            return Ok(EquipResolutionOutcome::SourceCannotAttach);
        };
        if source.zone != Zone::Battlefield
            || !source.characteristics.is_artifact_equipment()
            || source
                .characteristics
                .card_types
                .contains(&CardType::Creature)
        {
            return Ok(EquipResolutionOutcome::SourceCannotAttach);
        }
        let Some(target) = self.objects.get(&pending.target) else {
            return Ok(EquipResolutionOutcome::FailedTargetRevalidation);
        };
        if target.targeting_status != LegalityStatus::Allowed
            || target.attachment_status != LegalityStatus::Allowed
            || !self.object_matches(&pending.target_filter, target, pending.controller)
        {
            return Ok(EquipResolutionOutcome::FailedTargetRevalidation);
        }
        self.attachments.insert(
            pending.source,
            AttachmentRecord {
                source: pending.source,
                target: Some(pending.target.into()),
                kind: AttachmentKind::Equipment,
                filter: AttachmentFilter::Object(pending.legal_attachment_filter),
                bestow: false,
                program_digest: pending.program_digest,
            },
        );
        Ok(EquipResolutionOutcome::Attached {
            source: pending.source,
            target: pending.target,
        })
    }

    pub fn detach(&mut self, source: ObjectRef) {
        if let Some(attachment) = self.attachments.get_mut(&source) {
            attachment.target = None;
        }
    }

    pub fn perform_attachment_state_based_actions(
        &mut self,
    ) -> Result<StateBasedAttachmentResult, AttachmentRuntimeError> {
        let sources = self.attachments.keys().copied().collect::<Vec<_>>();
        let mut result = StateBasedAttachmentResult::default();
        for source in sources {
            let Some(record) = self.attachments.get(&source).cloned() else {
                continue;
            };
            let Some(source_state) = self.objects.get(&source).cloned() else {
                self.attachments.remove(&source);
                continue;
            };
            if source_state.zone != Zone::Battlefield {
                self.attachments.remove(&source);
                continue;
            }
            match record.kind {
                AttachmentKind::Aura => {
                    if record.bestow {
                        let legal = source_state.bestow_overlay_active
                            && source_state.characteristics.is_aura_enchantment()
                            && record.target.is_some_and(|target| {
                                self.validate_target_excluding_source(
                                    &record.filter,
                                    target,
                                    source_state.controller,
                                    false,
                                    Some(source),
                                )
                                .is_ok()
                            });
                        if !legal {
                            self.attachments.remove(&source);
                            if let Some(state) = self.objects.get_mut(&source) {
                                state.bestow_overlay_active = false;
                                state.characteristics = state.copiable_characteristics.clone();
                            }
                            result.bestow_effects_ended.push(source);
                        }
                    } else if !source_state.characteristics.is_aura_enchantment() {
                        self.attachments.remove(&source);
                        result.non_aura_objects_detached.push(source);
                    } else {
                        let legal = record.target.is_some_and(|target| {
                            self.validate_target_excluding_source(
                                &record.filter,
                                target,
                                source_state.controller,
                                false,
                                Some(source),
                            )
                            .is_ok()
                        });
                        if !legal {
                            self.attachments.remove(&source);
                            let graveyard = self.move_object(
                                source,
                                Zone::Graveyard,
                                source_state.owner,
                                false,
                            )?;
                            result.auras_moved_to_graveyard.push((source, graveyard));
                        }
                    }
                }
                AttachmentKind::Equipment => {
                    if record.target.is_none() {
                        continue;
                    }
                    let can_be_equipment = source_state.characteristics.is_artifact_equipment()
                        && !source_state
                            .characteristics
                            .card_types
                            .contains(&CardType::Creature);
                    let legal = can_be_equipment
                        && record.target.is_some_and(|target| {
                            self.validate_target_excluding_source(
                                &record.filter,
                                target,
                                source_state.controller,
                                false,
                                Some(source),
                            )
                            .is_ok()
                        });
                    if !legal {
                        if let Some(attachment) = self.attachments.get_mut(&source) {
                            attachment.target = None;
                        }
                        result.equipment_detached.push(source);
                    }
                }
            }
        }
        Ok(result)
    }

    fn require_pending_stack_source(
        &self,
        pending: &PendingAttachmentSpell,
    ) -> Result<(), AttachmentRuntimeError> {
        let source = self
            .objects
            .get(&pending.source)
            .ok_or(AttachmentRuntimeError::StaleOrMissingObject(pending.source))?;
        if source.zone != Zone::Stack || source.controller != pending.controller {
            return Err(AttachmentRuntimeError::StaleOrMissingObject(pending.source));
        }
        Ok(())
    }

    fn consume_external_payment(
        &mut self,
        receipt: ExternalSpellCostReceipt,
        source: ObjectRef,
        payer: PlayerId,
    ) -> Result<(), AttachmentRuntimeError> {
        if receipt.source != source || receipt.payer != payer {
            return Err(AttachmentRuntimeError::ExternalPaymentMismatch);
        }
        let Some((recorded_source, recorded_payer, consumed)) =
            self.external_payments.get_mut(&receipt.payment_id)
        else {
            return Err(AttachmentRuntimeError::ExternalPaymentMismatch);
        };
        if *recorded_source != source || *recorded_payer != payer {
            return Err(AttachmentRuntimeError::ExternalPaymentMismatch);
        }
        if *consumed {
            return Err(AttachmentRuntimeError::ExternalPaymentAlreadyConsumed(
                receipt.payment_id,
            ));
        }
        *consumed = true;
        Ok(())
    }

    fn pay_cost(
        &mut self,
        player: PlayerId,
        cost: &ManaCost,
        plan: &CostPaymentPlan,
    ) -> Result<(), AttachmentRuntimeError> {
        let alternative = cost
            .alternatives
            .get(plan.alternative_index)
            .ok_or(AttachmentRuntimeError::InvalidCostAlternative)?;
        if plan.phyrexian_paid_with_life.iter().any(|index| {
            !matches!(
                alternative.symbols.get(*index),
                Some(ManaSymbol::Phyrexian(_))
            )
        }) {
            return Err(AttachmentRuntimeError::InvalidPhyrexianChoice);
        }
        let resources = self
            .resources
            .get(&player)
            .cloned()
            .ok_or(AttachmentRuntimeError::MissingResources(player))?;
        let mut staged = resources;
        let mut generic = 0u32;
        let mut life = 0i32;
        for (index, symbol) in alternative.symbols.iter().enumerate() {
            match symbol {
                ManaSymbol::Generic(amount) => {
                    generic = generic.saturating_add(u32::from(*amount));
                }
                ManaSymbol::VariableX => {
                    generic = generic.saturating_add(u32::from(plan.x_value));
                }
                ManaSymbol::Colored(color) => {
                    spend_mana_unit(&mut staged, ManaUnit::for_color(*color), 1)?;
                }
                ManaSymbol::Phyrexian(_color) if plan.phyrexian_paid_with_life.contains(&index) => {
                    life = life.saturating_add(2);
                }
                ManaSymbol::Phyrexian(color) => {
                    spend_mana_unit(&mut staged, ManaUnit::for_color(*color), 1)?;
                }
            }
        }
        if staged.life < life {
            return Err(AttachmentRuntimeError::InsufficientLife);
        }
        staged.life -= life;
        spend_generic_mana(&mut staged, generic)?;
        self.resources.insert(player, staged);
        Ok(())
    }

    fn validate_target(
        &self,
        filter: &AttachmentFilter,
        target: AttachmentEntityRef,
        actor: PlayerId,
        require_targeting: bool,
    ) -> Result<(), AttachmentRuntimeError> {
        self.validate_target_excluding_source(filter, target, actor, require_targeting, None)
    }

    fn validate_target_excluding_source(
        &self,
        filter: &AttachmentFilter,
        target: AttachmentEntityRef,
        actor: PlayerId,
        require_targeting: bool,
        attachment_source: Option<ObjectRef>,
    ) -> Result<(), AttachmentRuntimeError> {
        match (filter, target) {
            (AttachmentFilter::Object(predicate), AttachmentEntityRef::Object(identity)) => {
                let object = self
                    .objects
                    .get(&identity)
                    .ok_or(AttachmentRuntimeError::StaleOrMissingObject(identity))?;
                if (require_targeting && object.targeting_status != LegalityStatus::Allowed)
                    || (object.attachment_status != LegalityStatus::Allowed
                        && !attachment_source
                            .is_some_and(|source| object.attachment_exceptions.contains(&source)))
                    || !self.object_matches_excluding_source(
                        predicate,
                        object,
                        actor,
                        attachment_source,
                    )
                {
                    return Err(AttachmentRuntimeError::IllegalOrUnprovenTarget);
                }
            }
            (AttachmentFilter::Player(relative), AttachmentEntityRef::Player(player)) => {
                let legality = self
                    .players
                    .get(&player)
                    .ok_or(AttachmentRuntimeError::MissingEntityLegality(target))?;
                if !relative_player_matches(*relative, player, actor)
                    || (require_targeting && legality.targeting_status != LegalityStatus::Allowed)
                    || legality.attachment_status != LegalityStatus::Allowed
                {
                    return Err(AttachmentRuntimeError::IllegalOrUnprovenTarget);
                }
            }
            (AttachmentFilter::Zone { zone, owner }, AttachmentEntityRef::Zone(target_zone)) => {
                let legality = self
                    .zones
                    .get(&target_zone)
                    .ok_or(AttachmentRuntimeError::MissingEntityLegality(target))?;
                if zone.is_some_and(|required| required != target_zone.zone)
                    || owner
                        .as_ref()
                        .is_some_and(|owner| !zone_owner_matches(owner, target_zone.owner, actor))
                    || (require_targeting && legality.targeting_status != LegalityStatus::Allowed)
                    || legality.attachment_status != LegalityStatus::Allowed
                {
                    return Err(AttachmentRuntimeError::IllegalOrUnprovenTarget);
                }
            }
            _ => return Err(AttachmentRuntimeError::IllegalOrUnprovenTarget),
        }
        Ok(())
    }

    fn object_matches(
        &self,
        predicate: &ObjectPredicate,
        object: &ObjectState,
        actor: PlayerId,
    ) -> bool {
        self.object_matches_excluding_source(predicate, object, actor, None)
    }

    fn object_matches_excluding_source(
        &self,
        predicate: &ObjectPredicate,
        object: &ObjectState,
        actor: PlayerId,
        attachment_source: Option<ObjectRef>,
    ) -> bool {
        match predicate {
            ObjectPredicate::Permanent => object.characteristics.is_permanent(),
            ObjectPredicate::CardType(card_type) => {
                object.characteristics.card_types.contains(card_type)
            }
            ObjectPredicate::Subtype(subtype) => {
                contains_ascii_case_insensitive(&object.characteristics.subtypes, subtype)
            }
            ObjectPredicate::Supertype(supertype) => {
                contains_ascii_case_insensitive(&object.characteristics.supertypes, supertype)
            }
            ObjectPredicate::Color(color) => object.characteristics.colors.contains(color),
            ObjectPredicate::Token => object.characteristics.is_token,
            ObjectPredicate::Commander => object.characteristics.is_commander,
            ObjectPredicate::Tapped => object.tapped,
            ObjectPredicate::HasFlying => object.characteristics.has_flying,
            ObjectPredicate::Modified => {
                object.counter_total > 0
                    || self.attachments.values().any(|attachment| {
                        attachment.target == Some(AttachmentEntityRef::Object(object.identity))
                            && (attachment.kind == AttachmentKind::Equipment
                                || (attachment.kind == AttachmentKind::Aura
                                    && self.objects.get(&attachment.source).is_some_and(
                                        |source| source.controller == object.controller,
                                    )))
                    })
            }
            ObjectPredicate::HasAnotherAuraAttached => {
                self.attachments.values().any(|attachment| {
                    attachment.kind == AttachmentKind::Aura
                        && Some(attachment.source) != attachment_source
                        && attachment.target == Some(AttachmentEntityRef::Object(object.identity))
                })
            }
            ObjectPredicate::PowerAtMost(power) => object.characteristics.power <= *power,
            ObjectPredicate::ManaValueAtMost(value) => object.characteristics.mana_value <= *value,
            ObjectPredicate::Controller(relative) => {
                relative_player_matches(*relative, object.controller, actor)
            }
            ObjectPredicate::Owner(relative) => {
                relative_player_matches(*relative, object.owner, actor)
            }
            ObjectPredicate::ZonePlayer(relative) => object
                .zone_player
                .is_some_and(|player| relative_player_matches(*relative, player, actor)),
            ObjectPredicate::Zone(zone) => object.zone == *zone,
            ObjectPredicate::Not(inner) => {
                !self.object_matches_excluding_source(inner, object, actor, attachment_source)
            }
            ObjectPredicate::All(predicates) => predicates.iter().all(|predicate| {
                self.object_matches_excluding_source(predicate, object, actor, attachment_source)
            }),
            ObjectPredicate::Any(predicates) => predicates.iter().any(|predicate| {
                self.object_matches_excluding_source(predicate, object, actor, attachment_source)
            }),
        }
    }
}

impl From<ObjectRef> for AttachmentEntityRef {
    fn from(value: ObjectRef) -> Self {
        Self::Object(value)
    }
}

fn bestow_characteristics(base: &ObjectCharacteristics) -> ObjectCharacteristics {
    let mut characteristics = base.clone();
    characteristics.card_types.clear();
    characteristics.card_types.insert(CardType::Enchantment);
    characteristics.subtypes.clear();
    characteristics.subtypes.insert("Aura".to_owned());
    characteristics
}

fn contains_ascii_case_insensitive(values: &BTreeSet<String>, needle: &str) -> bool {
    values
        .iter()
        .any(|value| value.eq_ignore_ascii_case(needle))
}

fn relative_player_matches(relative: RelativePlayer, player: PlayerId, actor: PlayerId) -> bool {
    match relative {
        RelativePlayer::You => player == actor,
        RelativePlayer::Opponent => player != actor,
        RelativePlayer::Any => true,
    }
}

fn private_zone_player(zone: Zone, owner: PlayerId) -> Option<PlayerId> {
    matches!(zone, Zone::Graveyard | Zone::Hand | Zone::Library).then_some(owner)
}

fn zone_owner_matches(owner: &ZoneOwner, target: Option<PlayerId>, actor: PlayerId) -> bool {
    match owner {
        ZoneOwner::Shared => target.is_none(),
        ZoneOwner::Player(relative) => {
            target.is_some_and(|player| relative_player_matches(*relative, player, actor))
        }
    }
}

fn spend_mana_unit(
    resources: &mut PlayerResources,
    unit: ManaUnit,
    amount: u16,
) -> Result<(), AttachmentRuntimeError> {
    let available = resources.mana.get(&unit).copied().unwrap_or(0);
    if available < amount {
        return Err(AttachmentRuntimeError::InsufficientMana);
    }
    resources.mana.insert(unit, available - amount);
    Ok(())
}

fn spend_generic_mana(
    resources: &mut PlayerResources,
    mut amount: u32,
) -> Result<(), AttachmentRuntimeError> {
    if resources.mana_count() < amount {
        return Err(AttachmentRuntimeError::InsufficientMana);
    }
    for unit in [
        ManaUnit::Colorless,
        ManaUnit::White,
        ManaUnit::Blue,
        ManaUnit::Black,
        ManaUnit::Red,
        ManaUnit::Green,
    ] {
        let available = resources.mana.get(&unit).copied().unwrap_or(0);
        let spend = u16::try_from(amount.min(u32::from(available))).unwrap_or(available);
        if spend > 0 {
            resources.mana.insert(unit, available - spend);
            amount -= u32::from(spend);
        }
        if amount == 0 {
            break;
        }
    }
    Ok(())
}
