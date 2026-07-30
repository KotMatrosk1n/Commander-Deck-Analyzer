//! Exact, content keyed programs for reviewed static and special procedures.
//!
//! This module owns only complete standalone clauses for Job select, For
//! Mirrodin!, Living metal, Banding, Phasing, Enlist, Training, Hidden agenda,
//! Double agenda, Double team, and the face-up draft instruction. Ability
//! grants, compounds, named variants, partial reminder text outside the
//! reviewed Banding form, and clauses without required source-type evidence
//! remain rejected.
//!
//! Program identity contains exact Oracle content, the smallest relevant
//! semantic source context, and versioned compiler and rules contracts. It
//! never contains card names, card identifiers, database rows, snapshot
//! metadata, clause addresses, or memory locations. Recognition is not live
//! production coverage. The standalone runtimes below require complete
//! transaction evidence and deliberately have no production adapter.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const STATIC_SPECIAL_KEYWORD_COMPILER_VERSION: &str = "static-special-keyword-compiler-0.1";
pub const STATIC_SPECIAL_KEYWORD_RUNTIME_VERSION: &str = "static-special-keyword-runtime-0.1";

const JOB_SELECT_RULES_CONTEXT: &str = "magic-rules-2026-06-19:job-select-entry-token-attach-v1";
const FOR_MIRRODIN_RULES_CONTEXT: &str =
    "magic-rules-2026-06-19:for-mirrodin-entry-token-attach-v1";
const LIVING_METAL_RULES_CONTEXT: &str =
    "magic-comprehensive-rules-2026-06-19:continuous-effects,613,living-metal";
const BANDING_RULES_CONTEXT: &str = "magic-comprehensive-rules-2026-06-19:506,508,509,510,702.22";
const PHASING_RULES_CONTEXT: &str = "magic-comprehensive-rules-2026-06-19:110.5,400.7,502,702.26";
const ENLIST_RULES_CONTEXT: &str = "magic-comprehensive-rules-2026-06-19:508,603.12,702.154";
const TRAINING_RULES_CONTEXT: &str = "magic-comprehensive-rules-2026-06-19:508,603,702.149";
const AGENDA_RULES_CONTEXT: &str =
    "magic-comprehensive-rules-2026-06-19:103,116.2e,313,702.106,905";
const DOUBLE_TEAM_RULES_CONTEXT: &str =
    "magic-digital-rules-2026-06-19:conjure,duplicate,perpetual,double-team-v1";
const FACE_UP_DRAFT_RULES_CONTEXT: &str =
    "magic-comprehensive-rules-2026-06-19:draft,905,face-up-draft-v1";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type EventId = u64;
pub type TriggerId = u64;
pub type CombatId = u64;
pub type TurnId = u64;
pub type DigitalCardId = u64;
pub type DraftCardId = u64;

pub const fn static_special_keyword_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StaticSpecialKeywordFamily {
    JobSelect,
    ForMirrodin,
    LivingMetal,
    Banding,
    Phasing,
    Enlist,
    Training,
    HiddenAgenda,
    DoubleAgenda,
    DoubleTeam,
    DraftFaceUp,
}

