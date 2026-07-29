//! Exact, name-independent land behavior used by mana and simulation paths.
//!
//! This module is deliberately narrower than the descriptive mana classifier.
//! Every accepted behavior either comes from an exact basic land subtype or
//! from one complete normalized Oracle paragraph. Nearby or mutated text does
//! not inherit execution.

use crate::ability_program::{
    AbilityCompilation, EXECUTABLE_ABILITY_PROGRAM_VERSION, ExecutableAbilityProgramV1,
};

pub(crate) const LAND_RUNTIME_CLASSIFIER_VERSION: &str = "exact-land-runtime/v1";
pub(crate) const BASIC_TYPE_MANA_EXECUTOR_ID: &str = "abstract-play.land.basic-type-mana";
pub(crate) const FIXED_MANA_EXECUTOR_ID: &str = "abstract-play.land.fixed-mana";
pub(crate) const ALWAYS_TAPPED_ENTRY_EXECUTOR_ID: &str = "abstract-play.land.entry.always-tapped";
pub(crate) const SHOCK_ENTRY_EXECUTOR_ID: &str = "abstract-play.land.entry.pay-two-life-or-tapped";
pub(crate) const MULTIPLAYER_ENTRY_EXECUTOR_ID: &str = "abstract-play.land.entry.two-opponents";
pub(crate) const FETCHLAND_EXECUTOR_ID: &str = "abstract-play.land.fetch-two-basic-land-types";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LandManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

impl LandManaColor {
    fn parse_symbol(value: &str) -> Option<Self> {
        match value {
            "{w}" => Some(Self::White),
            "{u}" => Some(Self::Blue),
            "{b}" => Some(Self::Black),
            "{r}" => Some(Self::Red),
            "{g}" => Some(Self::Green),
            "{c}" => Some(Self::Colorless),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BasicLandSubtype {
    Plains,
    Island,
    Swamp,
    Mountain,
    Forest,
}

impl BasicLandSubtype {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "plains" => Some(Self::Plains),
            "island" => Some(Self::Island),
            "swamp" => Some(Self::Swamp),
            "mountain" => Some(Self::Mountain),
            "forest" => Some(Self::Forest),
            _ => None,
        }
    }

    pub(crate) fn type_name(self) -> &'static str {
        match self {
            Self::Plains => "Plains",
            Self::Island => "Island",
            Self::Swamp => "Swamp",
            Self::Mountain => "Mountain",
            Self::Forest => "Forest",
        }
    }

