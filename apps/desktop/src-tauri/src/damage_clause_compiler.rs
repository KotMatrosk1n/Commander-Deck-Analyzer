//! Content-keyed compiler for a narrow family of self-source damage clauses.
//!
//! This compiler deliberately has no production execution or coverage bridge.
//! It recognizes only complete Oracle clauses whose surrounding timing, cost,
//! trigger, restriction, and reminder syntax is already accepted by the
//! bounded grammar. A compiled template still requires explicit runtime
//! bindings before it can produce a typed damage transaction request.

use std::collections::BTreeSet;
use std::fmt;

use crate::bounded_oracle_runtime::{
    ActivationRestriction, BoundedOracleClause, Condition, Cost, OracleClauseInput, Timing,
    compile_bounded_oracle_clause_core, normalize_oracle_clause,
};
use crate::damage_transaction_runtime::{
    DamageAssignment, DamageAssignmentId, DamageChoicePlan, DamageKind, DamageModifierChoice,
    DamagePreventability, DamageRecipient, DamageRecipientKind, DamageRecipients, DamageSelection,
    DamageSemanticInput, DamageSemanticInputError, DamageSourceEvidence, DamageSourceIdentity,
    DamageSourceKind, DamageSourceSnapshot, DamageTransactionRequest, DefinedDamageSet,
    LegalDamageTargetKind,
};

pub const DAMAGE_CLAUSE_COMPILER_VERSION: &str = "damage-clause-compiler-0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DamageClauseEnvelope {
    SpellResolution,
    ActivatedAbility,
    TriggeredAbility,
}

