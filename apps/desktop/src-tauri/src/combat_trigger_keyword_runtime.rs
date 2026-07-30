//! Exact, content keyed programs for a reviewed group of combat keywords.
//!
//! The compiler accepts only complete standalone Oracle clauses whose reminder
//! text supplies the full reviewed meaning. Ability grants, compounds,
//! modifiers, variable values, joke variants, and reminderless occurrences
//! remain rejected. Program identity contains exact Oracle content and
//! versioned rules context, never card names, card identifiers, database rows,
//! snapshot metadata, or addresses.
//!
//! The runtime keeps declaration, trigger creation, stack resolution, state
//! based actions, and cleanup as separate boundaries. It requires complete
//! evidence for multiplayer defenders, attack and block groups, targets,
//! current characteristics, sacrifice choices, and libraries. Nothing in this
//! module is connected to the production simulator yet.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const COMBAT_TRIGGER_KEYWORD_COMPILER_VERSION: &str = "combat-trigger-keyword-compiler-0.1";
pub const COMBAT_TRIGGER_KEYWORD_RUNTIME_VERSION: &str = "combat-trigger-keyword-runtime-0.1";
pub const COMBAT_TRIGGER_KEYWORD_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:101.3,119.3,122.1a,400.7,506.4,508.1,508.1b,\
     508.1m,508.4,508.5,509.1,509.1a-c,509.1g-i,603.2,603.3,603.3d,608.2b,608.2h,\
     609.3,611.2a,701.21,702.23,702.39,702.86,702.91,702.105,702.115,702.118,\
     702.121,702.130,802.2-4";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type CombatId = u64;
pub type TurnId = u64;
pub type TriggerId = u64;
pub type AbilityInstanceId = u64;

/// Recognition is not execution coverage. Production does not yet provide the
/// complete evidence contracts required by this runtime.
pub const fn combat_trigger_keyword_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CombatTriggerKeywordFamily {
    Afflict,
    Annihilator,
    BattleCry,
    Dethrone,
    Ingest,
    Melee,
    Provoke,
    Rampage,
    Skulk,
}

impl CombatTriggerKeywordFamily {
    pub const fn printed_label(self) -> &'static str {
        match self {
            Self::Afflict => "Afflict",
            Self::Annihilator => "Annihilator",
            Self::BattleCry => "Battle cry",
            Self::Dethrone => "Dethrone",
            Self::Ingest => "Ingest",
            Self::Melee => "Melee",
            Self::Provoke => "Provoke",
            Self::Rampage => "Rampage",
            Self::Skulk => "Skulk",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatTriggerKeywordKind {
    Afflict { amount: u32 },
    Annihilator { amount: u32 },
    BattleCry,
    Dethrone,
    Ingest,
    Melee,
    Provoke,
    Rampage { amount: u32 },
    Skulk,
}

impl CombatTriggerKeywordKind {
    pub const fn family(&self) -> CombatTriggerKeywordFamily {
        match self {
            Self::Afflict { .. } => CombatTriggerKeywordFamily::Afflict,
            Self::Annihilator { .. } => CombatTriggerKeywordFamily::Annihilator,
            Self::BattleCry => CombatTriggerKeywordFamily::BattleCry,
            Self::Dethrone => CombatTriggerKeywordFamily::Dethrone,
            Self::Ingest => CombatTriggerKeywordFamily::Ingest,
            Self::Melee => CombatTriggerKeywordFamily::Melee,
            Self::Provoke => CombatTriggerKeywordFamily::Provoke,
            Self::Rampage { .. } => CombatTriggerKeywordFamily::Rampage,
            Self::Skulk => CombatTriggerKeywordFamily::Skulk,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatTriggerKeywordProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    kind: CombatTriggerKeywordKind,
}

impl CombatTriggerKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &CombatTriggerKeywordKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        combat_trigger_keyword_production_adapter_connected()
    }
}

/// Reserved for exact clauses already owned earlier in a future backend order.
/// The authoritative snapshot census for this module currently has no earlier
/// owned occurrence in these nine standalone families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarlierCombatTriggerClauseOwner {
    OfficialKeywordRuntime,
    MechanicRuntime,
    CombatRestrictionRuntime,
    CombatSpecialKeywordRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatTriggerClauseClassification {
    Program(CombatTriggerKeywordProgram),
    EarlierOwner {
        family: CombatTriggerKeywordFamily,
        owner: EarlierCombatTriggerClauseOwner,
    },
    Rejected,
}

pub fn compile_combat_trigger_keyword_program(
    exact_source: &str,
    normalized_source: &str,
) -> Option<CombatTriggerKeywordProgram> {
    match classify_combat_trigger_keyword_clause(exact_source, normalized_source) {
        CombatTriggerClauseClassification::Program(program) => Some(program),
        CombatTriggerClauseClassification::EarlierOwner { .. }
        | CombatTriggerClauseClassification::Rejected => None,
    }
}

pub fn classify_combat_trigger_keyword_clause(
    exact_source: &str,
    normalized_source: &str,
) -> CombatTriggerClauseClassification {
    if !is_complete_single_line(exact_source)
        || !is_complete_single_line(normalized_source)
        || !normalized_source_is_content_derived(exact_source, normalized_source)
    {
        return CombatTriggerClauseClassification::Rejected;
    }

    let Some(kind) = parse_reviewed_kind(exact_source) else {
        return CombatTriggerClauseClassification::Rejected;
    };
    let semantic_digest = combat_trigger_semantic_digest(exact_source, &kind);
    CombatTriggerClauseClassification::Program(CombatTriggerKeywordProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        semantic_digest,
        kind,
    })
}

fn parse_reviewed_kind(exact_source: &str) -> Option<CombatTriggerKeywordKind> {
    if exact_source
        == "Battle cry (Whenever this creature attacks, each other attacking creature gets +1/+0 \
           until end of turn.)"
    {
        return Some(CombatTriggerKeywordKind::BattleCry);
    }
    if exact_source
        == "Dethrone (Whenever this creature attacks the player with the most life or tied for \
           most life, put a +1/+1 counter on it.)"
        || exact_source
            == "Dethrone (Whenever this creature attacks the player with the most life or tied for \
               the most life, put a +1/+1 counter on it.)"
    {
        return Some(CombatTriggerKeywordKind::Dethrone);
    }
    if exact_source
        == "Ingest (Whenever this creature deals combat damage to a player, that player exiles the \
           top card of their library.)"
    {
        return Some(CombatTriggerKeywordKind::Ingest);
    }
    if exact_source
        == "Melee (Whenever this creature attacks, it gets +1/+1 until end of turn for each \
           opponent you attacked this combat.)"
    {
        return Some(CombatTriggerKeywordKind::Melee);
    }
    if exact_source
        == "Provoke (Whenever this creature attacks, you may have target creature defending player \
           controls untap and block it if able.)"
    {
        return Some(CombatTriggerKeywordKind::Provoke);
    }
    if exact_source == "Skulk (This creature can't be blocked by creatures with greater power.)" {
        return Some(CombatTriggerKeywordKind::Skulk);
    }

    if let Some(amount) = parse_afflict(exact_source) {
        return Some(CombatTriggerKeywordKind::Afflict { amount });
    }
    if let Some(amount) = parse_annihilator(exact_source) {
        return Some(CombatTriggerKeywordKind::Annihilator { amount });
    }
    if let Some(amount) = parse_rampage(exact_source) {
        return Some(CombatTriggerKeywordKind::Rampage { amount });
    }
    None
}

fn parse_afflict(source: &str) -> Option<u32> {
    let amount_text = source
        .strip_prefix("Afflict ")?
        .split_once(" (")
        .map(|(amount, _)| amount)?;
    let amount = parse_positive_u32(amount_text)?;
    (source
        == format!(
            "Afflict {amount} (Whenever this creature becomes blocked, defending player loses \
             {amount} life.)"
        ))
    .then_some(amount)
}

fn parse_annihilator(source: &str) -> Option<u32> {
    let amount_text = source
        .strip_prefix("Annihilator ")?
        .split_once(" (")
        .map(|(amount, _)| amount)?;
    let amount = parse_positive_u32(amount_text)?;
    let sacrifice_phrase = if amount == 1 {
        "a permanent".to_owned()
    } else {
        format!("{} permanents", english_cardinal(amount)?)
    };
    (source
        == format!(
            "Annihilator {amount} (Whenever this creature attacks, defending player sacrifices \
             {sacrifice_phrase} of their choice.)"
        ))
    .then_some(amount)
}

fn parse_rampage(source: &str) -> Option<u32> {
    let amount_text = source
        .strip_prefix("Rampage ")?
        .split_once(" (")
        .map(|(amount, _)| amount)?;
    let amount = parse_positive_u32(amount_text)?;
    (source
        == format!(
            "Rampage {amount} (Whenever this creature becomes blocked, it gets +{amount}/+{amount} \
             until end of turn for each creature blocking it beyond the first.)"
        ))
    .then_some(amount)
}

fn english_cardinal(number: u32) -> Option<String> {
    const SMALL: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 10] = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    match number {
        0..=19 => Some(SMALL[number as usize].to_owned()),
        20..=99 => {
            let tens = TENS[(number / 10) as usize];
            let ones = number % 10;
            Some(if ones == 0 {
                tens.to_owned()
            } else {
                format!("{tens}-{ones}", ones = SMALL[ones as usize])
            })
        }
        _ => None,
    }
}