    pub(crate) fn mana_color(self) -> LandManaColor {
        match self {
            Self::Plains => LandManaColor::White,
            Self::Island => LandManaColor::Blue,
            Self::Swamp => LandManaColor::Black,
            Self::Mountain => LandManaColor::Red,
            Self::Forest => LandManaColor::Green,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExactLandClauseEvidence {
    pub clause_index: u16,
    pub normalized_clause: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactBasicTypeMana {
    pub subtypes: Vec<BasicLandSubtype>,
    pub colors: Vec<LandManaColor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactFixedLandMana {
    /// Every accepted printed ability adds exactly one mana chosen from this
    /// union. Simultaneous multi-mana outputs remain unsupported.
    pub colors: Vec<LandManaColor>,
    pub clauses: Vec<ExactLandClauseEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExactLandEntry {
    UntappedByDefault,
    AlwaysTapped {
        clause: ExactLandClauseEvidence,
    },
    PayTwoLifeOrTapped {
        life: u8,
        clause: ExactLandClauseEvidence,
    },
    UntappedWithAtLeastOpponents {
        minimum_opponents: u8,
        clause: ExactLandClauseEvidence,
    },
}

impl ExactLandEntry {
    pub(crate) fn clause(&self) -> Option<&ExactLandClauseEvidence> {
        match self {
            Self::UntappedByDefault => None,
            Self::AlwaysTapped { clause }
            | Self::PayTwoLifeOrTapped { clause, .. }
            | Self::UntappedWithAtLeastOpponents { clause, .. } => Some(clause),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReviewedFetchland {
    pub first_subtype: BasicLandSubtype,
    pub second_subtype: BasicLandSubtype,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactFetchlandLifecycle {
    pub descriptor: ReviewedFetchland,
    pub clause: ExactLandClauseEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExactLandRuntimeSubject {
    BasicTypeMana {
        subtypes: Vec<BasicLandSubtype>,
        colors: Vec<LandManaColor>,
    },
    FixedPrintedMana {
        colors: Vec<LandManaColor>,
    },
    AlwaysTappedEntry,
    PayTwoLifeOrTappedEntry {
        life: u8,
    },
    UntappedWithAtLeastOpponents {
        minimum_opponents: u8,
    },
    FetchTwoBasicLandTypes {
        first_subtype: BasicLandSubtype,
        second_subtype: BasicLandSubtype,
    },
}

impl ExactLandRuntimeSubject {
    pub(crate) fn executor_id(&self) -> &'static str {
        match self {
            Self::BasicTypeMana { .. } => BASIC_TYPE_MANA_EXECUTOR_ID,
            Self::FixedPrintedMana { .. } => FIXED_MANA_EXECUTOR_ID,
            Self::AlwaysTappedEntry => ALWAYS_TAPPED_ENTRY_EXECUTOR_ID,
            Self::PayTwoLifeOrTappedEntry { .. } => SHOCK_ENTRY_EXECUTOR_ID,
            Self::UntappedWithAtLeastOpponents { .. } => MULTIPLAYER_ENTRY_EXECUTOR_ID,
            Self::FetchTwoBasicLandTypes { .. } => FETCHLAND_EXECUTOR_ID,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactLandRuntimeBindingInput {
    pub subject: ExactLandRuntimeSubject,
    /// Basic land type mana is derived from the exact type line and therefore
    /// has no Oracle clause. Every Oracle-derived behavior carries one or more
    /// occurrence-addressed clauses here.
    pub covered_oracle_clauses: Vec<ExactLandClauseEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactLandRuntimeProgram {
    pub normalized_type_line: String,
    pub basic_type_mana: Option<ExactBasicTypeMana>,
    pub fixed_mana: Option<ExactFixedLandMana>,
    pub entry: Option<ExactLandEntry>,
    pub fetchland: Option<ExactFetchlandLifecycle>,
    pub has_unsupported_mana_clause: bool,
    pub has_unsupported_entry_clause: bool,
}

impl ExactLandRuntimeProgram {
    pub(crate) fn exact_mana_colors(&self) -> Option<Vec<LandManaColor>> {
        if self.has_unsupported_mana_clause {
            return None;
        }
        let mut colors = self
            .basic_type_mana
            .iter()
            .flat_map(|mana| mana.colors.iter().copied())
            .chain(
                self.fixed_mana
                    .iter()
                    .flat_map(|mana| mana.colors.iter().copied()),
            )
            .collect::<Vec<_>>();
        colors.sort();
        colors.dedup();
        (!colors.is_empty()).then_some(colors)
    }

    pub(crate) fn has_exact_trajectory_source(&self) -> bool {
        self.exact_mana_colors().is_some()
            && self.entry.is_some()
            && !self.has_unsupported_entry_clause
            && self.fetchland.is_none()
    }

    pub(crate) fn binding_inputs(&self) -> Vec<ExactLandRuntimeBindingInput> {
        let mut bindings = Vec::new();
        if let Some(basic) = &self.basic_type_mana {
            bindings.push(ExactLandRuntimeBindingInput {
                subject: ExactLandRuntimeSubject::BasicTypeMana {
                    subtypes: basic.subtypes.clone(),
                    colors: basic.colors.clone(),
                },
                covered_oracle_clauses: Vec::new(),
            });
        }
        if let Some(fixed) = &self.fixed_mana {
            bindings.push(ExactLandRuntimeBindingInput {
                subject: ExactLandRuntimeSubject::FixedPrintedMana {
                    colors: fixed.colors.clone(),
                },
                covered_oracle_clauses: fixed.clauses.clone(),
            });
        }
        if let Some(entry) = &self.entry {
            let subject = match entry {
                ExactLandEntry::UntappedByDefault => None,
                ExactLandEntry::AlwaysTapped { .. } => {
                    Some(ExactLandRuntimeSubject::AlwaysTappedEntry)
                }
                ExactLandEntry::PayTwoLifeOrTapped { life, .. } => {
                    Some(ExactLandRuntimeSubject::PayTwoLifeOrTappedEntry { life: *life })
                }
                ExactLandEntry::UntappedWithAtLeastOpponents {
                    minimum_opponents, ..
                } => Some(ExactLandRuntimeSubject::UntappedWithAtLeastOpponents {
                    minimum_opponents: *minimum_opponents,
                }),
            };
            if let Some(subject) = subject {
                bindings.push(ExactLandRuntimeBindingInput {
                    subject,
                    covered_oracle_clauses: entry.clause().cloned().into_iter().collect(),
                });
            }
        }
        if let Some(fetchland) = &self.fetchland {
            bindings.push(ExactLandRuntimeBindingInput {
                subject: ExactLandRuntimeSubject::FetchTwoBasicLandTypes {
                    first_subtype: fetchland.descriptor.first_subtype,
                    second_subtype: fetchland.descriptor.second_subtype,
                },
                covered_oracle_clauses: vec![fetchland.clause.clone()],
            });
        }
        bindings
    }
}

pub(crate) fn classify_exact_land_program(
    type_line: &str,
    program: &ExecutableAbilityProgramV1,
) -> Option<ExactLandRuntimeProgram> {
    if program.version != EXECUTABLE_ABILITY_PROGRAM_VERSION
        || !program.face_programs.is_empty()
        || program.necropotence_lifecycle.is_some()
        || program.self_transfer_tutor_permanent.is_some()
        || program.entry_linked_permanent.is_some()
        || program.atomic_transaction.is_some()
        || program.graveyard_reclamation.is_some()
    {
        return None;
    }
    let exact_type = exact_land_type(type_line)?;
    let clauses = program
        .abilities
        .iter()
        .map(|ability| {
            let (clause_index, normalized_oracle) = match ability {
                AbilityCompilation::Executable(ability) => {
                    (ability.clause_index, ability.normalized_oracle.as_str())
                }
                AbilityCompilation::Unsupported(ability) => {
                    (ability.clause_index, ability.normalized_oracle.as_str())
                }
            };
            Some(ExactLandClauseEvidence {
                clause_index: u16::try_from(clause_index).ok()?,
                normalized_clause: normalized_oracle.trim().to_ascii_lowercase(),
            })
        })
        .collect::<Option<Vec<_>>>()?;

    let basic_type_mana = {
        let mut subtypes = exact_type
            .subtypes
            .iter()
            .filter_map(|subtype| BasicLandSubtype::parse(subtype))
            .collect::<Vec<_>>();
        subtypes.sort();
        subtypes.dedup();
        (!subtypes.is_empty()).then(|| ExactBasicTypeMana {
            colors: subtypes
                .iter()
                .copied()
                .map(BasicLandSubtype::mana_color)
                .collect(),
            subtypes,
        })
    };

    let mut fixed_colors = Vec::new();
    let mut fixed_clauses = Vec::new();
    let mut has_unsupported_mana_clause = false;
    for clause in &clauses {
        match parse_fixed_land_mana_clause(&clause.normalized_clause) {
            FixedManaClauseDisposition::NotMana => {}
            FixedManaClauseDisposition::Unsupported => has_unsupported_mana_clause = true,
            FixedManaClauseDisposition::Exact(colors) => {
                fixed_colors.extend(colors);
                fixed_clauses.push(clause.clone());
            }
        }
    }
    fixed_colors.sort();
    fixed_colors.dedup();
    fixed_clauses.sort();
    let fixed_mana = (!fixed_colors.is_empty()).then_some(ExactFixedLandMana {
        colors: fixed_colors,
        clauses: fixed_clauses,
    });

    let entry_candidates = clauses
        .iter()
        .filter(|clause| is_entry_tapped_candidate(&clause.normalized_clause))
        .collect::<Vec<_>>();
    let (entry, has_unsupported_entry_clause) = match entry_candidates.as_slice() {
        [] => (Some(ExactLandEntry::UntappedByDefault), false),
        [clause] => match parse_exact_land_entry_clause(clause) {
            Some(entry) => (Some(entry), false),
            None => (None, true),
        },
        _ => (None, true),
    };

    let fetchland = exact_fetchland_lifecycle(&clauses);
    Some(ExactLandRuntimeProgram {
        normalized_type_line: type_line.trim().to_ascii_lowercase(),
        basic_type_mana,
        fixed_mana,
        entry,
        fetchland,
        has_unsupported_mana_clause,
        has_unsupported_entry_clause,
    })
}

#[derive(Debug)]
struct ExactLandType {
    subtypes: Vec<String>,
}

fn exact_land_type(type_line: &str) -> Option<ExactLandType> {
    if type_line.contains(" // ") {
        return None;
    }
    let mut split = type_line.split('-');
    let types = split.next()?.trim();
    let subtype_segment = split.next().map(str::trim);
    if split.next().is_some() || subtype_segment.is_some_and(str::is_empty) {
        return None;
    }
    let mut is_land = false;
    for word in types.split_whitespace() {
        match word.to_ascii_lowercase().as_str() {
            "basic" | "legendary" | "snow" | "world" | "artifact" | "creature" | "enchantment" => {}
            "land" => is_land = true,
            _ => return None,
        }
    }
    if !is_land {
        return None;
    }
    let subtypes = match subtype_segment {
        Some(subtypes) => subtypes
            .split_whitespace()
            .map(|subtype| {
                subtype
                    .chars()
                    .all(|character| {
                        character.is_alphabetic() || character == '\'' || character == '-'
                    })
                    .then(|| subtype.to_ascii_lowercase())
            })
            .collect::<Option<Vec<_>>>()?,
        None => Vec::new(),
    };
    Some(ExactLandType { subtypes })
}

enum FixedManaClauseDisposition {
    NotMana,
    Unsupported,
    Exact(Vec<LandManaColor>),
}

fn parse_fixed_land_mana_clause(clause: &str) -> FixedManaClauseDisposition {
    let mut lower = clause.trim();
    if lower.starts_with('(') && lower.ends_with(')') {
        lower = &lower[1..lower.len() - 1];
    }
    let lower = lower.trim_end_matches('.');
    let Some(output) = lower.strip_prefix("{t}: add ") else {
        return if lower.contains("add ") {
            FixedManaClauseDisposition::Unsupported
        } else {
            FixedManaClauseDisposition::NotMana
        };
    };
    let symbols = if let Some((left, right)) = output.split_once(", or ") {
        let mut symbols = left.split(", ").collect::<Vec<_>>();
        if symbols.len() < 2 {
            return FixedManaClauseDisposition::Unsupported;
        }
        symbols.push(right);
        symbols
    } else if let Some((left, right)) = output.split_once(" or ") {
        vec![left, right]
    } else {
        vec![output]
    };
    let mut colors = symbols
        .into_iter()
        .map(LandManaColor::parse_symbol)
        .collect::<Option<Vec<_>>>();
    let Some(colors) = colors.as_mut() else {
        return FixedManaClauseDisposition::Unsupported;
    };
    colors.sort();
    if colors.is_empty() || colors.windows(2).any(|pair| pair[0] == pair[1]) {
        return FixedManaClauseDisposition::Unsupported;
    }
    FixedManaClauseDisposition::Exact(colors.clone())
}

fn is_entry_tapped_candidate(clause: &str) -> bool {
    let lower = clause.trim().to_ascii_lowercase();
    lower.contains(" enters tapped") || lower.contains(" enters the battlefield tapped")
}

fn parse_exact_land_entry_clause(clause: &ExactLandClauseEvidence) -> Option<ExactLandEntry> {
    let lower = clause
        .normalized_clause
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    match lower.as_str() {
        "this permanent enters tapped" | "this permanent enters the battlefield tapped" => {
            Some(ExactLandEntry::AlwaysTapped {
                clause: clause.clone(),
            })
        }
        "as this permanent enters, you may pay 2 life. if you don't, it enters tapped"
        | "as this permanent enters the battlefield, you may pay 2 life. if you don't, it enters the battlefield tapped"
        | "as this permanent enters the battlefield, you may pay 2 life. if you don't, it enters tapped" => {
            Some(ExactLandEntry::PayTwoLifeOrTapped {
                life: 2,
                clause: clause.clone(),
            })
        }
        "this permanent enters tapped unless you have two or more opponents"
        | "this permanent enters the battlefield tapped unless you have two or more opponents" => {
            Some(ExactLandEntry::UntappedWithAtLeastOpponents {
                minimum_opponents: 2,
                clause: clause.clone(),
            })
        }
        _ => None,
    }
}

fn exact_fetchland_lifecycle(
    clauses: &[ExactLandClauseEvidence],
) -> Option<ExactFetchlandLifecycle> {
    if clauses.len() != 1 {
        return None;
    }
    const PREFIX: &str = "{t}, pay 1 life, sacrifice this permanent: search your library for ";
    const SUFFIX: &str = " card, put it onto the battlefield, then shuffle";
    let clause = &clauses[0];
    let lower = clause
        .normalized_clause
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let subtypes_with_article = lower.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    let subtypes = subtypes_with_article
        .strip_prefix("a ")
        .or_else(|| subtypes_with_article.strip_prefix("an "))?;
    let (first, second) = subtypes.split_once(" or ")?;
    let descriptor = ReviewedFetchland {
        first_subtype: BasicLandSubtype::parse(first)?,
        second_subtype: BasicLandSubtype::parse(second)?,
    };
    (descriptor.first_subtype != descriptor.second_subtype).then(|| ExactFetchlandLifecycle {
        descriptor,
        clause: clause.clone(),
    })
}
