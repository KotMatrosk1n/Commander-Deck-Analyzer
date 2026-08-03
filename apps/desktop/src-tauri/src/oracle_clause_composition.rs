//! Lossless, content keyed composition for complete Oracle instructions.
//!
//! This module recognizes composition only. Recognition never authorizes an
//! instruction to execute. A composition plan can be prepared only when every
//! semantic requirement exposed by the structure is covered by exactly one
//! already typed child program with matching exact source text.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const ORACLE_CLAUSE_COMPOSITION_COMPILER_VERSION: &str =
    "oracle-clause-composition-compiler-0.3";
pub const ORACLE_CLAUSE_COMPOSITION_RUNTIME_VERSION: &str = "oracle-clause-composition-runtime-0.2";
pub const ORACLE_CLAUSE_COMPOSITION_RULES_CONTEXT_VERSION: &str =
    "magic-comprehensive-rules-2026-06-19";

/// The production adapter stays disconnected until the runtime can execute
/// the complete typed composition atomically.
pub const fn oracle_clause_composition_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum OracleCompositionSemanticContext {
    #[default]
    CardFace,
    SpellAbility,
    PermanentAbility,
    Emblem,
    DungeonRoom,
    GrantedAbility,
}

impl OracleCompositionSemanticContext {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::CardFace => "card-face/v1",
            Self::SpellAbility => "spell-ability/v1",
            Self::PermanentAbility => "permanent-ability/v1",
            Self::Emblem => "emblem/v1",
            Self::DungeonRoom => "dungeon-room/v1",
            Self::GrantedAbility => "granted-ability/v1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleClauseCompositionInput<'a> {
    pub exact_oracle: &'a str,
    pub semantic_context: OracleCompositionSemanticContext,
}

