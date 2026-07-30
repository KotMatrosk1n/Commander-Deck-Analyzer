//! Content keyed assembly and transactional execution for complete Oracle
//! face modal groups.
//!
//! This module joins a modal header only to the immediately following bullet
//! branches on the same face. A branch becomes part of a program only after a
//! caller supplied typed child compiler proves that the complete branch body
//! is closed. Recognition alone is never executable coverage.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const ORACLE_FACE_PROGRAM_ASSEMBLER_COMPILER_VERSION: &str =
    "oracle-face-program-assembler-compiler-0.1";
pub const ORACLE_FACE_PROGRAM_ASSEMBLER_RUNTIME_VERSION: &str =
    "oracle-face-program-assembler-runtime-0.1";
pub const ORACLE_FACE_MODAL_RULES_CONTEXT_VERSION: &str =
    "magic-comprehensive-rules-2026-06-19:601.2b,700.2,700.2a-g";

pub const fn oracle_face_program_assembler_production_adapter_connected() -> bool {
    false
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

/// Occurrence data retained for diagnostics only. None of these fields are
/// included in semantic identity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OracleFaceProvenance<'a> {
    pub source_name: Option<&'a str>,
    pub snapshot_sha256: Option<&'a str>,
    pub face_index: Option<u16>,
    pub row_ordinal: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnedOracleFaceProvenance {
    pub source_name: Option<String>,
    pub snapshot_sha256: Option<String>,
    pub face_index: Option<u16>,
    pub row_ordinal: Option<u64>,
}

impl From<OracleFaceProvenance<'_>> for OwnedOracleFaceProvenance {
    fn from(value: OracleFaceProvenance<'_>) -> Self {
        Self {
            source_name: value.source_name.map(str::to_owned),
            snapshot_sha256: value.snapshot_sha256.map(str::to_owned),
            face_index: value.face_index,
            row_ordinal: value.row_ordinal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleFaceProgramInput<'a> {
    pub exact_oracle_text: &'a str,
    pub provenance: OracleFaceProvenance<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModalCardinality {
    ExactlyOne,
    OneOrBoth,
    OneOrMore,
    ExactlyTwo,
    ExactlyThree,
}

impl ModalCardinality {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::ExactlyOne => "exactly-one/v1",
            Self::OneOrBoth => "one-or-both/v1",
            Self::OneOrMore => "one-or-more/v1",
            Self::ExactlyTwo => "exactly-two/v1",
            Self::ExactlyThree => "exactly-three/v1",
        }
    }

    fn legal_range(self, branch_count: usize) -> Option<(usize, usize)> {
        match self {
            Self::ExactlyOne => (branch_count >= 2).then_some((1, 1)),
            Self::OneOrBoth => (branch_count == 2).then_some((1, 2)),
            Self::OneOrMore => (branch_count >= 2).then_some((1, branch_count)),
            Self::ExactlyTwo => (branch_count >= 2).then_some((2, 2)),
            Self::ExactlyThree => (branch_count >= 3).then_some((3, 3)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModalSelectionPolicy {
    pub cardinality: ModalCardinality,
    pub same_mode_may_repeat: bool,
}

impl ModalSelectionPolicy {
    fn stable_id(self) -> String {
        format!(
            "{};repeat={}",
            self.cardinality.stable_id(),
            self.same_mode_may_repeat
        )
    }

    fn legal_range(self, branch_count: usize) -> Option<(usize, usize)> {
        if self.same_mode_may_repeat {
            return (branch_count >= 2
                && matches!(
                    self.cardinality,
                    ModalCardinality::ExactlyTwo | ModalCardinality::ExactlyThree
                ))
            .then(|| match self.cardinality {
                ModalCardinality::ExactlyTwo => (2, 2),
                ModalCardinality::ExactlyThree => (3, 3),
                _ => unreachable!("repeat wording is restricted to exact multi-mode choices"),
            });
        }
        self.cardinality.legal_range(branch_count)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalChildRole {
    HeaderEnvelope,
    Branch { branch_index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModalChildSource<'a> {
    pub complete_face_source: &'a str,
    pub exact_source: &'a str,
    pub source_span: SourceSpan,
    pub role: ModalChildRole,
}

/// A typed child compiler returns this proof only when it consumed the entire
/// branch body. The assembler independently verifies the exact source and
/// span before accepting it. The child semantic digest must itself be keyed
/// only to the exact child content, relevant semantic context, and versioned
/// compiler and runtime contracts. Occurrence metadata and source coordinates
/// must not be included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedModalChildProgram<P> {
    pub program: P,
    pub exact_source: String,
    pub source_span: SourceSpan,
    pub semantic_digest: String,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalChildCompilation<P> {
    Closed(ClosedModalChildProgram<P>),
    Incomplete { detail: String },
    Unsupported { detail: String },
}

pub trait ClosedModalChildCompiler {
    type Program;

    fn compile_closed_child(
        &mut self,
        source: ModalChildSource<'_>,
    ) -> ModalChildCompilation<Self::Program>;
}

impl<P, F> ClosedModalChildCompiler for F
where
    F: for<'a> FnMut(ModalChildSource<'a>) -> ModalChildCompilation<P>,
{
    type Program = P;

    fn compile_closed_child(
        &mut self,
        source: ModalChildSource<'_>,
    ) -> ModalChildCompilation<Self::Program> {
        self(source)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledModalBranch<P> {
    pub marker_span: SourceSpan,
    pub body_span: SourceSpan,
    pub exact_body: String,
    pub child_semantic_digest: String,
    pub child_program: P,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledModalHeader<P> {
    pub source_span: SourceSpan,
    pub selection_span: SourceSpan,
    pub envelope_span: Option<SourceSpan>,
    pub exact_source: String,
    pub envelope_semantic_digest: Option<String>,
    pub envelope_program: Option<P>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledModalProgram<P> {
    exact_source: String,
    source_span: SourceSpan,
    header_span: SourceSpan,
    selection_span: SourceSpan,
    selection_policy: ModalSelectionPolicy,
    semantic_digest: String,
    provenance: OwnedOracleFaceProvenance,
    header: AssembledModalHeader<P>,
    branches: Vec<AssembledModalBranch<P>>,
}

impl<P> AssembledModalProgram<P> {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub const fn source_span(&self) -> SourceSpan {
        self.source_span
    }

    pub const fn header_span(&self) -> SourceSpan {
        self.header_span
    }

    pub const fn selection_span(&self) -> SourceSpan {
        self.selection_span
    }

    pub const fn selection_policy(&self) -> ModalSelectionPolicy {
        self.selection_policy
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn provenance(&self) -> &OwnedOracleFaceProvenance {
        &self.provenance
    }

    pub fn header(&self) -> &AssembledModalHeader<P> {
        &self.header
    }

    pub fn branches(&self) -> &[AssembledModalBranch<P>] {
        &self.branches
    }

    pub const fn production_adapter_connected(&self) -> bool {
        oracle_face_program_assembler_production_adapter_connected()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalAssemblyError {
    OrphanBranch {
        provenance: OwnedOracleFaceProvenance,
        marker_span: SourceSpan,
    },
    MissingBranches {
        provenance: OwnedOracleFaceProvenance,
        header_span: SourceSpan,
    },
    DuplicateBranch {
        provenance: OwnedOracleFaceProvenance,
        first_body_span: SourceSpan,
        duplicate_body_span: SourceSpan,
    },
    UnsupportedHeader {
        provenance: OwnedOracleFaceProvenance,
        header_span: SourceSpan,
    },
    CrossFaceBoundary {
        provenance: OwnedOracleFaceProvenance,
        boundary_span: SourceSpan,
    },
    IllegalCardinality {
        provenance: OwnedOracleFaceProvenance,
        header_span: SourceSpan,
        branch_count: usize,
        policy: ModalSelectionPolicy,
    },
    IncompleteChild {
        provenance: OwnedOracleFaceProvenance,
        body_span: SourceSpan,
        detail: String,
    },
    ChildSourceMismatch {
        provenance: OwnedOracleFaceProvenance,
        expected_span: SourceSpan,
        returned_span: SourceSpan,
    },
    DuplicateChildIdentity {
        provenance: OwnedOracleFaceProvenance,
        first_body_span: SourceSpan,
        duplicate_body_span: SourceSpan,
    },
}

impl fmt::Display for ModalAssemblyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OrphanBranch { marker_span, .. } => {
                write!(
                    formatter,
                    "modal branch at {marker_span:?} has no header on its face"
                )
            }
            Self::MissingBranches { header_span, .. } => {
                write!(
                    formatter,
                    "modal header at {header_span:?} has no complete branches"
                )
            }
            Self::DuplicateBranch {
                duplicate_body_span,
                ..
            } => write!(
                formatter,
                "modal branch at {duplicate_body_span:?} duplicates an earlier branch"
            ),
            Self::UnsupportedHeader { header_span, .. } => {
                write!(
                    formatter,
                    "modal header at {header_span:?} uses unsupported wording"
                )
            }
            Self::CrossFaceBoundary { boundary_span, .. } => write!(
                formatter,
                "face input contains a cross-face boundary at {boundary_span:?}"
            ),
            Self::IllegalCardinality {
                header_span,
                branch_count,
                ..
            } => write!(
                formatter,
                "modal header at {header_span:?} is illegal for {branch_count} branches"
            ),
            Self::IncompleteChild {
                body_span, detail, ..
            } => write!(
                formatter,
                "modal branch at {body_span:?} is not a closed typed child: {detail}"
            ),
            Self::ChildSourceMismatch {
                expected_span,
                returned_span,
                ..
            } => write!(
                formatter,
                "modal child source {returned_span:?} does not match {expected_span:?}"
            ),
            Self::DuplicateChildIdentity {
                duplicate_body_span,
                ..
            } => write!(
                formatter,
                "modal child at {duplicate_body_span:?} duplicates an earlier child identity"
            ),
        }
    }
}

impl std::error::Error for ModalAssemblyError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModalExecutionStep {
    pub branch_index: usize,
    pub repeat_ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalResolutionError<E> {
    InvalidSelection(InvalidModalSelection),
    Child { step: ModalExecutionStep, error: E },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidModalSelection {
    WrongCount {
        minimum: usize,
        maximum: usize,
        actual: usize,
    },
    UnknownMode {
        branch_index: usize,
    },
    RepeatedMode {
        branch_index: usize,
    },
}

impl<P> AssembledModalProgram<P> {
    pub fn validate_selection(
        &self,
        selected_branch_indices: &[usize],
    ) -> Result<(), InvalidModalSelection> {
        let (minimum, maximum) = self
            .selection_policy
            .legal_range(self.branches.len())
            .expect("assembled programs always retain a legal cardinality");
        if selected_branch_indices.len() < minimum || selected_branch_indices.len() > maximum {
            return Err(InvalidModalSelection::WrongCount {
                minimum,
                maximum,
                actual: selected_branch_indices.len(),
            });
        }

        let mut selected = BTreeSet::new();
        for branch_index in selected_branch_indices.iter().copied() {
            if branch_index >= self.branches.len() {
                return Err(InvalidModalSelection::UnknownMode { branch_index });
            }
            if !selected.insert(branch_index) && !self.selection_policy.same_mode_may_repeat {
                return Err(InvalidModalSelection::RepeatedMode { branch_index });
            }
        }
        Ok(())
    }

    /// Resolves the selected modes against a staged clone and commits only
    /// after every selected closed child succeeds. Repeated modes are
    /// evaluated as repeated instructions in printed branch order.
    pub fn resolve_transactionally<S, E, F>(
        &self,
        selected_branch_indices: &[usize],
        state: &mut S,
        mut execute_child: F,
    ) -> Result<(), ModalResolutionError<E>>
    where
        S: Clone,
        F: FnMut(&P, ModalExecutionStep, &mut S) -> Result<(), E>,
    {
        self.validate_selection(selected_branch_indices)
            .map_err(ModalResolutionError::InvalidSelection)?;

        let mut repetitions = BTreeMap::<usize, usize>::new();
        for branch_index in selected_branch_indices.iter().copied() {
            *repetitions.entry(branch_index).or_default() += 1;
        }

        let mut staged = state.clone();
        for (branch_index, repeat_count) in repetitions {
            let branch = &self.branches[branch_index];
            for repeat_ordinal in 0..repeat_count {
                let step = ModalExecutionStep {
                    branch_index,
                    repeat_ordinal,
                };
                execute_child(&branch.child_program, step, &mut staged)
                    .map_err(|error| ModalResolutionError::Child { step, error })?;
            }
        }
        *state = staged;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct PhysicalLine {
    trimmed_span: SourceSpan,
}

#[derive(Debug, Clone, Copy)]
struct ParsedModalHeader {
    header_span: SourceSpan,
    selection_span: SourceSpan,
    envelope_span: Option<SourceSpan>,
    policy: ModalSelectionPolicy,
}

#[derive(Debug, Clone, Copy)]
struct ParsedModalBranch {
    line_span: SourceSpan,
    marker_span: SourceSpan,
    body_span: SourceSpan,
}

#[derive(Debug)]
struct RawModalGroup {
    header: ParsedModalHeader,
    branches: Vec<ParsedModalBranch>,
}

pub fn assemble_oracle_face_modal_programs<C>(
    input: OracleFaceProgramInput<'_>,
    child_compiler: &mut C,
) -> Result<Vec<AssembledModalProgram<C::Program>>, ModalAssemblyError>
where
    C: ClosedModalChildCompiler,
{
    let provenance = OwnedOracleFaceProvenance::from(input.provenance);
    let lines = physical_lines(input.exact_oracle_text);

    if let Some(boundary_span) = lines.iter().find_map(|line| {
        (line.trimmed_span.slice(input.exact_oracle_text) == Some("//"))
            .then_some(line.trimmed_span)
    }) {
        return Err(ModalAssemblyError::CrossFaceBoundary {
            provenance,
            boundary_span,
        });
    }

    let mut raw_groups = Vec::new();
    let mut line_index = 0usize;
    while line_index < lines.len() {
        let line = lines[line_index];
        if line.trimmed_span.is_empty() {
            line_index += 1;
            continue;
        }
        if let Some(branch) = parse_modal_branch(input.exact_oracle_text, line.trimmed_span) {
            return Err(ModalAssemblyError::OrphanBranch {
                provenance,
                marker_span: branch.marker_span,
            });
        }
        let Some(header) = parse_supported_modal_header(input.exact_oracle_text, line.trimmed_span)
        else {
            if looks_like_unsupported_modal_header(input.exact_oracle_text, line.trimmed_span) {
                return Err(ModalAssemblyError::UnsupportedHeader {
                    provenance,
                    header_span: line.trimmed_span,
                });
            }
            line_index += 1;
            continue;
        };

        let mut branches = Vec::new();
        let mut branch_line_index = line_index + 1;
        while branch_line_index < lines.len() {
            let candidate = lines[branch_line_index];
            let Some(branch) = parse_modal_branch(input.exact_oracle_text, candidate.trimmed_span)
            else {
                break;
            };
            branches.push(branch);
            branch_line_index += 1;
        }
        if branches.is_empty() {
            return Err(ModalAssemblyError::MissingBranches {
                provenance,
                header_span: header.header_span,
            });
        }
        raw_groups.push(RawModalGroup { header, branches });
        line_index = branch_line_index;
    }

    let mut programs = Vec::with_capacity(raw_groups.len());
    for raw_group in raw_groups {
        programs.push(assemble_raw_group(
            input.exact_oracle_text,
            &provenance,
            raw_group,
            child_compiler,
        )?);
    }
    Ok(programs)
}

/// Assemble only the modal group that owns `source_offset`.
///
/// This occurrence-scoped entry point keeps an unrelated unsupported group on
/// the same face from hiding a complete target group. If the target offset is
/// itself inside an orphan branch or an unsupported header group, that exact
/// structural error is still returned.
pub fn assemble_oracle_face_modal_program_containing_offset<C>(
    input: OracleFaceProgramInput<'_>,
    source_offset: usize,
    child_compiler: &mut C,
) -> Result<Option<AssembledModalProgram<C::Program>>, ModalAssemblyError>
where
    C: ClosedModalChildCompiler,
{
    let provenance = OwnedOracleFaceProvenance::from(input.provenance);
    let lines = physical_lines(input.exact_oracle_text);
    if let Some(boundary_span) = lines.iter().find_map(|line| {
        (line.trimmed_span.slice(input.exact_oracle_text) == Some("//"))
            .then_some(line.trimmed_span)
    }) {
        return Err(ModalAssemblyError::CrossFaceBoundary {
            provenance,
            boundary_span,
        });
    }

    let owns_offset = |span: SourceSpan| span.start <= source_offset && source_offset < span.end;
    let mut line_index = 0usize;
    while line_index < lines.len() {
        let line = lines[line_index];
        if line.trimmed_span.is_empty() {
            line_index += 1;
            continue;
        }
        if let Some(branch) = parse_modal_branch(input.exact_oracle_text, line.trimmed_span) {
            if owns_offset(branch.line_span) {
                return Err(ModalAssemblyError::OrphanBranch {
                    provenance,
                    marker_span: branch.marker_span,
                });
            }
            line_index += 1;
            continue;
        }

        let supported_header =
            parse_supported_modal_header(input.exact_oracle_text, line.trimmed_span);
        let unsupported_header = supported_header.is_none()
            && looks_like_unsupported_modal_header(input.exact_oracle_text, line.trimmed_span);
        if supported_header.is_none() && !unsupported_header {
            line_index += 1;
            continue;
        }

        let mut branches = Vec::new();
        let mut branch_line_index = line_index + 1;
        while branch_line_index < lines.len() {
            let candidate = lines[branch_line_index];
            let Some(branch) = parse_modal_branch(input.exact_oracle_text, candidate.trimmed_span)
            else {
                break;
            };
            branches.push(branch);
            branch_line_index += 1;
        }
        let target_in_group = owns_offset(line.trimmed_span)
            || branches.iter().any(|branch| owns_offset(branch.line_span));
        if !target_in_group {
            line_index = branch_line_index.max(line_index + 1);
            continue;
        }
        if unsupported_header {
            return Err(ModalAssemblyError::UnsupportedHeader {
                provenance,
                header_span: line.trimmed_span,
            });
        }
        let header = supported_header.expect("unsupported headers returned above");
        if branches.is_empty() {
            return Err(ModalAssemblyError::MissingBranches {
                provenance,
                header_span: header.header_span,
            });
        }
        return assemble_raw_group(
            input.exact_oracle_text,
            &provenance,
            RawModalGroup { header, branches },
            child_compiler,
        )
        .map(Some);
    }
    Ok(None)
}

fn assemble_raw_group<C>(
    complete_face_source: &str,
    provenance: &OwnedOracleFaceProvenance,
    raw_group: RawModalGroup,
    child_compiler: &mut C,
) -> Result<AssembledModalProgram<C::Program>, ModalAssemblyError>
where
    C: ClosedModalChildCompiler,
{
    if raw_group
        .header
        .policy
        .legal_range(raw_group.branches.len())
        .is_none()
    {
        return Err(ModalAssemblyError::IllegalCardinality {
            provenance: provenance.clone(),
            header_span: raw_group.header.header_span,
            branch_count: raw_group.branches.len(),
            policy: raw_group.header.policy,
        });
    }

    let mut exact_branch_spans = BTreeMap::<String, SourceSpan>::new();
    for branch in &raw_group.branches {
        let Some(exact_body) = branch.body_span.slice(complete_face_source) else {
            return Err(ModalAssemblyError::IncompleteChild {
                provenance: provenance.clone(),
                body_span: branch.body_span,
                detail: "branch body span is outside the complete face source".into(),
            });
        };
        if exact_body.is_empty() {
            return Err(ModalAssemblyError::IncompleteChild {
                provenance: provenance.clone(),
                body_span: branch.body_span,
                detail: "branch body is empty".into(),
            });
        }
        if let Some(first_body_span) =
            exact_branch_spans.insert(exact_body.to_owned(), branch.body_span)
        {
            return Err(ModalAssemblyError::DuplicateBranch {
                provenance: provenance.clone(),
                first_body_span,
                duplicate_body_span: branch.body_span,
            });
        }
    }

    let header_envelope = if raw_group.header.envelope_span.is_some() {
        Some(compile_verified_child(
            complete_face_source,
            provenance,
            raw_group.header.header_span,
            ModalChildRole::HeaderEnvelope,
            child_compiler,
        )?)
    } else {
        None
    };
    let assembled_header = AssembledModalHeader {
        source_span: raw_group.header.header_span,
        selection_span: raw_group.header.selection_span,
        envelope_span: raw_group.header.envelope_span,
        exact_source: raw_group
            .header
            .header_span
            .slice(complete_face_source)
            .expect("parsed header span belongs to the complete face source")
            .to_owned(),
        envelope_semantic_digest: header_envelope
            .as_ref()
            .map(|header| header.semantic_digest.clone()),
        envelope_program: header_envelope.map(|header| header.program),
    };

    let mut assembled_branches = Vec::with_capacity(raw_group.branches.len());
    let mut child_identities = BTreeMap::<String, SourceSpan>::new();
    for (branch_index, branch) in raw_group.branches.iter().copied().enumerate() {
        let child = compile_verified_child(
            complete_face_source,
            provenance,
            branch.body_span,
            ModalChildRole::Branch { branch_index },
            child_compiler,
        )?;
        if let Some(first_body_span) =
            child_identities.insert(child.semantic_digest.clone(), branch.body_span)
        {
            return Err(ModalAssemblyError::DuplicateChildIdentity {
                provenance: provenance.clone(),
                first_body_span,
                duplicate_body_span: branch.body_span,
            });
        }
        assembled_branches.push(AssembledModalBranch {
            marker_span: branch.marker_span,
            body_span: branch.body_span,
            exact_body: child.exact_source,
            child_semantic_digest: child.semantic_digest,
            child_program: child.program,
        });
    }

    let source_span = SourceSpan::new(
        raw_group.header.header_span.start,
        raw_group
            .branches
            .last()
            .expect("nonempty branches checked above")
            .line_span
            .end,
    );
    let exact_source = source_span
        .slice(complete_face_source)
        .expect("assembled spans derive from the complete face source")
        .to_owned();
    let semantic_digest = modal_program_semantic_digest(
        &assembled_header,
        raw_group.header.policy,
        &assembled_branches,
    );

    Ok(AssembledModalProgram {
        exact_source,
        source_span,
        header_span: raw_group.header.header_span,
        selection_span: raw_group.header.selection_span,
        selection_policy: raw_group.header.policy,
        semantic_digest,
        provenance: provenance.clone(),
        header: assembled_header,
        branches: assembled_branches,
    })
}

fn compile_verified_child<C>(
    complete_face_source: &str,
    provenance: &OwnedOracleFaceProvenance,
    source_span: SourceSpan,
    role: ModalChildRole,
    child_compiler: &mut C,
) -> Result<ClosedModalChildProgram<C::Program>, ModalAssemblyError>
where
    C: ClosedModalChildCompiler,
{
    let exact_source = source_span.slice(complete_face_source).ok_or_else(|| {
        ModalAssemblyError::IncompleteChild {
            provenance: provenance.clone(),
            body_span: source_span,
            detail: "child span is outside the complete face source".into(),
        }
    })?;
    let compiled = child_compiler.compile_closed_child(ModalChildSource {
        complete_face_source,
        exact_source,
        source_span,
        role,
    });
    let child = match compiled {
        ModalChildCompilation::Closed(child) => child,
        ModalChildCompilation::Incomplete { detail }
        | ModalChildCompilation::Unsupported { detail } => {
            return Err(ModalAssemblyError::IncompleteChild {
                provenance: provenance.clone(),
                body_span: source_span,
                detail,
            });
        }
    };
    if !child.complete {
        return Err(ModalAssemblyError::IncompleteChild {
            provenance: provenance.clone(),
            body_span: source_span,
            detail: "child compiler did not prove complete source consumption".into(),
        });
    }
    if child.source_span != source_span || child.exact_source != exact_source {
        return Err(ModalAssemblyError::ChildSourceMismatch {
            provenance: provenance.clone(),
            expected_span: source_span,
            returned_span: child.source_span,
        });
    }
    if child.semantic_digest.trim().is_empty() {
        return Err(ModalAssemblyError::IncompleteChild {
            provenance: provenance.clone(),
            body_span: source_span,
            detail: "child semantic identity is empty".into(),
        });
    }
    Ok(child)
}

fn modal_program_semantic_digest<H, P>(
    header: &AssembledModalHeader<H>,
    policy: ModalSelectionPolicy,
    branches: &[AssembledModalBranch<P>],
) -> String {
    let mut hasher = Sha256::new();
    update_digest_field(
        &mut hasher,
        "compiler",
        ORACLE_FACE_PROGRAM_ASSEMBLER_COMPILER_VERSION,
    );
    update_digest_field(
        &mut hasher,
        "runtime",
        ORACLE_FACE_PROGRAM_ASSEMBLER_RUNTIME_VERSION,
    );
    update_digest_field(
        &mut hasher,
        "rules",
        ORACLE_FACE_MODAL_RULES_CONTEXT_VERSION,
    );
    update_digest_field(&mut hasher, "header", &header.exact_source);
    update_digest_field(
        &mut hasher,
        "header-envelope-child",
        header
            .envelope_semantic_digest
            .as_deref()
            .unwrap_or("assembler-owned-pure-selection/v1"),
    );
    update_digest_field(&mut hasher, "selection", &policy.stable_id());
    update_digest_field(&mut hasher, "branch-count", &branches.len().to_string());
    for (branch_index, branch) in branches.iter().enumerate() {
        update_digest_field(&mut hasher, "branch-index", &branch_index.to_string());
        update_digest_field(&mut hasher, "branch-source", &branch.exact_body);
        update_digest_field(&mut hasher, "branch-child", &branch.child_semantic_digest);
    }
    format!("{:X}", hasher.finalize())
}

fn update_digest_field(hasher: &mut Sha256, label: &str, value: &str) {
    hasher.update(
        u64::try_from(label.len())
            .expect("semantic digest label length fits u64")
            .to_le_bytes(),
    );
    hasher.update(label.as_bytes());
    hasher.update(
        u64::try_from(value.len())
            .expect("semantic digest value length fits u64")
            .to_le_bytes(),
    );
    hasher.update(value.as_bytes());
}

fn physical_lines(source: &str) -> Vec<PhysicalLine> {
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    for (byte_index, byte) in source.bytes().enumerate() {
        if byte != b'\n' {
            continue;
        }
        let mut line_end = byte_index;
        if line_end > line_start && source.as_bytes()[line_end - 1] == b'\r' {
            line_end -= 1;
        }
        lines.push(PhysicalLine {
            trimmed_span: trim_source_span(source, SourceSpan::new(line_start, line_end)),
        });
        line_start = byte_index + 1;
    }
    if line_start <= source.len() {
        let mut line_end = source.len();
        if line_end > line_start && source.as_bytes()[line_end - 1] == b'\r' {
            line_end -= 1;
        }
        lines.push(PhysicalLine {
            trimmed_span: trim_source_span(source, SourceSpan::new(line_start, line_end)),
        });
    }
    lines
}

fn trim_source_span(source: &str, span: SourceSpan) -> SourceSpan {
    let mut start = span.start.min(source.len());
    let mut end = span.end.min(source.len());
    while start < end {
        let Some(character) = source[start..end].chars().next() else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        start += character.len_utf8();
    }
    while start < end {
        let Some(character) = source[start..end].chars().next_back() else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        end -= character.len_utf8();
    }
    SourceSpan::new(start, end)
}

fn parse_modal_branch(source: &str, line_span: SourceSpan) -> Option<ParsedModalBranch> {
    let line = line_span.slice(source)?;
    if !line.starts_with('\u{2022}') {
        return None;
    }
    let marker_span = SourceSpan::new(line_span.start, line_span.start + '\u{2022}'.len_utf8());
    let body_span = trim_source_span(source, SourceSpan::new(marker_span.end, line_span.end));
    Some(ParsedModalBranch {
        line_span,
        marker_span,
        body_span,
    })
}

fn parse_supported_modal_header(source: &str, line_span: SourceSpan) -> Option<ParsedModalHeader> {
    let line = line_span.slice(source)?;
    let (header_core_end, has_modal_separator) =
        if let Some(without_dash) = line.strip_suffix('\u{2014}') {
            (without_dash.len(), true)
        } else {
            (line.len(), false)
        };
    let core_span = trim_source_span(
        source,
        SourceSpan::new(line_span.start, line_span.start + header_core_end),
    );
    let core = core_span.slice(source)?;
    let lower = core.to_ascii_lowercase();
    let (choose_start, policy) = lower
        .match_indices("choose ")
        .filter(|(choose_start, _)| {
            *choose_start == 0
                || lower[..*choose_start]
                    .chars()
                    .next_back()
                    .is_some_and(|character| !character.is_ascii_alphanumeric() && character != '_')
        })
        .filter_map(|(choose_start, _)| {
            parse_modal_selection_policy(&lower[choose_start..])
                .map(|policy| (choose_start, policy))
        })
        .last()?;
    if !has_modal_separator && !policy.same_mode_may_repeat {
        return None;
    }
    let selection_span = SourceSpan::new(core_span.start + choose_start, core_span.end);
    let envelope_span = trim_source_span(
        source,
        SourceSpan::new(line_span.start, selection_span.start),
    );
    Some(ParsedModalHeader {
        header_span: line_span,
        selection_span,
        envelope_span: (!envelope_span.is_empty()).then_some(envelope_span),
        policy,
    })
}

fn parse_modal_selection_policy(selection: &str) -> Option<ModalSelectionPolicy> {
    const REPEAT_SUFFIX: &str = ". you may choose the same mode more than once.";
    let (cardinality_text, same_mode_may_repeat) =
        if let Some(base) = selection.strip_suffix(REPEAT_SUFFIX) {
            (base, true)
        } else {
            (selection, false)
        };
    let cardinality = match cardinality_text {
        "choose one" => ModalCardinality::ExactlyOne,
        "choose one or both" => ModalCardinality::OneOrBoth,
        "choose one or more" => ModalCardinality::OneOrMore,
        "choose two" => ModalCardinality::ExactlyTwo,
        "choose three" => ModalCardinality::ExactlyThree,
        _ => return None,
    };
    if same_mode_may_repeat
        && !matches!(
            cardinality,
            ModalCardinality::ExactlyTwo | ModalCardinality::ExactlyThree
        )
    {
        return None;
    }
    Some(ModalSelectionPolicy {
        cardinality,
        same_mode_may_repeat,
    })
}

fn looks_like_unsupported_modal_header(source: &str, line_span: SourceSpan) -> bool {
    let Some(line) = line_span.slice(source) else {
        return false;
    };
    let (core, has_modal_separator) = if let Some(without_dash) = line.strip_suffix('\u{2014}') {
        (without_dash, true)
    } else {
        (line, false)
    };
    let lower = core.trim().to_ascii_lowercase();
    if !has_modal_separator && !lower.contains("choose the same mode more than once") {
        return false;
    }
    [
        "choose one",
        "choose two",
        "choose three",
        "choose four",
        "choose up to",
        "choose any number",
        "choose a mode",
    ]
    .into_iter()
    .any(|needle| {
        lower.match_indices(needle).any(|(index, _)| {
            index == 0
                || lower[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|character| !character.is_ascii_alphanumeric() && character != '_')
        })
    })
}
