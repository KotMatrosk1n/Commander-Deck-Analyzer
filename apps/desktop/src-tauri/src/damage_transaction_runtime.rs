//! Standalone typed damage transactions.
//!
//! This runtime is deliberately disconnected from the production Oracle
//! compiler and execution bridge. It stages a complete damage event, including
//! replacement and prevention choices, and commits only after every assignment
//! and derived consequence succeeds.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use sha2::{Digest, Sha256};

pub const DAMAGE_TRANSACTION_RUNTIME_VERSION: &str = "damage-transaction-runtime-0.1";
pub const DAMAGE_SEMANTIC_INPUT_VERSION: &str = "damage-semantic-input-0.1";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type DamageAssignmentId = u16;
pub type DamageModifierId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageSemanticInput {
    exact_oracle: String,
    normalized_oracle: String,
    canonical_program: Vec<String>,
    semantic_digest: String,
}

impl DamageSemanticInput {
    pub fn from_content(
        exact_oracle: impl Into<String>,
        normalized_oracle: impl Into<String>,
        canonical_program: Vec<String>,
    ) -> Result<Self, DamageSemanticInputError> {
        let exact_oracle = exact_oracle.into();
        let normalized_oracle = normalized_oracle.into();
        if exact_oracle.trim().is_empty() {
            return Err(DamageSemanticInputError::EmptyExactOracle);
        }
        if normalized_oracle.trim().is_empty() {
            return Err(DamageSemanticInputError::EmptyNormalizedOracle);
        }
        if canonical_program.is_empty() {
            return Err(DamageSemanticInputError::EmptyCanonicalProgram);
        }
        let canonical_program = canonical_program
            .into_iter()
            .enumerate()
            .map(|(index, component)| {
                let component = component.trim().to_owned();
                if component.is_empty() {
                    Err(DamageSemanticInputError::EmptyCanonicalComponent { index })
                } else {
                    Ok(component)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let semantic_digest = damage_semantic_digest_with_versions(
            DAMAGE_SEMANTIC_INPUT_VERSION,
            DAMAGE_TRANSACTION_RUNTIME_VERSION,
            &exact_oracle,
            &normalized_oracle,
            &canonical_program,
        );
        Ok(Self {
            exact_oracle,
            normalized_oracle,
            canonical_program,
            semantic_digest,
        })
    }

    pub fn exact_oracle(&self) -> &str {
        &self.exact_oracle
    }

    pub fn normalized_oracle(&self) -> &str {
        &self.normalized_oracle
    }

    pub fn canonical_program(&self) -> &[String] {
        &self.canonical_program
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageSemanticInputError {
    EmptyExactOracle,
    EmptyNormalizedOracle,
    EmptyCanonicalProgram,
    EmptyCanonicalComponent { index: usize },
}

impl fmt::Display for DamageSemanticInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DamageSemanticInputError {}

fn damage_semantic_digest_with_versions(
    semantic_input_version: &str,
    runtime_version: &str,
    exact_oracle: &str,
    normalized_oracle: &str,
    canonical_program: &[String],
) -> String {
    let mut hasher = Sha256::new();
    for component in [
        b"damage-semantic-content/v1".as_slice(),
        semantic_input_version.as_bytes(),
        runtime_version.as_bytes(),
        exact_oracle.as_bytes(),
        normalized_oracle.as_bytes(),
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component);
    }
    hasher.update((canonical_program.len() as u64).to_le_bytes());
    for component in canonical_program {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DamageSourceKeyword {
    Deathtouch,
    Infect,
    Lifelink,
    Wither,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageSourceKind {
    Creature,
    OtherPermanent,
    Spell,
    Player,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageSourceCharacteristics {
    pub kind: DamageSourceKind,
    pub keywords: BTreeSet<DamageSourceKeyword>,
}

impl DamageSourceCharacteristics {
    pub fn has_keyword(&self, keyword: DamageSourceKeyword) -> bool {
        self.keywords.contains(&keyword)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DamageSourceIdentity {
    Object(ObjectId),
    Player(PlayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageSourceEvidence {
    CurrentCharacteristics,
    LastKnownInformation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageSourceSnapshot {
    pub identity: DamageSourceIdentity,
    pub controller: PlayerId,
    pub evidence: DamageSourceEvidence,
    pub characteristics: DamageSourceCharacteristics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamagePlayerState {
    pub life: i64,
    pub poison_counters: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageCreatureState {
    pub controller: PlayerId,
    pub marked_damage: u32,
    pub minus_one_minus_one_counters: u32,
    pub has_deathtouch_damage: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamagePlaneswalkerState {
    pub controller: PlayerId,
    pub loyalty: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageBattleState {
    pub controller: PlayerId,
    pub defense: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageObjectState {
    Creature(DamageCreatureState),
    Planeswalker(DamagePlaneswalkerState),
    Battle(DamageBattleState),
}

impl DamageObjectState {
    pub fn controller(self) -> PlayerId {
        match self {
            Self::Creature(state) => state.controller,
            Self::Planeswalker(state) => state.controller,
            Self::Battle(state) => state.controller,
        }
    }

    pub fn kind(self) -> DamageRecipientKind {
        match self {
            Self::Creature(_) => DamageRecipientKind::Creature,
            Self::Planeswalker(_) => DamageRecipientKind::Planeswalker,
            Self::Battle(_) => DamageRecipientKind::Battle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DamageRuntimeState {
    pub players: BTreeMap<PlayerId, DamagePlayerState>,
    pub objects: BTreeMap<ObjectId, DamageObjectState>,
    pub modifiers: BTreeMap<DamageModifierId, DamageModifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DamageRecipientKind {
    Player,
    Creature,
    Planeswalker,
    Battle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DamageRecipient {
    Player(PlayerId),
    Creature(ObjectId),
    Planeswalker(ObjectId),
    Battle(ObjectId),
}

impl DamageRecipient {
    pub fn kind(self) -> DamageRecipientKind {
        match self {
            Self::Player(_) => DamageRecipientKind::Player,
            Self::Creature(_) => DamageRecipientKind::Creature,
            Self::Planeswalker(_) => DamageRecipientKind::Planeswalker,
            Self::Battle(_) => DamageRecipientKind::Battle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageAssignment {
    pub id: DamageAssignmentId,
    pub recipient: DamageRecipient,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageRecipients {
    Single(DamageAssignment),
    Set {
        kind: DamageRecipientKind,
        assignments: Vec<DamageAssignment>,
    },
    Mixed {
        assignments: Vec<DamageAssignment>,
    },
}

impl DamageRecipients {
    fn assignments(&self) -> &[DamageAssignment] {
        match self {
            Self::Single(assignment) => std::slice::from_ref(assignment),
            Self::Set { assignments, .. } | Self::Mixed { assignments } => assignments,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalDamageTargetKind {
    Player,
    OpponentOf(PlayerId),
    Creature,
    Planeswalker,
    Battle,
    CreatureOrPlaneswalker,
    PlayerOrPlaneswalker,
    AnyTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinedDamageSet {
    EachPlayer,
    EachOpponentOf(PlayerId),
    EachCreature,
    EachPlaneswalker,
    EachBattle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageSelection {
    Untargeted,
    Targeted(LegalDamageTargetKind),
    DefinedSet(DefinedDamageSet),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageKind {
    Combat,
    Noncombat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamagePreventability {
    Preventable,
    CannotBePrevented,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageSourceMatcher {
    Any,
    Identity(DamageSourceIdentity),
    Controller(PlayerId),
    HasKeyword(DamageSourceKeyword),
    Unsupported { semantic: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageRecipientMatcher {
    Any,
    Kind(DamageRecipientKind),
    Exact(DamageRecipient),
    ControlledBy(PlayerId),
    Unsupported { semantic: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageKindMatcher {
    Any,
    Combat,
    Noncombat,
    Unsupported { semantic: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageEventMatcher {
    pub source: DamageSourceMatcher,
    pub recipient: DamageRecipientMatcher,
    pub kind: DamageKindMatcher,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageReplacement {
    Double,
    HalfRoundedDown,
    IncreaseBy(u32),
    SetAmount(u32),
    Redirect(DamageRecipient),
    Unsupported { semantic: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamagePrevention {
    PreventAll,
    PreventAmount(u32),
    Shield { remaining: u32 },
    Unsupported { semantic: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageModifierOperation {
    Replacement(DamageReplacement),
    Prevention(DamagePrevention),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageModifierPersistence {
    Persistent,
    OneShot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageModifierRequirement {
    Mandatory,
    Optional,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageModifier {
    pub id: DamageModifierId,
    pub matcher: DamageEventMatcher,
    pub operation: DamageModifierOperation,
    pub persistence: DamageModifierPersistence,
    pub requirement: DamageModifierRequirement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageModifierDecision {
    Apply,
    Decline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageModifierChoice {
    pub assignment: DamageAssignmentId,
    pub chooser: PlayerId,
    pub modifier: DamageModifierId,
    pub decision: DamageModifierDecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageChoicePlan {
    pub assignment_order: Vec<DamageAssignmentId>,
    pub modifier_choices: Vec<DamageModifierChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageTransactionRequest {
    pub semantic: DamageSemanticInput,
    pub source: DamageSourceSnapshot,
    pub kind: DamageKind,
    pub preventability: DamagePreventability,
    pub recipients: DamageRecipients,
    pub selection: DamageSelection,
    pub choices: DamageChoicePlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppliedDamageModifierKind {
    Replacement,
    Prevention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageModifierDecisionReceipt {
    pub modifier: DamageModifierId,
    pub chooser: PlayerId,
    pub decision: DamageModifierDecision,
    pub kind: AppliedDamageModifierKind,
    pub recipient_before: DamageRecipient,
    pub recipient_after: DamageRecipient,
    pub amount_before: u32,
    pub amount_after: u32,
    pub prevented_damage: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageConsequenceReceipt {
    NoDamage,
    PlayerLife {
        player: PlayerId,
        before: i64,
        after: i64,
    },
    PlayerPoison {
        player: PlayerId,
        before: u32,
        after: u32,
    },
    CreatureMarkedDamage {
        object: ObjectId,
        before: u32,
        after: u32,
        deathtouch_before: bool,
        deathtouch_after: bool,
    },
    CreatureMinusOneCounters {
        object: ObjectId,
        before: u32,
        after: u32,
        deathtouch_before: bool,
        deathtouch_after: bool,
    },
    PlaneswalkerLoyalty {
        object: ObjectId,
        before: u32,
        after: u32,
        counters_removed: u32,
    },
    BattleDefense {
        object: ObjectId,
        before: u32,
        after: u32,
        counters_removed: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageAssignmentReceipt {
    pub assignment: DamageAssignmentId,
    pub original_recipient: DamageRecipient,
    pub final_recipient: DamageRecipient,
    pub original_amount: u32,
    pub actual_damage: u32,
    pub prevented_damage: u64,
    pub modifier_decisions: Vec<DamageModifierDecisionReceipt>,
    pub consequence: DamageConsequenceReceipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifelinkReceipt {
    pub player: PlayerId,
    pub actual_damage: u64,
    pub life_before: i64,
    pub life_after: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageTransactionReceipt {
    pub runtime_version: &'static str,
    pub semantic: DamageSemanticInput,
    pub source: DamageSourceSnapshot,
    pub kind: DamageKind,
    pub preventability: DamagePreventability,
    pub assignments: Vec<DamageAssignmentReceipt>,
    pub total_actual_damage: u64,
    pub lifelink: Option<LifelinkReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageTransactionError {
    MissingSourceController {
        player: PlayerId,
    },
    PlayerSourceControllerMismatch {
        source: PlayerId,
        controller: PlayerId,
    },
    PlayerSourceKindMismatch {
        actual: DamageSourceKind,
    },
    PlayerSourceEvidenceMismatch {
        actual: DamageSourceEvidence,
    },
    ObjectSourceKindMismatch {
        object: ObjectId,
        actual: DamageSourceKind,
    },
    MissingCurrentCreatureSource {
        object: ObjectId,
    },
    CurrentCreatureSourceKindMismatch {
        object: ObjectId,
        actual: DamageRecipientKind,
    },
    EmptyMixedRecipients,
    MixedRecipientsNeedMultipleKinds,
    SetRecipientKindMismatch {
        assignment: DamageAssignmentId,
        declared: DamageRecipientKind,
        actual: DamageRecipientKind,
    },
    DuplicateAssignment {
        assignment: DamageAssignmentId,
    },
    DuplicateRecipient {
        recipient: DamageRecipient,
    },
    MissingPlayer {
        player: PlayerId,
    },
    MissingObject {
        object: ObjectId,
    },
    ObjectKindMismatch {
        object: ObjectId,
        expected: DamageRecipientKind,
        actual: DamageRecipientKind,
    },
    IllegalTarget {
        assignment: DamageAssignmentId,
        recipient: DamageRecipient,
        legal: LegalDamageTargetKind,
    },
    DefinedSetRequiresSetRecipients,
    DefinedSetMismatch {
        defined: DefinedDamageSet,
        expected: Vec<DamageRecipient>,
        actual: Vec<DamageRecipient>,
    },
    ModifierKeyMismatch {
        key: DamageModifierId,
        modifier: DamageModifierId,
    },
    UnsupportedModifierShape {
        modifier: DamageModifierId,
        semantic: String,
    },
    ZeroPreventAmount {
        modifier: DamageModifierId,
    },
    UnknownRedirectRecipient {
        modifier: DamageModifierId,
        recipient: DamageRecipient,
    },
    AssignmentOrderMismatch {
        expected: Vec<DamageAssignmentId>,
        actual: Vec<DamageAssignmentId>,
    },
    ChoiceForUnknownAssignment {
        assignment: DamageAssignmentId,
    },
    ChoiceForUnknownModifier {
        modifier: DamageModifierId,
    },
    RepeatedModifierDecision {
        assignment: DamageAssignmentId,
        modifier: DamageModifierId,
    },
    MissingModifierChoice {
        assignment: DamageAssignmentId,
        chooser: PlayerId,
        applicable: Vec<DamageModifierId>,
    },
    DeclinedMandatoryModifier {
        assignment: DamageAssignmentId,
        modifier: DamageModifierId,
    },
    WrongModifierChooser {
        assignment: DamageAssignmentId,
        expected: PlayerId,
        actual: PlayerId,
    },
    InapplicableModifierChoice {
        assignment: DamageAssignmentId,
        modifier: DamageModifierId,
        applicable: Vec<DamageModifierId>,
    },
    UnexpectedModifierChoice {
        assignment: DamageAssignmentId,
        modifier: DamageModifierId,
    },
    ArithmeticOverflow {
        operation: &'static str,
    },
}

impl fmt::Display for DamageTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DamageTransactionError {}

#[derive(Debug, Clone, Copy)]
struct StagedDamagePacket {
    recipient: DamageRecipient,
    amount: u32,
}

pub fn execute_damage_transaction(
    state: &mut DamageRuntimeState,
    request: &DamageTransactionRequest,
) -> Result<DamageTransactionReceipt, DamageTransactionError> {
    let assignments = preflight_damage_transaction(state, request)?;
    let assignment_lookup = assignments
        .iter()
        .copied()
        .map(|assignment| (assignment.id, assignment))
        .collect::<BTreeMap<_, _>>();
    let mut choices = choices_by_assignment(&request.choices);
    let mut staged = state.clone();
    let mut receipts = Vec::with_capacity(assignments.len());
    let mut total_actual_damage = 0u64;

    for assignment_id in &request.choices.assignment_order {
        let assignment = assignment_lookup
            .get(assignment_id)
            .copied()
            .expect("preflight validates the complete assignment order");
        let mut packet = StagedDamagePacket {
            recipient: assignment.recipient,
            amount: assignment.amount,
        };
        let mut applied = BTreeSet::new();
        let mut modifier_decisions = Vec::new();
        let mut prevented_damage = 0u64;
        let assignment_choices = choices
            .get_mut(assignment_id)
            .expect("preflight creates one choice queue per assignment");

        loop {
            if packet.amount == 0 {
                break;
            }
            let applicable = applicable_modifier_ids(
                &staged,
                &request.source,
                request.kind,
                request.preventability,
                packet,
                &applied,
            )?;
            if applicable.is_empty() {
                break;
            }
            let chooser = affected_controller(&staged, packet.recipient)?;
            let Some(choice) = assignment_choices.pop_front() else {
                return Err(DamageTransactionError::MissingModifierChoice {
                    assignment: *assignment_id,
                    chooser,
                    applicable,
                });
            };
            if choice.chooser != chooser {
                return Err(DamageTransactionError::WrongModifierChooser {
                    assignment: *assignment_id,
                    expected: chooser,
                    actual: choice.chooser,
                });
            }
            if !applicable.contains(&choice.modifier) {
                return Err(DamageTransactionError::InapplicableModifierChoice {
                    assignment: *assignment_id,
                    modifier: choice.modifier,
                    applicable,
                });
            }
            let decision = match choice.decision {
                DamageModifierDecision::Apply => {
                    apply_damage_modifier(&mut staged, packet, choice, &mut prevented_damage)?
                }
                DamageModifierDecision::Decline => {
                    decline_damage_modifier(&staged, packet, choice)?
                }
            };
            packet.recipient = decision.recipient_after;
            packet.amount = decision.amount_after;
            applied.insert(choice.modifier);
            modifier_decisions.push(decision);
        }

        if let Some(choice) = assignment_choices.front() {
            return Err(DamageTransactionError::UnexpectedModifierChoice {
                assignment: *assignment_id,
                modifier: choice.modifier,
            });
        }

        let consequence = apply_damage_consequence(
            &mut staged,
            &request.source,
            packet.recipient,
            packet.amount,
        )?;
        total_actual_damage = total_actual_damage
            .checked_add(u64::from(packet.amount))
            .ok_or(DamageTransactionError::ArithmeticOverflow {
                operation: "sum actual damage",
            })?;
        receipts.push(DamageAssignmentReceipt {
            assignment: *assignment_id,
            original_recipient: assignment.recipient,
            final_recipient: packet.recipient,
            original_amount: assignment.amount,
            actual_damage: packet.amount,
            prevented_damage,
            modifier_decisions,
            consequence,
        });
    }

    let lifelink = apply_lifelink(&mut staged, &request.source, total_actual_damage)?;
    *state = staged;
    Ok(DamageTransactionReceipt {
        runtime_version: DAMAGE_TRANSACTION_RUNTIME_VERSION,
        semantic: request.semantic.clone(),
        source: request.source.clone(),
        kind: request.kind,
        preventability: request.preventability,
        assignments: receipts,
        total_actual_damage,
        lifelink,
    })
}

fn preflight_damage_transaction(
    state: &DamageRuntimeState,
    request: &DamageTransactionRequest,
) -> Result<Vec<DamageAssignment>, DamageTransactionError> {
    validate_damage_source(state, &request.source)?;
    let assignments = request.recipients.assignments().to_vec();
    validate_recipient_shape(&request.recipients)?;
    let mut assignment_ids = BTreeSet::new();
    let mut recipients = BTreeSet::new();
    for assignment in &assignments {
        if !assignment_ids.insert(assignment.id) {
            return Err(DamageTransactionError::DuplicateAssignment {
                assignment: assignment.id,
            });
        }
        if !recipients.insert(assignment.recipient) {
            return Err(DamageTransactionError::DuplicateRecipient {
                recipient: assignment.recipient,
            });
        }
        validate_recipient(state, assignment.recipient)?;
    }
    validate_selection(state, request.selection, &request.recipients, &assignments)?;
    validate_modifiers(state)?;
    validate_choice_plan(state, &assignments, &request.choices)?;
    Ok(assignments)
}

fn validate_damage_source(
    state: &DamageRuntimeState,
    source: &DamageSourceSnapshot,
) -> Result<(), DamageTransactionError> {
    if !state.players.contains_key(&source.controller) {
        return Err(DamageTransactionError::MissingSourceController {
            player: source.controller,
        });
    }
    match source.identity {
        DamageSourceIdentity::Player(player) => {
            if player != source.controller {
                return Err(DamageTransactionError::PlayerSourceControllerMismatch {
                    source: player,
                    controller: source.controller,
                });
            }
            if source.characteristics.kind != DamageSourceKind::Player {
                return Err(DamageTransactionError::PlayerSourceKindMismatch {
                    actual: source.characteristics.kind,
                });
            }
            if source.evidence != DamageSourceEvidence::CurrentCharacteristics {
                return Err(DamageTransactionError::PlayerSourceEvidenceMismatch {
                    actual: source.evidence,
                });
            }
        }
        DamageSourceIdentity::Object(object) => {
            if source.characteristics.kind == DamageSourceKind::Player {
                return Err(DamageTransactionError::ObjectSourceKindMismatch {
                    object,
                    actual: source.characteristics.kind,
                });
            }
            if source.characteristics.kind == DamageSourceKind::Creature
                && source.evidence == DamageSourceEvidence::CurrentCharacteristics
            {
                let Some(object_state) = state.objects.get(&object).copied() else {
                    return Err(DamageTransactionError::MissingCurrentCreatureSource { object });
                };
                if object_state.kind() != DamageRecipientKind::Creature {
                    return Err(DamageTransactionError::CurrentCreatureSourceKindMismatch {
                        object,
                        actual: object_state.kind(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_recipient_shape(recipients: &DamageRecipients) -> Result<(), DamageTransactionError> {
    match recipients {
        DamageRecipients::Single(_) => Ok(()),
        DamageRecipients::Set { kind, assignments } => {
            for assignment in assignments {
                let actual = assignment.recipient.kind();
                if actual != *kind {
                    return Err(DamageTransactionError::SetRecipientKindMismatch {
                        assignment: assignment.id,
                        declared: *kind,
                        actual,
                    });
                }
            }
            Ok(())
        }
        DamageRecipients::Mixed { assignments } => {
            if assignments.is_empty() {
                return Err(DamageTransactionError::EmptyMixedRecipients);
            }
            let kinds = assignments
                .iter()
                .map(|assignment| assignment.recipient.kind())
                .collect::<BTreeSet<_>>();
            if kinds.len() < 2 {
                return Err(DamageTransactionError::MixedRecipientsNeedMultipleKinds);
            }
            Ok(())
        }
    }
}

fn validate_recipient(
    state: &DamageRuntimeState,
    recipient: DamageRecipient,
) -> Result<(), DamageTransactionError> {
    match recipient {
        DamageRecipient::Player(player) => {
            if state.players.contains_key(&player) {
                Ok(())
            } else {
                Err(DamageTransactionError::MissingPlayer { player })
            }
        }
        DamageRecipient::Creature(object)
        | DamageRecipient::Planeswalker(object)
        | DamageRecipient::Battle(object) => {
            let Some(actual) = state.objects.get(&object).copied() else {
                return Err(DamageTransactionError::MissingObject { object });
            };
            let expected = recipient.kind();
            if actual.kind() == expected {
                Ok(())
            } else {
                Err(DamageTransactionError::ObjectKindMismatch {
                    object,
                    expected,
                    actual: actual.kind(),
                })
            }
        }
    }
}

fn validate_selection(
    state: &DamageRuntimeState,
    selection: DamageSelection,
    recipients: &DamageRecipients,
    assignments: &[DamageAssignment],
) -> Result<(), DamageTransactionError> {
    match selection {
        DamageSelection::Untargeted => Ok(()),
        DamageSelection::Targeted(legal) => {
            for assignment in assignments {
                if !legal_target_accepts(legal, assignment.recipient) {
                    return Err(DamageTransactionError::IllegalTarget {
                        assignment: assignment.id,
                        recipient: assignment.recipient,
                        legal,
                    });
                }
            }
            Ok(())
        }
        DamageSelection::DefinedSet(defined) => {
            if !matches!(recipients, DamageRecipients::Set { .. }) {
                return Err(DamageTransactionError::DefinedSetRequiresSetRecipients);
            }
            let expected = defined_set_recipients(state, defined);
            let actual = assignments
                .iter()
                .map(|assignment| assignment.recipient)
                .collect::<BTreeSet<_>>();
            if actual == expected {
                Ok(())
            } else {
                Err(DamageTransactionError::DefinedSetMismatch {
                    defined,
                    expected: expected.into_iter().collect(),
                    actual: actual.into_iter().collect(),
                })
            }
        }
    }
}

fn legal_target_accepts(legal: LegalDamageTargetKind, recipient: DamageRecipient) -> bool {
    match legal {
        LegalDamageTargetKind::Player => matches!(recipient, DamageRecipient::Player(_)),
        LegalDamageTargetKind::OpponentOf(player) => {
            matches!(recipient, DamageRecipient::Player(target) if target != player)
        }
        LegalDamageTargetKind::Creature => matches!(recipient, DamageRecipient::Creature(_)),
        LegalDamageTargetKind::Planeswalker => {
            matches!(recipient, DamageRecipient::Planeswalker(_))
        }
        LegalDamageTargetKind::Battle => matches!(recipient, DamageRecipient::Battle(_)),
        LegalDamageTargetKind::CreatureOrPlaneswalker => matches!(
            recipient,
            DamageRecipient::Creature(_) | DamageRecipient::Planeswalker(_)
        ),
        LegalDamageTargetKind::PlayerOrPlaneswalker => matches!(
            recipient,
            DamageRecipient::Player(_) | DamageRecipient::Planeswalker(_)
        ),
        LegalDamageTargetKind::AnyTarget => true,
    }
}

fn defined_set_recipients(
    state: &DamageRuntimeState,
    defined: DefinedDamageSet,
) -> BTreeSet<DamageRecipient> {
    match defined {
        DefinedDamageSet::EachPlayer => state
            .players
            .keys()
            .copied()
            .map(DamageRecipient::Player)
            .collect(),
        DefinedDamageSet::EachOpponentOf(player) => state
            .players
            .keys()
            .copied()
            .filter(|candidate| *candidate != player)
            .map(DamageRecipient::Player)
            .collect(),
        DefinedDamageSet::EachCreature => objects_of_kind(state, DamageRecipientKind::Creature),
        DefinedDamageSet::EachPlaneswalker => {
            objects_of_kind(state, DamageRecipientKind::Planeswalker)
        }
        DefinedDamageSet::EachBattle => objects_of_kind(state, DamageRecipientKind::Battle),
    }
}

fn objects_of_kind(
    state: &DamageRuntimeState,
    kind: DamageRecipientKind,
) -> BTreeSet<DamageRecipient> {
    state
        .objects
        .iter()
        .filter_map(|(object, object_state)| {
            if object_state.kind() != kind {
                return None;
            }
            Some(match kind {
                DamageRecipientKind::Creature => DamageRecipient::Creature(*object),
                DamageRecipientKind::Planeswalker => DamageRecipient::Planeswalker(*object),
                DamageRecipientKind::Battle => DamageRecipient::Battle(*object),
                DamageRecipientKind::Player => {
                    unreachable!("players are not stored in the object map")
                }
            })
        })
        .collect()
}

fn validate_modifiers(state: &DamageRuntimeState) -> Result<(), DamageTransactionError> {
    for (key, modifier) in &state.modifiers {
        if *key != modifier.id {
            return Err(DamageTransactionError::ModifierKeyMismatch {
                key: *key,
                modifier: modifier.id,
            });
        }
        if let Some(semantic) = unsupported_matcher_semantic(&modifier.matcher) {
            return Err(DamageTransactionError::UnsupportedModifierShape {
                modifier: modifier.id,
                semantic,
            });
        }
        match &modifier.operation {
            DamageModifierOperation::Replacement(DamageReplacement::Unsupported { semantic })
            | DamageModifierOperation::Prevention(DamagePrevention::Unsupported { semantic }) => {
                return Err(DamageTransactionError::UnsupportedModifierShape {
                    modifier: modifier.id,
                    semantic: semantic.clone(),
                });
            }
            DamageModifierOperation::Replacement(DamageReplacement::Redirect(recipient)) => {
                if validate_recipient(state, *recipient).is_err() {
                    return Err(DamageTransactionError::UnknownRedirectRecipient {
                        modifier: modifier.id,
                        recipient: *recipient,
                    });
                }
            }
            DamageModifierOperation::Prevention(DamagePrevention::PreventAmount(0)) => {
                return Err(DamageTransactionError::ZeroPreventAmount {
                    modifier: modifier.id,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn unsupported_matcher_semantic(matcher: &DamageEventMatcher) -> Option<String> {
    if let DamageSourceMatcher::Unsupported { semantic } = &matcher.source {
        return Some(semantic.clone());
    }
    if let DamageRecipientMatcher::Unsupported { semantic } = &matcher.recipient {
        return Some(semantic.clone());
    }
    match &matcher.kind {
        DamageKindMatcher::Unsupported { semantic } => Some(semantic.clone()),
        _ => None,
    }
}

fn validate_choice_plan(
    state: &DamageRuntimeState,
    assignments: &[DamageAssignment],
    choices: &DamageChoicePlan,
) -> Result<(), DamageTransactionError> {
    let expected = assignments
        .iter()
        .map(|assignment| assignment.id)
        .collect::<BTreeSet<_>>();
    let actual = choices
        .assignment_order
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if expected != actual || actual.len() != choices.assignment_order.len() {
        return Err(DamageTransactionError::AssignmentOrderMismatch {
            expected: expected.into_iter().collect(),
            actual: choices.assignment_order.clone(),
        });
    }
    let mut decided = BTreeSet::new();
    for choice in &choices.modifier_choices {
        if !actual.contains(&choice.assignment) {
            return Err(DamageTransactionError::ChoiceForUnknownAssignment {
                assignment: choice.assignment,
            });
        }
        if !state.modifiers.contains_key(&choice.modifier) {
            return Err(DamageTransactionError::ChoiceForUnknownModifier {
                modifier: choice.modifier,
            });
        }
        if !decided.insert((choice.assignment, choice.modifier)) {
            return Err(DamageTransactionError::RepeatedModifierDecision {
                assignment: choice.assignment,
                modifier: choice.modifier,
            });
        }
    }
    Ok(())
}

fn choices_by_assignment(
    plan: &DamageChoicePlan,
) -> BTreeMap<DamageAssignmentId, VecDeque<DamageModifierChoice>> {
    let mut choices = plan
        .assignment_order
        .iter()
        .copied()
        .map(|assignment| (assignment, VecDeque::new()))
        .collect::<BTreeMap<_, _>>();
    for choice in &plan.modifier_choices {
        choices
            .get_mut(&choice.assignment)
            .expect("preflight validates choice assignment identities")
            .push_back(*choice);
    }
    choices
}

fn applicable_modifier_ids(
    state: &DamageRuntimeState,
    source: &DamageSourceSnapshot,
    kind: DamageKind,
    preventability: DamagePreventability,
    packet: StagedDamagePacket,
    applied: &BTreeSet<DamageModifierId>,
) -> Result<Vec<DamageModifierId>, DamageTransactionError> {
    let mut applicable = Vec::new();
    for (id, modifier) in &state.modifiers {
        if applied.contains(id) || modifier_is_exhausted(modifier) {
            continue;
        }
        if preventability == DamagePreventability::CannotBePrevented
            && matches!(modifier.operation, DamageModifierOperation::Prevention(_))
        {
            continue;
        }
        if modifier_matches(state, modifier, source, kind, packet.recipient)? {
            applicable.push(*id);
        }
    }
    Ok(applicable)
}

fn modifier_is_exhausted(modifier: &DamageModifier) -> bool {
    matches!(
        modifier.operation,
        DamageModifierOperation::Prevention(DamagePrevention::Shield { remaining: 0 })
    )
}

fn modifier_matches(
    state: &DamageRuntimeState,
    modifier: &DamageModifier,
    source: &DamageSourceSnapshot,
    kind: DamageKind,
    recipient: DamageRecipient,
) -> Result<bool, DamageTransactionError> {
    let source_matches = match modifier.matcher.source {
        DamageSourceMatcher::Any => true,
        DamageSourceMatcher::Identity(identity) => source.identity == identity,
        DamageSourceMatcher::Controller(controller) => source.controller == controller,
        DamageSourceMatcher::HasKeyword(keyword) => source.characteristics.has_keyword(keyword),
        DamageSourceMatcher::Unsupported { ref semantic } => {
            return Err(DamageTransactionError::UnsupportedModifierShape {
                modifier: modifier.id,
                semantic: semantic.clone(),
            });
        }
    };
    let recipient_matches = match modifier.matcher.recipient {
        DamageRecipientMatcher::Any => true,
        DamageRecipientMatcher::Kind(expected) => recipient.kind() == expected,
        DamageRecipientMatcher::Exact(expected) => recipient == expected,
        DamageRecipientMatcher::ControlledBy(controller) => {
            affected_controller(state, recipient)? == controller
        }
        DamageRecipientMatcher::Unsupported { ref semantic } => {
            return Err(DamageTransactionError::UnsupportedModifierShape {
                modifier: modifier.id,
                semantic: semantic.clone(),
            });
        }
    };
    let kind_matches = match modifier.matcher.kind {
        DamageKindMatcher::Any => true,
        DamageKindMatcher::Combat => kind == DamageKind::Combat,
        DamageKindMatcher::Noncombat => kind == DamageKind::Noncombat,
        DamageKindMatcher::Unsupported { ref semantic } => {
            return Err(DamageTransactionError::UnsupportedModifierShape {
                modifier: modifier.id,
                semantic: semantic.clone(),
            });
        }
    };
    Ok(source_matches && recipient_matches && kind_matches)
}

fn affected_controller(
    state: &DamageRuntimeState,
    recipient: DamageRecipient,
) -> Result<PlayerId, DamageTransactionError> {
    match recipient {
        DamageRecipient::Player(player) => {
            if state.players.contains_key(&player) {
                Ok(player)
            } else {
                Err(DamageTransactionError::MissingPlayer { player })
            }
        }
        DamageRecipient::Creature(object)
        | DamageRecipient::Planeswalker(object)
        | DamageRecipient::Battle(object) => {
            let Some(object_state) = state.objects.get(&object).copied() else {
                return Err(DamageTransactionError::MissingObject { object });
            };
            Ok(object_state.controller())
        }
    }
}

fn apply_damage_modifier(
    state: &mut DamageRuntimeState,
    packet: StagedDamagePacket,
    choice: DamageModifierChoice,
    prevented_total: &mut u64,
) -> Result<DamageModifierDecisionReceipt, DamageTransactionError> {
    let modifier = state
        .modifiers
        .get(&choice.modifier)
        .cloned()
        .expect("preflight and applicability validate modifier identity");
    let mut after = packet;
    let mut prevented_damage = 0u32;
    let kind = match modifier.operation {
        DamageModifierOperation::Replacement(replacement) => {
            match replacement {
                DamageReplacement::Double => {
                    after.amount = after.amount.checked_mul(2).ok_or(
                        DamageTransactionError::ArithmeticOverflow {
                            operation: "double damage",
                        },
                    )?;
                }
                DamageReplacement::HalfRoundedDown => {
                    after.amount /= 2;
                }
                DamageReplacement::IncreaseBy(amount) => {
                    after.amount = after.amount.checked_add(amount).ok_or(
                        DamageTransactionError::ArithmeticOverflow {
                            operation: "increase damage",
                        },
                    )?;
                }
                DamageReplacement::SetAmount(amount) => {
                    after.amount = amount;
                }
                DamageReplacement::Redirect(recipient) => {
                    validate_recipient(state, recipient)?;
                    after.recipient = recipient;
                }
                DamageReplacement::Unsupported { semantic } => {
                    return Err(DamageTransactionError::UnsupportedModifierShape {
                        modifier: modifier.id,
                        semantic,
                    });
                }
            }
            AppliedDamageModifierKind::Replacement
        }
        DamageModifierOperation::Prevention(prevention) => {
            let maximum = match prevention {
                DamagePrevention::PreventAll => after.amount,
                DamagePrevention::PreventAmount(amount) => amount,
                DamagePrevention::Shield { remaining } => remaining,
                DamagePrevention::Unsupported { semantic } => {
                    return Err(DamageTransactionError::UnsupportedModifierShape {
                        modifier: modifier.id,
                        semantic,
                    });
                }
            };
            prevented_damage = maximum.min(after.amount);
            after.amount -= prevented_damage;
            if matches!(prevention, DamagePrevention::Shield { .. }) {
                let active = state
                    .modifiers
                    .get_mut(&modifier.id)
                    .expect("active shield remains staged until it is consumed");
                let DamageModifierOperation::Prevention(DamagePrevention::Shield { remaining }) =
                    &mut active.operation
                else {
                    unreachable!("cloned and staged modifier operations agree")
                };
                *remaining -= prevented_damage;
            }
            *prevented_total = prevented_total
                .checked_add(u64::from(prevented_damage))
                .ok_or(DamageTransactionError::ArithmeticOverflow {
                    operation: "sum prevented damage",
                })?;
            AppliedDamageModifierKind::Prevention
        }
    };

    let exhausted = state
        .modifiers
        .get(&modifier.id)
        .is_some_and(modifier_is_exhausted);
    if modifier.persistence == DamageModifierPersistence::OneShot || exhausted {
        state.modifiers.remove(&modifier.id);
    }

    Ok(DamageModifierDecisionReceipt {
        modifier: modifier.id,
        chooser: choice.chooser,
        decision: DamageModifierDecision::Apply,
        kind,
        recipient_before: packet.recipient,
        recipient_after: after.recipient,
        amount_before: packet.amount,
        amount_after: after.amount,
        prevented_damage,
    })
}

fn decline_damage_modifier(
    state: &DamageRuntimeState,
    packet: StagedDamagePacket,
    choice: DamageModifierChoice,
) -> Result<DamageModifierDecisionReceipt, DamageTransactionError> {
    let modifier = state
        .modifiers
        .get(&choice.modifier)
        .expect("preflight and applicability validate modifier identity");
    if modifier.requirement != DamageModifierRequirement::Optional {
        return Err(DamageTransactionError::DeclinedMandatoryModifier {
            assignment: choice.assignment,
            modifier: choice.modifier,
        });
    }
    let kind = match modifier.operation {
        DamageModifierOperation::Replacement(_) => AppliedDamageModifierKind::Replacement,
        DamageModifierOperation::Prevention(_) => AppliedDamageModifierKind::Prevention,
    };
    Ok(DamageModifierDecisionReceipt {
        modifier: modifier.id,
        chooser: choice.chooser,
        decision: DamageModifierDecision::Decline,
        kind,
        recipient_before: packet.recipient,
        recipient_after: packet.recipient,
        amount_before: packet.amount,
        amount_after: packet.amount,
        prevented_damage: 0,
    })
}

fn apply_damage_consequence(
    state: &mut DamageRuntimeState,
    source: &DamageSourceSnapshot,
    recipient: DamageRecipient,
    actual_damage: u32,
) -> Result<DamageConsequenceReceipt, DamageTransactionError> {
    if actual_damage == 0 {
        return Ok(DamageConsequenceReceipt::NoDamage);
    }
    let infect = source
        .characteristics
        .has_keyword(DamageSourceKeyword::Infect);
    let wither = source
        .characteristics
        .has_keyword(DamageSourceKeyword::Wither);
    let deathtouch = source
        .characteristics
        .has_keyword(DamageSourceKeyword::Deathtouch);

    match recipient {
        DamageRecipient::Player(player) => {
            let player_state = state
                .players
                .get_mut(&player)
                .ok_or(DamageTransactionError::MissingPlayer { player })?;
            if infect {
                let before = player_state.poison_counters;
                player_state.poison_counters = before.checked_add(actual_damage).ok_or(
                    DamageTransactionError::ArithmeticOverflow {
                        operation: "add poison counters",
                    },
                )?;
                Ok(DamageConsequenceReceipt::PlayerPoison {
                    player,
                    before,
                    after: player_state.poison_counters,
                })
            } else {
                let before = player_state.life;
                player_state.life = before.checked_sub(i64::from(actual_damage)).ok_or(
                    DamageTransactionError::ArithmeticOverflow {
                        operation: "subtract player life",
                    },
                )?;
                Ok(DamageConsequenceReceipt::PlayerLife {
                    player,
                    before,
                    after: player_state.life,
                })
            }
        }
        DamageRecipient::Creature(object) => {
            let Some(DamageObjectState::Creature(creature)) = state.objects.get_mut(&object) else {
                return validate_recipient(state, recipient).and_then(|()| unreachable!());
            };
            let deathtouch_before = creature.has_deathtouch_damage;
            if infect || wither {
                let before = creature.minus_one_minus_one_counters;
                creature.minus_one_minus_one_counters = before.checked_add(actual_damage).ok_or(
                    DamageTransactionError::ArithmeticOverflow {
                        operation: "add minus one counters",
                    },
                )?;
                creature.has_deathtouch_damage |= deathtouch;
                Ok(DamageConsequenceReceipt::CreatureMinusOneCounters {
                    object,
                    before,
                    after: creature.minus_one_minus_one_counters,
                    deathtouch_before,
                    deathtouch_after: creature.has_deathtouch_damage,
                })
            } else {
                let before = creature.marked_damage;
                creature.marked_damage = before.checked_add(actual_damage).ok_or(
                    DamageTransactionError::ArithmeticOverflow {
                        operation: "mark creature damage",
                    },
                )?;
                creature.has_deathtouch_damage |= deathtouch;
                Ok(DamageConsequenceReceipt::CreatureMarkedDamage {
                    object,
                    before,
                    after: creature.marked_damage,
                    deathtouch_before,
                    deathtouch_after: creature.has_deathtouch_damage,
                })
            }
        }
        DamageRecipient::Planeswalker(object) => {
            let Some(DamageObjectState::Planeswalker(planeswalker)) =
                state.objects.get_mut(&object)
            else {
                return validate_recipient(state, recipient).and_then(|()| unreachable!());
            };
            let before = planeswalker.loyalty;
            let counters_removed = before.min(actual_damage);
            planeswalker.loyalty -= counters_removed;
            Ok(DamageConsequenceReceipt::PlaneswalkerLoyalty {
                object,
                before,
                after: planeswalker.loyalty,
                counters_removed,
            })
        }
        DamageRecipient::Battle(object) => {
            let Some(DamageObjectState::Battle(battle)) = state.objects.get_mut(&object) else {
                return validate_recipient(state, recipient).and_then(|()| unreachable!());
            };
            let before = battle.defense;
            let counters_removed = before.min(actual_damage);
            battle.defense -= counters_removed;
            Ok(DamageConsequenceReceipt::BattleDefense {
                object,
                before,
                after: battle.defense,
                counters_removed,
            })
        }
    }
}

fn apply_lifelink(
    state: &mut DamageRuntimeState,
    source: &DamageSourceSnapshot,
    total_actual_damage: u64,
) -> Result<Option<LifelinkReceipt>, DamageTransactionError> {
    if total_actual_damage == 0
        || !source
            .characteristics
            .has_keyword(DamageSourceKeyword::Lifelink)
    {
        return Ok(None);
    }
    let gain = i64::try_from(total_actual_damage).map_err(|_| {
        DamageTransactionError::ArithmeticOverflow {
            operation: "convert lifelink damage",
        }
    })?;
    let player = state.players.get_mut(&source.controller).ok_or(
        DamageTransactionError::MissingSourceController {
            player: source.controller,
        },
    )?;
    let before = player.life;
    player.life = before
        .checked_add(gain)
        .ok_or(DamageTransactionError::ArithmeticOverflow {
            operation: "gain lifelink life",
        })?;
    Ok(Some(LifelinkReceipt {
        player: source.controller,
        actual_damage: total_actual_damage,
        life_before: before,
        life_after: player.life,
    }))
}
