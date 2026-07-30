//! Content keyed deck construction and pregame Oracle programs.
//!
//! This module covers complete clauses whose rules operate before normal turn
//! sequencing: explicit self commander permission, named card copy exceptions,
//! opening-hand zone changes, and opening-hand reveals that create delayed
//! effects. The compiler accepts only complete reviewed clauses. The standalone
//! transaction and delayed-effect boundaries deliberately have no production
//! adapter, so recognizing one of these programs is not live simulation
//! coverage.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const PREGAME_CLAUSE_COMPILER_VERSION: &str = "pregame-clause-compiler-0.1";
pub const PREGAME_CLAUSE_RUNTIME_VERSION: &str = "pregame-clause-runtime-0.1";
pub const PREGAME_RULES_CONTEXT_VERSION: &str =
    "magic-comprehensive-rules-2026-06-19:100.2,103.2-8,400.7,903.3,903.5";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type DelayedEffectId = u64;
pub type DueEffectId = u64;

const COMMANDER_PERMISSION_NORMALIZED: &str = "this object can be your commander.";
const STANDARD_OPENING_BATTLEFIELD_PREFIX: &str =
    "if this object is in your opening hand, you may begin the game with ";
const STANDARD_OPENING_BATTLEFIELD_SUFFIX: &str = " on the battlefield.";
const NONSTARTER_LUCK_ENTRY_NORMALIZED: &str = "if this object is in your opening hand and you're not the starting player, \
     you may begin the game with this object on the battlefield with a luck \
     counter on it. if you do, exile a card from your hand.";
const NONFIRST_PLAYER_LUCK_ENTRY_NORMALIZED: &str = "if this object is in your opening hand and you're not playing first, you \
     may begin the game with it on the battlefield with a luck counter on it. \
     if you do, exile a card from your hand.";
const OPENING_GRAVEYARD_NORMALIZED: &str =
    "you may begin the game with this object in your graveyard. if you do, you lose 1 life.";
const OPENING_REVEAL_PREFIX: &str =
    "you may reveal this object from your opening hand. if you do, ";

/// The main simulation does not yet provide the complete pregame choice,
/// hidden-information, zone-change, and delayed-trigger adapter this module
/// requires.
pub const fn pregame_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PregameClauseKind {
    ExplicitSelfCommanderPermission,
    DeckCopyLimit(DeckCopyLimit),
    OpeningHand(OpeningHandProgram),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PregameClauseProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    kind: PregameClauseKind,
}