impl DamageClauseEnvelope {
    const fn contract_tag(self) -> &'static str {
        match self {
            Self::SpellResolution => "spell-resolution",
            Self::ActivatedAbility => "activated-ability",
            Self::TriggeredAbility => "triggered-ability",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DamageAmountTemplate {
    Fixed(u32),
    X,
}

impl DamageAmountTemplate {
    fn contract_tag(self) -> String {
        match self {
            Self::Fixed(amount) => format!("fixed:{amount}"),
            Self::X => "x".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DamageRecipientTemplate {
    AnyTarget,
    TargetCreature,
    TargetCreatureOrPlaneswalker,
    TargetPlayer,
    TargetOpponent,
    TargetPlayerOrPlaneswalker,
    TargetOpponentOrPlaneswalker,
    TargetPlaneswalker,
    EachOpponent,
    EachPlayer,
    You,
    SourceItself,
}

impl DamageRecipientTemplate {
    const fn contract_tag(self) -> &'static str {
        match self {
            Self::AnyTarget => "any-target",
            Self::TargetCreature => "target-creature",
            Self::TargetCreatureOrPlaneswalker => "target-creature-or-planeswalker",
            Self::TargetPlayer => "target-player",
            Self::TargetOpponent => "target-opponent",
            Self::TargetPlayerOrPlaneswalker => "target-player-or-planeswalker",
            Self::TargetOpponentOrPlaneswalker => "target-opponent-or-planeswalker",
            Self::TargetPlaneswalker => "target-planeswalker",
            Self::EachOpponent => "each-opponent",
            Self::EachPlayer => "each-player",
            Self::You => "you",
            Self::SourceItself => "source-itself",
        }
    }

    const fn is_defined_set(self) -> bool {
        matches!(self, Self::EachOpponent | Self::EachPlayer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DamageClauseSourceSyntax {
    ExplicitThisObject,
    PronounIt,
}

impl DamageClauseSourceSyntax {
    const fn contract_tag(self) -> &'static str {
        match self {
            Self::ExplicitThisObject => "explicit-this-object",
            Self::PronounIt => "pronoun-it",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DamageSourceEvidenceTemplate {
    CurrentCharacteristics,
    LastKnownInformation,
    TriggerEventDetermined,
}

impl DamageSourceEvidenceTemplate {
    const fn contract_tag(self) -> &'static str {
        match self {
            Self::CurrentCharacteristics => "current-characteristics",
            Self::LastKnownInformation => "last-known-information",
            Self::TriggerEventDetermined => "trigger-event-determined",
        }
    }

    const fn accepts_direct(self, actual: DamageSourceEvidence) -> bool {
        match self {
            Self::CurrentCharacteristics => {
                matches!(actual, DamageSourceEvidence::CurrentCharacteristics)
            }
            Self::LastKnownInformation => {
                matches!(actual, DamageSourceEvidence::LastKnownInformation)
            }
            Self::TriggerEventDetermined => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageTriggerSourceState {
    Present,
    Departed,
}

impl DamageTriggerSourceState {
    const fn required_evidence(self) -> DamageSourceEvidence {
        match self {
            Self::Present => DamageSourceEvidence::CurrentCharacteristics,
            Self::Departed => DamageSourceEvidence::LastKnownInformation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DamageClauseShape {
    pub amount: DamageAmountTemplate,
    pub recipient: DamageRecipientTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageClauseInput<'a> {
    pub source_name: &'a str,
    pub source_type_line: &'a str,
    pub oracle_clause: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageRecipientBinding {
    pub assignment: DamageAssignmentId,
    pub recipient: DamageRecipient,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageClauseBindings {
    pub source: DamageSourceSnapshot,
    pub trigger_source_state: Option<DamageTriggerSourceState>,
    pub x_value: Option<u32>,
    pub recipients: Vec<DamageRecipientBinding>,
    pub modifier_choices: Vec<DamageModifierChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledDamageClause {
    source_clause: String,
    normalized_clause: String,
    semantic: DamageSemanticInput,
    envelope: DamageClauseEnvelope,
    source_syntax: DamageClauseSourceSyntax,
    source_evidence: DamageSourceEvidenceTemplate,
    amount: DamageAmountTemplate,
    recipient: DamageRecipientTemplate,
    wrapper: BoundedOracleClause,
}

impl CompiledDamageClause {
    pub fn compiler_version(&self) -> &'static str {
        DAMAGE_CLAUSE_COMPILER_VERSION
    }

    pub fn source_clause(&self) -> &str {
        &self.source_clause
    }

    pub fn normalized_clause(&self) -> &str {
        &self.normalized_clause
    }

    pub fn semantic_digest(&self) -> &str {
        self.semantic.semantic_digest()
    }

    pub fn envelope(&self) -> DamageClauseEnvelope {
        self.envelope
    }

    pub fn source_syntax(&self) -> DamageClauseSourceSyntax {
        self.source_syntax
    }

    pub fn source_evidence(&self) -> DamageSourceEvidenceTemplate {
        self.source_evidence
    }

    pub fn amount(&self) -> DamageAmountTemplate {
        self.amount
    }

    pub fn recipient(&self) -> DamageRecipientTemplate {
        self.recipient
    }

    pub fn timing(&self) -> &Timing {
        self.wrapper.timing()
    }

    pub fn costs(&self) -> &[Cost] {
        self.wrapper.costs()
    }

    pub fn conditions(&self) -> &[Condition] {
        self.wrapper.conditions()
    }

    pub fn activation_restriction(&self) -> Option<&ActivationRestriction> {
        self.wrapper.activation_restriction()
    }

    pub const fn damage_kind(&self) -> DamageKind {
        DamageKind::Noncombat
    }

    pub const fn preventability(&self) -> DamagePreventability {
        DamagePreventability::Preventable
    }

    pub const fn has_live_bridge(&self) -> bool {
        false
    }

    pub fn bind(
        &self,
        bindings: DamageClauseBindings,
    ) -> Result<DamageTransactionRequest, DamageClauseBindingError> {
        validate_source_binding(self, &bindings.source, bindings.trigger_source_state)?;
        let amount = match (self.amount, bindings.x_value) {
            (DamageAmountTemplate::Fixed(amount), None) => amount,
            (DamageAmountTemplate::Fixed(_), Some(value)) => {
                return Err(DamageClauseBindingError::UnexpectedXValue { value });
            }
            (DamageAmountTemplate::X, Some(value)) => value,
            (DamageAmountTemplate::X, None) => {
                return Err(DamageClauseBindingError::MissingXValue);
            }
        };
        validate_assignment_identity(&bindings.recipients)?;
        let (recipients, selection) = bind_recipients(
            self.recipient,
            &bindings.source,
            amount,
            &bindings.recipients,
        )?;
        let assignment_order = bindings
            .recipients
            .iter()
            .map(|binding| binding.assignment)
            .collect();
        Ok(DamageTransactionRequest {
            semantic: self.semantic.clone(),
            source: bindings.source,
            kind: DamageKind::Noncombat,
            preventability: DamagePreventability::Preventable,
            recipients,
            selection,
            choices: DamageChoicePlan {
                assignment_order,
                modifier_choices: bindings.modifier_choices,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageClauseCompileError {
    EmptySourceName,
    EmptySourceTypeLine,
    EmptyClause,
    AlreadyOwnedByBoundedCompiler,
    NotSimpleSelfSourceDamage,
    UnsupportedWrapper {
        envelope: DamageClauseEnvelope,
        shape: DamageClauseShape,
    },
    PronounDoesNotReferToSource {
        envelope: DamageClauseEnvelope,
        shape: DamageClauseShape,
    },
    SpellResolutionRequiresInstantOrSorcery,
    SourceItselfRequiresCreatureAbility,
    SemanticInput(DamageSemanticInputError),
}

impl fmt::Display for DamageClauseCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DamageClauseCompileError {}

impl From<DamageSemanticInputError> for DamageClauseCompileError {
    fn from(error: DamageSemanticInputError) -> Self {
        Self::SemanticInput(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageClauseBindingError {
    PlayerCannotBeDamageSource,
    ResolvingSpellSourceRequired {
        actual: DamageSourceKind,
    },
    SourceEvidenceMismatch {
        expected: DamageSourceEvidenceTemplate,
        actual: DamageSourceEvidence,
    },
    MissingTriggerSourceState,
    UnexpectedTriggerSourceState {
        actual: DamageTriggerSourceState,
    },
    TriggerSourceStateEvidenceMismatch {
        state: DamageTriggerSourceState,
        expected: DamageSourceEvidence,
        actual: DamageSourceEvidence,
    },
    SourceItselfRequiresCreatureSource {
        actual: DamageSourceKind,
    },
    UnexpectedXValue {
        value: u32,
    },
    MissingXValue,
    DuplicateAssignment {
        assignment: DamageAssignmentId,
    },
    DuplicateRecipient {
        recipient: DamageRecipient,
    },
    ExpectedSingleRecipient {
        actual: usize,
    },
    IllegalRecipient {
        expected: DamageRecipientTemplate,
        actual: DamageRecipient,
    },
}

impl fmt::Display for DamageClauseBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DamageClauseBindingError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedDamageInstruction {
    source_syntax: DamageClauseSourceSyntax,
    shape: DamageClauseShape,
}

pub fn compile_damage_clause(
    input: DamageClauseInput<'_>,
) -> Result<CompiledDamageClause, DamageClauseCompileError> {
    if input.source_name.trim().is_empty() {
        return Err(DamageClauseCompileError::EmptySourceName);
    }
    if input.source_type_line.trim().is_empty() {
        return Err(DamageClauseCompileError::EmptySourceTypeLine);
    }
    if input.oracle_clause.trim().is_empty() {
        return Err(DamageClauseCompileError::EmptyClause);
    }
    if compile_bounded_oracle_clause_core(OracleClauseInput {
        face_index: 0,
        clause_index: 0,
        source_name: input.source_name,
        source_type_line: input.source_type_line,
        oracle_clause: input.oracle_clause,
    })
    .is_ok()
    {
        return Err(DamageClauseCompileError::AlreadyOwnedByBoundedCompiler);
    }

    let normalized_clause = normalize_oracle_clause(
        input.oracle_clause,
        input.source_name,
        input.source_type_line,
    );
    let semantic_core = without_trailing_reminder(&normalized_clause);
    let (envelope, body) = effect_body(semantic_core);
    let instruction = strip_activation_restriction(body);
    let parsed = parse_damage_instruction(instruction)
        .ok_or(DamageClauseCompileError::NotSimpleSelfSourceDamage)?;
    let Some(wrapper) = compile_reviewed_wrapper(
        &normalized_clause,
        body,
        instruction,
        input.source_type_line,
    ) else {
        return Err(DamageClauseCompileError::UnsupportedWrapper {
            envelope,
            shape: parsed.shape,
        });
    };
    let body_offset = semantic_core
        .rfind(body)
        .expect("effect body is always a slice of the semantic core");
    let wrapper_prefix = &semantic_core[..body_offset];
    if parsed.source_syntax == DamageClauseSourceSyntax::PronounIt
        && !pronoun_refers_to_source(envelope, wrapper_prefix)
    {
        return Err(DamageClauseCompileError::PronounDoesNotReferToSource {
            envelope,
            shape: parsed.shape,
        });
    }
    validate_envelope_and_source_shape(envelope, parsed.shape, input.source_type_line)?;

    let source_evidence = source_evidence_template(envelope, wrapper_prefix);
    let canonical_program = vec![
        format!("compiler={DAMAGE_CLAUSE_COMPILER_VERSION}"),
        format!("envelope={}", envelope.contract_tag()),
        format!("source={}", parsed.source_syntax.contract_tag()),
        format!("source-evidence={}", source_evidence.contract_tag()),
        "damage-kind=noncombat".to_owned(),
        "preventability=preventable".to_owned(),
        format!("amount={}", parsed.shape.amount.contract_tag()),
        format!("recipient={}", parsed.shape.recipient.contract_tag()),
        format!("wrapper-timing={:?}", wrapper.timing()),
        format!("wrapper-costs={:?}", wrapper.costs()),
        format!("wrapper-conditions={:?}", wrapper.conditions()),
        format!(
            "wrapper-activation-restriction={:?}",
            wrapper.activation_restriction()
        ),
    ];
    let semantic_oracle = normalized_clause.trim().to_owned();
    let semantic = DamageSemanticInput::from_content(
        semantic_oracle.clone(),
        semantic_oracle.to_ascii_lowercase(),
        canonical_program,
    )?;
    Ok(CompiledDamageClause {
        source_clause: input.oracle_clause.trim().to_owned(),
        normalized_clause,
        semantic,
        envelope,
        source_syntax: parsed.source_syntax,
        source_evidence,
        amount: parsed.shape.amount,
        recipient: parsed.shape.recipient,
        wrapper,
    })
}

fn validate_envelope_and_source_shape(
    envelope: DamageClauseEnvelope,
    shape: DamageClauseShape,
    source_type_line: &str,
) -> Result<(), DamageClauseCompileError> {
    if envelope == DamageClauseEnvelope::SpellResolution
        && !source_has_card_type(source_type_line, "Instant")
        && !source_has_card_type(source_type_line, "Sorcery")
    {
        return Err(DamageClauseCompileError::SpellResolutionRequiresInstantOrSorcery);
    }
    if shape.recipient == DamageRecipientTemplate::SourceItself
        && (envelope == DamageClauseEnvelope::SpellResolution
            || !source_has_card_type(source_type_line, "Creature"))
    {
        return Err(DamageClauseCompileError::SourceItselfRequiresCreatureAbility);
    }
    Ok(())
}

fn source_has_card_type(source_type_line: &str, expected: &str) -> bool {
    let card_types = source_type_line
        .split_once('\u{2014}')
        .map(|(card_types, _)| card_types)
        .or_else(|| {
            source_type_line
                .split_once(" - ")
                .map(|(card_types, _)| card_types)
        })
        .unwrap_or(source_type_line);
    card_types
        .split_whitespace()
        .any(|card_type| card_type.eq_ignore_ascii_case(expected))
}

fn validate_source_binding(
    template: &CompiledDamageClause,
    source: &DamageSourceSnapshot,
    trigger_source_state: Option<DamageTriggerSourceState>,
) -> Result<(), DamageClauseBindingError> {
    if matches!(source.identity, DamageSourceIdentity::Player(_))
        || source.characteristics.kind == DamageSourceKind::Player
    {
        return Err(DamageClauseBindingError::PlayerCannotBeDamageSource);
    }
    if template.envelope == DamageClauseEnvelope::SpellResolution
        && source.characteristics.kind != DamageSourceKind::Spell
    {
        return Err(DamageClauseBindingError::ResolvingSpellSourceRequired {
            actual: source.characteristics.kind,
        });
    }
    if template.recipient == DamageRecipientTemplate::SourceItself
        && source.characteristics.kind != DamageSourceKind::Creature
    {
        return Err(
            DamageClauseBindingError::SourceItselfRequiresCreatureSource {
                actual: source.characteristics.kind,
            },
        );
    }
    match (template.source_evidence, trigger_source_state) {
        (DamageSourceEvidenceTemplate::TriggerEventDetermined, None) => {
            return Err(DamageClauseBindingError::MissingTriggerSourceState);
        }
        (DamageSourceEvidenceTemplate::TriggerEventDetermined, Some(state)) => {
            let expected = state.required_evidence();
            if source.evidence != expected {
                return Err(
                    DamageClauseBindingError::TriggerSourceStateEvidenceMismatch {
                        state,
                        expected,
                        actual: source.evidence,
                    },
                );
            }
        }
        (_, Some(actual)) => {
            return Err(DamageClauseBindingError::UnexpectedTriggerSourceState { actual });
        }
        (expected, None) if !expected.accepts_direct(source.evidence) => {
            return Err(DamageClauseBindingError::SourceEvidenceMismatch {
                expected,
                actual: source.evidence,
            });
        }
        (_, None) => {}
    }
    Ok(())
}

fn validate_assignment_identity(
    bindings: &[DamageRecipientBinding],
) -> Result<(), DamageClauseBindingError> {
    let mut assignment_ids = BTreeSet::new();
    let mut recipients = BTreeSet::new();
    for binding in bindings {
        if !assignment_ids.insert(binding.assignment) {
            return Err(DamageClauseBindingError::DuplicateAssignment {
                assignment: binding.assignment,
            });
        }
        if !recipients.insert(binding.recipient) {
            return Err(DamageClauseBindingError::DuplicateRecipient {
                recipient: binding.recipient,
            });
        }
    }
    Ok(())
}

fn bind_recipients(
    template: DamageRecipientTemplate,
    source: &DamageSourceSnapshot,
    amount: u32,
    bindings: &[DamageRecipientBinding],
) -> Result<(DamageRecipients, DamageSelection), DamageClauseBindingError> {
    if template.is_defined_set() {
        for binding in bindings {
            if !recipient_matches_template(template, source, binding.recipient) {
                return Err(DamageClauseBindingError::IllegalRecipient {
                    expected: template,
                    actual: binding.recipient,
                });
            }
        }
        let assignments = bindings
            .iter()
            .map(|binding| DamageAssignment {
                id: binding.assignment,
                recipient: binding.recipient,
                amount,
            })
            .collect();
        let defined = match template {
            DamageRecipientTemplate::EachOpponent => {
                DefinedDamageSet::EachOpponentOf(source.controller)
            }
            DamageRecipientTemplate::EachPlayer => DefinedDamageSet::EachPlayer,
            _ => unreachable!("defined set templates are handled above"),
        };
        return Ok((
            DamageRecipients::Set {
                kind: DamageRecipientKind::Player,
                assignments,
            },
            DamageSelection::DefinedSet(defined),
        ));
    }
    let [binding] = bindings else {
        return Err(DamageClauseBindingError::ExpectedSingleRecipient {
            actual: bindings.len(),
        });
    };
    if !recipient_matches_template(template, source, binding.recipient) {
        return Err(DamageClauseBindingError::IllegalRecipient {
            expected: template,
            actual: binding.recipient,
        });
    }
    let assignment = DamageAssignment {
        id: binding.assignment,
        recipient: binding.recipient,
        amount,
    };
    let selection = match template {
        DamageRecipientTemplate::AnyTarget => {
            DamageSelection::Targeted(LegalDamageTargetKind::AnyTarget)
        }
        DamageRecipientTemplate::TargetCreature => {
            DamageSelection::Targeted(LegalDamageTargetKind::Creature)
        }
        DamageRecipientTemplate::TargetCreatureOrPlaneswalker => {
            DamageSelection::Targeted(LegalDamageTargetKind::CreatureOrPlaneswalker)
        }
        DamageRecipientTemplate::TargetPlayer => {
            DamageSelection::Targeted(LegalDamageTargetKind::Player)
        }
        DamageRecipientTemplate::TargetOpponent => {
            DamageSelection::Targeted(LegalDamageTargetKind::OpponentOf(source.controller))
        }
        DamageRecipientTemplate::TargetPlayerOrPlaneswalker
        | DamageRecipientTemplate::TargetOpponentOrPlaneswalker => {
            DamageSelection::Targeted(LegalDamageTargetKind::PlayerOrPlaneswalker)
        }
        DamageRecipientTemplate::TargetPlaneswalker => {
            DamageSelection::Targeted(LegalDamageTargetKind::Planeswalker)
        }
        DamageRecipientTemplate::You => DamageSelection::Untargeted,
        DamageRecipientTemplate::SourceItself => DamageSelection::Untargeted,
        DamageRecipientTemplate::EachOpponent | DamageRecipientTemplate::EachPlayer => {
            unreachable!("defined set templates return above")
        }
    };
    Ok((DamageRecipients::Single(assignment), selection))
}

fn recipient_matches_template(
    template: DamageRecipientTemplate,
    source: &DamageSourceSnapshot,
    recipient: DamageRecipient,
) -> bool {
    match template {
        DamageRecipientTemplate::AnyTarget => true,
        DamageRecipientTemplate::TargetCreature => {
            matches!(recipient, DamageRecipient::Creature(_))
        }
        DamageRecipientTemplate::TargetCreatureOrPlaneswalker => matches!(
            recipient,
            DamageRecipient::Creature(_) | DamageRecipient::Planeswalker(_)
        ),
        DamageRecipientTemplate::TargetPlayer => {
            matches!(recipient, DamageRecipient::Player(_))
        }
        DamageRecipientTemplate::TargetOpponent => {
            matches!(recipient, DamageRecipient::Player(player) if player != source.controller)
        }
        DamageRecipientTemplate::TargetPlayerOrPlaneswalker => matches!(
            recipient,
            DamageRecipient::Player(_) | DamageRecipient::Planeswalker(_)
        ),
        DamageRecipientTemplate::TargetOpponentOrPlaneswalker => match recipient {
            DamageRecipient::Player(player) => player != source.controller,
            DamageRecipient::Planeswalker(_) => true,
            _ => false,
        },
        DamageRecipientTemplate::TargetPlaneswalker => {
            matches!(recipient, DamageRecipient::Planeswalker(_))
        }
        DamageRecipientTemplate::EachOpponent => {
            matches!(recipient, DamageRecipient::Player(player) if player != source.controller)
        }
        DamageRecipientTemplate::EachPlayer => {
            matches!(recipient, DamageRecipient::Player(_))
        }
        DamageRecipientTemplate::You => {
            matches!(recipient, DamageRecipient::Player(player) if player == source.controller)
        }
        DamageRecipientTemplate::SourceItself => match source.identity {
            DamageSourceIdentity::Object(source_object) => {
                matches!(recipient, DamageRecipient::Creature(object) if object == source_object)
            }
            DamageSourceIdentity::Player(_) => false,
        },
    }
}

fn compile_reviewed_wrapper(
    normalized_clause: &str,
    body: &str,
    instruction: &str,
    source_type_line: &str,
) -> Option<BoundedOracleClause> {
    let offset = normalized_clause.rfind(body)?;
    if !body.starts_with(instruction) {
        return None;
    }
    let suffix_start = offset + instruction.len();
    let surrogate = format!(
        "{}You gain 1 life.{}",
        &normalized_clause[..offset],
        &normalized_clause[suffix_start..]
    );
    compile_bounded_oracle_clause_core(OracleClauseInput {
        face_index: 0,
        clause_index: 0,
        source_name: "Source",
        source_type_line,
        oracle_clause: &surrogate,
    })
    .ok()
}

fn top_level_index(text: &str, needle: char) -> Option<usize> {
    let mut parenthesis_depth = 0u32;
    let mut quoted = false;
    for (index, character) in text.char_indices() {
        match character {
            '"' => quoted = !quoted,
            '(' if !quoted => parenthesis_depth = parenthesis_depth.saturating_add(1),
            ')' if !quoted => parenthesis_depth = parenthesis_depth.saturating_sub(1),
            _ if character == needle && parenthesis_depth == 0 && !quoted => {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn without_trailing_reminder(text: &str) -> &str {
    let trimmed = text.trim();
    if !trimmed.ends_with(')') {
        return trimmed;
    }
    let mut parenthesis_depth = 0u32;
    let mut quoted = false;
    for (index, character) in trimmed.char_indices().rev() {
        match character {
            '"' => quoted = !quoted,
            ')' if !quoted => parenthesis_depth = parenthesis_depth.saturating_add(1),
            '(' if !quoted => {
                parenthesis_depth = parenthesis_depth.saturating_sub(1);
                if parenthesis_depth == 0 && index > 0 && trimmed[..index].ends_with(' ') {
                    return trimmed[..index].trim_end();
                }
            }
            _ => {}
        }
    }
    trimmed
}

fn effect_body(semantic_core: &str) -> (DamageClauseEnvelope, &str) {
    let trimmed = semantic_core.trim();
    if let Some(colon) = top_level_index(trimmed, ':') {
        return (
            DamageClauseEnvelope::ActivatedAbility,
            trimmed[colon + 1..].trim(),
        );
    }
    let lower = trimmed.to_ascii_lowercase();
    if (lower.starts_with("when ") || lower.starts_with("whenever ") || lower.starts_with("at "))
        && let Some(comma) = top_level_index(trimmed, ',')
    {
        return (
            DamageClauseEnvelope::TriggeredAbility,
            trimmed[comma + 1..].trim(),
        );
    }
    (DamageClauseEnvelope::SpellResolution, trimmed)
}

fn strip_activation_restriction(body: &str) -> &str {
    let lower = body.to_ascii_lowercase();
    for marker in [
        " activate only ",
        " activate no more than ",
        " activate this ability only ",
    ] {
        if let Some(index) = lower.find(marker) {
            return body[..index].trim();
        }
    }
    body.trim()
}

fn parse_damage_instruction(instruction: &str) -> Option<ParsedDamageInstruction> {
    let instruction = instruction.trim();
    let sentence = instruction.strip_suffix('.')?;
    if sentence.ends_with('.') {
        return None;
    }
    let lower = sentence.to_ascii_lowercase();
    let (source_syntax, rest) = lower
        .strip_prefix("this object deals ")
        .map(|rest| (DamageClauseSourceSyntax::ExplicitThisObject, rest))
        .or_else(|| {
            lower
                .strip_prefix("it deals ")
                .map(|rest| (DamageClauseSourceSyntax::PronounIt, rest))
        })?;
    let (amount, recipient) = rest.split_once(" damage to ")?;
    if recipient.contains(" damage to ") {
        return None;
    }
    let amount = parse_damage_amount(amount)?;
    let recipient = match recipient {
        "any target" => DamageRecipientTemplate::AnyTarget,
        "target creature" => DamageRecipientTemplate::TargetCreature,
        "target creature or planeswalker" => DamageRecipientTemplate::TargetCreatureOrPlaneswalker,
        "target player" => DamageRecipientTemplate::TargetPlayer,
        "target opponent" => DamageRecipientTemplate::TargetOpponent,
        "target player or planeswalker" => DamageRecipientTemplate::TargetPlayerOrPlaneswalker,
        "target opponent or planeswalker" => DamageRecipientTemplate::TargetOpponentOrPlaneswalker,
        "target planeswalker" => DamageRecipientTemplate::TargetPlaneswalker,
        "each opponent" => DamageRecipientTemplate::EachOpponent,
        "each player" => DamageRecipientTemplate::EachPlayer,
        "you" => DamageRecipientTemplate::You,
        "itself" => DamageRecipientTemplate::SourceItself,
        _ => return None,
    };
    Some(ParsedDamageInstruction {
        source_syntax,
        shape: DamageClauseShape { amount, recipient },
    })
}

fn parse_damage_amount(amount: &str) -> Option<DamageAmountTemplate> {
    if amount == "x" {
        return Some(DamageAmountTemplate::X);
    }
    let fixed = match amount {
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        "eleven" => 11,
        "twelve" => 12,
        "thirteen" => 13,
        "fourteen" => 14,
        "fifteen" => 15,
        "sixteen" => 16,
        "seventeen" => 17,
        "eighteen" => 18,
        "nineteen" => 19,
        "twenty" => 20,
        _ => parse_printed_number(amount)?,
    };
    (fixed > 0).then_some(DamageAmountTemplate::Fixed(fixed))
}

fn parse_printed_number(amount: &str) -> Option<u32> {
    if amount.is_empty() {
        return None;
    }
    if !amount.contains(',') {
        if amount.len() > 3
            || (amount.len() > 1 && amount.starts_with('0'))
            || !amount.chars().all(|character| character.is_ascii_digit())
        {
            return None;
        }
        return amount.parse().ok();
    }
    let mut groups = amount.split(',');
    let first = groups.next()?;
    if first.is_empty()
        || first.len() > 3
        || first.starts_with('0')
        || !first.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    let rest = groups.collect::<Vec<_>>();
    if rest.is_empty()
        || rest.iter().any(|group| {
            group.len() != 3 || !group.chars().all(|character| character.is_ascii_digit())
        })
    {
        return None;
    }
    amount.replace(',', "").parse().ok()
}

fn pronoun_refers_to_source(envelope: DamageClauseEnvelope, wrapper_prefix: &str) -> bool {
    let lower = wrapper_prefix.trim().to_ascii_lowercase();
    match envelope {
        DamageClauseEnvelope::SpellResolution => false,
        DamageClauseEnvelope::ActivatedAbility => {
            lower.contains("this object") || lower.contains("this equipment")
        }
        DamageClauseEnvelope::TriggeredAbility => {
            let trigger = ["whenever ", "when ", "at "]
                .into_iter()
                .filter_map(|marker| lower.rfind(marker).map(|index| (index, &lower[index..])))
                .max_by_key(|(index, _)| *index)
                .map(|(_, trigger)| trigger)
                .unwrap_or(lower.as_str());
            trigger.starts_with("when this ")
                || trigger.starts_with("whenever this ")
                || trigger.starts_with("at this ")
                || trigger.starts_with("when you sacrifice this ")
                || trigger.starts_with("when you cycle this ")
                || trigger.starts_with("when you discard this ")
        }
    }
}

fn source_evidence_template(
    envelope: DamageClauseEnvelope,
    wrapper_prefix: &str,
) -> DamageSourceEvidenceTemplate {
    let lower = wrapper_prefix.trim().to_ascii_lowercase();
    match envelope {
        DamageClauseEnvelope::SpellResolution => {
            DamageSourceEvidenceTemplate::CurrentCharacteristics
        }
        DamageClauseEnvelope::ActivatedAbility => {
            if source_leaves_as_activation_cost(&lower) {
                DamageSourceEvidenceTemplate::LastKnownInformation
            } else {
                DamageSourceEvidenceTemplate::CurrentCharacteristics
            }
        }
        DamageClauseEnvelope::TriggeredAbility => {
            if source_departure_is_one_of_multiple_trigger_events(&lower) {
                DamageSourceEvidenceTemplate::TriggerEventDetermined
            } else if trigger_requires_departed_source(&lower) {
                DamageSourceEvidenceTemplate::LastKnownInformation
            } else {
                DamageSourceEvidenceTemplate::CurrentCharacteristics
            }
        }
    }
}

fn source_leaves_as_activation_cost(lower: &str) -> bool {
    [
        "sacrifice this object",
        "sacrifice this equipment",
        "discard this object",
        "exile this object",
        "return this object",
    ]
    .into_iter()
    .any(|marker| lower.contains(marker))
}

fn source_departure_is_one_of_multiple_trigger_events(lower: &str) -> bool {
    (lower.contains("this object or another ") && lower.contains(" dies"))
        || lower.contains("this object enters or leaves")
}

fn trigger_requires_departed_source(lower: &str) -> bool {
    [
        "this object dies",
        "this object leaves the battlefield",
        "this object is put into a graveyard",
        "you sacrifice this object",
        "you cycle this object",
        "you discard this object",
    ]
    .into_iter()
    .any(|marker| lower.contains(marker))
}