fn parse_positive_u32(source: &str) -> Option<u32> {
    if source.is_empty()
        || (source.len() > 1 && source.starts_with('0'))
        || !source.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    source.parse::<u32>().ok().filter(|amount| *amount > 0)
}

fn is_complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source.trim() == source
        && !source.contains(['\r', '\n'])
        && collapse_whitespace(source) == source
}

fn normalized_source_is_content_derived(exact_source: &str, normalized_source: &str) -> bool {
    normalized_source == exact_source
        || normalized_source == reviewed_combat_trigger_normalized_source(exact_source)
}

pub fn reviewed_combat_trigger_normalized_source(exact_source: &str) -> String {
    match exact_source {
        "Dethrone (Whenever this creature attacks the player with the most life or tied for the \
         most life, put a +1/+1 counter on it.)" => {
            "Dethrone (Whenever this creature attacks the player with the most life or tied for \
             most life, put a +1/+1 counter on it.)"
                .to_owned()
        }
        _ => exact_source.to_owned(),
    }
}

fn collapse_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn combat_trigger_semantic_digest(exact_source: &str, kind: &CombatTriggerKeywordKind) -> String {
    let semantics = canonical_semantics(kind);
    let mut hasher = Sha256::new();
    for component in [
        "combat-trigger-keyword-content/v1",
        COMBAT_TRIGGER_KEYWORD_COMPILER_VERSION,
        COMBAT_TRIGGER_KEYWORD_RUNTIME_VERSION,
        COMBAT_TRIGGER_KEYWORD_RULES_CONTEXT_VERSION,
        exact_source,
        &semantics,
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn canonical_semantics(kind: &CombatTriggerKeywordKind) -> String {
    match kind {
        CombatTriggerKeywordKind::Afflict { amount } => format!(
            "trigger=becomes-blocked;defending-player=attack-history;effect=life-loss:{amount};\
             instances=separate"
        ),
        CombatTriggerKeywordKind::Annihilator { amount } => format!(
            "trigger=attacks;defending-player=selected-defender;effect=sacrifice:{amount}:choice;\
             instances=separate"
        ),
        CombatTriggerKeywordKind::BattleCry => {
            "trigger=attacks;effect=each-other-current-attacker:+1/+0:eot;instances=separate"
                .to_owned()
        }
        CombatTriggerKeywordKind::Dethrone => {
            "trigger=attacks-player:tied-most-life-at-trigger;effect=self:+1/+1-counter;\
             instances=separate"
                .to_owned()
        }
        CombatTriggerKeywordKind::Ingest => {
            "trigger=combat-damage-to-player;effect=damaged-player:exile-library-top;\
             instances=separate"
                .to_owned()
        }
        CombatTriggerKeywordKind::Melee => {
            "trigger=attacks;effect=self:+1/+1:eot:per-distinct-opponent-attacked-with-creature;\
             instances=separate"
                .to_owned()
        }
        CombatTriggerKeywordKind::Provoke => {
            "trigger=attacks;target=defending-player-creature;choice=resolution;\
             effect=block-requirement-if-able+untap;instances=separate"
                .to_owned()
        }
        CombatTriggerKeywordKind::Rampage { amount } => format!(
            "trigger=becomes-blocked;resolution-count=current-blockers-beyond-first;\
             effect=self:+{amount}/+{amount}:eot;instances=separate"
        ),
        CombatTriggerKeywordKind::Skulk => {
            "static=block-restriction;blocker-power<=attacker-power;instances=redundant".to_owned()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

impl ObjectRef {
    fn next_incarnation(self) -> Result<Self, CombatTriggerRuntimeError> {
        Ok(Self {
            object_id: self.object_id,
            incarnation_id: self.incarnation_id.checked_add(1).ok_or(
                CombatTriggerRuntimeError::IncarnationOverflow {
                    object_id: self.object_id,
                },
            )?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Defender {
    Player(PlayerId),
    Planeswalker {
        permanent: ObjectRef,
        controller: PlayerId,
    },
    Battle {
        permanent: ObjectRef,
        protector: PlayerId,
    },
}

impl Defender {
    pub const fn defending_player(self) -> PlayerId {
        match self {
            Self::Player(player) => player,
            Self::Planeswalker { controller, .. } => controller,
            Self::Battle { protector, .. } => protector,
        }
    }

    pub const fn attacked_player(self) -> Option<PlayerId> {
        match self {
            Self::Player(player) => Some(player),
            Self::Planeswalker { .. } | Self::Battle { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLastKnownInformation {
    pub object_ref: ObjectRef,
    pub controller: PlayerId,
    pub was_creature: bool,
    pub power: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredAttacker {
    pub source: SourceLastKnownInformation,
    pub defender: Defender,
    /// False for a creature merely put onto the battlefield attacking.
    pub was_declared_as_attacker: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackDeclarationEvent {
    pub combat_id: CombatId,
    pub turn_id: TurnId,
    pub attackers: Vec<DeclaredAttacker>,
    pub declaration_complete: bool,
    pub simultaneous_group_complete: bool,
}

impl AttackDeclarationEvent {
    fn attacker(&self, source: ObjectRef) -> Result<&DeclaredAttacker, CombatTriggerRuntimeError> {
        let mut matches = self
            .attackers
            .iter()
            .filter(|attacker| attacker.source.object_ref == source);
        let first = matches
            .next()
            .ok_or(CombatTriggerRuntimeError::SourceDidNotAttack { source })?;
        if matches.next().is_some() {
            return Err(CombatTriggerRuntimeError::DuplicateAttackRecord { source });
        }
        Ok(first)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockGroup {
    pub attacker: ObjectRef,
    pub blockers: Vec<ObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDeclarationEvent {
    pub combat_id: CombatId,
    pub turn_id: TurnId,
    pub groups: Vec<BlockGroup>,
    /// Attackers that changed from unblocked to blocked in this declaration.
    pub newly_blocked_attackers: BTreeSet<ObjectRef>,
    pub declaration_complete: bool,
    pub simultaneous_group_complete: bool,
}

impl BlockDeclarationEvent {
    fn group(&self, source: ObjectRef) -> Result<&BlockGroup, CombatTriggerRuntimeError> {
        let mut matches = self.groups.iter().filter(|group| group.attacker == source);
        let first = matches
            .next()
            .ok_or(CombatTriggerRuntimeError::MissingBlockGroup { source })?;
        if matches.next().is_some() {
            return Err(CombatTriggerRuntimeError::DuplicateBlockGroup { source });
        }
        Ok(first)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageRecipient {
    Player(PlayerId),
    Permanent(ObjectRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatDamagePacket {
    pub source: SourceLastKnownInformation,
    pub recipient: DamageRecipient,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatDamageEvent {
    pub combat_id: CombatId,
    pub turn_id: TurnId,
    pub packets: Vec<CombatDamagePacket>,
    pub damage_event_complete: bool,
    pub simultaneous_group_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerPlayerEvidence {
    pub in_game: bool,
    pub life: i32,
    pub opponents: BTreeSet<PlayerId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerPermanentEvidence {
    pub object_ref: ObjectRef,
    pub controller: PlayerId,
    pub is_creature: bool,
    /// Controllers whose current triggered abilities may legally target this
    /// permanent after all shroud, hexproof, protection, and other effects.
    pub targetable_by: BTreeSet<PlayerId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerCreationEvidence {
    pub players: BTreeMap<PlayerId, TriggerPlayerEvidence>,
    pub permanents: BTreeMap<ObjectRef, TriggerPermanentEvidence>,
    pub players_complete: bool,
    pub opponent_relations_complete: bool,
    pub battlefield_complete: bool,
    pub life_totals_complete: bool,
    pub targeting_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatKeywordTriggerPayload {
    Afflict {
        defending_player: PlayerId,
        amount: u32,
    },
    Annihilator {
        defending_player: PlayerId,
        amount: u32,
    },
    BattleCry,
    Dethrone,
    Ingest {
        damaged_player: PlayerId,
    },
    Melee {
        opponents_attacked: BTreeSet<PlayerId>,
    },
    Provoke {
        defending_player: PlayerId,
        target: ObjectRef,
    },
    Rampage {
        amount: u32,
    },
}

impl CombatKeywordTriggerPayload {
    pub const fn family(&self) -> CombatTriggerKeywordFamily {
        match self {
            Self::Afflict { .. } => CombatTriggerKeywordFamily::Afflict,
            Self::Annihilator { .. } => CombatTriggerKeywordFamily::Annihilator,
            Self::BattleCry => CombatTriggerKeywordFamily::BattleCry,
            Self::Dethrone => CombatTriggerKeywordFamily::Dethrone,
            Self::Ingest { .. } => CombatTriggerKeywordFamily::Ingest,
            Self::Melee { .. } => CombatTriggerKeywordFamily::Melee,
            Self::Provoke { .. } => CombatTriggerKeywordFamily::Provoke,
            Self::Rampage { .. } => CombatTriggerKeywordFamily::Rampage,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatKeywordTrigger {
    pub trigger_id: TriggerId,
    pub ability_instance_id: AbilityInstanceId,
    pub program_digest: String,
    pub source_lki: SourceLastKnownInformation,
    pub controller: PlayerId,
    pub combat_id: CombatId,
    pub turn_id: TurnId,
    pub payload: CombatKeywordTriggerPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerCreationResult {
    Trigger(CombatKeywordTrigger),
    ConditionNotMet,
    RemovedFromStackNoLegalTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatTriggerRuntimeError {
    IncompleteEvidence(&'static str),
    WrongEventForKeyword {
        family: CombatTriggerKeywordFamily,
    },
    StaticKeywordHasNoTrigger,
    ProgramTriggerMismatch,
    SourceDidNotAttack {
        source: ObjectRef,
    },
    SourceWasNotDeclaredAsAttacker {
        source: ObjectRef,
    },
    DuplicateAttackRecord {
        source: ObjectRef,
    },
    MissingBlockGroup {
        source: ObjectRef,
    },
    DuplicateBlockGroup {
        source: ObjectRef,
    },
    SourceDidNotBecomeBlocked {
        source: ObjectRef,
    },
    InvalidDefendingPlayer {
        attacker_controller: PlayerId,
        defending_player: PlayerId,
    },
    MissingPlayer(PlayerId),
    MissingPermanent(ObjectRef),
    MissingRequiredTarget,
    IllegalProvokeTarget(ObjectRef),
    IllegalTargetAtResolution(ObjectRef),
    MissingResolutionChoice(&'static str),
    UnexpectedResolutionChoice,
    InvalidSacrificeSelection(&'static str),
    DuplicateObjectSelection(ObjectRef),
    CombatBoundaryMismatch,
    TurnBoundaryMismatch,
    SourceCharacteristicsUnavailable(ObjectRef),
    BlockerCharacteristicsUnavailable(ObjectRef),
    IncarnationOverflow {
        object_id: ObjectId,
    },
    ZoneObjectCollision(ObjectRef),
    ArithmeticOverflow(&'static str),
    StateBasedActionsPending,
    IncompleteLegalDeclarationSet,
    ActualDeclarationNotInLegalSet,
}

impl fmt::Display for CombatTriggerRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompleteEvidence(field) => write!(formatter, "incomplete evidence: {field}"),
            Self::WrongEventForKeyword { family } => {
                write!(formatter, "wrong event for {}", family.printed_label())
            }
            Self::StaticKeywordHasNoTrigger => write!(formatter, "static keyword has no trigger"),
            Self::ProgramTriggerMismatch => write!(formatter, "program and trigger do not match"),
            Self::SourceDidNotAttack { source } => {
                write!(formatter, "source {source:?} did not attack")
            }
            Self::SourceWasNotDeclaredAsAttacker { source } => {
                write!(
                    formatter,
                    "source {source:?} was put attacking rather than declared"
                )
            }
            Self::DuplicateAttackRecord { source } => {
                write!(formatter, "duplicate attack record for {source:?}")
            }
            Self::MissingBlockGroup { source } => {
                write!(formatter, "missing block group for {source:?}")
            }
            Self::DuplicateBlockGroup { source } => {
                write!(formatter, "duplicate block group for {source:?}")
            }
            Self::SourceDidNotBecomeBlocked { source } => {
                write!(formatter, "source {source:?} did not become blocked")
            }
            Self::InvalidDefendingPlayer {
                attacker_controller,
                defending_player,
            } => write!(
                formatter,
                "player {defending_player} is not a verified opponent of {attacker_controller}"
            ),
            Self::MissingPlayer(player) => write!(formatter, "missing player {player}"),
            Self::MissingPermanent(object) => write!(formatter, "missing permanent {object:?}"),
            Self::MissingRequiredTarget => write!(formatter, "required target was not supplied"),
            Self::IllegalProvokeTarget(target) => {
                write!(formatter, "illegal provoke target {target:?}")
            }
            Self::IllegalTargetAtResolution(target) => {
                write!(formatter, "target {target:?} is illegal at resolution")
            }
            Self::MissingResolutionChoice(choice) => {
                write!(formatter, "missing resolution choice: {choice}")
            }
            Self::UnexpectedResolutionChoice => write!(formatter, "unexpected resolution choice"),
            Self::InvalidSacrificeSelection(reason) => {
                write!(formatter, "invalid sacrifice selection: {reason}")
            }
            Self::DuplicateObjectSelection(object) => {
                write!(formatter, "object selected more than once: {object:?}")
            }
            Self::CombatBoundaryMismatch => write!(formatter, "combat boundary mismatch"),
            Self::TurnBoundaryMismatch => write!(formatter, "turn boundary mismatch"),
            Self::SourceCharacteristicsUnavailable(source) => {
                write!(
                    formatter,
                    "source characteristics unavailable for {source:?}"
                )
            }
            Self::BlockerCharacteristicsUnavailable(blocker) => {
                write!(
                    formatter,
                    "blocker characteristics unavailable for {blocker:?}"
                )
            }
            Self::IncarnationOverflow { object_id } => {
                write!(formatter, "incarnation overflow for object {object_id}")
            }
            Self::ZoneObjectCollision(object) => {
                write!(formatter, "zone object collision for {object:?}")
            }
            Self::ArithmeticOverflow(operation) => {
                write!(formatter, "arithmetic overflow while {operation}")
            }
            Self::StateBasedActionsPending => {
                write!(formatter, "state based actions must be processed first")
            }
            Self::IncompleteLegalDeclarationSet => {
                write!(formatter, "legal block declaration set is incomplete")
            }
            Self::ActualDeclarationNotInLegalSet => {
                write!(
                    formatter,
                    "actual block declaration is not in the legal set"
                )
            }
        }
    }
}

impl std::error::Error for CombatTriggerRuntimeError {}

fn require_trigger_evidence(
    evidence: &TriggerCreationEvidence,
) -> Result<(), CombatTriggerRuntimeError> {
    if !evidence.players_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence("players"));
    }
    if !evidence.opponent_relations_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "opponent relations",
        ));
    }
    Ok(())
}

fn validate_attacker_and_defender<'a>(
    attack: &'a AttackDeclarationEvent,
    source: ObjectRef,
    evidence: &TriggerCreationEvidence,
) -> Result<&'a DeclaredAttacker, CombatTriggerRuntimeError> {
    if !attack.declaration_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "attack declaration",
        ));
    }
    if !attack.simultaneous_group_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "simultaneous attacker group",
        ));
    }
    require_trigger_evidence(evidence)?;
    let attacker = attack.attacker(source)?;
    if !attacker.was_declared_as_attacker {
        return Err(CombatTriggerRuntimeError::SourceWasNotDeclaredAsAttacker { source });
    }
    let player = evidence.players.get(&attacker.source.controller).ok_or(
        CombatTriggerRuntimeError::MissingPlayer(attacker.source.controller),
    )?;
    let defending_player = attacker.defender.defending_player();
    if !player.opponents.contains(&defending_player) {
        return Err(CombatTriggerRuntimeError::InvalidDefendingPlayer {
            attacker_controller: attacker.source.controller,
            defending_player,
        });
    }
    Ok(attacker)
}

pub fn create_attack_trigger(
    program: &CombatTriggerKeywordProgram,
    trigger_id: TriggerId,
    ability_instance_id: AbilityInstanceId,
    source: ObjectRef,
    attack: &AttackDeclarationEvent,
    evidence: &TriggerCreationEvidence,
    provoke_target: Option<ObjectRef>,
) -> Result<TriggerCreationResult, CombatTriggerRuntimeError> {
    let attacker = validate_attacker_and_defender(attack, source, evidence)?;
    let controller = attacker.source.controller;
    let defending_player = attacker.defender.defending_player();
    let payload = match program.kind() {
        CombatTriggerKeywordKind::Annihilator { amount } => {
            if provoke_target.is_some() {
                return Err(CombatTriggerRuntimeError::UnexpectedResolutionChoice);
            }
            CombatKeywordTriggerPayload::Annihilator {
                defending_player,
                amount: *amount,
            }
        }
        CombatTriggerKeywordKind::BattleCry => {
            if provoke_target.is_some() {
                return Err(CombatTriggerRuntimeError::UnexpectedResolutionChoice);
            }
            CombatKeywordTriggerPayload::BattleCry
        }
        CombatTriggerKeywordKind::Dethrone => {
            if provoke_target.is_some() {
                return Err(CombatTriggerRuntimeError::UnexpectedResolutionChoice);
            }
            if !evidence.life_totals_complete {
                return Err(CombatTriggerRuntimeError::IncompleteEvidence("life totals"));
            }
            let Some(attacked_player) = attacker.defender.attacked_player() else {
                return Ok(TriggerCreationResult::ConditionNotMet);
            };
            let attacked_life = evidence
                .players
                .get(&attacked_player)
                .filter(|player| player.in_game)
                .ok_or(CombatTriggerRuntimeError::MissingPlayer(attacked_player))?
                .life;
            let maximum = evidence
                .players
                .values()
                .filter(|player| player.in_game)
                .map(|player| player.life)
                .max()
                .ok_or(CombatTriggerRuntimeError::IncompleteEvidence(
                    "in-game player life totals",
                ))?;
            if attacked_life != maximum {
                return Ok(TriggerCreationResult::ConditionNotMet);
            }
            CombatKeywordTriggerPayload::Dethrone
        }
        CombatTriggerKeywordKind::Melee => {
            if provoke_target.is_some() {
                return Err(CombatTriggerRuntimeError::UnexpectedResolutionChoice);
            }
            let opponents = evidence
                .players
                .get(&controller)
                .ok_or(CombatTriggerRuntimeError::MissingPlayer(controller))?
                .opponents
                .clone();
            let opponents_attacked = attack
                .attackers
                .iter()
                .filter(|declared| {
                    declared.was_declared_as_attacker && declared.source.controller == controller
                })
                .map(|declared| declared.defender.defending_player())
                .filter(|player| opponents.contains(player))
                .collect::<BTreeSet<_>>();
            CombatKeywordTriggerPayload::Melee { opponents_attacked }
        }
        CombatTriggerKeywordKind::Provoke => {
            if !evidence.battlefield_complete {
                return Err(CombatTriggerRuntimeError::IncompleteEvidence("battlefield"));
            }
            if !evidence.targeting_complete {
                return Err(CombatTriggerRuntimeError::IncompleteEvidence(
                    "trigger-time target legality",
                ));
            }
            let legal_targets = evidence
                .permanents
                .values()
                .filter(|permanent| {
                    permanent.controller == defending_player
                        && permanent.is_creature
                        && permanent.targetable_by.contains(&controller)
                })
                .map(|permanent| permanent.object_ref)
                .collect::<BTreeSet<_>>();
            let Some(target) = provoke_target else {
                return if legal_targets.is_empty() {
                    Ok(TriggerCreationResult::RemovedFromStackNoLegalTarget)
                } else {
                    Err(CombatTriggerRuntimeError::MissingRequiredTarget)
                };
            };
            if !legal_targets.contains(&target) {
                return Err(CombatTriggerRuntimeError::IllegalProvokeTarget(target));
            }
            CombatKeywordTriggerPayload::Provoke {
                defending_player,
                target,
            }
        }
        CombatTriggerKeywordKind::Afflict { .. }
        | CombatTriggerKeywordKind::Ingest
        | CombatTriggerKeywordKind::Rampage { .. } => {
            return Err(CombatTriggerRuntimeError::WrongEventForKeyword {
                family: program.kind().family(),
            });
        }
        CombatTriggerKeywordKind::Skulk => {
            return Err(CombatTriggerRuntimeError::StaticKeywordHasNoTrigger);
        }
    };
    Ok(TriggerCreationResult::Trigger(CombatKeywordTrigger {
        trigger_id,
        ability_instance_id,
        program_digest: program.semantic_digest().to_owned(),
        source_lki: attacker.source.clone(),
        controller,
        combat_id: attack.combat_id,
        turn_id: attack.turn_id,
        payload,
    }))
}

pub fn create_block_trigger(
    program: &CombatTriggerKeywordProgram,
    trigger_id: TriggerId,
    ability_instance_id: AbilityInstanceId,
    source: ObjectRef,
    attack: &AttackDeclarationEvent,
    blocks: &BlockDeclarationEvent,
    evidence: &TriggerCreationEvidence,
) -> Result<TriggerCreationResult, CombatTriggerRuntimeError> {
    if !blocks.declaration_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "block declaration",
        ));
    }
    if !blocks.simultaneous_group_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "simultaneous blocker groups",
        ));
    }
    if attack.combat_id != blocks.combat_id || attack.turn_id != blocks.turn_id {
        return Err(CombatTriggerRuntimeError::CombatBoundaryMismatch);
    }
    let attacker = validate_attacker_and_defender(attack, source, evidence)?;
    let group = blocks.group(source)?;
    if group.blockers.is_empty() || !blocks.newly_blocked_attackers.contains(&source) {
        return Err(CombatTriggerRuntimeError::SourceDidNotBecomeBlocked { source });
    }
    let payload = match program.kind() {
        CombatTriggerKeywordKind::Afflict { amount } => CombatKeywordTriggerPayload::Afflict {
            defending_player: attacker.defender.defending_player(),
            amount: *amount,
        },
        CombatTriggerKeywordKind::Rampage { amount } => {
            CombatKeywordTriggerPayload::Rampage { amount: *amount }
        }
        CombatTriggerKeywordKind::Annihilator { .. }
        | CombatTriggerKeywordKind::BattleCry
        | CombatTriggerKeywordKind::Dethrone
        | CombatTriggerKeywordKind::Ingest
        | CombatTriggerKeywordKind::Melee
        | CombatTriggerKeywordKind::Provoke => {
            return Err(CombatTriggerRuntimeError::WrongEventForKeyword {
                family: program.kind().family(),
            });
        }
        CombatTriggerKeywordKind::Skulk => {
            return Err(CombatTriggerRuntimeError::StaticKeywordHasNoTrigger);
        }
    };
    Ok(TriggerCreationResult::Trigger(CombatKeywordTrigger {
        trigger_id,
        ability_instance_id,
        program_digest: program.semantic_digest().to_owned(),
        source_lki: attacker.source.clone(),
        controller: attacker.source.controller,
        combat_id: attack.combat_id,
        turn_id: attack.turn_id,
        payload,
    }))
}

pub fn create_combat_damage_triggers(
    program: &CombatTriggerKeywordProgram,
    first_trigger_id: TriggerId,
    ability_instance_id: AbilityInstanceId,
    source: ObjectRef,
    damage: &CombatDamageEvent,
) -> Result<Vec<CombatKeywordTrigger>, CombatTriggerRuntimeError> {
    if !matches!(program.kind(), CombatTriggerKeywordKind::Ingest) {
        return Err(match program.kind() {
            CombatTriggerKeywordKind::Skulk => CombatTriggerRuntimeError::StaticKeywordHasNoTrigger,
            _ => CombatTriggerRuntimeError::WrongEventForKeyword {
                family: program.kind().family(),
            },
        });
    }
    if !damage.damage_event_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "combat damage event",
        ));
    }
    if !damage.simultaneous_group_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "simultaneous combat damage group",
        ));
    }
    let matching = damage
        .packets
        .iter()
        .filter(|packet| packet.source.object_ref == source && packet.amount > 0)
        .collect::<Vec<_>>();
    let source_lki = matching.first().map(|packet| packet.source.clone()).ok_or(
        CombatTriggerRuntimeError::IncompleteEvidence("source combat damage packet"),
    )?;
    if matching.iter().any(|packet| packet.source != source_lki) {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "consistent source last known information",
        ));
    }
    let damaged_players = matching
        .iter()
        .filter_map(|packet| match packet.recipient {
            DamageRecipient::Player(player) => Some(player),
            DamageRecipient::Permanent(_) => None,
        })
        .collect::<BTreeSet<_>>();
    damaged_players
        .into_iter()
        .enumerate()
        .map(|(offset, damaged_player)| {
            let trigger_id = first_trigger_id.checked_add(offset as u64).ok_or(
                CombatTriggerRuntimeError::ArithmeticOverflow(
                    "assigning ingest trigger identifiers",
                ),
            )?;
            Ok(CombatKeywordTrigger {
                trigger_id,
                ability_instance_id,
                program_digest: program.semantic_digest().to_owned(),
                source_lki: source_lki.clone(),
                controller: source_lki.controller,
                combat_id: damage.combat_id,
                turn_id: damage.turn_id,
                payload: CombatKeywordTriggerPayload::Ingest { damaged_player },
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CounterKind {
    PlusOnePlusOne,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermanentState {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub is_creature: bool,
    pub is_token: bool,
    pub tapped: bool,
    pub base_power: Option<i32>,
    pub counters: BTreeMap<CounterKind, u32>,
    /// Complete resolution-time target legality by trigger controller.
    pub targetable_by: Option<BTreeSet<PlayerId>>,
    /// None means sacrifice restrictions or permissions are not fully known.
    pub can_be_sacrificed_for_effect: Option<bool>,
    /// None means untap restrictions or replacement effects are not known.
    pub can_untap: Option<bool>,
    /// Final number of counters placed for each one instructed, after all
    /// applicable replacement and prevention effects. None is incomplete.
    pub plus_one_counter_multiplier: Option<u32>,
}

impl PermanentState {
    pub fn creature(
        object_ref: ObjectRef,
        owner: PlayerId,
        controller: PlayerId,
        base_power: i32,
    ) -> Self {
        Self {
            object_ref,
            owner,
            controller,
            is_creature: true,
            is_token: false,
            tapped: false,
            base_power: Some(base_power),
            counters: BTreeMap::new(),
            targetable_by: Some((PlayerId::MIN..=PlayerId::MAX).collect()),
            can_be_sacrificed_for_effect: Some(true),
            can_untap: Some(true),
            plus_one_counter_multiplier: Some(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneObject {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub is_token: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub in_game: bool,
    pub life: i32,
    /// None means life-change restrictions are not fully known.
    pub can_lose_life: Option<bool>,
    /// The top card is the final element.
    pub library: Vec<ZoneObject>,
    pub graveyard: Vec<ZoneObject>,
    pub exile: Vec<ZoneObject>,
}

impl PlayerState {
    pub fn new(life: i32) -> Self {
        Self {
            in_game: true,
            life,
            can_lose_life: Some(true),
            library: Vec::new(),
            graveyard: Vec::new(),
            exile: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentAttacker {
    pub attacker: ObjectRef,
    pub controller: PlayerId,
    pub defender: Defender,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentCombatState {
    pub combat_id: CombatId,
    pub turn_id: TurnId,
    pub attackers: BTreeMap<ObjectRef, CurrentAttacker>,
    pub blockers_by_attacker: BTreeMap<ObjectRef, BTreeSet<ObjectRef>>,
    pub attackers_complete: bool,
    pub blockers_complete: bool,
    pub simultaneous_groups_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntilEndOfTurnModifier {
    pub recipient: ObjectRef,
    pub power_delta: i32,
    pub toughness_delta: i32,
    pub expires_after_turn: TurnId,
    pub originating_trigger: TriggerId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvokeBlockRequirement {
    pub combat_id: CombatId,
    pub turn_id: TurnId,
    pub blocker: ObjectRef,
    pub attacker: ObjectRef,
    pub originating_trigger: TriggerId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatRuntimeState {
    pub turn_id: TurnId,
    pub players: BTreeMap<PlayerId, PlayerState>,
    pub opponents: BTreeMap<PlayerId, BTreeSet<PlayerId>>,
    pub permanents: BTreeMap<ObjectRef, PermanentState>,
    pub combat: Option<CurrentCombatState>,
    pub continuous_modifiers: Vec<UntilEndOfTurnModifier>,
    pub block_requirements: Vec<ProvokeBlockRequirement>,
    pub players_complete: bool,
    pub opponent_relations_complete: bool,
    pub battlefield_complete: bool,
    pub current_characteristics_complete: bool,
    pub complete_libraries: BTreeSet<PlayerId>,
    pub state_based_actions_pending: bool,
}

impl CombatRuntimeState {
    pub fn effective_power(&self, object_ref: ObjectRef) -> Result<i32, CombatTriggerRuntimeError> {
        if !self.current_characteristics_complete {
            return Err(CombatTriggerRuntimeError::IncompleteEvidence(
                "current characteristics",
            ));
        }
        let permanent = self
            .permanents
            .get(&object_ref)
            .ok_or(CombatTriggerRuntimeError::MissingPermanent(object_ref))?;
        let mut power = permanent.base_power.ok_or(
            CombatTriggerRuntimeError::SourceCharacteristicsUnavailable(object_ref),
        )?;
        let counters = permanent
            .counters
            .get(&CounterKind::PlusOnePlusOne)
            .copied()
            .unwrap_or_default();
        power = power
            .checked_add(i32::try_from(counters).map_err(|_| {
                CombatTriggerRuntimeError::ArithmeticOverflow("reading +1/+1 counters")
            })?)
            .ok_or(CombatTriggerRuntimeError::ArithmeticOverflow(
                "applying +1/+1 counters",
            ))?;
        for modifier in self
            .continuous_modifiers
            .iter()
            .filter(|modifier| modifier.recipient == object_ref)
        {
            power = power.checked_add(modifier.power_delta).ok_or(
                CombatTriggerRuntimeError::ArithmeticOverflow("applying continuous power modifier"),
            )?;
        }
        Ok(power)
    }

    pub fn cleanup_turn(&mut self, ending_turn: TurnId) {
        self.continuous_modifiers
            .retain(|modifier| modifier.expires_after_turn != ending_turn);
        self.block_requirements
            .retain(|requirement| requirement.turn_id != ending_turn);
        if self
            .combat
            .as_ref()
            .is_some_and(|combat| combat.turn_id == ending_turn)
        {
            self.combat = None;
        }
    }

    /// Tokens move to the destination during resolution, then cease to exist
    /// only after resolution when state based actions are checked.
    pub fn apply_post_resolution_state_based_actions(
        &mut self,
    ) -> Result<Vec<StateBasedAction>, CombatTriggerRuntimeError> {
        if !self.state_based_actions_pending {
            return Ok(Vec::new());
        }
        let mut actions = Vec::new();
        for (player_id, player) in &mut self.players {
            player.graveyard.retain(|object| {
                if object.is_token {
                    actions.push(StateBasedAction::TokenCeasedToExist {
                        object: object.object_ref,
                        owner: *player_id,
                    });
                    false
                } else {
                    true
                }
            });
            player.exile.retain(|object| {
                if object.is_token {
                    actions.push(StateBasedAction::TokenCeasedToExist {
                        object: object.object_ref,
                        owner: *player_id,
                    });
                    false
                } else {
                    true
                }
            });
        }
        self.state_based_actions_pending = false;
        Ok(actions)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateBasedAction {
    TokenCeasedToExist { object: ObjectRef, owner: PlayerId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerResolutionChoice {
    None,
    Sacrifice(Vec<ObjectRef>),
    Provoke { require_block_if_able: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionEffect {
    LifeLost {
        player: PlayerId,
        amount: u32,
    },
    PermanentSacrificed {
        old_object: ObjectRef,
        graveyard_object: ObjectRef,
        owner: PlayerId,
        was_token: bool,
    },
    ContinuousModifierCreated(UntilEndOfTurnModifier),
    CounterAdded {
        permanent: ObjectRef,
        counter: CounterKind,
        amount: u32,
    },
    LibraryTopExiled {
        player: PlayerId,
        old_object: ObjectRef,
        exile_object: ObjectRef,
        owner: PlayerId,
    },
    PermanentUntapped {
        permanent: ObjectRef,
    },
    BlockRequirementCreated(ProvokeBlockRequirement),
    NoEffect(ResolutionNoEffectReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionNoEffectReason {
    PlayerLeftGame,
    LifeTotalCannotChange,
    SourceIsNoLongerThatPermanent,
    EmptyLibrary,
    NoAdditionalBlockers,
    OptionalEffectDeclined,
    NoCurrentRecipients,
    CounterAdditionPrevented,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerResolutionReport {
    pub trigger_id: TriggerId,
    pub effects: Vec<ResolutionEffect>,
}

fn ensure_resolution_boundary(
    program: &CombatTriggerKeywordProgram,
    trigger: &CombatKeywordTrigger,
    state: &CombatRuntimeState,
) -> Result<(), CombatTriggerRuntimeError> {
    if trigger.program_digest != program.semantic_digest()
        || trigger.payload.family() != program.kind().family()
    {
        return Err(CombatTriggerRuntimeError::ProgramTriggerMismatch);
    }
    if trigger.turn_id != state.turn_id {
        return Err(CombatTriggerRuntimeError::TurnBoundaryMismatch);
    }
    if state.state_based_actions_pending {
        return Err(CombatTriggerRuntimeError::StateBasedActionsPending);
    }
    if !state.players_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence("players"));
    }
    if !state.battlefield_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence("battlefield"));
    }
    Ok(())
}

fn require_current_combat<'a>(
    trigger: &CombatKeywordTrigger,
    state: &'a CombatRuntimeState,
) -> Result<&'a CurrentCombatState, CombatTriggerRuntimeError> {
    let combat = state
        .combat
        .as_ref()
        .ok_or(CombatTriggerRuntimeError::CombatBoundaryMismatch)?;
    if combat.combat_id != trigger.combat_id || combat.turn_id != trigger.turn_id {
        return Err(CombatTriggerRuntimeError::CombatBoundaryMismatch);
    }
    if !combat.simultaneous_groups_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "simultaneous combat groups",
        ));
    }
    Ok(combat)
}

fn require_no_choice(choice: &TriggerResolutionChoice) -> Result<(), CombatTriggerRuntimeError> {
    if matches!(choice, TriggerResolutionChoice::None) {
        Ok(())
    } else {
        Err(CombatTriggerRuntimeError::UnexpectedResolutionChoice)
    }
}

pub fn resolve_combat_keyword_trigger(
    program: &CombatTriggerKeywordProgram,
    trigger: &CombatKeywordTrigger,
    choice: TriggerResolutionChoice,
    state: &mut CombatRuntimeState,
) -> Result<TriggerResolutionReport, CombatTriggerRuntimeError> {
    ensure_resolution_boundary(program, trigger, state)?;
    let effects = match &trigger.payload {
        CombatKeywordTriggerPayload::Afflict {
            defending_player,
            amount,
        } => {
            require_no_choice(&choice)?;
            resolve_afflict(*defending_player, *amount, state)?
        }
        CombatKeywordTriggerPayload::Annihilator {
            defending_player,
            amount,
        } => {
            let TriggerResolutionChoice::Sacrifice(selection) = choice else {
                return Err(CombatTriggerRuntimeError::MissingResolutionChoice(
                    "defending player's sacrifice selection",
                ));
            };
            resolve_annihilator(*defending_player, *amount, selection, state)?
        }
        CombatKeywordTriggerPayload::BattleCry => {
            require_no_choice(&choice)?;
            resolve_battle_cry(trigger, state)?
        }
        CombatKeywordTriggerPayload::Dethrone => {
            require_no_choice(&choice)?;
            resolve_dethrone(trigger, state)?
        }
        CombatKeywordTriggerPayload::Ingest { damaged_player } => {
            require_no_choice(&choice)?;
            resolve_ingest(*damaged_player, state)?
        }
        CombatKeywordTriggerPayload::Melee { opponents_attacked } => {
            require_no_choice(&choice)?;
            resolve_melee(trigger, opponents_attacked, state)?
        }
        CombatKeywordTriggerPayload::Provoke {
            defending_player,
            target,
        } => {
            let TriggerResolutionChoice::Provoke {
                require_block_if_able,
            } = choice
            else {
                return Err(CombatTriggerRuntimeError::MissingResolutionChoice(
                    "provoke optional choice",
                ));
            };
            resolve_provoke(
                trigger,
                *defending_player,
                *target,
                require_block_if_able,
                state,
            )?
        }
        CombatKeywordTriggerPayload::Rampage { amount } => {
            require_no_choice(&choice)?;
            resolve_rampage(trigger, *amount, state)?
        }
    };
    Ok(TriggerResolutionReport {
        trigger_id: trigger.trigger_id,
        effects,
    })
}

fn resolve_afflict(
    defending_player: PlayerId,
    amount: u32,
    state: &mut CombatRuntimeState,
) -> Result<Vec<ResolutionEffect>, CombatTriggerRuntimeError> {
    let player = state
        .players
        .get_mut(&defending_player)
        .ok_or(CombatTriggerRuntimeError::MissingPlayer(defending_player))?;
    if !player.in_game {
        return Ok(vec![ResolutionEffect::NoEffect(
            ResolutionNoEffectReason::PlayerLeftGame,
        )]);
    }
    match player.can_lose_life {
        Some(true) => {}
        Some(false) => {
            return Ok(vec![ResolutionEffect::NoEffect(
                ResolutionNoEffectReason::LifeTotalCannotChange,
            )]);
        }
        None => {
            return Err(CombatTriggerRuntimeError::IncompleteEvidence(
                "life-change restrictions",
            ));
        }
    }
    let life_loss = i32::try_from(amount)
        .map_err(|_| CombatTriggerRuntimeError::ArithmeticOverflow("converting life loss"))?;
    player.life =
        player
            .life
            .checked_sub(life_loss)
            .ok_or(CombatTriggerRuntimeError::ArithmeticOverflow(
                "applying life loss",
            ))?;
    Ok(vec![ResolutionEffect::LifeLost {
        player: defending_player,
        amount,
    }])
}

fn resolve_annihilator(
    defending_player: PlayerId,
    amount: u32,
    selection: Vec<ObjectRef>,
    state: &mut CombatRuntimeState,
) -> Result<Vec<ResolutionEffect>, CombatTriggerRuntimeError> {
    let player = state
        .players
        .get(&defending_player)
        .ok_or(CombatTriggerRuntimeError::MissingPlayer(defending_player))?;
    if !player.in_game {
        if selection.is_empty() {
            return Ok(vec![ResolutionEffect::NoEffect(
                ResolutionNoEffectReason::PlayerLeftGame,
            )]);
        }
        return Err(CombatTriggerRuntimeError::InvalidSacrificeSelection(
            "a player outside the game cannot select permanents",
        ));
    }
    let controlled = state
        .permanents
        .values()
        .filter(|permanent| permanent.controller == defending_player)
        .collect::<Vec<_>>();
    if controlled
        .iter()
        .any(|permanent| permanent.can_be_sacrificed_for_effect.is_none())
    {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "sacrifice restrictions",
        ));
    }
    let eligible = controlled
        .into_iter()
        .filter(|permanent| permanent.can_be_sacrificed_for_effect == Some(true))
        .map(|permanent| permanent.object_ref)
        .collect::<BTreeSet<_>>();
    let required = usize::try_from(amount)
        .unwrap_or(usize::MAX)
        .min(eligible.len());
    if selection.len() != required {
        return Err(CombatTriggerRuntimeError::InvalidSacrificeSelection(
            "selection must contain the requested number or every available permanent",
        ));
    }
    let selected = selection.iter().copied().collect::<BTreeSet<_>>();
    if selected.len() != selection.len() {
        let duplicate = selection
            .iter()
            .copied()
            .find(|object| {
                selection
                    .iter()
                    .filter(|candidate| *candidate == object)
                    .count()
                    > 1
            })
            .expect("duplicate exists");
        return Err(CombatTriggerRuntimeError::DuplicateObjectSelection(
            duplicate,
        ));
    }
    if !selected.is_subset(&eligible) {
        return Err(CombatTriggerRuntimeError::InvalidSacrificeSelection(
            "every selected object must be a current permanent that player controls",
        ));
    }
    if eligible.len() < usize::try_from(amount).unwrap_or(usize::MAX) && selected != eligible {
        return Err(CombatTriggerRuntimeError::InvalidSacrificeSelection(
            "when fewer permanents exist, all of them must be selected",
        ));
    }

    let plans = selection
        .iter()
        .map(|object_ref| {
            let permanent = state
                .permanents
                .get(object_ref)
                .ok_or(CombatTriggerRuntimeError::MissingPermanent(*object_ref))?;
            let graveyard_ref = permanent.object_ref.next_incarnation()?;
            if state.permanents.contains_key(&graveyard_ref)
                || state.players.values().any(|player| {
                    player
                        .library
                        .iter()
                        .chain(&player.graveyard)
                        .chain(&player.exile)
                        .any(|object| object.object_ref == graveyard_ref)
                })
            {
                return Err(CombatTriggerRuntimeError::ZoneObjectCollision(
                    graveyard_ref,
                ));
            }
            if !state.players.contains_key(&permanent.owner) {
                return Err(CombatTriggerRuntimeError::MissingPlayer(permanent.owner));
            }
            Ok((*object_ref, graveyard_ref, permanent.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut moved = Vec::with_capacity(plans.len());
    for (object_ref, graveyard_ref, planned) in plans {
        let permanent = state
            .permanents
            .remove(&object_ref)
            .ok_or(CombatTriggerRuntimeError::MissingPermanent(object_ref))?;
        debug_assert_eq!(permanent, planned);
        moved.push((permanent, graveyard_ref));
    }

    let mut effects = Vec::with_capacity(moved.len());
    for (permanent, graveyard_ref) in moved {
        let owner = state
            .players
            .get_mut(&permanent.owner)
            .expect("owners were validated before simultaneous sacrifice");
        owner.graveyard.push(ZoneObject {
            object_ref: graveyard_ref,
            owner: permanent.owner,
            is_token: permanent.is_token,
        });
        effects.push(ResolutionEffect::PermanentSacrificed {
            old_object: permanent.object_ref,
            graveyard_object: graveyard_ref,
            owner: permanent.owner,
            was_token: permanent.is_token,
        });
        if permanent.is_token {
            state.state_based_actions_pending = true;
        }
    }
    Ok(effects)
}

fn add_modifier(
    state: &mut CombatRuntimeState,
    modifier: UntilEndOfTurnModifier,
    effects: &mut Vec<ResolutionEffect>,
) {
    state.continuous_modifiers.push(modifier.clone());
    effects.push(ResolutionEffect::ContinuousModifierCreated(modifier));
}

fn resolve_battle_cry(
    trigger: &CombatKeywordTrigger,
    state: &mut CombatRuntimeState,
) -> Result<Vec<ResolutionEffect>, CombatTriggerRuntimeError> {
    let recipients = {
        let combat = require_current_combat(trigger, state)?;
        if !combat.attackers_complete {
            return Err(CombatTriggerRuntimeError::IncompleteEvidence(
                "current attackers",
            ));
        }
        combat
            .attackers
            .keys()
            .copied()
            .filter(|attacker| *attacker != trigger.source_lki.object_ref)
            .collect::<Vec<_>>()
    };
    let recipients = recipients
        .into_iter()
        .filter(|recipient| state.permanents.contains_key(recipient))
        .collect::<Vec<_>>();
    if recipients.is_empty() {
        return Ok(vec![ResolutionEffect::NoEffect(
            ResolutionNoEffectReason::NoCurrentRecipients,
        )]);
    }
    let mut effects = Vec::with_capacity(recipients.len());
    for recipient in recipients {
        add_modifier(
            state,
            UntilEndOfTurnModifier {
                recipient,
                power_delta: 1,
                toughness_delta: 0,
                expires_after_turn: trigger.turn_id,
                originating_trigger: trigger.trigger_id,
            },
            &mut effects,
        );
    }
    Ok(effects)
}

fn resolve_dethrone(
    trigger: &CombatKeywordTrigger,
    state: &mut CombatRuntimeState,
) -> Result<Vec<ResolutionEffect>, CombatTriggerRuntimeError> {
    let Some(permanent) = state.permanents.get_mut(&trigger.source_lki.object_ref) else {
        return Ok(vec![ResolutionEffect::NoEffect(
            ResolutionNoEffectReason::SourceIsNoLongerThatPermanent,
        )]);
    };
    let multiplier = permanent.plus_one_counter_multiplier.ok_or(
        CombatTriggerRuntimeError::IncompleteEvidence("+1/+1 counter replacement effects"),
    )?;
    if multiplier == 0 {
        return Ok(vec![ResolutionEffect::NoEffect(
            ResolutionNoEffectReason::CounterAdditionPrevented,
        )]);
    }
    let current = permanent
        .counters
        .get(&CounterKind::PlusOnePlusOne)
        .copied()
        .unwrap_or_default();
    let updated =
        current
            .checked_add(multiplier)
            .ok_or(CombatTriggerRuntimeError::ArithmeticOverflow(
                "adding dethrone counter",
            ))?;
    permanent
        .counters
        .insert(CounterKind::PlusOnePlusOne, updated);
    Ok(vec![ResolutionEffect::CounterAdded {
        permanent: trigger.source_lki.object_ref,
        counter: CounterKind::PlusOnePlusOne,
        amount: multiplier,
    }])
}

fn resolve_ingest(
    damaged_player: PlayerId,
    state: &mut CombatRuntimeState,
) -> Result<Vec<ResolutionEffect>, CombatTriggerRuntimeError> {
    if !state.complete_libraries.contains(&damaged_player) {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "damaged player's library",
        ));
    }
    let player = state
        .players
        .get(&damaged_player)
        .ok_or(CombatTriggerRuntimeError::MissingPlayer(damaged_player))?;
    if !player.in_game {
        return Ok(vec![ResolutionEffect::NoEffect(
            ResolutionNoEffectReason::PlayerLeftGame,
        )]);
    }
    let Some(top) = player.library.last() else {
        return Ok(vec![ResolutionEffect::NoEffect(
            ResolutionNoEffectReason::EmptyLibrary,
        )]);
    };
    let exile_ref = top.object_ref.next_incarnation()?;
    if player
        .library
        .iter()
        .chain(&player.graveyard)
        .chain(&player.exile)
        .any(|object| object.object_ref == exile_ref)
        || state.permanents.contains_key(&exile_ref)
    {
        return Err(CombatTriggerRuntimeError::ZoneObjectCollision(exile_ref));
    }
    let player = state
        .players
        .get_mut(&damaged_player)
        .expect("damaged player was validated before zone movement");
    let top = player
        .library
        .pop()
        .expect("library top was validated before zone movement");
    player.exile.push(ZoneObject {
        object_ref: exile_ref,
        owner: top.owner,
        is_token: top.is_token,
    });
    if top.is_token {
        state.state_based_actions_pending = true;
    }
    Ok(vec![ResolutionEffect::LibraryTopExiled {
        player: damaged_player,
        old_object: top.object_ref,
        exile_object: exile_ref,
        owner: top.owner,
    }])
}

fn resolve_melee(
    trigger: &CombatKeywordTrigger,
    opponents_attacked: &BTreeSet<PlayerId>,
    state: &mut CombatRuntimeState,
) -> Result<Vec<ResolutionEffect>, CombatTriggerRuntimeError> {
    if !state
        .permanents
        .contains_key(&trigger.source_lki.object_ref)
    {
        return Ok(vec![ResolutionEffect::NoEffect(
            ResolutionNoEffectReason::SourceIsNoLongerThatPermanent,
        )]);
    }
    if !state.opponent_relations_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "resolution-time opponent relations",
        ));
    }
    let current_opponents = state
        .opponents
        .get(&trigger.controller)
        .ok_or(CombatTriggerRuntimeError::MissingPlayer(trigger.controller))?;
    let amount = opponents_attacked
        .iter()
        .filter(|opponent| {
            current_opponents.contains(opponent)
                && state
                    .players
                    .get(opponent)
                    .is_some_and(|player| player.in_game)
        })
        .count();
    let amount = i32::try_from(amount)
        .map_err(|_| CombatTriggerRuntimeError::ArithmeticOverflow("counting melee opponents"))?;
    let modifier = UntilEndOfTurnModifier {
        recipient: trigger.source_lki.object_ref,
        power_delta: amount,
        toughness_delta: amount,
        expires_after_turn: trigger.turn_id,
        originating_trigger: trigger.trigger_id,
    };
    state.continuous_modifiers.push(modifier.clone());
    Ok(vec![ResolutionEffect::ContinuousModifierCreated(modifier)])
}

fn resolve_provoke(
    trigger: &CombatKeywordTrigger,
    defending_player: PlayerId,
    target: ObjectRef,
    require_block_if_able: bool,
    state: &mut CombatRuntimeState,
) -> Result<Vec<ResolutionEffect>, CombatTriggerRuntimeError> {
    let target_state = state
        .permanents
        .get(&target)
        .ok_or(CombatTriggerRuntimeError::IllegalTargetAtResolution(target))?;
    let targetable_by = target_state.targetable_by.as_ref().ok_or(
        CombatTriggerRuntimeError::IncompleteEvidence("resolution-time target legality"),
    )?;
    if target_state.controller != defending_player
        || !target_state.is_creature
        || !targetable_by.contains(&trigger.controller)
    {
        return Err(CombatTriggerRuntimeError::IllegalTargetAtResolution(target));
    }
    if !require_block_if_able {
        return Ok(vec![ResolutionEffect::NoEffect(
            ResolutionNoEffectReason::OptionalEffectDeclined,
        )]);
    }
    let target_state = state
        .permanents
        .get_mut(&target)
        .expect("target was just validated");
    let can_untap = target_state
        .can_untap
        .ok_or(CombatTriggerRuntimeError::IncompleteEvidence(
            "untap restrictions",
        ))?;
    let changed_tapped_status = target_state.tapped && can_untap;
    if changed_tapped_status {
        target_state.tapped = false;
    }
    let requirement = ProvokeBlockRequirement {
        combat_id: trigger.combat_id,
        turn_id: trigger.turn_id,
        blocker: target,
        attacker: trigger.source_lki.object_ref,
        originating_trigger: trigger.trigger_id,
    };
    state.block_requirements.push(requirement.clone());
    let mut effects = Vec::with_capacity(2);
    if changed_tapped_status {
        effects.push(ResolutionEffect::PermanentUntapped { permanent: target });
    }
    effects.push(ResolutionEffect::BlockRequirementCreated(requirement));
    Ok(effects)
}

fn resolve_rampage(
    trigger: &CombatKeywordTrigger,
    amount: u32,
    state: &mut CombatRuntimeState,
) -> Result<Vec<ResolutionEffect>, CombatTriggerRuntimeError> {
    if !state
        .permanents
        .contains_key(&trigger.source_lki.object_ref)
    {
        return Ok(vec![ResolutionEffect::NoEffect(
            ResolutionNoEffectReason::SourceIsNoLongerThatPermanent,
        )]);
    }
    let blocker_count = {
        let combat = require_current_combat(trigger, state)?;
        if !combat.blockers_complete {
            return Err(CombatTriggerRuntimeError::IncompleteEvidence(
                "current blockers",
            ));
        }
        combat
            .blockers_by_attacker
            .get(&trigger.source_lki.object_ref)
            .map(BTreeSet::len)
            .unwrap_or_default()
    };
    let beyond_first = blocker_count.saturating_sub(1);
    if beyond_first == 0 {
        return Ok(vec![ResolutionEffect::NoEffect(
            ResolutionNoEffectReason::NoAdditionalBlockers,
        )]);
    }
    let amount_i32 = i32::try_from(amount)
        .map_err(|_| CombatTriggerRuntimeError::ArithmeticOverflow("converting rampage amount"))?;
    let count_i32 = i32::try_from(beyond_first)
        .map_err(|_| CombatTriggerRuntimeError::ArithmeticOverflow("counting rampage blockers"))?;
    let delta =
        amount_i32
            .checked_mul(count_i32)
            .ok_or(CombatTriggerRuntimeError::ArithmeticOverflow(
                "multiplying rampage bonus",
            ))?;
    let modifier = UntilEndOfTurnModifier {
        recipient: trigger.source_lki.object_ref,
        power_delta: delta,
        toughness_delta: delta,
        expires_after_turn: trigger.turn_id,
        originating_trigger: trigger.trigger_id,
    };
    state.continuous_modifiers.push(modifier.clone());
    Ok(vec![ResolutionEffect::ContinuousModifierCreated(modifier)])
}

/// Checks skulk against the current power of every creature proposed to block
/// this attacker. The broader declaration solver must still check every other
/// restriction, requirement, capacity, cost, and defending player rule.
pub fn skulk_allows_block_group(
    program: &CombatTriggerKeywordProgram,
    attacker: ObjectRef,
    blockers: &[ObjectRef],
    state: &CombatRuntimeState,
) -> Result<bool, CombatTriggerRuntimeError> {
    if !matches!(program.kind(), CombatTriggerKeywordKind::Skulk) {
        return Err(CombatTriggerRuntimeError::WrongEventForKeyword {
            family: program.kind().family(),
        });
    }
    if !state.battlefield_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence("battlefield"));
    }
    let combat = state
        .combat
        .as_ref()
        .ok_or(CombatTriggerRuntimeError::CombatBoundaryMismatch)?;
    if !combat.attackers_complete || !combat.simultaneous_groups_complete {
        return Err(CombatTriggerRuntimeError::IncompleteEvidence(
            "current simultaneous combat group",
        ));
    }
    if !combat.attackers.contains_key(&attacker) {
        return Err(CombatTriggerRuntimeError::SourceDidNotAttack { source: attacker });
    }
    let attacker_power = state
        .effective_power(attacker)
        .map_err(|_| CombatTriggerRuntimeError::SourceCharacteristicsUnavailable(attacker))?;
    for blocker in blockers {
        let permanent = state
            .permanents
            .get(blocker)
            .ok_or(CombatTriggerRuntimeError::MissingPermanent(*blocker))?;
        if !permanent.is_creature {
            return Ok(false);
        }
        let blocker_power = state
            .effective_power(*blocker)
            .map_err(|_| CombatTriggerRuntimeError::BlockerCharacteristicsUnavailable(*blocker))?;
        if blocker_power > attacker_power {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockAssignment {
    pub blocker: ObjectRef,
    pub attacker: ObjectRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateBlockDeclaration {
    pub assignments: BTreeSet<BlockAssignment>,
    /// True only after all restrictions, capacities, defending player rules,
    /// and required costs outside this module have been checked.
    pub otherwise_legal: bool,
    /// Number of non-provoke blocking requirements obeyed by this declaration.
    /// The upstream declaration solver supplies this from complete evidence.
    pub other_requirements_satisfied: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExhaustiveBlockDeclarationEvidence {
    pub actual: CandidateBlockDeclaration,
    pub legal_candidates: Vec<CandidateBlockDeclaration>,
    pub legal_candidate_set_complete: bool,
}

fn satisfied_provoke_requirements(
    declaration: &CandidateBlockDeclaration,
    requirements: &[ProvokeBlockRequirement],
) -> usize {
    requirements
        .iter()
        .filter(|requirement| {
            declaration.assignments.contains(&BlockAssignment {
                blocker: requirement.blocker,
                attacker: requirement.attacker,
            })
        })
        .count()
}

fn total_requirements_satisfied(
    declaration: &CandidateBlockDeclaration,
    requirements: &[ProvokeBlockRequirement],
) -> Result<usize, CombatTriggerRuntimeError> {
    declaration
        .other_requirements_satisfied
        .checked_add(satisfied_provoke_requirements(declaration, requirements))
        .ok_or(CombatTriggerRuntimeError::ArithmeticOverflow(
            "counting blocking requirements",
        ))
}

/// Enforces the "if able" part of provoke using an exhaustive set of block
/// declarations already proven legal with respect to every restriction. This
/// preserves the maximum-requirements rule when several simultaneous provoke
/// requirements or unrelated block requirements conflict.
pub fn provoke_requirements_allow_declaration(
    combat_id: CombatId,
    evidence: &ExhaustiveBlockDeclarationEvidence,
    state: &CombatRuntimeState,
) -> Result<bool, CombatTriggerRuntimeError> {
    if !evidence.legal_candidate_set_complete {
        return Err(CombatTriggerRuntimeError::IncompleteLegalDeclarationSet);
    }
    let requirements = state
        .block_requirements
        .iter()
        .filter(|requirement| requirement.combat_id == combat_id)
        .cloned()
        .collect::<Vec<_>>();
    let legal = evidence
        .legal_candidates
        .iter()
        .filter(|candidate| candidate.otherwise_legal)
        .collect::<Vec<_>>();
    if !evidence.actual.otherwise_legal || !legal.contains(&&evidence.actual) {
        return Err(CombatTriggerRuntimeError::ActualDeclarationNotInLegalSet);
    }
    let maximum = legal
        .iter()
        .map(|candidate| total_requirements_satisfied(candidate, &requirements))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max()
        .unwrap_or_default();
    Ok(total_requirements_satisfied(&evidence.actual, &requirements)? == maximum)
}