impl PregameClauseProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &PregameClauseKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        pregame_production_adapter_connected()
    }

    pub fn explicitly_allows_source_as_commander(&self) -> bool {
        matches!(
            self.kind,
            PregameClauseKind::ExplicitSelfCommanderPermission
        )
    }

    pub fn permits_named_card_count(&self, exact_card_name: &str, copies: u32) -> Option<bool> {
        let PregameClauseKind::DeckCopyLimit(limit) = &self.kind else {
            return None;
        };
        Some(limit.permits(exact_card_name, copies))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CopyMaximum {
    Unlimited,
    AtMost(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckCopyLimit {
    pub exact_card_name: String,
    pub maximum: CopyMaximum,
}

impl DeckCopyLimit {
    pub fn permits(&self, exact_card_name: &str, copies: u32) -> bool {
        if exact_card_name != self.exact_card_name {
            return false;
        }
        match self.maximum {
            CopyMaximum::Unlimited => true,
            CopyMaximum::AtMost(maximum) => copies <= maximum,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpeningHandProgram {
    BeginOnBattlefield(BeginOnBattlefieldProgram),
    BeginInGraveyard(BeginInGraveyardProgram),
    RevealAndSchedule(RevealOpeningHandProgram),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StartingSeatRequirement {
    Any,
    NotStartingPlayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PregameCounterKind {
    Luck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PregameCounterPlacement {
    pub kind: PregameCounterKind,
    pub amount: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PostEntryInstruction {
    ExileOneCardFromYourHandIfPossible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeginOnBattlefieldProgram {
    pub starting_seat_requirement: StartingSeatRequirement,
    pub counters: Vec<PregameCounterPlacement>,
    pub post_entry_instructions: Vec<PostEntryInstruction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginInGraveyardProgram {
    pub life_loss: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FirstUpkeep {
    FirstUpkeepOfGame,
    ControllersFirstUpkeep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RevealSchedule {
    BeginningOfFirstUpkeep(FirstUpkeep),
    BeginningOfControllersFirstMainPhase,
    EachOpponentsFirstSpell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PregameManaKind {
    Green,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PregameTokenDefinition {
    pub name: String,
    pub power: i32,
    pub toughness: i32,
    pub colors: BTreeSet<String>,
    pub card_types: BTreeSet<String>,
    pub subtypes: BTreeSet<String>,
    pub keywords: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevealEffect {
    CreateToken {
        token: PregameTokenDefinition,
        amount: u32,
    },
    EachOpponentLosesLifeThenControllerGainsTotalLost {
        amount_each: i64,
    },
    EachOpponentMills {
        amount_each: u32,
    },
    ShuffleRevealedSourceIntoLibraryThenDraw {
        amount: u32,
    },
    SetControllersLifeTotal {
        total: i64,
    },
    AddMana {
        kind: PregameManaKind,
        amount: u32,
    },
    LookAtTopKeepOneOnTopExileRest {
        amount: u32,
        keep_up_to: u32,
    },
    Scry {
        amount: u32,
    },
    CounterEachOpponentsFirstSpellUnlessTheyPayGeneric {
        generic_mana: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealOpeningHandProgram {
    pub schedule: RevealSchedule,
    pub effect: RevealEffect,
}

pub fn compile_pregame_clause_program(
    exact_source: &str,
    normalized_source: &str,
) -> Option<PregameClauseProgram> {
    if !is_complete_single_line(exact_source) || !is_complete_single_line(normalized_source) {
        return None;
    }

    let normalized_lower = normalized_source.to_ascii_lowercase();
    let kind = if normalized_lower == COMMANDER_PERMISSION_NORMALIZED {
        PregameClauseKind::ExplicitSelfCommanderPermission
    } else if let Some(copy_limit) = parse_deck_copy_limit(exact_source) {
        PregameClauseKind::DeckCopyLimit(copy_limit)
    } else {
        let opening_hand = parse_opening_hand_program(&normalized_lower)?;
        PregameClauseKind::OpeningHand(opening_hand)
    };

    let semantic_digest = pregame_semantic_digest(exact_source, &kind);
    Some(PregameClauseProgram {
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

fn parse_deck_copy_limit(exact_source: &str) -> Option<DeckCopyLimit> {
    const UNLIMITED_PREFIX: &str = "A deck can have any number of cards named ";
    const UP_TO_PREFIX: &str = "A deck can have up to ";
    const NAMED_SEPARATOR: &str = " cards named ";

    if let Some(name) = exact_source
        .strip_prefix(UNLIMITED_PREFIX)
        .and_then(|remainder| remainder.strip_suffix('.'))
        .filter(|name| valid_exact_card_name(name))
    {
        return Some(DeckCopyLimit {
            exact_card_name: name.to_owned(),
            maximum: CopyMaximum::Unlimited,
        });
    }

    let remainder = exact_source.strip_prefix(UP_TO_PREFIX)?;
    let (amount, name) = remainder.split_once(NAMED_SEPARATOR)?;
    let name = name.strip_suffix('.')?;
    let maximum = match amount {
        "seven" => 7,
        "nine" => 9,
        _ => return None,
    };
    valid_exact_card_name(name).then(|| DeckCopyLimit {
        exact_card_name: name.to_owned(),
        maximum: CopyMaximum::AtMost(maximum),
    })
}

fn valid_exact_card_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && !name.contains('\n')
        && !name.contains('\r')
        && !name.ends_with('.')
}

fn parse_opening_hand_program(normalized_lower: &str) -> Option<OpeningHandProgram> {
    if normalized_lower == NONSTARTER_LUCK_ENTRY_NORMALIZED
        || normalized_lower == NONFIRST_PLAYER_LUCK_ENTRY_NORMALIZED
    {
        return Some(OpeningHandProgram::BeginOnBattlefield(
            BeginOnBattlefieldProgram {
                starting_seat_requirement: StartingSeatRequirement::NotStartingPlayer,
                counters: vec![PregameCounterPlacement {
                    kind: PregameCounterKind::Luck,
                    amount: 1,
                }],
                post_entry_instructions: vec![
                    PostEntryInstruction::ExileOneCardFromYourHandIfPossible,
                ],
            },
        ));
    }

    if let Some(subject) = normalized_lower
        .strip_prefix(STANDARD_OPENING_BATTLEFIELD_PREFIX)
        .and_then(|remainder| remainder.strip_suffix(STANDARD_OPENING_BATTLEFIELD_SUFFIX))
        && matches!(subject, "it" | "this object" | "him" | "her" | "them")
    {
        return Some(OpeningHandProgram::BeginOnBattlefield(
            BeginOnBattlefieldProgram {
                starting_seat_requirement: StartingSeatRequirement::Any,
                counters: Vec::new(),
                post_entry_instructions: Vec::new(),
            },
        ));
    }

    if normalized_lower == OPENING_GRAVEYARD_NORMALIZED {
        return Some(OpeningHandProgram::BeginInGraveyard(
            BeginInGraveyardProgram { life_loss: 1 },
        ));
    }

    let effect_text = normalized_lower.strip_prefix(OPENING_REVEAL_PREFIX)?;
    let reveal = parse_reveal_effect(effect_text)?;
    Some(OpeningHandProgram::RevealAndSchedule(reveal))
}

fn parse_reveal_effect(effect_text: &str) -> Option<RevealOpeningHandProgram> {
    let goblin = PregameTokenDefinition {
        name: "Phyrexian Goblin".to_owned(),
        power: 1,
        toughness: 1,
        colors: BTreeSet::from(["red".to_owned()]),
        card_types: BTreeSet::from(["creature".to_owned()]),
        subtypes: BTreeSet::from(["Goblin".to_owned(), "Phyrexian".to_owned()]),
        keywords: BTreeSet::from(["haste".to_owned()]),
    };
    let (schedule, effect) = match effect_text {
        "at the beginning of the first upkeep, create a 1/1 red phyrexian goblin \
         creature token with haste." => (
            RevealSchedule::BeginningOfFirstUpkeep(FirstUpkeep::FirstUpkeepOfGame),
            RevealEffect::CreateToken {
                token: goblin,
                amount: 1,
            },
        ),
        "at the beginning of the first upkeep, each opponent loses 3 life, then \
         you gain life equal to the life lost this way." => (
            RevealSchedule::BeginningOfFirstUpkeep(FirstUpkeep::FirstUpkeepOfGame),
            RevealEffect::EachOpponentLosesLifeThenControllerGainsTotalLost { amount_each: 3 },
        ),
        "at the beginning of the first upkeep, each opponent mills seven cards." => (
            RevealSchedule::BeginningOfFirstUpkeep(FirstUpkeep::FirstUpkeepOfGame),
            RevealEffect::EachOpponentMills { amount_each: 7 },
        ),
        "at the beginning of the first upkeep, shuffle it into your library and \
         draw a card." => (
            RevealSchedule::BeginningOfFirstUpkeep(FirstUpkeep::FirstUpkeepOfGame),
            RevealEffect::ShuffleRevealedSourceIntoLibraryThenDraw { amount: 1 },
        ),
        "at the beginning of the first upkeep, your life total becomes 26." => (
            RevealSchedule::BeginningOfFirstUpkeep(FirstUpkeep::FirstUpkeepOfGame),
            RevealEffect::SetControllersLifeTotal { total: 26 },
        ),
        "at the beginning of your first main phase of the game, add {g}." => (
            RevealSchedule::BeginningOfControllersFirstMainPhase,
            RevealEffect::AddMana {
                kind: PregameManaKind::Green,
                amount: 1,
            },
        ),
        "at the beginning of your first upkeep, look at the top four cards of \
         your library. you may put one of those cards back on top of your \
         library. exile the rest." => (
            RevealSchedule::BeginningOfFirstUpkeep(FirstUpkeep::ControllersFirstUpkeep),
            RevealEffect::LookAtTopKeepOneOnTopExileRest {
                amount: 4,
                keep_up_to: 1,
            },
        ),
        "scry 3 at the beginning of your first upkeep." => (
            RevealSchedule::BeginningOfFirstUpkeep(FirstUpkeep::ControllersFirstUpkeep),
            RevealEffect::Scry { amount: 3 },
        ),
        "when each opponent casts their first spell of the game, counter that \
         spell unless that player pays {1}." => (
            RevealSchedule::EachOpponentsFirstSpell,
            RevealEffect::CounterEachOpponentsFirstSpellUnlessTheyPayGeneric { generic_mana: 1 },
        ),
        _ => return None,
    };
    Some(RevealOpeningHandProgram { schedule, effect })
}

fn pregame_semantic_digest(exact_source: &str, kind: &PregameClauseKind) -> String {
    let mut hasher = Sha256::new();
    for component in [
        "pregame-clause-content/v1".to_owned(),
        PREGAME_CLAUSE_COMPILER_VERSION.to_owned(),
        PREGAME_CLAUSE_RUNTIME_VERSION.to_owned(),
        PREGAME_RULES_CONTEXT_VERSION.to_owned(),
        exact_source.to_owned(),
        canonical_program(kind),
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn canonical_program(kind: &PregameClauseKind) -> String {
    match kind {
        PregameClauseKind::ExplicitSelfCommanderPermission => {
            "commander-permission/v1;subject=self;permission=commander".to_owned()
        }
        PregameClauseKind::DeckCopyLimit(limit) => format!(
            "deck-copy-limit/v1;name={};maximum={}",
            limit.exact_card_name,
            match limit.maximum {
                CopyMaximum::Unlimited => "unlimited".to_owned(),
                CopyMaximum::AtMost(maximum) => maximum.to_string(),
            }
        ),
        PregameClauseKind::OpeningHand(OpeningHandProgram::BeginOnBattlefield(program)) => {
            let counters = program
                .counters
                .iter()
                .map(|counter| format!("{:?}:{}", counter.kind, counter.amount))
                .collect::<Vec<_>>()
                .join(",");
            let post_entry = program
                .post_entry_instructions
                .iter()
                .map(|instruction| format!("{instruction:?}"))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "opening-hand-battlefield/v1;subject=self;seat={:?};counters={counters};\
                 post-entry={post_entry}",
                program.starting_seat_requirement
            )
        }
        PregameClauseKind::OpeningHand(OpeningHandProgram::BeginInGraveyard(program)) => {
            format!(
                "opening-hand-graveyard/v1;subject=self;life-loss={}",
                program.life_loss
            )
        }
        PregameClauseKind::OpeningHand(OpeningHandProgram::RevealAndSchedule(program)) => {
            format!(
                "opening-hand-reveal/v1;subject=self;schedule={:?};effect={:?}",
                program.schedule, program.effect
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PregameZone {
    Battlefield,
    Command,
    Exile,
    Graveyard,
    Hand,
    Library,
    OutsideGame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpeningHandEvidence {
    pub player: PlayerId,
    pub starting_player: PlayerId,
    pub source: ObjectRef,
    pub source_zone: PregameZone,
    /// The kept hand after all mulligans and before start-of-game actions.
    pub kept_hand: BTreeSet<ObjectRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningHandDecision {
    Decline,
    Accept {
        /// Used only by procedures whose follow-up instruction exiles one
        /// remaining card from the player's hand.
        exile_from_hand: Option<ObjectRef>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PregameTransactionOutcome {
    Declined,
    Accepted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PregameTransaction {
    pub source: ObjectRef,
    pub player: PlayerId,
    pub semantic_digest: String,
    pub outcome: PregameTransactionOutcome,
    pub ordered_actions: Vec<PregameAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PregameAction {
    MoveSource {
        source: ObjectRef,
        from: PregameZone,
        to: PregameZone,
    },
    PutCounterOnMovedSource {
        kind: PregameCounterKind,
        amount: u32,
    },
    ExileCardFromHand {
        card: ObjectRef,
    },
    LoseLife {
        player: PlayerId,
        amount: i64,
    },
    RevealSourceFromOpeningHand {
        source: ObjectRef,
    },
    RegisterDelayedEffect {
        source: ObjectRef,
        controller: PlayerId,
        schedule: RevealSchedule,
        effect: RevealEffect,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PregameRuntimeError {
    ProgramDoesNotUseOpeningHand,
    SourceNotInHand {
        source: ObjectRef,
        actual_zone: PregameZone,
    },
    SourceNotInKeptOpeningHand(ObjectRef),
    StartingPlayerIneligible(PlayerId),
    UnexpectedExileSelection(ObjectRef),
    MissingRequiredExileSelection,
    ExileSelectionIsSource(ObjectRef),
    ExileSelectionNotInHand(ObjectRef),
    NonpositiveLifeLoss(i64),
    DelayedProgramExpected,
    DuplicateDelayedEffectId(DelayedEffectId),
    DuplicateDueEffectId(DueEffectId),
    UnknownDueEffect(DueEffectId),
    DueEffectAlreadyResolved(DueEffectId),
    SourceIncarnationMismatch {
        expected: ObjectRef,
        actual: ObjectRef,
    },
}

impl fmt::Display for PregameRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PregameRuntimeError {}

pub fn prepare_opening_hand_transaction(
    program: &PregameClauseProgram,
    evidence: &OpeningHandEvidence,
    decision: OpeningHandDecision,
) -> Result<PregameTransaction, PregameRuntimeError> {
    let PregameClauseKind::OpeningHand(opening_hand) = program.kind() else {
        return Err(PregameRuntimeError::ProgramDoesNotUseOpeningHand);
    };
    if evidence.source_zone != PregameZone::Hand {
        return Err(PregameRuntimeError::SourceNotInHand {
            source: evidence.source,
            actual_zone: evidence.source_zone,
        });
    }
    if !evidence.kept_hand.contains(&evidence.source) {
        return Err(PregameRuntimeError::SourceNotInKeptOpeningHand(
            evidence.source,
        ));
    }
    if matches!(decision, OpeningHandDecision::Decline) {
        return Ok(PregameTransaction {
            source: evidence.source,
            player: evidence.player,
            semantic_digest: program.semantic_digest().to_owned(),
            outcome: PregameTransactionOutcome::Declined,
            ordered_actions: Vec::new(),
        });
    }
    let OpeningHandDecision::Accept { exile_from_hand } = decision else {
        unreachable!("decline returned above");
    };

    let ordered_actions = match opening_hand {
        OpeningHandProgram::BeginOnBattlefield(battlefield) => {
            if battlefield.starting_seat_requirement == StartingSeatRequirement::NotStartingPlayer
                && evidence.player == evidence.starting_player
            {
                return Err(PregameRuntimeError::StartingPlayerIneligible(
                    evidence.player,
                ));
            }

            let requires_exile = battlefield
                .post_entry_instructions
                .contains(&PostEntryInstruction::ExileOneCardFromYourHandIfPossible);
            let remaining_hand = evidence
                .kept_hand
                .iter()
                .copied()
                .filter(|card| *card != evidence.source)
                .collect::<BTreeSet<_>>();
            validate_exile_selection(
                requires_exile,
                exile_from_hand,
                evidence.source,
                &remaining_hand,
            )?;

            let mut actions = vec![PregameAction::MoveSource {
                source: evidence.source,
                from: PregameZone::Hand,
                to: PregameZone::Battlefield,
            }];
            actions.extend(battlefield.counters.iter().map(|counter| {
                PregameAction::PutCounterOnMovedSource {
                    kind: counter.kind,
                    amount: counter.amount,
                }
            }));
            if let Some(card) = exile_from_hand {
                actions.push(PregameAction::ExileCardFromHand { card });
            }
            actions
        }
        OpeningHandProgram::BeginInGraveyard(graveyard) => {
            if let Some(card) = exile_from_hand {
                return Err(PregameRuntimeError::UnexpectedExileSelection(card));
            }
            if graveyard.life_loss <= 0 {
                return Err(PregameRuntimeError::NonpositiveLifeLoss(
                    graveyard.life_loss,
                ));
            }
            vec![
                PregameAction::MoveSource {
                    source: evidence.source,
                    from: PregameZone::Hand,
                    to: PregameZone::Graveyard,
                },
                PregameAction::LoseLife {
                    player: evidence.player,
                    amount: graveyard.life_loss,
                },
            ]
        }
        OpeningHandProgram::RevealAndSchedule(reveal) => {
            if let Some(card) = exile_from_hand {
                return Err(PregameRuntimeError::UnexpectedExileSelection(card));
            }
            vec![
                PregameAction::RevealSourceFromOpeningHand {
                    source: evidence.source,
                },
                PregameAction::RegisterDelayedEffect {
                    source: evidence.source,
                    controller: evidence.player,
                    schedule: reveal.schedule,
                    effect: reveal.effect.clone(),
                },
            ]
        }
    };

    Ok(PregameTransaction {
        source: evidence.source,
        player: evidence.player,
        semantic_digest: program.semantic_digest().to_owned(),
        outcome: PregameTransactionOutcome::Accepted,
        ordered_actions,
    })
}

fn validate_exile_selection(
    required_if_possible: bool,
    selection: Option<ObjectRef>,
    source: ObjectRef,
    remaining_hand: &BTreeSet<ObjectRef>,
) -> Result<(), PregameRuntimeError> {
    if !required_if_possible {
        if let Some(card) = selection {
            return Err(PregameRuntimeError::UnexpectedExileSelection(card));
        }
        return Ok(());
    }
    let Some(card) = selection else {
        return if remaining_hand.is_empty() {
            Ok(())
        } else {
            Err(PregameRuntimeError::MissingRequiredExileSelection)
        };
    };
    if card == source {
        return Err(PregameRuntimeError::ExileSelectionIsSource(card));
    }
    if !remaining_hand.contains(&card) {
        return Err(PregameRuntimeError::ExileSelectionNotInHand(card));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PregameEvent {
    BeginningOfFirstUpkeepOfGame { active_player: PlayerId },
    BeginningOfPlayersFirstUpkeep { active_player: PlayerId },
    BeginningOfPlayersFirstMainPhase { active_player: PlayerId },
    PlayerCastsFirstSpell { player: PlayerId, spell: ObjectRef },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledDelayedEffect {
    id: DelayedEffectId,
    source: ObjectRef,
    controller: PlayerId,
    semantic_digest: String,
    schedule: RevealSchedule,
    effect: RevealEffect,
    eligible_opponents: BTreeSet<PlayerId>,
    fired_for_players: BTreeSet<PlayerId>,
    fired_once: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuePregameEffect {
    pub id: DueEffectId,
    pub delayed_effect_id: DelayedEffectId,
    pub source: ObjectRef,
    pub controller: PlayerId,
    pub semantic_digest: String,
    pub event: PregameEvent,
    pub effect: RevealEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PregameResolutionTransaction {
    pub due_effect_id: DueEffectId,
    pub source: ObjectRef,
    pub controller: PlayerId,
    pub semantic_digest: String,
    pub effect: RevealEffect,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PregameDelayedEffectRegistry {
    next_delayed_effect_id: DelayedEffectId,
    next_due_effect_id: DueEffectId,
    installed: BTreeMap<DelayedEffectId, InstalledDelayedEffect>,
    due: BTreeMap<DueEffectId, DuePregameEffect>,
    resolved: BTreeSet<DueEffectId>,
}

impl PregameDelayedEffectRegistry {
    pub fn install_from_transaction(
        &mut self,
        transaction: &PregameTransaction,
        players: &BTreeSet<PlayerId>,
    ) -> Result<Vec<DelayedEffectId>, PregameRuntimeError> {
        let mut installed_ids = Vec::new();
        for action in &transaction.ordered_actions {
            let PregameAction::RegisterDelayedEffect {
                source,
                controller,
                schedule,
                effect,
            } = action
            else {
                continue;
            };
            if *source != transaction.source {
                return Err(PregameRuntimeError::SourceIncarnationMismatch {
                    expected: transaction.source,
                    actual: *source,
                });
            }
            let id = next_nonzero_id(&mut self.next_delayed_effect_id);
            if self.installed.contains_key(&id) {
                return Err(PregameRuntimeError::DuplicateDelayedEffectId(id));
            }
            let eligible_opponents = players
                .iter()
                .copied()
                .filter(|player| player != controller)
                .collect();
            self.installed.insert(
                id,
                InstalledDelayedEffect {
                    id,
                    source: *source,
                    controller: *controller,
                    semantic_digest: transaction.semantic_digest.clone(),
                    schedule: *schedule,
                    effect: effect.clone(),
                    eligible_opponents,
                    fired_for_players: BTreeSet::new(),
                    fired_once: false,
                },
            );
            installed_ids.push(id);
        }
        if installed_ids.is_empty() {
            return Err(PregameRuntimeError::DelayedProgramExpected);
        }
        Ok(installed_ids)
    }

    pub fn observe_event(
        &mut self,
        event: PregameEvent,
    ) -> Result<Vec<DuePregameEffect>, PregameRuntimeError> {
        let matching = self
            .installed
            .iter()
            .filter_map(|(id, installed)| delayed_effect_matches(installed, event).then_some(*id))
            .collect::<Vec<_>>();
        let mut created = Vec::with_capacity(matching.len());
        for installed_id in matching {
            let installed = self
                .installed
                .get_mut(&installed_id)
                .expect("matching delayed effect exists");
            match event {
                PregameEvent::PlayerCastsFirstSpell { player, .. } => {
                    installed.fired_for_players.insert(player);
                }
                _ => installed.fired_once = true,
            }
            let id = next_nonzero_id(&mut self.next_due_effect_id);
            if self.due.contains_key(&id) || self.resolved.contains(&id) {
                return Err(PregameRuntimeError::DuplicateDueEffectId(id));
            }
            let due = DuePregameEffect {
                id,
                delayed_effect_id: installed.id,
                source: installed.source,
                controller: installed.controller,
                semantic_digest: installed.semantic_digest.clone(),
                event,
                effect: installed.effect.clone(),
            };
            self.due.insert(id, due.clone());
            created.push(due);
        }
        Ok(created)
    }

    pub fn resolve(
        &mut self,
        due_effect_id: DueEffectId,
    ) -> Result<PregameResolutionTransaction, PregameRuntimeError> {
        if self.resolved.contains(&due_effect_id) {
            return Err(PregameRuntimeError::DueEffectAlreadyResolved(due_effect_id));
        }
        let due = self
            .due
            .remove(&due_effect_id)
            .ok_or(PregameRuntimeError::UnknownDueEffect(due_effect_id))?;
        self.resolved.insert(due_effect_id);
        Ok(PregameResolutionTransaction {
            due_effect_id,
            source: due.source,
            controller: due.controller,
            semantic_digest: due.semantic_digest,
            effect: due.effect,
        })
    }
}

fn delayed_effect_matches(installed: &InstalledDelayedEffect, event: PregameEvent) -> bool {
    match (installed.schedule, event) {
        (
            RevealSchedule::BeginningOfFirstUpkeep(FirstUpkeep::FirstUpkeepOfGame),
            PregameEvent::BeginningOfFirstUpkeepOfGame { .. },
        ) => !installed.fired_once,
        (
            RevealSchedule::BeginningOfFirstUpkeep(FirstUpkeep::ControllersFirstUpkeep),
            PregameEvent::BeginningOfPlayersFirstUpkeep { active_player },
        ) => active_player == installed.controller && !installed.fired_once,
        (
            RevealSchedule::BeginningOfControllersFirstMainPhase,
            PregameEvent::BeginningOfPlayersFirstMainPhase { active_player },
        ) => active_player == installed.controller && !installed.fired_once,
        (
            RevealSchedule::EachOpponentsFirstSpell,
            PregameEvent::PlayerCastsFirstSpell { player, .. },
        ) => {
            installed.eligible_opponents.contains(&player)
                && !installed.fired_for_players.contains(&player)
        }
        _ => false,
    }
}

fn next_nonzero_id(counter: &mut u64) -> u64 {
    *counter = counter.wrapping_add(1);
    if *counter == 0 {
        *counter = 1;
    }
    *counter
}