impl StaticSpecialKeywordFamily {
    pub const fn printed_label(self) -> &'static str {
        match self {
            Self::JobSelect => "Job select",
            Self::ForMirrodin => "For Mirrodin!",
            Self::LivingMetal => "Living metal",
            Self::Banding => "Banding",
            Self::Phasing => "Phasing",
            Self::Enlist => "Enlist",
            Self::Training => "Training",
            Self::HiddenAgenda => "Hidden agenda",
            Self::DoubleAgenda => "Double agenda",
            Self::DoubleTeam => "Double team",
            Self::DraftFaceUp => "Draft this card face up",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StaticSpecialSourceContext {
    pub is_artifact: bool,
    pub is_creature: bool,
    pub is_enchantment: bool,
    pub is_land: bool,
    pub is_planeswalker: bool,
    pub is_battle: bool,
    pub is_conspiracy: bool,
    pub is_equipment: bool,
    pub is_vehicle: bool,
}

impl StaticSpecialSourceContext {
    pub fn from_type_line(type_line: &str) -> Self {
        let words = type_line
            .split(|character: char| !character.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(|word| word.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        Self {
            is_artifact: words.contains("artifact"),
            is_creature: words.contains("creature"),
            is_enchantment: words.contains("enchantment"),
            is_land: words.contains("land"),
            is_planeswalker: words.contains("planeswalker"),
            is_battle: words.contains("battle"),
            is_conspiracy: words.contains("conspiracy"),
            is_equipment: words.contains("equipment"),
            is_vehicle: words.contains("vehicle"),
        }
    }

    pub const fn is_permanent_card(self) -> bool {
        self.is_artifact
            || self.is_creature
            || self.is_enchantment
            || self.is_land
            || self.is_planeswalker
            || self.is_battle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryAttachmentTokenKind {
    ColorlessHeroOneOne,
    RedRebelTwoTwo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticSpecialKeywordKind {
    EntryAttachment { token: EntryAttachmentTokenKind },
    LivingMetal,
    Banding,
    Phasing,
    Enlist,
    Training,
    Agenda { secret_name_count: u8 },
    DoubleTeam,
    DraftFaceUp,
}

impl StaticSpecialKeywordKind {
    pub const fn family(self) -> StaticSpecialKeywordFamily {
        match self {
            Self::EntryAttachment {
                token: EntryAttachmentTokenKind::ColorlessHeroOneOne,
            } => StaticSpecialKeywordFamily::JobSelect,
            Self::EntryAttachment {
                token: EntryAttachmentTokenKind::RedRebelTwoTwo,
            } => StaticSpecialKeywordFamily::ForMirrodin,
            Self::LivingMetal => StaticSpecialKeywordFamily::LivingMetal,
            Self::Banding => StaticSpecialKeywordFamily::Banding,
            Self::Phasing => StaticSpecialKeywordFamily::Phasing,
            Self::Enlist => StaticSpecialKeywordFamily::Enlist,
            Self::Training => StaticSpecialKeywordFamily::Training,
            Self::Agenda {
                secret_name_count: 1,
            } => StaticSpecialKeywordFamily::HiddenAgenda,
            Self::Agenda {
                secret_name_count: 2,
            } => StaticSpecialKeywordFamily::DoubleAgenda,
            Self::Agenda { .. } => StaticSpecialKeywordFamily::HiddenAgenda,
            Self::DoubleTeam => StaticSpecialKeywordFamily::DoubleTeam,
            Self::DraftFaceUp => StaticSpecialKeywordFamily::DraftFaceUp,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticSpecialKeywordProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    kind: StaticSpecialKeywordKind,
}

impl StaticSpecialKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub const fn kind(&self) -> StaticSpecialKeywordKind {
        self.kind
    }

    pub const fn family(&self) -> StaticSpecialKeywordFamily {
        self.kind.family()
    }

    pub const fn production_adapter_connected(&self) -> bool {
        static_special_keyword_production_adapter_connected()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticSpecialClauseClassification {
    Program(StaticSpecialKeywordProgram),
    Rejected,
}

const JOB_SELECT_CANONICAL: &str = "Job select (When this Equipment enters, create a 1/1 \
    colorless Hero creature token, then attach this to it.)";
const FOR_MIRRODIN_CANONICAL: &str = "For Mirrodin! (When this Equipment enters, create a 2/2 \
    red Rebel creature token, then attach this to it.)";
const LIVING_METAL_CANONICAL: &str =
    "Living metal (During your turn, this Vehicle is also a creature.)";
const BANDING_CANONICAL: &str = "Banding (Any creatures with banding, and up to one without, can \
    attack in a band. Bands are blocked as a group. If any creatures with banding you control \
    are blocking or being blocked by a creature, you divide that creature's combat damage, not \
    its controller, among any of the creatures it's being blocked by or is blocking.)";
const BANDING_BLOCKING_REMINDER: &str = "Banding (If any creatures with banding you control are \
    blocking a creature, you divide that creature's combat damage, not its controller, among any \
    of the creatures it's being blocked by.)";
const PHASING_CANONICAL: &str = "Phasing (This phases in or out before you untap during each of \
    your untap steps. While it's phased out, it's treated as though it doesn't exist.)";
const ENLIST_CANONICAL: &str = "Enlist (As this creature attacks, you may tap a nonattacking \
    creature you control without summoning sickness. When you do, add its power to this \
    creature's until end of turn.)";
const TRAINING_CANONICAL: &str = "Training (Whenever this creature attacks with another creature \
    with greater power, put a +1/+1 counter on this creature.)";
const HIDDEN_AGENDA_CANONICAL: &str = "Hidden agenda (Start the game with this conspiracy face \
    down in the command zone and secretly choose a card name. You may turn this conspiracy face \
    up any time and reveal that name.)";
const DOUBLE_AGENDA_CANONICAL: &str = "Double agenda (Start the game with this conspiracy face \
    down in the command zone and secretly choose two different card names. You may turn this \
    conspiracy face up any time and reveal those names.)";
const DOUBLE_TEAM_CANONICAL: &str = "Double team (When this creature attacks, conjure a duplicate \
    into your hand, then both of them perpetually lose double team.)";
const DRAFT_FACE_UP_CANONICAL: &str = "Draft this card face up.";

pub fn compile_static_special_keyword_program(
    exact_source: &str,
    normalized_source: &str,
    source_context: StaticSpecialSourceContext,
) -> Option<StaticSpecialKeywordProgram> {
    match classify_static_special_keyword_clause(exact_source, normalized_source, source_context) {
        StaticSpecialClauseClassification::Program(program) => Some(program),
        StaticSpecialClauseClassification::Rejected => None,
    }
}

pub fn classify_static_special_keyword_clause(
    exact_source: &str,
    normalized_source: &str,
    source_context: StaticSpecialSourceContext,
) -> StaticSpecialClauseClassification {
    if !is_complete_single_line(exact_source)
        || !is_complete_single_line(normalized_source)
        || !normalized_source_is_content_derived(exact_source, normalized_source)
    {
        return StaticSpecialClauseClassification::Rejected;
    }

    let kind = match exact_source {
        "Job select" | JOB_SELECT_CANONICAL if source_context.is_equipment => {
            Some(StaticSpecialKeywordKind::EntryAttachment {
                token: EntryAttachmentTokenKind::ColorlessHeroOneOne,
            })
        }
        "For Mirrodin!" | FOR_MIRRODIN_CANONICAL if source_context.is_equipment => {
            Some(StaticSpecialKeywordKind::EntryAttachment {
                token: EntryAttachmentTokenKind::RedRebelTwoTwo,
            })
        }
        "Living metal" | LIVING_METAL_CANONICAL if source_context.is_vehicle => {
            Some(StaticSpecialKeywordKind::LivingMetal)
        }
        "Banding" | BANDING_CANONICAL | BANDING_BLOCKING_REMINDER if source_context.is_creature => {
            Some(StaticSpecialKeywordKind::Banding)
        }
        "Phasing" | PHASING_CANONICAL if source_context.is_permanent_card() => {
            Some(StaticSpecialKeywordKind::Phasing)
        }
        "Enlist" | ENLIST_CANONICAL if source_context.is_creature => {
            Some(StaticSpecialKeywordKind::Enlist)
        }
        "Training" | TRAINING_CANONICAL if source_context.is_creature => {
            Some(StaticSpecialKeywordKind::Training)
        }
        "Hidden agenda" | HIDDEN_AGENDA_CANONICAL if source_context.is_conspiracy => {
            Some(StaticSpecialKeywordKind::Agenda {
                secret_name_count: 1,
            })
        }
        "Double agenda" | DOUBLE_AGENDA_CANONICAL if source_context.is_conspiracy => {
            Some(StaticSpecialKeywordKind::Agenda {
                secret_name_count: 2,
            })
        }
        "Double team" | DOUBLE_TEAM_CANONICAL if source_context.is_creature => {
            Some(StaticSpecialKeywordKind::DoubleTeam)
        }
        DRAFT_FACE_UP_CANONICAL => Some(StaticSpecialKeywordKind::DraftFaceUp),
        _ => None,
    };

    let Some(kind) = kind else {
        return StaticSpecialClauseClassification::Rejected;
    };
    let semantic_digest = static_special_semantic_digest(exact_source, kind, source_context);
    StaticSpecialClauseClassification::Program(StaticSpecialKeywordProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        semantic_digest,
        kind,
    })
}

fn static_special_semantic_digest(
    exact_source: &str,
    kind: StaticSpecialKeywordKind,
    source_context: StaticSpecialSourceContext,
) -> String {
    let mut digest = Sha256::new();
    digest_field(&mut digest, "semantic-schema", "static-special-keyword-v1");
    digest_field(
        &mut digest,
        "compiler",
        STATIC_SPECIAL_KEYWORD_COMPILER_VERSION,
    );
    digest_field(
        &mut digest,
        "runtime",
        STATIC_SPECIAL_KEYWORD_RUNTIME_VERSION,
    );
    digest_field(&mut digest, "rules", rules_context(kind));
    digest_field(&mut digest, "oracle", exact_source);
    digest_field(&mut digest, "kind", semantic_kind_key(kind));
    digest_field(
        &mut digest,
        "relevant-source-context",
        relevant_source_context_key(kind, source_context),
    );
    format!("{:X}", digest.finalize())
}

fn digest_field(digest: &mut Sha256, label: &str, value: &str) {
    digest.update(label.len().to_le_bytes());
    digest.update(label.as_bytes());
    digest.update(value.len().to_le_bytes());
    digest.update(value.as_bytes());
}

const fn rules_context(kind: StaticSpecialKeywordKind) -> &'static str {
    match kind.family() {
        StaticSpecialKeywordFamily::JobSelect => JOB_SELECT_RULES_CONTEXT,
        StaticSpecialKeywordFamily::ForMirrodin => FOR_MIRRODIN_RULES_CONTEXT,
        StaticSpecialKeywordFamily::LivingMetal => LIVING_METAL_RULES_CONTEXT,
        StaticSpecialKeywordFamily::Banding => BANDING_RULES_CONTEXT,
        StaticSpecialKeywordFamily::Phasing => PHASING_RULES_CONTEXT,
        StaticSpecialKeywordFamily::Enlist => ENLIST_RULES_CONTEXT,
        StaticSpecialKeywordFamily::Training => TRAINING_RULES_CONTEXT,
        StaticSpecialKeywordFamily::HiddenAgenda | StaticSpecialKeywordFamily::DoubleAgenda => {
            AGENDA_RULES_CONTEXT
        }
        StaticSpecialKeywordFamily::DoubleTeam => DOUBLE_TEAM_RULES_CONTEXT,
        StaticSpecialKeywordFamily::DraftFaceUp => FACE_UP_DRAFT_RULES_CONTEXT,
    }
}

const fn semantic_kind_key(kind: StaticSpecialKeywordKind) -> &'static str {
    match kind {
        StaticSpecialKeywordKind::EntryAttachment {
            token: EntryAttachmentTokenKind::ColorlessHeroOneOne,
        } => "entry-attachment:hero:1/1:colorless",
        StaticSpecialKeywordKind::EntryAttachment {
            token: EntryAttachmentTokenKind::RedRebelTwoTwo,
        } => "entry-attachment:rebel:2/2:red",
        StaticSpecialKeywordKind::LivingMetal => "living-metal",
        StaticSpecialKeywordKind::Banding => "banding",
        StaticSpecialKeywordKind::Phasing => "phasing",
        StaticSpecialKeywordKind::Enlist => "enlist",
        StaticSpecialKeywordKind::Training => "training",
        StaticSpecialKeywordKind::Agenda {
            secret_name_count: 1,
        } => "agenda:one-secret-name",
        StaticSpecialKeywordKind::Agenda {
            secret_name_count: 2,
        } => "agenda:two-distinct-secret-names",
        StaticSpecialKeywordKind::Agenda { .. } => "agenda:invalid",
        StaticSpecialKeywordKind::DoubleTeam => "double-team",
        StaticSpecialKeywordKind::DraftFaceUp => "draft-face-up",
    }
}

const fn relevant_source_context_key(
    kind: StaticSpecialKeywordKind,
    source_context: StaticSpecialSourceContext,
) -> &'static str {
    match kind.family() {
        StaticSpecialKeywordFamily::JobSelect | StaticSpecialKeywordFamily::ForMirrodin => {
            if source_context.is_equipment {
                "equipment=true"
            } else {
                "equipment=false"
            }
        }
        StaticSpecialKeywordFamily::LivingMetal => {
            if source_context.is_vehicle {
                "vehicle=true"
            } else {
                "vehicle=false"
            }
        }
        StaticSpecialKeywordFamily::Banding
        | StaticSpecialKeywordFamily::Enlist
        | StaticSpecialKeywordFamily::Training
        | StaticSpecialKeywordFamily::DoubleTeam => {
            if source_context.is_creature {
                "creature=true"
            } else {
                "creature=false"
            }
        }
        StaticSpecialKeywordFamily::Phasing => {
            if source_context.is_permanent_card() {
                "permanent-card=true"
            } else {
                "permanent-card=false"
            }
        }
        StaticSpecialKeywordFamily::HiddenAgenda | StaticSpecialKeywordFamily::DoubleAgenda => {
            if source_context.is_conspiracy {
                "conspiracy=true"
            } else {
                "conspiracy=false"
            }
        }
        StaticSpecialKeywordFamily::DraftFaceUp => "no-source-context",
    }
}

fn is_complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source.trim() == source
        && !source.contains(['\r', '\n'])
        && collapse_whitespace(source) == source
}

fn normalized_source_is_content_derived(exact_source: &str, normalized_source: &str) -> bool {
    normalized_source == exact_source
        || normalized_source == reviewed_static_special_normalized_source(exact_source)
}

pub fn reviewed_static_special_normalized_source(source: &str) -> String {
    collapse_whitespace(source)
        .replace(['\u{2018}', '\u{2019}'], "'")
        .to_lowercase()
}

fn collapse_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn contains_keyword_word(source: &str, keyword: &str) -> bool {
    let source = source.to_ascii_lowercase();
    let keyword = keyword.to_ascii_lowercase();
    source.match_indices(&keyword).any(|(start, _)| {
        let before = source[..start].chars().next_back();
        let after = source[start + keyword.len()..].chars().next();
        before.is_none_or(|character| !character.is_ascii_alphanumeric())
            && after.is_none_or(|character| !character.is_ascii_alphanumeric())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TokenColor {
    Colorless,
    Red,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryTokenDefinition {
    pub power: i32,
    pub toughness: i32,
    pub color: TokenColor,
    pub creature_subtype: &'static str,
}

impl EntryAttachmentTokenKind {
    pub const fn definition(self) -> EntryTokenDefinition {
        match self {
            Self::ColorlessHeroOneOne => EntryTokenDefinition {
                power: 1,
                toughness: 1,
                color: TokenColor::Colorless,
                creature_subtype: "Hero",
            },
            Self::RedRebelTwoTwo => EntryTokenDefinition {
                power: 2,
                toughness: 2,
                color: TokenColor::Red,
                creature_subtype: "Rebel",
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquipmentEntryEvent {
    pub event_id: EventId,
    pub source: ObjectRef,
    pub source_controller: PlayerId,
    pub entered_battlefield: bool,
    pub source_was_equipment: bool,
    pub evidence_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEntryAttachmentTrigger {
    pub trigger_id: TriggerId,
    pub program_digest: String,
    pub source: ObjectRef,
    pub controller: PlayerId,
    pub token: EntryAttachmentTokenKind,
    pub originating_event: EventId,
}

pub fn create_entry_attachment_trigger(
    program: &StaticSpecialKeywordProgram,
    trigger_id: TriggerId,
    event: EquipmentEntryEvent,
) -> Result<Option<PendingEntryAttachmentTrigger>, StaticSpecialRuntimeError> {
    let StaticSpecialKeywordKind::EntryAttachment { token } = program.kind() else {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    };
    if !event.evidence_complete {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "equipment entry event",
        ));
    }
    if !event.entered_battlefield {
        return Ok(None);
    }
    if !event.source_was_equipment {
        return Err(StaticSpecialRuntimeError::InvalidSource);
    }
    Ok(Some(PendingEntryAttachmentTrigger {
        trigger_id,
        program_digest: program.semantic_digest().to_owned(),
        source: event.source,
        controller: event.source_controller,
        token,
        originating_event: event.event_id,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedEntryToken {
    pub object: ObjectRef,
    pub controller: PlayerId,
    pub definition: EntryTokenDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenCreationResult {
    Prevented,
    Created(Vec<CreatedEntryToken>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryAttachmentResolutionEvidence {
    pub token_creation: TokenCreationResult,
    pub source_currently_on_battlefield: bool,
    pub source_currently_equipment: bool,
    pub legal_attachment_targets_complete: bool,
    pub legal_created_targets: BTreeSet<ObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryAttachmentResolutionChoice {
    pub attach_to: Option<ObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryAttachmentResolutionReceipt {
    pub trigger_id: TriggerId,
    pub created_tokens: Vec<CreatedEntryToken>,
    pub attached: Option<(ObjectRef, ObjectRef)>,
}

pub fn resolve_entry_attachment_trigger(
    program: &StaticSpecialKeywordProgram,
    trigger: PendingEntryAttachmentTrigger,
    evidence: EntryAttachmentResolutionEvidence,
    choice: EntryAttachmentResolutionChoice,
) -> Result<EntryAttachmentResolutionReceipt, StaticSpecialRuntimeError> {
    let StaticSpecialKeywordKind::EntryAttachment { token } = program.kind() else {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    };
    if trigger.program_digest != program.semantic_digest()
        || trigger.token != token
        || trigger.source.object_id == 0
    {
        return Err(StaticSpecialRuntimeError::StaleProgramEvidence);
    }
    let created_tokens = match evidence.token_creation {
        TokenCreationResult::Prevented => Vec::new(),
        TokenCreationResult::Created(tokens) => {
            if tokens
                .iter()
                .any(|created| created.controller != trigger.controller)
            {
                return Err(StaticSpecialRuntimeError::InvalidTokenCreationEvidence);
            }
            tokens
        }
    };

    let attached = if created_tokens.is_empty()
        || !evidence.source_currently_on_battlefield
        || !evidence.source_currently_equipment
    {
        if choice.attach_to.is_some() {
            return Err(StaticSpecialRuntimeError::IllegalChoice);
        }
        None
    } else {
        if !evidence.legal_attachment_targets_complete {
            return Err(StaticSpecialRuntimeError::IncompleteEvidence(
                "attachment legality",
            ));
        }
        let created_refs = created_tokens
            .iter()
            .map(|created| created.object)
            .collect::<BTreeSet<_>>();
        if !evidence
            .legal_created_targets
            .iter()
            .all(|object| created_refs.contains(object))
        {
            return Err(StaticSpecialRuntimeError::InvalidAttachmentEvidence);
        }
        let legal = evidence
            .legal_created_targets
            .intersection(&created_refs)
            .copied()
            .collect::<Vec<_>>();
        if legal.is_empty() {
            if choice.attach_to.is_some() {
                return Err(StaticSpecialRuntimeError::IllegalChoice);
            }
            None
        } else {
            let selected = match (legal.as_slice(), choice.attach_to) {
                ([only], None) => *only,
                (_, Some(selected)) if legal.contains(&selected) => selected,
                _ => return Err(StaticSpecialRuntimeError::AttachmentChoiceRequired),
            };
            Some((trigger.source, selected))
        }
    };

    Ok(EntryAttachmentResolutionReceipt {
        trigger_id: trigger.trigger_id,
        created_tokens,
        attached,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivingMetalEvidence {
    pub source: ObjectRef,
    pub source_on_battlefield: bool,
    pub source_controller: PlayerId,
    pub source_is_vehicle: bool,
    pub source_has_living_metal: bool,
    pub active_turn_player: PlayerId,
    pub turn_evidence_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivingMetalContinuousModifier {
    pub source: ObjectRef,
    pub add_creature_card_type: bool,
    pub retain_printed_power_toughness: bool,
}

pub fn evaluate_living_metal(
    program: &StaticSpecialKeywordProgram,
    evidence: LivingMetalEvidence,
) -> Result<Option<LivingMetalContinuousModifier>, StaticSpecialRuntimeError> {
    if program.kind() != StaticSpecialKeywordKind::LivingMetal {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if !evidence.turn_evidence_complete {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "active turn player",
        ));
    }
    if !evidence.source_on_battlefield {
        return Ok(None);
    }
    if !evidence.source_is_vehicle || !evidence.source_has_living_metal {
        return Err(StaticSpecialRuntimeError::InvalidSource);
    }
    Ok(
        (evidence.source_controller == evidence.active_turn_player).then_some(
            LivingMetalContinuousModifier {
                source: evidence.source,
                add_creature_card_type: true,
                retain_printed_power_toughness: true,
            },
        ),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackBandMemberEvidence {
    pub object: ObjectRef,
    pub controller: PlayerId,
    pub defender: PlayerId,
    pub declared_as_attacker: bool,
    pub attack_declaration_legal_without_banding: bool,
    pub has_banding: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackBandDeclarationEvidence {
    pub combat_id: CombatId,
    pub attacking_player: PlayerId,
    pub members: Vec<AttackBandMemberEvidence>,
    pub complete_attack_declaration: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackBand {
    pub combat_id: CombatId,
    pub controller: PlayerId,
    pub defender: PlayerId,
    pub members: BTreeSet<ObjectRef>,
}

pub fn declare_attack_band(
    program: &StaticSpecialKeywordProgram,
    evidence: AttackBandDeclarationEvidence,
) -> Result<AttackBand, StaticSpecialRuntimeError> {
    if program.kind() != StaticSpecialKeywordKind::Banding {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if !evidence.complete_attack_declaration {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "attack declaration",
        ));
    }
    if evidence.members.len() < 2 {
        return Err(StaticSpecialRuntimeError::InvalidBand);
    }
    let unique = evidence
        .members
        .iter()
        .map(|member| member.object)
        .collect::<BTreeSet<_>>();
    if unique.len() != evidence.members.len()
        || evidence.members.iter().any(|member| {
            member.controller != evidence.attacking_player
                || !member.declared_as_attacker
                || !member.attack_declaration_legal_without_banding
        })
    {
        return Err(StaticSpecialRuntimeError::InvalidBand);
    }
    let defenders = evidence
        .members
        .iter()
        .map(|member| member.defender)
        .collect::<BTreeSet<_>>();
    if defenders.len() != 1 {
        return Err(StaticSpecialRuntimeError::InvalidBand);
    }
    let without_banding = evidence
        .members
        .iter()
        .filter(|member| !member.has_banding)
        .count();
    if without_banding > 1 || evidence.members.iter().all(|member| !member.has_banding) {
        return Err(StaticSpecialRuntimeError::InvalidBand);
    }
    Ok(AttackBand {
        combat_id: evidence.combat_id,
        controller: evidence.attacking_player,
        defender: *defenders
            .iter()
            .next()
            .expect("a valid band has at least one defender"),
        members: unique,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockRelation {
    pub attacker: ObjectRef,
    pub blocker: ObjectRef,
}

/// Band members are blocked as a group. A legal block of one member creates
/// the same blocked relation for every member of that attack band.
pub fn expand_band_block_relations(
    band: &AttackBand,
    declared_relations: &BTreeSet<BlockRelation>,
) -> Result<BTreeSet<BlockRelation>, StaticSpecialRuntimeError> {
    let blockers = declared_relations
        .iter()
        .filter(|relation| band.members.contains(&relation.attacker))
        .map(|relation| relation.blocker)
        .collect::<BTreeSet<_>>();
    if blockers.is_empty() {
        return Ok(declared_relations.clone());
    }
    let mut expanded = declared_relations.clone();
    for blocker in blockers {
        for attacker in &band.members {
            expanded.insert(BlockRelation {
                attacker: *attacker,
                blocker,
            });
        }
    }
    Ok(expanded)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatCreatureEvidence {
    pub object: ObjectRef,
    pub controller: PlayerId,
    pub has_banding: bool,
    pub in_current_combat: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BandingDamageChoiceEvidence {
    pub damage_source: ObjectRef,
    pub damage_source_controller: PlayerId,
    pub opposing_creatures: Vec<CombatCreatureEvidence>,
    pub block_relations: BTreeSet<BlockRelation>,
    pub combat_evidence_complete: bool,
}

/// Returns the player who assigns the source creature's combat damage. When a
/// creature is blocking, or blocked by, one or more creatures with banding
/// controlled by the same opposing player, that opposing player assigns its
/// combat damage among those creatures. Multiple eligible opposing
/// controllers are not inferred.
pub fn banding_damage_assignment_player(
    program: &StaticSpecialKeywordProgram,
    evidence: &BandingDamageChoiceEvidence,
) -> Result<PlayerId, StaticSpecialRuntimeError> {
    if program.kind() != StaticSpecialKeywordKind::Banding {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if !evidence.combat_evidence_complete {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "combat damage assignment",
        ));
    }
    let eligible = evidence
        .opposing_creatures
        .iter()
        .filter(|opponent| {
            opponent.in_current_combat
                && opponent.has_banding
                && evidence.block_relations.iter().any(|relation| {
                    (relation.attacker == evidence.damage_source
                        && relation.blocker == opponent.object)
                        || (relation.blocker == evidence.damage_source
                            && relation.attacker == opponent.object)
                })
        })
        .map(|opponent| opponent.controller)
        .collect::<BTreeSet<_>>();
    match eligible.len() {
        0 => Ok(evidence.damage_source_controller),
        1 => Ok(*eligible.iter().next().expect("one eligible controller")),
        _ => Err(StaticSpecialRuntimeError::AmbiguousBandingController),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatDamageAssignment {
    pub recipient: ObjectRef,
    pub amount: u32,
}

pub fn validate_banding_combat_damage_division(
    assigning_player: PlayerId,
    expected_assigning_player: PlayerId,
    total_damage: u32,
    legal_opposing_creatures: &BTreeSet<ObjectRef>,
    assignments: &[CombatDamageAssignment],
) -> Result<(), StaticSpecialRuntimeError> {
    if assigning_player != expected_assigning_player {
        return Err(StaticSpecialRuntimeError::IllegalDamageAssignment);
    }
    let mut seen = BTreeSet::new();
    let mut assigned = 0u32;
    for assignment in assignments {
        if assignment.amount == 0
            || !legal_opposing_creatures.contains(&assignment.recipient)
            || !seen.insert(assignment.recipient)
        {
            return Err(StaticSpecialRuntimeError::IllegalDamageAssignment);
        }
        assigned = assigned
            .checked_add(assignment.amount)
            .ok_or(StaticSpecialRuntimeError::NumericOverflow)?;
    }
    if assigned != total_damage {
        return Err(StaticSpecialRuntimeError::IllegalDamageAssignment);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseStatus {
    PhasedIn,
    PhasedOutDirect {
        phased_out_under_player: PlayerId,
    },
    PhasedOutIndirect {
        direct_anchor: ObjectRef,
        phased_out_under_player: PlayerId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhasePermanentEvidence {
    pub object: ObjectRef,
    pub controller: PlayerId,
    pub has_phasing: bool,
    pub status: PhaseStatus,
    pub attached_to: Option<ObjectRef>,
    pub in_combat: bool,
    pub is_token: bool,
    pub counters: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhasingWorld {
    pub permanents: BTreeMap<ObjectRef, PhasePermanentEvidence>,
    pub battlefield_set_complete: bool,
    pub attachment_graph_complete: bool,
    pub controller_evidence_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntapPhasingPlan {
    pub active_player: PlayerId,
    pub phase_out_direct: BTreeSet<ObjectRef>,
    pub phase_out_indirect: BTreeMap<ObjectRef, ObjectRef>,
    pub phase_in_direct: BTreeSet<ObjectRef>,
    pub phase_in_indirect: BTreeSet<ObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntapPhasingReceipt {
    pub active_player: PlayerId,
    pub phased_out: BTreeSet<ObjectRef>,
    pub phased_in: BTreeSet<ObjectRef>,
    pub removed_from_combat: BTreeSet<ObjectRef>,
    pub zone_change_events_created: u32,
    pub enters_or_leaves_triggers_created: u32,
}

pub fn plan_untap_phasing(
    program: &StaticSpecialKeywordProgram,
    active_player: PlayerId,
    world: &PhasingWorld,
) -> Result<UntapPhasingPlan, StaticSpecialRuntimeError> {
    if program.kind() != StaticSpecialKeywordKind::Phasing {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if !world.battlefield_set_complete
        || !world.attachment_graph_complete
        || !world.controller_evidence_complete
    {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "phasing battlefield, control, and attachment graph",
        ));
    }
    validate_attachment_graph(world)?;

    let phase_out_direct = world
        .permanents
        .values()
        .filter(|permanent| {
            permanent.status == PhaseStatus::PhasedIn
                && permanent.controller == active_player
                && permanent.has_phasing
        })
        .map(|permanent| permanent.object)
        .collect::<BTreeSet<_>>();
    let phase_in_direct = world
        .permanents
        .values()
        .filter_map(|permanent| match permanent.status {
            PhaseStatus::PhasedOutDirect {
                phased_out_under_player,
            } if phased_out_under_player == active_player => Some(permanent.object),
            _ => None,
        })
        .collect::<BTreeSet<_>>();

    let phase_out_indirect = collect_indirect_phase_out(world, &phase_out_direct)?;
    let phase_in_indirect = world
        .permanents
        .values()
        .filter_map(|permanent| match permanent.status {
            PhaseStatus::PhasedOutIndirect {
                direct_anchor,
                phased_out_under_player,
            } if phased_out_under_player == active_player
                && phase_in_direct.contains(&direct_anchor) =>
            {
                Some(permanent.object)
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();

    Ok(UntapPhasingPlan {
        active_player,
        phase_out_direct,
        phase_out_indirect,
        phase_in_direct,
        phase_in_indirect,
    })
}

fn validate_attachment_graph(world: &PhasingWorld) -> Result<(), StaticSpecialRuntimeError> {
    for permanent in world.permanents.values() {
        let mut cursor = permanent.attached_to;
        let mut visited = BTreeSet::from([permanent.object]);
        while let Some(parent) = cursor {
            if !visited.insert(parent) {
                return Err(StaticSpecialRuntimeError::InvalidAttachmentEvidence);
            }
            let parent_state = world
                .permanents
                .get(&parent)
                .ok_or(StaticSpecialRuntimeError::InvalidAttachmentEvidence)?;
            cursor = parent_state.attached_to;
        }
    }
    Ok(())
}

fn collect_indirect_phase_out(
    world: &PhasingWorld,
    direct: &BTreeSet<ObjectRef>,
) -> Result<BTreeMap<ObjectRef, ObjectRef>, StaticSpecialRuntimeError> {
    let mut indirect = BTreeMap::new();
    for permanent in world.permanents.values() {
        if permanent.status != PhaseStatus::PhasedIn || direct.contains(&permanent.object) {
            continue;
        }
        let mut cursor = permanent.attached_to;
        let mut visited = BTreeSet::new();
        while let Some(parent) = cursor {
            if !visited.insert(parent) {
                return Err(StaticSpecialRuntimeError::InvalidAttachmentEvidence);
            }
            if direct.contains(&parent) {
                indirect.insert(permanent.object, parent);
                break;
            }
            cursor = world
                .permanents
                .get(&parent)
                .ok_or(StaticSpecialRuntimeError::InvalidAttachmentEvidence)?
                .attached_to;
        }
    }
    Ok(indirect)
}

pub fn apply_untap_phasing_plan(
    plan: UntapPhasingPlan,
    world: &mut PhasingWorld,
) -> Result<UntapPhasingReceipt, StaticSpecialRuntimeError> {
    let expected = plan_untap_phasing(
        &StaticSpecialKeywordProgram {
            exact_source: "Phasing".to_owned(),
            normalized_source: "phasing".to_owned(),
            semantic_digest: String::new(),
            kind: StaticSpecialKeywordKind::Phasing,
        },
        plan.active_player,
        world,
    )?;
    if plan != expected {
        return Err(StaticSpecialRuntimeError::StalePhasingPlan);
    }

    let mut phased_out = BTreeSet::new();
    let mut phased_in = BTreeSet::new();
    let mut removed_from_combat = BTreeSet::new();
    for object in &plan.phase_out_direct {
        let permanent = world
            .permanents
            .get_mut(object)
            .ok_or(StaticSpecialRuntimeError::StalePhasingPlan)?;
        permanent.status = PhaseStatus::PhasedOutDirect {
            phased_out_under_player: plan.active_player,
        };
        if permanent.in_combat {
            permanent.in_combat = false;
            removed_from_combat.insert(*object);
        }
        phased_out.insert(*object);
    }
    for (object, anchor) in &plan.phase_out_indirect {
        let permanent = world
            .permanents
            .get_mut(object)
            .ok_or(StaticSpecialRuntimeError::StalePhasingPlan)?;
        permanent.status = PhaseStatus::PhasedOutIndirect {
            direct_anchor: *anchor,
            phased_out_under_player: plan.active_player,
        };
        if permanent.in_combat {
            permanent.in_combat = false;
            removed_from_combat.insert(*object);
        }
        phased_out.insert(*object);
    }
    for object in plan
        .phase_in_direct
        .iter()
        .chain(plan.phase_in_indirect.iter())
    {
        let permanent = world
            .permanents
            .get_mut(object)
            .ok_or(StaticSpecialRuntimeError::StalePhasingPlan)?;
        permanent.status = PhaseStatus::PhasedIn;
        phased_in.insert(*object);
    }
    Ok(UntapPhasingReceipt {
        active_player: plan.active_player,
        phased_out,
        phased_in,
        removed_from_combat,
        zone_change_events_created: 0,
        enters_or_leaves_triggers_created: 0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredAttackerEvidence {
    pub object: ObjectRef,
    pub controller: PlayerId,
    pub declared_in_this_attack_declaration: bool,
    pub current_power: i32,
    pub power_evidence_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnlistHelperEvidence {
    pub object: ObjectRef,
    pub controller: PlayerId,
    pub is_creature: bool,
    pub is_attacking: bool,
    pub tapped: bool,
    pub controlled_continuously_since_turn_began: bool,
    pub has_haste: bool,
    pub current_power: i32,
    pub power_evidence_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnlistAttackDeclarationInput {
    pub event_id: EventId,
    pub combat_id: CombatId,
    pub turn_id: TurnId,
    pub source: DeclaredAttackerEvidence,
    pub helper: Option<EnlistHelperEvidence>,
    pub attack_declaration_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEnlistTrigger {
    pub trigger_id: TriggerId,
    pub program_digest: String,
    pub source: ObjectRef,
    pub helper: ObjectRef,
    pub controller: PlayerId,
    pub helper_power_lki: i32,
    pub combat_id: CombatId,
    pub turn_id: TurnId,
    pub originating_event: EventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnlistDeclarationReceipt {
    pub tapped_helper: Option<ObjectRef>,
    pub pending_trigger: Option<PendingEnlistTrigger>,
}

/// Enlist is handled inside attacker declaration. Tapping the helper is not an
/// activated ability and does not use the stack. The "when you do" reflexive
/// trigger is created only after the optional tap actually occurs.
pub fn apply_enlist_during_attack_declaration(
    program: &StaticSpecialKeywordProgram,
    trigger_id: TriggerId,
    input: EnlistAttackDeclarationInput,
) -> Result<EnlistDeclarationReceipt, StaticSpecialRuntimeError> {
    if program.kind() != StaticSpecialKeywordKind::Enlist {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if !input.attack_declaration_complete {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "attacker declaration",
        ));
    }
    if !input.source.declared_in_this_attack_declaration || !input.source.power_evidence_complete {
        return Err(StaticSpecialRuntimeError::InvalidSource);
    }
    let Some(helper) = input.helper else {
        return Ok(EnlistDeclarationReceipt {
            tapped_helper: None,
            pending_trigger: None,
        });
    };
    if !helper.power_evidence_complete {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "enlisted creature power",
        ));
    }
    if helper.object == input.source.object
        || helper.controller != input.source.controller
        || !helper.is_creature
        || helper.is_attacking
        || helper.tapped
        || (!helper.controlled_continuously_since_turn_began && !helper.has_haste)
    {
        return Err(StaticSpecialRuntimeError::IllegalEnlistHelper);
    }
    Ok(EnlistDeclarationReceipt {
        tapped_helper: Some(helper.object),
        pending_trigger: Some(PendingEnlistTrigger {
            trigger_id,
            program_digest: program.semantic_digest().to_owned(),
            source: input.source.object,
            helper: helper.object,
            controller: input.source.controller,
            helper_power_lki: helper.current_power,
            combat_id: input.combat_id,
            turn_id: input.turn_id,
            originating_event: input.event_id,
        }),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnlistResolutionEvidence {
    pub source_current: Option<ObjectRef>,
    pub helper_current: Option<(ObjectRef, i32)>,
    pub helper_lki_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UntilEndOfTurnPowerModifier {
    pub source: ObjectRef,
    pub power_delta: i32,
    pub expires_after_turn: TurnId,
    pub originating_trigger: TriggerId,
}

pub fn resolve_enlist_trigger(
    program: &StaticSpecialKeywordProgram,
    trigger: PendingEnlistTrigger,
    evidence: EnlistResolutionEvidence,
) -> Result<Option<UntilEndOfTurnPowerModifier>, StaticSpecialRuntimeError> {
    if program.kind() != StaticSpecialKeywordKind::Enlist {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if trigger.program_digest != program.semantic_digest() {
        return Err(StaticSpecialRuntimeError::StaleProgramEvidence);
    }
    if evidence.source_current != Some(trigger.source) {
        return Ok(None);
    }
    let helper_power = match evidence.helper_current {
        Some((object, power)) if object == trigger.helper => power,
        Some(_) => return Err(StaticSpecialRuntimeError::StaleObjectEvidence),
        None if evidence.helper_lki_available => trigger.helper_power_lki,
        None => {
            return Err(StaticSpecialRuntimeError::IncompleteEvidence(
                "enlisted creature last known power",
            ));
        }
    };
    Ok(Some(UntilEndOfTurnPowerModifier {
        source: trigger.source,
        power_delta: helper_power,
        expires_after_turn: trigger.turn_id,
        originating_trigger: trigger.trigger_id,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainingAttackEvent {
    pub event_id: EventId,
    pub combat_id: CombatId,
    pub turn_id: TurnId,
    pub attacking_player: PlayerId,
    pub declared_attackers: Vec<DeclaredAttackerEvidence>,
    pub declaration_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTrainingTrigger {
    pub trigger_id: TriggerId,
    pub program_digest: String,
    pub source: ObjectRef,
    pub controller: PlayerId,
    pub qualifying_attacker_at_trigger: ObjectRef,
    pub originating_event: EventId,
}

pub fn create_training_trigger(
    program: &StaticSpecialKeywordProgram,
    trigger_id: TriggerId,
    source: ObjectRef,
    event: &TrainingAttackEvent,
) -> Result<Option<PendingTrainingTrigger>, StaticSpecialRuntimeError> {
    if program.kind() != StaticSpecialKeywordKind::Training {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if !event.declaration_complete {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "declared attackers",
        ));
    }
    if event
        .declared_attackers
        .iter()
        .any(|attacker| !attacker.power_evidence_complete)
    {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "attacker power",
        ));
    }
    let source_evidence = event
        .declared_attackers
        .iter()
        .find(|attacker| attacker.object == source)
        .ok_or(StaticSpecialRuntimeError::InvalidSource)?;
    if source_evidence.controller != event.attacking_player
        || !source_evidence.declared_in_this_attack_declaration
    {
        return Err(StaticSpecialRuntimeError::InvalidSource);
    }
    let qualifier = event
        .declared_attackers
        .iter()
        .filter(|attacker| {
            attacker.object != source
                && attacker.controller == source_evidence.controller
                && attacker.declared_in_this_attack_declaration
                && attacker.current_power > source_evidence.current_power
        })
        .map(|attacker| attacker.object)
        .min();
    Ok(
        qualifier.map(|qualifying_attacker_at_trigger| PendingTrainingTrigger {
            trigger_id,
            program_digest: program.semantic_digest().to_owned(),
            source,
            controller: source_evidence.controller,
            qualifying_attacker_at_trigger,
            originating_event: event.event_id,
        }),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainingResolutionEvidence {
    pub source_current: Option<ObjectRef>,
    pub plus_one_counter_multiplier: Option<u32>,
    pub counter_placement_allowed: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainingResolutionReceipt {
    pub source: ObjectRef,
    pub plus_one_counters_added: u32,
    pub originating_trigger: TriggerId,
}

pub fn resolve_training_trigger(
    program: &StaticSpecialKeywordProgram,
    trigger: PendingTrainingTrigger,
    evidence: TrainingResolutionEvidence,
) -> Result<Option<TrainingResolutionReceipt>, StaticSpecialRuntimeError> {
    if program.kind() != StaticSpecialKeywordKind::Training {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if trigger.program_digest != program.semantic_digest() {
        return Err(StaticSpecialRuntimeError::StaleProgramEvidence);
    }
    if evidence.source_current != Some(trigger.source) {
        return Ok(None);
    }
    let allowed =
        evidence
            .counter_placement_allowed
            .ok_or(StaticSpecialRuntimeError::IncompleteEvidence(
                "counter placement restrictions",
            ))?;
    let multiplier = evidence.plus_one_counter_multiplier.ok_or(
        StaticSpecialRuntimeError::IncompleteEvidence("counter replacement effects"),
    )?;
    Ok(Some(TrainingResolutionReceipt {
        source: trigger.source,
        plus_one_counters_added: if allowed { multiplier } else { 0 },
        originating_trigger: trigger.trigger_id,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePreparationPhase {
    BeforePregameProcedures,
    PregameProcedures,
    GameStarted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardNameCatalogEvidence {
    pub legal_names: BTreeSet<String>,
    pub catalog_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgendaPreparationInput {
    pub source: ObjectRef,
    pub owner: PlayerId,
    pub source_is_conspiracy: bool,
    pub phase: GamePreparationPhase,
    pub chosen_names: Vec<String>,
    pub private_commitment_nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgendaPrivateState {
    pub source: ObjectRef,
    pub owner: PlayerId,
    pub chosen_names: Vec<String>,
    pub private_commitment_nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgendaPublicState {
    pub source: ObjectRef,
    pub owner: PlayerId,
    pub in_command_zone: bool,
    pub face_down: bool,
    pub committed_name_count: u8,
    pub commitment_digest: String,
    pub revealed_names: Option<Vec<String>>,
}

pub fn prepare_agenda(
    program: &StaticSpecialKeywordProgram,
    input: AgendaPreparationInput,
    catalog: &CardNameCatalogEvidence,
) -> Result<(AgendaPrivateState, AgendaPublicState), StaticSpecialRuntimeError> {
    let StaticSpecialKeywordKind::Agenda { secret_name_count } = program.kind() else {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    };
    if !matches!(secret_name_count, 1 | 2) {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if input.phase != GamePreparationPhase::PregameProcedures {
        return Err(StaticSpecialRuntimeError::WrongGameProcedurePhase);
    }
    if !input.source_is_conspiracy {
        return Err(StaticSpecialRuntimeError::InvalidSource);
    }
    if !catalog.catalog_complete {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "legal card-name catalog",
        ));
    }
    if input.chosen_names.len() != usize::from(secret_name_count)
        || input
            .chosen_names
            .iter()
            .any(|name| name.is_empty() || !catalog.legal_names.iter().any(|legal| legal == name))
        || input.chosen_names.iter().collect::<BTreeSet<_>>().len()
            != usize::from(secret_name_count)
    {
        return Err(StaticSpecialRuntimeError::IllegalAgendaNameChoice);
    }
    if input.private_commitment_nonce.is_empty() {
        return Err(StaticSpecialRuntimeError::MissingPrivateCommitmentNonce);
    }

    let commitment_digest = agenda_commitment_digest(
        program,
        input.source,
        input.owner,
        &input.chosen_names,
        &input.private_commitment_nonce,
    );
    let private = AgendaPrivateState {
        source: input.source,
        owner: input.owner,
        chosen_names: input.chosen_names,
        private_commitment_nonce: input.private_commitment_nonce,
    };
    let public = AgendaPublicState {
        source: private.source,
        owner: private.owner,
        in_command_zone: true,
        face_down: true,
        committed_name_count: secret_name_count,
        commitment_digest,
        revealed_names: None,
    };
    Ok((private, public))
}

fn agenda_commitment_digest(
    program: &StaticSpecialKeywordProgram,
    source: ObjectRef,
    owner: PlayerId,
    names: &[String],
    nonce: &str,
) -> String {
    let mut digest = Sha256::new();
    digest_field(&mut digest, "agenda-program", program.semantic_digest());
    digest_field(&mut digest, "source-object", &source.object_id.to_string());
    digest_field(
        &mut digest,
        "source-incarnation",
        &source.incarnation_id.to_string(),
    );
    digest_field(&mut digest, "owner", &owner.to_string());
    for name in names {
        digest_field(&mut digest, "secret-name", name);
    }
    digest_field(&mut digest, "private-nonce", nonce);
    format!("{:X}", digest.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgendaRevealReceipt {
    pub source: ObjectRef,
    pub revealed_names: Vec<String>,
    pub used_stack: bool,
    pub priority_required: bool,
}

/// Turning an agenda face up is a special action taken while its controller
/// has priority. It does not use the stack, and the chosen names become public
/// as part of the same atomic action.
pub fn reveal_agenda(
    program: &StaticSpecialKeywordProgram,
    private: &AgendaPrivateState,
    public: &mut AgendaPublicState,
) -> Result<AgendaRevealReceipt, StaticSpecialRuntimeError> {
    let StaticSpecialKeywordKind::Agenda { secret_name_count } = program.kind() else {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    };
    if !matches!(secret_name_count, 1 | 2) {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if private.source != public.source
        || private.owner != public.owner
        || !public.in_command_zone
        || !public.face_down
        || public.revealed_names.is_some()
        || private.chosen_names.len() != usize::from(secret_name_count)
    {
        return Err(StaticSpecialRuntimeError::StaleAgendaEvidence);
    }
    let expected_commitment = agenda_commitment_digest(
        program,
        private.source,
        private.owner,
        &private.chosen_names,
        &private.private_commitment_nonce,
    );
    if public.commitment_digest != expected_commitment {
        return Err(StaticSpecialRuntimeError::StaleAgendaEvidence);
    }
    public.face_down = false;
    public.revealed_names = Some(private.chosen_names.clone());
    Ok(AgendaRevealReceipt {
        source: private.source,
        revealed_names: private.chosen_names.clone(),
        used_stack: false,
        priority_required: true,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PerpetualModification {
    RemoveKeyword(&'static str),
    OpaqueReviewedModification(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalCardState {
    pub card_id: DigitalCardId,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: DigitalZone,
    pub definition_digest: String,
    pub copiable_definition: String,
    pub perpetual_modifications: Vec<PerpetualModification>,
    pub has_double_team: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigitalZone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoubleTeamAttackEvent {
    pub event_id: EventId,
    pub source_card_id: DigitalCardId,
    pub source_controller: PlayerId,
    pub source_was_declared_as_attacker: bool,
    pub source_had_double_team: bool,
    pub source_lki: DigitalCardState,
    pub evidence_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDoubleTeamTrigger {
    pub trigger_id: TriggerId,
    pub program_digest: String,
    pub source_card_id: DigitalCardId,
    pub controller: PlayerId,
    pub source_lki: DigitalCardState,
    pub originating_event: EventId,
}

pub fn create_double_team_trigger(
    program: &StaticSpecialKeywordProgram,
    trigger_id: TriggerId,
    event: DoubleTeamAttackEvent,
) -> Result<Option<PendingDoubleTeamTrigger>, StaticSpecialRuntimeError> {
    if program.kind() != StaticSpecialKeywordKind::DoubleTeam {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if !event.evidence_complete {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "digital attack event",
        ));
    }
    if !event.source_was_declared_as_attacker || !event.source_had_double_team {
        return Ok(None);
    }
    if event.source_lki.card_id != event.source_card_id
        || event.source_lki.controller != event.source_controller
        || event.source_lki.zone != DigitalZone::Battlefield
    {
        return Err(StaticSpecialRuntimeError::InvalidSource);
    }
    Ok(Some(PendingDoubleTeamTrigger {
        trigger_id,
        program_digest: program.semantic_digest().to_owned(),
        source_card_id: event.source_card_id,
        controller: event.source_controller,
        source_lki: event.source_lki,
        originating_event: event.event_id,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalGameState {
    pub cards: BTreeMap<DigitalCardId, DigitalCardState>,
    pub next_card_id: DigitalCardId,
    pub digital_service_available: bool,
    pub stable_card_identity_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoubleTeamResolutionReceipt {
    pub trigger_id: TriggerId,
    pub conjured_card_id: DigitalCardId,
    pub source_keyword_removed: bool,
    pub duplicate_keyword_removed: bool,
}

pub fn resolve_double_team_trigger(
    program: &StaticSpecialKeywordProgram,
    trigger: PendingDoubleTeamTrigger,
    state: &mut DigitalGameState,
) -> Result<DoubleTeamResolutionReceipt, StaticSpecialRuntimeError> {
    if program.kind() != StaticSpecialKeywordKind::DoubleTeam {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if trigger.program_digest != program.semantic_digest() {
        return Err(StaticSpecialRuntimeError::StaleProgramEvidence);
    }
    if !state.digital_service_available || !state.stable_card_identity_complete {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence(
            "digital conjure and stable card identity",
        ));
    }
    if state.cards.contains_key(&state.next_card_id) || state.next_card_id == 0 {
        return Err(StaticSpecialRuntimeError::InvalidDigitalIdentity);
    }

    let source_for_copy = state
        .cards
        .get(&trigger.source_card_id)
        .cloned()
        .unwrap_or_else(|| trigger.source_lki.clone());
    let conjured_card_id = state.next_card_id;
    state.next_card_id = state
        .next_card_id
        .checked_add(1)
        .ok_or(StaticSpecialRuntimeError::NumericOverflow)?;
    let mut duplicate = source_for_copy;
    duplicate.card_id = conjured_card_id;
    duplicate.owner = trigger.controller;
    duplicate.controller = trigger.controller;
    duplicate.zone = DigitalZone::Hand;
    apply_perpetual_double_team_removal(&mut duplicate);
    state.cards.insert(conjured_card_id, duplicate);

    let source_keyword_removed =
        if let Some(source_current) = state.cards.get_mut(&trigger.source_card_id) {
            apply_perpetual_double_team_removal(source_current);
            true
        } else {
            false
        };
    Ok(DoubleTeamResolutionReceipt {
        trigger_id: trigger.trigger_id,
        conjured_card_id,
        source_keyword_removed,
        duplicate_keyword_removed: true,
    })
}

fn apply_perpetual_double_team_removal(card: &mut DigitalCardState) {
    card.has_double_team = false;
    if !card
        .perpetual_modifications
        .iter()
        .any(|modification| modification == &PerpetualModification::RemoveKeyword("Double team"))
    {
        card.perpetual_modifications
            .push(PerpetualModification::RemoveKeyword("Double team"));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftCardVisibility {
    HiddenFromOtherDrafters,
    FaceUpToAllDrafters,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftCardState {
    pub card_id: DraftCardId,
    pub drafted_by: Option<PlayerId>,
    pub visibility: DraftCardVisibility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftState {
    pub cards: BTreeMap<DraftCardId, DraftCardState>,
    pub current_drafter: PlayerId,
    pub available_pick_ids: BTreeSet<DraftCardId>,
    pub available_pick_set_complete: bool,
    pub draft_in_progress: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceUpDraftReceipt {
    pub card_id: DraftCardId,
    pub drafted_by: PlayerId,
    pub visible_to_all_drafters: bool,
}

pub fn draft_card_face_up(
    program: &StaticSpecialKeywordProgram,
    card_id: DraftCardId,
    state: &mut DraftState,
) -> Result<FaceUpDraftReceipt, StaticSpecialRuntimeError> {
    if program.kind() != StaticSpecialKeywordKind::DraftFaceUp {
        return Err(StaticSpecialRuntimeError::WrongProgramKind);
    }
    if !state.draft_in_progress || !state.available_pick_set_complete {
        return Err(StaticSpecialRuntimeError::IncompleteEvidence("draft pick"));
    }
    if !state.available_pick_ids.remove(&card_id) {
        return Err(StaticSpecialRuntimeError::IllegalDraftPick);
    }
    let card = state
        .cards
        .get_mut(&card_id)
        .ok_or(StaticSpecialRuntimeError::IllegalDraftPick)?;
    if card.drafted_by.is_some() {
        return Err(StaticSpecialRuntimeError::IllegalDraftPick);
    }
    card.drafted_by = Some(state.current_drafter);
    card.visibility = DraftCardVisibility::FaceUpToAllDrafters;
    Ok(FaceUpDraftReceipt {
        card_id,
        drafted_by: state.current_drafter,
        visible_to_all_drafters: true,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticSpecialRuntimeError {
    WrongProgramKind,
    IncompleteEvidence(&'static str),
    InvalidSource,
    StaleProgramEvidence,
    StaleObjectEvidence,
    IllegalChoice,
    AttachmentChoiceRequired,
    InvalidTokenCreationEvidence,
    InvalidAttachmentEvidence,
    InvalidBand,
    AmbiguousBandingController,
    IllegalDamageAssignment,
    StalePhasingPlan,
    IllegalEnlistHelper,
    WrongGameProcedurePhase,
    IllegalAgendaNameChoice,
    MissingPrivateCommitmentNonce,
    StaleAgendaEvidence,
    InvalidDigitalIdentity,
    IllegalDraftPick,
    NumericOverflow,
}

impl fmt::Display for StaticSpecialRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProgramKind => write!(formatter, "wrong static or special program kind"),
            Self::IncompleteEvidence(detail) => {
                write!(formatter, "incomplete runtime evidence: {detail}")
            }
            Self::InvalidSource => write!(formatter, "invalid source evidence"),
            Self::StaleProgramEvidence => write!(formatter, "stale program evidence"),
            Self::StaleObjectEvidence => write!(formatter, "stale object evidence"),
            Self::IllegalChoice => write!(formatter, "illegal resolution choice"),
            Self::AttachmentChoiceRequired => {
                write!(formatter, "an explicit legal attachment choice is required")
            }
            Self::InvalidTokenCreationEvidence => {
                write!(formatter, "invalid token creation evidence")
            }
            Self::InvalidAttachmentEvidence => {
                write!(formatter, "invalid attachment graph or legality evidence")
            }
            Self::InvalidBand => write!(formatter, "invalid attack band"),
            Self::AmbiguousBandingController => {
                write!(formatter, "banding damage controller is ambiguous")
            }
            Self::IllegalDamageAssignment => write!(formatter, "illegal combat damage assignment"),
            Self::StalePhasingPlan => write!(formatter, "stale phasing plan"),
            Self::IllegalEnlistHelper => write!(formatter, "illegal enlist helper"),
            Self::WrongGameProcedurePhase => write!(formatter, "wrong game procedure phase"),
            Self::IllegalAgendaNameChoice => write!(formatter, "illegal agenda card-name choice"),
            Self::MissingPrivateCommitmentNonce => {
                write!(formatter, "missing private agenda commitment nonce")
            }
            Self::StaleAgendaEvidence => write!(formatter, "stale agenda evidence"),
            Self::InvalidDigitalIdentity => write!(formatter, "invalid digital card identity"),
            Self::IllegalDraftPick => write!(formatter, "illegal draft pick"),
            Self::NumericOverflow => write!(formatter, "numeric overflow"),
        }
    }
}

impl std::error::Error for StaticSpecialRuntimeError {}
