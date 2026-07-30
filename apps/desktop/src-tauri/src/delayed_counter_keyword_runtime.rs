//! Exact standalone rules programs for delayed counter sacrifice keywords.
//!
//! This module owns complete Echo, Cumulative Upkeep, Vanishing, and Fading
//! clauses that are not already owned by an earlier exact keyword compiler.
//! It models entry counter replacement, upkeep trigger creation, APNAP stack
//! ordering, payment transactions, counter removal, and sacrifice as separate
//! rules events. The production adapter remains disconnected until the main
//! simulator can supply every boundary represented by the standalone runtime.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const DELAYED_COUNTER_KEYWORD_COMPILER_VERSION: &str = "delayed-counter-keyword-compiler-0.1";
pub const DELAYED_COUNTER_KEYWORD_RUNTIME_VERSION: &str = "delayed-counter-keyword-runtime-0.1";
pub const DELAYED_COUNTER_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:101.4,117.5,603.2c,603.3b,603.4,\
     603.6c,603.10,702.24,702.28,702.32,702.63,704.5,701.17,701.20,122.6";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type BindingId = u64;
pub type TriggerId = u64;
pub type TriggerBatchId = u64;
pub type UpkeepId = u64;
pub type EventId = u64;
pub type ManaUnitId = u64;
pub type CounterReplacementId = u64;

