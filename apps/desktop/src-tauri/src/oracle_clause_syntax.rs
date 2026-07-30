//! Lossless syntax recognition for one complete normalized Oracle line.
//!
//! This module deliberately does not claim executable semantics. Its job is to
//! retain the complete source line, expose useful top-level structure, and keep
//! every unresolved leaf visible for later semantic compilers.

use std::fmt;

use sha2::{Digest, Sha256};

pub const ORACLE_CLAUSE_SYNTAX_COMPILER_VERSION: &str = "oracle-clause-syntax-compiler-0.2";
pub const ORACLE_CLAUSE_SYNTAX_RUNTIME_VERSION: &str = "oracle-clause-syntax-runtime-0.1";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OracleSyntaxSemanticContext {
    /// Rules text printed on a card or one of its retained faces. This includes
    /// Dungeon, Attraction, and sticker-sheet card records. The other variants
    /// below are reserved for rules objects parsed outside a card record.
    #[default]
    CardFace,
    Emblem,
    DungeonRoom,
    AttractionLight,
    StickerAbility,
}

impl OracleSyntaxSemanticContext {
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::CardFace => "card-face/v1",
            Self::Emblem => "emblem/v1",
            Self::DungeonRoom => "dungeon-room/v1",
            Self::AttractionLight => "attraction-light/v1",
            Self::StickerAbility => "sticker-ability/v1",
        }
    }
}

/// Source coordinates are retained for diagnostics only. They are
/// intentionally excluded from the syntax digest.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OracleSyntaxProvenance<'a> {
    pub source_name: Option<&'a str>,
    pub snapshot_sha256: Option<&'a str>,
    pub face_index: Option<u16>,
    pub clause_index: Option<u16>,
    pub row_ordinal: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleClauseSyntaxInput<'a> {
    pub normalized_line: &'a str,
    pub semantic_context: OracleSyntaxSemanticContext,
    pub provenance: OracleSyntaxProvenance<'a>,
}