impl<'a> OracleClauseCompositionInput<'a> {
    pub const fn card_face(exact_oracle: &'a str) -> Self {
        Self {
            exact_oracle,
            semantic_context: OracleCompositionSemanticContext::CardFace,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    pub fn slice(self, source: &str) -> Option<&str> {
        source.get(self.start..self.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SemanticCapability {
    SemanticAtom,
    Condition,
    TargetSelection,
    CostPayment,
    TimingWindow,
    DelayedTrigger,
    ReplacementEffect,
    OptionalChoice,
    ModalSelection,
    NestedGrantedAbility,
}

impl SemanticCapability {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::SemanticAtom => "semantic-atom",
            Self::Condition => "condition",
            Self::TargetSelection => "target-selection",
            Self::CostPayment => "cost-payment",
            Self::TimingWindow => "timing-window",
            Self::DelayedTrigger => "delayed-trigger",
            Self::ReplacementEffect => "replacement-effect",
            Self::OptionalChoice => "optional-choice",
            Self::ModalSelection => "modal-selection",
            Self::NestedGrantedAbility => "nested-granted-ability",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticRequirement {
    pub ordinal: u32,
    pub span: SourceSpan,
    pub capability: SemanticCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceSeparatorKind {
    SentenceBoundary,
    Then,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceSeparator {
    pub kind: SequenceSeparatorKind,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConjunctionKind {
    And,
    But,
    Comma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConjunctionSeparator {
    pub kind: ConjunctionKind,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlternativeKind {
    Or,
    AndOr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlternativeSeparator {
    pub kind: AlternativeKind,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalKind {
    If,
    Unless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalSelection {
    Exactly(u8),
    UpTo(u8),
    OneOrMore,
    OneOrBoth,
    AnyNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotedFragmentKind {
    GrantedAbility,
    QuotedAbility,
    LiteralText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotedFragment {
    pub quote_span: SourceSpan,
    pub content_span: SourceSpan,
    pub kind: QuotedFragmentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomNode {
    pub span: SourceSpan,
    pub required_capabilities: Vec<SemanticCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModalBranch {
    pub marker_span: SourceSpan,
    pub body_span: SourceSpan,
    pub body: OracleCompositionNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedAbility {
    pub quote_span: SourceSpan,
    pub content_span: SourceSpan,
    pub kind: QuotedFragmentKind,
    pub body: Box<OracleCompositionNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleCompositionNode {
    Atom(AtomNode),
    Sequence {
        span: SourceSpan,
        parts: Vec<OracleCompositionNode>,
        separators: Vec<SequenceSeparator>,
    },
    Conjunction {
        span: SourceSpan,
        parts: Vec<OracleCompositionNode>,
        separators: Vec<ConjunctionSeparator>,
    },
    Alternative {
        span: SourceSpan,
        parts: Vec<OracleCompositionNode>,
        separators: Vec<AlternativeSeparator>,
    },
    Conditional {
        span: SourceSpan,
        kind: ConditionalKind,
        marker_span: SourceSpan,
        condition: Box<OracleCompositionNode>,
        consequence: Box<OracleCompositionNode>,
        otherwise_marker_span: Option<SourceSpan>,
        otherwise_body: Option<Box<OracleCompositionNode>>,
    },
    OptionalChoice {
        span: SourceSpan,
        actor_span: SourceSpan,
        may_span: SourceSpan,
        body: Box<OracleCompositionNode>,
    },
    ActivatedAbility {
        span: SourceSpan,
        cost: AtomNode,
        colon_span: SourceSpan,
        instruction: Box<OracleCompositionNode>,
    },
    DelayedInstruction {
        span: SourceSpan,
        schedule: AtomNode,
        comma_span: SourceSpan,
        instruction: Box<OracleCompositionNode>,
    },
    ModalGroup {
        span: SourceSpan,
        header_span: SourceSpan,
        selection: Option<ModalSelection>,
        branches: Vec<ModalBranch>,
    },
    DetachedModalBranch {
        span: SourceSpan,
        marker_span: SourceSpan,
        body: Box<OracleCompositionNode>,
    },
    EmbeddedAbilities {
        span: SourceSpan,
        outer: Box<OracleCompositionNode>,
        abilities: Vec<EmbeddedAbility>,
    },
}

impl OracleCompositionNode {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Atom(atom) => atom.span,
            Self::Sequence { span, .. }
            | Self::Conjunction { span, .. }
            | Self::Alternative { span, .. }
            | Self::Conditional { span, .. }
            | Self::OptionalChoice { span, .. }
            | Self::ActivatedAbility { span, .. }
            | Self::DelayedInstruction { span, .. }
            | Self::ModalGroup { span, .. }
            | Self::DetachedModalBranch { span, .. }
            | Self::EmbeddedAbilities { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StructuralExclusionKind {
    IncompleteModalHeader,
    DetachedModalBranch,
    DetachedOtherwise,
    UnrecognizedModalSelection,
    EmptyModalBranch,
    AmbiguousMixedConnectors,
    MaximumNestingDepth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StructuralExclusion {
    pub kind: StructuralExclusionKind,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleClauseComposition {
    exact_oracle: String,
    semantic_context: OracleCompositionSemanticContext,
    structural_digest: String,
    root: OracleCompositionNode,
    requirements: Vec<SemanticRequirement>,
    quoted_fragments: Vec<QuotedFragment>,
    exclusions: Vec<StructuralExclusion>,
}

impl OracleClauseComposition {
    pub fn exact_oracle(&self) -> &str {
        &self.exact_oracle
    }

    pub const fn semantic_context(&self) -> OracleCompositionSemanticContext {
        self.semantic_context
    }

    pub fn structural_digest(&self) -> &str {
        &self.structural_digest
    }

    pub fn root(&self) -> &OracleCompositionNode {
        &self.root
    }

    pub fn requirements(&self) -> &[SemanticRequirement] {
        &self.requirements
    }

    pub fn quoted_fragments(&self) -> &[QuotedFragment] {
        &self.quoted_fragments
    }

    pub fn exclusions(&self) -> &[StructuralExclusion] {
        &self.exclusions
    }

    pub fn reconstruct(&self) -> &str {
        &self.exact_oracle
    }

    pub const fn production_adapter_connected(&self) -> bool {
        oracle_clause_composition_production_adapter_connected()
    }

    /// Prepare an all-or-nothing typed composition. No partial plan is
    /// returned when even one semantic requirement is unresolved.
    pub fn compose_typed_children(
        &self,
        bindings: &[TypedChildBinding<'_>],
    ) -> Result<TypedOracleComposition, CompositionAssemblyError> {
        if !self.exclusions.is_empty() {
            return Err(CompositionAssemblyError::UnresolvedStructure(
                self.exclusions.clone(),
            ));
        }

        for (binding_index, binding) in bindings.iter().enumerate() {
            let Some(expected_source) = binding.span.slice(&self.exact_oracle) else {
                return Err(CompositionAssemblyError::InvalidChildSpan {
                    binding_index,
                    span: binding.span,
                });
            };
            if expected_source != binding.program.exact_source() {
                return Err(CompositionAssemblyError::ChildSourceMismatch {
                    binding_index,
                    span: binding.span,
                });
            }
            if !valid_semantic_digest(binding.program.semantic_digest()) {
                return Err(CompositionAssemblyError::InvalidChildSemanticDigest { binding_index });
            }
        }

        let mut requirement_binding = Vec::<usize>::with_capacity(self.requirements.len());
        let mut unresolved = Vec::new();
        for requirement in &self.requirements {
            let matches = bindings
                .iter()
                .enumerate()
                .filter_map(|(binding_index, binding)| {
                    (binding.span == requirement.span
                        && binding
                            .program
                            .capabilities()
                            .contains(&requirement.capability))
                    .then_some(binding_index)
                })
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [binding_index] => requirement_binding.push(*binding_index),
                [] => unresolved.push(requirement.clone()),
                _ => {
                    return Err(CompositionAssemblyError::AmbiguousChildCoverage {
                        requirement: requirement.clone(),
                        binding_indices: matches,
                    });
                }
            }
        }
        if !unresolved.is_empty() {
            return Err(CompositionAssemblyError::UnresolvedRequirements(unresolved));
        }

        let used_bindings = requirement_binding.iter().copied().collect::<BTreeSet<_>>();
        let unexpected_bindings = (0..bindings.len())
            .filter(|binding_index| !used_bindings.contains(binding_index))
            .collect::<Vec<_>>();
        if !unexpected_bindings.is_empty() {
            return Err(CompositionAssemblyError::UnexpectedChildBindings(
                unexpected_bindings,
            ));
        }

        let mut ordered_bindings = used_bindings.into_iter().collect::<Vec<_>>();
        ordered_bindings.sort_by_key(|binding_index| {
            let first_requirement = requirement_binding
                .iter()
                .position(|candidate| candidate == binding_index)
                .unwrap_or(usize::MAX);
            (first_requirement, *binding_index)
        });
        let child_semantic_digests = ordered_bindings
            .iter()
            .map(|binding_index| {
                bindings[*binding_index]
                    .program
                    .semantic_digest()
                    .to_owned()
            })
            .collect::<Vec<_>>();
        let binding_rank = ordered_bindings
            .iter()
            .copied()
            .enumerate()
            .map(|(rank, binding_index)| (binding_index, rank as u32))
            .collect::<BTreeMap<_, _>>();
        let requirement_child_indices = requirement_binding
            .iter()
            .map(|binding_index| {
                *binding_rank
                    .get(binding_index)
                    .expect("every used requirement binding has a canonical child rank")
            })
            .collect::<Vec<_>>();
        let children = ordered_bindings
            .iter()
            .map(|binding_index| {
                let binding = &bindings[*binding_index];
                let mut capabilities = binding.program.capabilities().to_vec();
                capabilities.sort();
                capabilities.dedup();
                TypedOracleCompositionChild {
                    span: binding.span,
                    exact_source: binding.program.exact_source().to_owned(),
                    semantic_digest: binding.program.semantic_digest().to_owned(),
                    capabilities,
                }
            })
            .collect();
        let semantic_digest =
            typed_composition_digest(self, bindings, &ordered_bindings, &requirement_binding);

        Ok(TypedOracleComposition {
            exact_oracle: self.exact_oracle.clone(),
            semantic_context: self.semantic_context,
            structural_digest: self.structural_digest.clone(),
            semantic_digest,
            root: self.root.clone(),
            requirements: self.requirements.clone(),
            children,
            requirement_child_indices,
            child_semantic_digests,
            requirement_count: self.requirements.len() as u32,
        })
    }
}

/// A child compiler implements this trait only after it has produced a typed
/// semantic program for its exact source fragment.
pub trait TypedOracleChildProgram {
    fn exact_source(&self) -> &str;
    fn semantic_digest(&self) -> &str;
    fn capabilities(&self) -> &[SemanticCapability];
}

pub struct TypedChildBinding<'a> {
    pub span: SourceSpan,
    pub program: &'a dyn TypedOracleChildProgram,
}

impl fmt::Debug for TypedChildBinding<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypedChildBinding")
            .field("span", &self.span)
            .field("semantic_digest", &self.program.semantic_digest())
            .field("capabilities", &self.program.capabilities())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedOracleComposition {
    exact_oracle: String,
    semantic_context: OracleCompositionSemanticContext,
    structural_digest: String,
    semantic_digest: String,
    root: OracleCompositionNode,
    requirements: Vec<SemanticRequirement>,
    children: Vec<TypedOracleCompositionChild>,
    requirement_child_indices: Vec<u32>,
    child_semantic_digests: Vec<String>,
    requirement_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedOracleCompositionChild {
    span: SourceSpan,
    exact_source: String,
    semantic_digest: String,
    capabilities: Vec<SemanticCapability>,
}

impl TypedOracleCompositionChild {
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn capabilities(&self) -> &[SemanticCapability] {
        &self.capabilities
    }
}

impl TypedOracleComposition {
    pub fn exact_oracle(&self) -> &str {
        &self.exact_oracle
    }

    pub fn structural_digest(&self) -> &str {
        &self.structural_digest
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn root(&self) -> &OracleCompositionNode {
        &self.root
    }

    pub fn requirements(&self) -> &[SemanticRequirement] {
        &self.requirements
    }

    pub fn children(&self) -> &[TypedOracleCompositionChild] {
        &self.children
    }

    pub fn requirement_child_indices(&self) -> &[u32] {
        &self.requirement_child_indices
    }

    pub fn child_semantic_digests(&self) -> &[String] {
        &self.child_semantic_digests
    }

    pub const fn requirement_count(&self) -> u32 {
        self.requirement_count
    }

    pub const fn semantic_context(&self) -> OracleCompositionSemanticContext {
        self.semantic_context
    }

    pub const fn production_adapter_connected(&self) -> bool {
        oracle_clause_composition_production_adapter_connected()
    }

    /// The plan is constructed in one validation pass and has no partial
    /// execution state.
    pub const fn is_atomic(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionAssemblyError {
    UnresolvedStructure(Vec<StructuralExclusion>),
    InvalidChildSpan {
        binding_index: usize,
        span: SourceSpan,
    },
    ChildSourceMismatch {
        binding_index: usize,
        span: SourceSpan,
    },
    InvalidChildSemanticDigest {
        binding_index: usize,
    },
    UnresolvedRequirements(Vec<SemanticRequirement>),
    AmbiguousChildCoverage {
        requirement: SemanticRequirement,
        binding_indices: Vec<usize>,
    },
    UnexpectedChildBindings(Vec<usize>),
}

impl fmt::Display for CompositionAssemblyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CompositionAssemblyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleCompositionError {
    EmptySource,
    NonCanonicalOuterWhitespace,
    UnexpectedClosingDelimiter {
        byte_index: usize,
        found: char,
    },
    MismatchedClosingDelimiter {
        byte_index: usize,
        expected: char,
        found: char,
    },
    UnclosedDelimiter {
        byte_index: usize,
        opening: char,
        expected: char,
    },
    UnexpectedClosingQuote {
        byte_index: usize,
        found: char,
    },
    MismatchedClosingQuote {
        byte_index: usize,
        expected: char,
        found: char,
    },
    UnclosedQuote {
        byte_index: usize,
        opening: char,
        expected: char,
    },
}

impl fmt::Display for OracleCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for OracleCompositionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteStyle {
    Ascii,
    Curly,
}

impl QuoteStyle {
    const fn opening(self) -> char {
        match self {
            Self::Ascii => '"',
            Self::Curly => '“',
        }
    }

    const fn closing(self) -> char {
        match self {
            Self::Ascii => '"',
            Self::Curly => '”',
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Frame {
    Delimiter {
        opening: char,
        opening_index: usize,
        expected: char,
    },
    Quote {
        style: QuoteStyle,
        opening_index: usize,
    },
}

#[derive(Debug, Clone)]
struct LocalScan {
    top_level_at: Vec<bool>,
    quote_pairs: Vec<(SourceSpan, SourceSpan)>,
}

impl LocalScan {
    fn is_top_level(&self, byte_index: usize) -> bool {
        self.top_level_at.get(byte_index).copied().unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsciiQuoteRole {
    Opening,
    Closing,
    Literal,
}

pub fn parse_oracle_clause_composition(
    input: OracleClauseCompositionInput<'_>,
) -> Result<OracleClauseComposition, OracleCompositionError> {
    if input.exact_oracle.trim().is_empty() {
        return Err(OracleCompositionError::EmptySource);
    }
    if input.exact_oracle != input.exact_oracle.trim() {
        return Err(OracleCompositionError::NonCanonicalOuterWhitespace);
    }
    scan_balanced(input.exact_oracle)?;

    let mut parser = CompositionParser {
        source: input.exact_oracle,
        exclusions: Vec::new(),
        quoted_fragments: Vec::new(),
    };
    let root = parser.parse_fragment(SourceSpan::new(0, input.exact_oracle.len()), 0)?;
    let mut requirements = Vec::new();
    collect_requirements(&root, &mut requirements);
    requirements.sort_by_key(|requirement| {
        (
            requirement.span.start,
            requirement.span.end,
            requirement.capability,
        )
    });
    requirements.dedup_by_key(|requirement| (requirement.span, requirement.capability));
    for (ordinal, requirement) in requirements.iter_mut().enumerate() {
        requirement.ordinal = ordinal as u32;
    }
    parser.quoted_fragments.sort_by_key(|fragment| {
        (
            fragment.quote_span.start,
            fragment.quote_span.end,
            fragment.kind as u8,
        )
    });
    parser
        .quoted_fragments
        .dedup_by_key(|fragment| (fragment.quote_span, fragment.content_span, fragment.kind));
    parser
        .exclusions
        .sort_by_key(|exclusion| (exclusion.span.start, exclusion.span.end, exclusion.kind));
    parser.exclusions.dedup();

    let structural_digest = structural_digest(
        input.exact_oracle,
        input.semantic_context,
        &root,
        &requirements,
        &parser.exclusions,
    );
    Ok(OracleClauseComposition {
        exact_oracle: input.exact_oracle.to_owned(),
        semantic_context: input.semantic_context,
        structural_digest,
        root,
        requirements,
        quoted_fragments: parser.quoted_fragments,
        exclusions: parser.exclusions,
    })
}

struct CompositionParser<'a> {
    source: &'a str,
    exclusions: Vec<StructuralExclusion>,
    quoted_fragments: Vec<QuotedFragment>,
}

impl CompositionParser<'_> {
    fn parse_fragment(
        &mut self,
        span: SourceSpan,
        depth: u8,
    ) -> Result<OracleCompositionNode, OracleCompositionError> {
        let span = trim_span(self.source, span);
        if depth > 32 {
            self.exclusions.push(StructuralExclusion {
                kind: StructuralExclusionKind::MaximumNestingDepth,
                span,
            });
            return Ok(self.atom(span, &[]));
        }
        let fragment = span
            .slice(self.source)
            .expect("parser creates valid source spans");
        let scan = scan_balanced(fragment)?;

        let base = if let Some(modal) = self.parse_modal_group(span, fragment, &scan, depth)? {
            modal
        } else if let Some(detached) =
            self.parse_detached_modal_branch(span, fragment, &scan, depth)?
        {
            detached
        } else {
            let sentence_spans = sentence_spans(fragment, &scan)
                .into_iter()
                .map(|local| offset_span(span.start, local))
                .collect::<Vec<_>>();
            if sentence_spans.len() > 1 {
                self.parse_sentence_sequence(span, &sentence_spans, depth)?
            } else if let Some(activated) =
                self.parse_activated_ability(span, fragment, &scan, depth)?
            {
                activated
            } else if let Some(conditional) =
                self.parse_prefix_conditional(span, fragment, &scan, depth)?
            {
                conditional
            } else if let Some(delayed) =
                self.parse_delayed_instruction(span, fragment, &scan, depth)?
            {
                delayed
            } else if let Some(optional) =
                self.parse_optional_choice(span, fragment, &scan, depth)?
            {
                optional
            } else if let Some(conditional) =
                self.parse_postfix_conditional(span, fragment, &scan, depth)?
            {
                conditional
            } else if let Some(composite) = self.parse_connectors(span, fragment, &scan, depth)? {
                composite
            } else {
                self.atom(span, &[])
            }
        };

        self.wrap_embedded_abilities(span, fragment, &scan, base, depth)
    }

    fn atom(&self, span: SourceSpan, additional: &[SemanticCapability]) -> OracleCompositionNode {
        OracleCompositionNode::Atom(self.atom_value(span, additional))
    }

    fn atom_value(&self, span: SourceSpan, additional: &[SemanticCapability]) -> AtomNode {
        let text = span.slice(self.source).unwrap_or_default();
        let scan = scan_balanced(text).unwrap_or_else(|_| LocalScan {
            top_level_at: vec![true; text.len() + 1],
            quote_pairs: Vec::new(),
        });
        let mut required_capabilities = BTreeSet::from([SemanticCapability::SemanticAtom]);
        required_capabilities.extend(additional.iter().copied());
        if contains_top_level_word(text, &scan, "target")
            || contains_top_level_word(text, &scan, "targets")
        {
            required_capabilities.insert(SemanticCapability::TargetSelection);
        }
        let lower = top_level_text(text, &scan).to_ascii_lowercase();
        if lower.starts_with("when ")
            || lower.starts_with("whenever ")
            || lower.starts_with("at the beginning ")
            || lower.starts_with("during ")
            || lower.starts_with("until ")
        {
            required_capabilities.insert(SemanticCapability::TimingWindow);
        }
        if lower.contains("at the beginning of the next ")
            || lower.contains("the next time ")
            || lower.contains("until the beginning of ")
        {
            required_capabilities.insert(SemanticCapability::DelayedTrigger);
            required_capabilities.insert(SemanticCapability::TimingWindow);
        }
        if lower.contains(" instead")
            || lower.starts_with("instead")
            || (lower.starts_with("as ") && lower.contains(" enters"))
            || lower.contains(" would ")
        {
            required_capabilities.insert(SemanticCapability::ReplacementEffect);
        }
        AtomNode {
            span,
            required_capabilities: required_capabilities.into_iter().collect(),
        }
    }

    fn parse_sentence_sequence(
        &mut self,
        span: SourceSpan,
        sentence_spans: &[SourceSpan],
        depth: u8,
    ) -> Result<OracleCompositionNode, OracleCompositionError> {
        let mut parts = Vec::<OracleCompositionNode>::new();
        let mut separators = Vec::<SequenceSeparator>::new();
        let mut previous_end = span.start;

        for sentence_span in sentence_spans {
            let sentence_text = sentence_span
                .slice(self.source)
                .expect("sentence span is valid");
            let local_scan = scan_balanced(sentence_text)?;
            if let Some((marker_local, body_local)) = otherwise_parts(sentence_text, &local_scan) {
                let marker_span = offset_span(sentence_span.start, marker_local);
                let body_span = offset_span(sentence_span.start, body_local);
                if let Some(OracleCompositionNode::Conditional {
                    span: conditional_span,
                    otherwise_marker_span,
                    otherwise_body,
                    ..
                }) = parts.last_mut()
                    && otherwise_body.is_none()
                {
                    *conditional_span = SourceSpan::new(conditional_span.start, sentence_span.end);
                    *otherwise_marker_span = Some(SourceSpan::new(previous_end, marker_span.end));
                    *otherwise_body = Some(Box::new(self.parse_fragment(body_span, depth + 1)?));
                    previous_end = sentence_span.end;
                    continue;
                }
                self.exclusions.push(StructuralExclusion {
                    kind: StructuralExclusionKind::DetachedOtherwise,
                    span: *sentence_span,
                });
            }

            if !parts.is_empty() {
                separators.push(SequenceSeparator {
                    kind: SequenceSeparatorKind::SentenceBoundary,
                    span: SourceSpan::new(previous_end, sentence_span.start),
                });
            }
            parts.push(self.parse_fragment(*sentence_span, depth + 1)?);
            previous_end = sentence_span.end;
        }

        if parts.len() == 1 {
            Ok(parts.remove(0))
        } else {
            Ok(OracleCompositionNode::Sequence {
                span,
                parts,
                separators,
            })
        }
    }

    fn parse_prefix_conditional(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        scan: &LocalScan,
        depth: u8,
    ) -> Result<Option<OracleCompositionNode>, OracleCompositionError> {
        let conditional_start = if starts_word_ci(fragment, 0, "then") {
            fragment[4..]
                .char_indices()
                .find_map(|(offset, character)| (!character.is_whitespace()).then_some(4 + offset))
                .unwrap_or(4)
        } else {
            0
        };
        let (kind, marker_end) = if starts_word_ci(fragment, conditional_start, "if") {
            (ConditionalKind::If, conditional_start + 2)
        } else if starts_word_ci(fragment, conditional_start, "unless") {
            (ConditionalKind::Unless, conditional_start + 6)
        } else {
            return Ok(None);
        };
        let Some(comma) = find_top_level_char(fragment, scan, marker_end, fragment.len(), ',')
        else {
            return Ok(None);
        };
        let condition_local = trim_local(fragment, SourceSpan::new(marker_end, comma));
        let consequence_local = trim_local(fragment, SourceSpan::new(comma + 1, fragment.len()));
        if condition_local.is_empty() || consequence_local.is_empty() {
            return Ok(None);
        }
        let condition_span = offset_span(span.start, condition_local);
        let consequence_span = offset_span(span.start, consequence_local);
        Ok(Some(OracleCompositionNode::Conditional {
            span,
            kind,
            marker_span: SourceSpan::new(span.start + conditional_start, span.start + marker_end),
            condition: Box::new(self.atom(condition_span, &[SemanticCapability::Condition])),
            consequence: Box::new(self.parse_fragment(consequence_span, depth + 1)?),
            otherwise_marker_span: None,
            otherwise_body: None,
        }))
    }

    fn parse_postfix_conditional(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        scan: &LocalScan,
        depth: u8,
    ) -> Result<Option<OracleCompositionNode>, OracleCompositionError> {
        let mut candidate = None;
        for (word, kind) in [
            ("if", ConditionalKind::If),
            ("unless", ConditionalKind::Unless),
        ] {
            for word_span in find_top_level_words(fragment, scan, word) {
                if word_span.start == 0 {
                    continue;
                }
                let action_local = trim_local(fragment, SourceSpan::new(0, word_span.start));
                let condition_local =
                    trim_local(fragment, SourceSpan::new(word_span.end, fragment.len()));
                if !action_local.is_empty() && !condition_local.is_empty() {
                    candidate = Some((word_span, kind, action_local, condition_local));
                }
            }
        }
        let Some((marker_local, kind, consequence_local, condition_local)) = candidate else {
            return Ok(None);
        };
        let condition_span = offset_span(span.start, condition_local);
        let consequence_span = offset_span(span.start, consequence_local);
        Ok(Some(OracleCompositionNode::Conditional {
            span,
            kind,
            marker_span: offset_span(span.start, marker_local),
            condition: Box::new(self.atom(condition_span, &[SemanticCapability::Condition])),
            consequence: Box::new(self.parse_fragment(consequence_span, depth + 1)?),
            otherwise_marker_span: None,
            otherwise_body: None,
        }))
    }

    fn parse_optional_choice(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        scan: &LocalScan,
        depth: u8,
    ) -> Result<Option<OracleCompositionNode>, OracleCompositionError> {
        for may_local in find_top_level_words(fragment, scan, "may") {
            let actor_local = trim_local(fragment, SourceSpan::new(0, may_local.start));
            let body_local = trim_local(fragment, SourceSpan::new(may_local.end, fragment.len()));
            if actor_local.is_empty() || body_local.is_empty() {
                continue;
            }
            let actor_span = offset_span(span.start, actor_local);
            let body_span = offset_span(span.start, body_local);
            return Ok(Some(OracleCompositionNode::OptionalChoice {
                span,
                actor_span,
                may_span: offset_span(span.start, may_local),
                body: Box::new(self.parse_fragment(body_span, depth + 1)?),
            }));
        }
        Ok(None)
    }

    fn parse_activated_ability(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        scan: &LocalScan,
        depth: u8,
    ) -> Result<Option<OracleCompositionNode>, OracleCompositionError> {
        let Some(colon) = find_top_level_char(fragment, scan, 0, fragment.len(), ':') else {
            return Ok(None);
        };
        let cost_local = trim_local(fragment, SourceSpan::new(0, colon));
        let instruction_local = trim_local(fragment, SourceSpan::new(colon + 1, fragment.len()));
        if cost_local.is_empty()
            || instruction_local.is_empty()
            || !looks_like_activation_cost(cost_local.slice(fragment).unwrap_or_default())
        {
            return Ok(None);
        }
        let cost_span = offset_span(span.start, cost_local);
        let instruction_span = offset_span(span.start, instruction_local);
        Ok(Some(OracleCompositionNode::ActivatedAbility {
            span,
            cost: self.atom_value(cost_span, &[SemanticCapability::CostPayment]),
            colon_span: SourceSpan::new(span.start + colon, span.start + colon + 1),
            instruction: Box::new(self.parse_fragment(instruction_span, depth + 1)?),
        }))
    }

    fn parse_delayed_instruction(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        scan: &LocalScan,
        depth: u8,
    ) -> Result<Option<OracleCompositionNode>, OracleCompositionError> {
        let lower = fragment.to_ascii_lowercase();
        let delayed_prefix = lower.starts_with("at the beginning of the next ")
            || lower.starts_with("the next time ")
            || lower.starts_with("at the beginning of your next ")
            || lower.starts_with("at the beginning of that player's next ");
        if !delayed_prefix {
            return Ok(None);
        }
        let Some(comma) = find_top_level_char(fragment, scan, 0, fragment.len(), ',') else {
            return Ok(None);
        };
        let schedule_local = trim_local(fragment, SourceSpan::new(0, comma));
        let instruction_local = trim_local(fragment, SourceSpan::new(comma + 1, fragment.len()));
        if schedule_local.is_empty() || instruction_local.is_empty() {
            return Ok(None);
        }
        let schedule_span = offset_span(span.start, schedule_local);
        let instruction_span = offset_span(span.start, instruction_local);
        Ok(Some(OracleCompositionNode::DelayedInstruction {
            span,
            schedule: self.atom_value(
                schedule_span,
                &[
                    SemanticCapability::TimingWindow,
                    SemanticCapability::DelayedTrigger,
                ],
            ),
            comma_span: SourceSpan::new(span.start + comma, span.start + comma + 1),
            instruction: Box::new(self.parse_fragment(instruction_span, depth + 1)?),
        }))
    }

    fn parse_connectors(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        scan: &LocalScan,
        depth: u8,
    ) -> Result<Option<OracleCompositionNode>, OracleCompositionError> {
        let connectors = recognized_connectors(fragment, scan);
        if connectors.is_empty() {
            return self.parse_comma_conjunction(span, fragment, scan, depth);
        }

        let then_connectors = connectors
            .iter()
            .copied()
            .filter(|connector| {
                connector.kind == ConnectorKind::Then
                    && connector_is_instruction_boundary(fragment, *connector)
            })
            .collect::<Vec<_>>();
        if !then_connectors.is_empty() {
            return self
                .build_sequence_from_connectors(span, fragment, &then_connectors, depth)
                .map(Some);
        }

        let safe = connectors
            .into_iter()
            .filter(|connector| connector_is_instruction_boundary(fragment, *connector))
            .collect::<Vec<_>>();
        if safe.is_empty() {
            return Ok(None);
        }
        let has_conjunction = safe
            .iter()
            .any(|connector| matches!(connector.kind, ConnectorKind::And | ConnectorKind::But));
        let has_alternative = safe
            .iter()
            .any(|connector| matches!(connector.kind, ConnectorKind::Or | ConnectorKind::AndOr));
        if has_conjunction && has_alternative {
            self.exclusions.push(StructuralExclusion {
                kind: StructuralExclusionKind::AmbiguousMixedConnectors,
                span,
            });
            return Ok(None);
        }
        if has_alternative {
            self.build_alternative_from_connectors(span, fragment, &safe, depth)
                .map(Some)
        } else {
            self.build_conjunction_from_connectors(span, fragment, &safe, depth)
                .map(Some)
        }
    }

    fn parse_comma_conjunction(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        scan: &LocalScan,
        depth: u8,
    ) -> Result<Option<OracleCompositionNode>, OracleCompositionError> {
        let comma_indices = fragment
            .char_indices()
            .filter_map(|(index, character)| {
                (character == ',' && scan.is_top_level(index)).then_some(index)
            })
            .collect::<Vec<_>>();
        if comma_indices.is_empty() {
            return Ok(None);
        }

        let mut parts = Vec::with_capacity(comma_indices.len() + 1);
        let mut separators = Vec::with_capacity(comma_indices.len());
        let mut start = 0usize;
        for comma in comma_indices {
            let part = trim_local(fragment, SourceSpan::new(start, comma));
            if part.is_empty() {
                return Ok(None);
            }
            parts.push(self.parse_fragment(offset_span(span.start, part), depth + 1)?);
            separators.push(ConjunctionSeparator {
                kind: ConjunctionKind::Comma,
                span: SourceSpan::new(span.start + comma, span.start + comma + 1),
            });
            start = comma + 1;
        }
        let final_part = trim_local(fragment, SourceSpan::new(start, fragment.len()));
        if final_part.is_empty() {
            return Ok(None);
        }
        parts.push(self.parse_fragment(offset_span(span.start, final_part), depth + 1)?);
        Ok(Some(OracleCompositionNode::Conjunction {
            span,
            parts,
            separators,
        }))
    }

    fn build_sequence_from_connectors(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        connectors: &[Connector],
        depth: u8,
    ) -> Result<OracleCompositionNode, OracleCompositionError> {
        let (part_spans, separator_spans) =
            split_around_connectors(fragment, span.start, connectors);
        let mut parts = Vec::new();
        for part_span in part_spans {
            parts.push(self.parse_fragment(part_span, depth + 1)?);
        }
        let separators = separator_spans
            .into_iter()
            .map(|span| SequenceSeparator {
                kind: SequenceSeparatorKind::Then,
                span,
            })
            .collect();
        Ok(OracleCompositionNode::Sequence {
            span,
            parts,
            separators,
        })
    }

    fn build_conjunction_from_connectors(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        connectors: &[Connector],
        depth: u8,
    ) -> Result<OracleCompositionNode, OracleCompositionError> {
        let (part_spans, separator_spans) =
            split_around_connectors(fragment, span.start, connectors);
        let mut parts = Vec::new();
        for part_span in part_spans {
            parts.push(self.parse_fragment(part_span, depth + 1)?);
        }
        let separators = connectors
            .iter()
            .zip(separator_spans)
            .map(|(connector, span)| ConjunctionSeparator {
                kind: match connector.kind {
                    ConnectorKind::And => ConjunctionKind::And,
                    ConnectorKind::But => ConjunctionKind::But,
                    _ => unreachable!("conjunction builder receives conjunctions"),
                },
                span,
            })
            .collect();
        Ok(OracleCompositionNode::Conjunction {
            span,
            parts,
            separators,
        })
    }

    fn build_alternative_from_connectors(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        connectors: &[Connector],
        depth: u8,
    ) -> Result<OracleCompositionNode, OracleCompositionError> {
        let (part_spans, separator_spans) =
            split_around_connectors(fragment, span.start, connectors);
        let mut parts = Vec::new();
        for part_span in part_spans {
            parts.push(self.parse_fragment(part_span, depth + 1)?);
        }
        let separators = connectors
            .iter()
            .zip(separator_spans)
            .map(|(connector, span)| AlternativeSeparator {
                kind: match connector.kind {
                    ConnectorKind::Or => AlternativeKind::Or,
                    ConnectorKind::AndOr => AlternativeKind::AndOr,
                    _ => unreachable!("alternative builder receives alternatives"),
                },
                span,
            })
            .collect();
        Ok(OracleCompositionNode::Alternative {
            span,
            parts,
            separators,
        })
    }

    fn parse_modal_group(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        scan: &LocalScan,
        depth: u8,
    ) -> Result<Option<OracleCompositionNode>, OracleCompositionError> {
        if !starts_modal_header(fragment) {
            return Ok(None);
        }
        let bullet_positions = top_level_char_positions(fragment, scan, '•');
        if bullet_positions.is_empty() {
            self.exclusions.push(StructuralExclusion {
                kind: StructuralExclusionKind::IncompleteModalHeader,
                span,
            });
            return Ok(None);
        }
        let first_bullet = bullet_positions[0];
        let header_local = trim_local(fragment, SourceSpan::new(0, first_bullet));
        let header_span = offset_span(span.start, header_local);
        let selection = parse_modal_selection(
            header_local
                .slice(fragment)
                .unwrap_or_default()
                .trim_end_matches(['\u{2014}', '-', '.', '\n', '\r'])
                .trim(),
        );
        if selection.is_none() {
            self.exclusions.push(StructuralExclusion {
                kind: StructuralExclusionKind::UnrecognizedModalSelection,
                span: header_span,
            });
        }

        let mut branches = Vec::new();
        for (branch_index, bullet) in bullet_positions.iter().copied().enumerate() {
            let marker_end = bullet + '•'.len_utf8();
            let branch_end = bullet_positions
                .get(branch_index + 1)
                .copied()
                .unwrap_or(fragment.len());
            let body_local = trim_local(fragment, SourceSpan::new(marker_end, branch_end));
            let marker_span = SourceSpan::new(span.start + bullet, span.start + marker_end);
            if body_local.is_empty() {
                self.exclusions.push(StructuralExclusion {
                    kind: StructuralExclusionKind::EmptyModalBranch,
                    span: marker_span,
                });
                continue;
            }
            let body_span = offset_span(span.start, body_local);
            branches.push(ModalBranch {
                marker_span,
                body_span,
                body: self.parse_fragment(body_span, depth + 1)?,
            });
        }
        Ok(Some(OracleCompositionNode::ModalGroup {
            span,
            header_span,
            selection,
            branches,
        }))
    }

    fn parse_detached_modal_branch(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        scan: &LocalScan,
        depth: u8,
    ) -> Result<Option<OracleCompositionNode>, OracleCompositionError> {
        let Some(first_nonspace) = fragment
            .char_indices()
            .find_map(|(index, character)| (!character.is_whitespace()).then_some(index))
        else {
            return Ok(None);
        };
        if !scan.is_top_level(first_nonspace) || !fragment[first_nonspace..].starts_with('•') {
            return Ok(None);
        }
        let marker_end = first_nonspace + '•'.len_utf8();
        let body_local = trim_local(fragment, SourceSpan::new(marker_end, fragment.len()));
        if body_local.is_empty() {
            return Ok(None);
        }
        let marker_span = SourceSpan::new(span.start + first_nonspace, span.start + marker_end);
        self.exclusions.push(StructuralExclusion {
            kind: StructuralExclusionKind::DetachedModalBranch,
            span,
        });
        let body_span = offset_span(span.start, body_local);
        Ok(Some(OracleCompositionNode::DetachedModalBranch {
            span,
            marker_span,
            body: Box::new(self.parse_fragment(body_span, depth + 1)?),
        }))
    }

    fn wrap_embedded_abilities(
        &mut self,
        span: SourceSpan,
        fragment: &str,
        scan: &LocalScan,
        base: OracleCompositionNode,
        depth: u8,
    ) -> Result<OracleCompositionNode, OracleCompositionError> {
        let mut abilities = Vec::new();
        for (opening, closing) in &scan.quote_pairs {
            let quote_local = SourceSpan::new(opening.start, closing.end);
            let content_local = SourceSpan::new(opening.end, closing.start);
            let content = content_local.slice(fragment).unwrap_or_default();
            let kind = quoted_fragment_kind(fragment, *opening, content);
            let quote_span = offset_span(span.start, quote_local);
            let content_span = offset_span(span.start, content_local);
            self.quoted_fragments.push(QuotedFragment {
                quote_span,
                content_span,
                kind,
            });
            if matches!(
                kind,
                QuotedFragmentKind::GrantedAbility | QuotedFragmentKind::QuotedAbility
            ) && !content_span.is_empty()
            {
                abilities.push(EmbeddedAbility {
                    quote_span,
                    content_span,
                    kind,
                    body: Box::new(self.parse_fragment(content_span, depth + 1)?),
                });
            }
        }
        if abilities.is_empty() {
            Ok(base)
        } else {
            Ok(OracleCompositionNode::EmbeddedAbilities {
                span,
                outer: Box::new(base),
                abilities,
            })
        }
    }
}

fn scan_balanced(source: &str) -> Result<LocalScan, OracleCompositionError> {
    let ascii_roles = classify_ascii_quotes(source);
    let mut top_level_at = vec![false; source.len() + 1];
    let mut stack = Vec::<Frame>::new();
    let mut quote_pairs = Vec::<(SourceSpan, SourceSpan)>::new();

    for (byte_index, character) in source.char_indices() {
        top_level_at[byte_index] = stack.is_empty();
        let character_end = byte_index + character.len_utf8();
        match character {
            '"' => match ascii_quote_role(&ascii_roles, byte_index) {
                AsciiQuoteRole::Opening => stack.push(Frame::Quote {
                    style: QuoteStyle::Ascii,
                    opening_index: byte_index,
                }),
                AsciiQuoteRole::Closing => match stack.pop() {
                    Some(Frame::Quote {
                        style: QuoteStyle::Ascii,
                        opening_index,
                    }) => quote_pairs.push((
                        SourceSpan::new(opening_index, opening_index + 1),
                        SourceSpan::new(byte_index, character_end),
                    )),
                    Some(Frame::Quote { style, .. }) => {
                        return Err(OracleCompositionError::MismatchedClosingQuote {
                            byte_index,
                            expected: style.closing(),
                            found: character,
                        });
                    }
                    Some(Frame::Delimiter { expected, .. }) => {
                        return Err(OracleCompositionError::MismatchedClosingDelimiter {
                            byte_index,
                            expected,
                            found: character,
                        });
                    }
                    None => {
                        return Err(OracleCompositionError::UnexpectedClosingQuote {
                            byte_index,
                            found: character,
                        });
                    }
                },
                AsciiQuoteRole::Literal => {}
            },
            '“' => stack.push(Frame::Quote {
                style: QuoteStyle::Curly,
                opening_index: byte_index,
            }),
            '”' => match stack.pop() {
                Some(Frame::Quote {
                    style: QuoteStyle::Curly,
                    opening_index,
                }) => quote_pairs.push((
                    SourceSpan::new(opening_index, opening_index + '“'.len_utf8()),
                    SourceSpan::new(byte_index, character_end),
                )),
                Some(Frame::Quote { style, .. }) => {
                    return Err(OracleCompositionError::MismatchedClosingQuote {
                        byte_index,
                        expected: style.closing(),
                        found: character,
                    });
                }
                Some(Frame::Delimiter { expected, .. }) => {
                    return Err(OracleCompositionError::MismatchedClosingDelimiter {
                        byte_index,
                        expected,
                        found: character,
                    });
                }
                None => {
                    return Err(OracleCompositionError::UnexpectedClosingQuote {
                        byte_index,
                        found: character,
                    });
                }
            },
            '(' | '[' | '{' => {
                let expected = match character {
                    '(' => ')',
                    '[' => ']',
                    '{' => '}',
                    _ => unreachable!(),
                };
                stack.push(Frame::Delimiter {
                    opening: character,
                    opening_index: byte_index,
                    expected,
                });
            }
            ')' | ']' | '}' => match stack.pop() {
                Some(Frame::Delimiter { expected, .. }) if expected == character => {}
                Some(Frame::Delimiter { expected, .. }) => {
                    return Err(OracleCompositionError::MismatchedClosingDelimiter {
                        byte_index,
                        expected,
                        found: character,
                    });
                }
                Some(Frame::Quote { style, .. }) => {
                    return Err(OracleCompositionError::MismatchedClosingQuote {
                        byte_index,
                        expected: style.closing(),
                        found: character,
                    });
                }
                None => {
                    return Err(OracleCompositionError::UnexpectedClosingDelimiter {
                        byte_index,
                        found: character,
                    });
                }
            },
            _ => {}
        }
    }
    if let Some(frame) = stack.pop() {
        return match frame {
            Frame::Delimiter {
                opening,
                opening_index,
                expected,
            } => Err(OracleCompositionError::UnclosedDelimiter {
                byte_index: opening_index,
                opening,
                expected,
            }),
            Frame::Quote {
                style,
                opening_index,
            } => Err(OracleCompositionError::UnclosedQuote {
                byte_index: opening_index,
                opening: style.opening(),
                expected: style.closing(),
            }),
        };
    }
    top_level_at[source.len()] = true;
    quote_pairs.sort_by_key(|pair| pair.0.start);
    Ok(LocalScan {
        top_level_at,
        quote_pairs,
    })
}

fn classify_ascii_quotes(source: &str) -> Vec<(usize, AsciiQuoteRole)> {
    let positions = source
        .char_indices()
        .filter_map(|(index, character)| (character == '"').then_some(index))
        .collect::<Vec<_>>();
    let mut roles = positions
        .iter()
        .copied()
        .map(|index| (index, AsciiQuoteRole::Literal))
        .collect::<Vec<_>>();
    let mut opening: Option<usize> = None;
    for (role_index, byte_index) in positions.iter().copied().enumerate() {
        if let Some(opening_role_index) = opening {
            if ascii_quote_can_close(source, byte_index) {
                roles[opening_role_index].1 = AsciiQuoteRole::Opening;
                roles[role_index].1 = AsciiQuoteRole::Closing;
                opening = None;
            }
        } else if ascii_quote_can_open(source, byte_index) {
            opening = Some(role_index);
        }
    }
    roles
}

fn ascii_quote_role(roles: &[(usize, AsciiQuoteRole)], byte_index: usize) -> AsciiQuoteRole {
    roles
        .binary_search_by_key(&byte_index, |(candidate, _)| *candidate)
        .ok()
        .map(|index| roles[index].1)
        .unwrap_or(AsciiQuoteRole::Literal)
}

fn ascii_quote_can_open(source: &str, byte_index: usize) -> bool {
    let previous = source[..byte_index].chars().next_back();
    let next = source[byte_index + 1..].chars().next();
    next.is_some_and(|character| !character.is_whitespace())
        && previous.is_none_or(|character| {
            character.is_whitespace()
                || matches!(
                    character,
                    '(' | '[' | '{' | '=' | ',' | ':' | ';' | '\u{2014}' | '\u{2013}' | '-'
                )
        })
}

fn ascii_quote_can_close(source: &str, byte_index: usize) -> bool {
    let previous = source[..byte_index].chars().next_back();
    let next = source[byte_index + 1..].chars().next();
    previous.is_some_and(|character| !character.is_whitespace())
        && next.is_none_or(|character| {
            character.is_whitespace()
                || matches!(
                    character,
                    ')' | ']'
                        | '}'
                        | ','
                        | '.'
                        | ':'
                        | ';'
                        | '!'
                        | '?'
                        | '\u{2014}'
                        | '\u{2013}'
                        | '-'
                )
        })
}

fn trim_span(source: &str, span: SourceSpan) -> SourceSpan {
    let Some(text) = span.slice(source) else {
        return span;
    };
    let leading = text.len().saturating_sub(text.trim_start().len());
    let trailing = text.len().saturating_sub(text.trim_end().len());
    SourceSpan::new(span.start + leading, span.end.saturating_sub(trailing))
}

fn trim_local(source: &str, span: SourceSpan) -> SourceSpan {
    let mut trimmed = trim_span(source, span);
    loop {
        let Some(character) = trimmed
            .slice(source)
            .and_then(|text| text.chars().next_back())
        else {
            break;
        };
        if !matches!(character, ',') {
            break;
        }
        trimmed.end -= character.len_utf8();
        trimmed = trim_span(source, trimmed);
    }
    trimmed
}

fn offset_span(base: usize, local: SourceSpan) -> SourceSpan {
    SourceSpan::new(base + local.start, base + local.end)
}

fn find_top_level_char(
    source: &str,
    scan: &LocalScan,
    start: usize,
    end: usize,
    needle: char,
) -> Option<usize> {
    source[start..end]
        .char_indices()
        .find_map(|(offset, character)| {
            let byte_index = start + offset;
            (character == needle && scan.is_top_level(byte_index)).then_some(byte_index)
        })
}

fn top_level_char_positions(source: &str, scan: &LocalScan, needle: char) -> Vec<usize> {
    source
        .char_indices()
        .filter_map(|(byte_index, character)| {
            (character == needle && scan.is_top_level(byte_index)).then_some(byte_index)
        })
        .collect()
}

fn sentence_spans(source: &str, scan: &LocalScan) -> Vec<SourceSpan> {
    let mut spans = Vec::new();
    let mut start = 0;
    for (byte_index, character) in source.char_indices() {
        if !matches!(character, '.' | '!' | '?') || !scan.is_top_level(byte_index) {
            continue;
        }
        let previous = source[..byte_index].chars().next_back();
        let next = source[byte_index + character.len_utf8()..].chars().next();
        if character == '.'
            && previous.is_some_and(|value| value.is_ascii_digit())
            && next.is_some_and(|value| value.is_ascii_digit())
        {
            continue;
        }
        let end = byte_index + character.len_utf8();
        let candidate = trim_span(source, SourceSpan::new(start, end));
        if !candidate.is_empty() {
            spans.push(candidate);
        }
        start = end;
    }
    let trailing = trim_span(source, SourceSpan::new(start, source.len()));
    if !trailing.is_empty() {
        if let Some(last) = spans.last_mut()
            && is_parenthetical_only(trailing.slice(source).unwrap_or_default())
        {
            last.end = trailing.end;
            return spans;
        }
        spans.push(trailing);
    }
    spans
}

fn is_parenthetical_only(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with('(') && trimmed.ends_with(')')
}

fn starts_word_ci(source: &str, start: usize, word: &str) -> bool {
    let end = start + word.len();
    end <= source.len()
        && source.is_char_boundary(end)
        && source[start..end].eq_ignore_ascii_case(word)
        && is_word_boundary(source, start, end)
}

fn is_word_boundary(source: &str, start: usize, end: usize) -> bool {
    let before = source[..start].chars().next_back();
    let after = source[end..].chars().next();
    before.is_none_or(|character| !word_character(character))
        && after.is_none_or(|character| !word_character(character))
}

fn word_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '\'' | '’' | '-')
}

fn find_top_level_words(source: &str, scan: &LocalScan, word: &str) -> Vec<SourceSpan> {
    let mut spans = Vec::new();
    let mut cursor = 0;
    while cursor + word.len() <= source.len() {
        if source.is_char_boundary(cursor)
            && scan.is_top_level(cursor)
            && starts_word_ci(source, cursor, word)
        {
            spans.push(SourceSpan::new(cursor, cursor + word.len()));
            cursor += word.len();
        } else {
            cursor += source[cursor..]
                .chars()
                .next()
                .expect("cursor is within source")
                .len_utf8();
        }
    }
    spans
}

fn contains_top_level_word(source: &str, scan: &LocalScan, word: &str) -> bool {
    !find_top_level_words(source, scan, word).is_empty()
}

fn top_level_text(source: &str, scan: &LocalScan) -> String {
    source
        .char_indices()
        .map(|(byte_index, character)| {
            if scan.is_top_level(byte_index) {
                character
            } else {
                ' '
            }
        })
        .collect()
}

fn looks_like_activation_cost(cost: &str) -> bool {
    let lower = cost.trim().to_ascii_lowercase();
    lower.starts_with('{')
        || lower.starts_with('+')
        || lower.starts_with('-')
        || lower.starts_with('−')
        || lower.chars().next().is_some_and(|c| c.is_ascii_digit())
        || [
            "discard ",
            "exile ",
            "mill ",
            "pay ",
            "put ",
            "remove ",
            "return ",
            "reveal ",
            "sacrifice ",
            "tap ",
            "untap ",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

fn otherwise_parts(source: &str, scan: &LocalScan) -> Option<(SourceSpan, SourceSpan)> {
    if !starts_word_ci(source, 0, "otherwise") {
        return None;
    }
    let comma = find_top_level_char(source, scan, 9, source.len(), ',')?;
    let body = trim_local(source, SourceSpan::new(comma + 1, source.len()));
    (!body.is_empty()).then_some((SourceSpan::new(0, comma + 1), body))
}

fn starts_modal_header(source: &str) -> bool {
    let lower = source.trim_start().to_ascii_lowercase();
    lower.starts_with("choose one")
        || lower.starts_with("choose two")
        || lower.starts_with("choose three")
        || lower.starts_with("choose four")
        || lower.starts_with("choose up to ")
        || lower.starts_with("choose one or more")
        || lower.starts_with("choose one or both")
        || lower.starts_with("choose any number")
}

fn parse_modal_selection(header: &str) -> Option<ModalSelection> {
    let lower = header.to_ascii_lowercase();
    if lower.starts_with("choose one or both") {
        Some(ModalSelection::OneOrBoth)
    } else if lower.starts_with("choose one or more") {
        Some(ModalSelection::OneOrMore)
    } else if lower.starts_with("choose any number") {
        Some(ModalSelection::AnyNumber)
    } else if lower.starts_with("choose up to one") {
        Some(ModalSelection::UpTo(1))
    } else if lower.starts_with("choose up to two") {
        Some(ModalSelection::UpTo(2))
    } else if lower.starts_with("choose up to three") {
        Some(ModalSelection::UpTo(3))
    } else if lower.starts_with("choose up to four") {
        Some(ModalSelection::UpTo(4))
    } else if lower.starts_with("choose one") {
        Some(ModalSelection::Exactly(1))
    } else if lower.starts_with("choose two") {
        Some(ModalSelection::Exactly(2))
    } else if lower.starts_with("choose three") {
        Some(ModalSelection::Exactly(3))
    } else if lower.starts_with("choose four") {
        Some(ModalSelection::Exactly(4))
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectorKind {
    Then,
    And,
    But,
    Or,
    AndOr,
}

#[derive(Debug, Clone, Copy)]
struct Connector {
    kind: ConnectorKind,
    span: SourceSpan,
}

fn recognized_connectors(source: &str, scan: &LocalScan) -> Vec<Connector> {
    let mut connectors = Vec::new();
    for (word, kind) in [
        ("and/or", ConnectorKind::AndOr),
        ("then", ConnectorKind::Then),
        ("and", ConnectorKind::And),
        ("but", ConnectorKind::But),
        ("or", ConnectorKind::Or),
    ] {
        for span in find_top_level_words(source, scan, word) {
            if connectors
                .iter()
                .any(|existing: &Connector| spans_overlap(existing.span, span))
            {
                continue;
            }
            connectors.push(Connector { kind, span });
        }
    }
    connectors.sort_by_key(|connector| connector.span.start);
    connectors
}

fn spans_overlap(left: SourceSpan, right: SourceSpan) -> bool {
    left.start < right.end && right.start < left.end
}

fn connector_is_instruction_boundary(source: &str, connector: Connector) -> bool {
    let left = source[..connector.span.start].trim_end();
    let right = source[connector.span.end..].trim_start();
    if left.is_empty() || right.is_empty() {
        return false;
    }
    if connector.kind == ConnectorKind::Then {
        return true;
    }
    let comma_boundary = left.ends_with(',');
    let left_instruction = looks_like_instruction(left);
    let right_instruction = looks_like_instruction(right);
    comma_boundary || (left_instruction && right_instruction)
}

fn looks_like_instruction(text: &str) -> bool {
    let lower = text
        .trim_matches(|character: char| {
            character.is_whitespace() || matches!(character, ',' | '.' | ';' | '\u{2014}' | '•')
        })
        .to_ascii_lowercase();
    const ACTION_LEADS: &[&str] = &[
        "add ",
        "attach ",
        "counter ",
        "create ",
        "destroy ",
        "discard ",
        "draw ",
        "each ",
        "exile ",
        "gain ",
        "it ",
        "its controller ",
        "lose ",
        "mill ",
        "prevent ",
        "put ",
        "return ",
        "sacrifice ",
        "scry ",
        "search ",
        "surveil ",
        "tap ",
        "target ",
        "that player ",
        "they ",
        "untap ",
        "you ",
    ];
    ACTION_LEADS.iter().any(|lead| lower.starts_with(lead))
}

fn split_around_connectors(
    source: &str,
    base: usize,
    connectors: &[Connector],
) -> (Vec<SourceSpan>, Vec<SourceSpan>) {
    let mut parts = Vec::new();
    let mut separators = Vec::new();
    let mut cursor = 0;
    for connector in connectors {
        let left = trim_local(source, SourceSpan::new(cursor, connector.span.start));
        let next_start = connector.span.end;
        if !left.is_empty() {
            parts.push(offset_span(base, left));
        }
        let right_start = trim_local(source, SourceSpan::new(next_start, source.len())).start;
        separators.push(SourceSpan::new(base + left.end, base + right_start));
        cursor = right_start;
    }
    let trailing = trim_local(source, SourceSpan::new(cursor, source.len()));
    if !trailing.is_empty() {
        parts.push(offset_span(base, trailing));
    }
    while !separators.is_empty() && separators.len() + 1 > parts.len() {
        separators.pop();
    }
    (parts, separators)
}

fn quoted_fragment_kind(source: &str, opening: SourceSpan, content: &str) -> QuotedFragmentKind {
    let prefix_start = source[..opening.start]
        .char_indices()
        .rev()
        .nth(96)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let prefix = source[prefix_start..opening.start].to_ascii_lowercase();
    let granted = [" gains ", " gain ", " has ", " have ", " with "]
        .iter()
        .any(|needle| prefix.contains(needle))
        || prefix.trim_end().ends_with("gains")
        || prefix.trim_end().ends_with("has")
        || prefix.contains("ability")
        || prefix.contains("abilities");
    if granted {
        return QuotedFragmentKind::GrantedAbility;
    }
    let lower = content.trim().to_ascii_lowercase();
    if lower.contains(':')
        || lower.starts_with("when ")
        || lower.starts_with("whenever ")
        || lower.starts_with("at the beginning ")
        || lower.starts_with("this creature ")
        || lower.starts_with("this permanent ")
    {
        QuotedFragmentKind::QuotedAbility
    } else {
        QuotedFragmentKind::LiteralText
    }
}

fn collect_requirements(node: &OracleCompositionNode, requirements: &mut Vec<SemanticRequirement>) {
    match node {
        OracleCompositionNode::Atom(atom) => {
            for capability in &atom.required_capabilities {
                requirements.push(SemanticRequirement {
                    ordinal: 0,
                    span: atom.span,
                    capability: *capability,
                });
            }
        }
        OracleCompositionNode::Sequence { parts, .. }
        | OracleCompositionNode::Conjunction { parts, .. }
        | OracleCompositionNode::Alternative { parts, .. } => {
            for part in parts {
                collect_requirements(part, requirements);
            }
        }
        OracleCompositionNode::Conditional {
            condition,
            consequence,
            otherwise_body,
            ..
        } => {
            collect_requirements(condition, requirements);
            collect_requirements(consequence, requirements);
            if let Some(otherwise_body) = otherwise_body {
                collect_requirements(otherwise_body, requirements);
            }
        }
        OracleCompositionNode::OptionalChoice { span, body, .. } => {
            requirements.push(SemanticRequirement {
                ordinal: 0,
                span: *span,
                capability: SemanticCapability::OptionalChoice,
            });
            collect_requirements(body, requirements);
        }
        OracleCompositionNode::ActivatedAbility {
            cost, instruction, ..
        } => {
            for capability in &cost.required_capabilities {
                requirements.push(SemanticRequirement {
                    ordinal: 0,
                    span: cost.span,
                    capability: *capability,
                });
            }
            collect_requirements(instruction, requirements);
        }
        OracleCompositionNode::DelayedInstruction {
            schedule,
            instruction,
            ..
        } => {
            for capability in &schedule.required_capabilities {
                requirements.push(SemanticRequirement {
                    ordinal: 0,
                    span: schedule.span,
                    capability: *capability,
                });
            }
            collect_requirements(instruction, requirements);
        }
        OracleCompositionNode::ModalGroup {
            header_span,
            branches,
            ..
        } => {
            requirements.push(SemanticRequirement {
                ordinal: 0,
                span: *header_span,
                capability: SemanticCapability::ModalSelection,
            });
            for branch in branches {
                collect_requirements(&branch.body, requirements);
            }
        }
        OracleCompositionNode::DetachedModalBranch { body, .. } => {
            collect_requirements(body, requirements);
        }
        OracleCompositionNode::EmbeddedAbilities {
            outer, abilities, ..
        } => {
            collect_requirements(outer, requirements);
            for ability in abilities {
                requirements.push(SemanticRequirement {
                    ordinal: 0,
                    span: ability.content_span,
                    capability: SemanticCapability::NestedGrantedAbility,
                });
                collect_requirements(&ability.body, requirements);
            }
        }
    }
}

fn structural_digest(
    exact_oracle: &str,
    semantic_context: OracleCompositionSemanticContext,
    root: &OracleCompositionNode,
    requirements: &[SemanticRequirement],
    exclusions: &[StructuralExclusion],
) -> String {
    let mut structure = String::new();
    encode_node(root, &mut structure);
    for requirement in requirements {
        structure.push_str(&format!(
            "|req:{}:{}:{}",
            requirement.span.start,
            requirement.span.end,
            requirement.capability.stable_id()
        ));
    }
    for exclusion in exclusions {
        structure.push_str(&format!(
            "|exclude:{:?}:{}:{}",
            exclusion.kind, exclusion.span.start, exclusion.span.end
        ));
    }
    hash_components(&[
        "oracle-clause-composition-structure/v1",
        ORACLE_CLAUSE_COMPOSITION_COMPILER_VERSION,
        ORACLE_CLAUSE_COMPOSITION_RUNTIME_VERSION,
        ORACLE_CLAUSE_COMPOSITION_RULES_CONTEXT_VERSION,
        semantic_context.stable_id(),
        exact_oracle,
        &structure,
    ])
}

fn typed_composition_digest(
    composition: &OracleClauseComposition,
    bindings: &[TypedChildBinding<'_>],
    ordered_bindings: &[usize],
    requirement_binding: &[usize],
) -> String {
    let mut typed = String::new();
    let binding_rank = ordered_bindings
        .iter()
        .copied()
        .enumerate()
        .map(|(rank, binding_index)| (binding_index, rank))
        .collect::<BTreeMap<_, _>>();
    for (ordinal, binding_index) in requirement_binding.iter().copied().enumerate() {
        let requirement = &composition.requirements[ordinal];
        let child_rank = binding_rank
            .get(&binding_index)
            .copied()
            .expect("every requirement binding has a canonical rank");
        typed.push_str(&format!(
            "|requirement:{ordinal}:{}:{}:{}:child:{child_rank}",
            requirement.span.start,
            requirement.span.end,
            requirement.capability.stable_id()
        ));
    }
    for binding_index in ordered_bindings {
        let binding = &bindings[*binding_index];
        let mut capabilities = binding.program.capabilities().to_vec();
        capabilities.sort();
        capabilities.dedup();
        typed.push_str(&format!(
            "|child:{}:{}:{}:{}",
            binding.span.start,
            binding.span.end,
            capabilities
                .iter()
                .map(|capability| capability.stable_id())
                .collect::<Vec<_>>()
                .join(","),
            binding.program.semantic_digest()
        ));
    }
    hash_components(&[
        "oracle-clause-composition-typed/v1",
        ORACLE_CLAUSE_COMPOSITION_COMPILER_VERSION,
        ORACLE_CLAUSE_COMPOSITION_RUNTIME_VERSION,
        ORACLE_CLAUSE_COMPOSITION_RULES_CONTEXT_VERSION,
        composition.semantic_context.stable_id(),
        composition.exact_oracle(),
        composition.structural_digest(),
        &typed,
    ])
}

fn encode_node(node: &OracleCompositionNode, target: &mut String) {
    let span = node.span();
    match node {
        OracleCompositionNode::Atom(atom) => {
            target.push_str(&format!("atom({}:{})[", span.start, span.end));
            for capability in &atom.required_capabilities {
                target.push_str(capability.stable_id());
                target.push(',');
            }
            target.push(']');
        }
        OracleCompositionNode::Sequence {
            parts, separators, ..
        } => {
            target.push_str(&format!("sequence({}:{})[", span.start, span.end));
            for (index, part) in parts.iter().enumerate() {
                encode_node(part, target);
                if let Some(separator) = separators.get(index) {
                    target.push_str(&format!(
                        "<{:?}:{}:{}>",
                        separator.kind, separator.span.start, separator.span.end
                    ));
                }
            }
            target.push(']');
        }
        OracleCompositionNode::Conjunction {
            parts, separators, ..
        } => {
            target.push_str(&format!("conjunction({}:{})[", span.start, span.end));
            for (index, part) in parts.iter().enumerate() {
                encode_node(part, target);
                if let Some(separator) = separators.get(index) {
                    target.push_str(&format!(
                        "<{:?}:{}:{}>",
                        separator.kind, separator.span.start, separator.span.end
                    ));
                }
            }
            target.push(']');
        }
        OracleCompositionNode::Alternative {
            parts, separators, ..
        } => {
            target.push_str(&format!("alternative({}:{})[", span.start, span.end));
            for (index, part) in parts.iter().enumerate() {
                encode_node(part, target);
                if let Some(separator) = separators.get(index) {
                    target.push_str(&format!(
                        "<{:?}:{}:{}>",
                        separator.kind, separator.span.start, separator.span.end
                    ));
                }
            }
            target.push(']');
        }
        OracleCompositionNode::Conditional {
            kind,
            marker_span,
            condition,
            consequence,
            otherwise_marker_span,
            otherwise_body,
            ..
        } => {
            target.push_str(&format!(
                "conditional({kind:?}:{}:{}:{})[",
                span.start, span.end, marker_span.start
            ));
            encode_node(condition, target);
            target.push('|');
            encode_node(consequence, target);
            if let Some(otherwise_marker_span) = otherwise_marker_span {
                target.push_str(&format!(
                    "|otherwise:{}:{}|",
                    otherwise_marker_span.start, otherwise_marker_span.end
                ));
                if let Some(otherwise_body) = otherwise_body {
                    encode_node(otherwise_body, target);
                }
            }
            target.push(']');
        }
        OracleCompositionNode::OptionalChoice {
            actor_span,
            may_span,
            body,
            ..
        } => {
            target.push_str(&format!(
                "optional({}:{};actor:{}:{};may:{}:{})[",
                span.start,
                span.end,
                actor_span.start,
                actor_span.end,
                may_span.start,
                may_span.end
            ));
            encode_node(body, target);
            target.push(']');
        }
        OracleCompositionNode::ActivatedAbility {
            cost,
            colon_span,
            instruction,
            ..
        } => {
            target.push_str(&format!(
                "activated({}:{};colon:{}:{})[",
                span.start, span.end, colon_span.start, colon_span.end
            ));
            encode_node(&OracleCompositionNode::Atom(cost.clone()), target);
            target.push('|');
            encode_node(instruction, target);
            target.push(']');
        }
        OracleCompositionNode::DelayedInstruction {
            schedule,
            comma_span,
            instruction,
            ..
        } => {
            target.push_str(&format!(
                "delayed({}:{};comma:{}:{})[",
                span.start, span.end, comma_span.start, comma_span.end
            ));
            encode_node(&OracleCompositionNode::Atom(schedule.clone()), target);
            target.push('|');
            encode_node(instruction, target);
            target.push(']');
        }
        OracleCompositionNode::ModalGroup {
            header_span,
            selection,
            branches,
            ..
        } => {
            target.push_str(&format!(
                "modal({}:{};header:{}:{};selection:{selection:?})[",
                span.start, span.end, header_span.start, header_span.end
            ));
            for branch in branches {
                target.push_str(&format!(
                    "branch(marker:{}:{};body:{}:{})[",
                    branch.marker_span.start,
                    branch.marker_span.end,
                    branch.body_span.start,
                    branch.body_span.end
                ));
                encode_node(&branch.body, target);
                target.push(']');
            }
            target.push(']');
        }
        OracleCompositionNode::DetachedModalBranch {
            marker_span, body, ..
        } => {
            target.push_str(&format!(
                "detached-modal({}:{};marker:{}:{})[",
                span.start, span.end, marker_span.start, marker_span.end
            ));
            encode_node(body, target);
            target.push(']');
        }
        OracleCompositionNode::EmbeddedAbilities {
            outer, abilities, ..
        } => {
            target.push_str(&format!("embedded({}:{})[", span.start, span.end));
            encode_node(outer, target);
            for ability in abilities {
                target.push_str(&format!(
                    "|quote:{:?}:{}:{}:{}:{}[",
                    ability.kind,
                    ability.quote_span.start,
                    ability.quote_span.end,
                    ability.content_span.start,
                    ability.content_span.end
                ));
                encode_node(&ability.body, target);
                target.push(']');
            }
            target.push(']');
        }
    }
}

fn hash_components(components: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for component in components {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn valid_semantic_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}