/// Clause recognition is not live execution coverage. A production adapter
/// must supply complete game objects, hidden zones, mana provenance,
/// replacement effects, trigger ordering, payment choices, and stack timing.
pub const fn delayed_counter_keyword_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DelayedCounterKeywordFamily {
    Echo,
    CumulativeUpkeep,
    Vanishing,
    Fading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManaSymbol {
    Generic(u32),
    Colored(ManaColor),
    Colorless,
    Snow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaCost {
    pub symbols: Vec<ManaSymbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PermanentKind {
    Artifact,
    Creature,
    Enchantment,
    Land,
    Planeswalker,
    Battle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CounterKind {
    Age,
    Time,
    Fade,
    PlusOnePlusOne,
    MinusOneMinusOne,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenDefinition {
    pub name: String,
    pub power: i32,
    pub toughness: i32,
    pub colors: BTreeSet<ManaColor>,
    pub kinds: BTreeSet<PermanentKind>,
    pub subtypes: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordCost {
    Mana(ManaCost),
    PayLife(u32),
    AddMana { color: ManaColor, amount: u32 },
    OpponentGainsLife(u32),
    DiscardCards(u32),
    DrawCards(u32),
    ExileTopLibrary(u32),
    FlipCoins(u32),
    GainControlOfLandYouDontControl,
    OpponentCreatesToken(TokenDefinition),
    PayManaAndLife { mana: ManaCost, life: u32 },
    PutCounterOnOpponentCreature { kind: CounterKind, amount: u32 },
    PutCounterOnSource { kind: CounterKind, amount: u32 },
    MoveCardsFromSingleGraveyardToLibraryBottom { amount: u32 },
    SacrificePermanents { kind: PermanentKind, amount: u32 },
    SpeakWithoutPauseOrFumble { exact_phrase: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelayedCounterKeywordKind {
    Echo { cost: KeywordCost },
    CumulativeUpkeep { cost_per_age_counter: KeywordCost },
    Vanishing { initial_time_counters: Option<u32> },
    Fading { initial_fade_counters: u32 },
}

impl DelayedCounterKeywordKind {
    pub const fn family(&self) -> DelayedCounterKeywordFamily {
        match self {
            Self::Echo { .. } => DelayedCounterKeywordFamily::Echo,
            Self::CumulativeUpkeep { .. } => DelayedCounterKeywordFamily::CumulativeUpkeep,
            Self::Vanishing { .. } => DelayedCounterKeywordFamily::Vanishing,
            Self::Fading { .. } => DelayedCounterKeywordFamily::Fading,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedCounterKeywordProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    kind: DelayedCounterKeywordKind,
}

impl DelayedCounterKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &DelayedCounterKeywordKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        delayed_counter_keyword_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarlierClauseOwner {
    OfficialKeywordRuntimeCumulativeUpkeep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelayedCounterClauseClassification {
    Program(DelayedCounterKeywordProgram),
    EarlierOwner {
        family: DelayedCounterKeywordFamily,
        owner: EarlierClauseOwner,
    },
    Rejected,
}

pub fn compile_delayed_counter_keyword_program(
    exact_source: &str,
    normalized_source: &str,
) -> Option<DelayedCounterKeywordProgram> {
    match classify_delayed_counter_keyword_clause(exact_source, normalized_source) {
        DelayedCounterClauseClassification::Program(program) => Some(program),
        DelayedCounterClauseClassification::EarlierOwner { .. }
        | DelayedCounterClauseClassification::Rejected => None,
    }
}

pub fn classify_delayed_counter_keyword_clause(
    exact_source: &str,
    normalized_source: &str,
) -> DelayedCounterClauseClassification {
    if !is_complete_single_line(exact_source) || !is_complete_single_line(normalized_source) {
        return DelayedCounterClauseClassification::Rejected;
    }

    let Some((core, reminder)) = split_core_and_reminder(exact_source) else {
        return DelayedCounterClauseClassification::Rejected;
    };

    let parsed = if let Some(remainder) = core.strip_prefix("Echo") {
        if remainder.starts_with(" of ") {
            return DelayedCounterClauseClassification::Rejected;
        }
        let Some(cost_text) = strip_keyword_cost_separator(remainder) else {
            return DelayedCounterClauseClassification::Rejected;
        };
        if !validate_echo_reminder(reminder) {
            return DelayedCounterClauseClassification::Rejected;
        }
        parse_echo_cost(cost_text).map(|cost| DelayedCounterKeywordKind::Echo { cost })
    } else if let Some(remainder) = core.strip_prefix("Cumulative upkeep") {
        let Some(cost_text) = strip_keyword_cost_separator(remainder) else {
            return DelayedCounterClauseClassification::Rejected;
        };
        if !validate_cumulative_upkeep_reminder(reminder, cost_text) {
            return DelayedCounterClauseClassification::Rejected;
        }
        if cumulative_upkeep_cost_owned_earlier(cost_text) {
            return DelayedCounterClauseClassification::EarlierOwner {
                family: DelayedCounterKeywordFamily::CumulativeUpkeep,
                owner: EarlierClauseOwner::OfficialKeywordRuntimeCumulativeUpkeep,
            };
        }
        parse_new_cumulative_upkeep_cost(cost_text).map(|cost_per_age_counter| {
            DelayedCounterKeywordKind::CumulativeUpkeep {
                cost_per_age_counter,
            }
        })
    } else if let Some(remainder) = core.strip_prefix("Vanishing") {
        let initial_time_counters = if remainder.is_empty() {
            Some(None)
        } else {
            remainder
                .strip_prefix(' ')
                .and_then(parse_positive_u32)
                .map(Some)
        };
        initial_time_counters
            .filter(|count| validate_vanishing_reminder(reminder, *count))
            .map(
                |initial_time_counters| DelayedCounterKeywordKind::Vanishing {
                    initial_time_counters,
                },
            )
    } else if let Some(remainder) = core.strip_prefix("Fading") {
        remainder
            .strip_prefix(' ')
            .and_then(parse_positive_u32)
            .filter(|count| validate_fading_reminder(reminder, *count))
            .map(|initial_fade_counters| DelayedCounterKeywordKind::Fading {
                initial_fade_counters,
            })
    } else {
        None
    };

    let Some(kind) = parsed else {
        return DelayedCounterClauseClassification::Rejected;
    };
    let semantic_digest = semantic_digest(exact_source, &kind);
    DelayedCounterClauseClassification::Program(DelayedCounterKeywordProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        semantic_digest,
        kind,
    })
}

fn is_complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source.trim() == source
        && !source.contains('\n')
        && !source.contains('\r')
}

fn split_core_and_reminder(source: &str) -> Option<(&str, Option<&str>)> {
    if let Some((core, reminder_body)) = source.split_once(" (") {
        if core.is_empty()
            || reminder_body.is_empty()
            || !reminder_body.ends_with(')')
            || reminder_body[..reminder_body.len() - 1].contains(" (")
        {
            return None;
        }
        Some((core, Some(reminder_body.strip_suffix(')')?)))
    } else {
        Some((source, None))
    }
}

fn strip_keyword_cost_separator(remainder: &str) -> Option<&str> {
    let cost = if let Some(cost) = remainder.strip_prefix(' ') {
        cost
    } else {
        remainder.strip_prefix(['\u{fffd}', '\u{2013}', '\u{2014}'])?
    };
    let cost = cost.strip_suffix('.').unwrap_or(cost);
    (!cost.is_empty() && cost.trim() == cost).then_some(cost)
}

fn parse_positive_u32(value: &str) -> Option<u32> {
    let value = value.parse::<u32>().ok()?;
    (value > 0).then_some(value)
}

fn validate_echo_reminder(reminder: Option<&str>) -> bool {
    let Some(reminder) = reminder else {
        return true;
    };
    reminder.eq_ignore_ascii_case(
        "At the beginning of your upkeep, if this came under your control since the \
         beginning of your last upkeep, sacrifice it unless you pay its echo cost.",
    )
}

fn validate_cumulative_upkeep_reminder(reminder: Option<&str>, cost_text: &str) -> bool {
    let Some(reminder) = reminder else {
        return true;
    };
    let lower = reminder.to_ascii_lowercase();
    let standard = (lower.starts_with(
        "at the beginning of your upkeep, put an age counter on this permanent, then \
         sacrifice it unless you pay its upkeep cost for each age counter on it.",
    ) || lower.starts_with(
        "at the beginning of your upkeep, put an age counter on this creature, then \
         sacrifice it unless you pay its upkeep cost for each age counter on it.",
    )) && (lower.ends_with("age counter on it.")
        || lower.ends_with("mana from a snow source."));
    let explicit_mana = lower
        == "at the beginning of your upkeep, put an age counter on this permanent, then \
            sacrifice it unless you pay {2} for each age counter on it.";
    let toy_boat = cost_text == "Say \"Toy Boat\" quickly"
        && [
            "at the beginning of your upkeep, put an age counter on this creature, then \
             sacrifice it unless you say \"toy boat\" once for each age counter on \
             it\u{fffd}without pausing between or fumbling it.",
            "at the beginning of your upkeep, put an age counter on this creature, then \
             sacrifice it unless you say \"toy boat\" once for each age counter on \
             it\u{2013}without pausing between or fumbling it.",
            "at the beginning of your upkeep, put an age counter on this creature, then \
             sacrifice it unless you say \"toy boat\" once for each age counter on \
             it\u{2014}without pausing between or fumbling it.",
        ]
        .contains(&lower.as_str());
    standard || explicit_mana || toy_boat
}

fn validate_vanishing_reminder(reminder: Option<&str>, count: Option<u32>) -> bool {
    let Some(reminder) = reminder else {
        return true;
    };
    let lower = reminder.to_ascii_lowercase();
    let upkeep_tail = "at the beginning of your upkeep, remove a time counter from it. \
        when the last is removed, sacrifice it.";
    match count {
        None => {
            lower
                == "at the beginning of your upkeep, remove a time counter from this creature. \
                    when the last is removed, sacrifice it."
                || lower
                    == "at the beginning of your upkeep, remove a time counter from this \
                        enchantment. when the last is removed, sacrifice it."
        }
        Some(count) => {
            let counter_phrase = if count == 1 {
                "a time counter".to_owned()
            } else {
                let Some(word) = number_word(count) else {
                    return false;
                };
                format!("{word} time counters")
            };
            let enters = [
                format!("this creature enters with {counter_phrase} on it. "),
                format!("this enchantment enters with {counter_phrase} on it. "),
                format!("this aura enters with {counter_phrase} on it. "),
                format!("this land enters the battlefield with {counter_phrase} on it. "),
            ];
            enters
                .iter()
                .any(|prefix| lower == format!("{prefix}{upkeep_tail}"))
        }
    }
}

fn validate_fading_reminder(reminder: Option<&str>, count: u32) -> bool {
    let Some(reminder) = reminder else {
        return true;
    };
    let lower = reminder.to_ascii_lowercase();
    let counter_phrase = if count == 1 {
        "one fade counter".to_owned()
    } else {
        let Some(word) = number_word(count) else {
            return false;
        };
        format!("{word} fade counters")
    };
    let tail = "at the beginning of your upkeep, remove a fade counter from it. \
        if you can't, sacrifice it.";
    [
        format!("this artifact enters with {counter_phrase} on it. "),
        format!("this creature enters with {counter_phrase} on it. "),
        format!("this enchantment enters with {counter_phrase} on it. "),
    ]
    .iter()
    .any(|prefix| lower == format!("{prefix}{tail}"))
}

fn number_word(number: u32) -> Option<&'static str> {
    match number {
        1 => Some("one"),
        2 => Some("two"),
        3 => Some("three"),
        4 => Some("four"),
        5 => Some("five"),
        6 => Some("six"),
        7 => Some("seven"),
        8 => Some("eight"),
        9 => Some("nine"),
        10 => Some("ten"),
        11 => Some("eleven"),
        12 => Some("twelve"),
        _ => None,
    }
}

fn parse_echo_cost(cost_text: &str) -> Option<KeywordCost> {
    if cost_text.starts_with('{') {
        return parse_mana_cost(cost_text).map(KeywordCost::Mana);
    }
    match cost_text {
        "Discard a card" => Some(KeywordCost::DiscardCards(1)),
        "Sacrifice two lands" => Some(KeywordCost::SacrificePermanents {
            kind: PermanentKind::Land,
            amount: 2,
        }),
        _ => None,
    }
}

fn cumulative_upkeep_cost_owned_earlier(cost_text: &str) -> bool {
    parse_mana_alternatives(cost_text).is_some()
        || cost_text
            .strip_prefix("Pay ")
            .and_then(|value| value.strip_suffix(" life"))
            .and_then(|value| value.parse::<u32>().ok())
            .is_some()
}

fn parse_new_cumulative_upkeep_cost(cost_text: &str) -> Option<KeywordCost> {
    match cost_text {
        "Add {R}" => Some(KeywordCost::AddMana {
            color: ManaColor::Red,
            amount: 1,
        }),
        "An opponent gains 1 life" => Some(KeywordCost::OpponentGainsLife(1)),
        "Discard a card" => Some(KeywordCost::DiscardCards(1)),
        "Draw a card" => Some(KeywordCost::DrawCards(1)),
        "Exile the top card of your library" => Some(KeywordCost::ExileTopLibrary(1)),
        "Flip a coin" => Some(KeywordCost::FlipCoins(1)),
        "Gain control of a land you don't control" => {
            Some(KeywordCost::GainControlOfLandYouDontControl)
        }
        "Have an opponent create a 1/1 red Survivor creature token" => {
            Some(KeywordCost::OpponentCreatesToken(TokenDefinition {
                name: "Survivor".to_owned(),
                power: 1,
                toughness: 1,
                colors: BTreeSet::from([ManaColor::Red]),
                kinds: BTreeSet::from([PermanentKind::Creature]),
                subtypes: BTreeSet::from(["Survivor".to_owned()]),
            }))
        }
        "Pay {B} and 1 life" => Some(KeywordCost::PayManaAndLife {
            mana: parse_mana_cost("{B}")?,
            life: 1,
        }),
        "Put a +1/+1 counter on a creature an opponent controls" => {
            Some(KeywordCost::PutCounterOnOpponentCreature {
                kind: CounterKind::PlusOnePlusOne,
                amount: 1,
            })
        }
        "Put a -1/-1 counter on this creature" => Some(KeywordCost::PutCounterOnSource {
            kind: CounterKind::MinusOneMinusOne,
            amount: 1,
        }),
        "Put two cards from a single graveyard on the bottom of their owner's library" => {
            Some(KeywordCost::MoveCardsFromSingleGraveyardToLibraryBottom { amount: 2 })
        }
        "Sacrifice a creature" => Some(KeywordCost::SacrificePermanents {
            kind: PermanentKind::Creature,
            amount: 1,
        }),
        "Sacrifice a land" => Some(KeywordCost::SacrificePermanents {
            kind: PermanentKind::Land,
            amount: 1,
        }),
        "Say \"Toy Boat\" quickly" => Some(KeywordCost::SpeakWithoutPauseOrFumble {
            exact_phrase: "Toy Boat".to_owned(),
        }),
        _ => None,
    }
}

fn parse_mana_alternatives(cost_text: &str) -> Option<Vec<ManaCost>> {
    let alternatives = cost_text
        .split(" or ")
        .map(parse_mana_cost)
        .collect::<Option<Vec<_>>>()?;
    (!alternatives.is_empty()).then_some(alternatives)
}

fn parse_mana_cost(cost_text: &str) -> Option<ManaCost> {
    if cost_text.is_empty() {
        return None;
    }
    let mut symbols = Vec::new();
    let mut remainder = cost_text;
    while let Some(after_open) = remainder.strip_prefix('{') {
        let end = after_open.find('}')?;
        let symbol = &after_open[..end];
        let parsed = match symbol {
            "W" => ManaSymbol::Colored(ManaColor::White),
            "U" => ManaSymbol::Colored(ManaColor::Blue),
            "B" => ManaSymbol::Colored(ManaColor::Black),
            "R" => ManaSymbol::Colored(ManaColor::Red),
            "G" => ManaSymbol::Colored(ManaColor::Green),
            "C" => ManaSymbol::Colorless,
            "S" => ManaSymbol::Snow,
            value => ManaSymbol::Generic(value.parse::<u32>().ok()?),
        };
        symbols.push(parsed);
        remainder = &after_open[end + 1..];
    }
    (!symbols.is_empty() && remainder.is_empty()).then_some(ManaCost { symbols })
}

fn semantic_digest(exact_source: &str, kind: &DelayedCounterKeywordKind) -> String {
    let mut hasher = Sha256::new();
    for component in [
        "delayed-counter-keyword-content/v1".to_owned(),
        DELAYED_COUNTER_KEYWORD_COMPILER_VERSION.to_owned(),
        DELAYED_COUNTER_KEYWORD_RUNTIME_VERSION.to_owned(),
        DELAYED_COUNTER_RULES_CONTEXT_VERSION.to_owned(),
        exact_source.to_owned(),
        semantic_contract(kind),
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn semantic_contract(kind: &DelayedCounterKeywordKind) -> String {
    match kind {
        DelayedCounterKeywordKind::Echo { cost } => format!(
            "echo/v1;control-history=since-controller-last-upkeep;instances=separate;\
             resolution=recheck-intervening-if;unless-pay={}",
            cost_contract(cost)
        ),
        DelayedCounterKeywordKind::CumulativeUpkeep {
            cost_per_age_counter,
        } => format!(
            "cumulative-upkeep/v1;instances=separate;add-age=counter-replacement;\
             payment-copies=all-age-after-placement;partial=false;unless-pay={}",
            cost_contract(cost_per_age_counter)
        ),
        DelayedCounterKeywordKind::Vanishing {
            initial_time_counters,
        } => format!(
            "vanishing/v1;entry-time={initial_time_counters:?};upkeep=intervening-if-remove-one;\
             last-removed=separate-trigger-per-instance;sacrifice=true"
        ),
        DelayedCounterKeywordKind::Fading {
            initial_fade_counters,
        } => format!(
            "fading/v1;entry-fade={initial_fade_counters};upkeep=remove-one-if-possible;\
             failure=sacrifice"
        ),
    }
}

fn cost_contract(cost: &KeywordCost) -> String {
    match cost {
        KeywordCost::Mana(cost) => format!("mana:{}", mana_cost_contract(cost)),
        KeywordCost::PayLife(amount) => format!("life:{amount}"),
        KeywordCost::AddMana { color, amount } => format!("add-mana:{color:?}:{amount}"),
        KeywordCost::OpponentGainsLife(amount) => format!("opponent-gains-life:{amount}"),
        KeywordCost::DiscardCards(amount) => format!("discard:{amount}"),
        KeywordCost::DrawCards(amount) => format!("draw:{amount}"),
        KeywordCost::ExileTopLibrary(amount) => format!("exile-library-top:{amount}"),
        KeywordCost::FlipCoins(amount) => format!("flip-coins:{amount}"),
        KeywordCost::GainControlOfLandYouDontControl => "gain-control:opponent-land:1".to_owned(),
        KeywordCost::OpponentCreatesToken(token) => format!(
            "opponent-creates-token:{}:{}/{}:{:?}:{:?}:{:?}",
            token.name, token.power, token.toughness, token.colors, token.kinds, token.subtypes
        ),
        KeywordCost::PayManaAndLife { mana, life } => {
            format!("mana-and-life:{}:{life}", mana_cost_contract(mana))
        }
        KeywordCost::PutCounterOnOpponentCreature { kind, amount } => {
            format!("put-counter:opponent-creature:{kind:?}:{amount}")
        }
        KeywordCost::PutCounterOnSource { kind, amount } => {
            format!("put-counter:source:{kind:?}:{amount}")
        }
        KeywordCost::MoveCardsFromSingleGraveyardToLibraryBottom { amount } => {
            format!("graveyard-to-owner-library-bottom:{amount}")
        }
        KeywordCost::SacrificePermanents { kind, amount } => {
            format!("sacrifice:{kind:?}:{amount}")
        }
        KeywordCost::SpeakWithoutPauseOrFumble { exact_phrase } => {
            format!("speak-without-pause-or-fumble:{exact_phrase}")
        }
    }
}

fn mana_cost_contract(cost: &ManaCost) -> String {
    cost.symbols
        .iter()
        .map(|symbol| format!("{symbol:?}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Zone {
    Battlefield,
    Command,
    Exile,
    Graveyard,
    Hand,
    Library,
    Stack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedObject {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub controller: Option<PlayerId>,
    pub zone: Zone,
    pub kinds: BTreeSet<PermanentKind>,
    pub counters: BTreeMap<CounterKind, u32>,
    pub token: bool,
    pub token_definition: Option<TokenDefinition>,
    control_acquired_event: Option<EventId>,
}

impl TrackedObject {
    pub fn card(
        object_ref: ObjectRef,
        owner: PlayerId,
        zone: Zone,
        kinds: BTreeSet<PermanentKind>,
    ) -> Self {
        Self {
            object_ref,
            owner,
            controller: matches!(zone, Zone::Stack).then_some(owner),
            zone,
            kinds,
            counters: BTreeMap::new(),
            token: false,
            token_definition: None,
            control_acquired_event: None,
        }
    }

    pub fn permanent(
        object_ref: ObjectRef,
        owner: PlayerId,
        controller: PlayerId,
        kinds: BTreeSet<PermanentKind>,
    ) -> Self {
        Self {
            object_ref,
            owner,
            controller: Some(controller),
            zone: Zone::Battlefield,
            kinds,
            counters: BTreeMap::new(),
            token: false,
            token_definition: None,
            control_acquired_event: Some(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaUnit {
    pub id: ManaUnitId,
    pub color: Option<ManaColor>,
    pub is_colorless: bool,
    pub from_snow_source: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub life: i64,
    pub hand: Vec<ObjectRef>,
    /// Index zero is the top of the library. The final index is the bottom.
    pub library: Vec<ObjectRef>,
    pub graveyard: Vec<ObjectRef>,
    pub exile: Vec<ObjectRef>,
    pub mana_pool: BTreeMap<ManaUnitId, ManaUnit>,
}

impl PlayerState {
    pub fn new(life: i64) -> Self {
        Self {
            life,
            hand: Vec::new(),
            library: Vec::new(),
            graveyard: Vec::new(),
            exile: Vec::new(),
            mana_pool: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledProgram {
    binding_id: BindingId,
    source: ObjectRef,
    program: DelayedCounterKeywordProgram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CounterChangeDirection {
    Place,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterReplacementScope {
    pub object: Option<ObjectRef>,
    pub counter: Option<CounterKind>,
    pub direction: Option<CounterChangeDirection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterReplacementOperation {
    Multiply(u32),
    Add(u32),
    Prevent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterReplacementEffect {
    pub id: CounterReplacementId,
    pub scope: CounterReplacementScope,
    pub operation: CounterReplacementOperation,
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterReplacementChoice {
    pub effect_id: CounterReplacementId,
    pub apply: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterReplacementEvidence {
    pub chooser: PlayerId,
    pub applicable_effects_complete: bool,
    pub applicable_effect_ids: Vec<CounterReplacementId>,
    pub ordered_choices: Vec<CounterReplacementChoice>,
}

impl CounterReplacementEvidence {
    pub fn none(chooser: PlayerId) -> Self {
        Self {
            chooser,
            applicable_effects_complete: true,
            applicable_effect_ids: Vec::new(),
            ordered_choices: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterChangeEvidence {
    pub event_id: EventId,
    pub object: ObjectRef,
    pub counter: CounterKind,
    pub direction: CounterChangeDirection,
    pub requested: u32,
    pub actual: u32,
    pub before: u32,
    pub after: u32,
    pub applied_replacements: Vec<CounterReplacementId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneMoveEvidence {
    pub old_object: ObjectRef,
    pub new_object: ObjectRef,
    pub from: Zone,
    pub to: Zone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryCounterEvidence {
    pub binding_id: BindingId,
    pub counter_change: CounterChangeEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattlefieldEntryEvidence {
    pub object: ObjectRef,
    pub controller: PlayerId,
    pub placements: Vec<EntryCounterEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TriggerKind {
    Echo,
    CumulativeUpkeep,
    VanishingRemoveTimeCounter,
    VanishingLastTimeCounterRemoved,
    FadingRemoveFadeCounterOrSacrifice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTrigger {
    pub id: TriggerId,
    pub kind: TriggerKind,
    pub source: ObjectRef,
    pub binding_id: BindingId,
    pub source_semantic_digest: String,
    pub controller: PlayerId,
    pub event_id: EventId,
    pub previous_controller_upkeep_event: Option<EventId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTriggerBatch {
    pub id: TriggerBatchId,
    pub event_id: EventId,
    pub active_player: PlayerId,
    pub turn_order: Vec<PlayerId>,
    pub triggers: Vec<PendingTrigger>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerOrderEvidence {
    pub active_player: PlayerId,
    pub turn_order: Vec<PlayerId>,
    /// Each controller lists their triggers in bottom to top stack order.
    pub per_controller_bottom_to_top: BTreeMap<PlayerId, Vec<TriggerId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPaymentEvidence {
    pub mana_units: Vec<ManaUnitId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinResult {
    Heads,
    Tails,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechObservation {
    pub exact_phrase: String,
    pub paused_between_repetitions: bool,
    pub fumbled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostPaymentEvidence {
    Mana(ManaPaymentEvidence),
    PayLife {
        amount: u32,
    },
    AddMana {
        generated_unit_ids: Vec<ManaUnitId>,
    },
    OpponentGainsLife {
        opponent: PlayerId,
        amount: u32,
    },
    Discard {
        cards: Vec<ObjectRef>,
    },
    Draw {
        cards_in_order: Vec<ObjectRef>,
    },
    ExileTopLibrary {
        cards_in_order: Vec<ObjectRef>,
    },
    FlipCoins {
        results: Vec<CoinResult>,
    },
    GainControl {
        land: ObjectRef,
    },
    OpponentCreatesToken {
        opponent: PlayerId,
        token: ObjectRef,
    },
    PayManaAndLife {
        mana: ManaPaymentEvidence,
        life: u32,
    },
    PutCounter {
        target: ObjectRef,
        replacement: CounterReplacementEvidence,
    },
    MoveGraveyardCardsToLibraryBottom {
        graveyard_owner: PlayerId,
        cards_in_bottom_order: Vec<ObjectRef>,
    },
    Sacrifice {
        permanents: Vec<ObjectRef>,
    },
    Speak {
        repetitions: Vec<SpeechObservation>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerResolutionDecision {
    Echo {
        payment: Option<CostPaymentEvidence>,
    },
    CumulativeUpkeep {
        age_counter_replacement: CounterReplacementEvidence,
        payments: Option<Vec<CostPaymentEvidence>>,
    },
    Vanishing {
        time_counter_replacement: CounterReplacementEvidence,
    },
    Fading {
        fade_counter_replacement: CounterReplacementEvidence,
    },
    VanishingLastCounterRemoved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SacrificeOutcome {
    Sacrificed(ZoneMoveEvidence),
    SourceNoLongerPermanent,
    NotControlledByTriggerController,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionEvidence {
    EchoConditionFailed {
        trigger: TriggerId,
    },
    EchoPaid {
        trigger: TriggerId,
        payment: CostPaymentEvidence,
    },
    EchoUnpaid {
        trigger: TriggerId,
        sacrifice: SacrificeOutcome,
    },
    CumulativeSourceMissing {
        trigger: TriggerId,
    },
    CumulativeAgeCounterPlaced {
        trigger: TriggerId,
        counter: CounterChangeEvidence,
        total_age_counters: u32,
    },
    CumulativePaid {
        trigger: TriggerId,
        copies: u32,
        payments: Vec<CostPaymentEvidence>,
    },
    CumulativeUnpaid {
        trigger: TriggerId,
        sacrifice: SacrificeOutcome,
    },
    VanishingConditionFailed {
        trigger: TriggerId,
    },
    VanishingTimeCounterRemoved {
        trigger: TriggerId,
        counter: CounterChangeEvidence,
        generated_last_counter_triggers: Vec<TriggerId>,
    },
    FadingCounterRemoved {
        trigger: TriggerId,
        counter: CounterChangeEvidence,
    },
    FadingCouldNotRemove {
        trigger: TriggerId,
        counter: CounterChangeEvidence,
        sacrifice: SacrificeOutcome,
    },
    VanishingSacrificeResolved {
        trigger: TriggerId,
        sacrifice: SacrificeOutcome,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelayedCounterRuntimeError {
    DuplicatePlayer(PlayerId),
    MissingPlayer(PlayerId),
    DuplicateObject(ObjectRef),
    MissingObject(ObjectId),
    StaleObject {
        expected: ObjectRef,
        actual: ObjectRef,
    },
    InvalidObjectZone,
    MissingController(ObjectRef),
    DuplicateBinding(BindingId),
    MissingBinding(BindingId),
    DuplicateReplacement(CounterReplacementId),
    IncompleteReplacementCensus,
    WrongReplacementChooser {
        expected: PlayerId,
        actual: PlayerId,
    },
    ReplacementCandidateMismatch,
    ReplacementChoiceMismatch,
    MandatoryReplacementDeclined(CounterReplacementId),
    CounterOverflow,
    InvalidTurnOrder,
    DuplicateTriggerBatch(TriggerBatchId),
    MissingTriggerBatch(TriggerBatchId),
    TriggerOrderMismatch,
    EmptyStack,
    ResolutionDecisionMismatch,
    CostEvidenceMismatch,
    InvalidCostSelection,
    InsufficientLife,
    InsufficientMana,
    DuplicateManaUnit(ManaUnitId),
    MissingManaUnit(ManaUnitId),
    ZoneCollectionMismatch,
    ObjectIdAlreadyExists(ObjectId),
    TokenIdentityMustBeFresh,
    CounterTargetIllegal,
    SpeechCostFailed,
}

impl fmt::Display for DelayedCounterRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DelayedCounterRuntimeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedCounterKeywordRuntime {
    players: BTreeMap<PlayerId, PlayerState>,
    objects: BTreeMap<ObjectId, TrackedObject>,
    programs: BTreeMap<BindingId, InstalledProgram>,
    counter_replacements: BTreeMap<CounterReplacementId, CounterReplacementEffect>,
    trigger_stack: Vec<PendingTrigger>,
    pending_batches: BTreeMap<TriggerBatchId, PendingTriggerBatch>,
    waiting_triggers: Vec<PendingTrigger>,
    trigger_ledger: BTreeSet<(BindingId, TriggerKind, EventId)>,
    last_upkeep_event: BTreeMap<PlayerId, EventId>,
    current_active_player: Option<PlayerId>,
    current_turn_order: Vec<PlayerId>,
    next_binding_id: BindingId,
    next_trigger_id: TriggerId,
    next_batch_id: TriggerBatchId,
    next_upkeep_id: UpkeepId,
    next_event_id: EventId,
}

impl Default for DelayedCounterKeywordRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl DelayedCounterKeywordRuntime {
    pub fn new() -> Self {
        Self {
            players: BTreeMap::new(),
            objects: BTreeMap::new(),
            programs: BTreeMap::new(),
            counter_replacements: BTreeMap::new(),
            trigger_stack: Vec::new(),
            pending_batches: BTreeMap::new(),
            waiting_triggers: Vec::new(),
            trigger_ledger: BTreeSet::new(),
            last_upkeep_event: BTreeMap::new(),
            current_active_player: None,
            current_turn_order: Vec::new(),
            next_binding_id: 1,
            next_trigger_id: 1,
            next_batch_id: 1,
            next_upkeep_id: 1,
            next_event_id: 1,
        }
    }

    pub fn insert_player(
        &mut self,
        player: PlayerId,
        state: PlayerState,
    ) -> Result<(), DelayedCounterRuntimeError> {
        if self.players.insert(player, state).is_some() {
            return Err(DelayedCounterRuntimeError::DuplicatePlayer(player));
        }
        Ok(())
    }

    pub fn player(&self, player: PlayerId) -> Result<&PlayerState, DelayedCounterRuntimeError> {
        self.players
            .get(&player)
            .ok_or(DelayedCounterRuntimeError::MissingPlayer(player))
    }

    pub fn player_mut(
        &mut self,
        player: PlayerId,
    ) -> Result<&mut PlayerState, DelayedCounterRuntimeError> {
        self.players
            .get_mut(&player)
            .ok_or(DelayedCounterRuntimeError::MissingPlayer(player))
    }

    pub fn object(&self, object: ObjectId) -> Result<&TrackedObject, DelayedCounterRuntimeError> {
        self.objects
            .get(&object)
            .ok_or(DelayedCounterRuntimeError::MissingObject(object))
    }

    pub fn insert_object(
        &mut self,
        mut object: TrackedObject,
    ) -> Result<(), DelayedCounterRuntimeError> {
        if self.objects.contains_key(&object.object_ref.object_id) {
            return Err(DelayedCounterRuntimeError::DuplicateObject(
                object.object_ref,
            ));
        }
        self.player(object.owner)?;
        match object.zone {
            Zone::Battlefield => {
                let controller =
                    object
                        .controller
                        .ok_or(DelayedCounterRuntimeError::MissingController(
                            object.object_ref,
                        ))?;
                self.player(controller)?;
                object.control_acquired_event = Some(self.next_event_id.saturating_sub(1));
            }
            Zone::Stack => {
                let controller =
                    object
                        .controller
                        .ok_or(DelayedCounterRuntimeError::MissingController(
                            object.object_ref,
                        ))?;
                self.player(controller)?;
            }
            Zone::Hand | Zone::Library | Zone::Graveyard | Zone::Exile | Zone::Command => {
                if object.controller.is_some() {
                    return Err(DelayedCounterRuntimeError::InvalidObjectZone);
                }
            }
        }
        self.add_to_zone_collection(object.object_ref, object.owner, object.zone)?;
        self.objects.insert(object.object_ref.object_id, object);
        Ok(())
    }

    pub fn install_program(
        &mut self,
        source: ObjectRef,
        program: DelayedCounterKeywordProgram,
    ) -> Result<BindingId, DelayedCounterRuntimeError> {
        self.require_current(source)?;
        let binding_id = self.next_binding_id;
        self.next_binding_id = self
            .next_binding_id
            .checked_add(1)
            .ok_or(DelayedCounterRuntimeError::CounterOverflow)?;
        let installed = InstalledProgram {
            binding_id,
            source,
            program,
        };
        if self.programs.insert(binding_id, installed).is_some() {
            return Err(DelayedCounterRuntimeError::DuplicateBinding(binding_id));
        }
        Ok(binding_id)
    }

    pub fn install_counter_replacement(
        &mut self,
        effect: CounterReplacementEffect,
    ) -> Result<(), DelayedCounterRuntimeError> {
        if effect.id == 0 || matches!(effect.operation, CounterReplacementOperation::Multiply(0)) {
            return Err(DelayedCounterRuntimeError::InvalidCostSelection);
        }
        let id = effect.id;
        if self.counter_replacements.insert(id, effect).is_some() {
            return Err(DelayedCounterRuntimeError::DuplicateReplacement(id));
        }
        Ok(())
    }

    pub fn enter_battlefield(
        &mut self,
        object: ObjectRef,
        controller: PlayerId,
        replacement_by_binding: BTreeMap<BindingId, CounterReplacementEvidence>,
    ) -> Result<BattlefieldEntryEvidence, DelayedCounterRuntimeError> {
        self.player(controller)?;
        let tracked = self.require_current(object)?.clone();
        if tracked.zone == Zone::Battlefield {
            return Err(DelayedCounterRuntimeError::InvalidObjectZone);
        }

        let placements = self
            .programs
            .values()
            .filter(|installed| installed.source == object)
            .filter_map(|installed| match installed.program.kind() {
                DelayedCounterKeywordKind::Vanishing {
                    initial_time_counters: Some(amount),
                } => Some((installed.binding_id, CounterKind::Time, *amount)),
                DelayedCounterKeywordKind::Fading {
                    initial_fade_counters,
                } => Some((
                    installed.binding_id,
                    CounterKind::Fade,
                    *initial_fade_counters,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected_bindings = placements
            .iter()
            .map(|(binding, _, _)| *binding)
            .collect::<BTreeSet<_>>();
        if replacement_by_binding
            .keys()
            .copied()
            .collect::<BTreeSet<_>>()
            != expected_bindings
        {
            return Err(DelayedCounterRuntimeError::ReplacementCandidateMismatch);
        }

        let mut candidate = self.clone();
        let (owner, old_zone) = {
            let tracked = candidate.require_current(object)?;
            (tracked.owner, tracked.zone)
        };
        candidate.remove_from_zone_collection(object, owner, old_zone)?;
        let entry_event = candidate.bump_event()?;
        {
            let tracked = candidate.require_current_mut(object)?;
            tracked.zone = Zone::Battlefield;
            tracked.controller = Some(controller);
            tracked.control_acquired_event = Some(entry_event);
        }

        let mut placement_evidence = Vec::new();
        for (binding_id, counter, amount) in placements {
            let replacement = replacement_by_binding
                .get(&binding_id)
                .ok_or(DelayedCounterRuntimeError::ReplacementCandidateMismatch)?;
            let change = candidate.apply_counter_change(
                object,
                counter,
                CounterChangeDirection::Place,
                amount,
                replacement,
            )?;
            placement_evidence.push(EntryCounterEvidence {
                binding_id,
                counter_change: change,
            });
        }
        *self = candidate;
        Ok(BattlefieldEntryEvidence {
            object,
            controller,
            placements: placement_evidence,
        })
    }

    pub fn change_control(
        &mut self,
        object: ObjectRef,
        new_controller: PlayerId,
    ) -> Result<EventId, DelayedCounterRuntimeError> {
        self.player(new_controller)?;
        let event_id = self.bump_event()?;
        let tracked = self.require_current_mut(object)?;
        if tracked.zone != Zone::Battlefield {
            return Err(DelayedCounterRuntimeError::InvalidObjectZone);
        }
        tracked.controller = Some(new_controller);
        tracked.control_acquired_event = Some(event_id);
        Ok(event_id)
    }

    pub fn begin_upkeep(
        &mut self,
        active_player: PlayerId,
        turn_order: Vec<PlayerId>,
    ) -> Result<PendingTriggerBatch, DelayedCounterRuntimeError> {
        self.validate_turn_order(active_player, &turn_order)?;
        let event_id = self.bump_event()?;
        let _upkeep_id = self.next_upkeep_id;
        self.next_upkeep_id = self
            .next_upkeep_id
            .checked_add(1)
            .ok_or(DelayedCounterRuntimeError::CounterOverflow)?;
        let previous_upkeep = self.last_upkeep_event.get(&active_player).copied();

        let installed = self
            .programs
            .values()
            .filter(|program| {
                self.objects
                    .get(&program.source.object_id)
                    .is_some_and(|object| {
                        object.object_ref == program.source
                            && object.zone == Zone::Battlefield
                            && object.controller == Some(active_player)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut triggers = Vec::new();
        for program in installed {
            let kind = match program.program.kind() {
                DelayedCounterKeywordKind::Echo { .. }
                    if self.echo_condition(program.source, active_player, previous_upkeep)? =>
                {
                    Some(TriggerKind::Echo)
                }
                DelayedCounterKeywordKind::Echo { .. } => None,
                DelayedCounterKeywordKind::CumulativeUpkeep { .. } => {
                    Some(TriggerKind::CumulativeUpkeep)
                }
                DelayedCounterKeywordKind::Vanishing { .. }
                    if self.counter_amount(program.source, CounterKind::Time)? > 0 =>
                {
                    Some(TriggerKind::VanishingRemoveTimeCounter)
                }
                DelayedCounterKeywordKind::Vanishing { .. } => None,
                DelayedCounterKeywordKind::Fading { .. } => {
                    Some(TriggerKind::FadingRemoveFadeCounterOrSacrifice)
                }
            };
            if let Some(kind) = kind
                && self
                    .trigger_ledger
                    .insert((program.binding_id, kind, event_id))
            {
                triggers.push(self.new_trigger(
                    &program,
                    kind,
                    active_player,
                    event_id,
                    previous_upkeep,
                )?);
            }
        }
        self.last_upkeep_event.insert(active_player, event_id);
        self.current_active_player = Some(active_player);
        self.current_turn_order = turn_order.clone();
        self.create_trigger_batch(event_id, active_player, turn_order, triggers)
    }

    pub fn commit_trigger_batch(
        &mut self,
        batch_id: TriggerBatchId,
        evidence: TriggerOrderEvidence,
    ) -> Result<Vec<TriggerId>, DelayedCounterRuntimeError> {
        let batch = self
            .pending_batches
            .get(&batch_id)
            .cloned()
            .ok_or(DelayedCounterRuntimeError::MissingTriggerBatch(batch_id))?;
        if evidence.active_player != batch.active_player || evidence.turn_order != batch.turn_order
        {
            return Err(DelayedCounterRuntimeError::TriggerOrderMismatch);
        }

        let expected = batch
            .triggers
            .iter()
            .map(|trigger| trigger.id)
            .collect::<BTreeSet<_>>();
        let supplied = evidence
            .per_controller_bottom_to_top
            .values()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        if supplied.iter().copied().collect::<BTreeSet<_>>() != expected
            || supplied.len() != expected.len()
        {
            return Err(DelayedCounterRuntimeError::TriggerOrderMismatch);
        }
        let by_id = batch
            .triggers
            .iter()
            .cloned()
            .map(|trigger| (trigger.id, trigger))
            .collect::<BTreeMap<_, _>>();
        if evidence
            .per_controller_bottom_to_top
            .keys()
            .any(|controller| !batch.turn_order.contains(controller))
        {
            return Err(DelayedCounterRuntimeError::TriggerOrderMismatch);
        }
        for controller in &batch.turn_order {
            let expected_for_controller = batch
                .triggers
                .iter()
                .filter(|trigger| trigger.controller == *controller)
                .map(|trigger| trigger.id)
                .collect::<BTreeSet<_>>();
            let supplied_for_controller = evidence
                .per_controller_bottom_to_top
                .get(controller)
                .cloned()
                .unwrap_or_default();
            if supplied_for_controller
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                != expected_for_controller
                || supplied_for_controller.len() != expected_for_controller.len()
            {
                return Err(DelayedCounterRuntimeError::TriggerOrderMismatch);
            }
        }

        let mut pushed = Vec::new();
        for controller in &batch.turn_order {
            let supplied_for_controller = evidence
                .per_controller_bottom_to_top
                .get(controller)
                .cloned()
                .unwrap_or_default();
            for trigger_id in supplied_for_controller {
                self.trigger_stack.push(
                    by_id
                        .get(&trigger_id)
                        .cloned()
                        .ok_or(DelayedCounterRuntimeError::TriggerOrderMismatch)?,
                );
                pushed.push(trigger_id);
            }
        }
        self.pending_batches.remove(&batch_id);
        Ok(pushed)
    }

    pub fn flush_waiting_triggers(
        &mut self,
    ) -> Result<Option<PendingTriggerBatch>, DelayedCounterRuntimeError> {
        if self.waiting_triggers.is_empty() {
            return Ok(None);
        }
        let active_player = self
            .current_active_player
            .ok_or(DelayedCounterRuntimeError::InvalidTurnOrder)?;
        let turn_order = self.current_turn_order.clone();
        self.validate_turn_order(active_player, &turn_order)?;
        let event_id = self
            .waiting_triggers
            .first()
            .map(|trigger| trigger.event_id)
            .ok_or(DelayedCounterRuntimeError::EmptyStack)?;
        let triggers = std::mem::take(&mut self.waiting_triggers);
        self.create_trigger_batch(event_id, active_player, turn_order, triggers)
            .map(Some)
    }

    pub fn stack(&self) -> &[PendingTrigger] {
        &self.trigger_stack
    }

    pub fn waiting_triggers(&self) -> &[PendingTrigger] {
        &self.waiting_triggers
    }

    pub fn resolve_top_trigger(
        &mut self,
        decision: TriggerResolutionDecision,
    ) -> Result<Vec<ResolutionEvidence>, DelayedCounterRuntimeError> {
        let mut candidate = self.clone();
        let trigger = candidate
            .trigger_stack
            .pop()
            .ok_or(DelayedCounterRuntimeError::EmptyStack)?;
        let evidence = candidate.resolve_trigger(trigger, decision)?;
        *self = candidate;
        Ok(evidence)
    }

    fn resolve_trigger(
        &mut self,
        trigger: PendingTrigger,
        decision: TriggerResolutionDecision,
    ) -> Result<Vec<ResolutionEvidence>, DelayedCounterRuntimeError> {
        let installed = self.programs.get(&trigger.binding_id).cloned().ok_or(
            DelayedCounterRuntimeError::MissingBinding(trigger.binding_id),
        )?;
        if installed.source != trigger.source
            || installed.program.semantic_digest() != trigger.source_semantic_digest
        {
            return Err(DelayedCounterRuntimeError::MissingBinding(
                trigger.binding_id,
            ));
        }
        match (trigger.kind, installed.program.kind(), decision) {
            (
                TriggerKind::Echo,
                DelayedCounterKeywordKind::Echo { cost },
                TriggerResolutionDecision::Echo { payment },
            ) => self.resolve_echo(trigger, cost, payment),
            (
                TriggerKind::CumulativeUpkeep,
                DelayedCounterKeywordKind::CumulativeUpkeep {
                    cost_per_age_counter,
                },
                TriggerResolutionDecision::CumulativeUpkeep {
                    age_counter_replacement,
                    payments,
                },
            ) => self.resolve_cumulative_upkeep(
                trigger,
                cost_per_age_counter,
                age_counter_replacement,
                payments,
            ),
            (
                TriggerKind::VanishingRemoveTimeCounter,
                DelayedCounterKeywordKind::Vanishing { .. },
                TriggerResolutionDecision::Vanishing {
                    time_counter_replacement,
                },
            ) => self.resolve_vanishing_remove(trigger, time_counter_replacement),
            (
                TriggerKind::FadingRemoveFadeCounterOrSacrifice,
                DelayedCounterKeywordKind::Fading { .. },
                TriggerResolutionDecision::Fading {
                    fade_counter_replacement,
                },
            ) => self.resolve_fading(trigger, fade_counter_replacement),
            (
                TriggerKind::VanishingLastTimeCounterRemoved,
                DelayedCounterKeywordKind::Vanishing { .. },
                TriggerResolutionDecision::VanishingLastCounterRemoved,
            ) => {
                let sacrifice =
                    self.sacrifice_source_if_controlled(trigger.source, trigger.controller)?;
                Ok(vec![ResolutionEvidence::VanishingSacrificeResolved {
                    trigger: trigger.id,
                    sacrifice,
                }])
            }
            _ => Err(DelayedCounterRuntimeError::ResolutionDecisionMismatch),
        }
    }

    fn resolve_echo(
        &mut self,
        trigger: PendingTrigger,
        cost: &KeywordCost,
        payment: Option<CostPaymentEvidence>,
    ) -> Result<Vec<ResolutionEvidence>, DelayedCounterRuntimeError> {
        if !self
            .objects
            .get(&trigger.source.object_id)
            .is_some_and(|object| {
                object.object_ref == trigger.source
                    && object.zone == Zone::Battlefield
                    && object.controller == Some(trigger.controller)
            })
            || !self.echo_condition(
                trigger.source,
                trigger.controller,
                trigger.previous_controller_upkeep_event,
            )?
        {
            return Ok(vec![ResolutionEvidence::EchoConditionFailed {
                trigger: trigger.id,
            }]);
        }
        if let Some(payment) = payment {
            self.pay_cost(trigger.controller, trigger.source, cost, &payment)?;
            Ok(vec![ResolutionEvidence::EchoPaid {
                trigger: trigger.id,
                payment,
            }])
        } else {
            let sacrifice =
                self.sacrifice_source_if_controlled(trigger.source, trigger.controller)?;
            Ok(vec![ResolutionEvidence::EchoUnpaid {
                trigger: trigger.id,
                sacrifice,
            }])
        }
    }

    fn resolve_cumulative_upkeep(
        &mut self,
        trigger: PendingTrigger,
        cost: &KeywordCost,
        replacement: CounterReplacementEvidence,
        payments: Option<Vec<CostPaymentEvidence>>,
    ) -> Result<Vec<ResolutionEvidence>, DelayedCounterRuntimeError> {
        if !self
            .objects
            .get(&trigger.source.object_id)
            .is_some_and(|object| {
                object.object_ref == trigger.source && object.zone == Zone::Battlefield
            })
        {
            return Ok(vec![ResolutionEvidence::CumulativeSourceMissing {
                trigger: trigger.id,
            }]);
        }
        let counter = self.apply_counter_change(
            trigger.source,
            CounterKind::Age,
            CounterChangeDirection::Place,
            1,
            &replacement,
        )?;
        let total_age_counters = counter.after;
        let mut evidence = vec![ResolutionEvidence::CumulativeAgeCounterPlaced {
            trigger: trigger.id,
            counter,
            total_age_counters,
        }];
        if let Some(payments) = payments {
            if matches!(cost, KeywordCost::SpeakWithoutPauseOrFumble { .. }) {
                if payments.len() != 1 {
                    return Err(DelayedCounterRuntimeError::InvalidCostSelection);
                }
                let CostPaymentEvidence::Speak { repetitions } = &payments[0] else {
                    return Err(DelayedCounterRuntimeError::CostEvidenceMismatch);
                };
                if repetitions.len()
                    != usize::try_from(total_age_counters)
                        .map_err(|_| DelayedCounterRuntimeError::CounterOverflow)?
                {
                    return Err(DelayedCounterRuntimeError::InvalidCostSelection);
                }
                self.pay_cost(trigger.controller, trigger.source, cost, &payments[0])?;
            } else {
                if payments.len()
                    != usize::try_from(total_age_counters)
                        .map_err(|_| DelayedCounterRuntimeError::CounterOverflow)?
                {
                    return Err(DelayedCounterRuntimeError::InvalidCostSelection);
                }
                for payment in &payments {
                    self.pay_cost(trigger.controller, trigger.source, cost, payment)?;
                }
            }
            evidence.push(ResolutionEvidence::CumulativePaid {
                trigger: trigger.id,
                copies: total_age_counters,
                payments,
            });
        } else {
            let sacrifice =
                self.sacrifice_source_if_controlled(trigger.source, trigger.controller)?;
            evidence.push(ResolutionEvidence::CumulativeUnpaid {
                trigger: trigger.id,
                sacrifice,
            });
        }
        Ok(evidence)
    }

    fn resolve_vanishing_remove(
        &mut self,
        trigger: PendingTrigger,
        replacement: CounterReplacementEvidence,
    ) -> Result<Vec<ResolutionEvidence>, DelayedCounterRuntimeError> {
        if !self
            .objects
            .get(&trigger.source.object_id)
            .is_some_and(|object| {
                object.object_ref == trigger.source
                    && object.zone == Zone::Battlefield
                    && object
                        .counters
                        .get(&CounterKind::Time)
                        .copied()
                        .unwrap_or(0)
                        > 0
            })
        {
            return Ok(vec![ResolutionEvidence::VanishingConditionFailed {
                trigger: trigger.id,
            }]);
        }
        let counter = self.apply_counter_change(
            trigger.source,
            CounterKind::Time,
            CounterChangeDirection::Remove,
            1,
            &replacement,
        )?;
        let mut generated = Vec::new();
        if counter.before > 0 && counter.actual > 0 && counter.after == 0 {
            let controller = self.require_current(trigger.source)?.controller.ok_or(
                DelayedCounterRuntimeError::MissingController(trigger.source),
            )?;
            let programs = self
                .programs
                .values()
                .filter(|installed| {
                    installed.source == trigger.source
                        && matches!(
                            installed.program.kind(),
                            DelayedCounterKeywordKind::Vanishing { .. }
                        )
                })
                .cloned()
                .collect::<Vec<_>>();
            for program in programs {
                if self.trigger_ledger.insert((
                    program.binding_id,
                    TriggerKind::VanishingLastTimeCounterRemoved,
                    counter.event_id,
                )) {
                    let pending = self.new_trigger(
                        &program,
                        TriggerKind::VanishingLastTimeCounterRemoved,
                        controller,
                        counter.event_id,
                        None,
                    )?;
                    generated.push(pending.id);
                    self.waiting_triggers.push(pending);
                }
            }
        }
        Ok(vec![ResolutionEvidence::VanishingTimeCounterRemoved {
            trigger: trigger.id,
            counter,
            generated_last_counter_triggers: generated,
        }])
    }

    fn resolve_fading(
        &mut self,
        trigger: PendingTrigger,
        replacement: CounterReplacementEvidence,
    ) -> Result<Vec<ResolutionEvidence>, DelayedCounterRuntimeError> {
        if !self
            .objects
            .get(&trigger.source.object_id)
            .is_some_and(|object| {
                object.object_ref == trigger.source && object.zone == Zone::Battlefield
            })
        {
            let empty = CounterChangeEvidence {
                event_id: self.bump_event()?,
                object: trigger.source,
                counter: CounterKind::Fade,
                direction: CounterChangeDirection::Remove,
                requested: 1,
                actual: 0,
                before: 0,
                after: 0,
                applied_replacements: Vec::new(),
            };
            return Ok(vec![ResolutionEvidence::FadingCouldNotRemove {
                trigger: trigger.id,
                counter: empty,
                sacrifice: SacrificeOutcome::SourceNoLongerPermanent,
            }]);
        }
        let counter = self.apply_counter_change(
            trigger.source,
            CounterKind::Fade,
            CounterChangeDirection::Remove,
            1,
            &replacement,
        )?;
        if counter.actual > 0 {
            Ok(vec![ResolutionEvidence::FadingCounterRemoved {
                trigger: trigger.id,
                counter,
            }])
        } else {
            let sacrifice =
                self.sacrifice_source_if_controlled(trigger.source, trigger.controller)?;
            Ok(vec![ResolutionEvidence::FadingCouldNotRemove {
                trigger: trigger.id,
                counter,
                sacrifice,
            }])
        }
    }

    fn create_trigger_batch(
        &mut self,
        event_id: EventId,
        active_player: PlayerId,
        turn_order: Vec<PlayerId>,
        triggers: Vec<PendingTrigger>,
    ) -> Result<PendingTriggerBatch, DelayedCounterRuntimeError> {
        let id = self.next_batch_id;
        self.next_batch_id = self
            .next_batch_id
            .checked_add(1)
            .ok_or(DelayedCounterRuntimeError::CounterOverflow)?;
        let batch = PendingTriggerBatch {
            id,
            event_id,
            active_player,
            turn_order,
            triggers,
        };
        if self.pending_batches.insert(id, batch.clone()).is_some() {
            return Err(DelayedCounterRuntimeError::DuplicateTriggerBatch(id));
        }
        Ok(batch)
    }

    fn new_trigger(
        &mut self,
        installed: &InstalledProgram,
        kind: TriggerKind,
        controller: PlayerId,
        event_id: EventId,
        previous_controller_upkeep_event: Option<EventId>,
    ) -> Result<PendingTrigger, DelayedCounterRuntimeError> {
        let id = self.next_trigger_id;
        self.next_trigger_id = self
            .next_trigger_id
            .checked_add(1)
            .ok_or(DelayedCounterRuntimeError::CounterOverflow)?;
        Ok(PendingTrigger {
            id,
            kind,
            source: installed.source,
            binding_id: installed.binding_id,
            source_semantic_digest: installed.program.semantic_digest().to_owned(),
            controller,
            event_id,
            previous_controller_upkeep_event,
        })
    }

    fn validate_turn_order(
        &self,
        active_player: PlayerId,
        turn_order: &[PlayerId],
    ) -> Result<(), DelayedCounterRuntimeError> {
        if turn_order.first().copied() != Some(active_player)
            || turn_order.len() != self.players.len()
            || turn_order.iter().copied().collect::<BTreeSet<_>>()
                != self.players.keys().copied().collect::<BTreeSet<_>>()
        {
            return Err(DelayedCounterRuntimeError::InvalidTurnOrder);
        }
        Ok(())
    }

    fn echo_condition(
        &self,
        source: ObjectRef,
        controller: PlayerId,
        previous_upkeep: Option<EventId>,
    ) -> Result<bool, DelayedCounterRuntimeError> {
        let object = self.require_current(source)?;
        if object.zone != Zone::Battlefield || object.controller != Some(controller) {
            return Ok(false);
        }
        Ok(match previous_upkeep {
            None => true,
            Some(previous) => object
                .control_acquired_event
                .is_some_and(|acquired| acquired >= previous),
        })
    }

    fn counter_amount(
        &self,
        object: ObjectRef,
        counter: CounterKind,
    ) -> Result<u32, DelayedCounterRuntimeError> {
        Ok(self
            .require_current(object)?
            .counters
            .get(&counter)
            .copied()
            .unwrap_or(0))
    }

    fn require_current(
        &self,
        object: ObjectRef,
    ) -> Result<&TrackedObject, DelayedCounterRuntimeError> {
        let current = self
            .objects
            .get(&object.object_id)
            .ok_or(DelayedCounterRuntimeError::MissingObject(object.object_id))?;
        if current.object_ref != object {
            return Err(DelayedCounterRuntimeError::StaleObject {
                expected: object,
                actual: current.object_ref,
            });
        }
        Ok(current)
    }

    fn require_current_mut(
        &mut self,
        object: ObjectRef,
    ) -> Result<&mut TrackedObject, DelayedCounterRuntimeError> {
        let current = self
            .objects
            .get_mut(&object.object_id)
            .ok_or(DelayedCounterRuntimeError::MissingObject(object.object_id))?;
        if current.object_ref != object {
            return Err(DelayedCounterRuntimeError::StaleObject {
                expected: object,
                actual: current.object_ref,
            });
        }
        Ok(current)
    }

    fn bump_event(&mut self) -> Result<EventId, DelayedCounterRuntimeError> {
        let id = self.next_event_id;
        self.next_event_id = self
            .next_event_id
            .checked_add(1)
            .ok_or(DelayedCounterRuntimeError::CounterOverflow)?;
        Ok(id)
    }

    fn apply_counter_change(
        &mut self,
        object: ObjectRef,
        counter: CounterKind,
        direction: CounterChangeDirection,
        requested: u32,
        evidence: &CounterReplacementEvidence,
    ) -> Result<CounterChangeEvidence, DelayedCounterRuntimeError> {
        let tracked = self.require_current(object)?;
        let chooser = tracked.controller.unwrap_or(tracked.owner);
        if evidence.chooser != chooser {
            return Err(DelayedCounterRuntimeError::WrongReplacementChooser {
                expected: chooser,
                actual: evidence.chooser,
            });
        }
        if !evidence.applicable_effects_complete {
            return Err(DelayedCounterRuntimeError::IncompleteReplacementCensus);
        }

        let applicable = self
            .counter_replacements
            .values()
            .filter(|effect| {
                effect
                    .scope
                    .object
                    .is_none_or(|candidate| candidate == object)
                    && effect
                        .scope
                        .counter
                        .is_none_or(|candidate| candidate == counter)
                    && effect
                        .scope
                        .direction
                        .is_none_or(|candidate| candidate == direction)
            })
            .map(|effect| effect.id)
            .collect::<BTreeSet<_>>();
        if evidence
            .applicable_effect_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            != applicable
            || evidence.applicable_effect_ids.len() != applicable.len()
        {
            return Err(DelayedCounterRuntimeError::ReplacementCandidateMismatch);
        }
        if evidence
            .ordered_choices
            .iter()
            .map(|choice| choice.effect_id)
            .collect::<BTreeSet<_>>()
            != applicable
            || evidence.ordered_choices.len() != applicable.len()
        {
            return Err(DelayedCounterRuntimeError::ReplacementChoiceMismatch);
        }

        let mut amount = requested;
        let mut applied_replacements = Vec::new();
        let mut prevented = false;
        for choice in &evidence.ordered_choices {
            let effect = self
                .counter_replacements
                .get(&choice.effect_id)
                .ok_or(DelayedCounterRuntimeError::ReplacementChoiceMismatch)?;
            if !effect.optional && !choice.apply {
                return Err(DelayedCounterRuntimeError::MandatoryReplacementDeclined(
                    effect.id,
                ));
            }
            if prevented || !choice.apply {
                continue;
            }
            applied_replacements.push(effect.id);
            match effect.operation {
                CounterReplacementOperation::Multiply(multiplier) => {
                    amount = amount
                        .checked_mul(multiplier)
                        .ok_or(DelayedCounterRuntimeError::CounterOverflow)?;
                }
                CounterReplacementOperation::Add(additional) => {
                    amount = amount
                        .checked_add(additional)
                        .ok_or(DelayedCounterRuntimeError::CounterOverflow)?;
                }
                CounterReplacementOperation::Prevent => {
                    amount = 0;
                    prevented = true;
                }
            }
        }

        let before = self
            .require_current(object)?
            .counters
            .get(&counter)
            .copied()
            .unwrap_or(0);
        let (actual, after) = match direction {
            CounterChangeDirection::Place => {
                let after = before
                    .checked_add(amount)
                    .ok_or(DelayedCounterRuntimeError::CounterOverflow)?;
                (amount, after)
            }
            CounterChangeDirection::Remove => {
                let actual = amount.min(before);
                (actual, before - actual)
            }
        };
        if after == 0 {
            self.require_current_mut(object)?.counters.remove(&counter);
        } else {
            self.require_current_mut(object)?
                .counters
                .insert(counter, after);
        }
        let event_id = self.bump_event()?;
        Ok(CounterChangeEvidence {
            event_id,
            object,
            counter,
            direction,
            requested,
            actual,
            before,
            after,
            applied_replacements,
        })
    }

    fn pay_cost(
        &mut self,
        payer: PlayerId,
        source: ObjectRef,
        cost: &KeywordCost,
        evidence: &CostPaymentEvidence,
    ) -> Result<(), DelayedCounterRuntimeError> {
        let mut candidate = self.clone();
        candidate.pay_cost_in_place(payer, source, cost, evidence)?;
        *self = candidate;
        Ok(())
    }

    fn pay_cost_in_place(
        &mut self,
        payer: PlayerId,
        source: ObjectRef,
        cost: &KeywordCost,
        evidence: &CostPaymentEvidence,
    ) -> Result<(), DelayedCounterRuntimeError> {
        self.player(payer)?;
        match (cost, evidence) {
            (KeywordCost::Mana(cost), CostPaymentEvidence::Mana(payment)) => {
                self.pay_mana(payer, cost, payment)
            }
            (KeywordCost::PayLife(amount), CostPaymentEvidence::PayLife { amount: paid })
                if amount == paid =>
            {
                self.pay_life(payer, *amount)
            }
            (
                KeywordCost::AddMana { color, amount },
                CostPaymentEvidence::AddMana { generated_unit_ids },
            ) if generated_unit_ids.len()
                == usize::try_from(*amount)
                    .map_err(|_| DelayedCounterRuntimeError::CounterOverflow)? =>
            {
                for id in generated_unit_ids {
                    if self
                        .players
                        .values()
                        .any(|player| player.mana_pool.contains_key(id))
                    {
                        return Err(DelayedCounterRuntimeError::DuplicateManaUnit(*id));
                    }
                    self.player_mut(payer)?.mana_pool.insert(
                        *id,
                        ManaUnit {
                            id: *id,
                            color: Some(*color),
                            is_colorless: false,
                            from_snow_source: false,
                        },
                    );
                }
                Ok(())
            }
            (
                KeywordCost::OpponentGainsLife(amount),
                CostPaymentEvidence::OpponentGainsLife {
                    opponent,
                    amount: gained,
                },
            ) if amount == gained && *opponent != payer => {
                let player = self.player_mut(*opponent)?;
                player.life = player
                    .life
                    .checked_add(i64::from(*amount))
                    .ok_or(DelayedCounterRuntimeError::CounterOverflow)?;
                Ok(())
            }
            (KeywordCost::DiscardCards(amount), CostPaymentEvidence::Discard { cards })
                if cards.len()
                    == usize::try_from(*amount)
                        .map_err(|_| DelayedCounterRuntimeError::CounterOverflow)? =>
            {
                self.discard_cards(payer, cards)
            }
            (KeywordCost::DrawCards(amount), CostPaymentEvidence::Draw { cards_in_order })
                if cards_in_order.len()
                    == usize::try_from(*amount)
                        .map_err(|_| DelayedCounterRuntimeError::CounterOverflow)? =>
            {
                self.draw_cards(payer, cards_in_order)
            }
            (
                KeywordCost::ExileTopLibrary(amount),
                CostPaymentEvidence::ExileTopLibrary { cards_in_order },
            ) if cards_in_order.len()
                == usize::try_from(*amount)
                    .map_err(|_| DelayedCounterRuntimeError::CounterOverflow)? =>
            {
                self.exile_top_library(payer, cards_in_order)
            }
            (KeywordCost::FlipCoins(amount), CostPaymentEvidence::FlipCoins { results })
                if results.len()
                    == usize::try_from(*amount)
                        .map_err(|_| DelayedCounterRuntimeError::CounterOverflow)? =>
            {
                Ok(())
            }
            (
                KeywordCost::GainControlOfLandYouDontControl,
                CostPaymentEvidence::GainControl { land },
            ) => {
                let object = self.require_current(*land)?;
                if object.zone != Zone::Battlefield
                    || !object.kinds.contains(&PermanentKind::Land)
                    || object.controller == Some(payer)
                {
                    return Err(DelayedCounterRuntimeError::InvalidCostSelection);
                }
                self.change_control(*land, payer)?;
                Ok(())
            }
            (
                KeywordCost::OpponentCreatesToken(definition),
                CostPaymentEvidence::OpponentCreatesToken { opponent, token },
            ) if *opponent != payer => self.create_token(*opponent, *token, definition),
            (
                KeywordCost::PayManaAndLife { mana, life },
                CostPaymentEvidence::PayManaAndLife {
                    mana: payment,
                    life: paid_life,
                },
            ) if life == paid_life => {
                self.pay_mana(payer, mana, payment)?;
                self.pay_life(payer, *life)
            }
            (
                KeywordCost::PutCounterOnOpponentCreature { kind, amount },
                CostPaymentEvidence::PutCounter {
                    target,
                    replacement,
                },
            ) => {
                let object = self.require_current(*target)?;
                if object.zone != Zone::Battlefield
                    || !object.kinds.contains(&PermanentKind::Creature)
                    || object.controller == Some(payer)
                {
                    return Err(DelayedCounterRuntimeError::CounterTargetIllegal);
                }
                self.apply_counter_change(
                    *target,
                    *kind,
                    CounterChangeDirection::Place,
                    *amount,
                    replacement,
                )?;
                Ok(())
            }
            (
                KeywordCost::PutCounterOnSource { kind, amount },
                CostPaymentEvidence::PutCounter {
                    target,
                    replacement,
                },
            ) if *target == source => {
                let object = self.require_current(source)?;
                if object.zone != Zone::Battlefield
                    || !object.kinds.contains(&PermanentKind::Creature)
                {
                    return Err(DelayedCounterRuntimeError::CounterTargetIllegal);
                }
                self.apply_counter_change(
                    source,
                    *kind,
                    CounterChangeDirection::Place,
                    *amount,
                    replacement,
                )?;
                Ok(())
            }
            (
                KeywordCost::MoveCardsFromSingleGraveyardToLibraryBottom { amount },
                CostPaymentEvidence::MoveGraveyardCardsToLibraryBottom {
                    graveyard_owner,
                    cards_in_bottom_order,
                },
            ) if cards_in_bottom_order.len()
                == usize::try_from(*amount)
                    .map_err(|_| DelayedCounterRuntimeError::CounterOverflow)? =>
            {
                self.move_graveyard_cards_to_bottom(*graveyard_owner, cards_in_bottom_order)
            }
            (
                KeywordCost::SacrificePermanents { kind, amount },
                CostPaymentEvidence::Sacrifice { permanents },
            ) if permanents.len()
                == usize::try_from(*amount)
                    .map_err(|_| DelayedCounterRuntimeError::CounterOverflow)? =>
            {
                self.sacrifice_cost_permanents(payer, *kind, permanents)
            }
            (
                KeywordCost::SpeakWithoutPauseOrFumble { exact_phrase },
                CostPaymentEvidence::Speak { repetitions },
            ) if !repetitions.is_empty() => {
                if repetitions.iter().any(|observation| {
                    observation.exact_phrase != *exact_phrase
                        || observation.paused_between_repetitions
                        || observation.fumbled
                }) {
                    return Err(DelayedCounterRuntimeError::SpeechCostFailed);
                }
                Ok(())
            }
            _ => Err(DelayedCounterRuntimeError::CostEvidenceMismatch),
        }
    }

    fn pay_life(&mut self, payer: PlayerId, amount: u32) -> Result<(), DelayedCounterRuntimeError> {
        let amount = i64::from(amount);
        let player = self.player_mut(payer)?;
        if player.life < amount {
            return Err(DelayedCounterRuntimeError::InsufficientLife);
        }
        player.life -= amount;
        Ok(())
    }

    fn pay_mana(
        &mut self,
        payer: PlayerId,
        cost: &ManaCost,
        evidence: &ManaPaymentEvidence,
    ) -> Result<(), DelayedCounterRuntimeError> {
        let generic = cost
            .symbols
            .iter()
            .filter_map(|symbol| match symbol {
                ManaSymbol::Generic(amount) => Some(*amount),
                _ => None,
            })
            .try_fold(0_u32, |total, amount| total.checked_add(amount))
            .ok_or(DelayedCounterRuntimeError::CounterOverflow)?;
        let strict = cost
            .symbols
            .iter()
            .filter(|symbol| !matches!(symbol, ManaSymbol::Generic(_)))
            .copied()
            .collect::<Vec<_>>();
        let expected = strict
            .len()
            .checked_add(
                usize::try_from(generic)
                    .map_err(|_| DelayedCounterRuntimeError::CounterOverflow)?,
            )
            .ok_or(DelayedCounterRuntimeError::CounterOverflow)?;
        if evidence.mana_units.len() != expected
            || evidence
                .mana_units
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != expected
        {
            return Err(DelayedCounterRuntimeError::InsufficientMana);
        }
        let pool = &self.player(payer)?.mana_pool;
        let units = evidence
            .mana_units
            .iter()
            .map(|id| {
                pool.get(id)
                    .cloned()
                    .ok_or(DelayedCounterRuntimeError::MissingManaUnit(*id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !mana_units_can_satisfy(&strict, &units) {
            return Err(DelayedCounterRuntimeError::InsufficientMana);
        }
        let player = self.player_mut(payer)?;
        for id in &evidence.mana_units {
            player.mana_pool.remove(id);
        }
        Ok(())
    }

    fn discard_cards(
        &mut self,
        payer: PlayerId,
        cards: &[ObjectRef],
    ) -> Result<(), DelayedCounterRuntimeError> {
        if cards.iter().copied().collect::<BTreeSet<_>>().len() != cards.len() {
            return Err(DelayedCounterRuntimeError::InvalidCostSelection);
        }
        for card in cards {
            let object = self.require_current(*card)?;
            if object.owner != payer || object.zone != Zone::Hand {
                return Err(DelayedCounterRuntimeError::InvalidCostSelection);
            }
        }
        for card in cards {
            self.move_object(*card, Zone::Graveyard, None)?;
        }
        Ok(())
    }

    fn draw_cards(
        &mut self,
        payer: PlayerId,
        cards: &[ObjectRef],
    ) -> Result<(), DelayedCounterRuntimeError> {
        if self.player(payer)?.library.get(..cards.len()) != Some(cards) {
            return Err(DelayedCounterRuntimeError::InvalidCostSelection);
        }
        for card in cards {
            self.move_object(*card, Zone::Hand, None)?;
        }
        Ok(())
    }

    fn exile_top_library(
        &mut self,
        payer: PlayerId,
        cards: &[ObjectRef],
    ) -> Result<(), DelayedCounterRuntimeError> {
        if self.player(payer)?.library.get(..cards.len()) != Some(cards) {
            return Err(DelayedCounterRuntimeError::InvalidCostSelection);
        }
        for card in cards {
            self.move_object(*card, Zone::Exile, None)?;
        }
        Ok(())
    }

    fn create_token(
        &mut self,
        controller: PlayerId,
        token: ObjectRef,
        definition: &TokenDefinition,
    ) -> Result<(), DelayedCounterRuntimeError> {
        self.player(controller)?;
        if token.incarnation_id == 0 || self.objects.contains_key(&token.object_id) {
            return Err(DelayedCounterRuntimeError::TokenIdentityMustBeFresh);
        }
        let event_id = self.bump_event()?;
        self.objects.insert(
            token.object_id,
            TrackedObject {
                object_ref: token,
                owner: controller,
                controller: Some(controller),
                zone: Zone::Battlefield,
                kinds: definition.kinds.clone(),
                counters: BTreeMap::new(),
                token: true,
                token_definition: Some(definition.clone()),
                control_acquired_event: Some(event_id),
            },
        );
        Ok(())
    }

    fn move_graveyard_cards_to_bottom(
        &mut self,
        graveyard_owner: PlayerId,
        cards: &[ObjectRef],
    ) -> Result<(), DelayedCounterRuntimeError> {
        self.player(graveyard_owner)?;
        if cards.iter().copied().collect::<BTreeSet<_>>().len() != cards.len() {
            return Err(DelayedCounterRuntimeError::InvalidCostSelection);
        }
        for card in cards {
            let object = self.require_current(*card)?;
            if object.owner != graveyard_owner || object.zone != Zone::Graveyard {
                return Err(DelayedCounterRuntimeError::InvalidCostSelection);
            }
        }
        for card in cards {
            self.move_object(*card, Zone::Library, None)?;
        }
        Ok(())
    }

    fn sacrifice_cost_permanents(
        &mut self,
        payer: PlayerId,
        kind: PermanentKind,
        permanents: &[ObjectRef],
    ) -> Result<(), DelayedCounterRuntimeError> {
        if permanents.iter().copied().collect::<BTreeSet<_>>().len() != permanents.len() {
            return Err(DelayedCounterRuntimeError::InvalidCostSelection);
        }
        for permanent in permanents {
            let object = self.require_current(*permanent)?;
            if object.zone != Zone::Battlefield
                || object.controller != Some(payer)
                || !object.kinds.contains(&kind)
            {
                return Err(DelayedCounterRuntimeError::InvalidCostSelection);
            }
        }
        for permanent in permanents {
            self.move_object(*permanent, Zone::Graveyard, None)?;
        }
        Ok(())
    }

    fn sacrifice_source_if_controlled(
        &mut self,
        source: ObjectRef,
        trigger_controller: PlayerId,
    ) -> Result<SacrificeOutcome, DelayedCounterRuntimeError> {
        let Some(object) = self.objects.get(&source.object_id) else {
            return Ok(SacrificeOutcome::SourceNoLongerPermanent);
        };
        if object.object_ref != source || object.zone != Zone::Battlefield {
            return Ok(SacrificeOutcome::SourceNoLongerPermanent);
        }
        if object.controller != Some(trigger_controller) {
            return Ok(SacrificeOutcome::NotControlledByTriggerController);
        }
        self.move_object(source, Zone::Graveyard, None)
            .map(SacrificeOutcome::Sacrificed)
    }

    fn move_object(
        &mut self,
        old_object: ObjectRef,
        destination: Zone,
        destination_controller: Option<PlayerId>,
    ) -> Result<ZoneMoveEvidence, DelayedCounterRuntimeError> {
        let old = self.require_current(old_object)?.clone();
        if destination == Zone::Battlefield {
            return Err(DelayedCounterRuntimeError::InvalidObjectZone);
        }
        if let Some(controller) = destination_controller {
            self.player(controller)?;
            if destination != Zone::Stack {
                return Err(DelayedCounterRuntimeError::InvalidObjectZone);
            }
        }
        self.remove_from_zone_collection(old.object_ref, old.owner, old.zone)?;
        let new_object = ObjectRef {
            object_id: old.object_ref.object_id,
            incarnation_id: old
                .object_ref
                .incarnation_id
                .checked_add(1)
                .ok_or(DelayedCounterRuntimeError::CounterOverflow)?,
        };
        let mut moved = old.clone();
        moved.object_ref = new_object;
        moved.zone = destination;
        moved.controller = destination_controller;
        moved.counters.clear();
        if moved.token {
            moved.token_definition = old.token_definition.clone();
        }
        moved.control_acquired_event = None;
        self.add_to_zone_collection(new_object, moved.owner, destination)?;
        self.objects.insert(new_object.object_id, moved);
        Ok(ZoneMoveEvidence {
            old_object,
            new_object,
            from: old.zone,
            to: destination,
        })
    }

    fn add_to_zone_collection(
        &mut self,
        object: ObjectRef,
        owner: PlayerId,
        zone: Zone,
    ) -> Result<(), DelayedCounterRuntimeError> {
        let player = self.player_mut(owner)?;
        let collection = match zone {
            Zone::Hand => Some(&mut player.hand),
            Zone::Library => Some(&mut player.library),
            Zone::Graveyard => Some(&mut player.graveyard),
            Zone::Exile => Some(&mut player.exile),
            Zone::Battlefield | Zone::Command | Zone::Stack => None,
        };
        if let Some(collection) = collection {
            if collection.contains(&object) {
                return Err(DelayedCounterRuntimeError::ZoneCollectionMismatch);
            }
            collection.push(object);
        }
        Ok(())
    }

    fn remove_from_zone_collection(
        &mut self,
        object: ObjectRef,
        owner: PlayerId,
        zone: Zone,
    ) -> Result<(), DelayedCounterRuntimeError> {
        let player = self.player_mut(owner)?;
        let collection = match zone {
            Zone::Hand => Some(&mut player.hand),
            Zone::Library => Some(&mut player.library),
            Zone::Graveyard => Some(&mut player.graveyard),
            Zone::Exile => Some(&mut player.exile),
            Zone::Battlefield | Zone::Command | Zone::Stack => None,
        };
        if let Some(collection) = collection {
            let index = collection
                .iter()
                .position(|candidate| *candidate == object)
                .ok_or(DelayedCounterRuntimeError::ZoneCollectionMismatch)?;
            collection.remove(index);
        }
        Ok(())
    }
}

fn mana_units_can_satisfy(strict: &[ManaSymbol], units: &[ManaUnit]) -> bool {
    fn search(
        index: usize,
        strict: &[ManaSymbol],
        units: &[ManaUnit],
        used: &mut BTreeSet<usize>,
    ) -> bool {
        if index == strict.len() {
            return true;
        }
        for (unit_index, unit) in units.iter().enumerate() {
            if used.contains(&unit_index) || !mana_unit_matches(strict[index], unit) {
                continue;
            }
            used.insert(unit_index);
            if search(index + 1, strict, units, used) {
                return true;
            }
            used.remove(&unit_index);
        }
        false
    }
    search(0, strict, units, &mut BTreeSet::new())
}

fn mana_unit_matches(symbol: ManaSymbol, unit: &ManaUnit) -> bool {
    match symbol {
        ManaSymbol::Colored(color) => unit.color == Some(color),
        ManaSymbol::Colorless => unit.is_colorless,
        ManaSymbol::Snow => unit.from_snow_source,
        ManaSymbol::Generic(_) => true,
    }
}