impl<'a> OracleClauseSyntaxInput<'a> {
    pub fn card_face(normalized_line: &'a str) -> Self {
        Self {
            normalized_line,
            semantic_context: OracleSyntaxSemanticContext::CardFace,
            provenance: OracleSyntaxProvenance::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LosslessTokenKind {
    Whitespace,
    Word,
    Number,
    ManaSymbol,
    Bullet,
    QuoteDelimiter,
    LiteralQuoteGlyph,
    OpenDelimiter,
    CloseDelimiter,
    Punctuation,
    Symbol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LosslessToken {
    pub kind: LosslessTokenKind,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleAtomKind {
    KnownAction,
    KnownKeyword,
    RestrictionOrCharacteristic,
    ReferenceOrAmount,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleSyntaxAtom {
    pub kind: OracleAtomKind,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityWordSyntax {
    pub label_span: ByteSpan,
    pub separator_span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModalHeaderSyntax {
    pub header_span: ByteSpan,
    pub separator_span: Option<ByteSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModalBranchSyntax {
    pub marker_span: ByteSpan,
    pub label_span: Option<ByteSpan>,
    pub separator_span: Option<ByteSpan>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModalSyntax {
    pub header: Option<ModalHeaderSyntax>,
    pub branch: Option<ModalBranchSyntax>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadInKind {
    Trigger,
    Timing,
    Condition,
    Replacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeadInSyntax {
    pub kind: LeadInKind,
    pub prefix_span: ByteSpan,
    pub separator_span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivatedAbilitySyntax {
    pub cost_span: ByteSpan,
    pub separator_span: ByteSpan,
    /// A line may end with an activation colon when its effects are retained
    /// on following bullet lines. That is a continuation, not an empty effect
    /// silently accepted as executable.
    pub effect_span: Option<ByteSpan>,
    pub continues_on_following_line: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstructionSentenceSyntax {
    pub span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopLevelConjunctionKind {
    And,
    Or,
    AndOr,
    Then,
    But,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopLevelConjunctionSyntax {
    pub kind: TopLevelConjunctionKind,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotedAbilitySyntax {
    pub quote_span: ByteSpan,
    pub content_span: ByteSpan,
    pub is_granted_ability: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrailingReminderSyntax {
    pub reminder_span: ByteSpan,
    pub content_span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleClauseStructure {
    pub full_span: ByteSpan,
    pub ability_word: Option<AbilityWordSyntax>,
    pub modal: Option<ModalSyntax>,
    pub lead_in: Option<LeadInSyntax>,
    pub activation: Option<ActivatedAbilitySyntax>,
    pub instruction_sentences: Vec<InstructionSentenceSyntax>,
    pub top_level_conjunctions: Vec<TopLevelConjunctionSyntax>,
    pub quoted_abilities: Vec<QuotedAbilitySyntax>,
    /// Odd legacy straight quote glyphs occur in a small acorn-card slice.
    /// They are retained as literal source glyphs instead of being treated as
    /// a quotation that could swallow the remainder of the line.
    pub literal_quote_glyphs: Vec<ByteSpan>,
    pub trailing_reminder: Option<TrailingReminderSyntax>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecognizedOracleClauseSyntax {
    compiler_version: &'static str,
    runtime_version: &'static str,
    semantic_context: OracleSyntaxSemanticContext,
    syntax_digest: String,
    normalized_line: String,
    provenance: OwnedOracleSyntaxProvenance,
    lossless_tokens: Vec<LosslessToken>,
    structure: OracleClauseStructure,
    atoms: Vec<OracleSyntaxAtom>,
}

/// Proof that one exact line passed the canonical syntax gate. The source
/// reference is private so crate callers cannot fabricate this token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValidatedOracleClauseLine<'a> {
    line: &'a str,
}

impl ValidatedOracleClauseLine<'_> {
    pub(crate) fn line(&self) -> &str {
        self.line
    }
}

// Syntax artifacts are content addressed and may be recomputed across card
// snapshots. Provenance remains occurrence data and must never be used as part
// of the syntax identity. A syntax digest is not an executable receipt and
// cannot authorize simulation, scoring, or coverage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnedOracleSyntaxProvenance {
    pub source_name: Option<String>,
    pub snapshot_sha256: Option<String>,
    pub face_index: Option<u16>,
    pub clause_index: Option<u16>,
    pub row_ordinal: Option<u64>,
}

impl From<OracleSyntaxProvenance<'_>> for OwnedOracleSyntaxProvenance {
    fn from(value: OracleSyntaxProvenance<'_>) -> Self {
        Self {
            source_name: value.source_name.map(str::to_owned),
            snapshot_sha256: value.snapshot_sha256.map(str::to_owned),
            face_index: value.face_index,
            clause_index: value.clause_index,
            row_ordinal: value.row_ordinal,
        }
    }
}

impl RecognizedOracleClauseSyntax {
    pub fn compiler_version(&self) -> &'static str {
        self.compiler_version
    }

    pub fn runtime_version(&self) -> &'static str {
        self.runtime_version
    }

    pub fn semantic_context(&self) -> OracleSyntaxSemanticContext {
        self.semantic_context
    }

    /// Content identity for this syntax artifact. This is never proof that the
    /// line has executable game semantics.
    pub fn syntax_digest(&self) -> &str {
        &self.syntax_digest
    }

    pub fn normalized_line(&self) -> &str {
        &self.normalized_line
    }

    pub fn provenance(&self) -> &OwnedOracleSyntaxProvenance {
        &self.provenance
    }

    pub fn lossless_tokens(&self) -> &[LosslessToken] {
        &self.lossless_tokens
    }

    pub fn structure(&self) -> &OracleClauseStructure {
        &self.structure
    }

    pub fn atoms(&self) -> &[OracleSyntaxAtom] {
        &self.atoms
    }

    pub(crate) fn validated_line(&self) -> ValidatedOracleClauseLine<'_> {
        ValidatedOracleClauseLine {
            line: &self.normalized_line,
        }
    }

    pub fn atom_text(&self, atom: OracleSyntaxAtom) -> &str {
        atom.span
            .slice(&self.normalized_line)
            .expect("recognized atom span is always a UTF-8 boundary")
    }

    /// Rebuild the exact normalized line from the exhaustive token ledger.
    pub fn reconstruct(&self) -> String {
        let mut rebuilt = String::with_capacity(self.normalized_line.len());
        for token in &self.lossless_tokens {
            rebuilt.push_str(
                token
                    .span
                    .slice(&self.normalized_line)
                    .expect("recognized token span is always a UTF-8 boundary"),
            );
        }
        rebuilt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleClauseSyntaxError {
    EmptyLine,
    NonCanonicalOuterWhitespace,
    ContainsLineBreak {
        byte_index: usize,
    },
    ControlCharacter {
        byte_index: usize,
        character: char,
    },
    UnexpectedClosingDelimiter {
        byte_index: usize,
        found: char,
    },
    MismatchedClosingDelimiter {
        byte_index: usize,
        opening_byte_index: usize,
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
        opening_byte_index: usize,
        expected: char,
        found: char,
    },
    UnclosedQuote {
        byte_index: usize,
        opening: char,
        expected: char,
    },
    CrossedNestingBoundary {
        byte_index: usize,
        found: char,
        opening_byte_index: usize,
        expected: char,
    },
    EmptyActivatedCost {
        colon_byte_index: usize,
    },
    EmptyActivatedEffect {
        colon_byte_index: usize,
    },
    InternalCoverageGap {
        byte_index: usize,
    },
}

impl fmt::Display for OracleClauseSyntaxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLine => write!(formatter, "Oracle line is empty after trimming"),
            Self::NonCanonicalOuterWhitespace => {
                write!(formatter, "Oracle line must already be trimmed")
            }
            Self::ContainsLineBreak { byte_index } => write!(
                formatter,
                "complete Oracle line contains a line break at byte {byte_index}"
            ),
            Self::ControlCharacter {
                byte_index,
                character,
            } => write!(
                formatter,
                "Oracle line contains control character U+{:04X} at byte {byte_index}",
                *character as u32
            ),
            Self::UnexpectedClosingDelimiter { byte_index, found } => write!(
                formatter,
                "unexpected closing delimiter '{found}' at byte {byte_index}"
            ),
            Self::MismatchedClosingDelimiter {
                byte_index,
                opening_byte_index,
                expected,
                found,
            } => write!(
                formatter,
                "delimiter opened at byte {opening_byte_index} expects '{expected}', found '{found}' at byte {byte_index}"
            ),
            Self::UnclosedDelimiter {
                byte_index,
                opening,
                expected,
            } => write!(
                formatter,
                "delimiter '{opening}' opened at byte {byte_index} is not closed with '{expected}'"
            ),
            Self::UnexpectedClosingQuote { byte_index, found } => write!(
                formatter,
                "unexpected closing quote '{found}' at byte {byte_index}"
            ),
            Self::MismatchedClosingQuote {
                byte_index,
                opening_byte_index,
                expected,
                found,
            } => write!(
                formatter,
                "quote opened at byte {opening_byte_index} expects '{expected}', found '{found}' at byte {byte_index}"
            ),
            Self::UnclosedQuote {
                byte_index,
                opening,
                expected,
            } => write!(
                formatter,
                "quote '{opening}' opened at byte {byte_index} is not closed with '{expected}'"
            ),
            Self::CrossedNestingBoundary {
                byte_index,
                found,
                opening_byte_index,
                expected,
            } => write!(
                formatter,
                "'{found}' at byte {byte_index} crosses a nesting boundary opened at byte {opening_byte_index}; '{expected}' must close first"
            ),
            Self::EmptyActivatedCost { colon_byte_index } => write!(
                formatter,
                "top-level activation colon at byte {colon_byte_index} has no cost"
            ),
            Self::EmptyActivatedEffect { colon_byte_index } => write!(
                formatter,
                "top-level activation colon at byte {colon_byte_index} has no effect"
            ),
            Self::InternalCoverageGap { byte_index } => write!(
                formatter,
                "lossless tokenization left an internal coverage gap at byte {byte_index}"
            ),
        }
    }
}

impl std::error::Error for OracleClauseSyntaxError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteStyle {
    AsciiDouble,
    CurlyDouble,
}

impl QuoteStyle {
    const fn opening(self) -> char {
        match self {
            Self::AsciiDouble => '"',
            Self::CurlyDouble => '“',
        }
    }

    const fn closing(self) -> char {
        match self {
            Self::AsciiDouble => '"',
            Self::CurlyDouble => '”',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DelimiterFrame {
    opening: char,
    opening_byte_index: usize,
    expected: char,
    opened_at_top_level: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QuoteFrame {
    style: QuoteStyle,
    opening_byte_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NestingFrame {
    Delimiter(DelimiterFrame),
    Quote(QuoteFrame),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsciiQuoteRole {
    Opening,
    Closing,
    Literal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DelimiterPair {
    opening: char,
    opening_span: ByteSpan,
    closing_span: ByteSpan,
    opened_at_top_level: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QuotePair {
    opening_span: ByteSpan,
    closing_span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScanAnalysis {
    top_level_at: Vec<bool>,
    delimiter_pairs: Vec<DelimiterPair>,
    quote_pairs: Vec<QuotePair>,
    literal_quote_glyphs: Vec<ByteSpan>,
}

impl ScanAnalysis {
    fn is_top_level(&self, byte_index: usize) -> bool {
        self.top_level_at.get(byte_index).copied().unwrap_or(false)
    }
}

pub fn recognize_oracle_clause_syntax(
    input: OracleClauseSyntaxInput<'_>,
) -> Result<RecognizedOracleClauseSyntax, OracleClauseSyntaxError> {
    validate_canonical_line(input.normalized_line)?;
    let normalized_line = input.normalized_line.to_owned();
    validate_complete_line(&normalized_line)?;
    let scan = scan_balanced_structure(&normalized_line)?;
    let lossless_tokens = tokenize_losslessly(&normalized_line, &scan)?;
    let structure = recognize_structure(&normalized_line, &scan)?;
    let atoms = recognize_atoms(&normalized_line, &scan, &structure);
    let syntax_digest = syntax_digest(
        &normalized_line,
        input.semantic_context,
        ORACLE_CLAUSE_SYNTAX_COMPILER_VERSION,
        ORACLE_CLAUSE_SYNTAX_RUNTIME_VERSION,
    );

    Ok(RecognizedOracleClauseSyntax {
        compiler_version: ORACLE_CLAUSE_SYNTAX_COMPILER_VERSION,
        runtime_version: ORACLE_CLAUSE_SYNTAX_RUNTIME_VERSION,
        semantic_context: input.semantic_context,
        syntax_digest,
        normalized_line,
        provenance: input.provenance.into(),
        lossless_tokens,
        structure,
        atoms,
    })
}

/// Validate one canonical Oracle line without building a token ledger, syntax
/// tree, atom list, or content digest. This is a syntax gate only and never
/// authorizes executable behavior.
pub(crate) fn validate_oracle_clause_line(
    line: &str,
) -> Result<ValidatedOracleClauseLine<'_>, OracleClauseSyntaxError> {
    validate_canonical_line(line)?;
    validate_complete_line(line)?;
    scan_balanced_structure(line)?;
    Ok(ValidatedOracleClauseLine { line })
}

fn validate_canonical_line(line: &str) -> Result<(), OracleClauseSyntaxError> {
    if line.trim().is_empty() {
        return Err(OracleClauseSyntaxError::EmptyLine);
    }
    if line != line.trim() {
        return Err(OracleClauseSyntaxError::NonCanonicalOuterWhitespace);
    }
    Ok(())
}

fn validate_complete_line(line: &str) -> Result<(), OracleClauseSyntaxError> {
    for (byte_index, character) in line.char_indices() {
        if character == '\n' || character == '\r' {
            return Err(OracleClauseSyntaxError::ContainsLineBreak { byte_index });
        }
        if character.is_control() && character != '\t' {
            return Err(OracleClauseSyntaxError::ControlCharacter {
                byte_index,
                character,
            });
        }
    }
    Ok(())
}

fn scan_balanced_structure(line: &str) -> Result<ScanAnalysis, OracleClauseSyntaxError> {
    let mut top_level_at = vec![false; line.len() + 1];
    let mut nesting_stack = Vec::<NestingFrame>::new();
    let mut delimiter_pairs = Vec::new();
    let mut quote_pairs = Vec::new();
    let mut literal_quote_glyphs = Vec::new();
    let ascii_quote_roles = classify_ascii_quote_roles(line);

    for (byte_index, character) in line.char_indices() {
        top_level_at[byte_index] = nesting_stack.is_empty();
        let character_end = byte_index + character.len_utf8();
        match character {
            '"' => match ascii_quote_role(&ascii_quote_roles, byte_index) {
                AsciiQuoteRole::Opening => nesting_stack.push(NestingFrame::Quote(QuoteFrame {
                    style: QuoteStyle::AsciiDouble,
                    opening_byte_index: byte_index,
                })),
                AsciiQuoteRole::Closing => match nesting_stack.last().copied() {
                    Some(NestingFrame::Quote(frame)) if frame.style == QuoteStyle::AsciiDouble => {
                        nesting_stack.pop();
                        quote_pairs.push(QuotePair {
                            opening_span: ByteSpan::new(
                                frame.opening_byte_index,
                                frame.opening_byte_index + 1,
                            ),
                            closing_span: ByteSpan::new(byte_index, character_end),
                        });
                    }
                    Some(NestingFrame::Quote(frame)) => {
                        return Err(OracleClauseSyntaxError::MismatchedClosingQuote {
                            byte_index,
                            opening_byte_index: frame.opening_byte_index,
                            expected: frame.style.closing(),
                            found: character,
                        });
                    }
                    Some(NestingFrame::Delimiter(frame)) => {
                        return Err(OracleClauseSyntaxError::CrossedNestingBoundary {
                            byte_index,
                            found: character,
                            opening_byte_index: frame.opening_byte_index,
                            expected: frame.expected,
                        });
                    }
                    None => {
                        return Err(OracleClauseSyntaxError::UnexpectedClosingQuote {
                            byte_index,
                            found: character,
                        });
                    }
                },
                AsciiQuoteRole::Literal => {
                    literal_quote_glyphs.push(ByteSpan::new(byte_index, character_end));
                }
            },
            '“' => nesting_stack.push(NestingFrame::Quote(QuoteFrame {
                style: QuoteStyle::CurlyDouble,
                opening_byte_index: byte_index,
            })),
            '”' => match nesting_stack.last().copied() {
                Some(NestingFrame::Quote(frame)) if frame.style == QuoteStyle::CurlyDouble => {
                    nesting_stack.pop();
                    quote_pairs.push(QuotePair {
                        opening_span: ByteSpan::new(
                            frame.opening_byte_index,
                            frame.opening_byte_index + frame.style.opening().len_utf8(),
                        ),
                        closing_span: ByteSpan::new(byte_index, character_end),
                    });
                }
                Some(NestingFrame::Quote(frame)) => {
                    return Err(OracleClauseSyntaxError::MismatchedClosingQuote {
                        byte_index,
                        opening_byte_index: frame.opening_byte_index,
                        expected: frame.style.closing(),
                        found: character,
                    });
                }
                Some(NestingFrame::Delimiter(frame)) => {
                    return Err(OracleClauseSyntaxError::CrossedNestingBoundary {
                        byte_index,
                        found: character,
                        opening_byte_index: frame.opening_byte_index,
                        expected: frame.expected,
                    });
                }
                None => {
                    return Err(OracleClauseSyntaxError::UnexpectedClosingQuote {
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
                let opened_at_top_level = nesting_stack.is_empty();
                nesting_stack.push(NestingFrame::Delimiter(DelimiterFrame {
                    opening: character,
                    opening_byte_index: byte_index,
                    expected,
                    opened_at_top_level,
                }));
            }
            ')' | ']' | '}' => match nesting_stack.last().copied() {
                Some(NestingFrame::Delimiter(frame)) if frame.expected == character => {
                    nesting_stack.pop();
                    delimiter_pairs.push(DelimiterPair {
                        opening: frame.opening,
                        opening_span: ByteSpan::new(
                            frame.opening_byte_index,
                            frame.opening_byte_index + frame.opening.len_utf8(),
                        ),
                        closing_span: ByteSpan::new(byte_index, character_end),
                        opened_at_top_level: frame.opened_at_top_level,
                    });
                }
                Some(NestingFrame::Delimiter(frame)) => {
                    return Err(OracleClauseSyntaxError::MismatchedClosingDelimiter {
                        byte_index,
                        opening_byte_index: frame.opening_byte_index,
                        expected: frame.expected,
                        found: character,
                    });
                }
                Some(NestingFrame::Quote(frame)) => {
                    return Err(OracleClauseSyntaxError::CrossedNestingBoundary {
                        byte_index,
                        found: character,
                        opening_byte_index: frame.opening_byte_index,
                        expected: frame.style.closing(),
                    });
                }
                None => {
                    return Err(OracleClauseSyntaxError::UnexpectedClosingDelimiter {
                        byte_index,
                        found: character,
                    });
                }
            },
            _ => {}
        }
    }

    if let Some(frame) = nesting_stack.last().copied() {
        return match frame {
            NestingFrame::Delimiter(frame) => Err(OracleClauseSyntaxError::UnclosedDelimiter {
                byte_index: frame.opening_byte_index,
                opening: frame.opening,
                expected: frame.expected,
            }),
            NestingFrame::Quote(frame) => Err(OracleClauseSyntaxError::UnclosedQuote {
                byte_index: frame.opening_byte_index,
                opening: frame.style.opening(),
                expected: frame.style.closing(),
            }),
        };
    }
    top_level_at[line.len()] = true;
    delimiter_pairs.sort_by_key(|pair| pair.opening_span.start);
    quote_pairs.sort_by_key(|pair| pair.opening_span.start);
    Ok(ScanAnalysis {
        top_level_at,
        delimiter_pairs,
        quote_pairs,
        literal_quote_glyphs,
    })
}

fn classify_ascii_quote_roles(line: &str) -> Vec<(usize, AsciiQuoteRole)> {
    let positions = line
        .char_indices()
        .filter_map(|(byte_index, character)| (character == '"').then_some(byte_index))
        .collect::<Vec<_>>();
    let mut roles: Vec<(usize, AsciiQuoteRole)> = positions
        .iter()
        .copied()
        .map(|byte_index| (byte_index, AsciiQuoteRole::Literal))
        .collect::<Vec<_>>();
    let mut opening: Option<(usize, usize)> = None;

    for (role_index, byte_index) in positions.iter().copied().enumerate() {
        if let Some((opening_role_index, _)) = opening {
            if ascii_quote_can_close(line, byte_index) {
                roles[opening_role_index].1 = AsciiQuoteRole::Opening;
                roles[role_index].1 = AsciiQuoteRole::Closing;
                opening = None;
            }
        } else if ascii_quote_can_open(line, byte_index) {
            opening = Some((role_index, byte_index));
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

fn ascii_quote_can_open(line: &str, byte_index: usize) -> bool {
    let previous = line[..byte_index].chars().next_back();
    let next = line[byte_index + 1..].chars().next();
    next.is_some_and(|character| !character.is_whitespace())
        && previous.is_none_or(|character| {
            character.is_whitespace()
                || matches!(
                    character,
                    '(' | '[' | '{' | '=' | ',' | ':' | ';' | '\u{2014}' | '\u{2013}' | '-'
                )
        })
}

fn ascii_quote_can_close(line: &str, byte_index: usize) -> bool {
    let previous = line[..byte_index].chars().next_back();
    let next = line[byte_index + 1..].chars().next();
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

fn tokenize_losslessly(
    line: &str,
    scan: &ScanAnalysis,
) -> Result<Vec<LosslessToken>, OracleClauseSyntaxError> {
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < line.len() {
        let character = line[cursor..]
            .chars()
            .next()
            .expect("cursor is inside string");
        let character_end = cursor + character.len_utf8();
        let (kind, end) = if character.is_whitespace() {
            let mut end = character_end;
            while end < line.len() {
                let next = line[end..]
                    .chars()
                    .next()
                    .expect("valid character boundary");
                if !next.is_whitespace() {
                    break;
                }
                end += next.len_utf8();
            }
            (LosslessTokenKind::Whitespace, end)
        } else if character == '{' {
            let close = line[character_end..]
                .find('}')
                .map(|offset| character_end + offset + 1)
                .unwrap_or(character_end);
            (LosslessTokenKind::ManaSymbol, close)
        } else if character.is_alphabetic() || character == '_' {
            let mut end = character_end;
            while end < line.len() {
                let next = line[end..]
                    .chars()
                    .next()
                    .expect("valid character boundary");
                if !(next.is_alphanumeric() || matches!(next, '_' | '\'' | '’' | '-' | '/' | '‑'))
                {
                    break;
                }
                end += next.len_utf8();
            }
            (LosslessTokenKind::Word, end)
        } else if character.is_ascii_digit() {
            let mut end = character_end;
            while end < line.len() {
                let next = line[end..]
                    .chars()
                    .next()
                    .expect("valid character boundary");
                if !(next.is_ascii_digit() || matches!(next, '.' | '/' | '−')) {
                    break;
                }
                end += next.len_utf8();
            }
            (LosslessTokenKind::Number, end)
        } else {
            let kind = match character {
                '•' => LosslessTokenKind::Bullet,
                '"' if scan
                    .literal_quote_glyphs
                    .iter()
                    .any(|span| span.start == cursor) =>
                {
                    LosslessTokenKind::LiteralQuoteGlyph
                }
                '"' | '“' | '”' => LosslessTokenKind::QuoteDelimiter,
                '(' | '[' => LosslessTokenKind::OpenDelimiter,
                ')' | ']' | '}' => LosslessTokenKind::CloseDelimiter,
                ',' | '.' | ';' | ':' | '!' | '?' => LosslessTokenKind::Punctuation,
                _ => LosslessTokenKind::Symbol,
            };
            (kind, character_end)
        };
        tokens.push(LosslessToken {
            kind,
            span: ByteSpan::new(cursor, end),
        });
        cursor = end;
    }

    let mut expected = 0;
    for token in &tokens {
        if token.span.start != expected {
            return Err(OracleClauseSyntaxError::InternalCoverageGap {
                byte_index: expected,
            });
        }
        expected = token.span.end;
    }
    if expected != line.len() {
        return Err(OracleClauseSyntaxError::InternalCoverageGap {
            byte_index: expected,
        });
    }
    Ok(tokens)
}

fn recognize_structure(
    line: &str,
    scan: &ScanAnalysis,
) -> Result<OracleClauseStructure, OracleClauseSyntaxError> {
    let full_span = ByteSpan::new(0, line.len());
    let trailing_reminder = trailing_reminder(line, scan);
    let structural_end = trailing_reminder
        .map(|reminder| reminder.reminder_span.start)
        .unwrap_or(line.len());
    let mut cursor = skip_whitespace_forward(line, 0, structural_end);
    let mut modal = ModalSyntax::default();
    let mut ability_word = None;

    if line[cursor..structural_end].starts_with('•') {
        let marker_end = cursor + '•'.len_utf8();
        modal.branch = Some(ModalBranchSyntax {
            marker_span: ByteSpan::new(cursor, marker_end),
            label_span: None,
            separator_span: None,
        });
        cursor = skip_whitespace_forward(line, marker_end, structural_end);
        if let Some(dash) = find_top_level_char(line, scan, cursor, structural_end, '\u{2014}') {
            let candidate = trim_span(line, ByteSpan::new(cursor, dash));
            if !candidate.is_empty() && candidate.len() <= 96 {
                let separator_span = ByteSpan::new(dash, dash + '\u{2014}'.len_utf8());
                modal.branch = Some(ModalBranchSyntax {
                    marker_span: modal.branch.expect("installed branch").marker_span,
                    label_span: Some(candidate),
                    separator_span: Some(separator_span),
                });
                cursor = skip_whitespace_forward(line, separator_span.end, structural_end);
            }
        }
    } else if let Some(dash) = find_top_level_char(line, scan, cursor, structural_end, '\u{2014}') {
        let prefix = trim_span(line, ByteSpan::new(cursor, dash));
        let prefix_text = prefix.slice(line).unwrap_or_default();
        let separator_span = ByteSpan::new(dash, dash + '\u{2014}'.len_utf8());
        if is_modal_header(prefix_text) {
            modal.header = Some(ModalHeaderSyntax {
                header_span: ByteSpan::new(prefix.start, separator_span.end),
                separator_span: Some(separator_span),
            });
            cursor = skip_whitespace_forward(line, separator_span.end, structural_end);
        } else if is_chapter_marker(prefix_text) {
            modal.branch = Some(ModalBranchSyntax {
                marker_span: prefix,
                label_span: Some(prefix),
                separator_span: Some(separator_span),
            });
            cursor = skip_whitespace_forward(line, separator_span.end, structural_end);
        } else if !prefix.is_empty() && prefix.len() <= 128 {
            ability_word = Some(AbilityWordSyntax {
                label_span: prefix,
                separator_span,
            });
            cursor = skip_whitespace_forward(line, separator_span.end, structural_end);
        }
    } else if let Some(header_end) = modal_header_sentence_end(line, scan, cursor, structural_end) {
        modal.header = Some(ModalHeaderSyntax {
            header_span: trim_span(line, ByteSpan::new(cursor, header_end)),
            separator_span: None,
        });
        cursor = skip_whitespace_forward(line, header_end, structural_end);
    }

    let activation_colon = find_top_level_char(line, scan, cursor, structural_end, ':');
    let activation = if let Some(colon) = activation_colon.filter(|colon| {
        *colon == cursor || looks_like_activation_cost(line, ByteSpan::new(cursor, *colon))
    }) {
        let cost_span = trim_span(line, ByteSpan::new(cursor, colon));
        let separator_span = ByteSpan::new(colon, colon + 1);
        let effect_span = trim_span(line, ByteSpan::new(separator_span.end, structural_end));
        if cost_span.is_empty() {
            return Err(OracleClauseSyntaxError::EmptyActivatedCost {
                colon_byte_index: colon,
            });
        }
        Some(ActivatedAbilitySyntax {
            cost_span,
            separator_span,
            effect_span: (!effect_span.is_empty()).then_some(effect_span),
            continues_on_following_line: effect_span.is_empty(),
        })
    } else {
        None
    };

    let instruction_start = activation
        .map(|activated| {
            activated
                .effect_span
                .map(|effect| effect.start)
                .unwrap_or(structural_end)
        })
        .unwrap_or(cursor);
    let lead_in = recognize_lead_in(line, scan, instruction_start, structural_end);
    let sentence_start = lead_in
        .map(|lead| skip_whitespace_forward(line, lead.separator_span.end, structural_end))
        .unwrap_or(instruction_start);
    let instruction_sentences =
        recognize_instruction_sentences(line, scan, sentence_start, structural_end);
    let top_level_conjunctions =
        recognize_top_level_conjunctions(line, scan, cursor, structural_end);
    let quoted_abilities = recognize_quoted_abilities(line, scan);

    Ok(OracleClauseStructure {
        full_span,
        ability_word,
        modal: (modal != ModalSyntax::default()).then_some(modal),
        lead_in,
        activation,
        instruction_sentences,
        top_level_conjunctions,
        quoted_abilities,
        literal_quote_glyphs: scan.literal_quote_glyphs.clone(),
        trailing_reminder,
    })
}

fn trailing_reminder(line: &str, scan: &ScanAnalysis) -> Option<TrailingReminderSyntax> {
    let end = trim_span(line, ByteSpan::new(0, line.len())).end;
    scan.delimiter_pairs
        .iter()
        .filter(|pair| {
            pair.opening == '('
                && pair.opened_at_top_level
                && pair.closing_span.end == end
                && pair.opening_span.start > 0
                && line[..pair.opening_span.start]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace)
        })
        .max_by_key(|pair| pair.opening_span.start)
        .map(|pair| TrailingReminderSyntax {
            reminder_span: ByteSpan::new(pair.opening_span.start, pair.closing_span.end),
            content_span: ByteSpan::new(pair.opening_span.end, pair.closing_span.start),
        })
}

fn is_modal_header(text: &str) -> bool {
    let lower = text.trim().to_ascii_lowercase();
    starts_modal_phrase(&lower)
}

fn starts_modal_phrase(lower: &str) -> bool {
    lower.starts_with("choose one")
        || lower.starts_with("choose two")
        || lower.starts_with("choose three")
        || lower.starts_with("choose four")
        || lower.starts_with("choose a mode")
        || lower.starts_with("choose any number")
}

fn modal_header_sentence_end(
    line: &str,
    scan: &ScanAnalysis,
    start: usize,
    end: usize,
) -> Option<usize> {
    let lower = line[start..end].to_ascii_lowercase();
    if !starts_modal_phrase(&lower) {
        return None;
    }
    find_top_level_char(line, scan, start, end, '.').map(|period| period + 1)
}

fn is_chapter_marker(text: &str) -> bool {
    let mut saw_marker = false;
    for part in text.split(',') {
        let marker = part.trim();
        if marker.is_empty()
            || !marker
                .chars()
                .all(|character| matches!(character, 'I' | 'V' | 'X'))
        {
            return false;
        }
        saw_marker = true;
    }
    saw_marker
}

fn looks_like_activation_cost(line: &str, span: ByteSpan) -> bool {
    let cost = span
        .slice(line)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if cost.is_empty() {
        return true;
    }
    if cost.starts_with('{')
        || cost.starts_with('+')
        || cost.starts_with('-')
        || cost.starts_with('−')
        || cost
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    {
        return true;
    }
    const COST_LEADS: &[&str] = &[
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
    ];
    COST_LEADS.iter().any(|lead| cost.starts_with(lead))
}

fn recognize_lead_in(
    line: &str,
    scan: &ScanAnalysis,
    start: usize,
    end: usize,
) -> Option<LeadInSyntax> {
    let comma = find_top_level_char(line, scan, start, end, ',')?;
    let prefix_span = trim_span(line, ByteSpan::new(start, comma));
    if prefix_span.is_empty() {
        return None;
    }
    let prefix = prefix_span.slice(line)?.to_ascii_lowercase();
    let remainder = line[comma + 1..end].trim().to_ascii_lowercase();
    let kind = if ((prefix.contains(" would ") || prefix.ends_with(" would"))
        && (remainder.starts_with("instead") || remainder.contains(" instead")))
        || (prefix.starts_with("as ")
            && (prefix.contains(" enters")
                || prefix.contains(" is turned face up")
                || prefix.contains(" is cast")))
    {
        LeadInKind::Replacement
    } else if prefix.starts_with("when ")
        || prefix.starts_with("whenever ")
        || prefix.starts_with("at the beginning ")
        || prefix.starts_with("at the end ")
        || prefix.starts_with("the first time ")
        || prefix.starts_with("the next time ")
    {
        LeadInKind::Trigger
    } else if prefix.starts_with("if ")
        || prefix.starts_with("unless ")
        || prefix.starts_with("as long as ")
        || prefix.starts_with("while ")
        || prefix.starts_with("provided ")
        || prefix.starts_with("rather than ")
    {
        LeadInKind::Condition
    } else if prefix.starts_with("during ")
        || prefix.starts_with("before ")
        || prefix.starts_with("after ")
        || prefix.starts_with("until ")
        || prefix.starts_with("once ")
        || prefix.starts_with("only during ")
    {
        LeadInKind::Timing
    } else {
        return None;
    };
    Some(LeadInSyntax {
        kind,
        prefix_span,
        separator_span: ByteSpan::new(comma, comma + 1),
    })
}

fn recognize_instruction_sentences(
    line: &str,
    scan: &ScanAnalysis,
    start: usize,
    end: usize,
) -> Vec<InstructionSentenceSyntax> {
    let mut sentences = Vec::new();
    let mut sentence_start = skip_whitespace_forward(line, start, end);
    for (relative, character) in line[sentence_start..end].char_indices() {
        let byte_index = sentence_start + relative;
        if matches!(character, '.' | '!' | '?') && scan.is_top_level(byte_index) {
            let sentence_end = byte_index + character.len_utf8();
            let span = trim_span(line, ByteSpan::new(sentence_start, sentence_end));
            if !span.is_empty() {
                sentences.push(InstructionSentenceSyntax { span });
            }
            sentence_start = skip_whitespace_forward(line, sentence_end, end);
        }
    }
    let trailing = trim_span(line, ByteSpan::new(sentence_start, end));
    if !trailing.is_empty() {
        sentences.push(InstructionSentenceSyntax { span: trailing });
    }
    sentences
}

fn recognize_top_level_conjunctions(
    line: &str,
    scan: &ScanAnalysis,
    start: usize,
    end: usize,
) -> Vec<TopLevelConjunctionSyntax> {
    const CONJUNCTIONS: &[(&str, TopLevelConjunctionKind)] = &[
        ("and/or", TopLevelConjunctionKind::AndOr),
        ("then", TopLevelConjunctionKind::Then),
        ("and", TopLevelConjunctionKind::And),
        ("but", TopLevelConjunctionKind::But),
        ("or", TopLevelConjunctionKind::Or),
    ];
    let mut conjunctions = Vec::new();
    let mut cursor = start;
    while cursor < end {
        if !line.is_char_boundary(cursor) {
            cursor += 1;
            continue;
        }
        let mut matched = None;
        if scan.is_top_level(cursor) {
            for (word, kind) in CONJUNCTIONS {
                let word_end = cursor + word.len();
                if word_end <= end
                    && line.is_char_boundary(word_end)
                    && line[cursor..word_end].eq_ignore_ascii_case(word)
                    && is_word_boundary(line, cursor, word_end)
                {
                    matched = Some((*kind, word_end));
                    break;
                }
            }
        }
        if let Some((kind, word_end)) = matched {
            conjunctions.push(TopLevelConjunctionSyntax {
                kind,
                span: ByteSpan::new(cursor, word_end),
            });
            cursor = word_end;
        } else {
            cursor += line[cursor..]
                .chars()
                .next()
                .expect("cursor is inside string")
                .len_utf8();
        }
    }
    conjunctions
}

fn recognize_quoted_abilities(line: &str, scan: &ScanAnalysis) -> Vec<QuotedAbilitySyntax> {
    scan.quote_pairs
        .iter()
        .map(|pair| {
            let quote_span = ByteSpan::new(pair.opening_span.start, pair.closing_span.end);
            let content_span = ByteSpan::new(pair.opening_span.end, pair.closing_span.start);
            let prefix_start = line[..pair.opening_span.start]
                .char_indices()
                .rev()
                .nth(96)
                .map(|(index, _)| index)
                .unwrap_or(0);
            let prefix = line[prefix_start..pair.opening_span.start].to_ascii_lowercase();
            let is_granted_ability = [
                " gains ",
                " gain ",
                " has ",
                " have ",
                " with ",
                " ability ",
                " abilities ",
            ]
            .iter()
            .any(|needle| prefix.contains(needle))
                || prefix.trim_end().ends_with("gains")
                || prefix.trim_end().ends_with("has");
            QuotedAbilitySyntax {
                quote_span,
                content_span,
                is_granted_ability,
            }
        })
        .collect()
}

fn recognize_atoms(
    line: &str,
    scan: &ScanAnalysis,
    structure: &OracleClauseStructure,
) -> Vec<OracleSyntaxAtom> {
    let mut separators = Vec::<ByteSpan>::new();
    for (byte_index, character) in line.char_indices() {
        if scan.is_top_level(byte_index)
            && matches!(
                character,
                '•' | '\u{2014}' | ',' | ';' | ':' | '.' | '!' | '?'
            )
        {
            separators.push(ByteSpan::new(byte_index, byte_index + character.len_utf8()));
        }
    }
    separators.extend(
        structure
            .top_level_conjunctions
            .iter()
            .map(|conjunction| conjunction.span),
    );
    for quoted in &structure.quoted_abilities {
        let opening_end = quoted.content_span.start;
        separators.push(ByteSpan::new(quoted.quote_span.start, opening_end));
        separators.push(ByteSpan::new(
            quoted.content_span.end,
            quoted.quote_span.end,
        ));
    }
    if let Some(reminder) = structure.trailing_reminder {
        separators.push(ByteSpan::new(
            reminder.reminder_span.start,
            reminder.content_span.start,
        ));
        separators.push(ByteSpan::new(
            reminder.content_span.end,
            reminder.reminder_span.end,
        ));
    }
    separators.sort();
    separators.dedup();

    let mut non_overlapping = Vec::<ByteSpan>::new();
    for separator in separators {
        if separator.is_empty() {
            continue;
        }
        if let Some(previous) = non_overlapping.last()
            && separator.start < previous.end
        {
            continue;
        }
        non_overlapping.push(separator);
    }

    let mut atoms = Vec::new();
    let mut cursor = 0;
    for separator in non_overlapping {
        push_atom(line, ByteSpan::new(cursor, separator.start), &mut atoms);
        cursor = separator.end;
    }
    push_atom(line, ByteSpan::new(cursor, line.len()), &mut atoms);
    atoms
}

fn push_atom(line: &str, span: ByteSpan, atoms: &mut Vec<OracleSyntaxAtom>) {
    let span = trim_span(line, span);
    if span.is_empty() {
        return;
    }
    let text = span.slice(line).expect("atom span is valid");
    atoms.push(OracleSyntaxAtom {
        kind: classify_atom(text),
        span,
    });
}

fn classify_atom(text: &str) -> OracleAtomKind {
    let normalized = text
        .trim_matches(|character: char| {
            character.is_whitespace() || matches!(character, '(' | ')' | '[' | ']')
        })
        .to_ascii_lowercase();
    if normalized.is_empty() {
        return OracleAtomKind::Unresolved;
    }
    if starts_with_known_action(&normalized) {
        return OracleAtomKind::KnownAction;
    }
    if starts_with_known_keyword(&normalized) {
        return OracleAtomKind::KnownKeyword;
    }
    if is_restriction_or_characteristic(&normalized) {
        return OracleAtomKind::RestrictionOrCharacteristic;
    }
    if is_reference_or_amount(&normalized) {
        return OracleAtomKind::ReferenceOrAmount;
    }
    OracleAtomKind::Unresolved
}

fn starts_with_known_action(text: &str) -> bool {
    const ACTIONS: &[&str] = &[
        "activate ",
        "add ",
        "amass ",
        "attach ",
        "become ",
        "bid ",
        "bolster ",
        "cast ",
        "choose ",
        "cloak ",
        "collect evidence ",
        "connive",
        "counter ",
        "create ",
        "deal ",
        "destroy ",
        "detain ",
        "discard ",
        "discover ",
        "distribute ",
        "draw ",
        "exchange ",
        "exile ",
        "explore",
        "fight ",
        "goad ",
        "investigate",
        "learn",
        "look at ",
        "lose ",
        "manifest ",
        "mill ",
        "move ",
        "open an attraction",
        "pay ",
        "planeswalk",
        "play ",
        "populate",
        "proliferate",
        "put ",
        "regenerate ",
        "remove ",
        "reveal ",
        "return ",
        "roll ",
        "sacrifice ",
        "scry ",
        "search ",
        "seek ",
        "shuffle",
        "suspect ",
        "surveil ",
        "switch ",
        "tap ",
        "transform ",
        "untap ",
        "venture into the dungeon",
        "vote ",
    ];
    ACTIONS
        .iter()
        .any(|action| text == action.trim() || text.starts_with(action))
}

fn starts_with_known_keyword(text: &str) -> bool {
    // This list recognizes printed keyword syntax only. It is not an
    // executable keyword registry.
    const KEYWORDS: &[&str] = &[
        "absorb",
        "affinity",
        "afflict",
        "afterlife",
        "aftermath",
        "amplify",
        "annihilator",
        "ascend",
        "assist",
        "aura swap",
        "awaken",
        "backup",
        "banding",
        "bargain",
        "battle cry",
        "bestow",
        "blitz",
        "bloodthirst",
        "boast",
        "buyback",
        "cascade",
        "casualty",
        "champion",
        "changeling",
        "cipher",
        "cleave",
        "companion",
        "compleated",
        "conspire",
        "convoke",
        "craft",
        "crew",
        "cumulative upkeep",
        "cycling",
        "dash",
        "daybound",
        "deathtouch",
        "decayed",
        "defender",
        "delve",
        "demonstrate",
        "devoid",
        "devour",
        "disguise",
        "disturb",
        "double strike",
        "dredge",
        "echo",
        "embalm",
        "emerge",
        "enchant",
        "encore",
        "enlist",
        "entwine",
        "epic",
        "equip",
        "escalate",
        "escape",
        "eternalize",
        "evoke",
        "evolve",
        "exalted",
        "exploit",
        "extort",
        "fabricate",
        "fading",
        "fear",
        "first strike",
        "flanking",
        "flash",
        "flashback",
        "flying",
        "for mirrodin!",
        "forecast",
        "forestcycling",
        "fortify",
        "frenzy",
        "friends forever",
        "fuse",
        "gift",
        "graft",
        "gravestorm",
        "harmonize",
        "haste",
        "haunt",
        "hexproof",
        "hidden agenda",
        "hideaway",
        "horsemanship",
        "improvise",
        "indestructible",
        "infect",
        "ingest",
        "intimidate",
        "jump-start",
        "kicker",
        "landcycling",
        "landwalk",
        "level up",
        "lifelink",
        "living weapon",
        "madness",
        "megamorph",
        "melee",
        "menace",
        "mentor",
        "miracle",
        "modular",
        "more than meets the eye",
        "morph",
        "mountaincycling",
        "mutate",
        "myriad",
        "nightbound",
        "ninjutsu",
        "offering",
        "outlast",
        "overload",
        "partner",
        "persist",
        "phasing",
        "plainscycling",
        "poisonous",
        "protection",
        "prototype",
        "provoke",
        "prowess",
        "rampage",
        "reach",
        "read ahead",
        "rebound",
        "recover",
        "reinforce",
        "renown",
        "replicate",
        "retrace",
        "riot",
        "ripple",
        "saddle",
        "scavenge",
        "shadow",
        "shroud",
        "skulk",
        "soulbond",
        "soulshift",
        "spectacle",
        "splice",
        "split second",
        "storm",
        "sunburst",
        "surge",
        "suspend",
        "swampcycling",
        "toxic",
        "training",
        "trample",
        "transfigure",
        "transmute",
        "tribute",
        "typecycling",
        "undaunted",
        "undying",
        "unearth",
        "unleash",
        "vanishing",
        "vigilance",
        "ward",
        "web-slinging",
        "withhold",
        "wizardcycling",
    ];
    KEYWORDS.iter().any(|keyword| {
        text == *keyword
            || text
                .strip_prefix(keyword)
                .is_some_and(|suffix| suffix.starts_with(char::is_whitespace))
    })
}

fn is_restriction_or_characteristic(text: &str) -> bool {
    const LEADS: &[&str] = &[
        "as long as ",
        "can't ",
        "cannot ",
        "can ",
        "costs ",
        "doesn't ",
        "don't ",
        "each ",
        "has ",
        "have ",
        "is ",
        "isn't ",
        "must ",
        "only ",
        "spells ",
        "this ",
        "those ",
        "you can't ",
        "you may ",
    ];
    const MARKERS: &[&str] = &[
        " can attack",
        " can block",
        " can't attack",
        " can't block",
        " has base power",
        " has no color",
        " is all colors",
        " is colorless",
        " is every creature type",
        " loses all abilities",
        " maximum hand size",
        " only as a sorcery",
        " only once each turn",
        " power and toughness",
        " rather than",
        " spend this mana only",
        " type in addition",
    ];
    LEADS.iter().any(|lead| text.starts_with(lead))
        || MARKERS.iter().any(|marker| text.contains(marker))
}

fn is_reference_or_amount(text: &str) -> bool {
    const LEADS: &[&str] = &[
        "a card",
        "a creature",
        "a permanent",
        "an opponent",
        "another ",
        "any number",
        "each ",
        "equal to ",
        "for each ",
        "it ",
        "its ",
        "one ",
        "that ",
        "the chosen ",
        "the number ",
        "the rest ",
        "them ",
        "then ",
        "these ",
        "this card",
        "this creature",
        "those ",
        "three ",
        "two ",
        "up to ",
        "where x ",
        "x is ",
        "you ",
        "your ",
    ];
    LEADS.iter().any(|lead| text.starts_with(lead))
        || text == "x"
        || text.chars().all(|character| {
            character.is_ascii_digit()
                || character.is_whitespace()
                || matches!(character, '+' | '-' | '−' | '/' | '*' | 'x')
        })
}

fn find_top_level_char(
    line: &str,
    scan: &ScanAnalysis,
    start: usize,
    end: usize,
    needle: char,
) -> Option<usize> {
    line[start..end]
        .char_indices()
        .map(|(relative, character)| (start + relative, character))
        .find_map(|(byte_index, character)| {
            (character == needle && scan.is_top_level(byte_index)).then_some(byte_index)
        })
}

fn trim_span(line: &str, span: ByteSpan) -> ByteSpan {
    let mut start = span.start.min(line.len());
    let mut end = span.end.min(line.len());
    while start < end {
        let character = line[start..end]
            .chars()
            .next()
            .expect("nonempty valid string slice");
        if !character.is_whitespace() {
            break;
        }
        start += character.len_utf8();
    }
    while start < end {
        let character = line[start..end]
            .chars()
            .next_back()
            .expect("nonempty valid string slice");
        if !character.is_whitespace() {
            break;
        }
        end -= character.len_utf8();
    }
    ByteSpan::new(start, end)
}

fn skip_whitespace_forward(line: &str, mut cursor: usize, end: usize) -> usize {
    while cursor < end {
        let character = line[cursor..end]
            .chars()
            .next()
            .expect("nonempty valid string slice");
        if !character.is_whitespace() {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
}

fn is_word_boundary(line: &str, start: usize, end: usize) -> bool {
    let before = line[..start].chars().next_back();
    let after = line[end..].chars().next();
    before.is_none_or(|character| !is_word_character(character))
        && after.is_none_or(|character| !is_word_character(character))
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '\'' | '’' | '-')
}

fn syntax_digest(
    normalized_line: &str,
    semantic_context: OracleSyntaxSemanticContext,
    compiler_version: &str,
    runtime_version: &str,
) -> String {
    let mut hasher = Sha256::new();
    update_digest_field(&mut hasher, "schema", "oracle-clause-syntax-content/v1");
    update_digest_field(&mut hasher, "compiler", compiler_version);
    update_digest_field(&mut hasher, "runtime", runtime_version);
    update_digest_field(&mut hasher, "context", semantic_context.stable_id());
    update_digest_field(&mut hasher, "normalized-line", normalized_line);
    format!("{:x}", hasher.finalize())
}

fn update_digest_field(hasher: &mut Sha256, label: &str, value: &str) {
    hasher.update((label.len() as u64).to_be_bytes());
    hasher.update(label.as_bytes());
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}
