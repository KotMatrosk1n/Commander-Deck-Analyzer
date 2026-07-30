//! Content keyed cast-zone programs for Foretell, Plot, Warp, Retrace, and
//! Jump-start.
//!
//! Only complete standalone Oracle clauses are accepted. Recognition and a
//! complete transaction model do not make these programs production-live.
//! The production simulator must explicitly connect every state boundary
//! represented here before coverage may claim execution.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const EXTENDED_CAST_ZONE_COMPILER_VERSION: &str = "extended-cast-zone-keyword-compiler-0.2";
pub const EXTENDED_CAST_ZONE_RUNTIME_VERSION: &str = "extended-cast-zone-keyword-runtime-0.2";
pub const EXTENDED_CAST_ZONE_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:108.3,116,117,400.7,601.2,603.7,609.3,614.1,616.1,702.81,702.133,702.143,702.170,702.185";

const FORETELL_REMINDER: &str = "During your turn, you may pay {2} and exile this card from your hand face down. Cast it on a later turn for its foretell cost.";
const PLOT_REMINDER_SUFFIX: &str = " and exile this card from your hand. Cast it as a sorcery on a later turn without paying its mana cost. Plot only as a sorcery.";
const WARP_HAND_REMINDER_PREFIX: &str =
    "You may cast this card from your hand for its warp cost. Exile this ";
const WARP_HAND_REMINDER_SUFFIX: &str =
    " at the beginning of the next end step, then you may cast it from exile on a later turn.";
const WARP_HAND_OR_GRAVEYARD_REMINDER: &str = "You may cast this card from your hand or graveyard for its warp cost. If you do, exile this creature at the beginning of the next end step, then you may cast it from exile on a later turn.";
const RETRACE_EXACT: &str = "Retrace (You may cast this card from your graveyard by discarding a land card in addition to paying its other costs.)";
const JUMP_START_EXACT: &str = "Jump-start (You may cast this card from your graveyard by discarding a card in addition to paying its other costs. Then exile this card.)";

/// This remains false until the main simulator supplies all timing, hidden
/// information, cost, stack, object-incarnation, and replacement boundaries.
pub const fn extended_cast_zone_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncarnationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaUnitId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TurnId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingTriggerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplacementEffectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Stack,
    Command,
    OutsideGame,
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
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

