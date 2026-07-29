//! Exact, name-independent mana network programs.
//!
//! These classifiers retain complete structural roots for mana whose output
//! depends on commander configuration, other lands, continuous land grants,
//! or an entry-time land return. No card name selects behavior.

use crate::ability_program::{
    AbilityCompilation, EXECUTABLE_ABILITY_PROGRAM_VERSION, ExecutableAbilityProgramV1,
};

use crate::land_runtime::{
    BasicLandSubtype, ExactLandClauseEvidence, ExactLandEntry, LandManaColor,
};
use crate::mana::ManaColorMask;

pub(crate) const MANA_NETWORK_RUNTIME_VERSION: &str = "exact-mana-network/v2";
pub(crate) const COMMANDER_IDENTITY_MANA_EXECUTOR_ID: &str =
    "abstract-play.mana-network.commander-identity";
pub(crate) const CONTROLLED_LAND_CAPABILITY_MANA_EXECUTOR_ID: &str =
    "abstract-play.mana-network.controlled-land-capability";
pub(crate) const CONTROLLED_LAND_ANY_COLOR_GRANT_EXECUTOR_ID: &str =
    "abstract-play.mana-network.controlled-land-any-color-grant";
pub(crate) const GLOBAL_BASIC_LAND_SUBTYPE_GRANT_EXECUTOR_ID: &str =
    "abstract-play.mana-network.global-basic-land-subtype-grant";