impl ManaColor {
    fn stable_id(self) -> &'static str {
        match self {
            Self::White => "w",
            Self::Blue => "u",
            Self::Black => "b",
            Self::Red => "r",
            Self::Green => "g",
            Self::Colorless => "c",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaSymbol {
    Generic(u32),
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
    Snow,
    Hybrid(ManaColor, ManaColor),
    VariableX,
}

impl ManaSymbol {
    fn stable_id(self) -> String {
        match self {
            Self::Generic(value) => format!("generic/{value}"),
            Self::White => "w".into(),
            Self::Blue => "u".into(),
            Self::Black => "b".into(),
            Self::Red => "r".into(),
            Self::Green => "g".into(),
            Self::Colorless => "c".into(),
            Self::Snow => "s".into(),
            Self::Hybrid(first, second) => {
                format!("hybrid/{}/{}", first.stable_id(), second.stable_id())
            }
            Self::VariableX => "x".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaCost {
    pub exact: String,
    pub symbols: Vec<ManaSymbol>,
}

impl ManaCost {
    fn stable_id(&self) -> String {
        self.symbols
            .iter()
            .map(|symbol| symbol.stable_id())
            .collect::<Vec<_>>()
            .join(",")
    }

    fn contains_x(&self) -> bool {
        self.symbols.contains(&ManaSymbol::VariableX)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarpSourcePermission {
    Hand,
    HandOrOwnersGraveyard,
}

impl WarpSourcePermission {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Hand => "hand",
            Self::HandOrOwnersGraveyard => "hand-or-owners-graveyard",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarpPermanentKind {
    SourcePermanent,
    Creature,
    Enchantment,
}

impl WarpPermanentKind {
    fn stable_id(self) -> &'static str {
        match self {
            Self::SourcePermanent => "source-permanent",
            Self::Creature => "creature",
            Self::Enchantment => "enchantment",
        }
    }

    fn matches(self, card_types: &BTreeSet<CardType>) -> bool {
        match self {
            Self::SourcePermanent => card_types.iter().any(|card_type| {
                matches!(
                    card_type,
                    CardType::Artifact
                        | CardType::Battle
                        | CardType::Creature
                        | CardType::Enchantment
                        | CardType::Planeswalker
                )
            }),
            Self::Creature => card_types.contains(&CardType::Creature),
            Self::Enchantment => card_types.contains(&CardType::Enchantment),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForetellProgram {
    pub foretell_cost: ManaCost,
    pub special_action_cost: ManaCost,
    pub exile_face_down: bool,
    pub later_turn_required: bool,
    pub later_cast_uses_normal_timing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotProgram {
    pub plot_cost: ManaCost,
    pub special_action_uses_sorcery_timing: bool,
    pub exile_face_up: bool,
    pub later_turn_required: bool,
    pub later_cast_uses_sorcery_timing: bool,
    pub later_cast_without_paying_mana_cost: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarpProgram {
    pub warp_cost: ManaCost,
    pub life_cost: u32,
    pub source_permission: WarpSourcePermission,
    pub permanent_kind: WarpPermanentKind,
    pub delayed_exile_at_next_end_step: bool,
    pub later_turn_cast_from_exile: bool,
    pub later_cast_uses_normal_costs_and_timing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetraceProgram {
    pub cast_from_owners_graveyard: bool,
    pub discard_land_additional_cost: bool,
    pub retains_other_costs_and_timing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JumpStartProgram {
    pub cast_from_owners_graveyard: bool,
    pub discard_card_additional_cost: bool,
    pub retains_other_costs_and_timing: bool,
    pub every_stack_exit_replaced_with_exile: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtendedCastZoneKind {
    Foretell(ForetellProgram),
    Plot(PlotProgram),
    Warp(WarpProgram),
    Retrace(RetraceProgram),
    JumpStart(JumpStartProgram),
}

impl ExtendedCastZoneKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Foretell(_) => "Foretell",
            Self::Plot(_) => "Plot",
            Self::Warp(_) => "Warp",
            Self::Retrace(_) => "Retrace",
            Self::JumpStart(_) => "Jump-start",
        }
    }

    fn stable_id(&self) -> String {
        match self {
            Self::Foretell(program) => format!(
                "foretell/v1;special-action-cost={};foretell-cost={};from=hand;action=own-turn-priority;visibility=owner-only;later-turn=true;cast=exile-normal-timing-alternative-cost;other-costs=retained",
                program.special_action_cost.stable_id(),
                program.foretell_cost.stable_id()
            ),
            Self::Plot(program) => format!(
                "plot/v1;plot-cost={};from=hand;action=sorcery-timing;visibility=public;later-turn=true;cast=exile-sorcery-timing-without-mana;other-costs=retained",
                program.plot_cost.stable_id()
            ),
            Self::Warp(program) => format!(
                "warp/v1;warp-cost={};life={};from={};subject={};cast=normal-timing-alternative-cost;delayed=next-end-step-exile-that-incarnation;permission=exile-later-turn-normal-costs-and-timing;copies=not-cast",
                program.warp_cost.stable_id(),
                program.life_cost,
                program.source_permission.stable_id(),
                program.permanent_kind.stable_id()
            ),
            Self::Retrace(_) => "retrace/v1;from=owners-graveyard;additional-cost=discard-land;other-costs-and-normal-timing=retained;stack-exit=ordinary;copies=not-cast".into(),
            Self::JumpStart(_) => "jump-start/v1;from=owners-graveyard;additional-cost=discard-card;other-costs-and-normal-timing=retained;every-stack-exit=replaced-with-exile;copies=not-cast".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedCastZoneProgram {
    exact_source: String,
    kind: ExtendedCastZoneKind,
    semantic_digest: String,
}

impl ExtendedCastZoneProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn kind(&self) -> &ExtendedCastZoneKind {
        &self.kind
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub const fn production_adapter_connected(&self) -> bool {
        extended_cast_zone_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotCandidateClass {
    SupportedFamily,
    EarlierOfficialKeywordOwner,
    UnsupportedGrantedModifierCompoundOrPartial,
}

pub fn classify_extended_cast_zone_snapshot_candidate(
    exact_source: &str,
) -> Option<SnapshotCandidateClass> {
    let family = candidate_family(exact_source)?;
    if exact_source == RETRACE_EXACT {
        return Some(SnapshotCandidateClass::EarlierOfficialKeywordOwner);
    }
    if compile_extended_cast_zone_keyword_program(exact_source).is_some() {
        return Some(SnapshotCandidateClass::SupportedFamily);
    }
    let _ = family;
    Some(SnapshotCandidateClass::UnsupportedGrantedModifierCompoundOrPartial)
}

pub fn compile_extended_cast_zone_keyword_program(
    exact_source: &str,
) -> Option<ExtendedCastZoneProgram> {
    if exact_source.is_empty()
        || exact_source.trim() != exact_source
        || exact_source.contains(['\r', '\n'])
        || collapse_whitespace(exact_source) != exact_source
    {
        return None;
    }

    let kind = parse_foretell(exact_source)
        .or_else(|| parse_plot(exact_source))
        .or_else(|| parse_warp(exact_source))
        .or_else(|| parse_retrace(exact_source))
        .or_else(|| parse_jump_start(exact_source))?;
    let semantic_digest = extended_cast_zone_semantic_digest_with_versions(
        exact_source,
        &kind,
        EXTENDED_CAST_ZONE_COMPILER_VERSION,
        EXTENDED_CAST_ZONE_RUNTIME_VERSION,
        EXTENDED_CAST_ZONE_RULES_CONTEXT_VERSION,
    );
    Some(ExtendedCastZoneProgram {
        exact_source: exact_source.to_owned(),
        kind,
        semantic_digest,
    })
}

fn parse_foretell(exact_source: &str) -> Option<ExtendedCastZoneKind> {
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    if reminder.is_some_and(|reminder| reminder != FORETELL_REMINDER) {
        return None;
    }
    let cost = parse_mana_cost(core.strip_prefix("Foretell ")?)?;
    Some(ExtendedCastZoneKind::Foretell(ForetellProgram {
        foretell_cost: cost,
        special_action_cost: parse_mana_cost("{2}")?,
        exile_face_down: true,
        later_turn_required: true,
        later_cast_uses_normal_timing: true,
    }))
}

fn parse_plot(exact_source: &str) -> Option<ExtendedCastZoneKind> {
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    let cost = parse_mana_cost(core.strip_prefix("Plot ")?)?;
    let expected = format!("You may pay {}{PLOT_REMINDER_SUFFIX}", cost.exact);
    if reminder.is_some_and(|reminder| reminder != expected) {
        return None;
    }
    Some(ExtendedCastZoneKind::Plot(PlotProgram {
        plot_cost: cost,
        special_action_uses_sorcery_timing: true,
        exile_face_up: true,
        later_turn_required: true,
        later_cast_uses_sorcery_timing: true,
        later_cast_without_paying_mana_cost: true,
    }))
}

fn parse_warp(exact_source: &str) -> Option<ExtendedCastZoneKind> {
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    if let Some(cost_text) = core.strip_prefix("Warp ") {
        let warp_cost = parse_mana_cost(cost_text)?;
        let Some(reminder) = reminder else {
            return Some(ExtendedCastZoneKind::Warp(WarpProgram {
                warp_cost,
                life_cost: 0,
                source_permission: WarpSourcePermission::Hand,
                permanent_kind: WarpPermanentKind::SourcePermanent,
                delayed_exile_at_next_end_step: true,
                later_turn_cast_from_exile: true,
                later_cast_uses_normal_costs_and_timing: true,
            }));
        };
        let subject = reminder
            .strip_prefix(WARP_HAND_REMINDER_PREFIX)?
            .strip_suffix(WARP_HAND_REMINDER_SUFFIX)?;
        let permanent_kind = match subject {
            "creature" => WarpPermanentKind::Creature,
            "enchantment" => WarpPermanentKind::Enchantment,
            _ => return None,
        };
        return Some(ExtendedCastZoneKind::Warp(WarpProgram {
            warp_cost,
            life_cost: 0,
            source_permission: WarpSourcePermission::Hand,
            permanent_kind,
            delayed_exile_at_next_end_step: true,
            later_turn_cast_from_exile: true,
            later_cast_uses_normal_costs_and_timing: true,
        }));
    }

    let extended_cost = core.strip_prefix("Warp\u{2014}")?.strip_suffix('.')?;
    let (mana_text, life_text) = extended_cost.split_once(", Pay ")?;
    let life = life_text.strip_suffix(" life")?.parse::<u32>().ok()?;
    if life == 0 || reminder? != WARP_HAND_OR_GRAVEYARD_REMINDER {
        return None;
    }
    Some(ExtendedCastZoneKind::Warp(WarpProgram {
        warp_cost: parse_mana_cost(mana_text)?,
        life_cost: life,
        source_permission: WarpSourcePermission::HandOrOwnersGraveyard,
        permanent_kind: WarpPermanentKind::Creature,
        delayed_exile_at_next_end_step: true,
        later_turn_cast_from_exile: true,
        later_cast_uses_normal_costs_and_timing: true,
    }))
}

fn parse_retrace(exact_source: &str) -> Option<ExtendedCastZoneKind> {
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    if core != "Retrace"
        || reminder.is_some_and(|reminder| {
            reminder
                != "You may cast this card from your graveyard by discarding a land card in addition to paying its other costs."
                && reminder != "Sorry, no room for reminder text."
        })
    {
        return None;
    }
    Some(ExtendedCastZoneKind::Retrace(RetraceProgram {
        cast_from_owners_graveyard: true,
        discard_land_additional_cost: true,
        retains_other_costs_and_timing: true,
    }))
}

fn parse_jump_start(exact_source: &str) -> Option<ExtendedCastZoneKind> {
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    if core != "Jump-start"
        || reminder.is_some_and(|reminder| {
            reminder
                != "You may cast this card from your graveyard by discarding a card in addition to paying its other costs. Then exile this card."
        })
    {
        return None;
    }
    Some(ExtendedCastZoneKind::JumpStart(JumpStartProgram {
        cast_from_owners_graveyard: true,
        discard_card_additional_cost: true,
        retains_other_costs_and_timing: true,
        every_stack_exit_replaced_with_exile: true,
    }))
}

fn candidate_family(source: &str) -> Option<&'static str> {
    let lower = source.to_ascii_lowercase();
    if contains_word(&lower, "foretell") || contains_word(&lower, "foretold") {
        Some("Foretell")
    } else if contains_word(&lower, "jump-start") {
        Some("Jump-start")
    } else if contains_word(&lower, "retrace") {
        Some("Retrace")
    } else if contains_word(&lower, "warp") {
        Some("Warp")
    } else if contains_word(&lower, "plot") {
        Some("Plot")
    } else {
        None
    }
}

fn contains_word(source: &str, needle: &str) -> bool {
    source.match_indices(needle).any(|(start, _)| {
        let before = source[..start].chars().next_back();
        let end = start + needle.len();
        let after = source[end..].chars().next();
        before.is_none_or(|character| !character.is_alphanumeric())
            && after.is_none_or(|character| !character.is_alphanumeric())
    })
}

fn extended_cast_zone_semantic_digest_with_versions(
    exact_source: &str,
    kind: &ExtendedCastZoneKind,
    compiler_version: &str,
    runtime_version: &str,
    rules_context_version: &str,
) -> String {
    let kind_contract = kind.stable_id();
    let mut hasher = Sha256::new();
    for component in [
        "extended-cast-zone-content/v1",
        compiler_version,
        runtime_version,
        rules_context_version,
        exact_source,
        &kind_contract,
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    if source.is_empty() || source.trim() != source {
        return None;
    }
    let mut symbols = Vec::new();
    let mut offset = 0_usize;
    while offset < source.len() {
        if source.as_bytes().get(offset).copied()? != b'{' {
            return None;
        }
        let token_start = offset + 1;
        let token_end = token_start + source[token_start..].find('}')?;
        let token = &source[token_start..token_end];
        if token.is_empty() || token.contains('{') || token.trim() != token {
            return None;
        }
        let upper = token.to_ascii_uppercase();
        let symbol = if upper.bytes().all(|byte| byte.is_ascii_digit()) {
            if upper.len() > 1 && upper.starts_with('0') {
                return None;
            }
            ManaSymbol::Generic(upper.parse::<u32>().ok()?)
        } else {
            match upper.as_str() {
                "W" => ManaSymbol::White,
                "U" => ManaSymbol::Blue,
                "B" => ManaSymbol::Black,
                "R" => ManaSymbol::Red,
                "G" => ManaSymbol::Green,
                "C" => ManaSymbol::Colorless,
                "S" => ManaSymbol::Snow,
                "X" => ManaSymbol::VariableX,
                _ => {
                    let (first, second) = upper.split_once('/')?;
                    ManaSymbol::Hybrid(parse_mana_color(first)?, parse_mana_color(second)?)
                }
            }
        };
        symbols.push(symbol);
        offset = token_end + 1;
    }
    (!symbols.is_empty()).then(|| ManaCost {
        exact: source.to_owned(),
        symbols,
    })
}

fn parse_mana_color(source: &str) -> Option<ManaColor> {
    match source {
        "W" => Some(ManaColor::White),
        "U" => Some(ManaColor::Blue),
        "B" => Some(ManaColor::Black),
        "R" => Some(ManaColor::Red),
        "G" => Some(ManaColor::Green),
        "C" => Some(ManaColor::Colorless),
        _ => None,
    }
}

fn split_trailing_parenthetical(source: &str) -> Option<(&str, Option<&str>)> {
    let mut depth = 0_u32;
    let mut outer_start = None;
    for (index, character) in source.char_indices() {
        match character {
            '(' => {
                if depth == 0 {
                    if outer_start.is_some() {
                        return None;
                    }
                    outer_start = Some(index);
                }
                depth = depth.checked_add(1)?;
            }
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 && index + 1 != source.len() {
                    return None;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    let Some(start) = outer_start else {
        return Some((source, None));
    };
    if !source[..start].ends_with(' ') || !source.ends_with(')') {
        return None;
    }
    let core = source[..start].trim_end();
    let reminder = &source[start + 1..source.len() - 1];
    (!core.is_empty() && !reminder.is_empty()).then_some((core, Some(reminder)))
}

fn collapse_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManaUnit {
    pub id: ManaUnitId,
    pub color: ManaColor,
    pub from_snow_source: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub life: i32,
    pub mana_pool: BTreeMap<ManaUnitId, ManaUnit>,
}

impl PlayerState {
    pub fn new(life: i32) -> Self {
        Self {
            life,
            mana_pool: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    OwnerOnly(PlayerId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastMethod {
    Foretell { semantic_digest: String },
    Plot { semantic_digest: String },
    Warp { semantic_digest: String },
    WarpExilePermission { semantic_digest: String },
    Retrace { semantic_digest: String },
    JumpStart { semantic_digest: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedObject {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub controller: Option<PlayerId>,
    pub zone: Zone,
    pub visibility: Visibility,
    pub card_types: BTreeSet<CardType>,
    pub printed_mana_cost: Option<ManaCost>,
    pub cast_method: Option<CastMethod>,
    pub is_copy: bool,
    pub copied_from: Option<ObjectRef>,
}

impl TrackedObject {
    pub fn card(
        object_ref: ObjectRef,
        owner: PlayerId,
        zone: Zone,
        card_types: impl IntoIterator<Item = CardType>,
        printed_mana_cost: Option<ManaCost>,
    ) -> Self {
        Self {
            object_ref,
            owner,
            controller: matches!(zone, Zone::Battlefield | Zone::Stack).then_some(owner),
            zone,
            visibility: Visibility::Public,
            card_types: card_types.into_iter().collect(),
            printed_mana_cost,
            cast_method: None,
            is_copy: false,
            copied_from: None,
        }
    }

    fn has_type(&self, card_type: CardType) -> bool {
        self.card_types.contains(&card_type)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Beginning,
    PrecombatMain,
    Combat,
    PostcombatMain,
    Ending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriorityWindow {
    pub turn: TurnId,
    pub active_player: PlayerId,
    pub phase: Phase,
    pub stack_empty: bool,
    pub priority_player: PlayerId,
}

impl PriorityWindow {
    pub fn player_has_priority(self, player: PlayerId) -> bool {
        self.priority_player == player
    }

    pub fn is_during_players_turn(self, player: PlayerId) -> bool {
        self.active_player == player && self.priority_player == player
    }

    pub fn is_sorcery_timing_for(self, player: PlayerId) -> bool {
        self.active_player == player
            && self.priority_player == player
            && self.stack_empty
            && matches!(self.phase, Phase::PrecombatMain | Phase::PostcombatMain)
    }

    pub fn is_beginning_of_end_step(self) -> bool {
        self.phase == Phase::Ending && self.stack_empty
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastLegality {
    pub normal_timing_allows_cast: bool,
    pub required_targets_available: bool,
    pub prohibitions_allow_cast: bool,
    pub external_additional_costs_satisfied: bool,
}

impl CastLegality {
    pub const fn fully_legal() -> Self {
        Self {
            normal_timing_allows_cast: true,
            required_targets_available: true,
            prohibitions_allow_cast: true,
            external_additional_costs_satisfied: true,
        }
    }

    fn permits_normal_cast(self) -> bool {
        self.normal_timing_allows_cast
            && self.required_targets_available
            && self.prohibitions_allow_cast
            && self.external_additional_costs_satisfied
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseManaCostChoice {
    Printed,
    /// A separate rule supplied this exact legal alternative cost. The
    /// permission digest is retained as evidence rather than inferred here.
    ExternallyPermittedAlternative {
        exact_cost: ManaCost,
        permission_digest: String,
    },
    WithoutPayingManaCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalZoneReplacement {
    pub effect_id: ReplacementEffectId,
    pub replaces_destination: Option<Zone>,
    pub destination: Zone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReplacementOrderEntry {
    JumpStartExile,
    External(ReplacementEffectId),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ManaPayment {
    pub x_value: u32,
    pub mana_units: Vec<ManaUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastPayment {
    pub base_mana_cost: BaseManaCostChoice,
    pub mana: ManaPayment,
    pub life: u32,
    pub discarded_card: Option<ObjectRef>,
    pub discard_replacements: Vec<ExternalZoneReplacement>,
    pub discard_replacement_order: Vec<ReplacementEffectId>,
}

impl CastPayment {
    pub fn printed(mana_units: Vec<ManaUnitId>) -> Self {
        Self {
            base_mana_cost: BaseManaCostChoice::Printed,
            mana: ManaPayment {
                x_value: 0,
                mana_units,
            },
            life: 0,
            discarded_card: None,
            discard_replacements: Vec::new(),
            discard_replacement_order: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPaymentEvidence {
    pub exact_cost: String,
    pub x_value: u32,
    pub mana_units_spent: Vec<ManaUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementApplicationEvidence {
    pub entry: ReplacementOrderEntry,
    pub destination_before: Zone,
    pub destination_after: Zone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneChangeEvidence {
    pub before: ObjectRef,
    pub after: ObjectRef,
    pub from: Zone,
    pub requested_destination: Zone,
    pub actual_destination: Zone,
    pub replacement_order: Vec<ReplacementOrderEntry>,
    pub replacements_applied: Vec<ReplacementApplicationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastCostEvidence {
    pub base_mana_cost: BaseManaCostChoice,
    pub mana: Option<ManaPaymentEvidence>,
    pub life_paid: u32,
    pub discarded_card: Option<ZoneChangeEvidence>,
    pub external_additional_costs_satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastEvidence {
    pub caster: PlayerId,
    pub turn: TurnId,
    pub source_zone_change: ZoneChangeEvidence,
    pub stack_object: ObjectRef,
    pub method: CastMethod,
    pub cost: CastCostEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForetellSpecialActionEvidence {
    pub player: PlayerId,
    pub turn: TurnId,
    pub source_zone_change: ZoneChangeEvidence,
    pub payment: ManaPaymentEvidence,
    pub hidden_identity_visible_only_to: Option<PlayerId>,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotSpecialActionEvidence {
    pub player: PlayerId,
    pub turn: TurnId,
    pub source_zone_change: ZoneChangeEvidence,
    pub payment: ManaPaymentEvidence,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarpResolutionEvidence {
    pub stack_zone_change: ZoneChangeEvidence,
    pub permanent: ObjectRef,
    pub delayed_exile_scheduled: bool,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarpEndStepTrigger {
    pub trigger_id: PendingTriggerId,
    pub controller: PlayerId,
    pub expected_permanent: ObjectRef,
    pub created_turn: TurnId,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarpEndStepResolution {
    SourceNoLongerThatPermanent,
    Moved {
        zone_change: ZoneChangeEvidence,
        later_cast_permission_created: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackExitEvidence {
    pub zone_change: ZoneChangeEvidence,
    pub jump_start_replacement_required: bool,
    pub jump_start_replacement_applied: bool,
}

#[derive(Debug, Clone)]
struct ForetoldPermission {
    holder: PlayerId,
    expected_card: ObjectRef,
    foretell_turn: TurnId,
    program: ExtendedCastZoneProgram,
}

#[derive(Debug, Clone)]
struct PlottedPermission {
    holder: PlayerId,
    expected_card: ObjectRef,
    plot_turn: TurnId,
    program: ExtendedCastZoneProgram,
}

#[derive(Debug, Clone)]
struct WarpDelayedExile {
    permission_holder: PlayerId,
    expected_permanent: ObjectRef,
    cast_turn: TurnId,
    semantic_digest: String,
}

#[derive(Debug, Clone)]
struct PendingWarpExile {
    trigger: WarpEndStepTrigger,
}

#[derive(Debug, Clone)]
struct WarpExilePermission {
    holder: PlayerId,
    expected_card: ObjectRef,
    exile_turn: TurnId,
    semantic_digest: String,
}

#[derive(Debug, Clone)]
pub struct ExtendedCastZoneRuntime {
    players: BTreeMap<PlayerId, PlayerState>,
    objects: BTreeMap<ObjectId, TrackedObject>,
    foretold: BTreeMap<ObjectId, ForetoldPermission>,
    plotted: BTreeMap<ObjectId, PlottedPermission>,
    warp_delayed_exiles: BTreeMap<ObjectRef, WarpDelayedExile>,
    pending_warp_exile: BTreeMap<PendingTriggerId, PendingWarpExile>,
    warp_exile_permissions: BTreeMap<ObjectId, WarpExilePermission>,
    next_trigger_id: u64,
}

impl Default for ExtendedCastZoneRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtendedCastZoneRuntime {
    pub fn new() -> Self {
        Self {
            players: BTreeMap::new(),
            objects: BTreeMap::new(),
            foretold: BTreeMap::new(),
            plotted: BTreeMap::new(),
            warp_delayed_exiles: BTreeMap::new(),
            pending_warp_exile: BTreeMap::new(),
            warp_exile_permissions: BTreeMap::new(),
            next_trigger_id: 1,
        }
    }

    pub fn insert_player(
        &mut self,
        player: PlayerId,
        state: PlayerState,
    ) -> Result<(), ExtendedCastZoneRuntimeError> {
        if self.players.contains_key(&player) {
            return Err(ExtendedCastZoneRuntimeError::DuplicatePlayer(player));
        }
        self.players.insert(player, state);
        Ok(())
    }

    pub fn insert_object(
        &mut self,
        object: TrackedObject,
    ) -> Result<(), ExtendedCastZoneRuntimeError> {
        if self.objects.contains_key(&object.object_ref.object_id) {
            return Err(ExtendedCastZoneRuntimeError::DuplicateObject(
                object.object_ref.object_id,
            ));
        }
        self.objects.insert(object.object_ref.object_id, object);
        Ok(())
    }

    pub fn player(&self, player: PlayerId) -> Option<&PlayerState> {
        self.players.get(&player)
    }

    pub fn player_mut(&mut self, player: PlayerId) -> Option<&mut PlayerState> {
        self.players.get_mut(&player)
    }

    pub fn object(&self, object: ObjectRef) -> Option<&TrackedObject> {
        self.objects
            .get(&object.object_id)
            .filter(|tracked| tracked.object_ref == object)
    }

    pub fn current_object(&self, object_id: ObjectId) -> Option<&TrackedObject> {
        self.objects.get(&object_id)
    }

    // The explicit arguments keep the public special action transaction clear.
    #[allow(clippy::too_many_arguments)]
    pub fn perform_foretell_special_action(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        program: &ExtendedCastZoneProgram,
        window: PriorityWindow,
        payment: ManaPayment,
        replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<ForetellSpecialActionEvidence, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.perform_foretell_special_action_inner(
            player,
            source,
            program,
            window,
            payment,
            replacements,
            replacement_order,
        )?;
        *self = staged;
        Ok(evidence)
    }

    // The explicit arguments mirror the staged special action transaction.
    #[allow(clippy::too_many_arguments)]
    fn perform_foretell_special_action_inner(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        program: &ExtendedCastZoneProgram,
        window: PriorityWindow,
        payment: ManaPayment,
        replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<ForetellSpecialActionEvidence, ExtendedCastZoneRuntimeError> {
        let ExtendedCastZoneKind::Foretell(foretell) = program.kind() else {
            return Err(ExtendedCastZoneRuntimeError::WrongProgramKind);
        };
        if !window.is_during_players_turn(player) {
            return Err(ExtendedCastZoneRuntimeError::IllegalSpecialActionTiming);
        }
        let source_object = self.require_exact_object(source, Zone::Hand)?.clone();
        if source_object.owner != player || source_object.is_copy {
            return Err(ExtendedCastZoneRuntimeError::WrongOwner);
        }
        let payment_evidence = self.pay_mana(player, &foretell.special_action_cost, &payment)?;
        let change = self.move_object_with_external_replacements(
            source,
            Zone::Exile,
            &replacements,
            &replacement_order,
        )?;
        let visible_only = (change.actual_destination == Zone::Exile).then_some(player);
        if change.actual_destination == Zone::Exile {
            let tracked = self
                .objects
                .get_mut(&source.object_id)
                .expect("moved object remains tracked");
            tracked.visibility = Visibility::OwnerOnly(player);
            self.foretold.insert(
                source.object_id,
                ForetoldPermission {
                    holder: player,
                    expected_card: change.after,
                    foretell_turn: window.turn,
                    program: program.clone(),
                },
            );
        }
        Ok(ForetellSpecialActionEvidence {
            player,
            turn: window.turn,
            source_zone_change: change,
            payment: payment_evidence,
            hidden_identity_visible_only_to: visible_only,
            semantic_digest: program.semantic_digest().to_owned(),
        })
    }

    pub fn cast_foretold_card(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence =
            staged.cast_foretold_card_inner(player, source, window, legality, payment)?;
        *self = staged;
        Ok(evidence)
    }

    fn cast_foretold_card_inner(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        let permission = self
            .foretold
            .get(&source.object_id)
            .cloned()
            .ok_or(ExtendedCastZoneRuntimeError::MissingCastPermission)?;
        if permission.holder != player
            || permission.expected_card != source
            || window.turn.0 <= permission.foretell_turn.0
        {
            return Err(ExtendedCastZoneRuntimeError::CastPermissionNotYetUsable);
        }
        if !window.player_has_priority(player) || !legality.permits_normal_cast() {
            return Err(ExtendedCastZoneRuntimeError::CastIsNotLegal);
        }
        let ExtendedCastZoneKind::Foretell(foretell) = permission.program.kind() else {
            return Err(ExtendedCastZoneRuntimeError::WrongProgramKind);
        };
        let method = CastMethod::Foretell {
            semantic_digest: permission.program.semantic_digest().to_owned(),
        };
        let cost = self.pay_cast_cost(
            player,
            source.object_id,
            CostMode::ExactAlternative {
                mana: &foretell.foretell_cost,
                life: 0,
                permission_digest: permission.program.semantic_digest(),
            },
            &payment,
            legality,
        )?;
        let change = self.move_object_without_replacements(source, Zone::Stack)?;
        self.set_stack_cast_state(change.after, player, method.clone())?;
        self.foretold.remove(&source.object_id);
        Ok(CastEvidence {
            caster: player,
            turn: window.turn,
            stack_object: change.after,
            source_zone_change: change,
            method,
            cost,
        })
    }

    // The explicit arguments keep the public special action transaction clear.
    #[allow(clippy::too_many_arguments)]
    pub fn perform_plot_special_action(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        program: &ExtendedCastZoneProgram,
        window: PriorityWindow,
        payment: ManaPayment,
        replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<PlotSpecialActionEvidence, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.perform_plot_special_action_inner(
            player,
            source,
            program,
            window,
            payment,
            replacements,
            replacement_order,
        )?;
        *self = staged;
        Ok(evidence)
    }

    // The explicit arguments mirror the staged special action transaction.
    #[allow(clippy::too_many_arguments)]
    fn perform_plot_special_action_inner(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        program: &ExtendedCastZoneProgram,
        window: PriorityWindow,
        payment: ManaPayment,
        replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<PlotSpecialActionEvidence, ExtendedCastZoneRuntimeError> {
        let ExtendedCastZoneKind::Plot(plot) = program.kind() else {
            return Err(ExtendedCastZoneRuntimeError::WrongProgramKind);
        };
        if !window.is_sorcery_timing_for(player) {
            return Err(ExtendedCastZoneRuntimeError::IllegalSpecialActionTiming);
        }
        let source_object = self.require_exact_object(source, Zone::Hand)?.clone();
        if source_object.owner != player || source_object.is_copy {
            return Err(ExtendedCastZoneRuntimeError::WrongOwner);
        }
        let payment_evidence = self.pay_mana(player, &plot.plot_cost, &payment)?;
        let change = self.move_object_with_external_replacements(
            source,
            Zone::Exile,
            &replacements,
            &replacement_order,
        )?;
        if change.actual_destination == Zone::Exile {
            self.plotted.insert(
                source.object_id,
                PlottedPermission {
                    holder: player,
                    expected_card: change.after,
                    plot_turn: window.turn,
                    program: program.clone(),
                },
            );
        }
        Ok(PlotSpecialActionEvidence {
            player,
            turn: window.turn,
            source_zone_change: change,
            payment: payment_evidence,
            semantic_digest: program.semantic_digest().to_owned(),
        })
    }

    pub fn cast_plotted_card(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.cast_plotted_card_inner(player, source, window, legality, payment)?;
        *self = staged;
        Ok(evidence)
    }

    fn cast_plotted_card_inner(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        let permission = self
            .plotted
            .get(&source.object_id)
            .cloned()
            .ok_or(ExtendedCastZoneRuntimeError::MissingCastPermission)?;
        if permission.holder != player
            || permission.expected_card != source
            || window.turn.0 <= permission.plot_turn.0
        {
            return Err(ExtendedCastZoneRuntimeError::CastPermissionNotYetUsable);
        }
        if !window.is_sorcery_timing_for(player) || !legality.permits_normal_cast() {
            return Err(ExtendedCastZoneRuntimeError::CastIsNotLegal);
        }
        let method = CastMethod::Plot {
            semantic_digest: permission.program.semantic_digest().to_owned(),
        };
        let cost = self.pay_cast_cost(
            player,
            source.object_id,
            CostMode::WithoutPayingManaCost,
            &payment,
            legality,
        )?;
        let change = self.move_object_without_replacements(source, Zone::Stack)?;
        self.set_stack_cast_state(change.after, player, method.clone())?;
        self.plotted.remove(&source.object_id);
        Ok(CastEvidence {
            caster: player,
            turn: window.turn,
            stack_object: change.after,
            source_zone_change: change,
            method,
            cost,
        })
    }

    pub fn cast_with_warp(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        program: &ExtendedCastZoneProgram,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence =
            staged.cast_with_warp_inner(player, source, program, window, legality, payment)?;
        *self = staged;
        Ok(evidence)
    }

    fn cast_with_warp_inner(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        program: &ExtendedCastZoneProgram,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        let ExtendedCastZoneKind::Warp(warp) = program.kind() else {
            return Err(ExtendedCastZoneRuntimeError::WrongProgramKind);
        };
        if !window.player_has_priority(player) || !legality.permits_normal_cast() {
            return Err(ExtendedCastZoneRuntimeError::CastIsNotLegal);
        }
        let source_object = self
            .objects
            .get(&source.object_id)
            .filter(|tracked| tracked.object_ref == source)
            .cloned()
            .ok_or(ExtendedCastZoneRuntimeError::MissingObject(source))?;
        let zone_allowed = source_object.zone == Zone::Hand
            || (warp.source_permission == WarpSourcePermission::HandOrOwnersGraveyard
                && source_object.zone == Zone::Graveyard);
        if !zone_allowed {
            return Err(ExtendedCastZoneRuntimeError::WrongZone {
                expected: Zone::Hand,
                actual: source_object.zone,
            });
        }
        if source_object.owner != player
            || source_object.is_copy
            || !warp.permanent_kind.matches(&source_object.card_types)
        {
            return Err(ExtendedCastZoneRuntimeError::WrongOwnerOrCharacteristics);
        }
        let method = CastMethod::Warp {
            semantic_digest: program.semantic_digest().to_owned(),
        };
        let cost = self.pay_cast_cost(
            player,
            source.object_id,
            CostMode::ExactAlternative {
                mana: &warp.warp_cost,
                life: warp.life_cost,
                permission_digest: program.semantic_digest(),
            },
            &payment,
            legality,
        )?;
        let change = self.move_object_without_replacements(source, Zone::Stack)?;
        self.set_stack_cast_state(change.after, player, method.clone())?;
        let expected_permanent = ObjectRef {
            object_id: change.after.object_id,
            incarnation_id: IncarnationId(
                change
                    .after
                    .incarnation_id
                    .0
                    .checked_add(1)
                    .ok_or(ExtendedCastZoneRuntimeError::IncarnationOverflow)?,
            ),
        };
        self.warp_delayed_exiles.insert(
            change.after,
            WarpDelayedExile {
                permission_holder: source_object.owner,
                expected_permanent,
                cast_turn: window.turn,
                semantic_digest: program.semantic_digest().to_owned(),
            },
        );
        Ok(CastEvidence {
            caster: player,
            turn: window.turn,
            stack_object: change.after,
            source_zone_change: change,
            method,
            cost,
        })
    }

    pub fn resolve_warp_spell_as_permanent(
        &mut self,
        stack_object: ObjectRef,
        _resolution_turn: TurnId,
    ) -> Result<WarpResolutionEvidence, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.resolve_warp_spell_as_permanent_inner(stack_object)?;
        *self = staged;
        Ok(evidence)
    }

    fn resolve_warp_spell_as_permanent_inner(
        &mut self,
        stack_object: ObjectRef,
    ) -> Result<WarpResolutionEvidence, ExtendedCastZoneRuntimeError> {
        let stack = self
            .require_exact_object(stack_object, Zone::Stack)?
            .clone();
        let Some(CastMethod::Warp { semantic_digest }) = stack.cast_method.clone() else {
            return Err(ExtendedCastZoneRuntimeError::NotCastUsingWarp);
        };
        if stack.is_copy {
            return Err(ExtendedCastZoneRuntimeError::CopiedSpellWasNotCast);
        }
        let delayed = self.warp_delayed_exiles.get(&stack_object).cloned();
        if delayed
            .as_ref()
            .is_some_and(|delayed| delayed.semantic_digest != semantic_digest)
        {
            return Err(ExtendedCastZoneRuntimeError::WarpSemanticDigestMismatch);
        }
        let controller = stack
            .controller
            .ok_or(ExtendedCastZoneRuntimeError::MissingController)?;
        let change = self.move_object_without_replacements(stack_object, Zone::Battlefield)?;
        let tracked = self
            .objects
            .get_mut(&stack_object.object_id)
            .expect("resolved permanent remains tracked");
        tracked.controller = Some(controller);
        tracked.visibility = Visibility::Public;
        tracked.cast_method = Some(CastMethod::Warp {
            semantic_digest: semantic_digest.clone(),
        });
        if delayed
            .as_ref()
            .is_some_and(|delayed| delayed.expected_permanent != change.after)
        {
            return Err(ExtendedCastZoneRuntimeError::WarpIncarnationMismatch);
        }
        Ok(WarpResolutionEvidence {
            stack_zone_change: change.clone(),
            permanent: change.after,
            delayed_exile_scheduled: true,
            semantic_digest,
        })
    }

    pub fn begin_end_step(
        &mut self,
        window: PriorityWindow,
    ) -> Result<Vec<WarpEndStepTrigger>, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let triggers = staged.begin_end_step_inner(window)?;
        *self = staged;
        Ok(triggers)
    }

    fn begin_end_step_inner(
        &mut self,
        window: PriorityWindow,
    ) -> Result<Vec<WarpEndStepTrigger>, ExtendedCastZoneRuntimeError> {
        if !window.is_beginning_of_end_step() {
            return Err(ExtendedCastZoneRuntimeError::NotBeginningOfEndStep);
        }
        let due = self
            .warp_delayed_exiles
            .iter()
            .filter(|(_, record)| window.turn.0 >= record.cast_turn.0)
            .map(|(stack_ref, _)| *stack_ref)
            .collect::<Vec<_>>();
        let mut triggers = Vec::with_capacity(due.len());
        for stack_ref in due {
            let record = self
                .warp_delayed_exiles
                .remove(&stack_ref)
                .expect("due record exists");
            let trigger_id = PendingTriggerId(self.next_trigger_id);
            self.next_trigger_id = self
                .next_trigger_id
                .checked_add(1)
                .ok_or(ExtendedCastZoneRuntimeError::TriggerIdOverflow)?;
            let trigger = WarpEndStepTrigger {
                trigger_id,
                controller: record.permission_holder,
                expected_permanent: record.expected_permanent,
                created_turn: window.turn,
                semantic_digest: record.semantic_digest,
            };
            self.pending_warp_exile.insert(
                trigger_id,
                PendingWarpExile {
                    trigger: trigger.clone(),
                },
            );
            triggers.push(trigger);
        }
        Ok(triggers)
    }

    pub fn resolve_warp_end_step_trigger(
        &mut self,
        trigger_id: PendingTriggerId,
        replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<WarpEndStepResolution, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let resolution = staged.resolve_warp_end_step_trigger_inner(
            trigger_id,
            replacements,
            replacement_order,
        )?;
        *self = staged;
        Ok(resolution)
    }

    fn resolve_warp_end_step_trigger_inner(
        &mut self,
        trigger_id: PendingTriggerId,
        replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementEffectId>,
    ) -> Result<WarpEndStepResolution, ExtendedCastZoneRuntimeError> {
        let pending = self
            .pending_warp_exile
            .remove(&trigger_id)
            .ok_or(ExtendedCastZoneRuntimeError::MissingWarpTrigger(trigger_id))?;
        let Some(current) = self
            .objects
            .get(&pending.trigger.expected_permanent.object_id)
        else {
            return Ok(WarpEndStepResolution::SourceNoLongerThatPermanent);
        };
        if current.object_ref != pending.trigger.expected_permanent
            || current.zone != Zone::Battlefield
        {
            return Ok(WarpEndStepResolution::SourceNoLongerThatPermanent);
        }
        let change = self.move_object_with_external_replacements(
            pending.trigger.expected_permanent,
            Zone::Exile,
            &replacements,
            &replacement_order,
        )?;
        let permission_created = change.actual_destination == Zone::Exile;
        if permission_created {
            self.warp_exile_permissions.insert(
                change.after.object_id,
                WarpExilePermission {
                    holder: pending.trigger.controller,
                    expected_card: change.after,
                    exile_turn: pending.trigger.created_turn,
                    semantic_digest: pending.trigger.semantic_digest,
                },
            );
        }
        Ok(WarpEndStepResolution::Moved {
            zone_change: change,
            later_cast_permission_created: permission_created,
        })
    }

    pub fn cast_warped_card_from_exile(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence =
            staged.cast_warped_card_from_exile_inner(player, source, window, legality, payment)?;
        *self = staged;
        Ok(evidence)
    }

    fn cast_warped_card_from_exile_inner(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        let permission = self
            .warp_exile_permissions
            .get(&source.object_id)
            .cloned()
            .ok_or(ExtendedCastZoneRuntimeError::MissingCastPermission)?;
        if permission.holder != player
            || permission.expected_card != source
            || window.turn.0 <= permission.exile_turn.0
        {
            return Err(ExtendedCastZoneRuntimeError::CastPermissionNotYetUsable);
        }
        if !window.player_has_priority(player) || !legality.permits_normal_cast() {
            return Err(ExtendedCastZoneRuntimeError::CastIsNotLegal);
        }
        let method = CastMethod::WarpExilePermission {
            semantic_digest: permission.semantic_digest,
        };
        let cost = self.pay_cast_cost(
            player,
            source.object_id,
            CostMode::PrintedOrExternallyPermitted,
            &payment,
            legality,
        )?;
        let change = self.move_object_without_replacements(source, Zone::Stack)?;
        self.set_stack_cast_state(change.after, player, method.clone())?;
        self.warp_exile_permissions.remove(&source.object_id);
        Ok(CastEvidence {
            caster: player,
            turn: window.turn,
            stack_object: change.after,
            source_zone_change: change,
            method,
            cost,
        })
    }

    pub fn cast_with_retrace(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        program: &ExtendedCastZoneProgram,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence =
            staged.cast_with_retrace_inner(player, source, program, window, legality, payment)?;
        *self = staged;
        Ok(evidence)
    }

    fn cast_with_retrace_inner(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        program: &ExtendedCastZoneProgram,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        if !matches!(program.kind(), ExtendedCastZoneKind::Retrace(_)) {
            return Err(ExtendedCastZoneRuntimeError::WrongProgramKind);
        }
        self.validate_graveyard_cast_source(player, source)?;
        if !window.player_has_priority(player) || !legality.permits_normal_cast() {
            return Err(ExtendedCastZoneRuntimeError::CastIsNotLegal);
        }
        let method = CastMethod::Retrace {
            semantic_digest: program.semantic_digest().to_owned(),
        };
        let cost = self.pay_cast_cost(
            player,
            source.object_id,
            CostMode::PrintedWithDiscard {
                filter: DiscardFilter::Land,
            },
            &payment,
            legality,
        )?;
        let change = self.move_object_without_replacements(source, Zone::Stack)?;
        self.set_stack_cast_state(change.after, player, method.clone())?;
        Ok(CastEvidence {
            caster: player,
            turn: window.turn,
            stack_object: change.after,
            source_zone_change: change,
            method,
            cost,
        })
    }

    pub fn cast_with_jump_start(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        program: &ExtendedCastZoneProgram,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged
            .cast_with_jump_start_inner(player, source, program, window, legality, payment)?;
        *self = staged;
        Ok(evidence)
    }

    fn cast_with_jump_start_inner(
        &mut self,
        player: PlayerId,
        source: ObjectRef,
        program: &ExtendedCastZoneProgram,
        window: PriorityWindow,
        legality: CastLegality,
        payment: CastPayment,
    ) -> Result<CastEvidence, ExtendedCastZoneRuntimeError> {
        if !matches!(program.kind(), ExtendedCastZoneKind::JumpStart(_)) {
            return Err(ExtendedCastZoneRuntimeError::WrongProgramKind);
        }
        self.validate_graveyard_cast_source(player, source)?;
        let source_object = self.require_exact_object(source, Zone::Graveyard)?;
        if !source_object.has_type(CardType::Instant) && !source_object.has_type(CardType::Sorcery)
        {
            return Err(ExtendedCastZoneRuntimeError::WrongOwnerOrCharacteristics);
        }
        if !window.player_has_priority(player) || !legality.permits_normal_cast() {
            return Err(ExtendedCastZoneRuntimeError::CastIsNotLegal);
        }
        let method = CastMethod::JumpStart {
            semantic_digest: program.semantic_digest().to_owned(),
        };
        let cost = self.pay_cast_cost(
            player,
            source.object_id,
            CostMode::PrintedWithDiscard {
                filter: DiscardFilter::AnyCard,
            },
            &payment,
            legality,
        )?;
        let change = self.move_object_without_replacements(source, Zone::Stack)?;
        self.set_stack_cast_state(change.after, player, method.clone())?;
        Ok(CastEvidence {
            caster: player,
            turn: window.turn,
            stack_object: change.after,
            source_zone_change: change,
            method,
            cost,
        })
    }

    pub fn copy_spell(
        &mut self,
        original: ObjectRef,
        copy_object_id: ObjectId,
    ) -> Result<ObjectRef, ExtendedCastZoneRuntimeError> {
        let original = self.require_exact_object(original, Zone::Stack)?.clone();
        if self.objects.contains_key(&copy_object_id) {
            return Err(ExtendedCastZoneRuntimeError::DuplicateObject(
                copy_object_id,
            ));
        }
        let copy_ref = ObjectRef {
            object_id: copy_object_id,
            incarnation_id: IncarnationId(1),
        };
        self.objects.insert(
            copy_object_id,
            TrackedObject {
                object_ref: copy_ref,
                owner: original.owner,
                controller: original.controller,
                zone: Zone::Stack,
                visibility: Visibility::Public,
                card_types: original.card_types,
                printed_mana_cost: original.printed_mana_cost,
                cast_method: None,
                is_copy: true,
                copied_from: Some(original.object_ref),
            },
        );
        Ok(copy_ref)
    }

    pub fn leave_stack(
        &mut self,
        stack_object: ObjectRef,
        requested_destination: Zone,
        external_replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementOrderEntry>,
    ) -> Result<StackExitEvidence, ExtendedCastZoneRuntimeError> {
        let mut staged = self.clone();
        let evidence = staged.leave_stack_inner(
            stack_object,
            requested_destination,
            external_replacements,
            replacement_order,
        )?;
        *self = staged;
        Ok(evidence)
    }

    fn leave_stack_inner(
        &mut self,
        stack_object: ObjectRef,
        requested_destination: Zone,
        external_replacements: Vec<ExternalZoneReplacement>,
        replacement_order: Vec<ReplacementOrderEntry>,
    ) -> Result<StackExitEvidence, ExtendedCastZoneRuntimeError> {
        let object = self.require_exact_object(stack_object, Zone::Stack)?;
        let jump_start_required = matches!(object.cast_method, Some(CastMethod::JumpStart { .. }))
            && !object.is_copy
            && requested_destination != Zone::Exile;
        let change = self.move_object_with_stack_replacements(
            stack_object,
            requested_destination,
            &external_replacements,
            &replacement_order,
            jump_start_required,
        )?;
        let applied = change
            .replacements_applied
            .iter()
            .any(|step| step.entry == ReplacementOrderEntry::JumpStartExile);
        Ok(StackExitEvidence {
            zone_change: change,
            jump_start_replacement_required: jump_start_required,
            jump_start_replacement_applied: applied,
        })
    }

    fn validate_graveyard_cast_source(
        &self,
        player: PlayerId,
        source: ObjectRef,
    ) -> Result<(), ExtendedCastZoneRuntimeError> {
        let object = self.require_exact_object(source, Zone::Graveyard)?;
        if object.owner != player || object.is_copy {
            return Err(ExtendedCastZoneRuntimeError::WrongOwner);
        }
        let is_nonland_spell_card = !object.has_type(CardType::Land)
            && object.card_types.iter().any(|card_type| {
                matches!(
                    card_type,
                    CardType::Artifact
                        | CardType::Battle
                        | CardType::Creature
                        | CardType::Enchantment
                        | CardType::Instant
                        | CardType::Kindred
                        | CardType::Planeswalker
                        | CardType::Sorcery
                )
            });
        if !is_nonland_spell_card {
            return Err(ExtendedCastZoneRuntimeError::WrongOwnerOrCharacteristics);
        }
        Ok(())
    }

    fn pay_cast_cost(
        &mut self,
        player: PlayerId,
        source_object_id: ObjectId,
        mode: CostMode<'_>,
        payment: &CastPayment,
        legality: CastLegality,
    ) -> Result<CastCostEvidence, ExtendedCastZoneRuntimeError> {
        let (mana_cost, life_cost, expects_discard) = match mode {
            CostMode::ExactAlternative {
                mana,
                life,
                permission_digest,
            } => {
                match &payment.base_mana_cost {
                    BaseManaCostChoice::ExternallyPermittedAlternative {
                        exact_cost,
                        permission_digest: evidence_digest,
                    } if exact_cost == mana && evidence_digest == permission_digest => {}
                    _ => {
                        return Err(ExtendedCastZoneRuntimeError::BaseManaCostChoiceDoesNotMatch);
                    }
                }
                (Some(mana.clone()), life, None)
            }
            CostMode::WithoutPayingManaCost => {
                if payment.base_mana_cost != BaseManaCostChoice::WithoutPayingManaCost
                    || payment.mana.x_value != 0
                {
                    return Err(ExtendedCastZoneRuntimeError::BaseManaCostChoiceDoesNotMatch);
                }
                (None, 0, None)
            }
            CostMode::PrintedWithDiscard { filter } => {
                if payment.base_mana_cost != BaseManaCostChoice::Printed {
                    return Err(ExtendedCastZoneRuntimeError::BaseManaCostChoiceDoesNotMatch);
                }
                let printed = self
                    .objects
                    .get(&source_object_id)
                    .and_then(|object| object.printed_mana_cost.clone())
                    .ok_or(ExtendedCastZoneRuntimeError::MissingPrintedManaCost)?;
                (Some(printed), 0, Some(filter))
            }
            CostMode::PrintedOrExternallyPermitted => {
                let mana = match &payment.base_mana_cost {
                    BaseManaCostChoice::Printed => self
                        .objects
                        .get(&source_object_id)
                        .and_then(|object| object.printed_mana_cost.clone())
                        .ok_or(ExtendedCastZoneRuntimeError::MissingPrintedManaCost)?,
                    BaseManaCostChoice::ExternallyPermittedAlternative {
                        exact_cost,
                        permission_digest,
                    } if !permission_digest.is_empty() => exact_cost.clone(),
                    _ => {
                        return Err(ExtendedCastZoneRuntimeError::BaseManaCostChoiceDoesNotMatch);
                    }
                };
                (Some(mana), 0, None)
            }
        };

        if payment.life != life_cost {
            return Err(ExtendedCastZoneRuntimeError::WrongLifePayment {
                expected: life_cost,
                actual: payment.life,
            });
        }
        if life_cost > 0 {
            let state = self
                .players
                .get(&player)
                .ok_or(ExtendedCastZoneRuntimeError::MissingPlayer(player))?;
            if i64::from(state.life) <= i64::from(life_cost) {
                return Err(ExtendedCastZoneRuntimeError::CannotPayLife);
            }
        }

        let mana_evidence = if let Some(cost) = mana_cost.as_ref() {
            Some(self.pay_mana(player, cost, &payment.mana)?)
        } else {
            if !payment.mana.mana_units.is_empty() {
                return Err(ExtendedCastZoneRuntimeError::UnexpectedPaymentEvidence(
                    "mana",
                ));
            }
            None
        };

        let discard_evidence = match expects_discard {
            Some(filter) => {
                let discarded = payment
                    .discarded_card
                    .ok_or(ExtendedCastZoneRuntimeError::MissingDiscardPayment)?;
                let object = self.require_exact_object(discarded, Zone::Hand)?;
                if object.owner != player
                    || object.is_copy
                    || (filter == DiscardFilter::Land && !object.has_type(CardType::Land))
                {
                    return Err(ExtendedCastZoneRuntimeError::DiscardDoesNotMatchCost);
                }
                Some(self.move_object_with_external_replacements(
                    discarded,
                    Zone::Graveyard,
                    &payment.discard_replacements,
                    &payment.discard_replacement_order,
                )?)
            }
            None => {
                if payment.discarded_card.is_some()
                    || !payment.discard_replacements.is_empty()
                    || !payment.discard_replacement_order.is_empty()
                {
                    return Err(ExtendedCastZoneRuntimeError::UnexpectedPaymentEvidence(
                        "discard",
                    ));
                }
                None
            }
        };

        if life_cost > 0 {
            let state = self
                .players
                .get_mut(&player)
                .expect("life payment player was validated");
            state.life -= life_cost as i32;
        }

        Ok(CastCostEvidence {
            base_mana_cost: payment.base_mana_cost.clone(),
            mana: mana_evidence,
            life_paid: life_cost,
            discarded_card: discard_evidence,
            external_additional_costs_satisfied: legality.external_additional_costs_satisfied,
        })
    }

    fn pay_mana(
        &mut self,
        player: PlayerId,
        cost: &ManaCost,
        payment: &ManaPayment,
    ) -> Result<ManaPaymentEvidence, ExtendedCastZoneRuntimeError> {
        if !cost.contains_x() && payment.x_value != 0 {
            return Err(ExtendedCastZoneRuntimeError::UnexpectedVariableX);
        }
        let mut fixed = Vec::new();
        let mut generic = 0_u32;
        for symbol in &cost.symbols {
            match symbol {
                ManaSymbol::Generic(amount) => {
                    generic = generic
                        .checked_add(*amount)
                        .ok_or(ExtendedCastZoneRuntimeError::ManaRequirementOverflow)?;
                }
                ManaSymbol::VariableX => {
                    generic = generic
                        .checked_add(payment.x_value)
                        .ok_or(ExtendedCastZoneRuntimeError::ManaRequirementOverflow)?;
                }
                ManaSymbol::White => fixed.push(ManaRequirement::Color(ManaColor::White)),
                ManaSymbol::Blue => fixed.push(ManaRequirement::Color(ManaColor::Blue)),
                ManaSymbol::Black => fixed.push(ManaRequirement::Color(ManaColor::Black)),
                ManaSymbol::Red => fixed.push(ManaRequirement::Color(ManaColor::Red)),
                ManaSymbol::Green => fixed.push(ManaRequirement::Color(ManaColor::Green)),
                ManaSymbol::Colorless => fixed.push(ManaRequirement::Color(ManaColor::Colorless)),
                ManaSymbol::Snow => fixed.push(ManaRequirement::Snow),
                ManaSymbol::Hybrid(first, second) => {
                    fixed.push(ManaRequirement::Hybrid(*first, *second));
                }
            }
        }
        let expected = fixed
            .len()
            .checked_add(
                usize::try_from(generic)
                    .map_err(|_| ExtendedCastZoneRuntimeError::ManaRequirementOverflow)?,
            )
            .ok_or(ExtendedCastZoneRuntimeError::ManaRequirementOverflow)?;
        if payment.mana_units.len() != expected {
            return Err(ExtendedCastZoneRuntimeError::WrongManaSelectionCount {
                expected,
                actual: payment.mana_units.len(),
            });
        }
        require_distinct(payment.mana_units.iter().copied(), "mana")?;
        let state = self
            .players
            .get(&player)
            .ok_or(ExtendedCastZoneRuntimeError::MissingPlayer(player))?;
        let units = payment
            .mana_units
            .iter()
            .map(|id| {
                state
                    .mana_pool
                    .get(id)
                    .copied()
                    .ok_or(ExtendedCastZoneRuntimeError::MissingManaUnit(*id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !assign_fixed_mana(&fixed, &units, 0, &mut BTreeSet::new()) {
            return Err(ExtendedCastZoneRuntimeError::ManaPaymentDoesNotMatchCost);
        }
        let state = self
            .players
            .get_mut(&player)
            .expect("mana payment player was validated");
        for id in &payment.mana_units {
            state.mana_pool.remove(id);
        }
        Ok(ManaPaymentEvidence {
            exact_cost: cost.exact.clone(),
            x_value: payment.x_value,
            mana_units_spent: payment.mana_units.clone(),
        })
    }

    fn require_exact_object(
        &self,
        object_ref: ObjectRef,
        expected_zone: Zone,
    ) -> Result<&TrackedObject, ExtendedCastZoneRuntimeError> {
        let object = self
            .objects
            .get(&object_ref.object_id)
            .ok_or(ExtendedCastZoneRuntimeError::MissingObject(object_ref))?;
        if object.object_ref != object_ref {
            return Err(ExtendedCastZoneRuntimeError::StaleIncarnation(object_ref));
        }
        if object.zone != expected_zone {
            return Err(ExtendedCastZoneRuntimeError::WrongZone {
                expected: expected_zone,
                actual: object.zone,
            });
        }
        Ok(object)
    }

    fn move_object_without_replacements(
        &mut self,
        object_ref: ObjectRef,
        destination: Zone,
    ) -> Result<ZoneChangeEvidence, ExtendedCastZoneRuntimeError> {
        self.move_object_to_destination(
            object_ref,
            destination,
            destination,
            Vec::new(),
            Vec::new(),
        )
    }

    fn move_object_with_external_replacements(
        &mut self,
        object_ref: ObjectRef,
        requested_destination: Zone,
        replacements: &[ExternalZoneReplacement],
        replacement_order: &[ReplacementEffectId],
    ) -> Result<ZoneChangeEvidence, ExtendedCastZoneRuntimeError> {
        validate_external_replacement_order(replacements, replacement_order)?;
        let mut destination = requested_destination;
        let mut applications = Vec::new();
        for effect_id in replacement_order {
            let replacement = replacements
                .iter()
                .find(|replacement| replacement.effect_id == *effect_id)
                .expect("replacement order was validated");
            if replacement
                .replaces_destination
                .is_none_or(|expected| expected == destination)
            {
                let before = destination;
                destination = replacement.destination;
                applications.push(ReplacementApplicationEvidence {
                    entry: ReplacementOrderEntry::External(*effect_id),
                    destination_before: before,
                    destination_after: destination,
                });
            }
        }
        self.move_object_to_destination(
            object_ref,
            requested_destination,
            destination,
            replacement_order
                .iter()
                .copied()
                .map(ReplacementOrderEntry::External)
                .collect(),
            applications,
        )
    }

    fn move_object_with_stack_replacements(
        &mut self,
        object_ref: ObjectRef,
        requested_destination: Zone,
        replacements: &[ExternalZoneReplacement],
        replacement_order: &[ReplacementOrderEntry],
        jump_start_required: bool,
    ) -> Result<ZoneChangeEvidence, ExtendedCastZoneRuntimeError> {
        validate_stack_replacement_order(replacements, replacement_order, jump_start_required)?;
        let mut destination = requested_destination;
        let mut applications = Vec::new();
        for entry in replacement_order {
            match entry {
                ReplacementOrderEntry::JumpStartExile => {
                    if destination != Zone::Exile {
                        let before = destination;
                        destination = Zone::Exile;
                        applications.push(ReplacementApplicationEvidence {
                            entry: *entry,
                            destination_before: before,
                            destination_after: destination,
                        });
                    }
                }
                ReplacementOrderEntry::External(effect_id) => {
                    let replacement = replacements
                        .iter()
                        .find(|replacement| replacement.effect_id == *effect_id)
                        .expect("replacement order was validated");
                    if replacement
                        .replaces_destination
                        .is_none_or(|expected| expected == destination)
                    {
                        let before = destination;
                        destination = replacement.destination;
                        applications.push(ReplacementApplicationEvidence {
                            entry: *entry,
                            destination_before: before,
                            destination_after: destination,
                        });
                    }
                }
            }
        }
        self.move_object_to_destination(
            object_ref,
            requested_destination,
            destination,
            replacement_order.to_vec(),
            applications,
        )
    }

    fn move_object_to_destination(
        &mut self,
        object_ref: ObjectRef,
        requested_destination: Zone,
        actual_destination: Zone,
        replacement_order: Vec<ReplacementOrderEntry>,
        replacements_applied: Vec<ReplacementApplicationEvidence>,
    ) -> Result<ZoneChangeEvidence, ExtendedCastZoneRuntimeError> {
        let object = self
            .objects
            .get_mut(&object_ref.object_id)
            .ok_or(ExtendedCastZoneRuntimeError::MissingObject(object_ref))?;
        if object.object_ref != object_ref {
            return Err(ExtendedCastZoneRuntimeError::StaleIncarnation(object_ref));
        }
        let from = object.zone;
        let next_incarnation = object_ref
            .incarnation_id
            .0
            .checked_add(1)
            .ok_or(ExtendedCastZoneRuntimeError::IncarnationOverflow)?;
        let after = ObjectRef {
            object_id: object_ref.object_id,
            incarnation_id: IncarnationId(next_incarnation),
        };
        object.object_ref = after;
        object.zone = actual_destination;
        object.controller = None;
        object.visibility = Visibility::Public;
        object.cast_method = None;

        self.foretold.remove(&object_ref.object_id);
        self.plotted.remove(&object_ref.object_id);
        self.warp_exile_permissions.remove(&object_ref.object_id);

        Ok(ZoneChangeEvidence {
            before: object_ref,
            after,
            from,
            requested_destination,
            actual_destination,
            replacement_order,
            replacements_applied,
        })
    }

    fn set_stack_cast_state(
        &mut self,
        stack_object: ObjectRef,
        controller: PlayerId,
        method: CastMethod,
    ) -> Result<(), ExtendedCastZoneRuntimeError> {
        let object = self
            .objects
            .get_mut(&stack_object.object_id)
            .ok_or(ExtendedCastZoneRuntimeError::MissingObject(stack_object))?;
        if object.object_ref != stack_object {
            return Err(ExtendedCastZoneRuntimeError::StaleIncarnation(stack_object));
        }
        if object.zone != Zone::Stack {
            return Err(ExtendedCastZoneRuntimeError::WrongZone {
                expected: Zone::Stack,
                actual: object.zone,
            });
        }
        object.controller = Some(controller);
        object.visibility = Visibility::Public;
        object.cast_method = Some(method);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiscardFilter {
    AnyCard,
    Land,
}

#[derive(Debug, Clone, Copy)]
enum CostMode<'a> {
    ExactAlternative {
        mana: &'a ManaCost,
        life: u32,
        permission_digest: &'a str,
    },
    WithoutPayingManaCost,
    PrintedWithDiscard {
        filter: DiscardFilter,
    },
    PrintedOrExternallyPermitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManaRequirement {
    Color(ManaColor),
    Snow,
    Hybrid(ManaColor, ManaColor),
}

fn assign_fixed_mana(
    requirements: &[ManaRequirement],
    units: &[ManaUnit],
    offset: usize,
    used: &mut BTreeSet<usize>,
) -> bool {
    if offset == requirements.len() {
        return true;
    }
    for (index, unit) in units.iter().enumerate() {
        if used.contains(&index) || !mana_unit_matches(*unit, requirements[offset]) {
            continue;
        }
        used.insert(index);
        if assign_fixed_mana(requirements, units, offset + 1, used) {
            return true;
        }
        used.remove(&index);
    }
    false
}

fn mana_unit_matches(unit: ManaUnit, requirement: ManaRequirement) -> bool {
    match requirement {
        ManaRequirement::Color(color) => unit.color == color,
        ManaRequirement::Snow => unit.from_snow_source,
        ManaRequirement::Hybrid(first, second) => unit.color == first || unit.color == second,
    }
}

fn require_distinct<T: Ord + Copy>(
    values: impl IntoIterator<Item = T>,
    category: &'static str,
) -> Result<(), ExtendedCastZoneRuntimeError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(ExtendedCastZoneRuntimeError::DuplicateSelection(category));
        }
    }
    Ok(())
}

fn validate_external_replacement_order(
    replacements: &[ExternalZoneReplacement],
    order: &[ReplacementEffectId],
) -> Result<(), ExtendedCastZoneRuntimeError> {
    require_distinct(
        replacements.iter().map(|replacement| replacement.effect_id),
        "replacement declarations",
    )?;
    require_distinct(order.iter().copied(), "replacement order")?;
    let declared = replacements
        .iter()
        .map(|replacement| replacement.effect_id)
        .collect::<BTreeSet<_>>();
    let ordered = order.iter().copied().collect::<BTreeSet<_>>();
    if declared != ordered {
        return Err(ExtendedCastZoneRuntimeError::ReplacementOrderMismatch);
    }
    Ok(())
}

fn validate_stack_replacement_order(
    replacements: &[ExternalZoneReplacement],
    order: &[ReplacementOrderEntry],
    jump_start_required: bool,
) -> Result<(), ExtendedCastZoneRuntimeError> {
    require_distinct(
        replacements.iter().map(|replacement| replacement.effect_id),
        "replacement declarations",
    )?;
    require_distinct(order.iter().copied(), "replacement order")?;
    let declared = replacements
        .iter()
        .map(|replacement| replacement.effect_id)
        .collect::<BTreeSet<_>>();
    let ordered_external = order
        .iter()
        .filter_map(|entry| match entry {
            ReplacementOrderEntry::External(effect_id) => Some(*effect_id),
            ReplacementOrderEntry::JumpStartExile => None,
        })
        .collect::<BTreeSet<_>>();
    let jump_start_entries = order
        .iter()
        .filter(|entry| **entry == ReplacementOrderEntry::JumpStartExile)
        .count();
    if declared != ordered_external || jump_start_entries != usize::from(jump_start_required) {
        return Err(ExtendedCastZoneRuntimeError::ReplacementOrderMismatch);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtendedCastZoneRuntimeError {
    DuplicatePlayer(PlayerId),
    MissingPlayer(PlayerId),
    DuplicateObject(ObjectId),
    MissingObject(ObjectRef),
    StaleIncarnation(ObjectRef),
    IncarnationOverflow,
    WrongZone { expected: Zone, actual: Zone },
    MissingController,
    WrongOwner,
    WrongOwnerOrCharacteristics,
    WrongProgramKind,
    IllegalSpecialActionTiming,
    CastIsNotLegal,
    MissingCastPermission,
    CastPermissionNotYetUsable,
    NotCastUsingWarp,
    CopiedSpellWasNotCast,
    NotBeginningOfEndStep,
    MissingWarpTrigger(PendingTriggerId),
    WarpSemanticDigestMismatch,
    WarpIncarnationMismatch,
    TriggerIdOverflow,
    BaseManaCostChoiceDoesNotMatch,
    MissingPrintedManaCost,
    UnexpectedVariableX,
    ManaRequirementOverflow,
    WrongManaSelectionCount { expected: usize, actual: usize },
    MissingManaUnit(ManaUnitId),
    ManaPaymentDoesNotMatchCost,
    WrongLifePayment { expected: u32, actual: u32 },
    CannotPayLife,
    MissingDiscardPayment,
    DiscardDoesNotMatchCost,
    UnexpectedPaymentEvidence(&'static str),
    DuplicateSelection(&'static str),
    ReplacementOrderMismatch,
}

impl fmt::Display for ExtendedCastZoneRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ExtendedCastZoneRuntimeError {}