pub(crate) const SELF_BOUNCE_DUAL_LAND_EXECUTOR_ID: &str =
    "abstract-play.mana-network.self-bounce-dual-land";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExactManaNetworkSourceType {
    Artifact,
    Land,
    LegendaryLand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ExactManaNetworkDependency {
    CommanderColorIdentityAtActivation,
    ControlledLandManaCapabilitiesAtActivation,
    ControlledLandsWhileSourceIsOnBattlefield,
    AllLandsWhileSourceIsOnBattlefield,
    ControlledLandChoiceAtEntryResolution { source_is_eligible: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ExactManaNetworkOutput {
    OneFromCommanderColorIdentity,
    OneFromControlledLandCapabilities,
    OneAnyColor,
    BasicSubtypeMana(BasicLandSubtype),
    CoupledFixed([LandManaColor; 2]),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactCommanderIdentityMana {
    pub source_type: ExactManaNetworkSourceType,
    pub activation_clause: ExactLandClauseEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactControlledLandCapabilityMana {
    pub source_type: ExactManaNetworkSourceType,
    pub activation_clause: ExactLandClauseEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactControlledLandAnyColorGrant {
    pub source_type: ExactManaNetworkSourceType,
    pub grant_clause: ExactLandClauseEvidence,
    pub source_activation_clause: ExactLandClauseEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactGlobalBasicLandSubtypeGrant {
    pub source_type: ExactManaNetworkSourceType,
    pub granted_subtype: BasicLandSubtype,
    pub granted_mana_color: LandManaColor,
    pub grant_clause: ExactLandClauseEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExactSelfBounceDualLand {
    pub source_type: ExactManaNetworkSourceType,
    pub entry: ExactLandEntry,
    pub return_clause: ExactLandClauseEvidence,
    pub activation_clause: ExactLandClauseEvidence,
    pub coupled_output: [LandManaColor; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExactManaNetworkProgram {
    CommanderIdentityMana(ExactCommanderIdentityMana),
    ControlledLandCapabilityMana(ExactControlledLandCapabilityMana),
    ControlledLandAnyColorGrant(ExactControlledLandAnyColorGrant),
    GlobalBasicLandSubtypeGrant(ExactGlobalBasicLandSubtypeGrant),
    SelfBounceDualLand(ExactSelfBounceDualLand),
}

#[allow(dead_code)]
impl ExactManaNetworkProgram {
    pub(crate) fn executor_id(&self) -> &'static str {
        match self {
            Self::CommanderIdentityMana(_) => COMMANDER_IDENTITY_MANA_EXECUTOR_ID,
            Self::ControlledLandCapabilityMana(_) => CONTROLLED_LAND_CAPABILITY_MANA_EXECUTOR_ID,
            Self::ControlledLandAnyColorGrant(_) => CONTROLLED_LAND_ANY_COLOR_GRANT_EXECUTOR_ID,
            Self::GlobalBasicLandSubtypeGrant(_) => GLOBAL_BASIC_LAND_SUBTYPE_GRANT_EXECUTOR_ID,
            Self::SelfBounceDualLand(_) => SELF_BOUNCE_DUAL_LAND_EXECUTOR_ID,
        }
    }

    pub(crate) fn source_type(&self) -> ExactManaNetworkSourceType {
        match self {
            Self::CommanderIdentityMana(program) => program.source_type,
            Self::ControlledLandCapabilityMana(program) => program.source_type,
            Self::ControlledLandAnyColorGrant(program) => program.source_type,
            Self::GlobalBasicLandSubtypeGrant(program) => program.source_type,
            Self::SelfBounceDualLand(program) => program.source_type,
        }
    }

    pub(crate) fn dependencies(&self) -> Vec<ExactManaNetworkDependency> {
        match self {
            Self::CommanderIdentityMana(_) => {
                vec![ExactManaNetworkDependency::CommanderColorIdentityAtActivation]
            }
            Self::ControlledLandCapabilityMana(_) => {
                vec![ExactManaNetworkDependency::ControlledLandManaCapabilitiesAtActivation]
            }
            Self::ControlledLandAnyColorGrant(_) => {
                vec![ExactManaNetworkDependency::ControlledLandsWhileSourceIsOnBattlefield]
            }
            Self::GlobalBasicLandSubtypeGrant(_) => {
                vec![ExactManaNetworkDependency::AllLandsWhileSourceIsOnBattlefield]
            }
            Self::SelfBounceDualLand(_) => vec![
                ExactManaNetworkDependency::ControlledLandChoiceAtEntryResolution {
                    source_is_eligible: true,
                },
            ],
        }
    }

    pub(crate) fn output(&self) -> ExactManaNetworkOutput {
        match self {
            Self::CommanderIdentityMana(_) => ExactManaNetworkOutput::OneFromCommanderColorIdentity,
            Self::ControlledLandCapabilityMana(_) => {
                ExactManaNetworkOutput::OneFromControlledLandCapabilities
            }
            Self::ControlledLandAnyColorGrant(_) => ExactManaNetworkOutput::OneAnyColor,
            Self::GlobalBasicLandSubtypeGrant(program) => {
                ExactManaNetworkOutput::BasicSubtypeMana(program.granted_subtype)
            }
            Self::SelfBounceDualLand(program) => {
                ExactManaNetworkOutput::CoupledFixed(program.coupled_output)
            }
        }
    }

    pub(crate) fn potential_mana_mask(&self) -> Option<ManaColorMask> {
        match self.output() {
            ExactManaNetworkOutput::OneFromCommanderColorIdentity
            | ExactManaNetworkOutput::OneFromControlledLandCapabilities => None,
            ExactManaNetworkOutput::OneAnyColor => Some(ManaColorMask::ANY_COLOR),
            ExactManaNetworkOutput::BasicSubtypeMana(subtype) => {
                Some(color_mask(subtype.mana_color()))
            }
            ExactManaNetworkOutput::CoupledFixed(colors) => {
                Some(color_mask(colors[0]) | color_mask(colors[1]))
            }
        }
    }

    pub(crate) fn covered_clauses(&self) -> Vec<&ExactLandClauseEvidence> {
        match self {
            Self::CommanderIdentityMana(program) => vec![&program.activation_clause],
            Self::ControlledLandCapabilityMana(program) => vec![&program.activation_clause],
            Self::ControlledLandAnyColorGrant(program) => {
                vec![&program.grant_clause, &program.source_activation_clause]
            }
            Self::GlobalBasicLandSubtypeGrant(program) => vec![&program.grant_clause],
            Self::SelfBounceDualLand(program) => vec![
                program
                    .entry
                    .clause()
                    .expect("an exact self-bounce land always has a tapped-entry clause"),
                &program.return_clause,
                &program.activation_clause,
            ],
        }
    }

    pub(crate) fn has_exact_contract(&self) -> bool {
        match self {
            Self::CommanderIdentityMana(program) => {
                matches!(
                    program.source_type,
                    ExactManaNetworkSourceType::Artifact | ExactManaNetworkSourceType::Land
                ) && exact_clause(
                    &program.activation_clause,
                    0,
                    "{t}: add one mana of any color in your commander's color identity.",
                )
            }
            Self::ControlledLandCapabilityMana(program) => {
                program.source_type == ExactManaNetworkSourceType::Land
                    && exact_clause(
                        &program.activation_clause,
                        0,
                        "{t}: add one mana of any type that a land you control could produce.",
                    )
            }
            Self::ControlledLandAnyColorGrant(program) => {
                program.source_type == ExactManaNetworkSourceType::Artifact
                    && exact_clause(
                        &program.grant_clause,
                        0,
                        "lands you control have \"{t}: add one mana of any color.\"",
                    )
                    && exact_clause(
                        &program.source_activation_clause,
                        1,
                        "{t}: add one mana of any color.",
                    )
            }
            Self::GlobalBasicLandSubtypeGrant(program) => {
                program.source_type == ExactManaNetworkSourceType::LegendaryLand
                    && program.granted_subtype == BasicLandSubtype::Swamp
                    && program.granted_mana_color == LandManaColor::Black
                    && exact_clause(
                        &program.grant_clause,
                        0,
                        "each land is a swamp in addition to its other land types.",
                    )
            }
            Self::SelfBounceDualLand(program) => {
                let Some(entry_clause) = program.entry.clause() else {
                    return false;
                };
                let parsed_output =
                    parse_exact_coupled_land_output(&program.activation_clause.normalized_clause);
                program.source_type == ExactManaNetworkSourceType::Land
                    && entry_clause.clause_index == 0
                    && matches!(
                        entry_clause.normalized_clause.as_str(),
                        "this permanent enters tapped."
                            | "this permanent enters the battlefield tapped."
                    )
                    && program.return_clause.clause_index == 1
                    && matches!(
                        program.return_clause.normalized_clause.as_str(),
                        "when this permanent enters, return a land you control to its owner's hand."
                            | "when this permanent enters the battlefield, return a land you control to its owner's hand."
                    )
                    && program.activation_clause.clause_index == 2
                    && parsed_output == Some(program.coupled_output)
            }
        }
    }
}

fn exact_clause(clause: &ExactLandClauseEvidence, index: u16, normalized: &str) -> bool {
    clause.clause_index == index && clause.normalized_clause == normalized
}

pub(crate) fn classify_exact_mana_network_program(
    type_line: &str,
    program: &ExecutableAbilityProgramV1,
) -> Option<ExactManaNetworkProgram> {
    if !has_exact_standalone_root(program) {
        return None;
    }
    let source_type = exact_source_type(type_line)?;
    let clauses = exact_program_clauses(program)?;
    match source_type {
        ExactManaNetworkSourceType::Artifact => {
            classify_commander_identity_mana(source_type, &clauses)
                .or_else(|| classify_controlled_land_any_color_grant(source_type, &clauses))
        }
        ExactManaNetworkSourceType::Land => classify_commander_identity_mana(source_type, &clauses)
            .or_else(|| classify_controlled_land_capability_mana(source_type, &clauses))
            .or_else(|| classify_self_bounce_dual_land(source_type, &clauses)),
        ExactManaNetworkSourceType::LegendaryLand => {
            classify_global_basic_land_subtype_grant(source_type, &clauses)
        }
    }
}

fn has_exact_standalone_root(program: &ExecutableAbilityProgramV1) -> bool {
    program.version == EXECUTABLE_ABILITY_PROGRAM_VERSION
        && program.necropotence_lifecycle.is_none()
        && program.self_transfer_tutor_permanent.is_none()
        && program.entry_linked_permanent.is_none()
        && program.atomic_transaction.is_none()
        && program.graveyard_reclamation.is_none()
        && program.face_programs.is_empty()
}

fn exact_source_type(type_line: &str) -> Option<ExactManaNetworkSourceType> {
    let normalized = type_line
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    match normalized.as_str() {
        "artifact" => Some(ExactManaNetworkSourceType::Artifact),
        "land" => Some(ExactManaNetworkSourceType::Land),
        "legendary land" => Some(ExactManaNetworkSourceType::LegendaryLand),
        _ => None,
    }
}

fn exact_program_clauses(
    program: &ExecutableAbilityProgramV1,
) -> Option<Vec<ExactLandClauseEvidence>> {
    if program.abilities.is_empty() {
        return None;
    }
    let mut clauses = program
        .abilities
        .iter()
        .map(|compilation| {
            let (clause_index, normalized_oracle) = match compilation {
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
    clauses.sort();
    clauses
        .iter()
        .enumerate()
        .all(|(expected, clause)| usize::from(clause.clause_index) == expected)
        .then_some(clauses)
}

fn classify_commander_identity_mana(
    source_type: ExactManaNetworkSourceType,
    clauses: &[ExactLandClauseEvidence],
) -> Option<ExactManaNetworkProgram> {
    let [activation_clause] = clauses else {
        return None;
    };
    (activation_clause.normalized_clause
        == "{t}: add one mana of any color in your commander's color identity.")
        .then(|| {
            ExactManaNetworkProgram::CommanderIdentityMana(ExactCommanderIdentityMana {
                source_type,
                activation_clause: activation_clause.clone(),
            })
        })
}

fn classify_controlled_land_capability_mana(
    source_type: ExactManaNetworkSourceType,
    clauses: &[ExactLandClauseEvidence],
) -> Option<ExactManaNetworkProgram> {
    let [activation_clause] = clauses else {
        return None;
    };
    (activation_clause.normalized_clause
        == "{t}: add one mana of any type that a land you control could produce.")
        .then(|| {
            ExactManaNetworkProgram::ControlledLandCapabilityMana(
                ExactControlledLandCapabilityMana {
                    source_type,
                    activation_clause: activation_clause.clone(),
                },
            )
        })
}

fn classify_controlled_land_any_color_grant(
    source_type: ExactManaNetworkSourceType,
    clauses: &[ExactLandClauseEvidence],
) -> Option<ExactManaNetworkProgram> {
    let [grant_clause, source_activation_clause] = clauses else {
        return None;
    };
    (grant_clause.normalized_clause == "lands you control have \"{t}: add one mana of any color.\""
        && source_activation_clause.normalized_clause == "{t}: add one mana of any color.")
        .then(|| {
            ExactManaNetworkProgram::ControlledLandAnyColorGrant(ExactControlledLandAnyColorGrant {
                source_type,
                grant_clause: grant_clause.clone(),
                source_activation_clause: source_activation_clause.clone(),
            })
        })
}

fn classify_global_basic_land_subtype_grant(
    source_type: ExactManaNetworkSourceType,
    clauses: &[ExactLandClauseEvidence],
) -> Option<ExactManaNetworkProgram> {
    let [grant_clause] = clauses else {
        return None;
    };
    (grant_clause.normalized_clause == "each land is a swamp in addition to its other land types.")
        .then(|| {
            ExactManaNetworkProgram::GlobalBasicLandSubtypeGrant(ExactGlobalBasicLandSubtypeGrant {
                source_type,
                granted_subtype: BasicLandSubtype::Swamp,
                granted_mana_color: LandManaColor::Black,
                grant_clause: grant_clause.clone(),
            })
        })
}

fn classify_self_bounce_dual_land(
    source_type: ExactManaNetworkSourceType,
    clauses: &[ExactLandClauseEvidence],
) -> Option<ExactManaNetworkProgram> {
    let [entry_clause, return_clause, activation_clause] = clauses else {
        return None;
    };
    let exact_entry = matches!(
        entry_clause.normalized_clause.as_str(),
        "this permanent enters tapped." | "this permanent enters the battlefield tapped."
    );
    let exact_return = matches!(
        return_clause.normalized_clause.as_str(),
        "when this permanent enters, return a land you control to its owner's hand."
            | "when this permanent enters the battlefield, return a land you control to its owner's hand."
    );
    let coupled_output = parse_exact_coupled_land_output(&activation_clause.normalized_clause)?;
    (exact_entry && exact_return).then(|| {
        ExactManaNetworkProgram::SelfBounceDualLand(ExactSelfBounceDualLand {
            source_type,
            entry: ExactLandEntry::AlwaysTapped {
                clause: entry_clause.clone(),
            },
            return_clause: return_clause.clone(),
            activation_clause: activation_clause.clone(),
            coupled_output,
        })
    })
}

fn parse_exact_coupled_land_output(clause: &str) -> Option<[LandManaColor; 2]> {
    let output = clause
        .strip_prefix("{t}: add ")?
        .strip_suffix('.')?
        .as_bytes();
    if output.len() != 6
        || output[0] != b'{'
        || output[2] != b'}'
        || output[3] != b'{'
        || output[5] != b'}'
    {
        return None;
    }
    let first = parse_colored_symbol(output[1])?;
    let second = parse_colored_symbol(output[4])?;
    (first != second).then_some([first, second])
}

fn parse_colored_symbol(symbol: u8) -> Option<LandManaColor> {
    match symbol {
        b'w' => Some(LandManaColor::White),
        b'u' => Some(LandManaColor::Blue),
        b'b' => Some(LandManaColor::Black),
        b'r' => Some(LandManaColor::Red),
        b'g' => Some(LandManaColor::Green),
        _ => None,
    }
}

#[allow(dead_code)]
fn color_mask(color: LandManaColor) -> ManaColorMask {
    match color {
        LandManaColor::White => ManaColorMask::WHITE,
        LandManaColor::Blue => ManaColorMask::BLUE,
        LandManaColor::Black => ManaColorMask::BLACK,
        LandManaColor::Red => ManaColorMask::RED,
        LandManaColor::Green => ManaColorMask::GREEN,
        LandManaColor::Colorless => ManaColorMask::COLORLESS,
    }
}
